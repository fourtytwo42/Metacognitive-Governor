use chrono::{DateTime, Utc};
use mcg_contracts::{
    ActionEnvelope, ActionEvent, ActionExecution, ActionExecutionId, ActionExecutionState,
    ActionInspection, ActionReceipt, ActionSummary, ActionValidation, CorrelationId, EffectClass,
    EffectOutcome, EventId, ReconciliationJob, ReconciliationJobId, ReconciliationJobState,
    ReconciliationObservation, RuntimeActorId, StableTypeId, VersionedPayload, WorkflowAttempt,
    WorkflowState, decode_canonical_cbor_v1,
};
use rusqlite::{OptionalExtension, Transaction, params};
use std::collections::BTreeSet;

use crate::{SqliteEventStore, StorageError};

const ACTION_AUTHORIZED_TYPE_ID: &str = "mcg.action_authorized.v1";
const ACTION_DISPATCHED_TYPE_ID: &str = "mcg.action_dispatched.v1";
const ACTION_OBSERVED_TYPE_ID: &str = "mcg.action_observed.v1";
const ACTION_CLASSIFIED_TYPE_ID: &str = "mcg.action_classified.v1";
const ACTION_CANCEL_FENCED_TYPE_ID: &str = "mcg.action_cancel_fenced.v1";
const ACTION_VALIDATED_TYPE_ID: &str = "mcg.action_validated.v1";
const ACTION_COMMITTED_TYPE_ID: &str = "mcg.action_committed.v1";
const ACTION_COMPENSATED_TYPE_ID: &str = "mcg.action_compensated.v1";
const ACTION_EXECUTION_RECOVERED_TYPE_ID: &str = "mcg.action_execution_recovered.v1";
const ACTION_ABANDONED_TYPE_ID: &str = "mcg.action_abandoned_before_dispatch.v1";
const RECONCILIATION_PROBE_BINDING_TYPE_ID: &str = "mcg.reconciliation_probe_binding.v1";
const RECONCILIATION_CLAIMED_TYPE_ID: &str = "mcg.reconciliation_claimed.v1";
const RECONCILIATION_RESOLVED_TYPE_ID: &str = "mcg.reconciliation_resolved.v1";
const RECONCILIATION_DEFERRED_TYPE_ID: &str = "mcg.reconciliation_deferred.v1";
const RECONCILIATION_ESCALATED_TYPE_ID: &str = "mcg.reconciliation_escalated.v1";
const RECONCILIATION_LEASE_RECOVERED_TYPE_ID: &str = "mcg.reconciliation_lease_recovered.v1";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub(super) struct CancellationEffectReport {
    pub cancelled_before_dispatch: u64,
    pub observational_stopped: u64,
    pub marked_outcome_unknown: u64,
    pub unresolved: u64,
    pub reconciliation_required: u64,
    pub quarantined: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionAuthorizationReceipt {
    pub envelope: ActionEnvelope,
    pub execution: ActionExecution,
    pub event: ActionEvent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionCommitReceipt {
    pub execution: ActionExecution,
    pub event: ActionEvent,
    pub compensated_execution: Option<ActionExecution>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconciliationClaimReceipt {
    pub job: ReconciliationJob,
    pub execution: ActionExecution,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconciliationResolutionReceipt {
    pub job: ReconciliationJob,
    pub execution: ActionExecution,
    pub receipt: Option<ActionReceipt>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReconciliationRecoveryReport {
    pub inspected_running: u64,
    pub requeued: u64,
    pub escalated: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ActionExecutionRecoveryReport {
    pub inspected: u64,
    pub no_effect_confirmed: u64,
    pub outcome_unknown: u64,
    pub escalated: u64,
}

impl SqliteEventStore {
    /// Atomically validates and persists exact action intent plus its initial authorization.
    ///
    /// The calling runtime actor must own the current workflow generation. The immutable
    /// operation hash, authority, budget reservation, effect class, and reconciler requirement
    /// are validated before the authorization transaction commits.
    ///
    /// # Errors
    /// Returns a storage error for stale workflow ownership, invalid intent, replay
    /// substitution, or transaction failure.
    #[allow(clippy::too_many_arguments)]
    pub fn authorize_action(
        &mut self,
        envelope: &ActionEnvelope,
        execution: &ActionExecution,
        expected_workflow_revision: u64,
        expected_workflow_generation: u64,
        correlation_id: CorrelationId,
        now: DateTime<Utc>,
    ) -> Result<ActionAuthorizationReceipt, StorageError> {
        validate_action_authorization(envelope, execution, now)?;
        let transaction = self.connection.transaction()?;
        if let Some(existing_envelope) = load_envelope_by_idempotency(
            &transaction,
            envelope.workflow_id,
            &envelope.idempotency_key,
        )? {
            let existing_execution = load_initial_execution(&transaction, envelope.id)?
                .ok_or_else(|| {
                    StorageError::InvalidData(
                        "action authorization exists without its initial execution".into(),
                    )
                })?;
            let event = load_action_event_by_idempotency(
                &transaction,
                existing_envelope.id,
                &envelope.idempotency_key,
            )?
            .ok_or_else(|| {
                StorageError::InvalidData(
                    "action authorization exists without its immutable event".into(),
                )
            })?;
            if existing_envelope != *envelope
                || existing_execution != *execution
                || event.type_id.as_str() != ACTION_AUTHORIZED_TYPE_ID
            {
                return Err(StorageError::Conflict(
                    "action authorization idempotency key was replayed with different intent"
                        .into(),
                ));
            }
            transaction.commit()?;
            return Ok(ActionAuthorizationReceipt {
                envelope: existing_envelope,
                execution: existing_execution,
                event,
            });
        }

        let workflow = load_workflow_attempt(&transaction, envelope.attempt_id)?
            .ok_or(StorageError::WorkflowAttemptNotFound(envelope.attempt_id))?;
        if workflow.workflow_id != envelope.workflow_id
            || workflow.task_contract != envelope.task_contract
            || workflow.revision != expected_workflow_revision
            || workflow.lease_generation != expected_workflow_generation
            || workflow.lease_owner != Some(envelope.actor)
            || !matches!(
                workflow.state,
                WorkflowState::Running | WorkflowState::Verifying
            )
        {
            return Err(StorageError::Conflict(
                "action authorization does not bind the current workflow owner and generation"
                    .into(),
            ));
        }
        validate_compensation_authorization(&transaction, envelope)?;

        insert_action_envelope(&transaction, envelope)?;
        insert_action_execution(&transaction, execution)?;
        let event = action_event(
            envelope.id,
            Some(execution.id),
            1,
            correlation_id,
            None,
            &envelope.idempotency_key,
            ACTION_AUTHORIZED_TYPE_ID,
            now,
            &serde_json::json!({
                "envelope": envelope,
                "execution": execution,
                "workflow_revision": expected_workflow_revision,
                "workflow_generation": expected_workflow_generation,
            }),
        )?;
        insert_action_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(ActionAuthorizationReceipt {
            envelope: envelope.clone(),
            execution: execution.clone(),
            event,
        })
    }

    /// Atomically authorizes one observational-only probe for an exact running reconciliation
    /// job while the owning workflow remains fenced in `Waiting(Reconciliation)`.
    ///
    /// This path cannot authorize the original effect operation. The probe must contain exactly
    /// the immutable reconciliation adapter and bind the current job/action/execution generation
    /// in its sole precondition.
    ///
    /// # Errors
    /// Returns a storage error for stale job or workflow state, substituted probe intent,
    /// non-observational authority, replay substitution, or transaction failure.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub fn authorize_reconciliation_probe_action(
        &mut self,
        envelope: &ActionEnvelope,
        execution: &ActionExecution,
        job_id: ReconciliationJobId,
        expected_job_revision: u64,
        expected_job_generation: u64,
        expected_target_revision: u64,
        expected_target_generation: u64,
        correlation_id: CorrelationId,
        now: DateTime<Utc>,
    ) -> Result<ActionAuthorizationReceipt, StorageError> {
        validate_action_authorization(envelope, execution, now)?;
        let transaction = self.connection.transaction()?;
        let existing = if let Some(existing_envelope) = load_envelope_by_idempotency(
            &transaction,
            envelope.workflow_id,
            &envelope.idempotency_key,
        )? {
            let existing_execution = load_initial_execution(&transaction, envelope.id)?
                .ok_or_else(|| {
                    StorageError::InvalidData(
                        "reconciliation probe exists without its initial execution".into(),
                    )
                })?;
            let event = load_action_event_by_idempotency(
                &transaction,
                existing_envelope.id,
                &envelope.idempotency_key,
            )?
            .ok_or_else(|| {
                StorageError::InvalidData(
                    "reconciliation probe exists without its immutable event".into(),
                )
            })?;
            if existing_envelope != *envelope
                || existing_execution != *execution
                || event.type_id.as_str() != ACTION_AUTHORIZED_TYPE_ID
            {
                return Err(StorageError::Conflict(
                    "reconciliation probe idempotency key was replayed with different intent"
                        .into(),
                ));
            }
            Some((existing_envelope, existing_execution, event))
        } else {
            None
        };

        let job = load_reconciliation_job(&transaction, job_id)?.ok_or_else(|| {
            StorageError::Conflict("reconciliation probe job was not found".into())
        })?;
        let target = load_action_execution(&transaction, job.execution_id)?.ok_or_else(|| {
            StorageError::InvalidData("reconciliation probe target disappeared".into())
        })?;
        let target_envelope =
            load_action_envelope(&transaction, job.action_id)?.ok_or_else(|| {
                StorageError::InvalidData("reconciliation probe target intent disappeared".into())
            })?;
        let workflow = load_workflow_attempt(&transaction, envelope.attempt_id)?
            .ok_or(StorageError::WorkflowAttemptNotFound(envelope.attempt_id))?;
        let binding = VersionedPayload::canonical_v1(
            StableTypeId::parse(RECONCILIATION_PROBE_BINDING_TYPE_ID)
                .map_err(|error| StorageError::InvalidData(error.to_string()))?,
            &serde_json::json!({
                "job_id": job.id,
                "job_revision": job.revision,
                "job_generation": job.lease_generation,
                "target_action_id": job.action_id,
                "target_execution_id": job.execution_id,
                "target_revision": target.revision,
                "target_generation": target.lease_generation,
                "adapter_hash": envelope.operation_hash,
            }),
        )
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        if job.revision != expected_job_revision
            || job.lease_generation != expected_job_generation
            || job.state != ReconciliationJobState::Running
            || job.lease_owner != Some(envelope.actor)
            || job.lease_expires_at.is_none_or(|expiry| expiry <= now)
            || target.revision != expected_target_revision
            || target.lease_generation != expected_target_generation
            || target.state != ActionExecutionState::Reconciling
            || target.executor != Some(envelope.actor)
            || target.action_id != target_envelope.id
            || target_envelope.workflow_id != envelope.workflow_id
            || target_envelope.attempt_id != envelope.attempt_id
            || target_envelope.task_contract != envelope.task_contract
            || workflow.workflow_id != envelope.workflow_id
            || workflow.task_contract != envelope.task_contract
            || workflow.state != WorkflowState::Waiting(mcg_contracts::WaitReason::Reconciliation)
            || workflow.lease_owner.is_some()
            || workflow.lease_expires_at.is_some()
            || envelope.effect_class != EffectClass::Observational
            || envelope.operation != job.adapter
            || envelope.preconditions.as_slice() != [binding]
            || envelope.compensation.is_some()
            || envelope.compensates_action.is_some()
            || envelope.reconciler.is_some()
            || envelope.reconciliation_deadline.is_some()
        {
            return Err(StorageError::Conflict(
                "reconciliation probe does not bind the exact fenced workflow and running job"
                    .into(),
            ));
        }

        if let Some((existing_envelope, existing_execution, event)) = existing {
            transaction.commit()?;
            return Ok(ActionAuthorizationReceipt {
                envelope: existing_envelope,
                execution: existing_execution,
                event,
            });
        }

        insert_action_envelope(&transaction, envelope)?;
        insert_action_execution(&transaction, execution)?;
        let event = action_event(
            envelope.id,
            Some(execution.id),
            1,
            correlation_id,
            None,
            &envelope.idempotency_key,
            ACTION_AUTHORIZED_TYPE_ID,
            now,
            &serde_json::json!({
                "envelope": envelope,
                "execution": execution,
                "reconciliation_job": job,
                "target_execution": target,
            }),
        )?;
        insert_action_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(ActionAuthorizationReceipt {
            envelope: envelope.clone(),
            execution: execution.clone(),
            event,
        })
    }

    /// Loads immutable action intent.
    ///
    /// # Errors
    /// Returns a storage error for query or stored-data failures.
    pub fn action_envelope(
        &self,
        action_id: mcg_contracts::ActionId,
    ) -> Result<Option<ActionEnvelope>, StorageError> {
        self.connection
            .query_row(
                "SELECT envelope_json FROM action_envelopes WHERE action_id = ?1",
                [action_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
            .transpose()
    }

    /// Loads one mutable action execution projection.
    ///
    /// # Errors
    /// Returns a storage error for query or stored-data failures.
    pub fn action_execution(
        &self,
        execution_id: ActionExecutionId,
    ) -> Result<Option<ActionExecution>, StorageError> {
        load_action_execution(&self.connection, execution_id)
    }

    /// Loads the initial execution projection for one immutable action identity.
    ///
    /// # Errors
    /// Returns a storage error for query or stored-data failures.
    pub fn action_execution_for_action(
        &self,
        action_id: mcg_contracts::ActionId,
    ) -> Result<Option<ActionExecution>, StorageError> {
        self.connection
            .query_row(
                "SELECT execution_json FROM action_executions
                 WHERE action_id = ?1 ORDER BY attempt_number LIMIT 1",
                [action_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
            .transpose()
    }

    /// Returns immutable ordered action history.
    ///
    /// # Errors
    /// Returns a storage error for query or stored-data failures.
    pub fn action_events(
        &self,
        action_id: mcg_contracts::ActionId,
    ) -> Result<Vec<ActionEvent>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT event_json FROM action_events WHERE action_id = ?1 ORDER BY sequence",
        )?;
        let rows = statement.query_map([action_id.to_string()], |row| row.get::<_, String>(0))?;
        rows.map(|row| {
            let json = row?;
            serde_json::from_str(&json).map_err(StorageError::from)
        })
        .collect()
    }

    /// Returns a bounded newest-first projection of canonical action executions.
    ///
    /// # Errors
    /// Returns a storage error for an invalid limit, query failure, or malformed stored state.
    pub fn action_summaries(&self, limit: usize) -> Result<Vec<ActionSummary>, StorageError> {
        if limit == 0 || limit > 1_000 {
            return Err(StorageError::InvalidData(
                "action summary limit must be between 1 and 1000".into(),
            ));
        }
        let mut statement = self.connection.prepare(
            "SELECT envelope.envelope_json, execution.execution_json
             FROM action_executions AS execution
             JOIN action_envelopes AS envelope ON envelope.action_id = execution.action_id
             ORDER BY execution.updated_at DESC, execution.action_id, execution.attempt_number DESC
             LIMIT ?1",
        )?;
        let rows = statement.query_map(
            [i64::try_from(limit).map_err(|_| {
                StorageError::InvalidData("action summary limit exceeds SQLite range".into())
            })?],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )?;
        rows.map(|row| {
            let (envelope, execution) = row?;
            let envelope: ActionEnvelope = serde_json::from_str(&envelope)?;
            let execution: ActionExecution = serde_json::from_str(&execution)?;
            Ok(ActionSummary {
                action_id: envelope.id,
                execution_id: execution.id,
                workflow_id: envelope.workflow_id,
                attempt_id: envelope.attempt_id,
                actor: envelope.actor,
                effect_class: envelope.effect_class,
                state: execution.state,
                outcome: execution.outcome,
                revision: execution.revision,
                lease_generation: execution.lease_generation,
                updated_at: execution.updated_at,
            })
        })
        .collect()
    }

    /// Loads one bounded canonical action inspection with ordered immutable evidence.
    ///
    /// The newest `event_limit` events are returned in ascending sequence order. Execution,
    /// receipt, and reconciliation collections are independently capped at 1,000 entries.
    ///
    /// # Errors
    /// Returns a storage error for invalid bounds, query failures, or malformed stored state.
    pub fn action_inspection(
        &self,
        action_id: mcg_contracts::ActionId,
        event_limit: usize,
    ) -> Result<Option<ActionInspection>, StorageError> {
        if event_limit == 0 || event_limit > 1_000 {
            return Err(StorageError::InvalidData(
                "action event limit must be between 1 and 1000".into(),
            ));
        }
        let Some(envelope) = self.action_envelope(action_id)? else {
            return Ok(None);
        };
        let mut compensation_statement = self.connection.prepare(
            "SELECT action_id FROM action_envelopes WHERE compensates_action_id = ?1
             ORDER BY created_at, action_id LIMIT 1001",
        )?;
        let compensation_rows = compensation_statement
            .query_map([action_id.to_string()], |row| row.get::<_, String>(0))?;
        let compensation_actions = compensation_rows
            .map(|row| {
                serde_json::from_value(serde_json::Value::String(row?)).map_err(StorageError::from)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if compensation_actions.len() > 1_000 {
            return Err(StorageError::InvalidData(
                "action compensations exceed the bounded inspection limit".into(),
            ));
        }
        let executions = query_action_collection::<ActionExecution>(
            &self.connection,
            "SELECT execution_json FROM action_executions WHERE action_id = ?1
             ORDER BY attempt_number, execution_id LIMIT 1001",
            action_id,
            "action executions",
        )?;
        let receipts = query_action_collection::<ActionReceipt>(
            &self.connection,
            "SELECT receipt_json FROM action_receipts WHERE action_id = ?1
             ORDER BY observed_at, receipt_id LIMIT 1001",
            action_id,
            "action receipts",
        )?;
        let reconciliation_jobs = query_action_collection::<ReconciliationJob>(
            &self.connection,
            "SELECT job_json FROM reconciliation_jobs WHERE action_id = ?1
             ORDER BY created_at, reconciliation_job_id LIMIT 1001",
            action_id,
            "action reconciliation jobs",
        )?;
        let event_limit = i64::try_from(event_limit).map_err(|_| {
            StorageError::InvalidData("action event limit exceeds SQLite range".into())
        })?;
        let mut statement = self.connection.prepare(
            "SELECT event_json FROM action_events WHERE action_id = ?1
             ORDER BY sequence DESC LIMIT ?2",
        )?;
        let rows = statement.query_map(params![action_id.to_string(), event_limit], |row| {
            row.get::<_, String>(0)
        })?;
        let mut events = rows
            .map(|row| serde_json::from_str::<ActionEvent>(&row?).map_err(StorageError::from))
            .collect::<Result<Vec<_>, _>>()?;
        events.reverse();
        Ok(Some(ActionInspection {
            envelope,
            compensation_actions,
            executions,
            receipts,
            reconciliation_jobs,
            events,
        }))
    }

    /// Dispatches an authorized physical execution under a new generation fence.
    ///
    /// # Errors
    /// Returns a storage error for stale execution state, replay substitution, or transaction
    /// failure.
    #[allow(clippy::too_many_arguments)]
    pub fn dispatch_action(
        &mut self,
        execution_id: ActionExecutionId,
        expected_revision: u64,
        expected_generation: u64,
        executor: RuntimeActorId,
        correlation_id: CorrelationId,
        idempotency_key: &str,
        now: DateTime<Utc>,
    ) -> Result<ActionExecution, StorageError> {
        mutate_action_execution(
            &mut self.connection,
            execution_id,
            expected_revision,
            expected_generation,
            correlation_id,
            idempotency_key,
            ACTION_DISPATCHED_TYPE_ID,
            now,
            |original| original.executor == Some(executor),
            |execution, _envelope| {
                if execution.state != ActionExecutionState::Authorized {
                    return Err(StorageError::Conflict(
                        "only an authorized action can be dispatched".into(),
                    ));
                }
                execution.state = ActionExecutionState::Executing;
                execution.revision = increment(execution.revision, "action revision")?;
                execution.lease_generation =
                    increment(execution.lease_generation, "action generation")?;
                execution.executor = Some(executor);
                execution.updated_at = now;
                Ok(serde_json::json!({"executor": executor}))
            },
        )
    }

    /// Settles any authorized action as proven no-effect before physical dispatch.
    ///
    /// Authorization alone cannot cause an external effect. This retires a reconciliation probe
    /// whose lease was fenced or an effectful action whose executor/permit preparation failed.
    /// Already-dispatched actions are never eligible.
    ///
    /// # Errors
    /// Returns a storage error for stale state, replay substitution, or commit failure.
    #[allow(clippy::too_many_arguments)]
    pub fn abandon_authorized_action_before_dispatch(
        &mut self,
        execution_id: ActionExecutionId,
        expected_revision: u64,
        expected_generation: u64,
        correlation_id: CorrelationId,
        idempotency_key: &str,
        now: DateTime<Utc>,
    ) -> Result<ActionExecution, StorageError> {
        mutate_action_execution(
            &mut self.connection,
            execution_id,
            expected_revision,
            expected_generation,
            correlation_id,
            idempotency_key,
            ACTION_ABANDONED_TYPE_ID,
            now,
            |original| {
                original.state == ActionExecutionState::NoEffectConfirmed
                    && original.outcome == Some(EffectOutcome::NoEffectConfirmed)
            },
            |execution, _envelope| {
                if execution.state != ActionExecutionState::Authorized {
                    return Err(StorageError::Conflict(
                        "only an undispatched authorized action can be abandoned".into(),
                    ));
                }
                execution.state = ActionExecutionState::NoEffectConfirmed;
                execution.revision = increment(execution.revision, "action revision")?;
                execution.lease_generation =
                    increment(execution.lease_generation, "action generation")?;
                execution.executor = None;
                execution.outcome = Some(EffectOutcome::NoEffectConfirmed);
                execution.updated_at = now;
                Ok(serde_json::json!({"reason": "action was abandoned before physical dispatch"}))
            },
        )
    }

    /// Persists one immutable physical observation before Root classifies the effect.
    ///
    /// # Errors
    /// Returns a storage error for stale execution state, mismatched receipt identity, replay
    /// substitution, or transaction failure.
    #[allow(clippy::too_many_arguments)]
    pub fn observe_action(
        &mut self,
        receipt: &ActionReceipt,
        expected_revision: u64,
        expected_generation: u64,
        correlation_id: CorrelationId,
        idempotency_key: &str,
        now: DateTime<Utc>,
    ) -> Result<ActionExecution, StorageError> {
        if idempotency_key.trim().is_empty() {
            return Err(StorageError::InvalidData(
                "action observation idempotency key cannot be empty".into(),
            ));
        }
        let transaction = self.connection.transaction()?;
        let mut execution = load_action_execution(&transaction, receipt.execution_id)?
            .ok_or_else(|| StorageError::Conflict("action execution was not found".into()))?;
        if let Some(event) =
            load_action_event_by_idempotency(&transaction, execution.action_id, idempotency_key)?
        {
            let original = action_event_execution(&event)?;
            let original_receipt =
                load_action_receipt(&transaction, receipt.id)?.ok_or_else(|| {
                    StorageError::InvalidData("observation event has no receipt".into())
                })?;
            if event.type_id.as_str() != ACTION_OBSERVED_TYPE_ID || original_receipt != *receipt {
                return Err(StorageError::Conflict(
                    "action observation idempotency key was replayed with different evidence"
                        .into(),
                ));
            }
            transaction.commit()?;
            return Ok(original);
        }
        ensure_action_expected(&execution, expected_revision, expected_generation)?;
        if execution.state != ActionExecutionState::Executing
            || receipt.action_id != execution.action_id
            || receipt.execution_id != execution.id
        {
            return Err(StorageError::Conflict(
                "action receipt does not bind the current executing attempt".into(),
            ));
        }
        decode_canonical_cbor_v1(&receipt.observation.bytes)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        insert_action_receipt(&transaction, receipt)?;
        execution.state = ActionExecutionState::Observed;
        execution.revision = increment(execution.revision, "action revision")?;
        execution.updated_at = now;
        update_action_execution_cas(
            &transaction,
            &execution,
            expected_revision,
            expected_generation,
        )?;
        let (sequence, cause) = next_action_event_position(&transaction, execution.action_id)?;
        let event = action_event(
            execution.action_id,
            Some(execution.id),
            sequence,
            correlation_id,
            cause,
            idempotency_key,
            ACTION_OBSERVED_TYPE_ID,
            now,
            &serde_json::json!({"receipt": receipt, "execution": execution}),
        )?;
        insert_action_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(execution)
    }

    /// Classifies an observed effect, or marks a dispatched effect unknown after lost evidence.
    ///
    /// `OutcomeUnknown` atomically creates a durable reconciliation job and never redispatches
    /// the external operation.
    ///
    /// # Errors
    /// Returns a storage error for invalid classification, missing reconciler, stale state,
    /// replay substitution, or transaction failure.
    #[allow(
        clippy::too_many_arguments,
        clippy::too_many_lines,
        clippy::needless_pass_by_value
    )]
    pub fn classify_action_outcome(
        &mut self,
        execution_id: ActionExecutionId,
        expected_revision: u64,
        expected_generation: u64,
        outcome: EffectOutcome,
        reconciliation_deadline: Option<DateTime<Utc>>,
        correlation_id: CorrelationId,
        idempotency_key: &str,
        now: DateTime<Utc>,
    ) -> Result<(ActionExecution, Option<ReconciliationJob>), StorageError> {
        if idempotency_key.trim().is_empty() {
            return Err(StorageError::InvalidData(
                "action classification idempotency key cannot be empty".into(),
            ));
        }
        let transaction = self.connection.transaction()?;
        let mut execution = load_action_execution(&transaction, execution_id)?
            .ok_or_else(|| StorageError::Conflict("action execution was not found".into()))?;
        if let Some(event) =
            load_action_event_by_idempotency(&transaction, execution.action_id, idempotency_key)?
        {
            let original = action_event_execution(&event)?;
            if event.type_id.as_str() != ACTION_CLASSIFIED_TYPE_ID
                || original.outcome.as_ref() != Some(&outcome)
            {
                return Err(StorageError::Conflict(
                    "action classification idempotency key was replayed with another outcome"
                        .into(),
                ));
            }
            let job = load_reconciliation_job_for_execution(&transaction, execution_id)?;
            if job.as_ref().map(|candidate| candidate.deadline) != reconciliation_deadline {
                return Err(StorageError::Conflict(
                    "action classification replay changed its reconciliation deadline".into(),
                ));
            }
            transaction.commit()?;
            return Ok((original, job));
        }
        ensure_action_expected(&execution, expected_revision, expected_generation)?;
        let envelope = load_action_envelope(&transaction, execution.action_id)?
            .ok_or_else(|| StorageError::InvalidData("action execution has no envelope".into()))?;
        let next_state = match &outcome {
            EffectOutcome::NoEffectConfirmed => ActionExecutionState::NoEffectConfirmed,
            EffectOutcome::EffectConfirmed { .. } => ActionExecutionState::EffectConfirmed,
            EffectOutcome::OutcomeUnknown { .. } => ActionExecutionState::OutcomeUnknown,
        };
        let classification_allowed = match next_state {
            ActionExecutionState::OutcomeUnknown => matches!(
                execution.state,
                ActionExecutionState::Executing | ActionExecutionState::Observed
            ),
            _ => execution.state == ActionExecutionState::Observed,
        };
        if !classification_allowed {
            return Err(StorageError::Conflict(
                "effect classification is not legal from the current execution state".into(),
            ));
        }
        execution.state = next_state;
        execution.revision = increment(execution.revision, "action revision")?;
        execution.lease_generation = increment(execution.lease_generation, "action generation")?;
        execution.executor = None;
        execution.outcome = Some(outcome.clone());
        execution.updated_at = now;
        update_action_execution_cas(
            &transaction,
            &execution,
            expected_revision,
            expected_generation,
        )?;

        let job = if let EffectOutcome::OutcomeUnknown { hint } = &outcome {
            let adapter = envelope.reconciler.clone().ok_or_else(|| {
                StorageError::Conflict(
                    "an unknown effect cannot be classified without its bound reconciler".into(),
                )
            })?;
            let deadline = reconciliation_deadline.ok_or_else(|| {
                StorageError::InvalidData(
                    "an unknown effect requires a reconciliation deadline".into(),
                )
            })?;
            if envelope.reconciliation_deadline != Some(deadline) {
                return Err(StorageError::Conflict(
                    "reconciliation deadline does not match authorized action intent".into(),
                ));
            }
            if deadline <= now {
                return Err(StorageError::InvalidData(
                    "reconciliation deadline must be in the future".into(),
                ));
            }
            let job = ReconciliationJob {
                id: ReconciliationJobId::new(),
                action_id: envelope.id,
                execution_id,
                adapter,
                state: ReconciliationJobState::Pending,
                revision: 1,
                lease_generation: 0,
                lease_owner: None,
                lease_expires_at: None,
                next_attempt_at: now,
                attempts: 0,
                deadline,
                last_hint: Some(hint.clone()),
                created_at: now,
                updated_at: now,
            };
            insert_reconciliation_job(&transaction, &job)?;
            Some(job)
        } else {
            if reconciliation_deadline.is_some() {
                return Err(StorageError::InvalidData(
                    "definite effect outcomes cannot create reconciliation work".into(),
                ));
            }
            None
        };
        let (sequence, cause) = next_action_event_position(&transaction, execution.action_id)?;
        let event = action_event(
            execution.action_id,
            Some(execution.id),
            sequence,
            correlation_id,
            cause,
            idempotency_key,
            ACTION_CLASSIFIED_TYPE_ID,
            now,
            &serde_json::json!({"outcome": outcome, "execution": execution, "reconciliation_job": job}),
        )?;
        insert_action_event(&transaction, &event)?;
        transaction.commit()?;
        Ok((execution, job))
    }

    /// Persists exact post-effect validation evidence before authoritative commit.
    ///
    /// # Errors
    /// Returns a conflict for stale state or substituted evidence, and an invalid-data error for
    /// malformed validation postconditions.
    #[allow(clippy::too_many_arguments)]
    pub fn validate_action_effect(
        &mut self,
        execution_id: ActionExecutionId,
        expected_revision: u64,
        expected_generation: u64,
        validation: &ActionValidation,
        correlation_id: CorrelationId,
        idempotency_key: &str,
        now: DateTime<Utc>,
    ) -> Result<ActionExecution, StorageError> {
        if idempotency_key.trim().is_empty() {
            return Err(StorageError::InvalidData(
                "action validation idempotency key cannot be empty".into(),
            ));
        }
        let transaction = self.connection.transaction()?;
        let mut execution = load_action_execution(&transaction, execution_id)?
            .ok_or_else(|| StorageError::Conflict("action execution was not found".into()))?;
        if let Some(event) =
            load_action_event_by_idempotency(&transaction, execution.action_id, idempotency_key)?
        {
            let original_execution = action_event_execution(&event)?;
            let original_validation = action_event_validation(&event)?;
            if event.type_id.as_str() != ACTION_VALIDATED_TYPE_ID
                || original_validation != *validation
            {
                return Err(StorageError::Conflict(
                    "action validation idempotency key was replayed with different evidence".into(),
                ));
            }
            transaction.commit()?;
            return Ok(original_execution);
        }
        ensure_action_expected(&execution, expected_revision, expected_generation)?;
        if execution.state != ActionExecutionState::EffectConfirmed
            || !matches!(
                execution.outcome,
                Some(EffectOutcome::EffectConfirmed { .. })
            )
            || validation.action_id != execution.action_id
            || validation.execution_id != execution.id
            || validation.validated_at != now
        {
            return Err(StorageError::Conflict(
                "validation does not bind the current confirmed action effect".into(),
            ));
        }
        for postcondition in &validation.postconditions {
            decode_canonical_cbor_v1(&postcondition.bytes)
                .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        }
        execution.state = ActionExecutionState::Validated;
        execution.revision = increment(execution.revision, "action revision")?;
        execution.lease_generation = increment(execution.lease_generation, "action generation")?;
        execution.executor = None;
        execution.updated_at = now;
        update_action_execution_cas(
            &transaction,
            &execution,
            expected_revision,
            expected_generation,
        )?;
        let (sequence, cause) = next_action_event_position(&transaction, execution.action_id)?;
        let event = action_event(
            execution.action_id,
            Some(execution.id),
            sequence,
            correlation_id,
            cause,
            idempotency_key,
            ACTION_VALIDATED_TYPE_ID,
            now,
            &serde_json::json!({"validation": validation, "execution": execution}),
        )?;
        insert_action_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(execution)
    }

    /// Commits a validated effect. A compensation action atomically marks its original action
    /// compensated only after the new causal action itself commits.
    ///
    /// # Errors
    /// Returns a conflict for stale state, a substituted validation event, or an original action
    /// that is no longer committed.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub fn commit_action_effect(
        &mut self,
        execution_id: ActionExecutionId,
        expected_revision: u64,
        expected_generation: u64,
        expected_validation_event: EventId,
        correlation_id: CorrelationId,
        idempotency_key: &str,
        now: DateTime<Utc>,
    ) -> Result<ActionCommitReceipt, StorageError> {
        if idempotency_key.trim().is_empty() {
            return Err(StorageError::InvalidData(
                "action commit idempotency key cannot be empty".into(),
            ));
        }
        let transaction = self.connection.transaction()?;
        let mut execution = load_action_execution(&transaction, execution_id)?
            .ok_or_else(|| StorageError::Conflict("action execution was not found".into()))?;
        if let Some(event) =
            load_action_event_by_idempotency(&transaction, execution.action_id, idempotency_key)?
        {
            let payload = action_event_payload_value(&event)?;
            let original_execution = action_event_execution(&event)?;
            let validation_event: EventId =
                serde_json::from_value(payload.get("validation_event").cloned().ok_or_else(
                    || StorageError::InvalidData("commit has no validation".into()),
                )?)?;
            if event.type_id.as_str() != ACTION_COMMITTED_TYPE_ID
                || validation_event != expected_validation_event
            {
                return Err(StorageError::Conflict(
                    "action commit idempotency key was replayed for another validation".into(),
                ));
            }
            let compensated_execution = payload
                .get("compensated_execution")
                .filter(|value| !value.is_null())
                .cloned()
                .map(serde_json::from_value)
                .transpose()?;
            transaction.commit()?;
            return Ok(ActionCommitReceipt {
                execution: original_execution,
                event,
                compensated_execution,
            });
        }
        ensure_action_expected(&execution, expected_revision, expected_generation)?;
        if execution.state != ActionExecutionState::Validated {
            return Err(StorageError::Conflict(
                "only a validated action effect can be committed".into(),
            ));
        }
        let validation_event = load_action_event(&transaction, expected_validation_event)?
            .ok_or_else(|| StorageError::Conflict("validation event was not found".into()))?;
        if validation_event.action_id != execution.action_id
            || validation_event.execution_id != Some(execution.id)
            || validation_event.type_id.as_str() != ACTION_VALIDATED_TYPE_ID
        {
            return Err(StorageError::Conflict(
                "commit does not bind the exact validation event".into(),
            ));
        }
        let envelope = load_action_envelope(&transaction, execution.action_id)?
            .ok_or_else(|| StorageError::InvalidData("action execution has no envelope".into()))?;
        execution.state = ActionExecutionState::Committed;
        execution.revision = increment(execution.revision, "action revision")?;
        execution.lease_generation = increment(execution.lease_generation, "action generation")?;
        execution.executor = None;
        execution.updated_at = now;
        update_action_execution_cas(
            &transaction,
            &execution,
            expected_revision,
            expected_generation,
        )?;

        let compensated_execution = if let Some(original_action_id) = envelope.compensates_action {
            let mut original = load_initial_execution(&transaction, original_action_id)?
                .ok_or_else(|| StorageError::InvalidData("compensated action is missing".into()))?;
            if original.state != ActionExecutionState::Committed {
                return Err(StorageError::Conflict(
                    "original action is no longer eligible for compensation".into(),
                ));
            }
            let original_revision = original.revision;
            let original_generation = original.lease_generation;
            original.state = ActionExecutionState::Compensated;
            original.revision = increment(original.revision, "action revision")?;
            original.lease_generation = increment(original.lease_generation, "action generation")?;
            original.executor = None;
            original.updated_at = now;
            update_action_execution_cas(
                &transaction,
                &original,
                original_revision,
                original_generation,
            )?;
            Some(original)
        } else {
            None
        };

        let (sequence, _cause) = next_action_event_position(&transaction, execution.action_id)?;
        let event = action_event(
            execution.action_id,
            Some(execution.id),
            sequence,
            correlation_id,
            Some(expected_validation_event),
            idempotency_key,
            ACTION_COMMITTED_TYPE_ID,
            now,
            &serde_json::json!({
                "validation_event": expected_validation_event,
                "execution": execution,
                "compensated_execution": compensated_execution,
            }),
        )?;
        insert_action_event(&transaction, &event)?;
        if let Some(original) = &compensated_execution {
            let (sequence, _) = next_action_event_position(&transaction, original.action_id)?;
            let compensation_event = action_event(
                original.action_id,
                Some(original.id),
                sequence,
                correlation_id,
                Some(event.id),
                &format!("compensated-by:{}", execution.action_id),
                ACTION_COMPENSATED_TYPE_ID,
                now,
                &serde_json::json!({
                    "compensation_action": execution.action_id,
                    "compensation_event": event.id,
                    "execution": original,
                }),
            )?;
            insert_action_event(&transaction, &compensation_event)?;
        }
        transaction.commit()?;
        Ok(ActionCommitReceipt {
            execution,
            event,
            compensated_execution,
        })
    }

    /// Loads one durable reconciliation job.
    ///
    /// # Errors
    /// Returns a storage error for query or stored-data failures.
    pub fn reconciliation_job(
        &self,
        job_id: ReconciliationJobId,
    ) -> Result<Option<ReconciliationJob>, StorageError> {
        load_reconciliation_job(&self.connection, job_id)
    }

    /// Returns bounded, ordered reconciliation work that is due and still within policy time.
    ///
    /// # Errors
    /// Returns a storage error for an invalid limit, query, or stored-data failure.
    pub fn due_reconciliation_jobs(
        &self,
        now: DateTime<Utc>,
        limit: usize,
    ) -> Result<Vec<ReconciliationJob>, StorageError> {
        if limit == 0 || limit > 1_000 {
            return Err(StorageError::InvalidData(
                "reconciliation query limit must be between 1 and 1000".into(),
            ));
        }
        let mut statement = self.connection.prepare(
            "SELECT job_json FROM reconciliation_jobs
             WHERE state = 'pending' AND next_attempt_at <= ?1 AND deadline > ?1
             ORDER BY next_attempt_at, reconciliation_job_id LIMIT ?2",
        )?;
        let rows = statement.query_map(
            params![
                now.to_rfc3339(),
                i64::try_from(limit).map_err(|_| {
                    StorageError::InvalidData(
                        "reconciliation query limit exceeds SQLite range".into(),
                    )
                })?
            ],
            |row| row.get::<_, String>(0),
        )?;
        rows.map(|row| serde_json::from_str(&row?).map_err(StorageError::from))
            .collect()
    }

    /// Claims one due reconciliation job and atomically fences its action execution.
    ///
    /// # Errors
    /// Returns a conflict for stale ownership, a non-due/expired job, replay substitution, or
    /// an action that is no longer awaiting reconciliation.
    #[allow(clippy::too_many_arguments)]
    pub fn claim_reconciliation_job(
        &mut self,
        job_id: ReconciliationJobId,
        expected_job_revision: u64,
        expected_job_generation: u64,
        expected_execution_revision: u64,
        expected_execution_generation: u64,
        worker: RuntimeActorId,
        lease_expires_at: DateTime<Utc>,
        correlation_id: CorrelationId,
        idempotency_key: &str,
        now: DateTime<Utc>,
    ) -> Result<ReconciliationClaimReceipt, StorageError> {
        if idempotency_key.trim().is_empty() || lease_expires_at <= now {
            return Err(StorageError::InvalidData(
                "reconciliation claim requires an idempotency key and future lease".into(),
            ));
        }
        let transaction = self.connection.transaction()?;
        let mut job = load_reconciliation_job(&transaction, job_id)?
            .ok_or_else(|| StorageError::Conflict("reconciliation job was not found".into()))?;
        let mut execution = load_action_execution(&transaction, job.execution_id)?
            .ok_or_else(|| StorageError::InvalidData("reconciliation action is missing".into()))?;
        if let Some(event) =
            load_action_event_by_idempotency(&transaction, job.action_id, idempotency_key)?
        {
            let original_job = action_event_reconciliation_job(&event)?;
            let original_execution = action_event_execution(&event)?;
            if event.type_id.as_str() != RECONCILIATION_CLAIMED_TYPE_ID
                || original_job.id != job_id
                || original_job.lease_owner != Some(worker)
                || original_job.lease_expires_at != Some(lease_expires_at)
            {
                return Err(StorageError::Conflict(
                    "reconciliation claim idempotency key was replayed with different work".into(),
                ));
            }
            transaction.commit()?;
            return Ok(ReconciliationClaimReceipt {
                job: original_job,
                execution: original_execution,
            });
        }
        ensure_reconciliation_expected(&job, expected_job_revision, expected_job_generation)?;
        ensure_action_expected(
            &execution,
            expected_execution_revision,
            expected_execution_generation,
        )?;
        if job.state != ReconciliationJobState::Pending
            || job.next_attempt_at > now
            || job.deadline <= now
            || lease_expires_at > job.deadline
            || execution.state != ActionExecutionState::OutcomeUnknown
            || execution.action_id != job.action_id
        {
            return Err(StorageError::Conflict(
                "reconciliation job is not due or its action is no longer unknown".into(),
            ));
        }

        job.state = ReconciliationJobState::Running;
        job.revision = increment(job.revision, "reconciliation revision")?;
        job.lease_generation = increment(job.lease_generation, "reconciliation generation")?;
        job.lease_owner = Some(worker);
        job.lease_expires_at = Some(lease_expires_at);
        job.attempts = job
            .attempts
            .checked_add(1)
            .ok_or_else(|| StorageError::InvalidData("reconciliation attempts exhausted".into()))?;
        job.updated_at = now;

        execution.state = ActionExecutionState::Reconciling;
        execution.revision = increment(execution.revision, "action revision")?;
        execution.lease_generation = increment(execution.lease_generation, "action generation")?;
        execution.executor = Some(worker);
        execution.updated_at = now;

        update_reconciliation_job_cas(
            &transaction,
            &job,
            expected_job_revision,
            expected_job_generation,
        )?;
        update_action_execution_cas(
            &transaction,
            &execution,
            expected_execution_revision,
            expected_execution_generation,
        )?;
        let (sequence, cause) = next_action_event_position(&transaction, execution.action_id)?;
        let event = action_event(
            execution.action_id,
            Some(execution.id),
            sequence,
            correlation_id,
            cause,
            idempotency_key,
            RECONCILIATION_CLAIMED_TYPE_ID,
            now,
            &serde_json::json!({
                "worker": worker,
                "reconciliation_job": job,
                "execution": execution,
            }),
        )?;
        insert_action_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(ReconciliationClaimReceipt { job, execution })
    }

    /// Persists a reconciler observation and resolves, defers, or escalates the unknown effect.
    ///
    /// A confirmed occurrence stores its receipt before classifying the effect. `StillUnknown`
    /// returns the job to Pending before its deadline and escalates to user action at/after the
    /// deadline; it never redispatches the original operation.
    ///
    /// # Errors
    /// Returns a conflict for stale/expired ownership, mismatched evidence, invalid scheduling,
    /// or replay substitution.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub fn resolve_reconciliation_job(
        &mut self,
        job_id: ReconciliationJobId,
        expected_job_revision: u64,
        expected_job_generation: u64,
        expected_execution_revision: u64,
        expected_execution_generation: u64,
        worker: RuntimeActorId,
        observation: &ReconciliationObservation,
        correlation_id: CorrelationId,
        idempotency_key: &str,
        now: DateTime<Utc>,
    ) -> Result<ReconciliationResolutionReceipt, StorageError> {
        if idempotency_key.trim().is_empty() {
            return Err(StorageError::InvalidData(
                "reconciliation resolution idempotency key cannot be empty".into(),
            ));
        }
        let transaction = self.connection.transaction()?;
        let mut job = load_reconciliation_job(&transaction, job_id)?
            .ok_or_else(|| StorageError::Conflict("reconciliation job was not found".into()))?;
        let mut execution = load_action_execution(&transaction, job.execution_id)?
            .ok_or_else(|| StorageError::InvalidData("reconciliation action is missing".into()))?;
        if let Some(event) =
            load_action_event_by_idempotency(&transaction, job.action_id, idempotency_key)?
        {
            let original_job = action_event_reconciliation_job(&event)?;
            let original_execution = action_event_execution(&event)?;
            let original_observation = action_event_reconciliation_observation(&event)?;
            if !matches!(
                event.type_id.as_str(),
                RECONCILIATION_RESOLVED_TYPE_ID
                    | RECONCILIATION_DEFERRED_TYPE_ID
                    | RECONCILIATION_ESCALATED_TYPE_ID
            ) || original_job.id != job_id
                || original_observation != *observation
            {
                return Err(StorageError::Conflict(
                    "reconciliation resolution idempotency key was replayed with different evidence"
                        .into(),
                ));
            }
            let receipt = match observation {
                ReconciliationObservation::ConfirmedOccurred { receipt, .. } => {
                    let stored =
                        load_action_receipt(&transaction, receipt.id)?.ok_or_else(|| {
                            StorageError::InvalidData(
                                "confirmed reconciliation event has no immutable receipt".into(),
                            )
                        })?;
                    if stored != *receipt {
                        return Err(StorageError::Conflict(
                            "reconciliation receipt was substituted".into(),
                        ));
                    }
                    Some(stored)
                }
                _ => None,
            };
            transaction.commit()?;
            return Ok(ReconciliationResolutionReceipt {
                job: original_job,
                execution: original_execution,
                receipt,
            });
        }
        ensure_reconciliation_expected(&job, expected_job_revision, expected_job_generation)?;
        ensure_action_expected(
            &execution,
            expected_execution_revision,
            expected_execution_generation,
        )?;
        if job.state != ReconciliationJobState::Running
            || job.lease_owner != Some(worker)
            || job.lease_expires_at.is_none_or(|expiry| expiry <= now)
            || execution.state != ActionExecutionState::Reconciling
            || execution.executor != Some(worker)
            || execution.action_id != job.action_id
        {
            return Err(StorageError::Conflict(
                "reconciliation result does not bind the active worker lease".into(),
            ));
        }

        let (event_type, receipt) = match observation {
            ReconciliationObservation::ConfirmedNotOccurred => {
                job.state = ReconciliationJobState::Resolved;
                execution.state = ActionExecutionState::NoEffectConfirmed;
                execution.outcome = Some(EffectOutcome::NoEffectConfirmed);
                (RECONCILIATION_RESOLVED_TYPE_ID, None)
            }
            ReconciliationObservation::ConfirmedOccurred { receipt, evidence } => {
                if receipt.action_id != execution.action_id
                    || receipt.execution_id != execution.id
                    || receipt.observed_at > now
                {
                    return Err(StorageError::Conflict(
                        "reconciliation receipt does not bind this action execution".into(),
                    ));
                }
                decode_canonical_cbor_v1(&receipt.observation.bytes)
                    .map_err(|error| StorageError::InvalidData(error.to_string()))?;
                insert_action_receipt(&transaction, receipt)?;
                job.state = ReconciliationJobState::Resolved;
                execution.state = ActionExecutionState::EffectConfirmed;
                execution.outcome = Some(EffectOutcome::EffectConfirmed {
                    evidence: evidence.clone(),
                });
                (RECONCILIATION_RESOLVED_TYPE_ID, Some(receipt.clone()))
            }
            ReconciliationObservation::StillUnknown {
                hint,
                next_attempt_at,
            } => {
                decode_canonical_cbor_v1(&hint.bytes)
                    .map_err(|error| StorageError::InvalidData(error.to_string()))?;
                job.last_hint = Some(hint.clone());
                execution.state = ActionExecutionState::OutcomeUnknown;
                execution.outcome = Some(EffectOutcome::OutcomeUnknown { hint: hint.clone() });
                if now >= job.deadline {
                    job.state = ReconciliationJobState::UserActionRequired;
                    job.next_attempt_at = job.deadline;
                    (RECONCILIATION_ESCALATED_TYPE_ID, None)
                } else {
                    if *next_attempt_at <= now || *next_attempt_at > job.deadline {
                        return Err(StorageError::InvalidData(
                            "next reconciliation attempt must be after now and no later than the deadline"
                                .into(),
                        ));
                    }
                    job.state = ReconciliationJobState::Pending;
                    job.next_attempt_at = *next_attempt_at;
                    (RECONCILIATION_DEFERRED_TYPE_ID, None)
                }
            }
        };

        let previous_job_revision = job.revision;
        let previous_job_generation = job.lease_generation;
        job.revision = increment(job.revision, "reconciliation revision")?;
        job.lease_generation = increment(job.lease_generation, "reconciliation generation")?;
        job.lease_owner = None;
        job.lease_expires_at = None;
        job.updated_at = now;
        execution.revision = increment(execution.revision, "action revision")?;
        execution.lease_generation = increment(execution.lease_generation, "action generation")?;
        execution.executor = None;
        execution.updated_at = now;

        update_reconciliation_job_cas(
            &transaction,
            &job,
            previous_job_revision,
            previous_job_generation,
        )?;
        update_action_execution_cas(
            &transaction,
            &execution,
            expected_execution_revision,
            expected_execution_generation,
        )?;
        let (sequence, cause) = next_action_event_position(&transaction, execution.action_id)?;
        let event = action_event(
            execution.action_id,
            Some(execution.id),
            sequence,
            correlation_id,
            cause,
            idempotency_key,
            event_type,
            now,
            &serde_json::json!({
                "observation": observation,
                "receipt": receipt,
                "reconciliation_job": job,
                "execution": execution,
            }),
        )?;
        insert_action_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(ReconciliationResolutionReceipt {
            job,
            execution,
            receipt,
        })
    }

    /// Fences expired reconciliation workers and escalates jobs whose deadline elapsed.
    ///
    /// Recovery changes only durable reconciliation state; it never repeats the external action.
    ///
    /// # Errors
    /// Returns a storage error for malformed state, query, or atomic update failure.
    #[allow(clippy::too_many_lines)]
    pub fn recover_reconciliation_jobs(
        &mut self,
        correlation_id: CorrelationId,
        now: DateTime<Utc>,
    ) -> Result<ReconciliationRecoveryReport, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT job_json FROM reconciliation_jobs
             WHERE (state = 'running' AND lease_expires_at <= ?1)
                OR (state = 'pending' AND deadline <= ?1)
             ORDER BY deadline, reconciliation_job_id",
        )?;
        let rows = statement.query_map([now.to_rfc3339()], |row| row.get::<_, String>(0))?;
        let jobs = rows
            .map(|row| serde_json::from_str::<ReconciliationJob>(&row?).map_err(StorageError::from))
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let mut report = ReconciliationRecoveryReport::default();
        for mut job in jobs {
            if job.state == ReconciliationJobState::Running {
                report.inspected_running += 1;
            }
            let transaction = self.connection.transaction()?;
            let current = load_reconciliation_job(&transaction, job.id)?.ok_or_else(|| {
                StorageError::InvalidData("reconciliation job disappeared".into())
            })?;
            if current != job {
                return Err(StorageError::Conflict(
                    "reconciliation job changed during recovery scan".into(),
                ));
            }
            let mut execution =
                load_action_execution(&transaction, job.execution_id)?.ok_or_else(|| {
                    StorageError::InvalidData("reconciliation action is missing".into())
                })?;
            let expected_job_revision = job.revision;
            let expected_job_generation = job.lease_generation;
            let expected_execution_revision = execution.revision;
            let expected_execution_generation = execution.lease_generation;
            let expired_deadline = job.deadline <= now;
            job.state = if expired_deadline {
                ReconciliationJobState::UserActionRequired
            } else {
                ReconciliationJobState::Pending
            };
            job.revision = increment(job.revision, "reconciliation revision")?;
            job.lease_generation = increment(job.lease_generation, "reconciliation generation")?;
            job.lease_owner = None;
            job.lease_expires_at = None;
            if !expired_deadline {
                job.next_attempt_at = now;
            }
            job.updated_at = now;

            if execution.state == ActionExecutionState::Reconciling {
                execution.state = ActionExecutionState::OutcomeUnknown;
                execution.revision = increment(execution.revision, "action revision")?;
                execution.lease_generation =
                    increment(execution.lease_generation, "action generation")?;
                execution.executor = None;
                execution.updated_at = now;
                update_action_execution_cas(
                    &transaction,
                    &execution,
                    expected_execution_revision,
                    expected_execution_generation,
                )?;
            } else if execution.state != ActionExecutionState::OutcomeUnknown {
                return Err(StorageError::InvalidData(
                    "recoverable reconciliation job has a non-reconcilable action state".into(),
                ));
            }
            update_reconciliation_job_cas(
                &transaction,
                &job,
                expected_job_revision,
                expected_job_generation,
            )?;
            let (sequence, cause) = next_action_event_position(&transaction, job.action_id)?;
            let event_type = if expired_deadline {
                RECONCILIATION_ESCALATED_TYPE_ID
            } else {
                RECONCILIATION_LEASE_RECOVERED_TYPE_ID
            };
            let event = action_event(
                job.action_id,
                Some(job.execution_id),
                sequence,
                correlation_id,
                cause,
                &format!(
                    "recover-reconciliation:{}:{expected_job_revision}:{expected_job_generation}",
                    job.id
                ),
                event_type,
                now,
                &serde_json::json!({
                    "reconciliation_job": job,
                    "execution": execution,
                    "reason": if expired_deadline { "deadline_elapsed" } else { "lease_expired" },
                }),
            )?;
            insert_action_event(&transaction, &event)?;
            transaction.commit()?;
            if expired_deadline {
                report.escalated += 1;
            } else {
                report.requeued += 1;
            }
        }
        Ok(report)
    }

    /// Classifies action dispatches interrupted before their durable outcome was committed.
    ///
    /// Observational operations are safely settled as no-effect. Effectful operations become
    /// unknown and receive their immutable authorized reconciler; the original operation is
    /// never redispatched. Jobs whose deadline has already elapsed require user action.
    ///
    /// # Errors
    /// Returns a storage error for malformed authority, missing reconciliation policy, concurrent
    /// changes, or an atomic update failure.
    #[allow(clippy::too_many_lines)]
    pub fn recover_stranded_action_executions(
        &mut self,
        correlation_id: CorrelationId,
        now: DateTime<Utc>,
    ) -> Result<ActionExecutionRecoveryReport, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT execution_json FROM action_executions
             WHERE state IN ('executing', 'observed')
             ORDER BY updated_at, action_id, execution_id",
        )?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        let executions = rows
            .map(|row| serde_json::from_str::<ActionExecution>(&row?).map_err(StorageError::from))
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let mut report = ActionExecutionRecoveryReport::default();
        for mut execution in executions {
            report.inspected += 1;
            let transaction = self.connection.transaction()?;
            let current = load_action_execution(&transaction, execution.id)?.ok_or_else(|| {
                StorageError::InvalidData("stranded action execution disappeared".into())
            })?;
            if current != execution {
                return Err(StorageError::Conflict(
                    "stranded action execution changed during recovery scan".into(),
                ));
            }
            let envelope =
                load_action_envelope(&transaction, execution.action_id)?.ok_or_else(|| {
                    StorageError::InvalidData("action execution has no envelope".into())
                })?;
            let expected_revision = execution.revision;
            let expected_generation = execution.lease_generation;
            let hint = VersionedPayload::canonical_v1(
                StableTypeId::parse("mcg.action_restart_unknown.v1")
                    .map_err(|error| StorageError::InvalidData(error.to_string()))?,
                &serde_json::json!({
                    "action_id": execution.action_id,
                    "execution_id": execution.id,
                    "interrupted_state": execution.state,
                    "reason": "protected Root restarted before outcome classification",
                }),
            )
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
            let (outcome, reconciliation_job) =
                if envelope.effect_class == EffectClass::Observational {
                    report.no_effect_confirmed += 1;
                    (EffectOutcome::NoEffectConfirmed, None)
                } else {
                    let adapter = envelope.reconciler.clone().ok_or_else(|| {
                        StorageError::InvalidData(
                            "stranded effectful action has no authorized reconciler".into(),
                        )
                    })?;
                    let deadline = envelope.reconciliation_deadline.ok_or_else(|| {
                        StorageError::InvalidData(
                            "stranded effectful action has no reconciliation deadline".into(),
                        )
                    })?;
                    let state = if deadline > now {
                        report.outcome_unknown += 1;
                        ReconciliationJobState::Pending
                    } else {
                        report.escalated += 1;
                        ReconciliationJobState::UserActionRequired
                    };
                    let job = ReconciliationJob {
                        id: ReconciliationJobId::new(),
                        action_id: execution.action_id,
                        execution_id: execution.id,
                        adapter,
                        state,
                        revision: 1,
                        lease_generation: 0,
                        lease_owner: None,
                        lease_expires_at: None,
                        next_attempt_at: now,
                        attempts: 0,
                        deadline,
                        last_hint: Some(hint.clone()),
                        created_at: now,
                        updated_at: now,
                    };
                    (
                        EffectOutcome::OutcomeUnknown { hint: hint.clone() },
                        Some(job),
                    )
                };
            execution.state = match &outcome {
                EffectOutcome::NoEffectConfirmed => ActionExecutionState::NoEffectConfirmed,
                EffectOutcome::OutcomeUnknown { .. } => ActionExecutionState::OutcomeUnknown,
                EffectOutcome::EffectConfirmed { .. } => unreachable!("restart classification"),
            };
            execution.revision = increment(execution.revision, "action revision")?;
            execution.lease_generation =
                increment(execution.lease_generation, "action generation")?;
            execution.executor = None;
            execution.outcome = Some(outcome.clone());
            execution.updated_at = now;
            update_action_execution_cas(
                &transaction,
                &execution,
                expected_revision,
                expected_generation,
            )?;
            if let Some(job) = &reconciliation_job {
                insert_reconciliation_job(&transaction, job)?;
            }
            let (sequence, cause) = next_action_event_position(&transaction, execution.action_id)?;
            let event = action_event(
                execution.action_id,
                Some(execution.id),
                sequence,
                correlation_id,
                cause,
                &format!(
                    "recover-action-execution:{}:{expected_revision}:{expected_generation}",
                    execution.id
                ),
                ACTION_EXECUTION_RECOVERED_TYPE_ID,
                now,
                &serde_json::json!({
                    "outcome": outcome,
                    "execution": execution,
                    "reconciliation_job": reconciliation_job,
                }),
            )?;
            insert_action_event(&transaction, &event)?;
            transaction.commit()?;
        }
        Ok(report)
    }
}

#[allow(clippy::too_many_lines)]
pub(super) fn fence_actions_for_workflow_cancellation(
    transaction: &Transaction<'_>,
    attempt_id: mcg_contracts::AttemptId,
    correlation_id: CorrelationId,
    now: DateTime<Utc>,
) -> Result<CancellationEffectReport, StorageError> {
    let mut statement = transaction.prepare(
        "SELECT execution.execution_json
         FROM action_executions AS execution
         JOIN action_envelopes AS envelope ON envelope.action_id = execution.action_id
         WHERE envelope.workflow_attempt_id = ?1
         ORDER BY envelope.created_at, execution.attempt_number, execution.execution_id",
    )?;
    let rows = statement.query_map([attempt_id.to_string()], |row| row.get::<_, String>(0))?;
    let mut executions = rows
        .map(|row| serde_json::from_str::<ActionExecution>(&row?).map_err(StorageError::from))
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);

    let mut report = CancellationEffectReport::default();
    for execution in &mut executions {
        let envelope = load_action_envelope(transaction, execution.action_id)?
            .ok_or_else(|| StorageError::InvalidData("action execution has no envelope".into()))?;
        let expected_revision = execution.revision;
        let expected_generation = execution.lease_generation;
        let classification = match execution.state {
            ActionExecutionState::Authorized => {
                report.cancelled_before_dispatch += 1;
                Some(EffectOutcome::NoEffectConfirmed)
            }
            ActionExecutionState::Executing | ActionExecutionState::Observed
                if envelope.effect_class == EffectClass::Observational =>
            {
                report.observational_stopped += 1;
                Some(EffectOutcome::NoEffectConfirmed)
            }
            ActionExecutionState::Executing | ActionExecutionState::Observed => {
                report.marked_outcome_unknown += 1;
                let hint = VersionedPayload::canonical_v1(
                    StableTypeId::parse("mcg.action_cancellation_unknown.v1")
                        .map_err(|error| StorageError::InvalidData(error.to_string()))?,
                    &serde_json::json!({
                        "action_id": execution.action_id,
                        "execution_id": execution.id,
                        "reason": "workflow cancellation after dispatch",
                    }),
                )
                .map_err(|error| StorageError::InvalidData(error.to_string()))?;
                Some(EffectOutcome::OutcomeUnknown { hint })
            }
            _ => None,
        };
        if let Some(outcome) = classification {
            execution.state = match &outcome {
                EffectOutcome::NoEffectConfirmed => ActionExecutionState::NoEffectConfirmed,
                EffectOutcome::OutcomeUnknown { .. } => ActionExecutionState::OutcomeUnknown,
                EffectOutcome::EffectConfirmed { .. } => {
                    unreachable!("cancellation classification")
                }
            };
            execution.revision = increment(execution.revision, "action revision")?;
            execution.lease_generation =
                increment(execution.lease_generation, "action generation")?;
            execution.executor = None;
            execution.outcome = Some(outcome.clone());
            execution.updated_at = now;
            update_action_execution_cas(
                transaction,
                execution,
                expected_revision,
                expected_generation,
            )?;

            let reconciliation_job = if let EffectOutcome::OutcomeUnknown { hint } = &outcome {
                let adapter = envelope.reconciler.clone().ok_or_else(|| {
                    StorageError::Conflict(
                        "effectful cancellation has no authorized reconciler".into(),
                    )
                })?;
                let deadline = envelope.reconciliation_deadline.ok_or_else(|| {
                    StorageError::Conflict(
                        "effectful cancellation has no authorized reconciliation deadline".into(),
                    )
                })?;
                let job = ReconciliationJob {
                    id: ReconciliationJobId::new(),
                    action_id: execution.action_id,
                    execution_id: execution.id,
                    adapter,
                    state: if deadline > now {
                        ReconciliationJobState::Pending
                    } else {
                        ReconciliationJobState::UserActionRequired
                    },
                    revision: 1,
                    lease_generation: 0,
                    lease_owner: None,
                    lease_expires_at: None,
                    next_attempt_at: now,
                    attempts: 0,
                    deadline,
                    last_hint: Some(hint.clone()),
                    created_at: now,
                    updated_at: now,
                };
                insert_reconciliation_job(transaction, &job)?;
                Some(job)
            } else {
                None
            };
            let (sequence, cause) = next_action_event_position(transaction, execution.action_id)?;
            let idempotency_key = format!(
                "cancel-fence:{}:{expected_revision}:{expected_generation}",
                execution.id
            );
            let event = action_event(
                execution.action_id,
                Some(execution.id),
                sequence,
                correlation_id,
                cause,
                &idempotency_key,
                ACTION_CANCEL_FENCED_TYPE_ID,
                now,
                &serde_json::json!({
                    "outcome": outcome,
                    "execution": execution,
                    "reconciliation_job": reconciliation_job,
                }),
            )?;
            insert_action_event(transaction, &event)?;
        }
    }

    let unresolved = unresolved_actions_for_attempt(transaction, attempt_id)?;
    report.unresolved = unresolved.unresolved;
    report.reconciliation_required = unresolved.reconciliation_required;
    report.quarantined = unresolved.quarantined;
    Ok(report)
}

