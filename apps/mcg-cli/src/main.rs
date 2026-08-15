use std::{io::Read, path::PathBuf, str::FromStr};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use clap::{Parser, Subcommand, ValueEnum};
use mcg_agents::{
    AgentRunState, ExperimentBenchmark, ExperimentDecision, ExperimentDefinition, ExperimentState,
    OrganizationGenome, RoleKind, ShadowExecutionReceipt, SuccessCondition, SupervisionObservation,
    TaskComplexity,
};
use mcg_api::{
    ApplyComputerActionsRequest, ApprovalListResponse, AssembleContextRequest,
    AttachBrowserSessionRequest, CheckSiteHealthRequest, CorrectMemoryClaimRequest,
    CreateMemoryClaimRequest, CreateOrganizationRequest, CreatePlatformSessionRequest,
    CreateScheduleRequest, CreateSiteMonitorRequest, CreateSiteRequest, CreateSshHostRequest,
    CreateTaskRequest, DeploySiteRequest, DiscoverProjectRequest, EvaluationMutationResponse,
    ExecuteAgentWorkRequest, ExperimentBenchmarkListResponse, ExperimentDecisionListResponse,
    ExperimentListResponse, LaunchTaskRequest, LaunchTaskResponse, ManagedMemoryResponse,
    McpListResponse, MemoryClaimHistoryResponse, MemoryClaimListResponse,
    NavigateBrowserSessionRequest, OrganizationListResponse, OrganizationMutationResponse,
    PlanAgentRunRequest, PlanWorkRequest, PlatformSessionHistoryResponse,
    PlatformSessionListResponse, ProjectHistoryResponse, ProjectListResponse, ProviderListResponse,
    ResolveApprovalRequest, RunSshOperationRequest, ScheduleHistoryResponse, ScheduleListResponse,
    SchedulerNotificationListResponse, SetScheduleStateRequest, ShadowExecutionListResponse,
    SiteDeploymentListResponse, SiteHealthListResponse, SiteListResponse, SiteRevisionListResponse,
    SshHostListResponse, SshHostRevisionListResponse, SshOperationListResponse,
    StartOpenAiOAuthRequest, SupervisionObservationListResponse, TaskEventsResponse,
    TaskListResponse, TaskMutationResponse, ToolAuditResponse, TransitionAgentRunRequest,
    TransitionExperimentRequest, TransitionMemoryClaimRequest, TransitionTaskRequest,
    UpdateSiteRequest, UpdateSshHostRequest, UpdateUserSettingsRequest,
    UserSettingsHistoryResponse, WakeScheduleRequest,
};
use mcg_config::{AppConfig, AutonomyPreset, UserSettings};
use mcg_domain::{
    ComputerUseCapability, ComputerUseRegion, ComputerUseTarget, MemoryClaimId,
    NotificationSeverity, OrganizationRevisionId, PlatformCapabilitySnapshot, PlatformSession,
    PlatformSessionId, PlatformSessionKind, ProjectId, RunId, ScheduleId, ScheduleState,
    ScheduleTrigger, SchedulerNotification, SchedulerNotificationId, Site, SiteDeploymentCommand,
    SiteDeploymentId, SiteEnvironment, SiteHealthCheckId, SiteId, SiteMonitor, SiteState, SshHost,
    SshHostId, TaskEnvelope, TaskId, TaskState, WorkItemId,
};
use mcg_mcp::{
    MCP_CONNECTION_CONTRACT_VERSION, McpAuthMode, McpConnection, McpProtocolMode, McpToolCatalog,
    McpTransport,
};
use mcg_projects::{ClaimProvenance, MemoryLifecycleAction, ProvenanceKind};
use mcg_providers::{
    CONTRACT_VERSION, ModelProbeReport, ProviderAuthMode, ProviderConnection, ProviderKind,
    ProviderModelCatalog, ProxyConversation, ProxyUsageSummary,
};
use mcg_runtime::{
    AgentRunView, AgentTurnCommit, CandidateRevisionMutation, EvidenceMutation, ExperimentMutation,
    McpMutation, MemoryClaimMutation, OAuthLoginStart, OpenAiOAuthMethod, PlatformBrowserMutation,
    PlatformComputerActionMutation, PlatformComputerMutation, ProjectMutation, ProviderMutation,
    RuntimeStatus, SiteDeploymentMutation, SiteHealthMutation, SiteMonitorMutation, SiteMutation,
    SshHostMutation, SshOperationMutation, SupervisionMutation,
};
use mcg_tools::{
    ApprovalRequest, ApprovalState, ComputerUseFrame, PersistentBrowserMode,
    PersistentBrowserObservation,
};
use serde::Serialize;
use zeroize::Zeroize;

