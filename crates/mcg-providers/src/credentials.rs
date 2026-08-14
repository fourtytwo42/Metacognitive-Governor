use std::{collections::BTreeMap, sync::Mutex};

use crate::{OAuthTokenSet, ProviderAuthMode, ProviderCredential, ProviderError};

const SERVICE_NAME: &str = "com.metacognitive-governor.provider";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderSecretSlot {
    Primary,
    OAuthIdToken,
    OAuthRefreshToken,
}

impl ProviderSecretSlot {
    const fn name(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::OAuthIdToken => "oauth-id-token",
            Self::OAuthRefreshToken => "oauth-refresh-token",
        }
    }
}

pub trait ProviderCredentialStore: Send + Sync {
    /// Loads one credential, returning `None` only when the slot is absent.
    ///
    /// # Errors
    ///
    /// Returns an authentication error when the platform store is unavailable.
    fn load(
        &self,
        connection_id: &str,
        slot: ProviderSecretSlot,
    ) -> Result<Option<ProviderCredential>, ProviderError>;

    /// Stores one credential in the platform-protected store.
    ///
    /// # Errors
    ///
    /// Returns an authentication error when the platform store rejects the write.
    fn save(
        &self,
        connection_id: &str,
        slot: ProviderSecretSlot,
        credential: &ProviderCredential,
    ) -> Result<(), ProviderError>;

    /// Deletes one credential. Missing slots are treated as already deleted.
    ///
    /// # Errors
    ///
    /// Returns an authentication error when the platform store rejects deletion.
    fn delete(&self, connection_id: &str, slot: ProviderSecretSlot) -> Result<(), ProviderError>;
}

#[derive(Debug, Default)]
pub struct OsProviderCredentialStore;

impl OsProviderCredentialStore {
    fn entry(
        connection_id: &str,
        slot: ProviderSecretSlot,
    ) -> Result<keyring::Entry, ProviderError> {
        validate_connection_id(connection_id)?;
        keyring::Entry::new(SERVICE_NAME, &format!("{connection_id}/{}", slot.name()))
            .map_err(credential_store_error)
    }
}

impl ProviderCredentialStore for OsProviderCredentialStore {
    fn load(
        &self,
        connection_id: &str,
        slot: ProviderSecretSlot,
    ) -> Result<Option<ProviderCredential>, ProviderError> {
        match Self::entry(connection_id, slot)?.get_password() {
            Ok(secret) => ProviderCredential::new(secret).map(Some),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(credential_store_error(error)),
        }
    }

    fn save(
        &self,
        connection_id: &str,
        slot: ProviderSecretSlot,
        credential: &ProviderCredential,
    ) -> Result<(), ProviderError> {
        Self::entry(connection_id, slot)?
            .set_password(credential.expose())
            .map_err(credential_store_error)
    }

    fn delete(&self, connection_id: &str, slot: ProviderSecretSlot) -> Result<(), ProviderError> {
        match Self::entry(connection_id, slot)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(credential_store_error(error)),
        }
    }
}

#[derive(Debug, Default)]
pub struct MemoryProviderCredentialStore {
    entries: Mutex<BTreeMap<String, String>>,
}

impl ProviderCredentialStore for MemoryProviderCredentialStore {
    fn load(
        &self,
        connection_id: &str,
        slot: ProviderSecretSlot,
    ) -> Result<Option<ProviderCredential>, ProviderError> {
        validate_connection_id(connection_id)?;
        self.entries
            .lock()
            .map_err(|_| credential_store_error("credential-store lock was poisoned"))?
            .get(&credential_key(connection_id, slot))
            .cloned()
            .map(ProviderCredential::new)
            .transpose()
    }

    fn save(
        &self,
        connection_id: &str,
        slot: ProviderSecretSlot,
        credential: &ProviderCredential,
    ) -> Result<(), ProviderError> {
        validate_connection_id(connection_id)?;
        self.entries
            .lock()
            .map_err(|_| credential_store_error("credential-store lock was poisoned"))?
            .insert(
                credential_key(connection_id, slot),
                credential.expose().to_owned(),
            );
        Ok(())
    }