pub(super) fn cancellation_effect_status(
    connection: &rusqlite::Connection,
    attempt_id: mcg_contracts::AttemptId,
) -> Result<CancellationEffectReport, StorageError> {
    let unresolved = unresolved_actions_for_attempt(connection, attempt_id)?;
    Ok(CancellationEffectReport {
        unresolved: unresolved.unresolved,
        reconciliation_required: unresolved.reconciliation_required,
        quarantined: unresolved.quarantined,
        ..CancellationEffectReport::default()
    })
}

#[derive(Debug, Clone, Copy, Default)]
struct UnresolvedActions {
    unresolved: u64,
    reconciliation_required: u64,
    quarantined: u64,
}

fn unresolved_actions_for_attempt(
    connection: &rusqlite::Connection,
    attempt_id: mcg_contracts::AttemptId,
) -> Result<UnresolvedActions, StorageError> {
    let mut statement = connection.prepare(
        "SELECT execution.state
         FROM action_executions AS execution
         JOIN action_envelopes AS envelope ON envelope.action_id = execution.action_id
         WHERE envelope.workflow_attempt_id = ?1",
    )?;
    let states = statement
        .query_map([attempt_id.to_string()], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    let mut result = UnresolvedActions::default();
    for state in states {
        match state.as_str() {
            "no_effect_confirmed" | "committed" | "denied" | "compensated" => {}
            "outcome_unknown" | "reconciling" => {
                result.unresolved += 1;
                result.reconciliation_required += 1;
            }
            "quarantined" => {
                result.unresolved += 1;
                result.quarantined += 1;
            }
            _ => result.unresolved += 1,
        }
    }
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn mutate_action_execution<R, F>(
    connection: &mut rusqlite::Connection,
    execution_id: ActionExecutionId,
    expected_revision: u64,
    expected_generation: u64,
    correlation_id: CorrelationId,
    idempotency_key: &str,
    type_id: &str,
    now: DateTime<Utc>,
    replay_matches: R,
    mutate: F,
) -> Result<ActionExecution, StorageError>
where
    R: FnOnce(&ActionExecution) -> bool,
    F: FnOnce(&mut ActionExecution, &ActionEnvelope) -> Result<serde_json::Value, StorageError>,
{
    if idempotency_key.trim().is_empty() {
        return Err(StorageError::InvalidData(
            "action mutation idempotency key cannot be empty".into(),
        ));
    }
    let transaction = connection.transaction()?;
    let mut execution = load_action_execution(&transaction, execution_id)?
        .ok_or_else(|| StorageError::Conflict("action execution was not found".into()))?;
    if let Some(event) =
        load_action_event_by_idempotency(&transaction, execution.action_id, idempotency_key)?
    {
        let original = action_event_execution(&event)?;
        if event.type_id.as_str() != type_id || !replay_matches(&original) {
            return Err(StorageError::Conflict(
                "action idempotency key was replayed for another mutation or content".into(),
            ));
        }
        transaction.commit()?;
        return Ok(original);
    }
    ensure_action_expected(&execution, expected_revision, expected_generation)?;
    let envelope = load_action_envelope(&transaction, execution.action_id)?
        .ok_or_else(|| StorageError::InvalidData("action execution has no envelope".into()))?;
    let details = mutate(&mut execution, &envelope)?;
    update_action_execution_cas(
        &transaction,
        &execution,
        expected_revision,
        expected_generation,
    )?;
    let (sequence, cause) = next_action_event_position(&transaction, execution.action_id)?;
    let event = action_event(
        execution.action_id,
        Some(execution.id),
        sequence,
        correlation_id,
        cause,
        idempotency_key,
        type_id,
        now,
        &serde_json::json!({"details": details, "execution": execution}),
    )?;
    insert_action_event(&transaction, &event)?;
    transaction.commit()?;
    Ok(execution)
}

fn validate_action_authorization(
    envelope: &ActionEnvelope,
    execution: &ActionExecution,
    now: DateTime<Utc>,
) -> Result<(), StorageError> {
    if envelope.idempotency_key.trim().is_empty()
        || execution.action_id != envelope.id
        || execution.attempt_number != 1
        || execution.state != ActionExecutionState::Authorized
        || execution.revision != 1
        || execution.lease_generation != 0
        || execution.executor.is_some()
        || execution.outcome.is_some()
        || execution.created_at != now
        || execution.updated_at != now
        || envelope.created_at != now
    {
        return Err(StorageError::InvalidData(
            "action authorization must atomically begin as revision-1 unleased Authorized intent"
                .into(),
        ));
    }
    let operation_hash = envelope
        .operation
        .content_hash(&envelope.operation_hash.domain)
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    if operation_hash != envelope.operation_hash {
        return Err(StorageError::InvalidData(
            "action operation hash does not bind exact canonical operation bytes".into(),
        ));
    }
    validate_policy_binding(&envelope.authority)?;
    if let Some(origin) = &envelope.origin {
        validate_action_origin(origin)?;
    }
    if envelope.effect_class == EffectClass::Irreversible && envelope.elevated_authority.is_none() {
        return Err(StorageError::InvalidData(
            "an irreversible action requires a separate elevated authority binding".into(),
        ));
    }
    if let Some(binding) = &envelope.elevated_authority {
        validate_policy_binding(binding)?;
    }
    validate_policy_binding(&envelope.budget_reservation)?;
    for precondition in &envelope.preconditions {
        decode_canonical_cbor_v1(&precondition.bytes)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    }
    if envelope.compensates_action == Some(envelope.id) {
        return Err(StorageError::InvalidData(
            "an action cannot compensate itself".into(),
        ));
    }
    if let Some(compensation) = &envelope.compensation {
        decode_canonical_cbor_v1(&compensation.bytes)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    }
    if envelope.effect_class != EffectClass::Observational && envelope.reconciler.is_none() {
        return Err(StorageError::InvalidData(
            "a potentially effectful action requires an exact reconciler binding".into(),
        ));
    }
    if envelope.reconciler.is_some() != envelope.reconciliation_deadline.is_some() {
        return Err(StorageError::InvalidData(
            "action reconciler and deadline must be bound together".into(),
        ));
    }
    if envelope
        .reconciliation_deadline
        .is_some_and(|deadline| deadline <= now)
    {
        return Err(StorageError::InvalidData(
            "action reconciliation deadline must be in the future".into(),
        ));
    }
    if let Some(reconciler) = &envelope.reconciler {
        decode_canonical_cbor_v1(&reconciler.bytes)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    }
    Ok(())
}

fn validate_action_origin(origin: &mcg_contracts::ActionOrigin) -> Result<(), StorageError> {
    const MAX_ORIGIN_LOCATOR_BYTES: usize = 2_048;
    const MAX_ORIGIN_EVIDENCE: usize = 64;

    if origin.evidence.len() > MAX_ORIGIN_EVIDENCE {
        return Err(StorageError::InvalidData(
            "action origin contains too many evidence references".into(),
        ));
    }
    let sources = [
        origin.conversation.as_ref(),
        origin.invocation.as_ref(),
        origin.command.as_ref(),
        origin.approval.as_ref(),
        origin.agent.as_ref(),
    ]
    .into_iter()
    .flatten()
    .chain(origin.evidence.iter())
    .collect::<Vec<_>>();
    if sources.is_empty() {
        return Err(StorageError::InvalidData(
            "action origin must contain at least one source reference".into(),
        ));
    }
    let mut identities = BTreeSet::new();
    for source in sources {
        let locator = source.locator.trim();
        if locator.is_empty()
            || locator.len() > MAX_ORIGIN_LOCATOR_BYTES
            || source.content_hash.domain.trim().is_empty()
            || source.content_hash.digest_hex.len() != 64
            || !source
                .content_hash
                .digest_hex
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(StorageError::InvalidData(
                "action origin contains an invalid source locator or content hash".into(),
            ));
        }
        if !identities.insert((source.source_type.as_str(), locator)) {
            return Err(StorageError::InvalidData(
                "action origin repeats the same source reference".into(),
            ));
        }
    }
    Ok(())
}

fn validate_policy_binding(binding: &mcg_contracts::PolicyBinding) -> Result<(), StorageError> {
    let actual = binding
        .payload
        .content_hash(&binding.content_hash.domain)
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    if actual != binding.content_hash {
        return Err(StorageError::InvalidData(
            "action policy binding hash does not match canonical bytes".into(),
        ));
    }
    Ok(())
}

fn validate_compensation_authorization(
    transaction: &Transaction<'_>,
    envelope: &ActionEnvelope,
) -> Result<(), StorageError> {
    let Some(original_action_id) = envelope.compensates_action else {
        return Ok(());
    };
    let original_envelope = load_action_envelope(transaction, original_action_id)?
        .ok_or_else(|| StorageError::Conflict("compensated action was not found".into()))?;
    let original_execution = load_initial_execution(transaction, original_action_id)?
        .ok_or_else(|| StorageError::InvalidData("compensated action has no execution".into()))?;
    if original_envelope.workflow_id != envelope.workflow_id
        || original_envelope.attempt_id != envelope.attempt_id
        || original_envelope.task_contract != envelope.task_contract
        || original_envelope.compensation.as_ref() != Some(&envelope.operation)
        || original_execution.state != ActionExecutionState::Committed
        || envelope.effect_class == EffectClass::Observational
    {
        return Err(StorageError::Conflict(
            "compensation must be a new effectful action exactly bound to the committed original plan"
                .into(),
        ));
    }
    Ok(())
}

fn load_workflow_attempt(
    transaction: &Transaction<'_>,
    attempt_id: mcg_contracts::AttemptId,
) -> Result<Option<WorkflowAttempt>, StorageError> {
    transaction
        .query_row(
            "SELECT attempt_json FROM workflow_attempts WHERE attempt_id = ?1",
            [attempt_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
        .transpose()
}

fn insert_action_envelope(
    transaction: &Transaction<'_>,
    envelope: &ActionEnvelope,
) -> Result<(), StorageError> {
    transaction.execute(
        "INSERT INTO action_envelopes(
            action_id, workflow_id, workflow_attempt_id, task_contract_id,
            task_contract_version, operation_hash_json, effect_class, idempotency_key,
            envelope_json, created_at, compensates_action_id
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            envelope.id.to_string(),
            envelope.workflow_id.to_string(),
            envelope.attempt_id.to_string(),
            envelope.task_contract.id.to_string(),
            sql_u64(envelope.task_contract.version, "task contract version")?,
            serde_json::to_string(&envelope.operation_hash)?,
            effect_class_label(envelope.effect_class),
            envelope.idempotency_key,
            serde_json::to_string(envelope)?,
            envelope.created_at.to_rfc3339(),
            envelope.compensates_action.map(|id| id.to_string()),
        ],
    )?;
    Ok(())
}

fn load_action_envelope(
    transaction: &Transaction<'_>,
    action_id: mcg_contracts::ActionId,
) -> Result<Option<ActionEnvelope>, StorageError> {
    transaction
        .query_row(
            "SELECT envelope_json FROM action_envelopes WHERE action_id = ?1",
            [action_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
        .transpose()
}

fn load_envelope_by_idempotency(
    transaction: &Transaction<'_>,
    workflow_id: mcg_contracts::WorkflowId,
    idempotency_key: &str,
) -> Result<Option<ActionEnvelope>, StorageError> {
    transaction
        .query_row(
            "SELECT envelope_json FROM action_envelopes
             WHERE workflow_id = ?1 AND idempotency_key = ?2",
            params![workflow_id.to_string(), idempotency_key],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
        .transpose()
}

fn insert_action_execution(
    transaction: &Transaction<'_>,
    execution: &ActionExecution,
) -> Result<(), StorageError> {
    transaction.execute(
        "INSERT INTO action_executions(
            execution_id, action_id, attempt_number, state, revision, lease_generation,
            executor, execution_json, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            execution.id.to_string(),
            execution.action_id.to_string(),
            i64::from(execution.attempt_number),
            action_state_label(execution.state),
            sql_u64(execution.revision, "action revision")?,
            sql_u64(execution.lease_generation, "action generation")?,
            execution.executor.map(|id| id.to_string()),
            serde_json::to_string(execution)?,
            execution.created_at.to_rfc3339(),
            execution.updated_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

fn load_initial_execution(
    transaction: &Transaction<'_>,
    action_id: mcg_contracts::ActionId,
) -> Result<Option<ActionExecution>, StorageError> {
    transaction
        .query_row(
            "SELECT execution_json FROM action_executions
             WHERE action_id = ?1 AND attempt_number = 1",
            [action_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
        .transpose()
}

fn load_action_execution(
    connection: &rusqlite::Connection,
    execution_id: ActionExecutionId,
) -> Result<Option<ActionExecution>, StorageError> {
    connection
        .query_row(
            "SELECT execution_json FROM action_executions WHERE execution_id = ?1",
            [execution_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
        .transpose()
}

fn update_action_execution_cas(
    transaction: &Transaction<'_>,
    execution: &ActionExecution,
    expected_revision: u64,
    expected_generation: u64,
) -> Result<(), StorageError> {
    let changed = transaction.execute(
        "UPDATE action_executions SET
            state = ?1, revision = ?2, lease_generation = ?3, executor = ?4,
            execution_json = ?5, updated_at = ?6
         WHERE execution_id = ?7 AND revision = ?8 AND lease_generation = ?9",
        params![
            action_state_label(execution.state),
            sql_u64(execution.revision, "action revision")?,
            sql_u64(execution.lease_generation, "action generation")?,
            execution.executor.map(|id| id.to_string()),
            serde_json::to_string(execution)?,
            execution.updated_at.to_rfc3339(),
            execution.id.to_string(),
            sql_u64(expected_revision, "expected action revision")?,
            sql_u64(expected_generation, "expected action generation")?,
        ],
    )?;
    if changed != 1 {
        return Err(StorageError::Conflict(
            "action revision or generation changed before commit".into(),
        ));
    }
    Ok(())
}

fn ensure_action_expected(
    execution: &ActionExecution,
    expected_revision: u64,
    expected_generation: u64,
) -> Result<(), StorageError> {
    if execution.revision != expected_revision || execution.lease_generation != expected_generation
    {
        return Err(StorageError::Conflict(format!(
            "stale action execution: current revision/generation is {}/{}",
            execution.revision, execution.lease_generation
        )));
    }
    Ok(())
}

fn ensure_reconciliation_expected(
    job: &ReconciliationJob,
    expected_revision: u64,
    expected_generation: u64,
) -> Result<(), StorageError> {
    if job.revision != expected_revision || job.lease_generation != expected_generation {
        return Err(StorageError::Conflict(format!(
            "stale reconciliation job: current revision/generation is {}/{}",
            job.revision, job.lease_generation
        )));
    }
    Ok(())
}

fn insert_action_receipt(
    transaction: &Transaction<'_>,
    receipt: &ActionReceipt,
) -> Result<(), StorageError> {
    transaction.execute(
        "INSERT INTO action_receipts(
            receipt_id, action_id, execution_id, receipt_json, observed_at
         ) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            receipt.id.to_string(),
            receipt.action_id.to_string(),
            receipt.execution_id.to_string(),
            serde_json::to_string(receipt)?,
            receipt.observed_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

fn load_action_receipt(
    transaction: &Transaction<'_>,
    receipt_id: mcg_contracts::ActionReceiptId,
) -> Result<Option<ActionReceipt>, StorageError> {
    transaction
        .query_row(
            "SELECT receipt_json FROM action_receipts WHERE receipt_id = ?1",
            [receipt_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
        .transpose()
}

fn insert_reconciliation_job(
    transaction: &Transaction<'_>,
    job: &ReconciliationJob,
) -> Result<(), StorageError> {
    transaction.execute(
        "INSERT INTO reconciliation_jobs(
            reconciliation_job_id, action_id, execution_id, state, revision,
            lease_generation, lease_owner, lease_expires_at, next_attempt_at,
            deadline, job_json, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            job.id.to_string(),
            job.action_id.to_string(),
            job.execution_id.to_string(),
            reconciliation_state_label(job.state),
            sql_u64(job.revision, "reconciliation revision")?,
            sql_u64(job.lease_generation, "reconciliation generation")?,
            job.lease_owner.map(|id| id.to_string()),
            job.lease_expires_at.map(|at| at.to_rfc3339()),
            job.next_attempt_at.to_rfc3339(),
            job.deadline.to_rfc3339(),
            serde_json::to_string(job)?,
            job.created_at.to_rfc3339(),
            job.updated_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

fn update_reconciliation_job_cas(
    transaction: &Transaction<'_>,
    job: &ReconciliationJob,
    expected_revision: u64,
    expected_generation: u64,
) -> Result<(), StorageError> {
    let changed = transaction.execute(
        "UPDATE reconciliation_jobs SET
            state = ?1, revision = ?2, lease_generation = ?3, lease_owner = ?4,
            lease_expires_at = ?5, next_attempt_at = ?6, deadline = ?7,
            job_json = ?8, updated_at = ?9
         WHERE reconciliation_job_id = ?10 AND revision = ?11 AND lease_generation = ?12",
        params![
            reconciliation_state_label(job.state),
            sql_u64(job.revision, "reconciliation revision")?,
            sql_u64(job.lease_generation, "reconciliation generation")?,
            job.lease_owner.map(|id| id.to_string()),
            job.lease_expires_at.map(|at| at.to_rfc3339()),
            job.next_attempt_at.to_rfc3339(),
            job.deadline.to_rfc3339(),
            serde_json::to_string(job)?,
            job.updated_at.to_rfc3339(),
            job.id.to_string(),
            sql_u64(expected_revision, "expected reconciliation revision")?,
            sql_u64(expected_generation, "expected reconciliation generation")?,
        ],
    )?;
    if changed != 1 {
        return Err(StorageError::Conflict(
            "reconciliation revision or generation changed before commit".into(),
        ));
    }
    Ok(())
}

fn load_reconciliation_job(
    connection: &rusqlite::Connection,
    job_id: ReconciliationJobId,
) -> Result<Option<ReconciliationJob>, StorageError> {
    connection
        .query_row(
            "SELECT job_json FROM reconciliation_jobs WHERE reconciliation_job_id = ?1",
            [job_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
        .transpose()
}

fn load_reconciliation_job_for_execution(
    transaction: &rusqlite::Connection,
    execution_id: ActionExecutionId,
) -> Result<Option<ReconciliationJob>, StorageError> {
    transaction
        .query_row(
            "SELECT job_json FROM reconciliation_jobs WHERE execution_id = ?1",
            [execution_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
        .transpose()
}

fn next_action_event_position(
    transaction: &Transaction<'_>,
    action_id: mcg_contracts::ActionId,
) -> Result<(u64, Option<EventId>), StorageError> {
    let previous = transaction
        .query_row(
            "SELECT sequence, event_json FROM action_events
             WHERE action_id = ?1 ORDER BY sequence DESC LIMIT 1",
            [action_id.to_string()],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    match previous {
        Some((sequence, json)) => {
            let event: ActionEvent = serde_json::from_str(&json)?;
            let sequence = u64::try_from(sequence)
                .map_err(|_| StorageError::InvalidData("negative action sequence".into()))?;
            Ok((
                increment(sequence, "action event sequence")?,
                Some(event.id),
            ))
        }
        None => Ok((1, None)),
    }
}

#[allow(clippy::too_many_arguments)]
fn action_event(
    action_id: mcg_contracts::ActionId,
    execution_id: Option<ActionExecutionId>,
    sequence: u64,
    correlation_id: CorrelationId,
    causation_id: Option<EventId>,
    idempotency_key: &str,
    type_id: &str,
    occurred_at: DateTime<Utc>,
    payload: &serde_json::Value,
) -> Result<ActionEvent, StorageError> {
    Ok(ActionEvent {
        id: EventId::new(),
        type_id: StableTypeId::parse(type_id)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?,
        schema_version: ActionEvent::SCHEMA_VERSION,
        sequence,
        action_id,
        execution_id,
        correlation_id,
        causation_id,
        idempotency_key: idempotency_key.to_owned(),
        occurred_at,
        payload: VersionedPayload::canonical_v1(
            StableTypeId::parse(type_id)
                .map_err(|error| StorageError::InvalidData(error.to_string()))?,
            payload,
        )
        .map_err(|error| StorageError::InvalidData(error.to_string()))?,
    })
}

fn insert_action_event(
    transaction: &Transaction<'_>,
    event: &ActionEvent,
) -> Result<(), StorageError> {
    transaction.execute(
        "INSERT INTO action_events(
            event_id, action_id, sequence, execution_id, type_id, schema_version,
            correlation_id, causation_id, idempotency_key, event_json, occurred_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            event.id.to_string(),
            event.action_id.to_string(),
            sql_u64(event.sequence, "action event sequence")?,
            event.execution_id.map(|id| id.to_string()),
            event.type_id.as_str(),
            i64::from(event.schema_version),
            event.correlation_id.to_string(),
            event.causation_id.map(|id| id.to_string()),
            event.idempotency_key,
            serde_json::to_string(event)?,
            event.occurred_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

fn load_action_event_by_idempotency(
    transaction: &Transaction<'_>,
    action_id: mcg_contracts::ActionId,
    idempotency_key: &str,
) -> Result<Option<ActionEvent>, StorageError> {
    transaction
        .query_row(
            "SELECT event_json FROM action_events
             WHERE action_id = ?1 AND idempotency_key = ?2",
            params![action_id.to_string(), idempotency_key],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
        .transpose()
}

fn load_action_event(
    transaction: &Transaction<'_>,
    event_id: EventId,
) -> Result<Option<ActionEvent>, StorageError> {
    transaction
        .query_row(
            "SELECT event_json FROM action_events WHERE event_id = ?1",
            [event_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
        .transpose()
}

fn action_event_execution(event: &ActionEvent) -> Result<ActionExecution, StorageError> {
    let value = action_event_payload_value(event)?;
    serde_json::from_value(value.get("execution").cloned().ok_or_else(|| {
        StorageError::InvalidData("action event has no execution snapshot".into())
    })?)
    .map_err(StorageError::from)
}

fn action_event_payload_value(event: &ActionEvent) -> Result<serde_json::Value, StorageError> {
    decode_canonical_cbor_v1(&event.payload.bytes)
        .map_err(|error| StorageError::InvalidData(error.to_string()))
}

fn action_event_reconciliation_job(event: &ActionEvent) -> Result<ReconciliationJob, StorageError> {
    let value = action_event_payload_value(event)?;
    serde_json::from_value(value.get("reconciliation_job").cloned().ok_or_else(|| {
        StorageError::InvalidData("action event has no reconciliation job snapshot".into())
    })?)
    .map_err(StorageError::from)
}

fn action_event_reconciliation_observation(
    event: &ActionEvent,
) -> Result<ReconciliationObservation, StorageError> {
    let value = action_event_payload_value(event)?;
    serde_json::from_value(value.get("observation").cloned().ok_or_else(|| {
        StorageError::InvalidData("reconciliation event has no observation".into())
    })?)
    .map_err(StorageError::from)
}

fn action_event_validation(event: &ActionEvent) -> Result<ActionValidation, StorageError> {
    let value = action_event_payload_value(event)?;
    serde_json::from_value(value.get("validation").cloned().ok_or_else(|| {
        StorageError::InvalidData("action event has no validation evidence".into())
    })?)
    .map_err(StorageError::from)
}

const fn effect_class_label(effect: EffectClass) -> &'static str {
    match effect {
        EffectClass::Observational => "observational",
        EffectClass::Reversible => "reversible",
        EffectClass::Consequential => "consequential",
        EffectClass::Irreversible => "irreversible",
    }
}

const fn action_state_label(state: ActionExecutionState) -> &'static str {
    match state {
        ActionExecutionState::Proposed => "proposed",
        ActionExecutionState::Authorized => "authorized",
        ActionExecutionState::Executing => "executing",
        ActionExecutionState::Observed => "observed",
        ActionExecutionState::NoEffectConfirmed => "no_effect_confirmed",
        ActionExecutionState::EffectConfirmed => "effect_confirmed",
        ActionExecutionState::OutcomeUnknown => "outcome_unknown",
        ActionExecutionState::Reconciling => "reconciling",
        ActionExecutionState::Validated => "validated",
        ActionExecutionState::Committed => "committed",
        ActionExecutionState::Denied => "denied",
        ActionExecutionState::Quarantined => "quarantined",
        ActionExecutionState::Compensated => "compensated",
    }
}

const fn reconciliation_state_label(state: ReconciliationJobState) -> &'static str {
    match state {
        ReconciliationJobState::Pending => "pending",
        ReconciliationJobState::Running => "running",
        ReconciliationJobState::Resolved => "resolved",
        ReconciliationJobState::UserActionRequired => "user_action_required",
        ReconciliationJobState::Quarantined => "quarantined",
    }
}

fn query_action_collection<T: serde::de::DeserializeOwned>(
    connection: &rusqlite::Connection,
    sql: &str,
    action_id: mcg_contracts::ActionId,
    label: &str,
) -> Result<Vec<T>, StorageError> {
    let mut statement = connection.prepare(sql)?;
    let rows = statement.query_map([action_id.to_string()], |row| row.get::<_, String>(0))?;
    let values = rows
        .map(|row| serde_json::from_str::<T>(&row?).map_err(StorageError::from))
        .collect::<Result<Vec<_>, _>>()?;
    if values.len() > 1_000 {
        return Err(StorageError::InvalidData(format!(
            "{label} exceed the bounded inspection limit"
        )));
    }
    Ok(values)
}

fn sql_u64(value: u64, label: &str) -> Result<i64, StorageError> {
    i64::try_from(value)
        .map_err(|_| StorageError::InvalidData(format!("{label} exceeds SQLite integer range")))
}

fn increment(value: u64, label: &str) -> Result<u64, StorageError> {
    value
        .checked_add(1)
        .ok_or_else(|| StorageError::InvalidData(format!("{label} exhausted")))
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use mcg_contracts::{
        ActionExecutionId, ActionId, ActionOrigin, ActionReceiptId, ConfigurationId, ContentHash,
        ContractClause, HashAlgorithm, PolicyBinding, RetryPolicy, SourceRef, TaskContract,
        TaskContractId, TaskContractStatus, VersionedRef, WorkflowAttempt, WorkflowId,
    };

    use super::*;
    use crate::task_contract_content_hash;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 30, 13, 0, 0).unwrap()
    }

    #[allow(clippy::needless_pass_by_value)]
    fn payload(type_id: &str, value: serde_json::Value) -> VersionedPayload {
        VersionedPayload::canonical_v1(StableTypeId::parse(type_id).unwrap(), &value).unwrap()
    }

    fn hash(label: &str) -> ContentHash {
        payload("test.action_ref.v1", serde_json::json!({"label": label}))
            .content_hash("TestActionRef")
            .unwrap()
    }

    fn policy(label: &str) -> PolicyBinding {
        let payload = payload(
            "test.action_policy.v1",
            serde_json::json!({"policy": label}),
        );
        let content_hash = payload.content_hash("TestActionPolicy").unwrap();
        PolicyBinding {
            payload,
            content_hash,
        }
    }

    fn origin_source(source_type: &str, locator: &str, label: &str) -> SourceRef {
        SourceRef {
            source_type: StableTypeId::parse(source_type).unwrap(),
            locator: locator.into(),
            content_hash: hash(label),
        }
    }

    fn running_workflow(
        store: &mut SqliteEventStore,
    ) -> (TaskContract, WorkflowAttempt, RuntimeActorId) {
        let at = now();
        let task_id = TaskContractId::new();
        let source = SourceRef {
            source_type: StableTypeId::parse("test.action_intent.v1").unwrap(),
            locator: "conversation:action-test".into(),
            content_hash: hash("intent"),
        };
        let criterion = ContractClause {
            clause_id: "effect".into(),
            statement: "the effect is classified before commit".into(),
            source_refs: vec![source.clone()],
        };
        let contract = TaskContract {
            id: task_id,
            version: 1,
            previous_version: None,
            user_intent_ref: source,
            objective: "exercise the Root action ledger".into(),
            non_goals: Vec::new(),
            requirements: vec![criterion.clone()],
            constraints: Vec::new(),
            assumptions: Vec::new(),
            acceptance_criteria: vec![criterion],
            dependencies: Vec::new(),
            permitted_side_effects: policy("reversible-write"),
            authority: policy("root-authorized"),
            environment: policy("isolated-worktree"),
            risk: policy("bounded"),
            budgets: policy("reserved"),
            rollback: policy("compensate"),
            status: TaskContractStatus::Active,
            created_at: at,
        };
        let mut attempt = WorkflowAttempt {
            workflow_id: WorkflowId::new(),
            attempt_id: mcg_contracts::AttemptId::new(),
            task_contract: VersionedRef {
                id: task_id,
                version: 1,
                content_hash: ContentHash {
                    algorithm: HashAlgorithm::Blake3_256,
                    domain: "MetaCog.TaskContract".into(),
                    digest_hex: "0".repeat(64),
                },
            },
            parent_attempt_id: None,
            attempt_number: 1,
            state: WorkflowState::Ready,
            revision: 1,
            lease_owner: None,
            lease_generation: 0,
            lease_expires_at: None,
            heartbeat_at: None,
            checkpoint: None,
            environment_snapshot: hash("environment"),
            configuration: VersionedRef {
                id: ConfigurationId::new(),
                version: 1,
                content_hash: hash("configuration"),
            },
            idempotency_key: "action-workflow-create".into(),
            retry_policy: RetryPolicy {
                max_attempts: 2,
                initial_backoff_ms: 100,
                maximum_backoff_ms: 1_000,
            },
            compensation_policy: policy("compensation"),
            created_at: at,
            updated_at: at,
        };
        attempt.task_contract.content_hash = task_contract_content_hash(&contract).unwrap();
        store
            .create_workflow(&contract, &attempt, CorrelationId::new(), at)
            .unwrap();
        let actor = RuntimeActorId::new();
        let running = store
            .claim_workflow_attempt(
                attempt.attempt_id,
                attempt.revision,
                attempt.lease_generation,
                actor,
                at + chrono::Duration::minutes(5),
                CorrelationId::new(),
                "action-workflow-claim",
                at,
            )
            .unwrap();
        (contract, running, actor)
    }

    fn action_fixture(
        workflow: &WorkflowAttempt,
        actor: RuntimeActorId,
        effect_class: EffectClass,
    ) -> (ActionEnvelope, ActionExecution) {
        let operation = payload(
            "test.write_operation.v1",
            serde_json::json!({"path": "result.txt", "sha256": "a".repeat(64)}),
        );
        let envelope = ActionEnvelope {
            id: ActionId::new(),
            workflow_id: workflow.workflow_id,
            attempt_id: workflow.attempt_id,
            task_contract: workflow.task_contract.clone(),
            actor,
            origin: None,
            authority: policy("exact-worktree-write"),
            elevated_authority: (effect_class == EffectClass::Irreversible)
                .then(|| policy("explicit-irreversible-approval")),
            expected_versions: Vec::new(),
            operation_hash: operation.content_hash("TestActionOperation").unwrap(),
            operation,
            effect_class,
            preconditions: Vec::new(),
            budget_reservation: policy("one-write"),
            idempotency_key: "authorize-action-1".into(),
            compensation: Some(payload(
                "test.compensation_plan.v1",
                serde_json::json!({"kind": "restore-baseline"}),
            )),
            compensates_action: None,
            reconciler: Some(payload(
                "test.reconciler.v1",
                serde_json::json!({"kind": "exact-file-hash"}),
            )),
            reconciliation_deadline: Some(now() + chrono::Duration::minutes(5)),
            created_at: now(),
        };
        let execution = ActionExecution {
            id: ActionExecutionId::new(),
            action_id: envelope.id,
            attempt_number: 1,
            state: ActionExecutionState::Authorized,
            revision: 1,
            lease_generation: 0,
            executor: None,
            outcome: None,
            created_at: now(),
            updated_at: now(),
        };
        (envelope, execution)
    }

    fn authorize_and_confirm_action(
        store: &mut SqliteEventStore,
        workflow: &WorkflowAttempt,
        envelope: &ActionEnvelope,
        execution: &ActionExecution,
    ) -> ActionExecution {
        store
            .authorize_action(
                envelope,
                execution,
                workflow.revision,
                workflow.lease_generation,
                CorrelationId::new(),
                now(),
            )
            .unwrap();
        let dispatched = store
            .dispatch_action(
                execution.id,
                execution.revision,
                execution.lease_generation,
                RuntimeActorId::new(),
                CorrelationId::new(),
                &format!("dispatch:{}", envelope.id),
                now(),
            )
            .unwrap();
        let receipt = ActionReceipt {
            id: ActionReceiptId::new(),
            action_id: envelope.id,
            execution_id: execution.id,
            observation: payload(
                "test.action_observation.v1",
                serde_json::json!({"action_id": envelope.id, "status": "effect-observed"}),
            ),
            observed_at: now(),
        };
        let observed = store
            .observe_action(
                &receipt,
                dispatched.revision,
                dispatched.lease_generation,
                CorrelationId::new(),
                &format!("observe:{}", envelope.id),
                now(),
            )
            .unwrap();
        store
            .classify_action_outcome(
                execution.id,
                observed.revision,
                observed.lease_generation,
                EffectOutcome::EffectConfirmed {
                    evidence: Vec::new(),
                },
                None,
                CorrelationId::new(),
                &format!("classify:{}", envelope.id),
                now(),
            )
            .unwrap()
            .0
    }

    fn validate_and_commit_action(
        store: &mut SqliteEventStore,
        envelope: &ActionEnvelope,
        confirmed: &ActionExecution,
    ) -> ActionCommitReceipt {
        let validation = ActionValidation {
            action_id: envelope.id,
            execution_id: confirmed.id,
            validator: RuntimeActorId::new(),
            evidence: Vec::new(),
            postconditions: vec![payload(
                "test.action_postcondition.v1",
                serde_json::json!({"action_id": envelope.id, "valid": true}),
            )],
            validated_at: now(),
        };
        let validated = store
            .validate_action_effect(
                confirmed.id,
                confirmed.revision,
                confirmed.lease_generation,
                &validation,
                CorrelationId::new(),
                &format!("validate:{}", envelope.id),
                now(),
            )
            .unwrap();
        let validation_event = store.action_events(envelope.id).unwrap().last().unwrap().id;
        store
            .commit_action_effect(
                confirmed.id,
                validated.revision,
                validated.lease_generation,
                validation_event,
                CorrelationId::new(),
                &format!("commit:{}", envelope.id),
                now(),
            )
            .unwrap()
    }

    #[test]
    fn restart_recovery_never_redispatches_a_stranded_effectful_action() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (_contract, workflow, actor) = running_workflow(&mut store);
        let (envelope, execution) = action_fixture(&workflow, actor, EffectClass::Reversible);
        store
            .authorize_action(
                &envelope,
                &execution,
                workflow.revision,
                workflow.lease_generation,
                CorrelationId::new(),
                now(),
            )
            .unwrap();
        let dispatched = store
            .dispatch_action(
                execution.id,
                execution.revision,
                execution.lease_generation,
                RuntimeActorId::new(),
                CorrelationId::new(),
                "restart-stranded-dispatch",
                now(),
            )
            .unwrap();
        assert_eq!(dispatched.state, ActionExecutionState::Executing);

        let recovered_at = now() + chrono::Duration::seconds(1);
        let report = store
            .recover_stranded_action_executions(CorrelationId::new(), recovered_at)
            .unwrap();
        assert_eq!(report.inspected, 1);
        assert_eq!(report.outcome_unknown, 1);
        assert_eq!(report.no_effect_confirmed, 0);
        let recovered = store.action_execution(execution.id).unwrap().unwrap();
        assert_eq!(recovered.state, ActionExecutionState::OutcomeUnknown);
        assert_eq!(recovered.lease_generation, dispatched.lease_generation + 1);
        let jobs = store.due_reconciliation_jobs(recovered_at, 10).unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].adapter, envelope.reconciler.unwrap());

        assert_eq!(
            store
                .recover_stranded_action_executions(CorrelationId::new(), recovered_at)
                .unwrap(),
            ActionExecutionRecoveryReport::default()
        );
    }

    #[test]
    fn action_inspection_is_bounded_ordered_and_derived_from_canonical_records() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (_contract, workflow, actor) = running_workflow(&mut store);
        let (envelope, execution) = action_fixture(&workflow, actor, EffectClass::Reversible);
        store
            .authorize_action(
                &envelope,
                &execution,
                workflow.revision,
                workflow.lease_generation,
                CorrelationId::new(),
                now(),
            )
            .unwrap();
        let summaries = store.action_summaries(10).unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].action_id, envelope.id);
        assert_eq!(summaries[0].state, ActionExecutionState::Authorized);
        let inspection = store.action_inspection(envelope.id, 10).unwrap().unwrap();
        assert_eq!(inspection.envelope, envelope);
        assert_eq!(inspection.executions, vec![execution]);
        assert_eq!(inspection.events.len(), 1);
        assert_eq!(inspection.events[0].sequence, 1);
        assert!(store.action_summaries(0).is_err());
        assert!(store.action_inspection(envelope.id, 1_001).is_err());
        assert!(
            store
                .action_inspection(ActionId::new(), 10)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn undispatched_effectful_action_can_be_abandoned_as_proven_no_effect() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (_contract, workflow, actor) = running_workflow(&mut store);
        let (envelope, execution) = action_fixture(&workflow, actor, EffectClass::Reversible);
        store
            .authorize_action(
                &envelope,
                &execution,
                workflow.revision,
                workflow.lease_generation,
                CorrelationId::new(),
                now(),
            )
            .unwrap();
        let abandoned = store
            .abandon_authorized_action_before_dispatch(
                execution.id,
                execution.revision,
                execution.lease_generation,
                CorrelationId::new(),
                "abandon-effectful-before-dispatch",
                now(),
            )
            .unwrap();
        assert_eq!(abandoned.state, ActionExecutionState::NoEffectConfirmed);
        assert_eq!(abandoned.outcome, Some(EffectOutcome::NoEffectConfirmed));
        assert_eq!(
            store
                .abandon_authorized_action_before_dispatch(
                    execution.id,
                    execution.revision,
                    execution.lease_generation,
                    CorrelationId::new(),
                    "abandon-effectful-before-dispatch",
                    now(),
                )
                .unwrap(),
            abandoned
        );
        assert!(
            store
                .dispatch_action(
                    execution.id,
                    abandoned.revision,
                    abandoned.lease_generation,
                    actor,
                    CorrelationId::new(),
                    "dispatch-abandoned-effect",
                    now(),
                )
                .is_err()
        );
    }

    #[test]
    fn action_intent_observation_and_effect_classification_are_distinct_and_idempotent() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (_contract, workflow, actor) = running_workflow(&mut store);
        let (envelope, execution) = action_fixture(&workflow, actor, EffectClass::Reversible);
        let authorized = store
            .authorize_action(
                &envelope,
                &execution,
                workflow.revision,
                workflow.lease_generation,
                CorrelationId::new(),
                now(),
            )
            .unwrap();
        assert_eq!(authorized.execution.state, ActionExecutionState::Authorized);
        assert_eq!(
            store
                .authorize_action(
                    &envelope,
                    &execution,
                    workflow.revision,
                    workflow.lease_generation,
                    CorrelationId::new(),
                    now(),
                )
                .unwrap(),
            authorized
        );

        let executor = RuntimeActorId::new();
        let dispatched = store
            .dispatch_action(
                execution.id,
                1,
                0,
                executor,
                CorrelationId::new(),
                "dispatch-action-1",
                now(),
            )
            .unwrap();
        assert_eq!(dispatched.state, ActionExecutionState::Executing);
        assert_eq!(dispatched.lease_generation, 1);
        assert!(matches!(
            store.dispatch_action(
                execution.id,
                1,
                0,
                RuntimeActorId::new(),
                CorrelationId::new(),
                "dispatch-action-1",
                now(),
            ),
            Err(StorageError::Conflict(_))
        ));
        let receipt = ActionReceipt {
            id: ActionReceiptId::new(),
            action_id: envelope.id,
            execution_id: execution.id,
            observation: payload(
                "test.action_observation.v1",
                serde_json::json!({"status": "created", "sha256": "a".repeat(64)}),
            ),
            observed_at: now(),
        };
        let observed = store
            .observe_action(
                &receipt,
                dispatched.revision,
                dispatched.lease_generation,
                CorrelationId::new(),
                "observe-action-1",
                now(),
            )
            .unwrap();
        assert_eq!(observed.state, ActionExecutionState::Observed);
        assert!(observed.outcome.is_none());
        let outcome = EffectOutcome::EffectConfirmed {
            evidence: Vec::new(),
        };
        let (classified, job) = store
            .classify_action_outcome(
                execution.id,
                observed.revision,
                observed.lease_generation,
                outcome.clone(),
                None,
                CorrelationId::new(),
                "classify-action-1",
                now(),
            )
            .unwrap();
        assert_eq!(classified.state, ActionExecutionState::EffectConfirmed);
        assert_eq!(classified.outcome, Some(outcome));
        assert!(classified.executor.is_none());
        assert_eq!(classified.lease_generation, dispatched.lease_generation + 1);
        assert!(job.is_none());
        assert_eq!(store.action_events(envelope.id).unwrap().len(), 4);
    }

    #[test]
    fn confirmed_effect_requires_exact_validation_before_idempotent_commit() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (_contract, workflow, actor) = running_workflow(&mut store);
        let (envelope, execution) = action_fixture(&workflow, actor, EffectClass::Reversible);
        let confirmed = authorize_and_confirm_action(&mut store, &workflow, &envelope, &execution);
        assert!(matches!(
            store.commit_action_effect(
                confirmed.id,
                confirmed.revision,
                confirmed.lease_generation,
                EventId::new(),
                CorrelationId::new(),
                "commit-before-validation",
                now(),
            ),
            Err(StorageError::Conflict(_))
        ));
        let validation = ActionValidation {
            action_id: envelope.id,
            execution_id: execution.id,
            validator: RuntimeActorId::new(),
            evidence: Vec::new(),
            postconditions: vec![payload(
                "test.action_postcondition.v1",
                serde_json::json!({"sha256": "a".repeat(64)}),
            )],
            validated_at: now(),
        };
        let validated = store
            .validate_action_effect(
                execution.id,
                confirmed.revision,
                confirmed.lease_generation,
                &validation,
                CorrelationId::new(),
                "validate-confirmed-effect",
                now(),
            )
            .unwrap();
        assert_eq!(validated.state, ActionExecutionState::Validated);
        assert_eq!(
            store
                .validate_action_effect(
                    execution.id,
                    confirmed.revision,
                    confirmed.lease_generation,
                    &validation,
                    CorrelationId::new(),
                    "validate-confirmed-effect",
                    now(),
                )
                .unwrap(),
            validated
        );
        let mut substituted = validation.clone();
        substituted.validator = RuntimeActorId::new();
        assert!(matches!(
            store.validate_action_effect(
                execution.id,
                confirmed.revision,
                confirmed.lease_generation,
                &substituted,
                CorrelationId::new(),
                "validate-confirmed-effect",
                now(),
            ),
            Err(StorageError::Conflict(_))
        ));
        let validation_event = store.action_events(envelope.id).unwrap().last().unwrap().id;
        let committed = store
            .commit_action_effect(
                execution.id,
                validated.revision,
                validated.lease_generation,
                validation_event,
                CorrelationId::new(),
                "commit-confirmed-effect",
                now(),
            )
            .unwrap();
        assert_eq!(committed.execution.state, ActionExecutionState::Committed);
        assert!(committed.compensated_execution.is_none());
        assert_eq!(committed.event.causation_id, Some(validation_event));
        assert_eq!(
            store
                .commit_action_effect(
                    execution.id,
                    validated.revision,
                    validated.lease_generation,
                    validation_event,
                    CorrelationId::new(),
                    "commit-confirmed-effect",
                    now(),
                )
                .unwrap(),
            committed
        );
        assert!(matches!(
            store.commit_action_effect(
                execution.id,
                validated.revision,
                validated.lease_generation,
                EventId::new(),
                CorrelationId::new(),
                "commit-confirmed-effect",
                now(),
            ),
            Err(StorageError::Conflict(_))
        ));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn compensation_is_a_separate_authorized_action_with_causal_history() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (_contract, workflow, actor) = running_workflow(&mut store);
        let (original_envelope, original_execution) =
            action_fixture(&workflow, actor, EffectClass::Reversible);
        let original_confirmed = authorize_and_confirm_action(
            &mut store,
            &workflow,
            &original_envelope,
            &original_execution,
        );
        let original_commit =
            validate_and_commit_action(&mut store, &original_envelope, &original_confirmed);
        assert_eq!(
            original_commit.execution.state,
            ActionExecutionState::Committed
        );

        let (mut compensation_envelope, compensation_execution) =
            action_fixture(&workflow, actor, EffectClass::Reversible);
        compensation_envelope.idempotency_key = "authorize-compensation".into();
        compensation_envelope.operation = original_envelope.compensation.clone().unwrap();
        compensation_envelope.operation_hash = compensation_envelope
            .operation
            .content_hash("TestActionOperation")
            .unwrap();
        compensation_envelope.compensates_action = Some(original_envelope.id);
        let compensation_confirmed = authorize_and_confirm_action(
            &mut store,
            &workflow,
            &compensation_envelope,
            &compensation_execution,
        );
        let compensation_commit =
            validate_and_commit_action(&mut store, &compensation_envelope, &compensation_confirmed);
        assert_eq!(
            compensation_commit.execution.state,
            ActionExecutionState::Committed
        );
        let compensated = compensation_commit.compensated_execution.unwrap();
        assert_eq!(compensated.action_id, original_envelope.id);
        assert_eq!(compensated.state, ActionExecutionState::Compensated);
        assert_eq!(
            store
                .action_execution(original_execution.id)
                .unwrap()
                .unwrap(),
            compensated
        );
        let original_events = store.action_events(original_envelope.id).unwrap();
        let compensated_event = original_events.last().unwrap();
        assert_eq!(
            compensated_event.type_id.as_str(),
            ACTION_COMPENSATED_TYPE_ID
        );
        assert_eq!(
            compensated_event.causation_id,
            Some(compensation_commit.event.id)
        );
        assert!(
            original_events
                .iter()
                .any(|event| event.type_id.as_str() == ACTION_COMMITTED_TYPE_ID)
        );
        assert!(
            store
                .action_events(compensation_envelope.id)
                .unwrap()
                .iter()
                .any(|event| event.type_id.as_str() == ACTION_COMMITTED_TYPE_ID)
        );
        let original_inspection = store
            .action_inspection(original_envelope.id, 10)
            .unwrap()
            .unwrap();
        assert_eq!(
            original_inspection.compensation_actions,
            vec![compensation_envelope.id]
        );
        let compensation_inspection = store
            .action_inspection(compensation_envelope.id, 10)
            .unwrap()
            .unwrap();
        assert_eq!(
            compensation_inspection.envelope.compensates_action,
            Some(original_envelope.id)
        );
        assert!(compensation_inspection.compensation_actions.is_empty());

        let (mut duplicate_compensation, duplicate_execution) =
            action_fixture(&workflow, actor, EffectClass::Reversible);
        duplicate_compensation.idempotency_key = "duplicate-compensation".into();
        duplicate_compensation.operation = original_envelope.compensation.clone().unwrap();
        duplicate_compensation.operation_hash = duplicate_compensation
            .operation
            .content_hash("TestActionOperation")
            .unwrap();
        duplicate_compensation.compensates_action = Some(original_envelope.id);
        assert!(matches!(
            store.authorize_action(
                &duplicate_compensation,
                &duplicate_execution,
                workflow.revision,
                workflow.lease_generation,
                CorrelationId::new(),
                now(),
            ),
            Err(StorageError::Conflict(_))
        ));
    }

    #[test]
    fn lost_receipt_becomes_unknown_and_creates_reconciliation_without_redispatch() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (_contract, workflow, actor) = running_workflow(&mut store);
        let (envelope, execution) = action_fixture(&workflow, actor, EffectClass::Consequential);
        store
            .authorize_action(
                &envelope,
                &execution,
                workflow.revision,
                workflow.lease_generation,
                CorrelationId::new(),
                now(),
            )
            .unwrap();
        let dispatched = store
            .dispatch_action(
                execution.id,
                1,
                0,
                RuntimeActorId::new(),
                CorrelationId::new(),
                "dispatch-unknown",
                now(),
            )
            .unwrap();
        let outcome = EffectOutcome::OutcomeUnknown {
            hint: payload(
                "test.reconciliation_hint.v1",
                serde_json::json!({"reason": "receipt-lost-after-dispatch"}),
            ),
        };
        let deadline = now() + chrono::Duration::minutes(5);
        let first = store
            .classify_action_outcome(
                execution.id,
                dispatched.revision,
                dispatched.lease_generation,
                outcome,
                Some(deadline),
                CorrelationId::new(),
                "classify-unknown",
                now(),
            )
            .unwrap();
        assert_eq!(first.0.state, ActionExecutionState::OutcomeUnknown);
        assert_eq!(first.1.as_ref().unwrap().deadline, deadline);
        assert_eq!(
            store
                .classify_action_outcome(
                    execution.id,
                    dispatched.revision,
                    dispatched.lease_generation,
                    first.0.outcome.clone().unwrap(),
                    Some(deadline),
                    CorrelationId::new(),
                    "classify-unknown",
                    now(),
                )
                .unwrap(),
            first
        );
        assert!(matches!(
            store.classify_action_outcome(
                execution.id,
                dispatched.revision,
                dispatched.lease_generation,
                first.0.outcome.clone().unwrap(),
                Some(deadline + chrono::Duration::seconds(1)),
                CorrelationId::new(),
                "classify-unknown",
                now(),
            ),
            Err(StorageError::Conflict(_))
        ));
        assert!(matches!(
            store.dispatch_action(
                execution.id,
                first.0.revision,
                first.0.lease_generation,
                RuntimeActorId::new(),
                CorrelationId::new(),
                "blind-redispatch",
                now(),
            ),
            Err(StorageError::Conflict(_))
        ));
    }

    #[test]
    fn workflow_cancellation_fences_dispatched_effect_and_requires_reconciliation() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (_contract, workflow, actor) = running_workflow(&mut store);
        let (envelope, execution) = action_fixture(&workflow, actor, EffectClass::Consequential);
        store
            .authorize_action(
                &envelope,
                &execution,
                workflow.revision,
                workflow.lease_generation,
                CorrelationId::new(),
                now(),
            )
            .unwrap();
        store
            .dispatch_action(
                execution.id,
                execution.revision,
                execution.lease_generation,
                RuntimeActorId::new(),
                CorrelationId::new(),
                "dispatch-before-cancel",
                now(),
            )
            .unwrap();

        let cancelling = store
            .transition_workflow_attempt(
                workflow.attempt_id,
                workflow.revision,
                workflow.lease_generation,
                WorkflowState::CancelRequested,
                CorrelationId::new(),
                "cancel-with-dispatched-effect",
                now(),
            )
            .unwrap();
        let fenced = store.action_execution(execution.id).unwrap().unwrap();
        assert_eq!(fenced.state, ActionExecutionState::OutcomeUnknown);
        assert!(matches!(
            fenced.outcome,
            Some(EffectOutcome::OutcomeUnknown { .. })
        ));
        assert!(fenced.executor.is_none());
        assert!(matches!(
            store.transition_workflow_attempt(
                workflow.attempt_id,
                cancelling.revision,
                cancelling.lease_generation,
                WorkflowState::Cancelled,
                CorrelationId::new(),
                "cancel-too-early",
                now(),
            ),
            Err(StorageError::Conflict(_))
        ));
        let recovery = store
            .recover_expired_workflow_leases(CorrelationId::new(), now())
            .unwrap();
        assert_eq!(recovery.cancellations_awaiting_reconciliation, 1);
        let waiting = store
            .workflow_attempt(workflow.attempt_id)
            .unwrap()
            .unwrap();
        assert_eq!(
            waiting.state,
            WorkflowState::Waiting(mcg_contracts::WaitReason::Reconciliation)
        );
        assert_eq!(store.action_events(envelope.id).unwrap().len(), 3);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn reconciliation_confirms_no_effect_and_settles_cancellation_without_redispatch() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (_contract, workflow, actor) = running_workflow(&mut store);
        let (envelope, execution) = action_fixture(&workflow, actor, EffectClass::Consequential);
        store
            .authorize_action(
                &envelope,
                &execution,
                workflow.revision,
                workflow.lease_generation,
                CorrelationId::new(),
                now(),
            )
            .unwrap();
        store
            .dispatch_action(
                execution.id,
                execution.revision,
                execution.lease_generation,
                RuntimeActorId::new(),
                CorrelationId::new(),
                "dispatch-before-cancel-resolution",
                now(),
            )
            .unwrap();
        store
            .transition_workflow_attempt(
                workflow.attempt_id,
                workflow.revision,
                workflow.lease_generation,
                WorkflowState::CancelRequested,
                CorrelationId::new(),
                "cancel-for-reconciliation-resolution",
                now(),
            )
            .unwrap();
        store
            .recover_expired_workflow_leases(CorrelationId::new(), now())
            .unwrap();

        let unknown = store.action_execution(execution.id).unwrap().unwrap();
        let job = load_reconciliation_job_for_execution(&store.connection, execution.id)
            .unwrap()
            .unwrap();
        assert_eq!(
            store.due_reconciliation_jobs(now(), 10).unwrap(),
            vec![job.clone()]
        );
        let worker = RuntimeActorId::new();
        let lease_expires_at = now() + chrono::Duration::minutes(1);
        let claimed = store
            .claim_reconciliation_job(
                job.id,
                job.revision,
                job.lease_generation,
                unknown.revision,
                unknown.lease_generation,
                worker,
                lease_expires_at,
                CorrelationId::new(),
                "claim-cancel-reconciliation",
                now(),
            )
            .unwrap();
        assert_eq!(claimed.job.state, ReconciliationJobState::Running);
        assert_eq!(claimed.job.attempts, 1);
        assert_eq!(claimed.execution.state, ActionExecutionState::Reconciling);
        assert!(store.due_reconciliation_jobs(now(), 10).unwrap().is_empty());
        assert!(matches!(
            store.claim_reconciliation_job(
                job.id,
                job.revision,
                job.lease_generation,
                unknown.revision,
                unknown.lease_generation,
                RuntimeActorId::new(),
                lease_expires_at,
                CorrelationId::new(),
                "claim-cancel-reconciliation",
                now(),
            ),
            Err(StorageError::Conflict(_))
        ));

        let observation = ReconciliationObservation::ConfirmedNotOccurred;
        let resolved = store
            .resolve_reconciliation_job(
                claimed.job.id,
                claimed.job.revision,
                claimed.job.lease_generation,
                claimed.execution.revision,
                claimed.execution.lease_generation,
                worker,
                &observation,
                CorrelationId::new(),
                "resolve-cancel-no-effect",
                now(),
            )
            .unwrap();
        assert_eq!(resolved.job.state, ReconciliationJobState::Resolved);
        assert_eq!(
            resolved.execution.state,
            ActionExecutionState::NoEffectConfirmed
        );
        assert_eq!(
            store
                .resolve_reconciliation_job(
                    claimed.job.id,
                    claimed.job.revision,
                    claimed.job.lease_generation,
                    claimed.execution.revision,
                    claimed.execution.lease_generation,
                    worker,
                    &observation,
                    CorrelationId::new(),
                    "resolve-cancel-no-effect",
                    now(),
                )
                .unwrap(),
            resolved
        );
        let recovery = store
            .recover_expired_workflow_leases(CorrelationId::new(), now())
            .unwrap();
        assert_eq!(recovery.settled_cancellations, 1);
        assert_eq!(
            store
                .workflow_attempt(workflow.attempt_id)
                .unwrap()
                .unwrap()
                .state,
            WorkflowState::Cancelled
        );
        assert!(matches!(
            store.dispatch_action(
                execution.id,
                resolved.execution.revision,
                resolved.execution.lease_generation,
                RuntimeActorId::new(),
                CorrelationId::new(),
                "never-redispatch-reconciled-action",
                now(),
            ),
            Err(StorageError::Conflict(_))
        ));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn reconciliation_defers_unknown_then_confirms_occurrence_with_immutable_receipt() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (_contract, workflow, actor) = running_workflow(&mut store);
        let (envelope, execution) = action_fixture(&workflow, actor, EffectClass::Consequential);
        store
            .authorize_action(
                &envelope,
                &execution,
                workflow.revision,
                workflow.lease_generation,
                CorrelationId::new(),
                now(),
            )
            .unwrap();
        let dispatched = store
            .dispatch_action(
                execution.id,
                execution.revision,
                execution.lease_generation,
                RuntimeActorId::new(),
                CorrelationId::new(),
                "dispatch-for-reconciliation-loop",
                now(),
            )
            .unwrap();
        let unknown_outcome = EffectOutcome::OutcomeUnknown {
            hint: payload(
                "test.reconciliation_hint.v1",
                serde_json::json!({"reason": "lost receipt"}),
            ),
        };
        let (unknown, job) = store
            .classify_action_outcome(
                execution.id,
                dispatched.revision,
                dispatched.lease_generation,
                unknown_outcome,
                envelope.reconciliation_deadline,
                CorrelationId::new(),
                "classify-for-reconciliation-loop",
                now(),
            )
            .unwrap();
        let job = job.unwrap();
        let worker = RuntimeActorId::new();
        let first_claim = store
            .claim_reconciliation_job(
                job.id,
                job.revision,
                job.lease_generation,
                unknown.revision,
                unknown.lease_generation,
                worker,
                now() + chrono::Duration::minutes(1),
                CorrelationId::new(),
                "claim-reconciliation-first",
                now(),
            )
            .unwrap();
        let next_attempt_at = now() + chrono::Duration::minutes(2);
        let still_unknown = ReconciliationObservation::StillUnknown {
            hint: payload(
                "test.reconciliation_hint.v1",
                serde_json::json!({"reason": "filesystem temporarily unavailable"}),
            ),
            next_attempt_at,
        };
        let deferred = store
            .resolve_reconciliation_job(
                job.id,
                first_claim.job.revision,
                first_claim.job.lease_generation,
                first_claim.execution.revision,
                first_claim.execution.lease_generation,
                worker,
                &still_unknown,
                CorrelationId::new(),
                "defer-reconciliation",
                now(),
            )
            .unwrap();
        assert_eq!(deferred.job.state, ReconciliationJobState::Pending);
        assert_eq!(deferred.job.next_attempt_at, next_attempt_at);
        assert_eq!(
            deferred.execution.state,
            ActionExecutionState::OutcomeUnknown
        );
        assert!(
            store
                .due_reconciliation_jobs(now() + chrono::Duration::minutes(1), 10)
                .unwrap()
                .is_empty()
        );

        let second_worker = RuntimeActorId::new();
        let second_claim = store
            .claim_reconciliation_job(
                job.id,
                deferred.job.revision,
                deferred.job.lease_generation,
                deferred.execution.revision,
                deferred.execution.lease_generation,
                second_worker,
                now() + chrono::Duration::minutes(3),
                CorrelationId::new(),
                "claim-reconciliation-second",
                next_attempt_at,
            )
            .unwrap();
        let receipt = ActionReceipt {
            id: ActionReceiptId::new(),
            action_id: envelope.id,
            execution_id: execution.id,
            observation: payload(
                "test.action_observation.v1",
                serde_json::json!({"status": "already-created", "sha256": "a".repeat(64)}),
            ),
            observed_at: next_attempt_at,
        };
        let confirmed = ReconciliationObservation::ConfirmedOccurred {
            receipt: receipt.clone(),
            evidence: Vec::new(),
        };
        let resolved = store
            .resolve_reconciliation_job(
                job.id,
                second_claim.job.revision,
                second_claim.job.lease_generation,
                second_claim.execution.revision,
                second_claim.execution.lease_generation,
                second_worker,
                &confirmed,
                CorrelationId::new(),
                "resolve-reconciliation-occurred",
                next_attempt_at,
            )
            .unwrap();
        assert_eq!(resolved.job.state, ReconciliationJobState::Resolved);
        assert_eq!(
            resolved.execution.state,
            ActionExecutionState::EffectConfirmed
        );
        assert_eq!(resolved.receipt, Some(receipt));
        assert!(matches!(
            store.resolve_reconciliation_job(
                job.id,
                second_claim.job.revision,
                second_claim.job.lease_generation,
                second_claim.execution.revision,
                second_claim.execution.lease_generation,
                second_worker,
                &ReconciliationObservation::ConfirmedNotOccurred,
                CorrelationId::new(),
                "resolve-reconciliation-occurred",
                next_attempt_at,
            ),
            Err(StorageError::Conflict(_))
        ));
    }

    #[test]
    fn reconciliation_recovery_fences_late_workers_and_escalates_at_deadline() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (_contract, workflow, actor) = running_workflow(&mut store);
        let (envelope, execution) = action_fixture(&workflow, actor, EffectClass::Consequential);
        store
            .authorize_action(
                &envelope,
                &execution,
                workflow.revision,
                workflow.lease_generation,
                CorrelationId::new(),
                now(),
            )
            .unwrap();
        let dispatched = store
            .dispatch_action(
                execution.id,
                execution.revision,
                execution.lease_generation,
                RuntimeActorId::new(),
                CorrelationId::new(),
                "dispatch-before-reconciler-crash",
                now(),
            )
            .unwrap();
        let (unknown, job) = store
            .classify_action_outcome(
                execution.id,
                dispatched.revision,
                dispatched.lease_generation,
                EffectOutcome::OutcomeUnknown {
                    hint: payload(
                        "test.reconciliation_hint.v1",
                        serde_json::json!({"reason": "lost receipt"}),
                    ),
                },
                envelope.reconciliation_deadline,
                CorrelationId::new(),
                "classify-before-reconciler-crash",
                now(),
            )
            .unwrap();
        let job = job.unwrap();
        let worker = RuntimeActorId::new();
        let claimed = store
            .claim_reconciliation_job(
                job.id,
                job.revision,
                job.lease_generation,
                unknown.revision,
                unknown.lease_generation,
                worker,
                now() + chrono::Duration::seconds(30),
                CorrelationId::new(),
                "claim-before-reconciler-crash",
                now(),
            )
            .unwrap();
        let recovered_at = now() + chrono::Duration::minutes(1);
        let recovery = store
            .recover_reconciliation_jobs(CorrelationId::new(), recovered_at)
            .unwrap();
        assert_eq!(recovery.inspected_running, 1);
        assert_eq!(recovery.requeued, 1);
        let recovered_job = store.reconciliation_job(job.id).unwrap().unwrap();
        let recovered_execution = store.action_execution(execution.id).unwrap().unwrap();
        assert_eq!(recovered_job.state, ReconciliationJobState::Pending);
        assert_eq!(
            recovered_execution.state,
            ActionExecutionState::OutcomeUnknown
        );
        assert!(matches!(
            store.resolve_reconciliation_job(
                job.id,
                claimed.job.revision,
                claimed.job.lease_generation,
                claimed.execution.revision,
                claimed.execution.lease_generation,
                worker,
                &ReconciliationObservation::ConfirmedNotOccurred,
                CorrelationId::new(),
                "late-reconciler-result",
                recovered_at,
            ),
            Err(StorageError::Conflict(_))
        ));

        let deadline_recovery = store
            .recover_reconciliation_jobs(
                CorrelationId::new(),
                envelope.reconciliation_deadline.unwrap(),
            )
            .unwrap();
        assert_eq!(deadline_recovery.escalated, 1);
        assert_eq!(
            store.reconciliation_job(job.id).unwrap().unwrap().state,
            ReconciliationJobState::UserActionRequired
        );
        assert_eq!(
            store.action_execution(execution.id).unwrap().unwrap().state,
            ActionExecutionState::OutcomeUnknown
        );
    }

    #[test]
    fn reconciliation_lease_recovery_survives_real_store_reopen() {
        let temporary = tempfile::tempdir().unwrap();
        let database = temporary.path().join("reconciliation-restart.db");
        let (job_id, execution_id, claimed_generation) = {
            let mut store = SqliteEventStore::open(&database).unwrap();
            let (_contract, workflow, actor) = running_workflow(&mut store);
            let (envelope, execution) =
                action_fixture(&workflow, actor, EffectClass::Consequential);
            store
                .authorize_action(
                    &envelope,
                    &execution,
                    workflow.revision,
                    workflow.lease_generation,
                    CorrelationId::new(),
                    now(),
                )
                .unwrap();
            let dispatched = store
                .dispatch_action(
                    execution.id,
                    execution.revision,
                    execution.lease_generation,
                    RuntimeActorId::new(),
                    CorrelationId::new(),
                    "restart-dispatch",
                    now(),
                )
                .unwrap();
            let (unknown, job) = store
                .classify_action_outcome(
                    execution.id,
                    dispatched.revision,
                    dispatched.lease_generation,
                    EffectOutcome::OutcomeUnknown {
                        hint: payload(
                            "test.reconciliation_hint.v1",
                            serde_json::json!({"reason": "root stopped"}),
                        ),
                    },
                    envelope.reconciliation_deadline,
                    CorrelationId::new(),
                    "restart-classify",
                    now(),
                )
                .unwrap();
            let job = job.unwrap();
            let claimed = store
                .claim_reconciliation_job(
                    job.id,
                    job.revision,
                    job.lease_generation,
                    unknown.revision,
                    unknown.lease_generation,
                    RuntimeActorId::new(),
                    now() + chrono::Duration::seconds(30),
                    CorrelationId::new(),
                    "restart-claim",
                    now(),
                )
                .unwrap();
            (job.id, execution.id, claimed.job.lease_generation)
        };

        {
            let mut reopened = SqliteEventStore::open(&database).unwrap();
            let report = reopened
                .recover_reconciliation_jobs(
                    CorrelationId::new(),
                    now() + chrono::Duration::minutes(1),
                )
                .unwrap();
            assert_eq!(report.inspected_running, 1);
            assert_eq!(report.requeued, 1);
        }

        let reopened = SqliteEventStore::open(&database).unwrap();
        let job = reopened.reconciliation_job(job_id).unwrap().unwrap();
        let execution = reopened.action_execution(execution_id).unwrap().unwrap();
        assert_eq!(job.state, ReconciliationJobState::Pending);
        assert!(job.lease_generation > claimed_generation);
        assert!(job.lease_owner.is_none());
        assert_eq!(execution.state, ActionExecutionState::OutcomeUnknown);
        assert!(execution.executor.is_none());
        assert!(
            reopened
                .action_events(execution.action_id)
                .unwrap()
                .iter()
                .any(|event| event.type_id.as_str() == RECONCILIATION_LEASE_RECOVERED_TYPE_ID)
        );
    }

    #[test]
    fn schema_fifty_four_action_history_migrates_to_causal_compensation_schema() {
        let temporary = tempfile::tempdir().unwrap();
        let database = temporary.path().join("schema-54-actions.db");
        let action_id = {
            let mut store = SqliteEventStore::open(&database).unwrap();
            let (_contract, workflow, actor) = running_workflow(&mut store);
            let (envelope, execution) = action_fixture(&workflow, actor, EffectClass::Reversible);
            store
                .authorize_action(
                    &envelope,
                    &execution,
                    workflow.revision,
                    workflow.lease_generation,
                    CorrelationId::new(),
                    now(),
                )
                .unwrap();
            envelope.id
        };
        rusqlite::Connection::open(&database)
            .unwrap()
            .execute_batch(
                "DROP TABLE workflow_source_attachments;
                 DROP TABLE model_secret_handle_bindings;
                 DROP TABLE model_invocation_permits;
                 DROP TABLE model_invocation_proposals;
                 DROP TABLE completion_certificate_invocations;
                 DROP TABLE completion_certificates;
                 DROP TABLE model_run_heads;
                 DROP TABLE model_run_versions;
                 DROP TABLE invocation_manifests;
                 DROP TABLE context_manifests;
                 DROP INDEX action_envelopes_one_compensation;
                 ALTER TABLE action_envelopes DROP COLUMN compensates_action_id;
                 DELETE FROM mcg_schema WHERE version IN (55, 56, 57, 58, 59, 60);",
            )
            .unwrap();

        let store = SqliteEventStore::open(&database).unwrap();
        assert_eq!(
            store
                .connection
                .query_row("SELECT MAX(version) FROM mcg_schema", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap(),
            crate::STORAGE_SCHEMA_VERSION
        );
        let envelope = store.action_envelope(action_id).unwrap().unwrap();
        assert!(envelope.compensates_action.is_none());
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('action_envelopes')
                     WHERE name = 'compensates_action_id'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            1
        );
    }

    #[test]
    fn authorization_rejects_substitution_stale_ownership_and_missing_reconciler() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (_contract, workflow, actor) = running_workflow(&mut store);
        let (mut envelope, execution) = action_fixture(&workflow, actor, EffectClass::Irreversible);
        envelope.reconciler = None;
        assert!(matches!(
            store.authorize_action(
                &envelope,
                &execution,
                workflow.revision,
                workflow.lease_generation,
                CorrelationId::new(),
                now(),
            ),
            Err(StorageError::InvalidData(_))
        ));

        let (mut envelope, execution) = action_fixture(&workflow, actor, EffectClass::Irreversible);
        envelope.elevated_authority = None;
        assert!(matches!(
            store.authorize_action(
                &envelope,
                &execution,
                workflow.revision,
                workflow.lease_generation,
                CorrelationId::new(),
                now(),
            ),
            Err(StorageError::InvalidData(_))
        ));

        let (mut envelope, execution) = action_fixture(&workflow, actor, EffectClass::Reversible);
        envelope.operation.bytes.push(0);
        assert!(matches!(
            store.authorize_action(
                &envelope,
                &execution,
                workflow.revision,
                workflow.lease_generation,
                CorrelationId::new(),
                now(),
            ),
            Err(StorageError::InvalidData(_))
        ));

        let (envelope, execution) =
            action_fixture(&workflow, RuntimeActorId::new(), EffectClass::Reversible);
        assert!(matches!(
            store.authorize_action(
                &envelope,
                &execution,
                workflow.revision,
                workflow.lease_generation,
                CorrelationId::new(),
                now(),
            ),
            Err(StorageError::Conflict(_))
        ));
    }

    #[test]
    fn action_origin_is_backward_compatible_bounded_and_replay_immutable() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (_contract, workflow, actor) = running_workflow(&mut store);
        let (mut envelope, execution) = action_fixture(&workflow, actor, EffectClass::Reversible);
        let origin = ActionOrigin {
            conversation: Some(origin_source(
                "mcg.conversation.v1",
                "00000000-0000-4000-8000-000000000101",
                "conversation",
            )),
            invocation: Some(origin_source(
                "mcg.invocation_manifest.v1",
                "00000000-0000-4000-8000-000000000102",
                "invocation",
            )),
            command: Some(origin_source(
                "mcg.command_execution.v1",
                "command-103",
                "command",
            )),
            approval: Some(origin_source(
                "mcg.approval_request.v1",
                "00000000-0000-4000-8000-000000000104",
                "approval",
            )),
            agent: Some(origin_source(
                "mcg.runtime_actor.v1",
                &actor.to_string(),
                "actor",
            )),
            evidence: vec![origin_source(
                "mcg.action_evidence.v1",
                "artifact-105",
                "evidence",
            )],
        };
        envelope.origin = Some(origin.clone());
        store
            .authorize_action(
                &envelope,
                &execution,
                workflow.revision,
                workflow.lease_generation,
                CorrelationId::new(),
                now(),
            )
            .unwrap();
        assert_eq!(
            store
                .action_inspection(envelope.id, 10)
                .unwrap()
                .unwrap()
                .envelope
                .origin,
            Some(origin)
        );

        let mut substituted = envelope.clone();
        substituted
            .origin
            .as_mut()
            .unwrap()
            .approval
            .as_mut()
            .unwrap()
            .locator = "00000000-0000-4000-8000-000000000999".into();
        assert!(matches!(
            store.authorize_action(
                &substituted,
                &execution,
                workflow.revision,
                workflow.lease_generation,
                CorrelationId::new(),
                now(),
            ),
            Err(StorageError::Conflict(_))
        ));

        let mut legacy_json = serde_json::to_value(&envelope).unwrap();
        legacy_json.as_object_mut().unwrap().remove("origin");
        let legacy: ActionEnvelope = serde_json::from_value(legacy_json).unwrap();
        assert!(legacy.origin.is_none());

        let (mut invalid, invalid_execution) =
            action_fixture(&workflow, actor, EffectClass::Reversible);
        invalid.origin = Some(ActionOrigin {
            conversation: None,
            invocation: None,
            command: None,
            approval: None,
            agent: None,
            evidence: Vec::new(),
        });
        assert!(matches!(
            store.authorize_action(
                &invalid,
                &invalid_execution,
                workflow.revision,
                workflow.lease_generation,
                CorrelationId::new(),
                now(),
            ),
            Err(StorageError::InvalidData(_))
        ));
    }
}
