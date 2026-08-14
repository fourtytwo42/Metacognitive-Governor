use chrono::{DateTime, Duration, Utc};
use mcg_domain::{
    SCHEDULE_SCHEMA_VERSION, ScheduleDefinition, ScheduleId, ScheduleOccurrence,
    ScheduleOccurrenceId, ScheduleOccurrenceState, ScheduleState, ScheduleTrigger, TaskEnvelope,
    TaskId, TaskState,
};
use mcg_events::{EventEnvelope, EventKind, TaskCreatedPayload};
use rusqlite::{OptionalExtension, Transaction, params};

use crate::{SqliteEventStore, StorageError};

#[derive(Debug, Clone, PartialEq)]
pub struct ScheduleClaim {
    pub schedule: ScheduleDefinition,
    pub occurrence: ScheduleOccurrence,
    pub task: TaskEnvelope,
    pub task_created_event: Option<EventEnvelope>,
}

impl SqliteEventStore {
    /// Settles schedule occurrences whose approval-gated tasks became terminal.
    ///
    /// # Errors
    /// Returns an error when durable rows cannot be read, decoded, or updated atomically.
    pub fn reconcile_schedule_boundaries(
        &mut self,
        now: DateTime<Utc>,
    ) -> Result<u64, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT o.occurrence_json, t.task_json FROM schedule_occurrences o
             JOIN tasks t ON t.task_id = o.task_id
             WHERE o.state = 'waiting_for_approval' ORDER BY o.occurrence_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut settled = Vec::new();
        for row in rows {
            let (occurrence_json, task_json) = row?;
            let mut occurrence: ScheduleOccurrence = serde_json::from_str(&occurrence_json)?;
            let task: TaskEnvelope = serde_json::from_str(&task_json)?;
            let (state, detail) = match task.state {
                TaskState::Completed => (
                    ScheduleOccurrenceState::Completed,
                    "approved scheduled work completed",
                ),
                TaskState::Blocked | TaskState::Failed | TaskState::Cancelled => (
                    ScheduleOccurrenceState::Failed,
                    "scheduled work stopped after its owner boundary",
                ),
                _ => continue,
            };
            occurrence.state = state;
            occurrence.detail = Some(detail.into());
            occurrence.updated_at = now;
            occurrence.completed_at = Some(now);
            settled.push(occurrence);
        }
        drop(statement);
        let transaction = self.connection.transaction()?;
        for occurrence in &settled {
            update_occurrence_tx(&transaction, occurrence)?;
        }
        transaction.commit()?;
        u64::try_from(settled.len())
            .map_err(|_| StorageError::InvalidData("schedule reconciliation count overflow".into()))
    }

    /// Expires in-flight scheduler authority so a restarted daemon can reclaim it immediately.
    ///
    /// # Errors
    /// Returns an error when durable occurrences cannot be read, decoded, or updated atomically.
    pub fn recover_schedule_claims(&mut self, now: DateTime<Utc>) -> Result<u64, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT occurrence_json FROM schedule_occurrences
             WHERE state IN ('running', 'recovering') ORDER BY occurrence_id",
        )?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        let mut occurrences = Vec::new();
        for row in rows {
            occurrences.push(serde_json::from_str::<ScheduleOccurrence>(&row?)?);
        }
        drop(statement);
        let transaction = self.connection.transaction()?;
        for occurrence in &mut occurrences {
            occurrence.state = ScheduleOccurrenceState::Recovering;
            occurrence.lease_expires_at = Some(now);
            occurrence.updated_at = now;
            update_occurrence_tx(&transaction, occurrence)?;
        }
        transaction.commit()?;
        u64::try_from(occurrences.len())
            .map_err(|_| StorageError::InvalidData("schedule recovery count overflow".into()))
    }

    /// Persists one validated schedule definition.
    ///
    /// # Errors
    /// Returns an error for invalid definitions, duplicate identities, or database failures.
    pub fn create_schedule(&mut self, schedule: &ScheduleDefinition) -> Result<(), StorageError> {
        validate_schedule(schedule)?;
        self.connection.execute(
            "INSERT INTO schedules(schedule_id, state, next_run_at, schedule_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                schedule.id.to_string(),
                schedule_state_name(schedule.state),
                schedule.next_run_at.map(|value| value.to_rfc3339()),
                serde_json::to_string(schedule)?,
                schedule.created_at.to_rfc3339(),
                schedule.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Loads one schedule definition.
    ///
    /// # Errors
    /// Returns an error when its row cannot be read or decoded.
    pub fn schedule(
        &self,
        schedule_id: ScheduleId,
    ) -> Result<Option<ScheduleDefinition>, StorageError> {
        self.connection
            .query_row(
                "SELECT schedule_json FROM schedules WHERE schedule_id = ?1",
                [schedule_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
            .transpose()
    }

    /// Lists durable schedule definitions in stable order.
    ///
    /// # Errors
    /// Returns an error when rows cannot be read or decoded.
    pub fn schedules(&self) -> Result<Vec<ScheduleDefinition>, StorageError> {
        let mut statement = self
            .connection
            .prepare("SELECT schedule_json FROM schedules ORDER BY created_at, schedule_id")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.map(|row| {
            let json = row?;
            serde_json::from_str(&json).map_err(StorageError::from)
        })
        .collect()
    }

    /// Lists the complete occurrence history for one schedule.
    ///
    /// # Errors
    /// Returns an error when rows cannot be read or decoded.
    pub fn schedule_occurrences(
        &self,
        schedule_id: ScheduleId,
    ) -> Result<Vec<ScheduleOccurrence>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT occurrence_json FROM schedule_occurrences
             WHERE schedule_id = ?1 ORDER BY due_at, occurrence_id",
        )?;
        let rows = statement.query_map([schedule_id.to_string()], |row| row.get::<_, String>(0))?;
        rows.map(|row| {
            let json = row?;
            serde_json::from_str(&json).map_err(StorageError::from)
        })
        .collect()
    }

    /// Updates the admission state of one schedule without deleting history.
    ///
    /// # Errors
    /// Returns an error for missing schedules, illegal re-enablement, or persistence failures.
    pub fn set_schedule_state(
        &mut self,
        schedule_id: ScheduleId,
        state: ScheduleState,
        now: DateTime<Utc>,
    ) -> Result<ScheduleDefinition, StorageError> {
        let mut schedule = self
            .schedule(schedule_id)?
            .ok_or(StorageError::ScheduleNotFound(schedule_id))?;
        if state == ScheduleState::Enabled && schedule.next_run_at.is_none() {
            return Err(StorageError::Conflict(
                "a completed one-time schedule cannot be enabled".into(),
            ));
        }
        schedule.state = state;
        schedule.updated_at = now;
        if state == ScheduleState::Completed {
            schedule.next_run_at = None;
        }
        self.update_schedule(&schedule)?;
        Ok(schedule)
    }

    /// Atomically reclaims expired work or creates the next due task occurrence.
    ///
    /// # Errors
    /// Returns an error for invalid lease bounds, corrupt definitions, or transaction failures.
    pub fn claim_due_schedule(
        &mut self,
        now: DateTime<Utc>,
        lease_owner: &str,
        lease_seconds: i64,
    ) -> Result<Option<ScheduleClaim>, StorageError> {
        if lease_owner.trim().is_empty() || lease_seconds <= 0 {
            return Err(StorageError::InvalidData(
                "schedule lease owner and duration must be valid".into(),
            ));
        }
        let transaction = self.connection.transaction()?;
        if let Some(claim) = reclaim_expired(&transaction, now, lease_owner, lease_seconds)? {
            transaction.commit()?;
            return Ok(Some(claim));
        }

        let schedule_json = transaction
            .query_row(
                "SELECT schedule_json FROM schedules
                 WHERE state = 'enabled' AND next_run_at IS NOT NULL AND next_run_at <= ?1
                   AND NOT EXISTS (
                       SELECT 1 FROM schedule_occurrences o
                       WHERE o.schedule_id = schedules.schedule_id
                         AND o.state IN ('running', 'recovering', 'waiting_for_approval')
                   )
                 ORDER BY next_run_at, schedule_id LIMIT 1",
                [now.to_rfc3339()],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(schedule_json) = schedule_json else {
            transaction.commit()?;
            return Ok(None);
        };
        let mut schedule: ScheduleDefinition = serde_json::from_str(&schedule_json)?;
        let due_at = schedule
            .next_run_at
            .ok_or_else(|| StorageError::InvalidData("due schedule has no next run".into()))?;

        if budget_is_exhausted_before_start(&schedule) {
            let occurrence = ScheduleOccurrence {
                schema_version: SCHEDULE_SCHEMA_VERSION,
                id: ScheduleOccurrenceId::new(),
                schedule_id: schedule.id,
                task_id: None,
                due_at,
                state: ScheduleOccurrenceState::BudgetExhausted,
                lease_owner: None,
                lease_expires_at: None,
                attempt: 0,
                detail: Some("schedule budget cannot cover its configured model work".into()),
                started_at: now,
                updated_at: now,
                completed_at: Some(now),
            };
            insert_occurrence(&transaction, &occurrence)?;
            schedule.state = ScheduleState::Paused;
            schedule.updated_at = now;
            update_schedule_tx(&transaction, &schedule)?;
            transaction.commit()?;
            return Ok(None);
        }

        let task = TaskEnvelope {
            schema_version: mcg_domain::TASK_SCHEMA_VERSION,
            id: TaskId::new(),
            project_id: Some(schedule.project_id),
            thread_id: None,
            goal: schedule.goal.clone(),
            constraints: schedule.constraints.clone(),
            success_conditions: schedule.success_conditions.clone(),
            risk: schedule.risk,
            budget: schedule.budget.clone(),
            state: TaskState::Pending,
            created_at: now,
            updated_at: now,
        };
        let payload = serde_json::to_value(TaskCreatedPayload { task: task.clone() })?;
        let event = EventEnvelope::new(EventKind::TaskCreated, Some(task.id), payload);
        let event = SqliteEventStore::insert_event(&transaction, &event)?;
        SqliteEventStore::upsert_task(&transaction, &task, event.sequence)?;

        let occurrence = ScheduleOccurrence {
            schema_version: SCHEDULE_SCHEMA_VERSION,
            id: ScheduleOccurrenceId::new(),
            schedule_id: schedule.id,
            task_id: Some(task.id),
            due_at,
            state: ScheduleOccurrenceState::Running,
            lease_owner: Some(lease_owner.to_owned()),
            lease_expires_at: Some(now + Duration::seconds(lease_seconds)),
            attempt: 1,
            detail: None,
            started_at: now,
            updated_at: now,
            completed_at: None,
        };
        insert_occurrence(&transaction, &occurrence)?;
        advance_schedule(&mut schedule, due_at, now)?;
        update_schedule_tx(&transaction, &schedule)?;
        transaction.commit()?;
        Ok(Some(ScheduleClaim {
            schedule,
            occurrence,
            task,
            task_created_event: Some(event),
        }))
    }

    /// Settles one occurrence while verifying exact lease ownership.
    ///
    /// # Errors
    /// Returns an error for invalid states, lost authority, missing rows, or transaction failures.
    pub fn finish_schedule_occurrence(
        &mut self,
        occurrence_id: ScheduleOccurrenceId,
        lease_owner: &str,
        state: ScheduleOccurrenceState,
        detail: String,
        now: DateTime<Utc>,
    ) -> Result<ScheduleOccurrence, StorageError> {
        if !matches!(
            state,
            ScheduleOccurrenceState::WaitingForApproval
                | ScheduleOccurrenceState::Completed
                | ScheduleOccurrenceState::Failed
                | ScheduleOccurrenceState::BudgetExhausted
        ) {
            return Err(StorageError::InvalidData(
                "schedule occurrence can only settle into a boundary state".into(),
            ));
        }
        let transaction = self.connection.transaction()?;
        let json = transaction
            .query_row(
                "SELECT occurrence_json FROM schedule_occurrences WHERE occurrence_id = ?1",
                [occurrence_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| StorageError::InvalidData("schedule occurrence was not found".into()))?;
        let mut occurrence: ScheduleOccurrence = serde_json::from_str(&json)?;
        if !matches!(
            occurrence.state,
            ScheduleOccurrenceState::Running | ScheduleOccurrenceState::Recovering
        ) || occurrence.lease_owner.as_deref() != Some(lease_owner)
        {
            return Err(StorageError::Conflict(
                "schedule occurrence lease authority changed".into(),
            ));
        }
        occurrence.state = state;
        occurrence.detail = Some(detail);
        occurrence.updated_at = now;
        occurrence.lease_owner = None;
        occurrence.lease_expires_at = None;
        if state != ScheduleOccurrenceState::WaitingForApproval {
            occurrence.completed_at = Some(now);
        }
        update_occurrence_tx(&transaction, &occurrence)?;
        if matches!(
            state,
            ScheduleOccurrenceState::WaitingForApproval | ScheduleOccurrenceState::BudgetExhausted
        ) {
            let schedule_json: String = transaction.query_row(
                "SELECT schedule_json FROM schedules WHERE schedule_id = ?1",
                [occurrence.schedule_id.to_string()],
                |row| row.get(0),
            )?;
            let mut schedule: ScheduleDefinition = serde_json::from_str(&schedule_json)?;
            schedule.state = ScheduleState::Paused;
            schedule.updated_at = now;
            update_schedule_tx(&transaction, &schedule)?;
        }
        transaction.commit()?;
        Ok(occurrence)
    }

    fn update_schedule(&mut self, schedule: &ScheduleDefinition) -> Result<(), StorageError> {
        validate_schedule(schedule)?;
        let transaction = self.connection.transaction()?;
        update_schedule_tx(&transaction, schedule)?;
        transaction.commit()?;
        Ok(())
    }
}

fn reclaim_expired(
    transaction: &Transaction<'_>,
    now: DateTime<Utc>,
    lease_owner: &str,
    lease_seconds: i64,
) -> Result<Option<ScheduleClaim>, StorageError> {
    let row = transaction
        .query_row(
            "SELECT o.occurrence_json, s.schedule_json, t.task_json
             FROM schedule_occurrences o
             JOIN schedules s ON s.schedule_id = o.schedule_id
             JOIN tasks t ON t.task_id = o.task_id
             WHERE o.state IN ('running', 'recovering') AND o.lease_expires_at <= ?1
             ORDER BY o.lease_expires_at, o.occurrence_id LIMIT 1",
            [now.to_rfc3339()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?;
    let Some((occurrence_json, schedule_json, task_json)) = row else {
        return Ok(None);
    };
    let mut occurrence: ScheduleOccurrence = serde_json::from_str(&occurrence_json)?;
    let schedule: ScheduleDefinition = serde_json::from_str(&schedule_json)?;
    let task: TaskEnvelope = serde_json::from_str(&task_json)?;
    occurrence.state = ScheduleOccurrenceState::Recovering;
    occurrence.lease_owner = Some(lease_owner.to_owned());
    occurrence.lease_expires_at = Some(now + Duration::seconds(lease_seconds));
    occurrence.attempt = occurrence.attempt.saturating_add(1);
    occurrence.updated_at = now;
    update_occurrence_tx(transaction, &occurrence)?;
    Ok(Some(ScheduleClaim {
        schedule,
        occurrence,
        task,
        task_created_event: None,
    }))
}

fn validate_schedule(schedule: &ScheduleDefinition) -> Result<(), StorageError> {
    let valid_budget = schedule
        .budget
        .max_cost_usd
        .is_none_or(|value| value.is_finite() && value >= 0.0);
    if schedule.schema_version != SCHEDULE_SCHEMA_VERSION
        || schedule.name.trim().is_empty()
        || schedule.goal.trim().is_empty()
        || schedule.context_token_budget == 0
        || !valid_budget
        || matches!(schedule.trigger, ScheduleTrigger::Interval { seconds: 0 })
        || (schedule.state == ScheduleState::Enabled && schedule.next_run_at.is_none())
    {
        return Err(StorageError::InvalidData(
            "schedule definition is incomplete or inconsistent".into(),
        ));
    }
    Ok(())
}

fn budget_is_exhausted_before_start(schedule: &ScheduleDefinition) -> bool {
    schedule.max_model_calls == 0
        || schedule
            .budget
            .max_tokens
            .is_some_and(|limit| schedule.estimated_output_tokens > limit)
        || schedule.budget.max_cost_usd == Some(0.0)
        || schedule.budget.max_duration_seconds == Some(0)
}

fn advance_schedule(
    schedule: &mut ScheduleDefinition,
    due_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<(), StorageError> {
    match schedule.trigger {
        ScheduleTrigger::Once { .. } => {
            schedule.next_run_at = None;
            schedule.state = ScheduleState::Completed;
        }
        ScheduleTrigger::Interval { seconds } => {
            let seconds = i64::try_from(seconds)
                .map_err(|_| StorageError::InvalidData("schedule interval is too large".into()))?;
            let interval = Duration::seconds(seconds);
            let mut next = due_at + interval;
            while next <= now {
                next += interval;
            }
            schedule.next_run_at = Some(next);
        }
    }
    schedule.updated_at = now;
    Ok(())
}

fn insert_occurrence(
    transaction: &Transaction<'_>,
    occurrence: &ScheduleOccurrence,
) -> Result<(), StorageError> {
    transaction.execute(
        "INSERT INTO schedule_occurrences(
            occurrence_id, schedule_id, task_id, due_at, state, lease_owner,
            lease_expires_at, occurrence_json, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            occurrence.id.to_string(),
            occurrence.schedule_id.to_string(),
            occurrence.task_id.map(|id| id.to_string()),
            occurrence.due_at.to_rfc3339(),
            occurrence_state_name(occurrence.state),
            occurrence.lease_owner,
            occurrence.lease_expires_at.map(|value| value.to_rfc3339()),
            serde_json::to_string(occurrence)?,
            occurrence.updated_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

fn update_occurrence_tx(
    transaction: &Transaction<'_>,
    occurrence: &ScheduleOccurrence,
) -> Result<(), StorageError> {
    let changed = transaction.execute(
        "UPDATE schedule_occurrences SET state = ?1, lease_owner = ?2,
            lease_expires_at = ?3, occurrence_json = ?4, updated_at = ?5
         WHERE occurrence_id = ?6",
        params![
            occurrence_state_name(occurrence.state),
            occurrence.lease_owner,
            occurrence.lease_expires_at.map(|value| value.to_rfc3339()),
            serde_json::to_string(occurrence)?,
            occurrence.updated_at.to_rfc3339(),
            occurrence.id.to_string(),
        ],
    )?;
    if changed != 1 {
        return Err(StorageError::Conflict(
            "schedule occurrence changed during update".into(),
        ));
    }
    Ok(())
}

fn update_schedule_tx(
    transaction: &Transaction<'_>,
    schedule: &ScheduleDefinition,
) -> Result<(), StorageError> {
    validate_schedule(schedule)?;
    let changed = transaction.execute(
        "UPDATE schedules SET state = ?1, next_run_at = ?2, schedule_json = ?3,
            updated_at = ?4 WHERE schedule_id = ?5",
        params![
            schedule_state_name(schedule.state),
            schedule.next_run_at.map(|value| value.to_rfc3339()),
            serde_json::to_string(schedule)?,
            schedule.updated_at.to_rfc3339(),
            schedule.id.to_string(),
        ],
    )?;
    if changed != 1 {
        return Err(StorageError::ScheduleNotFound(schedule.id));
    }
    Ok(())
}

const fn schedule_state_name(state: ScheduleState) -> &'static str {
    match state {
        ScheduleState::Enabled => "enabled",
        ScheduleState::Paused => "paused",
        ScheduleState::Completed => "completed",
    }
}

const fn occurrence_state_name(state: ScheduleOccurrenceState) -> &'static str {
    match state {
        ScheduleOccurrenceState::Running => "running",
        ScheduleOccurrenceState::Recovering => "recovering",
        ScheduleOccurrenceState::WaitingForApproval => "waiting_for_approval",
        ScheduleOccurrenceState::Completed => "completed",
        ScheduleOccurrenceState::Failed => "failed",
        ScheduleOccurrenceState::BudgetExhausted => "budget_exhausted",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcg_domain::{ProjectId, RiskLevel, TaskBudget};

    fn interval_schedule(now: DateTime<Utc>) -> ScheduleDefinition {
        ScheduleDefinition {
            schema_version: SCHEDULE_SCHEMA_VERSION,
            id: ScheduleId::new(),
            name: "nightly verification".into(),
            project_id: ProjectId::new(),
            goal: "verify the project".into(),
            constraints: vec!["never auto-approve destructive work".into()],
            success_conditions: Vec::new(),
            risk: RiskLevel::Medium,
            budget: TaskBudget {
                max_tokens: Some(8_192),
                max_cost_usd: Some(2.0),
                max_duration_seconds: Some(600),
            },
            trigger: ScheduleTrigger::Interval { seconds: 60 },
            state: ScheduleState::Enabled,
            next_run_at: Some(now),
            target_path: None,
            context_token_budget: 8_192,
            requested_tools: Vec::new(),
            estimated_output_tokens: 2_048,
            max_model_calls: 4,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn due_claim_is_atomic_non_overlapping_and_restart_safe() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let database = directory.path().join("scheduler.db");
        let now = Utc::now();
        let schedule = interval_schedule(now);
        let task_id;
        let occurrence_id;
        {
            let mut store = SqliteEventStore::open(&database).expect("open store");
            store.create_schedule(&schedule).expect("create schedule");
            let claim = store
                .claim_due_schedule(now, "daemon-a", 30)
                .expect("claim due work")
                .expect("due claim");
            task_id = claim.task.id;
            occurrence_id = claim.occurrence.id;
            assert_eq!(claim.occurrence.attempt, 1);
            assert!(claim.task_created_event.is_some());
            assert!(store.load_task(task_id).expect("load task").is_some());
            assert!(
                store
                    .claim_due_schedule(now, "daemon-b", 30)
                    .expect("respect active claim")
                    .is_none()
            );
        }

        let mut reopened = SqliteEventStore::open(&database).expect("reopen store");
        assert_eq!(
            reopened
                .recover_schedule_claims(now + Duration::seconds(1))
                .expect("recover startup claims"),
            1
        );
        let recovered = reopened
            .claim_due_schedule(now + Duration::seconds(1), "daemon-c", 30)
            .expect("recover expired claim")
            .expect("recovered claim");
        assert_eq!(recovered.task.id, task_id);
        assert_eq!(recovered.occurrence.id, occurrence_id);
        assert_eq!(recovered.occurrence.attempt, 2);
        assert_eq!(
            recovered.occurrence.state,
            ScheduleOccurrenceState::Recovering
        );
        assert!(recovered.task_created_event.is_none());
        reopened
            .finish_schedule_occurrence(
                occurrence_id,
                "daemon-c",
                ScheduleOccurrenceState::Completed,
                "verified".into(),
                now + Duration::seconds(2),
            )
            .expect("settle recovered occurrence");
        let history = reopened
            .schedule_occurrences(schedule.id)
            .expect("load history");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].task_id, Some(task_id));
        assert_eq!(history[0].state, ScheduleOccurrenceState::Completed);
    }

    #[test]
    fn budget_and_approval_boundaries_pause_new_claims() {
        let now = Utc::now();
        let mut store = SqliteEventStore::open_in_memory().expect("open store");
        let mut budget_limited = interval_schedule(now);
        budget_limited.budget.max_tokens = Some(10);
        store
            .create_schedule(&budget_limited)
            .expect("create budget-limited schedule");
        assert!(
            store
                .claim_due_schedule(now, "daemon", 30)
                .expect("evaluate budget")
                .is_none()
        );
        assert_eq!(
            store
                .schedule(budget_limited.id)
                .expect("read schedule")
                .expect("schedule exists")
                .state,
            ScheduleState::Paused
        );
        let budget_history = store
            .schedule_occurrences(budget_limited.id)
            .expect("read budget history");
        assert_eq!(
            budget_history[0].state,
            ScheduleOccurrenceState::BudgetExhausted
        );
        assert!(budget_history[0].task_id.is_none());

        let approval_schedule = interval_schedule(now);
        store
            .create_schedule(&approval_schedule)
            .expect("create approval schedule");
        let claim = store
            .claim_due_schedule(now, "daemon", 30)
            .expect("claim approval work")
            .expect("claim exists");
        store
            .finish_schedule_occurrence(
                claim.occurrence.id,
                "daemon",
                ScheduleOccurrenceState::WaitingForApproval,
                "owner approval required".into(),
                now,
            )
            .expect("pause at approval");
        assert_eq!(
            store
                .schedule(approval_schedule.id)
                .expect("read schedule")
                .expect("schedule exists")
                .state,
            ScheduleState::Paused
        );
    }
}
