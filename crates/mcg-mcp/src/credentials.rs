use std::{collections::BTreeMap, sync::Mutex};

use crate::{McpCredential, McpError, valid_identity};

const SERVICE_NAME: &str = "com.metacognitive-governor.mcp";

pub trait McpCredentialStore: Send + Sync {
    /// Loads the bearer credential, returning `None` only when it is absent.
    ///
    /// # Errors
    /// Returns authentication failure when the protected store is unavailable.
    fn load(&self, connection_id: &str) -> Result<Option<McpCredential>, McpError>;

    /// Saves one bearer credential in the protected store.
    ///
    /// # Errors
    /// Returns authentication failure when the protected store rejects the write.
    fn save(&self, connection_id: &str, credential: &McpCredential) -> Result<(), McpError>;

    /// Deletes one bearer credential; absence is already-success.
    ///
    /// # Errors
    /// Returns authentication failure when the protected store rejects deletion.
    fn delete(&self, connection_id: &str) -> Result<(), McpError>;
}

#[derive(Debug, Default)]
pub struct OsMcpCredentialStore;

impl OsMcpCredentialStore {
    fn entry(connection_id: &str) -> Result<keyring::Entry, McpError> {
        validate_id(connection_id)?;
        keyring::Entry::new(SERVICE_NAME, connection_id).map_err(store_error)
    }
}

impl McpCredentialStore for OsMcpCredentialStore {
    fn load(&self, connection_id: &str) -> Result<Option<McpCredential>, McpError> {
        match Self::entry(connection_id)?.get_password() {
            Ok(value) => McpCredential::new(value).map(Some),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(store_error(error)),
        }
    }

    fn save(&self, connection_id: &str, credential: &McpCredential) -> Result<(), McpError> {
        Self::entry(connection_id)?
            .set_password(credential.expose())
            .map_err(store_error)
    }

    fn delete(&self, connection_id: &str) -> Result<(), McpError> {
        match Self::entry(connection_id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(store_error(error)),
        }
    }
}

#[derive(Debug, Default)]
pub struct MemoryMcpCredentialStore {
    entries: Mutex<BTreeMap<String, String>>,
}

impl McpCredentialStore for MemoryMcpCredentialStore {
    fn load(&self, connection_id: &str) -> Result<Option<McpCredential>, McpError> {
        validate_id(connection_id)?;
        self.entries
            .lock()
            .map_err(|_| store_error("credential-store lock was poisoned"))?
            .get(connection_id)
            .cloned()
            .map(McpCredential::new)
            .transpose()
    }

    fn save(&self, connection_id: &str, credential: &McpCredential) -> Result<(), McpError> {
        validate_id(connection_id)?;
        self.entries
            .lock()
            .map_err(|_| store_error("credential-store lock was poisoned"))?
            .insert(connection_id.to_owned(), credential.expose().to_owned());
        Ok(())
    }

    fn delete(&self, connection_id: &str) -> Result<(), McpError> {
        validate_id(connection_id)?;
        self.entries
            .lock()
            .map_err(|_| store_error("credential-store lock was poisoned"))?
            .remove(connection_id);
        Ok(())
    }
}

fn validate_id(connection_id: &str) -> Result<(), McpError> {
    if valid_identity(connection_id) {
        Ok(())
    } else {
        Err(McpError::Configuration(
            "MCP connection id must use 1-128 ASCII letters, digits, '-' or '_'".into(),
        ))
    }
}

fn store_error(error: impl std::fmt::Display) -> McpError {
    McpError::Authentication(format!("operating-system credential store failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trips_redacting_credentials() {
        let store = MemoryMcpCredentialStore::default();
        let credential = McpCredential::new("fixture-secret").expect("credential");
        store.save("fixture", &credential).expect("save");
        let loaded = store.load("fixture").expect("load").expect("present");
        assert_eq!(format!("{loaded:?}"), "McpCredential([REDACTED])");
        store.delete("fixture").expect("delete");
        assert!(store.load("fixture").expect("load after delete").is_none());
    }
}