    fn delete(&self, connection_id: &str, slot: ProviderSecretSlot) -> Result<(), ProviderError> {
        validate_connection_id(connection_id)?;
        self.entries
            .lock()
            .map_err(|_| credential_store_error("credential-store lock was poisoned"))?
            .remove(&credential_key(connection_id, slot));
        Ok(())
    }
}

/// Saves the complete credential shape required by an explicit authentication mode.
///
/// # Errors
///
/// Returns an error if the mode and supplied credential shape differ or a write fails.
pub fn save_provider_credentials<S: ProviderCredentialStore + ?Sized>(
    store: &S,
    connection_id: &str,
    auth_mode: ProviderAuthMode,
    primary: Option<&ProviderCredential>,
    oauth: Option<&OAuthTokenSet>,
) -> Result<(), ProviderError> {
    match (auth_mode, primary, oauth) {
        (ProviderAuthMode::None, None, None) => Ok(()),
        (ProviderAuthMode::ApiKey | ProviderAuthMode::BearerToken, Some(primary), None) => {
            store.save(connection_id, ProviderSecretSlot::Primary, primary)
        }
        (
            ProviderAuthMode::ChatGptBrowserOauth | ProviderAuthMode::ChatGptDeviceCode,
            None,
            Some(oauth),
        ) => {
            store.save(
                connection_id,
                ProviderSecretSlot::Primary,
                &oauth.access_token,
            )?;
            store.save(
                connection_id,
                ProviderSecretSlot::OAuthIdToken,
                &oauth.id_token,
            )?;
            store.save(
                connection_id,
                ProviderSecretSlot::OAuthRefreshToken,
                &oauth.refresh_token,
            )
        }
        _ => Err(ProviderError::Configuration(
            "credential shape does not match the explicit provider auth mode".into(),
        )),
    }
}

/// Removes every credential slot associated with one provider connection.
///
/// # Errors
///
/// Returns the first operating-system credential-store deletion error.
pub fn clear_provider_credentials<S: ProviderCredentialStore + ?Sized>(
    store: &S,
    connection_id: &str,
) -> Result<(), ProviderError> {
    for slot in [
        ProviderSecretSlot::Primary,
        ProviderSecretSlot::OAuthIdToken,
        ProviderSecretSlot::OAuthRefreshToken,
    ] {
        store.delete(connection_id, slot)?;
    }
    Ok(())
}

fn credential_key(connection_id: &str, slot: ProviderSecretSlot) -> String {
    format!("{connection_id}/{}", slot.name())
}

fn validate_connection_id(connection_id: &str) -> Result<(), ProviderError> {
    if connection_id.is_empty()
        || connection_id.len() > 128
        || !connection_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(ProviderError::Configuration(
            "provider connection id must use 1-128 ASCII letters, digits, '-' or '_'".into(),
        ));
    }
    Ok(())
}

fn credential_store_error(error: impl std::fmt::Display) -> ProviderError {
    ProviderError::Authentication(format!("operating-system credential store failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trips_without_serializable_credential_values() {
        let store = MemoryProviderCredentialStore::default();
        let credential = ProviderCredential::new("private-value").expect("credential");
        store
            .save("gpu45", ProviderSecretSlot::Primary, &credential)
            .expect("save");
        let loaded = store
            .load("gpu45", ProviderSecretSlot::Primary)
            .expect("load")
            .expect("present");
        assert_eq!(loaded.expose(), "private-value");
        assert!(!format!("{loaded:?}").contains("private-value"));
        store
            .delete("gpu45", ProviderSecretSlot::Primary)
            .expect("delete");
        assert!(
            store
                .load("gpu45", ProviderSecretSlot::Primary)
                .expect("load")
                .is_none()
        );
    }

    #[test]
    fn oauth_and_api_key_shapes_cannot_cross_billing_modes() {
        let store = MemoryProviderCredentialStore::default();
        let credential = ProviderCredential::new("api-key").expect("credential");
        assert!(
            save_provider_credentials(
                &store,
                "openai-oauth",
                ProviderAuthMode::ChatGptBrowserOauth,
                Some(&credential),
                None,
            )
            .is_err()
        );
        assert!(
            save_provider_credentials(&store, "openai-api", ProviderAuthMode::ApiKey, None, None,)
                .is_err()
        );
    }
}
