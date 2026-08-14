use chrono::{DateTime, Utc};
use mcg_events::{EventEnvelope, EventKind};
use mcg_mcp::{McpConnection, McpToolCatalog};
use rusqlite::{OptionalExtension, params};

use crate::{SqliteEventStore, StorageError};

impl SqliteEventStore {
    /// Atomically records secret-free MCP metadata, its discovered catalog, and
    /// both global audit events.
    ///
    /// # Errors
    /// Returns validation, serialization, or `SQLite` failures without partial writes.
    pub fn record_mcp_configuration(
        &mut self,
        connection: &McpConnection,
        catalog: &McpToolCatalog,
        at: DateTime<Utc>,
    ) -> Result<Vec<EventEnvelope>, StorageError> {
        connection
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        if catalog.connection_id != connection.id {
            return Err(StorageError::InvalidData(
                "MCP catalog identity does not match its connection".into(),
            ));
        }
        catalog
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO mcp_connections(connection_id, connection_json, updated_at)
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
            "INSERT INTO mcp_tool_catalogs(connection_id, catalog_json, discovered_at, expires_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(connection_id) DO UPDATE SET
                catalog_json = excluded.catalog_json,
                discovered_at = excluded.discovered_at,
                expires_at = excluded.expires_at",
            params![
                catalog.connection_id,
                serde_json::to_string(catalog)?,
                catalog.discovered_at.to_rfc3339(),
                catalog.expires_at.to_rfc3339(),
            ],
        )?;
        let connection_event = Self::insert_event(
            &transaction,
            &EventEnvelope::new(
                EventKind::McpConnectionRecorded,
                None,
                serde_json::json!({"connection": connection}),
            ),
        )?;
        let catalog_event = Self::insert_event(
            &transaction,
            &EventEnvelope::new(
                EventKind::McpCatalogDiscovered,
                None,
                serde_json::json!({"catalog": catalog}),
            ),
        )?;
        transaction.commit()?;
        Ok(vec![connection_event, catalog_event])
    }

    /// Lists all persisted secret-free MCP connections in stable identity order.
    ///
    /// # Errors
    /// Returns decoding or `SQLite` failures.
    pub fn mcp_connections(&self) -> Result<Vec<McpConnection>, StorageError> {
        self.read_all_json("SELECT connection_json FROM mcp_connections ORDER BY connection_id")
    }

    /// Loads one persisted MCP connection.
    ///
    /// # Errors
    /// Returns decoding or `SQLite` failures.
    pub fn mcp_connection(
        &self,
        connection_id: &str,
    ) -> Result<Option<McpConnection>, StorageError> {
        self.read_optional_json(
            "SELECT connection_json FROM mcp_connections WHERE connection_id = ?1",
            connection_id,
        )
    }

    /// Replaces one known connection's discovered catalog and appends its event.
    ///
    /// # Errors
    /// Returns not-found, validation, serialization, or `SQLite` failures.
    pub fn record_mcp_catalog(
        &mut self,
        catalog: &McpToolCatalog,
    ) -> Result<EventEnvelope, StorageError> {
        if self.mcp_connection(&catalog.connection_id)?.is_none() {
            return Err(StorageError::InvalidData(format!(
                "MCP connection {} was not found",
                catalog.connection_id
            )));
        }
        catalog
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO mcp_tool_catalogs(connection_id, catalog_json, discovered_at, expires_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(connection_id) DO UPDATE SET
                catalog_json = excluded.catalog_json,
                discovered_at = excluded.discovered_at,
                expires_at = excluded.expires_at",
            params![
                catalog.connection_id,
                serde_json::to_string(catalog)?,
                catalog.discovered_at.to_rfc3339(),
                catalog.expires_at.to_rfc3339(),
            ],
        )?;
        let event = Self::insert_event(
            &transaction,
            &EventEnvelope::new(
                EventKind::McpCatalogDiscovered,
                None,
                serde_json::json!({"catalog": catalog}),
            ),
        )?;
        transaction.commit()?;
        Ok(event)
    }

    /// Loads the latest tool catalog for one MCP connection.
    ///
    /// # Errors
    /// Returns decoding or `SQLite` failures.
    pub fn mcp_catalog(&self, connection_id: &str) -> Result<Option<McpToolCatalog>, StorageError> {
        let json = self
            .connection
            .query_row(
                "SELECT catalog_json FROM mcp_tool_catalogs WHERE connection_id = ?1",
                [connection_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        json.map(|value| serde_json::from_str(&value).map_err(StorageError::from))
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeDelta;
    use mcg_mcp::{
        MCP_CONNECTION_CONTRACT_VERSION, MCP_PROTOCOL_VERSION, McpAuthMode, McpProtocolMode,
        McpServerInfo, McpToolDescriptor, McpTransport,
    };
    use serde_json::{Value, json};

    use super::*;

    #[test]
    fn connection_and_catalog_survive_restart_without_credentials() {
        let temporary = tempfile::tempdir().expect("tempdir");
        let database = temporary.path().join("mcp.sqlite3");
        let now = Utc::now();
        let connection = McpConnection {
            contract_version: MCP_CONNECTION_CONTRACT_VERSION,
            id: "local_mcg".into(),
            display_name: "Local MCG".into(),
            transport: McpTransport::StreamableHttp {
                url: "http://127.0.0.1:7777/mcp".into(),
            },
            auth_mode: McpAuthMode::BearerToken,
            protocol_mode: McpProtocolMode::Auto,
            enabled: true,
        };
        let catalog = McpToolCatalog {
            connection_id: connection.id.clone(),
            protocol_version: MCP_PROTOCOL_VERSION.into(),
            server_info: McpServerInfo {
                name: "fixture".into(),
                version: Some("1".into()),
            },
            capabilities: json!({"tools": {}}),
            tools: vec![McpToolDescriptor {
                name: "read".into(),
                title: None,
                description: None,
                input_schema: json!({"type": "object"}),
                output_schema: None,
                annotations: Value::Null,
            }],
            discovered_at: now,
            expires_at: now + TimeDelta::minutes(5),
        };
        let mut store = SqliteEventStore::open(&database).expect("store");
        store
            .record_mcp_configuration(&connection, &catalog, now)
            .expect("record");
        drop(store);

        let reopened = SqliteEventStore::open(&database).expect("reopen");
        assert_eq!(
            reopened.mcp_connections().expect("connections"),
            vec![connection]
        );
        assert_eq!(
            reopened.mcp_catalog("local_mcg").expect("catalog"),
            Some(catalog)
        );
        let events = reopened.events_after(0).expect("events");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, EventKind::McpConnectionRecorded);
        assert_eq!(events[1].kind, EventKind::McpCatalogDiscovered);
        let encoded = serde_json::to_string(&events).expect("JSON");
        assert!(!encoded.contains("private-token"));
        assert!(!encoded.contains("authorization"));
    }
}
