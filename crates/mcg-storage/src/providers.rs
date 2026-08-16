use chrono::{DateTime, Utc};
use mcg_domain::ProxyConversationId;
use mcg_events::{EventEnvelope, EventKind};
use mcg_providers::{
    PROXY_LEDGER_SCHEMA_VERSION, ProviderAuthMode, ProviderConnection, ProviderModelCatalog,
    ProxyCallRecord, ProxyConversation, ProxyConversationView, ProxyUsageSummary, Usage,
};
use rusqlite::{OptionalExtension, params};

use crate::{SqliteEventStore, StorageError};

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
            if prior.project_id != conversation.project_id || prior.task_id != conversation.task_id
            {
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
        let calls: Vec<ProxyCallRecord> = self.read_all_json(
            "SELECT call_json FROM provider_proxy_calls ORDER BY completed_at, call_id",
        )?;
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
        let catalog_event = Self::insert_event(
            &transaction,
            &EventEnvelope::new(
                EventKind::ProviderCatalogDiscovered,
                None,
                serde_json::json!({"catalog": catalog}),
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
        let event = Self::insert_event(
            &transaction,
            &EventEnvelope::new(
                EventKind::ProviderCatalogDiscovered,
                None,
                serde_json::json!({"catalog": catalog}),
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

fn validate_proxy_record(
    conversation: &ProxyConversation,
    call: &ProxyCallRecord,
) -> Result<(), StorageError> {
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
    {
        return Err(StorageError::InvalidData(
            "provider proxy ledger record is invalid".into(),
        ));
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use mcg_domain::{ProxyCallId, ProxyConversationId};
    use mcg_providers::{
        CONTRACT_VERSION, ModelCapabilities, ModelDescriptor, PROXY_LEDGER_SCHEMA_VERSION,
        ProviderAuthMode, ProviderKind, ProxyCallRecord, ProxyConversation, ProxyInterface, Usage,
    };
    use serde_json::Value;

    use super::*;

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
}
