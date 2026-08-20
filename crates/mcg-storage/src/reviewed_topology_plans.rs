use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use mcg_domain::{ReviewedTopologyPlanId, ReviewedTopologyStepId};
use mcg_events::{EventEnvelope, EventKind};
use mcg_projects::{
    REVIEWED_TOPOLOGY_PLAN_SCHEMA_VERSION, ReviewedTopologyPlan, ReviewedTopologyPlanState,
    ReviewedTopologyStep, ReviewedTopologyStepState,
};
use rusqlite::params;

use crate::{SqliteEventStore, StorageError};

impl SqliteEventStore {
    /// Atomically records one immutable, snapshot-pinned reviewed plan and its event.
    /// Exact caller identity replay is observation-only.
    ///
    /// # Errors
    ///
    /// Returns an error when validation, project pinning, serialization, or the
    /// atomic `SQLite` transaction fails, or when the caller identity conflicts.
    pub fn create_reviewed_topology_plan(
        &mut self,
        plan: &ReviewedTopologyPlan,
    ) -> Result<(ReviewedTopologyPlan, bool, Vec<EventEnvelope>), StorageError> {
        validate_plan(plan, true)?;
        if let Some(existing) = self.reviewed_topology_plan(plan.id)? {
            if existing.definition_hash == plan.definition_hash {
                return Ok((existing, false, Vec::new()));
            }
            return Err(StorageError::Conflict(format!(
                "reviewed topology plan {} already exists with a different definition",
                plan.id
            )));
        }
        let current = self
            .project(plan.project_id)?
            .ok_or(StorageError::ProjectNotFound(plan.project_id))?;
        let snapshot =
            self.project_snapshot(plan.project_snapshot_id)?
                .ok_or(StorageError::InvalidData(
                    "reviewed topology plan references an unknown project snapshot".into(),
                ))?;
        if current.id != plan.project_snapshot_id
            || snapshot.project_id != plan.project_id
            || snapshot.fingerprint != plan.project_fingerprint
        {
            return Err(StorageError::Conflict(
                "reviewed topology plan does not pin the current project snapshot".into(),
            ));
        }
        let event = EventEnvelope::new(
            EventKind::ReviewedTopologyPlanCreated,
            None,
            serde_json::to_value(plan)?,
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO reviewed_topology_plans(
                plan_id, project_id, project_snapshot_id, state, definition_hash,
                plan_json, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                plan.id.to_string(),
                plan.project_id.to_string(),
                plan.project_snapshot_id.to_string(),
                plan_state_name(plan.state),
                plan.definition_hash,
                serde_json::to_string(plan)?,
                plan.created_at.to_rfc3339(),
                plan.updated_at.to_rfc3339(),
            ],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok((plan.clone(), true, vec![event]))
    }

    /// Reads one reviewed plan by its stable identity.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` access or stored-contract decoding fails.
    pub fn reviewed_topology_plan(
        &self,
        plan_id: ReviewedTopologyPlanId,
    ) -> Result<Option<ReviewedTopologyPlan>, StorageError> {
        self.read_optional_json(
            "SELECT plan_json FROM reviewed_topology_plans WHERE plan_id = ?1",
            &plan_id.to_string(),
        )
    }

    /// Lists reviewed plans for one project in stable creation order.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` access or stored-contract decoding fails.
    pub fn reviewed_topology_plans(
        &self,
        project_id: mcg_domain::ProjectId,
    ) -> Result<Vec<ReviewedTopologyPlan>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT plan_json FROM reviewed_topology_plans
             WHERE project_id = ?1 ORDER BY created_at, plan_id",
        )?;
        let rows = statement.query_map([project_id.to_string()], |row| row.get::<_, String>(0))?;
        rows.map(|row| serde_json::from_str(&row?).map_err(StorageError::from))
            .collect()
    }

    /// Atomically commits one legal plan/step transition and its event.
    ///
    /// # Errors
    ///
    /// Returns an error when the immutable definition or expected state changed,
    /// a transition is illegal, or the atomic `SQLite` transaction fails.
    pub fn update_reviewed_topology_plan(
        &mut self,
        expected_state: ReviewedTopologyPlanState,
        plan: &ReviewedTopologyPlan,
    ) -> Result<(ReviewedTopologyPlan, EventEnvelope), StorageError> {
        validate_plan(plan, false)?;
        let current = self
            .reviewed_topology_plan(plan.id)?
            .ok_or(StorageError::ReviewedTopologyPlanNotFound(plan.id))?;
        if current.state != expected_state
            || current.definition_hash != plan.definition_hash
            || !same_plan_definition(&current, plan)
        {
            return Err(StorageError::Conflict(
                "reviewed topology plan authority or immutable definition changed".into(),
            ));
        }
        if !plan_transition_allowed(expected_state, plan.state) {
            return Err(StorageError::InvalidData(format!(
                "invalid reviewed topology plan transition from {expected_state:?} to {:?}",
                plan.state
            )));
        }
        validate_step_transitions(&current.steps, &plan.steps)?;
        let event = EventEnvelope::new(
            EventKind::ReviewedTopologyPlanStateChanged,
            None,
            serde_json::json!({"from": expected_state, "plan": plan}),
        );
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE reviewed_topology_plans
             SET state = ?1, plan_json = ?2, updated_at = ?3
             WHERE plan_id = ?4 AND state = ?5 AND definition_hash = ?6",
            params![
                plan_state_name(plan.state),
                serde_json::to_string(plan)?,
                plan.updated_at.to_rfc3339(),
                plan.id.to_string(),
                plan_state_name(expected_state),
                plan.definition_hash,
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::Conflict(
                "reviewed topology plan state changed concurrently".into(),
            ));
        }
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok((plan.clone(), event))
    }

    /// Converts every in-flight local command to uncertain without replaying it.
    /// A running plan that crashed before claiming a command becomes safely resumable.
    ///
    /// # Errors
    ///
    /// Returns an error when project/plan loading or an atomic recovery update fails.
    pub fn recover_reviewed_topology_plans(
        &mut self,
        now: DateTime<Utc>,
    ) -> Result<(u64, Vec<EventEnvelope>), StorageError> {
        let project_ids = self
            .projects()?
            .into_iter()
            .map(|project| project.project_id);
        let mut plans = Vec::new();
        for project_id in project_ids {
            plans.extend(self.reviewed_topology_plans(project_id)?);
        }
        let mut recovered = 0_u64;
        let mut events = Vec::new();
        for mut plan in plans
            .into_iter()
            .filter(|plan| plan.state == ReviewedTopologyPlanState::Running)
        {
            let had_executing = plan
                .steps
                .iter()
                .any(|step| step.state == ReviewedTopologyStepState::Executing);
            if had_executing {
                for step in &mut plan.steps {
                    match step.state {
                        ReviewedTopologyStepState::Executing => {
                            step.state = ReviewedTopologyStepState::Uncertain;
                            step.detail =
                                "daemon restarted while this command could have been executing"
                                    .into();
                        }
                        ReviewedTopologyStepState::Pending => {
                            step.state = ReviewedTopologyStepState::Skipped;
                            step.detail = "not executed after uncertain predecessor".into();
                        }
                        _ => {}
                    }
                }
                plan.state = ReviewedTopologyPlanState::Uncertain;
            } else {
                plan.state = ReviewedTopologyPlanState::Waiting;
            }
            plan.updated_at = now;
            let (_, event) =
                self.update_reviewed_topology_plan(ReviewedTopologyPlanState::Running, &plan)?;
            events.push(event);
            recovered = recovered.saturating_add(1);
        }
        Ok((recovered, events))
    }
}

