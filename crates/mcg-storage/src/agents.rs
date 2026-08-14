use chrono::{DateTime, Utc};
use mcg_agents::{
    AGENT_RUN_SCHEMA_VERSION, AgentInstance, AgentLease, AgentLeaseState, AgentRun, AgentRunState,
    AgentState, AgentTurnResult, AgentUsageRecord, AssignedModel, ConfidenceEstimate,
    EvaluationReport, OrganizationGenome, OrganizationalMetrics, UsageKind, WorkPlan,
};
use mcg_domain::{
    AgentInstanceId, AgentLeaseId, OrganizationId, OrganizationRevisionId, RunId, TaskEnvelope,
    TaskState,
};
use mcg_events::{EventEnvelope, EventKind, TaskCreatedPayload, TaskStateChangedPayload};
use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};

use crate::{SqliteEventStore, StorageError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AgentRecoveryReport {
    pub runs_marked_recovering: u64,
    pub instances_expired: u64,
    pub leases_expired: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentTurnCommit {
    pub run: AgentRun,
    pub instance: AgentInstance,
    pub lease: AgentLease,
    pub usage: Vec<AgentUsageRecord>,
    pub result: AgentTurnResult,
    pub events: Vec<EventEnvelope>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentTurnDisposition {
    Waiting,
    Failed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceRunReceipt {
    pub task: TaskEnvelope,
    pub run: AgentRun,
    pub replayed: bool,
    pub events: Vec<EventEnvelope>,
}

impl SqliteEventStore {
    /// Atomically creates an idempotent interface-owned task, organization,
    /// immutable plan, active task projection, and running agent run.
    ///
    /// # Errors
    ///
    /// A request key may only be reused with the same canonical request hash.
    /// All task, organization, run, plan, project, and budget invariants must
    /// validate before any row or event is committed.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub fn create_interface_agent_run(
        &mut self,
        interface: &str,
        request_key: &str,
        request_hash: &str,
        task: &TaskEnvelope,
        genome: &OrganizationGenome,
        run: &AgentRun,
        plan: &WorkPlan,
    ) -> Result<InterfaceRunReceipt, StorageError> {
        if interface.is_empty()
            || interface.len() > 32
            || request_key.is_empty()
            || request_key.len() > 128
            || request_hash.len() != 64
            || !request_hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(StorageError::InvalidData(
                "interface request identity is invalid".into(),
            ));
        }
        let existing = self
            .connection
            .query_row(
                "SELECT request_hash, task_id, run_id FROM interface_run_requests
                 WHERE interface = ?1 AND request_key = ?2",
                params![interface, request_key],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?;
        if let Some((stored_hash, task_id, run_id)) = existing {
            if stored_hash != request_hash {
                return Err(StorageError::Conflict(
                    "interface request key was already used with different input".into(),
                ));
            }
            let task_id = task_id.parse().map_err(|_| {
                StorageError::InvalidData("stored interface task id is invalid".into())
            })?;
            let run_id = run_id.parse().map_err(|_| {
                StorageError::InvalidData("stored interface run id is invalid".into())
            })?;
            return Ok(InterfaceRunReceipt {
                task: self
                    .load_task(task_id)?
                    .ok_or(StorageError::TaskNotFound(task_id))?,
                run: self
                    .agent_run(run_id)?
                    .ok_or(StorageError::AgentRunNotFound(run_id))?,
                replayed: true,
                events: Vec::new(),
            });
        }

        if task.state != TaskState::Pending
            || run.schema_version != AGENT_RUN_SCHEMA_VERSION
            || run.state != AgentRunState::Planned
            || run.completed_at.is_some()
            || run.id != plan.run_id
            || task.id != run.task_id
            || task.id != plan.task_id
            || run.organization_revision_id != genome.revision_id
            || run.organization_revision_id != plan.organization_revision_id
            || genome.revision != 1
            || genome.parent_revision_id.is_some()
        {
            return Err(StorageError::InvalidData(
                "new interface task, organization, run, and plan identities are inconsistent"
                    .into(),
            ));
        }
        if let Some(project_id) = task.project_id
            && self.project(project_id)?.is_none()
        {
            return Err(StorageError::ProjectNotFound(project_id));
        }
        genome
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        plan.validate(genome)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        if !budgets_within_parent(plan, &task.budget) || !metrics_valid(&run.metrics) {
            return Err(StorageError::InvalidData(
                "interface run metrics or aggregate work budgets are invalid".into(),
            ));
        }
        if self.organization(genome.organization_id)?.is_some() {
            return Err(StorageError::Conflict(
                "interface organization identity already exists".into(),
            ));
        }

        let mut active_task = task.clone();
        active_task.transition_to(TaskState::Active)?;
        let mut running_run = run.clone();
        running_run.state = AgentRunState::Running;
        running_run.updated_at = active_task.updated_at;
        let created_event = EventEnvelope::new(
            EventKind::TaskCreated,
            Some(task.id),
            serde_json::to_value(TaskCreatedPayload { task: task.clone() })?,
        );
        let organization_event = EventEnvelope::new(
            EventKind::OrganizationRevisionRecorded,
            None,
            serde_json::to_value(genome)?,
        );
        let planned_event = EventEnvelope::new(
            EventKind::AgentRunPlanned,
            Some(task.id),
            serde_json::json!({"run": run, "plan": plan}),
        );
        let active_event = EventEnvelope::new(
            EventKind::TaskStateChanged,
            Some(task.id),
            serde_json::to_value(TaskStateChangedPayload {
                from: TaskState::Pending,
                to: TaskState::Active,
            })?,
        );
        let running_event = EventEnvelope::new(
            EventKind::AgentRunStateChanged,
            Some(task.id),
            serde_json::json!({
                "from": AgentRunState::Planned,
                "to": AgentRunState::Running,
                "run": &running_run,
            }),
        );

        let transaction = self.connection.transaction()?;
        let mut events = Vec::new();
        let event = Self::insert_event(&transaction, &created_event)?;
        Self::upsert_task(&transaction, task, event.sequence)?;
        events.push(event);
        transaction.execute(
            "INSERT INTO organization_revisions(
                revision_id, organization_id, revision, parent_revision_id,
                organization_json, created_at
             ) VALUES (?1, ?2, 1, NULL, ?3, ?4)",
            params![
                genome.revision_id.to_string(),
                genome.organization_id.to_string(),
                serde_json::to_string(genome)?,
                genome.created_at.to_rfc3339(),
            ],
        )?;
        transaction.execute(
            "INSERT INTO organizations(
                organization_id, current_revision_id, organization_json, updated_at
             ) VALUES (?1, ?2, ?3, ?4)",
            params![
                genome.organization_id.to_string(),
                genome.revision_id.to_string(),
                serde_json::to_string(genome)?,
                genome.created_at.to_rfc3339(),
            ],
        )?;
        events.push(Self::insert_event(&transaction, &organization_event)?);
        transaction.execute(
            "INSERT INTO agent_runs(
                run_id, task_id, organization_revision_id, state, run_json,
                created_at, updated_at, completed_at
             ) VALUES (?1, ?2, ?3, 'planned', ?4, ?5, ?6, NULL)",
            params![
                run.id.to_string(),
                task.id.to_string(),
                run.organization_revision_id.to_string(),
                serde_json::to_string(run)?,
                run.created_at.to_rfc3339(),
                run.updated_at.to_rfc3339(),
            ],
        )?;
        transaction.execute(
            "INSERT INTO work_plans(run_id, task_id, plan_json, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                run.id.to_string(),
                task.id.to_string(),
                serde_json::to_string(plan)?,
                plan.created_at.to_rfc3339(),
            ],
        )?;
        events.push(Self::insert_event(&transaction, &planned_event)?);
        let event = Self::insert_event(&transaction, &active_event)?;
        Self::upsert_task(&transaction, &active_task, event.sequence)?;
        events.push(event);
        let changed = transaction.execute(
            "UPDATE agent_runs SET state = 'running', run_json = ?1, updated_at = ?2
             WHERE run_id = ?3 AND state = 'planned'",
            params![
                serde_json::to_string(&running_run)?,
                running_run.updated_at.to_rfc3339(),
                running_run.id.to_string(),
            ],
        )?;
        ensure_single_change(changed, "interface run could not start atomically")?;
        events.push(Self::insert_event(&transaction, &running_event)?);
        transaction.execute(
            "INSERT INTO interface_run_requests(
                interface, request_key, request_hash, task_id, run_id, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                interface,
                request_key,
                request_hash,
                task.id.to_string(),
                run.id.to_string(),
                running_run.updated_at.to_rfc3339(),
            ],
        )?;
        transaction.commit()?;
        Ok(InterfaceRunReceipt {
            task: active_task,
            run: running_run,
            replayed: false,
            events,
        })
    }

    /// Atomically stores an immutable organization revision, updates its current
    /// projection, and appends replay evidence.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, stale, skipped, or conflicting revisions.
    pub fn record_organization_revision(
        &mut self,
        genome: &OrganizationGenome,
    ) -> Result<EventEnvelope, StorageError> {
        genome
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let current = self.organization(genome.organization_id)?;
        match current {
            None if genome.revision != 1 || genome.parent_revision_id.is_some() => {
                return Err(StorageError::Conflict(
                    "first organization revision must be revision 1 without a parent".into(),
                ));
            }
            Some(ref current)
                if genome.revision != current.revision.saturating_add(1)
                    || genome.parent_revision_id != Some(current.revision_id) =>
            {
                return Err(StorageError::Conflict(
                    "organization revision does not extend the current head".into(),
                ));
            }
            _ => {}
        }
        let event = EventEnvelope::new(
            EventKind::OrganizationRevisionRecorded,
            None,
            serde_json::to_value(genome)?,
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO organization_revisions(
                revision_id, organization_id, revision, parent_revision_id,
                organization_json, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                genome.revision_id.to_string(),
                genome.organization_id.to_string(),
                i64::try_from(genome.revision).map_err(|_| StorageError::InvalidData(
                    "organization revision exceeds SQLite range".into()
                ))?,
                genome.parent_revision_id.map(|id| id.to_string()),
                serde_json::to_string(genome)?,
                genome.created_at.to_rfc3339(),
            ],
        )?;
        transaction.execute(
            "INSERT INTO organizations(
                organization_id, current_revision_id, organization_json, updated_at
             ) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(organization_id) DO UPDATE SET
                current_revision_id = excluded.current_revision_id,
                organization_json = excluded.organization_json,
                updated_at = excluded.updated_at",
            params![
                genome.organization_id.to_string(),
                genome.revision_id.to_string(),
                serde_json::to_string(genome)?,
                genome.created_at.to_rfc3339(),
            ],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    /// Loads the current organization projection.
    ///
    /// # Errors
    ///
    /// Returns an error when its persisted JSON is invalid.
    pub fn organization(
        &self,
        organization_id: OrganizationId,
    ) -> Result<Option<OrganizationGenome>, StorageError> {
        self.read_optional_json(
            "SELECT organization_json FROM organizations WHERE organization_id = ?1",
            &organization_id.to_string(),
        )
    }

    /// Lists current organization projections in stable identity order.
    ///
    /// # Errors
    ///
    /// Returns an error when a row cannot be decoded.
    pub fn organizations(&self) -> Result<Vec<OrganizationGenome>, StorageError> {
        self.read_all_json("SELECT organization_json FROM organizations ORDER BY organization_id")
    }

    /// Loads one immutable organization revision.
    ///
    /// # Errors
    ///
    /// Returns an error when its persisted JSON is invalid.
    pub fn organization_revision(
        &self,
        revision_id: OrganizationRevisionId,
    ) -> Result<Option<OrganizationGenome>, StorageError> {
        self.read_optional_json(
            "SELECT organization_json FROM organization_revisions WHERE revision_id = ?1",
            &revision_id.to_string(),
        )
    }

    /// Loads organization revisions in numeric order.
    ///
    /// # Errors
    ///
    /// Returns an error when a row cannot be read or decoded.
    pub fn organization_history(
        &self,
        organization_id: OrganizationId,
    ) -> Result<Vec<OrganizationGenome>, StorageError> {
        self.read_json_rows(
            "SELECT organization_json FROM organization_revisions
             WHERE organization_id = ?1 ORDER BY revision",
            &organization_id.to_string(),
        )
    }

    /// Atomically persists a planned run, its immutable work plan, and event.
    ///
    /// # Errors
    ///
    /// The task and organization revision must exist and all identities and plan
    /// invariants must agree.
    pub fn create_agent_run(
        &mut self,
        run: &AgentRun,
        plan: &WorkPlan,
    ) -> Result<EventEnvelope, StorageError> {
        if run.schema_version != AGENT_RUN_SCHEMA_VERSION
            || run.state != AgentRunState::Planned
            || run.completed_at.is_some()
            || run.id != plan.run_id
            || run.task_id != plan.task_id
            || run.organization_revision_id != plan.organization_revision_id
        {
            return Err(StorageError::InvalidData(
                "new agent run and work plan identities or initial state are inconsistent".into(),
            ));
        }
        let task = self
            .load_task(run.task_id)?
            .ok_or(StorageError::TaskNotFound(run.task_id))?;
        let genome = self
            .organization_revision(run.organization_revision_id)?
            .ok_or(StorageError::OrganizationRevisionNotFound(
                run.organization_revision_id,
            ))?;
        plan.validate(&genome)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        if !budgets_within_parent(plan, &task.budget) || !metrics_valid(&run.metrics) {
            return Err(StorageError::InvalidData(
                "agent run metrics or aggregate work budgets are invalid".into(),
            ));
        }
        let event = EventEnvelope::new(
            EventKind::AgentRunPlanned,
            Some(run.task_id),
            serde_json::json!({"run": run, "plan": plan}),
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO agent_runs(
                run_id, task_id, organization_revision_id, state, run_json,
                created_at, updated_at, completed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)",
            params![
                run.id.to_string(),
                run.task_id.to_string(),
                run.organization_revision_id.to_string(),
                run_state_name(run.state),
                serde_json::to_string(run)?,
                run.created_at.to_rfc3339(),
                run.updated_at.to_rfc3339(),
            ],
        )?;
        transaction.execute(
            "INSERT INTO work_plans(run_id, task_id, plan_json, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                run.id.to_string(),
                run.task_id.to_string(),
                serde_json::to_string(plan)?,
                plan.created_at.to_rfc3339(),
            ],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    /// Loads a current run projection.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid persisted JSON.
    pub fn agent_run(&self, run_id: RunId) -> Result<Option<AgentRun>, StorageError> {
        self.read_optional_json(
            "SELECT run_json FROM agent_runs WHERE run_id = ?1",
            &run_id.to_string(),
        )
    }

    /// Loads the immutable work plan for a run.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid persisted JSON.
    pub fn work_plan(&self, run_id: RunId) -> Result<Option<WorkPlan>, StorageError> {
        self.read_optional_json(
            "SELECT plan_json FROM work_plans WHERE run_id = ?1",
            &run_id.to_string(),
        )
    }

    /// Transitions the run projection and appends its authoritative event.
    ///
    /// # Errors
    ///
    /// Returns an error for unknown runs, illegal transitions, or stale writes.
    pub fn transition_agent_run(
        &mut self,
        run_id: RunId,
        next: AgentRunState,
        metrics: OrganizationalMetrics,
        at: DateTime<Utc>,
    ) -> Result<(AgentRun, EventEnvelope), StorageError> {
        let mut run = self
            .agent_run(run_id)?
            .ok_or(StorageError::AgentRunNotFound(run_id))?;
        let previous = run.state;
        if !previous.can_transition_to(next) {
            return Err(StorageError::Conflict(format!(
                "illegal agent run transition from {previous:?} to {next:?}"
            )));
        }
        if !metrics_valid(&metrics) {
            return Err(StorageError::InvalidData(
                "agent run metrics must be finite and non-negative".into(),
            ));
        }
        run.state = next;
        run.metrics = metrics;
        run.updated_at = at;
        run.completed_at = next.is_terminal().then_some(at);
        let event = EventEnvelope::new(
            EventKind::AgentRunStateChanged,
            Some(run.task_id),
            serde_json::json!({"from": previous, "to": next, "run": run}),
        );
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE agent_runs SET state = ?1, run_json = ?2, updated_at = ?3,
                completed_at = ?4 WHERE run_id = ?5 AND state = ?6",
            params![
                run_state_name(next),
                serde_json::to_string(&run)?,
                at.to_rfc3339(),
                run.completed_at.map(|value| value.to_rfc3339()),
                run_id.to_string(),
                run_state_name(previous),
            ],
        )?;
        ensure_single_change(changed, "agent run changed concurrently")?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok((run, event))
    }

    /// Persists a newly created ephemeral agent instance and event.
    ///
    /// # Errors
    ///
    /// The run, selected role, task, schema, and initial state must agree.
    pub fn record_agent_instance(
        &mut self,
        instance: &AgentInstance,
    ) -> Result<EventEnvelope, StorageError> {
        let run = self
            .agent_run(instance.run_id)?
            .ok_or(StorageError::AgentRunNotFound(instance.run_id))?;
        let plan = self
            .work_plan(instance.run_id)?
            .ok_or(StorageError::InvalidData(
                "agent run has no work plan".into(),
            ))?;
        if instance.schema_version != AGENT_RUN_SCHEMA_VERSION
            || instance.state != AgentState::Created
            || instance.task_id != run.task_id
            || !plan.selected_roles.contains(&instance.role_id)
            || instance.context_fingerprint.trim().is_empty()
            || instance.confidence.basis_points > 10_000
        {
            return Err(StorageError::InvalidData(
                "agent instance is inconsistent with its selected run role".into(),
            ));
        }
        let event = EventEnvelope::new(
            EventKind::AgentInstanceRecorded,
            Some(instance.task_id),
            serde_json::to_value(instance)?,
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO agent_instances(
                instance_id, run_id, task_id, role_id, state, instance_json,
                created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                instance.id.to_string(),
                instance.run_id.to_string(),
                instance.task_id.to_string(),
                instance.role_id.to_string(),
                instance_state_name(instance.state),
                serde_json::to_string(instance)?,
                instance.created_at.to_rfc3339(),
                instance.updated_at.to_rfc3339(),
            ],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    /// Loads a current agent instance projection.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid persisted JSON.
    pub fn agent_instance(
        &self,
        instance_id: AgentInstanceId,
    ) -> Result<Option<AgentInstance>, StorageError> {
        self.read_optional_json(
            "SELECT instance_json FROM agent_instances WHERE instance_id = ?1",
            &instance_id.to_string(),
        )
    }

    /// Lists current agent instances for a run in creation order.
    ///
    /// # Errors
    ///
    /// Returns an error when a row cannot be decoded.
    pub fn agent_instances(&self, run_id: RunId) -> Result<Vec<AgentInstance>, StorageError> {
        self.read_json_rows(
            "SELECT instance_json FROM agent_instances WHERE run_id = ?1
             ORDER BY created_at, instance_id",
            &run_id.to_string(),
        )
    }

    /// Transitions an agent instance and records confidence at that boundary.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown instance or illegal/stale transition.
    pub fn transition_agent_instance(
        &mut self,
        instance_id: AgentInstanceId,
        next: AgentState,
        confidence: ConfidenceEstimate,
        at: DateTime<Utc>,
    ) -> Result<(AgentInstance, EventEnvelope), StorageError> {
        let mut instance = self
            .agent_instance(instance_id)?
            .ok_or(StorageError::AgentInstanceNotFound(instance_id))?;
        let previous = instance.state;
        if !instance_transition_allowed(previous, next) {
            return Err(StorageError::Conflict(format!(
                "illegal agent instance transition from {previous:?} to {next:?}"
            )));
        }
        if confidence.basis_points > 10_000 {
            return Err(StorageError::InvalidData(
                "agent confidence exceeds 10000 basis points".into(),
            ));
        }
        instance.state = next;
        instance.confidence = confidence;
        instance.updated_at = at;
        let event = EventEnvelope::new(
            EventKind::AgentInstanceStateChanged,
            Some(instance.task_id),
            serde_json::json!({"from": previous, "to": next, "instance": instance}),
        );
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE agent_instances SET state = ?1, instance_json = ?2, updated_at = ?3
             WHERE instance_id = ?4 AND state = ?5",
            params![
                instance_state_name(next),
                serde_json::to_string(&instance)?,
                at.to_rfc3339(),
                instance_id.to_string(),
                instance_state_name(previous),
            ],
        )?;
        ensure_single_change(changed, "agent instance changed concurrently")?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok((instance, event))
    }

    /// Persists an active bounded lease for selected work.
    ///
    /// # Errors
    ///
    /// The instance and assigned work item must exist and agree on role.
    pub fn record_agent_lease(
        &mut self,
        lease: &AgentLease,
    ) -> Result<EventEnvelope, StorageError> {
        let instance = self
            .agent_instance(lease.instance_id)?
            .ok_or(StorageError::AgentInstanceNotFound(lease.instance_id))?;
        let plan = self
            .work_plan(instance.run_id)?
            .ok_or(StorageError::InvalidData(
                "agent run has no work plan".into(),
            ))?;
        let item = plan
            .items
            .iter()
            .find(|item| item.id == lease.work_item_id)
            .ok_or_else(|| {
                StorageError::InvalidData("lease references unknown work item".into())
            })?;
        if lease.schema_version != AGENT_RUN_SCHEMA_VERSION
            || lease.state != AgentLeaseState::Active
            || item.assigned_role != instance.role_id
            || lease.expires_at <= lease.acquired_at
            || lease.heartbeat_at < lease.acquired_at
            || lease.reporting_interval_seconds == 0
            || instance.state != AgentState::Running
            || !budget_values_valid(&lease.budget.limit)
            || !lease.budget.consumed_cost_usd.is_finite()
            || lease.budget.consumed_cost_usd < 0.0
            || lease.budget.is_exhausted()
        {
            return Err(StorageError::InvalidData(
                "agent lease is inconsistent with its instance, work, or bounds".into(),
            ));
        }
        let event = EventEnvelope::new(
            EventKind::AgentLeaseRecorded,
            Some(instance.task_id),
            serde_json::to_value(lease)?,
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO agent_leases(
                lease_id, instance_id, work_item_id, state, lease_json,
                acquired_at, expires_at, heartbeat_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                lease.id.to_string(),
                lease.instance_id.to_string(),
                lease.work_item_id.to_string(),
                lease_state_name(lease.state),
                serde_json::to_string(lease)?,
                lease.acquired_at.to_rfc3339(),
                lease.expires_at.to_rfc3339(),
                lease.heartbeat_at.to_rfc3339(),
            ],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    /// Atomically creates fresh running instance authority and its active work lease.
    /// Dependency completion and organization concurrency limits are checked before
    /// any projection or event is written.
    ///
    /// # Errors
    ///
    /// Returns an error for stale run state, incomplete dependencies, duplicate
    /// work, excessive concurrency, inconsistent authority, or failed persistence.
    #[allow(clippy::too_many_lines)] // Keep all authority validation and atomic writes visible together.
    pub fn dispatch_agent_work(
        &mut self,
        created: &AgentInstance,
        lease: &AgentLease,
        at: DateTime<Utc>,
    ) -> Result<(AgentInstance, Vec<EventEnvelope>), StorageError> {
        let run = self
            .agent_run(created.run_id)?
            .ok_or(StorageError::AgentRunNotFound(created.run_id))?;
        if run.state != AgentRunState::Running {
            return Err(StorageError::Conflict(
                "agent work can only dispatch from a running run".into(),
            ));
        }
        let plan = self
            .work_plan(created.run_id)?
            .ok_or_else(|| StorageError::InvalidData("agent run has no work plan".into()))?;
        let organization = self
            .organization_revision(run.organization_revision_id)?
            .ok_or(StorageError::OrganizationRevisionNotFound(
                run.organization_revision_id,
            ))?;
        let item = plan
            .items
            .iter()
            .find(|item| item.id == lease.work_item_id)
            .ok_or_else(|| {
                StorageError::InvalidData("lease references unknown work item".into())
            })?;
        let total_instances: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM agent_instances WHERE run_id = ?1",
            [created.run_id.to_string()],
            |row| row.get(0),
        )?;
        let active_instances: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM agent_instances
             WHERE run_id = ?1 AND state IN ('created', 'running', 'waiting', 'blocked')",
            [created.run_id.to_string()],
            |row| row.get(0),
        )?;
        if total_instances >= i64::from(organization.policy.max_total_agents)
            || active_instances >= i64::from(organization.policy.max_parallel_agents)
        {
            return Err(StorageError::Conflict(
                "organization agent concurrency limit is exhausted".into(),
            ));
        }
        let prior_authority: bool = self.connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM agent_leases
                INNER JOIN agent_instances
                    ON agent_instances.instance_id = agent_leases.instance_id
                WHERE agent_instances.run_id = ?1 AND agent_leases.work_item_id = ?2
                  AND agent_leases.state IN ('active', 'completed')
             )",
            params![created.run_id.to_string(), lease.work_item_id.to_string()],
            |row| row.get(0),
        )?;
        if prior_authority {
            return Err(StorageError::Conflict(
                "work item is already active or complete".into(),
            ));
        }
        for dependency in &item.dependencies {
            let completed: bool = self.connection.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM agent_leases
                    INNER JOIN agent_instances
                        ON agent_instances.instance_id = agent_leases.instance_id
                    INNER JOIN agent_turn_results
                        ON agent_turn_results.lease_id = agent_leases.lease_id
                    WHERE agent_instances.run_id = ?1
                      AND agent_leases.work_item_id = ?2
                      AND agent_leases.state = 'completed'
                      AND agent_instances.state = 'completed'
                 )",
                params![created.run_id.to_string(), dependency.to_string()],
                |row| row.get(0),
            )?;
            if !completed {
                return Err(StorageError::Conflict(format!(
                    "work item dependency {dependency} is not complete"
                )));
            }
        }
        let lease_seconds = i64::try_from(organization.policy.default_lease_seconds)
            .map_err(|_| StorageError::InvalidData("default agent lease is too long".into()))?;
        let maximum_expiry = at
            .checked_add_signed(chrono::Duration::seconds(lease_seconds))
            .ok_or_else(|| {
                StorageError::InvalidData("default agent lease overflows time".into())
            })?;
        if created.schema_version != AGENT_RUN_SCHEMA_VERSION
            || created.state != AgentState::Created
            || created.task_id != run.task_id
            || created.role_id != item.assigned_role
            || created.context_fingerprint.trim().is_empty()
            || created.confidence.basis_points > 10_000
            || lease.schema_version != AGENT_RUN_SCHEMA_VERSION
            || lease.instance_id != created.id
            || lease.state != AgentLeaseState::Active
            || lease.goal != item.goal
            || lease.budget.limit != item.budget
            || lease.expires_at <= at
            || lease.expires_at > maximum_expiry
            || lease.acquired_at != at
            || lease.heartbeat_at != at
            || lease.reporting_interval_seconds == 0
            || lease.max_retries > 10
            || !budget_values_valid(&lease.budget.limit)
            || lease.budget.consumed_tokens != 0
            || lease.budget.consumed_cost_usd != 0.0
            || lease.budget.elapsed_seconds != 0
        {
            return Err(StorageError::InvalidData(
                "dispatched agent authority is inconsistent with its run and work item".into(),
            ));
        }
        let mut running = created.clone();
        running.state = AgentState::Running;
        running.updated_at = at;
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO agent_instances(
                instance_id, run_id, task_id, role_id, state, instance_json,
                created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, 'running', ?5, ?6, ?7)",
            params![
                running.id.to_string(),
                running.run_id.to_string(),
                running.task_id.to_string(),
                running.role_id.to_string(),
                serde_json::to_string(&running)?,
                running.created_at.to_rfc3339(),
                running.updated_at.to_rfc3339(),
            ],
        )?;
        transaction.execute(
            "INSERT INTO agent_leases(
                lease_id, instance_id, work_item_id, state, lease_json,
                acquired_at, expires_at, heartbeat_at
             ) VALUES (?1, ?2, ?3, 'active', ?4, ?5, ?6, ?7)",
            params![
                lease.id.to_string(),
                lease.instance_id.to_string(),
                lease.work_item_id.to_string(),
                serde_json::to_string(lease)?,
                lease.acquired_at.to_rfc3339(),
                lease.expires_at.to_rfc3339(),
                lease.heartbeat_at.to_rfc3339(),
            ],
        )?;
        let events = vec![
            Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentInstanceRecorded,
                    Some(created.task_id),
                    serde_json::to_value(created)?,
                ),
            )?,
            Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentInstanceStateChanged,
                    Some(created.task_id),
                    serde_json::json!({"from": AgentState::Created, "to": AgentState::Running, "instance": &running}),
                ),
            )?,
            Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentLeaseRecorded,
                    Some(created.task_id),
                    serde_json::to_value(lease)?,
                ),
            )?,
        ];
        transaction.commit()?;
        Ok((running, events))
    }

    /// Loads a current agent lease projection.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid persisted JSON.
    pub fn agent_lease(&self, lease_id: AgentLeaseId) -> Result<Option<AgentLease>, StorageError> {
        self.read_optional_json(
            "SELECT lease_json FROM agent_leases WHERE lease_id = ?1",
            &lease_id.to_string(),
        )
    }

    /// Lists current leases for every instance in one run.
    ///
    /// # Errors
    ///
    /// Returns an error when a row cannot be decoded.
    pub fn agent_leases_for_run(&self, run_id: RunId) -> Result<Vec<AgentLease>, StorageError> {
        self.read_json_rows(
            "SELECT agent_leases.lease_json FROM agent_leases
             INNER JOIN agent_instances
                ON agent_instances.instance_id = agent_leases.instance_id
             WHERE agent_instances.run_id = ?1
             ORDER BY agent_leases.acquired_at, agent_leases.lease_id",
            &run_id.to_string(),
        )
    }

    /// Finishes an active lease with its final measured budget consumption.
    ///
    /// # Errors
    ///
    /// Only an active lease may transition and consumed values must be finite.
    pub fn finish_agent_lease(
        &mut self,
        lease_id: AgentLeaseId,
        next: AgentLeaseState,
        consumed_tokens: u64,
        consumed_cost_usd: f64,
        elapsed_seconds: u64,
        at: DateTime<Utc>,
    ) -> Result<(AgentLease, EventEnvelope), StorageError> {
        let mut lease = self
            .agent_lease(lease_id)?
            .ok_or(StorageError::AgentLeaseNotFound(lease_id))?;
        if lease.state != AgentLeaseState::Active
            || next == AgentLeaseState::Active
            || !consumed_cost_usd.is_finite()
            || consumed_cost_usd < 0.0
            || at < lease.acquired_at
        {
            return Err(StorageError::Conflict(
                "agent lease cannot make the requested terminal transition".into(),
            ));
        }
        let instance = self
            .agent_instance(lease.instance_id)?
            .ok_or(StorageError::AgentInstanceNotFound(lease.instance_id))?;
        lease.state = next;
        lease.budget.consumed_tokens = consumed_tokens;
        lease.budget.consumed_cost_usd = consumed_cost_usd;
        lease.budget.elapsed_seconds = elapsed_seconds;
        lease.heartbeat_at = at;
        let event = EventEnvelope::new(
            EventKind::AgentLeaseStateChanged,
            Some(instance.task_id),
            serde_json::to_value(&lease)?,
        );
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE agent_leases SET state = ?1, lease_json = ?2, heartbeat_at = ?3
             WHERE lease_id = ?4 AND state = 'active'",
            params![
                lease_state_name(next),
                serde_json::to_string(&lease)?,
                at.to_rfc3339(),
                lease_id.to_string(),
            ],
        )?;
        ensure_single_change(changed, "agent lease changed concurrently")?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok((lease, event))
    }

    /// Atomically commits a completed model/tool turn, attributed usage, current
    /// run metrics, and terminal instance/lease projections.
    ///
    /// # Errors
    ///
    /// The run, instance, and lease must be active and mutually consistent. Any
    /// invalid evidence or failed write rolls back the complete turn commit.
    #[allow(clippy::too_many_lines)] // Transaction order is the crash-safety invariant.
    pub fn commit_agent_turn(
        &mut self,
        instance_id: AgentInstanceId,
        lease_id: AgentLeaseId,
        result: &AgentTurnResult,
        confidence: ConfidenceEstimate,
        at: DateTime<Utc>,
    ) -> Result<AgentTurnCommit, StorageError> {
        let mut instance = self
            .agent_instance(instance_id)?
            .ok_or(StorageError::AgentInstanceNotFound(instance_id))?;
        let mut lease = self
            .agent_lease(lease_id)?
            .ok_or(StorageError::AgentLeaseNotFound(lease_id))?;
        let mut run = self
            .agent_run(instance.run_id)?
            .ok_or(StorageError::AgentRunNotFound(instance.run_id))?;
        if instance.state != AgentState::Running
            || lease.state != AgentLeaseState::Active
            || lease.instance_id != instance.id
            || run.state != AgentRunState::Running
            || run.task_id != instance.task_id
            || confidence.basis_points > 10_000
            || at < lease.acquired_at
            || result.model_calls.is_empty()
            || result.response.id.trim().is_empty()
            || result.response.model != result.pinned_route.target.model_id
            || result
                .model_calls
                .iter()
                .any(|call| call.target != result.pinned_route.target)
        {
            return Err(StorageError::InvalidData(
                "completed agent turn authority, identity, or evidence is inconsistent".into(),
            ));
        }

        let mut usage = Vec::new();
        for call in &result.model_calls {
            usage.push(AgentUsageRecord {
                schema_version: AGENT_RUN_SCHEMA_VERSION,
                id: mcg_domain::UsageRecordId::new(),
                run_id: run.id,
                task_id: run.task_id,
                instance_id: Some(instance.id),
                kind: UsageKind::ProviderModel,
                provider_connection_id: Some(call.target.provider_id.clone()),
                model_id: Some(call.target.model_id.clone()),
                tool_execution_id: None,
                input_tokens: call.usage.input_tokens,
                cached_input_tokens: call.usage.cached_input_tokens,
                output_tokens: call.usage.output_tokens,
                cost_usd: call.cost_usd,
                duration_ms: call.duration_ms,
                occurred_at: at,
            });
        }
        for outcome in &result.tool_outcomes {
            let duration_ms = u64::try_from(
                (outcome.finished_at - outcome.started_at)
                    .num_milliseconds()
                    .max(0),
            )
            .unwrap_or(u64::MAX);
            usage.push(AgentUsageRecord {
                schema_version: AGENT_RUN_SCHEMA_VERSION,
                id: mcg_domain::UsageRecordId::new(),
                run_id: run.id,
                task_id: run.task_id,
                instance_id: Some(instance.id),
                kind: UsageKind::Tool,
                provider_connection_id: None,
                model_id: None,
                tool_execution_id: Some(outcome.execution_id),
                input_tokens: 0,
                cached_input_tokens: 0,
                output_tokens: 0,
                cost_usd: None,
                duration_ms,
                occurred_at: outcome.finished_at,
            });
        }
        if usage.iter().any(|record| {
            record.cached_input_tokens > record.input_tokens
                || record
                    .cost_usd
                    .is_some_and(|cost| !cost.is_finite() || cost < 0.0)
        }) {
            return Err(StorageError::InvalidData(
                "completed agent turn contains invalid usage values".into(),
            ));
        }

        let turn_tokens = result
            .model_calls
            .iter()
            .map(|call| call.usage.total_tokens)
            .sum::<u64>();
        let turn_cost = result
            .model_calls
            .iter()
            .filter_map(|call| call.cost_usd)
            .sum::<f64>();
        let elapsed_seconds =
            u64::try_from((at - lease.acquired_at).num_seconds().max(0)).unwrap_or(u64::MAX);
        lease.state = AgentLeaseState::Completed;
        lease.budget.consumed_tokens = lease.budget.consumed_tokens.saturating_add(turn_tokens);
        lease.budget.consumed_cost_usd += turn_cost;
        lease.budget.elapsed_seconds = lease.budget.elapsed_seconds.max(elapsed_seconds);
        lease.heartbeat_at = at;
        instance.state = AgentState::Completed;
        instance.model = Some(AssignedModel {
            provider_connection_id: result.pinned_route.target.provider_id.clone(),
            model_id: result.pinned_route.target.model_id.clone(),
            route_reason: "pinned by the bounded route plan".into(),
        });
        instance.confidence = confidence;
        instance.updated_at = at;
        run.metrics.model_calls = run
            .metrics
            .model_calls
            .saturating_add(u32::try_from(result.model_calls.len()).unwrap_or(u32::MAX));
        run.metrics.tool_calls = run
            .metrics
            .tool_calls
            .saturating_add(u32::try_from(result.tool_outcomes.len()).unwrap_or(u32::MAX));
        run.metrics.tokens = run.metrics.tokens.saturating_add(turn_tokens);
        run.metrics.cost_usd += turn_cost;
        run.metrics.useful_work_seconds = run
            .metrics
            .useful_work_seconds
            .saturating_add(elapsed_seconds);
        run.updated_at = at;
        if !metrics_valid(&run.metrics) || !lease.budget.consumed_cost_usd.is_finite() {
            return Err(StorageError::InvalidData(
                "completed agent turn exceeds numeric storage bounds".into(),
            ));
        }

        let transaction = self.connection.transaction()?;
        let mut events = Vec::new();
        for record in &usage {
            insert_agent_usage(&transaction, record)?;
            events.push(Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentUsageRecorded,
                    Some(run.task_id),
                    serde_json::to_value(record)?,
                ),
            )?);
        }
        transaction.execute(
            "INSERT INTO agent_turn_results(
                lease_id, run_id, instance_id, task_id, result_json, completed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                lease.id.to_string(),
                run.id.to_string(),
                instance.id.to_string(),
                run.task_id.to_string(),
                serde_json::to_string(result)?,
                at.to_rfc3339(),
            ],
        )?;
        let changed = transaction.execute(
            "UPDATE agent_leases SET state = 'completed', lease_json = ?1, heartbeat_at = ?2
             WHERE lease_id = ?3 AND state = 'active'",
            params![
                serde_json::to_string(&lease)?,
                at.to_rfc3339(),
                lease.id.to_string(),
            ],
        )?;
        ensure_single_change(changed, "agent lease changed before turn commit")?;
        let changed = transaction.execute(
            "UPDATE agent_instances SET state = 'completed', instance_json = ?1, updated_at = ?2
             WHERE instance_id = ?3 AND state = 'running'",
            params![
                serde_json::to_string(&instance)?,
                at.to_rfc3339(),
                instance.id.to_string(),
            ],
        )?;
        ensure_single_change(changed, "agent instance changed before turn commit")?;
        let changed = transaction.execute(
            "UPDATE agent_runs SET run_json = ?1, updated_at = ?2
             WHERE run_id = ?3 AND state = 'running'",
            params![
                serde_json::to_string(&run)?,
                at.to_rfc3339(),
                run.id.to_string(),
            ],
        )?;
        ensure_single_change(changed, "agent run changed before turn commit")?;
        for (kind, payload) in [
            (
                EventKind::AgentLeaseStateChanged,
                serde_json::to_value(&lease)?,
            ),
            (
                EventKind::AgentInstanceStateChanged,
                serde_json::to_value(&instance)?,
            ),
            (
                EventKind::AgentRunMetricsUpdated,
                serde_json::to_value(&run)?,
            ),
            (EventKind::AgentTurnCompleted, serde_json::to_value(result)?),
        ] {
            events.push(Self::insert_event(
                &transaction,
                &EventEnvelope::new(kind, Some(run.task_id), payload),
            )?);
        }
        transaction.commit()?;
        Ok(AgentTurnCommit {
            run,
            instance,
            lease,
            usage,
            result: result.clone(),
            events,
        })
    }

    /// Loads the immutable completed turn result for an agent lease.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid persisted JSON.
    pub fn agent_turn_result(
        &self,
        lease_id: AgentLeaseId,
    ) -> Result<Option<AgentTurnResult>, StorageError> {
        self.read_optional_json(
            "SELECT result_json FROM agent_turn_results WHERE lease_id = ?1",
            &lease_id.to_string(),
        )
    }

    /// Atomically settles a turn that could not complete, without inventing
    /// provider/tool usage or a successful result.
    ///
    /// # Errors
    ///
    /// The instance and lease must still hold active authority. Waiting retains
    /// the lease for explicit approval resume; failed work terminates it.
    #[allow(clippy::too_many_lines)] // State and evidence writes form one visible transaction.
    pub fn settle_agent_turn_error(
        &mut self,
        instance_id: AgentInstanceId,
        lease_id: AgentLeaseId,
        disposition: AgentTurnDisposition,
        reason_code: &str,
        at: DateTime<Utc>,
    ) -> Result<Vec<EventEnvelope>, StorageError> {
        if reason_code.trim().is_empty() || reason_code.len() > 100 {
            return Err(StorageError::InvalidData(
                "agent turn failure reason code is invalid".into(),
            ));
        }
        let mut instance = self
            .agent_instance(instance_id)?
            .ok_or(StorageError::AgentInstanceNotFound(instance_id))?;
        let mut lease = self
            .agent_lease(lease_id)?
            .ok_or(StorageError::AgentLeaseNotFound(lease_id))?;
        let mut run = self
            .agent_run(instance.run_id)?
            .ok_or(StorageError::AgentRunNotFound(instance.run_id))?;
        if instance.state != AgentState::Running
            || lease.state != AgentLeaseState::Active
            || lease.instance_id != instance.id
            || run.state != AgentRunState::Running
            || at < lease.acquired_at
        {
            return Err(StorageError::Conflict(
                "agent turn authority changed before error settlement".into(),
            ));
        }
        let (instance_state, lease_state, run_state, event_kind) = match disposition {
            AgentTurnDisposition::Waiting => (
                AgentState::Waiting,
                AgentLeaseState::Active,
                AgentRunState::Waiting,
                EventKind::AgentTurnWaiting,
            ),
            AgentTurnDisposition::Failed => (
                AgentState::Failed,
                AgentLeaseState::Failed,
                AgentRunState::Running,
                EventKind::AgentTurnFailed,
            ),
        };
        instance.state = instance_state;
        instance.updated_at = at;
        lease.state = lease_state;
        lease.heartbeat_at = at;
        run.state = run_state;
        run.updated_at = at;
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE agent_instances SET state = ?1, instance_json = ?2, updated_at = ?3
             WHERE instance_id = ?4 AND state = 'running'",
            params![
                instance_state_name(instance.state),
                serde_json::to_string(&instance)?,
                at.to_rfc3339(),
                instance.id.to_string(),
            ],
        )?;
        ensure_single_change(changed, "agent instance changed before error settlement")?;
        let changed = transaction.execute(
            "UPDATE agent_leases SET state = ?1, lease_json = ?2, heartbeat_at = ?3
             WHERE lease_id = ?4 AND state = 'active'",
            params![
                lease_state_name(lease.state),
                serde_json::to_string(&lease)?,
                at.to_rfc3339(),
                lease.id.to_string(),
            ],
        )?;
        ensure_single_change(changed, "agent lease changed before error settlement")?;
        let changed = transaction.execute(
            "UPDATE agent_runs SET state = ?1, run_json = ?2, updated_at = ?3
             WHERE run_id = ?4 AND state = 'running'",
            params![
                run_state_name(run.state),
                serde_json::to_string(&run)?,
                at.to_rfc3339(),
                run.id.to_string(),
            ],
        )?;
        ensure_single_change(changed, "agent run changed before error settlement")?;
        let mut events = Vec::new();
        events.push(Self::insert_event(
            &transaction,
            &EventEnvelope::new(
                EventKind::AgentInstanceStateChanged,
                Some(run.task_id),
                serde_json::to_value(&instance)?,
            ),
        )?);
        if disposition == AgentTurnDisposition::Failed {
            events.push(Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentLeaseStateChanged,
                    Some(run.task_id),
                    serde_json::to_value(&lease)?,
                ),
            )?);
        } else {
            events.push(Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentRunStateChanged,
                    Some(run.task_id),
                    serde_json::to_value(&run)?,
                ),
            )?);
        }
        events.push(Self::insert_event(
            &transaction,
            &EventEnvelope::new(
                event_kind,
                Some(run.task_id),
                serde_json::json!({
                    "run_id": run.id,
                    "instance_id": instance.id,
                    "lease_id": lease.id,
                    "reason_code": reason_code,
                }),
            ),
        )?);
        transaction.commit()?;
        Ok(events)
    }

    /// Records immutable provider, tool, or coordination usage evidence.
    ///
    /// # Errors
    ///
    /// Usage identity, kind-specific attribution, and numeric values are validated.
    pub fn record_agent_usage(
        &mut self,
        usage: &AgentUsageRecord,
    ) -> Result<EventEnvelope, StorageError> {
        let run = self
            .agent_run(usage.run_id)?
            .ok_or(StorageError::AgentRunNotFound(usage.run_id))?;
        let attribution_valid = match usage.kind {
            UsageKind::ProviderModel => {
                usage.provider_connection_id.is_some()
                    && usage.model_id.is_some()
                    && usage.tool_execution_id.is_none()
            }
            UsageKind::Tool => {
                usage.tool_execution_id.is_some()
                    && usage.provider_connection_id.is_none()
                    && usage.model_id.is_none()
            }
            UsageKind::Coordination => {
                usage.provider_connection_id.is_none()
                    && usage.model_id.is_none()
                    && usage.tool_execution_id.is_none()
            }
        };
        if usage.schema_version != AGENT_RUN_SCHEMA_VERSION
            || usage.task_id != run.task_id
            || !attribution_valid
            || usage.cached_input_tokens > usage.input_tokens
            || usage
                .cost_usd
                .is_some_and(|cost| !cost.is_finite() || cost < 0.0)
        {
            return Err(StorageError::InvalidData(
                "agent usage attribution or numeric values are invalid".into(),
            ));
        }
        if let Some(instance_id) = usage.instance_id {
            let instance = self
                .agent_instance(instance_id)?
                .ok_or(StorageError::AgentInstanceNotFound(instance_id))?;
            if instance.run_id != usage.run_id {
                return Err(StorageError::InvalidData(
                    "agent usage instance belongs to a different run".into(),
                ));
            }
        }
        let event = EventEnvelope::new(
            EventKind::AgentUsageRecorded,
            Some(usage.task_id),
            serde_json::to_value(usage)?,
        );
        let transaction = self.connection.transaction()?;
        insert_agent_usage(&transaction, usage)?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    /// Lists immutable usage evidence for a run.
    ///
    /// # Errors
    ///
    /// Returns an error when a row cannot be decoded.
    pub fn agent_usage(&self, run_id: RunId) -> Result<Vec<AgentUsageRecord>, StorageError> {
        self.read_json_rows(
            "SELECT usage_json FROM agent_usage WHERE run_id = ?1
             ORDER BY occurred_at, usage_id",
            &run_id.to_string(),
        )
    }

    /// Records an immutable evidence-backed evaluation report.
    ///
    /// # Errors
    ///
    /// The report must identify its run/task and satisfy the genome evidence policy.
    pub fn record_evaluation(
        &mut self,
        report: &EvaluationReport,
    ) -> Result<EventEnvelope, StorageError> {
        let run = self
            .agent_run(report.run_id)?
            .ok_or(StorageError::AgentRunNotFound(report.run_id))?;
        let genome = self
            .organization_revision(run.organization_revision_id)?
            .ok_or(StorageError::OrganizationRevisionNotFound(
                run.organization_revision_id,
            ))?;
        let plan = self
            .work_plan(report.run_id)?
            .ok_or(StorageError::InvalidData(
                "evaluated agent run has no work plan".into(),
            ))?;
        let covers_plan = plan.items.iter().all(|item| {
            item.success_conditions.iter().all(|required| {
                report
                    .conditions
                    .iter()
                    .any(|result| result.condition == *required)
            })
        });
        let outcome_consistent = match report.outcome {
            mcg_agents::EvaluationOutcome::Passed => {
                report.conditions.iter().all(|condition| condition.passed)
            }
            mcg_agents::EvaluationOutcome::Failed => {
                report.conditions.iter().any(|condition| !condition.passed)
            }
            mcg_agents::EvaluationOutcome::Inconclusive => true,
        };
        let plan_requires_independent_evaluation = genome.evaluation.require_independent_evaluation
            || plan.items.iter().any(|item| {
                genome.roles.iter().any(|role| {
                    role.id == item.assigned_role && role.kind == mcg_agents::RoleKind::Evaluator
                })
            });
        let independent_evaluator_present =
            !plan_requires_independent_evaluation || report.evaluator_instance_id.is_some();
        if report.schema_version != AGENT_RUN_SCHEMA_VERSION
            || report.task_id != run.task_id
            || report.conditions.is_empty()
            || (genome.evaluation.require_objective_evidence && report.evidence.is_empty())
            || !independent_evaluator_present
            || report.confidence.basis_points > 10_000
            || !covers_plan
            || !outcome_consistent
        {
            return Err(StorageError::InvalidData(
                "evaluation does not satisfy its run or objective evidence policy".into(),
            ));
        }
        if let Some(instance_id) = report.evaluator_instance_id {
            let instance = self
                .agent_instance(instance_id)?
                .ok_or(StorageError::AgentInstanceNotFound(instance_id))?;
            if instance.run_id != report.run_id {
                return Err(StorageError::InvalidData(
                    "evaluation instance belongs to a different run".into(),
                ));
            }
            let evaluator = genome.roles.iter().any(|role| {
                role.id == instance.role_id && role.kind == mcg_agents::RoleKind::Evaluator
            });
            if !evaluator {
                return Err(StorageError::InvalidData(
                    "evaluation instance does not have the Evaluator role".into(),
                ));
            }
        }
        let event = EventEnvelope::new(
            EventKind::EvaluationRecorded,
            Some(report.task_id),
            serde_json::to_value(report)?,
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO evaluations(
                evaluation_id, run_id, task_id, evaluation_json, evaluated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                report.id.to_string(),
                report.run_id.to_string(),
                report.task_id.to_string(),
                serde_json::to_string(report)?,
                report.evaluated_at.to_rfc3339(),
            ],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    /// Lists immutable evaluation reports for a run.
    ///
    /// # Errors
    ///
    /// Returns an error when a row cannot be decoded.
    pub fn evaluations(&self, run_id: RunId) -> Result<Vec<EvaluationReport>, StorageError> {
        self.read_json_rows(
            "SELECT evaluation_json FROM evaluations WHERE run_id = ?1
             ORDER BY evaluated_at, evaluation_id",
            &run_id.to_string(),
        )
    }

    /// Converts interrupted run work into explicit recovering/expired states and
    /// appends an event for every recovered projection update.
    ///
    /// # Errors
    ///
    /// Recovery is atomic and fails without partial state changes.
    pub fn recover_agent_work(
        &mut self,
        at: DateTime<Utc>,
    ) -> Result<AgentRecoveryReport, StorageError> {
        let mut runs = self.read_all_json::<AgentRun>(
            "SELECT run_json FROM agent_runs WHERE state IN ('running', 'waiting') ORDER BY run_id",
        )?;
        let mut instances = self.read_all_json::<AgentInstance>(
            "SELECT instance_json FROM agent_instances
             WHERE state IN ('running', 'waiting', 'blocked') ORDER BY instance_id",
        )?;
        let mut leases = self.read_all_json::<AgentLease>(
            "SELECT lease_json FROM agent_leases WHERE state = 'active' ORDER BY lease_id",
        )?;
        let lease_instance_tasks = self
            .read_all_json::<AgentInstance>(
                "SELECT DISTINCT agent_instances.instance_json FROM agent_instances
                 INNER JOIN agent_leases ON agent_leases.instance_id = agent_instances.instance_id
                 WHERE agent_leases.state = 'active' ORDER BY agent_instances.instance_id",
            )?
            .into_iter()
            .map(|instance| (instance.id, instance.task_id))
            .collect::<std::collections::BTreeMap<_, _>>();
        let transaction = self.connection.transaction()?;
        for run in &mut runs {
            let previous = run.state;
            run.state = AgentRunState::Recovering;
            run.updated_at = at;
            transaction.execute(
                "UPDATE agent_runs SET state = 'recovering', run_json = ?1, updated_at = ?2
                 WHERE run_id = ?3 AND state = ?4",
                params![
                    serde_json::to_string(run)?,
                    at.to_rfc3339(),
                    run.id.to_string(),
                    run_state_name(previous),
                ],
            )?;
            Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentRunStateChanged,
                    Some(run.task_id),
                    serde_json::json!({"from": previous, "to": AgentRunState::Recovering, "run": run, "reason": "daemon_restart"}),
                ),
            )?;
        }
        for instance in &mut instances {
            let previous = instance.state;
            instance.state = AgentState::Expired;
            instance.updated_at = at;
            transaction.execute(
                "UPDATE agent_instances SET state = 'expired', instance_json = ?1, updated_at = ?2
                 WHERE instance_id = ?3 AND state = ?4",
                params![
                    serde_json::to_string(instance)?,
                    at.to_rfc3339(),
                    instance.id.to_string(),
                    instance_state_name(previous),
                ],
            )?;
            Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentInstanceStateChanged,
                    Some(instance.task_id),
                    serde_json::json!({"from": previous, "to": AgentState::Expired, "instance": instance, "reason": "daemon_restart"}),
                ),
            )?;
        }
        for lease in &mut leases {
            lease.state = AgentLeaseState::Expired;
            lease.heartbeat_at = at;
            transaction.execute(
                "UPDATE agent_leases SET state = 'expired', lease_json = ?1, heartbeat_at = ?2
                 WHERE lease_id = ?3 AND state = 'active'",
                params![
                    serde_json::to_string(lease)?,
                    at.to_rfc3339(),
                    lease.id.to_string(),
                ],
            )?;
            let task_id = lease_instance_tasks.get(&lease.instance_id).copied();
            Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentLeaseStateChanged,
                    task_id,
                    serde_json::json!({"lease": lease, "reason": "daemon_restart"}),
                ),
            )?;
        }
        transaction.commit()?;
        Ok(AgentRecoveryReport {
            runs_marked_recovering: runs.len() as u64,
            instances_expired: instances.len() as u64,
            leases_expired: leases.len() as u64,
        })
    }

    pub(crate) fn read_optional_json<T: serde::de::DeserializeOwned>(
        &self,
        sql: &str,
        parameter: &str,
    ) -> Result<Option<T>, StorageError> {
        let json = self
            .connection
            .query_row(sql, [parameter], |row| row.get::<_, String>(0))
            .optional()?;
        json.map(|value| serde_json::from_str(&value).map_err(StorageError::from))
            .transpose()
    }

    fn read_json_rows<T: serde::de::DeserializeOwned>(
        &self,
        sql: &str,
        parameter: &str,
    ) -> Result<Vec<T>, StorageError> {
        let mut statement = self.connection.prepare(sql)?;
        let rows = statement
            .query_map([parameter], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|value| serde_json::from_str(&value).map_err(StorageError::from))
            .collect()
    }

    pub(crate) fn read_all_json<T: serde::de::DeserializeOwned>(
        &self,
        sql: &str,
    ) -> Result<Vec<T>, StorageError> {
        let mut statement = self.connection.prepare(sql)?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|value| serde_json::from_str(&value).map_err(StorageError::from))
            .collect()
    }
}

