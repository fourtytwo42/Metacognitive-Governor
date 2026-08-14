use chrono::{DateTime, Utc};
use mcg_events::{EventEnvelope, EventKind};
use mcg_providers::{ProviderAuthMode, ProviderConnection, ProviderModelCatalog};
use rusqlite::{OptionalExtension, params};

use crate::{SqliteEventStore, StorageError};

impl SqliteEventStore {
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
    use mcg_providers::{
        CONTRACT_VERSION, ModelCapabilities, ModelDescriptor, ProviderAuthMode, ProviderKind,
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

        let reopened = SqliteEventStore::open(&database).expect("reopen");
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
    }
}
