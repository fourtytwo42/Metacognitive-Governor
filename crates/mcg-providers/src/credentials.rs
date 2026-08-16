use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
};

use rand::RngCore;
use sha2::{Digest, Sha256};

use crate::{OAuthTokenSet, ProviderAuthMode, ProviderCredential, ProviderError};

const SERVICE_NAME: &str = "com.metacognitive-governor.provider";
const CHUNK_MANIFEST_PREFIX: &str = "mcg-keyring-chunks-v1";
const MAX_KEYRING_CHUNK_CHARS: usize = 512;
const MAX_KEYRING_CHUNKS: usize = 128;

static OS_CREDENTIAL_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

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
    fn entry_for_name(name: &str) -> Result<keyring::Entry, ProviderError> {
        keyring::Entry::new(SERVICE_NAME, name).map_err(credential_store_error)
    }

    fn base_name(connection_id: &str, slot: ProviderSecretSlot) -> Result<String, ProviderError> {
        validate_connection_id(connection_id)?;
        Ok(format!("{connection_id}/{}", slot.name()))
    }

    fn entry(
        connection_id: &str,
        slot: ProviderSecretSlot,
    ) -> Result<keyring::Entry, ProviderError> {
        Self::entry_for_name(&Self::base_name(connection_id, slot)?)
    }

    fn chunk_entry(
        connection_id: &str,
        slot: ProviderSecretSlot,
        generation: &str,
        index: usize,
    ) -> Result<keyring::Entry, ProviderError> {
        let base = Self::base_name(connection_id, slot)?;
        Self::entry_for_name(&format!("{base}/chunk/{generation}/{index}"))
    }

    fn load_unlocked(
        connection_id: &str,
        slot: ProviderSecretSlot,
    ) -> Result<Option<ProviderCredential>, ProviderError> {
        let stored = match Self::entry(connection_id, slot)?.get_password() {
            Ok(secret) => secret,
            Err(keyring::Error::NoEntry) => return Ok(None),
            Err(error) => return Err(credential_store_error(error)),
        };
        let Some(manifest) = ChunkManifest::parse(&stored)? else {
            return ProviderCredential::new(stored).map(Some);
        };
        let mut secret = String::new();
        for index in 0..manifest.count {
            let chunk = Self::chunk_entry(connection_id, slot, &manifest.generation, index)?
                .get_password()
                .map_err(|error| match error {
                    keyring::Error::NoEntry => credential_store_error(
                        "chunked credential is incomplete in the operating-system store",
                    ),
                    other => credential_store_error(other),
                })?;
            secret.push_str(&chunk);
        }
        if secret_digest(&secret) != manifest.digest {
            return Err(credential_store_error(
                "chunked credential failed its integrity check",
            ));
        }
        ProviderCredential::new(secret).map(Some)
    }

    fn delete_chunks_unlocked(
        connection_id: &str,
        slot: ProviderSecretSlot,
        manifest: &ChunkManifest,
    ) -> Result<(), ProviderError> {
        let mut first_error = None;
        for index in 0..manifest.count {
            match Self::chunk_entry(connection_id, slot, &manifest.generation, index)?
                .delete_credential()
            {
                Ok(()) | Err(keyring::Error::NoEntry) => {}
                Err(error) => {
                    first_error.get_or_insert_with(|| credential_store_error(error));
                }
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}

impl ProviderCredentialStore for OsProviderCredentialStore {
    fn load(
        &self,
        connection_id: &str,
        slot: ProviderSecretSlot,
    ) -> Result<Option<ProviderCredential>, ProviderError> {
        let _guard = os_credential_lock()?;
        Self::load_unlocked(connection_id, slot)
    }

    fn save(
        &self,
        connection_id: &str,
        slot: ProviderSecretSlot,
        credential: &ProviderCredential,
    ) -> Result<(), ProviderError> {
        let _guard = os_credential_lock()?;
        let previous = match Self::entry(connection_id, slot)?.get_password() {
            Ok(value) => ChunkManifest::parse(&value)?,
            Err(keyring::Error::NoEntry) => None,
            Err(error) => return Err(credential_store_error(error)),
        };
        let chunks = credential_chunks(credential.expose())?;
        let manifest = ChunkManifest::new(chunks.len(), credential.expose());
        for (index, chunk) in chunks.iter().enumerate() {
            if let Err(error) = Self::chunk_entry(connection_id, slot, &manifest.generation, index)?
                .set_password(chunk)
            {
                let _ = Self::delete_chunks_unlocked(connection_id, slot, &manifest);
                return Err(credential_store_error(error));
            }
        }
        if let Err(error) = Self::entry(connection_id, slot)?.set_password(&manifest.encode()) {
            let _ = Self::delete_chunks_unlocked(connection_id, slot, &manifest);
            return Err(credential_store_error(error));
        }
        if let Some(previous) = previous
            && previous.generation != manifest.generation
        {
            Self::delete_chunks_unlocked(connection_id, slot, &previous)?;
        }
        Ok(())
    }

    fn delete(&self, connection_id: &str, slot: ProviderSecretSlot) -> Result<(), ProviderError> {
        let _guard = os_credential_lock()?;
        let manifest = match Self::entry(connection_id, slot)?.get_password() {
            Ok(value) => ChunkManifest::parse(&value)?,
            Err(keyring::Error::NoEntry) => return Ok(()),
            Err(error) => return Err(credential_store_error(error)),
        };
        match Self::entry(connection_id, slot)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(error) => return Err(credential_store_error(error)),
        }
        if let Some(manifest) = manifest {
            Self::delete_chunks_unlocked(connection_id, slot, &manifest)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ChunkManifest {
    generation: String,
    count: usize,
    digest: String,
}

impl ChunkManifest {
    fn new(count: usize, secret: &str) -> Self {
        let mut generation = [0_u8; 16];
        rand::rng().fill_bytes(&mut generation);
        Self {
            generation: hex(&generation),
            count,
            digest: secret_digest(secret),
        }
    }

    fn encode(&self) -> String {
        format!(
            "{CHUNK_MANIFEST_PREFIX}|{}|{}|{}",
            self.generation, self.count, self.digest
        )
    }

    fn parse(value: &str) -> Result<Option<Self>, ProviderError> {
        if !value.starts_with(CHUNK_MANIFEST_PREFIX) {
            return Ok(None);
        }
        let parts = value.split('|').collect::<Vec<_>>();
        if parts.len() != 4
            || parts[0] != CHUNK_MANIFEST_PREFIX
            || parts[1].len() != 32
            || !parts[1].bytes().all(|byte| byte.is_ascii_hexdigit())
            || parts[3].len() != 64
            || !parts[3].bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(credential_store_error(
                "chunked credential manifest is malformed",
            ));
        }
        let count = parts[2]
            .parse::<usize>()
            .map_err(|_| credential_store_error("chunked credential count is malformed"))?;
        if !(1..=MAX_KEYRING_CHUNKS).contains(&count) {
            return Err(credential_store_error(
                "chunked credential count is outside the supported range",
            ));
        }
        Ok(Some(Self {
            generation: parts[1].to_ascii_lowercase(),
            count,
            digest: parts[3].to_ascii_lowercase(),
        }))
    }
}

fn credential_chunks(secret: &str) -> Result<Vec<String>, ProviderError> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut current_characters = 0;
    for character in secret.chars() {
        if current_characters == MAX_KEYRING_CHUNK_CHARS {
            chunks.push(std::mem::take(&mut current));
            current_characters = 0;
        }
        current.push(character);
        current_characters += 1;
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    if chunks.is_empty() || chunks.len() > MAX_KEYRING_CHUNKS {
        return Err(credential_store_error(
            "credential is outside the supported protected-store size range",
        ));
    }
    Ok(chunks)
}

fn secret_digest(secret: &str) -> String {
    hex(&Sha256::digest(secret.as_bytes()))
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn os_credential_lock() -> Result<std::sync::MutexGuard<'static, ()>, ProviderError> {
    OS_CREDENTIAL_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| credential_store_error("credential-store lock was poisoned"))
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
                credential.expose_to_host_executor().to_owned(),
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
        ) => replace_oauth_credentials(store, connection_id, oauth),
        _ => Err(ProviderError::Configuration(
            "credential shape does not match the explicit provider auth mode".into(),
        )),
    }
}

