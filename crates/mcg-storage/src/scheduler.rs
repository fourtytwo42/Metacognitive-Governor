use chrono::{DateTime, Duration, Utc};
use mcg_domain::{
    NotificationSeverity, SCHEDULE_SCHEMA_VERSION, ScheduleDefinition, ScheduleId,
    ScheduleOccurrence, ScheduleOccurrenceId, ScheduleOccurrenceState, ScheduleState,
    ScheduleTrigger, SchedulerNotification, SchedulerNotificationId, SiteHealthCheck,
    SiteHealthState, SiteMonitor, TaskEnvelope, TaskId, TaskState, WakeRequest, WakeRequestId,
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

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScheduleWakeMutation {
    pub wake: WakeRequest,
    pub schedule: ScheduleDefinition,
    pub notification: SchedulerNotification,
}

impl SqliteEventStore {
    /// Returns the greatest committed event sequence, or zero for an empty log.
    ///
    /// # Errors
    /// Returns an error when the event log cannot be read.
    pub fn latest_event_sequence(&self) -> Result<u64, StorageError> {
        let sequence = self.connection.query_row(
            "SELECT COALESCE(MAX(sequence), 0) FROM events",
            [],
            |row| row.get::<_, i64>(0),
        )?;
        u64::try_from(sequence)
            .map_err(|_| StorageError::InvalidData("event sequence is negative".into()))
    }

    /// Advances event-trigger cursors and wakes definitions with matching new events.
    ///
    /// # Errors
    /// Returns an error when definitions/events cannot be read, decoded, or updated atomically.
    pub fn poll_event_schedules(
        &mut self,
        now: DateTime<Utc>,
    ) -> Result<Vec<ScheduleId>, StorageError> {
        let schedules = self.schedules()?;
        let transaction = self.connection.transaction()?;
        let mut awakened = Vec::new();
        for mut schedule in schedules.into_iter().filter(|schedule| {
            schedule.state == ScheduleState::Enabled
                && matches!(schedule.trigger, ScheduleTrigger::Event { .. })
        }) {
            let ScheduleTrigger::Event { event_kind } = &schedule.trigger else {
                continue;
            };
            let mut statement = transaction.prepare(
                "SELECT e.sequence, e.kind,
                        EXISTS (
                          SELECT 1 FROM schedule_occurrences o
                          WHERE o.schedule_id = ?2 AND o.task_id = e.task_id
                        ) AS own_occurrence
                 FROM events e
                 WHERE e.sequence > ?1
                 ORDER BY e.sequence",
            )?;
            let rows = statement.query_map(
                params![
                    i64::try_from(schedule.last_event_sequence).map_err(|_| {
                        StorageError::InvalidData("event cursor exceeds database range".into())
                    })?,
                    schedule.id.to_string()
                ],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, bool>(2)?,
                    ))
                },
            )?;
            let mut latest = schedule.last_event_sequence;
            let mut matched = false;
            for row in rows {
                let (sequence, kind, own_occurrence) = row?;
                latest = u64::try_from(sequence)
                    .map_err(|_| StorageError::InvalidData("event sequence is negative".into()))?;
                matched |= !own_occurrence && kind == *event_kind;
            }
            drop(statement);
            schedule.last_event_sequence = latest;
            if matched && schedule.next_run_at.is_none() {
                schedule.next_run_at = Some(now);
                awakened.push(schedule.id);
            }
            schedule.updated_at = now;
            update_schedule_tx(&transaction, &schedule)?;
        }
        transaction.commit()?;
        Ok(awakened)
    }

    /// Records a file-watch fingerprint and wakes the schedule only after a baseline changes.
    ///
    /// # Errors
    /// Returns an error for missing/non-watch schedules or atomic persistence failures.
    pub fn record_watch_observation(
        &mut self,
        schedule_id: ScheduleId,
        fingerprint: String,
        now: DateTime<Utc>,
    ) -> Result<bool, StorageError> {
        let mut schedule = self
            .schedule(schedule_id)?
            .ok_or(StorageError::ScheduleNotFound(schedule_id))?;
        if !matches!(schedule.trigger, ScheduleTrigger::FileWatch { .. }) {
            return Err(StorageError::InvalidData(
                "watch observations require a file-watch trigger".into(),
            ));
        }
        let changed = schedule
            .watch_fingerprint
            .as_ref()
            .is_some_and(|previous| previous != &fingerprint);
        schedule.watch_fingerprint = Some(fingerprint);
        schedule.last_watch_at = Some(now);
        schedule.updated_at = now;
        if changed && schedule.state == ScheduleState::Enabled && schedule.next_run_at.is_none() {
            schedule.next_run_at = Some(now);
        }
        self.update_schedule(&schedule)?;
        Ok(changed)
    }

    /// Persists a subsystem wake request and wakes an enabled schedule without overriding pause.
    ///
    /// # Errors
    /// Returns an error for an empty reason, missing schedule, or transaction failure.
    pub fn wake_schedule(
        &mut self,
        schedule_id: ScheduleId,
        severity: NotificationSeverity,
        reason: &str,
        now: DateTime<Utc>,
    ) -> Result<ScheduleWakeMutation, StorageError> {
        if reason.trim().is_empty() || reason.len() > 4_096 {
            return Err(StorageError::InvalidData(
                "wake request reason must contain 1 to 4096 bytes".into(),
            ));
        }
        let mut schedule = self
            .schedule(schedule_id)?
            .ok_or(StorageError::ScheduleNotFound(schedule_id))?;
        let transaction = self.connection.transaction()?;
        let wake = WakeRequest {
            schema_version: SCHEDULE_SCHEMA_VERSION,
            id: WakeRequestId::new(),
            schedule_id,
            severity,
            reason: reason.trim().to_owned(),
            requested_at: now,
        };
        transaction.execute(
            "INSERT INTO schedule_wake_requests(wake_id, schedule_id, wake_json, requested_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                wake.id.to_string(),
                schedule_id.to_string(),
                serde_json::to_string(&wake)?,
                now.to_rfc3339()
            ],
        )?;
        if schedule.state == ScheduleState::Enabled {
            schedule.next_run_at = Some(now);
            schedule.updated_at = now;
            update_schedule_tx(&transaction, &schedule)?;
        }
        let notification = SchedulerNotification {
            schema_version: SCHEDULE_SCHEMA_VERSION,
            id: SchedulerNotificationId::new(),
            schedule_id,
            occurrence_id: None,
            severity,
            title: "Scheduler wake requested".into(),
            detail: wake.reason.clone(),
            read: false,
            created_at: now,
            read_at: None,
        };
        insert_notification(&transaction, &notification)?;
        transaction.commit()?;
        Ok(ScheduleWakeMutation {
            wake,
            schedule,
            notification,
        })
    }

    /// Lists scheduler notifications, optionally including acknowledged records.
    ///
    /// # Errors
    /// Returns an error when notification rows cannot be read or decoded.
    pub fn scheduler_notifications(
        &self,
        include_read: bool,
    ) -> Result<Vec<SchedulerNotification>, StorageError> {
        let sql = if include_read {
            "SELECT notification_json FROM scheduler_notifications ORDER BY created_at DESC, notification_id"
        } else {
            "SELECT notification_json FROM scheduler_notifications WHERE is_read = 0 ORDER BY created_at DESC, notification_id"
        };
        let mut statement = self.connection.prepare(sql)?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.map(|row| {
            let json = row?;
            serde_json::from_str(&json).map_err(StorageError::from)
        })
        .collect()
    }

    /// Marks one scheduler notification as read idempotently.
    ///
    /// # Errors
    /// Returns an error when the notification is missing, corrupt, or cannot be updated.
    pub fn acknowledge_scheduler_notification(
        &mut self,
        notification_id: SchedulerNotificationId,
        now: DateTime<Utc>,
    ) -> Result<SchedulerNotification, StorageError> {
        let json = self
            .connection
            .query_row(
                "SELECT notification_json FROM scheduler_notifications WHERE notification_id = ?1",
                [notification_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| {
                StorageError::InvalidData("scheduler notification was not found".into())
            })?;
        let mut notification: SchedulerNotification = serde_json::from_str(&json)?;
        if !notification.read {
            notification.read = true;
            notification.read_at = Some(now);
            self.connection.execute(
                "UPDATE scheduler_notifications SET is_read = 1, notification_json = ?1,
                    read_at = ?2 WHERE notification_id = ?3",
                params![
                    serde_json::to_string(&notification)?,
                    now.to_rfc3339(),
                    notification_id.to_string()
                ],
            )?;
        }
        Ok(notification)
    }

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
            let (severity, title) = if occurrence.state == ScheduleOccurrenceState::Completed {
                (
                    NotificationSeverity::Info,
                    "Approved scheduled work completed",
                )
            } else {
                (
                    NotificationSeverity::Warning,
                    "Approved scheduled work stopped",
                )
            };
            insert_notification(
                &transaction,
                &SchedulerNotification {
                    schema_version: SCHEDULE_SCHEMA_VERSION,
                    id: SchedulerNotificationId::new(),
                    schedule_id: occurrence.schedule_id,
                    occurrence_id: Some(occurrence.id),
                    severity,
                    title: title.into(),
                    detail: occurrence.detail.clone().unwrap_or_default(),
                    read: false,
                    created_at: now,
                    read_at: None,
                },
            )?;
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
        let transaction = self.connection.transaction()?;
        Self::insert_schedule_tx(&transaction, schedule)?;
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn insert_schedule_tx(
        transaction: &Transaction<'_>,
        schedule: &ScheduleDefinition,
    ) -> Result<(), StorageError> {
        validate_schedule(schedule)?;
        let inserted = transaction.execute(
            "INSERT OR IGNORE INTO schedules(schedule_id, state, next_run_at, schedule_json, created_at, updated_at)
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
        if inserted != 1 {
            return Err(StorageError::Conflict(
                "schedule identity already exists".into(),
            ));
        }
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
        if state == ScheduleState::Enabled
            && schedule.next_run_at.is_none()
            && !matches!(
                schedule.trigger,
                ScheduleTrigger::FileWatch { .. } | ScheduleTrigger::Event { .. }
            )
        {
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
            pause_for_budget_exhaustion(&transaction, &mut schedule, due_at, now)?;
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
            lease_expires_at: Some(schedule_lease_expiry(now, lease_seconds)?),
            attempt: 1,
            detail: None,
            started_at: now,
            updated_at: now,
            completed_at: None,
        };
        insert_occurrence(&transaction, &occurrence)?;
        let system_busy = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE state IN ('active', 'waiting_for_approval', 'blocked'))",
            [],
            |row| row.get::<_, bool>(0),
        )?;
        advance_schedule(&mut schedule, due_at, now, system_busy)?;
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
        let (severity, title) = match state {
            ScheduleOccurrenceState::Completed => {
                (NotificationSeverity::Info, "Scheduled work completed")
            }
            ScheduleOccurrenceState::WaitingForApproval => (
                NotificationSeverity::High,
                "Schedule paused for owner approval",
            ),
            ScheduleOccurrenceState::Failed => {
                (NotificationSeverity::Warning, "Scheduled work failed")
            }
            ScheduleOccurrenceState::BudgetExhausted => (
                NotificationSeverity::Warning,
                "Schedule paused at budget boundary",
            ),
            ScheduleOccurrenceState::Running | ScheduleOccurrenceState::Recovering => {
                unreachable!("validated settlement state")
            }
        };
        insert_notification(
            &transaction,
            &SchedulerNotification {
                schema_version: SCHEDULE_SCHEMA_VERSION,
                id: SchedulerNotificationId::new(),
                schedule_id: occurrence.schedule_id,
                occurrence_id: Some(occurrence.id),
                severity,
                title: title.into(),
                detail: occurrence.detail.clone().unwrap_or_default(),
                read: false,
                created_at: now,
                read_at: None,
            },
        )?;
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

    /// Settles a typed Site-monitor occurrence and atomically records its health transition.
    /// Unchanged health states do not emit another notification.
    ///
    /// # Errors
    /// Returns an error for lost lease authority, a non-monitor schedule, or invalid state.
    pub fn finish_site_monitor_occurrence(
        &mut self,
        occurrence_id: ScheduleOccurrenceId,
        lease_owner: &str,
        state: ScheduleOccurrenceState,
        detail: String,
        check: Option<&SiteHealthCheck>,
        now: DateTime<Utc>,
    ) -> Result<ScheduleOccurrence, StorageError> {
        if !matches!(
            state,
            ScheduleOccurrenceState::Completed | ScheduleOccurrenceState::Failed
        ) {
            return Err(StorageError::InvalidData(
                "Site monitor occurrence requires a terminal state".into(),
            ));
        }
        let transaction = self.connection.transaction()?;
        let (occurrence_json, monitor_json) = transaction
            .query_row(
                "SELECT o.occurrence_json, m.monitor_json FROM schedule_occurrences o
                 JOIN site_monitors m ON m.schedule_id = o.schedule_id
                 WHERE o.occurrence_id = ?1",
                [occurrence_id.to_string()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
            .ok_or_else(|| {
                StorageError::InvalidData("Site monitor occurrence was not found".into())
            })?;
        let mut occurrence: ScheduleOccurrence = serde_json::from_str(&occurrence_json)?;
        let mut monitor: SiteMonitor = serde_json::from_str(&monitor_json)?;
        if !matches!(
            occurrence.state,
            ScheduleOccurrenceState::Running | ScheduleOccurrenceState::Recovering
        ) || occurrence.lease_owner.as_deref() != Some(lease_owner)
        {
            return Err(StorageError::Conflict(
                "Site monitor occurrence lease authority changed".into(),
            ));
        }
        occurrence.state = state;
        occurrence.detail = Some(detail);
        occurrence.updated_at = now;
        occurrence.completed_at = Some(now);
        occurrence.lease_owner = None;
        occurrence.lease_expires_at = None;
        update_occurrence_tx(&transaction, &occurrence)?;
        if let Some(check) = check {
            if monitor.site_id != check.site_id {
                return Err(StorageError::InvalidData(
                    "Site monitor received health from a different Site".into(),
                ));
            }
            let observed = if check.status_code == Some(monitor.expected_status) {
                SiteHealthState::Healthy
            } else if check.status_code.is_some() {
                SiteHealthState::Degraded
            } else {
                SiteHealthState::Unavailable
            };
            let changed = monitor
                .last_state
                .is_some_and(|previous| previous != observed);
            monitor.last_state = Some(observed);
            monitor.updated_at = now;
            transaction.execute(
                "UPDATE site_monitors SET monitor_json = ?1, updated_at = ?2 WHERE site_id = ?3",
                params![
                    serde_json::to_string(&monitor)?,
                    now.to_rfc3339(),
                    monitor.site_id.to_string(),
                ],
            )?;
            if changed {
                let notification = SchedulerNotification {
                    schema_version: SCHEDULE_SCHEMA_VERSION,
                    id: SchedulerNotificationId::new(),
                    schedule_id: occurrence.schedule_id,
                    occurrence_id: Some(occurrence.id),
                    severity: if observed == SiteHealthState::Healthy {
                        NotificationSeverity::Info
                    } else {
                        NotificationSeverity::High
                    },
                    title: "Site health changed".into(),
                    detail: format!("Site {} health changed to {observed:?}", monitor.site_id),
                    read: false,
                    created_at: now,
                    read_at: None,
                };
                insert_notification(&transaction, &notification)?;
            }
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
    occurrence.lease_expires_at = Some(schedule_lease_expiry(now, lease_seconds)?);
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
    let valid_definition_hash = schedule.definition_hash.is_empty()
        || (schedule.definition_hash.len() == 64
            && schedule
                .definition_hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
    let valid_budget = schedule
        .budget
        .max_cost_usd
        .is_none_or(|value| value.is_finite() && value >= 0.0);
    let bounded_text = schedule.name.len() <= 256
        && schedule.goal.len() <= 32_768
        && schedule.constraints.len() <= 64
        && schedule
            .constraints
            .iter()
            .all(|value| value.len() <= 8_192)
        && schedule.success_conditions.len() <= 64
        && schedule
            .success_conditions
            .iter()
            .all(|value| value.len() <= 8_192)
        && schedule.requested_tools.len() <= 128
        && schedule
            .requested_tools
            .iter()
            .all(|value| !value.trim().is_empty() && value.len() <= 256)
        && schedule
            .target_path
            .as_ref()
            .is_none_or(|value| !value.trim().is_empty() && value.len() <= 4_096);
    let bounded_trigger = match &schedule.trigger {
        ScheduleTrigger::Cron { expression } => expression.len() <= 256,
        ScheduleTrigger::FileWatch { relative_path, .. } => relative_path.len() <= 4_096,
        ScheduleTrigger::Event { event_kind } => event_kind.len() <= 128,
        _ => true,
    };
    if schedule.schema_version != SCHEDULE_SCHEMA_VERSION
        || schedule.name.trim().is_empty()
        || schedule.goal.trim().is_empty()
        || !valid_definition_hash
        || !bounded_text
        || !bounded_trigger
        || schedule.context_token_budget == 0
        || !valid_budget
        || matches!(schedule.trigger, ScheduleTrigger::Interval { seconds: 0 })
        || matches!(
            schedule.trigger,
            ScheduleTrigger::FileWatch {
                poll_seconds: 0,
                ..
            }
        )
        || matches!(
            schedule.trigger,
            ScheduleTrigger::Heartbeat {
                idle_seconds: 0,
                ..
            } | ScheduleTrigger::Heartbeat {
                busy_seconds: 0,
                ..
            }
        )
        || matches!(&schedule.trigger, ScheduleTrigger::FileWatch { relative_path, .. } if !valid_relative_watch_path(relative_path))
        || matches!(&schedule.trigger, ScheduleTrigger::Event { event_kind } if mcg_events::EventKind::parse(event_kind).is_err())
        || matches!(&schedule.trigger, ScheduleTrigger::Cron { expression } if schedule.trigger.next_after(schedule.created_at, false).is_err() || expression.trim().is_empty())
        || (schedule.state == ScheduleState::Enabled
            && schedule.next_run_at.is_none()
            && !matches!(
                schedule.trigger,
                ScheduleTrigger::FileWatch { .. } | ScheduleTrigger::Event { .. }
            ))
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
    system_busy: bool,
) -> Result<(), StorageError> {
    match schedule.trigger {
        ScheduleTrigger::Once { .. } => {
            schedule.next_run_at = None;
            schedule.state = ScheduleState::Completed;
        }
        ScheduleTrigger::Interval { seconds } => {
            let seconds = i64::try_from(seconds)
                .map_err(|_| StorageError::InvalidData("schedule interval is too large".into()))?;
            let elapsed = now.signed_duration_since(due_at).num_seconds().max(0);
            let jumps = elapsed
                .checked_div(seconds)
                .and_then(|value| value.checked_add(1))
                .ok_or_else(|| StorageError::InvalidData("schedule interval overflow".into()))?;
            let advance_seconds = seconds
                .checked_mul(jumps)
                .ok_or_else(|| StorageError::InvalidData("schedule interval overflow".into()))?;
            schedule.next_run_at = Some(
                due_at
                    .checked_add_signed(Duration::seconds(advance_seconds))
                    .ok_or_else(|| {
                        StorageError::InvalidData(
                            "schedule interval exceeds the supported calendar".into(),
                        )
                    })?,
            );
        }
        ScheduleTrigger::Cron { .. } | ScheduleTrigger::Heartbeat { .. } => {
            schedule.next_run_at = schedule
                .trigger
                .next_after(now, system_busy)
                .map_err(StorageError::InvalidData)?;
        }
        ScheduleTrigger::FileWatch { .. } | ScheduleTrigger::Event { .. } => {
            schedule.next_run_at = None;
        }
    }
    schedule.updated_at = now;
    Ok(())
}

fn valid_relative_watch_path(path: &str) -> bool {
    let path = std::path::Path::new(path);
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
}

fn schedule_lease_expiry(
    now: DateTime<Utc>,
    lease_seconds: i64,
) -> Result<DateTime<Utc>, StorageError> {
    now.checked_add_signed(Duration::seconds(lease_seconds))
        .ok_or_else(|| StorageError::InvalidData("schedule lease expiry is out of range".into()))
}

fn pause_for_budget_exhaustion(
    transaction: &Transaction<'_>,
    schedule: &mut ScheduleDefinition,
    due_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<(), StorageError> {
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
    insert_occurrence(transaction, &occurrence)?;
    insert_notification(
        transaction,
        &SchedulerNotification {
            schema_version: SCHEDULE_SCHEMA_VERSION,
            id: SchedulerNotificationId::new(),
            schedule_id: schedule.id,
            occurrence_id: Some(occurrence.id),
            severity: NotificationSeverity::Warning,
            title: "Schedule paused at budget boundary".into(),
            detail: occurrence.detail.clone().unwrap_or_default(),
            read: false,
            created_at: now,
            read_at: None,
        },
    )?;
    schedule.state = ScheduleState::Paused;
    schedule.updated_at = now;
    update_schedule_tx(transaction, schedule)
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

pub(crate) fn insert_notification(
    transaction: &Transaction<'_>,
    notification: &SchedulerNotification,
) -> Result<(), StorageError> {
    transaction.execute(
        "INSERT INTO scheduler_notifications(
            notification_id, schedule_id, occurrence_id, severity, is_read,
            notification_json, created_at, read_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            notification.id.to_string(),
            notification.schedule_id.to_string(),
            notification.occurrence_id.map(|id| id.to_string()),
            notification_severity_name(notification.severity),
            i64::from(notification.read),
            serde_json::to_string(notification)?,
            notification.created_at.to_rfc3339(),
            notification.read_at.map(|value| value.to_rfc3339()),
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

const fn notification_severity_name(severity: NotificationSeverity) -> &'static str {
    match severity {
        NotificationSeverity::Info => "info",
        NotificationSeverity::Warning => "warning",
        NotificationSeverity::High => "high",
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
            definition_hash: String::new(),
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
            watch_fingerprint: None,
            last_watch_at: None,
            last_event_sequence: 0,
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
            assert!(matches!(
                store.create_schedule(&schedule),
                Err(StorageError::Conflict(_))
            ));
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
        store
            .transition_task(claim.task.id, TaskState::Active)
            .expect("activate approved task");
        store
            .transition_task(claim.task.id, TaskState::Completed)
            .expect("complete approved task");
        assert_eq!(
            store
                .reconcile_schedule_boundaries(now + Duration::seconds(1))
                .expect("reconcile approval completion"),
            1
        );
        let approval_titles = store
            .scheduler_notifications(false)
            .expect("approval notifications")
            .into_iter()
            .filter(|notification| notification.schedule_id == approval_schedule.id)
            .map(|notification| notification.title)
            .collect::<Vec<_>>();
        assert!(
            approval_titles
                .iter()
                .any(|title| title == "Schedule paused for owner approval")
        );
        assert!(
            approval_titles
                .iter()
                .any(|title| title == "Approved scheduled work completed")
        );
    }

    #[test]
    fn overdue_interval_skips_missed_ticks_and_watch_paths_are_normalized() {
        let due_at = "2000-01-01T00:00:00Z"
            .parse::<DateTime<Utc>>()
            .expect("due timestamp");
        let now = "2026-08-14T12:00:00Z"
            .parse::<DateTime<Utc>>()
            .expect("current timestamp");
        let mut store = SqliteEventStore::open_in_memory().expect("open store");
        let mut interval = interval_schedule(due_at);
        interval.trigger = ScheduleTrigger::Interval { seconds: 1 };
        store
            .create_schedule(&interval)
            .expect("create old schedule");
        let claim = store
            .claim_due_schedule(now, "daemon", 30)
            .expect("claim old schedule")
            .expect("old schedule is due");
        assert_eq!(claim.schedule.next_run_at, Some(now + Duration::seconds(1)));

        let mut invalid_watch = interval_schedule(now);
        invalid_watch.trigger = ScheduleTrigger::FileWatch {
            relative_path: "./README.md".into(),
            poll_seconds: 30,
        };
        invalid_watch.next_run_at = None;
        assert!(store.create_schedule(&invalid_watch).is_err());
        let mut invalid_hash = interval_schedule(now);
        invalid_hash.definition_hash = "not-a-server-definition-hash".into();
        assert!(store.create_schedule(&invalid_hash).is_err());
        assert!(
            store
                .wake_schedule(
                    interval.id,
                    NotificationSeverity::Info,
                    &"x".repeat(4_097),
                    now,
                )
                .is_err()
        );
    }

    #[test]
    fn file_watch_cursor_and_notifications_survive_reopen() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let database = directory.path().join("watch.db");
        let now = "2026-08-14T12:00:00Z"
            .parse::<DateTime<Utc>>()
            .expect("timestamp");
        let mut watch = interval_schedule(now);
        watch.trigger = ScheduleTrigger::FileWatch {
            relative_path: "status.txt".into(),
            poll_seconds: 30,
        };
        watch.next_run_at = None;
        let notification_id = {
            let mut store = SqliteEventStore::open(&database).expect("open store");
            store.create_schedule(&watch).expect("create watch");
            store
                .set_schedule_state(watch.id, ScheduleState::Paused, now)
                .expect("pause watch");
            assert!(
                !store
                    .record_watch_observation(watch.id, "sha256:first".into(), now)
                    .expect("baseline watch")
            );
            store
                .wake_schedule(
                    watch.id,
                    NotificationSeverity::Warning,
                    "watch subsystem warning",
                    now,
                )
                .expect("record notification")
                .notification
                .id
        };
        let mut reopened = SqliteEventStore::open(&database).expect("reopen store");
        let persisted = reopened
            .schedule(watch.id)
            .expect("read watch")
            .expect("watch exists");
        assert_eq!(persisted.watch_fingerprint.as_deref(), Some("sha256:first"));
        assert_eq!(
            reopened
                .scheduler_notifications(false)
                .expect("read notifications")[0]
                .id,
            notification_id
        );
        reopened
            .set_schedule_state(watch.id, ScheduleState::Enabled, now)
            .expect("resume watch");
        assert!(
            reopened
                .record_watch_observation(
                    watch.id,
                    "sha256:second".into(),
                    now + Duration::seconds(30),
                )
                .expect("changed watch")
        );
        assert_eq!(
            reopened
                .schedule(watch.id)
                .expect("read changed watch")
                .expect("watch exists")
                .next_run_at,
            Some(now + Duration::seconds(30))
        );
    }

    #[test]
    fn event_wake_notifications_and_adaptive_heartbeat_are_persistent() {
        let now = "2026-08-14T12:00:00Z"
            .parse::<DateTime<Utc>>()
            .expect("timestamp");
        let mut store = SqliteEventStore::open_in_memory().expect("open store");
        let mut event_schedule = interval_schedule(now);
        event_schedule.trigger = ScheduleTrigger::Event {
            event_kind: "task_created".into(),
        };
        event_schedule.next_run_at = None;
        event_schedule.last_event_sequence = store.latest_event_sequence().expect("event cursor");
        store
            .create_schedule(&event_schedule)
            .expect("create event schedule");
        let unrelated = TaskEnvelope::new("new durable event");
        store.create_task(&unrelated).expect("create event source");
        assert_eq!(
            store
                .poll_event_schedules(now)
                .expect("poll event schedules"),
            vec![event_schedule.id]
        );
        assert_eq!(
            store
                .schedule(event_schedule.id)
                .expect("schedule")
                .expect("exists")
                .next_run_at,
            Some(now)
        );
        store
            .claim_due_schedule(now, "event-daemon", 30)
            .expect("claim event wake")
            .expect("event occurrence");
        assert!(
            store
                .poll_event_schedules(now)
                .expect("ignore own occurrence event")
                .is_empty()
        );
        assert_eq!(
            store
                .schedule(event_schedule.id)
                .expect("schedule after own event")
                .expect("exists")
                .last_event_sequence,
            store
                .latest_event_sequence()
                .expect("latest event sequence")
        );

        store
            .set_schedule_state(event_schedule.id, ScheduleState::Paused, now)
            .expect("pause schedule");
        let wake = store
            .wake_schedule(
                event_schedule.id,
                NotificationSeverity::High,
                "agents disagree",
                now,
            )
            .expect("record wake");
        assert_eq!(wake.schedule.state, ScheduleState::Paused);
        let notifications = store.scheduler_notifications(false).expect("notifications");
        assert_eq!(notifications, vec![wake.notification.clone()]);
        let acknowledged = store
            .acknowledge_scheduler_notification(wake.notification.id, now)
            .expect("acknowledge");
        assert!(acknowledged.read);
        assert!(
            store
                .scheduler_notifications(false)
                .expect("unread notifications")
                .is_empty()
        );

        let mut active_task = TaskEnvelope::new("keep system busy");
        store.create_task(&active_task).expect("create busy task");
        active_task = store
            .transition_task(active_task.id, TaskState::Active)
            .expect("activate task")
            .0;
        assert_eq!(active_task.state, TaskState::Active);
        let mut heartbeat = interval_schedule(now);
        heartbeat.trigger = ScheduleTrigger::Heartbeat {
            idle_seconds: 300,
            busy_seconds: 30,
        };
        store.create_schedule(&heartbeat).expect("create heartbeat");
        let claim = store
            .claim_due_schedule(now, "heartbeat-daemon", 30)
            .expect("claim heartbeat")
            .expect("heartbeat due");
        assert_eq!(
            claim.schedule.next_run_at,
            Some(now + Duration::seconds(30))
        );
    }
}
