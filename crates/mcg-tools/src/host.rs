use std::sync::Arc;

use async_trait::async_trait;
use mcg_domain::ToolExecutionId;

use crate::{ToolError, ToolExecutor, ToolOutcome, ToolRequest, ToolRequestKind};

pub struct HostExecutor {
    pub filesystem: Arc<dyn ToolExecutor>,
    pub process: Arc<dyn ToolExecutor>,
    pub http: Arc<dyn ToolExecutor>,
    pub system: Arc<dyn ToolExecutor>,
    pub git: Arc<dyn ToolExecutor>,
    pub memory: Arc<dyn ToolExecutor>,
    pub external: Arc<dyn ToolExecutor>,
}

#[async_trait]
impl ToolExecutor for HostExecutor {
    async fn execute(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
    ) -> Result<ToolOutcome, ToolError> {
        let executor = match &request.kind {
            ToolRequestKind::FsRead { .. }
            | ToolRequestKind::FsWrite { .. }
            | ToolRequestKind::FsPatch { .. }
            | ToolRequestKind::FsList { .. }
            | ToolRequestKind::FsMkdir { .. }
            | ToolRequestKind::FsMove { .. }
            | ToolRequestKind::FsCopy { .. }
            | ToolRequestKind::FsDelete { .. } => &self.filesystem,
            ToolRequestKind::ProcessExec { .. }
            | ToolRequestKind::ProcessSpawn { .. }
            | ToolRequestKind::ProcessStatus { .. }
            | ToolRequestKind::ProcessStdin { .. }
            | ToolRequestKind::ProcessKill { .. }
            | ToolRequestKind::ShellExec { .. } => &self.process,
            ToolRequestKind::HttpRequest { .. } | ToolRequestKind::HttpDownload { .. } => {
                &self.http
            }
            ToolRequestKind::SystemInfo
            | ToolRequestKind::SystemEnv { .. }
            | ToolRequestKind::SystemProcesses => &self.system,
            ToolRequestKind::GitStatus { .. }
            | ToolRequestKind::GitDiff { .. }
            | ToolRequestKind::GitCommit { .. }
            | ToolRequestKind::GitCheckout { .. }
            | ToolRequestKind::GitWorktreeCreate { .. }
            | ToolRequestKind::GitWorktreeRemove { .. }
            | ToolRequestKind::GitApplyPatch { .. } => &self.git,
            ToolRequestKind::MemorySearch { .. }
            | ToolRequestKind::MemoryRead { .. }
            | ToolRequestKind::MemoryWrite { .. } => &self.memory,
            ToolRequestKind::McpCall { .. } => &self.external,
        };
        executor.execute(execution_id, request).await
    }
}
