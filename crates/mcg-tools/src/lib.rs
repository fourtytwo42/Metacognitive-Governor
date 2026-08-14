//! Typed tool requests are the only route from agents to real-world effects.

use async_trait::async_trait;
use mcg_domain::TaskId;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tool", rename_all = "snake_case")]
pub enum ToolRequestKind {
    FileRead {
        path: String,
    },
    FileWrite {
        path: String,
        content: String,
    },
    ProcessExec {
        executable: String,
        args: Vec<String>,
        cwd: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolRequest {
    pub task_id: TaskId,
    pub summary: String,
    pub timeout_seconds: u64,
    pub kind: ToolRequestKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolOutcome {
    pub success: bool,
    pub summary: String,
    pub artifact_hashes: Vec<String>,
}

#[derive(Debug, Error)]
pub enum ToolError {
    #[error("tool request was denied: {0}")]
    Denied(String),
    #[error("tool execution failed: {0}")]
    Execution(String),
}

#[async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute(&self, request: &ToolRequest) -> Result<ToolOutcome, ToolError>;
}
