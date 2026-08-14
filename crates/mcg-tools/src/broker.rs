use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{Duration, Utc};
use mcg_config::AutonomyPreset;
use mcg_domain::{ApprovalId, ToolExecutionId, ToolRequestId};
use mcg_policy::{
    ActionClass, CapabilityEffect, CapabilityPolicy, CapabilityRequest, OwnerConstitution,
    PolicyDecision, ResourceKind, evaluate_capability,
};
use sha2::{Digest, Sha256};
use url::Url;

use crate::{
    ApprovalRequest, ApprovalState, LeaseRegistry, LeaseState, OutputRedactor, TOOL_SCHEMA_VERSION,
    ToolAuditPhase, ToolAuditRecord, ToolError, ToolExecutor, ToolOutcome, ToolOutcomeStatus,
    ToolRequest, ToolRequestKind,
};

#[async_trait]
pub trait ApprovalRepository: Send + Sync {
    async fn find_for(
        &self,
        request_id: ToolRequestId,
    ) -> Result<Option<ApprovalRequest>, ToolError>;
    async fn create(&self, request: ApprovalRequest) -> Result<(), ToolError>;
}

#[async_trait]
pub trait ToolAuditSink: Send + Sync {
    async fn record(&self, record: ToolAuditRecord) -> Result<(), ToolError>;
}

pub struct ToolBroker<E, A, L> {
    executor: E,
    approvals: A,
    audit: L,
    preset: AutonomyPreset,
    constitution: OwnerConstitution,
    capabilities: CapabilityPolicy,
    leases: LeaseRegistry,
    redactor: OutputRedactor,
}

pub struct BrokerControls {
    pub preset: AutonomyPreset,
    pub constitution: OwnerConstitution,
    pub capabilities: CapabilityPolicy,
    pub leases: LeaseRegistry,
    pub redactor: OutputRedactor,
}

