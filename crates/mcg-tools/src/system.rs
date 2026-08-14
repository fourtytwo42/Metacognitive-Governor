use async_trait::async_trait;
use chrono::Utc;
use mcg_domain::ToolExecutionId;
use serde::Serialize;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

use crate::{
    TOOL_SCHEMA_VERSION, ToolError, ToolExecutor, ToolOutcome, ToolOutcomeStatus, ToolRequest,
    ToolRequestKind, VerificationEvidence,
};

#[derive(Debug, Default)]
pub struct SystemExecutor;

#[derive(Serialize)]
struct SystemSnapshot {
    operating_system: Option<String>,
    operating_system_version: Option<String>,
    architecture: &'static str,
    cpu_count: usize,
    total_memory_bytes: u64,
    available_memory_bytes: u64,
    uptime_seconds: u64,
}

#[derive(Serialize)]
struct ProcessSnapshot {
    pid: u32,
    name: String,
    cpu_percent: f32,
    memory_bytes: u64,
    status: String,
}

#[async_trait]
impl ToolExecutor for SystemExecutor {
    async fn execute(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
    ) -> Result<ToolOutcome, ToolError> {
        match &request.kind {
            ToolRequestKind::SystemInfo => system_info(execution_id, request),
            ToolRequestKind::SystemEnv { names } => system_env(execution_id, request, names),
            ToolRequestKind::SystemProcesses => system_processes(execution_id, request),
            _ => Err(ToolError::Invalid(
                "system executor received an unsupported tool".into(),
            )),
        }
    }
}

fn system_info(
    execution_id: ToolExecutionId,
    request: &ToolRequest,
) -> Result<ToolOutcome, ToolError> {
    let system = System::new_all();
    let snapshot = SystemSnapshot {
        operating_system: System::name(),
        operating_system_version: System::os_version(),
        architecture: std::env::consts::ARCH,
        cpu_count: system.cpus().len(),
        total_memory_bytes: system.total_memory(),
        available_memory_bytes: system.available_memory(),
        uptime_seconds: System::uptime(),
    };
    encoded_outcome(
        execution_id,
        request,
        "captured system information",
        &snapshot,
        "system.snapshot",
    )
}

fn system_env(
    execution_id: ToolExecutionId,
    request: &ToolRequest,
    names: &[String],
) -> Result<ToolOutcome, ToolError> {
    let mut values = std::collections::BTreeMap::new();
    for name in names {
        if is_sensitive_environment_name(name) {
            return Err(ToolError::Denied(format!(
                "environment variable {name} is secret-like and cannot enter tool output"
            )));
        }
        let value = std::env::var(name).ok();
        values.insert(name.clone(), value);
    }
    encoded_outcome(
        execution_id,
        request,
        "captured permitted environment variables",
        &values,
        "system.environment_snapshot",
    )
}

fn system_processes(
    execution_id: ToolExecutionId,
    request: &ToolRequest,
) -> Result<ToolOutcome, ToolError> {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_cpu().with_memory(),
    );
    let mut snapshots = system
        .processes()
        .iter()
        .map(|(pid, process)| ProcessSnapshot {
            pid: pid.as_u32(),
            name: process.name().to_string_lossy().into_owned(),
            cpu_percent: process.cpu_usage(),
            memory_bytes: process.memory(),
            status: format!("{:?}", process.status()).to_ascii_lowercase(),
        })
        .collect::<Vec<_>>();
    snapshots.sort_by_key(|process| process.pid);
    encoded_outcome(
        execution_id,
        request,
        "captured process list without arguments or environment",
        &snapshots,
        "system.process_snapshot",
    )
}

fn encoded_outcome<T: Serialize>(
    execution_id: ToolExecutionId,
    request: &ToolRequest,
    summary: &str,
    value: &T,
    method: &str,
) -> Result<ToolOutcome, ToolError> {
    let now = Utc::now();
    let encoded = serde_json::to_string(value)
        .map_err(|error| ToolError::Execution(format!("serialize system result: {error}")))?;
    Ok(ToolOutcome {
        schema_version: TOOL_SCHEMA_VERSION,
        execution_id,
        request_id: request.id,
        status: ToolOutcomeStatus::Succeeded,
        summary: summary.into(),
        exit_code: None,
        stdout: Some(encoded),
        stderr: None,
        artifacts: Vec::new(),
        verification: vec![VerificationEvidence {
            method: method.into(),
            passed: true,
            detail: "snapshot captured".into(),
        }],
        started_at: now,
        finished_at: Utc::now(),
    })
}

fn is_sensitive_environment_name(name: &str) -> bool {
    const SENSITIVE: &[&str] = &[
        "APIKEY",
        "API_KEY",
        "AUTH",
        "CREDENTIAL",
        "PASSWORD",
        "PRIVATE",
        "SECRET",
        "TOKEN",
    ];
    let upper = name.to_ascii_uppercase();
    SENSITIVE.iter().any(|needle| {
        upper == *needle
            || upper.starts_with(&format!("{needle}_"))
            || upper.ends_with(&format!("_{needle}"))
            || upper.contains(&format!("_{needle}_"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn system_snapshot_omits_hostname_and_user_identity() {
        let request = ToolRequest::new(
            mcg_domain::TaskId::new(),
            "system info",
            ToolRequestKind::SystemInfo,
        );
        let outcome = SystemExecutor
            .execute(ToolExecutionId::new(), &request)
            .await
            .expect("system info");
        let json: serde_json::Value =
            serde_json::from_str(outcome.stdout.as_deref().expect("output")).expect("json");
        assert!(json.get("architecture").is_some());
        assert!(json.get("hostname").is_none());
        assert!(json.get("username").is_none());
    }

    #[test]
    fn secret_like_environment_names_fail_closed() {
        assert!(is_sensitive_environment_name("OPENAI_API_KEY"));
        assert!(is_sensitive_environment_name("access_token"));
        assert!(!is_sensitive_environment_name("PATH"));
        assert!(!is_sensitive_environment_name("MONKEY"));
    }
}
