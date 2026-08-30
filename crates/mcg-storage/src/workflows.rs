use chrono::{DateTime, Utc};
use mcg_contracts::{
    AttemptId, ContentHash, CorrelationId, EventId, RuntimeActorId, StableTypeId, TaskContract,
    TaskContractId, TaskContractStatus, VersionedPayload, VersionedRef, WaitReason,
    WorkflowAttempt, WorkflowEvent, WorkflowId, WorkflowState, decode_canonical_cbor_v1,
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::{
    SqliteEventStore, StorageError,
    actions::{cancellation_effect_status, fence_actions_for_workflow_cancellation},
};

const TASK_CONTRACT_HASH_DOMAIN: &str = "MetaCog.TaskContract";
const WORKFLOW_CREATED_TYPE_ID: &str = "mcg.workflow_created.v1";
const WORKFLOW_CLAIMED_TYPE_ID: &str = "mcg.workflow_claimed.v1";
const WORKFLOW_HEARTBEAT_TYPE_ID: &str = "mcg.workflow_heartbeat.v1";
const WORKFLOW_LEASE_EXPIRED_TYPE_ID: &str = "mcg.workflow_lease_expired.v1";
const WORKFLOW_WAIT_SATISFIED_TYPE_ID: &str = "mcg.workflow_wait_satisfied.v1";
const WORKFLOW_CANCELLATION_RECOVERED_TYPE_ID: &str = "mcg.workflow_cancellation_recovered.v1";
const WORKFLOW_RETRIED_TYPE_ID: &str = "mcg.workflow_retried.v1";
const WORKFLOW_TRANSITIONED_TYPE_ID: &str = "mcg.workflow_transitioned.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkflowRecoveryReport {
    pub inspected_nonterminal: u64,
    pub fenced_expired: u64,
    pub awaiting_reconciliation: u64,
    pub requeued_for_verification: u64,
    pub requeued_satisfied_dependencies: u64,
    pub requeued_elapsed_backoffs: u64,
    pub settled_cancellations: u64,
    pub cancellations_awaiting_reconciliation: u64,
    pub quarantined_cancellations: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskContractHead {
    pub contract: TaskContract,
    pub revision: u64,
}

/// Computes the authoritative canonical hash for a task-contract version.
///
/// # Errors
/// Returns a storage error when the contract cannot be represented by canonical v1 encoding.
pub fn task_contract_content_hash(contract: &TaskContract) -> Result<ContentHash, StorageError> {
    let value = serde_json::to_value(contract)?;
    let payload = VersionedPayload::canonical_v1(stable_type_id(TaskContract::TYPE_ID)?, &value)
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    payload
        .content_hash(TASK_CONTRACT_HASH_DOMAIN)
        .map_err(|error| StorageError::InvalidData(error.to_string()))
}

impl SqliteEventStore {
    /// Atomically persists one task-contract version, its initial Ready attempt, and event 1.
    ///
    /// Replaying the exact `(workflow_id, idempotency_key)` request returns its original event.
    ///
    /// # Errors
    /// Returns a storage error for an invalid initial contract/attempt, conflicting replay, or
    /// failed transaction.
    pub fn create_workflow(
        &mut self,
        contract: &TaskContract,
        attempt: &WorkflowAttempt,
        correlation_id: CorrelationId,
        now: DateTime<Utc>,
    ) -> Result<WorkflowEvent, StorageError> {
        validate_initial_workflow(contract, attempt)?;
        let transaction = self.connection.transaction()?;

        if let Some(existing) = load_attempt_by_idempotency(
            &transaction,
            attempt.workflow_id,
            &attempt.idempotency_key,
        )? {
            let event = load_event_by_idempotency(
                &transaction,
                attempt.workflow_id,
                &attempt.idempotency_key,
            )?
            .ok_or_else(|| {
                StorageError::InvalidData(
                    "workflow exists without its atomic creation event".into(),
                )
            })?;
            if event.type_id.as_str() != WORKFLOW_CREATED_TYPE_ID
                || event_attempt(&event)? != *attempt
                || existing.workflow_id != attempt.workflow_id
            {
                return Err(StorageError::Conflict(
                    "workflow create idempotency key was replayed with different content".into(),
                ));
            }
            transaction.commit()?;
            return Ok(event);
        }

        let contract_hash = task_contract_content_hash(contract)?;
        if attempt.task_contract.content_hash != contract_hash {
            return Err(StorageError::InvalidData(
                "workflow attempt does not bind the canonical task-contract hash".into(),
            ));
        }

        transaction.execute(
            "INSERT INTO task_contract_versions(
                task_contract_id, version, type_id, schema_version, content_hash_json,
                contract_json, status, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                contract.id.to_string(),
                sql_u64(contract.version, "task contract version")?,
                TaskContract::TYPE_ID,
                i64::from(TaskContract::SCHEMA_VERSION),
                serde_json::to_string(&contract_hash)?,
                serde_json::to_string(contract)?,
                task_contract_status_label(contract.status),
                contract.created_at.to_rfc3339(),
            ],
        )?;
        transaction.execute(
            "INSERT INTO task_contract_heads(task_contract_id, current_version, revision)
             VALUES (?1, ?2, 1)",
            params![
                contract.id.to_string(),
                sql_u64(contract.version, "task contract version")?,
            ],
        )?;
        for dependency in &contract.dependencies {
            transaction.execute(
                "INSERT INTO task_contract_dependencies(
                    task_contract_id, task_contract_version, dependency_id, dependency_version
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    contract.id.to_string(),
                    sql_u64(contract.version, "task contract version")?,
                    dependency.id.to_string(),
                    sql_u64(dependency.version, "dependency version")?,
                ],
            )?;
        }
        insert_attempt(&transaction, attempt)?;

        let event = workflow_event(
            attempt,
            1,
            correlation_id,
            None,
            &attempt.idempotency_key,
            WORKFLOW_CREATED_TYPE_ID,
            now,
            &serde_json::json!({
                "task_contract_id": contract.id,
                "task_contract_version": contract.version,
                "attempt_id": attempt.attempt_id,
                "state": attempt.state,
                "attempt": attempt,
            }),
        )?;
        insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    /// Loads one authoritative task-contract version.
    ///
    /// # Errors
    /// Returns a storage error for query or stored-data failures.
    pub fn task_contract_version(
        &self,
        id: TaskContractId,
        version: u64,
    ) -> Result<Option<TaskContract>, StorageError> {
        self.connection
            .query_row(
                "SELECT contract_json FROM task_contract_versions
                 WHERE task_contract_id = ?1 AND version = ?2",
                params![id.to_string(), sql_u64(version, "task contract version")?],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
            .transpose()
    }

    /// Loads the immutable contract version selected by the authoritative head.
    ///
    /// # Errors
    /// Returns a storage error for query or stored-data failures.
    pub fn task_contract_head(
        &self,
        id: TaskContractId,
    ) -> Result<Option<TaskContractHead>, StorageError> {
        self.connection
            .query_row(
                "SELECT version.contract_json, head.revision
                 FROM task_contract_heads AS head
                 JOIN task_contract_versions AS version
                   ON version.task_contract_id = head.task_contract_id
                  AND version.version = head.current_version
                 WHERE head.task_contract_id = ?1",
                [id.to_string()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?
            .map(|(json, revision)| {
                Ok(TaskContractHead {
                    contract: serde_json::from_str(&json)?,
                    revision: u64::try_from(revision).map_err(|_| {
                        StorageError::InvalidData("negative task-contract head revision".into())
                    })?,
                })
            })
            .transpose()
    }

    /// Appends one immutable material contract revision and advances its head with CAS.
    ///
    /// Exact replay returns the original resulting head. A reused key, stale head, skipped
    /// version, or incorrect previous-version hash fails closed.
    ///
    /// # Errors
    /// Returns a storage error for invalid content, conflicting replay/CAS, or transaction failure.
    pub fn revise_task_contract(
        &mut self,
        contract: &TaskContract,
        expected_head_revision: u64,
        idempotency_key: &str,
    ) -> Result<TaskContractHead, StorageError> {
        if idempotency_key.trim().is_empty() || expected_head_revision == 0 {
            return Err(StorageError::InvalidData(
                "task-contract revision requires a nonempty key and positive head revision".into(),
            ));
        }
        validate_contract(contract)?;
        let transaction = self.connection.transaction()?;
        if let Some((json, expected, resulting)) = transaction
            .query_row(
                "SELECT contract_json, expected_head_revision, resulting_head_revision
                 FROM task_contract_mutations
                 WHERE task_contract_id = ?1 AND idempotency_key = ?2",
                params![contract.id.to_string(), idempotency_key],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?
        {
            let original: TaskContract = serde_json::from_str(&json)?;
            if original != *contract
                || expected != sql_u64(expected_head_revision, "expected head revision")?
            {
                return Err(StorageError::Conflict(
                    "task-contract idempotency key was replayed with different content".into(),
                ));
            }
            transaction.commit()?;
            return Ok(TaskContractHead {
                contract: original,
                revision: u64::try_from(resulting).map_err(|_| {
                    StorageError::InvalidData("negative task-contract head revision".into())
                })?,
            });
        }

        let (current_version, current_revision) = transaction
            .query_row(
                "SELECT current_version, revision FROM task_contract_heads
                 WHERE task_contract_id = ?1",
                [contract.id.to_string()],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?
            .ok_or_else(|| StorageError::Conflict("task-contract head does not exist".into()))?;
        let current_version = u64::try_from(current_version)
            .map_err(|_| StorageError::InvalidData("negative task-contract version".into()))?;
        let current_revision = u64::try_from(current_revision)
            .map_err(|_| StorageError::InvalidData("negative task-contract revision".into()))?;
        if current_revision != expected_head_revision
            || contract.version != current_version.saturating_add(1)
        {
            return Err(StorageError::Conflict(format!(
                "stale task-contract head: expected revision/version {expected_head_revision}/{}, current {current_revision}/{current_version}",
                contract.version
            )));
        }
        let prior =
            load_task_contract(&transaction, contract.id, current_version)?.ok_or_else(|| {
                StorageError::InvalidData("task-contract head target is missing".into())
            })?;
        let expected_previous = VersionedRef {
            id: prior.id,
            version: prior.version,
            content_hash: task_contract_content_hash(&prior)?,
        };
        if contract.previous_version.as_ref() != Some(&expected_previous) {
            return Err(StorageError::Conflict(
                "task-contract revision does not bind the exact current head".into(),
            ));
        }
        insert_contract_version(&transaction, contract)?;
        let resulting_revision = increment(current_revision, "task-contract head revision")?;
        let changed = transaction.execute(
            "UPDATE task_contract_heads SET current_version = ?1, revision = ?2
             WHERE task_contract_id = ?3 AND current_version = ?4 AND revision = ?5",
            params![
                sql_u64(contract.version, "task contract version")?,
                sql_u64(resulting_revision, "task contract head revision")?,
                contract.id.to_string(),
                sql_u64(current_version, "current task contract version")?,
                sql_u64(current_revision, "current task contract head revision")?,
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::Conflict(
                "task-contract head changed before commit".into(),
            ));
        }
        transaction.execute(
            "INSERT INTO task_contract_mutations(
                task_contract_id, idempotency_key, contract_version, expected_head_revision,
                resulting_head_revision, contract_json, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                contract.id.to_string(),
                idempotency_key,
                sql_u64(contract.version, "task contract version")?,
                sql_u64(expected_head_revision, "expected head revision")?,
                sql_u64(resulting_revision, "resulting head revision")?,
                serde_json::to_string(contract)?,
                contract.created_at.to_rfc3339(),
            ],
        )?;
        transaction.commit()?;
        Ok(TaskContractHead {
            contract: contract.clone(),
            revision: resulting_revision,
        })
    }

    /// Loads one authoritative workflow attempt.
    ///
    /// # Errors
    /// Returns a storage error for query or stored-data failures.
    pub fn workflow_attempt(
        &self,
        attempt_id: AttemptId,
    ) -> Result<Option<WorkflowAttempt>, StorageError> {
        load_attempt(&self.connection, attempt_id)
    }

    /// Lists the latest authoritative attempt for each canonical workflow.
    ///
    /// # Errors
    /// Returns a storage error for query or stored-data failures.
    pub fn workflow_attempts(&self) -> Result<Vec<WorkflowAttempt>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT candidate.attempt_json
             FROM workflow_attempts AS candidate
             WHERE candidate.attempt_number = (
                 SELECT MAX(latest.attempt_number) FROM workflow_attempts AS latest
                 WHERE latest.workflow_id = candidate.workflow_id
             )
             ORDER BY candidate.updated_at DESC, candidate.workflow_id",
        )?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.map(|row| {
            let json = row?;
            serde_json::from_str(&json).map_err(StorageError::from)
        })
        .collect()
    }

    /// Loads the latest authoritative attempt for one canonical workflow.
    ///
    /// # Errors
    /// Returns a storage error for query or stored-data failures.
    pub fn workflow_attempt_for_workflow(
        &self,
        workflow_id: WorkflowId,
    ) -> Result<Option<WorkflowAttempt>, StorageError> {
        self.connection
            .query_row(
                "SELECT attempt_json FROM workflow_attempts
                 WHERE workflow_id = ?1 ORDER BY attempt_number DESC LIMIT 1",
                [workflow_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
            .transpose()
    }

    /// Returns the ordered event history for one workflow.
    ///
    /// # Errors
    /// Returns a storage error for query or stored-data failures.
    pub fn workflow_events(
        &self,
        workflow_id: WorkflowId,
    ) -> Result<Vec<WorkflowEvent>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT event_json FROM workflow_events WHERE workflow_id = ?1 ORDER BY sequence",
        )?;
        let rows = statement.query_map([workflow_id.to_string()], |row| row.get::<_, String>(0))?;
        rows.map(|row| {
            let json = row?;
            serde_json::from_str(&json).map_err(StorageError::from)
        })
        .collect()
    }

    /// Atomically claims a Ready attempt and increments both revision and lease generation.
    ///
    /// # Errors
    /// Returns a conflict for stale expectations, invalid state, conflicting replay, or expiry.
    #[allow(clippy::too_many_arguments)]
    pub fn claim_workflow_attempt(
        &mut self,
        attempt_id: AttemptId,
        expected_revision: u64,
        expected_generation: u64,
        lease_owner: RuntimeActorId,
        lease_expires_at: DateTime<Utc>,
        correlation_id: CorrelationId,
        idempotency_key: &str,
        now: DateTime<Utc>,
    ) -> Result<WorkflowAttempt, StorageError> {
        if idempotency_key.is_empty() || lease_expires_at <= now {
            return Err(StorageError::InvalidData(
                "workflow claim requires a nonempty key and future lease expiry".into(),
            ));
        }
        let transaction = self.connection.transaction()?;
        let mut attempt = load_attempt(&transaction, attempt_id)?
            .ok_or(StorageError::WorkflowAttemptNotFound(attempt_id))?;
        if let Some(event) =
            load_event_by_idempotency(&transaction, attempt.workflow_id, idempotency_key)?
        {
            let original = event_attempt(&event)?;
            let expected = serde_json::json!({
                "attempt_id": attempt.attempt_id,
                "lease_owner": lease_owner,
                "lease_generation": event.lease_generation,
                "lease_expires_at": lease_expires_at,
                "attempt": original,
            });
            if event.type_id.as_str() != WORKFLOW_CLAIMED_TYPE_ID
                || event_payload_value(&event)? != expected
            {
                return Err(StorageError::Conflict(
                    "workflow claim idempotency key was replayed with different content".into(),
                ));
            }
            transaction.commit()?;
            return Ok(original);
        }
        ensure_expected(&attempt, expected_revision, expected_generation)?;
        if attempt.state != WorkflowState::Ready {
            return Err(StorageError::Conflict(format!(
                "only a Ready workflow can be claimed; current state is {:?}",
                attempt.state
            )));
        }
        if !dependencies_satisfied(&transaction, &attempt)? {
            return Err(StorageError::Conflict(
                "workflow task-contract dependencies are not yet satisfied".into(),
            ));
        }

        attempt.state = WorkflowState::Running;
        attempt.revision = increment(attempt.revision, "workflow revision")?;
        attempt.lease_generation = increment(attempt.lease_generation, "lease generation")?;
        attempt.lease_owner = Some(lease_owner);
        attempt.lease_expires_at = Some(lease_expires_at);
        attempt.heartbeat_at = Some(now);
        attempt.updated_at = now;
        update_attempt_cas(
            &transaction,
            &attempt,
            expected_revision,
            expected_generation,
        )?;

        let (sequence, cause) = next_event_position(&transaction, attempt.workflow_id)?;
        let event = workflow_event(
            &attempt,
            sequence,
            correlation_id,
            cause,
            idempotency_key,
            WORKFLOW_CLAIMED_TYPE_ID,
            now,
            &serde_json::json!({
                "attempt_id": attempt.attempt_id,
                "lease_owner": lease_owner,
                "lease_generation": attempt.lease_generation,
                "lease_expires_at": lease_expires_at,
                "attempt": attempt,
            }),
        )?;
        insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(attempt)
    }

    /// Extends the current lease without changing its owner, scope, or generation.
    ///
    /// # Errors
    /// Returns a conflict for stale expectations, wrong ownership, expired leases, shortened
    /// expiry, or idempotency substitution.
    #[allow(clippy::too_many_arguments)]
    pub fn heartbeat_workflow_attempt(
        &mut self,
        attempt_id: AttemptId,
        expected_revision: u64,
        expected_generation: u64,
        lease_owner: RuntimeActorId,
        lease_expires_at: DateTime<Utc>,
        correlation_id: CorrelationId,
        idempotency_key: &str,
        now: DateTime<Utc>,
    ) -> Result<WorkflowAttempt, StorageError> {
        if idempotency_key.is_empty() || lease_expires_at <= now {
            return Err(StorageError::InvalidData(
                "workflow heartbeat requires a nonempty key and future expiry".into(),
            ));
        }
        let transaction = self.connection.transaction()?;
        let mut attempt = load_attempt(&transaction, attempt_id)?
            .ok_or(StorageError::WorkflowAttemptNotFound(attempt_id))?;
        if let Some(event) =
            load_event_by_idempotency(&transaction, attempt.workflow_id, idempotency_key)?
        {
            let original = event_attempt(&event)?;
            let expected = serde_json::json!({
                "attempt_id": attempt.attempt_id,
                "lease_owner": lease_owner,
                "lease_generation": event.lease_generation,
                "lease_expires_at": lease_expires_at,
                "attempt": original,
            });
            if event.type_id.as_str() != WORKFLOW_HEARTBEAT_TYPE_ID
                || event_payload_value(&event)? != expected
            {
                return Err(StorageError::Conflict(
                    "workflow heartbeat idempotency key was replayed with different content".into(),
                ));
            }
            transaction.commit()?;
            return Ok(original);
        }
        ensure_expected(&attempt, expected_revision, expected_generation)?;
        if !matches!(
            attempt.state,
            WorkflowState::Running | WorkflowState::Verifying
        ) || attempt.lease_owner != Some(lease_owner)
            || attempt
                .lease_expires_at
                .is_none_or(|current| current <= now)
            || attempt
                .lease_expires_at
                .is_some_and(|current| lease_expires_at < current)
        {
            return Err(StorageError::Conflict(
                "heartbeat cannot revive, shorten, or transfer a workflow lease".into(),
            ));
        }

        attempt.revision = increment(attempt.revision, "workflow revision")?;
        attempt.lease_expires_at = Some(lease_expires_at);
        attempt.heartbeat_at = Some(now);
        attempt.updated_at = now;
        update_attempt_cas(
            &transaction,
            &attempt,
            expected_revision,
            expected_generation,
        )?;
        let (sequence, cause) = next_event_position(&transaction, attempt.workflow_id)?;
        let event = workflow_event(
            &attempt,
            sequence,
            correlation_id,
            cause,
            idempotency_key,
            WORKFLOW_HEARTBEAT_TYPE_ID,
            now,
            &serde_json::json!({
                "attempt_id": attempt.attempt_id,
                "lease_owner": lease_owner,
                "lease_generation": attempt.lease_generation,
                "lease_expires_at": lease_expires_at,
                "attempt": attempt,
            }),
        )?;
        insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(attempt)
    }

    /// Applies one legal Root-owned transition using revision and generation compare-and-swap.
    ///
    /// Transitioning to Running is reserved for [`Self::claim_workflow_attempt`]. Entering
    /// `CancelRequested` fences the current worker generation before the event is committed.
    ///
    /// # Errors
    /// Returns a conflict for stale expectations, illegal transitions, or replay substitution.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub fn transition_workflow_attempt(
        &mut self,
        attempt_id: AttemptId,
        expected_revision: u64,
        expected_generation: u64,
        next_state: WorkflowState,
        correlation_id: CorrelationId,
        idempotency_key: &str,
        now: DateTime<Utc>,
    ) -> Result<WorkflowAttempt, StorageError> {
        if idempotency_key.is_empty() {
            return Err(StorageError::InvalidData(
                "workflow transition idempotency key cannot be empty".into(),
            ));
        }
        let transaction = self.connection.transaction()?;
        let mut attempt = load_attempt(&transaction, attempt_id)?
            .ok_or(StorageError::WorkflowAttemptNotFound(attempt_id))?;
        if let Some(event) =
            load_event_by_idempotency(&transaction, attempt.workflow_id, idempotency_key)?
        {
            let payload = event_payload_value(&event)?;
            if event.type_id.as_str() != WORKFLOW_TRANSITIONED_TYPE_ID
                || payload.get("attempt_id") != Some(&serde_json::json!(attempt.attempt_id))
                || payload.get("to") != Some(&serde_json::to_value(next_state)?)
            {
                return Err(StorageError::Conflict(
                    "workflow transition idempotency key was replayed with different content"
                        .into(),
                ));
            }
            let original = event_attempt(&event)?;
            transaction.commit()?;
            return Ok(original);
        }
        ensure_expected(&attempt, expected_revision, expected_generation)?;
        if next_state == WorkflowState::Running || !attempt.state.can_transition_to(next_state) {
            return Err(StorageError::Conflict(format!(
                "illegal workflow transition {:?} -> {next_state:?}",
                attempt.state
            )));
        }

        let previous_state = attempt.state;
        let cancellation_effects = if next_state == WorkflowState::CancelRequested {
            Some(fence_actions_for_workflow_cancellation(
                &transaction,
                attempt.attempt_id,
                correlation_id,
                now,
            )?)
        } else if previous_state == WorkflowState::CancelRequested {
            let status = cancellation_effect_status(&transaction, attempt.attempt_id)?;
            if next_state == WorkflowState::Cancelled && status.unresolved > 0 {
                return Err(StorageError::Conflict(
                    "workflow cancellation cannot settle while an action effect is unresolved"
                        .into(),
                ));
            }
            if next_state == WorkflowState::Waiting(WaitReason::Reconciliation)
                && (status.reconciliation_required == 0 || status.quarantined > 0)
            {
                return Err(StorageError::Conflict(
                    "workflow may wait for reconciliation only for unresolved reconcilable effects"
                        .into(),
                ));
            }
            Some(status)
        } else if previous_state == WorkflowState::Waiting(WaitReason::Reconciliation)
            && next_state == WorkflowState::Cancelled
        {
            let status = cancellation_effect_status(&transaction, attempt.attempt_id)?;
            if !is_cancellation_reconciliation_wait(&transaction, &attempt)?
                || status.unresolved > 0
            {
                return Err(StorageError::Conflict(
                    "only a fully reconciled cancellation wait can settle as cancelled".into(),
                ));
            }
            Some(status)
        } else {
            None
        };
        attempt.state = next_state;
        attempt.revision = increment(attempt.revision, "workflow revision")?;
        if next_state == WorkflowState::CancelRequested {
            attempt.lease_generation = increment(attempt.lease_generation, "lease generation")?;
        }
        if next_state != WorkflowState::Verifying {
            attempt.lease_owner = None;
            attempt.lease_expires_at = None;
            attempt.heartbeat_at = None;
        }
        attempt.updated_at = now;
        update_attempt_cas(
            &transaction,
            &attempt,
            expected_revision,
            expected_generation,
        )?;

        let (sequence, cause) = next_event_position(&transaction, attempt.workflow_id)?;
        let event = workflow_event(
            &attempt,
            sequence,
            correlation_id,
            cause,
            idempotency_key,
            WORKFLOW_TRANSITIONED_TYPE_ID,
            now,
            &serde_json::json!({
                "attempt_id": attempt.attempt_id,
                "from": previous_state,
                "to": next_state,
                "cancellation_effects": cancellation_effects,
                "attempt": attempt,
            }),
        )?;
        insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(attempt)
    }

    /// Creates a new Ready attempt instead of rewinding a terminal failed/cancelled attempt.
    ///
    /// # Errors
    /// Returns a conflict for stale expectations, ineligible state, exhausted retry policy, or
    /// idempotency substitution.
    #[allow(clippy::too_many_arguments)]
    pub fn retry_workflow_attempt(
        &mut self,
        prior_attempt_id: AttemptId,
        expected_revision: u64,
        expected_generation: u64,
        correlation_id: CorrelationId,
        idempotency_key: &str,
        now: DateTime<Utc>,
    ) -> Result<WorkflowAttempt, StorageError> {
        if idempotency_key.is_empty() {
            return Err(StorageError::InvalidData(
                "workflow retry idempotency key cannot be empty".into(),
            ));
        }
        let transaction = self.connection.transaction()?;
        let prior = load_attempt(&transaction, prior_attempt_id)?
            .ok_or(StorageError::WorkflowAttemptNotFound(prior_attempt_id))?;
        if let Some(existing) =
            load_attempt_by_idempotency(&transaction, prior.workflow_id, idempotency_key)?
        {
            let event =
                load_event_by_idempotency(&transaction, prior.workflow_id, idempotency_key)?
                    .ok_or_else(|| {
                        StorageError::InvalidData(
                            "retry attempt exists without its atomic event".into(),
                        )
                    })?;
            if existing.parent_attempt_id != Some(prior_attempt_id)
                || event.type_id.as_str() != WORKFLOW_RETRIED_TYPE_ID
            {
                return Err(StorageError::Conflict(
                    "workflow retry idempotency key was replayed with different ancestry".into(),
                ));
            }
            let original = event_attempt(&event)?;
            transaction.commit()?;
            return Ok(original);
        }
        ensure_expected(&prior, expected_revision, expected_generation)?;
        if !matches!(
            prior.state,
            WorkflowState::Failed | WorkflowState::Cancelled
        ) || prior.attempt_number >= prior.retry_policy.max_attempts
        {
            return Err(StorageError::Conflict(
                "only failed or cancelled attempts within the retry ceiling may retry".into(),
            ));
        }
        let attempt = WorkflowAttempt {
            workflow_id: prior.workflow_id,
            attempt_id: AttemptId::new(),
            task_contract: prior.task_contract.clone(),
            parent_attempt_id: Some(prior.attempt_id),
            attempt_number: prior.attempt_number.checked_add(1).ok_or_else(|| {
                StorageError::InvalidData("workflow attempt number exhausted".into())
            })?,
            state: WorkflowState::Ready,
            revision: 1,
            lease_owner: None,
            lease_generation: 0,
            lease_expires_at: None,
            heartbeat_at: None,
            checkpoint: None,
            environment_snapshot: prior.environment_snapshot.clone(),
            configuration: prior.configuration.clone(),
            idempotency_key: idempotency_key.to_owned(),
            retry_policy: prior.retry_policy.clone(),
            compensation_policy: prior.compensation_policy.clone(),
            created_at: now,
            updated_at: now,
        };
        insert_attempt(&transaction, &attempt)?;
        let (sequence, cause) = next_event_position(&transaction, attempt.workflow_id)?;
        let event = workflow_event(
            &attempt,
            sequence,
            correlation_id,
            cause,
            idempotency_key,
            WORKFLOW_RETRIED_TYPE_ID,
            now,
            &serde_json::json!({
                "attempt_id": attempt.attempt_id,
                "parent_attempt_id": prior.attempt_id,
                "attempt_number": attempt.attempt_number,
                "state": attempt.state,
                "attempt": attempt,
            }),
        )?;
        insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(attempt)
    }

    /// Recovers workflow leases and objectively satisfied wait conditions during Root startup.
    ///
    /// Running work moves to `Waiting(Reconciliation)`; expired verification work can safely
    /// return to Ready. Dependency and retry-backoff waits resume only when their persisted
    /// condition is satisfied. Other waits, cancellation, and terminal work are not guessed.
    ///
    /// # Errors
    /// Returns a storage error for query, malformed stored data, or atomic update failures.
    #[allow(clippy::too_many_lines)]
    pub fn recover_expired_workflow_leases(
        &mut self,
        correlation_id: CorrelationId,
        now: DateTime<Utc>,
    ) -> Result<WorkflowRecoveryReport, StorageError> {
        let attempts = load_nonterminal_attempts(&self.connection)?;
        let mut report = WorkflowRecoveryReport {
            inspected_nonterminal: u64::try_from(attempts.len()).map_err(|_| {
                StorageError::InvalidData("workflow count exceeds supported range".into())
            })?,
            fenced_expired: 0,
            awaiting_reconciliation: 0,
            requeued_for_verification: 0,
            requeued_satisfied_dependencies: 0,
            requeued_elapsed_backoffs: 0,
            settled_cancellations: 0,
            cancellations_awaiting_reconciliation: 0,
            quarantined_cancellations: 0,
        };
        for mut attempt in attempts {
            let expired = matches!(
                attempt.state,
                WorkflowState::Running | WorkflowState::Verifying
            ) && attempt.lease_expires_at.is_some_and(|expiry| expiry <= now);
            let satisfied_dependency_wait = attempt.state
                == WorkflowState::Waiting(WaitReason::Dependency)
                && dependencies_satisfied(&self.connection, &attempt)?;
            let elapsed_retry_backoff = attempt.state
                == WorkflowState::Waiting(WaitReason::RetryBackoff)
                && retry_backoff_elapsed(&attempt, now);
            let is_cancellation_state = attempt.state == WorkflowState::CancelRequested
                || (attempt.state == WorkflowState::Waiting(WaitReason::Reconciliation)
                    && is_cancellation_reconciliation_wait(&self.connection, &attempt)?);
            let cancellation_effects = if is_cancellation_state {
                Some(cancellation_effect_status(
                    &self.connection,
                    attempt.attempt_id,
                )?)
            } else {
                None
            };
            let recovered_cancellation_state = cancellation_effects.and_then(|status| {
                if status.quarantined > 0 {
                    Some(WorkflowState::Quarantined)
                } else if status.reconciliation_required > 0 {
                    Some(WorkflowState::Waiting(WaitReason::Reconciliation))
                } else if status.unresolved == 0 {
                    Some(WorkflowState::Cancelled)
                } else {
                    None
                }
            });
            if !expired
                && !satisfied_dependency_wait
                && !elapsed_retry_backoff
                && recovered_cancellation_state.is_none()
            {
                continue;
            }
            let expected_revision = attempt.revision;
            let expected_generation = attempt.lease_generation;
            let previous_state = attempt.state;
            attempt.state = recovered_cancellation_state.unwrap_or_else(|| match previous_state {
                WorkflowState::Running => WorkflowState::Waiting(WaitReason::Reconciliation),
                WorkflowState::Verifying => WorkflowState::Ready,
                WorkflowState::Waiting(WaitReason::Dependency | WaitReason::RetryBackoff) => {
                    WorkflowState::Ready
                }
                _ => unreachable!("recovery filters restrict recoverable states"),
            });
            attempt.revision = increment(attempt.revision, "workflow revision")?;
            if expired {
                attempt.lease_generation = increment(attempt.lease_generation, "lease generation")?;
            }
            attempt.lease_owner = None;
            attempt.lease_expires_at = None;
            attempt.heartbeat_at = None;
            attempt.updated_at = now;

            let transaction = self.connection.transaction()?;
            update_attempt_cas(
                &transaction,
                &attempt,
                expected_revision,
                expected_generation,
            )?;
            let (sequence, cause) = next_event_position(&transaction, attempt.workflow_id)?;
            let (idempotency_key, type_id, recovery_reason) = if expired {
                (
                    format!(
                        "recover-expired:{}:{expected_revision}:{expected_generation}",
                        attempt.attempt_id
                    ),
                    WORKFLOW_LEASE_EXPIRED_TYPE_ID,
                    "lease_expired",
                )
            } else if satisfied_dependency_wait {
                (
                    format!(
                        "recover-dependency:{}:{expected_revision}:{expected_generation}",
                        attempt.attempt_id
                    ),
                    WORKFLOW_WAIT_SATISFIED_TYPE_ID,
                    "dependencies_satisfied",
                )
            } else if elapsed_retry_backoff {
                (
                    format!(
                        "recover-backoff:{}:{expected_revision}:{expected_generation}",
                        attempt.attempt_id
                    ),
                    WORKFLOW_WAIT_SATISFIED_TYPE_ID,
                    "retry_backoff_elapsed",
                )
            } else {
                (
                    format!(
                        "recover-cancellation:{}:{expected_revision}:{expected_generation}",
                        attempt.attempt_id
                    ),
                    WORKFLOW_CANCELLATION_RECOVERED_TYPE_ID,
                    match attempt.state {
                        WorkflowState::Cancelled => "cancellation_settled",
                        WorkflowState::Waiting(WaitReason::Reconciliation) => {
                            "cancellation_requires_reconciliation"
                        }
                        WorkflowState::Quarantined => "cancellation_quarantined",
                        _ => unreachable!("cancellation recovery target"),
                    },
                )
            };
            let event = workflow_event(
                &attempt,
                sequence,
                correlation_id,
                cause,
                &idempotency_key,
                type_id,
                now,
                &serde_json::json!({
                    "attempt_id": attempt.attempt_id,
                    "from": previous_state,
                    "to": attempt.state,
                    "fenced_generation": expected_generation,
                    "active_generation": attempt.lease_generation,
                    "recovery_reason": recovery_reason,
                    "cancellation_effects": cancellation_effects,
                    "attempt": attempt,
                }),
            )?;
            insert_event(&transaction, &event)?;
            transaction.commit()?;
            if expired {
                report.fenced_expired += 1;
            }
            match attempt.state {
                WorkflowState::Waiting(WaitReason::Reconciliation)
                    if previous_state == WorkflowState::CancelRequested =>
                {
                    report.cancellations_awaiting_reconciliation += 1;
                }
                WorkflowState::Waiting(WaitReason::Reconciliation) => {
                    report.awaiting_reconciliation += 1;
                }
                WorkflowState::Ready if previous_state == WorkflowState::Verifying => {
                    report.requeued_for_verification += 1;
                }
                WorkflowState::Ready if satisfied_dependency_wait => {
                    report.requeued_satisfied_dependencies += 1;
                }
                WorkflowState::Ready if elapsed_retry_backoff => {
                    report.requeued_elapsed_backoffs += 1;
                }
                WorkflowState::Cancelled if previous_state == WorkflowState::CancelRequested => {
                    report.settled_cancellations += 1;
                }
                WorkflowState::Quarantined if previous_state == WorkflowState::CancelRequested => {
                    report.quarantined_cancellations += 1;
                }
                WorkflowState::Cancelled
                    if previous_state == WorkflowState::Waiting(WaitReason::Reconciliation) =>
                {
                    report.settled_cancellations += 1;
                }
                WorkflowState::Quarantined
                    if previous_state == WorkflowState::Waiting(WaitReason::Reconciliation) =>
                {
                    report.quarantined_cancellations += 1;
                }
                _ => {}
            }
        }
        Ok(report)
    }
}

fn is_cancellation_reconciliation_wait(
    connection: &rusqlite::Connection,
    attempt: &WorkflowAttempt,
) -> Result<bool, StorageError> {
    if attempt.state != WorkflowState::Waiting(WaitReason::Reconciliation) {
        return Ok(false);
    }
    let event_json = connection
        .query_row(
            "SELECT event_json FROM workflow_events
             WHERE workflow_id = ?1 ORDER BY sequence DESC LIMIT 1",
            [attempt.workflow_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let Some(event_json) = event_json else {
        return Ok(false);
    };
    let event: WorkflowEvent = serde_json::from_str(&event_json)?;
    let payload = event_payload_value(&event)?;
    Ok(
        (event.type_id.as_str() == WORKFLOW_CANCELLATION_RECOVERED_TYPE_ID
            && payload.get("recovery_reason")
                == Some(&serde_json::json!("cancellation_requires_reconciliation")))
            || (event.type_id.as_str() == WORKFLOW_TRANSITIONED_TYPE_ID
                && payload.get("from") == Some(&serde_json::json!(WorkflowState::CancelRequested))
                && payload.get("to")
                    == Some(&serde_json::json!(WorkflowState::Waiting(
                        WaitReason::Reconciliation
                    )))),
    )
}

fn retry_backoff_elapsed(attempt: &WorkflowAttempt, now: DateTime<Utc>) -> bool {
    let elapsed_ms = now
        .signed_duration_since(attempt.updated_at)
        .num_milliseconds();
    elapsed_ms >= 0
        && u64::try_from(elapsed_ms)
            .is_ok_and(|elapsed| elapsed >= attempt.retry_policy.initial_backoff_ms)
}

fn validate_initial_workflow(
    contract: &TaskContract,
    attempt: &WorkflowAttempt,
) -> Result<(), StorageError> {
    let distinct_dependencies = contract
        .dependencies
        .iter()
        .map(|dependency| (dependency.id, dependency.version))
        .collect::<std::collections::BTreeSet<_>>();
    let policy_bindings = [
        &contract.permitted_side_effects,
        &contract.authority,
        &contract.environment,
        &contract.risk,
        &contract.budgets,
        &contract.rollback,
        &attempt.compensation_policy,
    ];
    for binding in policy_bindings {
        let actual = binding
            .payload
            .content_hash(&binding.content_hash.domain)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        if actual != binding.content_hash {
            return Err(StorageError::InvalidData(
                "policy binding hash does not match canonical payload".into(),
            ));
        }
    }
    let first_version = contract.version == 1 && contract.previous_version.is_none();
    if contract.objective.trim().is_empty()
        || !first_version
        || contract.status != TaskContractStatus::Active
        || contract
            .dependencies
            .iter()
            .any(|dependency| dependency.id == contract.id)
        || distinct_dependencies.len() != contract.dependencies.len()
        || attempt.task_contract.id != contract.id
        || attempt.task_contract.version != contract.version
        || attempt.parent_attempt_id.is_some()
        || attempt.attempt_number != 1
        || attempt.state != WorkflowState::Ready
        || attempt.revision != 1
        || attempt.lease_generation != 0
        || attempt.lease_owner.is_some()
        || attempt.lease_expires_at.is_some()
        || attempt.heartbeat_at.is_some()
        || attempt.idempotency_key.trim().is_empty()
        || attempt.retry_policy.max_attempts == 0
        || attempt.retry_policy.initial_backoff_ms > attempt.retry_policy.maximum_backoff_ms
        || attempt.created_at != attempt.updated_at
    {
        return Err(StorageError::InvalidData(
            "workflow must atomically begin as a valid active contract and unleased Ready attempt"
                .into(),
        ));
    }
    Ok(())
}

fn validate_contract(contract: &TaskContract) -> Result<(), StorageError> {
    let distinct_dependencies = contract
        .dependencies
        .iter()
        .map(|dependency| (dependency.id, dependency.version))
        .collect::<std::collections::BTreeSet<_>>();
    for binding in [
        &contract.permitted_side_effects,
        &contract.authority,
        &contract.environment,
        &contract.risk,
        &contract.budgets,
        &contract.rollback,
    ] {
        let actual = binding
            .payload
            .content_hash(&binding.content_hash.domain)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        if actual != binding.content_hash {
            return Err(StorageError::InvalidData(
                "policy binding hash does not match canonical payload".into(),
            ));
        }
    }
    let version_lineage = contract.version == 1 && contract.previous_version.is_none()
        || contract.version > 1
            && contract.previous_version.as_ref().is_some_and(|previous| {
                previous.id == contract.id
                    && previous
                        .version
                        .checked_add(1)
                        .is_some_and(|version| version == contract.version)
            });
    if contract.objective.trim().is_empty()
        || !version_lineage
        || contract.status != TaskContractStatus::Active
        || contract
            .dependencies
            .iter()
            .any(|dependency| dependency.id == contract.id)
        || distinct_dependencies.len() != contract.dependencies.len()
    {
        return Err(StorageError::InvalidData(
            "task-contract revision is invalid or not active".into(),
        ));
    }
    Ok(())
}

fn insert_contract_version(
    transaction: &Transaction<'_>,
    contract: &TaskContract,
) -> Result<(), StorageError> {
    let contract_hash = task_contract_content_hash(contract)?;
    transaction.execute(
        "INSERT INTO task_contract_versions(
            task_contract_id, version, type_id, schema_version, content_hash_json,
            contract_json, status, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            contract.id.to_string(),
            sql_u64(contract.version, "task contract version")?,
            TaskContract::TYPE_ID,
            i64::from(TaskContract::SCHEMA_VERSION),
            serde_json::to_string(&contract_hash)?,
            serde_json::to_string(contract)?,
            task_contract_status_label(contract.status),
            contract.created_at.to_rfc3339(),
        ],
    )?;
    for dependency in &contract.dependencies {
        transaction.execute(
            "INSERT INTO task_contract_dependencies(
                task_contract_id, task_contract_version, dependency_id, dependency_version
             ) VALUES (?1, ?2, ?3, ?4)",
            params![
                contract.id.to_string(),
                sql_u64(contract.version, "task contract version")?,
                dependency.id.to_string(),
                sql_u64(dependency.version, "dependency version")?,
            ],
        )?;
    }
    Ok(())
}

fn load_task_contract(
    connection: &Connection,
    id: TaskContractId,
    version: u64,
) -> Result<Option<TaskContract>, StorageError> {
    connection
        .query_row(
            "SELECT contract_json FROM task_contract_versions
             WHERE task_contract_id = ?1 AND version = ?2",
            params![id.to_string(), sql_u64(version, "task contract version")?],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
        .transpose()
}

fn insert_attempt(
    transaction: &Transaction<'_>,
    attempt: &WorkflowAttempt,
) -> Result<(), StorageError> {
    transaction.execute(
        "INSERT INTO workflow_attempts(
            attempt_id, workflow_id, attempt_number, parent_attempt_id,
            task_contract_id, task_contract_version, state, revision, lease_generation,
            lease_owner, lease_expires_at, heartbeat_at, idempotency_key, attempt_json,
            created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![
            attempt.attempt_id.to_string(),
            attempt.workflow_id.to_string(),
            i64::from(attempt.attempt_number),
            attempt.parent_attempt_id.map(|id| id.to_string()),
            attempt.task_contract.id.to_string(),
            sql_u64(attempt.task_contract.version, "task contract version")?,
            state_label(attempt.state),
            sql_u64(attempt.revision, "workflow revision")?,
            sql_u64(attempt.lease_generation, "lease generation")?,
            attempt.lease_owner.map(|id| id.to_string()),
            attempt.lease_expires_at.map(|value| value.to_rfc3339()),
            attempt.heartbeat_at.map(|value| value.to_rfc3339()),
            attempt.idempotency_key,
            serde_json::to_string(attempt)?,
            attempt.created_at.to_rfc3339(),
            attempt.updated_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

fn update_attempt_cas(
    transaction: &Transaction<'_>,
    attempt: &WorkflowAttempt,
    expected_revision: u64,
    expected_generation: u64,
) -> Result<(), StorageError> {
    let changed = transaction.execute(
        "UPDATE workflow_attempts SET
            state = ?1, revision = ?2, lease_generation = ?3, lease_owner = ?4,
            lease_expires_at = ?5, heartbeat_at = ?6, attempt_json = ?7, updated_at = ?8
         WHERE attempt_id = ?9 AND revision = ?10 AND lease_generation = ?11",
        params![
            state_label(attempt.state),
            sql_u64(attempt.revision, "workflow revision")?,
            sql_u64(attempt.lease_generation, "lease generation")?,
            attempt.lease_owner.map(|id| id.to_string()),
            attempt.lease_expires_at.map(|value| value.to_rfc3339()),
            attempt.heartbeat_at.map(|value| value.to_rfc3339()),
            serde_json::to_string(attempt)?,
            attempt.updated_at.to_rfc3339(),
            attempt.attempt_id.to_string(),
            sql_u64(expected_revision, "expected workflow revision")?,
            sql_u64(expected_generation, "expected lease generation")?,
        ],
    )?;
    if changed != 1 {
        return Err(StorageError::Conflict(
            "workflow revision or lease generation changed before commit".into(),
        ));
    }
    Ok(())
}

fn insert_event(transaction: &Transaction<'_>, event: &WorkflowEvent) -> Result<(), StorageError> {
    transaction.execute(
        "INSERT INTO workflow_events(
            event_id, workflow_id, sequence, attempt_id, attempt_revision, lease_generation,
            type_id, schema_version, correlation_id, causation_id, idempotency_key,
            event_json, occurred_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            event.id.to_string(),
            event.workflow_id.to_string(),
            sql_u64(event.sequence, "workflow event sequence")?,
            event.attempt_id.to_string(),
            sql_u64(event.attempt_revision, "workflow attempt revision")?,
            sql_u64(event.lease_generation, "lease generation")?,
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

fn load_attempt(
    connection: &Connection,
    attempt_id: AttemptId,
) -> Result<Option<WorkflowAttempt>, StorageError> {
    connection
        .query_row(
            "SELECT attempt_json FROM workflow_attempts WHERE attempt_id = ?1",
            [attempt_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
        .transpose()
}

fn load_attempt_by_idempotency(
    connection: &Connection,
    workflow_id: WorkflowId,
    idempotency_key: &str,
) -> Result<Option<WorkflowAttempt>, StorageError> {
    connection
        .query_row(
            "SELECT attempt_json FROM workflow_attempts
             WHERE workflow_id = ?1 AND idempotency_key = ?2",
            params![workflow_id.to_string(), idempotency_key],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
        .transpose()
}

fn load_event_by_idempotency(
    connection: &Connection,
    workflow_id: WorkflowId,
    idempotency_key: &str,
) -> Result<Option<WorkflowEvent>, StorageError> {
    connection
        .query_row(
            "SELECT event_json FROM workflow_events
             WHERE workflow_id = ?1 AND idempotency_key = ?2",
            params![workflow_id.to_string(), idempotency_key],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
        .transpose()
}

fn load_nonterminal_attempts(
    connection: &Connection,
) -> Result<Vec<WorkflowAttempt>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT attempt_json FROM workflow_attempts
         WHERE state NOT IN ('succeeded', 'failed', 'cancelled', 'quarantined')
         ORDER BY updated_at, workflow_id, attempt_number",
    )?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    rows.map(|row| {
        let json = row?;
        serde_json::from_str(&json).map_err(StorageError::from)
    })
    .collect()
}

fn dependencies_satisfied(
    connection: &Connection,
    attempt: &WorkflowAttempt,
) -> Result<bool, StorageError> {
    let unsatisfied = connection.query_row(
        "SELECT COUNT(*) FROM task_contract_dependencies AS dependency
         WHERE dependency.task_contract_id = ?1
           AND dependency.task_contract_version = ?2
           AND NOT EXISTS (
               SELECT 1 FROM workflow_attempts AS prerequisite
               WHERE prerequisite.task_contract_id = dependency.dependency_id
                 AND prerequisite.task_contract_version = dependency.dependency_version
                 AND prerequisite.state = 'succeeded'
           )",
        params![
            attempt.task_contract.id.to_string(),
            sql_u64(attempt.task_contract.version, "task contract version")?,
        ],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(unsatisfied == 0)
}

fn next_event_position(
    transaction: &Transaction<'_>,
    workflow_id: WorkflowId,
) -> Result<(u64, Option<EventId>), StorageError> {
    let current = transaction
        .query_row(
            "SELECT sequence, event_json FROM workflow_events
             WHERE workflow_id = ?1 ORDER BY sequence DESC LIMIT 1",
            [workflow_id.to_string()],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    match current {
        Some((sequence, json)) => {
            let event: WorkflowEvent = serde_json::from_str(&json)?;
            let sequence = u64::try_from(sequence).map_err(|_| {
                StorageError::InvalidData("negative workflow event sequence".into())
            })?;
            Ok((
                increment(sequence, "workflow event sequence")?,
                Some(event.id),
            ))
        }
        None => Ok((1, None)),
    }
}

#[allow(clippy::too_many_arguments)]
fn workflow_event(
    attempt: &WorkflowAttempt,
    sequence: u64,
    correlation_id: CorrelationId,
    causation_id: Option<EventId>,
    idempotency_key: &str,
    event_type: &str,
    occurred_at: DateTime<Utc>,
    value: &serde_json::Value,
) -> Result<WorkflowEvent, StorageError> {
    let type_id = stable_type_id(event_type)?;
    let payload = VersionedPayload::canonical_v1(type_id.clone(), value)
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    Ok(WorkflowEvent {
        id: EventId::new(),
        type_id,
        schema_version: WorkflowEvent::SCHEMA_VERSION,
        sequence,
        workflow_id: attempt.workflow_id,
        attempt_id: attempt.attempt_id,
        attempt_revision: attempt.revision,
        lease_generation: attempt.lease_generation,
        correlation_id,
        causation_id,
        idempotency_key: idempotency_key.to_owned(),
        occurred_at,
        payload,
    })
}

fn ensure_expected(
    attempt: &WorkflowAttempt,
    expected_revision: u64,
    expected_generation: u64,
) -> Result<(), StorageError> {
    if attempt.revision != expected_revision || attempt.lease_generation != expected_generation {
        return Err(StorageError::Conflict(format!(
            "stale workflow expectation: revision/generation {expected_revision}/{expected_generation}, current {}/{}",
            attempt.revision, attempt.lease_generation
        )));
    }
    Ok(())
}

fn state_label(state: WorkflowState) -> &'static str {
    match state {
        WorkflowState::Ready => "ready",
        WorkflowState::Running => "running",
        WorkflowState::Waiting(_) => "waiting",
        WorkflowState::Verifying => "verifying",
        WorkflowState::Succeeded => "succeeded",
        WorkflowState::Failed => "failed",
        WorkflowState::CancelRequested => "cancel_requested",
        WorkflowState::Cancelled => "cancelled",
        WorkflowState::Quarantined => "quarantined",
    }
}

fn task_contract_status_label(status: TaskContractStatus) -> &'static str {
    match status {
        TaskContractStatus::Draft => "draft",
        TaskContractStatus::Active => "active",
        TaskContractStatus::Superseded => "superseded",
        TaskContractStatus::Cancelled => "cancelled",
    }
}

fn stable_type_id(value: &str) -> Result<StableTypeId, StorageError> {
    StableTypeId::parse(value).map_err(|error| StorageError::InvalidData(error.to_string()))
}

fn event_payload_value(event: &WorkflowEvent) -> Result<serde_json::Value, StorageError> {
    decode_canonical_cbor_v1(&event.payload.bytes)
        .map_err(|error| StorageError::InvalidData(error.to_string()))
}

fn event_attempt(event: &WorkflowEvent) -> Result<WorkflowAttempt, StorageError> {
    let value = event_payload_value(event)?;
    serde_json::from_value(value.get("attempt").cloned().ok_or_else(|| {
        StorageError::InvalidData("workflow event has no attempt snapshot".into())
    })?)
    .map_err(StorageError::from)
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
        ConfigurationId, ContractClause, HashAlgorithm, PolicyBinding, RetryPolicy, SourceRef,
        VersionedRef, WaitReason,
    };

    use super::*;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 30, 12, 0, 0).unwrap()
    }

    fn policy(label: &str) -> PolicyBinding {
        let payload = VersionedPayload::canonical_v1(
            StableTypeId::parse("test.workflow_policy.v1").unwrap(),
            &serde_json::json!({"policy": label}),
        )
        .unwrap();
        let content_hash = payload.content_hash("TestWorkflowPolicy").unwrap();
        PolicyBinding {
            payload,
            content_hash,
        }
    }

    fn hash(label: &str) -> ContentHash {
        VersionedPayload::canonical_v1(
            StableTypeId::parse("test.workflow_ref.v1").unwrap(),
            &serde_json::json!({"label": label}),
        )
        .unwrap()
        .content_hash("TestWorkflowRef")
        .unwrap()
    }

    fn fixture() -> (TaskContract, WorkflowAttempt) {
        let task_id = TaskContractId::new();
        let source = SourceRef {
            source_type: StableTypeId::parse("test.user_intent.v1").unwrap(),
            locator: "conversation:test".to_owned(),
            content_hash: hash("intent"),
        };
        let clause = ContractClause {
            clause_id: "criterion-1".to_owned(),
            statement: "result is durable".to_owned(),
            source_refs: vec![source.clone()],
        };
        let contract = TaskContract {
            id: task_id,
            version: 1,
            previous_version: None,
            user_intent_ref: source,
            objective: "create a durable workflow".to_owned(),
            non_goals: Vec::new(),
            requirements: vec![clause.clone()],
            constraints: Vec::new(),
            assumptions: Vec::new(),
            acceptance_criteria: vec![clause],
            dependencies: Vec::new(),
            permitted_side_effects: policy("none"),
            authority: policy("root"),
            environment: policy("test"),
            risk: policy("low"),
            budgets: policy("bounded"),
            rollback: policy("not-required"),
            status: TaskContractStatus::Active,
            created_at: now(),
        };
        let placeholder = ContentHash {
            algorithm: HashAlgorithm::Blake3_256,
            domain: TASK_CONTRACT_HASH_DOMAIN.to_owned(),
            digest_hex: "0".repeat(64),
        };
        let mut attempt = WorkflowAttempt {
            workflow_id: WorkflowId::new(),
            attempt_id: AttemptId::new(),
            task_contract: VersionedRef {
                id: task_id,
                version: 1,
                content_hash: placeholder,
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
            idempotency_key: "create-workflow-1".to_owned(),
            retry_policy: RetryPolicy {
                max_attempts: 3,
                initial_backoff_ms: 100,
                maximum_backoff_ms: 1_000,
            },
            compensation_policy: policy("reconcile"),
            created_at: now(),
            updated_at: now(),
        };
        attempt.task_contract.content_hash = task_contract_content_hash(&contract).unwrap();
        (contract, attempt)
    }

    fn waiting_fixture(
        store: &mut SqliteEventStore,
        reason: WaitReason,
        key: &str,
    ) -> WorkflowAttempt {
        let (contract, attempt) = fixture();
        store
            .create_workflow(&contract, &attempt, CorrelationId::new(), now())
            .unwrap();
        let claimed = store
            .claim_workflow_attempt(
                attempt.attempt_id,
                attempt.revision,
                attempt.lease_generation,
                RuntimeActorId::new(),
                now() + chrono::Duration::minutes(5),
                CorrelationId::new(),
                &format!("{key}-claim"),
                now(),
            )
            .unwrap();
        store
            .transition_workflow_attempt(
                attempt.attempt_id,
                claimed.revision,
                claimed.lease_generation,
                WorkflowState::Waiting(reason),
                CorrelationId::new(),
                &format!("{key}-wait"),
                now(),
            )
            .unwrap()
    }

    #[test]
    fn workflow_create_is_atomic_and_exact_replay_is_idempotent() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (contract, attempt) = fixture();
        let correlation = CorrelationId::new();
        let event = store
            .create_workflow(&contract, &attempt, correlation, now())
            .unwrap();
        assert_eq!(event.sequence, 1);
        assert_eq!(event.attempt_revision, 1);
        assert_eq!(event.lease_generation, 0);
        assert_eq!(
            store.task_contract_version(contract.id, 1).unwrap(),
            Some(contract.clone())
        );
        assert_eq!(
            store.workflow_attempt(attempt.attempt_id).unwrap(),
            Some(attempt.clone())
        );
        assert_eq!(
            store
                .create_workflow(&contract, &attempt, correlation, now())
                .unwrap(),
            event
        );
        assert_eq!(store.workflow_events(attempt.workflow_id).unwrap().len(), 1);
    }

    #[test]
    fn task_contract_v1_golden_fixture_hash_is_stable() {
        let contract: TaskContract = serde_json::from_str(include_str!(
            "../../mcg-contracts/fixtures/task-contract-v1.json"
        ))
        .expect("checked-in task-contract fixture");
        let expected: serde_json::Value = serde_json::from_str(include_str!(
            "../../mcg-contracts/fixtures/task-contract-v1.expected.json"
        ))
        .expect("checked-in task-contract expectation");
        let content_hash = task_contract_content_hash(&contract).unwrap();
        assert_eq!(TaskContract::TYPE_ID, expected["stable_type_id"]);
        assert_eq!(TaskContract::SCHEMA_VERSION, expected["schema_version"]);
        assert_eq!(content_hash.domain, expected["semantic_domain"]);
        assert_eq!(content_hash.digest_hex, expected["blake3_256"]);
    }

    #[test]
    fn task_contract_revisions_are_immutable_cas_bound_and_exactly_replayable() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (contract, attempt) = fixture();
        store
            .create_workflow(&contract, &attempt, CorrelationId::new(), now())
            .unwrap();
        assert_eq!(
            store.task_contract_head(contract.id).unwrap().unwrap(),
            TaskContractHead {
                contract: contract.clone(),
                revision: 1,
            }
        );

        let mut revision = contract.clone();
        revision.version = 2;
        revision.previous_version = Some(VersionedRef {
            id: contract.id,
            version: contract.version,
            content_hash: task_contract_content_hash(&contract).unwrap(),
        });
        revision.objective = "create and revise a durable workflow".to_owned();
        revision.created_at = now() + chrono::Duration::seconds(1);
        let revised = store
            .revise_task_contract(&revision, 1, "contract-revision-2")
            .unwrap();
        assert_eq!(revised.revision, 2);
        assert_eq!(revised.contract, revision);
        assert_eq!(
            store.task_contract_version(contract.id, 1).unwrap(),
            Some(contract.clone())
        );
        assert_eq!(
            store.task_contract_head(contract.id).unwrap(),
            Some(revised.clone())
        );
        assert_eq!(
            store
                .revise_task_contract(&revision, 1, "contract-revision-2")
                .unwrap(),
            revised
        );

        let mut substituted = revision.clone();
        substituted.objective = "substituted content".to_owned();
        assert!(matches!(
            store.revise_task_contract(&substituted, 1, "contract-revision-2"),
            Err(StorageError::Conflict(_))
        ));

        let mut stale = revision.clone();
        stale.version = 3;
        stale.previous_version = Some(VersionedRef {
            id: revision.id,
            version: revision.version,
            content_hash: task_contract_content_hash(&revision).unwrap(),
        });
        assert!(matches!(
            store.revise_task_contract(&stale, 1, "contract-revision-3"),
            Err(StorageError::Conflict(_))
        ));
    }

    #[test]
    fn claim_and_transition_use_revision_and_generation_cas() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (contract, attempt) = fixture();
        store
            .create_workflow(&contract, &attempt, CorrelationId::new(), now())
            .unwrap();
        let owner = RuntimeActorId::new();
        let claimed = store
            .claim_workflow_attempt(
                attempt.attempt_id,
                1,
                0,
                owner,
                now() + chrono::Duration::minutes(1),
                CorrelationId::new(),
                "claim-1",
                now(),
            )
            .unwrap();
        assert_eq!(claimed.state, WorkflowState::Running);
        assert_eq!(claimed.revision, 2);
        assert_eq!(claimed.lease_generation, 1);
        assert_eq!(claimed.lease_owner, Some(owner));

        let stale = store.transition_workflow_attempt(
            attempt.attempt_id,
            1,
            0,
            WorkflowState::Verifying,
            CorrelationId::new(),
            "stale-transition",
            now(),
        );
        assert!(matches!(stale, Err(StorageError::Conflict(_))));

        let verifying = store
            .transition_workflow_attempt(
                attempt.attempt_id,
                2,
                1,
                WorkflowState::Verifying,
                CorrelationId::new(),
                "verify-1",
                now(),
            )
            .unwrap();
        assert_eq!(verifying.revision, 3);
        assert!(verifying.accepts_worker_result(attempt.attempt_id, 1));
        let succeeded = store
            .transition_workflow_attempt(
                attempt.attempt_id,
                3,
                1,
                WorkflowState::Succeeded,
                CorrelationId::new(),
                "succeed-1",
                now(),
            )
            .unwrap();
        assert!(succeeded.state.is_terminal());
        assert!(!succeeded.accepts_worker_result(attempt.attempt_id, 1));
        assert_eq!(store.workflow_events(attempt.workflow_id).unwrap().len(), 4);
    }

    #[test]
    fn cancellation_without_unresolved_effects_fences_worker_and_settles_cancelled() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (contract, attempt) = fixture();
        store
            .create_workflow(&contract, &attempt, CorrelationId::new(), now())
            .unwrap();
        let running = store
            .claim_workflow_attempt(
                attempt.attempt_id,
                1,
                0,
                RuntimeActorId::new(),
                now() + chrono::Duration::minutes(1),
                CorrelationId::new(),
                "claim-cancel",
                now(),
            )
            .unwrap();
        let cancelling = store
            .transition_workflow_attempt(
                attempt.attempt_id,
                running.revision,
                running.lease_generation,
                WorkflowState::CancelRequested,
                CorrelationId::new(),
                "cancel-request",
                now(),
            )
            .unwrap();
        assert_eq!(cancelling.lease_generation, running.lease_generation + 1);
        assert!(cancelling.lease_owner.is_none());
        assert!(!cancelling.accepts_worker_result(attempt.attempt_id, running.lease_generation));

        let invalid = store.transition_workflow_attempt(
            attempt.attempt_id,
            cancelling.revision,
            cancelling.lease_generation,
            WorkflowState::Waiting(WaitReason::User),
            CorrelationId::new(),
            "invalid-wait",
            now(),
        );
        assert!(matches!(invalid, Err(StorageError::Conflict(_))));
        let unsupported_reconciliation = store.transition_workflow_attempt(
            attempt.attempt_id,
            cancelling.revision,
            cancelling.lease_generation,
            WorkflowState::Waiting(WaitReason::Reconciliation),
            CorrelationId::new(),
            "reconcile-without-effect",
            now(),
        );
        assert!(matches!(
            unsupported_reconciliation,
            Err(StorageError::Conflict(_))
        ));
        let cancelled = store
            .transition_workflow_attempt(
                attempt.attempt_id,
                cancelling.revision,
                cancelling.lease_generation,
                WorkflowState::Cancelled,
                CorrelationId::new(),
                "cancel-settled",
                now(),
            )
            .unwrap();
        assert_eq!(cancelled.state, WorkflowState::Cancelled);
    }

    #[test]
    fn idempotency_replay_rejects_claim_and_transition_substitution() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (contract, attempt) = fixture();
        store
            .create_workflow(&contract, &attempt, CorrelationId::new(), now())
            .unwrap();
        let owner = RuntimeActorId::new();
        let expiry = now() + chrono::Duration::minutes(1);
        let claimed = store
            .claim_workflow_attempt(
                attempt.attempt_id,
                1,
                0,
                owner,
                expiry,
                CorrelationId::new(),
                "claim-exact",
                now(),
            )
            .unwrap();
        let replay = store
            .claim_workflow_attempt(
                attempt.attempt_id,
                1,
                0,
                owner,
                expiry,
                CorrelationId::new(),
                "claim-exact",
                now(),
            )
            .unwrap();
        assert_eq!(replay, claimed);
        let substituted_claim = store.claim_workflow_attempt(
            attempt.attempt_id,
            1,
            0,
            RuntimeActorId::new(),
            expiry,
            CorrelationId::new(),
            "claim-exact",
            now(),
        );
        assert!(matches!(substituted_claim, Err(StorageError::Conflict(_))));

        let verifying = store
            .transition_workflow_attempt(
                attempt.attempt_id,
                claimed.revision,
                claimed.lease_generation,
                WorkflowState::Verifying,
                CorrelationId::new(),
                "transition-exact",
                now(),
            )
            .unwrap();
        let replay = store
            .transition_workflow_attempt(
                attempt.attempt_id,
                claimed.revision,
                claimed.lease_generation,
                WorkflowState::Verifying,
                CorrelationId::new(),
                "transition-exact",
                now(),
            )
            .unwrap();
        assert_eq!(replay, verifying);

        // An exact claim replay must return the historical claimed projection even after the
        // authoritative attempt has advanced to Verifying.
        let late_claim_replay = store
            .claim_workflow_attempt(
                attempt.attempt_id,
                1,
                0,
                owner,
                expiry,
                CorrelationId::new(),
                "claim-exact",
                now(),
            )
            .unwrap();
        assert_eq!(late_claim_replay, claimed);
        assert_eq!(late_claim_replay.state, WorkflowState::Running);
        assert_eq!(
            store.workflow_attempt(attempt.attempt_id).unwrap().unwrap(),
            verifying
        );
        let substituted_transition = store.transition_workflow_attempt(
            attempt.attempt_id,
            claimed.revision,
            claimed.lease_generation,
            WorkflowState::Failed,
            CorrelationId::new(),
            "transition-exact",
            now(),
        );
        assert!(matches!(
            substituted_transition,
            Err(StorageError::Conflict(_))
        ));
    }

    #[test]
    fn heartbeat_extends_without_transferring_or_reviving_a_lease() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (contract, attempt) = fixture();
        store
            .create_workflow(&contract, &attempt, CorrelationId::new(), now())
            .unwrap();
        let owner = RuntimeActorId::new();
        let claimed = store
            .claim_workflow_attempt(
                attempt.attempt_id,
                1,
                0,
                owner,
                now() + chrono::Duration::minutes(1),
                CorrelationId::new(),
                "heartbeat-claim",
                now(),
            )
            .unwrap();
        let heartbeat_at = now() + chrono::Duration::seconds(10);
        let extended_expiry = now() + chrono::Duration::minutes(2);
        let heartbeat = store
            .heartbeat_workflow_attempt(
                attempt.attempt_id,
                claimed.revision,
                claimed.lease_generation,
                owner,
                extended_expiry,
                CorrelationId::new(),
                "heartbeat-1",
                heartbeat_at,
            )
            .unwrap();
        assert_eq!(heartbeat.revision, claimed.revision + 1);
        assert_eq!(heartbeat.lease_generation, claimed.lease_generation);
        assert_eq!(heartbeat.lease_owner, Some(owner));
        assert_eq!(heartbeat.heartbeat_at, Some(heartbeat_at));
        assert_eq!(heartbeat.lease_expires_at, Some(extended_expiry));

        let transfer = store.heartbeat_workflow_attempt(
            attempt.attempt_id,
            heartbeat.revision,
            heartbeat.lease_generation,
            RuntimeActorId::new(),
            extended_expiry + chrono::Duration::minutes(1),
            CorrelationId::new(),
            "heartbeat-transfer",
            heartbeat_at,
        );
        assert!(matches!(transfer, Err(StorageError::Conflict(_))));
    }

    #[test]
    fn restart_scanner_fences_expired_work_without_reactivating_terminals() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (contract, attempt) = fixture();
        store
            .create_workflow(&contract, &attempt, CorrelationId::new(), now())
            .unwrap();
        let running = store
            .claim_workflow_attempt(
                attempt.attempt_id,
                1,
                0,
                RuntimeActorId::new(),
                now() + chrono::Duration::minutes(1),
                CorrelationId::new(),
                "recovery-claim",
                now(),
            )
            .unwrap();
        let scan_time = now() + chrono::Duration::minutes(2);
        let report = store
            .recover_expired_workflow_leases(CorrelationId::new(), scan_time)
            .unwrap();
        assert_eq!(report.inspected_nonterminal, 1);
        assert_eq!(report.fenced_expired, 1);
        assert_eq!(report.awaiting_reconciliation, 1);
        assert_eq!(report.requeued_for_verification, 0);
        let recovered = store.workflow_attempt(attempt.attempt_id).unwrap().unwrap();
        assert_eq!(
            recovered.state,
            WorkflowState::Waiting(WaitReason::Reconciliation)
        );
        assert_eq!(recovered.revision, running.revision + 1);
        assert_eq!(recovered.lease_generation, running.lease_generation + 1);
        assert!(recovered.lease_owner.is_none());
        assert!(!recovered.accepts_worker_result(attempt.attempt_id, running.lease_generation));

        let second = store
            .recover_expired_workflow_leases(CorrelationId::new(), scan_time)
            .unwrap();
        assert_eq!(second.fenced_expired, 0);
        assert_eq!(store.workflow_events(attempt.workflow_id).unwrap().len(), 3);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn restart_scanner_requeues_expired_verification_and_ignores_terminal_history() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (verifying_contract, verifying_attempt) = fixture();
        store
            .create_workflow(
                &verifying_contract,
                &verifying_attempt,
                CorrelationId::new(),
                now(),
            )
            .unwrap();
        let claimed = store
            .claim_workflow_attempt(
                verifying_attempt.attempt_id,
                1,
                0,
                RuntimeActorId::new(),
                now() + chrono::Duration::minutes(1),
                CorrelationId::new(),
                "verify-recovery-claim",
                now(),
            )
            .unwrap();
        let verifying = store
            .transition_workflow_attempt(
                verifying_attempt.attempt_id,
                claimed.revision,
                claimed.lease_generation,
                WorkflowState::Verifying,
                CorrelationId::new(),
                "verify-recovery-transition",
                now(),
            )
            .unwrap();

        let (terminal_contract, terminal_attempt) = fixture();
        store
            .create_workflow(
                &terminal_contract,
                &terminal_attempt,
                CorrelationId::new(),
                now(),
            )
            .unwrap();
        let terminal_claim = store
            .claim_workflow_attempt(
                terminal_attempt.attempt_id,
                1,
                0,
                RuntimeActorId::new(),
                now() + chrono::Duration::minutes(1),
                CorrelationId::new(),
                "terminal-recovery-claim",
                now(),
            )
            .unwrap();
        let terminal_verifying = store
            .transition_workflow_attempt(
                terminal_attempt.attempt_id,
                terminal_claim.revision,
                terminal_claim.lease_generation,
                WorkflowState::Verifying,
                CorrelationId::new(),
                "terminal-recovery-verifying",
                now(),
            )
            .unwrap();
        let succeeded = store
            .transition_workflow_attempt(
                terminal_attempt.attempt_id,
                terminal_verifying.revision,
                terminal_verifying.lease_generation,
                WorkflowState::Succeeded,
                CorrelationId::new(),
                "terminal-recovery-succeeded",
                now(),
            )
            .unwrap();

        let report = store
            .recover_expired_workflow_leases(
                CorrelationId::new(),
                now() + chrono::Duration::minutes(2),
            )
            .unwrap();
        assert_eq!(report.inspected_nonterminal, 1);
        assert_eq!(report.fenced_expired, 1);
        assert_eq!(report.awaiting_reconciliation, 0);
        assert_eq!(report.requeued_for_verification, 1);

        let recovered = store
            .workflow_attempt(verifying_attempt.attempt_id)
            .unwrap()
            .unwrap();
        assert_eq!(recovered.state, WorkflowState::Ready);
        assert_eq!(recovered.revision, verifying.revision + 1);
        assert_eq!(recovered.lease_generation, verifying.lease_generation + 1);
        assert!(recovered.lease_owner.is_none());
        assert_eq!(
            store
                .workflow_attempt(terminal_attempt.attempt_id)
                .unwrap()
                .unwrap(),
            succeeded
        );
    }

    #[test]
    fn restart_scanner_resumes_only_objectively_satisfied_wait_conditions() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let dependency = waiting_fixture(&mut store, WaitReason::Dependency, "dependency");
        let backoff = waiting_fixture(&mut store, WaitReason::RetryBackoff, "backoff");
        let approval = waiting_fixture(&mut store, WaitReason::Approval, "approval");

        let (cancel_contract, cancel_attempt) = fixture();
        store
            .create_workflow(
                &cancel_contract,
                &cancel_attempt,
                CorrelationId::new(),
                now(),
            )
            .unwrap();
        let cancel_running = store
            .claim_workflow_attempt(
                cancel_attempt.attempt_id,
                cancel_attempt.revision,
                cancel_attempt.lease_generation,
                RuntimeActorId::new(),
                now() + chrono::Duration::minutes(5),
                CorrelationId::new(),
                "cancel-claim",
                now(),
            )
            .unwrap();
        let cancelling = store
            .transition_workflow_attempt(
                cancel_attempt.attempt_id,
                cancel_running.revision,
                cancel_running.lease_generation,
                WorkflowState::CancelRequested,
                CorrelationId::new(),
                "cancel-request",
                now(),
            )
            .unwrap();

        let early = store
            .recover_expired_workflow_leases(
                CorrelationId::new(),
                now() + chrono::Duration::milliseconds(99),
            )
            .unwrap();
        assert_eq!(early.requeued_satisfied_dependencies, 1);
        assert_eq!(early.requeued_elapsed_backoffs, 0);
        assert_eq!(early.settled_cancellations, 1);
        assert_eq!(
            store
                .workflow_attempt(dependency.attempt_id)
                .unwrap()
                .unwrap()
                .state,
            WorkflowState::Ready
        );
        assert_eq!(
            store
                .workflow_attempt(backoff.attempt_id)
                .unwrap()
                .unwrap()
                .state,
            WorkflowState::Waiting(WaitReason::RetryBackoff)
        );

        let due = store
            .recover_expired_workflow_leases(
                CorrelationId::new(),
                now() + chrono::Duration::milliseconds(100),
            )
            .unwrap();
        assert_eq!(due.requeued_satisfied_dependencies, 0);
        assert_eq!(due.requeued_elapsed_backoffs, 1);
        assert_eq!(
            store
                .workflow_attempt(backoff.attempt_id)
                .unwrap()
                .unwrap()
                .state,
            WorkflowState::Ready
        );
        assert_eq!(
            store
                .workflow_attempt(approval.attempt_id)
                .unwrap()
                .unwrap()
                .state,
            WorkflowState::Waiting(WaitReason::Approval)
        );
        let settled = store
            .workflow_attempt(cancelling.attempt_id)
            .unwrap()
            .unwrap();
        assert_eq!(settled.state, WorkflowState::Cancelled);
        assert_eq!(settled.revision, cancelling.revision + 1);
    }

    #[test]
    fn retry_creates_acyclic_attempt_lineage_without_rewinding_terminal_history() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (contract, attempt) = fixture();
        store
            .create_workflow(&contract, &attempt, CorrelationId::new(), now())
            .unwrap();
        let running = store
            .claim_workflow_attempt(
                attempt.attempt_id,
                1,
                0,
                RuntimeActorId::new(),
                now() + chrono::Duration::minutes(1),
                CorrelationId::new(),
                "retry-claim",
                now(),
            )
            .unwrap();
        let failed = store
            .transition_workflow_attempt(
                attempt.attempt_id,
                running.revision,
                running.lease_generation,
                WorkflowState::Failed,
                CorrelationId::new(),
                "retry-fail",
                now(),
            )
            .unwrap();
        let retry = store
            .retry_workflow_attempt(
                failed.attempt_id,
                failed.revision,
                failed.lease_generation,
                CorrelationId::new(),
                "retry-attempt-2",
                now(),
            )
            .unwrap();
        assert_ne!(retry.attempt_id, failed.attempt_id);
        assert_eq!(retry.parent_attempt_id, Some(failed.attempt_id));
        assert_eq!(retry.attempt_number, 2);
        assert_eq!(retry.state, WorkflowState::Ready);
        assert_eq!(retry.revision, 1);
        assert_eq!(retry.lease_generation, 0);
        assert_eq!(
            store.workflow_attempt(failed.attempt_id).unwrap().unwrap(),
            failed
        );
        assert_eq!(
            store
                .retry_workflow_attempt(
                    failed.attempt_id,
                    failed.revision,
                    failed.lease_generation,
                    CorrelationId::new(),
                    "retry-attempt-2",
                    now(),
                )
                .unwrap(),
            retry
        );
    }

    #[test]
    fn dependency_ordering_blocks_claim_until_exact_prerequisite_succeeds() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (prerequisite_contract, prerequisite) = fixture();
        store
            .create_workflow(
                &prerequisite_contract,
                &prerequisite,
                CorrelationId::new(),
                now(),
            )
            .unwrap();

        let (mut dependent_contract, mut dependent) = fixture();
        dependent_contract.dependencies = vec![prerequisite.task_contract.clone()];
        dependent.task_contract.content_hash =
            task_contract_content_hash(&dependent_contract).unwrap();
        store
            .create_workflow(&dependent_contract, &dependent, CorrelationId::new(), now())
            .unwrap();
        let blocked = store.claim_workflow_attempt(
            dependent.attempt_id,
            1,
            0,
            RuntimeActorId::new(),
            now() + chrono::Duration::minutes(1),
            CorrelationId::new(),
            "dependent-early-claim",
            now(),
        );
        assert!(matches!(blocked, Err(StorageError::Conflict(_))));

        let running = store
            .claim_workflow_attempt(
                prerequisite.attempt_id,
                1,
                0,
                RuntimeActorId::new(),
                now() + chrono::Duration::minutes(1),
                CorrelationId::new(),
                "prerequisite-claim",
                now(),
            )
            .unwrap();
        let verifying = store
            .transition_workflow_attempt(
                prerequisite.attempt_id,
                running.revision,
                running.lease_generation,
                WorkflowState::Verifying,
                CorrelationId::new(),
                "prerequisite-verify",
                now(),
            )
            .unwrap();
        store
            .transition_workflow_attempt(
                prerequisite.attempt_id,
                verifying.revision,
                verifying.lease_generation,
                WorkflowState::Succeeded,
                CorrelationId::new(),
                "prerequisite-succeed",
                now(),
            )
            .unwrap();

        let claimed = store
            .claim_workflow_attempt(
                dependent.attempt_id,
                1,
                0,
                RuntimeActorId::new(),
                now() + chrono::Duration::minutes(1),
                CorrelationId::new(),
                "dependent-late-claim",
                now(),
            )
            .unwrap();
        assert_eq!(claimed.state, WorkflowState::Running);
    }

    #[test]
    fn concurrent_claim_race_has_one_authoritative_winner() {
        use std::sync::{Arc, Barrier};

        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("workflow-race.sqlite3");
        let (contract, attempt) = fixture();
        {
            let mut store = SqliteEventStore::open(&database).unwrap();
            store
                .create_workflow(&contract, &attempt, CorrelationId::new(), now())
                .unwrap();
        }
        let attempt_id = attempt.attempt_id;
        let workflow_id = attempt.workflow_id;
        let barrier = Arc::new(Barrier::new(2));
        let handles = (0..2)
            .map(|index| {
                let database = database.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let mut store = SqliteEventStore::open(database).unwrap();
                    barrier.wait();
                    store
                        .claim_workflow_attempt(
                            attempt_id,
                            1,
                            0,
                            RuntimeActorId::new(),
                            now() + chrono::Duration::minutes(1),
                            CorrelationId::new(),
                            &format!("claim-race-{index}"),
                            now(),
                        )
                        .is_ok()
                })
            })
            .collect::<Vec<_>>();
        let successes = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .filter(|succeeded| *succeeded)
            .count();
        assert_eq!(successes, 1);

        let store = SqliteEventStore::open(&database).unwrap();
        let authoritative = store.workflow_attempt(attempt_id).unwrap().unwrap();
        assert_eq!(authoritative.state, WorkflowState::Running);
        assert_eq!(authoritative.revision, 2);
        assert_eq!(authoritative.lease_generation, 1);
        assert_eq!(store.workflow_events(workflow_id).unwrap().len(), 2);
    }
}