#[derive(Serialize)]
struct ProviderConfigurePayload {
    connection: ProviderConnection,
    credential: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct ProxyConversationListResponse {
    conversations: Vec<ProxyConversation>,
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
#[command(name = "mcg", version, about = "MetaCognitive Governor CLI")]
struct Cli {
    #[arg(long, default_value = "http://127.0.0.1:7777")]
    host: String,
    /// Override the local daemon bearer-token file.
    #[arg(long)]
    token_file: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Show daemon status.
    Status {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Inspect or change restart-safe owner settings.
    Settings {
        #[command(subcommand)]
        command: SettingsCommand,
    },
    /// Create and inspect durable tasks.
    Task {
        #[command(subcommand)]
        command: TaskCommand,
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
    /// Plan, inspect, start, and cancel durable agent runs.
    Run {
        #[command(subcommand)]
        command: RunCommand,
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

#[derive(Debug, Clone, Copy, ValueEnum)]
enum AutonomyArg {
    Guided,
    Balanced,
    Autonomous,
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
        #[arg(long, default_value_t = 4_096)]
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
    /// Re-scan a registered project and report invalidated model areas.
    Refresh {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Read immutable project discovery history.
    History { id: String },
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let config = AppConfig::default();
    let token = if let Ok(token) = std::env::var("MCG_AUTH_TOKEN") {
        token
    } else {
        let path = cli.token_file.unwrap_or_else(|| config.auth_token_path());
        std::fs::read_to_string(&path).map_err(|error| {
            format!(
                "cannot read local daemon token at {}: {error}; start mcg-daemon first",
                path.display()
            )
        })?
    };
    let host = cli.host.trim_end_matches('/');
    let client = reqwest::Client::new();

    match cli.command {
        Command::Status { json } => run_status(&client, host, token.trim(), json).await?,
        Command::Settings { command } => {
            run_settings(&client, host, token.trim(), command).await?;
        }
        Command::Task { command } => run_task(&client, host, token.trim(), command).await?,
        Command::Approval { command } => run_approval(&client, host, token.trim(), command).await?,
        Command::Project { command } => run_project(&client, host, token.trim(), command).await?,
        Command::Organization { command } => {
            run_organization(&client, host, token.trim(), command).await?;
        }
        Command::Run { command } => run_agent(&client, host, token.trim(), command).await?,
        Command::Provider { command } => {
            run_provider(&client, host, token.trim(), command).await?;
        }
        Command::Mcp { command } => {
            run_mcp(&client, host, token.trim(), command).await?;
        }
        Command::Schedule { command } => {
            run_schedule(&client, host, token.trim(), command).await?;
        }
        Command::Platform { command } => {
            run_platform(&client, host, token.trim(), command).await?;
        }
        Command::Site { command } => {
            run_site(&client, host, token.trim(), command).await?;
        }
        Command::Ssh { command } => {
            run_ssh(&client, host, token.trim(), command).await?;
        }
        Command::Experiment { command } => {
            run_experiment(&client, host, token.trim(), command).await?;
        }
    }

    Ok(())
}

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
        ScheduleCommand::Notifications { all } => {
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
            for notification in response.notifications {
                println!(
                    "{}: {:?} — {}",
                    notification.id, notification.severity, notification.title
                );
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
            estimated_output_tokens: 4_096,
            max_model_calls: 8,
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
            advanced_mode,
            autonomy,
            shadow_experiments,
            reason,
        } => {
            if advanced_mode.is_none() && autonomy.is_none() && shadow_experiments.is_none() {
                return Err("settings set requires at least one changed field".into());
            }
            let current = load_user_settings(client, host, token).await?;
            let updated: UserSettings = client
                .put(format!("{host}/api/v1/settings"))
                .bearer_auth(token)
                .json(&UpdateUserSettingsRequest {
                    expected_revision: current.revision,
                    advanced_mode: advanced_mode.unwrap_or(current.advanced_mode),
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
                        if settings.advanced_mode {
                            "advanced"
                        } else {
                            "simple"
                        },
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
            if settings.advanced_mode {
                "advanced"
            } else {
                "simple"
            },
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
        .json(&LaunchTaskRequest {
            target_path: None,
            context_token_budget: 8_192,
            requested_tools: Vec::new(),
            estimated_output_tokens: 4_096,
            max_model_calls: 8,
        })
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

#[allow(clippy::too_many_lines)] // CLI dispatch keeps related project subcommands discoverable.
async fn run_project(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    command: ProjectCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        ProjectCommand::Add { root, id, json } => {
            let project_id = id.map(|value| ProjectId::from_str(&value)).transpose()?;
            let mutation: ProjectMutation = client
                .post(format!("{host}/api/v1/projects"))
                .bearer_auth(token)
                .json(&DiscoverProjectRequest {
                    root: root.to_string_lossy().into_owned(),
                    project_id,
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
                    fresh_until: None,
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
        } => {
            let claim_id = MemoryClaimId::from_str(&claim_id)?;
            let mutation: MemoryClaimMutation = client
                .post(format!("{base}/{claim_id}/correct"))
                .bearer_auth(token)
                .json(&CorrectMemoryClaimRequest {
                    value: parse_cli_value(&value),
                    confidence,
                    provenance: owner_provenance(source),
                    fresh_until: None,
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
            cli.command,
            Command::Run {
                command: RunCommand::Evaluate { id, file }
            } if id == "00000000-0000-0000-0000-000000000001"
                && file == std::path::Path::new("evaluation.json")
        ));
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
            interval.command,
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
            once.command,
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
            cron.command,
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
            watch.command,
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
            event.command,
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
            heartbeat.command,
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
            wake.command,
            Command::Schedule {
                command: ScheduleCommand::Wake {
                    severity: NotificationSeverityArg::High,
                    ..
                }
            }
        ));
        let notifications = Cli::try_parse_from(["mcg", "schedule", "notifications", "--all"])
            .expect("notifications command");
        assert!(matches!(
            notifications.command,
            Command::Schedule {
                command: ScheduleCommand::Notifications { all: true }
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
            request.command,
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
            capabilities.command,
            Command::Platform {
                command: PlatformCommand::Capabilities { json: true }
            }
        ));
        let attach = Cli::try_parse_from(["mcg", "platform", "attach", session_id, "--visible"])
            .expect("attach");
        assert!(matches!(
            attach.command,
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
            navigate.command,
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
            register.command,
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
            deploy.command,
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
            monitor.command,
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
    fn experiment_commands_keep_versioned_json_and_compare_and_set_state_explicit() {
        let candidate = Cli::try_parse_from(["mcg", "experiment", "candidate", "candidate.json"])
            .expect("candidate command");
        assert!(matches!(
            candidate.command,
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
            transition.command,
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
            set.command,
            Command::Settings {
                command: SettingsCommand::Set {
                    advanced_mode: Some(true),
                    autonomy: Some(AutonomyArg::Guided),
                    shadow_experiments: Some(false),
                    ..
                }
            }
        ));
        let history = Cli::try_parse_from(["mcg", "settings", "history", "--json"])
            .expect("settings history command");
        assert!(matches!(
            history.command,
            Command::Settings {
                command: SettingsCommand::History { json: true }
            }
        ));
    }
}
