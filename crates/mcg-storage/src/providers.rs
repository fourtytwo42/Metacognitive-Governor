use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use mcg_agents::{
    PROVIDER_PROXY_ENSEMBLE_SCHEMA_VERSION, PROVIDER_PROXY_TURN_SCHEMA_VERSION,
    ProviderProxyEnsembleCheckpoint, ProviderProxyEnsembleMemberState, ProviderProxyEnsemblePhase,
    ProviderProxyShadowCheckpoint, ProviderProxyTurnCheckpoint,
};
use mcg_domain::{ProxyCallId, ProxyConversationId, TaskId};
use mcg_events::{EventEnvelope, EventKind};
use mcg_providers::{
    OutputItem, PROVIDER_DISPATCH_SCHEMA_VERSION, PROXY_LEDGER_SCHEMA_VERSION, ProviderAuthMode,
    ProviderConnection, ProviderDispatchBegin, ProviderDispatchFailure, ProviderDispatchRecord,
    ProviderDispatchState, ProviderModelCatalog, ProviderModelRef, ProxyCallOutcome,
    ProxyCallRecord, ProxyConversation, ProxyConversationView, ProxyModelCallEvidence,
    ProxyModelCallPurpose, ProxyShadowOutcome, ProxyUsageSummary, Usage,
};
use rusqlite::{OptionalExtension, params};

use crate::{SqliteEventStore, StorageError, model_descriptor_source_ref};