fn validate_plan(plan: &ReviewedTopologyPlan, creation: bool) -> Result<(), StorageError> {
    let valid = plan.schema_version == REVIEWED_TOPOLOGY_PLAN_SCHEMA_VERSION
        && is_sha256(&plan.definition_hash)
        && is_sha256(&plan.project_fingerprint)
        && is_sha256(&plan.topology_sha256)
        && !plan.display_name.trim().is_empty()
        && plan.display_name.chars().count() <= 160
        && !plan.reason.trim().is_empty()
        && plan.reason.chars().count() <= 1_024
        && !plan.reason.chars().any(char::is_control)
        && !plan.steps.is_empty()
        && plan.steps.len() <= 128
        && plan.created_at <= plan.updated_at;
    if !valid
        || (creation && plan.state != ReviewedTopologyPlanState::Reviewed)
        || (creation
            && plan.steps.iter().any(|step| {
                step.state != ReviewedTopologyStepState::Pending
                    || !step.detail.is_empty()
                    || step.exit_code.is_some()
                    || !step.log_artifact_sha256.is_empty()
            }))
    {
        return Err(StorageError::InvalidData(
            "invalid reviewed topology plan".into(),
        ));
    }
    let ids = plan
        .steps
        .iter()
        .map(|step| step.id)
        .collect::<BTreeSet<ReviewedTopologyStepId>>();
    if ids.len() != plan.steps.len()
        || plan
            .steps
            .windows(2)
            .any(|pair| (pair[0].stage_index, pair[0].id) >= (pair[1].stage_index, pair[1].id))
        || plan.steps.iter().any(invalid_step)
    {
        return Err(StorageError::InvalidData(
            "reviewed topology plan steps are invalid or unstable".into(),
        ));
    }
    Ok(())
}

