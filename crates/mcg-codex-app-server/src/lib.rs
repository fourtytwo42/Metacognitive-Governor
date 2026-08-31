//! Stable JSONL/stdio client for the pinned Codex app-server runtime.
//!
//! This is deliberately a conversation/thread harness, not a `ModelProvider`.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{BufRead as _, Read as _},
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::Duration,
};

use async_tungstenite::{
    WebSocketStream,
    tokio::connect_async,
    tungstenite::{
        Message,
        client::IntoClientRequest,
        http::{HeaderValue, header::AUTHORIZATION},
    },
};
use chrono::{DateTime, Utc};
use futures_util::StreamExt as _;
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, Command},
    sync::{Mutex, broadcast, mpsc, oneshot},
    task::JoinHandle,
};
use url::Url;
use zeroize::{Zeroize, Zeroizing};

mod protocol_generated;

pub use protocol_generated::{
    GENERATED_SCHEMA_SHA256 as CODEX_SCHEMA_SHA256, RequestId, ServerNotification, ServerRequest,
    ServerResponse,
};

pub const CODEX_BINARY_VERSION: &str = protocol_generated::GENERATED_CODEX_VERSION;
pub const CODEX_PROTOCOL_VERSION: &str = protocol_generated::GENERATED_PROTOCOL_VERSION;
const WINDOWS_SANDBOX_READINESS_METHOD: &str = "windowsSandbox/readiness";
const WINDOWS_SANDBOX_SETUP_START_METHOD: &str = "windowsSandbox/setupStart";
const WINDOWS_SANDBOX_SETUP_COMPLETED_METHOD: &str = "windowsSandbox/setupCompleted";

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
    pub local_providers: CodexLocalProviderDiscovery,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexLocalProviderDiscovery {
    pub configured_provider: Option<String>,
    pub providers: Vec<CodexLocalProviderStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexLocalProviderStatus {
    pub id: String,
    pub display_name: String,
    pub default_model: String,
    pub endpoint: String,
    pub running: bool,
    pub responses_supported: bool,
    pub version: Option<String>,
    pub models: Vec<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodexWindowsSandboxSetupMode {
    Elevated,
    Unelevated,
}

impl CodexWindowsSandboxSetupMode {
    fn protocol_name(self) -> &'static str {
        match self {
            Self::Elevated => "elevated",
            Self::Unelevated => "unelevated",
        }
    }
}

fn windows_sandbox_config_write_request(mode: CodexWindowsSandboxSetupMode) -> Value {
    json!({
        "edits": [
            {"keyPath": "windows.sandbox", "value": mode.protocol_name(), "mergeStrategy": "replace"},
            {"keyPath": "features.experimental_windows_sandbox", "value": null, "mergeStrategy": "replace"},
            {"keyPath": "features.elevated_windows_sandbox", "value": null, "mergeStrategy": "replace"},
            {"keyPath": "features.enable_experimental_windows_sandbox", "value": null, "mergeStrategy": "replace"}
        ],
        "filePath": null,
        "expectedVersion": null,
        "reloadUserConfig": true
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexWindowsSandboxSetupResult {
    pub mode: CodexWindowsSandboxSetupMode,
    pub readiness: String,
    pub config_write_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexWindowsSandboxReadGrant {
    pub path: String,
    pub permission_profile: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexResumeCwd {
    Current,
    Session,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexStableDiscovery {
    pub skills: Vec<Value>,
    pub hooks: Vec<CodexHooksListEntry>,
    #[serde(default)]
    pub apps: Vec<Value>,
    pub apps_available: bool,
    pub apps_unavailable_reason: Option<String>,
    pub refreshed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexHooksListEntry {
    pub cwd: String,
    #[serde(default)]
    pub hooks: Vec<CodexHookMetadata>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub errors: Vec<CodexHookError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexHookError {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexHookMetadata {
    pub key: String,
    pub event_name: String,
    pub handler_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#async: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    pub matcher: Option<String>,
    pub timeout_sec: u64,
    pub status_message: Option<String>,
    pub additional_context_limit: Option<usize>,
    pub source_path: String,
    pub source: String,
    pub plugin_id: Option<String>,
    pub display_order: i64,
    pub enabled: bool,
    pub is_managed: bool,
    pub current_hash: String,
    pub trust_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexHookStateUpdate {
    pub cwd: String,
    pub key: String,
    pub expected_current_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub trust_current_definition: bool,
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

/// Content-minimized projection of one Codex sub-agent thread returned by app-server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexRemoteAgentSummary {
    pub thread_id: String,
    pub session_id: String,
    pub parent_thread_id: String,
    pub nickname: Option<String>,
    pub role: Option<String>,
    pub status: String,
    pub can_accept_direct_input: Option<bool>,
    pub updated_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexRemoteAgentInventory {
    pub agents: Vec<CodexRemoteAgentSummary>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CodexTransportConfig {
    #[default]
    LocalStdio,
    RemoteWebSocket {
        endpoint: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        auth_token_env: Option<String>,
    },
    RemoteUnixSocket {
        socket_path: String,
    },
}

impl CodexTransportConfig {
    #[must_use]
    pub fn remote(endpoint: String, auth_token_env: Option<String>) -> Self {
        Self::RemoteWebSocket {
            endpoint,
            auth_token_env,
        }
    }

    #[must_use]
    pub const fn is_remote(&self) -> bool {
        matches!(
            self,
            Self::RemoteWebSocket { .. } | Self::RemoteUnixSocket { .. }
        )
    }

    #[must_use]
    pub fn endpoint(&self) -> Option<&str> {
        match self {
            Self::LocalStdio => None,
            Self::RemoteWebSocket { endpoint, .. } => Some(endpoint),
            Self::RemoteUnixSocket { socket_path } => Some(socket_path),
        }
    }

    #[must_use]
    pub fn auth_token_env(&self) -> Option<&str> {
        match self {
            Self::LocalStdio => None,
            Self::RemoteWebSocket { auth_token_env, .. } => auth_token_env.as_deref(),
            Self::RemoteUnixSocket { .. } => None,
        }
    }
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
    pub transport: CodexTransportConfig,
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
            transport: CodexTransportConfig::default(),
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexPermissionProfile {
    pub id: String,
    pub description: Option<String>,
    pub allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexPermissionProfileList {
    data: Vec<CodexPermissionProfile>,
    next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CodexConfigOverride {
    pub path: Vec<String>,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CodexExecutionPreferences {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_provider: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub service_tier: Option<String>,
    #[serde(default)]
    pub approval_mode: CodexApprovalMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<CodexApprovalPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox_mode: Option<CodexSandboxMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_profile: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub writable_roots: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub feature_overrides: BTreeMap<String, bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub config_overrides: Vec<CodexConfigOverride>,
    /// A validated, sanitized snapshot of the selected Codex profile. The runtime owns this
    /// field; callers cannot use it to bypass `MetaCog`'s configuration boundary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_config: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexResolvedProfile {
    pub config: Value,
    pub digest: String,
    pub ignored_keys: Vec<String>,
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

fn turn_start_params(
    thread_id: &str,
    prompt: &str,
    mentions: &[CodexMentionInput],
    preferences: &CodexExecutionPreferences,
    writable: bool,
    output_schema: Option<&Value>,
    plan_mode: bool,
) -> Value {
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
    json!({
        "threadId": thread_id,
        "input": codex_user_input(prompt, mentions),
        "model": preferences.model,
        "effort": preferences.effort,
        "serviceTier": preferences.service_tier,
        "approvalPolicy": preferences.approval_policy(writable),
        "approvalsReviewer": preferences.approvals_reviewer(),
        "sandboxPolicy": preferences.turn_sandbox(writable),
        "permissions": preferences.effective_permission_profile(writable),
        "outputSchema": output_schema,
        "collaborationMode": collaboration_mode
    })
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
                // This bridge is not an owner approval boundary: every invocation is pinned to
                // MCG's executable, one role bootstrap, and a loopback authorization check. Let
                // Codex call only that already-governed surface without weakening the thread's
                // read-only filesystem sandbox or its ordinary approval policy.
                "default_tools_approval_mode": "approve",
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
        let mut config = self
            .profile_config
            .clone()
            .unwrap_or_else(|| Value::Object(Default::default()));
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

    fn effective_permission_profile(&self, writable: bool) -> Option<&str> {
        writable
            .then_some(self.permission_profile.as_deref())
            .flatten()
    }

    fn thread_sandbox(&self, writable: bool) -> Option<&'static str> {
        if self.effective_permission_profile(writable).is_some() {
            return None;
        }
        Some(match self.effective_sandbox(writable) {
            CodexSandboxMode::ReadOnly => "read-only",
            CodexSandboxMode::WorkspaceWrite => "workspace-write",
            CodexSandboxMode::DangerFullAccess => "danger-full-access",
        })
    }

    fn turn_sandbox(&self, writable: bool) -> Option<Value> {
        if self.effective_permission_profile(writable).is_some() {
            return None;
        }
        Some(match self.effective_sandbox(writable) {
            CodexSandboxMode::ReadOnly => json!({"type": "readOnly"}),
            CodexSandboxMode::WorkspaceWrite if self.writable_roots.is_empty() => {
                json!({"type": "workspaceWrite"})
            }
            CodexSandboxMode::WorkspaceWrite => json!({
                "type": "workspaceWrite",
                "writableRoots": self.writable_roots
            }),
            CodexSandboxMode::DangerFullAccess => json!({"type": "dangerFullAccess"}),
        })
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

fn validate_profile_name(profile: &str) -> Result<(), CodexHarnessError> {
    if profile.is_empty()
        || profile.len() > 128
        || matches!(profile, "." | "..")
        || !profile
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(CodexHarnessError::Protocol(
            "Codex profile must use 1-128 letters, numbers, dashes, underscores, or dots".into(),
        ));
    }
    Ok(())
}

fn validate_hook_state_update(update: &CodexHookStateUpdate) -> Result<(), CodexHarnessError> {
    if update.cwd.is_empty() || update.cwd.len() > 4_096 || update.cwd.contains('\0') {
        return Err(CodexHarnessError::InvalidRequest(
            "hook cwd must be a non-empty bounded path".into(),
        ));
    }
    if update.key.is_empty() || update.key.len() > 2_048 || update.key.contains('\0') {
        return Err(CodexHarnessError::InvalidRequest(
            "hook key must be non-empty and at most 2,048 bytes".into(),
        ));
    }
    let Some(digest) = update.expected_current_hash.strip_prefix("sha256:") else {
        return Err(CodexHarnessError::InvalidRequest(
            "hook current hash must use Codex's sha256:<64 hex> identity".into(),
        ));
    };
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(CodexHarnessError::InvalidRequest(
            "hook current hash must use Codex's sha256:<64 hex> identity".into(),
        ));
    }
    Ok(())
}

fn codex_home() -> Result<PathBuf, CodexHarnessError> {
    if let Some(root) = std::env::var_os("CODEX_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(root));
    }
    #[cfg(windows)]
    let home = std::env::var_os("USERPROFILE");
    #[cfg(not(windows))]
    let home = std::env::var_os("HOME");
    home.map(|value| PathBuf::from(value).join(".codex"))
        .ok_or_else(|| CodexHarnessError::Protocol("Codex home is unavailable".into()))
}

fn governed_profile_segment(segment: &str, top_level: bool) -> bool {
    let normalized = segment.to_ascii_lowercase().replace('-', "_");
    let secret = [
        "api_key",
        "auth",
        "bearer",
        "credential",
        "password",
        "secret",
        "token",
    ]
    .iter()
    .any(|candidate| normalized.contains(candidate));
    if secret {
        return true;
    }
    top_level
        && [
            "agent",
            "agents",
            "app",
            "apps",
            "approval",
            "approval_policy",
            "browser",
            "computer_use",
            "developer_instructions",
            "dynamic_tools",
            "experimental_user_input",
            "features",
            "hooks",
            "instructions",
            "mcp_servers",
            "model_provider",
            "model_providers",
            "network_access",
            "oss_provider",
            "permissions",
            "plugin",
            "plugins",
            "process",
            "remote",
            "rules",
            "sandbox",
            "sandbox_mode",
            "shell_environment_policy",
            "skills",
            "show_raw_agent_reasoning",
            "tools",
            "user_instructions",
            "workspace_write",
        ]
        .contains(&normalized.as_str())
}

fn sanitize_profile_config(value: &mut Value, path: &mut Vec<String>, ignored: &mut Vec<String>) {
    let Value::Object(object) = value else {
        return;
    };
    let keys = object.keys().cloned().collect::<Vec<_>>();
    for key in keys {
        let remove = governed_profile_segment(&key, path.is_empty());
        path.push(key.clone());
        if remove {
            ignored.push(path.join("."));
            object.remove(&key);
        } else if let Some(child) = object.get_mut(&key) {
            sanitize_profile_config(child, path, ignored);
        }
        path.pop();
    }
}

fn parse_resume_cwd(value: Option<&Value>) -> Result<Option<CodexResumeCwd>, CodexHarnessError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if value == "current" => Ok(Some(CodexResumeCwd::Current)),
        Some(Value::String(value)) if value == "session" => Ok(Some(CodexResumeCwd::Session)),
        Some(value) => Err(CodexHarnessError::Protocol(format!(
            "Codex configuration returned invalid tui.resume_cwd value {value}"
        ))),
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexCloudApplyResult {
    pub stdout: String,
    pub stderr: String,
}

fn cloud_apply_arguments(
    task_id: &str,
    config_overrides: &[String],
    enable_features: &[String],
    disable_features: &[String],
) -> Vec<String> {
    let mut arguments = vec!["apply".to_owned()];
    for value in config_overrides {
        arguments.push("-c".to_owned());
        arguments.push(value.clone());
    }
    for value in enable_features {
        arguments.push("--enable".to_owned());
        arguments.push(value.clone());
    }
    for value in disable_features {
        arguments.push("--disable".to_owned());
        arguments.push(value.clone());
    }
    arguments.push(task_id.to_owned());
    arguments
}

struct Process {
    child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    #[cfg(windows)]
    _job: win32job::Job,
}

struct RemoteConnection {
    outbound: mpsc::Sender<String>,
    task: JoinHandle<()>,
}

impl Drop for RemoteConnection {
    fn drop(&mut self) {
        self.task.abort();
    }
}

enum Connection {
    Local(Process),
    Remote(RemoteConnection),
}

fn stop_connection(inner: &mut Inner) {
    if let Some(Connection::Remote(remote)) = inner.connection.take() {
        remote.task.abort();
    } else {
        inner.connection = None;
    }
    inner.status.process_state = CodexProcessState::Stopped;
    inner.status.initialized = false;
}

fn validate_login_credential(value: &str, label: &str) -> Result<(), CodexHarnessError> {
    if value.is_empty() || value.len() > 16_384 || value.chars().any(char::is_control) {
        return Err(CodexHarnessError::InvalidRequest(format!(
            "{label} is empty, oversized, or contains control characters"
        )));
    }
    Ok(())
}

pub fn validate_transport(transport: &CodexTransportConfig) -> Result<(), CodexHarnessError> {
    let (endpoint, auth_token_env) = match transport {
        CodexTransportConfig::LocalStdio => return Ok(()),
        CodexTransportConfig::RemoteUnixSocket { socket_path } => {
            if socket_path.is_empty()
                || socket_path.len() > 2_048
                || socket_path.chars().any(char::is_control)
                || !Path::new(socket_path).is_absolute()
            {
                return Err(CodexHarnessError::InvalidRequest(
                    "remote Codex Unix socket path must be an absolute printable path of at most 2048 bytes"
                        .into(),
                ));
            }
            return Ok(());
        }
        CodexTransportConfig::RemoteWebSocket {
            endpoint,
            auth_token_env,
        } => (endpoint, auth_token_env),
    };
    if endpoint.len() > 2_048 || endpoint.chars().any(char::is_control) {
        return Err(CodexHarnessError::InvalidRequest(
            "remote Codex endpoint must be at most 2048 printable bytes".into(),
        ));
    }
    let parsed = Url::parse(endpoint).map_err(|error| {
        CodexHarnessError::InvalidRequest(format!("remote Codex endpoint is invalid: {error}"))
    })?;
    if !matches!(parsed.scheme(), "ws" | "wss")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.fragment().is_some()
    {
        return Err(CodexHarnessError::InvalidRequest(
            "remote Codex endpoint must be a credential-free ws:// or wss:// URL".into(),
        ));
    }
    if parsed.scheme() == "ws" {
        let host = parsed.host_str().unwrap_or_default();
        let loopback = host.eq_ignore_ascii_case("localhost")
            || host
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback());
        if !loopback {
            return Err(CodexHarnessError::InvalidRequest(
                "unencrypted ws:// Codex endpoints are limited to loopback or SSH-forwarded connections; use wss:// remotely"
                    .into(),
            ));
        }
    }
    if let Some(name) = auth_token_env {
        let mut chars = name.chars();
        let valid_start = chars
            .next()
            .is_some_and(|character| character == '_' || character.is_ascii_alphabetic());
        if !valid_start
            || name.len() > 128
            || !chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
        {
            return Err(CodexHarnessError::InvalidRequest(
                "remote auth token environment variable name is invalid".into(),
            ));
        }
    }
    Ok(())
}

fn remote_websocket_has_explicit_port(endpoint: &str, parsed: &Url) -> bool {
    let Some(host) = parsed.host_str() else {
        return false;
    };
    if parsed.port().is_some() {
        return true;
    }
    let Some((_, rest)) = endpoint.split_once("://") else {
        return false;
    };
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let host_and_port = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host_and_port)| host_and_port);
    let default_port = match parsed.scheme() {
        "ws" => 80,
        "wss" => 443,
        _ => return false,
    };
    let expected_host = if host.contains(':') {
        format!("[{host}]")
    } else {
        host.to_owned()
    };
    host_and_port == format!("{expected_host}:{default_port}")
}

/// Validates the narrower per-invocation CLI transport contract independently of the caller.
pub fn validate_invocation_remote_transport(
    endpoint: &str,
    auth_token: Option<&str>,
) -> Result<(), CodexHarnessError> {
    if endpoint.starts_with("unix://") {
        let _ = normalize_invocation_remote_endpoint(endpoint, auth_token)?;
        return Ok(());
    }
    validate_transport(&CodexTransportConfig::remote(endpoint.to_owned(), None))?;
    let parsed = Url::parse(endpoint).map_err(|error| {
        CodexHarnessError::InvalidRequest(format!("remote Codex endpoint is invalid: {error}"))
    })?;
    if !remote_websocket_has_explicit_port(endpoint, &parsed)
        || parsed.path() != "/"
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(CodexHarnessError::InvalidRequest(
            "invocation remote endpoints must use ws://host:port or wss://host:port at the root path"
                .into(),
        ));
    }
    if let Some(token) = auth_token {
        validate_login_credential(token, "remote Codex bearer token")?;
        if parsed.scheme() != "wss" {
            let host = parsed.host_str().unwrap_or_default();
            let loopback = host.eq_ignore_ascii_case("localhost")
                || host
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|address| address.is_loopback());
            if !loopback {
                return Err(CodexHarnessError::InvalidRequest(
                    "remote Codex bearer tokens require wss:// or loopback ws://".into(),
                ));
            }
        }
    }
    Ok(())
}

/// Resolves Codex's invocation-local remote syntax without persisting credentials or relying on
/// the daemon's working directory for relative Unix socket paths.
pub fn normalize_invocation_remote_endpoint(
    endpoint: &str,
    auth_token: Option<&str>,
) -> Result<String, CodexHarnessError> {
    let endpoint = endpoint.trim();
    if endpoint.starts_with("unix://") {
        if auth_token.is_some() {
            return Err(CodexHarnessError::InvalidRequest(
                "remote Codex bearer tokens are unavailable for Unix sockets".into(),
            ));
        }
        #[cfg(windows)]
        return Err(CodexHarnessError::InvalidRequest(
            "unix:// remote app servers are unavailable on Windows".into(),
        ));
        #[cfg(not(windows))]
        {
            let socket_path = endpoint
                .strip_prefix("unix://")
                .expect("Unix endpoint prefix checked");
            let socket_path = if socket_path.is_empty() {
                codex_home()?
                    .join("app-server-control")
                    .join("app-server-control.sock")
            } else {
                let path = PathBuf::from(socket_path);
                if path.is_absolute() {
                    path
                } else {
                    std::env::current_dir()?.join(path)
                }
            };
            let rendered = socket_path.to_str().ok_or_else(|| {
                CodexHarnessError::InvalidRequest(
                    "remote Codex Unix socket path must be valid Unicode".into(),
                )
            })?;
            validate_transport(&CodexTransportConfig::RemoteUnixSocket {
                socket_path: rendered.to_owned(),
            })?;
            return Ok(format!("unix://{rendered}"));
        }
    }
    validate_invocation_remote_transport(endpoint, auth_token)?;
    Ok(endpoint.to_owned())
}

async fn dispatch_incoming_document(
    document: Value,
    pending: &Arc<Mutex<BTreeMap<u64, oneshot::Sender<Result<Value, String>>>>>,
    notifications: &broadcast::Sender<Value>,
    server_requests: &Arc<Mutex<BTreeMap<String, CodexPendingRequest>>>,
    notification_telemetry: &Arc<Mutex<(Option<String>, Option<String>)>>,
) {
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

async fn connect_remote(
    endpoint: &str,
    auth_token_env: Option<&str>,
    invocation_auth_token: Option<&str>,
    pending: Arc<Mutex<BTreeMap<u64, oneshot::Sender<Result<Value, String>>>>>,
    notifications: broadcast::Sender<Value>,
    server_requests: Arc<Mutex<BTreeMap<String, CodexPendingRequest>>>,
    notification_telemetry: Arc<Mutex<(Option<String>, Option<String>)>>,
) -> Result<RemoteConnection, CodexHarnessError> {
    let config =
        CodexTransportConfig::remote(endpoint.to_owned(), auth_token_env.map(str::to_owned));
    validate_transport(&config)?;
    let mut request = endpoint.into_client_request().map_err(|error| {
        CodexHarnessError::InvalidRequest(format!("remote Codex request is invalid: {error}"))
    })?;
    if auth_token_env.is_some() && invocation_auth_token.is_some() {
        return Err(CodexHarnessError::InvalidRequest(
            "remote Codex authentication must use either a configured environment variable or an invocation token"
                .into(),
        ));
    }
    if let Some(token_source) = invocation_auth_token
        .map(str::to_owned)
        .or_else(|| auth_token_env.map(|name| std::env::var(name).unwrap_or_default()))
    {
        let mut token = token_source;
        if let Some(name) = auth_token_env
            && token.is_empty()
        {
            return Err(CodexHarnessError::InvalidRequest(format!(
                "remote Codex bearer token environment variable `{name}` is missing"
            )));
        }
        if token.is_empty() || token.len() > 16_384 || token.chars().any(char::is_control) {
            token.zeroize();
            return Err(CodexHarnessError::InvalidRequest(
                "remote Codex bearer token is empty, oversized, or contains control characters"
                    .into(),
            ));
        }
        let mut authorization = Vec::with_capacity("Bearer ".len() + token.len());
        authorization.extend_from_slice(b"Bearer ");
        authorization.extend_from_slice(token.as_bytes());
        token.zeroize();
        let header_result = HeaderValue::from_bytes(&authorization);
        authorization.zeroize();
        let header = header_result.map_err(|error| {
            CodexHarnessError::InvalidRequest(format!(
                "remote Codex bearer token cannot be used in an HTTP header: {error}"
            ))
        })?;
        request.headers_mut().insert(AUTHORIZATION, header);
    }
    let (stream, _) = tokio::time::timeout(Duration::from_secs(20), connect_async(request))
        .await
        .map_err(|_| CodexHarnessError::Timeout)?
        .map_err(|error| CodexHarnessError::Start(format!("remote WebSocket failed: {error}")))?;
    Ok(remote_connection_from_stream(
        stream,
        pending,
        notifications,
        server_requests,
        notification_telemetry,
    ))
}

fn remote_connection_from_stream<S>(
    stream: WebSocketStream<S>,
    pending: Arc<Mutex<BTreeMap<u64, oneshot::Sender<Result<Value, String>>>>>,
    notifications: broadcast::Sender<Value>,
    server_requests: Arc<Mutex<BTreeMap<String, CodexPendingRequest>>>,
    notification_telemetry: Arc<Mutex<(Option<String>, Option<String>)>>,
) -> RemoteConnection
where
    S: futures_util::AsyncRead + futures_util::AsyncWrite + Unpin + Send + 'static,
{
    let (mut sink, mut source) = stream.split();
    let (outbound, mut outbound_rx) = mpsc::channel::<String>(256);
    let task = tokio::spawn(async move {
        loop {
            tokio::select! {
                outbound = outbound_rx.recv() => {
                    let Some(outbound) = outbound else { break; };
                    if sink.send(Message::Text(outbound.into())).await.is_err() {
                        break;
                    }
                }
                inbound = source.next() => {
                    let Some(Ok(inbound)) = inbound else { break; };
                    match inbound {
                        Message::Text(text) => {
                            if let Ok(document) = serde_json::from_str::<Value>(text.as_str()) {
                                dispatch_incoming_document(
                                    document,
                                    &pending,
                                    &notifications,
                                    &server_requests,
                                    &notification_telemetry,
                                ).await;
                            }
                        }
                        Message::Close(_) => break,
                        Message::Ping(payload) => {
                            if sink.send(Message::Pong(payload)).await.is_err() { break; }
                        }
                        Message::Binary(_) | Message::Pong(_) | Message::Frame(_) => {}
                    }
                }
            }
        }
        let mut pending = pending.lock().await;
        for (_, sender) in std::mem::take(&mut *pending) {
            let _ = sender.send(Err("Codex app-server stopped".into()));
        }
    });
    RemoteConnection { outbound, task }
}

#[cfg(unix)]
async fn connect_remote_unix(
    socket_path: &str,
    pending: Arc<Mutex<BTreeMap<u64, oneshot::Sender<Result<Value, String>>>>>,
    notifications: broadcast::Sender<Value>,
    server_requests: Arc<Mutex<BTreeMap<String, CodexPendingRequest>>>,
    notification_telemetry: Arc<Mutex<(Option<String>, Option<String>)>>,
) -> Result<RemoteConnection, CodexHarnessError> {
    let socket = tokio::time::timeout(
        Duration::from_secs(20),
        tokio::net::UnixStream::connect(socket_path),
    )
    .await
    .map_err(|_| CodexHarnessError::Timeout)??;
    let (stream, _) = tokio::time::timeout(
        Duration::from_secs(20),
        async_tungstenite::tokio::client_async("ws://localhost/rpc", socket),
    )
    .await
    .map_err(|_| CodexHarnessError::Timeout)?
    .map_err(|error| CodexHarnessError::Start(format!("remote Unix WebSocket failed: {error}")))?;
    Ok(remote_connection_from_stream(
        stream,
        pending,
        notifications,
        server_requests,
        notification_telemetry,
    ))
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
    connection: Option<Connection>,
    transport: CodexTransportConfig,
    /// A per-invocation bearer value supplied by an authenticated local client. It is never
    /// projected into status, settings, logs, or serialized request records.
    invocation_auth_token: Option<Zeroizing<String>>,
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
        Self::new_with_transport(executable, CodexTransportConfig::default())
    }

    #[must_use]
    pub fn new_with_transport(
        executable: impl Into<PathBuf>,
        transport: CodexTransportConfig,
    ) -> Self {
        let (notifications, _) = broadcast::channel(1_024);
        let mut status = CodexHarnessStatus::default();
        status.transport = transport.clone();
        Self {
            inner: Arc::new(Mutex::new(Inner {
                executable: executable.into(),
                connection: None,
                transport,
                invocation_auth_token: None,
                next_id: 1,
                pending: Arc::new(Mutex::new(BTreeMap::new())),
                notifications,
                status,
                server_requests: Arc::new(Mutex::new(BTreeMap::new())),
                notification_telemetry: Arc::new(Mutex::new((None, None))),
            })),
        }
    }

    /// Creates an invocation-scoped remote harness without persisting or exposing its bearer
    /// value. The caller must already have validated that the endpoint supports authentication.
    #[must_use]
    pub fn new_with_invocation_remote(
        executable: impl Into<PathBuf>,
        endpoint: String,
        auth_token: Option<Zeroizing<String>>,
    ) -> Result<Self, CodexHarnessError> {
        let endpoint = normalize_invocation_remote_endpoint(
            &endpoint,
            auth_token.as_deref().map(String::as_str),
        )?;
        let transport = endpoint.strip_prefix("unix://").map_or_else(
            || CodexTransportConfig::remote(endpoint.clone(), None),
            |socket_path| CodexTransportConfig::RemoteUnixSocket {
                socket_path: socket_path.to_owned(),
            },
        );
        let harness = Self::new_with_transport(executable, transport);
        if let Ok(mut inner) = harness.inner.try_lock() {
            inner.invocation_auth_token = auth_token;
        }
        Ok(harness)
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

    /// Applies a cloud-task diff only inside the caller-provided isolated workspace.
    ///
    /// The caller owns baseline capture, integration, and rollback. This helper deliberately
    /// exposes no arbitrary argv: only Codex's documented `apply` task identifier and already
    /// validated configuration/feature overrides can reach the pinned executable.
    ///
    /// # Errors
    ///
    /// Returns an error when the task identifier or override counts are invalid, the pinned
    /// executable cannot start, the bounded operation times out, or Codex rejects the apply.
    pub async fn apply_cloud_diff(
        &self,
        cwd: &Path,
        task_id: &str,
        config_overrides: &[String],
        enable_features: &[String],
        disable_features: &[String],
    ) -> Result<CodexCloudApplyResult, CodexHarnessError> {
        let task_id = task_id.trim();
        if task_id.is_empty()
            || task_id.len() > 256
            || task_id.starts_with('-')
            || task_id.chars().any(char::is_control)
        {
            return Err(CodexHarnessError::Protocol(
                "Codex cloud task ID must contain 1 to 256 non-control characters and cannot start with '-'"
                    .into(),
            ));
        }
        if config_overrides.len() > 32
            || enable_features.len().saturating_add(disable_features.len()) > 32
        {
            return Err(CodexHarnessError::Protocol(
                "Codex cloud apply override count exceeds the supported limit".into(),
            ));
        }
        let executable = self.inner.lock().await.executable.clone();
        let mut command = Command::new(executable);
        command
            .args(cloud_apply_arguments(
                task_id,
                config_overrides,
                enable_features,
                disable_features,
            ))
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let output = tokio::time::timeout(Duration::from_secs(300), command.output())
            .await
            .map_err(|_| CodexHarnessError::Timeout)?
            .map_err(|error| CodexHarnessError::Start(error.to_string()))?;
        let bounded = |bytes: &[u8]| {
            let value = String::from_utf8_lossy(bytes);
            let mut value = value.chars().take(16_384).collect::<String>();
            if bytes.len() > 16_384 {
                value.push_str("\n[output truncated]");
            }
            value
        };
        let stdout = bounded(&output.stdout);
        let stderr = bounded(&output.stderr);
        if !output.status.success() {
            let diagnostic = if stderr.trim().is_empty() {
                format!("Codex cloud apply exited with {}", output.status)
            } else {
                stderr.clone()
            };
            return Err(CodexHarnessError::Protocol(diagnostic));
        }
        Ok(CodexCloudApplyResult { stdout, stderr })
    }

    /// Loads the effective Codex configuration through the pinned binary's
    /// native strict loader. Closing stdin makes the temporary stdio server
    /// exit immediately after configuration initialization, so this does not
    /// create a second long-lived app-server or disturb active threads.
    pub async fn validate_strict_config(
        &self,
        cwd: Option<&Path>,
    ) -> Result<(), CodexHarnessError> {
        let executable = self.inner.lock().await.executable.clone();
        let mut command = Command::new(executable);
        command
            .arg("app-server")
            .arg("--strict-config")
            .arg("--listen")
            .arg("stdio://")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(cwd) = cwd {
            command.current_dir(cwd);
        }
        let output = tokio::time::timeout(Duration::from_secs(30), command.output())
            .await
            .map_err(|_| CodexHarnessError::Timeout)?
            .map_err(|error| CodexHarnessError::Start(error.to_string()))?;
        if output.status.success() {
            return Ok(());
        }
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        let diagnostic = diagnostic.trim();
        let diagnostic = if diagnostic.is_empty() {
            format!(
                "Codex strict configuration validation exited with {}",
                output.status
            )
        } else {
            diagnostic.chars().take(4_096).collect()
        };
        Err(CodexHarnessError::Protocol(diagnostic))
    }

    /// Resolves a named Codex runtime profile with the pinned native loader, then returns a
    /// sanitized snapshot suitable for a governed app-server thread. Authority- and
    /// credential-bearing keys are deliberately omitted and reported for auditability.
    pub async fn resolve_profile(
        &self,
        profile: &str,
        cwd: Option<&Path>,
        strict: bool,
    ) -> Result<CodexResolvedProfile, CodexHarnessError> {
        validate_profile_name(profile)?;
        let root = codex_home()?;
        let canonical_root = fs::canonicalize(&root).map_err(|error| {
            CodexHarnessError::Protocol(format!(
                "Codex home `{}` cannot be resolved: {error}",
                root.display()
            ))
        })?;
        let profile_path = canonical_root.join(format!("{profile}.config.toml"));
        let canonical_profile = fs::canonicalize(&profile_path).map_err(|error| {
            CodexHarnessError::Protocol(format!(
                "Codex profile `{profile}` is unavailable at `{}`: {error}",
                profile_path.display()
            ))
        })?;
        if canonical_profile.parent() != Some(canonical_root.as_path()) {
            return Err(CodexHarnessError::Protocol(
                "Codex profile resolved outside CODEX_HOME".into(),
            ));
        }
        let metadata = fs::metadata(&canonical_profile)?;
        const MAX_PROFILE_BYTES: u64 = 256 * 1024;
        if !metadata.is_file() || metadata.len() > MAX_PROFILE_BYTES {
            return Err(CodexHarnessError::Protocol(format!(
                "Codex profile must be a regular file no larger than {MAX_PROFILE_BYTES} bytes"
            )));
        }

        let executable = self.inner.lock().await.executable.clone();
        let mut command = Command::new(executable);
        if strict {
            command.arg("--strict-config");
        }
        command
            .arg("--profile")
            .arg(profile)
            .arg("mcp")
            .arg("list")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(cwd) = cwd {
            command.current_dir(cwd);
        }
        let output = tokio::time::timeout(Duration::from_secs(30), command.output())
            .await
            .map_err(|_| CodexHarnessError::Timeout)?
            .map_err(|error| CodexHarnessError::Start(error.to_string()))?;
        if !output.status.success() {
            let diagnostic = String::from_utf8_lossy(&output.stderr);
            let diagnostic = diagnostic.trim();
            return Err(CodexHarnessError::Protocol(if diagnostic.is_empty() {
                format!("Codex profile validation exited with {}", output.status)
            } else {
                diagnostic.chars().take(4_096).collect()
            }));
        }

        let bytes = fs::read(&canonical_profile)?;
        let parsed = toml::from_slice::<toml::Value>(&bytes).map_err(|error| {
            CodexHarnessError::Protocol(format!("Codex profile `{profile}` is invalid: {error}"))
        })?;
        let mut config = serde_json::to_value(parsed).map_err(|error| {
            CodexHarnessError::Protocol(format!("Codex profile could not be projected: {error}"))
        })?;
        let mut ignored_keys = Vec::new();
        sanitize_profile_config(&mut config, &mut Vec::new(), &mut ignored_keys);
        ignored_keys.sort();
        let serialized = serde_json::to_vec(&config).map_err(|error| {
            CodexHarnessError::Protocol(format!("Codex profile could not be bounded: {error}"))
        })?;
        const MAX_SANITIZED_PROFILE_BYTES: usize = 64 * 1024;
        if serialized.len() > MAX_SANITIZED_PROFILE_BYTES {
            return Err(CodexHarnessError::Protocol(format!(
                "sanitized Codex profile exceeds the {MAX_SANITIZED_PROFILE_BYTES}-byte per-run limit"
            )));
        }
        let digest = format!("{:x}", Sha256::digest(&bytes));
        Ok(CodexResolvedProfile {
            config,
            digest,
            ignored_keys,
        })
    }

    pub async fn subscribe(&self) -> broadcast::Receiver<Value> {
        self.inner.lock().await.notifications.subscribe()
    }

    async fn ensure_started(&self) -> Result<(), CodexHarnessError> {
        let mut inner = self.inner.lock().await;
        if let Some(connection) = inner.connection.as_mut() {
            let alive = match connection {
                Connection::Local(process) => process.child.try_wait()?.is_none(),
                Connection::Remote(remote) => !remote.task.is_finished(),
            };
            if alive {
                return Ok(());
            }
            inner.connection = None;
            inner.status.process_state = CodexProcessState::Stopped;
        }
        inner.status.process_state = CodexProcessState::Starting;
        let transport = inner.transport.clone();
        if transport.is_remote() {
            let remote = match transport {
                CodexTransportConfig::RemoteWebSocket {
                    endpoint,
                    auth_token_env,
                } => {
                    connect_remote(
                        &endpoint,
                        auth_token_env.as_deref(),
                        inner.invocation_auth_token.as_deref().map(String::as_str),
                        inner.pending.clone(),
                        inner.notifications.clone(),
                        inner.server_requests.clone(),
                        inner.notification_telemetry.clone(),
                    )
                    .await
                }
                CodexTransportConfig::RemoteUnixSocket { socket_path } => {
                    #[cfg(unix)]
                    {
                        connect_remote_unix(
                            &socket_path,
                            inner.pending.clone(),
                            inner.notifications.clone(),
                            inner.server_requests.clone(),
                            inner.notification_telemetry.clone(),
                        )
                        .await
                    }
                    #[cfg(not(unix))]
                    {
                        let _ = socket_path;
                        Err(CodexHarnessError::InvalidRequest(
                            "unix:// remote app servers are unavailable on this platform".into(),
                        ))
                    }
                }
                CodexTransportConfig::LocalStdio => unreachable!("remote transport checked"),
            }
            .map_err(|error| {
                inner.status.process_state = CodexProcessState::Failed;
                error
            })?;
            inner.connection = Some(Connection::Remote(remote));
            inner.status.process_state = CodexProcessState::Ready;
            inner.status.restart_count = inner.status.restart_count.saturating_add(1);
            drop(inner);
            self.initialize_connection().await?;
            return Ok(());
        }
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
        inner.connection = Some(Connection::Local(Process {
            child,
            stdin,
            #[cfg(windows)]
            _job: job,
        }));
        inner.status.process_state = CodexProcessState::Ready;
        inner.status.restart_count = inner.status.restart_count.saturating_add(1);
        drop(inner);
        self.initialize_connection().await?;
        Ok(())
    }

    async fn initialize_connection(&self) -> Result<(), CodexHarnessError> {
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
        enum Writer {
            Local(Arc<Mutex<ChildStdin>>),
            Remote(mpsc::Sender<String>),
        }
        let writer = {
            let inner = self.inner.lock().await;
            match inner
                .connection
                .as_ref()
                .ok_or(CodexHarnessError::Stopped)?
            {
                Connection::Local(process) => Writer::Local(process.stdin.clone()),
                Connection::Remote(remote) => Writer::Remote(remote.outbound.clone()),
            }
        };
        match writer {
            Writer::Local(stdin) => {
                let mut stdin = stdin.lock().await;
                stdin.write_all(&bytes).await?;
                stdin.write_all(b"\n").await?;
                stdin.flush().await?;
            }
            Writer::Remote(sender) => sender
                .send(String::from_utf8(bytes).map_err(|error| {
                    CodexHarnessError::Protocol(format!("request was not UTF-8: {error}"))
                })?)
                .await
                .map_err(|_| CodexHarnessError::Stopped)?,
        }
        Ok(())
    }

    pub async fn transport(&self) -> CodexTransportConfig {
        self.inner.lock().await.transport.clone()
    }

    /// Switches between the bundled stdio server and an authenticated remote
    /// WebSocket endpoint. The new transport must initialize successfully; on
    /// failure the prior transport is restored.
    pub async fn configure_transport(
        &self,
        transport: CodexTransportConfig,
    ) -> Result<CodexHarnessStatus, CodexHarnessError> {
        validate_transport(&transport)?;
        let previous = {
            let mut inner = self.inner.lock().await;
            if inner.status.active_threads > 0 || !inner.pending.lock().await.is_empty() {
                return Err(CodexHarnessError::InvalidRequest(
                    "Codex transport cannot change while work is active".into(),
                ));
            }
            let previous = inner.transport.clone();
            stop_connection(&mut inner);
            inner.transport = transport.clone();
            inner.invocation_auth_token = None;
            inner.status.transport = transport;
            previous
        };
        if let Err(error) = self.ensure_started().await {
            let mut inner = self.inner.lock().await;
            stop_connection(&mut inner);
            inner.transport = previous.clone();
            inner.status.transport = previous;
            return Err(error);
        }
        Ok(self.status().await)
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

    pub async fn permission_profiles(
        &self,
        cwd: Option<&str>,
    ) -> Result<Vec<CodexPermissionProfile>, CodexHarnessError> {
        let mut cursor: Option<String> = None;
        let mut profiles = Vec::new();
        let mut observed = BTreeSet::new();
        loop {
            let response = self
                .request(
                    "permissionProfile/list",
                    json!({"cursor": cursor, "cwd": cwd, "limit": 100}),
                )
                .await?;
            let page: CodexPermissionProfileList = serde_json::from_value(response)?;
            profiles.extend(page.data);
            let Some(next) = page.next_cursor else {
                break;
            };
            if !observed.insert(next.clone()) || profiles.len() >= 10_000 {
                return Err(CodexHarnessError::Protocol(
                    "permissionProfile/list returned an invalid pagination sequence".into(),
                ));
            }
            cursor = Some(next);
        }
        Ok(profiles)
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

    pub async fn start_device_code_login(&self) -> Result<Value, CodexHarnessError> {
        self.request("account/login/start", json!({"type": "chatgptDeviceCode"}))
            .await
    }

    pub async fn login_with_api_key(&self, api_key: &str) -> Result<Value, CodexHarnessError> {
        validate_login_credential(api_key, "API key")?;
        self.request(
            "account/login/start",
            json!({"type": "apiKey", "apiKey": api_key}),
        )
        .await
    }

    /// Uses Codex's stable CLI access-token login without placing the token in argv,
    /// logs, settings, or the app-server request stream.
    pub async fn login_with_access_token(
        &self,
        access_token: &str,
    ) -> Result<Value, CodexHarnessError> {
        validate_login_credential(access_token, "access token")?;
        let (executable, transport) = {
            let inner = self.inner.lock().await;
            (inner.executable.clone(), inner.transport.clone())
        };
        if transport.is_remote() {
            return Err(CodexHarnessError::InvalidRequest(
                "access-token login applies to the local Codex installation and is unavailable while a remote app-server is selected"
                    .into(),
            ));
        }
        let mut command = Command::new(executable);
        command
            .args(["login", "--with-access-token"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = command
            .spawn()
            .map_err(|error| CodexHarnessError::Start(error.to_string()))?;
        let mut secret = access_token.as_bytes().to_vec();
        secret.push(b'\n');
        let write_result = async {
            let mut stdin = child.stdin.take().ok_or_else(|| {
                CodexHarnessError::Start("Codex login stdin was unavailable".into())
            })?;
            stdin.write_all(&secret).await?;
            stdin.flush().await?;
            drop(stdin);
            Ok::<(), CodexHarnessError>(())
        }
        .await;
        secret.zeroize();
        write_result?;
        let output = tokio::time::timeout(Duration::from_secs(60), child.wait_with_output())
            .await
            .map_err(|_| CodexHarnessError::Timeout)?
            .map_err(CodexHarnessError::Io)?;
        if !output.status.success() {
            let diagnostic = String::from_utf8_lossy(&output.stderr);
            let diagnostic = diagnostic.trim();
            return Err(CodexHarnessError::Protocol(if diagnostic.is_empty() {
                format!("Codex access-token login exited with {}", output.status)
            } else {
                diagnostic.chars().take(4_096).collect()
            }));
        }
        {
            let mut inner = self.inner.lock().await;
            stop_connection(&mut inner);
        }
        serde_json::to_value(self.account().await?).map_err(Into::into)
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
        self.ensure_local_provider_ready(preferences).await?;
        let config = preferences.thread_config(instructions);
        let response = self
            .request(
                "thread/start",
                json!({
                    "cwd": cwd,
                    "modelProvider": preferences.model_provider,
                    "model": preferences.model,
                    "serviceTier": preferences.service_tier,
                    "approvalPolicy": preferences.approval_policy(writable),
                    "approvalsReviewer": preferences.approvals_reviewer(),
                    "sandbox": preferences.thread_sandbox(writable),
                    "permissions": preferences.effective_permission_profile(writable),
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
        self.ensure_local_provider_ready(preferences).await?;
        let config = preferences.thread_config(instructions);
        self.request(
            "thread/resume",
            json!({
                "threadId": thread_id,
                "cwd": cwd,
                "modelProvider": preferences.model_provider,
                "model": preferences.model,
                "serviceTier": preferences.service_tier,
                "approvalPolicy": preferences.approval_policy(writable),
                "approvalsReviewer": preferences.approvals_reviewer(),
                "sandbox": preferences.thread_sandbox(writable)
                ,"permissions": preferences.effective_permission_profile(writable)
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

    /// Starts an ordinary turn with typed mentions, progress events, and an
    /// optional JSON Schema for the final response. Callers should reserve the
    /// schema for the owner-visible completion turn; internal routing and
    /// coordination turns must remain unconstrained.
    pub async fn start_turn_with_mentions_and_progress_and_schema(
        &self,
        thread_id: &str,
        prompt: &str,
        mentions: &[CodexMentionInput],
        preferences: &CodexExecutionPreferences,
        writable: bool,
        progress: Option<mpsc::UnboundedSender<CodexTurnProgress>>,
        output_schema: Option<&Value>,
    ) -> Result<CodexTurnResult, CodexHarnessError> {
        self.start_turn_with_progress_and_schema(
            thread_id,
            prompt,
            mentions,
            preferences,
            writable,
            progress,
            output_schema,
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
        let response = self
            .request(
                "turn/start",
                turn_start_params(
                    thread_id,
                    prompt,
                    mentions,
                    preferences,
                    writable,
                    output_schema,
                    plan_mode,
                ),
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
        self.fork_thread_at_with_preferences(thread_id, last_turn_id, None, false)
            .await
    }

    pub async fn fork_thread_at_with_preferences(
        &self,
        thread_id: &str,
        last_turn_id: Option<&str>,
        preferences: Option<&CodexExecutionPreferences>,
        writable: bool,
    ) -> Result<String, CodexHarnessError> {
        if let Some(preferences) = preferences {
            self.ensure_local_provider_ready(preferences).await?;
        }
        let params = thread_fork_params(thread_id, last_turn_id, preferences, writable);
        let response = self.request("thread/fork", params).await?;
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

    /// Lists a bounded, content-minimized inventory of sub-agent threads from this app-server.
    ///
    /// # Errors
    /// Returns an error for an invalid bound, transport/protocol failure, or malformed identity.
    pub async fn list_remote_agents(
        &self,
        limit: usize,
    ) -> Result<CodexRemoteAgentInventory, CodexHarnessError> {
        if limit == 0 || limit > 200 {
            return Err(CodexHarnessError::InvalidRequest(
                "remote agent inventory limit must be between 1 and 200".into(),
            ));
        }
        let response = self
            .request(
                "thread/list",
                json!({
                    "limit": limit,
                    "sortKey": "updated_at",
                    "sortDirection": "desc",
                    "sourceKinds": [
                        "subAgent", "subAgentReview", "subAgentCompact",
                        "subAgentThreadSpawn", "subAgentOther"
                    ],
                    "archived": false,
                    "useStateDbOnly": true
                }),
            )
            .await?;
        decode_remote_agent_summaries(&response, limit)
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

    /// Discovers the two local providers supported by Codex's stable `--oss`
    /// flow. Probes bypass ambient HTTP proxies and accept only loopback HTTP
    /// endpoints so provider discovery cannot become an unreviewed network
    /// capability in MetaCog.
    pub async fn local_provider_discovery(
        &self,
    ) -> Result<CodexLocalProviderDiscovery, CodexHarnessError> {
        let configuration = self
            .request("config/read", json!({"includeLayers": false}))
            .await?;
        let configured_provider = configuration
            .pointer("/config/oss_provider")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .filter(|provider| matches!(provider.as_str(), "ollama" | "lmstudio"));
        let (ollama, lmstudio) = tokio::join!(
            probe_local_provider("ollama"),
            probe_local_provider("lmstudio")
        );
        Ok(CodexLocalProviderDiscovery {
            configured_provider,
            providers: vec![ollama, lmstudio],
        })
    }

    /// Persists the same `oss_provider` user preference written by Codex's
    /// interactive OSS picker. Provider activation remains an explicit
    /// per-run execution preference; this only supplies the next default.
    pub async fn set_default_local_provider(
        &self,
        provider: &str,
    ) -> Result<CodexLocalProviderDiscovery, CodexHarnessError> {
        if !matches!(provider, "ollama" | "lmstudio") {
            return Err(CodexHarnessError::InvalidRequest(
                "local Codex provider must be `ollama` or `lmstudio`".into(),
            ));
        }
        let response = self
            .request(
                "config/value/write",
                json!({
                    "keyPath": "oss_provider",
                    "value": provider,
                    "mergeStrategy": "replace",
                    "filePath": null,
                    "expectedVersion": null,
                }),
            )
            .await?;
        if !matches!(response.get("status").and_then(Value::as_str), Some("ok")) {
            return Err(CodexHarnessError::InvalidRequest(format!(
                "Codex did not accept the local provider preference: {}",
                response
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown write status")
            )));
        }
        let discovery = self.local_provider_discovery().await?;
        if discovery.configured_provider.as_deref() != Some(provider) {
            return Err(CodexHarnessError::InvalidRequest(
                "the local provider preference is overridden by managed Codex configuration".into(),
            ));
        }
        Ok(discovery)
    }

    async fn ensure_local_provider_ready(
        &self,
        preferences: &CodexExecutionPreferences,
    ) -> Result<(), CodexHarnessError> {
        let Some(provider) = preferences.model_provider.as_deref() else {
            return Ok(());
        };
        let model = preferences.model.as_deref().ok_or_else(|| {
            CodexHarnessError::InvalidRequest(
                "a local Codex provider requires an explicit resolved model".into(),
            )
        })?;
        ensure_local_provider_model(provider, model).await
    }

    /// Reads Codex's effective `tui.resume_cwd` value through the pinned native
    /// configuration loader. `None` preserves Codex's prompt-on-difference behavior.
    pub async fn resume_cwd(
        &self,
        cwd: Option<&str>,
        profile: Option<&str>,
        strict: bool,
    ) -> Result<Option<CodexResumeCwd>, CodexHarnessError> {
        let response = self
            .request("config/read", json!({"cwd": cwd, "includeLayers": false}))
            .await?;
        let mut preference = parse_resume_cwd(response.pointer("/config/tui/resume_cwd"))?;
        if let Some(profile) = profile {
            let resolved = self
                .resolve_profile(profile, cwd.map(Path::new), strict)
                .await?;
            if let Some(profile_preference) =
                parse_resume_cwd(resolved.config.pointer("/tui/resume_cwd"))?
            {
                preference = Some(profile_preference);
            }
        }
        Ok(preference)
    }

    pub async fn windows_sandbox_readiness(&self) -> Result<Value, CodexHarnessError> {
        self.request(WINDOWS_SANDBOX_READINESS_METHOD, json!({}))
            .await
    }

    /// Runs Codex's current Windows sandbox setup RPC, waits for its completion
    /// notification, and only then persists the matching native configuration.
    ///
    /// # Errors
    ///
    /// Returns an error when this is called off Windows, setup is rejected or
    /// fails, the completion notification is lost, or Codex cannot persist the
    /// selected sandbox mode.
    pub async fn setup_windows_sandbox(
        &self,
        mode: CodexWindowsSandboxSetupMode,
        cwd: Option<&Path>,
    ) -> Result<CodexWindowsSandboxSetupResult, CodexHarnessError> {
        #[cfg(not(windows))]
        {
            let _ = (mode, cwd);
            return Err(CodexHarnessError::Protocol(
                "Windows sandbox setup is only available on Windows".into(),
            ));
        }
        #[cfg(windows)]
        {
            let cwd = cwd
                .map(fs::canonicalize)
                .transpose()?
                .map(|path| path.to_string_lossy().into_owned());
            let mut notifications = self.subscribe().await;
            let response = self
                .request(
                    WINDOWS_SANDBOX_SETUP_START_METHOD,
                    json!({"mode": mode.protocol_name(), "cwd": cwd}),
                )
                .await?;
            if response.get("started").and_then(Value::as_bool) != Some(true) {
                return Err(CodexHarnessError::Protocol(
                    "Codex did not acknowledge Windows sandbox setup".into(),
                ));
            }

            let completion = tokio::time::timeout(Duration::from_secs(600), async {
                loop {
                    let document = notifications
                        .recv()
                        .await
                        .map_err(|error| CodexHarnessError::Protocol(error.to_string()))?;
                    if document.get("method").and_then(Value::as_str)
                        != Some(WINDOWS_SANDBOX_SETUP_COMPLETED_METHOD)
                    {
                        continue;
                    }
                    let params = document.get("params").cloned().unwrap_or(Value::Null);
                    if params.get("mode").and_then(Value::as_str) == Some(mode.protocol_name()) {
                        return Ok::<Value, CodexHarnessError>(params);
                    }
                }
            })
            .await
            .map_err(|_| CodexHarnessError::Timeout)??;
            if completion.get("success").and_then(Value::as_bool) != Some(true) {
                let detail = completion
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("Codex Windows sandbox setup failed");
                return Err(CodexHarnessError::Protocol(detail.to_owned()));
            }

            let config = self
                .request(
                    "config/batchWrite",
                    windows_sandbox_config_write_request(mode),
                )
                .await?;
            let config_write_status = config
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_owned();
            if config_write_status != "ok" {
                let detail = if config_write_status == "okOverridden" {
                    "Codex installed the Windows sandbox, but a higher-priority configuration layer overrides the requested default mode"
                } else {
                    "Codex installed the Windows sandbox, but did not confirm the default-mode configuration write"
                };
                return Err(CodexHarnessError::Protocol(detail.into()));
            }
            let readiness = self.windows_sandbox_readiness().await?;
            let readiness = readiness
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_owned();
            if readiness != "ready" {
                return Err(CodexHarnessError::Protocol(format!(
                    "Codex completed Windows sandbox setup, but readiness is `{readiness}`"
                )));
            }
            Ok(CodexWindowsSandboxSetupResult {
                mode,
                readiness,
                config_write_status,
            })
        }
    }

    /// Uses the pinned Codex sandbox engine to refresh the restricted-token ACL
    /// with one additional read root. MetaCog supplies only a safe no-op command;
    /// permission resolution and ACL mutation remain owned by the pinned Codex.
    ///
    /// # Errors
    ///
    /// Returns an error for unsupported platforms, invalid paths or profiles,
    /// startup failures, timeout, or a rejected sandbox refresh.
    pub async fn grant_windows_sandbox_read_directory(
        &self,
        cwd: &Path,
        read_root: &Path,
        permission_profile: Option<&str>,
    ) -> Result<CodexWindowsSandboxReadGrant, CodexHarnessError> {
        #[cfg(not(windows))]
        {
            let _ = (cwd, read_root, permission_profile);
            return Err(CodexHarnessError::Protocol(
                "Windows sandbox read-directory grants are only available on Windows".into(),
            ));
        }
        #[cfg(windows)]
        {
            if !cwd.is_absolute() || !read_root.is_absolute() {
                return Err(CodexHarnessError::Protocol(
                    "Windows sandbox paths must be absolute".into(),
                ));
            }
            let cwd = fs::canonicalize(cwd)?;
            let read_root = fs::canonicalize(read_root)?;
            if !cwd.is_dir() || !read_root.is_dir() {
                return Err(CodexHarnessError::Protocol(
                    "Windows sandbox paths must be existing directories".into(),
                ));
            }
            let permission_profile = permission_profile.unwrap_or(":workspace").trim();
            if !matches!(
                permission_profile,
                ":workspace" | ":read-only" | ":danger-full-access"
            ) {
                validate_profile_name(permission_profile)?;
            }
            let profiles = self.permission_profiles(cwd.to_str()).await?;
            match profiles
                .iter()
                .find(|profile| profile.id == permission_profile)
            {
                Some(profile) if profile.allowed => {}
                Some(_) => {
                    return Err(CodexHarnessError::Protocol(format!(
                        "Codex permission profile `{permission_profile}` is blocked by requirements"
                    )));
                }
                None => {
                    return Err(CodexHarnessError::Protocol(format!(
                        "Codex permission profile `{permission_profile}` is unavailable"
                    )));
                }
            }

            let system_root = std::env::var_os("SystemRoot")
                .map(PathBuf::from)
                .ok_or_else(|| CodexHarnessError::Protocol("SystemRoot is unavailable".into()))?;
            let noop = fs::canonicalize(system_root.join("System32").join("cmd.exe"))?;
            let executable = self.inner.lock().await.executable.clone();
            let mut command = Command::new(executable);
            command
                .arg("sandbox")
                .arg("--permission-profile")
                .arg(permission_profile)
                .arg("--cd")
                .arg(&cwd)
                .arg("--sandbox-state-readable-root")
                .arg(&read_root)
                .arg(noop)
                .args(["/d", "/c", "exit", "0"])
                .current_dir(&cwd)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .kill_on_drop(true);
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
            let output = tokio::time::timeout(Duration::from_secs(180), command.output())
                .await
                .map_err(|_| CodexHarnessError::Timeout)?
                .map_err(|error| CodexHarnessError::Start(error.to_string()))?;
            if !output.status.success() {
                let diagnostic = String::from_utf8_lossy(&output.stderr);
                let diagnostic = diagnostic.trim();
                return Err(CodexHarnessError::Protocol(if diagnostic.is_empty() {
                    format!("Codex sandbox refresh exited with {}", output.status)
                } else {
                    diagnostic.chars().take(4_096).collect()
                }));
            }
            Ok(CodexWindowsSandboxReadGrant {
                path: read_root.to_string_lossy().into_owned(),
                permission_profile: permission_profile.to_owned(),
            })
        }
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
            hooks: serde_json::from_value(
                hooks
                    .get("data")
                    .cloned()
                    .unwrap_or_else(|| Value::Array(Vec::new())),
            )
            .map_err(|error| {
                CodexHarnessError::Protocol(format!("invalid hooks/list response: {error}"))
            })?,
            apps,
            apps_available: true,
            apps_unavailable_reason: None,
            refreshed_at: Utc::now(),
        })
    }

    /// Applies an exact-definition hook trust or enablement update and returns refreshed discovery.
    ///
    /// # Errors
    ///
    /// Returns an error when the request is malformed, the discovered hook or hash changed, a
    /// managed hook is targeted, trust is insufficient for enablement, or Codex rejects the write.
    pub async fn update_hook_state(
        &self,
        update: &CodexHookStateUpdate,
    ) -> Result<CodexStableDiscovery, CodexHarnessError> {
        validate_hook_state_update(update)?;
        let discovery =
            Box::pin(self.stable_discovery(std::slice::from_ref(&update.cwd), true)).await?;
        let entry = discovery
            .hooks
            .iter()
            .find(|entry| entry.cwd == update.cwd)
            .ok_or_else(|| {
                CodexHarnessError::InvalidRequest(format!(
                    "Codex did not return hook discovery for {:?}",
                    update.cwd
                ))
            })?;
        let hook = entry
            .hooks
            .iter()
            .find(|hook| hook.key == update.key)
            .ok_or_else(|| {
                CodexHarnessError::InvalidRequest("hook is not present in current discovery".into())
            })?;
        if hook.is_managed {
            return Err(CodexHarnessError::InvalidRequest(
                "managed hooks are controlled by policy and cannot be changed".into(),
            ));
        }
        let current_hash = hook.current_hash.as_str();
        if current_hash != update.expected_current_hash {
            return Err(CodexHarnessError::InvalidRequest(
                "hook definition changed; refresh and review the new definition".into(),
            ));
        }
        let trust_status = hook.trust_status.as_str();
        if update.enabled == Some(true)
            && !update.trust_current_definition
            && !matches!(trust_status, "trusted" | "managed")
        {
            return Err(CodexHarnessError::InvalidRequest(
                "review and trust this exact hook definition before enabling it".into(),
            ));
        }

        let mut state = serde_json::Map::new();
        if let Some(enabled) = update.enabled {
            state.insert("enabled".into(), Value::Bool(enabled));
        }
        if update.trust_current_definition {
            state.insert("trusted_hash".into(), Value::String(current_hash.into()));
        }
        if state.is_empty() {
            return Err(CodexHarnessError::InvalidRequest(
                "hook update must change enablement or trust the current definition".into(),
            ));
        }
        self.request(
            "config/batchWrite",
            json!({
                "edits": [{
                    "keyPath": "hooks.state",
                    "value": { (update.key.clone()): Value::Object(state) },
                    "mergeStrategy": "upsert"
                }],
                "filePath": null,
                "expectedVersion": null,
                "reloadUserConfig": true
            }),
        )
        .await?;
        Box::pin(self.stable_discovery(std::slice::from_ref(&update.cwd), true)).await
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

fn thread_fork_params(
    thread_id: &str,
    last_turn_id: Option<&str>,
    preferences: Option<&CodexExecutionPreferences>,
    writable: bool,
) -> Value {
    let mut params = json!({
        "threadId": thread_id,
        "lastTurnId": last_turn_id,
        "threadSource": "mcg"
    });
    if let Some(preferences) = preferences {
        let values = params
            .as_object_mut()
            .expect("thread/fork parameters are an object");
        values.insert("modelProvider".into(), json!(preferences.model_provider));
        values.insert("model".into(), json!(preferences.model));
        values.insert("serviceTier".into(), json!(preferences.service_tier));
        values.insert(
            "approvalPolicy".into(),
            json!(preferences.approval_policy(writable)),
        );
        values.insert(
            "approvalsReviewer".into(),
            json!(preferences.approvals_reviewer()),
        );
        values.insert(
            "sandbox".into(),
            json!(preferences.thread_sandbox(writable)),
        );
        values.insert(
            "permissions".into(),
            json!(preferences.effective_permission_profile(writable)),
        );
        values.insert("config".into(), json!(preferences.thread_config(None)));
    }
    params
}

fn local_provider_endpoint(provider: &str) -> Result<String, CodexHarnessError> {
    let default_port = match provider {
        "ollama" => 11_434,
        "lmstudio" => 1_234,
        _ => {
            return Err(CodexHarnessError::InvalidRequest(format!(
                "unsupported Codex local provider `{provider}`"
            )));
        }
    };
    let port = std::env::var("CODEX_OSS_PORT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(default_port);
    let endpoint = std::env::var("CODEX_OSS_BASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("http://localhost:{port}/v1"));
    let parsed = Url::parse(&endpoint).map_err(|error| {
        CodexHarnessError::InvalidRequest(format!(
            "invalid CODEX_OSS_BASE_URL for {provider}: {error}"
        ))
    })?;
    let loopback = match parsed.host() {
        Some(url::Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(host)) => host.is_loopback(),
        Some(url::Host::Ipv6(host)) => host.is_loopback(),
        None => false,
    };
    if parsed.scheme() != "http"
        || !loopback
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(CodexHarnessError::InvalidRequest(
            "MetaCog local providers require a credential-free loopback HTTP endpoint".into(),
        ));
    }
    Ok(endpoint.trim_end_matches('/').to_owned())
}

fn local_provider_host_root(endpoint: &str) -> &str {
    endpoint.strip_suffix("/v1").unwrap_or(endpoint)
}

fn local_provider_http_client(timeout: Duration) -> Result<reqwest::Client, CodexHarnessError> {
    reqwest::Client::builder()
        .no_proxy()
        .connect_timeout(Duration::from_secs(5))
        .timeout(timeout)
        .build()
        .map_err(|error| CodexHarnessError::Start(error.to_string()))
}

async fn probe_local_provider(provider: &str) -> CodexLocalProviderStatus {
    let display_name = if provider == "ollama" {
        "Ollama"
    } else {
        "LM Studio"
    };
    let default_model = if provider == "ollama" {
        "gpt-oss:20b"
    } else {
        "openai/gpt-oss-20b"
    };
    let endpoint = match local_provider_endpoint(provider) {
        Ok(endpoint) => endpoint,
        Err(error) => {
            return CodexLocalProviderStatus {
                id: provider.into(),
                display_name: display_name.into(),
                default_model: default_model.into(),
                endpoint: String::new(),
                running: false,
                responses_supported: false,
                version: None,
                models: Vec::new(),
                detail: error.to_string(),
            };
        }
    };
    let client = match local_provider_http_client(Duration::from_secs(5)) {
        Ok(client) => client,
        Err(error) => {
            return CodexLocalProviderStatus {
                id: provider.into(),
                display_name: display_name.into(),
                default_model: default_model.into(),
                endpoint,
                running: false,
                responses_supported: false,
                version: None,
                models: Vec::new(),
                detail: error.to_string(),
            };
        }
    };
    let result = if provider == "ollama" {
        probe_ollama(&client, &endpoint).await
    } else {
        probe_lmstudio(&client, &endpoint).await
    };
    match result {
        Ok((version, models, responses_supported)) => CodexLocalProviderStatus {
            id: provider.into(),
            display_name: display_name.into(),
            default_model: default_model.into(),
            endpoint,
            running: true,
            responses_supported,
            version,
            models,
            detail: if responses_supported {
                "Ready for Codex Responses API work".into()
            } else {
                "Running, but the Responses API version requirement is not met".into()
            },
        },
        Err(detail) => CodexLocalProviderStatus {
            id: provider.into(),
            display_name: display_name.into(),
            default_model: default_model.into(),
            endpoint,
            running: false,
            responses_supported: false,
            version: None,
            models: Vec::new(),
            detail,
        },
    }
}

async fn probe_ollama(
    client: &reqwest::Client,
    endpoint: &str,
) -> Result<(Option<String>, Vec<String>, bool), String> {
    let root = local_provider_host_root(endpoint);
    let response = client
        .get(format!("{root}/api/tags"))
        .send()
        .await
        .map_err(|_| {
            "No running Ollama server detected. Start it with `ollama serve`.".to_owned()
        })?;
    if !response.status().is_success() {
        return Err(format!(
            "Ollama returned HTTP {} from its model catalog",
            response.status()
        ));
    }
    let value = response
        .json::<Value>()
        .await
        .map_err(|error| format!("Ollama returned an invalid model catalog: {error}"))?;
    let models = value
        .get("models")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|model| model.get("name").and_then(Value::as_str))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let version = client
        .get(format!("{root}/api/version"))
        .send()
        .await
        .ok()
        .filter(|response| response.status().is_success());
    let version = if let Some(response) = version {
        response.json::<Value>().await.ok().and_then(|value| {
            value
                .get("version")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
    } else {
        None
    };
    let responses_supported = ollama_responses_supported(version.as_deref());
    Ok((version, models, responses_supported))
}

fn ollama_responses_supported(raw: Option<&str>) -> bool {
    raw.is_none_or(|raw| {
        Version::parse(raw.trim_start_matches('v')).map_or(true, |version| {
            version == Version::new(0, 0, 0) || version >= Version::new(0, 13, 4)
        })
    })
}

async fn probe_lmstudio(
    client: &reqwest::Client,
    endpoint: &str,
) -> Result<(Option<String>, Vec<String>, bool), String> {
    let response = client
        .get(format!("{endpoint}/models"))
        .send()
        .await
        .map_err(|_| {
            "LM Studio is not responding. Install it and run `lms server start`.".to_owned()
        })?;
    if !response.status().is_success() {
        return Err(format!(
            "LM Studio returned HTTP {} from its model catalog",
            response.status()
        ));
    }
    let value = response
        .json::<Value>()
        .await
        .map_err(|error| format!("LM Studio returned an invalid model catalog: {error}"))?;
    let models = value
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|model| model.get("id").and_then(Value::as_str))
        .map(str::to_owned)
        .collect();
    Ok((None, models, true))
}

async fn ensure_local_provider_model(provider: &str, model: &str) -> Result<(), CodexHarnessError> {
    if !matches!(provider, "ollama" | "lmstudio") {
        return Err(CodexHarnessError::InvalidRequest(format!(
            "unsupported Codex local provider `{provider}`"
        )));
    }
    let status = probe_local_provider(provider).await;
    if !status.running {
        return Err(CodexHarnessError::Protocol(status.detail));
    }
    if !status.responses_supported {
        let version = status.version.as_deref().unwrap_or("unknown");
        return Err(CodexHarnessError::Protocol(format!(
            "Ollama {version} is too old. Codex requires Ollama 0.13.4 or newer."
        )));
    }
    if status.models.iter().any(|available| available == model) {
        return Ok(());
    }
    if provider == "ollama" {
        let client = local_provider_http_client(Duration::from_secs(7_200))?;
        let root = local_provider_host_root(&status.endpoint);
        let response = client
            .post(format!("{root}/api/pull"))
            .json(&json!({"model": model, "stream": false}))
            .send()
            .await
            .map_err(|error| {
                CodexHarnessError::Protocol(format!(
                    "failed to pull Ollama model `{model}`: {error}"
                ))
            })?;
        if !response.status().is_success() {
            return Err(CodexHarnessError::Protocol(format!(
                "failed to pull Ollama model `{model}`: HTTP {}",
                response.status()
            )));
        }
        return Ok(());
    }

    let mut command = Command::new("lms");
    command
        .args(["get", "--yes", model])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let status_result = tokio::time::timeout(Duration::from_secs(7_200), command.status()).await;
    let status_result = match status_result {
        Ok(Ok(status)) => status,
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                .map(PathBuf::from)
                .unwrap_or_default();
            let fallback = home.join(".lmstudio").join("bin").join(if cfg!(windows) {
                "lms.exe"
            } else {
                "lms"
            });
            if !fallback.is_file() {
                return Err(CodexHarnessError::Protocol(
                    "LM Studio CLI was not found. Install LM Studio before downloading a model."
                        .into(),
                ));
            }
            let mut fallback_command = Command::new(fallback);
            fallback_command
                .args(["get", "--yes", model])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .kill_on_drop(true);
            tokio::time::timeout(Duration::from_secs(7_200), fallback_command.status())
                .await
                .map_err(|_| CodexHarnessError::Timeout)??
        }
        Ok(Err(error)) => return Err(CodexHarnessError::Io(error)),
        Err(_) => return Err(CodexHarnessError::Timeout),
    };
    if !status_result.success() {
        return Err(CodexHarnessError::Protocol(format!(
            "LM Studio model download failed with exit code {}",
            status_result.code().unwrap_or(-1)
        )));
    }
    let client = local_provider_http_client(Duration::from_secs(5))?;
    let endpoint = status.endpoint.clone();
    let model = model.to_owned();
    tokio::spawn(async move {
        let _ = client
            .post(format!("{endpoint}/responses"))
            .json(&json!({"model": model, "input": "", "max_output_tokens": 1}))
            .send()
            .await;
    });
    Ok(())
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
            | "hook/started"
            | "hook/completed"
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

fn decode_remote_agent_summaries(
    response: &Value,
    limit: usize,
) -> Result<CodexRemoteAgentInventory, CodexHarnessError> {
    let rows = response
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| CodexHarnessError::Protocol("thread/list omitted its data array".into()))?;
    if rows.len() > limit {
        return Err(CodexHarnessError::Protocol(
            "thread/list exceeded the requested remote agent limit".into(),
        ));
    }
    let agents = rows
        .iter()
        .map(
            |thread| -> Result<CodexRemoteAgentSummary, CodexHarnessError> {
                let bounded = |field: &'static str, optional: bool| {
                    let value = thread.get(field).and_then(Value::as_str);
                    if value.is_none() && optional {
                        return Ok(None);
                    }
                    let value = value.ok_or_else(|| {
                        CodexHarnessError::Protocol(format!("remote agent omitted {field}"))
                    })?;
                    if value.is_empty() || value.len() > 512 || value.chars().any(char::is_control)
                    {
                        return Err(CodexHarnessError::Protocol(format!(
                            "remote agent {field} is invalid or oversized"
                        )));
                    }
                    Ok(Some(value.to_owned()))
                };
                let status = thread
                    .pointer("/status/type")
                    .and_then(Value::as_str)
                    .filter(|status| {
                        matches!(*status, "notLoaded" | "idle" | "systemError" | "active")
                    })
                    .ok_or_else(|| {
                        CodexHarnessError::Protocol("remote agent status is invalid".into())
                    })?;
                Ok(CodexRemoteAgentSummary {
                    thread_id: bounded("id", false)?.expect("required remote agent id"),
                    session_id: bounded("sessionId", false)?.expect("required remote session id"),
                    parent_thread_id: bounded("parentThreadId", false)?
                        .expect("required remote parent thread id"),
                    nickname: bounded("agentNickname", true)?,
                    role: bounded("agentRole", true)?,
                    status: status.to_owned(),
                    can_accept_direct_input: thread
                        .get("canAcceptDirectInput")
                        .and_then(Value::as_bool),
                    updated_at_unix: thread.get("updatedAt").and_then(Value::as_i64).ok_or_else(
                        || CodexHarnessError::Protocol("remote agent updatedAt is invalid".into()),
                    )?,
                })
            },
        )
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CodexRemoteAgentInventory {
        agents,
        truncated: response
            .get("nextCursor")
            .is_some_and(|cursor| !cursor.is_null()),
    })
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
    fn ollama_response_support_matches_pinned_codex_floor() {
        assert!(ollama_responses_supported(None));
        assert!(ollama_responses_supported(Some("0.0.0")));
        assert!(!ollama_responses_supported(Some("0.13.3")));
        assert!(ollama_responses_supported(Some("v0.13.4")));
        assert!(ollama_responses_supported(Some("0.14.0")));
        assert!(ollama_responses_supported(Some("development")));
    }

    #[test]
    fn remote_agent_inventory_is_bounded_content_minimized_and_parent_bound() {
        let response = json!({"data": [{
            "id": "019remote-agent",
            "sessionId": "019session",
            "parentThreadId": "019parent",
            "agentNickname": "Atlas",
            "agentRole": "coder",
            "status": {"type": "active", "activeFlags": []},
            "canAcceptDirectInput": true,
            "updatedAt": 1_788_000_000_i64,
            "preview": "secret prompt material",
            "cwd": "C:/private/repository"
        }]});
        let inventory = decode_remote_agent_summaries(&response, 1).unwrap();
        assert_eq!(inventory.agents.len(), 1);
        assert_eq!(inventory.agents[0].thread_id, "019remote-agent");
        assert_eq!(inventory.agents[0].parent_thread_id, "019parent");
        assert!(!inventory.truncated);
        let serialized = serde_json::to_string(&inventory).unwrap();
        assert!(!serialized.contains("secret prompt material"));
        assert!(!serialized.contains("private/repository"));

        let mut missing_parent = response;
        missing_parent["data"][0]["parentThreadId"] = Value::Null;
        assert!(decode_remote_agent_summaries(&missing_parent, 1).is_err());
        assert!(decode_remote_agent_summaries(&json!({"data": []}), 0).is_ok());
        assert!(decode_remote_agent_summaries(&json!({"data": [{}, {}]}), 1).is_err());
    }

    #[test]
    fn remote_transport_rejects_unsafe_or_credential_bearing_endpoints() {
        for endpoint in [
            "ws://example.com/codex",
            "https://example.com/codex",
            "wss://user:secret@example.com/codex",
            "wss://example.com/codex#fragment",
        ] {
            assert!(
                validate_transport(&CodexTransportConfig::remote(endpoint.into(), None)).is_err(),
                "unexpectedly accepted {endpoint}"
            );
        }
        assert!(
            validate_transport(&CodexTransportConfig::remote(
                "ws://127.0.0.1:4500".into(),
                Some("CODEX_REMOTE_TOKEN".into()),
            ))
            .is_ok()
        );
        assert!(
            validate_transport(&CodexTransportConfig::remote(
                "wss://codex.example.com/socket".into(),
                Some("9INVALID".into()),
            ))
            .is_err()
        );
    }

    #[test]
    fn login_credentials_are_bounded_and_control_free() {
        assert!(validate_login_credential("sk-test", "API key").is_ok());
        assert!(validate_login_credential("", "API key").is_err());
        assert!(validate_login_credential("line\nbreak", "access token").is_err());
        assert!(validate_login_credential(&"x".repeat(16_385), "access token").is_err());
    }

    #[test]
    fn hook_state_updates_require_exact_bounded_identity() {
        let valid = CodexHookStateUpdate {
            cwd: "C:\\workspace".into(),
            key: "project:preToolUse:0".into(),
            expected_current_hash: format!("sha256:{}", "a".repeat(64)),
            enabled: Some(false),
            trust_current_definition: false,
        };
        assert!(validate_hook_state_update(&valid).is_ok());
        assert!(
            validate_hook_state_update(&CodexHookStateUpdate {
                expected_current_hash: "a".repeat(64),
                ..valid.clone()
            })
            .is_err()
        );
        assert!(
            validate_hook_state_update(&CodexHookStateUpdate {
                expected_current_hash: "short".into(),
                ..valid.clone()
            })
            .is_err()
        );
        assert!(
            validate_hook_state_update(&CodexHookStateUpdate {
                key: String::new(),
                ..valid.clone()
            })
            .is_err()
        );
        assert!(
            validate_hook_state_update(&CodexHookStateUpdate {
                cwd: "bad\0cwd".into(),
                ..valid
            })
            .is_err()
        );
    }

    #[tokio::test]
    async fn remote_websocket_initializes_and_serves_account_and_agent_inventory() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind websocket fixture");
        let address = listener.local_addr().expect("fixture address");
        let server = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.expect("accept fixture client");
            let mut socket = async_tungstenite::tokio::accept_async(socket)
                .await
                .expect("upgrade fixture websocket");
            while let Some(message) = socket.next().await {
                let Message::Text(text) = message.expect("fixture websocket message") else {
                    continue;
                };
                let request: Value = serde_json::from_str(&text).expect("fixture JSON-RPC request");
                let Some(id) = request.get("id").cloned() else {
                    continue;
                };
                let method = request
                    .get("method")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let result = match method {
                    "account/read" => json!({"account": null, "requiresOpenaiAuth": true}),
                    "thread/list" => json!({"data": [{
                        "id": "019remote-agent",
                        "sessionId": "019session",
                        "parentThreadId": "019parent",
                        "agentNickname": "Atlas",
                        "agentRole": "coder",
                        "status": {"type": "idle"},
                        "canAcceptDirectInput": false,
                        "updatedAt": 1_788_000_000_i64
                    }], "nextCursor": null}),
                    _ => json!({}),
                };
                socket
                    .send(Message::Text(
                        json!({"id": id, "result": result}).to_string().into(),
                    ))
                    .await
                    .expect("send fixture response");
                if method == "thread/list" {
                    break;
                }
            }
        });
        let transport = CodexTransportConfig::remote(format!("ws://{address}/codex"), None);
        let harness = CodexAppServerHarness::new_with_transport("unused", transport.clone());
        let account = harness.account().await.expect("remote account request");
        assert!(!account.authenticated);
        let inventory = harness
            .list_remote_agents(200)
            .await
            .expect("remote agent inventory");
        assert_eq!(inventory.agents.len(), 1);
        assert_eq!(inventory.agents[0].parent_thread_id, "019parent");
        assert!(!inventory.truncated);
        assert_eq!(harness.status().await.transport, transport);
        server.await.expect("fixture server completion");
    }

    #[tokio::test]
    async fn invocation_remote_sends_bearer_without_exposing_it_in_status() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind authenticated websocket fixture");
        let address = listener.local_addr().expect("fixture address");
        let (authorization_tx, authorization_rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.expect("accept fixture client");
            let mut authorization_tx = Some(authorization_tx);
            let mut socket = async_tungstenite::tokio::accept_hdr_async(
                socket,
                move |request: &async_tungstenite::tungstenite::handshake::server::Request,
                      response| {
                    if let Some(sender) = authorization_tx.take() {
                        let value = request
                            .headers()
                            .get(AUTHORIZATION)
                            .and_then(|value| value.to_str().ok())
                            .map(str::to_owned);
                        let _ = sender.send(value);
                    }
                    Ok(response)
                },
            )
            .await
            .expect("upgrade authenticated fixture websocket");
            while let Some(message) = socket.next().await {
                let Message::Text(text) = message.expect("fixture websocket message") else {
                    continue;
                };
                let request: Value = serde_json::from_str(&text).expect("fixture JSON-RPC request");
                let Some(id) = request.get("id").cloned() else {
                    continue;
                };
                let method = request
                    .get("method")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let result = if method == "account/read" {
                    json!({"account": null, "requiresOpenaiAuth": true})
                } else {
                    json!({})
                };
                socket
                    .send(Message::Text(
                        json!({"id": id, "result": result}).to_string().into(),
                    ))
                    .await
                    .expect("send fixture response");
                if method == "account/read" {
                    break;
                }
            }
        });
        let endpoint = format!("ws://{address}/");
        let harness = CodexAppServerHarness::new_with_invocation_remote(
            "unused",
            endpoint.clone(),
            Some(Zeroizing::new("invocation-secret".into())),
        )
        .expect("invocation remote harness");
        let account = harness.account().await.expect("remote account request");
        assert!(!account.authenticated);
        assert_eq!(
            authorization_rx.await.expect("authorization capture"),
            Some("Bearer invocation-secret".into())
        );
        assert_eq!(
            harness.status().await.transport,
            CodexTransportConfig::remote(endpoint, None)
        );
        server.await.expect("fixture server completion");
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn hook_state_update_writes_exact_state_and_refreshes_discovery() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind hook websocket fixture");
        let address = listener.local_addr().expect("hook fixture address");
        let (batch_tx, batch_rx) = oneshot::channel();
        let current_hash = format!("sha256:{}", "a".repeat(64));
        let fixture_hash = current_hash.clone();
        let server = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.expect("accept hook fixture client");
            let mut socket = async_tungstenite::tokio::accept_async(socket)
                .await
                .expect("upgrade hook fixture websocket");
            let mut hook_requests = 0_usize;
            let mut completed_requests = 0_usize;
            let mut batch_tx = Some(batch_tx);
            while let Some(message) = socket.next().await {
                let Message::Text(text) = message.expect("hook fixture websocket message") else {
                    continue;
                };
                let request: Value =
                    serde_json::from_str(&text).expect("hook fixture JSON-RPC request");
                let Some(id) = request.get("id").cloned() else {
                    continue;
                };
                let method = request
                    .get("method")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let result = match method {
                    "skills/list" | "app/list" => json!({"data": [], "nextCursor": null}),
                    "hooks/list" => {
                        hook_requests += 1;
                        json!({"data": [{
                            "cwd": "C:\\workspace",
                            "hooks": [{
                                "key": "project:preToolUse:0",
                                "eventName": "preToolUse",
                                "handlerType": "command",
                                "command": "cargo check",
                                "async": false,
                                "timeoutSec": 30,
                                "sourcePath": "C:\\workspace\\.codex\\hooks.json",
                                "source": "project",
                                "displayOrder": 0,
                                "enabled": hook_requests > 1,
                                "isManaged": false,
                                "currentHash": fixture_hash,
                                "trustStatus": if hook_requests > 1 { "trusted" } else { "untrusted" }
                            }],
                            "warnings": [],
                            "errors": []
                        }]})
                    }
                    "config/batchWrite" => {
                        if let Some(sender) = batch_tx.take() {
                            let _ = sender.send(request["params"].clone());
                        }
                        json!({})
                    }
                    _ => json!({}),
                };
                socket
                    .send(Message::Text(
                        json!({"id": id, "result": result}).to_string().into(),
                    ))
                    .await
                    .expect("send hook fixture response");
                if matches!(
                    method,
                    "skills/list" | "hooks/list" | "app/list" | "config/batchWrite"
                ) {
                    completed_requests += 1;
                    if completed_requests == 7 {
                        break;
                    }
                }
            }
        });
        let harness = CodexAppServerHarness::new_with_transport(
            "unused",
            CodexTransportConfig::remote(format!("ws://{address}/"), None),
        );
        let discovery = Box::pin(harness.update_hook_state(&CodexHookStateUpdate {
            cwd: "C:\\workspace".into(),
            key: "project:preToolUse:0".into(),
            expected_current_hash: current_hash.clone(),
            enabled: Some(true),
            trust_current_definition: true,
        }))
        .await
        .expect("update hook state");
        assert_eq!(
            batch_rx.await.expect("batch write capture"),
            json!({
                "edits": [{
                    "keyPath": "hooks.state",
                    "value": {
                        "project:preToolUse:0": {
                            "enabled": true,
                            "trusted_hash": current_hash
                        }
                    },
                    "mergeStrategy": "upsert"
                }],
                "filePath": null,
                "expectedVersion": null,
                "reloadUserConfig": true
            })
        );
        let hook = &discovery.hooks[0].hooks[0];
        assert!(hook.enabled);
        assert_eq!(hook.trust_status, "trusted");
        server.await.expect("hook fixture server completion");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn invocation_remote_unix_socket_uses_the_codex_rpc_handshake() {
        let temporary = tempfile::tempdir().expect("create Unix socket fixture directory");
        let socket_path = temporary.path().join("app-server-control.sock");
        let listener =
            tokio::net::UnixListener::bind(&socket_path).expect("bind Unix socket fixture");
        let (request_uri_tx, request_uri_rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.expect("accept Unix fixture client");
            let mut request_uri_tx = Some(request_uri_tx);
            let mut socket = async_tungstenite::tokio::accept_hdr_async(
                socket,
                move |request: &async_tungstenite::tungstenite::handshake::server::Request,
                      response| {
                    if let Some(sender) = request_uri_tx.take() {
                        let _ = sender.send(request.uri().to_string());
                    }
                    Ok(response)
                },
            )
            .await
            .expect("upgrade Unix fixture websocket");
            while let Some(message) = socket.next().await {
                let Message::Text(text) = message.expect("fixture Unix websocket message") else {
                    continue;
                };
                let request: Value =
                    serde_json::from_str(&text).expect("fixture Unix JSON-RPC request");
                let Some(id) = request.get("id").cloned() else {
                    continue;
                };
                let method = request
                    .get("method")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let result = if method == "account/read" {
                    json!({"account": null, "requiresOpenaiAuth": true})
                } else {
                    json!({})
                };
                socket
                    .send(Message::Text(
                        json!({"id": id, "result": result}).to_string().into(),
                    ))
                    .await
                    .expect("send Unix fixture response");
                if method == "account/read" {
                    break;
                }
            }
        });
        let endpoint = format!("unix://{}", socket_path.display());
        let harness = CodexAppServerHarness::new_with_invocation_remote("unused", endpoint, None)
            .expect("Unix invocation remote harness");
        let account = harness
            .account()
            .await
            .expect("Unix remote account request");
        assert!(!account.authenticated);
        assert_eq!(
            request_uri_rx.await.expect("Unix request URI capture"),
            "/rpc"
        );
        server.await.expect("Unix fixture server completion");
    }

    #[test]
    fn invocation_remote_validation_requires_root_explicit_port_and_secure_scope() {
        assert!(validate_invocation_remote_transport("ws://127.0.0.1:4500", None).is_ok());
        assert!(
            validate_invocation_remote_transport(
                "wss://codex.example.test:443",
                Some("fixture-token")
            )
            .is_ok()
        );
        for endpoint in [
            "ws://127.0.0.1",
            "wss://codex.example.test",
            "ws://127.0.0.1:4500/rpc",
            "ws://127.0.0.1:4500/?query=1",
            "ws://example.test:4500",
        ] {
            assert!(
                validate_invocation_remote_transport(endpoint, None).is_err(),
                "{endpoint} must be rejected"
            );
        }
        assert!(
            validate_invocation_remote_transport(
                "wss://codex.example.test:443",
                Some("line\nbreak")
            )
            .is_err()
        );
        #[cfg(windows)]
        assert!(normalize_invocation_remote_endpoint("unix://", None).is_err());
    }

    #[test]
    fn cloud_apply_arguments_preserve_only_structured_supported_overrides() {
        assert_eq!(
            cloud_apply_arguments(
                "task-123",
                &["model_verbosity=\"high\"".into()],
                &["memories".into()],
                &["sqlite".into()],
            ),
            [
                "apply",
                "-c",
                "model_verbosity=\"high\"",
                "--enable",
                "memories",
                "--disable",
                "sqlite",
                "task-123",
            ]
        );
    }

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
    fn windows_sandbox_config_write_matches_current_codex_semantics() {
        let elevated = windows_sandbox_config_write_request(CodexWindowsSandboxSetupMode::Elevated);
        assert_eq!(elevated["edits"][0]["keyPath"], "windows.sandbox");
        assert_eq!(elevated["edits"][0]["value"], "elevated");
        assert_eq!(elevated["edits"][1]["value"], Value::Null);
        assert_eq!(
            elevated["edits"][3]["keyPath"],
            "features.enable_experimental_windows_sandbox"
        );
        assert_eq!(elevated["reloadUserConfig"], true);

        let unelevated =
            windows_sandbox_config_write_request(CodexWindowsSandboxSetupMode::Unelevated);
        assert_eq!(unelevated["edits"][0]["value"], "unelevated");
        assert_eq!(unelevated["edits"].as_array().map(Vec::len), Some(4));
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
        assert_eq!(
            ask.turn_sandbox(true),
            Some(json!({"type": "workspaceWrite"}))
        );

        let auto = CodexExecutionPreferences {
            approval_mode: CodexApprovalMode::AutoReview,
            ..Default::default()
        };
        assert_eq!(auto.approval_policy(true), "on-request");
        assert_eq!(auto.approvals_reviewer(), "auto_review");
        assert_eq!(
            auto.turn_sandbox(true),
            Some(json!({"type": "workspaceWrite"}))
        );

        let full = CodexExecutionPreferences {
            approval_mode: CodexApprovalMode::FullAccess,
            ..Default::default()
        };
        assert_eq!(full.approval_policy(true), "never");
        assert_eq!(full.approvals_reviewer(), "user");
        assert_eq!(
            full.turn_sandbox(true),
            Some(json!({"type": "dangerFullAccess"}))
        );

        assert_eq!(full.approval_policy(false), "never");
        assert_eq!(full.turn_sandbox(false), Some(json!({"type": "readOnly"})));

        let exact = CodexExecutionPreferences {
            approval_mode: CodexApprovalMode::Ask,
            approval_policy: Some(CodexApprovalPolicy::Untrusted),
            sandbox_mode: Some(CodexSandboxMode::ReadOnly),
            ..Default::default()
        };
        assert_eq!(exact.approval_policy(true), "untrusted");
        assert_eq!(exact.turn_sandbox(true), Some(json!({"type": "readOnly"})));

        let expanded = CodexExecutionPreferences {
            approval_mode: CodexApprovalMode::Ask,
            writable_roots: vec!["C:/shared-one".into(), "C:/shared-two".into()],
            ..Default::default()
        };
        assert_eq!(
            expanded.turn_sandbox(true),
            Some(json!({
                "type": "workspaceWrite",
                "writableRoots": ["C:/shared-one", "C:/shared-two"]
            }))
        );
        assert_eq!(
            expanded.turn_sandbox(false),
            Some(json!({"type": "readOnly"}))
        );

        let named = CodexExecutionPreferences {
            permission_profile: Some("locked-down".into()),
            ..Default::default()
        };
        assert_eq!(named.thread_sandbox(true), None);
        assert_eq!(named.turn_sandbox(true), None);
        assert_eq!(
            named.effective_permission_profile(true),
            Some("locked-down")
        );
        assert_eq!(named.effective_permission_profile(false), None);
        assert_eq!(named.turn_sandbox(false), Some(json!({"type": "readOnly"})));
    }

    #[test]
    fn structured_turn_payload_constrains_only_the_requested_turn() {
        let protocol: Value = serde_json::from_str(include_str!(
            "../../../protocol/codex-app-server/0.150.1/codex_app_server_protocol.v2.schemas.json"
        ))
        .expect("pinned protocol schema");
        assert!(
            protocol
                .pointer("/definitions/TurnStartParams/properties/outputSchema")
                .is_some(),
            "pinned Codex protocol must expose turn/start outputSchema"
        );
        let preferences = CodexExecutionPreferences {
            model: Some("gpt-5.6-terra".into()),
            effort: Some("medium".into()),
            ..Default::default()
        };
        let schema = json!({
            "type": "object",
            "properties": {"summary": {"type": "string"}},
            "required": ["summary"],
            "additionalProperties": false
        });
        let params = turn_start_params(
            "thread-structured",
            "Summarize this",
            &[],
            &preferences,
            false,
            Some(&schema),
            false,
        );
        assert_eq!(params["outputSchema"], schema);
        assert_eq!(params["collaborationMode"], Value::Null);
        assert_eq!(params["input"][0]["text"], "Summarize this");

        let ordinary = turn_start_params(
            "thread-text",
            "Answer normally",
            &[],
            &preferences,
            false,
            None,
            false,
        );
        assert_eq!(ordinary["outputSchema"], Value::Null);
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
            profile_config: Some(json!({
                "model_verbosity": "medium",
                "history": {"persistence": "save-all"},
                "features": {"memories": false, "browser_use": true},
                "web_search": "disabled"
            })),
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
    fn profile_sanitization_removes_authority_and_secret_bearing_settings() {
        let mut config = json!({
            "model": "gpt-5.6-terra",
            "model_verbosity": "low",
            "sandbox_mode": "danger-full-access",
            "mcp_servers": {"unsafe": {"command": "anything"}},
            "history": {"persistence": "save-all", "bearer_token": "secret"},
            "nested": {"api-key": "secret", "safe": true}
        });
        let mut ignored = Vec::new();
        sanitize_profile_config(&mut config, &mut Vec::new(), &mut ignored);
        ignored.sort();
        assert_eq!(config["model"], "gpt-5.6-terra");
        assert_eq!(config["model_verbosity"], "low");
        assert_eq!(config["history"]["persistence"], "save-all");
        assert_eq!(config["nested"]["safe"], true);
        assert!(config.get("sandbox_mode").is_none());
        assert!(config.get("mcp_servers").is_none());
        assert!(config["history"].get("bearer_token").is_none());
        assert_eq!(
            ignored,
            vec![
                "history.bearer_token",
                "mcp_servers",
                "nested.api-key",
                "sandbox_mode"
            ]
        );
    }

    #[test]
    fn profile_names_are_bounded_identifiers() {
        for valid in ["team", "team.fast", "team_1", "team-prod"] {
            validate_profile_name(valid).expect("valid profile name");
        }
        for invalid in ["", ".", "..", "../team", "team/config", "team config"] {
            assert!(validate_profile_name(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn resume_directory_preference_accepts_only_codex_values() {
        assert_eq!(parse_resume_cwd(None).expect("unset"), None);
        assert_eq!(
            parse_resume_cwd(Some(&json!("current"))).expect("current"),
            Some(CodexResumeCwd::Current)
        );
        assert_eq!(
            parse_resume_cwd(Some(&json!("session"))).expect("session"),
            Some(CodexResumeCwd::Session)
        );
        assert!(parse_resume_cwd(Some(&json!("elsewhere"))).is_err());
        assert!(parse_resume_cwd(Some(&json!(true))).is_err());
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
        assert_eq!(
            chat["mcp_servers"]["mcg_agent"]["default_tools_approval_mode"],
            "approve"
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

    #[tokio::test]
    #[ignore = "qualification gate: requires the packaged pinned Codex binary"]
    async fn installed_pinned_binary_lists_permission_profiles() {
        let executable = std::env::var_os("MCG_CODEX_APP_SERVER_EXECUTABLE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("codex"));
        let harness = CodexAppServerHarness::new(executable);
        let cwd = std::env::current_dir().expect("current directory");
        let cwd = cwd.to_string_lossy();
        let profiles = harness
            .permission_profiles(Some(cwd.as_ref()))
            .await
            .expect("Codex permission profiles");
        let mut ids = BTreeSet::new();
        for profile in profiles {
            assert!(!profile.id.trim().is_empty());
            assert!(ids.insert(profile.id), "duplicate Codex permission profile");
        }
    }

    #[tokio::test]
    #[ignore = "qualification gate: requires the packaged pinned Codex binary"]
    async fn installed_pinned_binary_accepts_current_strict_configuration() {
        let executable = std::env::var_os("MCG_CODEX_APP_SERVER_EXECUTABLE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("codex"));
        let harness = CodexAppServerHarness::new(executable);
        let cwd = std::env::current_dir().expect("current directory");
        harness
            .validate_strict_config(Some(&cwd))
            .await
            .expect("current Codex configuration should pass its native strict loader");
    }

    #[tokio::test]
    #[ignore = "qualification gate: requires the packaged pinned Codex binary"]
    async fn installed_pinned_binary_reads_resume_directory_preference() {
        let executable = std::env::var_os("MCG_CODEX_APP_SERVER_EXECUTABLE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("codex"));
        let harness = CodexAppServerHarness::new(executable);
        let cwd = std::env::current_dir().expect("current directory");
        let cwd = cwd.to_string_lossy();
        let preference = harness
            .resume_cwd(Some(cwd.as_ref()), None, false)
            .await
            .expect("Codex resume working-directory preference");
        assert!(matches!(
            preference,
            None | Some(CodexResumeCwd::Current | CodexResumeCwd::Session)
        ));
    }

    #[tokio::test]
    #[ignore = "qualification gate: requires the packaged pinned Codex binary"]
    async fn installed_pinned_binary_loads_named_runtime_profiles() {
        let executable = std::env::var_os("MCG_CODEX_APP_SERVER_EXECUTABLE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("codex"));
        let root = std::env::temp_dir().join(format!(
            "metacog-codex-profile-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir(&root).expect("temporary Codex home");
        let profile = root.join("team.config.toml");
        fs::write(&profile, "model_verbosity = \"low\"\n").expect("profile fixture");
        let output = Command::new(executable)
            .env("CODEX_HOME", &root)
            .args(["--profile", "team", "mcp", "list"])
            .output()
            .await
            .expect("run pinned Codex profile loader");
        fs::remove_file(profile).expect("remove profile fixture");
        fs::remove_dir(root).expect("remove temporary Codex home");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
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
    fn fork_payload_preserves_inheritance_unless_exact_overrides_are_supplied() {
        let inherited = thread_fork_params("thread-1", Some("turn-1"), None, false);
        assert_eq!(inherited["threadId"], "thread-1");
        assert_eq!(inherited["lastTurnId"], "turn-1");
        assert!(inherited.get("modelProvider").is_none());
        assert!(inherited.get("serviceTier").is_none());

        let local = CodexExecutionPreferences {
            model_provider: Some("ollama".into()),
            model: Some("gpt-oss:20b".into()),
            service_tier: None,
            approval_mode: CodexApprovalMode::Ask,
            ..CodexExecutionPreferences::default()
        };
        let overridden = thread_fork_params("thread-1", None, Some(&local), true);
        assert_eq!(overridden["modelProvider"], "ollama");
        assert_eq!(overridden["model"], "gpt-oss:20b");
        assert!(overridden["serviceTier"].is_null());
        assert_eq!(overridden["approvalPolicy"], "on-request");
        assert_eq!(overridden["sandbox"], "workspace-write");
    }

    #[test]
    fn v031_uses_only_stable_pinned_methods() {
        let schema = include_str!(
            "../../../protocol/codex-app-server/0.150.1/codex_app_server_protocol.v2.schemas.json"
        );
        for method in [
            "review/start",
            "permissionProfile/list",
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
        let schema_value: Value = serde_json::from_str(schema).expect("pinned protocol schema");
        for definition in ["ThreadStartParams", "ThreadResumeParams", "TurnStartParams"] {
            assert!(
                schema_value
                    .pointer(&format!("/definitions/{definition}/properties/permissions"))
                    .is_some(),
                "missing named permissions field in {definition}"
            );
        }
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
