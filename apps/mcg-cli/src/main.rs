use std::{io::Read, path::PathBuf, str::FromStr};

use clap::{Parser, Subcommand, ValueEnum};
use mcg_agents::{AgentRunState, RoleKind, SuccessCondition, TaskComplexity};
use mcg_api::{
    ApprovalListResponse, AssembleContextRequest, CorrectMemoryClaimRequest,
    CreateMemoryClaimRequest, CreateOrganizationRequest, CreateTaskRequest, DiscoverProjectRequest,
    EvaluationMutationResponse, ExecuteAgentWorkRequest, ManagedMemoryResponse, McpListResponse,
    MemoryClaimHistoryResponse, MemoryClaimListResponse, OrganizationListResponse,
    OrganizationMutationResponse, PlanAgentRunRequest, PlanWorkRequest, ProjectHistoryResponse,
    ProjectListResponse, ProviderListResponse, ResolveApprovalRequest, StartOpenAiOAuthRequest,
    TaskEventsResponse, TaskMutationResponse, ToolAuditResponse, TransitionAgentRunRequest,
    TransitionMemoryClaimRequest, TransitionTaskRequest,
};
use mcg_config::AppConfig;
use mcg_domain::{
    MemoryClaimId, OrganizationRevisionId, ProjectId, RunId, TaskEnvelope, TaskId, TaskState,
    WorkItemId,
};
use mcg_mcp::{
    MCP_CONNECTION_CONTRACT_VERSION, McpAuthMode, McpConnection, McpProtocolMode, McpToolCatalog,
    McpTransport,
};
use mcg_projects::{ClaimProvenance, MemoryLifecycleAction, ProvenanceKind};
use mcg_providers::{
    CONTRACT_VERSION, ModelProbeReport, ProviderAuthMode, ProviderConnection, ProviderKind,
    ProviderModelCatalog,
};
use mcg_runtime::{
    AgentRunView, AgentTurnCommit, McpMutation, MemoryClaimMutation, OAuthLoginStart,
    OpenAiOAuthMethod, ProjectMutation, ProviderMutation, RuntimeStatus,
};
use mcg_tools::{ApprovalRequest, ApprovalState};
use serde::Serialize;
use zeroize::Zeroize;

#[derive(Serialize)]
struct ProviderConfigurePayload {
    connection: ProviderConnection,
    credential: Option<String>,
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
    }

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
    }
    Ok(())
}

async fn run_task(
    client: &reqwest::Client,
    host: &str,
    token: &str,
    command: TaskCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        TaskCommand::Create { goal, json } => {
            let created: TaskMutationResponse = client
                .post(format!("{host}/api/v1/tasks"))
                .bearer_auth(token)
                .json(&CreateTaskRequest {
                    goal,
                    project_id: None,
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
            let response = client
                .post(format!("{host}/api/v1/mcp/connections"))
                .bearer_auth(token)
                .json(&payload)
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
        }
        McpCommand::List { json } => {
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
}