impl<E, A, L> ToolBroker<E, A, L>
where
    E: ToolExecutor,
    A: ApprovalRepository,
    L: ToolAuditSink,
{
    #[must_use]
    pub fn new(executor: E, approvals: A, audit: L, controls: BrokerControls) -> Self {
        Self {
            executor,
            approvals,
            audit,
            preset: controls.preset,
            constitution: controls.constitution,
            capabilities: controls.capabilities,
            leases: controls.leases,
            redactor: controls.redactor,
        }
    }

    /// Evaluates, audits, and executes a tool request.
    ///
    /// # Errors
    ///
    /// Returns a validation, policy, approval, audit, or executor error. The
    /// executor is never invoked unless policy allows the complete request and
    /// the pre-execution audit record is durable.
    #[allow(clippy::too_many_lines)] // The ordering is the broker's fail-closed transaction.
    pub async fn execute(&self, request: &ToolRequest) -> Result<ToolOutcome, ToolError> {
        validate_request(request)?;
        let execution_id = ToolExecutionId::new();
        let fingerprint = request_fingerprint(request)?;
        let resource = primary_resource(request);
        let decisions = capability_requests(request)?
            .iter()
            .map(|policy_request| {
                evaluate_capability(
                    self.preset,
                    &self.constitution,
                    &self.capabilities,
                    policy_request,
                )
            })
            .collect::<Vec<_>>();

        if let Some((reason, rules)) = decisions.iter().find_map(|decision| match decision {
            PolicyDecision::Deny {
                reason,
                matched_rules,
            } => Some((reason, matched_rules)),
            _ => None,
        }) {
            let decision = format_decision("deny", reason, rules);
            self.record_decision(
                request,
                execution_id,
                &fingerprint,
                &resource,
                decision,
                Some(ToolOutcomeStatus::Denied),
            )
            .await?;
            return Err(ToolError::Denied(reason.clone()));
        }

        if let Some((reason, rules)) = decisions.iter().find_map(|decision| match decision {
            PolicyDecision::RequireApproval {
                reason,
                matched_rules,
            } => Some((reason, matched_rules)),
            _ => None,
        }) {
            match self.approvals.find_for(request.id).await? {
                Some(ApprovalRequest {
                    state: ApprovalState::Approved,
                    ..
                }) => {}
                Some(ApprovalRequest {
                    state: ApprovalState::Denied | ApprovalState::Expired | ApprovalState::Cancelled,
                    ..
                }) => {
                    self.record_decision(
                        request,
                        execution_id,
                        &fingerprint,
                        &resource,
                        "deny: approval was not granted".into(),
                        Some(ToolOutcomeStatus::Denied),
                    )
                    .await?;
                    return Err(ToolError::Denied("approval was not granted".into()));
                }
                pending @ (Some(ApprovalRequest {
                    state: ApprovalState::Pending,
                    ..
                })
                | None) => {
                    let approval_id = pending
                        .as_ref()
                        .map_or_else(ApprovalId::new, |approval| approval.id);
                    if pending.is_none() {
                        self.approvals
                            .create(ApprovalRequest {
                                schema_version: TOOL_SCHEMA_VERSION,
                                id: approval_id,
                                tool_request_id: request.id,
                                task_id: request.task_id,
                                action: request.kind.stable_name().into(),
                                resource: resource.clone(),
                                reason: reason.clone(),
                                state: ApprovalState::Pending,
                                requested_at: Utc::now(),
                                expires_at: Utc::now() + Duration::hours(24),
                            })
                            .await?;
                    }
                    self.record_decision(
                        request,
                        execution_id,
                        &fingerprint,
                        &resource,
                        format_decision("require_approval", reason, rules),
                        Some(ToolOutcomeStatus::ApprovalRequired),
                    )
                    .await?;
                    return Err(ToolError::ApprovalRequired {
                        approval_id,
                        reason: reason.clone(),
                    });
                }
            }
        }

        self.record_decision(
            request,
            execution_id,
            &fingerprint,
            &resource,
            "allow: all capability and constitutional rules passed".into(),
            None,
        )
        .await?;
        let lease = self.leases.acquire(
            request.id,
            execution_id,
            Duration::seconds(
                i64::try_from(request.timeout_seconds)
                    .map_err(|_| ToolError::Invalid("timeout is too large".into()))?,
            ),
        )?;
        let execution = self.executor.execute(execution_id, request).await;
        let mut outcome = match execution {
            Ok(outcome) => outcome,
            Err(error) => {
                self.leases.finish(lease.id, LeaseState::Failed)?;
                self.audit
                    .record(ToolAuditRecord {
                        schema_version: TOOL_SCHEMA_VERSION,
                        execution_id,
                        request_id: request.id,
                        task_id: request.task_id,
                        tool: request.kind.stable_name().into(),
                        resource,
                        request_fingerprint: fingerprint,
                        phase: ToolAuditPhase::Completion,
                        decision: "execution failed".into(),
                        result: Some(ToolOutcomeStatus::Failed),
                        recorded_at: Utc::now(),
                    })
                    .await?;
                return Err(error);
            }
        };
        if outcome.execution_id != execution_id || outcome.request_id != request.id {
            self.leases.finish(lease.id, LeaseState::Failed)?;
            return Err(ToolError::Execution(
                "executor returned mismatched request or execution identity".into(),
            ));
        }
        self.redactor.redact_outcome(&mut outcome);
        self.leases.finish(
            lease.id,
            if outcome.status == ToolOutcomeStatus::Succeeded {
                LeaseState::Completed
            } else {
                LeaseState::Failed
            },
        )?;
        self.audit
            .record(ToolAuditRecord {
                schema_version: TOOL_SCHEMA_VERSION,
                execution_id: outcome.execution_id,
                request_id: request.id,
                task_id: request.task_id,
                tool: request.kind.stable_name().into(),
                resource,
                request_fingerprint: fingerprint,
                phase: ToolAuditPhase::Completion,
                decision: "execution completed".into(),
                result: Some(outcome.status),
                recorded_at: Utc::now(),
            })
            .await?;
        Ok(outcome)
    }

    async fn record_decision(
        &self,
        request: &ToolRequest,
        execution_id: ToolExecutionId,
        fingerprint: &str,
        resource: &str,
        decision: String,
        result: Option<ToolOutcomeStatus>,
    ) -> Result<(), ToolError> {
        self.audit
            .record(ToolAuditRecord {
                schema_version: TOOL_SCHEMA_VERSION,
                execution_id,
                request_id: request.id,
                task_id: request.task_id,
                tool: request.kind.stable_name().into(),
                resource: resource.into(),
                request_fingerprint: fingerprint.into(),
                phase: ToolAuditPhase::Decision,
                decision,
                result,
                recorded_at: Utc::now(),
            })
            .await
    }
}

