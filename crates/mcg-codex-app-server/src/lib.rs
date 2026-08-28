//! Stable JSONL/stdio client for the pinned Codex app-server runtime.
//!
//! This is deliberately a conversation/thread harness, not a `ModelProvider`.

use std::{
    collections::{BTreeMap, BTreeSet},
    io::{BufRead as _, Read as _},
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::Duration,
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, Command},
    sync::{Mutex, broadcast, mpsc, oneshot},
};

mod protocol_generated;

pub use protocol_generated::{
    GENERATED_SCHEMA_SHA256 as CODEX_SCHEMA_SHA256, RequestId, ServerNotification, ServerRequest,
    ServerResponse,
};

pub const CODEX_BINARY_VERSION: &str = protocol_generated::GENERATED_CODEX_VERSION;
pub const CODEX_PROTOCOL_VERSION: &str = protocol_generated::GENERATED_PROTOCOL_VERSION;
const WINDOWS_SANDBOX_READINESS_METHOD: &str = "windowsSandbox/readiness";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodexTurnProgress {
    Started { turn_id: String },
    TextDelta { item_id: String, delta: String },
    ItemStarted { item: Value },
    ItemCompleted { item: Value },
    CommandOutputDelta { item_id: String, delta: String },
    ApprovalRequested(CodexApprovalRequest),
    UserInputRequested(CodexPendingRequest),
    Notification { method: String, params: Value },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexApprovalRequest {
    pub request_id: Value,
    pub method: String,
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub reason: Option<String>,
    pub resource: String,
    pub params: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexPendingRequest {
    pub request_id: Value,
    pub method: String,
    pub params: Value,
    pub supported: bool,
    pub received_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexConfigurationStatus {
    pub requirements: Value,
    pub provider_capabilities: Value,
    pub windows_sandbox: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexStableDiscovery {
    pub skills: Vec<Value>,
    pub hooks: Vec<Value>,
    #[serde(default)]
    pub apps: Vec<Value>,
    pub apps_available: bool,
    pub apps_unavailable_reason: Option<String>,
    pub refreshed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodexFeatureStage {
    Beta,
    UnderDevelopment,
    Stable,
    Deprecated,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexFeature {
    pub name: String,
    pub stage: CodexFeatureStage,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub announcement: Option<String>,
    pub enabled: bool,
    pub default_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexFeatureList {
    pub data: Vec<CodexFeature>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexReviewResult {
    pub review_thread_id: String,
    pub turn: Value,
    pub delivery: CodexReviewDelivery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CodexReviewDelivery {
    Inline,
    Detached,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CodexReviewTarget {
    UncommittedChanges,
    BaseBranch { branch: String },
    Commit { sha: String, title: Option<String> },
    Custom { instructions: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexCommandExecRequest {
    pub command: Vec<String>,
    pub cwd: Option<String>,
    pub timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_id: Option<String>,
    #[serde(default)]
    pub writable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexCommandExecResult {
    pub exit_code: i64,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexFsWatch {
    pub watch_id: String,
    pub path: String,
}

#[derive(Debug, Error)]
pub enum CodexHarnessError {
    #[error("Codex app-server could not be started: {0}")]
    Start(String),
    #[error("Codex app-server I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Codex app-server returned invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Codex app-server request failed: {0}")]
    Protocol(String),
    #[error("invalid Codex request: {0}")]
    InvalidRequest(String),
    #[error("Codex app-server did not respond before the timeout")]
    Timeout,
    #[error("Codex app-server stopped while a request was active")]
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexProcessState {
    Stopped,
    Starting,
    Ready,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexHarnessStatus {
    pub bundled_version: String,
    pub protocol_version: String,
    pub process_state: CodexProcessState,
    pub account_state: String,
    pub active_threads: u64,
    pub pending_approvals: u64,
    pub last_event: Option<String>,
    pub last_notification: Option<String>,
    pub last_unknown_notification: Option<String>,
    pub restart_count: u64,
    pub schema_sha256: String,
    pub initialized: bool,
    pub pending_requests: u64,
    pub unsupported_requests: u64,
    pub last_unsupported_request: Option<String>,
    pub configuration_state: String,
    pub windows_sandbox_state: String,
    pub mcp_bridge_state: String,
    pub recovery_state: String,
}

impl Default for CodexHarnessStatus {
    fn default() -> Self {
        Self {
            bundled_version: CODEX_BINARY_VERSION.into(),
            protocol_version: CODEX_PROTOCOL_VERSION.into(),
            process_state: CodexProcessState::Stopped,
            account_state: "unknown".into(),
            active_threads: 0,
            pending_approvals: 0,
            last_event: None,
            last_notification: None,
            last_unknown_notification: None,
            restart_count: 0,
            schema_sha256: CODEX_SCHEMA_SHA256.into(),
            initialized: false,
            pending_requests: 0,
            unsupported_requests: 0,
            last_unsupported_request: None,
            configuration_state: "unknown".into(),
            windows_sandbox_state: "unknown".into(),
            mcp_bridge_state: "not_started".into(),
            recovery_state: "idle".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexServiceTier {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexReasoningEffort {
    #[serde(rename = "reasoningEffort")]
    pub reasoning_effort: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexModel {
    pub id: String,
    pub model: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub description: String,
    pub hidden: bool,
    #[serde(rename = "isDefault")]
    pub is_default: bool,
    #[serde(rename = "defaultReasoningEffort")]
    pub default_reasoning_effort: String,
    #[serde(rename = "supportedReasoningEfforts")]
    pub supported_reasoning_efforts: Vec<CodexReasoningEffort>,
    #[serde(rename = "inputModalities", default)]
    pub input_modalities: Vec<String>,
    #[serde(rename = "serviceTiers", default)]
    pub service_tiers: Vec<CodexServiceTier>,
    #[serde(rename = "defaultServiceTier", default)]
    pub default_service_tier: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexAccount {
    pub authenticated: bool,
    pub account_type: Option<String>,
    pub email: Option<String>,
    pub plan_type: Option<String>,
    pub requires_openai_auth: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexRateLimitWindow {
    pub used_percent: i64,
    pub remaining_percent: i64,
    pub resets_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexRateLimits {
    pub plan_type: Option<String>,
    pub primary: Option<CodexRateLimitWindow>,
    pub secondary: Option<CodexRateLimitWindow>,
    #[serde(default)]
    pub buckets: BTreeMap<String, CodexRateLimitBucket>,
    #[serde(default)]
    pub credits: Option<Value>,
    pub refreshed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexRateLimitBucket {
    pub limit_id: String,
    pub limit_name: Option<String>,
    pub plan_type: Option<String>,
    pub primary: Option<CodexRateLimitWindow>,
    pub secondary: Option<CodexRateLimitWindow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CodexConfigOverride {
    pub path: Vec<String>,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CodexExecutionPreferences {
    pub model: Option<String>,
    pub effort: Option<String>,
    pub service_tier: Option<String>,
    #[serde(default)]
    pub approval_mode: CodexApprovalMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<CodexApprovalPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox_mode: Option<CodexSandboxMode>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub writable_roots: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub feature_overrides: BTreeMap<String, bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub config_overrides: Vec<CodexConfigOverride>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CodexThreadInstructions {
    pub base: Option<String>,
    pub developer: Option<String>,
    /// A pinned, per-thread Codex configuration override. Two-agent callers use this for their
    /// role-scoped MCP bridge and built-in feature boundary; it is never merged globally.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexMentionInput {
    pub name: String,
    pub path: String,
    pub byte_start: u32,
    pub byte_end: u32,
    pub placeholder: String,
}

fn codex_user_input(prompt: &str, mentions: &[CodexMentionInput]) -> Vec<Value> {
    let text_elements = mentions
        .iter()
        .map(|mention| {
            json!({
                "byteRange": {"start": mention.byte_start, "end": mention.byte_end},
                "placeholder": mention.placeholder,
            })
        })
        .collect::<Vec<_>>();
    let mut input = vec![json!({
        "type": "text",
        "text": prompt,
        "text_elements": text_elements,
    })];
    input.extend(
        mentions
            .iter()
            .map(|mention| json!({"type": "mention", "name": mention.name, "path": mention.path})),
    );
    input
}

/// Verifies the exact pinned binary and the feature/config surface required by the two-agent
/// security boundary. Callers must fail rollout closed on any error.
pub fn verify_pinned_two_agent_conformance(executable: &Path) -> Result<(), CodexHarnessError> {
    let version = run_conformance_command(executable, &["--version"])?;
    if !String::from_utf8_lossy(&version.stdout).contains(CODEX_BINARY_VERSION) {
        return Err(CodexHarnessError::Protocol(format!(
            "multi-agent rollout requires pinned Codex {CODEX_BINARY_VERSION}"
        )));
    }
    let features = run_conformance_command(executable, &["features", "list"])?;
    let features = String::from_utf8_lossy(&features.stdout);
    for required in [
        "shell_tool",
        "unified_exec",
        "multi_agent",
        "browser_use",
        "computer_use",
        "apps",
    ] {
        if !features.lines().any(|line| line.starts_with(required)) {
            return Err(CodexHarnessError::Protocol(format!(
                "pinned Codex does not expose required feature boundary {required}"
            )));
        }
    }
    let schema = include_str!(
        "../../../protocol/codex-app-server/0.150.1/codex_app_server_protocol.v2.schemas.json"
    );
    let schema: Value = serde_json::from_str(schema).map_err(|error| {
        CodexHarnessError::Protocol(format!("pinned schema is not valid JSON: {error}"))
    })?;
    let thread_start_properties = schema
        .pointer("/definitions/ThreadStartParams/properties")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            CodexHarnessError::Protocol("pinned schema has no ThreadStartParams properties".into())
        })?;
    if !thread_start_properties.contains_key("config")
        || !thread_start_properties.contains_key("cwd")
    {
        return Err(CodexHarnessError::Protocol(
            "pinned thread/start cannot carry per-thread configuration".into(),
        ));
    }
    Ok(())
}

fn run_conformance_command(
    executable: &Path,
    arguments: &[&str],
) -> Result<std::process::Output, CodexHarnessError> {
    for attempt in 0..3 {
        let output = std::process::Command::new(executable)
            .args(arguments)
            .output()
            .map_err(CodexHarnessError::Io)?;
        if output.status.success() {
            return Ok(output);
        }
        if transient_process_initialization_failure(&output.status) && attempt < 2 {
            std::thread::sleep(Duration::from_millis(100_u64 << attempt));
            continue;
        }
        return Err(CodexHarnessError::Protocol(format!(
            "pinned Codex conformance command `{}` failed with status {} after {} attempt(s): {}",
            arguments.join(" "),
            output.status,
            attempt + 1,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    unreachable!("bounded conformance retry loop always returns")
}

fn transient_process_initialization_failure(status: &std::process::ExitStatus) -> bool {
    #[cfg(windows)]
    {
        status.code() == Some(-1_073_741_502)
    }
    #[cfg(not(windows))]
    {
        let _ = status;
        false
    }
}

impl CodexThreadInstructions {
    /// Builds the fail-closed Chat role configuration. The Chat thread receives only the MCG MCP
    /// bridge; mutation, browsing, computer use, apps, plugins, skills and nested agents are off.
    #[must_use]
    pub fn isolated_chat_config(mcp_command: &str, bootstrap_file: &str) -> Value {
        role_thread_config(mcp_command, bootstrap_file, false)
    }

    /// Builds the fail-closed Coordinator role configuration. Coordination is performed only
    /// through the role-scoped MCG bridge; native shell/UI and nested Codex agents remain off.
    #[must_use]
    pub fn isolated_coordinator_config(mcp_command: &str, bootstrap_file: &str) -> Value {
        role_thread_config(mcp_command, bootstrap_file, false)
    }

    /// Builds the Coder role configuration. Shell execution remains available inside the
    /// workspace-write sandbox, while networked/UI tools and nested delegation remain disabled.
    #[must_use]
    pub fn isolated_coder_config(mcp_command: &str, bootstrap_file: &str) -> Value {
        role_thread_config(mcp_command, bootstrap_file, true)
    }

    /// Builds the read-only Tester configuration. Testers cannot use the shell or mutate the
    /// project; they receive only the bundled in-app Browser plugin for governed UAT.
    #[must_use]
    pub fn isolated_tester_config(mcp_command: &str, bootstrap_file: &str) -> Value {
        let mut config = role_thread_config(mcp_command, bootstrap_file, false);
        config["features"]["browser_use"] = json!(true);
        config["features"]["in_app_browser"] = json!(true);
        config["features"]["apps"] = json!(true);
        config["features"]["plugins"] = json!(true);
        config["plugins"] = json!({
            "browser@openai-bundled": { "enabled": true },
            "sites@openai-bundled": { "enabled": false },
            "chrome@openai-bundled": { "enabled": false },
            "computer-use@openai-bundled": { "enabled": false },
            "visualize@openai-bundled": { "enabled": false },
            "codex-app-tools@openai-bundled": { "enabled": false },
            "documents@openai-primary-runtime": { "enabled": false },
            "pdf@openai-primary-runtime": { "enabled": false },
            "spreadsheets@openai-primary-runtime": { "enabled": false },
            "presentations@openai-primary-runtime": { "enabled": false },
            "template-creator@openai-primary-runtime": { "enabled": false }
        });
        config
    }

    /// Builds the Site-owning Coder configuration. It retains the ordinary Coder sandbox and
    /// enables only the bundled Sites plugin in addition to the role-scoped MCG bridge.
    #[must_use]
    pub fn isolated_site_coder_config(mcp_command: &str, bootstrap_file: &str) -> Value {
        let mut config = role_thread_config(mcp_command, bootstrap_file, true);
        config["features"]["apps"] = json!(true);
        config["features"]["plugins"] = json!(true);
        config["plugins"] = json!({
            "sites@openai-bundled": { "enabled": true },
            "browser@openai-bundled": { "enabled": false },
            "chrome@openai-bundled": { "enabled": false },
            "computer-use@openai-bundled": { "enabled": false },
            "visualize@openai-bundled": { "enabled": false },
            "codex-app-tools@openai-bundled": { "enabled": false },
            "documents@openai-primary-runtime": { "enabled": false },
            "pdf@openai-primary-runtime": { "enabled": false },
            "spreadsheets@openai-primary-runtime": { "enabled": false },
            "presentations@openai-primary-runtime": { "enabled": false },
            "template-creator@openai-primary-runtime": { "enabled": false }
        });
        config
    }
}

fn role_thread_config(mcp_command: &str, bootstrap_file: &str, coder: bool) -> Value {
    json!({
        "mcp_servers": {
            "mcg_agent": {
                "command": mcp_command,
                "args": ["--bootstrap-file", bootstrap_file],
                "startup_timeout_sec": 10,
                "tool_timeout_sec": 60
            }
        },
        "features": {
            "shell_tool": coder,
            "unified_exec": false,
            "multi_agent": false,
            "enable_fanout": false,
            "browser_use": false,
            "browser_use_external": false,
            "browser_use_full_cdp_access": false,
            "in_app_browser": false,
            "computer_use": false,
            "apps": false,
            "plugins": false,
            "image_generation": false,
            "workspace_dependencies": false
        }
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CodexApprovalMode {
    #[default]
    Ask,
    AutoReview,
    ReadOnly,
    FullAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CodexApprovalPolicy {
    Untrusted,
    OnRequest,
    Never,
}

impl CodexApprovalPolicy {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Untrusted => "untrusted",
            Self::OnRequest => "on-request",
            Self::Never => "never",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CodexSandboxMode {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

impl CodexExecutionPreferences {
    fn thread_config(&self, instructions: Option<&CodexThreadInstructions>) -> Option<Value> {
        let mut config = Value::Object(Default::default());
        for override_value in &self.config_overrides {
            apply_json_path(
                &mut config,
                &override_value.path,
                override_value.value.clone(),
            );
        }
        if !self.feature_overrides.is_empty() {
            merge_json_objects(&mut config, &json!({"features": self.feature_overrides}));
        }
        if let Some(role_config) = instructions.and_then(|value| value.config.as_ref()) {
            merge_json_objects(&mut config, role_config);
        }
        if config.as_object().is_some_and(|object| object.is_empty()) {
            None
        } else {
            Some(config)
        }
    }

    fn approval_policy(&self, writable: bool) -> &'static str {
        if !writable {
            return "never";
        }
        self.approval_policy.map_or_else(
            || match self.approval_mode {
                CodexApprovalMode::Ask | CodexApprovalMode::AutoReview => "on-request",
                CodexApprovalMode::ReadOnly | CodexApprovalMode::FullAccess => "never",
            },
            CodexApprovalPolicy::as_str,
        )
    }

    fn approvals_reviewer(&self) -> &'static str {
        if matches!(self.approval_mode, CodexApprovalMode::AutoReview) {
            "auto_review"
        } else {
            "user"
        }
    }

    fn effective_sandbox(&self, writable: bool) -> CodexSandboxMode {
        if !writable {
            return CodexSandboxMode::ReadOnly;
        }
        self.sandbox_mode.unwrap_or(match self.approval_mode {
            CodexApprovalMode::ReadOnly => CodexSandboxMode::ReadOnly,
            CodexApprovalMode::FullAccess => CodexSandboxMode::DangerFullAccess,
            CodexApprovalMode::Ask | CodexApprovalMode::AutoReview => {
                CodexSandboxMode::WorkspaceWrite
            }
        })
    }

    fn thread_sandbox(&self, writable: bool) -> &'static str {
        match self.effective_sandbox(writable) {
            CodexSandboxMode::ReadOnly => "read-only",
            CodexSandboxMode::WorkspaceWrite => "workspace-write",
            CodexSandboxMode::DangerFullAccess => "danger-full-access",
        }
    }

    fn turn_sandbox(&self, writable: bool) -> Value {
        match self.effective_sandbox(writable) {
            CodexSandboxMode::ReadOnly => json!({"type": "readOnly"}),
            CodexSandboxMode::WorkspaceWrite if self.writable_roots.is_empty() => {
                json!({"type": "workspaceWrite"})
            }
            CodexSandboxMode::WorkspaceWrite => json!({
                "type": "workspaceWrite",
                "writableRoots": self.writable_roots
            }),
            CodexSandboxMode::DangerFullAccess => json!({"type": "dangerFullAccess"}),
        }
    }
}

fn apply_json_path(root: &mut Value, path: &[String], value: Value) {
    if path.is_empty() {
        return;
    }
    let mut current = root;
    for segment in &path[..path.len() - 1] {
        if !current.is_object() {
            *current = Value::Object(Default::default());
        }
        current = current
            .as_object_mut()
            .expect("configuration path was normalized to an object")
            .entry(segment.clone())
            .or_insert_with(|| Value::Object(Default::default()));
    }
    if !current.is_object() {
        *current = Value::Object(Default::default());
    }
    current
        .as_object_mut()
        .expect("configuration parent was normalized to an object")
        .insert(path[path.len() - 1].clone(), value);
}

fn merge_json_objects(base: &mut Value, overlay: &Value) {
    match (base, overlay) {
        (Value::Object(base), Value::Object(overlay)) => {
            for (key, value) in overlay {
                match base.get_mut(key) {
                    Some(existing) => merge_json_objects(existing, value),
                    None => {
                        base.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (base, overlay) => *base = overlay.clone(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexTurnResult {
    pub thread_id: String,
    pub turn_id: String,
    pub text: String,
    pub terminal_state: String,
    pub raw_turn: Value,
}

struct Process {
    child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    #[cfg(windows)]
    _job: win32job::Job,
}

#[cfg(windows)]
fn create_kill_on_close_job(child: &Child) -> Result<win32job::Job, CodexHarnessError> {
    let mut limits = win32job::ExtendedLimitInfo::new();
    limits.limit_kill_on_job_close();
    let job = win32job::Job::create_with_limit_info(&limits).map_err(|error| {
        CodexHarnessError::Start(format!("Windows Job Object creation failed: {error}"))
    })?;
    let process_handle = child
        .raw_handle()
        .ok_or_else(|| CodexHarnessError::Start("Codex process handle was unavailable".into()))?;
    job.assign_process(process_handle as isize)
        .map_err(|error| {
            CodexHarnessError::Start(format!("Codex process containment failed: {error}"))
        })?;
    Ok(job)
}

struct Inner {
    executable: PathBuf,
    process: Option<Process>,
    next_id: u64,
    pending: Arc<Mutex<BTreeMap<u64, oneshot::Sender<Result<Value, String>>>>>,
    notifications: broadcast::Sender<Value>,
    status: CodexHarnessStatus,
    server_requests: Arc<Mutex<BTreeMap<String, CodexPendingRequest>>>,
    notification_telemetry: Arc<Mutex<(Option<String>, Option<String>)>>,
}

#[derive(Clone)]
pub struct CodexAppServerHarness {
    inner: Arc<Mutex<Inner>>,
}

impl std::fmt::Debug for CodexAppServerHarness {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CodexAppServerHarness")
            .finish_non_exhaustive()
    }
}

impl CodexAppServerHarness {
    #[must_use]
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        let (notifications, _) = broadcast::channel(1_024);
        Self {
            inner: Arc::new(Mutex::new(Inner {
                executable: executable.into(),
                process: None,
                next_id: 1,
                pending: Arc::new(Mutex::new(BTreeMap::new())),
                notifications,
                status: CodexHarnessStatus::default(),
                server_requests: Arc::new(Mutex::new(BTreeMap::new())),
                notification_telemetry: Arc::new(Mutex::new((None, None))),
            })),
        }
    }

    pub async fn status(&self) -> CodexHarnessStatus {
        let inner = self.inner.lock().await;
        let mut status = inner.status.clone();
        let requests = inner.server_requests.lock().await;
        status.pending_requests = requests.len() as u64;
        status.unsupported_requests = requests
            .values()
            .filter(|request| !request.supported)
            .count() as u64;
        status.last_unsupported_request = requests
            .values()
            .rev()
            .find(|request| !request.supported)
            .map(|request| request.method.clone());
        let telemetry = inner.notification_telemetry.lock().await;
        status.last_notification.clone_from(&telemetry.0);
        status.last_unknown_notification.clone_from(&telemetry.1);
        status
    }

    pub async fn pending_server_requests(&self) -> Vec<CodexPendingRequest> {
        let inner = self.inner.lock().await;
        inner
            .server_requests
            .lock()
            .await
            .values()
            .cloned()
            .collect()
    }

    pub async fn subscribe(&self) -> broadcast::Receiver<Value> {
        self.inner.lock().await.notifications.subscribe()
    }

    async fn ensure_started(&self) -> Result<(), CodexHarnessError> {
        let mut inner = self.inner.lock().await;
        if let Some(process) = inner.process.as_mut() {
            match process.child.try_wait()? {
                None => return Ok(()),
                Some(_) => {
                    inner.process = None;
                    inner.status.process_state = CodexProcessState::Stopped;
                }
            }
        }
        inner.status.process_state = CodexProcessState::Starting;
        let mut command = Command::new(&inner.executable);
        command
            .arg("app-server")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = command.spawn().map_err(|error| {
            inner.status.process_state = CodexProcessState::Failed;
            CodexHarnessError::Start(error.to_string())
        })?;
        #[cfg(windows)]
        let job = create_kill_on_close_job(&child)?;
        let stdin =
            Arc::new(Mutex::new(child.stdin.take().ok_or_else(|| {
                CodexHarnessError::Start("stdin was unavailable".into())
            })?));
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| CodexHarnessError::Start("stdout was unavailable".into()))?;
        let pending = inner.pending.clone();
        let notifications = inner.notifications.clone();
        let server_requests = inner.server_requests.clone();
        let notification_telemetry = inner.notification_telemetry.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let Ok(document) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                if document.get("method").is_some() && document.get("id").is_some() {
                    let _ = notifications.send(document.clone());
                    let method = document
                        .get("method")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let request_id = document.get("id").cloned().unwrap_or(Value::Null);
                    server_requests.lock().await.insert(
                        request_id.to_string(),
                        CodexPendingRequest {
                            request_id,
                            method: method.to_owned(),
                            params: document.get("params").cloned().unwrap_or(Value::Null),
                            supported: is_supported_server_request(method),
                            received_at: Utc::now(),
                        },
                    );
                    // Requests are intentionally left unanswered until MCG has durably
                    // persisted and resolved them. Unknown requests fail closed and stay
                    // visible in harness health; never fabricate a protocol response.
                } else if let Some(id) = document.get("id").and_then(Value::as_u64) {
                    if let Some(sender) = pending.lock().await.remove(&id) {
                        let result = document.get("error").map_or_else(
                            || Ok(document.get("result").cloned().unwrap_or(Value::Null)),
                            |error| Err(error.to_string()),
                        );
                        let _ = sender.send(result);
                    }
                } else {
                    if let Some(method) = document.get("method").and_then(Value::as_str) {
                        let mut telemetry = notification_telemetry.lock().await;
                        telemetry.0 = Some(method.to_owned());
                        if !protocol_generated::is_known_notification(method) {
                            telemetry.1 = Some(method.to_owned());
                        }
                    }
                    let _ = notifications.send(document);
                }
            }
            let mut pending = pending.lock().await;
            for (_, sender) in std::mem::take(&mut *pending) {
                let _ = sender.send(Err("Codex app-server stopped".into()));
            }
        });
        inner.process = Some(Process {
            child,
            stdin,
            #[cfg(windows)]
            _job: job,
        });
        inner.status.process_state = CodexProcessState::Ready;
        inner.status.restart_count = inner.status.restart_count.saturating_add(1);
        drop(inner);
        self.request_started("initialize", json!({
            "clientInfo": {"name": "mcg", "title": "Metacognitive Governor", "version": env!("CARGO_PKG_VERSION")},
            "capabilities": {"experimentalApi": true}
        })).await?;
        self.notify("initialized", json!({})).await?;
        self.inner.lock().await.status.initialized = true;
        Ok(())
    }

    async fn write(&self, value: &Value) -> Result<(), CodexHarnessError> {
        let bytes = serde_json::to_vec(value)?;
        let stdin = {
            let inner = self.inner.lock().await;
            inner
                .process
                .as_ref()
                .ok_or(CodexHarnessError::Stopped)?
                .stdin
                .clone()
        };
        let mut stdin = stdin.lock().await;
        stdin.write_all(&bytes).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;
        Ok(())
    }

    pub async fn resolve_approval_request(
        &self,
        request: &CodexApprovalRequest,
        approved: bool,
    ) -> Result<(), CodexHarnessError> {
        let result = approval_response(request, approved);
        self.write(&json!({"id": request.request_id, "result": result}))
            .await?;
        let mut inner = self.inner.lock().await;
        inner
            .server_requests
            .lock()
            .await
            .remove(&request.request_id.to_string());
        inner.status.pending_approvals = inner.status.pending_approvals.saturating_sub(1);
        inner.status.last_event = Some(if approved {
            "approval/accepted".into()
        } else {
            "approval/declined".into()
        });
        Ok(())
    }

    pub async fn resolve_mcp_elicitation(
        &self,
        request_id: &Value,
        accepted_content: Option<Value>,
    ) -> Result<(), CodexHarnessError> {
        let accepted = accepted_content.is_some();
        self.write(&json!({
            "id": request_id,
            "result": {
                "action": if accepted { "accept" } else { "decline" },
                "content": accepted_content,
                "_meta": null
            }
        }))
        .await?;
        let inner = self.inner.lock().await;
        inner
            .server_requests
            .lock()
            .await
            .remove(&request_id.to_string());
        Ok(())
    }

    pub async fn resolve_user_input_request(
        &self,
        request_id: &Value,
        answers: Value,
    ) -> Result<(), CodexHarnessError> {
        self.write(&json!({"id": request_id, "result": {"answers": answers}}))
            .await?;
        let inner = self.inner.lock().await;
        inner
            .server_requests
            .lock()
            .await
            .remove(&request_id.to_string());
        Ok(())
    }

    async fn notify(&self, method: &str, params: Value) -> Result<(), CodexHarnessError> {
        self.write(&json!({"method": method, "params": params}))
            .await
    }

    pub async fn request(&self, method: &str, params: Value) -> Result<Value, CodexHarnessError> {
        self.ensure_started().await?;
        self.request_started(method, params).await
    }

    async fn request_started(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Value, CodexHarnessError> {
        let (id, receiver, document) = {
            let mut inner = self.inner.lock().await;
            let id = inner.next_id;
            inner.next_id = inner.next_id.saturating_add(1);
            let (sender, receiver) = oneshot::channel();
            inner.pending.lock().await.insert(id, sender);
            (
                id,
                receiver,
                json!({"id": id, "method": method, "params": params}),
            )
        };
        if let Err(error) = self.write(&document).await {
            self.inner.lock().await.pending.lock().await.remove(&id);
            return Err(error);
        }
        match tokio::time::timeout(Duration::from_secs(180), receiver).await {
            Ok(Ok(Ok(value))) => Ok(value),
            Ok(Ok(Err(error))) => Err(CodexHarnessError::Protocol(error)),
            Ok(Err(_)) => Err(CodexHarnessError::Stopped),
            Err(_) => {
                self.inner.lock().await.pending.lock().await.remove(&id);
                Err(CodexHarnessError::Timeout)
            }
        }
    }

    pub async fn models(&self) -> Result<Vec<CodexModel>, CodexHarnessError> {
        let response = self.request("model/list", json!({"limit": 100})).await?;
        serde_json::from_value(response.get("data").cloned().unwrap_or_else(|| json!([])))
            .map_err(Into::into)
    }

    pub async fn features(&self) -> Result<Vec<CodexFeature>, CodexHarnessError> {
        let mut cursor: Option<String> = None;
        let mut features = Vec::new();
        loop {
            let response = self
                .request(
                    "experimentalFeature/list",
                    json!({"cursor": cursor, "limit": 100}),
                )
                .await?;
            let page: CodexFeatureList = serde_json::from_value(response)?;
            features.extend(page.data);
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        Ok(features)
    }

    pub async fn set_feature(
        &self,
        name: &str,
        enabled: bool,
    ) -> Result<Vec<CodexFeature>, CodexHarnessError> {
        let name = name.trim();
        if name.is_empty()
            || name.len() > 128
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        {
            return Err(CodexHarnessError::InvalidRequest(
                "feature name must be a non-empty lowercase ASCII key".into(),
            ));
        }
        let known = self.features().await?;
        if !known.iter().any(|feature| feature.name == name) {
            return Err(CodexHarnessError::InvalidRequest(format!(
                "pinned Codex does not expose feature `{name}`"
            )));
        }
        self.request(
            "experimentalFeature/enablement/set",
            json!({"enablement": {(name): enabled}}),
        )
        .await?;
        self.features().await
    }

    pub async fn account(&self) -> Result<CodexAccount, CodexHarnessError> {
        let response = self.request("account/read", json!({})).await?;
        let account = response.get("account");
        let result = CodexAccount {
            authenticated: account.is_some_and(|value| !value.is_null()),
            account_type: account
                .and_then(|value| value.get("type"))
                .and_then(Value::as_str)
                .map(str::to_owned),
            email: account
                .and_then(|value| value.get("email"))
                .and_then(Value::as_str)
                .map(str::to_owned),
            plan_type: account
                .and_then(|value| value.get("planType"))
                .and_then(Value::as_str)
                .map(str::to_owned),
            requires_openai_auth: response
                .get("requiresOpenaiAuth")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        };
        let mut inner = self.inner.lock().await;
        inner.status.account_state = if result.authenticated {
            "authenticated"
        } else {
            "signed_out"
        }
        .into();
        Ok(result)
    }

    pub async fn rate_limits(&self) -> Result<CodexRateLimits, CodexHarnessError> {
        let response = self.request("account/rateLimits/read", json!({})).await?;
        let snapshot = response.get("rateLimits").cloned().unwrap_or(Value::Null);
        let window = |value: Option<&Value>| {
            value.filter(|value| !value.is_null()).map(|value| {
                let used = value
                    .get("usedPercent")
                    .and_then(Value::as_i64)
                    .unwrap_or(0)
                    .clamp(0, 100);
                CodexRateLimitWindow {
                    used_percent: used,
                    remaining_percent: 100 - used,
                    resets_at: value.get("resetsAt").and_then(Value::as_i64),
                }
            })
        };
        let mut buckets = BTreeMap::new();
        if let Some(entries) = response
            .get("rateLimitsByLimitId")
            .and_then(Value::as_object)
        {
            for (id, value) in entries {
                buckets.insert(
                    id.clone(),
                    CodexRateLimitBucket {
                        limit_id: value
                            .get("limitId")
                            .and_then(Value::as_str)
                            .unwrap_or(id)
                            .to_owned(),
                        limit_name: value
                            .get("limitName")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                        plan_type: value
                            .get("planType")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                        primary: window(value.get("primary")),
                        secondary: window(value.get("secondary")),
                    },
                );
            }
        }
        Ok(CodexRateLimits {
            plan_type: snapshot
                .get("planType")
                .and_then(Value::as_str)
                .map(str::to_owned),
            primary: window(snapshot.get("primary")),
            secondary: window(snapshot.get("secondary")),
            buckets,
            credits: snapshot
                .get("credits")
                .cloned()
                .filter(|value| !value.is_null()),
            refreshed_at: Utc::now(),
        })
    }

    pub async fn start_login(&self) -> Result<Value, CodexHarnessError> {
        self.request("account/login/start", json!({"type": "chatgpt"}))
            .await
    }

    pub async fn cancel_login(&self, login_id: &str) -> Result<Value, CodexHarnessError> {
        self.request("account/login/cancel", json!({"loginId": login_id}))
            .await
    }

    pub async fn logout(&self) -> Result<Value, CodexHarnessError> {
        self.request("account/logout", json!({})).await
    }

    pub async fn start_thread(
        &self,
        cwd: Option<&str>,
        preferences: &CodexExecutionPreferences,
        writable: bool,
    ) -> Result<String, CodexHarnessError> {
        self.start_thread_with_instructions(cwd, preferences, writable, None)
            .await
    }

    pub async fn start_thread_with_instructions(
        &self,
        cwd: Option<&str>,
        preferences: &CodexExecutionPreferences,
        writable: bool,
        instructions: Option<&CodexThreadInstructions>,
    ) -> Result<String, CodexHarnessError> {
        let config = preferences.thread_config(instructions);
        let response = self
            .request(
                "thread/start",
                json!({
                    "cwd": cwd,
                    "model": preferences.model,
                    "serviceTier": preferences.service_tier,
                    "approvalPolicy": preferences.approval_policy(writable),
                    "approvalsReviewer": preferences.approvals_reviewer(),
                    "sandbox": preferences.thread_sandbox(writable),
                    "baseInstructions": instructions.and_then(|value| value.base.as_deref()),
                    "developerInstructions": instructions.and_then(|value| value.developer.as_deref()),
                    "config": config,
                    "threadSource": "mcg"
                }),
            )
            .await?;
        let thread_id = response
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                CodexHarnessError::Protocol("thread/start did not return a thread id".into())
            })?
            .to_owned();
        let mut inner = self.inner.lock().await;
        inner.status.active_threads = inner.status.active_threads.saturating_add(1);
        Ok(thread_id)
    }

    /// Starts a non-persistent thread for thread-scoped account connector calls.
    /// The returned thread never enters MCG's durable conversation registry.
    pub async fn start_ephemeral_thread(&self) -> Result<String, CodexHarnessError> {
        self.request("thread/start", json!({"ephemeral": true}))
            .await?
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| {
                CodexHarnessError::Protocol(
                    "ephemeral thread/start did not return a thread id".into(),
                )
            })
    }

    pub async fn resume_thread(
        &self,
        thread_id: &str,
        cwd: Option<&str>,
        preferences: &CodexExecutionPreferences,
        writable: bool,
    ) -> Result<(), CodexHarnessError> {
        self.resume_thread_with_instructions(thread_id, cwd, preferences, writable, None)
            .await
    }

    pub async fn resume_thread_with_instructions(
        &self,
        thread_id: &str,
        cwd: Option<&str>,
        preferences: &CodexExecutionPreferences,
        writable: bool,
        instructions: Option<&CodexThreadInstructions>,
    ) -> Result<(), CodexHarnessError> {
        let config = preferences.thread_config(instructions);
        self.request(
            "thread/resume",
            json!({
                "threadId": thread_id,
                "cwd": cwd,
                "model": preferences.model,
                "serviceTier": preferences.service_tier,
                "approvalPolicy": preferences.approval_policy(writable),
                "approvalsReviewer": preferences.approvals_reviewer(),
                "sandbox": preferences.thread_sandbox(writable)
                ,"baseInstructions": instructions.and_then(|value| value.base.as_deref())
                ,"developerInstructions": instructions.and_then(|value| value.developer.as_deref())
                ,"config": config
            }),
        )
        .await
        .map(drop)
    }

    pub async fn start_turn(
        &self,
        thread_id: &str,
        prompt: &str,
        preferences: &CodexExecutionPreferences,
        writable: bool,
    ) -> Result<CodexTurnResult, CodexHarnessError> {
        self.start_turn_with_progress(thread_id, prompt, preferences, writable, None)
            .await
    }

    pub async fn start_turn_with_mentions(
        &self,
        thread_id: &str,
        prompt: &str,
        mentions: &[CodexMentionInput],
        preferences: &CodexExecutionPreferences,
        writable: bool,
    ) -> Result<CodexTurnResult, CodexHarnessError> {
        self.start_turn_with_progress_and_schema_and_mode(
            thread_id,
            prompt,
            mentions,
            preferences,
            writable,
            None,
            None,
            false,
        )
        .await
    }

    pub async fn start_turn_with_progress(
        &self,
        thread_id: &str,
        prompt: &str,
        preferences: &CodexExecutionPreferences,
        writable: bool,
        progress: Option<mpsc::UnboundedSender<CodexTurnProgress>>,
    ) -> Result<CodexTurnResult, CodexHarnessError> {
        self.start_turn_with_progress_and_schema(
            thread_id,
            prompt,
            &[],
            preferences,
            writable,
            progress,
            None,
        )
        .await
    }

    pub async fn start_plan_turn_with_progress(
        &self,
        thread_id: &str,
        prompt: &str,
        preferences: &CodexExecutionPreferences,
        progress: Option<mpsc::UnboundedSender<CodexTurnProgress>>,
    ) -> Result<CodexTurnResult, CodexHarnessError> {
        self.start_turn_with_progress_and_schema_and_mode(
            thread_id,
            prompt,
            &[],
            preferences,
            false,
            progress,
            None,
            true,
        )
        .await
    }

    pub async fn start_turn_with_mentions_and_progress(
        &self,
        thread_id: &str,
        prompt: &str,
        mentions: &[CodexMentionInput],
        preferences: &CodexExecutionPreferences,
        writable: bool,
        progress: Option<mpsc::UnboundedSender<CodexTurnProgress>>,
    ) -> Result<CodexTurnResult, CodexHarnessError> {
        self.start_turn_with_progress_and_schema_and_mode(
            thread_id,
            prompt,
            mentions,
            preferences,
            writable,
            progress,
            None,
            false,
        )
        .await
    }

    pub async fn start_plan_turn_with_mentions_and_progress(
        &self,
        thread_id: &str,
        prompt: &str,
        mentions: &[CodexMentionInput],
        preferences: &CodexExecutionPreferences,
        progress: Option<mpsc::UnboundedSender<CodexTurnProgress>>,
    ) -> Result<CodexTurnResult, CodexHarnessError> {
        self.start_turn_with_progress_and_schema_and_mode(
            thread_id,
            prompt,
            mentions,
            preferences,
            false,
            progress,
            None,
            true,
        )
        .await
    }

    pub async fn start_structured_turn(
        &self,
        thread_id: &str,
        prompt: &str,
        preferences: &CodexExecutionPreferences,
        writable: bool,
        output_schema: &Value,
    ) -> Result<CodexTurnResult, CodexHarnessError> {
        self.start_turn_with_progress_and_schema(
            thread_id,
            prompt,
            &[],
            preferences,
            writable,
            None,
            Some(output_schema),
        )
        .await
    }

    async fn start_turn_with_progress_and_schema(
        &self,
        thread_id: &str,
        prompt: &str,
        mentions: &[CodexMentionInput],
        preferences: &CodexExecutionPreferences,
        writable: bool,
        progress: Option<mpsc::UnboundedSender<CodexTurnProgress>>,
        output_schema: Option<&Value>,
    ) -> Result<CodexTurnResult, CodexHarnessError> {
        self.start_turn_with_progress_and_schema_and_mode(
            thread_id,
            prompt,
            mentions,
            preferences,
            writable,
            progress,
            output_schema,
            false,
        )
        .await
    }

    async fn start_turn_with_progress_and_schema_and_mode(
        &self,
        thread_id: &str,
        prompt: &str,
        mentions: &[CodexMentionInput],
        preferences: &CodexExecutionPreferences,
        writable: bool,
        progress: Option<mpsc::UnboundedSender<CodexTurnProgress>>,
        output_schema: Option<&Value>,
        plan_mode: bool,
    ) -> Result<CodexTurnResult, CodexHarnessError> {
        let mut notifications = {
            self.ensure_started().await?;
            self.inner.lock().await.notifications.subscribe()
        };
        let collaboration_mode = plan_mode.then(|| {
            json!({
                "mode": "plan",
                "settings": {
                    "model": preferences.model.clone().unwrap_or_default(),
                    "reasoning_effort": preferences.effort,
                    "developer_instructions": null
                }
            })
        });
        let response = self
            .request(
                "turn/start",
                json!({
                    "threadId": thread_id,
                    "input": codex_user_input(prompt, mentions),
                    "model": preferences.model,
                    "effort": preferences.effort,
                    "serviceTier": preferences.service_tier,
                    "approvalPolicy": preferences.approval_policy(writable),
                    "approvalsReviewer": preferences.approvals_reviewer(),
                    "sandboxPolicy": preferences.turn_sandbox(writable),
                    "outputSchema": output_schema
                    ,"collaborationMode": collaboration_mode
                }),
            )
            .await?;
        let turn_id = response
            .pointer("/turn/id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                CodexHarnessError::Protocol("turn/start did not return a turn id".into())
            })?
            .to_owned();
        if let Some(progress) = &progress {
            let _ = progress.send(CodexTurnProgress::Started {
                turn_id: turn_id.clone(),
            });
        }
        let mut text = String::new();
        loop {
            let notification = tokio::time::timeout(Duration::from_secs(180), notifications.recv())
                .await
                .map_err(|_| CodexHarnessError::Timeout)?
                .map_err(|_| CodexHarnessError::Stopped)?;
            let method = notification
                .get("method")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let params = notification.get("params").cloned().unwrap_or(Value::Null);
            {
                let mut inner = self.inner.lock().await;
                inner.status.last_event = Some(method.to_owned());
            }
            if is_approval_request(method)
                && params.get("turnId").and_then(Value::as_str) == Some(turn_id.as_str())
            {
                let request = parse_approval_request(&notification)?;
                {
                    let mut inner = self.inner.lock().await;
                    inner.status.pending_approvals =
                        inner.status.pending_approvals.saturating_add(1);
                }
                if let Some(progress) = &progress {
                    let _ = progress.send(CodexTurnProgress::ApprovalRequested(request));
                } else {
                    // Callers that do not expose an approval surface must never leave
                    // app-server blocked on an unanswered request.
                    self.resolve_approval_request(&request, false).await?;
                }
            }
            if method == "item/tool/requestUserInput"
                && params.get("turnId").and_then(Value::as_str) == Some(turn_id.as_str())
            {
                let request_id = notification.get("id").cloned().unwrap_or(Value::Null);
                let request = CodexPendingRequest {
                    request_id: request_id.clone(),
                    method: method.to_owned(),
                    params: params.clone(),
                    supported: true,
                    received_at: Utc::now(),
                };
                if let Some(progress) = &progress {
                    let _ = progress.send(CodexTurnProgress::UserInputRequested(request));
                } else {
                    self.resolve_user_input_request(&request_id, json!({}))
                        .await?;
                }
            }
            if method == "mcpServer/elicitation/request"
                && params.get("turnId").and_then(Value::as_str) == Some(turn_id.as_str())
            {
                let trusted_internal_bridge =
                    params.get("serverName").and_then(Value::as_str) == Some("mcg_agent");
                if trusted_internal_bridge {
                    let request_id = notification.get("id").ok_or_else(|| {
                        CodexHarnessError::Protocol(
                            "MCG agent MCP elicitation omitted its request id".into(),
                        )
                    })?;
                    // `mcg_agent` is injected only by MCG's per-thread role configuration. Its
                    // bearer and exact tool list are authorized again by the loopback runtime,
                    // so this is not an owner permission boundary and must not hang a role turn.
                    self.resolve_mcp_elicitation(request_id, Some(json!({})))
                        .await?;
                } else if let Some(progress) = &progress {
                    let _ = progress.send(CodexTurnProgress::Notification {
                        method: method.to_owned(),
                        params: notification.clone(),
                    });
                } else if let Some(request_id) = notification.get("id") {
                    // Ordinary MCP elicitations still fail closed when the caller exposes no
                    // approval surface.
                    self.resolve_mcp_elicitation(request_id, None).await?;
                }
            }
            if matches!(method, "item/agentMessage/delta" | "item/plan/delta")
                && params.get("turnId").and_then(Value::as_str) == Some(turn_id.as_str())
            {
                if let (Some(item_id), Some(delta)) = (
                    params.get("itemId").and_then(Value::as_str),
                    params.get("delta").and_then(Value::as_str),
                ) {
                    text.push_str(delta);
                    if let Some(progress) = &progress {
                        let _ = progress.send(CodexTurnProgress::TextDelta {
                            item_id: item_id.to_owned(),
                            delta: delta.to_owned(),
                        });
                    }
                }
            }
            if matches!(method, "item/started" | "item/completed")
                && params.get("turnId").and_then(Value::as_str) == Some(turn_id.as_str())
                && let Some(item) = params.get("item").cloned()
                && let Some(progress) = &progress
            {
                let update = if method == "item/started" {
                    CodexTurnProgress::ItemStarted { item }
                } else {
                    CodexTurnProgress::ItemCompleted { item }
                };
                let _ = progress.send(update);
            }
            if method == "item/commandExecution/outputDelta"
                && params.get("turnId").and_then(Value::as_str) == Some(turn_id.as_str())
                && let (Some(item_id), Some(delta)) = (
                    params.get("itemId").and_then(Value::as_str),
                    params.get("delta").and_then(Value::as_str),
                )
                && let Some(progress) = &progress
            {
                let _ = progress.send(CodexTurnProgress::CommandOutputDelta {
                    item_id: item_id.to_owned(),
                    delta: delta.to_owned(),
                });
            }
            if is_visible_turn_notification(method)
                && params.get("turnId").and_then(Value::as_str) == Some(turn_id.as_str())
                && let Some(progress) = &progress
            {
                let _ = progress.send(CodexTurnProgress::Notification {
                    method: method.to_owned(),
                    params: params.clone(),
                });
            }
            if method == "turn/completed"
                && params
                    .get("turn")
                    .and_then(|turn| turn.get("id"))
                    .and_then(Value::as_str)
                    == Some(turn_id.as_str())
            {
                let turn = params.get("turn").cloned().unwrap_or(Value::Null);
                let state = turn
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("completed")
                    .to_owned();
                return Ok(CodexTurnResult {
                    thread_id: thread_id.into(),
                    turn_id,
                    text,
                    terminal_state: state,
                    raw_turn: turn,
                });
            }
        }
    }

    pub async fn interrupt_turn(
        &self,
        thread_id: &str,
        turn_id: &str,
    ) -> Result<(), CodexHarnessError> {
        self.request(
            "turn/interrupt",
            json!({"threadId": thread_id, "turnId": turn_id}),
        )
        .await
        .map(drop)
    }

    pub async fn steer_turn(
        &self,
        thread_id: &str,
        turn_id: &str,
        content: &str,
    ) -> Result<String, CodexHarnessError> {
        self.steer_turn_with_mentions(thread_id, turn_id, content, &[])
            .await
    }

    pub async fn steer_turn_with_mentions(
        &self,
        thread_id: &str,
        turn_id: &str,
        content: &str,
        mentions: &[CodexMentionInput],
    ) -> Result<String, CodexHarnessError> {
        let result = self
            .request(
                "turn/steer",
                json!({
                    "threadId": thread_id,
                    "input": codex_user_input(content, mentions),
                    "expectedTurnId": turn_id,
                }),
            )
            .await?;
        result
            .get("turnId")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| CodexHarnessError::Protocol("turn/steer response has no turnId".into()))
    }

    pub async fn read_thread(&self, thread_id: &str) -> Result<Value, CodexHarnessError> {
        self.request(
            "thread/read",
            json!({"threadId": thread_id, "includeTurns": true}),
        )
        .await
    }

    pub async fn fork_thread(&self, thread_id: &str) -> Result<String, CodexHarnessError> {
        self.fork_thread_at(thread_id, None).await
    }

    pub async fn fork_thread_at(
        &self,
        thread_id: &str,
        last_turn_id: Option<&str>,
    ) -> Result<String, CodexHarnessError> {
        let response = self
            .request(
                "thread/fork",
                json!({
                    "threadId": thread_id,
                    "lastTurnId": last_turn_id,
                    "threadSource": "mcg"
                }),
            )
            .await?;
        response
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| {
                CodexHarnessError::Protocol("thread/fork did not return a thread id".into())
            })
    }

    pub async fn set_thread_name(
        &self,
        thread_id: &str,
        name: &str,
    ) -> Result<(), CodexHarnessError> {
        self.request(
            "thread/name/set",
            json!({"threadId": thread_id, "name": name}),
        )
        .await
        .map(drop)
    }

    pub async fn archive_thread(&self, thread_id: &str) -> Result<(), CodexHarnessError> {
        self.request("thread/archive", json!({"threadId": thread_id}))
            .await
            .map(drop)
    }

    /// Archives every non-archived app-server thread carrying an exact client analytics tag.
    ///
    /// MCG uses this to keep its recoverable role threads out of another Codex client's
    /// user-facing Recents list. The exact `threadSource` match is deliberate: ordinary Codex,
    /// extension, CLI, and other app-server threads are never touched.
    pub async fn archive_threads_with_source(
        &self,
        thread_source: &str,
    ) -> Result<Vec<String>, CodexHarnessError> {
        let mut cursor: Option<String> = None;
        let mut matching = Vec::new();
        loop {
            let response = self
                .request(
                    "thread/list",
                    json!({
                        "cursor": cursor,
                        "limit": 100,
                        "sortKey": "created_at",
                        "sortDirection": "desc",
                        // The embedded binary inherits the launching Codex surface's source
                        // kind (for example `vscode` on Codex Desktop). Authority comes from
                        // the exact `threadSource` tag below, not this broad discovery filter.
                        "sourceKinds": [
                            "cli", "vscode", "exec", "appServer", "subAgent",
                            "subAgentReview", "subAgentCompact", "subAgentThreadSpawn",
                            "subAgentOther", "unknown"
                        ],
                        "archived": false
                    }),
                )
                .await?;
            matching.extend(thread_ids_with_source(&response, thread_source));
            cursor = response
                .get("nextCursor")
                .and_then(Value::as_str)
                .map(str::to_owned);
            if cursor.is_none() {
                break;
            }
        }
        for thread_id in &matching {
            self.archive_thread(thread_id).await?;
        }
        Ok(matching)
    }

    pub async fn unarchive_thread(&self, thread_id: &str) -> Result<(), CodexHarnessError> {
        self.request("thread/unarchive", json!({"threadId": thread_id}))
            .await
            .map(drop)
    }

    pub async fn unsubscribe_thread(&self, thread_id: &str) -> Result<(), CodexHarnessError> {
        self.request("thread/unsubscribe", json!({"threadId": thread_id}))
            .await
            .map(drop)
    }

    pub async fn compact_thread(&self, thread_id: &str) -> Result<(), CodexHarnessError> {
        self.request("thread/compact/start", json!({"threadId": thread_id}))
            .await
            .map(drop)
    }

    pub async fn set_thread_goal(
        &self,
        thread_id: &str,
        objective: Option<&str>,
        status: Option<&str>,
        token_budget: Option<u64>,
    ) -> Result<Value, CodexHarnessError> {
        self.request(
            "thread/goal/set",
            json!({
                "threadId": thread_id,
                "objective": objective,
                "status": status,
                "tokenBudget": token_budget
            }),
        )
        .await
    }

    pub async fn get_thread_goal(&self, thread_id: &str) -> Result<Value, CodexHarnessError> {
        self.request("thread/goal/get", json!({"threadId": thread_id}))
            .await
    }

    pub async fn clear_thread_goal(&self, thread_id: &str) -> Result<(), CodexHarnessError> {
        self.request("thread/goal/clear", json!({"threadId": thread_id}))
            .await
            .map(drop)
    }

    pub async fn account_usage(&self) -> Result<Value, CodexHarnessError> {
        self.request("account/usage/read", json!({})).await
    }

    /// Calls one tool exposed by an initialized MCP server for an existing thread.
    pub async fn call_mcp_tool(
        &self,
        thread_id: &str,
        server: &str,
        tool: &str,
        arguments: Value,
    ) -> Result<Value, CodexHarnessError> {
        self.request(
            "mcpServer/tool/call",
            json!({
                "threadId": thread_id,
                "server": server,
                "tool": tool,
                "arguments": arguments,
            }),
        )
        .await
    }

    pub async fn model_provider_capabilities(&self) -> Result<Value, CodexHarnessError> {
        self.request("modelProvider/capabilities/read", json!({}))
            .await
    }

    pub async fn configuration_requirements(&self) -> Result<Value, CodexHarnessError> {
        self.request("configRequirements/read", json!({})).await
    }

    pub async fn windows_sandbox_readiness(&self) -> Result<Value, CodexHarnessError> {
        self.request(WINDOWS_SANDBOX_READINESS_METHOD, json!({}))
            .await
    }

    pub async fn stable_discovery(
        &self,
        cwds: &[String],
        force_reload: bool,
    ) -> Result<CodexStableDiscovery, CodexHarnessError> {
        let (skills, hooks, first_apps) = tokio::try_join!(
            self.request(
                "skills/list",
                json!({"cwds": cwds, "forceReload": force_reload}),
            ),
            self.request("hooks/list", json!({"cwds": cwds})),
            self.request(
                "app/list",
                json!({"cursor": null, "limit": 100, "forceRefetch": force_reload}),
            ),
        )?;
        let mut apps = first_apps
            .get("data")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut cursor = first_apps
            .get("nextCursor")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let mut observed_cursors = BTreeSet::new();
        for _ in 0..99 {
            let Some(next_cursor) = cursor.take() else {
                break;
            };
            if !observed_cursors.insert(next_cursor.clone()) {
                return Err(CodexHarnessError::Protocol(format!(
                    "app/list repeated pagination cursor {next_cursor:?}"
                )));
            }
            let page = self
                .request(
                    "app/list",
                    json!({"cursor": next_cursor, "limit": 100, "forceRefetch": false}),
                )
                .await?;
            apps.extend(
                page.get("data")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .cloned(),
            );
            cursor = page
                .get("nextCursor")
                .and_then(Value::as_str)
                .map(str::to_owned);
        }
        if cursor.is_some() {
            return Err(CodexHarnessError::Protocol(format!(
                "app/list exceeded the bounded 10,000-app discovery limit ({} entries; next cursor {:?})",
                apps.len(),
                cursor
            )));
        }
        Ok(CodexStableDiscovery {
            skills: skills
                .get("data")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
            hooks: hooks
                .get("data")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
            apps,
            apps_available: true,
            apps_unavailable_reason: None,
            refreshed_at: Utc::now(),
        })
    }

    pub async fn start_review(
        &self,
        thread_id: &str,
        target: CodexReviewTarget,
        delivery: CodexReviewDelivery,
    ) -> Result<CodexReviewResult, CodexHarnessError> {
        let response = self
            .request(
                "review/start",
                json!({"threadId": thread_id, "target": target, "delivery": delivery}),
            )
            .await?;
        let review_thread_id = response
            .get("reviewThreadId")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                CodexHarnessError::Protocol("review/start did not return reviewThreadId".into())
            })?
            .to_owned();
        Ok(CodexReviewResult {
            review_thread_id,
            turn: response.get("turn").cloned().unwrap_or(Value::Null),
            delivery,
        })
    }

    pub async fn exec_command(
        &self,
        request: CodexCommandExecRequest,
    ) -> Result<CodexCommandExecResult, CodexHarnessError> {
        if request.command.is_empty() {
            return Err(CodexHarnessError::Protocol(
                "command/exec requires a non-empty argv vector".into(),
            ));
        }
        let sandbox = if request.writable {
            let cwd = request.cwd.as_deref().ok_or_else(|| {
                CodexHarnessError::Protocol(
                    "writable command/exec requires an explicit governed cwd".into(),
                )
            })?;
            json!({
                "type": "workspaceWrite",
                "writableRoots": [cwd],
                "networkAccess": false,
                "excludeTmpdirEnvVar": false,
                "excludeSlashTmp": false
            })
        } else {
            json!({"type": "readOnly", "networkAccess": false})
        };
        let response = self
            .request(
                "command/exec",
                json!({
                    "command": request.command,
                    "processId": request.process_id,
                    "cwd": request.cwd,
                    "timeoutMs": request.timeout_ms,
                    "outputBytesCap": 1_048_576,
                    "sandboxPolicy": sandbox,
                }),
            )
            .await?;
        Ok(CodexCommandExecResult {
            exit_code: response
                .get("exitCode")
                .and_then(Value::as_i64)
                .unwrap_or(-1),
            stdout: response
                .get("stdout")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            stderr: response
                .get("stderr")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        })
    }

    pub async fn terminate_command(&self, process_id: &str) -> Result<(), CodexHarnessError> {
        self.request("command/exec/terminate", json!({"processId": process_id}))
            .await
            .map(drop)
    }

    pub async fn watch_path(
        &self,
        watch_id: &str,
        path: &str,
    ) -> Result<CodexFsWatch, CodexHarnessError> {
        let response = self
            .request("fs/watch", json!({"watchId": watch_id, "path": path}))
            .await?;
        Ok(CodexFsWatch {
            watch_id: watch_id.into(),
            path: response
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or(path)
                .to_owned(),
        })
    }

    pub async fn unwatch_path(&self, watch_id: &str) -> Result<(), CodexHarnessError> {
        self.request("fs/unwatch", json!({"watchId": watch_id}))
            .await
            .map(drop)
    }
}

fn is_approval_request(method: &str) -> bool {
    matches!(
        method,
        "item/commandExecution/requestApproval"
            | "item/fileChange/requestApproval"
            | "item/permissions/requestApproval"
    )
}

fn is_supported_server_request(method: &str) -> bool {
    protocol_generated::is_known_server_request(method)
        && (is_approval_request(method)
            || method == "mcpServer/elicitation/request"
            || method == "item/tool/requestUserInput")
}

fn is_visible_turn_notification(method: &str) -> bool {
    matches!(
        method,
        "turn/plan/updated"
            | "turn/diff/updated"
            | "item/plan/delta"
            | "item/fileChange/outputDelta"
            | "item/fileChange/patchUpdated"
            | "item/mcpToolCall/progress"
            | "item/reasoning/summaryTextDelta"
            | "item/reasoning/summaryPartAdded"
            | "thread/tokenUsage/updated"
            | "thread/compacted"
            | "warning"
            | "guardianWarning"
            | "model/rerouted"
    )
}

fn parse_approval_request(document: &Value) -> Result<CodexApprovalRequest, CodexHarnessError> {
    let method = document
        .get("method")
        .and_then(Value::as_str)
        .ok_or_else(|| CodexHarnessError::Protocol("approval request has no method".into()))?;
    let params = document.get("params").cloned().unwrap_or(Value::Null);
    let string = |name: &str| {
        params
            .get(name)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let resource = match method {
        "item/commandExecution/requestApproval" => params
            .get("command")
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| value.to_string())
            })
            .unwrap_or_else(|| "Run a command".into()),
        "item/fileChange/requestApproval" => params
            .get("grantRoot")
            .and_then(Value::as_str)
            .unwrap_or("Modify project files")
            .to_owned(),
        "item/permissions/requestApproval" => {
            let permissions = params.get("permissions").cloned().unwrap_or(Value::Null);
            if permissions
                .pointer("/network/enabled")
                .and_then(Value::as_bool)
                == Some(true)
            {
                "Allow network access for this turn".into()
            } else {
                permissions.to_string()
            }
        }
        _ => "Codex action".into(),
    };
    Ok(CodexApprovalRequest {
        request_id: document.get("id").cloned().unwrap_or(Value::Null),
        method: method.to_owned(),
        thread_id: string("threadId"),
        turn_id: string("turnId"),
        item_id: string("itemId"),
        reason: params
            .get("reason")
            .and_then(Value::as_str)
            .map(str::to_owned),
        resource,
        params,
    })
}

fn approval_response(request: &CodexApprovalRequest, approved: bool) -> Value {
    if request.method == "item/permissions/requestApproval" {
        return json!({
            "permissions": if approved {
                request.params.get("permissions").cloned().unwrap_or_else(|| json!({}))
            } else {
                json!({})
            },
            "scope": "turn"
        });
    }
    json!({"decision": if approved { "accept" } else { "decline" }})
}

fn thread_ids_with_source(response: &Value, thread_source: &str) -> Vec<String> {
    response
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|thread| thread_has_source(thread, thread_source))
        .filter_map(|thread| thread.get("id").and_then(Value::as_str).map(str::to_owned))
        .collect()
}

fn thread_has_source(thread: &Value, thread_source: &str) -> bool {
    if thread.get("threadSource").and_then(Value::as_str) == Some(thread_source) {
        return true;
    }
    // Codex 0.150.1 persists the analytics tag in rollout metadata but can omit it from
    // `thread/list`. Read only the bounded session-metadata line from the server-supplied local
    // rollout path; never infer ownership from titles, prompts, or working directories.
    let Some(path) = thread.get("path").and_then(Value::as_str) else {
        return false;
    };
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let mut first_line = String::new();
    let mut reader = std::io::BufReader::new(file).take(64 * 1024);
    let Ok(read) = reader.read_line(&mut first_line) else {
        return false;
    };
    if read == 0 || (read == 64 * 1024 && !first_line.ends_with('\n')) {
        return false;
    }
    serde_json::from_str::<Value>(&first_line)
        .ok()
        .and_then(|document| {
            document
                .pointer("/payload/thread_source")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .as_deref()
        == Some(thread_source)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn pinned_contract_and_unknown_fields_are_tolerated() {
        assert_eq!(CODEX_BINARY_VERSION, "0.150.1");
        let model: CodexModel = serde_json::from_value(json!({
            "id":"gpt-test","model":"gpt-test","displayName":"Test","description":"fixture",
            "hidden":false,"isDefault":true,"defaultReasoningEffort":"medium",
            "supportedReasoningEfforts":[],"serviceTiers":[],"futureField":true
        }))
        .expect("model fixture");
        assert!(model.is_default);
    }

    #[test]
    fn checked_in_schema_matches_the_generated_protocol_contract() {
        let schema = include_bytes!(
            "../../../protocol/codex-app-server/0.150.1/codex_app_server_protocol.v2.schemas.json"
        );
        assert_eq!(format!("{:x}", Sha256::digest(schema)), CODEX_SCHEMA_SHA256);
        assert_eq!(
            protocol_generated::GENERATED_CODEX_VERSION,
            CODEX_BINARY_VERSION
        );
        assert_eq!(
            protocol_generated::GENERATED_PROTOCOL_VERSION,
            CODEX_PROTOCOL_VERSION
        );
        assert_eq!(WINDOWS_SANDBOX_READINESS_METHOD, "windowsSandbox/readiness");
        let schema_text = String::from_utf8_lossy(schema);
        assert!(schema_text.contains(r#""windowsSandbox/readiness""#));
        assert!(!schema_text.contains(r#""windowsSandbox/readiness/read""#));
        assert!(protocol_generated::SERVER_NOTIFICATION_METHODS.contains(&"item/started"));
        assert!(
            protocol_generated::SERVER_REQUEST_METHODS.contains(&"mcpServer/elicitation/request")
        );
    }

    #[test]
    fn unsupported_requests_fail_closed_without_becoming_approvals() {
        assert!(is_supported_server_request("item/tool/requestUserInput"));
        assert!(!is_supported_server_request("future/unknown/request"));
        assert!(is_supported_server_request(
            "item/commandExecution/requestApproval"
        ));
    }

    #[test]
    fn recents_cleanup_selects_only_exact_mcg_owned_threads() {
        let rollout = std::env::temp_dir().join(format!(
            "mcg-thread-source-{}-{}.jsonl",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::write(
            &rollout,
            "{\"type\":\"session_meta\",\"payload\":{\"thread_source\":\"mcg\"}}\n",
        )
        .expect("rollout fixture");
        let response = json!({
            "data": [
                {"id": "mcg-chat", "threadSource": "mcg"},
                {"id": "ordinary-codex", "threadSource": "codex"},
                {"id": "other-integration", "threadSource": "other"},
                {"id": "mcg-coder", "threadSource": "mcg"},
                {"id": "mcg-pinned-fallback", "threadSource": null, "path": rollout},
                {"id": "missing-tag"}
            ]
        });
        assert_eq!(
            thread_ids_with_source(&response, "mcg"),
            vec!["mcg-chat", "mcg-coder", "mcg-pinned-fallback"]
        );
        std::fs::remove_file(rollout).expect("remove rollout fixture");
    }

    #[test]
    fn approval_requests_are_parsed_and_answered_by_type() {
        let request = parse_approval_request(&json!({
            "id": 42,
            "method": "item/permissions/requestApproval",
            "params": {
                "threadId": "thread-1",
                "turnId": "turn-1",
                "itemId": "item-1",
                "reason": "Download the starter package",
                "permissions": {"network": {"enabled": true}}
            }
        }))
        .expect("approval fixture");
        assert_eq!(request.resource, "Allow network access for this turn");
        assert_eq!(approval_response(&request, true)["scope"], "turn");
        assert_eq!(
            approval_response(&request, true)["permissions"]["network"]["enabled"],
            true
        );
        assert_eq!(approval_response(&request, false)["permissions"], json!({}));
    }

    #[test]
    fn approval_modes_map_to_native_codex_controls() {
        let ask = CodexExecutionPreferences {
            approval_mode: CodexApprovalMode::Ask,
            ..Default::default()
        };
        assert_eq!(ask.approval_policy(true), "on-request");
        assert_eq!(ask.approvals_reviewer(), "user");
        assert_eq!(ask.turn_sandbox(true), json!({"type": "workspaceWrite"}));

        let auto = CodexExecutionPreferences {
            approval_mode: CodexApprovalMode::AutoReview,
            ..Default::default()
        };
        assert_eq!(auto.approval_policy(true), "on-request");
        assert_eq!(auto.approvals_reviewer(), "auto_review");
        assert_eq!(auto.turn_sandbox(true), json!({"type": "workspaceWrite"}));

        let full = CodexExecutionPreferences {
            approval_mode: CodexApprovalMode::FullAccess,
            ..Default::default()
        };
        assert_eq!(full.approval_policy(true), "never");
        assert_eq!(full.approvals_reviewer(), "user");
        assert_eq!(full.turn_sandbox(true), json!({"type": "dangerFullAccess"}));

        assert_eq!(full.approval_policy(false), "never");
        assert_eq!(full.turn_sandbox(false), json!({"type": "readOnly"}));

        let exact = CodexExecutionPreferences {
            approval_mode: CodexApprovalMode::Ask,
            approval_policy: Some(CodexApprovalPolicy::Untrusted),
            sandbox_mode: Some(CodexSandboxMode::ReadOnly),
            ..Default::default()
        };
        assert_eq!(exact.approval_policy(true), "untrusted");
        assert_eq!(exact.turn_sandbox(true), json!({"type": "readOnly"}));

        let expanded = CodexExecutionPreferences {
            approval_mode: CodexApprovalMode::Ask,
            writable_roots: vec!["C:/shared-one".into(), "C:/shared-two".into()],
            ..Default::default()
        };
        assert_eq!(
            expanded.turn_sandbox(true),
            json!({
                "type": "workspaceWrite",
                "writableRoots": ["C:/shared-one", "C:/shared-two"]
            })
        );
        assert_eq!(expanded.turn_sandbox(false), json!({"type": "readOnly"}));
    }

    #[test]
    fn session_features_merge_beneath_role_authority_config() {
        let preferences = CodexExecutionPreferences {
            feature_overrides: BTreeMap::from([
                ("memories".into(), true),
                ("browser_use".into(), true),
            ]),
            ..Default::default()
        };
        let role = CodexThreadInstructions {
            config: Some(json!({
                "features": {"browser_use": false, "shell_tool": false},
                "custom": {"value": 1}
            })),
            ..Default::default()
        };
        let config = preferences
            .thread_config(Some(&role))
            .expect("merged config");
        assert_eq!(config["features"]["memories"], true);
        assert_eq!(config["features"]["browser_use"], false);
        assert_eq!(config["features"]["shell_tool"], false);
        assert_eq!(config["custom"]["value"], 1);
    }

    #[test]
    fn ordered_session_config_merges_beneath_features_and_role_authority() {
        let preferences = CodexExecutionPreferences {
            config_overrides: vec![
                CodexConfigOverride {
                    path: vec!["model_verbosity".into()],
                    value: json!("low"),
                },
                CodexConfigOverride {
                    path: vec!["history".into(), "persistence".into()],
                    value: json!("save-none"),
                },
                CodexConfigOverride {
                    path: vec!["model_verbosity".into()],
                    value: json!("high"),
                },
                CodexConfigOverride {
                    path: vec!["web_search".into()],
                    value: json!("live"),
                },
            ],
            feature_overrides: BTreeMap::from([("memories".into(), true)]),
            ..Default::default()
        };
        let role = CodexThreadInstructions {
            config: Some(json!({
                "history": {"persistence": "save-all"},
                "features": {"memories": false},
                "web_search": "cached"
            })),
            ..Default::default()
        };
        let config = preferences
            .thread_config(Some(&role))
            .expect("merged config");
        assert_eq!(config["model_verbosity"], "high");
        assert_eq!(config["history"]["persistence"], "save-all");
        assert_eq!(config["features"]["memories"], false);
        assert_eq!(config["web_search"], "cached");
    }

    #[test]
    fn two_agent_role_configs_are_thread_scoped_and_fail_closed() {
        let chat = CodexThreadInstructions::isolated_chat_config(
            "C:/MCG/mcg-agent-mcp.exe",
            "C:/MCG/bootstrap/one-use.json",
        );
        assert_eq!(chat["features"]["shell_tool"], false);
        assert_eq!(chat["features"]["unified_exec"], false);
        assert_eq!(chat["features"]["multi_agent"], false);
        assert_eq!(chat["features"]["browser_use"], false);
        assert_eq!(chat["features"]["computer_use"], false);
        assert_eq!(chat["features"]["apps"], false);
        assert_eq!(
            chat["mcp_servers"]["mcg_agent"]["args"],
            json!(["--bootstrap-file", "C:/MCG/bootstrap/one-use.json"])
        );

        let coordinator = CodexThreadInstructions::isolated_coordinator_config(
            "C:/MCG/mcg-agent-mcp.exe",
            "C:/MCG/bootstrap/coordinator.json",
        );
        assert_eq!(coordinator["features"]["shell_tool"], false);
        assert_eq!(coordinator["features"]["unified_exec"], false);
        assert_eq!(coordinator["features"]["multi_agent"], false);
        assert_eq!(coordinator["features"]["browser_use"], false);
        assert_eq!(coordinator["features"]["computer_use"], false);
        assert_eq!(coordinator["features"]["apps"], false);

        let coder = CodexThreadInstructions::isolated_coder_config(
            "C:/MCG/mcg-agent-mcp.exe",
            "C:/MCG/bootstrap/coder.json",
        );
        assert_eq!(coder["features"]["shell_tool"], true);
        assert_eq!(coder["features"]["multi_agent"], false);
        assert_eq!(coder["features"]["browser_use"], false);
        assert_eq!(coder["features"]["computer_use"], false);

        let site_coder = CodexThreadInstructions::isolated_site_coder_config(
            "C:/MCG/mcg-agent-mcp.exe",
            "C:/MCG/bootstrap/site-coder.json",
        );
        assert_eq!(site_coder["features"]["shell_tool"], true);
        assert_eq!(site_coder["features"]["apps"], true);
        assert_eq!(site_coder["features"]["plugins"], true);
        assert_eq!(site_coder["features"]["multi_agent"], false);
        assert_eq!(site_coder["features"]["browser_use"], false);
        assert_eq!(site_coder["features"]["computer_use"], false);
        assert_eq!(
            site_coder["plugins"]["sites@openai-bundled"]["enabled"],
            true
        );
        assert_eq!(
            site_coder["plugins"]["computer-use@openai-bundled"]["enabled"],
            false
        );

        let tester = CodexThreadInstructions::isolated_tester_config(
            "C:/MCG/mcg-agent-mcp.exe",
            "C:/MCG/bootstrap/tester.json",
        );
        assert_eq!(tester["features"]["shell_tool"], false);
        assert_eq!(tester["features"]["unified_exec"], false);
        assert_eq!(tester["features"]["multi_agent"], false);
        assert_eq!(tester["features"]["browser_use"], true);
        assert_eq!(tester["features"]["in_app_browser"], true);
        assert_eq!(tester["features"]["computer_use"], false);
        assert_eq!(tester["plugins"]["browser@openai-bundled"]["enabled"], true);
        assert_eq!(tester["plugins"]["chrome@openai-bundled"]["enabled"], false);
    }

    #[cfg(windows)]
    #[test]
    fn windows_process_initialization_failure_is_retryable_for_conformance() {
        use std::os::windows::process::ExitStatusExt;

        assert!(transient_process_initialization_failure(
            &std::process::ExitStatus::from_raw(0xC000_0142)
        ));
        assert!(!transient_process_initialization_failure(
            &std::process::ExitStatus::from_raw(1)
        ));
    }

    #[test]
    #[ignore = "qualification gate: requires the packaged pinned Codex binary on PATH"]
    fn installed_pinned_binary_conforms_to_two_agent_boundaries() {
        let executable = std::env::var_os("MCG_CODEX_APP_SERVER_EXECUTABLE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("codex"));
        verify_pinned_two_agent_conformance(&executable)
            .expect("pinned Codex two-agent conformance");
    }

    #[tokio::test]
    #[ignore = "qualification gate: requires the packaged pinned Codex binary and account context"]
    async fn installed_pinned_binary_exposes_stable_app_discovery() {
        let executable = std::env::var_os("MCG_CODEX_APP_SERVER_EXECUTABLE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("codex"));
        let harness = CodexAppServerHarness::new(executable);
        let discovery = harness
            .stable_discovery(&[], true)
            .await
            .expect("stable Codex discovery");
        assert!(discovery.apps_available);
        assert!(discovery.apps_unavailable_reason.is_none());
        assert!(!discovery.apps.is_empty());
    }

    #[test]
    fn stable_review_and_command_payloads_match_the_pinned_schema() {
        assert_eq!(
            serde_json::to_value(CodexReviewTarget::UncommittedChanges).unwrap(),
            json!({"type":"uncommittedChanges"})
        );
        assert_eq!(
            serde_json::to_value(CodexReviewTarget::Commit {
                sha: "abc123".into(),
                title: Some("Review this".into()),
            })
            .unwrap(),
            json!({"type":"commit","sha":"abc123","title":"Review this"})
        );
        assert_eq!(
            serde_json::to_value(CodexReviewDelivery::Detached).unwrap(),
            json!("detached")
        );
        let command: CodexCommandExecRequest = serde_json::from_value(json!({
            "command":["git","status"],
            "cwd":"C:/workspace",
            "timeoutMs":30000,
            "writable":false
        }))
        .unwrap();
        assert_eq!(command.command, vec!["git", "status"]);
        assert!(!command.writable);
    }

    #[test]
    fn v031_uses_only_stable_pinned_methods() {
        let schema = include_str!(
            "../../../protocol/codex-app-server/0.150.1/codex_app_server_protocol.v2.schemas.json"
        );
        for method in [
            "review/start",
            "skills/list",
            "hooks/list",
            "app/list",
            "app/read",
            "fs/watch",
            "fs/unwatch",
            "command/exec",
        ] {
            assert!(
                schema.contains(&format!("\"{method}\"")),
                "missing stable pinned request {method}"
            );
        }
        assert!(protocol_generated::SERVER_NOTIFICATION_METHODS.contains(&"fs/changed"));
        assert!(protocol_generated::SERVER_NOTIFICATION_METHODS.contains(&"skills/changed"));
    }

    #[test]
    fn structured_mentions_preserve_text_elements_and_native_inputs() {
        let input = codex_user_input(
            "open @src",
            &[CodexMentionInput {
                name: "src".into(),
                path: "C:/workspace/src".into(),
                byte_start: 5,
                byte_end: 9,
                placeholder: "@src".into(),
            }],
        );
        assert_eq!(
            input,
            vec![
                json!({"type":"text","text":"open @src","text_elements":[{"byteRange":{"start":5,"end":9},"placeholder":"@src"}]}),
                json!({"type":"mention","name":"src","path":"C:/workspace/src"}),
            ]
        );
    }

    #[test]
    fn experimental_feature_payloads_match_the_pinned_schema() {
        let page: CodexFeatureList = serde_json::from_value(json!({
            "data": [{
                "name": "memories",
                "stage": "beta",
                "displayName": "Memories",
                "description": "Use durable memories.",
                "announcement": null,
                "enabled": true,
                "defaultEnabled": false
            }],
            "nextCursor": "next"
        }))
        .unwrap();
        assert_eq!(page.data[0].stage, CodexFeatureStage::Beta);
        assert_eq!(page.data[0].display_name.as_deref(), Some("Memories"));
        assert!(page.data[0].enabled);
        assert_eq!(page.next_cursor.as_deref(), Some("next"));

        let schema = include_str!(
            "../../../protocol/codex-app-server/0.150.1/codex_app_server_protocol.v2.schemas.json"
        );
        assert!(schema.contains("\"experimentalFeature/list\""));
        assert!(schema.contains("\"experimentalFeature/enablement/set\""));
    }
}
