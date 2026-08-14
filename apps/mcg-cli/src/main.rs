use std::{path::PathBuf, str::FromStr};

use clap::{Parser, Subcommand, ValueEnum};
use mcg_api::{
    ApprovalListResponse, CreateTaskRequest, ResolveApprovalRequest, TaskEventsResponse,
    TaskMutationResponse, ToolAuditResponse, TransitionTaskRequest,
};
use mcg_config::AppConfig;
use mcg_domain::{TaskEnvelope, TaskId, TaskState};
use mcg_runtime::RuntimeStatus;
use mcg_tools::{ApprovalRequest, ApprovalState};

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