fn validate_request(request: &ToolRequest) -> Result<(), ToolError> {
    if request.schema_version != TOOL_SCHEMA_VERSION {
        return Err(ToolError::Invalid("unsupported tool schema version".into()));
    }
    if request.summary.trim().is_empty() {
        return Err(ToolError::Invalid("summary is required".into()));
    }
    if request.timeout_seconds == 0 || request.timeout_seconds > 86_400 {
        return Err(ToolError::Invalid(
            "timeout must be between 1 and 86400 seconds".into(),
        ));
    }
    Ok(())
}

fn request_fingerprint(request: &ToolRequest) -> Result<String, ToolError> {
    let serialized = serde_json::to_vec(request)
        .map_err(|error| ToolError::Invalid(format!("cannot fingerprint request: {error}")))?;
    Ok(format!("{:x}", Sha256::digest(serialized)))
}

fn format_decision(kind: &str, reason: &str, rules: &[String]) -> String {
    format!("{kind}: {reason}; rules={}", rules.join(","))
}

#[allow(clippy::too_many_lines)] // Exhaustive typed-tool-to-capability mapping stays centralized.
fn capability_requests(request: &ToolRequest) -> Result<Vec<CapabilityRequest>, ToolError> {
    let make = |action, effect, kind, resource: String| CapabilityRequest {
        task_id: request.task_id,
        action,
        effect,
        kind,
        resource,
        summary: request.summary.clone(),
    };
    let read_path = |path: &str| {
        make(
            ActionClass::ProjectRead,
            CapabilityEffect::Read,
            ResourceKind::File,
            path.into(),
        )
    };
    let write_path = |path: &str| {
        make(
            ActionClass::ReversibleProjectWrite,
            CapabilityEffect::Write,
            ResourceKind::File,
            path.into(),
        )
    };
    let requests = match &request.kind {
        ToolRequestKind::FsRead { path, .. } | ToolRequestKind::FsList { path, .. } => {
            vec![read_path(path)]
        }
        ToolRequestKind::FsWrite { path, .. }
        | ToolRequestKind::FsPatch { path, .. }
        | ToolRequestKind::FsMkdir { path, .. } => vec![write_path(path)],
        ToolRequestKind::FsMove { from, to } | ToolRequestKind::FsCopy { from, to } => {
            vec![read_path(from), write_path(to)]
        }
        ToolRequestKind::FsDelete { path, .. } => vec![make(
            ActionClass::DestructiveDelete,
            CapabilityEffect::Delete,
            ResourceKind::File,
            path.into(),
        )],
        ToolRequestKind::ProcessExec { options } | ToolRequestKind::ProcessSpawn { options } => {
            let mut values = vec![make(
                ActionClass::BuildOrTest,
                CapabilityEffect::Execute,
                ResourceKind::Process,
                executable_name(&options.executable),
            )];
            values.push(read_path(&options.cwd));
            values.extend(options.env.iter().map(|variable| {
                make(
                    ActionClass::BuildOrTest,
                    CapabilityEffect::Read,
                    ResourceKind::EnvironmentVariable,
                    variable.name.clone(),
                )
            }));
            values
        }
        ToolRequestKind::ProcessStatus { process_id }
        | ToolRequestKind::ProcessStdin { process_id, .. }
        | ToolRequestKind::ProcessKill { process_id } => vec![make(
            ActionClass::BuildOrTest,
            CapabilityEffect::Execute,
            ResourceKind::Process,
            process_id.clone(),
        )],
        ToolRequestKind::ShellExec { cwd, .. } => vec![
            make(
                ActionClass::BuildOrTest,
                CapabilityEffect::Execute,
                ResourceKind::Shell,
                "shell".into(),
            ),
            read_path(cwd),
        ],
        ToolRequestKind::HttpRequest { method, url, .. } => vec![make(
            if method.eq_ignore_ascii_case("GET") || method.eq_ignore_ascii_case("HEAD") {
                ActionClass::ProjectRead
            } else {
                ActionClass::ExternalPublication
            },
            if method.eq_ignore_ascii_case("GET") || method.eq_ignore_ascii_case("HEAD") {
                CapabilityEffect::Read
            } else {
                CapabilityEffect::Publish
            },
            ResourceKind::NetworkOrigin,
            network_origin(url)?,
        )],
        ToolRequestKind::HttpDownload {
            url, destination, ..
        } => vec![
            make(
                ActionClass::ProjectRead,
                CapabilityEffect::Read,
                ResourceKind::NetworkOrigin,
                network_origin(url)?,
            ),
            write_path(destination),
        ],
        ToolRequestKind::SystemInfo | ToolRequestKind::SystemProcesses => vec![make(
            ActionClass::ProjectRead,
            CapabilityEffect::Read,
            ResourceKind::Process,
            "system".into(),
        )],
        ToolRequestKind::SystemEnv { names } => names
            .iter()
            .map(|name| {
                make(
                    ActionClass::ProjectRead,
                    CapabilityEffect::Read,
                    ResourceKind::EnvironmentVariable,
                    name.clone(),
                )
            })
            .collect(),
        ToolRequestKind::GitStatus { repository } | ToolRequestKind::GitDiff { repository, .. } => {
            vec![make(
                ActionClass::ProjectRead,
                CapabilityEffect::Read,
                ResourceKind::GitRepository,
                repository.clone(),
            )]
        }
        ToolRequestKind::GitCommit { repository, .. }
        | ToolRequestKind::GitCheckout { repository, .. } => vec![make(
            ActionClass::ReversibleProjectWrite,
            CapabilityEffect::Write,
            ResourceKind::GitRepository,
            repository.clone(),
        )],
        ToolRequestKind::MemorySearch { query, .. } => vec![make(
            ActionClass::ProjectRead,
            CapabilityEffect::Read,
            ResourceKind::Memory,
            query.clone(),
        )],
        ToolRequestKind::MemoryRead { key } => vec![make(
            ActionClass::ProjectRead,
            CapabilityEffect::Read,
            ResourceKind::Memory,
            key.clone(),
        )],
        ToolRequestKind::MemoryWrite { key, .. } => vec![make(
            ActionClass::ReversibleProjectWrite,
            CapabilityEffect::Write,
            ResourceKind::Memory,
            key.clone(),
        )],
    };
    Ok(requests)
}