fn invalid_step(step: &ReviewedTopologyStep) -> bool {
    step.name.trim().is_empty()
        || step.name.chars().count() > 160
        || step.command_name.trim().is_empty()
        || step.command.trim().is_empty()
        || step.executable.trim().is_empty()
        || step.source.trim().is_empty()
        || step.timeout_seconds == 0
        || step.timeout_seconds > 7_200
        || step.arguments.len() > 128
        || step
            .arguments
            .iter()
            .any(|value| value.len() > 4_096 || value.contains('\0'))
        || step.detail.len() > 4_096
        || step.log_artifact_sha256.len() > 4
        || step
            .log_artifact_sha256
            .iter()
            .any(|value| !is_sha256(value))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn same_plan_definition(left: &ReviewedTopologyPlan, right: &ReviewedTopologyPlan) -> bool {
    left.id == right.id
        && left.project_id == right.project_id
        && left.project_snapshot_id == right.project_snapshot_id
        && left.project_fingerprint == right.project_fingerprint
        && left.topology_sha256 == right.topology_sha256
        && left.display_name == right.display_name
        && left.reason == right.reason
        && left.created_at == right.created_at
        && left.steps.len() == right.steps.len()
        && left
            .steps
            .iter()
            .zip(&right.steps)
            .all(|(left, right)| same_step_definition(left, right))
}

fn same_step_definition(left: &ReviewedTopologyStep, right: &ReviewedTopologyStep) -> bool {
    left.id == right.id
        && left.stage_index == right.stage_index
        && left.name == right.name
        && left.command_name == right.command_name
        && left.command == right.command
        && left.executable == right.executable
        && left.arguments == right.arguments
        && left.source == right.source
        && left.timeout_seconds == right.timeout_seconds
}

fn validate_step_transitions(
    before: &[ReviewedTopologyStep],
    after: &[ReviewedTopologyStep],
) -> Result<(), StorageError> {
    for (before, after) in before.iter().zip(after) {
        let legal = match (before.state, after.state) {
            (left, right) if left == right => {
                before.detail == after.detail
                    && before.exit_code == after.exit_code
                    && before.log_artifact_sha256 == after.log_artifact_sha256
            }
            (ReviewedTopologyStepState::Pending, ReviewedTopologyStepState::Executing) => {
                after.detail.is_empty()
                    && after.exit_code.is_none()
                    && after.log_artifact_sha256.is_empty()
            }
            (ReviewedTopologyStepState::Pending, ReviewedTopologyStepState::Skipped)
            | (
                ReviewedTopologyStepState::Executing,
                ReviewedTopologyStepState::Succeeded
                | ReviewedTopologyStepState::Failed
                | ReviewedTopologyStepState::Uncertain,
            ) => true,
            _ => false,
        };
        if !legal {
            return Err(StorageError::InvalidData(format!(
                "invalid reviewed topology step transition for {}",
                before.id
            )));
        }
    }
    Ok(())
}

const fn plan_transition_allowed(
    from: ReviewedTopologyPlanState,
    to: ReviewedTopologyPlanState,
) -> bool {
    matches!(
        (from, to),
        (
            ReviewedTopologyPlanState::Reviewed | ReviewedTopologyPlanState::Waiting,
            ReviewedTopologyPlanState::Running | ReviewedTopologyPlanState::Cancelled
        ) | (
            ReviewedTopologyPlanState::Running,
            ReviewedTopologyPlanState::Running
                | ReviewedTopologyPlanState::Waiting
                | ReviewedTopologyPlanState::Succeeded
                | ReviewedTopologyPlanState::Failed
                | ReviewedTopologyPlanState::Uncertain
                | ReviewedTopologyPlanState::Cancelled
        )
    )
}

const fn plan_state_name(state: ReviewedTopologyPlanState) -> &'static str {
    match state {
        ReviewedTopologyPlanState::Reviewed => "reviewed",
        ReviewedTopologyPlanState::Running => "running",
        ReviewedTopologyPlanState::Waiting => "waiting",
        ReviewedTopologyPlanState::Succeeded => "succeeded",
        ReviewedTopologyPlanState::Failed => "failed",
        ReviewedTopologyPlanState::Uncertain => "uncertain",
        ReviewedTopologyPlanState::Cancelled => "cancelled",
    }
}