fn insert_agent_usage(
    transaction: &Transaction<'_>,
    usage: &AgentUsageRecord,
) -> Result<(), StorageError> {
    transaction.execute(
        "INSERT INTO agent_usage(usage_id, run_id, task_id, usage_json, occurred_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            usage.id.to_string(),
            usage.run_id.to_string(),
            usage.task_id.to_string(),
            serde_json::to_string(usage)?,
            usage.occurred_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

fn budgets_within_parent(plan: &WorkPlan, parent: &mcg_domain::TaskBudget) -> bool {
    if !budget_values_valid(parent)
        || plan
            .items
            .iter()
            .any(|item| !budget_values_valid(&item.budget))
    {
        return false;
    }
    let tokens = parent.max_tokens.is_none_or(|limit| {
        plan.items
            .iter()
            .try_fold(0_u64, |sum, item| sum.checked_add(item.budget.max_tokens?))
            .is_some_and(|sum| sum <= limit)
    });
    let duration = parent.max_duration_seconds.is_none_or(|limit| {
        plan.items
            .iter()
            .try_fold(0_u64, |sum, item| {
                sum.checked_add(item.budget.max_duration_seconds?)
            })
            .is_some_and(|sum| sum <= limit)
    });
    let cost = parent.max_cost_usd.is_none_or(|limit| {
        let values = plan
            .items
            .iter()
            .map(|item| item.budget.max_cost_usd)
            .collect::<Option<Vec<_>>>();
        values.is_some_and(|values| values.into_iter().sum::<f64>() <= limit + f64::EPSILON)
    });
    tokens && duration && cost
}

fn budget_values_valid(budget: &mcg_domain::TaskBudget) -> bool {
    budget
        .max_cost_usd
        .is_none_or(|cost| cost.is_finite() && cost >= 0.0)
}

fn metrics_valid(metrics: &OrganizationalMetrics) -> bool {
    metrics.cost_usd.is_finite() && metrics.cost_usd >= 0.0
}

fn ensure_single_change(changed: usize, detail: &str) -> Result<(), StorageError> {
    if changed == 1 {
        Ok(())
    } else {
        Err(StorageError::Conflict(detail.into()))
    }
}

const fn run_state_name(state: AgentRunState) -> &'static str {
    match state {
        AgentRunState::Planned => "planned",
        AgentRunState::Running => "running",
        AgentRunState::Waiting => "waiting",
        AgentRunState::Recovering => "recovering",
        AgentRunState::Completed => "completed",
        AgentRunState::Failed => "failed",
        AgentRunState::Cancelled => "cancelled",
    }
}

const fn instance_state_name(state: AgentState) -> &'static str {
    match state {
        AgentState::Created => "created",
        AgentState::Running => "running",
        AgentState::Waiting => "waiting",
        AgentState::Blocked => "blocked",
        AgentState::Completed => "completed",
        AgentState::Failed => "failed",
        AgentState::Cancelled => "cancelled",
        AgentState::Expired => "expired",
    }
}