fn network_origin(value: &str) -> Result<String, ToolError> {
    let url =
        Url::parse(value).map_err(|error| ToolError::Invalid(format!("invalid URL: {error}")))?;
    if !matches!(url.scheme(), "http" | "https") || url.username() != "" || url.password().is_some()
    {
        return Err(ToolError::Invalid(
            "only credential-free HTTP(S) URLs are supported".into(),
        ));
    }
    Ok(url.origin().ascii_serialization())
}

fn executable_name(value: &str) -> String {
    Path::new(value)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or(value)
        .to_ascii_lowercase()
}

fn primary_resource(request: &ToolRequest) -> String {
    match &request.kind {
        ToolRequestKind::FsRead { path, .. }
        | ToolRequestKind::FsWrite { path, .. }
        | ToolRequestKind::FsPatch { path, .. }
        | ToolRequestKind::FsList { path, .. }
        | ToolRequestKind::FsMkdir { path, .. }
        | ToolRequestKind::FsDelete { path, .. } => path.clone(),
        ToolRequestKind::FsMove { from, to } | ToolRequestKind::FsCopy { from, to } => {
            format!("{from} -> {to}")
        }
        ToolRequestKind::HttpRequest { url, .. } | ToolRequestKind::HttpDownload { url, .. } => {
            url.clone()
        }
        ToolRequestKind::GitStatus { repository }
        | ToolRequestKind::GitDiff { repository, .. }
        | ToolRequestKind::GitCommit { repository, .. }
        | ToolRequestKind::GitCheckout { repository, .. } => repository.clone(),
        _ => request.kind.stable_name().into(),
    }
}

#[derive(Default, Clone)]
pub struct InMemoryApprovalRepository {
    entries: Arc<Mutex<Vec<ApprovalRequest>>>,
}

impl InMemoryApprovalRepository {
    pub fn set_state(&self, request_id: ToolRequestId, state: ApprovalState) {
        if let Some(entry) = self
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter_mut()
            .find(|entry| entry.tool_request_id == request_id)
        {
            entry.state = state;
        }
    }
}

#[async_trait]
impl ApprovalRepository for InMemoryApprovalRepository {
    async fn find_for(
        &self,
        request_id: ToolRequestId,
    ) -> Result<Option<ApprovalRequest>, ToolError> {
        Ok(self
            .entries
            .lock()
            .map_err(|_| ToolError::Execution("approval lock poisoned".into()))?
            .iter()
            .find(|entry| entry.tool_request_id == request_id)
            .cloned())
    }

