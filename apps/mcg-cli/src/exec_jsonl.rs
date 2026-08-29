use std::collections::{BTreeMap, BTreeSet};

use mcg_domain::{
    ConversationActivity, ConversationActivityKind, ConversationActivityState, ConversationMessage,
    ConversationRole, ConversationTurnId, ConversationView,
};
use serde::Serialize;
use serde_json::{Value, json};

/// Codex-compatible top-level events emitted by `mcg exec --json`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum ExecJsonlEvent {
    #[serde(rename = "thread.started")]
    ThreadStarted { thread_id: String },
    #[serde(rename = "turn.started")]
    TurnStarted {},
    #[serde(rename = "turn.completed")]
    TurnCompleted { usage: ExecUsage },
    #[serde(rename = "turn.failed")]
    TurnFailed { error: ExecError },
    #[serde(rename = "item.started")]
    ItemStarted { item: Value },
    #[serde(rename = "item.updated")]
    ItemUpdated { item: Value },
    #[serde(rename = "item.completed")]
    ItemCompleted { item: Value },
    #[serde(rename = "error")]
    Error { message: String },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ExecUsage {
    pub input_tokens: i64,
    pub cached_input_tokens: i64,
    #[serde(default)]
    pub cache_write_input_tokens: i64,
    pub output_tokens: i64,
    pub reasoning_output_tokens: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExecError {
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SeenActivity {
    state: ConversationActivityState,
    fingerprint: u64,
}

/// Converts durable MetaCog conversation projections into the public Codex
/// `exec --json` contract without exposing internal governance receipts.
pub struct ExecJsonlProjector {
    turn_id: ConversationTurnId,
    seen_activities: BTreeMap<String, SeenActivity>,
    seen_messages: BTreeSet<String>,
    usage: ExecUsage,
}

impl ExecJsonlProjector {
    pub fn new(turn_id: ConversationTurnId) -> Self {
        Self {
            turn_id,
            seen_activities: BTreeMap::new(),
            seen_messages: BTreeSet::new(),
            usage: ExecUsage::default(),
        }
    }

    pub fn project_view(&mut self, view: &ConversationView) -> Vec<ExecJsonlEvent> {
        let mut events = Vec::new();
        let turn_id = self.turn_id;
        for activity in view
            .activities
            .iter()
            .filter(|activity| activity.turn_id == turn_id)
        {
            events.extend(self.project_activity(activity));
        }

        for message in view.messages.iter().filter(|message| {
            message.turn_id == turn_id && message.role == ConversationRole::Assistant
        }) {
            events.extend(self.project_message(message));
        }
        events
    }

    pub fn project_activity(&mut self, activity: &ConversationActivity) -> Vec<ExecJsonlEvent> {
        if activity.turn_id != self.turn_id {
            return Vec::new();
        }
        if activity.kind == ConversationActivityKind::Usage {
            self.usage = usage_from_activity(activity).unwrap_or_else(|| self.usage.clone());
            return Vec::new();
        }
        if activity.codex_item_id.starts_with("notification:")
            && !matches!(
                activity.kind,
                ConversationActivityKind::Plan | ConversationActivityKind::Failure
            )
            || matches!(
                activity.kind,
                ConversationActivityKind::Completion
                    | ConversationActivityKind::Interruption
                    | ConversationActivityKind::Working
                    | ConversationActivityKind::Waiting
                    | ConversationActivityKind::Approval
                    | ConversationActivityKind::Elicitation
                    | ConversationActivityKind::ContextCompaction
                    | ConversationActivityKind::Recovery
                    | ConversationActivityKind::ImageView
            )
        {
            return Vec::new();
        }
        let fingerprint = activity_fingerprint(activity);
        let previous = self.seen_activities.get(&activity.codex_item_id).copied();
        if previous
            .is_some_and(|seen| seen.state == activity.state && seen.fingerprint == fingerprint)
        {
            return Vec::new();
        }
        if activity.kind == ConversationActivityKind::Failure {
            let message = activity
                .output
                .as_deref()
                .or(activity.command.as_deref())
                .unwrap_or("Codex reported an error")
                .to_owned();
            self.remember(activity, fingerprint);
            return vec![ExecJsonlEvent::Error { message }];
        }
        let Some(item) = activity_item(activity) else {
            self.remember(activity, fingerprint);
            return Vec::new();
        };
        let event = match (activity.kind, previous, activity.state) {
            (ConversationActivityKind::Plan, None, ConversationActivityState::Running) => {
                ExecJsonlEvent::ItemStarted { item }
            }
            (ConversationActivityKind::Plan, Some(_), ConversationActivityState::Running) => {
                ExecJsonlEvent::ItemUpdated { item }
            }
            (_, _, ConversationActivityState::Running) if previous.is_none() => {
                ExecJsonlEvent::ItemStarted { item }
            }
            (_, _, ConversationActivityState::Running) => {
                self.remember(activity, fingerprint);
                return Vec::new();
            }
            _ => ExecJsonlEvent::ItemCompleted { item },
        };
        self.remember(activity, fingerprint);
        vec![event]
    }

    pub fn project_message(&mut self, message: &ConversationMessage) -> Vec<ExecJsonlEvent> {
        if message.turn_id != self.turn_id
            || message.role != ConversationRole::Assistant
            || !self.seen_messages.insert(message.id.to_string())
        {
            return Vec::new();
        }
        vec![ExecJsonlEvent::ItemCompleted {
            item: json!({
                "id": message.codex_item_id.clone().unwrap_or_else(|| message.id.to_string()),
                "type": "agent_message",
                "text": message.content,
            }),
        }]
    }

    pub fn completed(&self) -> ExecJsonlEvent {
        ExecJsonlEvent::TurnCompleted {
            usage: self.usage.clone(),
        }
    }

    pub fn finalize_items(&mut self, view: &ConversationView) -> Vec<ExecJsonlEvent> {
        let turn_id = self.turn_id;
        let pending = view
            .activities
            .iter()
            .filter(|activity| {
                activity.turn_id == turn_id
                    && activity.state == ConversationActivityState::Running
                    && self
                        .seen_activities
                        .get(&activity.codex_item_id)
                        .is_some_and(|seen| seen.state == ConversationActivityState::Running)
            })
            .cloned()
            .collect::<Vec<_>>();
        pending
            .into_iter()
            .filter_map(|mut activity| {
                activity.state = ConversationActivityState::Completed;
                let item = activity_item(&activity)?;
                self.remember(&activity, activity_fingerprint(&activity));
                Some(ExecJsonlEvent::ItemCompleted { item })
            })
            .collect()
    }

    pub fn failed(message: impl Into<String>) -> ExecJsonlEvent {
        ExecJsonlEvent::TurnFailed {
            error: ExecError {
                message: message.into(),
            },
        }
    }

    fn remember(&mut self, activity: &ConversationActivity, fingerprint: u64) {
        self.seen_activities.insert(
            activity.codex_item_id.clone(),
            SeenActivity {
                state: activity.state,
                fingerprint,
            },
        );
    }
}

fn activity_item(activity: &ConversationActivity) -> Option<Value> {
    let payload = activity.codex_payload.as_ref();
    let id = activity.codex_item_id.clone();
    let status = activity_status(activity.state);
    match activity.kind {
        ConversationActivityKind::ReasoningSummary => Some(json!({
            "id": id,
            "type": "reasoning",
            "text": activity.command.as_deref().or(activity.output.as_deref()).unwrap_or_default(),
        })),
        ConversationActivityKind::Command => Some(json!({
            "id": id,
            "type": "command_execution",
            "command": payload_string(payload, "command").or_else(|| activity.command.clone()).unwrap_or_default(),
            "aggregated_output": payload_string(payload, "aggregatedOutput").or_else(|| activity.output.clone()).unwrap_or_default(),
            "exit_code": payload_i64(payload, "exitCode").or(activity.exit_code),
            "status": status,
        })),
        ConversationActivityKind::FileChange => Some(json!({
            "id": id,
            "type": "file_change",
            "changes": file_changes(payload, activity.output.as_deref()),
            "status": if status == "declined" { "failed" } else { status },
        })),
        ConversationActivityKind::McpCall => Some(json!({
            "id": id,
            "type": "mcp_tool_call",
            "server": payload_string(payload, "server").or_else(|| payload_string(payload, "serverName")).unwrap_or_default(),
            "tool": payload_string(payload, "tool").or_else(|| payload_string(payload, "toolName")).or_else(|| activity.command.clone()).unwrap_or_default(),
            "arguments": payload_value(payload, "arguments").unwrap_or_else(|| json!({})),
            "result": mcp_result(payload, activity.output.as_deref()),
            "error": mcp_error(payload),
            "status": if status == "declined" { "failed" } else { status },
        })),
        ConversationActivityKind::CollaborationCall => Some(json!({
            "id": id,
            "type": "collab_tool_call",
            "tool": collab_tool(payload_string(payload, "tool").as_deref()),
            "sender_thread_id": payload_string(payload, "senderThreadId").unwrap_or_default(),
            "receiver_thread_ids": payload_value(payload, "receiverThreadIds").unwrap_or_else(|| json!([])),
            "prompt": payload_value(payload, "prompt"),
            "agents_states": collab_agent_states(payload),
            "status": if status == "declined" { "failed" } else { status },
        })),
        ConversationActivityKind::WebSearch => Some(json!({
            "id": id,
            "type": "web_search",
            "query": payload_string(payload, "query").or_else(|| activity.command.clone()).unwrap_or_default(),
            "action": payload_value(payload, "action").unwrap_or_else(|| json!({"type": "other"})),
        })),
        ConversationActivityKind::Plan => Some(json!({
            "id": id,
            "type": "todo_list",
            "items": todo_items(payload, activity.output.as_deref()),
        })),
        _ => None,
    }
}

fn usage_from_activity(activity: &ConversationActivity) -> Option<ExecUsage> {
    let payload = activity
        .codex_payload
        .clone()
        .or_else(|| parse_json(activity.output.as_deref()))?;
    let total = payload
        .pointer("/tokenUsage/total")
        .or_else(|| payload.pointer("/token_usage/total"))
        .unwrap_or(&payload);
    Some(ExecUsage {
        input_tokens: json_i64(total, "inputTokens", "input_tokens"),
        cached_input_tokens: json_i64(total, "cachedInputTokens", "cached_input_tokens"),
        cache_write_input_tokens: json_i64(
            total,
            "cacheWriteInputTokens",
            "cache_write_input_tokens",
        ),
        output_tokens: json_i64(total, "outputTokens", "output_tokens"),
        reasoning_output_tokens: json_i64(
            total,
            "reasoningOutputTokens",
            "reasoning_output_tokens",
        ),
    })
}

fn json_i64(value: &Value, camel: &str, snake: &str) -> i64 {
    value
        .get(camel)
        .or_else(|| value.get(snake))
        .and_then(Value::as_i64)
        .unwrap_or_default()
}

fn activity_status(state: ConversationActivityState) -> &'static str {
    match state {
        ConversationActivityState::Running => "in_progress",
        ConversationActivityState::Completed => "completed",
        ConversationActivityState::Failed => "failed",
        ConversationActivityState::Declined => "declined",
    }
}

fn payload_string(payload: Option<&Value>, key: &str) -> Option<String> {
    payload?.get(key)?.as_str().map(str::to_owned)
}

fn payload_i64(payload: Option<&Value>, key: &str) -> Option<i64> {
    payload?.get(key)?.as_i64()
}

fn payload_value(payload: Option<&Value>, key: &str) -> Option<Value> {
    payload?.get(key).cloned()
}

fn parse_json(value: Option<&str>) -> Option<Value> {
    serde_json::from_str(value?).ok()
}

fn file_changes(payload: Option<&Value>, fallback: Option<&str>) -> Vec<Value> {
    payload
        .and_then(|value| value.get("changes"))
        .cloned()
        .or_else(|| parse_json(fallback))
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|change| {
            let path = change.get("path")?.as_str()?;
            let kind = change
                .get("kind")
                .and_then(|kind| kind.as_str().or_else(|| kind.get("type")?.as_str()))
                .unwrap_or("update")
                .to_ascii_lowercase();
            Some(json!({"path": path, "kind": kind}))
        })
        .collect()
}

