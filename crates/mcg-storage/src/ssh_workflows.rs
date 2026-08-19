use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use mcg_domain::{
    SSH_WORKFLOW_SCHEMA_VERSION, SshHostState, SshOperationState, SshWorkflow, SshWorkflowId,
    SshWorkflowState, SshWorkflowStepState,
};
use mcg_events::{EventEnvelope, EventKind};
use rusqlite::params;

use crate::{SqliteEventStore, StorageError};

impl SqliteEventStore {
    /// Atomically records one immutable workflow definition and its initial event.
    /// Exact caller identity replay returns the retained workflow without another event.
    ///
    /// # Errors
    ///
    /// Returns an error when the definition, task, host pins, or durable transaction is invalid.
    pub fn create_ssh_workflow(
        &mut self,
        workflow: &SshWorkflow,
    ) -> Result<(SshWorkflow, bool, Vec<EventEnvelope>), StorageError> {
        validate_workflow(workflow, true)?;
        if let Some(existing) = self.ssh_workflow(workflow.id)? {
            if existing.definition_hash == workflow.definition_hash {
                return Ok((existing, false, Vec::new()));
            }
            return Err(StorageError::Conflict(format!(
                "SSH workflow {} already exists with a different definition",
                workflow.id
            )));
        }
        if let Some(task_id) = workflow.task_id
            && self.load_task(task_id)?.is_none()
        {
            return Err(StorageError::TaskNotFound(task_id));
        }
        for step in &workflow.steps {
            let host = self
                .ssh_host(step.host_id)?
                .ok_or(StorageError::SshHostNotFound(step.host_id))?;
            if host.state != SshHostState::Active
                || host.revision != step.host_revision
                || step
                    .required_capabilities
                    .iter()
                    .any(|required| host.capabilities.binary_search(required).is_err())
            {
                return Err(StorageError::Conflict(format!(
                    "SSH workflow step {} does not pin the current active host revision",
                    step.id
                )));
            }
        }
        let event = EventEnvelope::new(
            EventKind::SshWorkflowCreated,
            workflow.task_id,
            serde_json::to_value(workflow)?,
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO ssh_workflows(
                workflow_id, task_id, state, definition_hash, workflow_json, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                workflow.id.to_string(),
                workflow.task_id.map(|id| id.to_string()),
                workflow_state_name(workflow.state),
                workflow.definition_hash,
                serde_json::to_string(workflow)?,
                workflow.created_at.to_rfc3339(),
                workflow.updated_at.to_rfc3339(),
            ],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok((workflow.clone(), true, vec![event]))
    }

    /// Loads one current workflow projection.
    ///
    /// # Errors
    ///
    /// Returns an error when the durable projection cannot be read or decoded.
    pub fn ssh_workflow(
        &self,
        workflow_id: SshWorkflowId,
    ) -> Result<Option<SshWorkflow>, StorageError> {
        self.read_optional_json(
            "SELECT workflow_json FROM ssh_workflows WHERE workflow_id = ?1",
            &workflow_id.to_string(),
        )
    }

    /// Lists workflows in deterministic creation order.
    ///
    /// # Errors
    ///
    /// Returns an error when the durable projections cannot be read or decoded.
    pub fn ssh_workflows(&self) -> Result<Vec<SshWorkflow>, StorageError> {
        let mut statement = self
            .connection
            .prepare("SELECT workflow_json FROM ssh_workflows ORDER BY created_at, workflow_id")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.map(|row| serde_json::from_str(&row?).map_err(StorageError::from))
            .collect()
    }

    /// Commits one legal workflow projection change and its event atomically.
    ///
    /// # Errors
    ///
    /// Returns an error for stale authority, illegal transitions, or transaction failures.
    pub fn update_ssh_workflow(
        &mut self,
        expected_state: SshWorkflowState,
        workflow: &SshWorkflow,
    ) -> Result<(SshWorkflow, EventEnvelope), StorageError> {
        validate_workflow(workflow, false)?;
        let current = self
            .ssh_workflow(workflow.id)?
            .ok_or(StorageError::SshWorkflowNotFound(workflow.id))?;
        if current.definition_hash != workflow.definition_hash
            || current.created_at != workflow.created_at
            || current.task_id != workflow.task_id
            || current.state != expected_state
        {
            return Err(StorageError::Conflict(
                "SSH workflow authority or immutable definition changed".into(),
            ));
        }
        if !workflow_transition_allowed(expected_state, workflow.state) {
            return Err(StorageError::InvalidData(format!(
                "invalid SSH workflow transition from {expected_state:?} to {:?}",
                workflow.state
            )));
        }
        if !same_workflow_definition(&current, workflow) {
            return Err(StorageError::Conflict(
                "SSH workflow steps or pinned host authority changed".into(),
            ));
        }
        let event = EventEnvelope::new(
            EventKind::SshWorkflowStateChanged,
            workflow.task_id,
            serde_json::json!({"from": expected_state, "workflow": workflow}),
        );
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE ssh_workflows
             SET state = ?1, workflow_json = ?2, updated_at = ?3
             WHERE workflow_id = ?4 AND state = ?5 AND definition_hash = ?6",
            params![
                workflow_state_name(workflow.state),
                serde_json::to_string(workflow)?,
                workflow.updated_at.to_rfc3339(),
                workflow.id.to_string(),
                workflow_state_name(expected_state),
                workflow.definition_hash,
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::Conflict(
                "SSH workflow state authority changed".into(),
            ));
        }
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok((workflow.clone(), event))
    }