const fn lease_state_name(state: AgentLeaseState) -> &'static str {
    match state {
        AgentLeaseState::Active => "active",
        AgentLeaseState::Completed => "completed",
        AgentLeaseState::Failed => "failed",
        AgentLeaseState::Expired => "expired",
        AgentLeaseState::Reassigned => "reassigned",
        AgentLeaseState::Terminated => "terminated",
    }
}

const fn instance_transition_allowed(from: AgentState, to: AgentState) -> bool {
    matches!(
        (from, to),
        (
            AgentState::Created,
            AgentState::Running | AgentState::Cancelled
        ) | (
            AgentState::Running,
            AgentState::Waiting
                | AgentState::Blocked
                | AgentState::Completed
                | AgentState::Failed
                | AgentState::Cancelled
                | AgentState::Expired
        ) | (
            AgentState::Waiting | AgentState::Blocked,
            AgentState::Running | AgentState::Failed | AgentState::Cancelled | AgentState::Expired
        )
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use mcg_agents::{
        AGENT_RUN_SCHEMA_VERSION, AgentLease, AgentRun, AgentUsageRecord, ConditionResult,
        EvaluationEvidence, EvaluationOutcome, ExecutivePlanner, LeaseBudget, PlannedWork,
        PlanningRequest, RoleKind, SuccessCondition, TaskComplexity, UsageKind,
    };
    use mcg_domain::{
        AgentInstanceId, AgentLeaseId, EvaluationId, RunId, TaskBudget, UsageRecordId,
    };
    use tempfile::tempdir;

    #[test]
    fn interface_run_creation_is_atomic_idempotent_and_restart_durable() {
        let directory = tempdir().expect("temp directory");
        let database = directory.path().join("mcg.sqlite3");
        let now = Utc::now();
        let mut task = TaskEnvelope::new("Explain the fixture project");
        task.budget = TaskBudget {
            max_tokens: Some(4_000),
            max_cost_usd: Some(1.0),
            max_duration_seconds: Some(300),
        };
        let genome = OrganizationGenome::consumer_default(now);
        let request = PlanningRequest {
            run_id: RunId::new(),
            task: task.clone(),
            complexity: TaskComplexity::Atomic,
            work: vec![PlannedWork::atomic(
                "Explain the fixture project",
                RoleKind::Conversation,
                vec![SuccessCondition::Custom {
                    description: "return a grounded explanation".into(),
                }],
            )],
            require_independent_evaluation: false,
        };
        let plan = ExecutivePlanner::default()
            .plan(&genome, &request, now)
            .expect("plan");
        let run = AgentRun {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: request.run_id,
            task_id: task.id,
            organization_revision_id: genome.revision_id,
            state: AgentRunState::Planned,
            metrics: OrganizationalMetrics::default(),
            created_at: now,
            updated_at: now,
            completed_at: None,
        };
        let hash = "a".repeat(64);
        let mut store = SqliteEventStore::open(&database).expect("open store");
        let created = store
            .create_interface_agent_run(
                "mcp",
                "fixture-request",
                &hash,
                &task,
                &genome,
                &run,
                &plan,
            )
            .expect("create interface run");
        assert!(!created.replayed);
        assert_eq!(created.task.state, TaskState::Active);
        assert_eq!(created.run.state, AgentRunState::Running);
        assert_eq!(created.events.len(), 5);
        let event_count = store.event_count().expect("event count");
        drop(store);

        let mut reopened = SqliteEventStore::open(&database).expect("reopen store");
        let replayed = reopened
            .create_interface_agent_run(
                "mcp",
                "fixture-request",
                &hash,
                &task,
                &genome,
                &run,
                &plan,
            )
            .expect("replay interface run");
        assert!(replayed.replayed);
        assert_eq!(replayed.task.id, task.id);
        assert_eq!(replayed.run.id, run.id);
        assert!(replayed.events.is_empty());
        assert_eq!(reopened.event_count().expect("event count"), event_count);
        assert!(
            reopened
                .create_interface_agent_run(
                    "mcp",
                    "fixture-request",
                    &"b".repeat(64),
                    &task,
                    &genome,
                    &run,
                    &plan,
                )
                .is_err()
        );
        assert_eq!(reopened.event_count().expect("event count"), event_count);
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One contiguous crash/reopen scenario is intentional evidence.
    fn organization_run_evidence_and_crash_recovery_survive_reopen() {
        let directory = tempdir().expect("temp directory");
        let database = directory.path().join("mcg.sqlite3");
        let now = Utc::now();
        let mut store = SqliteEventStore::open(&database).expect("open store");
        let mut task = mcg_domain::TaskEnvelope::new("Build and evaluate the fixture site");
        task.budget = TaskBudget {
            max_tokens: Some(12_000),
            max_cost_usd: Some(5.0),
            max_duration_seconds: Some(900),
        };
        store.create_task(&task).expect("create task");

        let genome = OrganizationGenome::consumer_default(now);
        store
            .record_organization_revision(&genome)
            .expect("record organization");
        let planned = PlannedWork::atomic(
            "Implement the fixture site",
            RoleKind::Coder,
            vec![SuccessCondition::Test {
                command: "npm test".into(),
            }],
        );
        let request = PlanningRequest {
            run_id: RunId::new(),
            task: task.clone(),
            complexity: TaskComplexity::Atomic,
            work: vec![planned],
            require_independent_evaluation: false,
        };
        let plan = ExecutivePlanner::default()
            .plan(&genome, &request, now)
            .expect("plan");
        let run = AgentRun {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: request.run_id,
            task_id: task.id,
            organization_revision_id: genome.revision_id,
            state: AgentRunState::Planned,
            metrics: OrganizationalMetrics::default(),
            created_at: now,
            updated_at: now,
            completed_at: None,
        };
        store.create_agent_run(&run, &plan).expect("create run");
        store
            .transition_agent_run(
                run.id,
                AgentRunState::Running,
                OrganizationalMetrics::default(),
                now + Duration::seconds(1),
            )
            .expect("start run");

        let instance = AgentInstance {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: AgentInstanceId::new(),
            role_id: plan.items[0].assigned_role,
            run_id: run.id,
            task_id: task.id,
            state: AgentState::Created,
            model: None,
            context_fingerprint: "sha256:fixture-context".into(),
            confidence: ConfidenceEstimate::new(7_500, 0).expect("confidence"),
            created_at: now,
            updated_at: now,
        };
        let lease = AgentLease {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: AgentLeaseId::new(),
            instance_id: instance.id,
            work_item_id: plan.items[0].id,
            goal: plan.items[0].goal.clone(),
            budget: LeaseBudget {
                limit: plan.items[0].budget.clone(),
                consumed_tokens: 0,
                consumed_cost_usd: 0.0,
                elapsed_seconds: 0,
            },
            tool_permissions: vec!["filesystem.project".into()],
            reporting_interval_seconds: 30,
            max_retries: 2,
            escalation_conditions: Vec::new(),
            state: AgentLeaseState::Active,
            acquired_at: now + Duration::seconds(2),
            expires_at: now + Duration::minutes(15),
            heartbeat_at: now + Duration::seconds(2),
        };
        let (running_instance, dispatch_events) = store
            .dispatch_agent_work(&instance, &lease, now + Duration::seconds(2))
            .expect("atomic dispatch");
        assert_eq!(running_instance.state, AgentState::Running);
        assert_eq!(dispatch_events.len(), 3);
        let event_count_after_dispatch = store.event_count().expect("event count");
        assert!(
            store
                .dispatch_agent_work(&instance, &lease, now + Duration::seconds(2))
                .is_err()
        );
        assert_eq!(
            store.event_count().expect("event count"),
            event_count_after_dispatch
        );

        let usage = AgentUsageRecord {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: UsageRecordId::new(),
            run_id: run.id,
            task_id: task.id,
            instance_id: Some(instance.id),
            kind: UsageKind::ProviderModel,
            provider_connection_id: Some("fixture-provider".into()),
            model_id: Some("fixture-model".into()),
            tool_execution_id: None,
            input_tokens: 100,
            cached_input_tokens: 20,
            output_tokens: 40,
            cost_usd: Some(0.01),
            duration_ms: 250,
            occurred_at: now + Duration::seconds(3),
        };
        store.record_agent_usage(&usage).expect("record usage");
        let report = EvaluationReport {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: EvaluationId::new(),
            run_id: run.id,
            task_id: task.id,
            evaluator_instance_id: None,
            conditions: vec![ConditionResult {
                condition: SuccessCondition::Test {
                    command: "npm test".into(),
                },
                passed: true,
                detail: "fixture suite passed".into(),
            }],
            outcome: EvaluationOutcome::Passed,
            confidence: ConfidenceEstimate::new(9_000, 1).expect("confidence"),
            regressions: Vec::new(),
            evidence: vec![EvaluationEvidence {
                method: "process_exit".into(),
                artifact_sha256: Some("fixture-sha256".into()),
                event_sequence: None,
                detail: "exit code 0".into(),
            }],
            evaluated_at: now + Duration::seconds(4),
        };
        store.record_evaluation(&report).expect("record evaluation");
        drop(store);

        let mut reopened = SqliteEventStore::open(&database).expect("reopen store");
        let projection_recovery = reopened
            .recover_projections()
            .expect("rebuild task projections with dependent agent rows");
        assert_eq!(projection_recovery.tasks_rebuilt, 1);
        assert_eq!(
            reopened
                .organization_history(genome.organization_id)
                .expect("history"),
            vec![genome.clone()]
        );
        assert_eq!(
            reopened.work_plan(run.id).expect("plan"),
            Some(plan.clone())
        );
        assert_eq!(
            reopened.agent_usage(run.id).expect("usage"),
            vec![usage.clone()]
        );
        assert_eq!(
            reopened.evaluations(run.id).expect("evaluations"),
            vec![report.clone()]
        );

        let recovery = reopened
            .recover_agent_work(now + Duration::seconds(30))
            .expect("recover work");
        assert_eq!(
            recovery,
            AgentRecoveryReport {
                runs_marked_recovering: 1,
                instances_expired: 1,
                leases_expired: 1,
            }
        );
        assert_eq!(
            reopened
                .agent_run(run.id)
                .expect("run")
                .expect("present")
                .state,
            AgentRunState::Recovering
        );
        assert_eq!(
            reopened
                .agent_instance(instance.id)
                .expect("instance")
                .expect("present")
                .state,
            AgentState::Expired
        );
        assert_eq!(
            reopened
                .agent_lease(lease.id)
                .expect("lease")
                .expect("present")
                .state,
            AgentLeaseState::Expired
        );
        let events = reopened.events_for_task(task.id).expect("task events");
        assert!(
            events
                .windows(2)
                .all(|pair| pair[0].sequence < pair[1].sequence)
        );
        assert!(
            events
                .iter()
                .any(|event| event.kind == EventKind::AgentUsageRecorded)
        );
        assert!(
            events
                .iter()
                .any(|event| event.kind == EventKind::EvaluationRecorded)
        );
        assert!(events.iter().any(|event| {
            event.kind == EventKind::AgentRunStateChanged
                && event.payload.get("reason").and_then(|value| value.as_str())
                    == Some("daemon_restart")
        }));

        reopened
            .transition_agent_run(
                run.id,
                AgentRunState::Running,
                OrganizationalMetrics::default(),
                now + Duration::seconds(31),
            )
            .expect("resume recovering run");
        let retry_instance = AgentInstance {
            id: AgentInstanceId::new(),
            state: AgentState::Created,
            created_at: now + Duration::seconds(31),
            updated_at: now + Duration::seconds(31),
            ..instance.clone()
        };
        reopened
            .record_agent_instance(&retry_instance)
            .expect("retry instance");
        reopened
            .transition_agent_instance(
                retry_instance.id,
                AgentState::Running,
                retry_instance.confidence,
                now + Duration::seconds(31),
            )
            .expect("start retry instance");
        let retry_lease = AgentLease {
            id: AgentLeaseId::new(),
            instance_id: retry_instance.id,
            state: AgentLeaseState::Active,
            acquired_at: now + Duration::seconds(31),
            expires_at: now + Duration::minutes(15),
            heartbeat_at: now + Duration::seconds(31),
            ..lease.clone()
        };
        reopened
            .record_agent_lease(&retry_lease)
            .expect("retry lease");
        reopened
            .settle_agent_turn_error(
                retry_instance.id,
                retry_lease.id,
                AgentTurnDisposition::Waiting,
                "approval_required",
                now + Duration::seconds(32),
            )
            .expect("wait for approval");
        assert_eq!(
            reopened
                .agent_run(run.id)
                .expect("run")
                .expect("present")
                .state,
            AgentRunState::Waiting
        );
        assert_eq!(
            reopened
                .agent_lease(retry_lease.id)
                .expect("lease")
                .expect("present")
                .state,
            AgentLeaseState::Active
        );
        reopened
            .transition_agent_run(
                run.id,
                AgentRunState::Running,
                OrganizationalMetrics::default(),
                now + Duration::seconds(33),
            )
            .expect("resume approved run");
        reopened
            .transition_agent_instance(
                retry_instance.id,
                AgentState::Running,
                retry_instance.confidence,
                now + Duration::seconds(33),
            )
            .expect("resume approved instance");
        reopened
            .settle_agent_turn_error(
                retry_instance.id,
                retry_lease.id,
                AgentTurnDisposition::Failed,
                "provider_failure",
                now + Duration::seconds(34),
            )
            .expect("settle failure");
        assert_eq!(
            reopened
                .agent_instance(retry_instance.id)
                .expect("instance")
                .expect("present")
                .state,
            AgentState::Failed
        );
        assert_eq!(
            reopened
                .agent_lease(retry_lease.id)
                .expect("lease")
                .expect("present")
                .state,
            AgentLeaseState::Failed
        );
        assert_eq!(reopened.agent_usage(run.id).expect("usage"), vec![usage]);
        assert_eq!(
            reopened.agent_turn_result(retry_lease.id).expect("result"),
            None
        );
    }
}