/// Loads the complete token set for an explicitly OAuth-billed connection.
///
/// # Errors
///
/// Returns an error for a non-OAuth mode, unavailable store, or incomplete token set.
pub fn load_oauth_credentials<S: ProviderCredentialStore + ?Sized>(
    store: &S,
    connection_id: &str,
    auth_mode: ProviderAuthMode,
) -> Result<OAuthTokenSet, ProviderError> {
    if !matches!(
        auth_mode,
        ProviderAuthMode::ChatGptBrowserOauth | ProviderAuthMode::ChatGptDeviceCode
    ) {
        return Err(ProviderError::Configuration(
            "OAuth credentials require an explicit ChatGPT OAuth auth mode".into(),
        ));
    }
    let required = |slot| {
        store.load(connection_id, slot)?.ok_or_else(|| {
            ProviderError::Authentication(format!(
                "provider connection {connection_id} has incomplete OAuth credentials"
            ))
        })
    };
    Ok(OAuthTokenSet {
        access_token: required(ProviderSecretSlot::Primary)?,
        id_token: required(ProviderSecretSlot::OAuthIdToken)?,
        refresh_token: required(ProviderSecretSlot::OAuthRefreshToken)?,
    })
}

fn replace_oauth_credentials<S: ProviderCredentialStore + ?Sized>(
    store: &S,
    connection_id: &str,
    oauth: &OAuthTokenSet,
) -> Result<(), ProviderError> {
    let replacements = [
        (ProviderSecretSlot::Primary, &oauth.access_token),
        (ProviderSecretSlot::OAuthIdToken, &oauth.id_token),
        (ProviderSecretSlot::OAuthRefreshToken, &oauth.refresh_token),
    ];
    let previous = replacements
        .iter()
        .map(|(slot, _)| store.load(connection_id, *slot))
        .collect::<Result<Vec<_>, _>>()?;
    for (index, (slot, credential)) in replacements.iter().enumerate() {
        if let Err(error) = store.save(connection_id, *slot, credential) {
            let rollback = replacements
                .iter()
                .zip(previous.iter())
                .take(index)
                .try_for_each(|((rollback_slot, _), old)| match old {
                    Some(old) => store.save(connection_id, *rollback_slot, old),
                    None => store.delete(connection_id, *rollback_slot),
                });
            return match rollback {
                Ok(()) => Err(error),
                Err(_) => Err(ProviderError::Authentication(
                    "OAuth credential rotation failed and protected-store rollback failed".into(),
                )),
            };
        }
    }
    Ok(())
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
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    #[derive(Default)]
    struct FailingRotationStore {
        inner: MemoryProviderCredentialStore,
        fail_id_token_write: AtomicBool,
    }

    impl ProviderCredentialStore for FailingRotationStore {
        fn load(
            &self,
            connection_id: &str,
            slot: ProviderSecretSlot,
        ) -> Result<Option<ProviderCredential>, ProviderError> {
            self.inner.load(connection_id, slot)
        }

        fn save(
            &self,
            connection_id: &str,
            slot: ProviderSecretSlot,
            credential: &ProviderCredential,
        ) -> Result<(), ProviderError> {
            if slot == ProviderSecretSlot::OAuthIdToken
                && self.fail_id_token_write.load(Ordering::SeqCst)
            {
                return Err(ProviderError::Authentication(
                    "injected protected-store failure".into(),
                ));
            }
            self.inner.save(connection_id, slot, credential)
        }

        fn delete(
            &self,
            connection_id: &str,
            slot: ProviderSecretSlot,
        ) -> Result<(), ProviderError> {
            self.inner.delete(connection_id, slot)
        }
    }

    fn oauth_tokens(prefix: &str) -> OAuthTokenSet {
        OAuthTokenSet {
            id_token: ProviderCredential::new(format!("{prefix}-id")).expect("id token"),
            access_token: ProviderCredential::new(format!("{prefix}-access"))
                .expect("access token"),
            refresh_token: ProviderCredential::new(format!("{prefix}-refresh"))
                .expect("refresh token"),
        }
    }

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

    #[test]
    fn failed_oauth_rotation_restores_the_complete_previous_token_set() {
        let store = FailingRotationStore::default();
        let original = oauth_tokens("original");
        save_provider_credentials(
            &store,
            "openai-subscription",
            ProviderAuthMode::ChatGptDeviceCode,
            None,
            Some(&original),
        )
        .expect("initial token set");
        store.fail_id_token_write.store(true, Ordering::SeqCst);

        let error = save_provider_credentials(
            &store,
            "openai-subscription",
            ProviderAuthMode::ChatGptDeviceCode,
            None,
            Some(&oauth_tokens("replacement")),
        )
        .expect_err("rotation should fail");
        assert!(error.to_string().contains("injected"));
        let restored = load_oauth_credentials(
            &store,
            "openai-subscription",
            ProviderAuthMode::ChatGptDeviceCode,
        )
        .expect("restored tokens");
        assert_eq!(
            restored.access_token.expose(),
            original.access_token.expose()
        );
        assert_eq!(restored.id_token.expose(), original.id_token.expose());
        assert_eq!(
            restored.refresh_token.expose(),
            original.refresh_token.expose()
        );
    }

    #[test]
    fn oversized_credentials_split_on_character_boundaries_with_integrity() {
        let secret = format!("{}{}", "a".repeat(3_200), "\u{1f980}".repeat(700));
        let chunks = credential_chunks(&secret).expect("chunks");
        assert!(chunks.len() > 1);
        assert!(
            chunks
                .iter()
                .all(|chunk| chunk.chars().count() <= MAX_KEYRING_CHUNK_CHARS)
        );
        let reassembled = chunks.concat();
        assert_eq!(reassembled, secret);

        let manifest = ChunkManifest::new(chunks.len(), &secret);
        let parsed = ChunkManifest::parse(&manifest.encode())
            .expect("valid manifest")
            .expect("chunked manifest");
        assert_eq!(parsed.count, chunks.len());
        assert_eq!(parsed.digest, secret_digest(&reassembled));
        assert!(
            ChunkManifest::parse("ordinary-legacy-secret")
                .expect("legacy value")
                .is_none()
        );
        assert!(ChunkManifest::parse("mcg-keyring-chunks-v1|bad|2|bad").is_err());
    }

    #[test]
    fn oversized_os_credential_round_trip_is_opt_in() {
        if std::env::var("MCG_LIVE_OS_CREDENTIAL_STORE").as_deref() != Ok("1") {
            return;
        }
        let store = OsProviderCredentialStore;
        let connection_id = format!("mcg-keyring-live-{}", std::process::id());
        let slot = ProviderSecretSlot::Primary;
        let _ = store.delete(&connection_id, slot);
        let oversized = ProviderCredential::new("live-secret-".repeat(600)).expect("credential");
        store
            .save(&connection_id, slot, &oversized)
            .expect("chunked OS credential save");
        let loaded = store
            .load(&connection_id, slot)
            .expect("OS credential load")
            .expect("stored credential");
        assert_eq!(loaded.expose(), oversized.expose());

        let replacement = ProviderCredential::new("short-replacement").expect("replacement");
        store
            .save(&connection_id, slot, &replacement)
            .expect("atomic replacement");
        assert_eq!(
            store
                .load(&connection_id, slot)
                .expect("replacement load")
                .expect("replacement stored")
                .expose(),
            replacement.expose()
        );
        store.delete(&connection_id, slot).expect("cleanup");
        assert!(
            store
                .load(&connection_id, slot)
                .expect("deleted load")
                .is_none()
        );
    }
}