    /// Reconciles in-flight workflow projections from the authoritative SSH operation ledger.
    /// No remote operation is started or retried during recovery.
    ///
    /// # Errors
    ///
    /// Returns an error when operation or workflow projections cannot be reconciled durably.
    pub fn recover_ssh_workflows(
        &mut self,
        now: DateTime<Utc>,
    ) -> Result<(u64, Vec<EventEnvelope>), StorageError> {
        let workflows = self.ssh_workflows()?;
        let mut recovered = 0_u64;
        let mut events = Vec::new();
        for mut workflow in workflows
            .into_iter()
            .filter(|workflow| !workflow.state.is_terminal())
        {
            let previous = workflow.state;
            let mut changed = false;
            for step in &mut workflow.steps {
                if step.state != SshWorkflowStepState::Pending {
                    continue;
                }
                let Some(operation) = self.ssh_operation(step.operation_id)? else {
                    continue;
                };
                let next = match operation.state {
                    SshOperationState::Executing | SshOperationState::Uncertain => {
                        SshWorkflowStepState::Uncertain
                    }
                    SshOperationState::Succeeded => SshWorkflowStepState::Succeeded,
                    SshOperationState::Failed => SshWorkflowStepState::Failed,
                };
                step.state = next;
                step.detail = operation.detail;
                changed = true;
            }
            if !changed {
                continue;
            }
            skip_dependents(&mut workflow);
            workflow.state = derive_workflow_state(&workflow);
            workflow.updated_at = now;
            let (_, event) = self.update_ssh_workflow(previous, &workflow)?;
            events.push(event);
            recovered = recovered.saturating_add(1);
        }
        Ok((recovered, events))
    }
}

fn validate_workflow(workflow: &SshWorkflow, creation: bool) -> Result<(), StorageError> {
    let valid_header = workflow.schema_version == SSH_WORKFLOW_SCHEMA_VERSION
        && workflow.definition_hash.len() == 64
        && workflow
            .definition_hash
            .bytes()
            .all(|value| value.is_ascii_hexdigit())
        && !workflow.display_name.trim().is_empty()
        && workflow.display_name.chars().count() <= 160
        && !workflow.steps.is_empty()
        && workflow.steps.len() <= 128
        && workflow.created_at <= workflow.updated_at;
    if !valid_header
        || (creation && workflow.state != SshWorkflowState::Planned)
        || (creation
            && workflow
                .steps
                .iter()
                .any(|step| step.state != SshWorkflowStepState::Pending || !step.detail.is_empty()))
    {
        return Err(StorageError::InvalidData("invalid SSH workflow".into()));
    }
    let ids = workflow
        .steps
        .iter()
        .map(|step| step.id)
        .collect::<BTreeSet<_>>();
    let operation_ids = workflow
        .steps
        .iter()
        .map(|step| step.operation_id)
        .collect::<BTreeSet<_>>();
    if ids.len() != workflow.steps.len()
        || operation_ids.len() != workflow.steps.len()
        || workflow.steps.iter().any(|step| {
            step.name.trim().is_empty()
                || step.name.chars().count() > 160
                || step.host_revision == 0
                || step.required_capabilities.len() > 64
                || step
                    .required_capabilities
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
                || step.required_capabilities.iter().any(|value| {
                    value.is_empty()
                        || value.chars().count() > 96
                        || !value.chars().all(|character| {
                            character.is_ascii_lowercase()
                                || character.is_ascii_digit()
                                || matches!(character, ':' | '.' | '_' | '-')
                        })
                })
                || !(1..=3_600).contains(&step.timeout_seconds)
                || step.detail.chars().count() > 1_024
                || step.dependencies.len() > 64
                || step
                    .dependencies
                    .iter()
                    .any(|dependency| *dependency == step.id || !ids.contains(dependency))
        })
        || workflow_has_cycle(workflow)
    {
        return Err(StorageError::InvalidData(
            "invalid SSH workflow step graph".into(),
        ));
    }
    Ok(())
}