impl SqliteEventStore {
    /// Atomically creates or advances a privacy-preserving conversation projection and
    /// appends one immutable completed-call record.
    ///
    /// Exact call retries are idempotent. Reusing either identity for different data,
    /// or rebinding an existing conversation to different governed context, fails closed.
    ///
    /// # Errors
    /// Returns validation, context, conflict, serialization, or `SQLite` failures.
    pub fn record_proxy_call(
        &mut self,
        conversation: &ProxyConversation,
        call: &ProxyCallRecord,
    ) -> Result<(), StorageError> {
        validate_proxy_record(conversation, call)?;
        if let Some(project_id) = conversation.project_id {
            let exists = self.connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE project_id = ?1)",
                [project_id.to_string()],
                |row| row.get::<_, bool>(0),
            )?;
            if !exists {
                return Err(StorageError::ProjectNotFound(project_id));
            }
        }
        if let Some(task_id) = conversation.task_id {
            let task = self
                .load_task(task_id)?
                .ok_or(StorageError::TaskNotFound(task_id))?;
            if let Some(project_id) = conversation.project_id
                && task.project_id != Some(project_id)
            {
                return Err(StorageError::Conflict(
                    "provider proxy task does not belong to the requested project".into(),
                ));
            }
        }

        let transaction = self.connection.transaction()?;
        persist_proxy_call(&transaction, conversation, call)?;
        transaction.commit()?;
        Ok(())
    }

    /// Durably claims one physical upstream request position before network I/O.
    /// An exact existing record is returned rather than overwritten so callers
    /// can fail closed instead of automatically replaying indeterminate work.
    ///
    /// # Errors
    /// Returns validation, identity-conflict, serialization, or `SQLite` failures.
    pub fn begin_provider_dispatch(
        &mut self,
        dispatch: &ProviderDispatchRecord,
    ) -> Result<ProviderDispatchBegin, StorageError> {
        validate_provider_dispatch(dispatch)?;
        if dispatch.state != ProviderDispatchState::Started {
            return Err(StorageError::InvalidData(
                "a provider dispatch must begin in started state".into(),
            ));
        }
        let by_id = self.provider_dispatch(&dispatch.client_request_id)?;
        let by_position = self
            .connection
            .query_row(
                "SELECT dispatch_json FROM provider_proxy_dispatches
                 WHERE call_id = ?1 AND purpose = ?2 AND ordinal = ?3",
                params![
                    dispatch.call_id.to_string(),
                    dispatch_purpose_name(dispatch.purpose),
                    dispatch.ordinal,
                ],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|value| serde_json::from_str::<ProviderDispatchRecord>(&value))
            .transpose()?;
        let mut existing_records = by_id.into_iter().chain(by_position);
        if let Some(existing) = existing_records.next() {
            if existing.call_id != dispatch.call_id
                || existing.client_request_id != dispatch.client_request_id
                || existing.target != dispatch.target
                || existing.purpose != dispatch.purpose
                || existing.ordinal != dispatch.ordinal
            {
                return Err(StorageError::Conflict(
                    "provider dispatch identity or physical call position was rebound".into(),
                ));
            }
            if existing_records.any(|other| other != existing) {
                return Err(StorageError::Conflict(
                    "provider dispatch identity and physical call position disagree".into(),
                ));
            }
            return Ok(ProviderDispatchBegin::AlreadyExists(existing));
        }
        self.connection.execute(
            "INSERT INTO provider_proxy_dispatches(
                client_request_id, call_id, purpose, ordinal, state,
                dispatch_json, started_at, completed_at
             ) VALUES (?1, ?2, ?3, ?4, 'started', ?5, ?6, NULL)",
            params![
                dispatch.client_request_id,
                dispatch.call_id.to_string(),
                dispatch_purpose_name(dispatch.purpose),
                dispatch.ordinal,
                serde_json::to_string(dispatch)?,
                dispatch.started_at.to_rfc3339(),
            ],
        )?;
        Ok(ProviderDispatchBegin::Begun)
    }

    /// Marks a previously started physical request as having returned one
    /// validated upstream response. Exact retries are idempotent; response
    /// identity cannot be rebound.
    ///
    /// # Errors
    /// Returns missing-dispatch, validation, conflict, serialization, or
    /// `SQLite` failures.
    pub fn complete_provider_dispatch(
        &mut self,
        client_request_id: &str,
        response_id: &str,
        completed_at: DateTime<Utc>,
    ) -> Result<ProviderDispatchRecord, StorageError> {
        if response_id.trim().is_empty() || response_id.len() > 1_024 {
            return Err(StorageError::InvalidData(
                "provider dispatch response identity is invalid".into(),
            ));
        }
        let prior = self
            .provider_dispatch(client_request_id)?
            .ok_or_else(|| StorageError::InvalidData("provider dispatch was not started".into()))?;
        let completed_at = completed_at.max(prior.started_at);
        if prior.state == ProviderDispatchState::Completed {
            if prior.response_id.as_deref() == Some(response_id) {
                return Ok(prior);
            }
            return Err(StorageError::Conflict(
                "provider dispatch response identity was rebound".into(),
            ));
        }
        if prior.state == ProviderDispatchState::Failed {
            return Err(StorageError::Conflict(
                "failed provider dispatch cannot be completed".into(),
            ));
        }
        let completed = ProviderDispatchRecord {
            state: ProviderDispatchState::Completed,
            response_id: Some(response_id.to_owned()),
            failure: None,
            completed_at: Some(completed_at),
            ..prior
        };
        validate_provider_dispatch(&completed)?;
        let changed = self.connection.execute(
            "UPDATE provider_proxy_dispatches
             SET state = 'completed', dispatch_json = ?2, completed_at = ?3
             WHERE client_request_id = ?1 AND state = 'started'",
            params![
                client_request_id,
                serde_json::to_string(&completed)?,
                completed_at.to_rfc3339(),
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::Conflict(
                "provider dispatch did not advance exactly once".into(),
            ));
        }
        Ok(completed)
    }

    /// Marks a claimed physical request as a definite, classified failure.
    /// Ambiguous transport failures must remain started and must not use this method.
    ///
    /// # Errors
    /// Returns missing-dispatch, validation, conflict, serialization, or `SQLite` failures.
    pub fn fail_provider_dispatch(
        &mut self,
        client_request_id: &str,
        failure: ProviderDispatchFailure,
        failed_at: DateTime<Utc>,
    ) -> Result<ProviderDispatchRecord, StorageError> {
        let prior = self
            .provider_dispatch(client_request_id)?
            .ok_or_else(|| StorageError::InvalidData("provider dispatch was not started".into()))?;
        let failed_at = failed_at.max(prior.started_at);
        match prior.state {
            ProviderDispatchState::Failed if prior.failure == Some(failure) => return Ok(prior),
            ProviderDispatchState::Failed => {
                return Err(StorageError::Conflict(
                    "provider dispatch failure classification was rebound".into(),
                ));
            }
            ProviderDispatchState::Completed => {
                return Err(StorageError::Conflict(
                    "completed provider dispatch cannot be failed".into(),
                ));
            }
            ProviderDispatchState::Started => {}
        }
        let failed = ProviderDispatchRecord {
            state: ProviderDispatchState::Failed,
            response_id: None,
            failure: Some(failure),
            completed_at: Some(failed_at),
            ..prior
        };
        validate_provider_dispatch(&failed)?;
        let changed = self.connection.execute(
            "UPDATE provider_proxy_dispatches
             SET state = 'failed', dispatch_json = ?2, completed_at = ?3
             WHERE client_request_id = ?1 AND state = 'started'",
            params![
                client_request_id,
                serde_json::to_string(&failed)?,
                failed_at.to_rfc3339(),
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::Conflict(
                "provider dispatch did not fail exactly once".into(),
            ));
        }
        Ok(failed)
    }

    /// Loads content-free physical request evidence by its stable client ID.
    ///
    /// # Errors
    /// Returns decoding or `SQLite` failures.
    pub fn provider_dispatch(
        &self,
        client_request_id: &str,
    ) -> Result<Option<ProviderDispatchRecord>, StorageError> {
        self.read_optional_json(
            "SELECT dispatch_json FROM provider_proxy_dispatches WHERE client_request_id = ?1",
            client_request_id,
        )
    }

    /// Lists physical dispatch evidence for one logical proxy call.
    ///
    /// # Errors
    /// Returns decoding or `SQLite` failures.
    pub fn provider_dispatches_for_call(
        &self,
        call_id: ProxyCallId,
    ) -> Result<Vec<ProviderDispatchRecord>, StorageError> {
        self.read_json_rows(
            "SELECT dispatch_json FROM provider_proxy_dispatches
             WHERE call_id = ?1 ORDER BY ordinal, purpose",
            &call_id.to_string(),
        )
    }

    /// Replaces the private continuation checkpoint for one MCG-managed
    /// provider-proxy call. Immutable request, context, conversation, and route
    /// bindings cannot be changed by a retry.
    ///
    /// # Errors
    /// Returns validation, stale-progress, identity-conflict, serialization, or
    /// `SQLite` failures.
    pub fn save_provider_proxy_turn_checkpoint(
        &mut self,
        checkpoint: &ProviderProxyTurnCheckpoint,
    ) -> Result<(), StorageError> {
        validate_provider_proxy_turn_checkpoint(checkpoint)?;
        validate_checkpoint_task(self, checkpoint)?;
        let project_id = checkpoint
            .conversation
            .project_id
            .ok_or_else(|| StorageError::InvalidData("managed turn has no project".into()))?;
        let task_id = checkpoint
            .conversation
            .task_id
            .ok_or_else(|| StorageError::InvalidData("managed turn has no task".into()))?;
        let prior = self.provider_proxy_turn_checkpoint(checkpoint.call_id)?;
        if let Some(prior) = prior.as_ref() {
            validate_checkpoint_advance(prior, checkpoint)?;
        }
        self.connection.execute(
            "INSERT INTO provider_proxy_turn_checkpoints(
                call_id, conversation_id, project_id, task_id, request_sha256,
                checkpoint_json, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(call_id) DO UPDATE SET
                checkpoint_json = excluded.checkpoint_json,
                updated_at = excluded.updated_at",
            params![
                checkpoint.call_id.to_string(),
                checkpoint.conversation.id.to_string(),
                project_id.to_string(),
                task_id.to_string(),
                checkpoint.request_sha256,
                serde_json::to_string(checkpoint)?,
                checkpoint.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Loads one private managed provider-proxy continuation checkpoint.
    ///
    /// # Errors
    /// Returns decoding or `SQLite` failures.
    pub fn provider_proxy_turn_checkpoint(
        &self,
        call_id: ProxyCallId,
    ) -> Result<Option<ProviderProxyTurnCheckpoint>, StorageError> {
        self.read_optional_json(
            "SELECT checkpoint_json FROM provider_proxy_turn_checkpoints WHERE call_id = ?1",
            &call_id.to_string(),
        )
    }

    /// Creates or monotonically advances one private multi-model proxy
    /// checkpoint. Member output exists only in this incomplete-call record.
    ///
    /// # Errors
    /// Returns validation, context, stale-progress, identity-conflict,
    /// serialization, or `SQLite` failures.
    pub fn save_provider_proxy_ensemble_checkpoint(
        &mut self,
        checkpoint: &ProviderProxyEnsembleCheckpoint,
    ) -> Result<(), StorageError> {
        validate_provider_proxy_ensemble_checkpoint(checkpoint)?;
        validate_ensemble_checkpoint_context(self, checkpoint)?;
        if let Some(prior) = self.provider_proxy_ensemble_checkpoint(checkpoint.call_id)? {
            validate_ensemble_checkpoint_advance(&prior, checkpoint)?;
        }
        self.connection.execute(
            "INSERT INTO provider_proxy_ensemble_checkpoints(
                call_id, conversation_id, project_id, task_id, request_sha256,
                checkpoint_json, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(call_id) DO UPDATE SET
                checkpoint_json = excluded.checkpoint_json,
                updated_at = excluded.updated_at",
            params![
                checkpoint.call_id.to_string(),
                checkpoint.conversation.id.to_string(),
                checkpoint.conversation.project_id.map(|id| id.to_string()),
                checkpoint.conversation.task_id.map(|id| id.to_string()),
                checkpoint.request_sha256,
                serde_json::to_string(checkpoint)?,
                checkpoint.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Loads one private multi-model proxy checkpoint.
    ///
    /// # Errors
    /// Returns decoding or `SQLite` failures.
    pub fn provider_proxy_ensemble_checkpoint(
        &self,
        call_id: mcg_domain::ProxyCallId,
    ) -> Result<Option<ProviderProxyEnsembleCheckpoint>, StorageError> {
        self.read_optional_json(
            "SELECT checkpoint_json FROM provider_proxy_ensemble_checkpoints WHERE call_id = ?1",
            &call_id.to_string(),
        )
    }

    /// Atomically replaces one private ensemble checkpoint with its immutable,
    /// content-free terminal ledger record.
    ///
    /// # Errors
    /// Returns validation, checkpoint-conflict, serialization, or `SQLite`
    /// failures.
    pub fn complete_provider_proxy_ensemble(
        &mut self,
        checkpoint: &ProviderProxyEnsembleCheckpoint,
        call: &ProxyCallRecord,
    ) -> Result<(), StorageError> {
        validate_provider_proxy_ensemble_checkpoint(checkpoint)?;
        validate_proxy_record(&checkpoint.conversation, call)?;
        validate_ensemble_checkpoint_completion(checkpoint, call)?;
        validate_ensemble_checkpoint_context(self, checkpoint)?;
        let transaction = self.connection.transaction()?;
        let stored = transaction
            .query_row(
                "SELECT checkpoint_json FROM provider_proxy_ensemble_checkpoints WHERE call_id = ?1",
                [checkpoint.call_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if let Some(stored) = stored {
            let stored: ProviderProxyEnsembleCheckpoint = serde_json::from_str(&stored)?;
            if stored != *checkpoint {
                return Err(StorageError::Conflict(
                    "provider proxy ensemble completion did not match its durable checkpoint"
                        .into(),
                ));
            }
        } else {
            let completed = transaction
                .query_row(
                    "SELECT call_json FROM provider_proxy_calls WHERE call_id = ?1",
                    [checkpoint.call_id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            let exact_retry = completed
                .map(|completed| serde_json::from_str::<ProxyCallRecord>(&completed))
                .transpose()?
                .is_some_and(|completed| completed == *call);
            if !exact_retry {
                return Err(StorageError::Conflict(
                    "provider proxy ensemble has no durable checkpoint".into(),
                ));
            }
        }
        persist_proxy_call(&transaction, &checkpoint.conversation, call)?;
        transaction.execute(
            "DELETE FROM provider_proxy_ensemble_checkpoints WHERE call_id = ?1",
            [checkpoint.call_id.to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Loads one immutable privacy-preserving provider call by its globally
    /// caller-owned identity.
    ///
    /// # Errors
    /// Returns decoding or `SQLite` failures.
    pub fn proxy_call(
        &self,
        call_id: ProxyCallId,
    ) -> Result<Option<ProxyCallRecord>, StorageError> {
        self.read_optional_json(
            "SELECT call_json FROM provider_proxy_calls WHERE call_id = ?1",
            &call_id.to_string(),
        )
    }

    /// Lists every immutable content-free proxy call in deterministic order for
    /// local routing evidence. Generated content and request bodies are absent.
    ///
    /// # Errors
    /// Returns decoding or `SQLite` failures.
    pub fn proxy_calls(&self) -> Result<Vec<ProxyCallRecord>, StorageError> {
        self.read_all_json(
            "SELECT call_json FROM provider_proxy_calls ORDER BY completed_at, call_id",
        )
    }

    /// Lists private managed provider-proxy continuation checkpoints for one
    /// durable task in deterministic update order.
    ///
    /// # Errors
    /// Returns decoding or `SQLite` failures.
    pub fn provider_proxy_turn_checkpoints_for_task(
        &self,
        task_id: TaskId,
    ) -> Result<Vec<ProviderProxyTurnCheckpoint>, StorageError> {
        self.read_json_rows(
            "SELECT checkpoint_json FROM provider_proxy_turn_checkpoints
             WHERE task_id = ?1 ORDER BY updated_at, call_id",
            &task_id.to_string(),
        )
    }

    /// Atomically commits the final privacy-preserving call ledger record and
    /// removes its exact private continuation checkpoint. Exact completion
    /// retries remain idempotent.
    ///
    /// # Errors
    /// Returns validation, checkpoint-conflict, serialization, or `SQLite`
    /// failures.
    pub fn complete_provider_proxy_turn(
        &mut self,
        checkpoint: &ProviderProxyTurnCheckpoint,
        call: &ProxyCallRecord,
    ) -> Result<(), StorageError> {
        validate_provider_proxy_turn_checkpoint(checkpoint)?;
        validate_proxy_record(&checkpoint.conversation, call)?;
        validate_checkpoint_completion(checkpoint, call)?;
        validate_checkpoint_task(self, checkpoint)?;
        let transaction = self.connection.transaction()?;
        let stored = transaction
            .query_row(
                "SELECT checkpoint_json FROM provider_proxy_turn_checkpoints WHERE call_id = ?1",
                [checkpoint.call_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if let Some(stored) = stored {
            let stored: ProviderProxyTurnCheckpoint = serde_json::from_str(&stored)?;
            if stored != *checkpoint {
                return Err(StorageError::Conflict(
                    "provider proxy turn completion did not match its durable checkpoint".into(),
                ));
            }
        } else {
            let completed = transaction
                .query_row(
                    "SELECT call_json FROM provider_proxy_calls WHERE call_id = ?1",
                    [checkpoint.call_id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            let exact_retry = completed
                .map(|completed| serde_json::from_str::<ProxyCallRecord>(&completed))
                .transpose()?
                .is_some_and(|completed| completed == *call);
            if !exact_retry {
                return Err(StorageError::Conflict(
                    "provider proxy turn has no durable checkpoint".into(),
                ));
            }
        }
        persist_proxy_call(&transaction, &checkpoint.conversation, call)?;
        transaction.execute(
            "DELETE FROM provider_proxy_turn_checkpoints WHERE call_id = ?1",
            [checkpoint.call_id.to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Lists durable proxy conversations without prompt or generated content.
    ///
    /// # Errors
    /// Returns decoding or `SQLite` failures.
    pub fn proxy_conversations(&self) -> Result<Vec<ProxyConversation>, StorageError> {
        self.read_all_json(
            "SELECT conversation_json FROM provider_proxy_conversations
             ORDER BY updated_at DESC, conversation_id",
        )
    }

    /// Loads one conversation and its immutable call history.
    ///
    /// # Errors
    /// Returns decoding or `SQLite` failures.
    pub fn proxy_conversation(
        &self,
        conversation_id: ProxyConversationId,
    ) -> Result<Option<ProxyConversationView>, StorageError> {
        let Some(conversation) = self.read_optional_json(
            "SELECT conversation_json FROM provider_proxy_conversations
             WHERE conversation_id = ?1",
            &conversation_id.to_string(),
        )?
        else {
            return Ok(None);
        };
        let calls = self.read_json_rows(
            "SELECT call_json FROM provider_proxy_calls WHERE conversation_id = ?1
             ORDER BY completed_at, call_id",
            &conversation_id.to_string(),
        )?;
        Ok(Some(ProxyConversationView {
            conversation,
            calls,
        }))
    }

    /// Aggregates durable usage and only costs whose pricing was known at call time.
    ///
    /// # Errors
    /// Returns decoding or `SQLite` failures.
    pub fn proxy_usage_summary(&self) -> Result<ProxyUsageSummary, StorageError> {
        let conversations = self.proxy_conversations()?;
        let calls = self.proxy_calls()?;
        let mut usage = Usage::default();
        let mut known_actual_cost_usd = 0.0;
        let mut unknown_cost_call_count = 0;
        for call in &calls {
            usage.input_tokens = usage.input_tokens.saturating_add(call.usage.input_tokens);
            usage.cached_input_tokens = usage
                .cached_input_tokens
                .saturating_add(call.usage.cached_input_tokens);
            usage.output_tokens = usage.output_tokens.saturating_add(call.usage.output_tokens);
            usage.reasoning_tokens = usage
                .reasoning_tokens
                .saturating_add(call.usage.reasoning_tokens);
            usage.total_tokens = usage.total_tokens.saturating_add(call.usage.total_tokens);
            if let Some(cost) = call.actual_cost_usd {
                known_actual_cost_usd += cost;
            } else {
                unknown_cost_call_count += 1;
            }
        }
        Ok(ProxyUsageSummary {
            conversation_count: conversations.len() as u64,
            call_count: calls.len() as u64,
            usage,
            known_actual_cost_usd,
            unknown_cost_call_count,
        })
    }

    /// Atomically records secret-free provider metadata, a discovered catalog,
    /// and both global configuration events.
    ///
    /// # Errors
    ///
    /// Returns an error for inconsistent metadata or failed atomic persistence.
    pub fn record_provider_configuration(
        &mut self,
        connection: &ProviderConnection,
        catalog: &ProviderModelCatalog,
        at: DateTime<Utc>,
    ) -> Result<Vec<EventEnvelope>, StorageError> {
        connection
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        if catalog.connection_id != connection.id {
            return Err(StorageError::InvalidData(
                "provider catalog identity does not match its connection".into(),
            ));
        }
        validate_catalog(catalog)?;
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO provider_connections(connection_id, connection_json, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(connection_id) DO UPDATE SET
                connection_json = excluded.connection_json,
                updated_at = excluded.updated_at",
            params![
                connection.id,
                serde_json::to_string(connection)?,
                at.to_rfc3339(),
            ],
        )?;
        transaction.execute(
            "INSERT INTO provider_catalogs(connection_id, catalog_json, discovered_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(connection_id) DO UPDATE SET
                catalog_json = excluded.catalog_json,
                discovered_at = excluded.discovered_at",
            params![
                catalog.connection_id,
                serde_json::to_string(catalog)?,
                catalog.discovered_at.to_rfc3339(),
            ],
        )?;
        let connection_event = Self::insert_event(
            &transaction,
            &EventEnvelope::new(
                EventKind::ProviderConnectionRecorded,
                None,
                serde_json::json!({"connection": connection}),
            ),
        )?;
        let catalog_payload = provider_catalog_event_payload(catalog)?;
        let catalog_event = Self::insert_event(
            &transaction,
            &EventEnvelope::new(
                EventKind::ProviderCatalogDiscovered,
                None,
                serde_json::json!({"catalog": catalog_payload}),
            ),
        )?;
        transaction.commit()?;
        Ok(vec![connection_event, catalog_event])
    }

    /// Lists all secret-free provider connections in stable identity order.
    ///
    /// # Errors
    ///
    /// Returns an error when a row cannot be decoded.
    pub fn provider_connections(&self) -> Result<Vec<ProviderConnection>, StorageError> {
        self.read_all_json(
            "SELECT connection_json FROM provider_connections ORDER BY connection_id",
        )
    }

    /// Loads one secret-free provider connection.
    ///
    /// # Errors
    ///
    /// Returns an error when a row cannot be decoded.
    pub fn provider_connection(
        &self,
        connection_id: &str,
    ) -> Result<Option<ProviderConnection>, StorageError> {
        self.read_optional_json(
            "SELECT connection_json FROM provider_connections WHERE connection_id = ?1",
            connection_id,
        )
    }

    /// Replaces one provider's discovered model snapshot and appends its global event.
    ///
    /// # Errors
    ///
    /// Returns not-found for an unknown connection or on failed atomic persistence.
    pub fn record_provider_catalog(
        &mut self,
        catalog: &ProviderModelCatalog,
    ) -> Result<EventEnvelope, StorageError> {
        let known = self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM provider_connections WHERE connection_id = ?1)",
            [&catalog.connection_id],
            |row| row.get::<_, bool>(0),
        )?;
        if !known {
            return Err(StorageError::InvalidData(format!(
                "provider connection {} was not found",
                catalog.connection_id
            )));
        }
        validate_catalog(catalog)?;
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO provider_catalogs(connection_id, catalog_json, discovered_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(connection_id) DO UPDATE SET
                catalog_json = excluded.catalog_json,
                discovered_at = excluded.discovered_at",
            params![
                catalog.connection_id,
                serde_json::to_string(catalog)?,
                catalog.discovered_at.to_rfc3339(),
            ],
        )?;
        let catalog_payload = provider_catalog_event_payload(catalog)?;
        let event = Self::insert_event(
            &transaction,
            &EventEnvelope::new(
                EventKind::ProviderCatalogDiscovered,
                None,
                serde_json::json!({"catalog": catalog_payload}),
            ),
        )?;
        transaction.commit()?;
        Ok(event)
    }

    /// Loads the latest discovered model catalog for one provider.
    ///
    /// # Errors
    ///
    /// Returns an error when the row cannot be decoded.
    pub fn provider_catalog(
        &self,
        connection_id: &str,
    ) -> Result<Option<ProviderModelCatalog>, StorageError> {
        let json = self
            .connection
            .query_row(
                "SELECT catalog_json FROM provider_catalogs WHERE connection_id = ?1",
                [connection_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        json.map(|value| serde_json::from_str(&value).map_err(StorageError::from))
            .transpose()
    }

    /// Appends secret-free evidence that an OAuth connection rotated credentials.
    ///
    /// # Errors
    ///
    /// Returns not-found for an unknown connection, rejects non-OAuth connections,
    /// and returns persistence failures.
    pub fn record_provider_credentials_refreshed(
        &mut self,
        connection_id: &str,
        at: DateTime<Utc>,
    ) -> Result<EventEnvelope, StorageError> {
        let connection = self.provider_connection(connection_id)?.ok_or_else(|| {
            StorageError::InvalidData(format!("provider connection {connection_id} was not found"))
        })?;
        if !matches!(
            connection.auth_mode,
            ProviderAuthMode::ChatGptBrowserOauth | ProviderAuthMode::ChatGptDeviceCode
        ) {
            return Err(StorageError::InvalidData(
                "provider credential refresh requires an OAuth connection".into(),
            ));
        }
        let transaction = self.connection.transaction()?;
        let event = Self::insert_event(
            &transaction,
            &EventEnvelope::new(
                EventKind::ProviderCredentialsRefreshed,
                None,
                serde_json::json!({
                    "connection_id": connection_id,
                    "auth_mode": connection.auth_mode,
                    "refreshed_at": at,
                }),
            ),
        )?;
        transaction.commit()?;
        Ok(event)
    }

    /// Atomically removes one active provider projection and catalog while
    /// retaining its immutable call/accounting history and a secret-free audit event.
    ///
    /// # Errors
    /// Returns not-found or persistence failures without partially removing the
    /// active projection.
    pub fn record_provider_connection_removed(
        &mut self,
        connection_id: &str,
        at: DateTime<Utc>,
    ) -> Result<EventEnvelope, StorageError> {
        let connection = self.provider_connection(connection_id)?.ok_or_else(|| {
            StorageError::InvalidData(format!("provider connection {connection_id} was not found"))
        })?;
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "DELETE FROM provider_catalogs WHERE connection_id = ?1",
            [connection_id],
        )?;
        let removed = transaction.execute(
            "DELETE FROM provider_connections WHERE connection_id = ?1",
            [connection_id],
        )?;
        if removed != 1 {
            return Err(StorageError::Conflict(format!(
                "provider connection {connection_id} changed during removal"
            )));
        }
        let event = Self::insert_event(
            &transaction,
            &EventEnvelope::new(
                EventKind::ProviderConnectionRemoved,
                None,
                serde_json::json!({
                    "connection_id": connection_id,
                    "auth_mode": connection.auth_mode,
                    "removed_at": at,
                }),
            ),
        )?;
        transaction.commit()?;
        Ok(event)
    }
}

fn proxy_shadow_is_valid(call: &ProxyCallRecord) -> bool {
    let shadow_calls = call
        .model_calls
        .iter()
        .filter(|evidence| evidence.purpose == mcg_providers::ProxyModelCallPurpose::Shadow)
        .collect::<Vec<_>>();
    match call.shadow.as_ref() {
        None => shadow_calls.is_empty(),
        Some(shadow) => {
            !shadow.target.provider_id.trim().is_empty()
                && !shadow.target.model_id.trim().is_empty()
                && shadow.target
                    != (mcg_providers::ProviderModelRef {
                        provider_id: call.provider_id.clone(),
                        model_id: call.model_id.clone(),
                    })
                && shadow.duration_ms > 0
                && shadow.duration_ms <= 86_400_000
                && match shadow.outcome {
                    mcg_providers::ProxyShadowOutcome::Completed => {
                        shadow.failure.is_none()
                            && matches!(shadow_calls.as_slice(), [evidence]
                                if evidence.target == shadow.target
                                    && evidence.duration_ms == shadow.duration_ms)
                    }
                    mcg_providers::ProxyShadowOutcome::Failed => {
                        shadow.failure.is_some() && shadow_calls.is_empty()
                    }
                    mcg_providers::ProxyShadowOutcome::TimedOut => {
                        shadow.failure.is_none() && shadow_calls.is_empty()
                    }
                }
        }
    }
}

fn checkpoint_shadow_result_is_valid(
    shadow: &ProviderProxyShadowCheckpoint,
    active: &ProviderModelRef,
) -> bool {
    let route = &shadow.route;
    if route.target.provider_id.trim().is_empty()
        || route.target.model_id.trim().is_empty()
        || &route.target == active
        || route
            .estimated_cost
            .amount_usd
            .is_some_and(|cost| !cost.is_finite() || cost < 0.0)
    {
        return false;
    }
    match (shadow.receipt.as_ref(), shadow.evidence.as_ref()) {
        (None, None) => true,
        (None, Some(_)) => false,
        (Some(receipt), evidence) => {
            receipt.target == route.target
                && receipt.duration_ms > 0
                && receipt.duration_ms <= 86_400_000
                && match receipt.outcome {
                    ProxyShadowOutcome::Completed => {
                        receipt.failure.is_none()
                            && matches!(evidence, Some(evidence)
                                if evidence.target == route.target
                                    && evidence.purpose == ProxyModelCallPurpose::Shadow
                                    && !evidence.response_id.trim().is_empty()
                                    && evidence.duration_ms == receipt.duration_ms
                                    && evidence.actual_cost_usd
                                        .is_none_or(|cost| cost.is_finite() && cost >= 0.0))
                    }
                    ProxyShadowOutcome::Failed => receipt.failure.is_some() && evidence.is_none(),
                    ProxyShadowOutcome::TimedOut => receipt.failure.is_none() && evidence.is_none(),
                }
        }
    }
}

fn checkpoint_shadow_is_valid(checkpoint: &ProviderProxyTurnCheckpoint) -> bool {
    if !checkpoint.shadow_requested && checkpoint.shadow.is_some() {
        return false;
    }
    checkpoint.shadow.as_ref().is_none_or(|shadow| {
        checkpoint_shadow_result_is_valid(shadow, &checkpoint.pinned_route.target)
    })
}

fn checkpoint_shadow_advance_is_valid(
    prior: &ProviderProxyTurnCheckpoint,
    next: &ProviderProxyTurnCheckpoint,
) -> bool {
    match (prior.shadow.as_ref(), next.shadow.as_ref()) {
        (None, None) => true,
        (Some(prior), Some(next)) if prior.route == next.route => prior
            .receipt
            .as_ref()
            .is_none_or(|_| prior.receipt == next.receipt && prior.evidence == next.evidence),
        _ => false,
    }
}

fn validate_proxy_record(
    conversation: &ProxyConversation,
    call: &ProxyCallRecord,
) -> Result<(), StorageError> {
    let mut model_usage = Usage::default();
    for evidence in &call.model_calls {
        model_usage.input_tokens = model_usage
            .input_tokens
            .saturating_add(evidence.usage.input_tokens);
        model_usage.cached_input_tokens = model_usage
            .cached_input_tokens
            .saturating_add(evidence.usage.cached_input_tokens);
        model_usage.output_tokens = model_usage
            .output_tokens
            .saturating_add(evidence.usage.output_tokens);
        model_usage.reasoning_tokens = model_usage
            .reasoning_tokens
            .saturating_add(evidence.usage.reasoning_tokens);
        model_usage.total_tokens = model_usage
            .total_tokens
            .saturating_add(evidence.usage.total_tokens);
    }
    let model_cost = call.model_calls.iter().try_fold(0.0, |total, evidence| {
        Some(total + evidence.actual_cost_usd?)
    });
    let model_duration = call.model_calls.iter().fold(0_u64, |total, evidence| {
        total.saturating_add(evidence.duration_ms)
    });
    let logical_response_matches = call
        .model_calls
        .iter()
        .rev()
        .find(|evidence| evidence.purpose != mcg_providers::ProxyModelCallPurpose::Shadow)
        .is_none_or(|evidence| evidence.response_id == call.response_id);
    if conversation.schema_version != PROXY_LEDGER_SCHEMA_VERSION
        || call.schema_version != PROXY_LEDGER_SCHEMA_VERSION
    {
        return Err(StorageError::InvalidData(
            "unsupported provider proxy ledger schema version".into(),
        ));
    }
    if call.conversation_id != conversation.id || conversation.updated_at != call.completed_at {
        return Err(StorageError::InvalidData(
            "provider proxy call does not match its conversation projection".into(),
        ));
    }
    if conversation.created_at > conversation.updated_at
        || call.alias.trim().is_empty()
        || call.provider_id.trim().is_empty()
        || call.model_id.trim().is_empty()
        || call.response_id.trim().is_empty()
        || call.request_sha256.len() != 64
        || !call
            .request_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || call
            .estimated_cost_usd
            .is_some_and(|cost| !cost.is_finite() || cost < 0.0)
        || call
            .actual_cost_usd
            .is_some_and(|cost| !cost.is_finite() || cost < 0.0)
        || call.duration_ms > 86_400_000
        || call.model_calls.len() > 64
        || call.model_calls.iter().any(|evidence| {
            evidence.target.provider_id.trim().is_empty()
                || evidence.target.model_id.trim().is_empty()
                || evidence.response_id.trim().is_empty()
                || evidence.duration_ms == 0
                || evidence.duration_ms > 86_400_000
                || evidence
                    .actual_cost_usd
                    .is_some_and(|cost| !cost.is_finite() || cost < 0.0)
        })
        || !proxy_shadow_is_valid(call)
        || !call.model_calls.is_empty()
            && (model_usage != call.usage
                || model_cost != call.actual_cost_usd
                || model_duration != call.duration_ms
                || !logical_response_matches)
    {
        return Err(StorageError::InvalidData(
            "provider proxy ledger record is invalid".into(),
        ));
    }
    Ok(())
}

fn persist_proxy_call(
    transaction: &rusqlite::Transaction<'_>,
    conversation: &ProxyConversation,
    call: &ProxyCallRecord,
) -> Result<(), StorageError> {
    let prior_conversation = transaction
        .query_row(
            "SELECT conversation_json FROM provider_proxy_conversations
             WHERE conversation_id = ?1",
            [conversation.id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    if let Some(prior) = prior_conversation {
        let prior: ProxyConversation = serde_json::from_str(&prior)?;
        if prior.project_id != conversation.project_id || prior.task_id != conversation.task_id {
            return Err(StorageError::Conflict(
                "provider proxy conversation context is immutable".into(),
            ));
        }
    }

    let prior_call = transaction
        .query_row(
            "SELECT call_json FROM provider_proxy_calls WHERE call_id = ?1",
            [call.id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    if let Some(prior) = prior_call {
        let prior: ProxyCallRecord = serde_json::from_str(&prior)?;
        if prior == *call {
            return Ok(());
        }
        return Err(StorageError::Conflict(
            "provider proxy call identity was reused with different data".into(),
        ));
    }

    transaction.execute(
        "INSERT INTO provider_proxy_conversations(
            conversation_id, project_id, task_id, conversation_json, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(conversation_id) DO UPDATE SET
            conversation_json = excluded.conversation_json,
            updated_at = excluded.updated_at",
        params![
            conversation.id.to_string(),
            conversation.project_id.map(|id| id.to_string()),
            conversation.task_id.map(|id| id.to_string()),
            serde_json::to_string(conversation)?,
            conversation.created_at.to_rfc3339(),
            conversation.updated_at.to_rfc3339(),
        ],
    )?;
    transaction.execute(
        "INSERT INTO provider_proxy_calls(call_id, conversation_id, call_json, completed_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            call.id.to_string(),
            call.conversation_id.to_string(),
            serde_json::to_string(call)?,
            call.completed_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

#[allow(clippy::too_many_lines)] // Keep the complete persisted-state invariant visible as one boundary.
fn validate_provider_proxy_ensemble_checkpoint(
    checkpoint: &ProviderProxyEnsembleCheckpoint,
) -> Result<(), StorageError> {
    let conversation = &checkpoint.conversation;
    let targets = checkpoint
        .members
        .iter()
        .map(|member| {
            (
                member.route.target.provider_id.clone(),
                member.route.target.model_id.clone(),
            )
        })
        .collect::<BTreeSet<_>>();
    let successful = checkpoint
        .members
        .iter()
        .filter_map(|member| match &member.state {
            ProviderProxyEnsembleMemberState::Succeeded { response, evidence } => {
                Some((member, response, evidence))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let pending = checkpoint
        .members
        .iter()
        .filter(|member| matches!(member.state, ProviderProxyEnsembleMemberState::Pending))
        .count();
    let evidence_ids = checkpoint
        .model_calls
        .iter()
        .map(|evidence| {
            (
                evidence.target.provider_id.as_str(),
                evidence.target.model_id.as_str(),
                evidence.response_id.as_str(),
            )
        })
        .collect::<BTreeSet<_>>();
    let tool_call_count = checkpoint.response.as_ref().map_or(0, |response| {
        response
            .output
            .iter()
            .filter(|item| matches!(item, OutputItem::ToolCall { .. }))
            .count()
    });
    let request_ids = checkpoint
        .tool_outcomes
        .iter()
        .map(|outcome| outcome.request_id)
        .collect::<BTreeSet<_>>();
    let member_evidence_valid = successful.iter().all(|(member, response, evidence)| {
        evidence.purpose == mcg_providers::ProxyModelCallPurpose::EnsembleMember
            && evidence.target == member.route.target
            && evidence.response_id == response.id
            && evidence.usage == response.usage
            && response.model == member.route.target.model_id
            && checkpoint
                .model_calls
                .iter()
                .filter(|candidate| *candidate == *evidence)
                .count()
                == 1
    });
    let member_call_count = checkpoint
        .model_calls
        .iter()
        .filter(|evidence| evidence.purpose == mcg_providers::ProxyModelCallPurpose::EnsembleMember)
        .count();
    let synthesis_calls = checkpoint
        .model_calls
        .iter()
        .filter(|evidence| {
            evidence.purpose == mcg_providers::ProxyModelCallPurpose::EnsembleSynthesis
        })
        .collect::<Vec<_>>();
    let physical_sequence_valid =
        match checkpoint.phase {
            ProviderProxyEnsemblePhase::Members => checkpoint.model_calls.iter().all(|evidence| {
                evidence.purpose == mcg_providers::ProxyModelCallPurpose::EnsembleMember
            }),
            ProviderProxyEnsemblePhase::Synthesis => {
                checkpoint
                    .model_calls
                    .iter()
                    .enumerate()
                    .all(|(index, evidence)| {
                        if index < member_call_count {
                            evidence.purpose == mcg_providers::ProxyModelCallPurpose::EnsembleMember
                        } else {
                            index == member_call_count
                                && evidence.purpose
                                    == mcg_providers::ProxyModelCallPurpose::EnsembleSynthesis
                                && evidence.target == checkpoint.synthesis_route.target
                        }
                    })
            }
            ProviderProxyEnsemblePhase::ManagedTools => checkpoint
                .model_calls
                .iter()
                .enumerate()
                .all(|(index, evidence)| match index.cmp(&member_call_count) {
                    std::cmp::Ordering::Less => {
                        evidence.purpose == mcg_providers::ProxyModelCallPurpose::EnsembleMember
                    }
                    std::cmp::Ordering::Equal => {
                        evidence.purpose == mcg_providers::ProxyModelCallPurpose::EnsembleSynthesis
                            && evidence.target == checkpoint.synthesis_route.target
                    }
                    std::cmp::Ordering::Greater => {
                        evidence.purpose
                            == mcg_providers::ProxyModelCallPurpose::ManagedContinuation
                            && evidence.target == checkpoint.synthesis_route.target
                    }
                }),
        };
    let synthesis_state_valid = match checkpoint.phase {
        ProviderProxyEnsemblePhase::Members => {
            checkpoint.model_request.is_none()
                && checkpoint.response.is_none()
                && synthesis_calls.is_empty()
                && checkpoint.tool_outcomes.is_empty()
                && checkpoint.next_tool_call_index == 0
                && checkpoint.pending_tool_request_id.is_none()
        }
        ProviderProxyEnsemblePhase::Synthesis => {
            pending == 0
                && successful.len() >= 2
                && checkpoint.model_request.as_ref().is_some_and(|request| {
                    request.model == checkpoint.synthesis_route.target.model_id
                })
                && match (&checkpoint.response, synthesis_calls.as_slice()) {
                    (None, []) => true,
                    (Some(response), [evidence]) => {
                        response.model == checkpoint.synthesis_route.target.model_id
                            && evidence.response_id == response.id
                            && evidence.usage == response.usage
                            && checkpoint.model_calls.last() == Some(*evidence)
                    }
                    _ => false,
                }
                && checkpoint.tool_outcomes.is_empty()
                && checkpoint.next_tool_call_index == 0
                && checkpoint.pending_tool_request_id.is_none()
        }
        ProviderProxyEnsemblePhase::ManagedTools => {
            pending == 0
                && successful.len() >= 2
                && checkpoint.model_request.as_ref().is_some_and(|request| {
                    request.model == checkpoint.synthesis_route.target.model_id
                })
                && checkpoint.response.as_ref().is_some_and(|response| {
                    response.model == checkpoint.synthesis_route.target.model_id
                        && checkpoint
                            .model_calls
                            .last()
                            .is_some_and(|evidence| evidence.response_id == response.id)
                })
                && synthesis_calls.len() == 1
                && checkpoint.next_tool_call_index <= tool_call_count
                && (checkpoint.pending_tool_request_id.is_none()
                    || checkpoint.next_tool_call_index < tool_call_count)
        }
    };
    if checkpoint.schema_version != PROVIDER_PROXY_ENSEMBLE_SCHEMA_VERSION
        || conversation.schema_version != PROXY_LEDGER_SCHEMA_VERSION
        || checkpoint.alias != "mcg:ensemble"
        || !valid_sha256(&checkpoint.caller_request_sha256)
        || !valid_sha256(&checkpoint.request_sha256)
        || checkpoint
            .context_sha256
            .as_deref()
            .is_some_and(|digest| !valid_sha256(digest))
        || checkpoint.context_items_injected > 4_096
        || checkpoint.context_estimated_tokens > 4_096
        || checkpoint.members.len() < 2
        || checkpoint.members.len() > 3
        || targets.len() != checkpoint.members.len()
        || checkpoint.members.iter().any(|member| {
            member.route.target.provider_id.trim().is_empty()
                || member.route.target.model_id.trim().is_empty()
                || member
                    .route
                    .estimated_cost
                    .amount_usd
                    .is_some_and(|cost| !cost.is_finite() || cost < 0.0)
        })
        || checkpoint
            .synthesis_route
            .target
            .provider_id
            .trim()
            .is_empty()
        || checkpoint.synthesis_route.target.model_id.trim().is_empty()
        || checkpoint
            .synthesis_route
            .estimated_cost
            .amount_usd
            .is_some_and(|cost| !cost.is_finite() || cost < 0.0)
        || successful.len() != member_call_count
        || !member_evidence_valid
        || evidence_ids.len() != checkpoint.model_calls.len()
        || checkpoint.model_calls.len() > 64
        || checkpoint.model_calls.iter().any(|evidence| {
            evidence.target.provider_id.trim().is_empty()
                || evidence.target.model_id.trim().is_empty()
                || evidence.response_id.trim().is_empty()
                || evidence.duration_ms == 0
                || evidence.duration_ms > 86_400_000
                || evidence
                    .actual_cost_usd
                    .is_some_and(|cost| !cost.is_finite() || cost < 0.0)
        })
        || !physical_sequence_valid
        || !synthesis_state_valid
        || checkpoint.tool_outcomes.len() > 128
        || request_ids.len() != checkpoint.tool_outcomes.len()
    {
        return Err(StorageError::InvalidData(
            "provider proxy ensemble checkpoint is invalid".into(),
        ));
    }
    Ok(())
}

fn validate_ensemble_checkpoint_context(
    store: &SqliteEventStore,
    checkpoint: &ProviderProxyEnsembleCheckpoint,
) -> Result<(), StorageError> {
    if checkpoint.conversation.task_id.is_some() && checkpoint.conversation.project_id.is_none() {
        return Err(StorageError::InvalidData(
            "provider proxy ensemble task has no project".into(),
        ));
    }
    if let Some(project_id) = checkpoint.conversation.project_id
        && store.project(project_id)?.is_none()
    {
        return Err(StorageError::ProjectNotFound(project_id));
    }
    if let Some(task_id) = checkpoint.conversation.task_id {
        let task = store
            .load_task(task_id)?
            .ok_or(StorageError::TaskNotFound(task_id))?;
        if task.project_id != checkpoint.conversation.project_id {
            return Err(StorageError::Conflict(
                "provider proxy ensemble task does not belong to its project".into(),
            ));
        }
    }
    Ok(())
}

fn validate_ensemble_checkpoint_advance(
    prior: &ProviderProxyEnsembleCheckpoint,
    next: &ProviderProxyEnsembleCheckpoint,
) -> Result<(), StorageError> {
    let immutable_match = prior.call_id == next.call_id
        && prior.conversation.id == next.conversation.id
        && prior.conversation.project_id == next.conversation.project_id
        && prior.conversation.task_id == next.conversation.task_id
        && prior.alias == next.alias
        && prior.interface == next.interface
        && prior.streamed == next.streamed
        && prior.caller_request_sha256 == next.caller_request_sha256
        && prior.request_sha256 == next.request_sha256
        && prior.context_sha256 == next.context_sha256
        && prior.context_items_injected == next.context_items_injected
        && prior.context_omitted_items == next.context_omitted_items
        && prior.context_estimated_tokens == next.context_estimated_tokens
        && prior.member_request == next.member_request
        && prior.synthesis_route == next.synthesis_route
        && prior
            .members
            .iter()
            .map(|member| &member.route)
            .eq(next.members.iter().map(|member| &member.route));
    let members_monotonic =
        prior
            .members
            .iter()
            .zip(&next.members)
            .all(|(prior, next)| match &prior.state {
                ProviderProxyEnsembleMemberState::Pending => true,
                _ => prior.state == next.state,
            });
    let position_monotonic = next.model_calls.len() > prior.model_calls.len()
        || next.next_tool_call_index >= prior.next_tool_call_index;
    if !immutable_match
        || !members_monotonic
        || next.phase < prior.phase
        || !next.model_calls.starts_with(&prior.model_calls)
        || !next.tool_outcomes.starts_with(&prior.tool_outcomes)
        || !position_monotonic
        || next.updated_at < prior.updated_at
    {
        return Err(StorageError::Conflict(
            "provider proxy ensemble checkpoint regressed or changed immutable authority".into(),
        ));
    }
    Ok(())
}

fn validate_ensemble_checkpoint_completion(
    checkpoint: &ProviderProxyEnsembleCheckpoint,
    call: &ProxyCallRecord,
) -> Result<(), StorageError> {
    let completed_response = checkpoint.response.as_ref();
    let last_call = checkpoint.model_calls.last();
    let completed_without_managed_calls = completed_response.is_some_and(|response| {
        response.output.iter().all(|item| {
            !matches!(item, OutputItem::ToolCall { name, .. } if name.starts_with("host::") || name.starts_with("mcg::"))
        })
    });
    let selected_target = last_call.map_or(&checkpoint.synthesis_route.target, |evidence| {
        &evidence.target
    });
    if call.id != checkpoint.call_id
        || call.conversation_id != checkpoint.conversation.id
        || call.interface != checkpoint.interface
        || call.alias != checkpoint.alias
        || call.provider_id != selected_target.provider_id
        || call.model_id != selected_target.model_id
        || call.request_sha256 != checkpoint.request_sha256
        || call.streamed != checkpoint.streamed
        || call.model_calls != checkpoint.model_calls
        || checkpoint.pending_tool_request_id.is_some()
        || call.outcome == ProxyCallOutcome::Completed
            && (!completed_without_managed_calls
                || completed_response.is_none_or(|response| response.id != call.response_id))
        || call.outcome != ProxyCallOutcome::Completed
            && last_call.is_some_and(|evidence| evidence.response_id != call.response_id)
    {
        return Err(StorageError::InvalidData(
            "provider proxy completion does not match its ensemble checkpoint".into(),
        ));
    }
    Ok(())
}

fn validate_provider_proxy_turn_checkpoint(
    checkpoint: &ProviderProxyTurnCheckpoint,
) -> Result<(), StorageError> {
    let conversation = &checkpoint.conversation;
    let tool_call_count = checkpoint
        .response
        .output
        .iter()
        .filter(|item| matches!(item, OutputItem::ToolCall { .. }))
        .count();
    let request_ids = checkpoint
        .tool_outcomes
        .iter()
        .map(|outcome| outcome.request_id)
        .collect::<BTreeSet<_>>();
    if checkpoint.schema_version != PROVIDER_PROXY_TURN_SCHEMA_VERSION
        || conversation.schema_version != PROXY_LEDGER_SCHEMA_VERSION
        || conversation.project_id.is_none()
        || conversation.task_id.is_none()
        || checkpoint.alias.trim().is_empty()
        || !valid_sha256(&checkpoint.caller_request_sha256)
        || !valid_sha256(&checkpoint.request_sha256)
        || checkpoint
            .context_sha256
            .as_deref()
            .is_some_and(|digest| !valid_sha256(digest))
        || checkpoint.model_calls.is_empty()
        || checkpoint.model_calls.len() > 32
        || checkpoint.tool_outcomes.len() > 128
        || request_ids.len() != checkpoint.tool_outcomes.len()
        || checkpoint.next_tool_call_index > tool_call_count
        || checkpoint.context_items_injected > 4_096
        || checkpoint.context_estimated_tokens > 4_096
        || !checkpoint_shadow_is_valid(checkpoint)
        || checkpoint.pending_tool_request_id.is_some()
            && checkpoint.next_tool_call_index >= tool_call_count
        || checkpoint.model_request.model != checkpoint.pinned_route.target.model_id
        || checkpoint.response.model != checkpoint.pinned_route.target.model_id
        || checkpoint.model_calls.iter().any(|evidence| {
            evidence.target != checkpoint.pinned_route.target
                || evidence.response_id.trim().is_empty()
                || evidence
                    .cost_usd
                    .is_some_and(|cost| !cost.is_finite() || cost < 0.0)
        })
        || checkpoint
            .model_calls
            .last()
            .is_none_or(|evidence| evidence.response_id != checkpoint.response.id)
    {
        return Err(StorageError::InvalidData(
            "provider proxy turn checkpoint is invalid".into(),
        ));
    }
    Ok(())
}

fn validate_checkpoint_task(
    store: &SqliteEventStore,
    checkpoint: &ProviderProxyTurnCheckpoint,
) -> Result<(), StorageError> {
    let project_id = checkpoint
        .conversation
        .project_id
        .ok_or_else(|| StorageError::InvalidData("managed turn has no project".into()))?;
    let task_id = checkpoint
        .conversation
        .task_id
        .ok_or_else(|| StorageError::InvalidData("managed turn has no task".into()))?;
    if store.project(project_id)?.is_none() {
        return Err(StorageError::ProjectNotFound(project_id));
    }
    let task = store
        .load_task(task_id)?
        .ok_or(StorageError::TaskNotFound(task_id))?;
    if task.project_id != Some(project_id) {
        return Err(StorageError::Conflict(
            "provider proxy checkpoint task does not belong to its project".into(),
        ));
    }
    Ok(())
}

fn validate_checkpoint_advance(
    prior: &ProviderProxyTurnCheckpoint,
    next: &ProviderProxyTurnCheckpoint,
) -> Result<(), StorageError> {
    let immutable_match = prior.call_id == next.call_id
        && prior.conversation.id == next.conversation.id
        && prior.conversation.project_id == next.conversation.project_id
        && prior.conversation.task_id == next.conversation.task_id
        && prior.alias == next.alias
        && prior.interface == next.interface
        && prior.streamed == next.streamed
        && prior.caller_request_sha256 == next.caller_request_sha256
        && prior.request_sha256 == next.request_sha256
        && prior.context_sha256 == next.context_sha256
        && prior.context_items_injected == next.context_items_injected
        && prior.context_omitted_items == next.context_omitted_items
        && prior.context_estimated_tokens == next.context_estimated_tokens
        && prior.shadow_requested == next.shadow_requested
        && prior.pinned_route == next.pinned_route;
    let model_prefix = next.model_calls.starts_with(&prior.model_calls);
    let outcome_prefix = next.tool_outcomes.starts_with(&prior.tool_outcomes);
    let position_monotonic = next.model_calls.len() > prior.model_calls.len()
        || next.next_tool_call_index >= prior.next_tool_call_index;
    if !immutable_match
        || !model_prefix
        || !outcome_prefix
        || !checkpoint_shadow_advance_is_valid(prior, next)
        || !position_monotonic
        || next.updated_at < prior.updated_at
    {
        return Err(StorageError::Conflict(
            "provider proxy turn checkpoint regressed or changed immutable authority".into(),
        ));
    }
    Ok(())
}

fn validate_checkpoint_completion(
    checkpoint: &ProviderProxyTurnCheckpoint,
    call: &ProxyCallRecord,
) -> Result<(), StorageError> {
    let mut expected_model_calls = checkpoint
        .model_calls
        .iter()
        .enumerate()
        .map(|(index, evidence)| ProxyModelCallEvidence {
            target: evidence.target.clone(),
            purpose: if index == 0 {
                ProxyModelCallPurpose::Primary
            } else {
                ProxyModelCallPurpose::ManagedContinuation
            },
            response_id: evidence.response_id.clone(),
            usage: evidence.usage.clone(),
            actual_cost_usd: evidence.cost_usd,
            duration_ms: evidence.duration_ms,
        })
        .collect::<Vec<_>>();
    if let Some(evidence) = checkpoint
        .shadow
        .as_ref()
        .and_then(|shadow| shadow.evidence.as_ref())
    {
        expected_model_calls.push(evidence.clone());
    }
    let mut usage = Usage::default();
    for evidence in &expected_model_calls {
        usage.input_tokens = usage
            .input_tokens
            .saturating_add(evidence.usage.input_tokens);
        usage.cached_input_tokens = usage
            .cached_input_tokens
            .saturating_add(evidence.usage.cached_input_tokens);
        usage.output_tokens = usage
            .output_tokens
            .saturating_add(evidence.usage.output_tokens);
        usage.reasoning_tokens = usage
            .reasoning_tokens
            .saturating_add(evidence.usage.reasoning_tokens);
        usage.total_tokens = usage
            .total_tokens
            .saturating_add(evidence.usage.total_tokens);
    }
    let actual_cost = expected_model_calls
        .iter()
        .try_fold(0.0, |total, evidence| {
            Some(total + evidence.actual_cost_usd?)
        });
    let mut estimated_cost = checkpoint
        .pinned_route
        .estimated_cost
        .amount_usd
        .map(|cost| {
            cost * f64::from(u32::try_from(checkpoint.model_calls.len()).unwrap_or(u32::MAX))
        });
    if let Some(shadow) = checkpoint.shadow.as_ref()
        && shadow.evidence.is_some()
    {
        estimated_cost = estimated_cost
            .zip(shadow.route.estimated_cost.amount_usd)
            .map(|(active, shadow)| active + shadow);
    }
    let duration_ms = expected_model_calls.iter().fold(0_u64, |total, evidence| {
        total.saturating_add(evidence.duration_ms)
    });
    let shadow_receipt = checkpoint
        .shadow
        .as_ref()
        .and_then(|shadow| shadow.receipt.as_ref());
    if call.id != checkpoint.call_id
        || call.conversation_id != checkpoint.conversation.id
        || call.interface != checkpoint.interface
        || call.alias != checkpoint.alias
        || call.provider_id != checkpoint.pinned_route.target.provider_id
        || call.model_id != checkpoint.pinned_route.target.model_id
        || call.response_id != checkpoint.response.id
        || call.request_sha256 != checkpoint.request_sha256
        || call.streamed != checkpoint.streamed
        || call.usage != usage
        || call.estimated_cost_usd != estimated_cost
        || call.actual_cost_usd != actual_cost
        || call.duration_ms != duration_ms
        || call.model_calls != expected_model_calls
        || call.shadow.as_ref() != shadow_receipt
        || checkpoint
            .shadow
            .as_ref()
            .is_some_and(|shadow| shadow.receipt.is_none())
        || checkpoint.pending_tool_request_id.is_some()
        || call.outcome == ProxyCallOutcome::Completed
            && checkpoint.response.output.iter().any(|item| {
                matches!(item, OutputItem::ToolCall { name, .. } if name.starts_with("host::") || name.starts_with("mcg::"))
            })
    {
        return Err(StorageError::InvalidData(
            "provider proxy completion does not match its managed turn checkpoint".into(),
        ));
    }
    Ok(())
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn validate_catalog(catalog: &ProviderModelCatalog) -> Result<(), StorageError> {
    if catalog
        .models
        .iter()
        .any(|model| model.id.trim().is_empty())
    {
        return Err(StorageError::InvalidData(
            "provider catalog contains an empty model id".into(),
        ));
    }
    Ok(())
}

fn provider_catalog_event_payload(
    catalog: &ProviderModelCatalog,
) -> Result<serde_json::Value, StorageError> {
    let models = catalog
        .models
        .iter()
        .map(|model| model_descriptor_source_ref(&catalog.connection_id, model))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(serde_json::json!({
        "connection_id": catalog.connection_id,
        "models": models,
        "discovered_at": catalog.discovered_at,
    }))
}

fn dispatch_purpose_name(purpose: ProxyModelCallPurpose) -> &'static str {
    match purpose {
        ProxyModelCallPurpose::Primary => "primary",
        ProxyModelCallPurpose::ManagedContinuation => "managed_continuation",
        ProxyModelCallPurpose::EnsembleMember => "ensemble_member",
        ProxyModelCallPurpose::EnsembleSynthesis => "ensemble_synthesis",
        ProxyModelCallPurpose::Shadow => "shadow",
    }
}

fn validate_provider_dispatch(dispatch: &ProviderDispatchRecord) -> Result<(), StorageError> {
    let identity_valid = !dispatch.client_request_id.is_empty()
        && dispatch.client_request_id.len() <= 512
        && dispatch.client_request_id.is_ascii()
        && !dispatch.target.provider_id.trim().is_empty()
        && dispatch.target.provider_id.len() <= 256
        && !dispatch.target.model_id.trim().is_empty()
        && dispatch.target.model_id.len() <= 512
        && dispatch.ordinal <= 1_024;
    let state_valid = match dispatch.state {
        ProviderDispatchState::Started => {
            dispatch.response_id.is_none()
                && dispatch.failure.is_none()
                && dispatch.completed_at.is_none()
        }
        ProviderDispatchState::Completed => {
            dispatch.response_id.as_ref().is_some_and(|response_id| {
                !response_id.trim().is_empty() && response_id.len() <= 1_024
            }) && dispatch.failure.is_none()
                && dispatch
                    .completed_at
                    .is_some_and(|completed_at| completed_at >= dispatch.started_at)
        }
        ProviderDispatchState::Failed => {
            dispatch.response_id.is_none()
                && dispatch.failure.is_some()
                && dispatch
                    .completed_at
                    .is_some_and(|completed_at| completed_at >= dispatch.started_at)
        }
    };
    if dispatch.schema_version != PROVIDER_DISPATCH_SCHEMA_VERSION
        || !identity_valid
        || !state_valid
    {
        return Err(StorageError::InvalidData(
            "provider dispatch record is invalid".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use mcg_agents::{
        ModelCallEvidence, PROVIDER_PROXY_ENSEMBLE_SCHEMA_VERSION,
        PROVIDER_PROXY_TURN_SCHEMA_VERSION, ProviderProxyEnsembleCheckpoint,
        ProviderProxyEnsembleMember, ProviderProxyEnsembleMemberState, ProviderProxyEnsemblePhase,
        ProviderProxyMemberFailure, ProviderProxyTurnCheckpoint,
    };
    use mcg_domain::{
        ProjectId, ProxyCallId, ProxyConversationId, TaskEnvelope, ToolExecutionId, ToolRequestId,
    };
    use mcg_projects::ProjectDiscoverer;
    use mcg_providers::{
        CONTRACT_VERSION, ContentPart, Message, MessageRole, ModelCapabilities, ModelDescriptor,
        ModelRequest, ModelResponse, OutputItem, PROXY_LEDGER_SCHEMA_VERSION, PlannedRoute,
        ProviderAuthMode, ProviderKind, ProviderModelRef, ProxyCallRecord, ProxyConversation,
        ProxyInterface, ProxyModelCallEvidence, ProxyModelCallPurpose, ToolDefinition, Usage,
    };
    use mcg_tools::{TOOL_SCHEMA_VERSION, ToolOutcome, ToolOutcomeStatus};
    use serde_json::Value;

    use super::*;

    fn started_dispatch(
        call_id: ProxyCallId,
        client_request_id: &str,
        ordinal: u32,
        started_at: DateTime<Utc>,
    ) -> ProviderDispatchRecord {
        ProviderDispatchRecord {
            schema_version: PROVIDER_DISPATCH_SCHEMA_VERSION,
            call_id,
            client_request_id: client_request_id.into(),
            target: ProviderModelRef {
                provider_id: "openai".into(),
                model_id: "fixture".into(),
            },
            purpose: ProxyModelCallPurpose::Primary,
            ordinal,
            state: ProviderDispatchState::Started,
            response_id: None,
            failure: None,
            started_at,
            completed_at: None,
        }
    }

    #[test]
    fn physical_provider_dispatch_is_durable_monotonic_and_non_replayable() {
        let temporary = tempfile::tempdir().expect("tempdir");
        let database = temporary.path().join("provider-dispatch.sqlite3");
        let call_id = ProxyCallId::new();
        let started_at = Utc::now();
        let started = started_dispatch(
            call_id,
            "mcg-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            0,
            started_at,
        );
        let mut store = SqliteEventStore::open(&database).expect("store");
        assert_eq!(
            store.begin_provider_dispatch(&started).expect("begin"),
            ProviderDispatchBegin::Begun
        );
        let mut exact_retry = started.clone();
        exact_retry.started_at += chrono::Duration::seconds(1);
        assert!(matches!(
            store
                .begin_provider_dispatch(&exact_retry)
                .expect("exact existing dispatch"),
            ProviderDispatchBegin::AlreadyExists(existing) if existing == started
        ));
        let mut rebound = started.clone();
        rebound.target.model_id = "other".into();
        assert!(matches!(
            store.begin_provider_dispatch(&rebound),
            Err(StorageError::Conflict(_))
        ));
        drop(store);

        let mut reopened = SqliteEventStore::open(&database).expect("reopen");
        assert_eq!(
            reopened
                .provider_dispatch(&started.client_request_id)
                .expect("durable dispatch"),
            Some(started.clone())
        );
        let completed_at = started_at + chrono::Duration::seconds(2);
        let completed = reopened
            .complete_provider_dispatch(&started.client_request_id, "resp_fixture", completed_at)
            .expect("complete dispatch");
        assert_eq!(completed.state, ProviderDispatchState::Completed);
        assert_eq!(completed.response_id.as_deref(), Some("resp_fixture"));
        assert_eq!(
            reopened
                .complete_provider_dispatch(
                    &started.client_request_id,
                    "resp_fixture",
                    completed_at,
                )
                .expect("idempotent completion"),
            completed
        );
        assert!(matches!(
            reopened.complete_provider_dispatch(
                &started.client_request_id,
                "resp_other",
                completed_at,
            ),
            Err(StorageError::Conflict(_))
        ));
        assert!(matches!(
            reopened
                .begin_provider_dispatch(&started)
                .expect("completed dispatch remains non-replayable"),
            ProviderDispatchBegin::AlreadyExists(existing) if existing == completed
        ));
    }

    #[test]
    fn physical_provider_dispatch_failure_is_terminal_and_non_replayable() {
        let temporary = tempfile::tempdir().expect("tempdir");
        let database = temporary.path().join("provider-dispatch-failure.sqlite3");
        let call_id = ProxyCallId::new();
        let started_at = Utc::now();
        let failed_start = started_dispatch(
            call_id,
            "mcg-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            1,
            started_at,
        );
        let completed_at = started_at + chrono::Duration::seconds(2);
        let mut reopened = SqliteEventStore::open(&database).expect("store");
        assert_eq!(
            reopened
                .begin_provider_dispatch(&failed_start)
                .expect("begin failed dispatch"),
            ProviderDispatchBegin::Begun
        );
        let failed = reopened
            .fail_provider_dispatch(
                &failed_start.client_request_id,
                ProviderDispatchFailure::Authentication,
                completed_at,
            )
            .expect("fail dispatch");
        assert_eq!(failed.state, ProviderDispatchState::Failed);
        assert_eq!(
            failed.failure,
            Some(ProviderDispatchFailure::Authentication)
        );
        assert_eq!(
            reopened
                .fail_provider_dispatch(
                    &failed_start.client_request_id,
                    ProviderDispatchFailure::Authentication,
                    completed_at,
                )
                .expect("idempotent failed dispatch"),
            failed
        );
        assert!(matches!(
            reopened.fail_provider_dispatch(
                &failed_start.client_request_id,
                ProviderDispatchFailure::Http,
                completed_at,
            ),
            Err(StorageError::Conflict(_))
        ));
        assert!(matches!(
            reopened.complete_provider_dispatch(
                &failed_start.client_request_id,
                "late-response",
                completed_at,
            ),
            Err(StorageError::Conflict(_))
        ));
        assert!(matches!(
            reopened
                .begin_provider_dispatch(&failed_start)
                .expect("failed dispatch remains non-replayable"),
            ProviderDispatchBegin::AlreadyExists(existing) if existing == failed
        ));
        assert_eq!(
            reopened
                .provider_dispatches_for_call(call_id)
                .expect("dispatch history"),
            vec![failed]
        );
    }

    #[test]
    fn connection_and_catalog_survive_restart_without_credentials() {
        let temporary = tempfile::tempdir().expect("tempdir");
        let database = temporary.path().join("provider.sqlite3");
        let now = Utc::now();
        let connection = ProviderConnection {
            contract_version: CONTRACT_VERSION,
            id: "gpu45".into(),
            display_name: "GPU45".into(),
            kind: ProviderKind::OpenAiCompatible,
            base_url: "http://192-168-50-189.nip.io/v1".into(),
            auth_mode: ProviderAuthMode::None,
            organization: None,
            project: None,
        };
        let catalog = ProviderModelCatalog {
            connection_id: connection.id.clone(),
            models: vec![ModelDescriptor {
                id: "fixture".into(),
                display_name: "Fixture".into(),
                owned_by: None,
                context_window: Some(32_768),
                capabilities: ModelCapabilities::default(),
                price: None,
                metadata: Value::Null,
            }],
            discovered_at: now,
        };
        let mut store = SqliteEventStore::open(&database).expect("store");
        store
            .record_provider_configuration(&connection, &catalog, now)
            .expect("configuration");
        drop(store);

        let mut reopened = SqliteEventStore::open(&database).expect("reopen");
        assert_eq!(
            reopened.provider_connections().expect("connections"),
            vec![connection]
        );
        assert_eq!(
            reopened.provider_catalog("gpu45").expect("catalog"),
            Some(catalog)
        );
        let events = reopened.events_after(0).expect("events");
        assert_eq!(events.len(), 2);
        let encoded = serde_json::to_string(&events).expect("event JSON");
        assert!(!encoded.contains("credential"));
        assert!(!encoded.contains("token"));

        let removed = reopened
            .record_provider_connection_removed("gpu45", Utc::now())
            .expect("remove provider");
        assert_eq!(removed.kind, EventKind::ProviderConnectionRemoved);
        assert!(
            reopened
                .provider_connections()
                .expect("connections")
                .is_empty()
        );
        assert!(
            reopened
                .provider_catalog("gpu45")
                .expect("catalog")
                .is_none()
        );
        drop(reopened);
        let reopened = SqliteEventStore::open(&database).expect("reopen after removal");
        assert!(
            reopened
                .provider_connections()
                .expect("connections")
                .is_empty()
        );
        assert_eq!(reopened.events_after(0).expect("events").len(), 3);
    }

    #[test]
    fn proxy_ledger_survives_restart_without_retaining_content() {
        let temporary = tempfile::tempdir().expect("tempdir");
        let database = temporary.path().join("proxy-ledger.sqlite3");
        let now = Utc::now();
        let conversation = ProxyConversation {
            schema_version: PROXY_LEDGER_SCHEMA_VERSION,
            id: ProxyConversationId::new(),
            project_id: None,
            task_id: None,
            created_at: now,
            updated_at: now,
        };
        let call = ProxyCallRecord {
            schema_version: PROXY_LEDGER_SCHEMA_VERSION,
            id: ProxyCallId::new(),
            conversation_id: conversation.id,
            interface: ProxyInterface::Responses,
            alias: "mcg:auto".into(),
            provider_id: "fixture".into(),
            model_id: "fixture-model".into(),
            response_id: "response-1".into(),
            request_sha256: "a".repeat(64),
            streamed: false,
            usage: Usage {
                input_tokens: 10,
                cached_input_tokens: 2,
                output_tokens: 5,
                reasoning_tokens: 1,
                total_tokens: 15,
            },
            estimated_cost_usd: Some(0.02),
            actual_cost_usd: Some(0.01),
            duration_ms: 25,
            model_calls: Vec::new(),
            shadow: None,
            outcome: mcg_providers::ProxyCallOutcome::Completed,
            completed_at: now,
        };
        let mut store = SqliteEventStore::open(&database).expect("store");
        store
            .record_proxy_call(&conversation, &call)
            .expect("first record");
        store
            .record_proxy_call(&conversation, &call)
            .expect("exact retry");
        drop(store);

        let reopened = SqliteEventStore::open(&database).expect("reopen");
        let view = reopened
            .proxy_conversation(conversation.id)
            .expect("read")
            .expect("conversation");
        assert_eq!(view.conversation, conversation);
        assert_eq!(view.calls, vec![call]);
        let summary = reopened.proxy_usage_summary().expect("summary");
        assert_eq!(summary.conversation_count, 1);
        assert_eq!(summary.call_count, 1);
        assert_eq!(summary.usage.total_tokens, 15);
        assert!((summary.known_actual_cost_usd - 0.01).abs() < f64::EPSILON);

        let persisted = std::fs::read(&database).expect("database bytes");
        assert!(
            !persisted
                .windows(b"private prompt".len())
                .any(|bytes| bytes == b"private prompt")
        );
        assert!(
            !persisted
                .windows(b"private output".len())
                .any(|bytes| bytes == b"private output")
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One end-to-end checkpoint/reopen/atomic-completion contract.
    fn managed_proxy_turn_survives_restart_and_completes_atomically() {
        let temporary = tempfile::tempdir().expect("tempdir");
        let database = temporary.path().join("managed-proxy.sqlite3");
        let project_root = tempfile::tempdir().expect("project root");
        std::fs::write(project_root.path().join("Cargo.toml"), "[workspace]")
            .expect("project manifest");
        let project_id = ProjectId::new();
        let project =
            ProjectDiscoverer::discover(project_root.path(), project_id).expect("project");
        let mut task = TaskEnvelope::new("durable managed provider turn");
        task.project_id = Some(project_id);
        let now = Utc::now();
        let request_id = ToolRequestId::new();
        let call_id = ProxyCallId::new();
        let conversation = ProxyConversation {
            schema_version: PROXY_LEDGER_SCHEMA_VERSION,
            id: ProxyConversationId::new(),
            project_id: Some(project_id),
            task_id: Some(task.id),
            created_at: now,
            updated_at: now,
        };
        let target = ProviderModelRef {
            provider_id: "fixture".into(),
            model_id: "fixture-model".into(),
        };
        let usage_1 = Usage {
            input_tokens: 10,
            output_tokens: 2,
            total_tokens: 12,
            ..Usage::default()
        };
        let model_request = ModelRequest {
            contract_version: CONTRACT_VERSION,
            model: target.model_id.clone(),
            client_request_id: None,
            instructions: None,
            messages: vec![Message {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "private managed prompt".into(),
                }],
                name: None,
            }],
            tools: vec![ToolDefinition {
                name: "host::fs.read".into(),
                description: None,
                parameters: serde_json::json!({"type":"object"}),
                strict: Some(true),
            }],
            tool_choice: None,
            structured_output: None,
            reasoning: None,
            temperature: None,
            top_p: None,
            max_output_tokens: Some(100),
            metadata: Value::Null,
        };
        let first_response = ModelResponse {
            contract_version: CONTRACT_VERSION,
            id: "managed-response-1".into(),
            model: target.model_id.clone(),
            status: Some("requires_action".into()),
            output: vec![OutputItem::ToolCall {
                id: "call-1".into(),
                name: "host::fs.read".into(),
                arguments: r#"{"path":"README.md","max_bytes":1024}"#.into(),
            }],
            usage: usage_1.clone(),
            provider_metadata: Value::Null,
        };
        let checkpoint = ProviderProxyTurnCheckpoint {
            schema_version: PROVIDER_PROXY_TURN_SCHEMA_VERSION,
            call_id,
            conversation: conversation.clone(),
            alias: "mcg:auto".into(),
            interface: ProxyInterface::Responses,
            streamed: false,
            caller_request_sha256: "c".repeat(64),
            request_sha256: "a".repeat(64),
            context_sha256: Some("b".repeat(64)),
            context_items_injected: 3,
            context_omitted_items: 1,
            context_estimated_tokens: 128,
            shadow_requested: false,
            shadow: None,
            pinned_route: PlannedRoute {
                target: target.clone(),
                estimated_cost: mcg_providers::CostEstimate {
                    amount_usd: Some(0.02),
                    provenance: mcg_providers::PriceProvenance::ProviderReported,
                },
            },
            model_request: model_request.clone(),
            response: first_response.clone(),
            model_calls: vec![ModelCallEvidence {
                target: target.clone(),
                response_id: first_response.id.clone(),
                usage: usage_1,
                cost_usd: Some(0.01),
                duration_ms: 5,
            }],
            tool_outcomes: Vec::new(),
            next_tool_call_index: 0,
            pending_tool_request_id: Some(request_id),
            updated_at: now,
        };
        {
            let mut store = SqliteEventStore::open(&database).expect("store");
            store.record_project_snapshot(&project).expect("project");
            store.create_task(&task).expect("task");
            store
                .save_provider_proxy_turn_checkpoint(&checkpoint)
                .expect("checkpoint");
        }

        let mut reopened = SqliteEventStore::open(&database).expect("reopen");
        assert_eq!(
            reopened
                .provider_proxy_turn_checkpoint(call_id)
                .expect("checkpoint after reopen"),
            Some(checkpoint.clone())
        );
        assert_eq!(
            reopened
                .provider_proxy_turn_checkpoints_for_task(task.id)
                .expect("task checkpoints"),
            vec![checkpoint.clone()]
        );

        let finished_at = now + chrono::Duration::seconds(1);
        let usage_2 = Usage {
            input_tokens: 14,
            output_tokens: 4,
            total_tokens: 18,
            ..Usage::default()
        };
        let final_response = ModelResponse {
            contract_version: CONTRACT_VERSION,
            id: "managed-response-2".into(),
            model: target.model_id.clone(),
            status: Some("completed".into()),
            output: vec![OutputItem::Message {
                role: MessageRole::Assistant,
                content: vec![ContentPart::Text {
                    text: "private managed output".into(),
                }],
            }],
            usage: usage_2.clone(),
            provider_metadata: Value::Null,
        };
        let mut completed_checkpoint = checkpoint.clone();
        completed_checkpoint.response = final_response.clone();
        completed_checkpoint.model_calls.push(ModelCallEvidence {
            target: target.clone(),
            response_id: final_response.id.clone(),
            usage: usage_2,
            cost_usd: Some(0.02),
            duration_ms: 6,
        });
        completed_checkpoint.tool_outcomes.push(ToolOutcome {
            schema_version: TOOL_SCHEMA_VERSION,
            execution_id: ToolExecutionId::new(),
            request_id,
            status: ToolOutcomeStatus::Succeeded,
            summary: "read complete".into(),
            exit_code: None,
            stdout: None,
            stderr: None,
            artifacts: Vec::new(),
            verification: Vec::new(),
            started_at: now,
            finished_at,
        });
        completed_checkpoint.next_tool_call_index = 0;
        completed_checkpoint.pending_tool_request_id = None;
        completed_checkpoint.updated_at = finished_at;
        reopened
            .save_provider_proxy_turn_checkpoint(&completed_checkpoint)
            .expect("advanced checkpoint");
        let call = ProxyCallRecord {
            schema_version: PROXY_LEDGER_SCHEMA_VERSION,
            id: call_id,
            conversation_id: conversation.id,
            interface: ProxyInterface::Responses,
            alias: "mcg:auto".into(),
            provider_id: target.provider_id,
            model_id: target.model_id,
            response_id: final_response.id,
            request_sha256: "a".repeat(64),
            streamed: false,
            usage: Usage {
                input_tokens: 24,
                output_tokens: 6,
                total_tokens: 30,
                ..Usage::default()
            },
            estimated_cost_usd: Some(0.04),
            actual_cost_usd: Some(0.01_f64 + 0.02),
            duration_ms: 11,
            model_calls: vec![
                mcg_providers::ProxyModelCallEvidence {
                    target: completed_checkpoint.model_calls[0].target.clone(),
                    purpose: mcg_providers::ProxyModelCallPurpose::Primary,
                    response_id: completed_checkpoint.model_calls[0].response_id.clone(),
                    usage: completed_checkpoint.model_calls[0].usage.clone(),
                    actual_cost_usd: completed_checkpoint.model_calls[0].cost_usd,
                    duration_ms: completed_checkpoint.model_calls[0].duration_ms,
                },
                mcg_providers::ProxyModelCallEvidence {
                    target: completed_checkpoint.model_calls[1].target.clone(),
                    purpose: mcg_providers::ProxyModelCallPurpose::ManagedContinuation,
                    response_id: completed_checkpoint.model_calls[1].response_id.clone(),
                    usage: completed_checkpoint.model_calls[1].usage.clone(),
                    actual_cost_usd: completed_checkpoint.model_calls[1].cost_usd,
                    duration_ms: completed_checkpoint.model_calls[1].duration_ms,
                },
            ],
            shadow: None,
            outcome: mcg_providers::ProxyCallOutcome::Completed,
            completed_at: finished_at,
        };
        let mut final_conversation = completed_checkpoint.conversation.clone();
        final_conversation.updated_at = finished_at;
        completed_checkpoint.conversation = final_conversation;
        reopened
            .save_provider_proxy_turn_checkpoint(&completed_checkpoint)
            .expect("completion timestamp checkpoint");
        reopened
            .complete_provider_proxy_turn(&completed_checkpoint, &call)
            .expect("atomic completion");
        assert!(
            reopened
                .provider_proxy_turn_checkpoint(call_id)
                .expect("cleared checkpoint")
                .is_none()
        );
        assert_eq!(
            reopened
                .proxy_conversation(conversation.id)
                .expect("conversation")
                .expect("completed conversation")
                .calls,
            vec![call.clone()]
        );
        reopened
            .complete_provider_proxy_turn(&completed_checkpoint, &call)
            .expect("exact completion retry");
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One full member/restart/synthesis/atomic-privacy contract.
    fn ensemble_checkpoint_survives_restart_and_settles_content_free() {
        let temporary = tempfile::tempdir().expect("tempdir");
        let database = temporary.path().join("ensemble-proxy.sqlite3");
        let now = Utc::now();
        let call_id = ProxyCallId::new();
        let conversation = ProxyConversation {
            schema_version: PROXY_LEDGER_SCHEMA_VERSION,
            id: ProxyConversationId::new(),
            project_id: None,
            task_id: None,
            created_at: now,
            updated_at: now,
        };
        let route = |provider: &str, model: &str| PlannedRoute {
            target: ProviderModelRef {
                provider_id: provider.into(),
                model_id: model.into(),
            },
            estimated_cost: mcg_providers::CostEstimate {
                amount_usd: Some(0.01),
                provenance: mcg_providers::PriceProvenance::ProviderReported,
            },
        };
        let member_request = ModelRequest {
            contract_version: CONTRACT_VERSION,
            model: "mcg:ensemble".into(),
            client_request_id: None,
            instructions: None,
            messages: vec![Message {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "private ensemble prompt".into(),
                }],
                name: None,
            }],
            tools: Vec::new(),
            tool_choice: None,
            structured_output: None,
            reasoning: None,
            temperature: None,
            top_p: None,
            max_output_tokens: Some(100),
            metadata: Value::Null,
        };
        let member_routes = [
            route("provider-a", "model-a"),
            route("provider-b", "model-b"),
            route("provider-c", "model-c"),
        ];
        let synthesis_route = route("provider-a", "model-a");
        let initial = ProviderProxyEnsembleCheckpoint {
            schema_version: PROVIDER_PROXY_ENSEMBLE_SCHEMA_VERSION,
            call_id,
            conversation: conversation.clone(),
            alias: "mcg:ensemble".into(),
            interface: ProxyInterface::Responses,
            streamed: true,
            caller_request_sha256: "c".repeat(64),
            request_sha256: "a".repeat(64),
            context_sha256: None,
            context_items_injected: 0,
            context_omitted_items: 0,
            context_estimated_tokens: 0,
            member_request: member_request.clone(),
            members: member_routes
                .iter()
                .cloned()
                .map(|route| ProviderProxyEnsembleMember {
                    route,
                    state: ProviderProxyEnsembleMemberState::Pending,
                })
                .collect(),
            synthesis_route: synthesis_route.clone(),
            phase: ProviderProxyEnsemblePhase::Members,
            model_request: None,
            response: None,
            model_calls: Vec::new(),
            tool_outcomes: Vec::new(),
            next_tool_call_index: 0,
            pending_tool_request_id: None,
            updated_at: now,
        };
        {
            let mut store = SqliteEventStore::open(&database).expect("store");
            store
                .save_provider_proxy_ensemble_checkpoint(&initial)
                .expect("initial checkpoint before spend");
        }
        let mut reopened = SqliteEventStore::open(&database).expect("reopen");
        assert_eq!(
            reopened
                .provider_proxy_ensemble_checkpoint(call_id)
                .expect("checkpoint after reopen"),
            Some(initial.clone())
        );
        let mut rebound = initial.clone();
        rebound.members[0].route = route("provider-z", "model-z");
        assert!(matches!(
            reopened.save_provider_proxy_ensemble_checkpoint(&rebound),
            Err(StorageError::Conflict(_))
        ));
        let mut premature_synthesis = initial.clone();
        premature_synthesis.phase = ProviderProxyEnsemblePhase::Synthesis;
        let mut premature_request = member_request.clone();
        premature_request.model = synthesis_route.target.model_id.clone();
        premature_synthesis.model_request = Some(premature_request);
        assert!(matches!(
            reopened.save_provider_proxy_ensemble_checkpoint(&premature_synthesis),
            Err(StorageError::InvalidData(_))
        ));

        let member_result = |route: &PlannedRoute, id: &str, private_text: &str, duration_ms| {
            let usage = Usage {
                input_tokens: 2,
                output_tokens: 1,
                total_tokens: 3,
                ..Usage::default()
            };
            let response = ModelResponse {
                contract_version: CONTRACT_VERSION,
                id: id.into(),
                model: route.target.model_id.clone(),
                status: Some("completed".into()),
                output: vec![OutputItem::Message {
                    role: MessageRole::Assistant,
                    content: vec![ContentPart::Text {
                        text: private_text.into(),
                    }],
                }],
                usage: usage.clone(),
                provider_metadata: Value::Null,
            };
            let evidence = ProxyModelCallEvidence {
                target: route.target.clone(),
                purpose: ProxyModelCallPurpose::EnsembleMember,
                response_id: id.into(),
                usage,
                actual_cost_usd: Some(0.01),
                duration_ms,
            };
            (response, evidence)
        };
        let (response_a, evidence_a) =
            member_result(&member_routes[0], "member-a", "private candidate A", 2);
        let (response_b, evidence_b) =
            member_result(&member_routes[1], "member-b", "private candidate B", 3);
        let mut synthesis = initial.clone();
        synthesis.members[0].state = ProviderProxyEnsembleMemberState::Succeeded {
            response: Box::new(response_a),
            evidence: evidence_a.clone(),
        };
        synthesis.members[1].state = ProviderProxyEnsembleMemberState::Succeeded {
            response: Box::new(response_b),
            evidence: evidence_b.clone(),
        };
        synthesis.members[2].state = ProviderProxyEnsembleMemberState::Failed {
            class: ProviderProxyMemberFailure::Timeout,
        };
        synthesis.model_calls = vec![evidence_a, evidence_b];
        synthesis.phase = ProviderProxyEnsemblePhase::Synthesis;
        let mut synthesis_request = member_request;
        synthesis_request.model = synthesis_route.target.model_id.clone();
        synthesis_request.messages = vec![Message {
            role: MessageRole::Developer,
            content: vec![ContentPart::Text {
                text: "private typed candidate bundle".into(),
            }],
            name: Some("mcg_ensemble_synthesis".into()),
        }];
        synthesis.model_request = Some(synthesis_request);
        synthesis.updated_at = now + chrono::Duration::milliseconds(1);
        reopened
            .save_provider_proxy_ensemble_checkpoint(&synthesis)
            .expect("member outcomes");

        let final_usage = Usage {
            input_tokens: 2,
            output_tokens: 1,
            total_tokens: 3,
            ..Usage::default()
        };
        let final_response = ModelResponse {
            contract_version: CONTRACT_VERSION,
            id: "synthesis-final".into(),
            model: synthesis_route.target.model_id.clone(),
            status: Some("completed".into()),
            output: vec![OutputItem::Message {
                role: MessageRole::Assistant,
                content: vec![ContentPart::Text {
                    text: "private synthesized answer".into(),
                }],
            }],
            usage: final_usage.clone(),
            provider_metadata: Value::Null,
        };
        let synthesis_evidence = ProxyModelCallEvidence {
            target: synthesis_route.target.clone(),
            purpose: ProxyModelCallPurpose::EnsembleSynthesis,
            response_id: final_response.id.clone(),
            usage: final_usage,
            actual_cost_usd: Some(0.01),
            duration_ms: 4,
        };
        let finished_at = now + chrono::Duration::seconds(1);
        let mut completed = synthesis;
        completed.response = Some(final_response.clone());
        completed.model_calls.push(synthesis_evidence);
        completed.updated_at = finished_at;
        completed.conversation.updated_at = finished_at;
        reopened
            .save_provider_proxy_ensemble_checkpoint(&completed)
            .expect("synthesis response");
        let call = ProxyCallRecord {
            schema_version: PROXY_LEDGER_SCHEMA_VERSION,
            id: call_id,
            conversation_id: conversation.id,
            interface: ProxyInterface::Responses,
            alias: "mcg:ensemble".into(),
            provider_id: synthesis_route.target.provider_id,
            model_id: synthesis_route.target.model_id,
            response_id: final_response.id,
            request_sha256: "a".repeat(64),
            streamed: true,
            usage: Usage {
                input_tokens: 6,
                output_tokens: 3,
                total_tokens: 9,
                ..Usage::default()
            },
            estimated_cost_usd: Some(0.04),
            actual_cost_usd: Some(0.01 + 0.01 + 0.01),
            duration_ms: 9,
            model_calls: completed.model_calls.clone(),
            shadow: None,
            outcome: ProxyCallOutcome::Completed,
            completed_at: finished_at,
        };
        reopened
            .complete_provider_proxy_ensemble(&completed, &call)
            .expect("atomic completion");
        assert!(
            reopened
                .provider_proxy_ensemble_checkpoint(call_id)
                .expect("cleared private checkpoint")
                .is_none()
        );
        let persisted = reopened.proxy_call(call_id).expect("ledger").expect("call");
        assert_eq!(persisted, call);
        let encoded = serde_json::to_string(&persisted).expect("ledger JSON");
        assert!(!encoded.contains("private candidate"));
        assert!(!encoded.contains("private synthesized"));
        reopened
            .complete_provider_proxy_ensemble(&completed, &call)
            .expect("exact terminal retry");
    }
}
