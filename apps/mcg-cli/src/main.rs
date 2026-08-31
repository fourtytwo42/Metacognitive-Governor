use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs::OpenOptions,
    io::{IsTerminal, Read, Write},
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, Stdio},
    str::FromStr,
};

mod commands;
mod exec_jsonl;
mod interactive;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::{DateTime, TimeDelta, Utc};
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{Shell, generate};
use eventsource_stream::Eventsource as _;
use futures_util::StreamExt as _;
use mcg_agents::{
    AgentRunState, ExperimentBenchmark, ExperimentDecision, ExperimentDefinition, ExperimentState,
    GovernorProposalState, OrganizationGenome, OrganizationSharePackage,
    PortableOrganizationComparison, PortableOrganizationGenome, RoleKind, ShadowExecutionReceipt,
    SuccessCondition, SupervisionObservation, TaskComplexity,
};
use mcg_api::{
    AnswerConversationInputRequest, ApplyCloudDiffRequest, ApplyComputerActionsRequest,
    ApprovalListResponse, ArchiveConversationRequest, ArchiveProjectRequest,
    AssembleContextRequest, AttachBrowserSessionRequest, BackupListResponse, BackupSummaryResponse,
    CheckSiteHealthRequest, CodexResumeCwdQuery, ConversationDeletionPreview,
    ConversationDeletionResult, ConversationListResponse, ConversationStreamEvent,
    CorrectMemoryClaimRequest, CreateConversationRequest, CreateEncryptedBackupRequest,
    CreateMemoryClaimRequest, CreateOrganizationRequest, CreatePlatformSessionRequest,
    CreateScheduleRequest, CreateSiteMonitorRequest, CreateSiteRequest, CreateSshHostRequest,
    CreateTaskRequest, DecideSharedOrganizationRequest, DelegationMutationRequest,
    DelegationRetryRequest, DeploySiteRequest, DiscoverProjectRequest, EvaluationMutationResponse,
    ExecuteAgentWorkRequest, ExperimentBenchmarkListResponse, ExperimentDecisionListResponse,
    ExperimentListResponse, ForkConversationRequest, GlobalAgentSessionListResponse,
    ImportOrganizationRequest, ImportSharedOrganizationRequest, InheritOrganizationRequest,
    InstallLocalPluginRequest, InstallRegistryPluginRequest, InvokePluginRequest,
    LaunchTaskRequest, LaunchTaskResponse, ManagedMemoryResponse, McpListResponse,
    MemoryClaimHistoryResponse, MemoryClaimListResponse, MentionCandidateListResponse,
    NavigateBrowserSessionRequest, OrganizationHistoryResponse, OrganizationListResponse,
    OrganizationMutationResponse, OrganizationPublisherListResponse,
    OrganizationShareReviewListResponse, PlanAgentRunRequest, PlanWorkRequest,
    PlatformSessionHistoryResponse, PlatformSessionListResponse, PluginInvocationHistoryResponse,
    PluginListResponse, PrepareCloudDiffApplyRequest, ProjectHistoryResponse, ProjectListResponse,
    ProjectMemoryTransferListResponse, ProviderListResponse, ResolveApprovalRequest,
    ReviewSharedOrganizationRequest, ReviewedTopologyPlanListResponse,
    RevokeOrganizationPublisherRequest, RunSshOperationRequest, ScheduleHistoryResponse,
    ScheduleListResponse, SchedulerNotificationListResponse, SendConversationMessageRequest,
    SetPluginEnabledRequest, SetPluginGrantsRequest, SetScheduleStateRequest,
    ShadowExecutionListResponse, ShutdownResponse, SiteDeploymentListResponse,
    SiteHealthListResponse, SiteListResponse, SiteRevisionListResponse, SshHostListResponse,
    SshHostRevisionListResponse, SshOperationListResponse, SshWorkflowListResponse,
    StartOpenAiOAuthRequest, SteerConversationTurnRequest, SupervisionObservationListResponse,
    TaskEventsResponse, TaskListResponse, TaskMutationResponse, ToolAuditResponse,
    TransferProjectMemoryRequest, TransitionAgentRunRequest, TransitionExperimentRequest,
    TransitionMemoryClaimRequest, TransitionTaskRequest, TrustOrganizationPublisherRequest,
    UpdateConversationGoalRequest, UpdateConversationModeRequest, UpdateConversationRequest,
    UpdateSiteRequest, UpdateSshHostRequest, UpdateUserSettingsRequest,
    UserSettingsHistoryResponse, WakeScheduleRequest, WorktreeMutationRequest,
};
use mcg_codex_app_server::{
    CodexConfigurationStatus, CodexHookMetadata, CodexHookStateUpdate, CodexHooksListEntry,
    CodexResumeCwd, CodexStableDiscovery,
};
use mcg_config::{AppConfig, AutonomyPreset, CliTuiPreferences, InterfaceComplexity, UserSettings};
use mcg_contracts::{
    ActionId, InvocationId, OwnerActionCompensationRequest, WorkflowCancelRequest, WorkflowId,
    WorkflowState,
};
use mcg_domain::{
    AgentWakeState, ApprovalMode, CloudDiffApply, CodexConfigOverride, ComposerMentionInput,
    ComputerUseCapability, ComputerUseRegion, ComputerUseTarget, Conversation,
    ConversationAgentSessionId, ConversationGoalStatus, ConversationId,
    ConversationInteractionKind, ConversationMode, ConversationRole, ConversationState,
    ConversationTurnId, ConversationTurnState, ConversationView, CoordinationBatchState,
    ExecutionApprovalPolicy, ExecutionPreferences, ExecutionSandboxMode, MemoryClaimId,
    MemoryTransferId, MentionCandidate, MentionKind, NotificationSeverity, OrganizationId,
    OrganizationImportId, OrganizationRevisionId, OrganizationShareReviewId,
    PlatformCapabilitySnapshot, PlatformSession, PlatformSessionId, PlatformSessionKind,
    PluginCapabilityGrant, PluginInstallSource, PluginInstallation, PluginInvocationId,
    PluginRevision, PluginRevisionId, PluginSignature, PluginSignatureState, ProjectId,
    ReviewedTopologyPlanId, RunId, ScheduleId, ScheduleState, ScheduleTrigger,
    SchedulerNotification, SchedulerNotificationId, Site, SiteDeploymentCommand, SiteDeploymentId,
    SiteEnvironment, SiteHealthCheckId, SiteId, SiteMonitor, SiteState, SshHost, SshHostId,
    SshWorkflow, SshWorkflowId, TaskEnvelope, TaskId, TaskState, WorkItemId,
    effective_delegation_attempt_ids,
};
use mcg_events::EventEnvelope;
use mcg_mcp::{
    MCP_CONNECTION_CONTRACT_VERSION, McpAuthMode, McpConnection, McpProtocolMode, McpToolCatalog,
    McpTransport, OsMcpCredentialStore,
};
use mcg_plugin_protocol::PluginManifest;
use mcg_projects::{ClaimProvenance, MemoryLifecycleAction, ProvenanceKind};
use mcg_providers::{
    CONTRACT_VERSION, ModelProbeReport, OsProviderCredentialStore, ProviderAuthMode,
    ProviderConnection, ProviderKind, ProviderModelCatalog, ProxyConversation, ProxyUsageSummary,
};
use mcg_runtime::{
    AgentRunView, AgentTurnCommit, CandidateRevisionMutation, DiagnosticReport, DiagnosticSeverity,
    EvidenceMutation, ExperimentMutation, McpMutation, MemoryClaimMutation, OAuthLoginStart,
    OpenAiOAuthMethod, OrganizationPublisherMutation, OrganizationShareImportMutation,
    OrganizationShareReviewMutation, PlatformBrowserMutation, PlatformComputerActionMutation,
    PlatformComputerMutation, PluginInvocationMutation, PortableOrganizationImportMutation,
    ProjectMemoryTransferMutation, ProjectMemoryTransferPreview, ProjectMutation, ProviderMutation,
    ReviewedTopologyPlanCreateRequest, ReviewedTopologyPlanMutation, RunReplayReport,
    RuntimeStatus, SiteDeploymentMutation, SiteHealthMutation, SiteMonitorMutation, SiteMutation,
    SshHostMutation, SshOperationMutation, SshWorkflowCreateRequest, SshWorkflowMutation,
    SupervisionMutation,
};
use mcg_storage::{
    BackupPassphrase, restore_backup_bundle, restore_encrypted_backup_bundle, verify_backup_bundle,
};
use mcg_tools::{
    ApprovalRequest, ApprovalState, ComputerUseFrame, PersistentBrowserMode,
    PersistentBrowserObservation,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

use commands::{CommandContext, QueuePolicy, SLASH_COMMANDS, command_named};
use exec_jsonl::{ExecJsonlEvent, ExecJsonlProjector};
use interactive::{
    PaletteCandidate, PaletteKind, PaletteRequest, SessionPickerCandidate, SlashCandidate,
    SubmittedLine, TuiAction, TuiModel, TuiTerminal, prompt_for_question, select_agent_session,
    select_local_provider, select_resume_cwd, select_session,
};

#[derive(Serialize)]
struct ProviderConfigurePayload {
    connection: ProviderConnection,
    credential: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct ProxyConversationListResponse {
    conversations: Vec<ProxyConversation>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
struct HookSelectionToken {
    cwd: String,
    key: String,
    current_hash: String,
}

fn encode_hook_selection(entry: &CodexHooksListEntry, hook: &CodexHookMetadata) -> String {
    STANDARD.encode(
        serde_json::to_vec(&HookSelectionToken {
            cwd: entry.cwd.clone(),
            key: hook.key.clone(),
            current_hash: hook.current_hash.clone(),
        })
        .expect("hook selection token is serializable"),
    )
}

fn decode_hook_selection(value: &str) -> Result<HookSelectionToken, String> {
    let bytes = STANDARD
        .decode(value)
        .map_err(|_| "the hook selection token is invalid; reopen /hooks".to_owned())?;
    serde_json::from_slice(&bytes)
        .map_err(|_| "the hook selection token is invalid; reopen /hooks".to_owned())
}

fn hook_handler_summary(hook: &CodexHookMetadata) -> String {
    match hook.handler_type.as_str() {
        "command" => hook
            .command
            .as_deref()
            .unwrap_or("command")
            .chars()
            .take(96)
            .collect(),
        "mcpTool" => format!(
            "{} / {}",
            hook.server.as_deref().unwrap_or("MCP"),
            hook.tool.as_deref().unwrap_or("tool")
        ),
        value => value.to_owned(),
    }
}

#[derive(Serialize)]
struct McpConfigurePayload {
    connection: McpConnection,
    credential: Option<String>,
}

impl Drop for McpConfigurePayload {
    fn drop(&mut self) {
        if let Some(credential) = &mut self.credential {
            credential.zeroize();
        }
    }
}

impl Drop for ProviderConfigurePayload {
    fn drop(&mut self) {
        if let Some(credential) = &mut self.credential {
            credential.zeroize();
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "mcg",
    version,
    about = "MetaCognitive Governor CLI",
    after_help = "Consumer usage:\n  mcg\n  mcg \"fix the failing tests\"\n  mcg -C PATH \"build and verify the website\"\n  git diff | mcg \"review this diff\"\n  mcg -C PATH run --json \"fix issue 419\"\n  mcg -C PATH run --stream-json \"fix issue 419\""
)]
struct Cli {
    #[arg(long, default_value = "http://127.0.0.1:7777")]
    host: String,
    /// Override the local daemon bearer-token file.
    #[arg(long)]
    token_file: Option<PathBuf>,
    #[command(flatten)]
    remote: InteractiveRemoteArgs,
    /// Select the project directory for consumer work (defaults to the current directory).
    #[arg(short = 'C', long = "project", global = true)]
    project: Option<PathBuf>,
    /// Model used for a new interactive session or bare prompt.
    #[arg(short = 'm', long, global = true)]
    model: Option<String>,
    /// Use a local open-source model provider.
    #[arg(long, global = true)]
    oss: bool,
    /// Select the local provider used with --oss.
    #[arg(long = "local-provider", value_enum, global = true)]
    local_provider: Option<CodexLocalProviderArg>,
    /// Sandbox applied to model-generated commands.
    #[arg(short = 's', long, value_enum, global = true)]
    sandbox: Option<CodexSandboxArg>,
    /// Configure when model-generated commands require approval.
    #[arg(short = 'a', long = "ask-for-approval", value_enum, global = true)]
    approval_policy: Option<CodexApprovalPolicyArg>,
    /// Route approval requests through automatic review using workspace-write sandboxing.
    #[arg(long, global = true, conflicts_with_all = ["approval_policy", "dangerously_bypass_approvals_and_sandbox"])]
    approve_for_me: bool,
    /// Disable both approval prompts and sandboxing for this new session.
    #[arg(long, alias = "yolo", global = true)]
    dangerously_bypass_approvals_and_sandbox: bool,
    /// Run enabled hooks without reviewing their current definitions for this invocation only.
    #[arg(long = "dangerously-bypass-hook-trust", global = true)]
    dangerously_bypass_hook_trust: bool,
    /// Attach image files to the first prompt or interactive draft.
    #[arg(short = 'i', long = "image", value_name = "FILE", global = true)]
    images: Vec<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Show daemon status.
    Status {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Start the sibling MCG daemon and wait until its authenticated API is ready.
    Start {
        /// Seconds to wait for authenticated readiness.
        #[arg(long, default_value_t = 15)]
        timeout_seconds: u64,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Cleanly restart the sibling MCG daemon and wait until it is ready.
    Restart {
        /// Seconds to wait for shutdown and authenticated readiness.
        #[arg(long, default_value_t = 20)]
        timeout_seconds: u64,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Ask the authenticated local daemon to drain and stop cleanly.
    Stop {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Run secret-free local production-readiness checks.
    Doctor {
        /// Emit the versioned report as JSON.
        #[arg(long)]
        json: bool,
        /// Return a non-zero exit status for warnings as well as errors.
        #[arg(long)]
        strict: bool,
    },
    /// Inspect the Protected Root process, protocol, storage, and migration boundary.
    Runtime {
        #[command(subcommand)]
        command: RuntimeCommand,
    },
    /// Inspect and cancel canonical Protected Root workflows.
    Workflow {
        #[command(subcommand)]
        command: WorkflowCommand,
    },
    /// Inspect canonical Protected Root actions and their evidence.
    Action {
        #[command(subcommand)]
        command: ActionCommand,
    },
    /// Inspect reconstructable Protected Root model invocations.
    Invocation {
        #[command(subcommand)]
        command: InvocationCommand,
    },
    /// Inspect or change restart-safe owner settings.
    Settings {
        #[command(subcommand)]
        command: SettingsCommand,
    },
    /// Create, inspect, and restore portable secret-free backups.
    Backup {
        #[command(subcommand)]
        command: BackupCommand,
    },
    /// Create and inspect durable tasks.
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
    /// Build and manage projects through the Chat → Coordinator → Coder workflow.
    Conversation {
        #[command(subcommand)]
        command: ConversationCommand,
    },
    /// Inspect and control the pinned Codex app-server account and model surface.
    Codex {
        #[command(subcommand)]
        command: CodexCommand,
    },
    /// Connect GitHub and manage repositories linked to MCG projects.
    Github {
        #[command(subcommand)]
        command: GithubCommand,
    },
    /// Inspect and operate bounded dormant-Governor proposals.
    Governor {
        #[command(subcommand)]
        command: GovernorCommand,
    },
    /// Inspect and resolve owner approval requests.
    Approval {
        #[command(subcommand)]
        command: ApprovalCommand,
    },
    /// Discover projects, assemble context, and manage durable memory.
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Create and inspect versioned agent organizations.
    Organization {
        #[command(subcommand)]
        command: OrganizationCommand,
    },
    /// Run a consumer goal or manage lower-level durable agent runs.
    Run {
        #[command(subcommand)]
        command: Option<RunCommand>,
        /// Emit a completed consumer submission as one JSON document.
        #[arg(long)]
        json: bool,
        /// Emit task events and the final result as newline-delimited JSON.
        #[arg(long, conflicts_with = "json")]
        stream_json: bool,
        /// Codex model for the governed Chat/Coordinator/Coder workflow.
        #[arg(long)]
        model: Option<String>,
        /// Model reasoning effort.
        #[arg(long)]
        effort: Option<String>,
        /// Model service tier/speed.
        #[arg(long)]
        speed: Option<String>,
        /// Integration permission for generated changes.
        #[arg(long, value_enum, default_value_t = ConversationApprovalArg::FullAccess)]
        approval: ConversationApprovalArg,
        /// Attach a typed reference (`file:path`, `folder:path`, or `plugin:id`). Repeatable.
        #[arg(long = "mention", value_name = "KIND:RESOURCE")]
        mentions: Vec<String>,
        /// Consumer goal when no administrative run subcommand is supplied.
        #[arg(value_name = "GOAL", num_args = 0..)]
        goal: Vec<String>,
    },
    /// Configure and discover model providers.
    Provider {
        #[command(subcommand)]
        command: ProviderCommand,
    },
    /// Configure and discover Model Context Protocol servers.
    Mcp {
        #[command(subcommand)]
        command: McpCommand,
    },
    /// Install, inspect, authorize, and invoke sandboxed plugins.
    Plugin {
        #[command(subcommand)]
        command: PluginCommand,
    },
    /// Create and control durable unattended work.
    Schedule {
        #[command(subcommand)]
        command: ScheduleCommand,
    },
    /// Inspect host capabilities and reserve browser/computer sessions.
    Platform {
        #[command(subcommand)]
        command: PlatformCommand,
    },
    /// Register, inspect, monitor, and deploy durable Sites.
    Site {
        #[command(subcommand)]
        command: SiteCommand,
    },
    /// Manage pinned outbound SSH hosts and typed operations.
    Ssh {
        #[command(subcommand)]
        command: SshCommand,
    },
    /// Inspect and control measured metacognitive experiments.
    Experiment {
        #[command(subcommand)]
        command: ExperimentCommand,
    },
    /// Treat an otherwise unknown command line as a consumer goal.
    #[command(external_subcommand)]
    Prompt(Vec<OsString>),
}

#[derive(Debug, Parser)]
#[command(name = "mcg", version, about = "MetaCog Codex-compatible CLI commands")]
struct CodexCompatCli {
    #[arg(long, default_value = "http://127.0.0.1:7777")]
    host: String,
    #[arg(long)]
    token_file: Option<PathBuf>,
    #[command(flatten)]
    remote: InteractiveRemoteArgs,
    #[arg(short = 'C', long = "cd", visible_alias = "project", global = true)]
    project: Option<PathBuf>,
    #[arg(short = 'c', long = "config", value_name = "KEY=VALUE", global = true)]
    config: Vec<String>,
    #[arg(short = 'm', long, global = true)]
    model: Option<String>,
    #[arg(long, global = true)]
    oss: bool,
    #[arg(long = "local-provider", value_enum, global = true)]
    local_provider: Option<CodexLocalProviderArg>,
    #[arg(short = 's', long, value_enum, global = true)]
    sandbox: Option<CodexSandboxArg>,
    #[arg(short = 'a', long = "ask-for-approval", value_enum, global = true)]
    approval_policy: Option<CodexApprovalPolicyArg>,
    #[arg(long, global = true, conflicts_with_all = ["approval_policy", "dangerously_bypass_approvals_and_sandbox"])]
    approve_for_me: bool,
    #[arg(long, alias = "yolo", global = true)]
    dangerously_bypass_approvals_and_sandbox: bool,
    #[arg(long = "dangerously-bypass-hook-trust", global = true)]
    dangerously_bypass_hook_trust: bool,
    #[arg(short = 'i', long = "image", value_name = "FILE", global = true)]
    images: Vec<PathBuf>,
    #[arg(long = "add-dir", value_name = "DIR", global = true)]
    add_dirs: Vec<PathBuf>,
    #[arg(long = "enable", value_name = "FEATURE", global = true)]
    enable_features: Vec<String>,
    #[arg(long = "disable", value_name = "FEATURE", global = true)]
    disable_features: Vec<String>,
    #[command(subcommand)]
    command: CodexCompatCommand,
}

#[derive(Debug, Subcommand)]
enum CodexCompatCommand {
    /// Browse all governed agent sessions on the shared MetaCog daemon.
    Agents(Box<AgentsArgs>),
    /// Hand a workspace path to the MetaCog desktop app.
    App(Box<AppArgs>),
    /// Run MetaCog non-interactively through the governed agent workflow.
    #[command(alias = "e")]
    Exec(Box<ExecArgs>),
    /// Apply the latest diff produced by a Codex cloud task to the current project.
    #[command(alias = "a")]
    Apply(ApplyArgs),
    /// Check for and apply an owner-signed MetaCog release update.
    Update,
    /// Continue a previous interactive conversation.
    Resume(Box<ResumeArgs>),
    /// Queue a message for an existing conversation.
    Queue(Box<QueueArgs>),
    /// Inspect or migrate legacy Codex sessions to paginated thread history.
    MigrateRollouts(Box<MigrateRolloutsArgs>),
    /// Fork a previous conversation and open the fork in the terminal UI.
    Fork(Box<ForkArgs>),
    /// Archive a saved conversation by ID or exact name.
    Archive(Box<SessionArchiveArgs>),
    /// Restore an archived conversation by ID or exact name.
    Unarchive(Box<SessionArchiveArgs>),
    /// Permanently delete a saved conversation.
    Delete(Box<DeleteArgs>),
    /// Run a native, read-only code review.
    Review(Box<ReviewArgs>),
    /// Generate a shell-completion script on standard output.
    Completion { shell: CompletionShell },
    /// Authenticate the shared Codex app-server account.
    Login(Box<LoginArgs>),
    /// Remove shared Codex authentication.
    Logout,
    /// Inspect feature flags exposed by the pinned Codex runtime.
    Features {
        #[command(subcommand)]
        command: FeaturesCommand,
    },
    /// Run an owner-supplied command inside the pinned Codex Windows sandbox.
    Sandbox(Box<SandboxArgs>),
}

#[derive(Debug, Args)]
struct ApplyArgs {
    /// Codex cloud task identifier whose latest diff should be applied.
    task_id: String,
}

#[derive(Debug, Args)]
#[command(trailing_var_arg = true)]
struct SandboxArgs {
    /// JSON value returned by `codex/sandbox-state-meta` to apply directly.
    #[arg(long)]
    sandbox_state_json: Option<String>,
    /// Add a readable root to the supplied sandbox state. Repeatable.
    #[arg(long = "sandbox-state-readable-root")]
    sandbox_state_readable_roots: Vec<PathBuf>,
    /// Disable direct network access in the supplied sandbox state.
    #[arg(long)]
    sandbox_state_disable_network: bool,
    /// Named permissions profile from the active Codex configuration stack.
    #[arg(short = 'P', long)]
    permission_profile: Option<String>,
    /// Layer `$CODEX_HOME/<name>.config.toml` over the base Codex configuration.
    #[arg(short = 'p', long)]
    profile: Option<String>,
    /// Include managed requirements while resolving an explicit permissions profile.
    #[arg(long)]
    include_managed_config: bool,
    /// Full command and arguments to execute under the Codex sandbox.
    #[arg(value_name = "COMMAND", required = true, allow_hyphen_values = true)]
    command: Vec<OsString>,
}

#[derive(Debug, Subcommand)]
enum FeaturesCommand {
    /// List known features with their stage and effective state.
    List,
    /// Enable a feature in the shared Codex configuration.
    Enable { feature: String },
    /// Disable a feature in the shared Codex configuration.
    Disable { feature: String },
}

#[derive(Debug, Args)]
struct AppArgs {
    /// Workspace path to open in MetaCog Desktop.
    #[arg(default_value = ".")]
    path: PathBuf,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CodexSandboxArg {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum CodexLocalProviderArg {
    Ollama,
    #[value(name = "lmstudio")]
    LmStudio,
}

impl CodexLocalProviderArg {
    const fn id(self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::LmStudio => "lmstudio",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedLocalProvider {
    id: String,
    default_model: String,
}

async fn resolve_oss_provider(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    oss: bool,
    explicit: Option<CodexLocalProviderArg>,
    interactive: bool,
    no_alt_screen: bool,
) -> Result<Option<ResolvedLocalProvider>, Box<dyn std::error::Error>> {
    if !oss {
        // Codex accepts --local-provider without --oss but does not activate it.
        return Ok(None);
    }
    let status: CodexConfigurationStatus = decode_api_response(
        client
            .get(format!("{host}/api/v1/codex/configuration"))
            .bearer_auth(token)
            .send()
            .await?,
        "discover local Codex providers",
    )
    .await?;
    let resolve = |id: &str| -> Result<ResolvedLocalProvider, Box<dyn std::error::Error>> {
        let provider = status
            .local_providers
            .providers
            .iter()
            .find(|provider| provider.id == id)
            .ok_or_else(|| format!("Codex local provider `{id}` was not discovered"))?;
        Ok(ResolvedLocalProvider {
            id: provider.id.clone(),
            default_model: provider.default_model.clone(),
        })
    };
    if let Some(explicit) = explicit {
        return resolve(explicit.id()).map(Some);
    }
    if let Some(configured) = status.local_providers.configured_provider.as_deref() {
        return resolve(configured).map(Some);
    }
    if !interactive {
        return Err(
            "No default OSS provider configured. Use --local-provider=provider or set oss_provider to one of: lmstudio, ollama in config.toml"
                .into(),
        );
    }
    let running = status
        .local_providers
        .providers
        .iter()
        .filter(|provider| provider.running)
        .collect::<Vec<_>>();
    if running.len() == 1 {
        return resolve(&running[0].id).map(Some);
    }
    let candidates = status
        .local_providers
        .providers
        .iter()
        .map(|provider| SessionPickerCandidate {
            id: provider.id.clone(),
            title: provider.display_name.clone(),
            detail: if provider.running {
                format!(
                    "Running · {} · default {}",
                    provider.endpoint, provider.default_model
                )
            } else {
                format!("Not running · {}", provider.detail)
            },
        })
        .collect::<Vec<_>>();
    let selected = select_local_provider(&candidates, no_alt_screen)?
        .ok_or("local provider selection was cancelled")?;
    let _: mcg_codex_app_server::CodexLocalProviderDiscovery = decode_api_response(
        client
            .post(format!("{host}/api/v1/codex/local-provider/default"))
            .bearer_auth(token)
            .json(&serde_json::json!({ "provider": selected }))
            .send()
            .await?,
        "persist local Codex provider",
    )
    .await?;
    resolve(&selected).map(Some)
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CodexApprovalPolicyArg {
    Untrusted,
    OnRequest,
    Never,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CompletionShell {
    Bash,
    Zsh,
    Fish,
    #[value(name = "power-shell", alias = "powershell")]
    PowerShell,
    Elvish,
}

impl From<CompletionShell> for Shell {
    fn from(value: CompletionShell) -> Self {
        match value {
            CompletionShell::Bash => Self::Bash,
            CompletionShell::Zsh => Self::Zsh,
            CompletionShell::Fish => Self::Fish,
            CompletionShell::PowerShell => Self::PowerShell,
            CompletionShell::Elvish => Self::Elvish,
        }
    }
}

#[derive(Debug, Args)]
struct ExecArgs {
    #[command(subcommand)]
    command: Option<ExecCommand>,
    #[arg(long, alias = "experimental-json")]
    json: bool,
    #[arg(long)]
    ephemeral: bool,
    #[arg(long)]
    skip_git_repo_check: bool,
    #[arg(short = 'o', long)]
    output_last_message: Option<PathBuf>,
    /// JSON Schema describing the final owner-visible response.
    #[arg(long)]
    output_schema: Option<PathBuf>,
    #[arg(value_name = "PROMPT", num_args = 0..)]
    prompt: Vec<String>,
}

#[derive(Debug, Args)]
struct AgentsArgs {
    #[command(flatten)]
    remote: InteractiveRemoteArgs,
}

#[derive(Debug, Args)]
struct ResumeArgs {
    #[arg(long)]
    all: bool,
    #[arg(long)]
    include_non_interactive: bool,
    #[arg(long)]
    last: bool,
    session: Option<String>,
    #[command(flatten)]
    remote: InteractiveRemoteArgs,
}

#[derive(Debug, Args)]
struct ForkArgs {
    #[arg(long)]
    all: bool,
    #[arg(long)]
    last: bool,
    session: Option<String>,
    #[command(flatten)]
    remote: InteractiveRemoteArgs,
}

#[derive(Debug, Args)]
struct QueueArgs {
    /// Conversation UUID or exact conversation name.
    #[arg(long)]
    thread: String,
    /// Message text to queue.
    #[arg(long)]
    message: String,
    #[command(flatten)]
    remote: InteractiveRemoteArgs,
}

#[derive(Debug, Args)]
struct SessionArchiveArgs {
    session: String,
    #[command(flatten)]
    remote: InteractiveRemoteArgs,
}

#[derive(Debug, Args)]
struct MigrateRolloutsArgs {
    /// Publish the migration; otherwise report eligible sessions only.
    #[arg(long)]
    apply: bool,
    /// Restrict inspection or migration to one or more thread IDs.
    #[arg(long = "thread")]
    threads: Vec<String>,
    /// Limit aggregate rollout I/O throughput in MiB per second.
    #[arg(long)]
    max_mib_per_second: Option<u64>,
    /// Emit the complete per-thread report as JSON.
    #[arg(long)]
    json: bool,
    /// Print one line for every inspected rollout.
    #[arg(long)]
    verbose: bool,
}

#[derive(Debug, Args)]
struct DeleteArgs {
    session: String,
    #[arg(long)]
    force: bool,
    #[command(flatten)]
    remote: InteractiveRemoteArgs,
}

#[derive(Debug, Default, Args)]
struct InteractiveRemoteArgs {
    /// Connect this invocation to a remote Codex app server.
    #[arg(long = "remote", value_name = "ADDR")]
    remote: Option<String>,
    /// Environment variable containing the bearer token for the remote app server.
    #[arg(long = "remote-auth-token-env", value_name = "ENV_VAR")]
    remote_auth_token_env: Option<String>,
}

struct InvocationRemote {
    endpoint: String,
    auth_token: Option<String>,
}

fn remote_websocket_has_explicit_port(endpoint: &str, url: &reqwest::Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    if url.port().is_some() {
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
    let default_port = match url.scheme() {
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

impl Drop for InvocationRemote {
    fn drop(&mut self) {
        if let Some(auth_token) = &mut self.auth_token {
            auth_token.zeroize();
        }
    }
}

fn resolve_invocation_remote(
    root: InteractiveRemoteArgs,
    command: InteractiveRemoteArgs,
) -> Result<Option<InvocationRemote>, Box<dyn std::error::Error>> {
    let endpoint = command.remote.or(root.remote);
    let auth_token_env = command.remote_auth_token_env.or(root.remote_auth_token_env);
    let Some(endpoint) = endpoint else {
        if auth_token_env.is_some() {
            return Err("`--remote-auth-token-env` requires `--remote`.".into());
        }
        return Ok(None);
    };
    let endpoint = endpoint.trim().to_owned();
    if endpoint.starts_with("unix://") {
        if auth_token_env.is_some() {
            return Err(
                "`--remote-auth-token-env` requires a `wss://` or loopback `ws://` remote.".into(),
            );
        }
        let endpoint = mcg_codex_app_server::normalize_invocation_remote_endpoint(&endpoint, None)?;
        return Ok(Some(InvocationRemote {
            endpoint,
            auth_token: None,
        }));
    }
    let url = reqwest::Url::parse(&endpoint)
        .map_err(|error| format!("invalid remote app-server endpoint: {error}"))?;
    if !matches!(url.scheme(), "ws" | "wss") {
        return Err(
            "remote app-server endpoints must use ws://, wss://, unix://, or unix://PATH".into(),
        );
    }
    if !remote_websocket_has_explicit_port(&endpoint, &url) {
        return Err("remote WebSocket endpoints require an explicit port".into());
    }
    if url.username() != "" || url.password().is_some() {
        return Err("remote app-server endpoints cannot contain credentials".into());
    }
    if !matches!(url.path(), "" | "/") || url.query().is_some() || url.fragment().is_some() {
        return Err(
            "remote WebSocket endpoints must use the root path without query or fragment".into(),
        );
    }
    let host = url.host_str().unwrap_or_default();
    let loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    // MetaCog deliberately retains its stronger transport boundary: cleartext WebSockets never
    // cross the local host, even when no bearer token is configured.
    if url.scheme() == "ws" && !loopback {
        return Err(
            "unencrypted remote app-server endpoints are permitted only on loopback".into(),
        );
    }
    let auth_token = if let Some(variable) = auth_token_env {
        if url.scheme() != "wss" && !(url.scheme() == "ws" && loopback) {
            return Err(
                "`--remote-auth-token-env` requires a `wss://` or loopback `ws://` remote.".into(),
            );
        }
        let variable = variable.trim();
        if variable.is_empty() || variable.chars().any(char::is_whitespace) {
            return Err("remote bearer-token environment variable name is invalid".into());
        }
        let token = std::env::var(variable).map_err(|_| {
            format!("remote bearer-token environment variable `{variable}` is not set")
        })?;
        if token.is_empty() {
            return Err(
                format!("remote bearer-token environment variable `{variable}` is empty").into(),
            );
        }
        if token.len() > 16_384 || token.chars().any(char::is_control) {
            return Err("remote bearer token is invalid or exceeds 16384 bytes".into());
        }
        Some(token)
    } else {
        None
    };
    Ok(Some(InvocationRemote {
        endpoint,
        auth_token,
    }))
}

fn reject_invocation_remote_for_subcommand(
    remote: &InteractiveRemoteArgs,
    subcommand: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(endpoint) = remote.remote.as_deref() {
        return Err(format!(
            "`--remote {endpoint}` is only supported for interactive TUI commands, not `mcg {subcommand}`"
        )
        .into());
    }
    if remote.remote_auth_token_env.is_some() {
        return Err(format!(
            "`--remote-auth-token-env` is only supported for interactive TUI commands, not `mcg {subcommand}`"
        )
        .into());
    }
    Ok(())
}

async fn bind_invocation_remote(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    conversation_id: ConversationId,
    remote: &InvocationRemote,
) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .put(format!(
            "{host}/api/v1/conversations/{conversation_id}/codex-transport"
        ))
        .bearer_auth(token)
        .json(&serde_json::json!({
            "endpoint": remote.endpoint,
            "auth_token": remote.auth_token.as_deref(),
        }))
        .send()
        .await?;
    let _: mcg_codex_app_server::CodexHarnessStatus =
        decode_api_response(response, "connect remote Codex app server").await?;
    Ok(())
}

async fn release_invocation_remote(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    conversation_id: ConversationId,
) -> Result<bool, Box<dyn std::error::Error>> {
    let response: serde_json::Value = decode_api_response(
        client
            .delete(format!(
                "{host}/api/v1/conversations/{conversation_id}/codex-transport"
            ))
            .bearer_auth(token)
            .send()
            .await?,
        "release remote Codex app server",
    )
    .await?;
    Ok(response
        .get("released")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false))
}

async fn bind_invocation_hook_trust_bypass(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    conversation_id: ConversationId,
) -> Result<(), Box<dyn std::error::Error>> {
    let _: serde_json::Value = decode_api_response(
        client
            .put(format!(
                "{host}/api/v1/conversations/{conversation_id}/codex-hook-trust-bypass"
            ))
            .bearer_auth(token)
            .send()
            .await?,
        "enable invocation-only hook trust bypass",
    )
    .await?;
    Ok(())
}

fn warn_hook_trust_bypass() {
    eprintln!(
        "Warning: `--dangerously-bypass-hook-trust` is enabled. Enabled hooks may run without review for this invocation."
    );
}

async fn release_invocation_hook_trust_bypass(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    conversation_id: ConversationId,
) -> Result<bool, Box<dyn std::error::Error>> {
    let response: serde_json::Value = decode_api_response(
        client
            .delete(format!(
                "{host}/api/v1/conversations/{conversation_id}/codex-hook-trust-bypass"
            ))
            .bearer_auth(token)
            .send()
            .await?,
        "release invocation-only hook trust bypass",
    )
    .await?;
    Ok(response
        .get("released")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false))
}

async fn finish_invocation_remote_binding<T>(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    conversation_id: ConversationId,
    was_bound: bool,
    result: Result<T, Box<dyn std::error::Error>>,
) -> Result<T, Box<dyn std::error::Error>> {
    let release = if was_bound {
        Some(release_invocation_remote(client, host, token, conversation_id).await)
    } else {
        None
    };
    match result {
        Ok(value) => {
            if let Some(release) = release {
                let _ = release?;
            }
            Ok(value)
        }
        Err(error) => {
            // Preserve the operation failure while still making a best-effort attempt to remove
            // the invocation-local credential and transport binding.
            let _ = release;
            Err(error)
        }
    }
}

async fn finish_invocation_hook_trust_binding<T>(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    conversation_id: ConversationId,
    was_bound: bool,
    result: Result<T, Box<dyn std::error::Error>>,
) -> Result<T, Box<dyn std::error::Error>> {
    let release = if was_bound {
        Some(release_invocation_hook_trust_bypass(client, host, token, conversation_id).await)
    } else {
        None
    };
    match result {
        Ok(value) => {
            if let Some(release) = release {
                let _ = release?;
            }
            Ok(value)
        }
        Err(error) => {
            let _ = release;
            Err(error)
        }
    }
}

async fn bind_invocation_bindings(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    conversation_id: ConversationId,
    remote: Option<&InvocationRemote>,
    bypass_hook_trust: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(remote) = remote {
        bind_invocation_remote(client, host, token, conversation_id, remote).await?;
    }
    if bypass_hook_trust
        && let Err(error) =
            bind_invocation_hook_trust_bypass(client, host, token, conversation_id).await
    {
        if remote.is_some() {
            let _ = release_invocation_remote(client, host, token, conversation_id).await;
        }
        return Err(error);
    }
    Ok(())
}

async fn finish_invocation_bindings<T>(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    conversation_id: ConversationId,
    remote_bound: bool,
    hook_trust_bound: bool,
    result: Result<T, Box<dyn std::error::Error>>,
) -> Result<T, Box<dyn std::error::Error>> {
    let remote_release = if remote_bound {
        Some(release_invocation_remote(client, host, token, conversation_id).await)
    } else {
        None
    };
    let hook_release = if hook_trust_bound {
        Some(release_invocation_hook_trust_bypass(client, host, token, conversation_id).await)
    } else {
        None
    };
    match result {
        Ok(value) => {
            if let Some(release) = remote_release {
                let _ = release?;
            }
            if let Some(release) = hook_release {
                let _ = release?;
            }
            Ok(value)
        }
        Err(error) => {
            let _ = remote_release;
            let _ = hook_release;
            Err(error)
        }
    }
}

async fn transfer_invocation_bindings(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    from: ConversationId,
    to: ConversationId,
    remote: Option<&InvocationRemote>,
    bypass_hook_trust: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if from == to {
        return Ok(());
    }
    bind_invocation_bindings(client, host, token, to, remote, bypass_hook_trust).await?;
    let remote_release = if remote.is_some() {
        Some(release_invocation_remote(client, host, token, from).await)
    } else {
        None
    };
    let hook_release = if bypass_hook_trust {
        Some(release_invocation_hook_trust_bypass(client, host, token, from).await)
    } else {
        None
    };
    if let Some(release) = remote_release {
        let _ = release?;
    }
    if let Some(release) = hook_release {
        let _ = release?;
    }
    Ok(())
}

#[derive(Debug, Args)]
struct ReviewArgs {
    #[arg(long, conflicts_with_all = ["base", "commit", "prompt"])]
    uncommitted: bool,
    #[arg(long, conflicts_with_all = ["uncommitted", "commit", "prompt"])]
    base: Option<String>,
    #[arg(long, conflicts_with_all = ["uncommitted", "base", "prompt"])]
    commit: Option<String>,
    #[arg(long, requires = "commit")]
    title: Option<String>,
    #[arg(value_name = "PROMPT", num_args = 0.., conflicts_with_all = ["uncommitted", "base", "commit"])]
    prompt: Vec<String>,
}

#[derive(Debug, Args)]
struct LoginArgs {
    #[command(subcommand)]
    command: Option<LoginCommand>,
    #[arg(long, conflicts_with_all = ["with_api_key", "with_access_token"])]
    device_auth: bool,
    /// Read an OpenAI API key from standard input.
    #[arg(long, conflicts_with_all = ["device_auth", "with_access_token"])]
    with_api_key: bool,
    /// Read a ChatGPT access token from standard input.
    #[arg(long, conflicts_with_all = ["device_auth", "with_api_key"])]
    with_access_token: bool,
}

#[derive(Debug, Subcommand)]
enum ExecCommand {
    /// Resume a previous non-interactive conversation.
    Resume {
        #[arg(long)]
        last: bool,
        #[arg(long)]
        all: bool,
        session: Option<String>,
        #[arg(long)]
        output_schema: Option<PathBuf>,
        #[arg(value_name = "PROMPT", num_args = 0..)]
        prompt: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
enum LoginCommand {
    /// Print the active authentication state.
    Status,
}

#[derive(Debug, Subcommand)]
enum SettingsCommand {
    /// Show current owner settings.
    Show {
        #[arg(long)]
        json: bool,
    },
    /// Change one or more live settings with optimistic revision protection.
    Set {
        /// Select Simple, Advanced, or Developer presentation density.
        #[arg(long, value_enum)]
        interface_complexity: Option<InterfaceComplexityArg>,
        /// Legacy compatibility alias; prefer --interface-complexity.
        #[arg(long)]
        advanced_mode: Option<bool>,
        #[arg(long, value_enum)]
        autonomy: Option<AutonomyArg>,
        #[arg(long)]
        shadow_experiments: Option<bool>,
        #[arg(long, default_value = "updated from CLI")]
        reason: String,
    },
    /// Show immutable settings revisions.
    History {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum RuntimeCommand {
    /// Show concise Protected Root connectivity and authoritative storage health.
    Status {
        /// Emit the versioned status contract as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Show Root identity, protocol, session, schema, queue, and migration evidence.
    Inspect {
        /// Emit the versioned status contract as JSON.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum WorkflowCommand {
    /// List the latest authoritative attempt for every workflow.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Show the exact contract, current attempt, and ordered event history.
    Show {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Show the exact workflow together with bounded Work/Verify invocation activity.
    Activity {
        id: String,
        #[arg(long, default_value_t = 100)]
        invocation_limit: usize,
        #[arg(long)]
        json: bool,
    },
    /// Durably request cancellation using current revision/generation fencing.
    Cancel {
        id: String,
        /// Stable key to reuse when retrying an uncertain cancellation request.
        #[arg(long)]
        idempotency_key: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum ActionCommand {
    /// List a bounded newest-first projection of canonical actions.
    List {
        #[arg(long, default_value_t = 100)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    /// Show immutable intent, executions, receipts, reconciliation, and ordered events.
    Show {
        id: String,
        #[arg(long, default_value_t = 500)]
        event_limit: usize,
        #[arg(long)]
        json: bool,
    },
    /// Initiate the exact compensation already bound to a committed reversible action.
    Compensate {
        id: String,
        /// Owner-visible reason retained in the immutable authorization evidence.
        #[arg(long)]
        reason: String,
        /// Explicitly confirm that the exact bound compensation should be executed.
        #[arg(long)]
        confirm: bool,
        /// Stable key to reuse when retrying an uncertain compensation request.
        #[arg(long)]
        idempotency_key: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum InvocationCommand {
    /// List a bounded newest-first projection of material model invocations.
    List {
        #[arg(long, default_value_t = 100)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    /// Show exact content-minimized reconstruction evidence and model-run history.
    Show {
        id: String,
        #[arg(long, default_value_t = 100)]
        model_run_limit: usize,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum BackupCommand {
    /// Create an online-consistent backup through the running daemon.
    Create {
        #[arg(long)]
        backup_id: Option<uuid::Uuid>,
        /// Include provider and MCP credentials under passphrase encryption.
        #[arg(long)]
        include_secrets: bool,
        /// Read passphrase and confirmation from redirected standard input.
        #[arg(long, requires = "include_secrets")]
        passphrase_stdin: bool,
        #[arg(long)]
        json: bool,
    },
    /// List and verify backups owned by the running daemon.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Verify any transferred .mcgb bundle without contacting the daemon.
    Verify {
        bundle: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Restore a verified bundle while the daemon is stopped.
    Restore {
        bundle: PathBuf,
        #[arg(long)]
        confirm: bool,
        /// Authenticate and restore the bundle's encrypted provider and MCP credentials.
        #[arg(long)]
        restore_secrets: bool,
        /// Read the encrypted-backup passphrase from redirected standard input.
        #[arg(long, requires = "restore_secrets")]
        passphrase_stdin: bool,
    },
}

#[derive(Debug, Subcommand)]
enum PluginCommand {
    /// List installed plugin heads, manifests, grants, trust, and enabled state.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Import one local WASI module and manifest. The new revision remains disabled.
    InstallLocal {
        manifest: PathBuf,
        module: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Install a registry package only after its detached signature verifies against owner trust roots.
    InstallRegistry {
        request: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Replace the complete scoped grant set from a JSON array; this disables the plugin.
    SetGrants {
        plugin_id: String,
        expected_revision: u64,
        grants: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Explicitly enable or disable one plugin head.
    SetEnabled {
        plugin_id: String,
        expected_revision: u64,
        #[arg(action = clap::ArgAction::Set)]
        enabled: bool,
        #[arg(long)]
        json: bool,
    },
    /// Invoke an enabled plugin with a caller-owned UUID and JSON input file.
    Invoke {
        plugin_id: String,
        input: PathBuf,
        #[arg(long)]
        invocation_id: Option<PluginInvocationId>,
        #[arg(long)]
        task_id: Option<TaskId>,
        #[arg(long)]
        json: bool,
    },
    /// Read one invocation's current projection and immutable lifecycle history.
    Invocation {
        plugin_id: String,
        invocation_id: PluginInvocationId,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum AutonomyArg {
    Guided,
    Balanced,
    Autonomous,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum InterfaceComplexityArg {
    Simple,
    Advanced,
    Developer,
}

impl From<InterfaceComplexityArg> for InterfaceComplexity {
    fn from(value: InterfaceComplexityArg) -> Self {
        match value {
            InterfaceComplexityArg::Simple => Self::Simple,
            InterfaceComplexityArg::Advanced => Self::Advanced,
            InterfaceComplexityArg::Developer => Self::Developer,
        }
    }
}

impl From<AutonomyArg> for AutonomyPreset {
    fn from(value: AutonomyArg) -> Self {
        match value {
            AutonomyArg::Guided => Self::Guided,
            AutonomyArg::Balanced => Self::Balanced,
            AutonomyArg::Autonomous => Self::Autonomous,
        }
    }
}

#[derive(Debug, Subcommand)]
enum ExperimentCommand {
    /// List experiments for an organization.
    List { organization_id: String },
    /// Show an experiment and all shadow, benchmark, and decision evidence.
    Show { id: String },
    /// Record an inactive candidate organization revision from JSON.
    Candidate { request: PathBuf },
    /// Record an immutable supervision observation from JSON.
    Observe { request: PathBuf },
    /// List all immutable supervision observations.
    Observations,
    /// Create an experiment from JSON.
    Create { request: PathBuf },
    /// Apply a compare-and-set experiment state transition.
    Transition {
        id: String,
        #[arg(value_enum)]
        expected: ExperimentStateArg,
        #[arg(value_enum)]
        next: ExperimentStateArg,
    },
    /// Record an evaluation-only shadow receipt from JSON.
    Shadow { id: String, request: PathBuf },
    /// Record paired benchmark evidence from JSON.
    Benchmark { id: String, request: PathBuf },
    /// Record a measured promotion, rejection, or reversion decision from JSON.
    Decide { id: String, request: PathBuf },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ExperimentStateArg {
    Draft,
    Ready,
    Running,
    AwaitingDecision,
    Promoted,
    Rejected,
    Reverted,
}

impl From<ExperimentStateArg> for ExperimentState {
    fn from(value: ExperimentStateArg) -> Self {
        match value {
            ExperimentStateArg::Draft => Self::Draft,
            ExperimentStateArg::Ready => Self::Ready,
            ExperimentStateArg::Running => Self::Running,
            ExperimentStateArg::AwaitingDecision => Self::AwaitingDecision,
            ExperimentStateArg::Promoted => Self::Promoted,
            ExperimentStateArg::Rejected => Self::Rejected,
            ExperimentStateArg::Reverted => Self::Reverted,
        }
    }
}

#[derive(Debug, Subcommand)]
enum SshCommand {
    /// List all durable SSH hosts.
    List,
    /// Show one host, immutable revisions, and operation history.
    Show { id: String },
    /// Register a host from a JSON request file.
    Register { request: PathBuf },
    /// Update a host from a complete revision-checked JSON request file.
    Update { id: String, request: PathBuf },
    /// Run a typed operation from a JSON request file.
    Run { id: String, request: PathBuf },
    /// List all durable multi-host workflows.
    WorkflowList,
    /// Show one durable multi-host workflow.
    WorkflowShow { id: String },
    /// Create an immutable workflow graph from a JSON request file.
    WorkflowCreate { request: PathBuf },
    /// Run or resume all dependency-ready workflow steps.
    WorkflowRun { id: String },
    /// Cancel one non-terminal workflow without starting more remote work.
    WorkflowCancel { id: String },
}

#[derive(Debug, Subcommand)]
enum SiteCommand {
    /// List all durable Sites.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Show one Site and all immutable revision, health, and deployment history.
    Show { id: String },
    /// Register one project-bound Site with an optional structured deployment command.
    Register {
        display_name: String,
        project_id: String,
        public_url: String,
        health_url: String,
        hosting_provider: String,
        #[arg(long)]
        site_id: Option<String>,
        #[arg(long, value_enum, default_value_t = SiteEnvironmentArg::Production)]
        environment: SiteEnvironmentArg,
        #[arg(long)]
        deploy_executable: Option<String>,
        #[arg(long = "deploy-arg")]
        deploy_arguments: Vec<String>,
        #[arg(long, default_value = "")]
        deploy_cwd: String,
        #[arg(long)]
        credential_reference: Option<String>,
        #[arg(long)]
        monitoring_paused: bool,
        #[arg(long)]
        json: bool,
    },
    /// Run or replay one bounded health check.
    Health {
        id: String,
        #[arg(long)]
        check_id: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Enable or exactly replay durable interval monitoring for a Site.
    Monitor {
        id: String,
        #[arg(long, default_value_t = 300)]
        interval_seconds: u64,
        #[arg(long, default_value_t = 200)]
        expected_status: u16,
        #[arg(long)]
        schedule_id: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Request or replay one approval-governed deployment.
    Deploy {
        id: String,
        #[arg(long)]
        deployment_id: Option<String>,
        #[arg(long)]
        task_id: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Apply a complete revision-checked Site definition from JSON.
    Update {
        id: String,
        request: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Pause or resume monitoring, or archive the Site while preserving history.
    SetState {
        id: String,
        #[arg(value_enum)]
        state: SiteStateArg,
        reason: String,
        #[arg(long)]
        expected_revision: u32,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SiteEnvironmentArg {
    Development,
    Preview,
    Production,
}

impl From<SiteEnvironmentArg> for SiteEnvironment {
    fn from(value: SiteEnvironmentArg) -> Self {
        match value {
            SiteEnvironmentArg::Development => Self::Development,
            SiteEnvironmentArg::Preview => Self::Preview,
            SiteEnvironmentArg::Production => Self::Production,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SiteStateArg {
    Active,
    MonitoringPaused,
    Archived,
}

impl From<SiteStateArg> for SiteState {
    fn from(value: SiteStateArg) -> Self {
        match value {
            SiteStateArg::Active => Self::Active,
            SiteStateArg::MonitoringPaused => Self::MonitoringPaused,
            SiteStateArg::Archived => Self::Archived,
        }
    }
}

#[derive(Debug, Subcommand)]
enum PlatformCommand {
    /// Show the latest privacy-safe host capability snapshot.
    Capabilities {
        #[arg(long)]
        json: bool,
    },
    /// List durable platform sessions.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Reserve a browser session. This does not claim an executor is attached.
    RequestBrowser {
        display_name: String,
        #[arg(long)]
        session_id: Option<String>,
        #[arg(long)]
        project_id: Option<String>,
        #[arg(long)]
        task_id: Option<String>,
        #[arg(long = "origin")]
        origins: Vec<String>,
        #[arg(long, default_value_t = 3_600)]
        lifetime_seconds: u32,
        #[arg(long)]
        json: bool,
    },
    /// Reserve a computer-use session. This does not claim an executor is attached.
    RequestComputer {
        display_name: String,
        #[arg(long)]
        session_id: Option<String>,
        #[arg(long)]
        project_id: Option<String>,
        #[arg(long)]
        task_id: Option<String>,
        /// Grant only screen capture, without pointer or keyboard control.
        #[arg(long)]
        screen_only: bool,
        /// Bind a specific platform display identity instead of the primary display.
        #[arg(long)]
        display_id: Option<u32>,
        /// Bind a display-relative region formatted as x,y,width,height.
        #[arg(long)]
        region: Option<String>,
        #[arg(long, default_value_t = 3_600)]
        lifetime_seconds: u32,
        #[arg(long)]
        json: bool,
    },
    /// Start the reserved browser in an isolated profile.
    Attach {
        id: String,
        /// Show a local window so control can later be handed to its owner.
        #[arg(long)]
        visible: bool,
        #[arg(long)]
        json: bool,
    },
    /// Navigate an active browser inside its immutable origin grant.
    Navigate {
        id: String,
        url: String,
        #[arg(long)]
        json: bool,
    },
    /// Hand a visible browser window to the local owner.
    Handoff {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Blank owner-visible content and return browser control to MCG.
    Reclaim {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Bind the reserved computer session to its immutable display region.
    AttachComputer {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Capture the active computer region. PNG bytes are written only when requested.
    CaptureComputer {
        id: String,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Apply a typed JSON action request after revalidating its frame digest.
    ActComputer {
        id: String,
        request: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Stop MCG observation/input while the local owner uses the computer.
    HandoffComputer {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Return computer authority to MCG; a new capture is required before input.
    ReclaimComputer {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Show immutable lifecycle history for one session.
    History { id: String },
    /// Close a session while retaining its history.
    Close { id: String, reason: String },
}

#[derive(Debug, Subcommand)]
enum ScheduleCommand {
    /// Run once at an RFC 3339 timestamp.
    CreateOnce {
        name: String,
        project_id: String,
        goal: String,
        at: String,
        /// Stable UUID to reuse when retrying after an uncertain response.
        #[arg(long)]
        schedule_id: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Run repeatedly, starting immediately.
    CreateInterval {
        name: String,
        project_id: String,
        goal: String,
        seconds: u64,
        /// Stable UUID to reuse when retrying after an uncertain response.
        #[arg(long)]
        schedule_id: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Run on a five-field UTC cron expression.
    CreateCron {
        name: String,
        project_id: String,
        goal: String,
        expression: String,
        /// Stable UUID to reuse when retrying after an uncertain response.
        #[arg(long)]
        schedule_id: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Run when a project-relative file's content changes.
    CreateWatch {
        name: String,
        project_id: String,
        goal: String,
        relative_path: String,
        #[arg(long, default_value_t = 30)]
        poll_seconds: u64,
        /// Stable UUID to reuse when retrying after an uncertain response.
        #[arg(long)]
        schedule_id: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Run when a new durable event kind is observed.
    CreateEvent {
        name: String,
        project_id: String,
        goal: String,
        event_kind: String,
        /// Stable UUID to reuse when retrying after an uncertain response.
        #[arg(long)]
        schedule_id: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Run an adaptive system heartbeat.
    CreateHeartbeat {
        name: String,
        project_id: String,
        goal: String,
        #[arg(long, default_value_t = 300)]
        idle_seconds: u64,
        #[arg(long, default_value_t = 30)]
        busy_seconds: u64,
        /// Stable UUID to reuse when retrying after an uncertain response.
        #[arg(long)]
        schedule_id: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// List durable schedules.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Show occurrence history.
    History { id: String },
    /// Allow a paused schedule to claim work again.
    Resume { id: String },
    /// Stop a schedule without deleting its history.
    Pause { id: String },
    /// Permanently stop a schedule while retaining its history.
    Remove { id: String },
    /// Request immediate attention without overriding an owner pause.
    Wake {
        id: String,
        reason: String,
        #[arg(long, value_enum, default_value_t = NotificationSeverityArg::Warning)]
        severity: NotificationSeverityArg,
    },
    /// List scheduler notifications.
    Notifications {
        #[arg(long)]
        all: bool,
        #[arg(long)]
        json: bool,
    },
    /// Mark one scheduler notification as read.
    Acknowledge { id: String },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum NotificationSeverityArg {
    Info,
    Warning,
    High,
}

impl From<NotificationSeverityArg> for NotificationSeverity {
    fn from(value: NotificationSeverityArg) -> Self {
        match value {
            NotificationSeverityArg::Info => Self::Info,
            NotificationSeverityArg::Warning => Self::Warning,
            NotificationSeverityArg::High => Self::High,
        }
    }
}

#[derive(Debug, Subcommand)]
enum McpCommand {
    /// Add a Streamable HTTP server and immediately discover its tools.
    AddHttp {
        id: String,
        name: String,
        url: String,
        #[arg(long, value_enum, default_value_t = McpAuthArg::None)]
        auth: McpAuthArg,
        #[arg(long, value_enum, default_value_t = McpProtocolArg::Auto)]
        protocol: McpProtocolArg,
        #[arg(long)]
        disabled: bool,
        #[arg(long)]
        json: bool,
    },
    /// Add a directly executed stdio server and immediately discover its tools.
    AddStdio {
        id: String,
        name: String,
        executable: String,
        #[arg(long, value_enum, default_value_t = McpProtocolArg::Auto)]
        protocol: McpProtocolArg,
        #[arg(long)]
        disabled: bool,
        #[arg(long)]
        json: bool,
        /// Arguments passed directly to the executable (no shell expansion).
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// List configured MCP servers and their latest tool catalogs.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Show one server's latest durable tool catalog.
    Tools { id: String },
    /// Refresh one server's live discovery snapshot.
    Discover { id: String },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum McpAuthArg {
    None,
    BearerToken,
}

impl From<McpAuthArg> for McpAuthMode {
    fn from(value: McpAuthArg) -> Self {
        match value {
            McpAuthArg::None => Self::None,
            McpAuthArg::BearerToken => Self::BearerToken,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum McpProtocolArg {
    Auto,
    Modern20260728,
    Legacy,
}

impl From<McpProtocolArg> for McpProtocolMode {
    fn from(value: McpProtocolArg) -> Self {
        match value {
            McpProtocolArg::Auto => Self::Auto,
            McpProtocolArg::Modern20260728 => Self::Modern20260728,
            McpProtocolArg::Legacy => Self::Legacy,
        }
    }
}

#[derive(Debug, Subcommand)]
enum ProviderCommand {
    /// Sign in to an `OpenAI` subscription without using API billing.
    Login {
        id: String,
        name: String,
        #[arg(long, value_enum, default_value_t = OAuthMethodArg::Browser)]
        method: OAuthMethodArg,
        /// Print the verification link without opening the system browser.
        #[arg(long)]
        no_open: bool,
    },
    /// Add a provider and immediately discover its models.
    Add {
        id: String,
        name: String,
        base_url: String,
        #[arg(long, value_enum, default_value_t = ProviderKindArg::Compatible)]
        kind: ProviderKindArg,
        #[arg(long, value_enum, default_value_t = ProviderAuthArg::None)]
        auth: ProviderAuthArg,
        #[arg(long)]
        json: bool,
    },
    /// List configured providers and their latest catalogs.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Show the latest durable model catalog.
    Models { id: String },
    /// Refresh live model discovery.
    Discover { id: String },
    /// Rotate the tokens for an `OpenAI` subscription connection.
    Refresh { id: String },
    /// Disconnect a provider and remove its protected credentials.
    Disconnect { id: String },
    /// Probe one model's live Responses, streaming, and tool-call capabilities.
    Probe { id: String, model: String },
    /// Show durable token and known-cost totals for local proxy calls.
    Usage {
        #[arg(long)]
        json: bool,
    },
    /// List privacy-preserving local proxy conversation records.
    Conversations {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OAuthMethodArg {
    Browser,
    DeviceCode,
}

impl From<OAuthMethodArg> for OpenAiOAuthMethod {
    fn from(value: OAuthMethodArg) -> Self {
        match value {
            OAuthMethodArg::Browser => Self::Browser,
            OAuthMethodArg::DeviceCode => Self::DeviceCode,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ProviderKindArg {
    Openai,
    Openrouter,
    Compatible,
}

impl From<ProviderKindArg> for ProviderKind {
    fn from(value: ProviderKindArg) -> Self {
        match value {
            ProviderKindArg::Openai => Self::OpenAi,
            ProviderKindArg::Openrouter => Self::OpenRouter,
            ProviderKindArg::Compatible => Self::OpenAiCompatible,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ProviderAuthArg {
    None,
    ApiKey,
    BearerToken,
}

impl From<ProviderAuthArg> for ProviderAuthMode {
    fn from(value: ProviderAuthArg) -> Self {
        match value {
            ProviderAuthArg::None => Self::None,
            ProviderAuthArg::ApiKey => Self::ApiKey,
            ProviderAuthArg::BearerToken => Self::BearerToken,
        }
    }
}

#[derive(Debug, Subcommand)]
enum OrganizationCommand {
    /// Create the safe simple-by-default core organization.
    Default {
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// List current organizations.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Inspect one current organization.
    Get { id: String },
    /// List every immutable revision of one organization.
    History {
        organization_id: String,
        #[arg(long)]
        json: bool,
    },
    /// Create a new materialized organization pinned to the current revision of `BASE_ID`.
    Inherit {
        base_id: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        json: bool,
    },
    /// Export one immutable revision as a portable content-addressed JSON genome.
    Export {
        revision_id: String,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Import a portable JSON genome as a fresh local organization.
    Import {
        file: PathBuf,
        #[arg(long)]
        import_id: Option<String>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Compare two immutable revisions without treating local identities as changes.
    Compare {
        left_revision_id: String,
        right_revision_id: String,
        #[arg(long)]
        json: bool,
    },
    /// List owner-selected signed-organization publishers and revocation state.
    Publishers {
        #[arg(long)]
        json: bool,
    },
    /// Trust or revise one publisher under exact canonical HTTPS registry scopes.
    TrustPublisher {
        publisher_id: String,
        #[arg(long)]
        display_name: String,
        #[arg(long)]
        public_key_base64: String,
        #[arg(long = "registry", required = true)]
        registries: Vec<String>,
        #[arg(long)]
        expected_revision: Option<u64>,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        json: bool,
    },
    /// Permanently revoke one publisher identity.
    RevokePublisher {
        publisher_id: String,
        expected_revision: u64,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        json: bool,
    },
    /// List durable signed-organization reviews and decisions.
    ShareReviews {
        #[arg(long)]
        json: bool,
    },
    /// Verify a signed share package into pending review without importing it.
    ReviewShare {
        file: PathBuf,
        #[arg(long)]
        review_id: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Explicitly import one pending share after current publisher revalidation.
    ImportShare {
        review_id: String,
        #[arg(long)]
        import_id: Option<String>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        json: bool,
    },
    /// Reject one pending signed-organization review.
    RejectShare {
        review_id: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum RunCommand {
    /// Plan one atomic work item with a minimal organization.
    PlanAtomic {
        task_id: String,
        organization_revision_id: String,
        goal: String,
        #[arg(long, value_enum, default_value_t = RoleArg::Coder)]
        role: RoleArg,
        #[arg(long, default_value = "Goal completed with objective evidence")]
        check: String,
        #[arg(long)]
        independent_evaluation: bool,
        #[arg(long)]
        json: bool,
    },
    /// Plan compound work from a `PlanAgentRunRequest` JSON file.
    PlanFile { task_id: String, file: PathBuf },
    /// Inspect a run, plan, instances, leases, usage, evaluations, and results.
    Get { id: String },
    /// Deterministically verify a run from durable evidence without provider or tool side effects.
    Replay {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Start or resume a planned/waiting/recovering run.
    Start { id: String },
    /// Execute one ready work item through configured providers and governed tools.
    Execute {
        id: String,
        work_item_id: String,
        project_id: String,
        #[arg(long)]
        target_path: Option<String>,
        #[arg(long, default_value_t = 16_384)]
        context_token_budget: u32,
        #[arg(long, value_delimiter = ',')]
        tools: Vec<String>,
        #[arg(long, default_value_t = 8_192)]
        estimated_output_tokens: u64,
        #[arg(long, default_value_t = 8)]
        max_model_calls: u16,
    },
    /// Record an immutable `EvaluationReport` JSON file for a run.
    Evaluate { id: String, file: PathBuf },
    /// Cancel a run.
    Cancel { id: String },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum RoleArg {
    Conversation,
    Coder,
    Evaluator,
    ToolWorker,
    Researcher,
    Tester,
}

impl From<RoleArg> for RoleKind {
    fn from(value: RoleArg) -> Self {
        match value {
            RoleArg::Conversation => Self::Conversation,
            RoleArg::Coder => Self::Coder,
            RoleArg::Evaluator => Self::Evaluator,
            RoleArg::ToolWorker => Self::ToolWorker,
            RoleArg::Researcher => Self::Researcher,
            RoleArg::Tester => Self::Tester,
        }
    }
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    /// Discover and register a project root.
    Add {
        root: PathBuf,
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        initialize_git: bool,
        #[arg(long)]
        json: bool,
    },
    /// List registered projects.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Read the current project model.
    Get {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Safely initialize a registered plain folder as a local Git repository.
    InitGit {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Archive or unarchive a project without deleting its source.
    Archive {
        id: String,
        #[arg(long)]
        undo: bool,
        #[arg(long)]
        json: bool,
    },
    /// Remove a project from the MCG catalog without deleting its source.
    Remove {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Re-scan a registered project and report invalidated model areas.
    Refresh {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Read immutable project discovery history.
    History { id: String },
    /// Derive stable stages from explicit project-topology dependencies.
    Plan { id: String },
    /// Review, inspect, run, or cancel snapshot-pinned topology execution plans.
    Reviewed {
        project_id: String,
        #[command(subcommand)]
        command: ReviewedTopologyCommand,
    },
    /// Assemble bounded project context from current facts and memory.
    Context {
        id: String,
        #[arg(long, default_value_t = 8_000)]
        token_budget: u32,
        #[arg(long)]
        target: Option<String>,
    },
    /// Manage project-scoped structured memory.
    Memory {
        project_id: String,
        #[command(subcommand)]
        command: MemoryCommand,
    },
}

#[derive(Debug, Subcommand)]
enum ReviewedTopologyCommand {
    /// Create a reviewed plan from a JSON request file.
    Create { file: PathBuf },
    /// List retained reviewed plans for the project.
    List,
    /// Read one retained reviewed plan.
    Get { plan_id: String },
    /// Run dependency-ready commands through normal policy and approval controls.
    Run { plan_id: String },
    /// Cancel a plan while no command may still be executing.
    Cancel { plan_id: String },
}

#[derive(Debug, Subcommand)]
enum MemoryCommand {
    /// Add a provenance-bearing claim; VALUE must be JSON.
    Add {
        namespace: String,
        key: String,
        value: String,
        #[arg(long, default_value_t = 1.0)]
        confidence: f64,
        #[arg(long, default_value = "local owner via CLI")]
        source: String,
        /// Optional RFC 3339 deadline after which automatic consolidation starts decay.
        #[arg(long, value_name = "RFC3339")]
        fresh_until: Option<DateTime<Utc>>,
    },
    /// List current claim heads.
    List {
        #[arg(long)]
        namespace: Option<String>,
        #[arg(long)]
        include_stale: bool,
    },
    /// Correct a claim with a superseding JSON value.
    Correct {
        claim_id: String,
        value: String,
        #[arg(long, default_value_t = 1.0)]
        confidence: f64,
        #[arg(long, default_value = "local owner correction via CLI")]
        source: String,
        /// Optional RFC 3339 replacement freshness deadline.
        #[arg(long, value_name = "RFC3339")]
        fresh_until: Option<DateTime<Utc>>,
    },
    /// Reinforce, decay, or retract a claim with a successor version.
    Transition {
        claim_id: String,
        action: MemoryActionArg,
        #[arg(long, default_value = "local owner lifecycle action via CLI")]
        source: String,
    },
    /// Read every version of one claim key.
    History { namespace: String, key: String },
    /// Review the exact transferable source claim and canonical digest.
    TransferPreview { claim_id: String },
    /// Copy one reviewed durable claim into another project.
    Transfer {
        claim_id: String,
        target_project_id: String,
        /// Canonical digest returned by `transfer-preview`.
        #[arg(long)]
        source_sha256: String,
        /// Printable owner review explaining why this claim belongs in the target.
        #[arg(long)]
        reason: String,
        /// Stable UUID for safe retry after a lost response.
        #[arg(long)]
        transfer_id: Option<String>,
    },
    /// List immutable inbound and outbound transfer evidence.
    Transfers,
    /// Render current working/durable memory as Markdown.
    Managed,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum MemoryActionArg {
    Reinforce,
    Decay,
    Retract,
}

impl From<MemoryActionArg> for MemoryLifecycleAction {
    fn from(value: MemoryActionArg) -> Self {
        match value {
            MemoryActionArg::Reinforce => Self::Reinforce,
            MemoryActionArg::Decay => Self::Decay,
            MemoryActionArg::Retract => Self::Retract,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ConversationApprovalArg {
    Ask,
    AutoReview,
    FullAccess,
}

impl From<ConversationApprovalArg> for ApprovalMode {
    fn from(value: ConversationApprovalArg) -> Self {
        match value {
            ConversationApprovalArg::Ask => Self::Ask,
            ConversationApprovalArg::AutoReview => Self::AutoReview,
            ConversationApprovalArg::FullAccess => Self::FullAccess,
        }
    }
}

#[derive(Debug, Subcommand)]
enum ConversationCommand {
    Create {
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        project_id: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        effort: Option<String>,
        #[arg(long)]
        speed: Option<String>,
        #[arg(long, value_enum, default_value_t = ConversationApprovalArg::Ask)]
        approval: ConversationApprovalArg,
        #[arg(long)]
        json: bool,
    },
    List {
        #[arg(long)]
        archived: bool,
        #[arg(long)]
        json: bool,
    },
    Show {
        id: String,
        #[arg(long)]
        json: bool,
    },
    Send {
        id: String,
        #[arg(value_name = "MESSAGE", num_args = 1..)]
        message: Vec<String>,
        /// Attach a typed reference (`file:path`, `folder:path`, or `plugin:id`). Repeatable.
        #[arg(long = "mention", value_name = "KIND:RESOURCE")]
        mentions: Vec<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        effort: Option<String>,
        #[arg(long)]
        speed: Option<String>,
        #[arg(long, value_enum)]
        approval: Option<ConversationApprovalArg>,
        /// Wait for delegated work and the final Chat callback.
        #[arg(long)]
        wait: bool,
        #[arg(long, default_value_t = 1_800)]
        timeout_seconds: u64,
        #[arg(long, default_value_t = 2)]
        poll_seconds: u64,
        #[arg(long)]
        json: bool,
    },
    Team {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Emit a compact machine-readable outcome and efficiency receipt for the latest batch.
    Receipt {
        id: String,
    },
    Search {
        id: String,
        query: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    Review {
        id: String,
        delegation_id: String,
    },
    Cancel {
        id: String,
        delegation_id: String,
        expected_revision: u64,
    },
    Retry {
        id: String,
        delegation_id: String,
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Apply {
        id: String,
        delegation_id: String,
        expected_revision: u64,
    },
    Discard {
        id: String,
        delegation_id: String,
        expected_revision: u64,
    },
    PreviewStart {
        id: String,
        preview_id: String,
    },
    PreviewStop {
        id: String,
        preview_id: String,
    },
    Interrupt {
        id: String,
        turn_id: String,
    },
    Steer {
        id: String,
        turn_id: String,
        #[arg(value_name = "MESSAGE", num_args = 1..)]
        message: Vec<String>,
        #[arg(long = "mention", value_name = "KIND:RESOURCE")]
        mentions: Vec<String>,
    },
    Archive {
        id: String,
        #[arg(long)]
        undo: bool,
    },
}

#[derive(Debug, Subcommand)]
enum CodexCommand {
    Status,
    Models,
    Account,
    RateLimits,
    Usage,
    Configuration,
    Pending,
    Threads,
    Discover {
        #[arg(long = "cwd")]
        cwds: Vec<String>,
        #[arg(long)]
        force: bool,
    },
    Login,
    LoginCancel {
        login_id: String,
    },
    Logout,
}

#[derive(Debug, Subcommand)]
enum GithubCommand {
    Status,
    Login,
    LoginComplete {
        session_id: String,
    },
    Disconnect,
    Repositories,
    Create {
        name: String,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        public: bool,
    },
    Link {
        project_id: String,
        clone_url: String,
    },
}

#[derive(Debug, Subcommand)]
enum GovernorCommand {
    Policy,
    List {
        organization_id: String,
    },
    Show {
        proposal_id: String,
    },
    Create {
        file: PathBuf,
    },
    Approve {
        proposal_id: String,
        #[arg(long)]
        reject: bool,
    },
    RequestImplementation {
        proposal_id: String,
        conversation_id: String,
    },
}

#[derive(Debug, Subcommand)]
enum TaskCommand {
    /// Create a durable task.
    Create {
        goal: String,
        #[arg(long)]
        project_id: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Idempotently plan and start autonomous governed work for a task.
    Launch {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Explicitly retry incomplete work for a blocked consumer task.
    Retry {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Continue an approval-waiting turn after approving its tool request.
    Resume {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// List current durable task projections.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Read a task projection.
    Get {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Read a task's ordered event history.
    Events { id: String },
    /// Read immutable tool audit evidence for a task.
    Audit { id: String },
    /// Apply a validated task state transition.
    Transition { id: String, state: TaskStateArg },
}

#[derive(Debug, Subcommand)]
enum ApprovalCommand {
    /// List pending approvals by default.
    List {
        #[arg(long)]
        all: bool,
        #[arg(long)]
        json: bool,
    },
    /// Read one approval.
    Get {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Approve a pending action.
    Approve {
        id: String,
        #[arg(long)]
        reason: Option<String>,
    },
    /// Deny a pending action.
    Deny {
        id: String,
        #[arg(long)]
        reason: Option<String>,
    },
    /// Cancel a pending action.
    Cancel {
        id: String,
        #[arg(long)]
        reason: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum TaskStateArg {
    Pending,
    Active,
    WaitingForApproval,
    Blocked,
    Completed,
    Failed,
    Cancelled,
}

impl From<TaskStateArg> for TaskState {
    fn from(value: TaskStateArg) -> Self {
        match value {
            TaskStateArg::Pending => Self::Pending,
            TaskStateArg::Active => Self::Active,
            TaskStateArg::WaitingForApproval => Self::WaitingForApproval,
            TaskStateArg::Blocked => Self::Blocked,
            TaskStateArg::Completed => Self::Completed,
            TaskStateArg::Failed => Self::Failed,
            TaskStateArg::Cancelled => Self::Cancelled,
        }
    }
}

fn main() {
    // The compatibility and MetaCog command trees intentionally coexist in one
    // binary. Keep their broad generated parser/dispatcher off Windows' small
    // process-main stack so additive typed preferences cannot destabilize startup.
    let status = std::thread::Builder::new()
        .name("mcg-cli-dispatch".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("initialize MetaCog CLI runtime");
            match runtime.block_on(Box::pin(run_cli())) {
                Ok(()) => 0,
                Err(error) => {
                    // Keep every failure machine-readable, including argument-independent API,
                    // authentication, and transport errors. This makes the CLI safe to compose
                    // in harnesses without scraping Rust's `Error: ...` debug wrapper.
                    eprintln!(
                        "{}",
                        serde_json::json!({
                            "error": "cli_error",
                            "message": error.to_string(),
                        })
                    );
                    1
                }
            }
        })
        .expect("start MetaCog CLI dispatcher")
        .join()
        .unwrap_or(1);
    if status != 0 {
        std::process::exit(status);
    }
}

#[allow(clippy::too_many_lines)] // Keeps offline recovery, authentication, and all CLI modes visible together.
async fn run_cli() -> Result<(), Box<dyn std::error::Error>> {
    if root_help_requested() {
        combined_cli_command().print_long_help()?;
        println!();
        return Ok(());
    }
    if codex_compat_command_requested() {
        return Box::pin(run_codex_compat_cli()).await;
    }
    let (native_arguments, native_overrides) = extract_native_overrides(std::env::args_os())?;
    let add_dirs = native_overrides.add_dirs;
    let feature_overrides = native_overrides.feature_overrides;
    let config_overrides = native_overrides.config_overrides;
    let codex_profile = native_overrides.codex_profile;
    let strict_config = native_overrides.strict_config;
    let no_alt_screen = native_overrides.no_alt_screen;
    let cli = Cli::parse_from(native_arguments);
    if cli.dangerously_bypass_hook_trust
        && !matches!(
            &cli.command,
            None | Some(Command::Prompt(_)) | Some(Command::Run { command: None, .. })
        )
    {
        return Err(
            "`--dangerously-bypass-hook-trust` is supported only for interactive or consumer conversation work"
                .into(),
        );
    }
    let root_is_interactive = cli.command.is_none() && std::io::stdin().is_terminal();
    if !root_is_interactive {
        reject_invocation_remote_for_subcommand(
            &cli.remote,
            cli.command.as_ref().map_or("prompt", |_| "subcommand"),
        )?;
    }
    let invocation_remote = if root_is_interactive {
        resolve_invocation_remote(cli.remote, InteractiveRemoteArgs::default())?
    } else {
        None
    };
    let config = AppConfig::default();
    let host = cli.host.trim_end_matches('/').to_owned();
    let token_path = cli
        .token_file
        .clone()
        .unwrap_or_else(|| config.auth_token_path());
    if let Some(Command::Start {
        timeout_seconds,
        json,
    }) = &cli.command
    {
        run_start(&host, &token_path, *timeout_seconds, *json).await?;
        return Ok(());
    }
    if let Some(Command::Runtime { command }) = &cli.command {
        run_root_runtime(&config.data_directory, command).await?;
        return Ok(());
    }
    if let Some(Command::Workflow { command }) = &cli.command {
        run_root_workflow(&config.data_directory, command).await?;
        return Ok(());
    }
    if let Some(Command::Action { command }) = &cli.command {
        run_root_action(&config.data_directory, command).await?;
        return Ok(());
    }
    if let Some(Command::Invocation { command }) = &cli.command {
        run_root_invocation(&config.data_directory, command).await?;
        return Ok(());
    }
    if let Some(Command::Backup { command }) = &cli.command {
        match command {
            BackupCommand::Verify { bundle, json } => {
                print_backup_bundle(&verify_backup_bundle(bundle)?, *json)?;
                return Ok(());
            }
            BackupCommand::Restore {
                bundle,
                confirm,
                restore_secrets,
                passphrase_stdin,
            } => {
                if !confirm {
                    return Err("backup restore requires --confirm and a stopped daemon".into());
                }
                let summary = if *restore_secrets {
                    let passphrase = BackupPassphrase::new(read_backup_passphrase(
                        "Encrypted backup passphrase: ",
                        *passphrase_stdin,
                    )?)?;
                    restore_encrypted_backup_bundle(
                        bundle,
                        &config.data_directory,
                        Utc::now(),
                        &passphrase,
                        &OsProviderCredentialStore,
                        &OsMcpCredentialStore,
                    )?
                } else {
                    restore_backup_bundle(bundle, &config.data_directory, Utc::now())?
                };
                println!(
                    "Restored {}{}. A pre-restore database backup was retained. Start mcg-daemon to migrate and recover projections.",
                    summary.path.display(),
                    if *restore_secrets {
                        " with encrypted credentials"
                    } else {
                        ""
                    }
                );
                return Ok(());
            }
            BackupCommand::Create { .. } | BackupCommand::List { .. } => {}
        }
    }
    let token = if let Ok(token) = std::env::var("MCG_AUTH_TOKEN") {
        token
    } else {
        std::fs::read_to_string(&token_path).map_err(|error| {
            format!(
                "cannot read local daemon token at {}: {error}; start mcg-daemon first",
                token_path.display()
            )
        })?
    };
    let client = reqwest::Client::new();
    let project = cli.project.unwrap_or(std::env::current_dir()?);
    let uses_local_provider = matches!(
        &cli.command,
        None | Some(Command::Prompt(_)) | Some(Command::Run { command: None, .. })
    );
    let provider_is_interactive = cli.command.is_none() && std::io::stdin().is_terminal();
    let resolved_local_provider = if uses_local_provider {
        resolve_oss_provider(
            &client,
            &host,
            token.trim(),
            cli.oss,
            cli.local_provider,
            provider_is_interactive,
            no_alt_screen,
        )
        .await?
    } else {
        None
    };
    let model_provider = resolved_local_provider
        .as_ref()
        .map(|provider| provider.id.clone());
    let model = cli.model.or_else(|| {
        resolved_local_provider
            .as_ref()
            .map(|provider| provider.default_model.clone())
    });
    let sandbox = cli.sandbox;
    let approval_policy = cli.approval_policy;
    let approve_for_me = cli.approve_for_me;
    let dangerously_bypass_approvals_and_sandbox = cli.dangerously_bypass_approvals_and_sandbox;
    let dangerously_bypass_hook_trust = cli.dangerously_bypass_hook_trust;
    let images = cli.images;
    let host = host.as_str();

    match cli.command {
        Some(Command::Status { json }) => run_status(&client, host, token.trim(), json).await?,
        Some(Command::Stop { json }) => run_stop(&client, host, token.trim(), json).await?,
        Some(Command::Start { .. }) => unreachable!("start handled before token loading"),
        Some(Command::Restart {
            timeout_seconds,
            json,
        }) => {
            request_stop(&client, host, token.trim()).await?;
            wait_for_daemon_stop(&client, host, token.trim(), timeout_seconds).await?;
            run_start(host, &token_path, timeout_seconds, json).await?;
        }
        Some(Command::Doctor { json, strict }) => {
            run_doctor(&client, host, token.trim(), json, strict).await?;
        }
        Some(Command::Runtime { .. }) => {
            unreachable!("runtime inspection handled before bearer-token loading")
        }
        Some(Command::Workflow { .. }) => {
            unreachable!("workflow commands handled before bearer-token loading")
        }
        Some(Command::Action { .. }) => {
            unreachable!("action commands handled before bearer-token loading")
        }
        Some(Command::Invocation { .. }) => {
            unreachable!("invocation commands handled before bearer-token loading")
        }
        Some(Command::Settings { command }) => {
            run_settings(&client, host, token.trim(), command).await?;
        }
        Some(Command::Backup { command }) => {
            run_backup(&client, host, token.trim(), command).await?;
        }
        Some(Command::Task { command }) => run_task(&client, host, token.trim(), command).await?,
        Some(Command::Conversation { command }) => {
            run_conversation(&client, host, token.trim(), command).await?;
        }
        Some(Command::Codex { command }) => {
            run_codex(&client, host, token.trim(), command).await?;
        }
        Some(Command::Github { command }) => {
            run_github(&client, host, token.trim(), command).await?;
        }
        Some(Command::Governor { command }) => {
            run_governor(&client, host, token.trim(), command).await?;
        }
        Some(Command::Approval { command }) => {
            run_approval(&client, host, token.trim(), command).await?;
        }
        Some(Command::Project { command }) => {
            run_project(&client, host, token.trim(), command).await?;
        }
        Some(Command::Organization { command }) => {
            run_organization(&client, host, token.trim(), command).await?;
        }
        Some(Command::Run {
            command: Some(command),
            json,
            stream_json,
            model: run_model,
            effort,
            speed,
            approval: _,
            mentions,
            goal,
        }) => {
            if json
                || stream_json
                || run_model.is_some()
                || model.is_some()
                || effort.is_some()
                || speed.is_some()
                || !mentions.is_empty()
                || !images.is_empty()
                || !add_dirs.is_empty()
                || !feature_overrides.is_empty()
                || !config_overrides.is_empty()
                || sandbox.is_some()
                || approval_policy.is_some()
                || approve_for_me
                || dangerously_bypass_approvals_and_sandbox
                || dangerously_bypass_hook_trust
                || !goal.is_empty()
            {
                return Err(
                    "administrative run subcommands cannot be combined with a consumer goal or output mode"
                        .into(),
                );
            }
            run_agent(&client, host, token.trim(), command).await?;
        }
        Some(Command::Run {
            command: None,
            json,
            stream_json,
            model: run_model,
            effort,
            speed,
            approval,
            mut mentions,
            goal,
        }) => {
            let goal = validated_goal(&goal.join(" "))?;
            let supplemental = read_optional_piped_input()?;
            mentions.extend(codex_exec_mentions(&images)?);
            let additional_writable_roots = codex_additional_writable_roots(&add_dirs)?;
            let preferences = codex_execution_preferences(
                run_model.or(model.clone()),
                model_provider.clone(),
                sandbox,
                approval_policy,
                approve_for_me,
                dangerously_bypass_approvals_and_sandbox,
                &additional_writable_roots,
                &feature_overrides,
                &config_overrides,
                codex_profile.clone(),
                strict_config,
                conversation_preferences(None, effort, speed, approval.into()),
            );
            run_consumer_submission(
                &client,
                host,
                token.trim(),
                &project,
                &goal,
                supplemental,
                consumer_output(json, stream_json),
                preferences,
                &mentions,
                None,
                false,
                dangerously_bypass_hook_trust,
            )
            .await?;
        }
        Some(Command::Prompt(words)) => {
            let goal = os_words_to_goal(words)?;
            let supplemental = read_optional_piped_input()?;
            let additional_writable_roots = codex_additional_writable_roots(&add_dirs)?;
            let preferences = codex_execution_preferences(
                model.clone(),
                model_provider.clone(),
                sandbox,
                approval_policy,
                approve_for_me,
                dangerously_bypass_approvals_and_sandbox,
                &additional_writable_roots,
                &feature_overrides,
                &config_overrides,
                codex_profile.clone(),
                strict_config,
                conversation_preferences(None, None, None, ApprovalMode::Ask),
            );
            let mentions = codex_exec_mentions(&images)?;
            run_consumer_submission(
                &client,
                host,
                token.trim(),
                &project,
                &goal,
                supplemental,
                ConsumerOutput::Human,
                preferences,
                &mentions,
                None,
                false,
                dangerously_bypass_hook_trust,
            )
            .await?;
        }
        None if std::io::stdin().is_terminal() => {
            let additional_writable_roots = codex_additional_writable_roots(&add_dirs)?;
            let preferences = codex_execution_preferences(
                model.clone(),
                model_provider.clone(),
                sandbox,
                approval_policy,
                approve_for_me,
                dangerously_bypass_approvals_and_sandbox,
                &additional_writable_roots,
                &feature_overrides,
                &config_overrides,
                codex_profile.clone(),
                strict_config,
                conversation_preferences(None, None, None, ApprovalMode::Ask),
            );
            let mentions = codex_exec_mentions(&images)?;
            run_interactive(
                &client,
                host,
                token.trim(),
                &project,
                None,
                Some(preferences),
                mentions,
                no_alt_screen,
                None,
                invocation_remote.as_ref(),
                dangerously_bypass_hook_trust,
            )
            .await?;
        }
        None => {
            let goal = validated_goal(&read_bounded_stdin()?)?;
            let additional_writable_roots = codex_additional_writable_roots(&add_dirs)?;
            let preferences = codex_execution_preferences(
                model,
                model_provider,
                sandbox,
                approval_policy,
                approve_for_me,
                dangerously_bypass_approvals_and_sandbox,
                &additional_writable_roots,
                &feature_overrides,
                &config_overrides,
                codex_profile,
                strict_config,
                conversation_preferences(None, None, None, ApprovalMode::Ask),
            );
            let mentions = codex_exec_mentions(&images)?;
            run_consumer_submission(
                &client,
                host,
                token.trim(),
                &project,
                &goal,
                None,
                ConsumerOutput::Human,
                preferences,
                &mentions,
                None,
                false,
                dangerously_bypass_hook_trust,
            )
            .await?;
        }
        Some(Command::Provider { command }) => {
            run_provider(&client, host, token.trim(), command).await?;
        }
        Some(Command::Mcp { command }) => {
            run_mcp(&client, host, token.trim(), command).await?;
        }
        Some(Command::Plugin { command }) => {
            run_plugin(&client, host, token.trim(), command).await?;
        }
        Some(Command::Schedule { command }) => {
            run_schedule(&client, host, token.trim(), command).await?;
        }
        Some(Command::Platform { command }) => {
            run_platform(&client, host, token.trim(), command).await?;
        }
        Some(Command::Site { command }) => {
            run_site(&client, host, token.trim(), command).await?;
        }
        Some(Command::Ssh { command }) => {
            run_ssh(&client, host, token.trim(), command).await?;
        }
        Some(Command::Experiment { command }) => {
            run_experiment(&client, host, token.trim(), command).await?;
        }
    }

    Ok(())
}

fn codex_compat_command_requested() -> bool {
    codex_compat_command_requested_from(std::env::args_os().skip(1))
}

fn codex_compat_command_requested_from(arguments: impl IntoIterator<Item = OsString>) -> bool {
    const COMMANDS: &[&str] = &[
        "exec",
        "e",
        "apply",
        "a",
        "update",
        "agents",
        "queue",
        "migrate-rollouts",
        "app",
        "resume",
        "fork",
        "archive",
        "unarchive",
        "delete",
        "review",
        "completion",
        "login",
        "logout",
        "features",
        "sandbox",
    ];
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        let Some(argument) = argument.to_str() else {
            continue;
        };
        if matches!(
            argument,
            "--host"
                | "--token-file"
                | "--project"
                | "-C"
                | "--model"
                | "-m"
                | "--local-provider"
                | "--sandbox"
                | "-s"
                | "--ask-for-approval"
                | "-a"
                | "--image"
                | "-i"
                | "--add-dir"
                | "--config"
                | "-c"
                | "--profile"
                | "-p"
                | "--enable"
                | "--disable"
                | "--remote"
                | "--remote-auth-token-env"
        ) {
            let _ = arguments.next();
            continue;
        }
        if argument.starts_with("--host=")
            || argument.starts_with("--token-file=")
            || argument.starts_with("--project=")
            || argument.starts_with("--model=")
            || argument.starts_with("--sandbox=")
            || argument.starts_with("--ask-for-approval=")
            || argument.starts_with("--image=")
            || argument.starts_with("--add-dir=")
            || argument.starts_with("--config=")
            || argument.starts_with("--profile=")
            || (argument.starts_with("-c") && argument.len() > 2)
            || (argument.starts_with("-p") && argument.len() > 2)
            || argument.starts_with("--enable=")
            || argument.starts_with("--disable=")
            || argument.starts_with("--remote=")
            || argument.starts_with("--remote-auth-token-env=")
            || (argument.starts_with("-C") && argument.len() > 2)
            || (argument.starts_with("-m") && argument.len() > 2)
            || (argument.starts_with("-s") && argument.len() > 2)
            || (argument.starts_with("-a") && argument.len() > 2)
            || (argument.starts_with("-i") && argument.len() > 2)
        {
            continue;
        }
        if argument.starts_with('-') {
            continue;
        }
        return COMMANDS.contains(&argument);
    }
    false
}

fn root_help_requested() -> bool {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    matches!(arguments.as_slice(), [argument] if argument == "--help" || argument == "-h")
}

#[derive(Debug, Default, PartialEq, Eq)]
struct NativeOverrides {
    add_dirs: Vec<PathBuf>,
    feature_overrides: BTreeMap<String, bool>,
    config_overrides: Vec<CodexConfigOverride>,
    raw_config_overrides: Vec<String>,
    codex_profile: Option<String>,
    strict_config: bool,
    no_alt_screen: bool,
}

fn codex_config_key_path(key: &str) -> Result<Vec<String>, String> {
    let mut table = toml::from_str::<toml::Table>(&format!("{key} = 0"))
        .map_err(|error| format!("invalid Codex configuration key `{key}`: {error}"))?;
    let mut path = Vec::new();
    loop {
        if table.len() != 1 {
            return Err(format!(
                "Codex configuration key `{key}` must identify one dotted path"
            ));
        }
        let (segment, value) = table.into_iter().next().expect("single TOML path segment");
        if segment.trim().is_empty() {
            return Err("Codex configuration key segments cannot be empty".into());
        }
        path.push(segment);
        match value {
            toml::Value::Table(next) => table = next,
            toml::Value::Integer(0) => break,
            _ => return Err(format!("unsupported Codex configuration key `{key}`")),
        }
    }
    Ok(path)
}

fn record_codex_config_override(
    expression: &str,
    overrides: &mut NativeOverrides,
) -> Result<(), String> {
    let (key, raw_value) = expression
        .split_once('=')
        .ok_or_else(|| "Codex configuration overrides use KEY=VALUE".to_owned())?;
    let key = key.trim();
    if key.is_empty() {
        return Err("Codex configuration key cannot be empty".into());
    }
    let path = codex_config_key_path(key)?;
    overrides.raw_config_overrides.push(expression.to_owned());
    let parsed = toml::from_str::<toml::Table>(&format!("value = {raw_value}"))
        .ok()
        .and_then(|mut table| table.remove("value"))
        .map_or_else(
            || serde_json::Value::String(raw_value.to_owned()),
            |value| serde_json::to_value(value).expect("TOML values serialize to JSON"),
        );
    if path.len() == 2 && path[0] == "features" {
        let enabled = parsed
            .as_bool()
            .ok_or_else(|| format!("Codex feature override `{key}` requires a boolean value"))?;
        overrides.feature_overrides.insert(path[1].clone(), enabled);
        return Ok(());
    }
    if path.as_slice() == ["features"] {
        let features = parsed.as_object().ok_or_else(|| {
            "Codex `features` configuration requires an inline table of boolean values".to_owned()
        })?;
        for (feature, value) in features {
            let enabled = value.as_bool().ok_or_else(|| {
                format!("Codex feature override `features.{feature}` requires a boolean value")
            })?;
            overrides.feature_overrides.insert(feature.clone(), enabled);
        }
        return Ok(());
    }
    overrides.config_overrides.push(CodexConfigOverride {
        key: key.to_owned(),
        value: parsed,
    });
    Ok(())
}

fn extract_native_overrides(
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<(Vec<OsString>, NativeOverrides), String> {
    let mut arguments = arguments.into_iter();
    let program = arguments
        .next()
        .ok_or_else(|| "CLI argument vector is missing the program name".to_owned())?;
    let mut cleaned = vec![program];
    let mut overrides = NativeOverrides::default();
    let mut literal_arguments = false;
    while let Some(argument) = arguments.next() {
        if literal_arguments {
            cleaned.push(argument);
            continue;
        }
        if argument == "--" {
            literal_arguments = true;
            cleaned.push(argument);
            continue;
        }
        if argument == "--add-dir" {
            let value = arguments
                .next()
                .ok_or_else(|| "--add-dir requires a directory".to_owned())?;
            overrides.add_dirs.push(PathBuf::from(value));
            continue;
        }
        if let Some(value) = argument
            .to_str()
            .and_then(|value| value.strip_prefix("--add-dir="))
        {
            if value.is_empty() {
                return Err("--add-dir requires a directory".into());
            }
            overrides.add_dirs.push(PathBuf::from(value));
            continue;
        }
        if argument == "--search" {
            record_codex_config_override("web_search=\"live\"", &mut overrides)?;
            continue;
        }
        if argument == "--strict-config" {
            overrides.strict_config = true;
            continue;
        }
        let profile = if argument == "--profile" || argument == "-p" {
            Some(
                arguments
                    .next()
                    .ok_or_else(|| "--profile requires a profile name".to_owned())?
                    .into_string()
                    .map_err(|_| "Codex profile names must be valid Unicode".to_owned())?,
            )
        } else {
            argument.to_str().and_then(|value| {
                value
                    .strip_prefix("--profile=")
                    .or_else(|| value.strip_prefix("-p"))
                    .map(str::to_owned)
            })
        };
        if let Some(profile) = profile {
            if profile.is_empty() {
                return Err("--profile requires a profile name".into());
            }
            overrides.codex_profile = Some(profile);
            continue;
        }
        if argument == "--no-alt-screen" {
            overrides.no_alt_screen = true;
            continue;
        }
        let config_expression = if argument == "--config" || argument == "-c" {
            Some(
                arguments
                    .next()
                    .ok_or_else(|| "--config requires KEY=VALUE".to_owned())?
                    .into_string()
                    .map_err(|_| {
                        "Codex configuration overrides must be valid Unicode".to_owned()
                    })?,
            )
        } else {
            argument.to_str().and_then(|value| {
                value
                    .strip_prefix("--config=")
                    .or_else(|| value.strip_prefix("-c"))
                    .map(str::to_owned)
            })
        };
        if let Some(expression) = config_expression {
            if expression.is_empty() {
                return Err("--config requires KEY=VALUE".into());
            }
            record_codex_config_override(&expression, &mut overrides)?;
            continue;
        }
        let feature_state = if argument == "--enable" {
            Some(true)
        } else if argument == "--disable" {
            Some(false)
        } else {
            None
        };
        if let Some(enabled) = feature_state {
            let value = arguments.next().ok_or_else(|| {
                format!(
                    "{} requires a feature name",
                    if enabled { "--enable" } else { "--disable" }
                )
            })?;
            let value = value
                .into_string()
                .map_err(|_| "Codex feature names must be valid Unicode".to_owned())?;
            if value.is_empty() {
                return Err("Codex feature name cannot be empty".into());
            }
            overrides.feature_overrides.insert(value, enabled);
            continue;
        }
        if let Some((enabled, value)) = argument.to_str().and_then(|value| {
            value
                .strip_prefix("--enable=")
                .map(|value| (true, value))
                .or_else(|| value.strip_prefix("--disable=").map(|value| (false, value)))
        }) {
            if value.is_empty() {
                return Err("Codex feature name cannot be empty".into());
            }
            overrides
                .feature_overrides
                .insert(value.to_owned(), enabled);
            continue;
        }
        cleaned.push(argument);
    }
    Ok((cleaned, overrides))
}

fn combined_cli_command() -> clap::Command {
    let mut command = Cli::command()
        .arg(
            clap::Arg::new("add-dir")
                .long("add-dir")
                .value_name("DIR")
                .help("Additional directories writable alongside the primary project. Repeatable.")
                .action(clap::ArgAction::Append),
        )
        .arg(
            clap::Arg::new("search")
                .long("search")
                .help("Enable live web search for this session.")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            clap::Arg::new("profile")
                .short('p')
                .long("profile")
                .value_name("NAME")
                .help("Layer $CODEX_HOME/<name>.config.toml over the base Codex configuration."),
        )
        .arg(
            clap::Arg::new("strict-config")
                .long("strict-config")
                .help("Fail before dispatch when Codex rejects the effective user or project configuration.")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            clap::Arg::new("no-alt-screen")
                .long("no-alt-screen")
                .help("Run the terminal UI inline and preserve terminal scrollback history.")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            clap::Arg::new("config")
                .short('c')
                .long("config")
                .value_name("KEY=VALUE")
                .help("Override a Codex configuration value for this session. Repeatable; later values win.")
                .action(clap::ArgAction::Append),
        )
        .arg(
            clap::Arg::new("enable")
                .long("enable")
                .value_name("FEATURE")
                .help("Enable a Codex feature for this session only. Repeatable; later flags win.")
                .action(clap::ArgAction::Append),
        )
        .arg(
            clap::Arg::new("disable")
                .long("disable")
                .value_name("FEATURE")
                .help("Disable a Codex feature for this session only. Repeatable; later flags win.")
                .action(clap::ArgAction::Append),
        );
    let compatibility = CodexCompatCli::command();
    for subcommand in compatibility.get_subcommands() {
        command = command.subcommand(subcommand.clone());
    }
    command
}

fn run_codex_app(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let path = std::fs::canonicalize(path)
        .map_err(|error| format!("workspace path {} is unavailable: {error}", path.display()))?;
    if !path.is_dir() {
        return Err(format!("workspace path is not a directory: {}", path.display()).into());
    }

    #[cfg(target_os = "macos")]
    {
        let status = ProcessCommand::new("open")
            .args(["-a", "MetaCog"])
            .arg(&path)
            .status()?;
        if !status.success() {
            return Err(format!("MetaCog Desktop could not be opened: {status}").into());
        }
        println!("Opened MetaCog Desktop at {}.", path.display());
    }
    #[cfg(windows)]
    {
        println!(
            "Open this workspace in MetaCog Desktop:\n{}",
            path.display()
        );
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        return Err(format!(
            "MetaCog Desktop handoff is available on Windows and macOS; workspace: {}",
            path.display()
        )
        .into());
    }
    Ok(())
}

fn run_codex_rollout_migration(
    args: &MigrateRolloutsArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let executable = std::env::var_os("MCG_CODEX_APP_SERVER_EXECUTABLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("codex"));
    let mut command = ProcessCommand::new(executable);
    command.arg("migrate-rollouts");
    if args.apply {
        command.arg("--apply");
    }
    for thread in &args.threads {
        command.args(["--thread", thread]);
    }
    if let Some(limit) = args.max_mib_per_second {
        command.args(["--max-mib-per-second", &limit.to_string()]);
    }
    if args.json {
        command.arg("--json");
    }
    if args.verbose {
        command.arg("--verbose");
    }
    let status = command.status()?;
    if !status.success() {
        return Err(format!("Codex rollout migration failed with {status}").into());
    }
    Ok(())
}

fn run_codex_sandbox(
    args: &SandboxArgs,
    project: Option<&std::path::Path>,
    overrides: &NativeOverrides,
) -> Result<(), Box<dyn std::error::Error>> {
    let executable = std::env::var_os("MCG_CODEX_APP_SERVER_EXECUTABLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("codex"));
    let mut command = ProcessCommand::new(executable);
    command.args(codex_sandbox_arguments(args, project, overrides));
    let status = command.status()?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

fn codex_sandbox_arguments(
    args: &SandboxArgs,
    project: Option<&std::path::Path>,
    overrides: &NativeOverrides,
) -> Vec<OsString> {
    let mut command = vec![OsString::from("sandbox")];
    for expression in &overrides.raw_config_overrides {
        command.push(OsString::from("--config"));
        command.push(OsString::from(expression));
    }
    for (feature, enabled) in &overrides.feature_overrides {
        command.push(OsString::from(if *enabled {
            "--enable"
        } else {
            "--disable"
        }));
        command.push(OsString::from(feature));
    }
    if let Some(state) = &args.sandbox_state_json {
        command.push(OsString::from("--sandbox-state-json"));
        command.push(OsString::from(state));
    }
    for root in &args.sandbox_state_readable_roots {
        command.push(OsString::from("--sandbox-state-readable-root"));
        command.push(root.as_os_str().to_owned());
    }
    if args.sandbox_state_disable_network {
        command.push(OsString::from("--sandbox-state-disable-network"));
    }
    if let Some(profile) = &args.permission_profile {
        command.push(OsString::from("--permission-profile"));
        command.push(OsString::from(profile));
    }
    if let Some(profile) = args.profile.as_ref().or(overrides.codex_profile.as_ref()) {
        command.push(OsString::from("--profile"));
        command.push(OsString::from(profile));
    }
    if let Some(project) = project {
        command.push(OsString::from("--cd"));
        command.push(project.as_os_str().to_owned());
    }
    if args.include_managed_config {
        command.push(OsString::from("--include-managed-config"));
    }
    command.push(OsString::from("--"));
    command.extend(args.command.iter().cloned());
    command
}

async fn run_codex_compat_cli() -> Result<(), Box<dyn std::error::Error>> {
    let (arguments, native_overrides) = extract_native_overrides(std::env::args_os())?;
    let cli = CodexCompatCli::parse_from(arguments);
    debug_assert!(
        cli.config.is_empty(),
        "native config flags are extracted before parsing"
    );
    if cli.dangerously_bypass_hook_trust
        && !matches!(
            &cli.command,
            CodexCompatCommand::Exec(_)
                | CodexCompatCommand::Resume(_)
                | CodexCompatCommand::Fork(_)
                | CodexCompatCommand::Review(_)
        )
    {
        return Err(
            "`--dangerously-bypass-hook-trust` is supported only for exec, resume, fork, and review invocations"
                .into(),
        );
    }
    let remote_supported = matches!(
        &cli.command,
        CodexCompatCommand::Agents(_)
            | CodexCompatCommand::Resume(_)
            | CodexCompatCommand::Queue(_)
            | CodexCompatCommand::Fork(_)
            | CodexCompatCommand::Archive(_)
            | CodexCompatCommand::Unarchive(_)
            | CodexCompatCommand::Delete(_)
    );
    if !remote_supported {
        reject_invocation_remote_for_subcommand(&cli.remote, "subcommand")?;
    }
    if let CodexCompatCommand::Completion { shell } = &cli.command {
        return generate_cli_completion(*shell);
    }
    if let CodexCompatCommand::App(args) = &cli.command {
        return run_codex_app(&args.path);
    }
    if let CodexCompatCommand::MigrateRollouts(args) = &cli.command {
        return run_codex_rollout_migration(args);
    }
    if let CodexCompatCommand::Sandbox(args) = &cli.command {
        return run_codex_sandbox(args, cli.project.as_deref(), &native_overrides);
    }
    if matches!(cli.command, CodexCompatCommand::Update) {
        if cli.model.is_some()
            || cli.oss
            || cli.local_provider.is_some()
            || cli.project.is_some()
            || cli.sandbox.is_some()
            || cli.approval_policy.is_some()
            || cli.approve_for_me
            || cli.dangerously_bypass_approvals_and_sandbox
            || !cli.images.is_empty()
            || !cli.add_dirs.is_empty()
            || native_overrides.codex_profile.is_some()
            || native_overrides.strict_config
            || native_overrides.no_alt_screen
            || !native_overrides.raw_config_overrides.is_empty()
            || !native_overrides.feature_overrides.is_empty()
            || cli.remote.remote.is_some()
            || cli.remote.remote_auth_token_env.is_some()
        {
            return Err("`mcg update` does not accept runtime or project overrides".into());
        }
        let config = AppConfig::default();
        let token_path = cli
            .token_file
            .clone()
            .unwrap_or_else(|| config.auth_token_path());
        return run_codex_compatible_update(
            &reqwest::Client::new(),
            cli.host.trim_end_matches('/'),
            &config,
            &token_path,
        )
        .await;
    }
    let uses_local_provider = matches!(
        &cli.command,
        CodexCompatCommand::Exec(_)
            | CodexCompatCommand::Resume(_)
            | CodexCompatCommand::Queue(_)
            | CodexCompatCommand::Fork(_)
            | CodexCompatCommand::Review(_)
    );
    let provider_is_interactive = matches!(
        cli.command,
        CodexCompatCommand::Resume(_) | CodexCompatCommand::Fork(_)
    );
    let sandbox = cli.sandbox;
    let approval_policy = cli.approval_policy;
    let approve_for_me = cli.approve_for_me;
    let dangerously_bypass_approvals_and_sandbox = cli.dangerously_bypass_approvals_and_sandbox;
    let dangerously_bypass_hook_trust = cli.dangerously_bypass_hook_trust;
    let images = cli.images;
    let mut add_dirs = cli.add_dirs;
    add_dirs.extend(native_overrides.add_dirs);
    let additional_writable_roots = codex_additional_writable_roots(&add_dirs)?;
    let mut feature_overrides = native_overrides.feature_overrides;
    let config_overrides = native_overrides.config_overrides;
    let raw_config_overrides = native_overrides.raw_config_overrides;
    let codex_profile = native_overrides.codex_profile;
    let strict_config = native_overrides.strict_config;
    let no_alt_screen = native_overrides.no_alt_screen;
    let root_remote = cli.remote;
    for feature in cli.enable_features {
        feature_overrides.insert(feature, true);
    }
    for feature in cli.disable_features {
        feature_overrides.insert(feature, false);
    }
    let config = AppConfig::default();
    let host = cli.host.trim_end_matches('/').to_owned();
    let token_path = cli.token_file.unwrap_or_else(|| config.auth_token_path());
    let token = if let Ok(token) = std::env::var("MCG_AUTH_TOKEN") {
        token
    } else {
        std::fs::read_to_string(&token_path).map_err(|error| {
            format!(
                "cannot read local daemon token at {}: {error}; start mcg-daemon first",
                token_path.display()
            )
        })?
    };
    let project_was_explicit = cli.project.is_some();
    let project = cli.project.unwrap_or(std::env::current_dir()?);
    let client = reqwest::Client::new();
    let host = host.as_str();
    let token = token.trim();
    let resolved_local_provider = if uses_local_provider {
        resolve_oss_provider(
            &client,
            host,
            token,
            cli.oss,
            cli.local_provider,
            provider_is_interactive,
            no_alt_screen,
        )
        .await?
    } else {
        None
    };
    let model_provider = resolved_local_provider
        .as_ref()
        .map(|provider| provider.id.clone());
    let model = cli.model.or_else(|| {
        resolved_local_provider
            .as_ref()
            .map(|provider| provider.default_model.clone())
    });
    match cli.command {
        CodexCompatCommand::App(_) => unreachable!("app handled before daemon authentication"),
        CodexCompatCommand::MigrateRollouts(_) => {
            unreachable!("rollout migration handled before daemon authentication")
        }
        CodexCompatCommand::Sandbox(_) => {
            unreachable!("sandbox handled before daemon authentication")
        }
        CodexCompatCommand::Agents(args) => {
            let invocation_remote = resolve_invocation_remote(root_remote, args.remote)?;
            run_codex_agents(
                &client,
                host,
                token,
                &project,
                no_alt_screen,
                invocation_remote.as_ref(),
            )
            .await?;
        }
        CodexCompatCommand::Apply(args) => {
            if model.is_some()
                || sandbox.is_some()
                || approval_policy.is_some()
                || approve_for_me
                || dangerously_bypass_approvals_and_sandbox
                || !images.is_empty()
                || !additional_writable_roots.is_empty()
                || codex_profile.is_some()
                || strict_config
                || no_alt_screen
            {
                return Err(
                    "`mcg apply` accepts Codex's documented -c/--config and feature overrides only"
                        .into(),
                );
            }
            run_codex_compatible_apply(
                &client,
                host,
                token,
                &project,
                &args.task_id,
                raw_config_overrides,
                feature_overrides,
            )
            .await?;
        }
        CodexCompatCommand::Update => unreachable!("update handled before daemon authentication"),
        CodexCompatCommand::Exec(args) => {
            let ExecArgs {
                command,
                json,
                ephemeral,
                skip_git_repo_check,
                output_last_message,
                output_schema,
                prompt,
            } = *args;
            Box::pin(run_codex_compatible_exec(
                &client,
                host,
                token,
                &project,
                command,
                json,
                ephemeral,
                skip_git_repo_check,
                model,
                model_provider,
                sandbox,
                approval_policy,
                approve_for_me,
                dangerously_bypass_approvals_and_sandbox,
                dangerously_bypass_hook_trust,
                &additional_writable_roots,
                &feature_overrides,
                &config_overrides,
                codex_profile.clone(),
                strict_config,
                images,
                output_last_message,
                output_schema,
                prompt,
            ))
            .await?;
        }
        CodexCompatCommand::Resume(args) => {
            let ResumeArgs {
                all,
                include_non_interactive,
                last,
                session,
                remote,
            } = *args;
            let invocation_remote = resolve_invocation_remote(root_remote, remote)?;
            let mut conversation = resolve_cli_session(
                &client,
                host,
                token,
                &project,
                session.as_deref(),
                last,
                all,
                Some(false),
                include_non_interactive,
                no_alt_screen,
            )
            .await?;
            if should_resume_in_current_directory(
                &client,
                host,
                token,
                &conversation,
                &project,
                project_was_explicit,
                &config_overrides,
                codex_profile.as_deref(),
                strict_config,
                no_alt_screen,
            )
            .await?
            {
                conversation = rebind_conversation_to_current_project(
                    &client,
                    host,
                    token,
                    &project,
                    conversation,
                )
                .await?;
            }
            let preferences = codex_execution_preferences_override(
                model.clone(),
                model_provider.clone(),
                sandbox,
                approval_policy,
                approve_for_me,
                dangerously_bypass_approvals_and_sandbox,
                &additional_writable_roots,
                &feature_overrides,
                &config_overrides,
                codex_profile.clone(),
                strict_config,
                &conversation.execution_preferences,
            );
            let mentions = codex_exec_mentions(&images)?;
            Box::pin(run_interactive(
                &client,
                host,
                token,
                &project,
                Some(conversation),
                preferences,
                mentions,
                no_alt_screen,
                None,
                invocation_remote.as_ref(),
                dangerously_bypass_hook_trust,
            ))
            .await?;
        }
        CodexCompatCommand::Queue(args) => {
            let QueueArgs {
                thread,
                message,
                remote,
            } = *args;
            let invocation_remote = resolve_invocation_remote(root_remote, remote)?;
            let conversation = resolve_cli_session(
                &client,
                host,
                token,
                &project,
                Some(&thread),
                false,
                true,
                Some(false),
                true,
                false,
            )
            .await?;
            if let Some(remote) = invocation_remote.as_ref() {
                bind_invocation_remote(&client, host, token, conversation.id, remote).await?;
            }
            let result: Result<(), Box<dyn std::error::Error>> = async {
                let preferences = codex_execution_preferences_override(
                    model,
                    model_provider.clone(),
                    sandbox,
                    approval_policy,
                    approve_for_me,
                    dangerously_bypass_approvals_and_sandbox,
                    &additional_writable_roots,
                    &feature_overrides,
                    &config_overrides,
                    codex_profile.clone(),
                    strict_config,
                    &conversation.execution_preferences,
                );
                let pending_mentions = codex_exec_mentions(&images)?;
                let (content, mentions) =
                    resolve_cli_mentions(&client, host, token, message, &pending_mentions).await?;
                let _: ConversationView = decode_api_response(
                    client
                        .post(format!(
                            "{host}/api/v1/conversations/{}/queue",
                            conversation.id
                        ))
                        .bearer_auth(token)
                        .json(&SendConversationMessageRequest {
                            content,
                            execution_preferences: preferences,
                            mentions,
                        })
                        .send()
                        .await?,
                    "queue conversation message",
                )
                .await?;
                Ok(())
            }
            .await;
            finish_invocation_remote_binding(
                &client,
                host,
                token,
                conversation.id,
                invocation_remote.is_some(),
                result,
            )
            .await?;
            println!(
                "Queued message for {} ({}).",
                conversation.title, conversation.id
            );
        }
        CodexCompatCommand::Fork(args) => {
            let ForkArgs {
                all,
                last,
                session,
                remote,
            } = *args;
            let invocation_remote = resolve_invocation_remote(root_remote, remote)?;
            let source = resolve_cli_session(
                &client,
                host,
                token,
                &project,
                session.as_deref(),
                last,
                all,
                Some(false),
                true,
                no_alt_screen,
            )
            .await?;
            bind_invocation_bindings(
                &client,
                host,
                token,
                source.id,
                invocation_remote.as_ref(),
                dangerously_bypass_hook_trust,
            )
            .await?;
            if dangerously_bypass_hook_trust {
                warn_hook_trust_bypass();
            }
            let fork_result: Result<_, Box<dyn std::error::Error>> = async {
                let use_current_directory = should_resume_in_current_directory(
                    &client,
                    host,
                    token,
                    &source,
                    &project,
                    project_was_explicit,
                    &config_overrides,
                    codex_profile.as_deref(),
                    strict_config,
                    no_alt_screen,
                )
                .await?;
                let preferences = codex_execution_preferences_override(
                    model.clone(),
                    model_provider,
                    sandbox,
                    approval_policy,
                    approve_for_me,
                    dangerously_bypass_approvals_and_sandbox,
                    &additional_writable_roots,
                    &feature_overrides,
                    &config_overrides,
                    codex_profile,
                    strict_config,
                    &source.execution_preferences,
                );
                let fork: Conversation = decode_api_response(
                    client
                        .post(format!("{host}/api/v1/conversations/{}/fork", source.id))
                        .bearer_auth(token)
                        .json(&ForkConversationRequest {
                            origin_turn_id: None,
                            execution_preferences: preferences.clone(),
                        })
                        .send()
                        .await?,
                    "fork conversation",
                )
                .await?;
                Ok((fork, use_current_directory, preferences))
            }
            .await;
            let (mut fork, use_current_directory, preferences) = finish_invocation_bindings(
                &client,
                host,
                token,
                source.id,
                invocation_remote.is_some(),
                dangerously_bypass_hook_trust,
                fork_result,
            )
            .await?;
            if use_current_directory {
                match rebind_conversation_to_current_project(
                    &client,
                    host,
                    token,
                    &project,
                    fork.clone(),
                )
                .await
                {
                    Ok(rebound) => fork = rebound,
                    Err(error) => {
                        if invocation_remote.is_some() {
                            let _ = release_invocation_remote(&client, host, token, fork.id).await;
                        }
                        if dangerously_bypass_hook_trust {
                            let _ =
                                release_invocation_hook_trust_bypass(&client, host, token, fork.id)
                                    .await;
                        }
                        return Err(error);
                    }
                }
            }
            let mentions = codex_exec_mentions(&images)?;
            Box::pin(run_interactive(
                &client,
                host,
                token,
                &project,
                Some(fork),
                preferences,
                mentions,
                no_alt_screen,
                None,
                invocation_remote.as_ref(),
                dangerously_bypass_hook_trust,
            ))
            .await?;
        }
        CodexCompatCommand::Archive(args) => {
            let SessionArchiveArgs { session, remote } = *args;
            let invocation_remote = resolve_invocation_remote(root_remote, remote)?;
            let conversation = resolve_cli_session(
                &client,
                host,
                token,
                &project,
                Some(&session),
                false,
                true,
                None,
                true,
                false,
            )
            .await?;
            if let Some(remote) = invocation_remote.as_ref() {
                bind_invocation_remote(&client, host, token, conversation.id, remote).await?;
            }
            let result = run_cli_archive(&client, host, token, &project, &session, true).await;
            finish_invocation_remote_binding(
                &client,
                host,
                token,
                conversation.id,
                invocation_remote.is_some(),
                result,
            )
            .await?;
        }
        CodexCompatCommand::Unarchive(args) => {
            let SessionArchiveArgs { session, remote } = *args;
            let invocation_remote = resolve_invocation_remote(root_remote, remote)?;
            let conversation = resolve_cli_session(
                &client,
                host,
                token,
                &project,
                Some(&session),
                false,
                true,
                None,
                true,
                false,
            )
            .await?;
            if let Some(remote) = invocation_remote.as_ref() {
                bind_invocation_remote(&client, host, token, conversation.id, remote).await?;
            }
            let result = run_cli_archive(&client, host, token, &project, &session, false).await;
            finish_invocation_remote_binding(
                &client,
                host,
                token,
                conversation.id,
                invocation_remote.is_some(),
                result,
            )
            .await?;
        }
        CodexCompatCommand::Delete(args) => {
            let DeleteArgs {
                session,
                force,
                remote,
            } = *args;
            let invocation_remote = resolve_invocation_remote(root_remote, remote)?;
            let conversation = resolve_cli_session(
                &client,
                host,
                token,
                &project,
                Some(&session),
                false,
                true,
                None,
                true,
                false,
            )
            .await?;
            if let Some(remote) = invocation_remote.as_ref() {
                bind_invocation_remote(&client, host, token, conversation.id, remote).await?;
            }
            if let Err(error) =
                run_cli_delete(&client, host, token, &project, &session, force).await
            {
                if invocation_remote.is_some() {
                    let _ = release_invocation_remote(&client, host, token, conversation.id).await;
                }
                return Err(error);
            }
        }
        CodexCompatCommand::Review(args) => {
            let ReviewArgs {
                uncommitted,
                base,
                commit,
                title,
                prompt,
            } = *args;
            let preferences = codex_execution_preferences(
                model,
                model_provider,
                sandbox,
                approval_policy,
                approve_for_me,
                dangerously_bypass_approvals_and_sandbox,
                &additional_writable_roots,
                &feature_overrides,
                &config_overrides,
                codex_profile,
                strict_config,
                conversation_preferences(None, None, None, ApprovalMode::Ask),
            );
            Box::pin(run_codex_compatible_review(
                &client,
                host,
                token,
                &project,
                uncommitted,
                base,
                commit,
                title,
                prompt,
                preferences,
                dangerously_bypass_hook_trust,
            ))
            .await?;
        }
        CodexCompatCommand::Completion { .. } => unreachable!("completion handled offline"),
        CodexCompatCommand::Login(args) if matches!(args.command, Some(LoginCommand::Status)) => {
            run_codex(&client, host, token, CodexCommand::Account).await?;
        }
        CodexCompatCommand::Login(args) => {
            let LoginArgs {
                command: None,
                device_auth,
                with_api_key,
                with_access_token,
            } = *args
            else {
                unreachable!("login subcommand handled above")
            };
            let method = if device_auth {
                "device_code"
            } else if with_api_key {
                "api_key"
            } else if with_access_token {
                "access_token"
            } else {
                "browser"
            };
            let mut credential = read_optional_standard_input_credential(
                with_api_key || with_access_token,
                if with_api_key {
                    "API key"
                } else {
                    "access token"
                },
            )?;
            let result = run_codex_login(&client, host, token, method, credential.as_deref()).await;
            if let Some(value) = &mut credential {
                value.zeroize();
            }
            result?;
        }
        CodexCompatCommand::Logout => {
            run_codex(&client, host, token, CodexCommand::Logout).await?;
        }
        CodexCompatCommand::Features { command } => {
            run_codex_features(&client, host, token, command).await?;
        }
    }
    Ok(())
}

async fn run_codex_compatible_apply(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    project_root: &std::path::Path,
    task_id: &str,
    config_overrides: Vec<String>,
    feature_overrides: BTreeMap<String, bool>,
) -> Result<(), Box<dyn std::error::Error>> {
    let canonical = std::fs::canonicalize(project_root).map_err(|error| {
        format!(
            "cannot resolve project directory {}: {error}",
            project_root.display()
        )
    })?;
    if !canonical.is_dir() {
        return Err(format!("project path is not a directory: {}", canonical.display()).into());
    }
    let canonical_text = canonical
        .to_str()
        .ok_or("project path must be valid Unicode")?;
    let project = resolve_apply_project(client, host, token, canonical_text).await?;
    if !project.project.repository.is_git {
        return Err("`mcg apply` requires a Git repository".into());
    }
    let (enable_features, disable_features): (Vec<_>, Vec<_>) = feature_overrides
        .into_iter()
        .partition(|(_, enabled)| *enabled);
    let prepared: CloudDiffApply = decode_api_response(
        client
            .post(format!(
                "{host}/api/v1/projects/{}/codex-cloud-applies",
                project.project.project_id
            ))
            .bearer_auth(token)
            .json(&PrepareCloudDiffApplyRequest {
                task_id: task_id.to_owned(),
                config_overrides,
                enable_features: enable_features.into_iter().map(|(name, _)| name).collect(),
                disable_features: disable_features.into_iter().map(|(name, _)| name).collect(),
            })
            .send()
            .await?,
        "prepare Codex cloud diff",
    )
    .await?;
    let applied: CloudDiffApply = decode_api_response(
        client
            .post(format!(
                "{host}/api/v1/codex-cloud-applies/{}/apply",
                prepared.id
            ))
            .bearer_auth(token)
            .json(&ApplyCloudDiffRequest {
                expected_revision: prepared.revision,
            })
            .send()
            .await?,
        "apply Codex cloud diff",
    )
    .await?;
    if let Some(output) = applied
        .output
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        println!("{}", output.trim());
    }
    for path in &applied.changed_paths {
        println!("{path}");
    }
    Ok(())
}

async fn run_codex_compatible_update(
    client: &reqwest::Client,
    host: &str,
    config: &AppConfig,
    token_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let updater = mcg_updater::TrustedUpdaterConfig::compiled()?;
    let Some(candidate) = updater.check(env!("CARGO_PKG_VERSION")).await? else {
        println!("MetaCog is up to date ({}).", env!("CARGO_PKG_VERSION"));
        return Ok(());
    };
    println!(
        "MetaCog {} is available from {}.",
        candidate.version,
        updater.trusted_origin()
    );
    if let Some(notes) = candidate
        .notes
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        println!("{}", notes.trim());
    }

    #[cfg(not(windows))]
    {
        let _ = (client, host, config, token_path);
        return Err("This packaged CLI cannot safely replace the complete MetaCog application on this platform yet. Open Settings → Updates in MetaCog Desktop to install the verified release.".into());
    }

    #[cfg(windows)]
    {
        let installer = updater.download_and_verify(&candidate).await?;
        let reviewed_again = updater
            .check(env!("CARGO_PKG_VERSION"))
            .await?
            .ok_or("The checked update is no longer available.")?;
        if reviewed_again != candidate {
            return Err(format!(
                "The available update changed while downloading. Run `mcg update` again to review {}.",
                reviewed_again.version
            )
            .into());
        }
        let token = if let Ok(token) = std::env::var("MCG_AUTH_TOKEN") {
            token
        } else {
            std::fs::read_to_string(token_path).map_err(|error| {
                format!(
                    "cannot read local daemon token at {}: {error}; start MetaCog before installing the update",
                    token_path.display()
                )
            })?
        };
        install_verified_windows_cli_update(
            client,
            host,
            token.trim(),
            config,
            token_path,
            &candidate.version,
            &installer,
        )
        .await
    }
}

#[cfg(windows)]
async fn install_verified_windows_cli_update(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    config: &AppConfig,
    token_path: &std::path::Path,
    expected_version: &str,
    installer: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let current = std::env::current_exe()?.canonicalize()?;
    let install_root = current
        .parent()
        .ok_or("the packaged MetaCog CLI has no installation directory")?;
    let packaged_cli = install_root.join("mcg.exe");
    let metadata = std::fs::symlink_metadata(&packaged_cli)
        .map_err(|_| "`mcg update` is available only from a complete installed MetaCog release")?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || packaged_cli.canonicalize()? != current
        || !install_root.join("mcg-desktop.exe").is_file()
    {
        return Err(
            "`mcg update` is available only from a complete installed MetaCog release".into(),
        );
    }
    let supervisor_path = install_root.join("mcg-updater.exe");
    let supervisor_metadata = std::fs::symlink_metadata(&supervisor_path)
        .map_err(|_| "the packaged MetaCog update supervisor is missing")?;
    if supervisor_metadata.file_type().is_symlink() || !supervisor_metadata.is_file() {
        return Err("the packaged MetaCog update supervisor is not a regular file".into());
    }
    let supervisor = std::fs::read(&supervisor_path)?;
    let backup_id = uuid::Uuid::new_v4();
    create_and_verify_cli_update_backup(client, host, token, backup_id).await?;
    let staged = mcg_updater::stage_verified_windows_cli_installer(
        &config.data_directory,
        install_root,
        backup_id,
        std::process::id(),
        expected_version,
        installer,
        &supervisor,
    )?;
    if let Err(error) = request_stop(client, host, token).await {
        remove_staged_cli_update(&staged);
        return Err(format!("The local service could not be stopped for update: {error}").into());
    }
    if let Err(error) = wait_for_daemon_stop(client, host, token, 10).await {
        remove_staged_cli_update(&staged);
        let recovery = restart_packaged_service(host, token_path, install_root, config).await;
        return Err(format!("{error}. {recovery}").into());
    }
    let mut command = ProcessCommand::new(&staged.supervisor_path);
    command
        .arg("--apply")
        .arg(&staged.request_path)
        .env("MCG_DATA_DIR", &config.data_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command.creation_flags(0x0800_0000);
    if let Err(error) = command.spawn() {
        remove_staged_cli_update(&staged);
        let recovery = restart_packaged_service(host, token_path, install_root, config).await;
        return Err(
            format!("The supervised Windows updater could not start: {error}. {recovery}").into(),
        );
    }
    println!("Verified MetaCog {expected_version}; created and verified backup {backup_id}.");
    println!("The supervised installer will begin after this CLI process exits.");
    Ok(())
}

#[cfg(windows)]
fn remove_staged_cli_update(staged: &mcg_updater::StagedWindowsUpdate) {
    let _ = std::fs::remove_file(&staged.request_path);
    let _ = std::fs::remove_file(&staged.installer_path);
    let _ = std::fs::remove_file(&staged.supervisor_path);
}

async fn create_and_verify_cli_update_backup(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    backup_id: uuid::Uuid,
) -> Result<(), Box<dyn std::error::Error>> {
    let requested_at = Utc::now();
    let created: BackupSummaryResponse = decode_api_response(
        client
            .post(format!("{host}/api/v1/backups/{backup_id}"))
            .bearer_auth(token)
            .send()
            .await?,
        "create pre-update backup",
    )
    .await?;
    let verified: BackupSummaryResponse = decode_api_response(
        client
            .get(format!("{host}/api/v1/backups/{backup_id}/verify"))
            .bearer_auth(token)
            .send()
            .await?,
        "verify pre-update backup",
    )
    .await?;
    if created != verified {
        return Err("The automatic pre-update backup changed during verification.".into());
    }
    validate_cli_update_backup(&verified, backup_id, requested_at, Utc::now())?;
    Ok(())
}

fn validate_cli_update_backup(
    summary: &BackupSummaryResponse,
    backup_id: uuid::Uuid,
    requested_at: DateTime<Utc>,
    verified_at: DateTime<Utc>,
) -> Result<(), String> {
    if summary.backup_id != backup_id || summary.file_name != format!("{backup_id}.mcgb") {
        return Err("The automatic pre-update backup identity is not canonical.".into());
    }
    if summary.size_bytes == 0
        || summary.sha256.len() != 64
        || !summary
            .sha256
            .bytes()
            .all(|value| value.is_ascii_digit() || (b'a'..=b'f').contains(&value))
    {
        return Err("The automatic pre-update backup digest or size is invalid.".into());
    }
    if summary.manifest.application_version != env!("CARGO_PKG_VERSION") {
        return Err("The automatic pre-update backup came from another MetaCog version.".into());
    }
    if summary.manifest.secrets_included || summary.manifest.secret_encryption.is_some() {
        return Err("Automatic pre-update backups must not contain credentials.".into());
    }
    if !summary
        .manifest
        .entries
        .iter()
        .any(|entry| entry.path == "state/mcg.sqlite3" && entry.size_bytes > 0)
    {
        return Err(
            "The automatic pre-update backup does not contain the MetaCog database.".into(),
        );
    }
    let skew = TimeDelta::seconds(5);
    if summary.manifest.created_at < requested_at - skew
        || summary.manifest.created_at > verified_at + skew
    {
        return Err("The automatic pre-update backup is not fresh for this update.".into());
    }
    Ok(())
}

#[cfg(windows)]
async fn restart_packaged_service(
    host: &str,
    token_path: &std::path::Path,
    install_root: &std::path::Path,
    config: &AppConfig,
) -> String {
    let service = install_root.join("mcg-service.exe");
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let metadata = std::fs::symlink_metadata(&service)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err("the packaged service is not a regular file".into());
        }
        let bind_address = daemon_bind_address(host)?;
        let log_path = config.data_directory.join("mcg-daemon.log");
        let stdout_log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;
        let stderr_log = stdout_log.try_clone()?;
        let mut command = ProcessCommand::new(service);
        command
            .env("MCG_BIND_ADDRESS", bind_address)
            .env("MCG_DATA_DIR", &config.data_directory)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout_log))
            .stderr(Stdio::from(stderr_log));
        command.creation_flags(0x0800_0000 | 0x0000_0008);
        command.spawn()?;
        Ok(())
    })();
    match result {
        Ok(()) => {
            let client = reqwest::Client::new();
            let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
            loop {
                if daemon_status_if_ready(&client, host, token_path)
                    .await
                    .is_some()
                {
                    break "The previous local service was restarted.".into();
                }
                if tokio::time::Instant::now() >= deadline {
                    break "The previous local service was launched but did not become ready."
                        .into();
                }
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
        }
        Err(error) => format!("The previous local service could not restart: {error}"),
    }
}

async fn run_codex_features(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    command: FeaturesCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut features: Vec<mcg_codex_app_server::CodexFeature> = match command {
        FeaturesCommand::List => {
            decode_api_response(
                client
                    .get(format!("{host}/api/v1/codex/features"))
                    .bearer_auth(token)
                    .send()
                    .await?,
                "list Codex features",
            )
            .await?
        }
        FeaturesCommand::Enable { feature } => {
            set_codex_feature_cli(client, host, token, &feature, true).await?;
            return Ok(());
        }
        FeaturesCommand::Disable { feature } => {
            set_codex_feature_cli(client, host, token, &feature, false).await?;
            return Ok(());
        }
    };
    features.sort_unstable_by(|left, right| left.name.cmp(&right.name));
    let rows = features
        .iter()
        .map(|feature| {
            let stage = match feature.stage {
                mcg_codex_app_server::CodexFeatureStage::Beta => "beta",
                mcg_codex_app_server::CodexFeatureStage::UnderDevelopment => "under development",
                mcg_codex_app_server::CodexFeatureStage::Stable => "stable",
                mcg_codex_app_server::CodexFeatureStage::Deprecated => "deprecated",
                mcg_codex_app_server::CodexFeatureStage::Removed => "removed",
            };
            (feature.name.as_str(), stage, feature.enabled)
        })
        .collect::<Vec<_>>();
    let name_width = rows
        .iter()
        .map(|(name, _, _)| name.len())
        .max()
        .unwrap_or(0);
    let stage_width = rows
        .iter()
        .map(|(_, stage, _)| stage.len())
        .max()
        .unwrap_or(0);
    for (name, stage, enabled) in rows {
        println!("{name:<name_width$}  {stage:<stage_width$}  {enabled}");
    }
    Ok(())
}

async fn run_codex_agents(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    project: &std::path::Path,
    no_alt_screen: bool,
    remote: Option<&InvocationRemote>,
) -> Result<(), Box<dyn std::error::Error>> {
    let response: GlobalAgentSessionListResponse =
        get_authenticated(client, host, token, "/api/v1/agent-sessions?limit=200").await?;
    if !std::io::stdin().is_terminal() {
        if remote.is_some() {
            return Err("`mcg agents --remote` requires an interactive terminal".into());
        }
        for session in &response.sessions {
            println!(
                "{} · {} · {:?} · {:?} · {}",
                session.conversation_title,
                session.display_name,
                session.kind,
                session.state,
                session.session_id
            );
        }
        if response.sessions.is_empty() {
            println!("No governed agent sessions found.");
        }
        return Ok(());
    }
    if response.sessions.is_empty() {
        println!("No governed agent sessions found.");
        return Ok(());
    }
    let candidates = response
        .sessions
        .iter()
        .map(|session| SessionPickerCandidate {
            id: session.session_id.to_string(),
            title: format!("{} · {}", session.display_name, session.conversation_title),
            detail: format!(
                "{:?} · {:?} · {}",
                session.kind, session.state, session.session_id
            ),
        })
        .collect::<Vec<_>>();
    let selected = select_agent_session(&candidates, no_alt_screen)?
        .ok_or("agent session selection was cancelled")?;
    let selected = selected.parse::<ConversationAgentSessionId>()?;
    let summary = response
        .sessions
        .iter()
        .find(|session| session.session_id == selected)
        .ok_or("selected agent session is no longer available")?;
    let conversation =
        fetch_conversation(client, host, token, &summary.conversation_id.to_string())
            .await?
            .conversation;
    Box::pin(run_interactive(
        client,
        host,
        token,
        project,
        Some(conversation),
        None,
        Vec::new(),
        no_alt_screen,
        Some(selected),
        remote,
        false,
    ))
    .await
}

async fn set_codex_feature_cli(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    feature: &str,
    enabled: bool,
) -> Result<Vec<mcg_codex_app_server::CodexFeature>, Box<dyn std::error::Error>> {
    let feature = feature.trim();
    if feature.is_empty()
        || feature.len() > 128
        || !feature
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err("feature name must be a non-empty lowercase ASCII key".into());
    }
    decode_api_response(
        client
            .put(format!("{host}/api/v1/codex/features/{feature}"))
            .bearer_auth(token)
            .json(&serde_json::json!({"enabled": enabled}))
            .send()
            .await?,
        if enabled {
            "enable Codex feature"
        } else {
            "disable Codex feature"
        },
    )
    .await
}

const MAX_CONSUMER_INPUT_BYTES: u64 = 1_048_576;
const MAX_CONSUMER_GOAL_CHARS: usize = 32_768;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ConsumerOutput {
    Human,
    Json,
    StreamJson,
}

const fn consumer_output(json: bool, stream_json: bool) -> ConsumerOutput {
    if json {
        ConsumerOutput::Json
    } else if stream_json {
        ConsumerOutput::StreamJson
    } else {
        ConsumerOutput::Human
    }
}

fn validated_goal(value: &str) -> Result<String, Box<dyn std::error::Error>> {
    let goal = value.trim();
    if goal.is_empty() {
        return Err("consumer goal cannot be empty".into());
    }
    if goal.chars().count() > MAX_CONSUMER_GOAL_CHARS {
        return Err(
            format!("consumer goal exceeds the {MAX_CONSUMER_GOAL_CHARS}-character limit").into(),
        );
    }
    Ok(goal.to_owned())
}

fn os_words_to_goal(words: Vec<OsString>) -> Result<String, Box<dyn std::error::Error>> {
    let words = words
        .into_iter()
        .map(|word| {
            word.into_string()
                .map_err(|_| "consumer goals must be valid Unicode")
        })
        .collect::<Result<Vec<_>, _>>()?;
    validated_goal(&words.join(" "))
}

fn read_optional_piped_input() -> Result<Option<String>, Box<dyn std::error::Error>> {
    if std::io::stdin().is_terminal() {
        return Ok(None);
    }
    let input = read_bounded_stdin()?;
    if input.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(input))
    }
}

fn read_bounded_stdin() -> Result<String, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(MAX_CONSUMER_INPUT_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_CONSUMER_INPUT_BYTES {
        return Err(
            format!("standard input exceeds the {MAX_CONSUMER_INPUT_BYTES}-byte limit").into(),
        );
    }
    Ok(String::from_utf8(bytes).map_err(|_| "standard input must be valid UTF-8")?)
}

fn generate_cli_completion(shell: CompletionShell) -> Result<(), Box<dyn std::error::Error>> {
    // clap_complete recursively walks MetaCog's unusually broad compatibility + native command
    // tree. Generate on a bounded worker stack so Windows' small main-thread stack cannot fail.
    let output = std::thread::Builder::new()
        .name("mcg-completion".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(move || {
            let mut output = Vec::new();
            generate(
                Shell::from(shell),
                &mut combined_cli_command(),
                "mcg",
                &mut output,
            );
            output
        })?
        .join()
        .map_err(|_| "shell completion generator panicked")?;
    std::io::stdout().write_all(&output)?;
    Ok(())
}

async fn list_cli_sessions(
    client: &reqwest::Client,
    host: &str,
    token: &str,
) -> Result<Vec<Conversation>, Box<dyn std::error::Error>> {
    let response: ConversationListResponse = decode_api_response(
        client
            .get(format!("{host}/api/v1/conversations"))
            .query(&[("include_archived", true)])
            .bearer_auth(token)
            .send()
            .await?,
        "list saved conversations",
    )
    .await?;
    Ok(response.conversations)
}

async fn current_cli_project_id(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    project: &std::path::Path,
) -> Result<ProjectId, Box<dyn std::error::Error>> {
    let canonical = std::fs::canonicalize(project)?;
    let root = canonical
        .to_str()
        .ok_or("project directory must be valid Unicode")?;
    Ok(resolve_consumer_project(client, host, token, root)
        .await?
        .project
        .project_id)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResumeCwdDecision {
    Current,
    Session,
    Prompt,
}

fn resume_cwd_config_override(
    overrides: &[CodexConfigOverride],
) -> Result<Option<CodexResumeCwd>, Box<dyn std::error::Error>> {
    let Some(value) = overrides
        .iter()
        .rev()
        .find(|override_value| override_value.key == "tui.resume_cwd")
        .map(|override_value| &override_value.value)
    else {
        return Ok(None);
    };
    match value.as_str() {
        Some("current") => Ok(Some(CodexResumeCwd::Current)),
        Some("session") => Ok(Some(CodexResumeCwd::Session)),
        _ => Err(format!(
            "Codex configuration override `tui.resume_cwd` must be `current` or `session`, not {value}"
        )
        .into()),
    }
}

fn resume_cwd_decision(
    current: &std::path::Path,
    session: Option<&std::path::Path>,
    explicit_current: bool,
    preference: Option<CodexResumeCwd>,
    interactive_terminal: bool,
) -> Result<ResumeCwdDecision, Box<dyn std::error::Error>> {
    let Some(session) = session else {
        return Ok(if explicit_current {
            ResumeCwdDecision::Current
        } else {
            ResumeCwdDecision::Session
        });
    };
    if paths_match_for_host(session, &current.to_string_lossy()) {
        return Ok(ResumeCwdDecision::Session);
    }
    if explicit_current {
        return Ok(ResumeCwdDecision::Current);
    }
    match preference {
        Some(CodexResumeCwd::Current) => Ok(ResumeCwdDecision::Current),
        Some(CodexResumeCwd::Session) => Ok(ResumeCwdDecision::Session),
        None if interactive_terminal => Ok(ResumeCwdDecision::Prompt),
        None => Err(
            "the saved session uses a different working directory; pass -C <DIR> or set `tui.resume_cwd` to `current` or `session`"
                .into(),
        ),
    }
}

async fn conversation_project_root(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    conversation: &Conversation,
) -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
    let Some(project_id) = conversation.project_id else {
        return Ok(None);
    };
    let value: serde_json::Value = decode_api_response(
        client
            .get(format!("{host}/api/v1/projects/{project_id}"))
            .bearer_auth(token)
            .send()
            .await?,
        "load the saved session project",
    )
    .await?;
    let root = value
        .get("root")
        .and_then(serde_json::Value::as_str)
        .ok_or("saved session project did not contain a root")?;
    Ok(Some(std::fs::canonicalize(root)?))
}

async fn effective_resume_cwd(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    cwd: &std::path::Path,
    profile: Option<&str>,
    strict: bool,
) -> Result<Option<CodexResumeCwd>, Box<dyn std::error::Error>> {
    let cwd = cwd
        .to_str()
        .ok_or("current directory must be valid Unicode")?;
    decode_api_response(
        client
            .get(format!("{host}/api/v1/codex/resume-cwd"))
            .bearer_auth(token)
            .query(&CodexResumeCwdQuery {
                cwd: Some(cwd.to_owned()),
                profile: profile.map(str::to_owned),
                strict,
            })
            .send()
            .await?,
        "resolve Codex resume working-directory preference",
    )
    .await
}

async fn should_resume_in_current_directory(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    conversation: &Conversation,
    current: &std::path::Path,
    explicit_current: bool,
    config_overrides: &[CodexConfigOverride],
    profile: Option<&str>,
    strict: bool,
    no_alt_screen: bool,
) -> Result<bool, Box<dyn std::error::Error>> {
    let current = std::fs::canonicalize(current)?;
    let session = conversation_project_root(client, host, token, conversation).await?;
    let explicit_preference = resume_cwd_config_override(config_overrides)?;
    let preference = if explicit_preference.is_some() || explicit_current || session.is_none() {
        explicit_preference
    } else {
        effective_resume_cwd(client, host, token, &current, profile, strict).await?
    };
    match resume_cwd_decision(
        &current,
        session.as_deref(),
        explicit_current,
        preference,
        std::io::stdin().is_terminal(),
    )? {
        ResumeCwdDecision::Current => Ok(true),
        ResumeCwdDecision::Session => Ok(false),
        ResumeCwdDecision::Prompt => {
            let session = session.expect("prompt requires a saved session directory");
            match select_resume_cwd(
                &current.display().to_string(),
                &session.display().to_string(),
                no_alt_screen,
            )?
            .as_deref()
            {
                Some("current") => Ok(true),
                Some("session") => Ok(false),
                Some(choice) => Err(format!("unknown working-directory choice `{choice}`").into()),
                None => Err("working-directory selection was cancelled".into()),
            }
        }
    }
}

async fn rebind_conversation_to_current_project(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    project: &std::path::Path,
    conversation: Conversation,
) -> Result<Conversation, Box<dyn std::error::Error>> {
    let project_id = current_cli_project_id(client, host, token, project).await?;
    if conversation.project_id == Some(project_id) {
        return Ok(conversation);
    }
    decode_api_response(
        client
            .patch(format!("{host}/api/v1/conversations/{}", conversation.id))
            .bearer_auth(token)
            .json(&UpdateConversationRequest {
                title: conversation.title.clone(),
                project_id: Some(project_id),
            })
            .send()
            .await?,
        "use the current directory for the conversation",
    )
    .await
}

fn filter_cli_session_candidates(
    sessions: &mut Vec<Conversation>,
    project_id: Option<ProjectId>,
    include_non_interactive: bool,
) {
    if !include_non_interactive {
        sessions.retain(|conversation| {
            conversation.interaction_kind == ConversationInteractionKind::Interactive
        });
    }
    if let Some(project_id) = project_id {
        sessions.retain(|conversation| conversation.project_id == Some(project_id));
    }
}

fn choose_cli_session(
    sessions: &[Conversation],
    no_alt_screen: bool,
) -> Result<Conversation, Box<dyn std::error::Error>> {
    if !std::io::stdin().is_terminal() {
        return Err("SESSION or --last is required when standard input is not a terminal".into());
    }
    if sessions.is_empty() {
        return Err("no matching saved conversations were found".into());
    }
    let candidates = sessions
        .iter()
        .map(|conversation| SessionPickerCandidate {
            id: conversation.id.to_string(),
            title: conversation.title.clone(),
            detail: format!(
                "{} · {} · {}",
                conversation
                    .project_id
                    .map_or_else(|| "No project".to_owned(), |id| id.to_string()),
                match conversation.interaction_kind {
                    ConversationInteractionKind::Interactive => "interactive",
                    ConversationInteractionKind::NonInteractive => "non-interactive",
                },
                conversation.updated_at.format("%Y-%m-%d %H:%M UTC")
            ),
        })
        .collect::<Vec<_>>();
    let selected = select_session(&candidates, no_alt_screen)?
        .ok_or("conversation selection was cancelled")?;
    let selected = ConversationId::from_str(&selected)?;
    sessions
        .iter()
        .find(|conversation| conversation.id == selected)
        .cloned()
        .ok_or_else(|| "selected conversation is no longer available".into())
}

async fn resolve_cli_session(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    project: &std::path::Path,
    selector: Option<&str>,
    last: bool,
    all: bool,
    archived_state: Option<bool>,
    include_non_interactive: bool,
    no_alt_screen: bool,
) -> Result<Conversation, Box<dyn std::error::Error>> {
    if selector.is_some() && last {
        return Err("SESSION and --last cannot be used together".into());
    }
    let mut sessions = list_cli_sessions(client, host, token).await?;
    if let Some(archived) = archived_state {
        sessions.retain(|conversation| {
            conversation.state
                == if archived {
                    ConversationState::Archived
                } else {
                    ConversationState::Active
                }
        });
    }
    if let Some(selector) = selector {
        if let Ok(id) = ConversationId::from_str(selector)
            && let Some(conversation) = sessions.iter().find(|item| item.id == id)
        {
            return Ok(conversation.clone());
        }
        let matches = sessions
            .iter()
            .filter(|conversation| conversation.title == selector)
            .cloned()
            .collect::<Vec<_>>();
        return match matches.as_slice() {
            [conversation] => Ok(conversation.clone()),
            [] => Err(format!("saved conversation `{selector}` was not found").into()),
            _ => Err(format!(
                "multiple conversations are named `{selector}`; use a conversation UUID"
            )
            .into()),
        };
    }
    let project_id = if all {
        None
    } else {
        Some(current_cli_project_id(client, host, token, project).await?)
    };
    filter_cli_session_candidates(&mut sessions, project_id, include_non_interactive);
    sessions.sort_by_key(|conversation| std::cmp::Reverse(conversation.updated_at));
    if last {
        sessions
            .into_iter()
            .next()
            .ok_or_else(|| "no matching saved conversations were found".into())
    } else {
        choose_cli_session(&sessions, no_alt_screen)
    }
}

async fn run_cli_archive(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    project: &std::path::Path,
    selector: &str,
    archived: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let conversation = resolve_cli_session(
        client,
        host,
        token,
        project,
        Some(selector),
        false,
        true,
        Some(!archived),
        true,
        false,
    )
    .await?;
    let updated: Conversation = decode_api_response(
        client
            .post(format!(
                "{host}/api/v1/conversations/{}/archive",
                conversation.id
            ))
            .bearer_auth(token)
            .json(&ArchiveConversationRequest { archived })
            .send()
            .await?,
        if archived {
            "archive conversation"
        } else {
            "restore conversation"
        },
    )
    .await?;
    println!(
        "{} {} ({})",
        if archived { "Archived" } else { "Restored" },
        updated.title,
        updated.id
    );
    Ok(())
}

async fn delete_cli_conversation_by_id(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    id: ConversationId,
) -> Result<ConversationDeletionResult, Box<dyn std::error::Error>> {
    let preview: ConversationDeletionPreview = decode_api_response(
        client
            .get(format!("{host}/api/v1/conversations/{id}/delete-preview"))
            .bearer_auth(token)
            .send()
            .await?,
        "preview conversation deletion",
    )
    .await?;
    decode_api_response(
        client
            .delete(format!("{host}/api/v1/conversations/{id}"))
            .bearer_auth(token)
            .json(&mcg_api::DeleteConversationRequest {
                confirmation: preview.confirmation,
            })
            .send()
            .await?,
        "delete conversation",
    )
    .await
}

async fn run_cli_delete(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    project: &std::path::Path,
    selector: &str,
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if force && ConversationId::from_str(selector).is_err() {
        return Err("--force requires a conversation UUID; names require confirmation".into());
    }
    let conversation = resolve_cli_session(
        client,
        host,
        token,
        project,
        Some(selector),
        false,
        true,
        None,
        true,
        false,
    )
    .await?;
    let preview: ConversationDeletionPreview = decode_api_response(
        client
            .get(format!(
                "{host}/api/v1/conversations/{}/delete-preview",
                conversation.id
            ))
            .bearer_auth(token)
            .send()
            .await?,
        "preview conversation deletion",
    )
    .await?;
    if !force {
        if !std::io::stdin().is_terminal() {
            return Err(
                "deletion requires an interactive confirmation or UUID plus --force".into(),
            );
        }
        eprintln!(
            "Permanently delete '{}' ({}) and {} child conversation(s)? Type DELETE to continue:",
            preview.conversation.title,
            preview.conversation.id,
            preview.children.len()
        );
        let mut confirmation = String::new();
        std::io::stdin().read_line(&mut confirmation)?;
        if confirmation.trim() != "DELETE" {
            return Err("conversation deletion cancelled".into());
        }
    }
    let result: ConversationDeletionResult = decode_api_response(
        client
            .delete(format!("{host}/api/v1/conversations/{}", conversation.id))
            .bearer_auth(token)
            .json(&mcg_api::DeleteConversationRequest {
                confirmation: preview.confirmation,
            })
            .send()
            .await?,
        "delete conversation",
    )
    .await?;
    println!(
        "Deleted {} conversation(s).",
        result.deleted_conversation_ids.len()
    );
    Ok(())
}

async fn run_interactive(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    project: &std::path::Path,
    initial_conversation: Option<Conversation>,
    initial_preferences: Option<ExecutionPreferences>,
    initial_mentions: Vec<String>,
    no_alt_screen: bool,
    initial_agent_session_id: Option<ConversationAgentSessionId>,
    remote: Option<&InvocationRemote>,
    bypass_hook_trust: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let (mut conversation, mut project_id, mut project_root, root, mut preferences) =
        if let Some(conversation) = initial_conversation {
            let project_id = conversation.project_id;
            let project_root = match project_id {
                Some(project_id) => {
                    let value: serde_json::Value = decode_api_response(
                        client
                            .get(format!("{host}/api/v1/projects/{project_id}"))
                            .bearer_auth(token)
                            .send()
                            .await?,
                        "load the resumed conversation project",
                    )
                    .await?;
                    let root = value
                        .get("root")
                        .and_then(serde_json::Value::as_str)
                        .ok_or("resumed project did not contain a root")?;
                    Some(std::fs::canonicalize(root)?)
                }
                None => None,
            };
            let root = project_root.as_ref().map_or_else(
                || "project-free".to_owned(),
                |root| root.display().to_string(),
            );
            let preferences =
                initial_preferences.unwrap_or_else(|| conversation.execution_preferences.clone());
            (conversation, project_id, project_root, root, preferences)
        } else {
            let canonical_root = std::fs::canonicalize(project)?;
            let root = canonical_root
                .to_str()
                .ok_or("project directory must be valid Unicode")?
                .to_owned();
            let resolved = resolve_consumer_project(client, host, token, &root).await?;
            let project_id = Some(resolved.project.project_id);
            let project_root = Some(canonical_root);
            let preferences = initial_preferences
                .unwrap_or_else(|| conversation_preferences(None, None, None, ApprovalMode::Ask));
            let conversation = create_interactive_conversation(
                client,
                host,
                token,
                project_id,
                &preferences,
                "MetaCog CLI",
            )
            .await?;
            (conversation, project_id, project_root, root, preferences)
        };
    bind_invocation_bindings(
        client,
        host,
        token,
        conversation.id,
        remote,
        bypass_hook_trust,
    )
    .await?;
    if bypass_hook_trust {
        warn_hook_trust_bypass();
    }
    let result: Result<(), Box<dyn std::error::Error>> = async {
        let mut pending_mentions = initial_mentions;
        let initial_view =
            fetch_conversation(client, host, token, &conversation.id.to_string()).await?;
        let branch = git_branch(project_root.as_deref()).unwrap_or_else(|| "no branch".into());
        let mut model = TuiModel::new(initial_view, root.clone(), branch);
        if let Some(session_id) = initial_agent_session_id
            && !model.focus_agent_session(session_id)
        {
            return Err("selected agent session is absent from its conversation".into());
        }
        if let Ok(tui_preferences) = interactive_tui_preferences(client, host, token).await {
            model.set_cli_preferences(tui_preferences);
        }
        let (stream_tx, mut stream_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut stream_task = spawn_conversation_stream(
            client.clone(),
            host.to_owned(),
            token.to_owned(),
            conversation.id,
            stream_tx.clone(),
        );
        let mut terminal = TuiTerminal::enter(no_alt_screen)?;
        loop {
            while let Ok(event) = stream_rx.try_recv() {
                if event.view.conversation.id == conversation.id {
                    conversation = event.view.conversation.clone();
                    model.set_view(event.view);
                }
            }

            if !model.is_active() {
                if let Some(line) = model.pop_queue() {
                    let mut command_output = Vec::new();
                    match dispatch_interactive_line(
                        client,
                        host,
                        token,
                        &mut conversation,
                        &mut project_id,
                        &mut project_root,
                        &mut preferences,
                        &mut pending_mentions,
                        line,
                        &mut command_output,
                    )
                    .await
                    {
                        Ok(Some(view)) => {
                            let changed_conversation = view.conversation.id != conversation.id;
                            transfer_invocation_bindings(
                                client,
                                host,
                                token,
                                conversation.id,
                                view.conversation.id,
                                remote,
                                bypass_hook_trust,
                            )
                            .await?;
                            conversation = view.conversation.clone();
                            model.set_view(view);
                            if changed_conversation {
                                stream_task.abort();
                                stream_task = spawn_conversation_stream(
                                    client.clone(),
                                    host.to_owned(),
                                    token.to_owned(),
                                    conversation.id,
                                    stream_tx.clone(),
                                );
                            }
                        }
                        Ok(None) => {}
                        Err(error) => model.set_status(format!("Queued message failed: {error}")),
                    }
                    model.push_notices(command_output);
                    if let Ok(tui_preferences) =
                        interactive_tui_preferences(client, host, token).await
                    {
                        model.set_cli_preferences(tui_preferences);
                    }
                }
            }

            if let Some(request) = model.palette_request() {
                let plugins = interactive_plugin_candidates(client, host, token).await?;
                let candidates = interactive_palette_candidates(
                    client,
                    host,
                    token,
                    conversation.id,
                    project_id,
                    project_root.as_deref(),
                    &preferences,
                    &plugins,
                    request,
                )
                .await;
                model.set_candidates(candidates);
            } else {
                model.set_candidates(Vec::new());
            }
            terminal.draw(&mut model)?;
            if !crossterm::event::poll(std::time::Duration::from_millis(75))? {
                continue;
            }
            let Some(action) = model.handle_event(crossterm::event::read()?)? else {
                continue;
            };
            match action {
                TuiAction::Exit => break,
                TuiAction::AnswerInput {
                    request_id,
                    answers,
                } => {
                    match answer_interactive_input(
                        client,
                        host,
                        token,
                        conversation.id,
                        request_id,
                        answers,
                    )
                    .await
                    {
                        Ok(view) => {
                            conversation = view.conversation.clone();
                            model.set_view(view);
                        }
                        Err(error) => model.set_status(format!("Answer failed: {error}")),
                    }
                }
                TuiAction::Queue(line) => {
                    model.queue(line);
                    model.set_status(format!("{} queued", model.queued_len()));
                }
                TuiAction::MessageAgent {
                    session_id,
                    content,
                } => {
                    let result: Result<mcg_domain::ConversationTeamState, _> = decode_api_response(
                    client
                        .post(format!(
                            "{host}/api/v1/conversations/{}/agent-sessions/{session_id}/messages",
                            conversation.id
                        ))
                        .bearer_auth(token)
                        .json(&mcg_api::MessageAgentSessionRequest { content })
                        .send()
                        .await?,
                    "message agent thread",
                )
                .await;
                    match result {
                        Ok(_) => {
                            match fetch_conversation(
                                client,
                                host,
                                token,
                                &conversation.id.to_string(),
                            )
                            .await
                            {
                                Ok(view) => {
                                    conversation = view.conversation.clone();
                                    model.set_view(view);
                                    model.set_status("Agent message delivered");
                                }
                                Err(error) => {
                                    model.set_status(format!(
                                        "Refresh after agent reply failed: {error}"
                                    ));
                                }
                            }
                        }
                        Err(error) => model.set_status(format!("Agent message failed: {error}")),
                    }
                }
                TuiAction::CopyLatest => {
                    model.set_status(
                        if copy_to_clipboard(model.latest_assistant_text().unwrap_or_default()) {
                            "Copied latest response"
                        } else {
                            "Clipboard unavailable"
                        },
                    );
                }
                TuiAction::ForkPrevious => {
                    let line = SubmittedLine {
                        content: "/fork".into(),
                        mentions: Vec::new(),
                    };
                    let mut command_output = Vec::new();
                    let previous_conversation_id = conversation.id;
                    if let Err(error) = run_interactive_slash(
                        client,
                        host,
                        token,
                        &mut conversation,
                        &mut project_id,
                        &mut project_root,
                        &mut preferences,
                        &mut pending_mentions,
                        line,
                        &mut command_output,
                    )
                    .await
                    {
                        model.set_status(format!("Fork failed: {error}"));
                    } else if conversation.id != previous_conversation_id {
                        transfer_invocation_bindings(
                            client,
                            host,
                            token,
                            previous_conversation_id,
                            conversation.id,
                            remote,
                            bypass_hook_trust,
                        )
                        .await?;
                        let view =
                            fetch_conversation(client, host, token, &conversation.id.to_string())
                                .await?;
                        model.set_view(view);
                        stream_task.abort();
                        stream_task = spawn_conversation_stream(
                            client.clone(),
                            host.to_owned(),
                            token.to_owned(),
                            conversation.id,
                            stream_tx.clone(),
                        );
                    }
                    model.push_notices(command_output);
                    if let Ok(tui_preferences) =
                        interactive_tui_preferences(client, host, token).await
                    {
                        model.set_cli_preferences(tui_preferences);
                    }
                }
                TuiAction::Steer(line) => {
                    if let Some((requested, _, _)) = parse_interactive_slash(&line) {
                        let context = interactive_command_context(
                            client,
                            host,
                            token,
                            conversation.id,
                            project_id,
                            &preferences,
                        )
                        .await;
                        if let Some(command) =
                            command_named(&requested.to_ascii_lowercase(), context)
                        {
                            match command.queue_policy {
                                QueuePolicy::QueueWhenActive => {
                                    model.queue(line);
                                    model.set_status(format!("{} queued", model.queued_len()));
                                }
                                QueuePolicy::RejectWhileActive => {
                                    model.set_status(format!(
                                        "/{} is unavailable while the conversation is active",
                                        command.name
                                    ));
                                }
                                QueuePolicy::Immediate => {
                                    let mut command_output = Vec::new();
                                    match dispatch_interactive_line(
                                        client,
                                        host,
                                        token,
                                        &mut conversation,
                                        &mut project_id,
                                        &mut project_root,
                                        &mut preferences,
                                        &mut pending_mentions,
                                        line,
                                        &mut command_output,
                                    )
                                    .await
                                    {
                                        Ok(Some(view)) => {
                                            let changed_conversation =
                                                view.conversation.id != conversation.id;
                                            transfer_invocation_bindings(
                                                client,
                                                host,
                                                token,
                                                conversation.id,
                                                view.conversation.id,
                                                remote,
                                                bypass_hook_trust,
                                            )
                                            .await?;
                                            conversation = view.conversation.clone();
                                            model.set_view(view);
                                            if changed_conversation {
                                                stream_task.abort();
                                                stream_task = spawn_conversation_stream(
                                                    client.clone(),
                                                    host.to_owned(),
                                                    token.to_owned(),
                                                    conversation.id,
                                                    stream_tx.clone(),
                                                );
                                            }
                                        }
                                        Ok(None) => {}
                                        Err(error) => {
                                            model.set_status(format!("Command failed: {error}"));
                                        }
                                    }
                                    model.push_notices(command_output);
                                    if let Ok(tui_preferences) =
                                        interactive_tui_preferences(client, host, token).await
                                    {
                                        model.set_cli_preferences(tui_preferences);
                                    }
                                }
                            }
                            continue;
                        }
                    }
                    let Some(turn) = model.active_turn() else {
                        continue;
                    };
                    match steer_interactive_message(client, host, token, &conversation, &turn, line)
                        .await
                    {
                        Ok(view) => model.set_view(view),
                        Err(error) => model.set_status(format!("Steer failed: {error}")),
                    }
                }
                TuiAction::Submit(line) => {
                    let mut command_output = Vec::new();
                    match dispatch_interactive_line(
                        client,
                        host,
                        token,
                        &mut conversation,
                        &mut project_id,
                        &mut project_root,
                        &mut preferences,
                        &mut pending_mentions,
                        line,
                        &mut command_output,
                    )
                    .await
                    {
                        Ok(Some(view)) => {
                            let changed_conversation = view.conversation.id != conversation.id;
                            transfer_invocation_bindings(
                                client,
                                host,
                                token,
                                conversation.id,
                                view.conversation.id,
                                remote,
                                bypass_hook_trust,
                            )
                            .await?;
                            conversation = view.conversation.clone();
                            model.set_view(view);
                            if changed_conversation {
                                stream_task.abort();
                                stream_task = spawn_conversation_stream(
                                    client.clone(),
                                    host.to_owned(),
                                    token.to_owned(),
                                    conversation.id,
                                    stream_tx.clone(),
                                );
                            }
                            let project_label = project_root.as_ref().map_or_else(
                                || "No project".into(),
                                |path| path.display().to_string(),
                            );
                            model.set_workspace(
                                project_label,
                                git_branch(project_root.as_deref())
                                    .unwrap_or_else(|| "no branch".into()),
                            );
                        }
                        Ok(None) => {}
                        Err(error) => model.set_status(format!("Command failed: {error}")),
                    }
                    model.push_notices(command_output);
                    if let Ok(tui_preferences) =
                        interactive_tui_preferences(client, host, token).await
                    {
                        model.set_cli_preferences(tui_preferences);
                    }
                }
            }
        }
        stream_task.abort();
        Ok(())
    }
    .await;
    finish_invocation_bindings(
        client,
        host,
        token,
        conversation.id,
        remote.is_some(),
        bypass_hook_trust,
        result,
    )
    .await
}

async fn answer_interactive_input(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    conversation_id: ConversationId,
    request_id: serde_json::Value,
    answers: serde_json::Value,
) -> Result<ConversationView, Box<dyn std::error::Error>> {
    decode_api_response(
        client
            .post(format!(
                "{host}/api/v1/conversations/{conversation_id}/input"
            ))
            .bearer_auth(token)
            .json(&AnswerConversationInputRequest {
                request_id,
                answers,
            })
            .send()
            .await?,
        "answer interactive question",
    )
    .await
}

async fn dispatch_interactive_line(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    conversation: &mut Conversation,
    project_id: &mut Option<ProjectId>,
    project_root: &mut Option<PathBuf>,
    preferences: &mut ExecutionPreferences,
    pending_mentions: &mut Vec<String>,
    line: SubmittedLine,
    command_output: &mut Vec<String>,
) -> Result<Option<ConversationView>, Box<dyn std::error::Error>> {
    if line.content.trim_start().starts_with('/') {
        let control = run_interactive_slash(
            client,
            host,
            token,
            conversation,
            project_id,
            project_root,
            preferences,
            pending_mentions,
            line,
            command_output,
        )
        .await?;
        if control == InteractiveControl::Exit {
            return Err("/exit".into());
        }
        return Ok(Some(
            fetch_conversation(client, host, token, &conversation.id.to_string()).await?,
        ));
    }
    if let Some(command) = line.content.trim_start().strip_prefix('!') {
        let command = command.trim();
        if command.is_empty() {
            return Err("add a command after !".into());
        }
        let (command, background) = command
            .strip_suffix(" &")
            .map_or((command, false), |command| (command.trim_end(), true));
        let argv = if cfg!(windows) {
            vec![
                "powershell.exe".to_owned(),
                "-NoProfile".to_owned(),
                "-NonInteractive".to_owned(),
                "-Command".to_owned(),
                command.to_owned(),
            ]
        } else {
            vec!["sh".to_owned(), "-lc".to_owned(), command.to_owned()]
        };
        let request = mcg_codex_app_server::CodexCommandExecRequest {
            command: argv,
            cwd: None,
            timeout_ms: Some(120_000),
            process_id: None,
            writable: preferences.sandbox_mode.map_or(
                !matches!(preferences.approval_mode, ApprovalMode::ReadOnly),
                |mode| !matches!(mode, ExecutionSandboxMode::ReadOnly),
            ),
        };
        command_output.push(format!("$ {command}"));
        if background {
            let execution: mcg_runtime::ConversationCommandExecution = decode_api_response(
                client
                    .post(format!(
                        "{host}/api/v1/conversations/{}/codex/commands",
                        conversation.id
                    ))
                    .bearer_auth(token)
                    .json(&request)
                    .send()
                    .await?,
                "start governed background command",
            )
            .await?;
            command_output.push(format!(
                "started {} · use /ps or /stop {}",
                execution.id, execution.id
            ));
            return Ok(Some(
                fetch_conversation(client, host, token, &conversation.id.to_string()).await?,
            ));
        }
        let result: mcg_codex_app_server::CodexCommandExecResult = decode_api_response(
            client
                .post(format!(
                    "{host}/api/v1/conversations/{}/codex/command",
                    conversation.id
                ))
                .bearer_auth(token)
                .json(&request)
                .send()
                .await?,
            "run governed shell command",
        )
        .await?;
        if !result.stdout.trim().is_empty() {
            command_output.push(result.stdout.trim_end().to_owned());
        }
        if !result.stderr.trim().is_empty() {
            command_output.push(format!("stderr: {}", result.stderr.trim_end()));
        }
        command_output.push(format!("exit {}", result.exit_code));
        return Ok(Some(
            fetch_conversation(client, host, token, &conversation.id.to_string()).await?,
        ));
    }
    let view = start_interactive_message(
        client,
        host,
        token,
        conversation,
        preferences,
        line,
        pending_mentions,
    )
    .await?;
    pending_mentions.clear();
    Ok(Some(view))
}

fn git_branch(project: Option<&std::path::Path>) -> Option<String> {
    let project = project?;
    let output = ProcessCommand::new("git")
        .args(["branch", "--show-current"])
        .current_dir(project)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn spawn_conversation_stream(
    client: reqwest::Client,
    host: String,
    token: String,
    conversation_id: mcg_domain::ConversationId,
    sender: tokio::sync::mpsc::UnboundedSender<ConversationStreamEvent>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut after = 0_u64;
        loop {
            let response = client
                .get(format!(
                    "{host}/api/v1/conversations/{conversation_id}/events/stream"
                ))
                .query(&[("after", after)])
                .bearer_auth(&token)
                .header(reqwest::header::ACCEPT, "text/event-stream")
                .send()
                .await;
            let Ok(response) = response else {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                continue;
            };
            if !response.status().is_success() {
                tokio::time::sleep(std::time::Duration::from_millis(750)).await;
                continue;
            }
            let mut stream = response.bytes_stream().eventsource();
            while let Some(frame) = stream.next().await {
                let Ok(frame) = frame else {
                    break;
                };
                if frame.data.is_empty() {
                    continue;
                }
                let Ok(event) = serde_json::from_str::<ConversationStreamEvent>(&frame.data) else {
                    continue;
                };
                after = after.max(event.sequence);
                if sender.send(event).is_err() {
                    return;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
    })
}

async fn start_interactive_message(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    conversation: &Conversation,
    preferences: &ExecutionPreferences,
    line: SubmittedLine,
    pending_mentions: &[String],
) -> Result<ConversationView, Box<dyn std::error::Error>> {
    let (content, appended_mentions) =
        resolve_cli_mentions(client, host, token, line.content, pending_mentions).await?;
    let mut mentions = line.mentions;
    mentions.extend(appended_mentions);
    mentions.sort_by_key(|mention| mention.byte_start);
    decode_api_response(
        client
            .post(format!(
                "{host}/api/v1/conversations/{}/messages",
                conversation.id
            ))
            .bearer_auth(token)
            .json(&SendConversationMessageRequest {
                content,
                execution_preferences: Some(preferences.clone()),
                mentions,
            })
            .send()
            .await?,
        "send interactive message",
    )
    .await
}

async fn steer_interactive_message(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    conversation: &Conversation,
    turn: &mcg_domain::ConversationTurn,
    line: SubmittedLine,
) -> Result<ConversationView, Box<dyn std::error::Error>> {
    decode_api_response(
        client
            .post(format!(
                "{host}/api/v1/conversations/{}/turns/{}/steer",
                conversation.id, turn.id
            ))
            .bearer_auth(token)
            .json(&SteerConversationTurnRequest {
                content: line.content,
                mentions: line.mentions,
            })
            .send()
            .await?,
        "steer active turn",
    )
    .await
}

fn copy_to_clipboard(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    arboard::Clipboard::new()
        .and_then(|mut clipboard| clipboard.set_text(value.to_owned()))
        .is_ok()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InteractiveControl {
    Continue,
    Exit,
}

async fn create_interactive_conversation(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    project_id: Option<ProjectId>,
    preferences: &ExecutionPreferences,
    title: &str,
) -> Result<Conversation, Box<dyn std::error::Error>> {
    decode_api_response(
        client
            .post(format!("{host}/api/v1/conversations"))
            .bearer_auth(token)
            .json(&CreateConversationRequest {
                title: Some(title.to_owned()),
                project_id,
                execution_preferences: preferences.clone(),
                interaction_kind: ConversationInteractionKind::Interactive,
            })
            .send()
            .await?,
        "create interactive conversation",
    )
    .await
}

async fn interactive_plugin_candidates(
    client: &reqwest::Client,
    host: &str,
    token: &str,
) -> Result<Vec<MentionCandidate>, Box<dyn std::error::Error>> {
    let catalog: PluginListResponse = decode_api_response(
        client
            .get(format!("{host}/api/v1/plugins"))
            .bearer_auth(token)
            .send()
            .await?,
        "list plugins for interactive mentions",
    )
    .await?;
    Ok(catalog
        .plugins
        .into_iter()
        .filter(|plugin| plugin.installation.enabled)
        .map(|plugin| MentionCandidate {
            kind: MentionKind::Plugin,
            label: plugin.active_revision.manifest.display_name,
            resource: plugin.installation.plugin_id,
            detail: Some("Enabled plugin".into()),
            plugin_revision_id: Some(plugin.active_revision.id),
            installation_revision: Some(plugin.installation.revision),
        })
        .collect())
}

async fn interactive_palette_candidates(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    conversation_id: ConversationId,
    project_id: Option<ProjectId>,
    project_root: Option<&Path>,
    preferences: &ExecutionPreferences,
    plugins: &[MentionCandidate],
    request: PaletteRequest,
) -> Vec<PaletteCandidate> {
    let needle = request.query.to_ascii_lowercase();
    if request.kind == PaletteKind::Slash {
        let context = interactive_command_context(
            client,
            host,
            token,
            conversation_id,
            project_id,
            preferences,
        )
        .await;
        return SLASH_COMMANDS
            .iter()
            .filter(|command| command.is_available(context) && command.name.starts_with(&needle))
            .map(|command| {
                PaletteCandidate::Slash(SlashCandidate {
                    name: command.name,
                    description: command.description,
                })
            })
            .collect();
    }
    if request.kind == PaletteKind::CommandOption {
        return interactive_command_options(
            client,
            host,
            token,
            conversation_id,
            project_root,
            &request,
        )
        .await
        .into_iter()
        .map(|(value, label, detail)| PaletteCandidate::Choice {
            value,
            label,
            detail,
        })
        .collect();
    }
    if request.kind == PaletteKind::Dollar {
        let discovery = client
            .post(format!(
                "{host}/api/v1/conversations/{conversation_id}/codex/discovery"
            ))
            .bearer_auth(token)
            .json(&serde_json::json!({ "cwds": [], "force_reload": false }))
            .send()
            .await;
        let Ok(response) = discovery else {
            return Vec::new();
        };
        let Ok(response) = response.error_for_status() else {
            return Vec::new();
        };
        let Ok(discovery) = response.json::<serde_json::Value>().await else {
            return Vec::new();
        };
        let skills = discovery
            .get("skills")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|skill| {
                let name = skill
                    .get("name")
                    .or_else(|| skill.get("displayName"))
                    .and_then(serde_json::Value::as_str)?;
                (needle.is_empty() || name.to_ascii_lowercase().contains(&needle)).then(|| {
                    PaletteCandidate::Reference {
                        value: name.to_owned(),
                        label: name.to_owned(),
                        detail: skill
                            .get("description")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("Enabled skill")
                            .to_owned(),
                    }
                })
            });
        let apps = discovery
            .get("apps")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter(|app| {
                app.get("isEnabled").and_then(serde_json::Value::as_bool) != Some(false)
                    && app.get("isAccessible").and_then(serde_json::Value::as_bool) != Some(false)
            })
            .filter_map(|app| {
                let id = app.get("id").and_then(serde_json::Value::as_str)?;
                let name = app
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(id);
                (needle.is_empty()
                    || id.to_ascii_lowercase().contains(&needle)
                    || name.to_ascii_lowercase().contains(&needle))
                .then(|| PaletteCandidate::Reference {
                    value: id.to_owned(),
                    label: name.to_owned(),
                    detail: app
                        .get("description")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("Enabled app")
                        .to_owned(),
                })
            });
        return apps.chain(skills).take(100).collect();
    }
    let mut candidates = if let Some(project_id) = project_id {
        match client
            .get(format!("{host}/api/v1/projects/{project_id}/mentions"))
            .query(&[("q", request.query.as_str()), ("limit", "100")])
            .bearer_auth(token)
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => response
                .json::<MentionCandidateListResponse>()
                .await
                .map_or_else(|_| Vec::new(), |response| response.candidates),
            _ => Vec::new(),
        }
    } else {
        Vec::new()
    };
    candidates.extend(
        plugins
            .iter()
            .filter(|candidate| {
                needle.is_empty()
                    || candidate.label.to_ascii_lowercase().contains(&needle)
                    || candidate.resource.to_ascii_lowercase().contains(&needle)
            })
            .cloned(),
    );
    candidates
        .into_iter()
        .take(100)
        .map(PaletteCandidate::Mention)
        .collect()
}

async fn interactive_command_options(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    conversation_id: ConversationId,
    project_root: Option<&Path>,
    request: &PaletteRequest,
) -> Vec<(String, String, String)> {
    let command = request.command.as_deref().unwrap_or_default();
    let mut values = match command {
        "permissions" => vec![
            (
                "ask".into(),
                "Ask for approval".into(),
                "Ask before external edits and network use".into(),
            ),
            (
                "auto-review".into(),
                "Approve for me".into(),
                "Ask only for actions detected as unsafe".into(),
            ),
            (
                "read-only".into(),
                "Read only".into(),
                "Inspect without file changes or unrestricted commands".into(),
            ),
            (
                "full-access".into(),
                "Full access".into(),
                "Unrestricted project and internet authority".into(),
            ),
        ],
        "personality" => vec![
            (
                "friendly".into(),
                "Friendly".into(),
                "Warm and collaborative".into(),
            ),
            (
                "pragmatic".into(),
                "Pragmatic".into(),
                "Direct and implementation-focused".into(),
            ),
            (
                "none".into(),
                "None".into(),
                "No personality override".into(),
            ),
        ],
        "theme" => vec![
            (
                "system".into(),
                "System".into(),
                "Follow terminal colors".into(),
            ),
            (
                "dark".into(),
                "Dark".into(),
                "High-contrast dark palette".into(),
            ),
            (
                "light".into(),
                "Light".into(),
                "High-contrast light palette".into(),
            ),
        ],
        "keymap" => vec![
            (
                "default".into(),
                "Default".into(),
                "Codex-style terminal keys".into(),
            ),
            ("vim".into(), "Vim".into(), "Modal composer editing".into()),
        ],
        "statusline" => vec![
            (
                "project,branch,model,effort,permissions,goal,usage".into(),
                "Standard".into(),
                "Project, execution, goal, and usage".into(),
            ),
            (
                "project,branch,model".into(),
                "Compact".into(),
                "Project, branch, and model".into(),
            ),
        ],
        "title" => vec![
            (
                "project,conversation".into(),
                "Project and conversation".into(),
                "Descriptive terminal title".into(),
            ),
            (
                "conversation".into(),
                "Conversation only".into(),
                "Short terminal title".into(),
            ),
        ],
        "pets" => vec![
            ("none".into(), "None".into(), "Hide the terminal pet".into()),
            ("ferris".into(), "Ferris".into(), "A small Rust crab".into()),
            ("owl".into(), "Owl".into(), "A quiet MetaCog owl".into()),
        ],
        "agent" | "subagents" => {
            let mut sessions = Vec::new();
            if let Ok(response) = client
                .get(format!("{host}/api/v1/conversations/{conversation_id}"))
                .bearer_auth(token)
                .send()
                .await
                .and_then(reqwest::Response::error_for_status)
            {
                if let Ok(view) = response.json::<ConversationView>().await {
                    sessions.extend(view.team.sessions.into_iter().map(|session| {
                        (
                            session.id.to_string(),
                            session.display_name,
                            format!("{:?} · {:?}", session.kind, session.state),
                        )
                    }));
                }
            }
            sessions
        }
        "model" => {
            let mut models = vec![(
                "model=automatic".into(),
                "Model · Automatic".into(),
                "Use the catalog default".into(),
            )];
            if let Ok(response) = client
                .get(format!(
                    "{host}/api/v1/conversations/{conversation_id}/codex/models"
                ))
                .bearer_auth(token)
                .send()
                .await
                .and_then(reqwest::Response::error_for_status)
            {
                if let Ok(catalog) = response.json::<serde_json::Value>().await {
                    if let Some(catalog) = catalog.as_array() {
                        models.extend(catalog.iter().filter_map(|model| {
                            if model.get("hidden").and_then(serde_json::Value::as_bool)
                                == Some(true)
                            {
                                return None;
                            }
                            let id = model.get("id")?.as_str()?.to_owned();
                            let label = model
                                .get("displayName")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or(&id)
                                .to_owned();
                            let detail = model
                                .get("description")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("Codex model")
                                .to_owned();
                            Some((format!("model={id}"), format!("Model · {label}"), detail))
                        }));
                    }
                }
            }
            models.extend([
                (
                    "effort=low".into(),
                    "Effort · Low".into(),
                    "Use less reasoning".into(),
                ),
                (
                    "effort=medium".into(),
                    "Effort · Medium".into(),
                    "Balanced reasoning".into(),
                ),
                (
                    "effort=high".into(),
                    "Effort · High".into(),
                    "Use more reasoning".into(),
                ),
                (
                    "effort=xhigh".into(),
                    "Effort · Extra high".into(),
                    "Use substantially more reasoning".into(),
                ),
                (
                    "speed=standard".into(),
                    "Speed · Standard".into(),
                    "Default service tier".into(),
                ),
                (
                    "speed=fast".into(),
                    "Speed · Fast".into(),
                    "Faster responses, more usage".into(),
                ),
            ]);
            models
        }
        "resume" => {
            let mut conversations = Vec::new();
            if let Ok(response) = client
                .get(format!("{host}/api/v1/conversations?include_archived=true"))
                .bearer_auth(token)
                .send()
                .await
                .and_then(reqwest::Response::error_for_status)
            {
                if let Ok(list) = response.json::<ConversationListResponse>().await {
                    conversations.extend(list.conversations.into_iter().map(|conversation| {
                        (
                            conversation.id.to_string(),
                            conversation.title,
                            format!("{:?} · {}", conversation.state, conversation.updated_at),
                        )
                    }));
                }
            }
            conversations
        }
        "fork" => {
            let mut turns = Vec::new();
            if let Ok(response) = client
                .get(format!("{host}/api/v1/conversations/{conversation_id}"))
                .bearer_auth(token)
                .send()
                .await
                .and_then(reqwest::Response::error_for_status)
            {
                if let Ok(view) = response.json::<ConversationView>().await {
                    for turn in view.turns {
                        let label = view
                            .messages
                            .iter()
                            .find(|message| {
                                message.turn_id == turn.id && message.role == ConversationRole::User
                            })
                            .map(|message| {
                                message
                                    .content
                                    .replace(['\r', '\n'], " ")
                                    .chars()
                                    .take(72)
                                    .collect()
                            })
                            .unwrap_or_else(|| "User request".into());
                        turns.push((turn.id.to_string(), label, turn.created_at.to_rfc3339()));
                    }
                }
            }
            turns.reverse();
            turns
        }
        "stop" => {
            let mut commands = Vec::new();
            if let Ok(response) = client
                .get(format!(
                    "{host}/api/v1/conversations/{conversation_id}/codex/commands"
                ))
                .bearer_auth(token)
                .send()
                .await
                .and_then(reqwest::Response::error_for_status)
            {
                if let Ok(items) = response
                    .json::<Vec<mcg_runtime::ConversationCommandExecution>>()
                    .await
                {
                    commands.extend(items.into_iter().filter_map(|execution| {
                        (execution.state == mcg_runtime::ConversationCommandState::Running).then(
                            || {
                                (
                                    execution.id,
                                    execution.command.join(" "),
                                    format!("Started {}", execution.started_at),
                                )
                            },
                        )
                    }));
                }
            }
            commands
        }
        "apps" => {
            let mut apps = Vec::new();
            if let Ok(response) = client
                .post(format!(
                    "{host}/api/v1/conversations/{conversation_id}/codex/discovery"
                ))
                .bearer_auth(token)
                .json(&serde_json::json!({ "cwds": [], "force_reload": false }))
                .send()
                .await
                .and_then(reqwest::Response::error_for_status)
            {
                if let Ok(discovery) = response.json::<serde_json::Value>().await {
                    apps.extend(
                        discovery
                            .get("apps")
                            .and_then(serde_json::Value::as_array)
                            .into_iter()
                            .flatten()
                            .filter(|app| {
                                app.get("isEnabled").and_then(serde_json::Value::as_bool)
                                    != Some(false)
                                    && app.get("isAccessible").and_then(serde_json::Value::as_bool)
                                        != Some(false)
                            })
                            .filter_map(|app| {
                                let id = app.get("id")?.as_str()?.to_owned();
                                let name = app
                                    .get("name")
                                    .and_then(serde_json::Value::as_str)
                                    .unwrap_or(&id)
                                    .to_owned();
                                let detail = app
                                    .get("description")
                                    .and_then(serde_json::Value::as_str)
                                    .unwrap_or("Enabled app")
                                    .to_owned();
                                Some((id, name, detail))
                            }),
                    );
                }
            }
            apps
        }
        "hooks" => {
            let cwd = project_root
                .map(|path| path.display().to_string())
                .or_else(|| {
                    std::env::current_dir()
                        .ok()
                        .map(|path| path.display().to_string())
                });
            let cwds = cwd.into_iter().collect::<Vec<_>>();
            let mut hooks = Vec::new();
            if let Ok(response) = client
                .post(format!(
                    "{host}/api/v1/conversations/{conversation_id}/codex/discovery"
                ))
                .bearer_auth(token)
                .json(&serde_json::json!({ "cwds": cwds, "force_reload": false }))
                .send()
                .await
                .and_then(reqwest::Response::error_for_status)
                && let Ok(discovery) = response.json::<CodexStableDiscovery>().await
            {
                for entry in discovery.hooks {
                    for hook in &entry.hooks {
                        let (action, action_label) = if hook.is_managed {
                            ("inspect", "Inspect managed")
                        } else {
                            match (hook.trust_status.as_str(), hook.enabled) {
                                ("trusted", true) => ("disable", "Disable"),
                                ("trusted", false) => ("enable", "Enable"),
                                _ => ("trust", "Review and trust"),
                            }
                        };
                        let token = encode_hook_selection(&entry, hook);
                        let matcher = hook.matcher.as_deref().unwrap_or("all matching events");
                        hooks.push((
                            format!("{action} {token}"),
                            format!("{action_label} · {}", hook.event_name),
                            format!(
                                "{} · {} · {} · {matcher}",
                                hook.source,
                                hook.trust_status,
                                hook_handler_summary(hook)
                            ),
                        ));
                    }
                }
            }
            hooks
        }
        _ => Vec::new(),
    };
    if command == "permissions" {
        let project_id = fetch_conversation(client, host, token, &conversation_id.to_string())
            .await
            .ok()
            .and_then(|view| view.conversation.project_id);
        if let Some(project_id) = project_id
            && let Ok(response) = client
                .get(format!(
                    "{host}/api/v1/conversations/{conversation_id}/codex/permission-profiles"
                ))
                .query(&[("project_id", project_id.to_string())])
                .bearer_auth(token)
                .send()
                .await
                .and_then(reqwest::Response::error_for_status)
            && let Ok(profiles) = response
                .json::<Vec<mcg_codex_app_server::CodexPermissionProfile>>()
                .await
        {
            values.extend(
                profiles
                    .into_iter()
                    .filter(|profile| profile.allowed)
                    .map(|profile| {
                        let id = profile.id;
                        (
                            format!("profile:{id}"),
                            id,
                            profile
                                .description
                                .unwrap_or_else(|| "Codex permission profile".into()),
                        )
                    }),
            );
        }
    }
    let needle = request.query.to_ascii_lowercase();
    values.retain(|(value, label, _)| {
        needle.is_empty()
            || value.to_ascii_lowercase().contains(&needle)
            || label.to_ascii_lowercase().contains(&needle)
    });
    values
}

async fn interactive_command_context(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    conversation_id: ConversationId,
    project_id: Option<ProjectId>,
    preferences: &ExecutionPreferences,
) -> CommandContext {
    let models = client
        .get(format!(
            "{host}/api/v1/conversations/{conversation_id}/codex/models"
        ))
        .bearer_auth(token)
        .send()
        .await
        .ok()
        .and_then(|response| response.error_for_status().ok());
    let fast_model = if let Some(response) = models {
        response
            .json::<serde_json::Value>()
            .await
            .ok()
            .and_then(|value| value.as_array().cloned())
            .is_some_and(|models| {
                models.iter().any(|model| {
                    let selected = preferences.model_id.as_deref().map_or_else(
                        || {
                            model.get("isDefault").and_then(serde_json::Value::as_bool)
                                == Some(true)
                        },
                        |selected| {
                            model.get("id").and_then(serde_json::Value::as_str) == Some(selected)
                                || model.get("model").and_then(serde_json::Value::as_str)
                                    == Some(selected)
                        },
                    );
                    selected
                        && model
                            .get("serviceTiers")
                            .and_then(serde_json::Value::as_array)
                            .is_some_and(|tiers| {
                                tiers.iter().any(|tier| {
                                    tier.get("name")
                                        .or_else(|| tier.get("serviceTier"))
                                        .and_then(serde_json::Value::as_str)
                                        .is_some_and(|value| value.eq_ignore_ascii_case("fast"))
                                })
                            })
                })
            })
    } else {
        false
    };
    let apps = match client
        .post(format!(
            "{host}/api/v1/conversations/{conversation_id}/codex/discovery"
        ))
        .bearer_auth(token)
        .json(&serde_json::json!({ "cwds": [], "force_reload": false }))
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
    {
        Ok(response) => response
            .json::<serde_json::Value>()
            .await
            .ok()
            .and_then(|discovery| {
                discovery
                    .get("appsAvailable")
                    .and_then(serde_json::Value::as_bool)
            })
            .unwrap_or(false),
        Err(_) => false,
    };
    let windows_sandbox_setup = if cfg!(windows) {
        let configuration = client
            .get(format!(
                "{host}/api/v1/conversations/{conversation_id}/codex/configuration"
            ))
            .bearer_auth(token)
            .send()
            .await
            .ok()
            .and_then(|response| response.error_for_status().ok());
        if let Some(response) = configuration {
            response
                .json::<serde_json::Value>()
                .await
                .ok()
                .and_then(|configuration| {
                    configuration
                        .pointer("/windows_sandbox/status")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned)
                })
                .is_some_and(|status| status != "ready")
        } else {
            false
        }
    } else {
        false
    };
    CommandContext {
        has_project: project_id.is_some(),
        fast_model,
        // An IDE-flavoured terminal does not prove that the app-server exposed
        // an IDE context attachment contract. Keep /ide hidden until capability
        // discovery returns a handler-compatible endpoint.
        ide: false,
        apps,
        windows: cfg!(windows),
        windows_sandbox_setup,
        experimental: false,
    }
}

async fn send_interactive_message(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    conversation: &Conversation,
    preferences: &ExecutionPreferences,
    line: SubmittedLine,
    pending_mentions: &[String],
) -> Result<ConversationView, Box<dyn std::error::Error>> {
    let (content, appended_mentions) =
        resolve_cli_mentions(client, host, token, line.content, pending_mentions).await?;
    let mut mentions = line.mentions;
    mentions.extend(appended_mentions);
    mentions.sort_by_key(|mention| mention.byte_start);
    let mut view: ConversationView = decode_api_response(
        client
            .post(format!(
                "{host}/api/v1/conversations/{}/messages",
                conversation.id
            ))
            .bearer_auth(token)
            .json(&SendConversationMessageRequest {
                content,
                execution_preferences: Some(preferences.clone()),
                mentions,
            })
            .send()
            .await?,
        "send interactive message",
    )
    .await?;
    let turn_id = view
        .messages
        .iter()
        .rev()
        .find(|message| message.role == ConversationRole::User)
        .map(|message| message.turn_id)
        .ok_or("interactive response did not contain the submitted user turn")?;
    view = wait_for_interactive_conversation(
        client,
        host,
        token,
        &conversation.id.to_string(),
        turn_id,
        3_600,
        1,
    )
    .await?;
    if let Some(message) =
        view.messages.iter().rev().find(|message| {
            message.turn_id == turn_id && message.role == ConversationRole::Assistant
        })
    {
        println!("\n{}\n", message.content);
    }
    Ok(view)
}

async fn wait_for_interactive_conversation(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    id: &str,
    origin_turn_id: ConversationTurnId,
    timeout_seconds: u64,
    poll_seconds: u64,
) -> Result<ConversationView, Box<dyn std::error::Error>> {
    let deadline = tokio::time::Instant::now()
        + std::time::Duration::from_secs(timeout_seconds.clamp(1, 86_400));
    let interval = std::time::Duration::from_secs(poll_seconds.clamp(1, 60));
    loop {
        let mut view = fetch_conversation(client, host, token, id).await?;
        let requests = view.conversation.pending_input_requests.clone();
        for request in requests {
            let mut answers = serde_json::Map::new();
            for question in &request.questions {
                let answer = prompt_for_question(question)?;
                answers.insert(
                    question.id.clone(),
                    serde_json::json!({ "answers": [answer] }),
                );
            }
            view = decode_api_response(
                client
                    .post(format!("{host}/api/v1/conversations/{id}/input"))
                    .bearer_auth(token)
                    .json(&AnswerConversationInputRequest {
                        request_id: request.request_id,
                        answers: serde_json::Value::Object(answers),
                    })
                    .send()
                    .await?,
                "answer planning questions",
            )
            .await?;
        }
        if conversation_settled(&view, origin_turn_id) {
            return Ok(view);
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(format!(
                "conversation {id} did not settle within {timeout_seconds} seconds; durable work remains inspectable with `mcg conversation team {id}`"
            )
            .into());
        }
        tokio::time::sleep(interval).await;
    }
}

fn parse_interactive_slash(line: &SubmittedLine) -> Option<(&str, usize, &str)> {
    let trimmed_start = line.content.trim_start();
    if !trimmed_start.starts_with('/') {
        return None;
    }
    let leading = line.content.len() - trimmed_start.len();
    let command_end = trimmed_start
        .find(char::is_whitespace)
        .unwrap_or(trimmed_start.len());
    let name = &trimmed_start[1..command_end];
    let remainder = &trimmed_start[command_end..];
    let argument = remainder.trim_start();
    let argument_start = leading + command_end + (remainder.len() - argument.len());
    Some((name, argument_start, argument))
}

fn slash_argument_line(line: SubmittedLine, argument_start: usize) -> SubmittedLine {
    let content = line.content[argument_start..].to_owned();
    let mentions = line
        .mentions
        .into_iter()
        .filter_map(|mut mention| {
            let start = usize::try_from(mention.byte_start).ok()?;
            if start < argument_start {
                return None;
            }
            let offset = u32::try_from(argument_start).ok()?;
            mention.byte_start -= offset;
            mention.byte_end -= offset;
            Some(mention)
        })
        .collect();
    SubmittedLine { content, mentions }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
async fn run_interactive_slash(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    conversation: &mut Conversation,
    project_id: &mut Option<ProjectId>,
    project_root: &mut Option<PathBuf>,
    preferences: &mut ExecutionPreferences,
    pending_mentions: &mut Vec<String>,
    line: SubmittedLine,
    command_output: &mut Vec<String>,
) -> Result<InteractiveControl, Box<dyn std::error::Error>> {
    macro_rules! println {
        ($($argument:tt)*) => {
            command_output.push(format!($($argument)*))
        };
    }
    let (name, argument_start, argument) =
        parse_interactive_slash(&line).ok_or("invalid slash command")?;
    let requested_name = name.to_ascii_lowercase();
    let command_context = interactive_command_context(
        client,
        host,
        token,
        conversation.id,
        *project_id,
        preferences,
    )
    .await;
    let command = command_named(&requested_name, command_context).ok_or_else(|| {
        format!("unknown or unavailable command /{requested_name}; type /help to list commands")
    })?;
    let (name, argument_owned) = if command.name == "mcg" {
        let (subcommand, remainder) = argument
            .split_once(char::is_whitespace)
            .map_or((argument, ""), |(name, value)| (name, value.trim_start()));
        if ![
            "project",
            "task",
            "worktree",
            "local",
            "reasoning",
            "cloud",
            "cloud-environment",
            "ide-context",
        ]
        .contains(&subcommand)
        {
            return Err("use /mcg project|task|worktree|local|reasoning|cloud|cloud-environment|ide-context".into());
        }
        (subcommand.to_owned(), remainder.to_owned())
    } else {
        (command.name.to_owned(), argument.to_owned())
    };
    let argument = argument_owned.as_str();
    match name.as_str() {
        "exit" | "quit" => return Ok(InteractiveControl::Exit),
        "help" => {
            println!(
                "\nKeyboard: type / or @ to open a live picker; ↑/↓ select; Enter/Tab inserts; Escape closes."
            );
            println!(
                "Left/Right/Home/End edit, Up/Down recall history when no picker is open, Ctrl+C exits.\n"
            );
            for command in SLASH_COMMANDS
                .iter()
                .filter(|command| command.is_available(command_context))
            {
                println!("/{:<18} {}", command.name, command.description);
            }
        }
        "clear" => {
            let title = if argument.trim().is_empty() {
                "MetaCog CLI"
            } else {
                argument.trim()
            };
            *conversation = create_interactive_conversation(
                client,
                host,
                token,
                *project_id,
                preferences,
                title,
            )
            .await?;
            pending_mentions.clear();
        }
        "rename" => {
            if argument.trim().is_empty() {
                return Err("add the new conversation title after /rename".into());
            }
            *conversation = decode_api_response(
                client
                    .patch(format!("{host}/api/v1/conversations/{}", conversation.id))
                    .bearer_auth(token)
                    .json(&UpdateConversationRequest {
                        title: argument.trim().to_owned(),
                        project_id: *project_id,
                    })
                    .send()
                    .await?,
                "rename conversation",
            )
            .await?;
        }
        "resume" => {
            let requested = argument
                .trim()
                .parse::<ConversationId>()
                .map_err(|_| "choose a conversation from the /resume picker")?;
            let resumed = fetch_conversation(client, host, token, &requested.to_string()).await?;
            *conversation = resumed.conversation;
            *project_id = conversation.project_id;
            *project_root = if let Some(resumed_project_id) = *project_id {
                let projects: ProjectListResponse = decode_api_response(
                    client
                        .get(format!("{host}/api/v1/projects"))
                        .bearer_auth(token)
                        .send()
                        .await?,
                    "resolve resumed project",
                )
                .await?;
                projects
                    .projects
                    .into_iter()
                    .find(|project| project.project_id == resumed_project_id)
                    .map(|project| PathBuf::from(project.root))
            } else {
                None
            };
            pending_mentions.clear();
        }
        "archive" => {
            if !argument.eq_ignore_ascii_case("confirm") {
                println!(
                    "Archive '{}' ({}) and close this CLI session? Re-run `/archive confirm` to continue.",
                    conversation.title, conversation.id
                );
                return Ok(InteractiveControl::Continue);
            }
            let archived: Conversation = decode_api_response(
                client
                    .post(format!(
                        "{host}/api/v1/conversations/{}/archive",
                        conversation.id
                    ))
                    .bearer_auth(token)
                    .json(&ArchiveConversationRequest { archived: true })
                    .send()
                    .await?,
                "archive conversation",
            )
            .await?;
            *conversation = archived;
            println!("Archived conversation {}.", conversation.id);
            return Ok(InteractiveControl::Exit);
        }
        "new" => {
            *conversation = create_interactive_conversation(
                client,
                host,
                token,
                *project_id,
                preferences,
                "MetaCog CLI",
            )
            .await?;
            pending_mentions.clear();
            println!("Started conversation {}.", conversation.id);
            if !argument.is_empty() {
                let view = send_interactive_message(
                    client,
                    host,
                    token,
                    conversation,
                    preferences,
                    slash_argument_line(line, argument_start),
                    pending_mentions,
                )
                .await?;
                *conversation = view.conversation;
            }
        }
        "side" => {
            let view: ConversationView = decode_api_response(
                client
                    .post(format!(
                        "{host}/api/v1/conversations/{}/side",
                        conversation.id
                    ))
                    .bearer_auth(token)
                    .json(&ForkConversationRequest {
                        origin_turn_id: None,
                        execution_preferences: Some(preferences.clone()),
                    })
                    .send()
                    .await?,
                "create side conversation",
            )
            .await?;
            *conversation = view.conversation;
            pending_mentions.clear();
            println!("Started ephemeral side conversation {}.", conversation.id);
            if !argument.is_empty() {
                let view = send_interactive_message(
                    client,
                    host,
                    token,
                    conversation,
                    preferences,
                    slash_argument_line(line, argument_start),
                    pending_mentions,
                )
                .await?;
                *conversation = view.conversation;
            }
        }
        "plan" => {
            let view: ConversationView = decode_api_response(
                client
                    .post(format!(
                        "{host}/api/v1/conversations/{}/mode",
                        conversation.id
                    ))
                    .bearer_auth(token)
                    .json(&UpdateConversationModeRequest {
                        mode: ConversationMode::Plan,
                    })
                    .send()
                    .await?,
                "enter plan mode",
            )
            .await?;
            *conversation = view.conversation;
            println!("Plan mode enabled.");
            if !argument.is_empty() {
                let view = send_interactive_message(
                    client,
                    host,
                    token,
                    conversation,
                    preferences,
                    slash_argument_line(line, argument_start),
                    pending_mentions,
                )
                .await?;
                *conversation = view.conversation;
                pending_mentions.clear();
            }
        }
        "goal" => {
            let (objective, status, clear) = if argument.is_empty() {
                let view =
                    fetch_conversation(client, host, token, &conversation.id.to_string()).await?;
                if let Some(goal) = &view.conversation.goal {
                    println!(
                        "Goal: {}\nStatus: {:?}\nTokens: {}",
                        goal.objective, goal.status, goal.tokens_used
                    );
                } else {
                    println!("No active goal. Use /goal <objective>.");
                }
                *conversation = view.conversation;
                return Ok(InteractiveControl::Continue);
            } else if argument.eq_ignore_ascii_case("clear") {
                (None, None, true)
            } else if argument.eq_ignore_ascii_case("pause") {
                (None, Some(ConversationGoalStatus::Paused), false)
            } else if argument.eq_ignore_ascii_case("resume") {
                (None, Some(ConversationGoalStatus::Active), false)
            } else if let Some(objective) = argument.strip_prefix("edit ") {
                (
                    Some(validated_goal(objective)?),
                    Some(ConversationGoalStatus::Active),
                    false,
                )
            } else {
                (
                    Some(validated_goal(argument)?),
                    Some(ConversationGoalStatus::Active),
                    false,
                )
            };
            let response = if clear {
                client
                    .delete(format!(
                        "{host}/api/v1/conversations/{}/goal",
                        conversation.id
                    ))
                    .bearer_auth(token)
                    .send()
                    .await?
            } else {
                client
                    .post(format!(
                        "{host}/api/v1/conversations/{}/goal",
                        conversation.id
                    ))
                    .bearer_auth(token)
                    .json(&UpdateConversationGoalRequest { objective, status })
                    .send()
                    .await?
            };
            let view: ConversationView = decode_api_response(response, "update goal").await?;
            *conversation = view.conversation;
            println!("Goal {}.", if clear { "cleared" } else { "updated" });
        }
        "mention" => {
            if !line.mentions.is_empty() {
                for mention in &line.mentions {
                    pending_mentions.push(format!(
                        "{}:{}",
                        match mention.kind {
                            MentionKind::File => "file",
                            MentionKind::Folder => "folder",
                            MentionKind::Plugin => "plugin",
                        },
                        mention.resource
                    ));
                    println!("Attached {} to the next message.", mention.label);
                }
            } else if argument.is_empty() {
                println!("Type @ to search the project, or use /mention <path>.");
            } else {
                let path = PathBuf::from(argument);
                let resolved_path = if path.is_absolute() {
                    path
                } else if let Some(root) = project_root.as_ref() {
                    root.join(path)
                } else {
                    path
                };
                let metadata = std::fs::metadata(&resolved_path).map_err(|error| {
                    format!("cannot attach {}: {error}", resolved_path.display())
                })?;
                let resource = if let Some(root) = project_root.as_ref() {
                    resolved_path.strip_prefix(root).map_or_else(
                        |_| resolved_path.to_string_lossy().into_owned(),
                        |relative| relative.to_string_lossy().replace('\\', "/"),
                    )
                } else {
                    resolved_path.to_string_lossy().into_owned()
                };
                pending_mentions.push(format!(
                    "{}:{resource}",
                    if metadata.is_dir() { "folder" } else { "file" }
                ));
                println!("Attached {resource} to the next message.");
            }
        }
        "model" => {
            if argument.is_empty() {
                println!(
                    "Current model: {}",
                    preferences.model_id.as_deref().unwrap_or("Automatic")
                );
            } else {
                if let Some(model) = argument.strip_prefix("model=") {
                    preferences.model_id = (model != "automatic").then(|| model.to_owned());
                    println!("Model set to {model} for later turns.");
                } else if let Some(effort) = argument.strip_prefix("effort=") {
                    preferences.reasoning_effort = Some(effort.to_owned());
                    println!("Reasoning effort set to {effort}.");
                } else if let Some(speed) = argument.strip_prefix("speed=") {
                    preferences.service_tier = (speed != "standard").then(|| speed.to_owned());
                    println!("Speed set to {speed}.");
                } else {
                    preferences.model_id = Some(argument.to_owned());
                    println!("Model set to {argument} for later turns.");
                }
            }
        }
        "reasoning" => {
            if argument.is_empty() {
                println!(
                    "Current reasoning effort: {}",
                    preferences
                        .reasoning_effort
                        .as_deref()
                        .unwrap_or("Automatic")
                );
            } else {
                let effort = argument.to_ascii_lowercase().replace(' ', "");
                let effort = if effort == "extrahigh" {
                    "xhigh".to_owned()
                } else {
                    effort
                };
                if !["low", "medium", "high", "xhigh", "max", "ultra"].contains(&effort.as_str()) {
                    return Err(
                        "reasoning must be low, medium, high, extra high, max, or ultra".into(),
                    );
                }
                preferences.reasoning_effort = Some(effort.clone());
                println!("Reasoning effort set to {effort}.");
            }
        }
        "fast" => {
            let enabled = preferences.service_tier.is_some();
            preferences.service_tier = (!enabled).then(|| "fast".into());
            println!("Fast responses {}.", if enabled { "off" } else { "on" });
        }
        "permissions" => {
            if argument.is_empty() {
                if let Some(profile) = &preferences.permission_profile {
                    println!("Current permissions: Codex profile {profile}.");
                } else {
                    println!(
                        "Current permissions: {:?}. Choose ask, auto-review, read-only, full-access, or profile:<id>.",
                        preferences.approval_mode
                    );
                }
            } else {
                if let Some(profile) = argument.strip_prefix("profile:") {
                    preferences.permission_profile = Some(profile.to_owned());
                    preferences.approval_mode = ApprovalMode::Ask;
                    preferences.approval_policy = None;
                    preferences.sandbox_mode = None;
                    println!("Permissions set to Codex profile {profile}.");
                    return Ok(InteractiveControl::Continue);
                }
                preferences.approval_mode = match argument.to_ascii_lowercase().as_str() {
                    "ask" => ApprovalMode::Ask,
                    "auto" | "auto-review" | "approve-for-me" => ApprovalMode::AutoReview,
                    "read-only" | "readonly" => ApprovalMode::ReadOnly,
                    "full" | "full-access" => ApprovalMode::FullAccess,
                    _ => {
                        return Err(
                            "permissions must be ask, auto-review, read-only, full-access, or profile:<id>"
                                .into(),
                        );
                    }
                };
                preferences.approval_policy = None;
                preferences.sandbox_mode = None;
                preferences.permission_profile = None;
                println!("Permissions set to {:?}.", preferences.approval_mode);
            }
        }
        "setup-default-sandbox" => {
            println!("Windows may request elevation while Codex configures its default sandbox.");
            let result: mcg_codex_app_server::CodexWindowsSandboxSetupResult = decode_api_response(
                client
                    .post(format!("{host}/api/v1/codex/windows-sandbox/setup"))
                    .bearer_auth(token)
                    .json(&mcg_api::WindowsSandboxSetupRequest {
                        mode: mcg_codex_app_server::CodexWindowsSandboxSetupMode::Elevated,
                        project_id: *project_id,
                    })
                    .send()
                    .await?,
                "configure Windows sandbox",
            )
            .await?;
            println!(
                "Windows sandbox configured as {:?}; readiness {}.",
                result.mode, result.readiness
            );
        }
        "sandbox-add-read-dir" => {
            let project_id = (*project_id).ok_or(
                "/sandbox-add-read-dir requires a linked project so Codex can resolve its permission profile",
            )?;
            if argument.is_empty() {
                return Err("Usage: /sandbox-add-read-dir <absolute-directory-path>".into());
            }
            let path = PathBuf::from(argument);
            if !path.is_absolute() {
                return Err("sandbox read directory must be an absolute path".into());
            }
            let result: mcg_codex_app_server::CodexWindowsSandboxReadGrant = decode_api_response(
                client
                    .post(format!(
                        "{host}/api/v1/codex/windows-sandbox/read-directories"
                    ))
                    .bearer_auth(token)
                    .json(&mcg_api::WindowsSandboxReadDirectoryRequest {
                        project_id,
                        path: argument.to_owned(),
                        permission_profile: preferences.permission_profile.clone(),
                    })
                    .send()
                    .await?,
                "grant Windows sandbox read directory",
            )
            .await?;
            println!("Sandbox read access granted for {}.", result.path);
        }
        "personality" => {
            if argument.is_empty() {
                println!(
                    "Current personality: {}",
                    preferences.personality.as_deref().unwrap_or("none")
                );
            } else {
                let value = argument.to_ascii_lowercase();
                if !["friendly", "pragmatic", "none"].contains(&value.as_str()) {
                    return Err("personality must be friendly, pragmatic, or none".into());
                }
                preferences.personality = (value != "none").then_some(value.clone());
                println!("Personality set to {value}.");
            }
        }
        "status" => {
            let view =
                fetch_conversation(client, host, token, &conversation.id.to_string()).await?;
            println!("Conversation: {}", view.conversation.id);
            println!(
                "Project: {}",
                project_root
                    .as_ref()
                    .map_or_else(|| "None".into(), |root| root.display().to_string())
            );
            println!("Mode: {:?}", view.conversation.mode);
            println!(
                "Model: {}",
                preferences.model_id.as_deref().unwrap_or("Automatic")
            );
            println!(
                "Reasoning: {}",
                preferences
                    .reasoning_effort
                    .as_deref()
                    .unwrap_or("Automatic")
            );
            println!("Permissions: {:?}", preferences.approval_mode);
            println!(
                "Writable roots: {}",
                project_root.as_ref().map_or_else(
                    || "None (read-only)".into(),
                    |root| root.display().to_string()
                )
            );
            if let Ok(models) = client
                .get(format!(
                    "{host}/api/v1/conversations/{}/codex/models",
                    conversation.id
                ))
                .bearer_auth(token)
                .send()
                .await
                .and_then(reqwest::Response::error_for_status)
            {
                if let Ok(models) = models.json::<serde_json::Value>().await {
                    if let Some(selected) = models.as_array().and_then(|models| {
                        models.iter().find(|model| {
                            preferences.model_id.as_deref().map_or_else(
                                || {
                                    model.get("isDefault").and_then(serde_json::Value::as_bool)
                                        == Some(true)
                                },
                                |id| {
                                    model.get("id").and_then(serde_json::Value::as_str) == Some(id)
                                },
                            )
                        })
                    }) {
                        println!(
                            "Context capacity: {}",
                            selected
                                .get("contextWindow")
                                .or_else(|| selected.get("context_window"))
                                .map_or_else(|| "Unknown".into(), serde_json::Value::to_string)
                        );
                    }
                }
            }
            println!(
                "Goal: {}",
                view.conversation
                    .goal
                    .as_ref()
                    .map_or("None", |goal| goal.objective.as_str())
            );
            *conversation = view.conversation;
        }
        "usage" => {
            let limits: serde_json::Value = decode_api_response(
                client
                    .get(format!("{host}/api/v1/codex/rate-limits"))
                    .bearer_auth(token)
                    .send()
                    .await?,
                "read Codex usage limits",
            )
            .await?;
            let usage: serde_json::Value = decode_api_response(
                client
                    .get(format!("{host}/api/v1/codex/account/usage"))
                    .bearer_auth(token)
                    .send()
                    .await?,
                "read Codex account usage",
            )
            .await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "usage": usage,
                    "limits": limits,
                }))?
            );
        }
        "raw" => {
            let mut tui = interactive_tui_preferences(client, host, token).await?;
            tui.raw_mode = !tui.raw_mode;
            save_interactive_tui_preferences(client, host, token, &tui).await?;
            println!(
                "Raw event display {}.",
                if tui.raw_mode { "on" } else { "off" }
            );
        }
        "ps" => {
            let commands: Vec<mcg_runtime::ConversationCommandExecution> = decode_api_response(
                client
                    .get(format!(
                        "{host}/api/v1/conversations/{}/codex/commands",
                        conversation.id
                    ))
                    .bearer_auth(token)
                    .send()
                    .await?,
                "list conversation commands",
            )
            .await?;
            if commands.is_empty() {
                println!("No commands belong to this conversation.");
            }
            for command in commands {
                println!(
                    "{} · {:?} · {}",
                    command.id,
                    command.state,
                    command.command.join(" ")
                );
            }
        }
        "stop" => {
            let commands: Vec<mcg_runtime::ConversationCommandExecution> = decode_api_response(
                client
                    .get(format!(
                        "{host}/api/v1/conversations/{}/codex/commands",
                        conversation.id
                    ))
                    .bearer_auth(token)
                    .send()
                    .await?,
                "list conversation commands",
            )
            .await?;
            let running = commands
                .into_iter()
                .filter(|execution| {
                    execution.state == mcg_runtime::ConversationCommandState::Running
                })
                .collect::<Vec<_>>();
            if running.is_empty() {
                println!("No running commands belong to this conversation.");
            } else {
                for command in &running {
                    let execution: mcg_runtime::ConversationCommandExecution = decode_api_response(
                        client
                            .post(format!(
                                "{host}/api/v1/conversations/{}/codex/commands/{}/stop",
                                conversation.id, command.id
                            ))
                            .bearer_auth(token)
                            .send()
                            .await?,
                        "stop conversation command",
                    )
                    .await?;
                    println!("{} · {:?}", execution.id, execution.state);
                }
                println!("Stopped {} conversation command(s).", running.len());
            }
        }
        "copy" => {
            let view =
                fetch_conversation(client, host, token, &conversation.id.to_string()).await?;
            let text = view
                .messages
                .iter()
                .rev()
                .find(|message| message.role == ConversationRole::Assistant)
                .map(|message| message.content.as_str())
                .ok_or("there is no completed assistant output to copy")?;
            if !copy_to_clipboard(text) {
                return Err("clipboard is unavailable".into());
            }
            *conversation = view.conversation;
        }
        "diff" => {
            let root = project_root.as_ref().ok_or("/diff requires a project")?;
            let output = ProcessCommand::new("git")
                .args(["diff", "--no-ext-diff", "--"])
                .current_dir(root)
                .output()?;
            if !output.status.success() {
                return Err(String::from_utf8_lossy(&output.stderr)
                    .trim()
                    .to_owned()
                    .into());
            }
            println!("{}", String::from_utf8_lossy(&output.stdout));
        }
        "compact" => {
            let _: serde_json::Value = decode_api_response(
                client
                    .post(format!(
                        "{host}/api/v1/codex/owned-threads/{}/compact",
                        conversation.id
                    ))
                    .bearer_auth(token)
                    .send()
                    .await?,
                "compact",
            )
            .await?;
            println!("Context compact completed.");
        }
        "fork" => {
            let origin_turn_id = if argument.is_empty() {
                None
            } else {
                Some(
                    argument
                        .parse::<ConversationTurnId>()
                        .map_err(|_| "choose a user turn from the /fork picker")?,
                )
            };
            let view: ConversationView = decode_api_response(
                client
                    .post(format!(
                        "{host}/api/v1/conversations/{}/fork",
                        conversation.id
                    ))
                    .bearer_auth(token)
                    .json(&ForkConversationRequest {
                        origin_turn_id,
                        execution_preferences: Some(preferences.clone()),
                    })
                    .send()
                    .await?,
                "fork conversation",
            )
            .await?;
            *conversation = view.conversation;
            pending_mentions.clear();
            println!("Forked to conversation {}.", conversation.id);
        }
        "delete" => {
            let preview: ConversationDeletionPreview = decode_api_response(
                client
                    .get(format!(
                        "{host}/api/v1/conversations/{}/delete-preview",
                        conversation.id
                    ))
                    .bearer_auth(token)
                    .send()
                    .await?,
                "preview conversation deletion",
            )
            .await?;
            if argument != preview.confirmation {
                println!(
                    "Delete '{}' ({}) and {} child conversation(s)? Re-run `/delete {}` to confirm.",
                    preview.conversation.title,
                    preview.conversation.id,
                    preview.children.len(),
                    preview.confirmation
                );
            } else {
                let result: ConversationDeletionResult = decode_api_response(
                    client
                        .delete(format!("{host}/api/v1/conversations/{}", conversation.id))
                        .bearer_auth(token)
                        .json(&mcg_api::DeleteConversationRequest {
                            confirmation: argument.to_owned(),
                        })
                        .send()
                        .await?,
                    "delete conversation",
                )
                .await?;
                println!(
                    "Deleted {} conversation(s).",
                    result.deleted_conversation_ids.len()
                );
                *conversation = create_interactive_conversation(
                    client,
                    host,
                    token,
                    *project_id,
                    preferences,
                    "MetaCog CLI",
                )
                .await?;
                pending_mentions.clear();
            }
        }
        "logout" => {
            if !argument.eq_ignore_ascii_case("confirm") {
                println!(
                    "This disconnects the device-wide Codex account. Re-run `/logout confirm` to continue."
                );
            } else {
                let _: serde_json::Value = decode_api_response(
                    client
                        .post(format!("{host}/api/v1/codex/account/logout"))
                        .bearer_auth(token)
                        .send()
                        .await?,
                    "logout Codex account",
                )
                .await?;
                println!("Codex account disconnected.");
            }
        }
        "approve" => {
            if !argument.eq_ignore_ascii_case("confirm") {
                return Err("review the exact pending action, then run /approve confirm".into());
            }
            let view =
                fetch_conversation(client, host, token, &conversation.id.to_string()).await?;
            let task_ids = view
                .turns
                .iter()
                .filter_map(|turn| turn.task_id)
                .collect::<BTreeSet<_>>();
            let approvals: ApprovalListResponse = decode_api_response(
                client
                    .get(format!("{host}/api/v1/approvals?state=pending"))
                    .bearer_auth(token)
                    .send()
                    .await?,
                "list pending approvals",
            )
            .await?;
            let matching = approvals
                .approvals
                .iter()
                .rev()
                .find(|approval| task_ids.contains(&approval.task_id));
            if let Some(approval) = matching {
                resolve_approval(
                    client,
                    host,
                    token,
                    &approval.id.to_string(),
                    ApprovalState::Approved,
                    Some("Approved once from the interactive CLI".into()),
                )
                .await?;
            } else {
                println!("No pending approval belongs to this conversation.");
            }
            *conversation = view.conversation;
        }
        "project" => {
            if argument.is_empty() {
                println!(
                    "Current project: {}",
                    project_root
                        .as_ref()
                        .map_or_else(|| "None".into(), |root| root.display().to_string())
                );
            } else {
                let canonical = std::fs::canonicalize(argument)?;
                let root = canonical
                    .to_str()
                    .ok_or("project directory must be valid Unicode")?
                    .to_owned();
                let resolved = resolve_consumer_project(client, host, token, &root).await?;
                *project_id = Some(resolved.project.project_id);
                *project_root = Some(canonical);
                *conversation = create_interactive_conversation(
                    client,
                    host,
                    token,
                    *project_id,
                    preferences,
                    "MetaCog CLI",
                )
                .await?;
                pending_mentions.clear();
                println!("Switched to {root}; conversation {}.", conversation.id);
            }
        }
        "task" => {
            *project_id = None;
            *project_root = None;
            *conversation = create_interactive_conversation(
                client,
                host,
                token,
                None,
                preferences,
                "MetaCog CLI task",
            )
            .await?;
            pending_mentions.clear();
            println!("Started project-free conversation {}.", conversation.id);
        }
        "review" => {
            if project_id.is_none() {
                return Err("/review requires a project".into());
            }
            let target = if argument.is_empty() {
                serde_json::json!({ "type": "uncommittedChanges" })
            } else {
                serde_json::json!({ "type": "custom", "instructions": argument })
            };
            let result: serde_json::Value = decode_api_response(
                client
                    .post(format!(
                        "{host}/api/v1/codex/owned-threads/{}/review",
                        conversation.id
                    ))
                    .bearer_auth(token)
                    .json(&serde_json::json!({
                        "target": target,
                        "delivery": "inline",
                    }))
                    .send()
                    .await?,
                "start native read-only review",
            )
            .await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        "init" | "worktree" => {
            if project_id.is_none() {
                return Err(format!("/{name} requires a project").into());
            }
            let message = match name.as_str() {
                "init" => "Create or update AGENTS.md for this project. Inspect the repository first, preserve existing instructions, and add concise project-specific guidance.".into(),
                "worktree" if !argument.is_empty() => format!("Complete this coding task through a managed Git worktree: {argument}"),
                "worktree" => return Err("add the coding objective after /worktree".into()),
                _ => unreachable!(),
            };
            let view = send_interactive_message(
                client,
                host,
                token,
                conversation,
                preferences,
                SubmittedLine {
                    content: message,
                    mentions: Vec::new(),
                },
                pending_mentions,
            )
            .await?;
            *conversation = view.conversation;
            pending_mentions.clear();
        }
        "local" => println!(
            "Local workspace: {}",
            project_root.as_ref().map_or_else(
                || "project-free read-only chat".into(),
                |root| root.display().to_string()
            )
        ),
        "skills" | "apps" => {
            let discovery: serde_json::Value = decode_api_response(
                client
                    .post(format!(
                        "{host}/api/v1/conversations/{}/codex/discovery",
                        conversation.id
                    ))
                    .bearer_auth(token)
                    .json(&serde_json::json!({
                        "cwds": project_root.iter().map(|path| path.display().to_string()).collect::<Vec<_>>(),
                        "force_reload": false,
                    }))
                    .send()
                    .await?,
                "discover Codex context",
            )
            .await?;
            let key = name.as_str();
            println!(
                "{}",
                serde_json::to_string_pretty(
                    discovery
                        .get(key)
                        .unwrap_or(&serde_json::Value::Array(Vec::new()))
                )?
            );
        }
        "hooks" => {
            let cwd = project_root
                .as_deref()
                .map(|path| path.display().to_string())
                .or_else(|| {
                    std::env::current_dir()
                        .ok()
                        .map(|path| path.display().to_string())
                })
                .ok_or("cannot resolve a working directory for hook discovery")?;
            if argument.trim().is_empty() {
                let discovery: CodexStableDiscovery = decode_api_response(
                    client
                        .post(format!(
                            "{host}/api/v1/conversations/{}/codex/discovery",
                            conversation.id
                        ))
                        .bearer_auth(token)
                        .json(&serde_json::json!({
                            "cwds": [cwd],
                            "force_reload": true,
                        }))
                        .send()
                        .await?,
                    "discover Codex hooks",
                )
                .await?;
                let mut total = 0usize;
                for entry in discovery.hooks {
                    for warning in &entry.warnings {
                        println!("Warning · {warning}");
                    }
                    for error in &entry.errors {
                        println!("Error · {} · {}", error.path, error.message);
                    }
                    for hook in entry.hooks {
                        total += 1;
                        println!(
                            "{} · {} · {} · {}\n  {}\n  {}",
                            hook.event_name,
                            if hook.enabled { "enabled" } else { "disabled" },
                            hook.trust_status,
                            hook.source,
                            hook_handler_summary(&hook),
                            hook.source_path,
                        );
                    }
                }
                if total == 0 {
                    println!("No hooks configured for {cwd}.");
                } else {
                    println!("Type /hooks and choose an exact hook action from the picker.");
                }
            } else {
                let mut parts = argument.split_whitespace();
                let action = parts.next().ok_or("choose a hook action from /hooks")?;
                let selection_token = parts.next().ok_or("choose a hook from /hooks")?;
                if parts.next().is_some() {
                    return Err("hook actions accept exactly one selected hook".into());
                }
                let selected = decode_hook_selection(selection_token)?;
                if action == "inspect" {
                    let discovery: CodexStableDiscovery = decode_api_response(
                        client
                            .post(format!(
                                "{host}/api/v1/conversations/{}/codex/discovery",
                                conversation.id
                            ))
                            .bearer_auth(token)
                            .json(&serde_json::json!({
                                "cwds": [selected.cwd],
                                "force_reload": true,
                            }))
                            .send()
                            .await?,
                        "refresh selected hook",
                    )
                    .await?;
                    let hook = discovery
                        .hooks
                        .iter()
                        .flat_map(|entry| &entry.hooks)
                        .find(|hook| hook.key == selected.key)
                        .ok_or("the selected hook no longer exists; reopen /hooks")?;
                    println!(
                        "{} · {} · {} · {}\nHandler: {}\nMatcher: {}\nTimeout: {}s\nSource: {}\nHash: {}",
                        hook.event_name,
                        if hook.enabled { "enabled" } else { "disabled" },
                        hook.trust_status,
                        hook.source,
                        hook_handler_summary(hook),
                        hook.matcher.as_deref().unwrap_or("all matching events"),
                        hook.timeout_sec,
                        hook.source_path,
                        hook.current_hash,
                    );
                } else {
                    let update = match action {
                        "trust" => CodexHookStateUpdate {
                            cwd: selected.cwd,
                            key: selected.key,
                            expected_current_hash: selected.current_hash,
                            enabled: None,
                            trust_current_definition: true,
                        },
                        "enable" => CodexHookStateUpdate {
                            cwd: selected.cwd,
                            key: selected.key,
                            expected_current_hash: selected.current_hash,
                            enabled: Some(true),
                            trust_current_definition: false,
                        },
                        "disable" => CodexHookStateUpdate {
                            cwd: selected.cwd,
                            key: selected.key,
                            expected_current_hash: selected.current_hash,
                            enabled: Some(false),
                            trust_current_definition: false,
                        },
                        _ => return Err("unknown hook action; reopen /hooks".into()),
                    };
                    let refreshed: CodexStableDiscovery = decode_api_response(
                        client
                            .post(format!(
                                "{host}/api/v1/conversations/{}/codex/hooks/state",
                                conversation.id
                            ))
                            .bearer_auth(token)
                            .json(&update)
                            .send()
                            .await?,
                        "update selected hook",
                    )
                    .await?;
                    let hook = refreshed
                        .hooks
                        .iter()
                        .flat_map(|entry| &entry.hooks)
                        .find(|hook| hook.key == update.key)
                        .ok_or("the selected hook disappeared after the update")?;
                    println!(
                        "Hook {} · {} · {}",
                        hook.event_name,
                        if hook.enabled { "enabled" } else { "disabled" },
                        hook.trust_status,
                    );
                }
            }
        }
        "plugins" => {
            let response: PluginListResponse =
                get_authenticated(client, host, token, "/api/v1/plugins").await?;
            if response.plugins.is_empty() {
                println!("No plugins installed.");
            }
            for plugin in response.plugins {
                println!(
                    "{} {} · {}",
                    plugin.active_revision.manifest.display_name,
                    plugin.active_revision.manifest.version,
                    if plugin.installation.enabled {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
            }
        }
        "mcp" => {
            let response: McpListResponse =
                get_authenticated(client, host, token, "/api/v1/mcp/connections").await?;
            if response.connections.is_empty() {
                println!("No MCP servers configured.");
            }
            for item in response.connections {
                println!(
                    "{} · {} · {} tool(s)",
                    item.connection.display_name,
                    item.connection.id,
                    item.catalog.map_or(0, |catalog| catalog.tools.len())
                );
            }
        }
        "agent" | "subagents" => {
            let view =
                fetch_conversation(client, host, token, &conversation.id.to_string()).await?;
            let (selector, message) = argument
                .trim()
                .split_once(char::is_whitespace)
                .map_or((argument.trim(), None), |(selector, message)| {
                    (selector, Some(message.trim()))
                });
            let requested = selector.to_ascii_lowercase();
            let matching = view
                .team
                .sessions
                .iter()
                .filter(|session| {
                    requested.is_empty()
                        || session.id.to_string() == requested
                        || format!("{:?}", session.kind).to_ascii_lowercase() == requested
                        || session
                            .display_name
                            .to_ascii_lowercase()
                            .contains(&requested)
                })
                .collect::<Vec<_>>();
            if let Some(message) = message.filter(|message| !message.is_empty()) {
                let target = matching
                    .last()
                    .ok_or("no agent session matches that selector")?;
                let team: mcg_domain::ConversationTeamState = decode_api_response(
                    client
                        .post(format!(
                            "{host}/api/v1/conversations/{}/agent-sessions/{}/messages",
                            conversation.id, target.id
                        ))
                        .bearer_auth(token)
                        .json(&mcg_api::MessageAgentSessionRequest {
                            content: message.to_owned(),
                        })
                        .send()
                        .await?,
                    "message agent session",
                )
                .await?;
                if let Some(response) = team
                    .messages
                    .iter()
                    .rev()
                    .find(|item| item.from_session_id == target.id)
                {
                    println!("{}: {}", target.display_name, response.content);
                }
                *conversation = view.conversation;
                return Ok(InteractiveControl::Continue);
            }
            let sessions = matching.into_iter();
            for session in sessions {
                println!(
                    "{} · {:?} · {:?} · {}",
                    session.display_name, session.kind, session.state, session.id
                );
                for message in view.team.messages.iter().filter(|message| {
                    message.from_session_id == session.id || message.to_session_id == session.id
                }) {
                    println!("  {:?}: {}", message.kind, message.content);
                }
            }
            *conversation = view.conversation;
        }
        "keymap" | "theme" | "statusline" | "title" | "pets" => {
            let mut tui = interactive_tui_preferences(client, host, token).await?;
            match name.as_str() {
                "keymap" if !argument.is_empty() => {
                    tui.keymap = argument.to_owned();
                    tui.vim_mode = argument == "vim";
                }
                "theme" if !argument.is_empty() => tui.theme = argument.to_owned(),
                "statusline" if !argument.is_empty() => {
                    tui.status_line = argument
                        .split(',')
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_owned)
                        .collect();
                }
                "title" if !argument.is_empty() => {
                    tui.title_fields = argument
                        .split(',')
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_owned)
                        .collect();
                }
                "pets" if argument.eq_ignore_ascii_case("none") => tui.pet = None,
                "pets" if !argument.is_empty() => tui.pet = Some(argument.to_owned()),
                _ => {}
            }
            if !argument.is_empty() {
                save_interactive_tui_preferences(client, host, token, &tui).await?;
            }
            println!("{}", serde_json::to_string_pretty(&tui)?);
        }
        "vim" => {
            let mut tui = interactive_tui_preferences(client, host, token).await?;
            tui.vim_mode = !tui.vim_mode;
            tui.keymap = if tui.vim_mode { "vim" } else { "default" }.into();
            save_interactive_tui_preferences(client, host, token, &tui).await?;
            println!(
                "Vim editing mode {}.",
                if tui.vim_mode { "on" } else { "off" }
            );
        }
        "debug-config" => println!(
            "{}",
            serde_json::to_string_pretty(&interactive_tui_preferences(client, host, token).await?)?
        ),
        "app" => {
            if let Some(root) = project_root.as_deref() {
                run_codex_app(root)?;
                println!("Conversation: {}", conversation.id);
            } else {
                println!(
                    "Open MetaCog Desktop and select conversation {} (project-free).",
                    conversation.id
                );
            }
        }
        "feedback" => println!(
            "Use `mcg doctor` for a support report and include it with your GitHub feedback."
        ),
        "memories" => println!(
            "Memory controls are available through `mcg project memory --help` and MetaCog Settings."
        ),
        "cloud" | "cloud-environment" => println!(
            "Cloud execution is not connected; this CLI uses governed local projects and worktrees."
        ),
        "ide-context" => println!(
            "IDE selection context is unavailable in the standalone CLI; use @ references instead."
        ),
        _ => unreachable!("all commands are handled above"),
    }
    Ok(InteractiveControl::Continue)
}

async fn interactive_tui_preferences(
    client: &reqwest::Client,
    host: &str,
    token: &str,
) -> Result<CliTuiPreferences, Box<dyn std::error::Error>> {
    decode_api_response(
        client
            .get(format!("{host}/api/v1/cli/preferences"))
            .bearer_auth(token)
            .send()
            .await?,
        "read CLI preferences",
    )
    .await
}

async fn save_interactive_tui_preferences(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    preferences: &CliTuiPreferences,
) -> Result<(), Box<dyn std::error::Error>> {
    let _: CliTuiPreferences = decode_api_response(
        client
            .put(format!("{host}/api/v1/cli/preferences"))
            .bearer_auth(token)
            .json(preferences)
            .send()
            .await?,
        "save CLI preferences",
    )
    .await?;
    Ok(())
}

fn codex_exec_prompt(words: &[String]) -> Result<String, Box<dyn std::error::Error>> {
    if words == ["-"] || (words.is_empty() && !std::io::stdin().is_terminal()) {
        return validated_goal(&read_bounded_stdin()?);
    }
    validated_goal(&words.join(" "))
}

fn codex_exec_mentions(images: &[PathBuf]) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut mentions = Vec::new();
    for image in images {
        let value = image.to_str().ok_or("image paths must be valid Unicode")?;
        for path in value.split(',').filter(|path| !path.trim().is_empty()) {
            mentions.push(format!("file:{}", path.trim()));
        }
    }
    Ok(mentions)
}

fn codex_additional_writable_roots(
    directories: &[PathBuf],
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    const MAX_ADDITIONAL_WRITABLE_ROOTS: usize = 16;
    if directories.len() > MAX_ADDITIONAL_WRITABLE_ROOTS {
        return Err(format!(
            "at most {MAX_ADDITIONAL_WRITABLE_ROOTS} --add-dir values are allowed"
        )
        .into());
    }
    let mut roots = Vec::with_capacity(directories.len());
    let mut identities = std::collections::BTreeSet::new();
    for directory in directories {
        let canonical = std::fs::canonicalize(directory).map_err(|error| {
            format!(
                "additional writable directory {} is unavailable: {error}",
                directory.display()
            )
        })?;
        if !canonical.is_dir() {
            return Err(format!(
                "additional writable path is not a directory: {}",
                canonical.display()
            )
            .into());
        }
        let value = canonical
            .to_str()
            .ok_or("additional writable directory paths must be valid Unicode")?;
        #[cfg(windows)]
        let identity = value.to_lowercase();
        #[cfg(not(windows))]
        let identity = value.to_owned();
        if identities.insert(identity) {
            roots.push(value.to_owned());
        }
    }
    Ok(roots)
}

fn require_git_repository(project: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let output = ProcessCommand::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(project)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if !output.success() {
        return Err(
            "not inside a Git repository; pass --skip-git-repo-check to allow MetaCog to initialize and govern this directory"
                .into(),
        );
    }
    Ok(())
}

fn read_output_schema(
    path: &std::path::Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    const MAX_OUTPUT_SCHEMA_BYTES: u64 = 64 * 1024;
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("cannot read output schema {}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("output schema is not a regular file: {}", path.display()).into());
    }
    if metadata.len() > MAX_OUTPUT_SCHEMA_BYTES {
        return Err(format!(
            "output schema exceeds the {MAX_OUTPUT_SCHEMA_BYTES}-byte limit: {}",
            path.display()
        )
        .into());
    }
    let schema: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)
        .map_err(|error| format!("output schema {} is invalid JSON: {error}", path.display()))?;
    if !schema.is_object() {
        return Err(format!(
            "output schema must contain a JSON object: {}",
            path.display()
        )
        .into());
    }
    Ok(schema)
}

async fn run_codex_compatible_exec(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    project: &std::path::Path,
    command: Option<ExecCommand>,
    json: bool,
    ephemeral: bool,
    skip_git_repo_check: bool,
    model: Option<String>,
    model_provider: Option<String>,
    sandbox: Option<CodexSandboxArg>,
    approval_policy: Option<CodexApprovalPolicyArg>,
    approve_for_me: bool,
    dangerously_bypass_approvals_and_sandbox: bool,
    dangerously_bypass_hook_trust: bool,
    additional_writable_roots: &[String],
    feature_overrides: &BTreeMap<String, bool>,
    config_overrides: &[CodexConfigOverride],
    codex_profile: Option<String>,
    strict_config: bool,
    images: Vec<PathBuf>,
    output_last_message: Option<PathBuf>,
    output_schema: Option<PathBuf>,
    prompt: Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if !skip_git_repo_check {
        require_git_repository(project)?;
    }
    let mut preferences = codex_execution_preferences(
        model,
        model_provider,
        sandbox,
        approval_policy,
        approve_for_me,
        dangerously_bypass_approvals_and_sandbox,
        additional_writable_roots,
        feature_overrides,
        config_overrides,
        codex_profile,
        strict_config,
        conversation_preferences(None, None, None, ApprovalMode::Ask),
    );
    preferences.output_schema = output_schema
        .as_deref()
        .map(read_output_schema)
        .transpose()?;
    let mentions = codex_exec_mentions(&images)?;
    match command {
        None => {
            let goal = codex_exec_prompt(&prompt)?;
            run_consumer_submission(
                client,
                host,
                token,
                project,
                &goal,
                None,
                if json {
                    ConsumerOutput::StreamJson
                } else {
                    ConsumerOutput::Human
                },
                preferences,
                &mentions,
                output_last_message.as_deref(),
                ephemeral,
                dangerously_bypass_hook_trust,
            )
            .await
        }
        Some(ExecCommand::Resume {
            last,
            all,
            session,
            output_schema,
            prompt: follow_up,
        }) => {
            if !prompt.is_empty() {
                return Err("place the follow-up prompt after `exec resume`".into());
            }
            let conversation = resolve_cli_session(
                client,
                host,
                token,
                project,
                session.as_deref(),
                last,
                all,
                Some(false),
                true,
                false,
            )
            .await?;
            if dangerously_bypass_hook_trust {
                bind_invocation_hook_trust_bypass(client, host, token, conversation.id).await?;
                warn_hook_trust_bypass();
            }
            let result: Result<(), Box<dyn std::error::Error>> = async {
                if let Some(path) = output_schema.as_deref() {
                    preferences.output_schema = Some(read_output_schema(path)?);
                }
                let content = codex_exec_prompt(&follow_up)?;
                let (content, mentions) =
                    resolve_cli_mentions(client, host, token, content, &mentions).await?;
                let mut view: ConversationView = decode_api_response(
                    client
                        .post(format!(
                            "{host}/api/v1/conversations/{}/messages",
                            conversation.id
                        ))
                        .bearer_auth(token)
                        .json(&SendConversationMessageRequest {
                            content,
                            execution_preferences: Some(preferences),
                            mentions,
                        })
                        .send()
                        .await?,
                    "resume non-interactive conversation",
                )
                .await?;
                let turn_id = view
                    .messages
                    .iter()
                    .rev()
                    .find(|message| message.role == ConversationRole::User)
                    .map(|message| message.turn_id)
                    .ok_or("resumed conversation did not contain the submitted turn")?;
                if json {
                    emit_exec_jsonl(&ExecJsonlEvent::ThreadStarted {
                        thread_id: conversation.id.to_string(),
                    })?;
                    emit_exec_jsonl(&ExecJsonlEvent::TurnStarted {})?;
                }
                let mut projector = ExecJsonlProjector::new(turn_id);
                view = if json {
                    match wait_for_conversation_jsonl(
                        client,
                        host,
                        token,
                        view,
                        turn_id,
                        &mut projector,
                        3_600,
                    )
                    .await
                    {
                        Ok(view) => view,
                        Err(error) => {
                            emit_exec_jsonl(&ExecJsonlEvent::Error {
                                message: error.to_string(),
                            })?;
                            emit_exec_jsonl(&ExecJsonlProjector::failed(error.to_string()))?;
                            return Err(error);
                        }
                    }
                } else {
                    wait_for_conversation(
                        client,
                        host,
                        token,
                        &conversation.id.to_string(),
                        turn_id,
                        3_600,
                        2,
                    )
                    .await?
                };
                let final_message = view
                    .messages
                    .iter()
                    .rev()
                    .find(|message| {
                        message.turn_id == turn_id && message.role == ConversationRole::Assistant
                    })
                    .map(|message| message.content.as_str())
                    .unwrap_or_default();
                if let Some(path) = output_last_message.as_deref() {
                    std::fs::write(path, final_message)?;
                }
                if json {
                    for event in projector.finalize_items(&view) {
                        emit_exec_jsonl(&event)?;
                    }
                    if let Err(error) = ensure_governed_consumer_success(&view, turn_id) {
                        emit_exec_jsonl(&ExecJsonlProjector::failed(error.to_string()))?;
                        return Err(error);
                    }
                    emit_exec_jsonl(&projector.completed())?;
                } else {
                    println!("{final_message}");
                    ensure_governed_consumer_success(&view, turn_id)?;
                }
                Ok(())
            }
            .await;
            finish_invocation_hook_trust_binding(
                client,
                host,
                token,
                conversation.id,
                dangerously_bypass_hook_trust,
                result,
            )
            .await?;
            if ephemeral {
                let _ = delete_cli_conversation_by_id(client, host, token, conversation.id).await?;
            }
            Ok(())
        }
    }
}

async fn run_codex_compatible_review(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    project: &std::path::Path,
    uncommitted: bool,
    base: Option<String>,
    commit: Option<String>,
    title: Option<String>,
    prompt: Vec<String>,
    preferences: ExecutionPreferences,
    bypass_hook_trust: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    require_git_repository(project)?;
    let canonical_root = std::fs::canonicalize(project)?;
    let root = canonical_root
        .to_str()
        .ok_or("project directory must be valid Unicode")?;
    let project = resolve_consumer_project(client, host, token, root).await?;
    let target = if let Some(branch) = base {
        mcg_codex_app_server::CodexReviewTarget::BaseBranch { branch }
    } else if let Some(sha) = commit {
        mcg_codex_app_server::CodexReviewTarget::Commit { sha, title }
    } else if !prompt.is_empty() {
        mcg_codex_app_server::CodexReviewTarget::Custom {
            instructions: codex_exec_prompt(&prompt)?,
        }
    } else {
        let _ = uncommitted;
        mcg_codex_app_server::CodexReviewTarget::UncommittedChanges
    };
    let conversation = create_interactive_conversation(
        client,
        host,
        token,
        Some(project.project.project_id),
        &preferences,
        "MetaCog CLI review",
    )
    .await?;
    if bypass_hook_trust {
        if let Err(error) =
            bind_invocation_hook_trust_bypass(client, host, token, conversation.id).await
        {
            let _ = decode_api_response::<Conversation>(
                client
                    .post(format!(
                        "{host}/api/v1/conversations/{}/archive",
                        conversation.id
                    ))
                    .bearer_auth(token)
                    .json(&ArchiveConversationRequest { archived: true })
                    .send()
                    .await?,
                "archive failed review control conversation",
            )
            .await;
            return Err(error);
        }
        warn_hook_trust_bypass();
    }
    let result: Result<String, Box<dyn std::error::Error>> = async {
        let result: mcg_codex_app_server::CodexReviewResult = decode_api_response(
            client
                .post(format!(
                    "{host}/api/v1/codex/owned-threads/{}/review",
                    conversation.id
                ))
                .bearer_auth(token)
                .json(&mcg_api::CodexReviewRequest {
                    target,
                    delivery: mcg_codex_app_server::CodexReviewDelivery::Inline,
                })
                .send()
                .await?,
            "run native read-only review",
        )
        .await?;
        render_codex_review_turn(&result.turn)
    }
    .await;
    let rendered = finish_invocation_hook_trust_binding(
        client,
        host,
        token,
        conversation.id,
        bypass_hook_trust,
        result,
    )
    .await;
    let _: Conversation = decode_api_response(
        client
            .post(format!(
                "{host}/api/v1/conversations/{}/archive",
                conversation.id
            ))
            .bearer_auth(token)
            .json(&ArchiveConversationRequest { archived: true })
            .send()
            .await?,
        "archive review control conversation",
    )
    .await?;
    println!("{}", rendered?);
    Ok(())
}

fn render_codex_review_turn(
    turn: &serde_json::Value,
) -> Result<String, Box<dyn std::error::Error>> {
    if turn.get("status").and_then(serde_json::Value::as_str) == Some("failed") {
        let message = turn
            .pointer("/error/message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("Codex review failed without a diagnostic");
        return Err(message.to_owned().into());
    }
    let finding = turn
        .get("items")
        .and_then(serde_json::Value::as_array)
        .and_then(|items| {
            items.iter().rev().find_map(|item| {
                (item.get("type").and_then(serde_json::Value::as_str) == Some("agentMessage"))
                    .then(|| item.get("text").and_then(serde_json::Value::as_str))
                    .flatten()
                    .map(str::trim)
                    .filter(|text| !text.is_empty())
            })
        })
        .ok_or("Codex review completed without human-readable findings")?;
    Ok(finding.to_owned())
}

async fn run_consumer_submission(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    project_root: &std::path::Path,
    goal: &str,
    supplemental_input: Option<String>,
    output: ConsumerOutput,
    preferences: ExecutionPreferences,
    mention_specs: &[String],
    output_last_message: Option<&std::path::Path>,
    ephemeral: bool,
    bypass_hook_trust: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let canonical_root = std::fs::canonicalize(project_root)?;
    let root = canonical_root
        .to_str()
        .ok_or("project directory must be valid Unicode")?;
    let project = resolve_consumer_project(client, host, token, root).await?;
    let title = goal.chars().take(96).collect::<String>();
    let conversation: Conversation = decode_api_response(
        client
            .post(format!("{host}/api/v1/conversations"))
            .bearer_auth(token)
            .json(&CreateConversationRequest {
                title: Some(title),
                project_id: Some(project.project.project_id),
                execution_preferences: preferences.clone(),
                interaction_kind: ConversationInteractionKind::NonInteractive,
            })
            .send()
            .await?,
        "create governed consumer conversation",
    )
    .await?;

    if bypass_hook_trust {
        if let Err(error) =
            bind_invocation_hook_trust_bypass(client, host, token, conversation.id).await
        {
            let _ = delete_cli_conversation_by_id(client, host, token, conversation.id).await;
            return Err(error);
        }
        warn_hook_trust_bypass();
    }

    let result: Result<(), Box<dyn std::error::Error>> = async {
        let content = if let Some(input) = supplemental_input {
            format!("{goal}\n\nOwner-supplied standard input (untrusted data):\n{input}")
        } else {
            goal.to_owned()
        };
        let (content, mentions) =
            resolve_cli_mentions(client, host, token, content, mention_specs).await?;
        let mut view: ConversationView = decode_api_response(
            client
                .post(format!(
                    "{host}/api/v1/conversations/{}/messages",
                    conversation.id
                ))
                .bearer_auth(token)
                .json(&SendConversationMessageRequest {
                    content,
                    execution_preferences: Some(preferences),
                    mentions,
                })
                .send()
                .await?,
            "submit governed consumer request",
        )
        .await?;
        let origin_turn_id = view
            .messages
            .iter()
            .rev()
            .find(|message| message.role == ConversationRole::User)
            .map(|message| message.turn_id)
            .ok_or("governed consumer response did not contain the owner turn")?;

        if output == ConsumerOutput::StreamJson {
            emit_exec_jsonl(&ExecJsonlEvent::ThreadStarted {
                thread_id: conversation.id.to_string(),
            })?;
            emit_exec_jsonl(&ExecJsonlEvent::TurnStarted {})?;
        }
        let mut projector = ExecJsonlProjector::new(origin_turn_id);
        view = if output == ConsumerOutput::StreamJson {
            match wait_for_conversation_jsonl(
                client,
                host,
                token,
                view,
                origin_turn_id,
                &mut projector,
                3_600,
            )
            .await
            {
                Ok(view) => view,
                Err(error) => {
                    emit_exec_jsonl(&ExecJsonlEvent::Error {
                        message: error.to_string(),
                    })?;
                    emit_exec_jsonl(&ExecJsonlProjector::failed(error.to_string()))?;
                    return Err(error);
                }
            }
        } else {
            wait_for_conversation(
                client,
                host,
                token,
                &conversation.id.to_string(),
                origin_turn_id,
                3_600,
                2,
            )
            .await?
        };
        let final_message = view
            .messages
            .iter()
            .rev()
            .find(|message| {
                message.turn_id == origin_turn_id && message.role == ConversationRole::Assistant
            })
            .map(|message| message.content.clone());
        if let Some(path) = output_last_message {
            std::fs::write(path, final_message.as_deref().unwrap_or_default())?;
        }
        if output == ConsumerOutput::Json {
            println!(
                "{}",
                serde_json::to_string_pretty(&conversation_receipt(&view, Some(origin_turn_id))?)?
            );
        } else if output == ConsumerOutput::StreamJson {
            for event in projector.finalize_items(&view) {
                emit_exec_jsonl(&event)?;
            }
            if let Err(error) = ensure_governed_consumer_success(&view, origin_turn_id) {
                emit_exec_jsonl(&ExecJsonlProjector::failed(error.to_string()))?;
                return Err(error);
            }
            emit_exec_jsonl(&projector.completed())?;
        } else {
            println!("Project: {}", project.project.root);
            println!("Conversation: {}", conversation.id);
            if let Some(text) = final_message.as_deref() {
                println!("\n{text}");
            }
        }
        if output != ConsumerOutput::StreamJson {
            ensure_governed_consumer_success(&view, origin_turn_id)?;
        }
        Ok(())
    }
    .await;
    finish_invocation_hook_trust_binding(
        client,
        host,
        token,
        conversation.id,
        bypass_hook_trust,
        result,
    )
    .await?;
    if ephemeral {
        let _ = delete_cli_conversation_by_id(client, host, token, conversation.id).await?;
    }
    Ok(())
}

async fn resolve_cli_mentions(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    mut content: String,
    specs: &[String],
) -> Result<(String, Vec<ComposerMentionInput>), Box<dyn std::error::Error>> {
    if specs.len() > 20 {
        return Err("a turn can contain at most 20 --mention values".into());
    }
    let plugin_catalog = if specs.iter().any(|spec| spec.starts_with("plugin:")) {
        Some(
            decode_api_response::<PluginListResponse>(
                client
                    .get(format!("{host}/api/v1/plugins"))
                    .bearer_auth(token)
                    .send()
                    .await?,
                "list plugins for mentions",
            )
            .await?,
        )
    } else {
        None
    };
    let mut mentions = Vec::with_capacity(specs.len());
    for spec in specs {
        let (kind, resource) = spec
            .split_once(':')
            .ok_or("--mention must use file:path, folder:path, or plugin:id")?;
        if resource.trim().is_empty() {
            return Err("--mention resource cannot be empty".into());
        }
        let (kind, label, plugin_revision_id, installation_revision) = match kind {
            "file" | "folder" => {
                let label = std::path::Path::new(resource)
                    .file_name()
                    .and_then(|value| value.to_str())
                    .filter(|value| !value.is_empty())
                    .unwrap_or(resource)
                    .to_owned();
                (
                    if kind == "file" {
                        MentionKind::File
                    } else {
                        MentionKind::Folder
                    },
                    label,
                    None,
                    None,
                )
            }
            "plugin" => {
                let plugin = plugin_catalog
                    .as_ref()
                    .and_then(|catalog| {
                        catalog.plugins.iter().find(|plugin| {
                            plugin.installation.plugin_id == resource
                                || plugin
                                    .active_revision
                                    .manifest
                                    .display_name
                                    .eq_ignore_ascii_case(resource)
                        })
                    })
                    .ok_or("mentioned plugin is not installed")?;
                if !plugin.installation.enabled {
                    return Err("mentioned plugin is disabled".into());
                }
                (
                    MentionKind::Plugin,
                    plugin.active_revision.manifest.display_name.clone(),
                    Some(plugin.active_revision.id),
                    Some(plugin.installation.revision),
                )
            }
            _ => return Err("--mention kind must be file, folder, or plugin".into()),
        };
        let separator = if content.is_empty() { "" } else { "\n" };
        content.push_str(separator);
        let byte_start = u32::try_from(content.len())?;
        content.push('@');
        content.push_str(&label);
        let byte_end = u32::try_from(content.len())?;
        mentions.push(ComposerMentionInput {
            kind,
            label,
            resource: resource.to_owned(),
            byte_start,
            byte_end,
            plugin_revision_id,
            installation_revision,
        });
    }
    Ok((content, mentions))
}

fn ensure_governed_consumer_success(
    view: &ConversationView,
    origin_turn_id: ConversationTurnId,
) -> Result<(), Box<dyn std::error::Error>> {
    let batch = view
        .team
        .coordination_batches
        .iter()
        .find(|batch| batch.origin_turn_id == origin_turn_id)
        .ok_or("governed consumer request completed without a coordination batch")?;
    if batch.state != CoordinationBatchState::Completed {
        return Err(format!(
            "governed consumer batch {} ended in {:?}; inspect with `mcg conversation receipt {}`",
            batch.id, batch.state, view.conversation.id
        )
        .into());
    }
    let delegations = view
        .team
        .delegations
        .iter()
        .filter(|delegation| delegation.coordination_batch_id == Some(batch.id))
        .collect::<Vec<_>>();
    let effective_ids = effective_delegation_attempt_ids(
        delegations
            .iter()
            .map(|delegation| (delegation.id, delegation.retry_of_id)),
    );
    let effective_delegations = delegations
        .iter()
        .copied()
        .filter(|delegation| effective_ids.contains(&delegation.id))
        .collect::<Vec<_>>();
    if delegations.is_empty()
        || effective_delegations.iter().any(|delegation| {
            !delegation.governance.outcome.successful
                || !delegation.integration_ready
                || !delegation.unresolved_issues.is_empty()
        })
    {
        return Err(format!(
            "governed consumer batch {} did not pass its Coder hard gate; inspect with `mcg conversation receipt {}`",
            batch.id, view.conversation.id
        )
        .into());
    }
    Ok(())
}

fn conversation_receipt(
    view: &ConversationView,
    origin_turn_id: Option<ConversationTurnId>,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let batch = match origin_turn_id {
        Some(turn_id) => view
            .team
            .coordination_batches
            .iter()
            .find(|batch| batch.origin_turn_id == turn_id),
        None => view
            .team
            .coordination_batches
            .iter()
            .max_by_key(|batch| batch.created_at),
    }
    .ok_or("conversation has no matching coordination batch")?;
    let delegations = view
        .team
        .delegations
        .iter()
        .filter(|delegation| delegation.coordination_batch_id == Some(batch.id))
        .collect::<Vec<_>>();
    let effective_ids = effective_delegation_attempt_ids(
        delegations
            .iter()
            .map(|delegation| (delegation.id, delegation.retry_of_id)),
    );
    let effective_delegations = delegations
        .iter()
        .copied()
        .filter(|delegation| effective_ids.contains(&delegation.id))
        .collect::<Vec<_>>();
    let wakes = view
        .team
        .wakes
        .iter()
        .filter(|wake| wake.coordination_batch_id == batch.id)
        .collect::<Vec<_>>();
    let completion = conversation_batch_terminal(batch.state)
        .then(|| {
            view.messages.iter().rev().find(|message| {
                message.turn_id == batch.origin_turn_id
                    && message.role == ConversationRole::Assistant
                    && message.created_at >= batch.created_at
            })
        })
        .flatten();
    let coder_model_turns = delegations
        .iter()
        .map(|delegation| u64::from(delegation.governance.outcome.model_turns))
        .sum::<u64>();
    let coder_approximate_tokens = delegations
        .iter()
        .map(|delegation| delegation.governance.outcome.approximate_tokens)
        .sum::<u64>();
    let tool_calls = delegations
        .iter()
        .map(|delegation| u64::from(delegation.governance.outcome.tool_calls))
        .sum::<u64>();
    Ok(serde_json::json!({
        "schema_version": 1,
        "conversation_id": view.conversation.id,
        "batch_id": batch.id,
        "origin_turn_id": batch.origin_turn_id,
        "state": batch.state,
        "delivery_target": batch.delivery_target,
        "delivery_rationale": batch.delivery_rationale,
        "coordinator": {
            "bypassed": batch.coordinator_bypassed,
            "model_turns": batch.coordinator_turn_count,
            "approximate_tokens": batch.coordinator_approximate_tokens,
            "routing_rationale": batch.routing_rationale,
        },
        "coders": {
            "count": delegations.len(),
            "effective_count": effective_delegations.len(),
            "model_turns": coder_model_turns,
            "approximate_tokens": coder_approximate_tokens,
            "tool_calls": tool_calls,
            "delegations": delegations.iter().map(|delegation| {
                let evidence_json = serde_json::to_vec(&delegation.evidence)
                    .expect("delegation evidence is JSON serializable");
                serde_json::json!({
                    "id": delegation.id,
                    "state": delegation.state,
                    "revision": delegation.revision,
                    "model": view.team.sessions.iter()
                        .find(|session| Some(session.id) == delegation.coder_session_id)
                        .map(|session| session.model.clone()),
                    "attempts": delegation.governance.outcome.attempts,
                    "latency_ms": delegation.governance.outcome.latency_ms,
                    "changed_files": delegation.changed_files,
                    "checks": delegation.checks,
                    // Full attributed evidence remains durable and searchable through
                    // `conversation team`; the qualification receipt stays compact and
                    // tamper-evident so it is practical for scripts and CI artifacts.
                    "evidence_count": delegation.evidence.len(),
                    "evidence_sha256": format!("{:x}", Sha256::digest(&evidence_json)),
                    "unresolved_issues": delegation.unresolved_issues,
                    "integration_ready": delegation.integration_ready,
                    "successful": delegation.governance.outcome.successful,
                    "failure_class": delegation.governance.failure_class,
                    "recovery": delegation.governance.recovery,
                    "superseded_by_retry": !effective_ids.contains(&delegation.id),
                })
            }).collect::<Vec<_>>(),
        },
        "wakes": wakes.iter().map(|wake| serde_json::json!({
            "id": wake.id,
            "kind": wake.kind,
            "target": wake.target,
            "state": wake.state,
            "attempts": wake.attempt_count,
            "last_error": wake.last_error,
        })).collect::<Vec<_>>(),
        "completion": completion.map(|message| serde_json::json!({
            "message_id": message.id,
            "content": message.content,
            "created_at": message.created_at,
        })),
        "previews": view.team.previews.iter()
            .filter(|preview| preview.coordination_batch_id == batch.id)
            .collect::<Vec<_>>(),
        "hard_gate_ready": conversation_batch_terminal(batch.state)
            && completion.is_some()
            && !effective_delegations.is_empty()
            && effective_delegations.iter().all(|delegation| {
                delegation.governance.outcome.successful
                    && delegation.integration_ready
            })
            && effective_delegations.iter().all(|delegation| delegation.unresolved_issues.is_empty())
            && wakes.iter().all(|wake| wake.state == AgentWakeState::Completed),
    }))
}

async fn resolve_consumer_project(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    canonical_root: &str,
) -> Result<ProjectMutation, Box<dyn std::error::Error>> {
    let projects: ProjectListResponse = client
        .get(format!("{host}/api/v1/projects"))
        .bearer_auth(token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if let Some(project) = projects
        .projects
        .into_iter()
        .find(|project| paths_match_for_host(std::path::Path::new(&project.root), canonical_root))
    {
        let refreshed: ProjectMutation = client
            .post(format!(
                "{host}/api/v1/projects/{}/refresh",
                project.project_id
            ))
            .bearer_auth(token)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        return Ok(client
            .post(format!(
                "{host}/api/v1/projects/{}/initialize-git",
                refreshed.project.project_id
            ))
            .bearer_auth(token)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?);
    }
    Ok(client
        .post(format!("{host}/api/v1/projects"))
        .bearer_auth(token)
        .json(&DiscoverProjectRequest {
            root: canonical_root.to_owned(),
            project_id: None,
            initialize_git: true,
        })
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

async fn resolve_apply_project(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    canonical_root: &str,
) -> Result<ProjectMutation, Box<dyn std::error::Error>> {
    let projects: ProjectListResponse = decode_api_response(
        client
            .get(format!("{host}/api/v1/projects"))
            .bearer_auth(token)
            .send()
            .await?,
        "list projects",
    )
    .await?;
    if let Some(project) = projects
        .projects
        .into_iter()
        .find(|project| paths_match_for_host(std::path::Path::new(&project.root), canonical_root))
    {
        return decode_api_response(
            client
                .post(format!(
                    "{host}/api/v1/projects/{}/refresh",
                    project.project_id
                ))
                .bearer_auth(token)
                .send()
                .await?,
            "refresh project",
        )
        .await;
    }
    decode_api_response(
        client
            .post(format!("{host}/api/v1/projects"))
            .bearer_auth(token)
            .json(&DiscoverProjectRequest {
                root: canonical_root.to_owned(),
                project_id: None,
                initialize_git: false,
            })
            .send()
            .await?,
        "discover project",
    )
    .await
}

fn paths_match_for_host(existing: &std::path::Path, canonical_root: &str) -> bool {
    #[cfg(windows)]
    {
        existing
            .to_string_lossy()
            .eq_ignore_ascii_case(canonical_root)
    }
    #[cfg(not(windows))]
    {
        existing == std::path::Path::new(canonical_root)
    }
}

fn default_launch_request() -> LaunchTaskRequest {
    LaunchTaskRequest {
        organization_id: None,
        target_path: None,
        context_token_budget: 8_192,
        requested_tools: Vec::new(),
        estimated_output_tokens: 8_192,
        max_model_calls: 16,
        provider_id: None,
        model_id: None,
        reasoning_effort: None,
        service_tier: None,
    }
}

#[allow(clippy::too_many_lines)] // Keeps the complete host and workflow HTTP contract together.
async fn run_ssh(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    command: SshCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        SshCommand::List => {
            let response: SshHostListResponse = client
                .get(format!("{host}/api/v1/ssh/hosts"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        SshCommand::Show { id } => {
            let id = SshHostId::from_str(&id)?;
            let host_record: SshHost = client
                .get(format!("{host}/api/v1/ssh/hosts/{id}"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let revisions: SshHostRevisionListResponse = client
                .get(format!("{host}/api/v1/ssh/hosts/{id}/revisions"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let operations: SshOperationListResponse = client
                .get(format!("{host}/api/v1/ssh/hosts/{id}/operations"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&(host_record, revisions, operations))?
            );
        }
        SshCommand::Register { request } => {
            let request: CreateSshHostRequest =
                serde_json::from_reader(std::fs::File::open(request)?)?;
            let response: SshHost = client
                .post(format!("{host}/api/v1/ssh/hosts"))
                .bearer_auth(token)
                .json(&request)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        SshCommand::Update { id, request } => {
            let id = SshHostId::from_str(&id)?;
            let request: UpdateSshHostRequest =
                serde_json::from_reader(std::fs::File::open(request)?)?;
            let response: SshHostMutation = client
                .put(format!("{host}/api/v1/ssh/hosts/{id}"))
                .bearer_auth(token)
                .json(&request)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        SshCommand::Run { id, request } => {
            let id = SshHostId::from_str(&id)?;
            let request: RunSshOperationRequest =
                serde_json::from_reader(std::fs::File::open(request)?)?;
            let response: SshOperationMutation = client
                .post(format!("{host}/api/v1/ssh/hosts/{id}/operations"))
                .bearer_auth(token)
                .json(&request)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        SshCommand::WorkflowList => {
            let response: SshWorkflowListResponse = client
                .get(format!("{host}/api/v1/ssh/workflows"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        SshCommand::WorkflowShow { id } => {
            let id = SshWorkflowId::from_str(&id)?;
            let response: SshWorkflow = client
                .get(format!("{host}/api/v1/ssh/workflows/{id}"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        SshCommand::WorkflowCreate { request } => {
            let request: SshWorkflowCreateRequest =
                serde_json::from_reader(std::fs::File::open(request)?)?;
            let response: SshWorkflowMutation = client
                .post(format!("{host}/api/v1/ssh/workflows"))
                .bearer_auth(token)
                .json(&request)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        SshCommand::WorkflowRun { id } => {
            let id = SshWorkflowId::from_str(&id)?;
            let response: SshWorkflowMutation = client
                .post(format!("{host}/api/v1/ssh/workflows/{id}/run"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        SshCommand::WorkflowCancel { id } => {
            let id = SshWorkflowId::from_str(&id)?;
            let response: SshWorkflowMutation = client
                .post(format!("{host}/api/v1/ssh/workflows/{id}/cancel"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_lines)] // Keeps the complete experiment HTTP contract auditable together.
async fn run_experiment(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    command: ExperimentCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    let value = match command {
        ExperimentCommand::List { organization_id } => {
            let response: ExperimentListResponse = client
                .get(format!(
                    "{host}/api/v1/organizations/{organization_id}/experiments"
                ))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            serde_json::to_value(response)?
        }
        ExperimentCommand::Show { id } => {
            let experiment: ExperimentDefinition = client
                .get(format!("{host}/api/v1/experiments/{id}"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let shadow: ShadowExecutionListResponse = client
                .get(format!("{host}/api/v1/experiments/{id}/shadow-executions"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let benchmarks: ExperimentBenchmarkListResponse = client
                .get(format!("{host}/api/v1/experiments/{id}/benchmarks"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let decisions: ExperimentDecisionListResponse = client
                .get(format!("{host}/api/v1/experiments/{id}/decisions"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            serde_json::json!({
                "experiment": experiment,
                "shadow_executions": shadow.receipts,
                "benchmarks": benchmarks.benchmarks,
                "decisions": decisions.decisions,
            })
        }
        ExperimentCommand::Candidate { request } => {
            let body: OrganizationGenome = serde_json::from_reader(std::fs::File::open(request)?)?;
            let response: CandidateRevisionMutation = client
                .post(format!("{host}/api/v1/metacognition/candidates"))
                .bearer_auth(token)
                .json(&body)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            serde_json::to_value(response)?
        }
        ExperimentCommand::Observe { request } => {
            let body: SupervisionObservation =
                serde_json::from_reader(std::fs::File::open(request)?)?;
            let response: SupervisionMutation = client
                .post(format!("{host}/api/v1/metacognition/observations"))
                .bearer_auth(token)
                .json(&body)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            serde_json::to_value(response)?
        }
        ExperimentCommand::Observations => {
            let response: SupervisionObservationListResponse = client
                .get(format!("{host}/api/v1/metacognition/observations"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            serde_json::to_value(response)?
        }
        ExperimentCommand::Create { request } => {
            let body: ExperimentDefinition =
                serde_json::from_reader(std::fs::File::open(request)?)?;
            let response: ExperimentMutation = client
                .post(format!("{host}/api/v1/experiments"))
                .bearer_auth(token)
                .json(&body)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            serde_json::to_value(response)?
        }
        ExperimentCommand::Transition { id, expected, next } => {
            let response: ExperimentMutation = client
                .post(format!("{host}/api/v1/experiments/{id}/transition"))
                .bearer_auth(token)
                .json(&TransitionExperimentRequest {
                    expected: expected.into(),
                    next: next.into(),
                })
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            serde_json::to_value(response)?
        }
        ExperimentCommand::Shadow { id, request } => {
            let body: ShadowExecutionReceipt =
                serde_json::from_reader(std::fs::File::open(request)?)?;
            let response: EvidenceMutation<ShadowExecutionReceipt> = client
                .post(format!("{host}/api/v1/experiments/{id}/shadow-executions"))
                .bearer_auth(token)
                .json(&body)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            serde_json::to_value(response)?
        }
        ExperimentCommand::Benchmark { id, request } => {
            let body: ExperimentBenchmark = serde_json::from_reader(std::fs::File::open(request)?)?;
            let response: EvidenceMutation<ExperimentBenchmark> = client
                .post(format!("{host}/api/v1/experiments/{id}/benchmarks"))
                .bearer_auth(token)
                .json(&body)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            serde_json::to_value(response)?
        }
        ExperimentCommand::Decide { id, request } => {
            let body: ExperimentDecision = serde_json::from_reader(std::fs::File::open(request)?)?;
            let response: ExperimentMutation = client
                .post(format!("{host}/api/v1/experiments/{id}/decisions"))
                .bearer_auth(token)
                .json(&body)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            serde_json::to_value(response)?
        }
    };
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

#[allow(clippy::too_many_lines)] // Keeps the consumer Site command-to-HTTP contract auditable in one place.
async fn run_site(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    command: SiteCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        SiteCommand::List { json } => {
            let response: SiteListResponse = client
                .get(format!("{host}/api/v1/sites"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else if response.sites.is_empty() {
                println!("No Sites registered");
            } else {
                for site in response.sites {
                    println!(
                        "{}  {}  {:?}  {}",
                        site.id, site.display_name, site.state, site.public_url
                    );
                }
            }
        }
        SiteCommand::Show { id } => {
            let id = SiteId::from_str(&id)?;
            let site: Site = client
                .get(format!("{host}/api/v1/sites/{id}"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let revisions: SiteRevisionListResponse = client
                .get(format!("{host}/api/v1/sites/{id}/revisions"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let health: SiteHealthListResponse = client
                .get(format!("{host}/api/v1/sites/{id}/health"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let deployments: SiteDeploymentListResponse = client
                .get(format!("{host}/api/v1/sites/{id}/deployments"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let monitor_response = client
                .get(format!("{host}/api/v1/sites/{id}/monitor"))
                .bearer_auth(token)
                .send()
                .await?;
            let monitor = if monitor_response.status() == reqwest::StatusCode::NOT_FOUND {
                None
            } else {
                Some(
                    monitor_response
                        .error_for_status()?
                        .json::<SiteMonitor>()
                        .await?,
                )
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "site": site,
                    "revisions": revisions.revisions,
                    "health_checks": health.checks,
                    "deployments": deployments.deployments,
                    "monitor": monitor
                }))?
            );
        }
        SiteCommand::Register {
            display_name,
            project_id,
            public_url,
            health_url,
            hosting_provider,
            site_id,
            environment,
            deploy_executable,
            deploy_arguments,
            deploy_cwd,
            credential_reference,
            monitoring_paused,
            json,
        } => {
            let site_id = site_id
                .map(|value| SiteId::from_str(&value))
                .transpose()?
                .unwrap_or_else(SiteId::new);
            if deploy_executable.is_none() && !deploy_arguments.is_empty() {
                return Err("--deploy-arg requires --deploy-executable".into());
            }
            let deployment_command = deploy_executable.map(|executable| SiteDeploymentCommand {
                executable,
                arguments: deploy_arguments,
                working_directory: deploy_cwd,
            });
            let site: Site = client
                .post(format!("{host}/api/v1/sites"))
                .bearer_auth(token)
                .json(&CreateSiteRequest {
                    site_id,
                    project_id: ProjectId::from_str(&project_id)?,
                    display_name,
                    environment: environment.into(),
                    public_url,
                    health_url,
                    hosting_provider,
                    deployment_command,
                    credential_reference,
                    state: if monitoring_paused {
                        SiteState::MonitoringPaused
                    } else {
                        SiteState::Active
                    },
                })
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&site)?);
            } else {
                println!("{}: registered {}", site.id, site.public_url);
            }
        }
        SiteCommand::Health { id, check_id, json } => {
            let id = SiteId::from_str(&id)?;
            let check_id = check_id
                .map(|value| SiteHealthCheckId::from_str(&value))
                .transpose()?
                .unwrap_or_else(SiteHealthCheckId::new);
            let mutation: SiteHealthMutation = client
                .post(format!("{host}/api/v1/sites/{id}/health"))
                .bearer_auth(token)
                .json(&CheckSiteHealthRequest { check_id })
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&mutation)?);
            } else {
                println!(
                    "{}: {:?}{}",
                    mutation.check.id,
                    mutation.check.state,
                    if mutation.replayed { " (replay)" } else { "" }
                );
            }
        }
        SiteCommand::Monitor {
            id,
            interval_seconds,
            expected_status,
            schedule_id,
            json,
        } => {
            let id = SiteId::from_str(&id)?;
            let schedule_id = schedule_id
                .map(|value| ScheduleId::from_str(&value))
                .transpose()?
                .unwrap_or_else(ScheduleId::new);
            let mutation: SiteMonitorMutation = client
                .post(format!("{host}/api/v1/sites/{id}/monitor"))
                .bearer_auth(token)
                .json(&CreateSiteMonitorRequest {
                    schedule_id,
                    interval_seconds,
                    expected_status,
                    first_run_at: None,
                })
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&mutation)?);
            } else {
                println!(
                    "{}: every {} seconds, expecting HTTP {}{}",
                    mutation.monitor.schedule_id,
                    mutation.monitor.interval_seconds,
                    mutation.monitor.expected_status,
                    if mutation.replayed { " (replay)" } else { "" }
                );
            }
        }
        SiteCommand::Deploy {
            id,
            deployment_id,
            task_id,
            json,
        } => {
            let id = SiteId::from_str(&id)?;
            let deployment_id = deployment_id
                .map(|value| SiteDeploymentId::from_str(&value))
                .transpose()?
                .unwrap_or_else(SiteDeploymentId::new);
            let mutation: SiteDeploymentMutation = client
                .post(format!("{host}/api/v1/sites/{id}/deployments"))
                .bearer_auth(token)
                .json(&DeploySiteRequest {
                    deployment_id,
                    task_id: task_id.map(|value| TaskId::from_str(&value)).transpose()?,
                })
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&mutation)?);
            } else {
                println!(
                    "{}: {:?}{}",
                    mutation.deployment.id,
                    mutation.deployment.state,
                    if mutation.replayed { " (replay)" } else { "" }
                );
            }
        }
        SiteCommand::Update { id, request, json } => {
            let id = SiteId::from_str(&id)?;
            let request: UpdateSiteRequest =
                serde_json::from_reader(std::fs::File::open(request)?)?;
            let mutation = update_site_cli(client, host, token, id, &request).await?;
            print_site_mutation(&mutation, json)?;
        }
        SiteCommand::SetState {
            id,
            state,
            reason,
            expected_revision,
            json,
        } => {
            let id = SiteId::from_str(&id)?;
            let site: Site = client
                .get(format!("{host}/api/v1/sites/{id}"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let request = UpdateSiteRequest {
                expected_revision,
                display_name: site.display_name,
                environment: site.environment,
                public_url: site.public_url,
                health_url: site.health_url,
                hosting_provider: site.hosting_provider,
                deployment_command: site.deployment_command,
                credential_reference: site.credential_reference,
                state: state.into(),
                reason,
            };
            let mutation = update_site_cli(client, host, token, id, &request).await?;
            print_site_mutation(&mutation, json)?;
        }
    }
    Ok(())
}

async fn update_site_cli(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    site_id: SiteId,
    request: &UpdateSiteRequest,
) -> Result<SiteMutation, Box<dyn std::error::Error>> {
    Ok(client
        .put(format!("{host}/api/v1/sites/{site_id}"))
        .bearer_auth(token)
        .json(request)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

fn print_site_mutation(mutation: &SiteMutation, json: bool) -> Result<(), serde_json::Error> {
    if json {
        println!("{}", serde_json::to_string_pretty(mutation)?);
    } else {
        println!(
            "{}: revision {} {:?}{}",
            mutation.site.id,
            mutation.site.revision,
            mutation.site.state,
            if mutation.replayed { " (replay)" } else { "" }
        );
    }
    Ok(())
}

#[allow(clippy::too_many_lines)] // Keeps the small platform command-to-HTTP mapping together.
async fn run_platform(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    command: PlatformCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        PlatformCommand::Capabilities { json } => {
            let snapshot: PlatformCapabilitySnapshot = client
                .get(format!("{host}/api/v1/platform/capabilities"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&snapshot)?);
            } else {
                println!("{} / {}", snapshot.os, snapshot.architecture);
                for capability in snapshot.capabilities {
                    println!(
                        "{:?}: {:?} - {}",
                        capability.kind, capability.availability, capability.detail
                    );
                }
            }
        }
        PlatformCommand::List { json } => {
            let response: PlatformSessionListResponse = client
                .get(format!("{host}/api/v1/platform/sessions"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else {
                for session in response.sessions {
                    println!(
                        "{}: {} ({:?}, {:?})",
                        session.id, session.display_name, session.kind, session.state
                    );
                }
            }
        }
        PlatformCommand::RequestBrowser {
            display_name,
            session_id,
            project_id,
            task_id,
            origins,
            lifetime_seconds,
            json,
        } => {
            request_platform_session_cli(
                client,
                host,
                token,
                PlatformSessionKind::Browser,
                display_name,
                session_id,
                project_id,
                task_id,
                origins,
                Vec::new(),
                None,
                lifetime_seconds,
                json,
            )
            .await?;
        }
        PlatformCommand::RequestComputer {
            display_name,
            session_id,
            project_id,
            task_id,
            screen_only,
            display_id,
            region,
            lifetime_seconds,
            json,
        } => {
            request_platform_session_cli(
                client,
                host,
                token,
                PlatformSessionKind::ComputerUse,
                display_name,
                session_id,
                project_id,
                task_id,
                Vec::new(),
                if screen_only {
                    vec![ComputerUseCapability::ScreenRead]
                } else {
                    vec![
                        ComputerUseCapability::ScreenRead,
                        ComputerUseCapability::Mouse,
                        ComputerUseCapability::Keyboard,
                    ]
                },
                Some(ComputerUseTarget {
                    display_id,
                    region: region.as_deref().map(parse_computer_region).transpose()?,
                }),
                lifetime_seconds,
                json,
            )
            .await?;
        }
        PlatformCommand::Attach { id, visible, json } => {
            let id = PlatformSessionId::from_str(&id)?;
            let mutation: PlatformBrowserMutation = client
                .post(format!(
                    "{host}/api/v1/platform/sessions/{id}/browser/attach"
                ))
                .bearer_auth(token)
                .json(&AttachBrowserSessionRequest {
                    mode: if visible {
                        PersistentBrowserMode::Visible
                    } else {
                        PersistentBrowserMode::Headless
                    },
                })
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            print_browser_mutation(&mutation, json)?;
        }
        PlatformCommand::Navigate { id, url, json } => {
            let id = PlatformSessionId::from_str(&id)?;
            let observation: PersistentBrowserObservation = client
                .post(format!(
                    "{host}/api/v1/platform/sessions/{id}/browser/navigate"
                ))
                .bearer_auth(token)
                .json(&NavigateBrowserSessionRequest { url })
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&observation)?);
            } else {
                println!("{}: {}", observation.session_id, observation.safe_url);
            }
        }
        PlatformCommand::Handoff { id, json } => {
            let id = PlatformSessionId::from_str(&id)?;
            let mutation: PlatformBrowserMutation = client
                .post(format!(
                    "{host}/api/v1/platform/sessions/{id}/browser/handoff"
                ))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            print_browser_mutation(&mutation, json)?;
        }
        PlatformCommand::Reclaim { id, json } => {
            let id = PlatformSessionId::from_str(&id)?;
            let mutation: PlatformBrowserMutation = client
                .post(format!(
                    "{host}/api/v1/platform/sessions/{id}/browser/reclaim"
                ))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            print_browser_mutation(&mutation, json)?;
        }
        PlatformCommand::AttachComputer { id, json } => {
            let id = PlatformSessionId::from_str(&id)?;
            let mutation: PlatformComputerMutation = client
                .post(format!(
                    "{host}/api/v1/platform/sessions/{id}/computer/attach"
                ))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            print_computer_mutation(&mutation, json)?;
        }
        PlatformCommand::CaptureComputer { id, output, json } => {
            let id = PlatformSessionId::from_str(&id)?;
            let frame: ComputerUseFrame = client
                .post(format!(
                    "{host}/api/v1/platform/sessions/{id}/computer/capture"
                ))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            write_computer_frame(&frame, output.as_deref())?;
            if json {
                println!("{}", serde_json::to_string_pretty(&frame)?);
            } else {
                println!(
                    "{}: {}x{} frame {}",
                    id,
                    frame.observation.region.width,
                    frame.observation.region.height,
                    frame.sha256
                );
            }
        }
        PlatformCommand::ActComputer {
            id,
            request,
            output,
            json,
        } => {
            let id = PlatformSessionId::from_str(&id)?;
            let request: ApplyComputerActionsRequest =
                serde_json::from_slice(&std::fs::read(request)?)?;
            let result: PlatformComputerActionMutation = client
                .post(format!(
                    "{host}/api/v1/platform/sessions/{id}/computer/actions"
                ))
                .bearer_auth(token)
                .json(&request)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if let Some(execution) = &result.result {
                write_computer_frame(&execution.frame, output.as_deref())?;
            }
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!(
                    "{}: executed {} actions; frame {}",
                    id,
                    result.batch.action_count,
                    result
                        .result
                        .as_ref()
                        .map_or("replayed without a new frame", |value| value
                            .frame
                            .sha256
                            .as_str())
                );
            }
        }
        PlatformCommand::HandoffComputer { id, json } => {
            let id = PlatformSessionId::from_str(&id)?;
            let mutation: PlatformComputerMutation = client
                .post(format!(
                    "{host}/api/v1/platform/sessions/{id}/computer/handoff"
                ))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            print_computer_mutation(&mutation, json)?;
        }
        PlatformCommand::ReclaimComputer { id, json } => {
            let id = PlatformSessionId::from_str(&id)?;
            let mutation: PlatformComputerMutation = client
                .post(format!(
                    "{host}/api/v1/platform/sessions/{id}/computer/reclaim"
                ))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            print_computer_mutation(&mutation, json)?;
        }
        PlatformCommand::History { id } => {
            let id = PlatformSessionId::from_str(&id)?;
            let response: PlatformSessionHistoryResponse = client
                .get(format!("{host}/api/v1/platform/sessions/{id}/history"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        PlatformCommand::Close { id, reason } => {
            let id = PlatformSessionId::from_str(&id)?;
            let session: PlatformSession = client
                .post(format!("{host}/api/v1/platform/sessions/{id}/close"))
                .bearer_auth(token)
                .json(&mcg_api::ClosePlatformSessionRequest { reason })
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}: {:?}", session.id, session.state);
        }
    }
    Ok(())
}

fn print_browser_mutation(
    mutation: &PlatformBrowserMutation,
    json: bool,
) -> Result<(), serde_json::Error> {
    if json {
        println!("{}", serde_json::to_string_pretty(mutation)?);
    } else {
        println!(
            "{}: {:?} ({})",
            mutation.session.id, mutation.session.state, mutation.browser.safe_url
        );
    }
    Ok(())
}

fn print_computer_mutation(
    mutation: &PlatformComputerMutation,
    json: bool,
) -> Result<(), serde_json::Error> {
    if json {
        println!("{}", serde_json::to_string_pretty(mutation)?);
    } else {
        println!(
            "{}: {:?} (display {}, {}x{})",
            mutation.session.id,
            mutation.session.state,
            mutation.computer.display_id,
            mutation.computer.region.width,
            mutation.computer.region.height
        );
    }
    Ok(())
}

fn write_computer_frame(
    frame: &ComputerUseFrame,
    output: Option<&std::path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(output) = output {
        let bytes = STANDARD.decode(&frame.png_base64)?;
        std::fs::write(output, bytes)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn request_platform_session_cli(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    kind: PlatformSessionKind,
    display_name: String,
    session_id: Option<String>,
    project_id: Option<String>,
    task_id: Option<String>,
    allowed_origins: Vec<String>,
    computer_capabilities: Vec<ComputerUseCapability>,
    computer_target: Option<ComputerUseTarget>,
    lifetime_seconds: u32,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let session_id = match session_id {
        Some(value) => PlatformSessionId::from_str(&value)?,
        None => PlatformSessionId::new(),
    };
    let session: PlatformSession = client
        .post(format!("{host}/api/v1/platform/sessions"))
        .bearer_auth(token)
        .json(&CreatePlatformSessionRequest {
            session_id,
            kind,
            project_id: project_id
                .map(|value| ProjectId::from_str(&value))
                .transpose()?,
            task_id: task_id.map(|value| TaskId::from_str(&value)).transpose()?,
            display_name,
            allowed_origins,
            computer_capabilities,
            computer_target,
            lifetime_seconds,
        })
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&session)?);
    } else {
        println!("{}: requested (waiting for executor)", session.id);
    }
    Ok(())
}

fn parse_computer_region(value: &str) -> Result<ComputerUseRegion, Box<dyn std::error::Error>> {
    let values = value.split(',').map(str::trim).collect::<Vec<_>>();
    if values.len() != 4 {
        return Err("computer region must use x,y,width,height".into());
    }
    Ok(ComputerUseRegion {
        x: values[0].parse()?,
        y: values[1].parse()?,
        width: values[2].parse()?,
        height: values[3].parse()?,
    })
}

async fn run_schedule(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    command: ScheduleCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        creation @ (ScheduleCommand::CreateOnce { .. }
        | ScheduleCommand::CreateInterval { .. }
        | ScheduleCommand::CreateCron { .. }
        | ScheduleCommand::CreateWatch { .. }
        | ScheduleCommand::CreateEvent { .. }
        | ScheduleCommand::CreateHeartbeat { .. }) => {
            run_schedule_creation(client, host, token, creation).await?;
        }
        ScheduleCommand::List { json } => {
            list_schedules_cli(client, host, token, json).await?;
        }
        ScheduleCommand::History { id } => {
            let id = ScheduleId::from_str(&id)?;
            let response: ScheduleHistoryResponse = client
                .get(format!("{host}/api/v1/schedules/{id}/history"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        ScheduleCommand::Resume { id } => {
            set_schedule_state_cli(client, host, token, &id, ScheduleState::Enabled).await?;
        }
        ScheduleCommand::Pause { id } => {
            set_schedule_state_cli(client, host, token, &id, ScheduleState::Paused).await?;
        }
        ScheduleCommand::Remove { id } => {
            set_schedule_state_cli(client, host, token, &id, ScheduleState::Completed).await?;
        }
        ScheduleCommand::Wake {
            id,
            reason,
            severity,
        } => {
            let id = ScheduleId::from_str(&id)?;
            let mutation: mcg_runtime::ScheduleWakeMutation = client
                .post(format!("{host}/api/v1/schedules/{id}/wake"))
                .bearer_auth(token)
                .json(&WakeScheduleRequest {
                    severity: severity.into(),
                    reason,
                })
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}: {}", mutation.wake.id, mutation.wake.reason);
        }
        ScheduleCommand::Notifications { all, json } => {
            let response: SchedulerNotificationListResponse = client
                .get(format!(
                    "{host}/api/v1/scheduler/notifications?include_read={all}"
                ))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else {
                for notification in response.notifications {
                    println!(
                        "{}: {:?} — {}",
                        notification.id, notification.severity, notification.title
                    );
                }
            }
        }
        ScheduleCommand::Acknowledge { id } => {
            let id = SchedulerNotificationId::from_str(&id)?;
            let notification: SchedulerNotification = client
                .post(format!(
                    "{host}/api/v1/scheduler/notifications/{id}/acknowledge"
                ))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}: read={}", notification.id, notification.read);
        }
    }
    Ok(())
}

async fn run_schedule_creation(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    command: ScheduleCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    let (name, project_id, goal, trigger, schedule_id, json) = schedule_creation_parts(command)?;
    let schedule_id = match schedule_id {
        Some(value) => ScheduleId::from_str(&value)?,
        None => ScheduleId::new(),
    };
    create_schedule_cli(
        client,
        host,
        token,
        name,
        ProjectId::from_str(&project_id)?,
        goal,
        trigger,
        schedule_id,
        json,
    )
    .await
}

type ScheduleCreationParts = (
    String,
    String,
    String,
    ScheduleTrigger,
    Option<String>,
    bool,
);

#[allow(clippy::too_many_lines)] // Keeps the six public trigger shapes visibly symmetric.
fn schedule_creation_parts(
    command: ScheduleCommand,
) -> Result<ScheduleCreationParts, Box<dyn std::error::Error>> {
    let parts = match command {
        ScheduleCommand::CreateOnce {
            name,
            project_id,
            goal,
            at,
            schedule_id,
            json,
        } => (
            name,
            project_id,
            goal,
            ScheduleTrigger::Once {
                at: chrono::DateTime::parse_from_rfc3339(&at)?.with_timezone(&chrono::Utc),
            },
            schedule_id,
            json,
        ),
        ScheduleCommand::CreateInterval {
            name,
            project_id,
            goal,
            seconds,
            schedule_id,
            json,
        } => (
            name,
            project_id,
            goal,
            ScheduleTrigger::Interval { seconds },
            schedule_id,
            json,
        ),
        ScheduleCommand::CreateCron {
            name,
            project_id,
            goal,
            expression,
            schedule_id,
            json,
        } => (
            name,
            project_id,
            goal,
            ScheduleTrigger::Cron { expression },
            schedule_id,
            json,
        ),
        ScheduleCommand::CreateWatch {
            name,
            project_id,
            goal,
            relative_path,
            poll_seconds,
            schedule_id,
            json,
        } => (
            name,
            project_id,
            goal,
            ScheduleTrigger::FileWatch {
                relative_path,
                poll_seconds,
            },
            schedule_id,
            json,
        ),
        ScheduleCommand::CreateEvent {
            name,
            project_id,
            goal,
            event_kind,
            schedule_id,
            json,
        } => (
            name,
            project_id,
            goal,
            ScheduleTrigger::Event { event_kind },
            schedule_id,
            json,
        ),
        ScheduleCommand::CreateHeartbeat {
            name,
            project_id,
            goal,
            idle_seconds,
            busy_seconds,
            schedule_id,
            json,
        } => (
            name,
            project_id,
            goal,
            ScheduleTrigger::Heartbeat {
                idle_seconds,
                busy_seconds,
            },
            schedule_id,
            json,
        ),
        _ => return Err("schedule command is not a creation command".into()),
    };
    Ok(parts)
}

async fn list_schedules_cli(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let response: ScheduleListResponse = client
        .get(format!("{host}/api/v1/schedules"))
        .bearer_auth(token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        for schedule in response.schedules {
            println!("{}: {} ({:?})", schedule.id, schedule.name, schedule.state);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn create_schedule_cli(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    name: String,
    project_id: ProjectId,
    goal: String,
    trigger: ScheduleTrigger,
    schedule_id: ScheduleId,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let schedule: mcg_domain::ScheduleDefinition = client
        .post(format!("{host}/api/v1/schedules"))
        .bearer_auth(token)
        .json(&CreateScheduleRequest {
            schedule_id,
            name,
            project_id,
            goal,
            constraints: Vec::new(),
            success_conditions: Vec::new(),
            risk: None,
            budget: None,
            trigger,
            first_run_at: None,
            target_path: None,
            context_token_budget: 16_384,
            requested_tools: Vec::new(),
            estimated_output_tokens: 8_192,
            max_model_calls: 16,
        })
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&schedule)?);
    } else {
        println!("{}", schedule.id);
    }
    Ok(())
}

async fn set_schedule_state_cli(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    id: &str,
    state: ScheduleState,
) -> Result<(), Box<dyn std::error::Error>> {
    let id = ScheduleId::from_str(id)?;
    let schedule: mcg_domain::ScheduleDefinition = client
        .post(format!("{host}/api/v1/schedules/{id}"))
        .bearer_auth(token)
        .json(&SetScheduleStateRequest { state })
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    println!("{}: {:?}", schedule.id, schedule.state);
    Ok(())
}

async fn run_root_runtime(
    data_directory: &Path,
    command: &RuntimeCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    let status = mcg_platform::query_root_status(data_directory).await?;
    let (inspect, json) = match command {
        RuntimeCommand::Status { json } => (false, *json),
        RuntimeCommand::Inspect { json } => (true, *json),
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else if inspect {
        println!("Protected Root");
        println!("  process: {} ({})", status.service, status.process_id);
        println!(
            "  runtime identity: {}",
            status.runtime_identity.instance_id
        );
        println!("  protocol: v{}", status.protocol_version);
        println!("  IPC session: {}", status.session_id);
        println!("  storage schema: {}", status.storage_schema_version);
        println!("  writer queue capacity: {}", status.writer_queue_capacity);
        println!(
            "  executable: {:?} {}",
            status.runtime_identity.build_hash.algorithm,
            status.runtime_identity.build_hash.digest_hex
        );
        if let (Some(path), Some(digest)) = (
            status.migration_receipt.backup_path.as_deref(),
            status.migration_receipt.backup_sha256.as_deref(),
        ) {
            println!("  migration backup: {path}");
            println!("  migration backup SHA-256: {digest}");
        } else {
            println!("  migration backup: fresh installation; none required");
        }
        println!(
            "  migration: schema {} -> {} at {}",
            status.migration_receipt.from_storage_schema,
            status.migration_receipt.to_storage_schema,
            status.migration_receipt.migrated_at
        );
    } else {
        println!(
            "Protected Root ready · protocol v{} · storage schema {} · runtime {}",
            status.protocol_version,
            status.storage_schema_version,
            status.runtime_identity.instance_id
        );
    }
    Ok(())
}

async fn run_root_workflow(
    data_directory: &Path,
    command: &WorkflowCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        WorkflowCommand::List { json } => {
            let attempts = mcg_platform::query_root_workflows(data_directory).await?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&attempts)?);
            } else if attempts.is_empty() {
                println!("No canonical workflows.");
            } else {
                for attempt in attempts {
                    println!(
                        "{}  {:?}  attempt {}  revision {}  generation {}",
                        attempt.workflow_id,
                        attempt.state,
                        attempt.attempt_number,
                        attempt.revision,
                        attempt.lease_generation
                    );
                }
            }
        }
        WorkflowCommand::Show { id, json } => {
            let workflow_id = WorkflowId(uuid::Uuid::parse_str(id)?);
            let inspection = mcg_platform::query_root_workflow(data_directory, workflow_id)
                .await?
                .ok_or_else(|| format!("canonical workflow {workflow_id} was not found"))?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&inspection)?);
            } else {
                println!("Workflow {}", inspection.attempt.workflow_id);
                println!("  objective: {}", inspection.task_contract.objective);
                println!("  state: {:?}", inspection.attempt.state);
                println!("  attempt: {}", inspection.attempt.attempt_number);
                println!("  revision: {}", inspection.attempt.revision);
                println!("  generation: {}", inspection.attempt.lease_generation);
                println!("  events: {}", inspection.events.len());
            }
        }
        WorkflowCommand::Activity {
            id,
            invocation_limit,
            json,
        } => {
            let workflow_id = WorkflowId(uuid::Uuid::parse_str(id)?);
            let activity = mcg_platform::query_root_workflow_activity(
                data_directory,
                workflow_id,
                *invocation_limit,
            )
            .await?
            .ok_or_else(|| format!("canonical workflow {workflow_id} was not found"))?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&activity)?);
            } else {
                println!(
                    "Workflow {} activity",
                    activity.workflow.attempt.workflow_id
                );
                println!("  objective: {}", activity.workflow.task_contract.objective);
                println!("  state: {:?}", activity.workflow.attempt.state);
                println!("  attempt: {}", activity.workflow.attempt.attempt_number);
                println!(
                    "  invocations: {}{}",
                    activity.invocations.len(),
                    if activity.invocations_truncated {
                        " (bounded window)"
                    } else {
                        ""
                    }
                );
                for invocation in activity.invocations {
                    println!(
                        "  {:?}  {}  {:?}",
                        invocation.role, invocation.invocation_id, invocation.model_run_state
                    );
                }
            }
        }
        WorkflowCommand::Cancel {
            id,
            idempotency_key,
            json,
        } => {
            let workflow_id = WorkflowId(uuid::Uuid::parse_str(id)?);
            let inspection = mcg_platform::query_root_workflow(data_directory, workflow_id)
                .await?
                .ok_or_else(|| format!("canonical workflow {workflow_id} was not found"))?;
            if matches!(
                inspection.attempt.state,
                WorkflowState::Ready
                    | WorkflowState::CancelRequested
                    | WorkflowState::Succeeded
                    | WorkflowState::Failed
                    | WorkflowState::Cancelled
                    | WorkflowState::Quarantined
            ) {
                return Err(format!(
                    "workflow {workflow_id} cannot request cancellation from {:?}",
                    inspection.attempt.state
                )
                .into());
            }
            let request = WorkflowCancelRequest {
                attempt_id: inspection.attempt.attempt_id,
                expected_revision: inspection.attempt.revision,
                expected_generation: inspection.attempt.lease_generation,
                idempotency_key: idempotency_key
                    .clone()
                    .unwrap_or_else(|| format!("cli-cancel-{}", uuid::Uuid::new_v4())),
            };
            let cancelled = mcg_platform::cancel_root_workflow(data_directory, &request).await?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&cancelled)?);
            } else {
                println!(
                    "Workflow {} cancellation requested at revision {} (generation {}).",
                    cancelled.workflow_id, cancelled.revision, cancelled.lease_generation
                );
            }
        }
    }
    Ok(())
}

async fn run_root_action(
    data_directory: &Path,
    command: &ActionCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        ActionCommand::List { limit, json } => {
            let actions = mcg_platform::query_root_actions(data_directory, *limit).await?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&actions)?);
            } else if actions.is_empty() {
                println!("No canonical actions.");
            } else {
                for action in actions {
                    println!(
                        "{}  {:?}  {:?}  revision {}  generation {}",
                        action.action_id,
                        action.effect_class,
                        action.state,
                        action.revision,
                        action.lease_generation
                    );
                }
            }
        }
        ActionCommand::Show {
            id,
            event_limit,
            json,
        } => {
            let action_id = ActionId(uuid::Uuid::parse_str(id)?);
            let inspection =
                mcg_platform::query_root_action(data_directory, action_id, *event_limit)
                    .await?
                    .ok_or_else(|| format!("canonical action {action_id} was not found"))?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&inspection)?);
            } else {
                println!("Action {}", inspection.envelope.id);
                println!("  workflow: {}", inspection.envelope.workflow_id);
                println!("  attempt: {}", inspection.envelope.attempt_id);
                println!("  actor: {}", inspection.envelope.actor);
                println!("  effect: {:?}", inspection.envelope.effect_class);
                if let Some(origin) = &inspection.envelope.origin {
                    for line in canonical_action_origin_lines(origin) {
                        println!("{line}");
                    }
                }
                println!(
                    "  operation: {} ({}:{})",
                    inspection.envelope.operation.type_id.as_str(),
                    inspection.envelope.operation_hash.domain,
                    inspection.envelope.operation_hash.digest_hex
                );
                if let Some(execution) = inspection.executions.last() {
                    println!("  state: {:?}", execution.state);
                    println!("  revision: {}", execution.revision);
                    println!("  generation: {}", execution.lease_generation);
                }
                println!("  executions: {}", inspection.executions.len());
                println!("  receipts: {}", inspection.receipts.len());
                println!(
                    "  reconciliation jobs: {}",
                    inspection.reconciliation_jobs.len()
                );
                println!("  events returned: {}", inspection.events.len());
            }
        }
        ActionCommand::Compensate {
            id,
            reason,
            confirm,
            idempotency_key,
            json,
        } => {
            if !confirm {
                return Err("action compensation requires --confirm".into());
            }
            let action_id = ActionId(uuid::Uuid::parse_str(id)?);
            let inspection = mcg_platform::query_root_action(data_directory, action_id, 500)
                .await?
                .ok_or_else(|| format!("canonical action {action_id} was not found"))?;
            let execution = inspection
                .executions
                .last()
                .ok_or_else(|| format!("canonical action {action_id} has no execution"))?;
            let workflow =
                mcg_platform::query_root_workflow(data_directory, inspection.envelope.workflow_id)
                    .await?
                    .ok_or_else(|| {
                        format!(
                            "source workflow {} was not found",
                            inspection.envelope.workflow_id
                        )
                    })?;
            let request = OwnerActionCompensationRequest {
                action_id,
                execution_id: execution.id,
                expected_action_revision: execution.revision,
                expected_action_generation: execution.lease_generation,
                expected_workflow_revision: workflow.attempt.revision,
                expected_workflow_generation: workflow.attempt.lease_generation,
                reason: reason.clone(),
                confirmed: true,
                idempotency_key: idempotency_key
                    .clone()
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            };
            let receipt = mcg_platform::compensate_root_action(data_directory, &request).await?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&receipt)?);
            } else {
                println!(
                    "Compensation {} for action {} is {:?}{}.",
                    receipt.compensation.envelope.id,
                    receipt.original_action_id,
                    receipt
                        .compensation
                        .executions
                        .last()
                        .map(|execution| execution.state)
                        .ok_or("compensation receipt has no execution")?,
                    if receipt.replayed {
                        " (exact replay; no redispatch)"
                    } else {
                        ""
                    }
                );
            }
        }
    }
    Ok(())
}

async fn run_root_invocation(
    data_directory: &Path,
    command: &InvocationCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        InvocationCommand::List { limit, json } => {
            let invocations = mcg_platform::query_root_invocations(data_directory, *limit).await?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&invocations)?);
            } else if invocations.is_empty() {
                println!("No reconstructable invocations.");
            } else {
                for invocation in invocations {
                    println!(
                        "{}  {:?}  {}  revision {}  workflow {}",
                        invocation.invocation_id,
                        invocation.role,
                        invocation
                            .model_run_state
                            .map_or_else(|| "unplanned".to_owned(), |state| format!("{state:?}")),
                        invocation.model_run_revision.unwrap_or_default(),
                        invocation.workflow_id
                    );
                }
            }
        }
        InvocationCommand::Show {
            id,
            model_run_limit,
            json,
        } => {
            let invocation_id = InvocationId(uuid::Uuid::parse_str(id)?);
            let inspection = mcg_platform::query_root_invocation(
                data_directory,
                invocation_id,
                *model_run_limit,
            )
            .await?
            .ok_or_else(|| format!("invocation {invocation_id} was not found"))?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&inspection)?);
            } else {
                println!("Invocation {}", inspection.manifest.id);
                println!("  role: {:?}", inspection.manifest.role);
                println!("  attempt: {}", inspection.manifest.attempt_id);
                println!(
                    "  task contract: {} v{}",
                    inspection.manifest.task_contract.id, inspection.manifest.task_contract.version
                );
                println!(
                    "  runtime: {}",
                    inspection.manifest.runtime_identity.instance_id
                );
                println!(
                    "  provider: {} {}",
                    inspection.manifest.resolved_provider.source_type.as_str(),
                    inspection.manifest.resolved_provider.locator
                );
                println!(
                    "  model: {} {}",
                    inspection.manifest.resolved_model.source_type.as_str(),
                    inspection.manifest.resolved_model.locator
                );
                println!(
                    "  context: {} ({}:{})",
                    inspection.context_manifest.id,
                    inspection.context_manifest_hash.domain,
                    inspection.context_manifest_hash.digest_hex
                );
                println!(
                    "  request: {}:{}",
                    inspection.manifest.request_hash.domain,
                    inspection.manifest.request_hash.digest_hex
                );
                if let Some(response) = &inspection.manifest.response_hash {
                    println!("  response: {}:{}", response.domain, response.digest_hex);
                }
                for run in &inspection.model_runs {
                    println!(
                        "  run {} revision {}: {:?} at {}",
                        run.run.id, run.revision, run.run.state, run.recorded_at
                    );
                }
                if inspection.model_runs_truncated {
                    println!("  model-run history truncated at {model_run_limit} revisions");
                }
            }
        }
    }
    Ok(())
}

fn canonical_action_origin_lines(origin: &mcg_contracts::ActionOrigin) -> Vec<String> {
    let named = [
        ("conversation", origin.conversation.as_ref()),
        ("invocation", origin.invocation.as_ref()),
        ("command", origin.command.as_ref()),
        ("approval", origin.approval.as_ref()),
        ("agent", origin.agent.as_ref()),
    ];
    named
        .into_iter()
        .filter_map(|(label, source)| source.map(|source| (label, source)))
        .chain(origin.evidence.iter().map(|source| ("evidence", source)))
        .map(|(label, source)| {
            format!(
                "  source {label}: {} {} ({}:{})",
                source.source_type.as_str(),
                source.locator,
                source.content_hash.domain,
                source.content_hash.digest_hex
            )
        })
        .collect()
}

async fn run_status(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let status: RuntimeStatus = client
        .get(format!("{host}/api/v1/system/status"))
        .bearer_auth(token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        println!("{} {} ({:?})", status.service, status.version, status.state);
        println!(
            "automatic shadow experiments: {}",
            if status.shadow_experiments {
                "enabled"
            } else {
                "disabled"
            }
        );
    }
    Ok(())
}

fn daemon_bind_address(host: &str) -> Result<String, Box<dyn std::error::Error>> {
    let url = reqwest::Url::parse(host)?;
    let host_name = url
        .host_str()
        .ok_or("daemon host must include a host name")?;
    if !matches!(host_name, "127.0.0.1" | "localhost" | "[::1]" | "::1") {
        return Err("the CLI may start only a loopback MCG daemon".into());
    }
    let port = url
        .port_or_known_default()
        .ok_or("daemon host must include a port")?;
    Ok(format!("127.0.0.1:{port}"))
}

fn sibling_daemon_executable() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let current = std::env::current_exe()?;
    let name = format!("mcg-daemon{}", std::env::consts::EXE_SUFFIX);
    let daemon = current
        .parent()
        .ok_or("cannot resolve the CLI executable directory")?
        .join(name);
    if !daemon.is_file() {
        return Err(format!(
            "the bundled daemon is missing at {}; install mcg and mcg-daemon together",
            daemon.display()
        )
        .into());
    }
    Ok(daemon)
}

async fn daemon_status_if_ready(
    client: &reqwest::Client,
    host: &str,
    token_path: &std::path::Path,
) -> Option<RuntimeStatus> {
    let token = std::fs::read_to_string(token_path).ok()?;
    client
        .get(format!("{host}/api/v1/system/status"))
        .bearer_auth(token.trim())
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json()
        .await
        .ok()
}

async fn run_start(
    host: &str,
    token_path: &std::path::Path,
    timeout_seconds: u64,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    if let Some(status) = daemon_status_if_ready(&client, host, token_path).await {
        if json {
            println!(
                "{}",
                serde_json::json!({"started": false, "already_running": true, "status": status})
            );
        } else {
            println!("MCG daemon is already ready at {host}.");
        }
        return Ok(());
    }

    let daemon = sibling_daemon_executable()?;
    let bind_address = daemon_bind_address(host)?;
    let data_directory = token_path
        .parent()
        .ok_or("daemon token path has no parent data directory")?;
    std::fs::create_dir_all(data_directory)?;
    let log_path = data_directory.join("mcg-daemon.log");
    let stdout_log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let stderr_log = stdout_log.try_clone()?;
    let mut command = ProcessCommand::new(&daemon);
    command
        .env("MCG_BIND_ADDRESS", bind_address)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout_log))
        .stderr(Stdio::from(stderr_log));
    #[cfg(windows)]
    command.creation_flags(0x0800_0000 | 0x0000_0008);
    let child = command.spawn()?;
    let pid = child.id();
    let deadline =
        tokio::time::Instant::now() + std::time::Duration::from_secs(timeout_seconds.clamp(1, 300));
    loop {
        if let Some(status) = daemon_status_if_ready(&client, host, token_path).await {
            if json {
                println!(
                    "{}",
                    serde_json::json!({"started": true, "already_running": false, "pid": pid, "log_path": log_path, "status": status})
                );
            } else {
                println!("MCG daemon is ready at {host} (pid {pid}).");
            }
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(format!(
                "MCG daemon process {pid} did not become ready at {host} within {timeout_seconds} seconds; inspect {} or run {} directly for diagnostics",
                log_path.display(),
                daemon.display(),
            )
            .into());
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
}

async fn wait_for_daemon_stop(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    timeout_seconds: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let deadline =
        tokio::time::Instant::now() + std::time::Duration::from_secs(timeout_seconds.clamp(1, 300));
    loop {
        let stopped = client
            .get(format!("{host}/api/v1/system/status"))
            .bearer_auth(token)
            .send()
            .await
            .is_err();
        if stopped {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(format!("MCG daemon did not stop within {timeout_seconds} seconds").into());
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
}

async fn run_stop(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let response = request_stop(client, host, token).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        println!("MCG daemon is draining and will stop cleanly.");
    }
    Ok(())
}

async fn request_stop(
    client: &reqwest::Client,
    host: &str,
    token: &str,
) -> Result<ShutdownResponse, Box<dyn std::error::Error>> {
    Ok(client
        .post(format!("{host}/api/v1/system/shutdown"))
        .bearer_auth(token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

async fn run_doctor(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    json: bool,
    strict: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let report: DiagnosticReport = client
        .get(format!("{host}/api/v1/system/diagnostics"))
        .bearer_auth(token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "MCG diagnostics: {}",
            diagnostic_severity_name(report.overall)
        );
        for check in &report.checks {
            println!(
                "[{}] {} - {}",
                diagnostic_severity_name(check.severity),
                check.label,
                check.detail
            );
            if let Some(remediation) = &check.remediation {
                println!("  Remediation: {remediation}");
            }
        }
    }
    let failed = report.overall == DiagnosticSeverity::Error
        || (strict && report.overall == DiagnosticSeverity::Warning);
    if failed {
        return Err(format!(
            "diagnostics completed with {}",
            diagnostic_severity_name(report.overall).to_ascii_lowercase()
        )
        .into());
    }
    Ok(())
}

const fn diagnostic_severity_name(severity: DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::Pass => "PASS",
        DiagnosticSeverity::Warning => "WARNING",
        DiagnosticSeverity::Error => "ERROR",
    }
}

async fn run_settings(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    command: SettingsCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        SettingsCommand::Show { json } => {
            let settings = load_user_settings(client, host, token).await?;
            print_user_settings(&settings, json)?;
        }
        SettingsCommand::Set {
            interface_complexity,
            advanced_mode,
            autonomy,
            shadow_experiments,
            reason,
        } => {
            if interface_complexity.is_none()
                && advanced_mode.is_none()
                && autonomy.is_none()
                && shadow_experiments.is_none()
            {
                return Err("settings set requires at least one changed field".into());
            }
            let current = load_user_settings(client, host, token).await?;
            let updated: UserSettings = client
                .put(format!("{host}/api/v1/settings"))
                .bearer_auth(token)
                .json(&UpdateUserSettingsRequest {
                    expected_revision: current.revision,
                    interface_complexity: Some(interface_complexity.map_or_else(
                        || {
                            advanced_mode.map_or(current.interface_complexity, |enabled| {
                                if enabled {
                                    InterfaceComplexity::Advanced
                                } else {
                                    InterfaceComplexity::Simple
                                }
                            })
                        },
                        Into::into,
                    )),
                    advanced_mode: false,
                    autonomy: autonomy.map_or(current.autonomy, Into::into),
                    shadow_experiments: shadow_experiments.unwrap_or(current.shadow_experiments),
                    reason,
                })
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            print_user_settings(&updated, false)?;
        }
        SettingsCommand::History { json } => {
            let history: UserSettingsHistoryResponse = client
                .get(format!("{host}/api/v1/settings/history"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&history)?);
            } else {
                for revision in history.revisions {
                    let settings = revision.settings;
                    println!(
                        "revision {}  {:?}  {}  shadow={}  {}  {}",
                        settings.revision,
                        settings.autonomy,
                        format!("{:?}", settings.interface_complexity).to_lowercase(),
                        settings.shadow_experiments,
                        revision.changed_at,
                        revision.reason,
                    );
                }
            }
        }
    }
    Ok(())
}

async fn run_backup(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    command: BackupCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        BackupCommand::Create {
            backup_id,
            include_secrets,
            passphrase_stdin,
            json,
        } => {
            let backup_id = backup_id.unwrap_or_else(uuid::Uuid::new_v4);
            let request = if include_secrets {
                let mut passphrase =
                    read_backup_passphrase("Encrypted backup passphrase: ", passphrase_stdin)?;
                let mut confirmation = read_backup_passphrase(
                    "Confirm encrypted backup passphrase: ",
                    passphrase_stdin,
                )?;
                if passphrase != confirmation {
                    passphrase.zeroize();
                    confirmation.zeroize();
                    return Err("encrypted backup passphrases do not match".into());
                }
                confirmation.zeroize();
                Some(CreateEncryptedBackupRequest { passphrase })
            } else {
                None
            };
            let endpoint = if include_secrets {
                format!("{host}/api/v1/backups/{backup_id}/encrypted")
            } else {
                format!("{host}/api/v1/backups/{backup_id}")
            };
            let builder = client.post(endpoint).bearer_auth(token);
            let builder = if let Some(request) = request.as_ref() {
                builder.json(request)
            } else {
                builder
            };
            let summary: BackupSummaryResponse =
                builder.send().await?.error_for_status()?.json().await?;
            print_backup_summary(&summary, json)?;
        }
        BackupCommand::List { json } => {
            let response: BackupListResponse = client
                .get(format!("{host}/api/v1/backups"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else if response.backups.is_empty() {
                println!("No portable backups have been created.");
            } else {
                for backup in response.backups {
                    print_backup_summary(&backup, false)?;
                }
            }
        }
        BackupCommand::Verify { .. } | BackupCommand::Restore { .. } => {
            unreachable!("offline backup commands return before daemon authentication")
        }
    }
    Ok(())
}

fn read_backup_passphrase(
    prompt: &str,
    passphrase_stdin: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    use std::io::{BufRead as _, Read as _};

    if !passphrase_stdin {
        return Ok(rpassword::prompt_password(prompt)?);
    }
    if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        return Err("--passphrase-stdin requires redirected standard input".into());
    }
    let stdin = std::io::stdin();
    let mut limited = stdin.lock().take(1_026);
    let mut bytes = Vec::new();
    limited.read_until(b'\n', &mut bytes)?;
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
    }
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    if bytes.len() > 1_024 {
        bytes.zeroize();
        return Err("encrypted backup passphrase exceeds 1024 bytes".into());
    }
    let passphrase = if let Ok(value) = std::str::from_utf8(&bytes) {
        value.to_owned()
    } else {
        bytes.zeroize();
        return Err("encrypted backup passphrase must be UTF-8".into());
    };
    bytes.zeroize();
    Ok(passphrase)
}

fn print_backup_summary(
    summary: &BackupSummaryResponse,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if json {
        println!("{}", serde_json::to_string_pretty(summary)?);
    } else {
        println!(
            "{}  {} bytes  sha256={}  created={}  secrets=excluded",
            summary.file_name, summary.size_bytes, summary.sha256, summary.manifest.created_at
        );
    }
    Ok(())
}

fn print_backup_bundle(
    summary: &mcg_storage::BackupBundleSummary,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if json {
        println!("{}", serde_json::to_string_pretty(summary)?);
    } else {
        println!(
            "{} is valid: {} bytes, {} entries, sha256={}, secrets excluded",
            summary.path.display(),
            summary.size_bytes,
            summary.manifest.entries.len(),
            summary.sha256
        );
    }
    Ok(())
}

async fn load_user_settings(
    client: &reqwest::Client,
    host: &str,
    token: &str,
) -> Result<UserSettings, Box<dyn std::error::Error>> {
    Ok(client
        .get(format!("{host}/api/v1/settings"))
        .bearer_auth(token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

fn print_user_settings(
    settings: &UserSettings,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if json {
        println!("{}", serde_json::to_string_pretty(settings)?);
    } else {
        println!(
            "revision {}: {:?}, {} mode, shadow experiments {}",
            settings.revision,
            settings.autonomy,
            format!("{:?}", settings.interface_complexity).to_lowercase(),
            if settings.shadow_experiments {
                "enabled"
            } else {
                "disabled"
            }
        );
    }
    Ok(())
}

#[allow(clippy::too_many_lines)] // Keep task command-to-HTTP routing in one exhaustive match.
async fn run_task(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    command: TaskCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        TaskCommand::Create {
            goal,
            project_id,
            json,
        } => {
            let project_id = project_id.as_deref().map(ProjectId::from_str).transpose()?;
            let created: TaskMutationResponse = client
                .post(format!("{host}/api/v1/tasks"))
                .bearer_auth(token)
                .json(&CreateTaskRequest {
                    goal,
                    project_id,
                    thread_id: None,
                    constraints: Vec::new(),
                    success_conditions: Vec::new(),
                    risk: None,
                    budget: None,
                })
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&created)?);
            } else {
                println!("{}", created.task.id);
            }
        }
        TaskCommand::Launch { id, json } => {
            run_consumer_task(client, host, token, &id, "launch", json).await?;
        }
        TaskCommand::Retry { id, json } => {
            run_consumer_task(client, host, token, &id, "retry", json).await?;
        }
        TaskCommand::Resume { id, json } => {
            run_consumer_task(client, host, token, &id, "resume", json).await?;
        }
        TaskCommand::List { json } => {
            list_tasks(client, host, token, json).await?;
        }
        TaskCommand::Get { id, json } => {
            let task_id = TaskId::from_str(&id)?;
            let task: TaskEnvelope = client
                .get(format!("{host}/api/v1/tasks/{task_id}"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&task)?);
            } else {
                println!("{}: {} ({})", task.id, task.goal, task.state);
            }
        }
        TaskCommand::Events { id } => {
            let task_id = TaskId::from_str(&id)?;
            let events: TaskEventsResponse = client
                .get(format!("{host}/api/v1/tasks/{task_id}/events"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&events)?);
        }
        TaskCommand::Audit { id } => {
            let task_id = TaskId::from_str(&id)?;
            let audit: ToolAuditResponse = client
                .get(format!("{host}/api/v1/tasks/{task_id}/tool-audit"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&audit)?);
        }
        TaskCommand::Transition { id, state } => {
            let task_id = TaskId::from_str(&id)?;
            let task: TaskEnvelope = client
                .post(format!("{host}/api/v1/tasks/{task_id}"))
                .bearer_auth(token)
                .json(&TransitionTaskRequest {
                    state: state.into(),
                })
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}: {}", task.id, task.state);
        }
    }
    Ok(())
}

async fn run_consumer_task(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    id: &str,
    action: &str,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let task_id = TaskId::from_str(id)?;
    let result: LaunchTaskResponse = client
        .post(format!("{host}/api/v1/tasks/{task_id}/{action}"))
        .bearer_auth(token)
        .json(&default_launch_request())
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!(
            "{}: {:?} ({:?})",
            result.run.run.id, result.execution, result.run.run.state
        );
    }
    Ok(())
}

async fn list_tasks(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let response: TaskListResponse = client
        .get(format!("{host}/api/v1/tasks"))
        .bearer_auth(token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else if response.tasks.is_empty() {
        println!("No tasks.");
    } else {
        for task in response.tasks {
            println!("{}  {}  {}", task.id, task.state, task.goal);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_lines)] // One exhaustive CLI command adapter keeps every organization operation visibly aligned with its REST route.
async fn run_organization(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    command: OrganizationCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        OrganizationCommand::Default { name, json } => {
            let mutation: OrganizationMutationResponse = client
                .post(format!("{host}/api/v1/organizations"))
                .bearer_auth(token)
                .json(&CreateOrganizationRequest { name })
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&mutation)?);
            } else {
                println!(
                    "{}  revision {}  {}",
                    mutation.organization.organization_id,
                    mutation.organization.revision_id,
                    mutation.organization.name
                );
            }
        }
        OrganizationCommand::List { json } => {
            let response: OrganizationListResponse = client
                .get(format!("{host}/api/v1/organizations"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else if response.organizations.is_empty() {
                println!("No organizations.");
            } else {
                for organization in response.organizations {
                    println!(
                        "{}  revision {}  {}",
                        organization.organization_id, organization.revision, organization.name
                    );
                }
            }
        }
        OrganizationCommand::Get { id } => {
            let response: mcg_agents::OrganizationGenome = client
                .get(format!("{host}/api/v1/organizations/{id}"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        OrganizationCommand::History {
            organization_id,
            json,
        } => {
            let organization_id = OrganizationId::from_str(&organization_id)?;
            let response: OrganizationHistoryResponse = client
                .get(format!(
                    "{host}/api/v1/organizations/{organization_id}/history"
                ))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else {
                for revision in response.revisions {
                    println!(
                        "{}  revision {}  {}",
                        revision.revision_id, revision.revision, revision.name
                    );
                }
            }
        }
        OrganizationCommand::Inherit {
            base_id,
            name,
            json,
        } => {
            let mutation: OrganizationMutationResponse = client
                .post(format!("{host}/api/v1/organizations/{base_id}/inherit"))
                .bearer_auth(token)
                .json(&InheritOrganizationRequest { name })
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&mutation)?);
            } else {
                println!(
                    "{}  inherits {}  {}",
                    mutation.organization.organization_id,
                    mutation
                        .organization
                        .inherited_from_revision_id
                        .map_or_else(|| "none".into(), |id| id.to_string()),
                    mutation.organization.name
                );
            }
        }
        OrganizationCommand::Export {
            revision_id,
            output,
        } => {
            let revision_id = OrganizationRevisionId::from_str(&revision_id)?;
            let portable: PortableOrganizationGenome = client
                .get(format!(
                    "{host}/api/v1/organization-revisions/{revision_id}/export"
                ))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let encoded = serde_json::to_vec_pretty(&portable)?;
            if let Some(output) = output {
                std::fs::write(&output, encoded)?;
                println!(
                    "Exported {} to {}",
                    portable.content_sha256,
                    output.display()
                );
            } else {
                println!("{}", String::from_utf8(encoded)?);
            }
        }
        OrganizationCommand::Import {
            file,
            import_id,
            name,
            json,
        } => {
            let portable: PortableOrganizationGenome =
                serde_json::from_reader(std::fs::File::open(file)?)?;
            let generated_import_id = import_id.is_none();
            let import_id = import_id
                .as_deref()
                .map(OrganizationImportId::from_str)
                .transpose()?
                .unwrap_or_default();
            if generated_import_id {
                eprintln!(
                    "Import identity: {import_id} (reuse with --import-id if this request must be retried)"
                );
            }
            let mutation: PortableOrganizationImportMutation = client
                .post(format!("{host}/api/v1/organizations/import"))
                .bearer_auth(token)
                .json(&ImportOrganizationRequest {
                    import_id,
                    portable,
                    name,
                })
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&mutation)?);
            } else {
                println!(
                    "{}  revision {}  import {}  {}",
                    mutation.organization.organization_id,
                    mutation.organization.revision_id,
                    import_id,
                    if mutation.replayed {
                        "replayed"
                    } else {
                        "created"
                    }
                );
            }
        }
        OrganizationCommand::Compare {
            left_revision_id,
            right_revision_id,
            json,
        } => {
            let left = OrganizationRevisionId::from_str(&left_revision_id)?;
            let right = OrganizationRevisionId::from_str(&right_revision_id)?;
            let comparison: PortableOrganizationComparison = client
                .get(format!(
                    "{host}/api/v1/organization-revisions/{left}/compare/{right}"
                ))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&comparison)?);
            } else if comparison.equivalent {
                println!("Equivalent portable organization definitions.");
            } else {
                println!(
                    "Different: {} role changes, {} relationship changes, changed sections={}",
                    comparison.roles_added.len()
                        + comparison.roles_removed.len()
                        + comparison.roles_changed.len(),
                    comparison.relationships_added.len() + comparison.relationships_removed.len(),
                    comparison
                        .sections_changed
                        .iter()
                        .map(|section| format!("{section:?}").to_ascii_lowercase())
                        .collect::<Vec<_>>()
                        .join(",")
                );
            }
        }
        OrganizationCommand::Publishers { json } => {
            let response: OrganizationPublisherListResponse = client
                .get(format!("{host}/api/v1/organization-publishers"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else if response.publishers.is_empty() {
                println!("No trusted organization publishers.");
            } else {
                for publisher in response.publishers {
                    println!(
                        "{}  revision {}  {:?}  key={}  registries={}",
                        publisher.id,
                        publisher.revision,
                        publisher.state,
                        publisher.public_key_sha256()?,
                        publisher.registries.join(",")
                    );
                }
            }
        }
        OrganizationCommand::TrustPublisher {
            publisher_id,
            display_name,
            public_key_base64,
            registries,
            expected_revision,
            reason,
            json,
        } => {
            let mutation: OrganizationPublisherMutation = client
                .post(format!("{host}/api/v1/organization-publishers"))
                .bearer_auth(token)
                .json(&TrustOrganizationPublisherRequest {
                    id: publisher_id,
                    display_name,
                    public_key_base64,
                    registries,
                    expected_revision,
                    reason,
                })
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&mutation)?);
            } else {
                println!(
                    "{}  revision {}  {:?}  key={}  {}",
                    mutation.publisher.id,
                    mutation.publisher.revision,
                    mutation.publisher.state,
                    mutation.publisher.public_key_sha256()?,
                    if mutation.replayed {
                        "replayed"
                    } else {
                        "recorded"
                    }
                );
            }
        }
        OrganizationCommand::RevokePublisher {
            publisher_id,
            expected_revision,
            reason,
            json,
        } => {
            let mutation: OrganizationPublisherMutation = client
                .post(format!(
                    "{host}/api/v1/organization-publishers/{publisher_id}/revoke"
                ))
                .bearer_auth(token)
                .json(&RevokeOrganizationPublisherRequest {
                    expected_revision,
                    reason,
                })
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&mutation)?);
            } else {
                println!(
                    "{}  revision {}  revoked  {}",
                    mutation.publisher.id,
                    mutation.publisher.revision,
                    if mutation.replayed {
                        "replayed"
                    } else {
                        "recorded"
                    }
                );
            }
        }
        OrganizationCommand::ShareReviews { json } => {
            let response: OrganizationShareReviewListResponse = client
                .get(format!("{host}/api/v1/organization-share-reviews"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else if response.reviews.is_empty() {
                println!("No organization share reviews.");
            } else {
                for review in response.reviews {
                    println!(
                        "{}  {:?}  {}/{}  publisher={}  package_sha256={}",
                        review.id,
                        review.state,
                        review.package.registry,
                        review.package.package,
                        review.evidence.publisher,
                        review.evidence.package_sha256
                    );
                }
            }
        }
        OrganizationCommand::ReviewShare {
            file,
            review_id,
            json,
        } => {
            let package: OrganizationSharePackage =
                serde_json::from_reader(std::fs::File::open(file)?)?;
            let generated_review_id = review_id.is_none();
            let review_id = review_id
                .as_deref()
                .map(OrganizationShareReviewId::from_str)
                .transpose()?
                .unwrap_or_default();
            if generated_review_id {
                eprintln!(
                    "Review identity: {review_id} (reuse with --review-id if this request must be retried)"
                );
            }
            let mutation: OrganizationShareReviewMutation = client
                .post(format!("{host}/api/v1/organization-share-reviews"))
                .bearer_auth(token)
                .json(&ReviewSharedOrganizationRequest { review_id, package })
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&mutation)?);
            } else {
                println!(
                    "{}  pending  publisher={}  package_sha256={}  {}",
                    mutation.review.id,
                    mutation.review.evidence.publisher,
                    mutation.review.evidence.package_sha256,
                    if mutation.replayed {
                        "replayed"
                    } else {
                        "reviewed"
                    }
                );
            }
        }
        OrganizationCommand::ImportShare {
            review_id,
            import_id,
            name,
            reason,
            json,
        } => {
            let review_id = OrganizationShareReviewId::from_str(&review_id)?;
            let generated_import_id = import_id.is_none();
            let import_id = import_id
                .as_deref()
                .map(OrganizationImportId::from_str)
                .transpose()?
                .unwrap_or_default();
            if generated_import_id {
                eprintln!(
                    "Import identity: {import_id} (reuse with --import-id if this request must be retried)"
                );
            }
            let mutation: OrganizationShareImportMutation = client
                .post(format!(
                    "{host}/api/v1/organization-share-reviews/{review_id}/import"
                ))
                .bearer_auth(token)
                .json(&ImportSharedOrganizationRequest {
                    import_id,
                    name,
                    reason,
                })
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&mutation)?);
            } else {
                println!(
                    "{}  organization={}  import={}  {}",
                    mutation.review.id,
                    mutation.organization.organization_id,
                    import_id,
                    if mutation.replayed {
                        "replayed"
                    } else {
                        "imported"
                    }
                );
            }
        }
        OrganizationCommand::RejectShare {
            review_id,
            reason,
            json,
        } => {
            let review_id = OrganizationShareReviewId::from_str(&review_id)?;
            let mutation: OrganizationShareReviewMutation = client
                .post(format!(
                    "{host}/api/v1/organization-share-reviews/{review_id}/reject"
                ))
                .bearer_auth(token)
                .json(&DecideSharedOrganizationRequest { reason })
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&mutation)?);
            } else {
                println!(
                    "{}  rejected  {}",
                    mutation.review.id,
                    if mutation.replayed {
                        "replayed"
                    } else {
                        "recorded"
                    }
                );
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_lines)] // Keeps the complete run command family visible together.
async fn run_agent(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    command: RunCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        RunCommand::PlanAtomic {
            task_id,
            organization_revision_id,
            goal,
            role,
            check,
            independent_evaluation,
            json,
        } => {
            let task_id = TaskId::from_str(&task_id)?;
            let organization_revision_id =
                OrganizationRevisionId::from_str(&organization_revision_id)?;
            let request = PlanAgentRunRequest {
                organization_revision_id,
                complexity: TaskComplexity::Atomic,
                work: vec![PlanWorkRequest {
                    key: "work".into(),
                    goal,
                    role: role.into(),
                    dependencies: Vec::new(),
                    success_conditions: vec![SuccessCondition::Custom { description: check }],
                    risk_notes: Vec::new(),
                    budget_weight: 1,
                    estimated_useful_seconds: 60,
                }],
                require_independent_evaluation: independent_evaluation,
            };
            let view = post_plan(client, host, token, task_id, &request).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&view)?);
            } else {
                println!("{}: {:?}", view.run.id, view.run.state);
            }
        }
        RunCommand::PlanFile { task_id, file } => {
            let task_id = TaskId::from_str(&task_id)?;
            let request: PlanAgentRunRequest = serde_json::from_slice(&std::fs::read(file)?)?;
            let view = post_plan(client, host, token, task_id, &request).await?;
            println!("{}", serde_json::to_string_pretty(&view)?);
        }
        RunCommand::Get { id } => {
            let run_id = RunId::from_str(&id)?;
            let view: AgentRunView = client
                .get(format!("{host}/api/v1/runs/{run_id}"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&view)?);
        }
        RunCommand::Replay { id, json } => {
            let run_id = RunId::from_str(&id)?;
            let report: RunReplayReport = client
                .get(format!("{host}/api/v1/runs/{run_id}/replay"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                let passed = report.invariants.iter().filter(|item| item.passed).count();
                println!(
                    "verified run {}: {passed}/{} invariants passed; evidence sha256 {}",
                    report.run_id,
                    report.invariants.len(),
                    report.evidence_sha256
                );
            }
        }
        RunCommand::Start { id } => {
            transition_run(client, host, token, &id, AgentRunState::Running).await?;
        }
        RunCommand::Execute {
            id,
            work_item_id,
            project_id,
            target_path,
            context_token_budget,
            tools,
            estimated_output_tokens,
            max_model_calls,
        } => {
            let run_id = RunId::from_str(&id)?;
            let work_item_id = WorkItemId::from_str(&work_item_id)?;
            let project_id = ProjectId::from_str(&project_id)?;
            let commit: AgentTurnCommit = client
                .post(format!(
                    "{host}/api/v1/runs/{run_id}/work/{work_item_id}/execute"
                ))
                .bearer_auth(token)
                .json(&ExecuteAgentWorkRequest {
                    project_id,
                    target_path,
                    context_token_budget,
                    requested_tools: tools,
                    estimated_output_tokens,
                    max_model_calls,
                })
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&commit)?);
        }
        RunCommand::Evaluate { id, file } => {
            let run_id = RunId::from_str(&id)?;
            let report: mcg_agents::EvaluationReport =
                serde_json::from_slice(&std::fs::read(file)?)?;
            let mutation: EvaluationMutationResponse = client
                .post(format!("{host}/api/v1/runs/{run_id}/evaluations"))
                .bearer_auth(token)
                .json(&report)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&mutation)?);
        }
        RunCommand::Cancel { id } => {
            transition_run(client, host, token, &id, AgentRunState::Cancelled).await?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_lines)] // Keeps the complete provider subcommand dispatch in one place.
async fn run_provider(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    command: ProviderCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        ProviderCommand::Login {
            id,
            name,
            method,
            no_open,
        } => {
            login_openai(client, host, token, id, name, method, no_open).await?;
        }
        ProviderCommand::Add {
            id,
            name,
            base_url,
            kind,
            auth,
            json,
        } => {
            add_provider(client, host, token, id, name, base_url, kind, auth, json).await?;
        }
        ProviderCommand::List { json } => {
            let response: ProviderListResponse = client
                .get(format!("{host}/api/v1/providers"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else if response.providers.is_empty() {
                println!("No providers configured.");
            } else {
                for provider in response.providers {
                    println!(
                        "{}  {}  {:?}  {} model(s)",
                        provider.connection.id,
                        provider.connection.display_name,
                        provider.connection.auth_mode,
                        provider.catalog.map_or(0, |catalog| catalog.models.len())
                    );
                }
            }
        }
        ProviderCommand::Models { id } => {
            let catalog: ProviderModelCatalog = client
                .get(format!("{host}/api/v1/providers/{id}/models"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&catalog)?);
        }
        ProviderCommand::Discover { id } => {
            let catalog: ProviderModelCatalog = client
                .post(format!("{host}/api/v1/providers/{id}/discover"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!(
                "{}: {} model(s)",
                catalog.connection_id,
                catalog.models.len()
            );
        }
        ProviderCommand::Refresh { id } => {
            let event: mcg_events::EventEnvelope = client
                .post(format!("{host}/api/v1/providers/{id}/oauth/refresh"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!(
                "{id}: OAuth credentials refreshed (event {})",
                event.sequence
            );
        }
        ProviderCommand::Disconnect { id } => {
            let event: EventEnvelope = client
                .delete(format!("{host}/api/v1/providers/{id}"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{id}: disconnected (event {})", event.sequence);
        }
        ProviderCommand::Probe { id, model } => {
            let report: ModelProbeReport = client
                .post(format!("{host}/api/v1/providers/{id}/models/{model}/probe"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        ProviderCommand::Usage { json } => {
            print_provider_proxy_usage(client, host, token, json).await?;
        }
        ProviderCommand::Conversations { json } => {
            print_provider_proxy_conversations(client, host, token, json).await?;
        }
    }
    Ok(())
}

async fn print_provider_proxy_usage(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let usage: ProxyUsageSummary = client
        .get(format!("{host}/api/v1/provider-proxy/usage"))
        .bearer_auth(token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&usage)?);
    } else {
        println!(
            "{} call(s) across {} conversation(s): {} tokens, ${:.6} known cost, {} call(s) with unknown cost",
            usage.call_count,
            usage.conversation_count,
            usage.usage.total_tokens,
            usage.known_actual_cost_usd,
            usage.unknown_cost_call_count
        );
    }
    Ok(())
}

async fn print_provider_proxy_conversations(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let response: ProxyConversationListResponse = client
        .get(format!("{host}/api/v1/provider-proxy/conversations"))
        .bearer_auth(token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else if response.conversations.is_empty() {
        println!("No provider proxy conversations recorded.");
    } else {
        for conversation in response.conversations {
            println!(
                "{}  updated {}  project={}  task={}",
                conversation.id,
                conversation.updated_at,
                conversation
                    .project_id
                    .map_or_else(|| "-".into(), |id| id.to_string()),
                conversation
                    .task_id
                    .map_or_else(|| "-".into(), |id| id.to_string())
            );
        }
    }
    Ok(())
}

async fn run_mcp(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    command: McpCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        McpCommand::AddHttp {
            id,
            name,
            url,
            auth,
            protocol,
            disabled,
            json,
        } => {
            let auth_mode: McpAuthMode = auth.into();
            let credential = read_optional_standard_input_credential(
                auth_mode == McpAuthMode::BearerToken,
                "MCP bearer credential",
            )?;
            let payload = McpConfigurePayload {
                connection: McpConnection {
                    contract_version: MCP_CONNECTION_CONTRACT_VERSION,
                    id,
                    display_name: name,
                    transport: McpTransport::StreamableHttp { url },
                    auth_mode,
                    protocol_mode: protocol.into(),
                    enabled: !disabled,
                },
                credential,
            };
            post_mcp_configuration(client, host, token, &payload, json).await?;
        }
        McpCommand::AddStdio {
            id,
            name,
            executable,
            protocol,
            disabled,
            json,
            args,
        } => {
            let payload = McpConfigurePayload {
                connection: McpConnection {
                    contract_version: MCP_CONNECTION_CONTRACT_VERSION,
                    id,
                    display_name: name,
                    transport: McpTransport::Stdio {
                        command: executable,
                        args,
                    },
                    auth_mode: McpAuthMode::None,
                    protocol_mode: protocol.into(),
                    enabled: !disabled,
                },
                credential: None,
            };
            post_mcp_configuration(client, host, token, &payload, json).await?;
        }
        McpCommand::List { json } => {
            list_mcp_configurations(client, host, token, json).await?;
        }
        McpCommand::Tools { id } => {
            let catalog: McpToolCatalog = client
                .get(format!("{host}/api/v1/mcp/connections/{id}/tools"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&catalog)?);
        }
        McpCommand::Discover { id } => {
            let catalog: McpToolCatalog = client
                .post(format!("{host}/api/v1/mcp/connections/{id}/discover"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}: {} tool(s)", catalog.connection_id, catalog.tools.len());
        }
    }
    Ok(())
}

#[allow(clippy::too_many_lines)] // Keeps the explicit plugin grant/enable state machine visible.
async fn run_plugin(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    command: PluginCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        PluginCommand::List { json } => {
            let response: PluginListResponse =
                get_authenticated(client, host, token, "/api/v1/plugins").await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else if response.plugins.is_empty() {
                println!("No plugins installed.");
            } else {
                for plugin in response.plugins {
                    println!(
                        "{} {}  {}  {}  grants={}  revision={}",
                        plugin.active_revision.manifest.display_name,
                        plugin.active_revision.manifest.version,
                        plugin.installation.plugin_id,
                        if plugin.installation.enabled {
                            "enabled"
                        } else {
                            "disabled"
                        },
                        plugin.installation.capability_grants.len(),
                        plugin.installation.revision
                    );
                }
            }
        }
        PluginCommand::InstallLocal {
            manifest,
            module,
            json,
        } => {
            let manifest: PluginManifest = read_bounded_json(&manifest, 64 * 1_024)?;
            let module = read_bounded_file(&module, 64 * 1_024 * 1_024)?;
            let revision = PluginRevision {
                schema_version: mcg_domain::PLUGIN_CATALOG_SCHEMA_VERSION,
                id: PluginRevisionId::new(),
                manifest,
                module_sha256: format!("{:x}", Sha256::digest(&module)),
                module_size_bytes: u64::try_from(module.len())?,
                source: PluginInstallSource::LocalImport,
                signature: PluginSignature {
                    state: PluginSignatureState::Unsigned,
                    scheme: None,
                    signer: None,
                    signature_sha256: None,
                },
                installed_at: Utc::now(),
            };
            let installation: PluginInstallation = post_authenticated(
                client,
                host,
                token,
                "/api/v1/plugins/install/local",
                &InstallLocalPluginRequest {
                    revision,
                    module_base64: STANDARD.encode(module),
                },
            )
            .await?;
            print_plugin_installation(&installation, json)?;
        }
        PluginCommand::InstallRegistry { request, json } => {
            let request: InstallRegistryPluginRequest =
                read_bounded_json(&request, 90 * 1_024 * 1_024)?;
            let installation: PluginInstallation = post_authenticated(
                client,
                host,
                token,
                "/api/v1/plugins/install/registry",
                &request,
            )
            .await?;
            print_plugin_installation(&installation, json)?;
        }
        PluginCommand::SetGrants {
            plugin_id,
            expected_revision,
            grants,
            json,
        } => {
            let grants: Vec<PluginCapabilityGrant> = read_bounded_json(&grants, 256 * 1_024)?;
            let installation: PluginInstallation = post_authenticated(
                client,
                host,
                token,
                &format!("/api/v1/plugins/{plugin_id}/grants"),
                &SetPluginGrantsRequest {
                    expected_revision,
                    grants,
                },
            )
            .await?;
            print_plugin_installation(&installation, json)?;
        }
        PluginCommand::SetEnabled {
            plugin_id,
            expected_revision,
            enabled,
            json,
        } => {
            let installation: PluginInstallation = post_authenticated(
                client,
                host,
                token,
                &format!("/api/v1/plugins/{plugin_id}/enabled"),
                &SetPluginEnabledRequest {
                    expected_revision,
                    enabled,
                },
            )
            .await?;
            print_plugin_installation(&installation, json)?;
        }
        PluginCommand::Invoke {
            plugin_id,
            input,
            invocation_id,
            task_id,
            json,
        } => {
            let input = read_bounded_json(&input, 1_024 * 1_024)?;
            let invocation_id = invocation_id.unwrap_or_default();
            let mutation: PluginInvocationMutation = post_authenticated(
                client,
                host,
                token,
                &format!("/api/v1/plugins/{plugin_id}/invocations/{invocation_id}"),
                &InvokePluginRequest { task_id, input },
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&mutation)?);
            } else {
                println!(
                    "{}: {:?}{}",
                    mutation.invocation.id,
                    mutation.invocation.state,
                    if mutation.replayed { " (replay)" } else { "" }
                );
                if let Some(result) = mutation.result {
                    println!("{}", serde_json::to_string_pretty(&result.output)?);
                }
            }
        }
        PluginCommand::Invocation {
            plugin_id,
            invocation_id,
            json,
        } => {
            let response: PluginInvocationHistoryResponse = get_authenticated(
                client,
                host,
                token,
                &format!("/api/v1/plugins/{plugin_id}/invocations/{invocation_id}"),
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else {
                println!(
                    "{}: {:?}",
                    response.invocation.id, response.invocation.state
                );
                for snapshot in response.history {
                    println!("  {:?}  {}", snapshot.state, snapshot.started_at);
                }
            }
        }
    }
    Ok(())
}

async fn get_authenticated<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    path: &str,
) -> Result<T, Box<dyn std::error::Error>> {
    Ok(client
        .get(format!("{host}{path}"))
        .bearer_auth(token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

async fn post_authenticated<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    path: &str,
    body: &impl Serialize,
) -> Result<T, Box<dyn std::error::Error>> {
    Ok(client
        .post(format!("{host}{path}"))
        .bearer_auth(token)
        .json(body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

fn read_bounded_file(
    path: &std::path::Path,
    maximum_bytes: u64,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let metadata = std::fs::metadata(path)?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > maximum_bytes {
        return Err(format!("{} is not a bounded regular file", path.display()).into());
    }
    Ok(std::fs::read(path)?)
}

fn read_bounded_json<T: serde::de::DeserializeOwned>(
    path: &std::path::Path,
    maximum_bytes: u64,
) -> Result<T, Box<dyn std::error::Error>> {
    Ok(serde_json::from_slice(&read_bounded_file(
        path,
        maximum_bytes,
    )?)?)
}

fn print_plugin_installation(
    installation: &PluginInstallation,
    json: bool,
) -> Result<(), serde_json::Error> {
    if json {
        println!("{}", serde_json::to_string_pretty(installation)?);
    } else {
        println!(
            "{}: {} (revision {}, grants={})",
            installation.plugin_id,
            if installation.enabled {
                "enabled"
            } else {
                "disabled"
            },
            installation.revision,
            installation.capability_grants.len()
        );
    }
    Ok(())
}

async fn list_mcp_configurations(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let response: McpListResponse = client
        .get(format!("{host}/api/v1/mcp/connections"))
        .bearer_auth(token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else if response.connections.is_empty() {
        println!("No MCP servers configured.");
    } else {
        for item in response.connections {
            println!(
                "{}  {}  {:?}  {} tool(s)",
                item.connection.id,
                item.connection.display_name,
                item.connection.auth_mode,
                item.catalog.map_or(0, |catalog| catalog.tools.len())
            );
        }
    }
    Ok(())
}

async fn post_mcp_configuration(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    payload: &McpConfigurePayload,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .post(format!("{host}/api/v1/mcp/connections"))
        .bearer_auth(token)
        .json(payload)
        .send()
        .await?;
    if !response.status().is_success() {
        let status = response.status();
        let detail = response.text().await?;
        return Err(format!("MCP setup failed ({status}): {detail}").into());
    }
    let mutation: McpMutation = response.json().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&mutation)?);
    } else {
        println!(
            "{}: {} tool(s) discovered",
            mutation.connection.id,
            mutation.catalog.tools.len()
        );
    }
    Ok(())
}

fn read_optional_standard_input_credential(
    required: bool,
    label: &str,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    if !required {
        return Ok(None);
    }
    let mut secret = String::new();
    std::io::stdin().read_to_string(&mut secret)?;
    let trimmed = secret.trim_end_matches(['\r', '\n']).to_owned();
    secret.zeroize();
    if trimmed.is_empty() {
        return Err(format!("{label} must be supplied on standard input").into());
    }
    Ok(Some(trimmed))
}

#[allow(clippy::too_many_arguments)] // Mirrors the explicit login command fields.
async fn login_openai(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    id: String,
    name: String,
    method: OAuthMethodArg,
    no_open: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .post(format!("{host}/api/v1/oauth/openai/start"))
        .bearer_auth(token)
        .json(&StartOpenAiOAuthRequest {
            connection_id: id,
            display_name: name,
            method: method.into(),
        })
        .send()
        .await?;
    if !response.status().is_success() {
        let status = response.status();
        let detail = response.text().await?;
        return Err(format!("OpenAI login could not start ({status}): {detail}").into());
    }
    let started: OAuthLoginStart = response.json().await?;
    println!("Open this verification page: {}", started.verification_url);
    if let Some(code) = &started.user_code {
        println!("Enter this one-time code: {code}");
    }
    if !no_open && webbrowser::open(&started.verification_url).is_err() {
        eprintln!("The browser could not be opened automatically; use the link above.");
    }
    println!("Waiting for OpenAI authorization. Press Ctrl+C to cancel.");

    let complete_url = format!("{host}/api/v1/oauth/openai/{}/complete", started.session_id);
    let response = tokio::select! {
        result = client.post(complete_url).bearer_auth(token).send() => result?,
        signal = tokio::signal::ctrl_c() => {
            signal?;
            let cancel_url = format!(
                "{host}/api/v1/oauth/openai/{}/cancel",
                started.session_id
            );
            let _ = client.post(cancel_url).bearer_auth(token).send().await;
            return Err("OpenAI login cancelled".into());
        }
    };
    if !response.status().is_success() {
        let status = response.status();
        let detail = response.text().await?;
        return Err(format!("OpenAI login failed ({status}): {detail}").into());
    }
    let mutation: ProviderMutation = response.json().await?;
    println!(
        "{} signed in with {:?}; API-key billing was not configured.",
        mutation.connection.id, mutation.connection.auth_mode
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)] // Mirrors the explicit CLI fields without a second command DTO.
async fn add_provider(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    id: String,
    name: String,
    base_url: String,
    kind: ProviderKindArg,
    auth: ProviderAuthArg,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let auth_mode: ProviderAuthMode = auth.into();
    let credential = if auth_mode == ProviderAuthMode::None {
        None
    } else {
        let mut secret = String::new();
        std::io::stdin().read_to_string(&mut secret)?;
        let trimmed = secret.trim_end_matches(['\r', '\n']).to_owned();
        secret.zeroize();
        if trimmed.is_empty() {
            return Err("provider credential must be supplied on standard input".into());
        }
        Some(trimmed)
    };
    let body = ProviderConfigurePayload {
        connection: ProviderConnection {
            contract_version: CONTRACT_VERSION,
            id,
            display_name: name,
            kind: kind.into(),
            base_url,
            auth_mode,
            organization: None,
            project: None,
        },
        credential,
    };
    let response = client
        .post(format!("{host}/api/v1/providers"))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await?;
    if !response.status().is_success() {
        let status = response.status();
        let detail = response.text().await?;
        return Err(format!("provider setup failed ({status}): {detail}").into());
    }
    let mutation: ProviderMutation = response.json().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&mutation)?);
    } else {
        println!(
            "{}: {} model(s) discovered",
            mutation.connection.id,
            mutation.catalog.models.len()
        );
    }
    Ok(())
}

async fn post_plan(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    task_id: TaskId,
    request: &PlanAgentRunRequest,
) -> Result<AgentRunView, Box<dyn std::error::Error>> {
    Ok(client
        .post(format!("{host}/api/v1/tasks/{task_id}/runs"))
        .bearer_auth(token)
        .json(request)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

async fn transition_run(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    id: &str,
    state: AgentRunState,
) -> Result<(), Box<dyn std::error::Error>> {
    let run_id = RunId::from_str(id)?;
    let view: AgentRunView = client
        .post(format!("{host}/api/v1/runs/{run_id}/transition"))
        .bearer_auth(token)
        .json(&TransitionAgentRunRequest { state })
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    println!("{}: {:?}", view.run.id, view.run.state);
    Ok(())
}

fn conversation_preferences(
    model: Option<String>,
    effort: Option<String>,
    speed: Option<String>,
    approval: ApprovalMode,
) -> ExecutionPreferences {
    conversation_preferences_exact(model, effort, speed, approval, None, None)
}

fn conversation_preferences_exact(
    model: Option<String>,
    effort: Option<String>,
    speed: Option<String>,
    approval: ApprovalMode,
    approval_policy: Option<ExecutionApprovalPolicy>,
    sandbox_mode: Option<ExecutionSandboxMode>,
) -> ExecutionPreferences {
    ExecutionPreferences {
        model_provider: None,
        model_id: model,
        reasoning_effort: effort,
        service_tier: speed,
        approval_mode: approval,
        approval_policy,
        sandbox_mode,
        permission_profile: None,
        additional_writable_roots: Vec::new(),
        codex_feature_overrides: BTreeMap::new(),
        codex_config_overrides: Vec::new(),
        codex_profile: None,
        codex_profile_config: None,
        codex_profile_digest: None,
        codex_profile_ignored_keys: Vec::new(),
        strict_config: false,
        output_schema: None,
        team_preset: Default::default(),
        personality: None,
    }
}

fn codex_execution_preferences(
    model: Option<String>,
    model_provider: Option<String>,
    sandbox: Option<CodexSandboxArg>,
    approval_policy: Option<CodexApprovalPolicyArg>,
    approve_for_me: bool,
    dangerously_bypass_approvals_and_sandbox: bool,
    additional_writable_roots: &[String],
    feature_overrides: &BTreeMap<String, bool>,
    config_overrides: &[CodexConfigOverride],
    codex_profile: Option<String>,
    strict_config: bool,
    mut fallback: ExecutionPreferences,
) -> ExecutionPreferences {
    let execution_controls_overridden =
        sandbox.is_some() || approval_policy.is_some() || approve_for_me;
    if execution_controls_overridden || dangerously_bypass_approvals_and_sandbox {
        fallback.permission_profile = None;
    }
    if let Some(model) = model {
        fallback.model_id = Some(model);
    }
    fallback.model_provider = model_provider;
    if fallback.model_provider.is_some() {
        fallback.service_tier = None;
    }
    fallback.additional_writable_roots = additional_writable_roots.to_vec();
    fallback.codex_feature_overrides = feature_overrides.clone();
    fallback.codex_config_overrides = config_overrides.to_vec();
    if let Some(codex_profile) = codex_profile {
        fallback.codex_profile = Some(codex_profile);
    }
    fallback.strict_config = strict_config;
    if dangerously_bypass_approvals_and_sandbox {
        fallback.approval_mode = ApprovalMode::FullAccess;
        fallback.approval_policy = Some(ExecutionApprovalPolicy::Never);
        fallback.sandbox_mode = Some(ExecutionSandboxMode::DangerFullAccess);
        return fallback;
    }
    if approve_for_me {
        fallback.approval_mode = ApprovalMode::AutoReview;
        fallback.approval_policy = Some(ExecutionApprovalPolicy::OnRequest);
        fallback.sandbox_mode = Some(ExecutionSandboxMode::WorkspaceWrite);
        return fallback;
    }
    if let Some(policy) = approval_policy {
        fallback.approval_policy = Some(match policy {
            CodexApprovalPolicyArg::Untrusted => ExecutionApprovalPolicy::Untrusted,
            CodexApprovalPolicyArg::OnRequest => ExecutionApprovalPolicy::OnRequest,
            CodexApprovalPolicyArg::Never => ExecutionApprovalPolicy::Never,
        });
    }
    if let Some(mode) = sandbox {
        fallback.sandbox_mode = Some(match mode {
            CodexSandboxArg::ReadOnly => ExecutionSandboxMode::ReadOnly,
            CodexSandboxArg::WorkspaceWrite => ExecutionSandboxMode::WorkspaceWrite,
            CodexSandboxArg::DangerFullAccess => ExecutionSandboxMode::DangerFullAccess,
        });
    }
    if execution_controls_overridden {
        fallback.approval_mode =
            if matches!(fallback.sandbox_mode, Some(ExecutionSandboxMode::ReadOnly)) {
                ApprovalMode::ReadOnly
            } else if matches!(
                (fallback.approval_policy, fallback.sandbox_mode),
                (
                    Some(ExecutionApprovalPolicy::Never),
                    Some(ExecutionSandboxMode::DangerFullAccess)
                )
            ) {
                ApprovalMode::FullAccess
            } else {
                ApprovalMode::Ask
            };
    }
    fallback
}

fn codex_execution_preferences_override(
    model: Option<String>,
    model_provider: Option<String>,
    sandbox: Option<CodexSandboxArg>,
    approval_policy: Option<CodexApprovalPolicyArg>,
    approve_for_me: bool,
    dangerously_bypass_approvals_and_sandbox: bool,
    additional_writable_roots: &[String],
    feature_overrides: &BTreeMap<String, bool>,
    config_overrides: &[CodexConfigOverride],
    codex_profile: Option<String>,
    strict_config: bool,
    fallback: &ExecutionPreferences,
) -> Option<ExecutionPreferences> {
    if model.is_none()
        && model_provider.is_none()
        && sandbox.is_none()
        && approval_policy.is_none()
        && !approve_for_me
        && additional_writable_roots.is_empty()
        && feature_overrides.is_empty()
        && config_overrides.is_empty()
        && codex_profile.is_none()
        && !strict_config
        && !dangerously_bypass_approvals_and_sandbox
    {
        return None;
    }
    Some(codex_execution_preferences(
        model,
        model_provider,
        sandbox,
        approval_policy,
        approve_for_me,
        dangerously_bypass_approvals_and_sandbox,
        additional_writable_roots,
        feature_overrides,
        config_overrides,
        codex_profile,
        strict_config,
        fallback.clone(),
    ))
}

fn conversation_batch_terminal(state: CoordinationBatchState) -> bool {
    matches!(
        state,
        CoordinationBatchState::Completed
            | CoordinationBatchState::Failed
            | CoordinationBatchState::RecoveryRequired
    )
}

fn conversation_turn_terminal(state: ConversationTurnState) -> bool {
    matches!(
        state,
        ConversationTurnState::Completed
            | ConversationTurnState::Failed
            | ConversationTurnState::Interrupted
            | ConversationTurnState::Indeterminate
    )
}

fn conversation_settled(view: &ConversationView, origin_turn_id: ConversationTurnId) -> bool {
    let batches = view
        .team
        .coordination_batches
        .iter()
        .filter(|batch| batch.origin_turn_id == origin_turn_id)
        .collect::<Vec<_>>();
    if batches.is_empty() {
        return view
            .turns
            .iter()
            .find(|turn| turn.id == origin_turn_id)
            .is_some_and(|turn| conversation_turn_terminal(turn.state));
    }
    let batch_ids = batches
        .iter()
        .map(|batch| batch.id)
        .collect::<BTreeSet<_>>();
    batches
        .iter()
        .all(|batch| conversation_batch_terminal(batch.state))
        && !view.team.wakes.iter().any(|wake| {
            batch_ids.contains(&wake.coordination_batch_id)
                && matches!(
                    wake.state,
                    AgentWakeState::Pending | AgentWakeState::Claimed
                )
        })
        && view.messages.iter().any(|message| {
            message.turn_id == origin_turn_id && message.role == ConversationRole::Assistant
        })
}

async fn fetch_conversation(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    id: &str,
) -> Result<ConversationView, Box<dyn std::error::Error>> {
    decode_api_response(
        client
            .get(format!("{host}/api/v1/conversations/{id}"))
            .bearer_auth(token)
            .send()
            .await?,
        "read conversation",
    )
    .await
}

async fn decode_api_response<T: DeserializeOwned>(
    response: reqwest::Response,
    operation: &str,
) -> Result<T, Box<dyn std::error::Error>> {
    if !response.status().is_success() {
        let status = response.status();
        let detail = response.text().await?;
        return Err(format!("{operation} failed ({status}): {detail}").into());
    }
    Ok(response.json().await?)
}

async fn wait_for_conversation(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    id: &str,
    origin_turn_id: ConversationTurnId,
    timeout_seconds: u64,
    poll_seconds: u64,
) -> Result<ConversationView, Box<dyn std::error::Error>> {
    let deadline = tokio::time::Instant::now()
        + std::time::Duration::from_secs(timeout_seconds.clamp(1, 86_400));
    let interval = std::time::Duration::from_secs(poll_seconds.clamp(1, 60));
    loop {
        let view = fetch_conversation(client, host, token, id).await?;
        if conversation_settled(&view, origin_turn_id) {
            return Ok(view);
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(format!(
                "conversation {id} did not settle within {timeout_seconds} seconds; durable work remains inspectable with `mcg conversation team {id}`"
            )
            .into());
        }
        tokio::time::sleep(interval).await;
    }
}

fn emit_exec_jsonl(event: &ExecJsonlEvent) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", serde_json::to_string(event)?);
    std::io::stdout().flush()?;
    Ok(())
}

async fn wait_for_conversation_jsonl(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    mut view: ConversationView,
    origin_turn_id: ConversationTurnId,
    projector: &mut ExecJsonlProjector,
    timeout_seconds: u64,
) -> Result<ConversationView, Box<dyn std::error::Error>> {
    for event in projector.project_view(&view) {
        emit_exec_jsonl(&event)?;
    }
    if conversation_settled(&view, origin_turn_id) {
        return Ok(view);
    }

    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let stream = spawn_conversation_stream(
        client.clone(),
        host.to_owned(),
        token.to_owned(),
        view.conversation.id,
        sender,
    );
    let deadline = tokio::time::Instant::now()
        + std::time::Duration::from_secs(timeout_seconds.clamp(1, 86_400));
    loop {
        if tokio::time::Instant::now() >= deadline {
            stream.abort();
            return Err(format!(
                "conversation {} did not settle within {timeout_seconds} seconds; durable work remains inspectable with `mcg conversation team {}`",
                view.conversation.id, view.conversation.id
            )
            .into());
        }
        match tokio::time::timeout(std::time::Duration::from_secs(2), receiver.recv()).await {
            Ok(Some(event)) => {
                if let Some(activity) = event.activity.as_ref() {
                    for event in projector.project_activity(activity) {
                        emit_exec_jsonl(&event)?;
                    }
                }
                if let Some(message) = event.message.as_ref() {
                    for event in projector.project_message(message) {
                        emit_exec_jsonl(&event)?;
                    }
                }
                view = event.view;
            }
            Ok(None) | Err(_) => {
                view = fetch_conversation(client, host, token, &view.conversation.id.to_string())
                    .await?;
            }
        }
        for event in projector.project_view(&view) {
            emit_exec_jsonl(&event)?;
        }
        if conversation_settled(&view, origin_turn_id) {
            stream.abort();
            return Ok(view);
        }
    }
}

fn print_conversation(
    view: &ConversationView,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if json {
        println!("{}", serde_json::to_string_pretty(view)?);
        return Ok(());
    }
    println!("{} · {}", view.conversation.id, view.conversation.title);
    for message in &view.messages {
        println!("\n{:?}:\n{}", message.role, message.content);
    }
    if !view.team.coordination_batches.is_empty() {
        println!("\nBatches:");
        for batch in &view.team.coordination_batches {
            println!(
                "{} · {:?} · {} delegation(s) · {} Coordinator turn(s) · ~{} tokens",
                batch.id,
                batch.state,
                batch.delegation_ids.len(),
                batch.coordinator_turn_count,
                batch.coordinator_approximate_tokens
            );
        }
    }
    Ok(())
}

async fn run_conversation(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    command: ConversationCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        ConversationCommand::Create {
            title,
            project_id,
            model,
            effort,
            speed,
            approval,
            json,
        } => {
            let project_id = project_id
                .map(|value| ProjectId::from_str(&value))
                .transpose()?;
            let conversation: Conversation = decode_api_response(
                client
                    .post(format!("{host}/api/v1/conversations"))
                    .bearer_auth(token)
                    .json(&CreateConversationRequest {
                        title,
                        project_id,
                        execution_preferences: conversation_preferences(
                            model,
                            effort,
                            speed,
                            approval.into(),
                        ),
                        interaction_kind: ConversationInteractionKind::Interactive,
                    })
                    .send()
                    .await?,
                "create conversation",
            )
            .await?;
            let view =
                fetch_conversation(client, host, token, &conversation.id.to_string()).await?;
            print_conversation(&view, json)?;
        }
        ConversationCommand::List { archived, json } => {
            let value: serde_json::Value = client
                .get(format!("{host}/api/v1/conversations"))
                .query(&[("include_archived", archived)])
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else if let Some(items) = value
                .get("conversations")
                .and_then(serde_json::Value::as_array)
            {
                for item in items {
                    println!(
                        "{} · {} · {}",
                        item["id"].as_str().unwrap_or("?"),
                        item["state"].as_str().unwrap_or("?"),
                        item["title"].as_str().unwrap_or("")
                    );
                }
            }
        }
        ConversationCommand::Show { id, json } => {
            print_conversation(&fetch_conversation(client, host, token, &id).await?, json)?
        }
        ConversationCommand::Send {
            id,
            message,
            mentions,
            model,
            effort,
            speed,
            approval,
            wait,
            timeout_seconds,
            poll_seconds,
            json,
        } => {
            let content = validated_goal(&message.join(" "))?;
            let (content, mentions) =
                resolve_cli_mentions(client, host, token, content, &mentions).await?;
            let execution_preferences =
                if model.is_some() || effort.is_some() || speed.is_some() || approval.is_some() {
                    Some(conversation_preferences(
                        model,
                        effort,
                        speed,
                        approval.unwrap_or(ConversationApprovalArg::Ask).into(),
                    ))
                } else {
                    None
                };
            let mut view: ConversationView = decode_api_response(
                client
                    .post(format!("{host}/api/v1/conversations/{id}/messages"))
                    .bearer_auth(token)
                    .json(&SendConversationMessageRequest {
                        content,
                        execution_preferences,
                        mentions,
                    })
                    .send()
                    .await?,
                "send conversation message",
            )
            .await?;
            let turn_id = view
                .messages
                .iter()
                .rev()
                .find(|message| message.role == ConversationRole::User)
                .map(|message| message.turn_id)
                .ok_or("conversation response did not contain the submitted user turn")?;
            if wait {
                view = wait_for_conversation(
                    client,
                    host,
                    token,
                    &id,
                    turn_id,
                    timeout_seconds,
                    poll_seconds,
                )
                .await?;
            }
            print_conversation(&view, json)?;
        }
        ConversationCommand::Team { id, json } => {
            let value: serde_json::Value = client
                .get(format!("{host}/api/v1/conversations/{id}/team"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!("{}", serde_json::to_string_pretty(&value)?);
            }
        }
        ConversationCommand::Receipt { id } => {
            let view = fetch_conversation(client, host, token, &id).await?;
            let receipt = conversation_receipt(&view, None)?;
            println!("{}", serde_json::to_string_pretty(&receipt)?);
        }
        ConversationCommand::Search { id, query, limit } => {
            let value: serde_json::Value = client
                .get(format!("{host}/api/v1/conversations/{id}/context/search"))
                .query(&[("q", query), ("limit", limit.to_string())])
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&value)?);
        }
        ConversationCommand::Review { id, delegation_id } => {
            print_json_response(
                client
                    .get(format!(
                        "{host}/api/v1/conversations/{id}/delegations/{delegation_id}/review"
                    ))
                    .bearer_auth(token)
                    .send()
                    .await?,
            )
            .await?;
        }
        ConversationCommand::Cancel {
            id,
            delegation_id,
            expected_revision,
        } => {
            print_json_response(
                client
                    .post(format!(
                        "{host}/api/v1/conversations/{id}/delegations/{delegation_id}/cancel"
                    ))
                    .bearer_auth(token)
                    .json(&DelegationMutationRequest { expected_revision })
                    .send()
                    .await?,
            )
            .await?;
        }
        ConversationCommand::Retry {
            id,
            delegation_id,
            expected_revision,
            idempotency_key,
        } => {
            print_json_response(
                client
                    .post(format!(
                        "{host}/api/v1/conversations/{id}/delegations/{delegation_id}/retry"
                    ))
                    .bearer_auth(token)
                    .json(&DelegationRetryRequest {
                        expected_revision,
                        idempotency_key: idempotency_key
                            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                    })
                    .send()
                    .await?,
            )
            .await?;
        }
        ConversationCommand::Apply {
            id,
            delegation_id,
            expected_revision,
        } => {
            print_json_response(
                client
                    .post(format!(
                        "{host}/api/v1/conversations/{id}/delegations/{delegation_id}/apply"
                    ))
                    .bearer_auth(token)
                    .json(&WorktreeMutationRequest { expected_revision })
                    .send()
                    .await?,
            )
            .await?;
        }
        ConversationCommand::Discard {
            id,
            delegation_id,
            expected_revision,
        } => {
            print_json_response(
                client
                    .post(format!(
                        "{host}/api/v1/conversations/{id}/delegations/{delegation_id}/discard"
                    ))
                    .bearer_auth(token)
                    .json(&WorktreeMutationRequest { expected_revision })
                    .send()
                    .await?,
            )
            .await?;
        }
        ConversationCommand::PreviewStart { id, preview_id } => {
            print_json_response(
                client
                    .post(format!(
                        "{host}/api/v1/conversations/{id}/previews/{preview_id}/start"
                    ))
                    .bearer_auth(token)
                    .send()
                    .await?,
            )
            .await?
        }
        ConversationCommand::PreviewStop { id, preview_id } => {
            print_json_response(
                client
                    .post(format!(
                        "{host}/api/v1/conversations/{id}/previews/{preview_id}/stop"
                    ))
                    .bearer_auth(token)
                    .send()
                    .await?,
            )
            .await?
        }
        ConversationCommand::Interrupt { id, turn_id } => {
            print_json_response(
                client
                    .post(format!(
                        "{host}/api/v1/conversations/{id}/turns/{turn_id}/interrupt"
                    ))
                    .bearer_auth(token)
                    .send()
                    .await?,
            )
            .await?
        }
        ConversationCommand::Steer {
            id,
            turn_id,
            message,
            mentions,
        } => {
            let content = validated_goal(&message.join(" "))?;
            let (content, mentions) =
                resolve_cli_mentions(client, host, token, content, &mentions).await?;
            print_json_response(
                client
                    .post(format!(
                        "{host}/api/v1/conversations/{id}/turns/{turn_id}/steer"
                    ))
                    .bearer_auth(token)
                    .json(&SteerConversationTurnRequest { content, mentions })
                    .send()
                    .await?,
            )
            .await?
        }
        ConversationCommand::Archive { id, undo } => {
            print_json_response(
                client
                    .post(format!("{host}/api/v1/conversations/{id}/archive"))
                    .bearer_auth(token)
                    .json(&ArchiveConversationRequest { archived: !undo })
                    .send()
                    .await?,
            )
            .await?
        }
    }
    Ok(())
}

async fn print_json_response(
    response: reqwest::Response,
) -> Result<(), Box<dyn std::error::Error>> {
    if !response.status().is_success() {
        let status = response.status();
        let detail = response.text().await?;
        return Err(format!("MCG API request failed ({status}): {detail}").into());
    }
    if response.status() == reqwest::StatusCode::NO_CONTENT {
        println!("{{}}");
    } else {
        let value: serde_json::Value = response.json().await?;
        println!("{}", serde_json::to_string_pretty(&value)?);
    }
    Ok(())
}

async fn run_codex_login(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    method: &str,
    credential: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .post(format!("{host}/api/v1/codex/account/login"))
        .bearer_auth(token)
        .json(&serde_json::json!({"method": method, "credential": credential}))
        .send()
        .await?;
    print_json_response(response).await
}

async fn run_codex(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    command: CodexCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    let response = match command {
        CodexCommand::Status => {
            client
                .get(format!("{host}/api/v1/codex/harness"))
                .bearer_auth(token)
                .send()
                .await?
        }
        CodexCommand::Models => {
            client
                .get(format!("{host}/api/v1/codex/models"))
                .bearer_auth(token)
                .send()
                .await?
        }
        CodexCommand::Account => {
            client
                .get(format!("{host}/api/v1/codex/account"))
                .bearer_auth(token)
                .send()
                .await?
        }
        CodexCommand::RateLimits => {
            client
                .get(format!("{host}/api/v1/codex/rate-limits"))
                .bearer_auth(token)
                .send()
                .await?
        }
        CodexCommand::Usage => {
            client
                .get(format!("{host}/api/v1/codex/account/usage"))
                .bearer_auth(token)
                .send()
                .await?
        }
        CodexCommand::Configuration => {
            client
                .get(format!("{host}/api/v1/codex/configuration"))
                .bearer_auth(token)
                .send()
                .await?
        }
        CodexCommand::Pending => {
            client
                .get(format!("{host}/api/v1/codex/pending-requests"))
                .bearer_auth(token)
                .send()
                .await?
        }
        CodexCommand::Threads => {
            client
                .get(format!("{host}/api/v1/codex/owned-threads"))
                .bearer_auth(token)
                .send()
                .await?
        }
        CodexCommand::Discover { cwds, force } => {
            client
                .post(format!("{host}/api/v1/codex/discovery"))
                .bearer_auth(token)
                .json(&serde_json::json!({"cwds": cwds, "force_reload": force}))
                .send()
                .await?
        }
        CodexCommand::Login => {
            client
                .post(format!("{host}/api/v1/codex/account/login"))
                .bearer_auth(token)
                .send()
                .await?
        }
        CodexCommand::LoginCancel { login_id } => {
            client
                .post(format!("{host}/api/v1/codex/account/login/cancel"))
                .bearer_auth(token)
                .json(&serde_json::json!({"login_id": login_id}))
                .send()
                .await?
        }
        CodexCommand::Logout => {
            client
                .post(format!("{host}/api/v1/codex/account/logout"))
                .bearer_auth(token)
                .send()
                .await?
        }
    };
    print_json_response(response).await
}

async fn run_github(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    command: GithubCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    let response = match command {
        GithubCommand::Status => {
            client
                .get(format!("{host}/api/v1/github"))
                .bearer_auth(token)
                .send()
                .await?
        }
        GithubCommand::Login => {
            client
                .post(format!("{host}/api/v1/github/login/start"))
                .bearer_auth(token)
                .send()
                .await?
        }
        GithubCommand::LoginComplete { session_id } => {
            client
                .post(format!("{host}/api/v1/github/login/{session_id}/complete"))
                .bearer_auth(token)
                .send()
                .await?
        }
        GithubCommand::Disconnect => {
            client
                .delete(format!("{host}/api/v1/github"))
                .bearer_auth(token)
                .send()
                .await?
        }
        GithubCommand::Repositories => {
            client
                .get(format!("{host}/api/v1/github/repositories"))
                .bearer_auth(token)
                .send()
                .await?
        }
        GithubCommand::Create {
            name,
            description,
            public,
        } => client
            .post(format!("{host}/api/v1/github/repositories"))
            .bearer_auth(token)
            .json(
                &serde_json::json!({"name": name, "description": description, "private": !public}),
            )
            .send()
            .await?,
        GithubCommand::Link {
            project_id,
            clone_url,
        } => {
            client
                .post(format!("{host}/api/v1/github/repositories/link"))
                .bearer_auth(token)
                .json(&serde_json::json!({"project_id": project_id, "clone_url": clone_url}))
                .send()
                .await?
        }
    };
    print_json_response(response).await
}

async fn run_governor(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    command: GovernorCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    let response = match command {
        GovernorCommand::Policy => client.get(format!("{host}/api/v1/governor/policy")).bearer_auth(token).send().await?,
        GovernorCommand::List { organization_id } => client.get(format!("{host}/api/v1/governor/organizations/{organization_id}/proposals")).bearer_auth(token).send().await?,
        GovernorCommand::Show { proposal_id } => client.get(format!("{host}/api/v1/governor/proposals/{proposal_id}")).bearer_auth(token).send().await?,
        GovernorCommand::Create { file } => { let value: serde_json::Value = serde_json::from_slice(&std::fs::read(file)?)?; client.post(format!("{host}/api/v1/governor/proposals")).bearer_auth(token).json(&value).send().await? },
        GovernorCommand::Approve { proposal_id, reject } => client.post(format!("{host}/api/v1/governor/proposals/{proposal_id}/owner-approval")).bearer_auth(token).json(&serde_json::json!({"expected": GovernorProposalState::AwaitingOwnerApproval, "approve": !reject})).send().await?,
        GovernorCommand::RequestImplementation { proposal_id, conversation_id } => client.post(format!("{host}/api/v1/governor/proposals/{proposal_id}/implementation")).bearer_auth(token).json(&serde_json::json!({"conversation_id": conversation_id})).send().await?,
    };
    print_json_response(response).await
}

fn print_json_or_project(
    mutation: &ProjectMutation,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if json {
        println!("{}", serde_json::to_string_pretty(mutation)?);
    } else {
        print_project(&mutation.project);
    }
    Ok(())
}

#[allow(clippy::too_many_lines)] // CLI dispatch keeps related project subcommands discoverable.
async fn run_project(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    command: ProjectCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        ProjectCommand::Add {
            root,
            id,
            initialize_git,
            json,
        } => {
            let project_id = id.map(|value| ProjectId::from_str(&value)).transpose()?;
            let mutation: ProjectMutation = client
                .post(format!("{host}/api/v1/projects"))
                .bearer_auth(token)
                .json(&DiscoverProjectRequest {
                    root: root.to_string_lossy().into_owned(),
                    project_id,
                    initialize_git,
                })
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&mutation)?);
            } else {
                print_project(&mutation.project);
            }
        }
        ProjectCommand::List { json } => {
            let response: ProjectListResponse = client
                .get(format!("{host}/api/v1/projects"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else if response.projects.is_empty() {
                println!("No projects.");
            } else {
                for project in &response.projects {
                    print_project(project);
                }
            }
        }
        ProjectCommand::Get { id, json } => {
            let project_id = ProjectId::from_str(&id)?;
            let project: mcg_projects::ProjectModel = client
                .get(format!("{host}/api/v1/projects/{project_id}"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&project)?);
            } else {
                print_project(&project);
            }
        }
        ProjectCommand::InitGit { id, json } => {
            let project_id = ProjectId::from_str(&id)?;
            let mutation: ProjectMutation = client
                .post(format!(
                    "{host}/api/v1/projects/{project_id}/initialize-git"
                ))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            print_json_or_project(&mutation, json)?;
        }
        ProjectCommand::Archive { id, undo, json } => {
            let project_id = ProjectId::from_str(&id)?;
            let project: mcg_projects::ProjectModel = client
                .post(format!("{host}/api/v1/projects/{project_id}/archive"))
                .bearer_auth(token)
                .json(&ArchiveProjectRequest { archived: !undo })
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&project)?);
            } else {
                print_project(&project);
            }
        }
        ProjectCommand::Remove { id, json } => {
            let project_id = ProjectId::from_str(&id)?;
            let response = client
                .delete(format!("{host}/api/v1/projects/{project_id}"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({"removed": true, "project_id": project_id})
                );
            } else {
                println!(
                    "Removed project {project_id} from the MCG catalog; source was not deleted."
                );
            }
            drop(response);
        }
        ProjectCommand::Refresh { id, json } => {
            let project_id = ProjectId::from_str(&id)?;
            let mutation: ProjectMutation = client
                .post(format!("{host}/api/v1/projects/{project_id}/refresh"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&mutation)?);
            } else {
                print_project(&mutation.project);
                if let Some(invalidation) = mutation.invalidation {
                    println!("Changed inputs: {}", invalidation.changed_inputs.len());
                }
            }
        }
        ProjectCommand::History { id } => {
            let project_id = ProjectId::from_str(&id)?;
            let response: ProjectHistoryResponse = client
                .get(format!("{host}/api/v1/projects/{project_id}/history"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        ProjectCommand::Plan { id } => {
            let project_id = ProjectId::from_str(&id)?;
            let response: mcg_projects::ProjectTopologyPlan = client
                .get(format!("{host}/api/v1/projects/{project_id}/topology-plan"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        ProjectCommand::Reviewed {
            project_id,
            command,
        } => {
            run_reviewed_topology(
                client,
                host,
                token,
                ProjectId::from_str(&project_id)?,
                command,
            )
            .await?;
        }
        ProjectCommand::Context {
            id,
            token_budget,
            target,
        } => {
            let project_id = ProjectId::from_str(&id)?;
            let response: mcg_projects::ContextAssembly = client
                .post(format!("{host}/api/v1/projects/{project_id}/context"))
                .bearer_auth(token)
                .json(&AssembleContextRequest {
                    items: Vec::new(),
                    token_budget,
                    target_path: target,
                })
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        ProjectCommand::Memory {
            project_id,
            command,
        } => {
            run_memory(
                client,
                host,
                token,
                ProjectId::from_str(&project_id)?,
                command,
            )
            .await?;
        }
    }
    Ok(())
}

async fn run_reviewed_topology(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    project_id: ProjectId,
    command: ReviewedTopologyCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    let base = format!("{host}/api/v1/projects/{project_id}/reviewed-topology-plans");
    match command {
        ReviewedTopologyCommand::Create { file } => {
            let request: ReviewedTopologyPlanCreateRequest =
                serde_json::from_slice(&std::fs::read(file)?)?;
            let mutation: ReviewedTopologyPlanMutation = client
                .post(&base)
                .bearer_auth(token)
                .json(&request)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&mutation)?);
        }
        ReviewedTopologyCommand::List => {
            let response: ReviewedTopologyPlanListResponse = client
                .get(&base)
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        ReviewedTopologyCommand::Get { plan_id } => {
            let plan_id = ReviewedTopologyPlanId::from_str(&plan_id)?;
            let response: mcg_projects::ReviewedTopologyPlan = client
                .get(format!("{base}/{plan_id}"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        ReviewedTopologyCommand::Run { plan_id } => {
            let plan_id = ReviewedTopologyPlanId::from_str(&plan_id)?;
            let mutation: ReviewedTopologyPlanMutation = client
                .post(format!("{base}/{plan_id}/run"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&mutation)?);
        }
        ReviewedTopologyCommand::Cancel { plan_id } => {
            let plan_id = ReviewedTopologyPlanId::from_str(&plan_id)?;
            let mutation: ReviewedTopologyPlanMutation = client
                .post(format!("{base}/{plan_id}/cancel"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&mutation)?);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_lines)] // CLI dispatch keeps memory lifecycle commands together.
async fn run_memory(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    project_id: ProjectId,
    command: MemoryCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    let base = format!("{host}/api/v1/projects/{project_id}/memory");
    match command {
        MemoryCommand::Add {
            namespace,
            key,
            value,
            confidence,
            source,
            fresh_until,
        } => {
            let mutation: MemoryClaimMutation = client
                .post(&base)
                .bearer_auth(token)
                .json(&CreateMemoryClaimRequest {
                    namespace,
                    key,
                    value: parse_cli_value(&value),
                    confidence,
                    provenance: owner_provenance(source),
                    fresh_until,
                })
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", mutation.claim.id);
        }
        MemoryCommand::List {
            namespace,
            include_stale,
        } => {
            let mut request = client.get(&base).bearer_auth(token);
            let fresh_only = (!include_stale).to_string();
            request = request.query(&[("fresh_only", fresh_only.as_str())]);
            if let Some(namespace) = namespace.as_deref() {
                request = request.query(&[("namespace", namespace)]);
            }
            let response: MemoryClaimListResponse =
                request.send().await?.error_for_status()?.json().await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        MemoryCommand::Correct {
            claim_id,
            value,
            confidence,
            source,
            fresh_until,
        } => {
            let claim_id = MemoryClaimId::from_str(&claim_id)?;
            let mutation: MemoryClaimMutation = client
                .post(format!("{base}/{claim_id}/correct"))
                .bearer_auth(token)
                .json(&CorrectMemoryClaimRequest {
                    value: parse_cli_value(&value),
                    confidence,
                    provenance: owner_provenance(source),
                    fresh_until,
                })
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", mutation.claim.id);
        }
        MemoryCommand::Transition {
            claim_id,
            action,
            source,
        } => {
            let claim_id = MemoryClaimId::from_str(&claim_id)?;
            let mutation: MemoryClaimMutation = client
                .post(format!("{base}/{claim_id}/transition"))
                .bearer_auth(token)
                .json(&TransitionMemoryClaimRequest {
                    action: action.into(),
                    provenance: owner_provenance(source),
                })
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}: {:?}", mutation.claim.id, mutation.claim.state);
        }
        MemoryCommand::History { namespace, key } => {
            let response: MemoryClaimHistoryResponse = client
                .get(format!("{base}/history"))
                .bearer_auth(token)
                .query(&[("namespace", namespace), ("key", key)])
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        MemoryCommand::TransferPreview { claim_id } => {
            let claim_id = MemoryClaimId::from_str(&claim_id)?;
            let response: ProjectMemoryTransferPreview = client
                .get(format!("{base}/{claim_id}/transfer-preview"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        MemoryCommand::Transfer {
            claim_id,
            target_project_id,
            source_sha256,
            reason,
            transfer_id,
        } => {
            let transfer_id = transfer_id
                .as_deref()
                .map(MemoryTransferId::from_str)
                .transpose()?
                .unwrap_or_else(MemoryTransferId::new);
            let mutation: ProjectMemoryTransferMutation = client
                .post(format!("{base}/transfers"))
                .bearer_auth(token)
                .json(&TransferProjectMemoryRequest {
                    transfer_id,
                    source_claim_id: MemoryClaimId::from_str(&claim_id)?,
                    source_claim_sha256: source_sha256,
                    target_project_id: ProjectId::from_str(&target_project_id)?,
                    reason,
                })
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&mutation)?);
        }
        MemoryCommand::Transfers => {
            let response: ProjectMemoryTransferListResponse = client
                .get(format!("{base}/transfers"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        MemoryCommand::Managed => {
            let response: ManagedMemoryResponse = client
                .get(format!("{base}/managed"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", response.markdown);
        }
    }
    Ok(())
}

fn print_project(project: &mcg_projects::ProjectModel) {
    println!("{}  {}", project.project_id, project.root);
    println!(
        "  languages: {}  commands: {}  conflicts: {}",
        project.languages.join(", "),
        project.commands.len(),
        project.instructions.conflicts.len()
    );
}

fn owner_provenance(source: String) -> ClaimProvenance {
    ClaimProvenance {
        source_kind: ProvenanceKind::User,
        source,
        task_id: None,
        artifact_sha256: None,
    }
}

fn parse_cli_value(value: &str) -> serde_json::Value {
    serde_json::from_str(value).unwrap_or_else(|_| serde_json::Value::String(value.to_owned()))
}

async fn run_approval(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    command: ApprovalCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        ApprovalCommand::List { all, json } => {
            let suffix = if all { "" } else { "?state=pending" };
            let response: ApprovalListResponse = client
                .get(format!("{host}/api/v1/approvals{suffix}"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else if response.approvals.is_empty() {
                println!("No approvals.");
            } else {
                for approval in response.approvals {
                    println!(
                        "{}  {:?}  {}  {}",
                        approval.id, approval.state, approval.action, approval.resource
                    );
                }
            }
        }
        ApprovalCommand::Get { id, json } => {
            let approval: ApprovalRequest = client
                .get(format!("{host}/api/v1/approvals/{id}"))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&approval)?);
            } else {
                println!("{}: {:?}", approval.id, approval.state);
                println!("Action: {}", approval.action);
                println!("Resource: {}", approval.resource);
                println!("Reason: {}", approval.reason);
            }
        }
        ApprovalCommand::Approve { id, reason } => {
            resolve_approval(client, host, token, &id, ApprovalState::Approved, reason).await?;
        }
        ApprovalCommand::Deny { id, reason } => {
            resolve_approval(client, host, token, &id, ApprovalState::Denied, reason).await?;
        }
        ApprovalCommand::Cancel { id, reason } => {
            resolve_approval(client, host, token, &id, ApprovalState::Cancelled, reason).await?;
        }
    }
    Ok(())
}

async fn resolve_approval(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    id: &str,
    state: ApprovalState,
    reason: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let approval: ApprovalRequest = client
        .post(format!("{host}/api/v1/approvals/{id}/resolve"))
        .bearer_auth(token)
        .json(&ResolveApprovalRequest { state, reason })
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    println!("{}: {:?}", approval.id, approval.state);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcg_storage::{BackupBundleEntry, BackupBundleManifest};

    #[test]
    fn hook_selection_tokens_round_trip_exact_identity() {
        let entry = CodexHooksListEntry {
            cwd: "C:\\workspace with unicode Ω".into(),
            hooks: Vec::new(),
            warnings: Vec::new(),
            errors: Vec::new(),
        };
        let hook = CodexHookMetadata {
            key: "project:preToolUse:0".into(),
            event_name: "preToolUse".into(),
            handler_type: "command".into(),
            command: Some("cargo check".into()),
            r#async: Some(false),
            server: None,
            tool: None,
            matcher: Some("Bash".into()),
            timeout_sec: 30,
            status_message: None,
            additional_context_limit: None,
            source_path: "C:\\workspace\\.codex\\hooks.json".into(),
            source: "project".into(),
            plugin_id: None,
            display_order: 0,
            enabled: true,
            is_managed: false,
            current_hash: format!("sha256:{}", "a".repeat(64)),
            trust_status: "untrusted".into(),
        };
        let decoded = decode_hook_selection(&encode_hook_selection(&entry, &hook))
            .expect("valid hook selection");
        assert_eq!(decoded.cwd, entry.cwd);
        assert_eq!(decoded.key, hook.key);
        assert_eq!(decoded.current_hash, hook.current_hash);
        assert!(decode_hook_selection("not-base64!").is_err());
    }

    #[test]
    fn hook_trust_bypass_flag_parses_for_interactive_and_session_work() {
        let interactive = Cli::try_parse_from(["mcg", "--dangerously-bypass-hook-trust"])
            .expect("interactive hook bypass flag");
        assert!(interactive.dangerously_bypass_hook_trust);

        let exec = CodexCompatCli::try_parse_from([
            "mcg",
            "exec",
            "--dangerously-bypass-hook-trust",
            "check hooks",
        ])
        .expect("exec hook bypass flag");
        assert!(exec.dangerously_bypass_hook_trust);
        assert!(matches!(exec.command, CodexCompatCommand::Exec(_)));

        let resume = CodexCompatCli::try_parse_from([
            "mcg",
            "--dangerously-bypass-hook-trust",
            "resume",
            "conversation",
        ])
        .expect("resume hook bypass flag");
        assert!(resume.dangerously_bypass_hook_trust);
        assert!(matches!(resume.command, CodexCompatCommand::Resume(_)));
    }

    #[test]
    fn invocation_remote_flags_parse_at_root_and_supported_session_commands() {
        let root = Cli::try_parse_from(["mcg", "--remote", "ws://127.0.0.1:4500"])
            .expect("root remote flag");
        assert_eq!(root.remote.remote.as_deref(), Some("ws://127.0.0.1:4500"));

        let resume = CodexCompatCli::try_parse_from([
            "mcg",
            "resume",
            "--remote",
            "wss://codex.example.test:443",
            "conversation",
        ])
        .expect("resume remote flag");
        let CodexCompatCommand::Resume(args) = resume.command else {
            panic!("resume command");
        };
        assert_eq!(
            args.remote.remote.as_deref(),
            Some("wss://codex.example.test:443")
        );

        let archive = CodexCompatCli::try_parse_from([
            "mcg",
            "archive",
            "conversation",
            "--remote",
            "ws://localhost:4500",
        ])
        .expect("archive remote flag");
        let CodexCompatCommand::Archive(args) = archive.command else {
            panic!("archive command");
        };
        assert_eq!(args.remote.remote.as_deref(), Some("ws://localhost:4500"));

        assert!(codex_compat_command_requested_from(
            ["--remote", "ws://127.0.0.1:4500", "resume", "conversation",]
                .into_iter()
                .map(OsString::from),
        ));
        assert!(codex_compat_command_requested_from(
            [
                "--remote=wss://codex.example.test:443",
                "archive",
                "conversation",
            ]
            .into_iter()
            .map(OsString::from),
        ));
    }

    #[test]
    fn invocation_remote_resolution_enforces_security_and_subcommand_precedence() {
        assert!(
            resolve_invocation_remote(
                InteractiveRemoteArgs::default(),
                InteractiveRemoteArgs {
                    remote: None,
                    remote_auth_token_env: Some("MISSING".into()),
                },
            )
            .is_err()
        );
        assert!(
            resolve_invocation_remote(
                InteractiveRemoteArgs {
                    remote: Some("ws://example.test:4500".into()),
                    remote_auth_token_env: None,
                },
                InteractiveRemoteArgs::default(),
            )
            .is_err()
        );
        let resolved = resolve_invocation_remote(
            InteractiveRemoteArgs {
                remote: Some("ws://127.0.0.1:4500".into()),
                remote_auth_token_env: None,
            },
            InteractiveRemoteArgs {
                remote: Some("wss://codex.example.test:443".into()),
                remote_auth_token_env: None,
            },
        )
        .expect("secure remote")
        .expect("remote selection");
        assert_eq!(resolved.endpoint, "wss://codex.example.test:443");
    }

    fn cli_update_backup(backup_id: uuid::Uuid) -> BackupSummaryResponse {
        BackupSummaryResponse {
            backup_id,
            file_name: format!("{backup_id}.mcgb"),
            sha256: "a".repeat(64),
            size_bytes: 1_024,
            manifest: BackupBundleManifest {
                schema_version: 2,
                application_version: env!("CARGO_PKG_VERSION").into(),
                created_at: Utc::now(),
                secrets_included: false,
                secret_encryption: None,
                entries: vec![BackupBundleEntry {
                    path: "state/mcg.sqlite3".into(),
                    size_bytes: 512,
                    sha256: "b".repeat(64),
                }],
            },
        }
    }

    #[test]
    fn cli_update_backup_must_be_current_fresh_and_credential_free() {
        let backup_id = uuid::Uuid::new_v4();
        let now = Utc::now();
        let valid = cli_update_backup(backup_id);
        assert!(validate_cli_update_backup(&valid, backup_id, now, now).is_ok());

        let mut stale = valid.clone();
        stale.manifest.created_at = now - TimeDelta::minutes(10);
        assert!(validate_cli_update_backup(&stale, backup_id, now, now).is_err());

        let mut secret = valid.clone();
        secret.manifest.secrets_included = true;
        assert!(validate_cli_update_backup(&secret, backup_id, now, now).is_err());

        let mut wrong_version = valid.clone();
        wrong_version.manifest.application_version = "999.0.0".into();
        assert!(validate_cli_update_backup(&wrong_version, backup_id, now, now).is_err());

        let mut missing_database = valid;
        missing_database.manifest.entries.clear();
        assert!(validate_cli_update_backup(&missing_database, backup_id, now, now).is_err());
    }

    #[test]
    fn native_review_turn_renders_only_the_latest_human_finding() {
        let turn = serde_json::json!({
            "id": "turn-1",
            "status": "completed",
            "items": [
                {"id":"reasoning","type":"reasoning","summary":[]},
                {"id":"message-1","type":"agentMessage","text":"First draft"},
                {"id":"message-2","type":"agentMessage","text":"## Findings\n\nNo issues found."}
            ]
        });
        assert_eq!(
            render_codex_review_turn(&turn).expect("review text"),
            "## Findings\n\nNo issues found."
        );
    }

    #[test]
    fn failed_or_empty_native_review_turns_fail_visibly() {
        let failed = serde_json::json!({
            "status": "failed",
            "error": {"message": "review engine unavailable"},
            "items": []
        });
        assert_eq!(
            render_codex_review_turn(&failed)
                .expect_err("failed review")
                .to_string(),
            "review engine unavailable"
        );
        assert!(
            render_codex_review_turn(&serde_json::json!({"status":"completed","items":[]}))
                .is_err()
        );
    }

    #[test]
    fn resume_candidate_filter_honors_project_and_non_interactive_switches() {
        let project = ProjectId::new();
        let other_project = ProjectId::new();
        let mut interactive = Conversation::new("Interactive", Some(project));
        interactive.interaction_kind = ConversationInteractionKind::Interactive;
        let mut non_interactive = Conversation::new("Exec", Some(project));
        non_interactive.interaction_kind = ConversationInteractionKind::NonInteractive;
        let other = Conversation::new("Other", Some(other_project));

        let mut default_candidates =
            vec![interactive.clone(), non_interactive.clone(), other.clone()];
        filter_cli_session_candidates(&mut default_candidates, Some(project), false);
        assert_eq!(default_candidates, vec![interactive.clone()]);

        let mut inclusive = vec![interactive, non_interactive.clone(), other];
        filter_cli_session_candidates(&mut inclusive, Some(project), true);
        assert_eq!(inclusive.len(), 2);
        assert!(inclusive.contains(&non_interactive));
    }

    #[test]
    fn consumer_launch_defaults_allow_complete_generated_file_calls() {
        let request = default_launch_request();
        assert_eq!(request.context_token_budget, 8_192);
        assert_eq!(request.estimated_output_tokens, 8_192);
        assert_eq!(request.max_model_calls, 16);
    }

    #[test]
    fn consumer_modes_preserve_bare_project_json_stream_and_interactive_forms() {
        let bare =
            Cli::try_parse_from(["mcg", "fix the failing tests"]).expect("bare consumer goal");
        assert!(matches!(
            bare.command,
            Some(Command::Prompt(words))
                if words == vec![OsString::from("fix the failing tests")]
        ));

        let structured = Cli::try_parse_from([
            "mcg",
            "-C",
            "fixture-project",
            "run",
            "--json",
            "--model",
            "gpt-5.6-luna",
            "--effort",
            "medium",
            "--approval",
            "full-access",
            "fix",
            "issue",
            "419",
        ])
        .expect("structured consumer goal");
        assert_eq!(structured.project, Some(PathBuf::from("fixture-project")));
        assert!(matches!(
            structured.command,
            Some(Command::Run {
                command: None,
                json: true,
                stream_json: false,
                model: Some(model),
                effort: Some(effort),
                approval: ConversationApprovalArg::FullAccess,
                goal,
                ..
            }) if model == "gpt-5.6-luna" && effort == "medium" && goal == ["fix", "issue", "419"]
        ));

        let streaming = Cli::try_parse_from(["mcg", "run", "--stream-json", "build a website"])
            .expect("streaming consumer goal");
        assert!(matches!(
            streaming.command,
            Some(Command::Run {
                command: None,
                json: false,
                stream_json: true,
                ..
            })
        ));

        let interactive = Cli::try_parse_from(["mcg"]).expect("interactive mode");
        assert!(interactive.command.is_none());
        assert!(Cli::try_parse_from(["mcg", "run", "--json", "--stream-json", "invalid"]).is_err());
    }

    #[test]
    fn evaluation_file_command_is_available_for_replayable_evidence() {
        let cli = Cli::try_parse_from([
            "mcg",
            "run",
            "evaluate",
            "00000000-0000-0000-0000-000000000001",
            "evaluation.json",
        ])
        .expect("evaluation command");
        assert!(matches!(
            cli.command.expect("command"),
            Command::Run {
                command: Some(RunCommand::Evaluate { id, file }),
                ..
            } if id == "00000000-0000-0000-0000-000000000001"
                && file == std::path::Path::new("evaluation.json")
        ));
    }

    #[test]
    fn run_replay_command_supports_machine_readable_evidence() {
        let cli = Cli::try_parse_from([
            "mcg",
            "run",
            "replay",
            "00000000-0000-0000-0000-000000000001",
            "--json",
        ])
        .expect("replay command");
        assert!(matches!(
            cli.command.expect("command"),
            Command::Run {
                command: Some(RunCommand::Replay { id, json: true }),
                ..
            } if id == "00000000-0000-0000-0000-000000000001"
        ));
    }

    #[test]
    fn organization_inheritance_command_requires_a_named_base() {
        let cli = Cli::try_parse_from([
            "mcg",
            "organization",
            "inherit",
            "00000000-0000-0000-0000-000000000001",
            "--name",
            "Website specialists",
            "--json",
        ])
        .expect("organization inheritance command");
        assert!(matches!(
            cli.command.expect("command"),
            Command::Organization {
                command: OrganizationCommand::Inherit {
                    base_id,
                    name,
                    json: true,
                }
            } if base_id == "00000000-0000-0000-0000-000000000001"
                && name == "Website specialists"
        ));
    }

    #[test]
    fn organization_portability_commands_preserve_explicit_file_and_import_identity() {
        let import_id = "00000000-0000-0000-0000-000000000042";
        let cli = Cli::try_parse_from([
            "mcg",
            "organization",
            "import",
            "portable.json",
            "--import-id",
            import_id,
            "--name",
            "Imported team",
            "--json",
        ])
        .expect("organization import command");
        assert!(matches!(
            cli.command.expect("command"),
            Command::Organization {
                command: OrganizationCommand::Import {
                    file,
                    import_id: Some(parsed),
                    name: Some(name),
                    json: true,
                }
            } if file == std::path::Path::new("portable.json")
                && parsed == import_id
                && name == "Imported team"
        ));

        let compare = Cli::try_parse_from([
            "mcg",
            "organization",
            "compare",
            "00000000-0000-0000-0000-000000000001",
            "00000000-0000-0000-0000-000000000002",
            "--json",
        ])
        .expect("organization compare command");
        assert!(matches!(
            compare.command.expect("command"),
            Command::Organization {
                command: OrganizationCommand::Compare { json: true, .. }
            }
        ));

        let history = Cli::try_parse_from([
            "mcg",
            "organization",
            "history",
            "00000000-0000-0000-0000-000000000001",
            "--json",
        ])
        .expect("organization history command");
        assert!(matches!(
            history.command.expect("command"),
            Command::Organization {
                command: OrganizationCommand::History { json: true, .. }
            }
        ));
    }

    #[test]
    fn signed_organization_sharing_commands_preserve_review_authority() {
        let trust = Cli::try_parse_from([
            "mcg",
            "organization",
            "trust-publisher",
            "publisher.example",
            "--display-name",
            "Example Publisher",
            "--public-key-base64",
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
            "--registry",
            "https://registry.example/",
            "--reason",
            "Owner reviewed the publisher fingerprint.",
            "--json",
        ])
        .expect("publisher trust command");
        assert!(matches!(
            trust.command.expect("command"),
            Command::Organization {
                command: OrganizationCommand::TrustPublisher {
                    publisher_id,
                    registries,
                    json: true,
                    ..
                }
            } if publisher_id == "publisher.example"
                && registries == ["https://registry.example/"]
        ));

        let review_id = "00000000-0000-0000-0000-000000000043";
        let review = Cli::try_parse_from([
            "mcg",
            "organization",
            "review-share",
            "shared-organization.json",
            "--review-id",
            review_id,
            "--json",
        ])
        .expect("share review command");
        assert!(matches!(
            review.command.expect("command"),
            Command::Organization {
                command: OrganizationCommand::ReviewShare {
                    review_id: Some(parsed),
                    json: true,
                    ..
                }
            } if parsed == review_id
        ));

        let import = Cli::try_parse_from([
            "mcg",
            "organization",
            "import-share",
            review_id,
            "--import-id",
            "00000000-0000-0000-0000-000000000044",
            "--reason",
            "Owner reviewed the signed organization.",
            "--json",
        ])
        .expect("share import command");
        assert!(matches!(
            import.command.expect("command"),
            Command::Organization {
                command: OrganizationCommand::ImportShare {
                    review_id: parsed,
                    import_id: Some(_),
                    json: true,
                    ..
                }
            } if parsed == review_id
        ));
    }

    #[test]
    fn reviewed_topology_commands_keep_review_and_execution_separate() {
        let create = Cli::try_parse_from([
            "mcg",
            "project",
            "reviewed",
            "00000000-0000-0000-0000-000000000001",
            "create",
            "reviewed-plan.json",
        ])
        .expect("reviewed plan create");
        assert!(matches!(
            create.command.expect("command"),
            Command::Project {
                command: ProjectCommand::Reviewed {
                    command: ReviewedTopologyCommand::Create { .. },
                    ..
                }
            }
        ));
        let run = Cli::try_parse_from([
            "mcg",
            "project",
            "reviewed",
            "00000000-0000-0000-0000-000000000001",
            "run",
            "00000000-0000-0000-0000-000000000002",
        ])
        .expect("reviewed plan run");
        assert!(matches!(
            run.command.expect("command"),
            Command::Project {
                command: ProjectCommand::Reviewed {
                    command: ReviewedTopologyCommand::Run { .. },
                    ..
                }
            }
        ));
    }

    #[test]
    fn project_memory_accepts_an_explicit_rfc3339_freshness_deadline() {
        let cli = Cli::try_parse_from([
            "mcg",
            "project",
            "memory",
            "00000000-0000-0000-0000-000000000001",
            "add",
            "project",
            "temporary_endpoint",
            "http://127.0.0.1:4173",
            "--fresh-until",
            "2026-08-20T12:00:00Z",
        ])
        .expect("memory freshness deadline");
        assert!(matches!(
            cli.command.expect("command"),
            Command::Project {
                command: ProjectCommand::Memory {
                    command: MemoryCommand::Add { fresh_until: Some(deadline), .. },
                    ..
                }
            } if deadline == "2026-08-20T12:00:00Z".parse::<DateTime<Utc>>().expect("deadline")
        ));
        assert!(
            Cli::try_parse_from([
                "mcg",
                "project",
                "memory",
                "00000000-0000-0000-0000-000000000001",
                "add",
                "project",
                "temporary_endpoint",
                "value",
                "--fresh-until",
                "tomorrow",
            ])
            .is_err()
        );
    }

    #[test]
    fn project_memory_transfer_requires_review_evidence_and_supports_stable_retry_identity() {
        let cli = Cli::try_parse_from([
            "mcg",
            "project",
            "memory",
            "00000000-0000-0000-0000-000000000001",
            "transfer",
            "00000000-0000-0000-0000-000000000002",
            "00000000-0000-0000-0000-000000000003",
            "--source-sha256",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "--reason",
            "Both projects share the reviewed convention",
            "--transfer-id",
            "00000000-0000-0000-0000-000000000004",
        ])
        .expect("memory transfer command");
        assert!(matches!(
            cli.command.expect("command"),
            Command::Project {
                command: ProjectCommand::Memory {
                    project_id,
                    command: MemoryCommand::Transfer {
                        claim_id,
                        target_project_id,
                        source_sha256,
                        reason,
                        transfer_id: Some(transfer_id),
                    },
                }
            } if project_id == "00000000-0000-0000-0000-000000000001"
                && claim_id == "00000000-0000-0000-0000-000000000002"
                && target_project_id == "00000000-0000-0000-0000-000000000003"
                && source_sha256.len() == 64
                && reason == "Both projects share the reviewed convention"
                && transfer_id == "00000000-0000-0000-0000-000000000004"
        ));
        assert!(
            Cli::try_parse_from([
                "mcg",
                "project",
                "memory",
                "00000000-0000-0000-0000-000000000001",
                "transfer",
                "00000000-0000-0000-0000-000000000002",
                "00000000-0000-0000-0000-000000000003",
                "--reason",
                "missing digest",
            ])
            .is_err()
        );
    }

    #[test]
    fn project_plan_accepts_one_exact_project_id() {
        let project_id = "00000000-0000-0000-0000-000000000001";
        let cli = Cli::try_parse_from(["mcg", "project", "plan", project_id])
            .expect("project plan command");
        assert!(matches!(
            cli.command.expect("command"),
            Command::Project {
                command: ProjectCommand::Plan { id }
            } if id == project_id
        ));
        assert!(Cli::try_parse_from(["mcg", "project", "plan"]).is_err());
        assert!(Cli::try_parse_from(["mcg", "project", "plan", project_id, "extra"]).is_err());
    }

    #[test]
    fn memory_values_accept_plain_consumer_strings_and_structured_json() {
        assert_eq!(parse_cli_value("pnpm"), serde_json::json!("pnpm"));
        assert_eq!(
            parse_cli_value(r#"{"enabled":true}"#),
            serde_json::json!({"enabled": true})
        );
    }

    #[test]
    fn plugin_commands_expose_explicit_install_grant_enable_and_invoke_steps() {
        let install = Cli::try_parse_from([
            "mcg",
            "plugin",
            "install-local",
            "manifest.json",
            "plugin.wasm",
        ])
        .expect("plugin install command");
        assert!(matches!(
            install.command,
            Some(Command::Plugin {
                command: PluginCommand::InstallLocal { .. }
            })
        ));

        let registry = Cli::try_parse_from([
            "mcg",
            "plugin",
            "install-registry",
            "signed-package.json",
            "--json",
        ])
        .expect("verified registry plugin install command");
        assert!(matches!(
            registry.command,
            Some(Command::Plugin {
                command: PluginCommand::InstallRegistry { json: true, .. }
            })
        ));

        let enabled = Cli::try_parse_from([
            "mcg",
            "plugin",
            "set-enabled",
            "example.plugin",
            "3",
            "true",
        ])
        .expect("plugin enable command");
        assert!(matches!(
            enabled.command,
            Some(Command::Plugin {
                command: PluginCommand::SetEnabled {
                    expected_revision: 3,
                    enabled: true,
                    ..
                }
            })
        ));

        let invoke = Cli::try_parse_from([
            "mcg",
            "plugin",
            "invoke",
            "example.plugin",
            "input.json",
            "--invocation-id",
            "00000000-0000-4000-8000-000000000099",
            "--task-id",
            "00000000-0000-4000-8000-000000000001",
            "--json",
        ])
        .expect("plugin invoke command");
        assert!(matches!(
            invoke.command,
            Some(Command::Plugin {
                command: PluginCommand::Invoke {
                    invocation_id: Some(_),
                    task_id: Some(_),
                    json: true,
                    ..
                }
            })
        ));
    }

    #[test]
    fn scheduler_commands_expose_simple_once_and_interval_triggers() {
        let schedule_id = "00000000-0000-4000-8000-000000000099";
        let interval = Cli::try_parse_from([
            "mcg",
            "schedule",
            "create-interval",
            "health",
            "00000000-0000-0000-0000-000000000001",
            "check site",
            "300",
            "--schedule-id",
            schedule_id,
        ])
        .expect("interval command");
        assert!(matches!(
            interval.command.expect("command"),
            Command::Schedule {
                command: ScheduleCommand::CreateInterval {
                    seconds: 300,
                    schedule_id: Some(id),
                    ..
                }
            } if id == schedule_id
        ));
        let once = Cli::try_parse_from([
            "mcg",
            "schedule",
            "create-once",
            "release",
            "00000000-0000-0000-0000-000000000001",
            "verify release",
            "2026-08-15T03:00:00Z",
        ])
        .expect("once command");
        assert!(matches!(
            once.command.expect("command"),
            Command::Schedule {
                command: ScheduleCommand::CreateOnce { .. }
            }
        ));
    }

    #[test]
    fn scheduler_commands_expose_advanced_triggers() {
        let project_id = "00000000-0000-0000-0000-000000000001";
        let cron = Cli::try_parse_from([
            "mcg",
            "schedule",
            "create-cron",
            "nightly",
            project_id,
            "verify site",
            "0 2 * * *",
        ])
        .expect("cron command");
        assert!(matches!(
            cron.command.expect("command"),
            Command::Schedule {
                command: ScheduleCommand::CreateCron { expression, .. }
            } if expression == "0 2 * * *"
        ));
        let watch = Cli::try_parse_from([
            "mcg",
            "schedule",
            "create-watch",
            "docs",
            project_id,
            "review docs",
            "README.md",
            "--poll-seconds",
            "15",
        ])
        .expect("watch command");
        assert!(matches!(
            watch.command.expect("command"),
            Command::Schedule {
                command: ScheduleCommand::CreateWatch {
                    poll_seconds: 15,
                    ..
                }
            }
        ));
        let event = Cli::try_parse_from([
            "mcg",
            "schedule",
            "create-event",
            "failures",
            project_id,
            "triage failure",
            "task_failed",
        ])
        .expect("event command");
        assert!(matches!(
            event.command.expect("command"),
            Command::Schedule {
                command: ScheduleCommand::CreateEvent { event_kind, .. }
            } if event_kind == "task_failed"
        ));
        let heartbeat = Cli::try_parse_from([
            "mcg",
            "schedule",
            "create-heartbeat",
            "health",
            project_id,
            "inspect health",
            "--idle-seconds",
            "600",
            "--busy-seconds",
            "60",
        ])
        .expect("heartbeat command");
        assert!(matches!(
            heartbeat.command.expect("command"),
            Command::Schedule {
                command: ScheduleCommand::CreateHeartbeat {
                    idle_seconds: 600,
                    busy_seconds: 60,
                    ..
                }
            }
        ));
    }

    #[test]
    fn scheduler_commands_expose_wake_and_notification_controls() {
        let project_id = "00000000-0000-0000-0000-000000000001";
        let wake = Cli::try_parse_from([
            "mcg",
            "schedule",
            "wake",
            project_id,
            "agents disagree",
            "--severity",
            "high",
        ])
        .expect("wake command");
        assert!(matches!(
            wake.command.expect("command"),
            Command::Schedule {
                command: ScheduleCommand::Wake {
                    severity: NotificationSeverityArg::High,
                    ..
                }
            }
        ));
        let notifications =
            Cli::try_parse_from(["mcg", "schedule", "notifications", "--all", "--json"])
                .expect("notifications command");
        assert!(matches!(
            notifications.command.expect("command"),
            Command::Schedule {
                command: ScheduleCommand::Notifications {
                    all: true,
                    json: true
                }
            }
        ));
    }

    #[test]
    fn platform_commands_preserve_retry_identity_and_explicit_origins() {
        let session_id = "00000000-0000-4000-8000-000000000088";
        let request = Cli::try_parse_from([
            "mcg",
            "platform",
            "request-browser",
            "research",
            "--session-id",
            session_id,
            "--origin",
            "https://example.com",
            "--lifetime-seconds",
            "900",
        ])
        .expect("platform request");
        assert!(matches!(
            request.command.expect("command"),
            Command::Platform {
                command: PlatformCommand::RequestBrowser {
                    session_id: Some(id),
                    origins,
                    lifetime_seconds: 900,
                    ..
                }
            } if id == session_id && origins == ["https://example.com"]
        ));
        let capabilities = Cli::try_parse_from(["mcg", "platform", "capabilities", "--json"])
            .expect("capabilities");
        assert!(matches!(
            capabilities.command.expect("command"),
            Command::Platform {
                command: PlatformCommand::Capabilities { json: true }
            }
        ));
        let attach = Cli::try_parse_from(["mcg", "platform", "attach", session_id, "--visible"])
            .expect("attach");
        assert!(matches!(
            attach.command.expect("command"),
            Command::Platform {
                command: PlatformCommand::Attach { visible: true, .. }
            }
        ));
        let navigate = Cli::try_parse_from([
            "mcg",
            "platform",
            "navigate",
            session_id,
            "https://example.com/result",
            "--json",
        ])
        .expect("navigate");
        assert!(matches!(
            navigate.command.expect("command"),
            Command::Platform {
                command: PlatformCommand::Navigate { json: true, .. }
            }
        ));
    }

    #[test]
    fn site_commands_preserve_caller_identity_and_structured_deployment() {
        let site_id = "00000000-0000-4000-8000-000000000099";
        let project_id = "00000000-0000-4000-8000-000000000001";
        let register = Cli::try_parse_from([
            "mcg",
            "site",
            "register",
            "Production",
            project_id,
            "https://example.com",
            "https://example.com/health",
            "fixture",
            "--site-id",
            site_id,
            "--deploy-executable",
            "npm",
            "--deploy-arg",
            "run",
            "--deploy-arg",
            "deploy",
            "--deploy-cwd",
            "web",
        ])
        .expect("Site registration command");
        assert!(matches!(
            register.command.expect("command"),
            Command::Site {
                command: SiteCommand::Register {
                    site_id: Some(id),
                    deploy_executable: Some(executable),
                    deploy_arguments,
                    deploy_cwd,
                    ..
                }
            } if id == site_id
                && executable == "npm"
                && deploy_arguments == ["run", "deploy"]
                && deploy_cwd == "web"
        ));
        let deploy = Cli::try_parse_from([
            "mcg",
            "site",
            "deploy",
            site_id,
            "--deployment-id",
            "00000000-0000-4000-8000-000000000100",
            "--json",
        ])
        .expect("Site deployment command");
        assert!(matches!(
            deploy.command.expect("command"),
            Command::Site {
                command: SiteCommand::Deploy { json: true, .. }
            }
        ));
        let monitor = Cli::try_parse_from([
            "mcg",
            "site",
            "monitor",
            site_id,
            "--schedule-id",
            "00000000-0000-4000-8000-000000000101",
            "--interval-seconds",
            "60",
            "--expected-status",
            "204",
        ])
        .expect("Site monitor command");
        assert!(matches!(
            monitor.command.expect("command"),
            Command::Site {
                command: SiteCommand::Monitor {
                    interval_seconds: 60,
                    expected_status: 204,
                    ..
                }
            }
        ));
    }

    #[test]
    fn ssh_workflow_commands_expose_create_inspect_run_and_cancel() {
        let workflow_id = "00000000-0000-4000-8000-000000000211";
        let create = Cli::try_parse_from(["mcg", "ssh", "workflow-create", "workflow.json"])
            .expect("workflow create");
        assert!(matches!(
            create.command.expect("command"),
            Command::Ssh {
                command: SshCommand::WorkflowCreate { request }
            } if request.as_path() == std::path::Path::new("workflow.json")
        ));
        for (verb, expected) in [
            ("workflow-show", "show"),
            ("workflow-run", "run"),
            ("workflow-cancel", "cancel"),
        ] {
            let parsed =
                Cli::try_parse_from(["mcg", "ssh", verb, workflow_id]).expect("workflow command");
            let matched = match parsed.command.expect("command") {
                Command::Ssh {
                    command: SshCommand::WorkflowShow { id },
                } => expected == "show" && id == workflow_id,
                Command::Ssh {
                    command: SshCommand::WorkflowRun { id },
                } => expected == "run" && id == workflow_id,
                Command::Ssh {
                    command: SshCommand::WorkflowCancel { id },
                } => expected == "cancel" && id == workflow_id,
                _ => false,
            };
            assert!(matched, "{verb} did not preserve workflow identity");
        }
        let list = Cli::try_parse_from(["mcg", "ssh", "workflow-list"]).expect("workflow list");
        assert!(matches!(
            list.command.expect("command"),
            Command::Ssh {
                command: SshCommand::WorkflowList
            }
        ));
    }

    #[test]
    fn experiment_commands_keep_versioned_json_and_compare_and_set_state_explicit() {
        let candidate = Cli::try_parse_from(["mcg", "experiment", "candidate", "candidate.json"])
            .expect("candidate command");
        assert!(matches!(
            candidate.command.expect("command"),
            Command::Experiment {
                command: ExperimentCommand::Candidate { request }
            } if request == std::path::Path::new("candidate.json")
        ));
        let transition = Cli::try_parse_from([
            "mcg",
            "experiment",
            "transition",
            "00000000-0000-4000-8000-000000000001",
            "running",
            "awaiting-decision",
        ])
        .expect("transition command");
        assert!(matches!(
            transition.command.expect("command"),
            Command::Experiment {
                command: ExperimentCommand::Transition {
                    expected: ExperimentStateArg::Running,
                    next: ExperimentStateArg::AwaitingDecision,
                    ..
                }
            }
        ));
    }

    #[test]
    fn settings_commands_expose_safe_live_owner_controls() {
        let set = Cli::try_parse_from([
            "mcg",
            "settings",
            "set",
            "--advanced-mode",
            "true",
            "--autonomy",
            "guided",
            "--shadow-experiments",
            "false",
            "--reason",
            "owner requested guided work",
        ])
        .expect("settings set command");
        assert!(matches!(
            set.command.expect("command"),
            Command::Settings {
                command: SettingsCommand::Set {
                    advanced_mode: Some(true),
                    autonomy: Some(AutonomyArg::Guided),
                    shadow_experiments: Some(false),
                    ..
                }
            }
        ));
        let developer = Cli::try_parse_from([
            "mcg",
            "settings",
            "set",
            "--interface-complexity",
            "developer",
        ])
        .expect("developer interface setting");
        assert!(matches!(
            developer.command.expect("command"),
            Command::Settings {
                command: SettingsCommand::Set {
                    interface_complexity: Some(InterfaceComplexityArg::Developer),
                    ..
                }
            }
        ));
        let history = Cli::try_parse_from(["mcg", "settings", "history", "--json"])
            .expect("settings history command");
        assert!(matches!(
            history.command.expect("command"),
            Command::Settings {
                command: SettingsCommand::History { json: true }
            }
        ));
    }

    #[test]
    fn backup_commands_separate_online_creation_from_confirmed_offline_restore() {
        let create = Cli::try_parse_from([
            "mcg",
            "backup",
            "create",
            "--backup-id",
            "00000000-0000-4000-8000-000000000401",
            "--json",
        ])
        .expect("backup create command");
        assert!(matches!(
            create.command.expect("command"),
            Command::Backup {
                command: BackupCommand::Create {
                    backup_id: Some(_),
                    include_secrets: false,
                    passphrase_stdin: false,
                    json: true
                }
            }
        ));
        let restore = Cli::try_parse_from(["mcg", "backup", "restore", "owner.mcgb", "--confirm"])
            .expect("backup restore command");
        assert!(matches!(
            restore.command.expect("command"),
            Command::Backup {
                command: BackupCommand::Restore { confirm: true, .. }
            }
        ));
        let encrypted = Cli::try_parse_from([
            "mcg",
            "backup",
            "restore",
            "owner.mcgb",
            "--confirm",
            "--restore-secrets",
            "--passphrase-stdin",
        ])
        .expect("encrypted backup restore command");
        assert!(matches!(
            encrypted.command.expect("command"),
            Command::Backup {
                command: BackupCommand::Restore {
                    confirm: true,
                    restore_secrets: true,
                    passphrase_stdin: true,
                    ..
                }
            }
        ));
        assert!(Cli::try_parse_from(["mcg", "backup", "create", "--passphrase-stdin"]).is_err());
        assert!(
            Cli::try_parse_from([
                "mcg",
                "backup",
                "restore",
                "owner.mcgb",
                "--confirm",
                "--passphrase-stdin",
            ])
            .is_err()
        );
    }

    #[test]
    fn stop_supports_human_and_machine_readable_daemon_shutdown() {
        let human = Cli::try_parse_from(["mcg", "stop"]).expect("stop command");
        assert!(matches!(
            human.command.expect("command"),
            Command::Stop { json: false }
        ));
        let json = Cli::try_parse_from(["mcg", "stop", "--json"]).expect("JSON stop command");
        assert!(matches!(
            json.command.expect("command"),
            Command::Stop { json: true }
        ));
        let start = Cli::try_parse_from(["mcg", "start", "--timeout-seconds", "30", "--json"])
            .expect("JSON start command");
        assert!(matches!(
            start.command.expect("command"),
            Command::Start {
                timeout_seconds: 30,
                json: true
            }
        ));
        let restart = Cli::try_parse_from(["mcg", "restart", "--timeout-seconds", "45"])
            .expect("restart command");
        assert!(matches!(
            restart.command.expect("command"),
            Command::Restart {
                timeout_seconds: 45,
                json: false
            }
        ));
    }

    #[test]
    fn doctor_supports_machine_readable_strict_health_gates() {
        let cli =
            Cli::try_parse_from(["mcg", "doctor", "--json", "--strict"]).expect("doctor command");
        assert!(matches!(
            cli.command.expect("command"),
            Command::Doctor {
                json: true,
                strict: true
            }
        ));
    }

    #[test]
    fn runtime_commands_expose_protected_root_status_and_inspection() {
        let status = Cli::try_parse_from(["mcg", "runtime", "status", "--json"])
            .expect("Root status command");
        assert!(matches!(
            status.command.expect("command"),
            Command::Runtime {
                command: RuntimeCommand::Status { json: true }
            }
        ));
        let inspect =
            Cli::try_parse_from(["mcg", "runtime", "inspect"]).expect("Root inspect command");
        assert!(matches!(
            inspect.command.expect("command"),
            Command::Runtime {
                command: RuntimeCommand::Inspect { json: false }
            }
        ));
    }

    #[test]
    fn workflow_commands_expose_canonical_root_inspection_and_cancellation() {
        let id = "00000000-0000-4000-8000-000000000501";
        let list = Cli::try_parse_from(["mcg", "workflow", "list", "--json"])
            .expect("workflow list command");
        assert!(matches!(
            list.command.expect("command"),
            Command::Workflow {
                command: WorkflowCommand::List { json: true }
            }
        ));
        let show =
            Cli::try_parse_from(["mcg", "workflow", "show", id]).expect("workflow show command");
        assert!(matches!(
            show.command.expect("command"),
            Command::Workflow {
                command: WorkflowCommand::Show { json: false, .. }
            }
        ));
        let activity = Cli::try_parse_from([
            "mcg",
            "workflow",
            "activity",
            id,
            "--invocation-limit",
            "25",
            "--json",
        ])
        .expect("workflow activity command");
        assert!(matches!(
            activity.command.expect("command"),
            Command::Workflow {
                command: WorkflowCommand::Activity {
                    invocation_limit: 25,
                    json: true,
                    ..
                }
            }
        ));
        let cancel = Cli::try_parse_from([
            "mcg",
            "workflow",
            "cancel",
            id,
            "--idempotency-key",
            "owner-cancel-1",
            "--json",
        ])
        .expect("workflow cancel command");
        assert!(matches!(
            cancel.command.expect("command"),
            Command::Workflow {
                command: WorkflowCommand::Cancel {
                    idempotency_key: Some(key),
                    json: true,
                    ..
                }
            } if key == "owner-cancel-1"
        ));
    }

    #[test]
    fn action_commands_expose_bounded_canonical_root_evidence() {
        let id = "00000000-0000-4000-8000-000000000502";
        let list = Cli::try_parse_from(["mcg", "action", "list", "--limit", "25", "--json"])
            .expect("action list command");
        assert!(matches!(
            list.command.expect("command"),
            Command::Action {
                command: ActionCommand::List {
                    limit: 25,
                    json: true
                }
            }
        ));
        let compensate = Cli::try_parse_from([
            "mcg",
            "action",
            "compensate",
            id,
            "--reason",
            "Remove the exact owner-approved created file",
            "--confirm",
            "--idempotency-key",
            "owner-compensate-1",
            "--json",
        ])
        .expect("action compensate command");
        assert!(matches!(
            compensate.command.expect("command"),
            Command::Action {
                command: ActionCommand::Compensate {
                    confirm: true,
                    idempotency_key: Some(key),
                    json: true,
                    ..
                }
            } if key == "owner-compensate-1"
        ));
        let show = Cli::try_parse_from(["mcg", "action", "show", id, "--event-limit", "75"])
            .expect("action show command");
        assert!(matches!(
            show.command.expect("command"),
            Command::Action {
                command: ActionCommand::Show {
                    event_limit: 75,
                    json: false,
                    ..
                }
            }
        ));
    }

    #[test]
    fn invocation_commands_expose_bounded_reconstruction_evidence() {
        let id = "00000000-0000-4000-8000-000000000504";
        let list = Cli::try_parse_from(["mcg", "invocation", "list", "--limit", "25", "--json"])
            .expect("invocation list command");
        assert!(matches!(
            list.command.expect("command"),
            Command::Invocation {
                command: InvocationCommand::List {
                    limit: 25,
                    json: true
                }
            }
        ));
        let show =
            Cli::try_parse_from(["mcg", "invocation", "show", id, "--model-run-limit", "75"])
                .expect("invocation show command");
        assert!(matches!(
            show.command.expect("command"),
            Command::Invocation {
                command: InvocationCommand::Show {
                    model_run_limit: 75,
                    json: false,
                    ..
                }
            }
        ));
    }

    #[test]
    fn human_action_origin_output_is_exact_and_content_minimized() {
        let source = |source_type: &str, locator: &str, digest: char| mcg_contracts::SourceRef {
            source_type: mcg_contracts::StableTypeId::parse(source_type).unwrap(),
            locator: locator.into(),
            content_hash: mcg_contracts::ContentHash {
                algorithm: mcg_contracts::HashAlgorithm::Blake3_256,
                domain: "MetaCog.ActionSource".into(),
                digest_hex: std::iter::repeat_n(digest, 64).collect(),
            },
        };
        let origin = mcg_contracts::ActionOrigin {
            conversation: Some(source(
                "mcg.conversation.v1",
                "00000000-0000-4000-8000-000000000503",
                'a',
            )),
            invocation: None,
            command: None,
            approval: Some(source(
                "mcg.approval_request.v1",
                "00000000-0000-4000-8000-000000000504",
                'b',
            )),
            agent: None,
            evidence: Vec::new(),
        };
        let lines = canonical_action_origin_lines(&origin);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("source conversation: mcg.conversation.v1"));
        assert!(lines[0].contains("00000000-0000-4000-8000-000000000503"));
        assert!(lines[1].contains("source approval: mcg.approval_request.v1"));
        assert!(!lines.join("\n").contains("authority"));
    }

    #[test]
    fn conversation_commands_expose_cli_first_agent_workflows() {
        let project_id = "00000000-0000-4000-8000-000000000001";
        let conversation_id = "00000000-0000-4000-8000-000000000002";
        let delegation_id = "00000000-0000-4000-8000-000000000003";
        let create = Cli::try_parse_from([
            "mcg",
            "conversation",
            "create",
            "--title",
            "CLI qualification",
            "--project-id",
            project_id,
            "--model",
            "gpt-5.6-luna",
            "--effort",
            "medium",
            "--speed",
            "standard",
            "--approval",
            "full-access",
        ])
        .expect("conversation create");
        assert!(matches!(
            create.command.expect("command"),
            Command::Conversation {
                command: ConversationCommand::Create {
                    project_id: Some(id),
                    model: Some(model),
                    approval: ConversationApprovalArg::FullAccess,
                    ..
                }
            } if id == project_id && model == "gpt-5.6-luna"
        ));

        let send = Cli::try_parse_from([
            "mcg",
            "conversation",
            "send",
            conversation_id,
            "build the fixture",
            "--mention",
            "file:src/main.rs",
            "--mention",
            "plugin:fixture-plugin",
            "--wait",
            "--timeout-seconds",
            "900",
        ])
        .expect("conversation send");
        assert!(matches!(
            send.command.expect("command"),
            Command::Conversation {
                command: ConversationCommand::Send {
                    wait: true,
                    timeout_seconds: 900,
                    mentions,
                    ..
                }
            } if mentions == vec!["file:src/main.rs", "plugin:fixture-plugin"]
        ));

        let receipt = Cli::try_parse_from(["mcg", "conversation", "receipt", conversation_id])
            .expect("conversation receipt");
        assert!(matches!(
            receipt.command.expect("command"),
            Command::Conversation {
                command: ConversationCommand::Receipt { .. }
            }
        ));

        let review = Cli::try_parse_from([
            "mcg",
            "conversation",
            "review",
            conversation_id,
            delegation_id,
        ])
        .expect("delegation review command");
        assert!(matches!(review.command, Some(Command::Conversation { .. })));
        for verb in ["cancel", "retry", "apply", "discard"] {
            let parsed = Cli::try_parse_from([
                "mcg",
                "conversation",
                verb,
                conversation_id,
                delegation_id,
                "3",
            ])
            .expect("revision-checked delegation lifecycle command");
            assert!(matches!(parsed.command, Some(Command::Conversation { .. })));
        }
    }

    #[test]
    fn codex_github_and_governor_commands_are_available_without_the_gui() {
        let models =
            Cli::try_parse_from(["mcg", "codex", "models"]).expect("Codex model discovery");
        assert!(matches!(
            models.command.expect("command"),
            Command::Codex {
                command: CodexCommand::Models
            }
        ));

        let github = Cli::try_parse_from(["mcg", "github", "login"]).expect("GitHub device login");
        assert!(matches!(
            github.command.expect("command"),
            Command::Github {
                command: GithubCommand::Login
            }
        ));

        let proposal_id = "00000000-0000-4000-8000-000000000010";
        let conversation_id = "00000000-0000-4000-8000-000000000011";
        let implementation = Cli::try_parse_from([
            "mcg",
            "governor",
            "request-implementation",
            proposal_id,
            conversation_id,
        ])
        .expect("governed implementation request");
        assert!(matches!(
            implementation.command.expect("command"),
            Command::Governor {
                command: GovernorCommand::RequestImplementation { .. }
            }
        ));
    }

    #[test]
    fn project_commands_expose_git_and_lifecycle_management() {
        let project_id = "00000000-0000-4000-8000-000000000020";
        for verb in ["init-git", "archive", "remove"] {
            let parsed = Cli::try_parse_from(["mcg", "project", verb, project_id])
                .expect("project lifecycle command");
            assert!(matches!(parsed.command, Some(Command::Project { .. })));
        }
        let add = Cli::try_parse_from(["mcg", "project", "add", ".", "--initialize-git"])
            .expect("project add with Git initialization");
        assert!(matches!(
            add.command.expect("command"),
            Command::Project {
                command: ProjectCommand::Add {
                    initialize_git: true,
                    ..
                }
            }
        ));
    }

    #[test]
    fn codex_compatible_top_level_commands_parse_with_documented_shapes() {
        let app = CodexCompatCli::try_parse_from(["mcg", "app", "."])
            .expect("Codex-compatible desktop handoff");
        assert!(matches!(
            app.command,
            CodexCompatCommand::App(args) if args.path == PathBuf::from(".")
        ));

        for arguments in [
            vec!["mcg", "login", "--device-auth"],
            vec!["mcg", "login", "--with-api-key"],
            vec!["mcg", "login", "--with-access-token"],
        ] {
            let login = CodexCompatCli::try_parse_from(arguments).expect("Codex login shape");
            assert!(matches!(login.command, CodexCompatCommand::Login(_)));
        }
        assert!(
            CodexCompatCli::try_parse_from(["mcg", "login", "--device-auth", "--with-api-key"])
                .is_err()
        );

        let exec = CodexCompatCli::try_parse_from([
            "mcg",
            "e",
            "--json",
            "--ephemeral",
            "--skip-git-repo-check",
            "--model",
            "gpt-5.6-terra",
            "--sandbox",
            "read-only",
            "--ask-for-approval",
            "untrusted",
            "--image",
            "screen.png",
            "--add-dir",
            ".",
            "--output-last-message",
            "answer.txt",
            "--output-schema",
            "schema.json",
            "fix the tests",
        ])
        .expect("Codex-compatible exec command");
        assert!(matches!(
            &exec.command,
            CodexCompatCommand::Exec(args)
                if args.json
                    && args.ephemeral
                    && args.skip_git_repo_check
                    && args.output_last_message.as_deref() == Some(std::path::Path::new("answer.txt"))
                    && args.output_schema.as_deref() == Some(std::path::Path::new("schema.json"))
                    && args.prompt == ["fix the tests"]
        ));
        assert_eq!(exec.model.as_deref(), Some("gpt-5.6-terra"));
        assert!(matches!(exec.sandbox, Some(CodexSandboxArg::ReadOnly)));
        assert!(matches!(
            exec.approval_policy,
            Some(CodexApprovalPolicyArg::Untrusted)
        ));
        assert_eq!(exec.images, [PathBuf::from("screen.png")]);
        assert_eq!(exec.add_dirs, [PathBuf::from(".")]);

        let oss = CodexCompatCli::try_parse_from([
            "mcg",
            "--oss",
            "--local-provider",
            "lmstudio",
            "exec",
            "inspect locally",
        ])
        .expect("Codex-compatible local provider flags");
        assert!(oss.oss);
        assert_eq!(oss.local_provider, Some(CodexLocalProviderArg::LmStudio));
        let native_oss = Cli::try_parse_from([
            "mcg",
            "--oss",
            "--local-provider",
            "ollama",
            "run",
            "inspect locally",
        ])
        .expect("native local provider flags");
        assert!(native_oss.oss);
        assert_eq!(
            native_oss.local_provider,
            Some(CodexLocalProviderArg::Ollama)
        );

        let exec_resume = CodexCompatCli::try_parse_from([
            "mcg",
            "exec",
            "resume",
            "--last",
            "--output-schema",
            "follow-up-schema.json",
            "continue",
        ])
        .expect("Codex-compatible structured exec resume");
        let CodexCompatCommand::Exec(exec_resume_args) = exec_resume.command else {
            panic!("expected exec command");
        };
        assert!(matches!(
            exec_resume_args.command,
            Some(ExecCommand::Resume {
                last: true,
                output_schema: Some(path),
                ..
            }) if path == PathBuf::from("follow-up-schema.json")
        ));

        let before_command = CodexCompatCli::try_parse_from([
            "mcg",
            "--approve-for-me",
            "--model",
            "gpt-5.6-terra",
            "--sandbox",
            "workspace-write",
            "--image",
            "screen.png",
            "--add-dir",
            ".",
            "exec",
            "--json",
            "fix the tests",
        ])
        .expect("global Codex-compatible flags before exec");
        assert_eq!(before_command.model.as_deref(), Some("gpt-5.6-terra"));
        assert!(matches!(
            before_command.sandbox,
            Some(CodexSandboxArg::WorkspaceWrite)
        ));
        assert!(before_command.approval_policy.is_none());
        assert_eq!(before_command.images, [PathBuf::from("screen.png")]);
        assert_eq!(before_command.add_dirs, [PathBuf::from(".")]);
        assert!(before_command.approve_for_me);
        assert!(matches!(
            &before_command.command,
            CodexCompatCommand::Exec(args) if args.json && args.prompt == ["fix the tests"]
        ));

        let (interactive_arguments, interactive_overrides) = extract_native_overrides(
            [
                "mcg",
                "--model",
                "gpt-5.6-luna",
                "--sandbox",
                "read-only",
                "--ask-for-approval",
                "never",
                "--image",
                "diagram.png",
                "--add-dir",
                ".",
                "--enable",
                "memories",
                "--search",
                "--strict-config",
                "--profile",
                "team.fast",
                "--no-alt-screen",
                "-c",
                "web_search=\"disabled\"",
                "--search",
                "--config=features={memories=true, sqlite=false}",
                "-c",
                "model_verbosity=\"low\"",
                "--config=history.persistence=save-all",
                "-cfeatures.sqlite=false",
                "--disable=memories",
                "--enable=sqlite",
            ]
            .into_iter()
            .map(OsString::from),
        )
        .expect("extract interactive writable roots");
        let interactive =
            Cli::try_parse_from(interactive_arguments).expect("interactive root execution flags");
        assert!(interactive.command.is_none());
        assert_eq!(interactive.model.as_deref(), Some("gpt-5.6-luna"));
        assert!(matches!(
            interactive.sandbox,
            Some(CodexSandboxArg::ReadOnly)
        ));
        assert!(matches!(
            interactive.approval_policy,
            Some(CodexApprovalPolicyArg::Never)
        ));
        assert_eq!(interactive.images, [PathBuf::from("diagram.png")]);
        assert_eq!(interactive_overrides.add_dirs, [PathBuf::from(".")]);
        assert!(interactive_overrides.strict_config);
        assert_eq!(
            interactive_overrides.codex_profile.as_deref(),
            Some("team.fast")
        );
        assert!(interactive_overrides.no_alt_screen);
        assert_eq!(
            interactive_overrides.feature_overrides.get("memories"),
            Some(&false)
        );
        assert_eq!(
            interactive_overrides.feature_overrides.get("sqlite"),
            Some(&true)
        );
        assert_eq!(
            interactive_overrides.config_overrides,
            [
                CodexConfigOverride {
                    key: "web_search".into(),
                    value: serde_json::json!("live"),
                },
                CodexConfigOverride {
                    key: "web_search".into(),
                    value: serde_json::json!("disabled"),
                },
                CodexConfigOverride {
                    key: "web_search".into(),
                    value: serde_json::json!("live"),
                },
                CodexConfigOverride {
                    key: "model_verbosity".into(),
                    value: serde_json::json!("low"),
                },
                CodexConfigOverride {
                    key: "history.persistence".into(),
                    value: serde_json::json!("save-all"),
                },
            ]
        );

        let (run_arguments, run_overrides) = extract_native_overrides(
            ["mcg", "run", "--add-dir=.", "fix the tests"]
                .into_iter()
                .map(OsString::from),
        )
        .expect("extract run writable roots");
        let run = Cli::try_parse_from(run_arguments).expect("consumer run after extraction");
        assert!(matches!(
            run.command,
            Some(Command::Run { command: None, .. })
        ));
        assert_eq!(run_overrides.add_dirs, [PathBuf::from(".")]);

        let exact = conversation_preferences_exact(
            None,
            None,
            None,
            ApprovalMode::ReadOnly,
            Some(ExecutionApprovalPolicy::Untrusted),
            Some(ExecutionSandboxMode::ReadOnly),
        );
        assert_eq!(
            exact.approval_policy,
            Some(ExecutionApprovalPolicy::Untrusted)
        );
        assert_eq!(exact.sandbox_mode, Some(ExecutionSandboxMode::ReadOnly));

        let independent = codex_execution_preferences(
            Some("gpt-5.6-terra".to_owned()),
            None,
            Some(CodexSandboxArg::DangerFullAccess),
            Some(CodexApprovalPolicyArg::OnRequest),
            false,
            false,
            &["C:/shared".into()],
            &BTreeMap::from([("memories".into(), true)]),
            &interactive_overrides.config_overrides,
            interactive_overrides.codex_profile.clone(),
            true,
            conversation_preferences(None, None, None, ApprovalMode::Ask),
        );
        assert_eq!(independent.model_id.as_deref(), Some("gpt-5.6-terra"));
        assert_eq!(
            independent.approval_policy,
            Some(ExecutionApprovalPolicy::OnRequest)
        );
        assert_eq!(
            independent.sandbox_mode,
            Some(ExecutionSandboxMode::DangerFullAccess)
        );
        assert_eq!(independent.approval_mode, ApprovalMode::Ask);
        assert_eq!(independent.additional_writable_roots, ["C:/shared"]);
        assert_eq!(
            independent.codex_feature_overrides.get("memories"),
            Some(&true)
        );
        assert_eq!(
            independent.codex_config_overrides,
            interactive_overrides.config_overrides
        );
        assert!(independent.strict_config);

        let local = codex_execution_preferences(
            Some("gpt-oss:20b".into()),
            Some("ollama".into()),
            None,
            None,
            false,
            false,
            &[],
            &BTreeMap::new(),
            &[],
            None,
            false,
            conversation_preferences(None, None, Some("fast".into()), ApprovalMode::Ask),
        );
        assert_eq!(local.model_provider.as_deref(), Some("ollama"));
        assert_eq!(local.model_id.as_deref(), Some("gpt-oss:20b"));
        assert!(local.service_tier.is_none());

        let yolo = codex_execution_preferences(
            None,
            None,
            Some(CodexSandboxArg::ReadOnly),
            Some(CodexApprovalPolicyArg::Untrusted),
            false,
            true,
            &[],
            &BTreeMap::new(),
            &[],
            None,
            false,
            conversation_preferences(None, None, None, ApprovalMode::Ask),
        );
        assert_eq!(yolo.approval_mode, ApprovalMode::FullAccess);
        assert_eq!(yolo.approval_policy, Some(ExecutionApprovalPolicy::Never));
        assert_eq!(
            yolo.sandbox_mode,
            Some(ExecutionSandboxMode::DangerFullAccess)
        );

        let approve_for_me = codex_execution_preferences(
            None,
            None,
            None,
            None,
            true,
            false,
            &[],
            &BTreeMap::new(),
            &[],
            None,
            false,
            conversation_preferences(None, None, None, ApprovalMode::Ask),
        );
        assert_eq!(approve_for_me.approval_mode, ApprovalMode::AutoReview);
        assert_eq!(
            approve_for_me.approval_policy,
            Some(ExecutionApprovalPolicy::OnRequest)
        );
        assert_eq!(
            approve_for_me.sandbox_mode,
            Some(ExecutionSandboxMode::WorkspaceWrite)
        );

        let queue = CodexCompatCli::try_parse_from([
            "mcg",
            "queue",
            "--thread",
            "release work",
            "--message",
            "run the remaining verification",
        ])
        .expect("Codex-compatible queue command");
        assert!(matches!(
            queue.command,
            CodexCompatCommand::Queue(args)
                if args.thread == "release work"
                    && args.message == "run the remaining verification"
        ));

        let agents = CodexCompatCli::try_parse_from(["mcg", "agents"])
            .expect("Codex-compatible agents command");
        assert!(matches!(agents.command, CodexCompatCommand::Agents(_)));
        let remote_agents = CodexCompatCli::try_parse_from([
            "mcg",
            "agents",
            "--remote",
            "wss://codex.example.test:443",
            "--remote-auth-token-env",
            "CODEX_REMOTE_TOKEN",
        ])
        .expect("Codex-compatible remote agents command");
        let CodexCompatCommand::Agents(remote_agents_args) = &remote_agents.command else {
            panic!("remote agents command did not preserve its argument group");
        };
        assert_eq!(
            remote_agents_args.remote.remote.as_deref(),
            Some("wss://codex.example.test:443")
        );
        assert_eq!(
            remote_agents_args.remote.remote_auth_token_env.as_deref(),
            Some("CODEX_REMOTE_TOKEN")
        );

        let apply = CodexCompatCli::try_parse_from(["mcg", "a", "task-123"])
            .expect("Codex-compatible apply alias");
        assert!(matches!(
            apply.command,
            CodexCompatCommand::Apply(ApplyArgs { task_id }) if task_id == "task-123"
        ));
        let update = CodexCompatCli::try_parse_from(["mcg", "update"])
            .expect("Codex-compatible update command");
        assert!(matches!(update.command, CodexCompatCommand::Update));

        let migrate = CodexCompatCli::try_parse_from([
            "mcg",
            "migrate-rollouts",
            "--thread",
            "thread-a",
            "--thread",
            "thread-b",
            "--max-mib-per-second",
            "8",
            "--json",
            "--verbose",
            "--apply",
        ])
        .expect("Codex-compatible rollout migration command");
        assert!(matches!(
            migrate.command,
            CodexCompatCommand::MigrateRollouts(args)
                if args.apply
                    && args.json
                    && args.verbose
                    && args.threads == ["thread-a", "thread-b"]
                    && args.max_mib_per_second == Some(8)
        ));

        let resume = CodexCompatCli::try_parse_from([
            "mcg",
            "resume",
            "--last",
            "--all",
            "--include-non-interactive",
        ])
        .expect("Codex-compatible resume command");
        assert!(matches!(
            resume.command,
            CodexCompatCommand::Resume(args)
                if args.last && args.all && args.include_non_interactive
        ));

        let review = CodexCompatCli::try_parse_from([
            "mcg",
            "review",
            "--commit",
            "abc123",
            "--title",
            "Fix regression",
        ])
        .expect("Codex-compatible review command");
        assert!(matches!(
            review.command,
            CodexCompatCommand::Review(args)
                if args.commit.as_deref() == Some("abc123")
                    && args.title.as_deref() == Some("Fix regression")
        ));
        assert!(
            CodexCompatCli::try_parse_from(["mcg", "review", "--uncommitted", "--base", "main",])
                .is_err()
        );

        let completion = CodexCompatCli::try_parse_from(["mcg", "completion", "power-shell"])
            .expect("official PowerShell completion spelling");
        assert!(matches!(
            completion.command,
            CodexCompatCommand::Completion {
                shell: CompletionShell::PowerShell
            }
        ));

        let features = CodexCompatCli::try_parse_from(["mcg", "features", "enable", "memories"])
            .expect("Codex-compatible feature command");
        assert!(matches!(
            features.command,
            CodexCompatCommand::Features {
                command: FeaturesCommand::Enable { feature }
            } if feature == "memories"
        ));

        let sandbox = CodexCompatCli::try_parse_from([
            "mcg",
            "--project",
            ".",
            "sandbox",
            "--sandbox-state-json",
            r#"{"type":"windowsRestrictedToken"}"#,
            "--sandbox-state-readable-root",
            "docs",
            "--sandbox-state-disable-network",
            "--permission-profile",
            "build",
            "--profile",
            "ci",
            "--include-managed-config",
            "powershell.exe",
            "-NoProfile",
            "-Command",
            "Get-Location",
        ])
        .expect("Codex-compatible sandbox command");
        assert!(matches!(
            sandbox.command,
            CodexCompatCommand::Sandbox(args)
                if args.permission_profile.as_deref() == Some("build")
                    && args.profile.as_deref() == Some("ci")
                    && args.sandbox_state_disable_network
                    && args.command.first() == Some(&OsString::from("powershell.exe"))
        ));
    }

    #[test]
    fn resume_cwd_override_uses_the_last_exact_codex_value() {
        let overrides = vec![
            CodexConfigOverride {
                key: "tui.resume_cwd".into(),
                value: serde_json::json!("session"),
            },
            CodexConfigOverride {
                key: "model_verbosity".into(),
                value: serde_json::json!("low"),
            },
            CodexConfigOverride {
                key: "tui.resume_cwd".into(),
                value: serde_json::json!("current"),
            },
        ];
        assert_eq!(
            resume_cwd_config_override(&overrides).expect("valid override"),
            Some(CodexResumeCwd::Current)
        );
        let invalid = [CodexConfigOverride {
            key: "tui.resume_cwd".into(),
            value: serde_json::json!("somewhere"),
        }];
        assert!(resume_cwd_config_override(&invalid).is_err());
    }

    #[test]
    fn resume_cwd_decision_preserves_authority_and_honors_precedence() {
        let current = std::path::Path::new("current-project");
        let session = std::path::Path::new("saved-project");
        assert_eq!(
            resume_cwd_decision(current, Some(current), false, None, true).expect("matching roots"),
            ResumeCwdDecision::Session
        );
        assert_eq!(
            resume_cwd_decision(
                current,
                Some(session),
                true,
                Some(CodexResumeCwd::Session),
                true,
            )
            .expect("explicit -C wins"),
            ResumeCwdDecision::Current
        );
        assert_eq!(
            resume_cwd_decision(
                current,
                Some(session),
                false,
                Some(CodexResumeCwd::Current),
                true,
            )
            .expect("configured current"),
            ResumeCwdDecision::Current
        );
        assert_eq!(
            resume_cwd_decision(current, Some(session), false, None, true)
                .expect("interactive prompt"),
            ResumeCwdDecision::Prompt
        );
        assert!(resume_cwd_decision(current, Some(session), false, None, false).is_err());
        assert_eq!(
            resume_cwd_decision(current, None, false, Some(CodexResumeCwd::Current), true,)
                .expect("project-free chats remain unbound"),
            ResumeCwdDecision::Session
        );
        assert_eq!(
            resume_cwd_decision(current, None, true, None, true)
                .expect("explicit project binds project-free chat"),
            ResumeCwdDecision::Current
        );
    }

    #[test]
    fn combined_help_and_completion_registry_retain_metacog_commands() {
        let command = combined_cli_command();
        for name in [
            "app",
            "exec",
            "resume",
            "fork",
            "archive",
            "delete",
            "review",
            "completion",
            "features",
            "sandbox",
            "login",
            "project",
            "organization",
            "governor",
            "plugin",
        ] {
            assert!(
                command
                    .get_subcommands()
                    .any(|subcommand| subcommand.get_name() == name),
                "missing {name} from combined CLI registry"
            );
        }
    }

    #[test]
    fn sandbox_forwarding_preserves_every_codex_option_and_literal_command() {
        let (cleaned, overrides) = extract_native_overrides(
            [
                "mcg",
                "-c",
                "model_verbosity=\"low\"",
                "--enable",
                "memories",
                "--disable",
                "sqlite",
                "sandbox",
                "cmd.exe",
                "/c",
                "echo ok",
            ]
            .into_iter()
            .map(OsString::from),
        )
        .expect("extract sandbox-native overrides");
        let parsed = CodexCompatCli::try_parse_from(cleaned).expect("parse sandbox command");
        let CodexCompatCommand::Sandbox(args) = parsed.command else {
            panic!("expected sandbox command");
        };
        let arguments =
            codex_sandbox_arguments(&args, Some(std::path::Path::new("C:/work")), &overrides);
        assert_eq!(
            arguments,
            [
                "sandbox",
                "--config",
                "model_verbosity=\"low\"",
                "--enable",
                "memories",
                "--disable",
                "sqlite",
                "--cd",
                "C:/work",
                "--",
                "cmd.exe",
                "/c",
                "echo ok",
            ]
            .map(OsString::from)
        );
    }

    #[test]
    fn output_schema_files_are_bounded_json_objects() {
        let directory = tempfile::tempdir().expect("temporary output schema directory");
        let valid = directory.path().join("valid.json");
        std::fs::write(
            &valid,
            r#"{"type":"object","properties":{"answer":{"type":"string"}}}"#,
        )
        .expect("write valid schema");
        assert_eq!(
            read_output_schema(&valid).expect("valid schema")["type"],
            "object"
        );

        let array = directory.path().join("array.json");
        std::fs::write(&array, "[]").expect("write array schema");
        assert!(read_output_schema(&array).is_err());

        let malformed = directory.path().join("malformed.json");
        std::fs::write(&malformed, "{").expect("write malformed schema");
        assert!(read_output_schema(&malformed).is_err());

        let oversized = directory.path().join("oversized.json");
        std::fs::write(
            &oversized,
            format!("{{\"description\":\"{}\"}}", "x".repeat(65 * 1024)),
        )
        .expect("write oversized schema");
        assert!(read_output_schema(&oversized).is_err());
    }
}