fn workflow_has_cycle(workflow: &SshWorkflow) -> bool {
    let dependencies = workflow
        .steps
        .iter()
        .map(|step| (step.id, step.dependencies.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut remaining = dependencies.clone();
    loop {
        let ready = remaining
            .iter()
            .filter(|(_, values)| values.iter().all(|value| !remaining.contains_key(value)))
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        if ready.is_empty() {
            return !remaining.is_empty();
        }
        for id in ready {
            remaining.remove(&id);
        }
    }
}

fn same_workflow_definition(current: &SshWorkflow, next: &SshWorkflow) -> bool {
    current.display_name == next.display_name
        && current.steps.len() == next.steps.len()
        && current.steps.iter().zip(&next.steps).all(|(left, right)| {
            left.id == right.id
                && left.name == right.name
                && left.host_id == right.host_id
                && left.host_revision == right.host_revision
                && left.required_capabilities == right.required_capabilities
                && left.routed_automatically == right.routed_automatically
                && left.dependencies == right.dependencies
                && left.operation_id == right.operation_id
                && left.operation == right.operation
                && left.timeout_seconds == right.timeout_seconds
        })
}

fn workflow_transition_allowed(from: SshWorkflowState, to: SshWorkflowState) -> bool {
    from == to
        || matches!(
            (from, to),
            (
                SshWorkflowState::Planned | SshWorkflowState::Waiting,
                SshWorkflowState::Running
            ) | (
                SshWorkflowState::Planned | SshWorkflowState::Running | SshWorkflowState::Waiting,
                SshWorkflowState::Cancelled
            ) | (
                SshWorkflowState::Running,
                SshWorkflowState::Waiting
                    | SshWorkflowState::Succeeded
                    | SshWorkflowState::Failed
                    | SshWorkflowState::Uncertain
            ) | (
                SshWorkflowState::Waiting,
                SshWorkflowState::Failed | SshWorkflowState::Uncertain
            )
        )
}

fn skip_dependents(workflow: &mut SshWorkflow) {
    let terminal = workflow
        .steps
        .iter()
        .filter(|step| {
            matches!(
                step.state,
                SshWorkflowStepState::Failed
                    | SshWorkflowStepState::Uncertain
                    | SshWorkflowStepState::Skipped
            )
        })
        .map(|step| step.id)
        .collect::<BTreeSet<_>>();
    let mut blocked = terminal;
    loop {
        let additions = workflow
            .steps
            .iter()
            .filter(|step| {
                step.state == SshWorkflowStepState::Pending
                    && step
                        .dependencies
                        .iter()
                        .any(|dependency| blocked.contains(dependency))
            })
            .map(|step| step.id)
            .collect::<Vec<_>>();
        if additions.is_empty() {
            break;
        }
        for id in additions {
            blocked.insert(id);
            if let Some(step) = workflow.steps.iter_mut().find(|step| step.id == id) {
                step.state = SshWorkflowStepState::Skipped;
                step.detail = "dependency did not complete successfully".into();
            }
        }
    }
}

fn derive_workflow_state(workflow: &SshWorkflow) -> SshWorkflowState {
    if workflow
        .steps
        .iter()
        .any(|step| step.state == SshWorkflowStepState::Uncertain)
    {
        SshWorkflowState::Uncertain
    } else if workflow.steps.iter().any(|step| {
        matches!(
            step.state,
            SshWorkflowStepState::Failed | SshWorkflowStepState::Skipped
        )
    }) {
        SshWorkflowState::Failed
    } else if workflow
        .steps
        .iter()
        .all(|step| step.state == SshWorkflowStepState::Succeeded)
    {
        SshWorkflowState::Succeeded
    } else {
        SshWorkflowState::Running
    }
}

const fn workflow_state_name(state: SshWorkflowState) -> &'static str {
    match state {
        SshWorkflowState::Planned => "planned",
        SshWorkflowState::Running => "running",
        SshWorkflowState::Waiting => "waiting",
        SshWorkflowState::Succeeded => "succeeded",
        SshWorkflowState::Failed => "failed",
        SshWorkflowState::Uncertain => "uncertain",
        SshWorkflowState::Cancelled => "cancelled",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcg_domain::{
        SSH_SCHEMA_VERSION, SshAuthentication, SshHost, SshHostId, SshHostPlatform,
        SshHostRevision, SshHostRevisionId, SshOperation, SshOperationId, SshOperationKind,
        SshWorkflowStep, SshWorkflowStepId,
    };

    fn host(now: DateTime<Utc>) -> (SshHost, SshHostRevision) {
        let host = SshHost {
            schema_version: SSH_SCHEMA_VERSION,
            id: SshHostId::new(),
            definition_hash: "a".repeat(64),
            revision: 1,
            display_name: "Build host".into(),
            hostname: "build.example.test".into(),
            port: 22,
            username: "builder".into(),
            platform: SshHostPlatform::Posix,
            authentication: SshAuthentication::Agent,
            known_hosts_entry: "build.example.test ssh-ed25519 AAAAC3NzaFixture".into(),
            host_key_sha256: "SHA256:fixture".into(),
            capabilities: vec!["os:linux".into()],
            state: SshHostState::Active,
            created_at: now,
            updated_at: now,
        };
        let revision = SshHostRevision {
            schema_version: SSH_SCHEMA_VERSION,
            id: SshHostRevisionId::new(),
            host_id: host.id,
            revision: 1,
            definition_hash: host.definition_hash.clone(),
            host: host.clone(),
            reason: "fixture".into(),
            created_at: now,
        };
        (host, revision)
    }

    fn workflow(host: &SshHost, now: DateTime<Utc>) -> SshWorkflow {
        let first = SshWorkflowStepId::new();
        let second = SshWorkflowStepId::new();
        SshWorkflow {
            schema_version: SSH_WORKFLOW_SCHEMA_VERSION,
            id: SshWorkflowId::new(),
            definition_hash: "b".repeat(64),
            display_name: "Build and verify".into(),
            task_id: None,
            state: SshWorkflowState::Planned,
            steps: vec![
                SshWorkflowStep {
                    id: first,
                    name: "inspect".into(),
                    host_id: host.id,
                    host_revision: host.revision,
                    required_capabilities: Vec::new(),
                    routed_automatically: false,
                    dependencies: Vec::new(),
                    operation_id: SshOperationId::new(),
                    operation: SshOperationKind::Inspect,
                    timeout_seconds: 60,
                    state: SshWorkflowStepState::Pending,
                    detail: String::new(),
                },
                SshWorkflowStep {
                    id: second,
                    name: "verify".into(),
                    host_id: host.id,
                    host_revision: host.revision,
                    required_capabilities: Vec::new(),
                    routed_automatically: false,
                    dependencies: vec![first],
                    operation_id: SshOperationId::new(),
                    operation: SshOperationKind::Inspect,
                    timeout_seconds: 60,
                    state: SshWorkflowStepState::Pending,
                    detail: String::new(),
                },
            ],
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn workflow_creation_is_atomic_idempotent_and_graph_validated() {
        let mut store = SqliteEventStore::open_in_memory().expect("store");
        let now = Utc::now();
        let (host, revision) = host(now);
        store.create_ssh_host(&host, &revision).expect("host");
        let workflow = workflow(&host, now);
        let created = store.create_ssh_workflow(&workflow).expect("workflow");
        assert!(created.1);
        assert_eq!(created.2.len(), 1);
        assert!(!store.create_ssh_workflow(&workflow).expect("replay").1);

        let mut cyclic = workflow.clone();
        cyclic.id = SshWorkflowId::new();
        cyclic.steps[0].dependencies = vec![cyclic.steps[1].id];
        assert!(matches!(
            store.create_ssh_workflow(&cyclic),
            Err(StorageError::InvalidData(_))
        ));
    }

    #[test]
    fn crash_recovery_marks_claimed_step_uncertain_without_reexecution() {
        let directory = tempfile::tempdir().expect("directory");
        let database = directory.path().join("workflow-crash.sqlite3");
        let now = Utc::now();
        let (host, revision) = host(now);
        let mut workflow = workflow(&host, now);
        {
            let mut store = SqliteEventStore::open(&database).expect("store");
            store.create_ssh_host(&host, &revision).expect("host");
            store.create_ssh_workflow(&workflow).expect("workflow");
            let previous = workflow.state;
            workflow.state = SshWorkflowState::Running;
            store
                .update_ssh_workflow(previous, &workflow)
                .expect("running");
            let step = &workflow.steps[0];
            store
                .claim_ssh_operation(&SshOperation {
                    schema_version: SSH_SCHEMA_VERSION,
                    id: step.operation_id,
                    host_id: step.host_id,
                    host_revision: step.host_revision,
                    task_id: workflow.task_id,
                    request_hash: "c".repeat(64),
                    kind: step.operation.clone(),
                    state: SshOperationState::Executing,
                    detail: String::new(),
                    exit_code: None,
                    artifact_sha256: Vec::new(),
                    created_at: now,
                    updated_at: now,
                })
                .expect("claim");
        }

        let mut reopened = SqliteEventStore::open(&database).expect("reopen");
        assert_eq!(reopened.recover_ssh_operations(now).expect("operations"), 1);
        let (recovered, events) = reopened.recover_ssh_workflows(now).expect("workflows");
        assert_eq!(recovered, 1);
        assert_eq!(events.len(), 1);
        let recovered = reopened
            .ssh_workflow(workflow.id)
            .expect("workflow")
            .expect("present");
        assert_eq!(recovered.state, SshWorkflowState::Uncertain);
        assert_eq!(recovered.steps[0].state, SshWorkflowStepState::Uncertain);
        assert_eq!(recovered.steps[1].state, SshWorkflowStepState::Skipped);
        assert_eq!(
            reopened
                .ssh_operation(workflow.steps[0].operation_id)
                .expect("operation")
                .expect("present")
                .state,
            SshOperationState::Uncertain
        );
        assert_eq!(reopened.ssh_operations(host.id).expect("history").len(), 1);
        assert_eq!(reopened.recover_ssh_workflows(now).expect("replay").0, 0);
    }

    #[test]
    fn recovery_closes_succeeded_operation_projection_gap() {
        let directory = tempfile::tempdir().expect("directory");
        let database = directory.path().join("workflow-success.sqlite3");
        let now = Utc::now();
        let (host, revision) = host(now);
        let mut workflow = workflow(&host, now);
        {
            let mut store = SqliteEventStore::open(&database).expect("store");
            store.create_ssh_host(&host, &revision).expect("host");
            store.create_ssh_workflow(&workflow).expect("workflow");
            workflow.state = SshWorkflowState::Running;
            store
                .update_ssh_workflow(SshWorkflowState::Planned, &workflow)
                .expect("running");
            let step = &workflow.steps[0];
            store
                .claim_ssh_operation(&SshOperation {
                    schema_version: SSH_SCHEMA_VERSION,
                    id: step.operation_id,
                    host_id: step.host_id,
                    host_revision: step.host_revision,
                    task_id: workflow.task_id,
                    request_hash: "d".repeat(64),
                    kind: step.operation.clone(),
                    state: SshOperationState::Executing,
                    detail: String::new(),
                    exit_code: None,
                    artifact_sha256: Vec::new(),
                    created_at: now,
                    updated_at: now,
                })
                .expect("claim");
            store
                .finish_ssh_operation(
                    step.operation_id,
                    SshOperationState::Succeeded,
                    "remote inspection completed",
                    Some(0),
                    Vec::new(),
                    now,
                )
                .expect("finish");
        }

        let mut reopened = SqliteEventStore::open(&database).expect("reopen");
        assert_eq!(reopened.recover_ssh_operations(now).expect("operations"), 0);
        assert_eq!(reopened.recover_ssh_workflows(now).expect("workflows").0, 1);
        let recovered = reopened
            .ssh_workflow(workflow.id)
            .expect("workflow")
            .expect("present");
        assert_eq!(recovered.state, SshWorkflowState::Running);
        assert_eq!(recovered.steps[0].state, SshWorkflowStepState::Succeeded);
        assert_eq!(recovered.steps[0].detail, "remote inspection completed");
        assert_eq!(recovered.steps[1].state, SshWorkflowStepState::Pending);
        assert_eq!(reopened.recover_ssh_workflows(now).expect("replay").0, 0);
    }
}