fn todo_items(payload: Option<&Value>, fallback: Option<&str>) -> Vec<Value> {
    let plan = payload
        .and_then(|value| value.get("plan"))
        .cloned()
        .or_else(|| parse_json(fallback));
    plan.and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|step| {
            let text = step.get("step").or_else(|| step.get("text"))?.as_str()?;
            let completed = step
                .get("completed")
                .and_then(Value::as_bool)
                .unwrap_or_else(|| step.get("status").and_then(Value::as_str) == Some("completed"));
            Some(json!({"text": text, "completed": completed}))
        })
        .collect()
}

fn collab_tool(tool: Option<&str>) -> &'static str {
    match tool.unwrap_or_default() {
        "spawnAgent" | "spawn_agent" => "spawn_agent",
        "sendInput" | "send_input" => "send_input",
        "wait" | "resumeAgent" | "resume_agent" => "wait",
        "closeAgent" | "close_agent" => "close_agent",
        _ => "send_input",
    }
}

fn mcp_result(payload: Option<&Value>, fallback: Option<&str>) -> Option<Value> {
    let result = payload_value(payload, "result").or_else(|| parse_json(fallback))?;
    let mut mapped = serde_json::Map::from_iter([
        (
            "content".to_owned(),
            result.get("content").cloned().unwrap_or_else(|| json!([])),
        ),
        (
            "structured_content".to_owned(),
            result
                .get("structuredContent")
                .cloned()
                .or_else(|| result.get("structured_content").cloned())
                .unwrap_or(Value::Null),
        ),
    ]);
    if let Some(meta) = result
        .get("_meta")
        .cloned()
        .or_else(|| result.get("meta").cloned())
    {
        mapped.insert("_meta".to_owned(), meta);
    }
    Some(Value::Object(mapped))
}