    async fn create(&self, request: ApprovalRequest) -> Result<(), ToolError> {
        self.entries
            .lock()
            .map_err(|_| ToolError::Execution("approval lock poisoned".into()))?
            .push(request);
        Ok(())
    }
}

#[derive(Default, Clone)]
pub struct InMemoryAuditSink {
    records: Arc<Mutex<Vec<ToolAuditRecord>>>,
}

impl InMemoryAuditSink {
    #[must_use]
    pub fn records(&self) -> Vec<ToolAuditRecord> {
        self.records
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

#[async_trait]
impl ToolAuditSink for InMemoryAuditSink {
    async fn record(&self, record: ToolAuditRecord) -> Result<(), ToolError> {
        self.records
            .lock()
            .map_err(|_| ToolError::Execution("audit lock poisoned".into()))?
            .push(record);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use chrono::Utc;
    use mcg_domain::TaskId;
    use tempfile::tempdir;

    use super::*;
    use crate::{ToolNamespace, VerificationEvidence};

    struct CountingExecutor(Arc<AtomicUsize>);

    #[async_trait]
    impl ToolExecutor for CountingExecutor {
        async fn execute(
            &self,
            execution_id: ToolExecutionId,
            request: &ToolRequest,
        ) -> Result<ToolOutcome, ToolError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(ToolOutcome {
                schema_version: TOOL_SCHEMA_VERSION,
                execution_id,
                request_id: request.id,
                status: ToolOutcomeStatus::Succeeded,
                summary: "done".into(),
                exit_code: Some(0),
                stdout: None,
                stderr: None,
                artifacts: Vec::new(),
                verification: vec![VerificationEvidence {
                    method: "fixture".into(),
                    passed: true,
                    detail: "observed".into(),
                }],
                started_at: Utc::now(),
                finished_at: Utc::now(),
            })
        }
    }

    fn request(task_id: TaskId, kind: ToolRequestKind) -> ToolRequest {
        let mut request = ToolRequest::new(task_id, "fixture request", kind);
        request.namespace = ToolNamespace::Host;
        request
    }

    #[tokio::test]
    async fn denial_is_audited_and_has_zero_executor_side_effects() {
        let root = tempdir().expect("tempdir");
        let count = Arc::new(AtomicUsize::new(0));
        let audit = InMemoryAuditSink::default();
        let broker = ToolBroker::new(
            CountingExecutor(count.clone()),
            InMemoryApprovalRepository::default(),
            audit.clone(),
            BrokerControls {
                preset: AutonomyPreset::Autonomous,
                constitution: OwnerConstitution::secure_defaults(),
                capabilities: CapabilityPolicy::project(root.path()),
                leases: LeaseRegistry::default(),
                redactor: OutputRedactor::default(),
            },
        );
        let request = request(
            TaskId::new(),
            ToolRequestKind::FsRead {
                path: root
                    .path()
                    .join("..")
                    .join("outside.txt")
                    .to_string_lossy()
                    .into(),
                max_bytes: 100,
            },
        );
        assert!(matches!(
            broker.execute(&request).await,
            Err(ToolError::Denied(_))
        ));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        assert_eq!(audit.records().len(), 1);
        assert_eq!(audit.records()[0].result, Some(ToolOutcomeStatus::Denied));
    }

    #[tokio::test]
    async fn destructive_request_waits_for_approval_before_execution() {
        let root = tempdir().expect("tempdir");
        let count = Arc::new(AtomicUsize::new(0));
        let approvals = InMemoryApprovalRepository::default();
        let broker = ToolBroker::new(
            CountingExecutor(count.clone()),
            approvals.clone(),
            InMemoryAuditSink::default(),
            BrokerControls {
                preset: AutonomyPreset::Autonomous,
                constitution: OwnerConstitution::secure_defaults(),
                capabilities: CapabilityPolicy::project(root.path()),
                leases: LeaseRegistry::default(),
                redactor: OutputRedactor::default(),
            },
        );
        let request = request(
            TaskId::new(),
            ToolRequestKind::FsDelete {
                path: root.path().join("generated.txt").to_string_lossy().into(),
                recursive: false,
            },
        );
        assert!(matches!(
            broker.execute(&request).await,
            Err(ToolError::ApprovalRequired { .. })
        ));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        approvals.set_state(request.id, ApprovalState::Approved);
        assert!(broker.execute(&request).await.is_ok());
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }
}