fn mcp_error(payload: Option<&Value>) -> Option<Value> {
    let error = payload_value(payload, "error")?;
    if error.is_object() {
        Some(error)
    } else {
        Some(json!({"message": error.as_str().unwrap_or("MCP tool call failed")}))
    }
}

fn collab_agent_states(payload: Option<&Value>) -> Value {
    let Some(states) = payload
        .and_then(|payload| payload.get("agentsStates"))
        .and_then(Value::as_object)
    else {
        return json!({});
    };
    Value::Object(
        states
            .iter()
            .map(|(thread_id, state)| {
                let status = match state
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                {
                    "pendingInit" | "pending_init" => "pending_init",
                    "running" => "running",
                    "interrupted" => "interrupted",
                    "completed" => "completed",
                    "errored" => "errored",
                    "shutdown" => "shutdown",
                    "notFound" | "not_found" => "not_found",
                    _ => "not_found",
                };
                (
                    thread_id.clone(),
                    json!({
                        "status": status,
                        "message": state.get("message").cloned().unwrap_or(Value::Null),
                    }),
                )
            })
            .collect(),
    )
}

fn activity_fingerprint(activity: &ConversationActivity) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    activity.command.hash(&mut hasher);
    activity.output.hash(&mut hasher);
    activity.exit_code.hash(&mut hasher);
    activity.duration_ms.hash(&mut hasher);
    activity
        .codex_payload
        .as_ref()
        .map(Value::to_string)
        .hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use mcg_domain::{
        ApprovalMode, CONVERSATION_SCHEMA_VERSION, Conversation, ConversationActivity,
        ConversationMessage, ConversationMessageId, ConversationState, ConversationTurn,
        ExecutionPreferences,
    };

    use super::*;

    fn view_with(turn_id: ConversationTurnId) -> ConversationView {
        let mut conversation = Conversation::new("exec", None);
        conversation.state = ConversationState::Active;
        ConversationView {
            conversation,
            turns: vec![ConversationTurn {
                schema_version: CONVERSATION_SCHEMA_VERSION,
                id: turn_id,
                conversation_id: Conversation::new("unused", None).id,
                state: mcg_domain::ConversationTurnState::Streaming,
                execution_preferences: ExecutionPreferences {
                    approval_mode: ApprovalMode::Ask,
                    ..Default::default()
                },
                codex_thread_id: None,
                codex_turn_id: None,
                task_id: None,
                run_id: None,
                error: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }],
            messages: Vec::new(),
            activities: Vec::new(),
            team: Default::default(),
        }
    }

    #[test]
    fn command_lifecycle_and_agent_message_match_codex_shape() {
        let turn_id = ConversationTurnId::new();
        let mut view = view_with(turn_id);
        let conversation_id = view.conversation.id;
        view.activities.push(ConversationActivity {
            schema_version: CONVERSATION_SCHEMA_VERSION,
            conversation_id,
            turn_id,
            agent_session_id: None,
            delegation_id: None,
            codex_item_id: "command-1".into(),
            kind: ConversationActivityKind::Command,
            state: ConversationActivityState::Running,
            command: Some("cargo test".into()),
            cwd: None,
            output: None,
            exit_code: None,
            duration_ms: None,
            codex_payload: Some(json!({
                "id": "command-1",
                "type": "commandExecution",
                "command": "cargo test",
                "status": "inProgress"
            })),
            started_at: Utc::now(),
            updated_at: Utc::now(),
        });
        let mut projector = ExecJsonlProjector::new(turn_id);
        assert_eq!(
            serde_json::to_value(&projector.project_view(&view)[0]).unwrap(),
            json!({
                "type": "item.started",
                "item": {
                    "id": "command-1",
                    "type": "command_execution",
                    "command": "cargo test",
                    "aggregated_output": "",
                    "exit_code": null,
                    "status": "in_progress"
                }
            })
        );

        view.activities[0].state = ConversationActivityState::Completed;
        view.activities[0].output = Some("ok".into());
        view.activities[0].exit_code = Some(0);
        view.messages.push(ConversationMessage {
            schema_version: CONVERSATION_SCHEMA_VERSION,
            id: ConversationMessageId::new(),
            conversation_id,
            turn_id,
            role: ConversationRole::Assistant,
            content: "done".into(),
            mentions: Vec::new(),
            codex_item_id: Some("message-1".into()),
            created_at: Utc::now(),
        });
        let events = projector.project_view(&view);
        assert_eq!(events.len(), 2);
        assert_eq!(
            serde_json::to_value(&events[1]).unwrap(),
            json!({
                "type": "item.completed",
                "item": {"id": "message-1", "type": "agent_message", "text": "done"}
            })
        );
        assert!(projector.project_view(&view).is_empty());
    }

    #[test]
    fn usage_and_failure_envelopes_match_codex_shape() {
        let turn_id = ConversationTurnId::new();
        let mut view = view_with(turn_id);
        view.activities.push(ConversationActivity {
            schema_version: CONVERSATION_SCHEMA_VERSION,
            conversation_id: view.conversation.id,
            turn_id,
            agent_session_id: None,
            delegation_id: None,
            codex_item_id: "notification:usage".into(),
            kind: ConversationActivityKind::Usage,
            state: ConversationActivityState::Completed,
            command: None,
            cwd: None,
            output: None,
            exit_code: None,
            duration_ms: None,
            codex_payload: Some(json!({"tokenUsage":{"total":{
                "inputTokens":10,
                "cachedInputTokens":3,
                "cacheWriteInputTokens":4,
                "outputTokens":7,
                "reasoningOutputTokens":2
            }}})),
            started_at: Utc::now(),
            updated_at: Utc::now(),
        });
        let mut projector = ExecJsonlProjector::new(turn_id);
        assert!(projector.project_view(&view).is_empty());
        assert_eq!(
            serde_json::to_value(projector.completed()).unwrap(),
            json!({"type":"turn.completed","usage":{
                "input_tokens":10,
                "cached_input_tokens":3,
                "cache_write_input_tokens":4,
                "output_tokens":7,
                "reasoning_output_tokens":2
            }})
        );
        assert_eq!(
            serde_json::to_value(ExecJsonlProjector::failed("boom")).unwrap(),
            json!({"type":"turn.failed","error":{"message":"boom"}})
        );
    }

    #[test]
    fn plan_updates_and_finalization_match_codex_todo_lifecycle() {
        let turn_id = ConversationTurnId::new();
        let mut view = view_with(turn_id);
        view.activities.push(ConversationActivity {
            schema_version: CONVERSATION_SCHEMA_VERSION,
            conversation_id: view.conversation.id,
            turn_id,
            agent_session_id: None,
            delegation_id: None,
            codex_item_id: "notification:plan".into(),
            kind: ConversationActivityKind::Plan,
            state: ConversationActivityState::Running,
            command: Some("Plan updated".into()),
            cwd: None,
            output: None,
            exit_code: None,
            duration_ms: None,
            codex_payload: Some(json!({"plan":[
                {"step":"inspect", "status":"completed"},
                {"step":"fix", "status":"inProgress"}
            ]})),
            started_at: Utc::now(),
            updated_at: Utc::now(),
        });
        let mut projector = ExecJsonlProjector::new(turn_id);
        assert_eq!(
            serde_json::to_value(&projector.project_view(&view)[0]).unwrap(),
            json!({"type":"item.started","item":{
                "id":"notification:plan",
                "type":"todo_list",
                "items":[
                    {"text":"inspect","completed":true},
                    {"text":"fix","completed":false}
                ]
            }})
        );
        view.activities[0].codex_payload = Some(json!({"plan":[
            {"step":"inspect", "status":"completed"},
            {"step":"fix", "status":"completed"}
        ]}));
        assert!(matches!(
            projector.project_view(&view).as_slice(),
            [ExecJsonlEvent::ItemUpdated { .. }]
        ));
        assert!(matches!(
            projector.finalize_items(&view).as_slice(),
            [ExecJsonlEvent::ItemCompleted { .. }]
        ));
    }

    #[test]
    fn typed_file_mcp_collaboration_and_web_items_preserve_payloads() {
        let turn_id = ConversationTurnId::new();
        let mut view = view_with(turn_id);
        let now = Utc::now();
        let conversation_id = view.conversation.id;
        let base = |id: &str, kind, payload: Value| ConversationActivity {
            schema_version: CONVERSATION_SCHEMA_VERSION,
            conversation_id,
            turn_id,
            agent_session_id: None,
            delegation_id: None,
            codex_item_id: id.into(),
            kind,
            state: ConversationActivityState::Completed,
            command: None,
            cwd: None,
            output: None,
            exit_code: None,
            duration_ms: None,
            codex_payload: Some(payload),
            started_at: now,
            updated_at: now,
        };
        view.activities.extend([
            base(
                "file-1",
                ConversationActivityKind::FileChange,
                json!({"changes":[{"path":"src/lib.rs","kind":"update"}]}),
            ),
            base(
                "mcp-1",
                ConversationActivityKind::McpCall,
                json!({
                    "server":"docs",
                    "tool":"search",
                    "arguments":{"q":"jsonl"},
                    "result":{"content":[],"structuredContent":{"hits":1}},
                    "status":"completed"
                }),
            ),
            base(
                "collab-1",
                ConversationActivityKind::CollaborationCall,
                json!({
                    "tool":"spawnAgent",
                    "senderThreadId":"root",
                    "receiverThreadIds":["root/a"],
                    "prompt":"audit",
                    "agentsStates":{"root/a":{"status":"pendingInit","message":null}}
                }),
            ),
            base(
                "web-1",
                ConversationActivityKind::WebSearch,
                json!({"query":"Codex JSONL","action":{"type":"search","query":"Codex JSONL"}}),
            ),
        ]);
        let mut projector = ExecJsonlProjector::new(turn_id);
        let values = projector
            .project_view(&view)
            .iter()
            .map(|event| serde_json::to_value(event).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(values.len(), 4);
        assert_eq!(values[0]["item"]["type"], "file_change");
        assert_eq!(values[0]["item"]["changes"][0]["kind"], "update");
        assert_eq!(values[1]["item"]["type"], "mcp_tool_call");
        assert_eq!(values[1]["item"]["result"]["structured_content"]["hits"], 1);
        assert_eq!(values[2]["item"]["type"], "collab_tool_call");
        assert_eq!(values[2]["item"]["tool"], "spawn_agent");
        assert_eq!(
            values[2]["item"]["agents_states"]["root/a"]["status"],
            "pending_init"
        );
        assert_eq!(values[3]["item"]["type"], "web_search");
    }
}
