use std::collections::BTreeSet;

use argon2::{Algorithm, Argon2, Params, Version};
use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use mcg_mcp::McpCredential;
use mcg_providers::{OAuthTokenSet, ProviderAuthMode, ProviderCredential};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::StorageError;

pub const BACKUP_SECRET_ENCRYPTION_SCHEMA_VERSION: u16 = 1;
pub const BACKUP_SECRET_ENTRY: &str = "secrets/credentials.v1.enc";
const SECRET_PAYLOAD_SCHEMA_VERSION: u16 = 1;
const ENCRYPTION_ALGORITHM: &str = "xchacha20-poly1305";
const KDF_ALGORITHM: &str = "argon2id";
const ARGON2_VERSION: u32 = 19;
const ARGON2_MEMORY_KIB: u32 = 64 * 1024;
const ARGON2_ITERATIONS: u32 = 3;
const ARGON2_PARALLELISM: u32 = 1;
const SALT_BYTES: usize = 16;
const NONCE_BYTES: usize = 24;
const KEY_BYTES: usize = 32;
const MIN_PASSPHRASE_BYTES: usize = 12;
const MAX_PASSPHRASE_BYTES: usize = 1024;
const MAX_SECRET_RECORDS: usize = 10_000;
const MAX_SECRET_PLAINTEXT_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MAX_SECRET_CIPHERTEXT_BYTES: usize = MAX_SECRET_PLAINTEXT_BYTES + 16;
const MAX_AAD_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupSecretEncryptionMetadata {
    pub schema_version: u16,
    pub entry_path: String,
    pub algorithm: String,
    pub kdf: String,
    pub argon2_version: u32,
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    pub salt_base64: String,
    pub nonce_base64: String,
}

impl std::fmt::Debug for BackupSecretEncryptionMetadata {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BackupSecretEncryptionMetadata")
            .field("schema_version", &self.schema_version)
            .field("entry_path", &self.entry_path)
            .field("algorithm", &self.algorithm)
            .field("kdf", &self.kdf)
            .field("argon2_version", &self.argon2_version)
            .field("memory_kib", &self.memory_kib)
            .field("iterations", &self.iterations)
            .field("parallelism", &self.parallelism)
            .field("salt_base64", &"[PUBLIC RANDOM SALT]")
            .field("nonce_base64", &"[PUBLIC RANDOM NONCE]")
            .finish()
    }
}

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct BackupPassphrase(String);

impl BackupPassphrase {
    /// Creates one bounded passphrase for explicit encrypted backup operations.
    ///
    /// # Errors
    /// Returns invalid data when the passphrase is too short or too large.
    pub fn new(value: impl Into<String>) -> Result<Self, StorageError> {
        let mut value = value.into();
        if !(MIN_PASSPHRASE_BYTES..=MAX_PASSPHRASE_BYTES).contains(&value.len()) {
            value.zeroize();
            return Err(StorageError::InvalidData(format!(
                "encrypted backup passphrase must contain {MIN_PASSPHRASE_BYTES}-{MAX_PASSPHRASE_BYTES} UTF-8 bytes"
            )));
        }
        Ok(Self(value))
    }

    fn expose(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl std::fmt::Debug for BackupPassphrase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("BackupPassphrase([REDACTED])")
    }
}

pub enum BackupProviderSecret {
    None {
        connection_id: String,
    },
    Primary {
        connection_id: String,
        auth_mode: ProviderAuthMode,
        credential: ProviderCredential,
    },
    OAuth {
        connection_id: String,
        auth_mode: ProviderAuthMode,
        tokens: OAuthTokenSet,
    },
}

impl BackupProviderSecret {
    fn connection_id(&self) -> &str {
        match self {
            Self::None { connection_id }
            | Self::Primary { connection_id, .. }
            | Self::OAuth { connection_id, .. } => connection_id,
        }
    }
}

pub enum BackupMcpSecret {
    None {
        connection_id: String,
    },
    Bearer {
        connection_id: String,
        credential: McpCredential,
    },
}

impl BackupMcpSecret {
    fn connection_id(&self) -> &str {
        match self {
            Self::None { connection_id } | Self::Bearer { connection_id, .. } => connection_id,
        }
    }
}

pub struct BackupSecretSnapshot {
    pub providers: Vec<BackupProviderSecret>,
    pub mcp_connections: Vec<BackupMcpSecret>,
}

impl BackupSecretSnapshot {
    /// Validates and canonicalizes one explicit protected-store snapshot.
    ///
    /// # Errors
    /// Returns invalid data for duplicate/invalid identities or credential shapes.
    pub fn new(
        mut providers: Vec<BackupProviderSecret>,
        mut mcp_connections: Vec<BackupMcpSecret>,
    ) -> Result<Self, StorageError> {
        if providers.len().saturating_add(mcp_connections.len()) > MAX_SECRET_RECORDS {
            return Err(invalid_secret_backup(
                "credential record count exceeds its bound",
            ));
        }
        providers.sort_by(|left, right| left.connection_id().cmp(right.connection_id()));
        mcp_connections.sort_by(|left, right| left.connection_id().cmp(right.connection_id()));
        validate_unique_identities(providers.iter().map(BackupProviderSecret::connection_id))?;
        validate_unique_identities(mcp_connections.iter().map(BackupMcpSecret::connection_id))?;
        for provider in &providers {
            validate_identity(provider.connection_id())?;
            match provider {
                BackupProviderSecret::None { .. }
                | BackupProviderSecret::Primary {
                    auth_mode: ProviderAuthMode::ApiKey | ProviderAuthMode::BearerToken,
                    ..
                }
                | BackupProviderSecret::OAuth {
                    auth_mode:
                        ProviderAuthMode::ChatGptBrowserOauth | ProviderAuthMode::ChatGptDeviceCode,
                    ..
                } => {}
                _ => {
                    return Err(invalid_secret_backup(
                        "provider authentication mode does not match its credential shape",
                    ));
                }
            }
        }
        for connection in &mcp_connections {
            validate_identity(connection.connection_id())?;
        }
        Ok(Self {
            providers,
            mcp_connections,
        })
    }
}

#[derive(Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
struct PlainSecretPayload {
    schema_version: u16,
    providers: Vec<PlainProviderSecret>,
    mcp_connections: Vec<PlainMcpSecret>,
}

#[derive(Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
struct PlainProviderSecret {
    connection_id: String,
    auth_mode: String,
    primary: Option<String>,
    oauth_id_token: Option<String>,
    oauth_refresh_token: Option<String>,
}

#[derive(Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
struct PlainMcpSecret {
    connection_id: String,
    auth_mode: String,
    bearer_token: Option<String>,
}

impl PlainSecretPayload {
    fn from_snapshot(snapshot: &BackupSecretSnapshot) -> Self {
        let providers = snapshot
            .providers
            .iter()
            .map(|provider| match provider {
                BackupProviderSecret::None { connection_id } => PlainProviderSecret {
                    connection_id: connection_id.clone(),
                    auth_mode: "none".into(),
                    primary: None,
                    oauth_id_token: None,
                    oauth_refresh_token: None,
                },
                BackupProviderSecret::Primary {
                    connection_id,
                    auth_mode,
                    credential,
                } => PlainProviderSecret {
                    connection_id: connection_id.clone(),
                    auth_mode: provider_auth_mode_name(*auth_mode).into(),
                    primary: Some(credential.expose_to_host_executor().to_owned()),
                    oauth_id_token: None,
                    oauth_refresh_token: None,
                },
                BackupProviderSecret::OAuth {
                    connection_id,
                    auth_mode,
                    tokens,
                } => PlainProviderSecret {
                    connection_id: connection_id.clone(),
                    auth_mode: provider_auth_mode_name(*auth_mode).into(),
                    primary: Some(tokens.access_token.expose_to_host_executor().to_owned()),
                    oauth_id_token: Some(tokens.id_token.expose_to_host_executor().to_owned()),
                    oauth_refresh_token: Some(
                        tokens.refresh_token.expose_to_host_executor().to_owned(),
                    ),
                },
            })
            .collect();
        let mcp_connections = snapshot
            .mcp_connections
            .iter()
            .map(|connection| match connection {
                BackupMcpSecret::None { connection_id } => PlainMcpSecret {
                    connection_id: connection_id.clone(),
                    auth_mode: "none".into(),
                    bearer_token: None,
                },
                BackupMcpSecret::Bearer {
                    connection_id,
                    credential,
                } => PlainMcpSecret {
                    connection_id: connection_id.clone(),
                    auth_mode: "bearer_token".into(),
                    bearer_token: Some(credential.expose_to_host_executor().to_owned()),
                },
            })
            .collect();
        Self {
            schema_version: SECRET_PAYLOAD_SCHEMA_VERSION,
            providers,
            mcp_connections,
        }
    }

    fn into_snapshot(mut self) -> Result<BackupSecretSnapshot, StorageError> {
        if self.schema_version != SECRET_PAYLOAD_SCHEMA_VERSION
            || self
                .providers
                .len()
                .saturating_add(self.mcp_connections.len())
                > MAX_SECRET_RECORDS
        {
            return Err(invalid_secret_backup(
                "credential payload contract is invalid",
            ));
        }
        let providers = self
            .providers
            .drain(..)
            .map(PlainProviderSecret::into_protected)
            .collect::<Result<Vec<_>, _>>()?;
        let mcp_connections = self
            .mcp_connections
            .drain(..)
            .map(PlainMcpSecret::into_protected)
            .collect::<Result<Vec<_>, _>>()?;
        BackupSecretSnapshot::new(providers, mcp_connections)
    }
}

impl PlainProviderSecret {
    fn into_protected(mut self) -> Result<BackupProviderSecret, StorageError> {
        let connection_id = std::mem::take(&mut self.connection_id);
        match self.auth_mode.as_str() {
            "none"
                if self.primary.is_none()
                    && self.oauth_id_token.is_none()
                    && self.oauth_refresh_token.is_none() =>
            {
                Ok(BackupProviderSecret::None { connection_id })
            }
            "api_key" | "bearer_token"
                if self.primary.is_some()
                    && self.oauth_id_token.is_none()
                    && self.oauth_refresh_token.is_none() =>
            {
                let auth_mode = if self.auth_mode == "api_key" {
                    ProviderAuthMode::ApiKey
                } else {
                    ProviderAuthMode::BearerToken
                };
                Ok(BackupProviderSecret::Primary {
                    connection_id,
                    auth_mode,
                    credential: ProviderCredential::new(
                        self.primary.take().expect("validated primary credential"),
                    )
                    .map_err(|_| invalid_secret_backup("provider credential is invalid"))?,
                })
            }
            "chat_gpt_browser_oauth" | "chat_gpt_device_code"
                if self.primary.is_some()
                    && self.oauth_id_token.is_some()
                    && self.oauth_refresh_token.is_some() =>
            {
                let auth_mode = if self.auth_mode == "chat_gpt_browser_oauth" {
                    ProviderAuthMode::ChatGptBrowserOauth
                } else {
                    ProviderAuthMode::ChatGptDeviceCode
                };
                Ok(BackupProviderSecret::OAuth {
                    connection_id,
                    auth_mode,
                    tokens: OAuthTokenSet {
                        access_token: protected_provider_value(self.primary.take())?,
                        id_token: protected_provider_value(self.oauth_id_token.take())?,
                        refresh_token: protected_provider_value(self.oauth_refresh_token.take())?,
                    },
                })
            }
            _ => Err(invalid_secret_backup(
                "provider credential payload shape is invalid",
            )),
        }
    }
}

impl PlainMcpSecret {
    fn into_protected(mut self) -> Result<BackupMcpSecret, StorageError> {
        let connection_id = std::mem::take(&mut self.connection_id);
        match self.auth_mode.as_str() {
            "none" if self.bearer_token.is_none() => Ok(BackupMcpSecret::None { connection_id }),
            "bearer_token" if self.bearer_token.is_some() => Ok(BackupMcpSecret::Bearer {
                connection_id,
                credential: McpCredential::new(
                    self.bearer_token
                        .take()
                        .expect("validated MCP bearer credential"),
                )
                .map_err(|_| invalid_secret_backup("MCP credential is invalid"))?,
            }),
            _ => Err(invalid_secret_backup(
                "MCP credential payload shape is invalid",
            )),
        }
    }
}

/// Encrypts a validated protected-store snapshot for one bound backup manifest.
///
/// # Errors
/// Returns invalid data for bad inputs, serialization, KDF, or encryption failure.
pub fn encrypt_backup_secrets(
    snapshot: &BackupSecretSnapshot,
    passphrase: &BackupPassphrase,
    associated_data: &[u8],
) -> Result<(BackupSecretEncryptionMetadata, Vec<u8>), StorageError> {
    validate_associated_data(associated_data)?;
    let payload = PlainSecretPayload::from_snapshot(snapshot);
    let plaintext = Zeroizing::new(serde_json::to_vec(&payload)?);
    if plaintext.is_empty() || plaintext.len() > MAX_SECRET_PLAINTEXT_BYTES {
        return Err(invalid_secret_backup(
            "credential payload exceeds its size bound",
        ));
    }
    let mut salt = [0_u8; SALT_BYTES];
    let mut nonce = [0_u8; NONCE_BYTES];
    rand::rng().fill_bytes(&mut salt);
    rand::rng().fill_bytes(&mut nonce);
    let key = derive_key(passphrase, &salt)?;
    let cipher = XChaCha20Poly1305::new_from_slice(key.as_ref())
        .map_err(|_| invalid_secret_backup("credential encryption key is invalid"))?;
    let nonce = XNonce::try_from(nonce.as_slice())
        .map_err(|_| invalid_secret_backup("credential encryption nonce is invalid"))?;
    let ciphertext = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext.as_ref(),
                aad: associated_data,
            },
        )
        .map_err(|_| invalid_secret_backup("credential encryption failed"))?;
    if ciphertext.len() > MAX_SECRET_CIPHERTEXT_BYTES {
        return Err(invalid_secret_backup(
            "encrypted credential payload is too large",
        ));
    }
    Ok((
        BackupSecretEncryptionMetadata {
            schema_version: BACKUP_SECRET_ENCRYPTION_SCHEMA_VERSION,
            entry_path: BACKUP_SECRET_ENTRY.into(),
            algorithm: ENCRYPTION_ALGORITHM.into(),
            kdf: KDF_ALGORITHM.into(),
            argon2_version: ARGON2_VERSION,
            memory_kib: ARGON2_MEMORY_KIB,
            iterations: ARGON2_ITERATIONS,
            parallelism: ARGON2_PARALLELISM,
            salt_base64: STANDARD_NO_PAD.encode(salt),
            nonce_base64: STANDARD_NO_PAD.encode(nonce),
        },
        ciphertext,
    ))
}

/// Authenticates and decrypts one encrypted credential entry.
///
/// # Errors
/// Returns a generic authentication/format failure without exposing secret data.
pub fn decrypt_backup_secrets(
    metadata: &BackupSecretEncryptionMetadata,
    ciphertext: &[u8],
    passphrase: &BackupPassphrase,
    associated_data: &[u8],
) -> Result<BackupSecretSnapshot, StorageError> {
    validate_associated_data(associated_data)?;
    validate_backup_secret_metadata(metadata)?;
    if ciphertext.len() < 16 || ciphertext.len() > MAX_SECRET_CIPHERTEXT_BYTES {
        return Err(invalid_secret_backup(
            "encrypted credential payload size is invalid",
        ));
    }
    let salt = decode_fixed::<SALT_BYTES>(&metadata.salt_base64, "salt")?;
    let nonce = decode_fixed::<NONCE_BYTES>(&metadata.nonce_base64, "nonce")?;
    let key = derive_key(passphrase, &salt)?;
    let cipher = XChaCha20Poly1305::new_from_slice(key.as_ref())
        .map_err(|_| invalid_secret_backup("encrypted credentials could not be authenticated"))?;
    let nonce = XNonce::try_from(nonce.as_slice())
        .map_err(|_| invalid_secret_backup("encrypted credential nonce is invalid"))?;
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(
                &nonce,
                Payload {
                    msg: ciphertext,
                    aad: associated_data,
                },
            )
            .map_err(|_| {
                invalid_secret_backup("encrypted credentials could not be authenticated")
            })?,
    );
    if plaintext.is_empty() || plaintext.len() > MAX_SECRET_PLAINTEXT_BYTES {
        return Err(invalid_secret_backup(
            "decrypted credential payload size is invalid",
        ));
    }
    let payload: PlainSecretPayload = serde_json::from_slice(plaintext.as_ref())
        .map_err(|_| invalid_secret_backup("decrypted credential payload is invalid"))?;
    payload.into_snapshot()
}

fn derive_key(
    passphrase: &BackupPassphrase,
    salt: &[u8; SALT_BYTES],
) -> Result<Zeroizing<[u8; KEY_BYTES]>, StorageError> {
    let params = Params::new(
        ARGON2_MEMORY_KIB,
        ARGON2_ITERATIONS,
        ARGON2_PARALLELISM,
        Some(KEY_BYTES),
    )
    .map_err(|_| invalid_secret_backup("credential KDF parameters are invalid"))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = Zeroizing::new([0_u8; KEY_BYTES]);
    argon
        .hash_password_into(passphrase.expose(), salt, key.as_mut())
        .map_err(|_| invalid_secret_backup("credential key derivation failed"))?;
    Ok(key)
}

pub(crate) fn validate_backup_secret_metadata(
    metadata: &BackupSecretEncryptionMetadata,
) -> Result<(), StorageError> {
    if metadata.schema_version != BACKUP_SECRET_ENCRYPTION_SCHEMA_VERSION
        || metadata.entry_path != BACKUP_SECRET_ENTRY
        || metadata.algorithm != ENCRYPTION_ALGORITHM
        || metadata.kdf != KDF_ALGORITHM
        || metadata.argon2_version != ARGON2_VERSION
        || metadata.memory_kib != ARGON2_MEMORY_KIB
        || metadata.iterations != ARGON2_ITERATIONS
        || metadata.parallelism != ARGON2_PARALLELISM
    {
        return Err(invalid_secret_backup(
            "encrypted credential metadata is unsupported",
        ));
    }
    Ok(())
}

fn validate_associated_data(associated_data: &[u8]) -> Result<(), StorageError> {
    if associated_data.is_empty() || associated_data.len() > MAX_AAD_BYTES {
        return Err(invalid_secret_backup(
            "encrypted credential binding is outside its size bound",
        ));
    }
    Ok(())
}

fn decode_fixed<const N: usize>(value: &str, label: &str) -> Result<[u8; N], StorageError> {
    let bytes = STANDARD_NO_PAD
        .decode(value)
        .map_err(|_| invalid_secret_backup(&format!("encrypted credential {label} is invalid")))?;
    bytes.try_into().map_err(|_| {
        invalid_secret_backup(&format!(
            "encrypted credential {label} has the wrong length"
        ))
    })
}

fn protected_provider_value(value: Option<String>) -> Result<ProviderCredential, StorageError> {
    ProviderCredential::new(value.expect("validated OAuth credential"))
        .map_err(|_| invalid_secret_backup("OAuth credential is invalid"))
}

fn provider_auth_mode_name(mode: ProviderAuthMode) -> &'static str {
    match mode {
        ProviderAuthMode::ChatGptBrowserOauth => "chat_gpt_browser_oauth",
        ProviderAuthMode::ChatGptDeviceCode => "chat_gpt_device_code",
        ProviderAuthMode::ApiKey => "api_key",
        ProviderAuthMode::BearerToken => "bearer_token",
        ProviderAuthMode::None => "none",
    }
}

fn validate_unique_identities<'a>(
    values: impl Iterator<Item = &'a str>,
) -> Result<(), StorageError> {
    let mut identities = BTreeSet::new();
    for value in values {
        if !identities.insert(value) {
            return Err(invalid_secret_backup(
                "credential identities are duplicated",
            ));
        }
    }
    Ok(())
}

fn validate_identity(value: &str) -> Result<(), StorageError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(invalid_secret_backup("credential identity is invalid"));
    }
    Ok(())
}

fn invalid_secret_backup(message: &str) -> StorageError {
    StorageError::InvalidData(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_snapshot() -> BackupSecretSnapshot {
        BackupSecretSnapshot::new(
            vec![
                BackupProviderSecret::Primary {
                    connection_id: "openrouter".into(),
                    auth_mode: ProviderAuthMode::ApiKey,
                    credential: ProviderCredential::new("private-openrouter-key")
                        .expect("credential"),
                },
                BackupProviderSecret::OAuth {
                    connection_id: "openai-subscription".into(),
                    auth_mode: ProviderAuthMode::ChatGptBrowserOauth,
                    tokens: OAuthTokenSet {
                        access_token: ProviderCredential::new("private-access").expect("access"),
                        id_token: ProviderCredential::new("private-id").expect("id"),
                        refresh_token: ProviderCredential::new("private-refresh").expect("refresh"),
                    },
                },
                BackupProviderSecret::None {
                    connection_id: "gpu45".into(),
                },
            ],
            vec![
                BackupMcpSecret::Bearer {
                    connection_id: "private-mcp".into(),
                    credential: McpCredential::new("private-mcp-token").expect("MCP token"),
                },
                BackupMcpSecret::None {
                    connection_id: "local-stdio".into(),
                },
            ],
        )
        .expect("snapshot")
    }

    fn passphrase() -> BackupPassphrase {
        BackupPassphrase::new("correct horse battery staple").expect("passphrase")
    }

    #[test]
    fn encrypted_credentials_round_trip_without_plaintext_or_debug_exposure() {
        let associated_data = b"bound manifest fixture";
        let (metadata, ciphertext) =
            encrypt_backup_secrets(&fixture_snapshot(), &passphrase(), associated_data)
                .expect("encrypt");
        for secret in [
            "private-openrouter-key",
            "private-access",
            "private-id",
            "private-refresh",
            "private-mcp-token",
        ] {
            assert!(
                !ciphertext
                    .windows(secret.len())
                    .any(|window| window == secret.as_bytes())
            );
        }
        assert!(!format!("{metadata:?}").contains(&metadata.salt_base64));
        let decrypted =
            decrypt_backup_secrets(&metadata, &ciphertext, &passphrase(), associated_data)
                .expect("decrypt");
        assert_eq!(decrypted.providers.len(), 3);
        assert_eq!(decrypted.mcp_connections.len(), 2);
        let BackupProviderSecret::OAuth { tokens, .. } = &decrypted.providers[1] else {
            panic!("sorted OAuth provider");
        };
        assert_eq!(
            tokens.access_token.expose_to_host_executor(),
            "private-access"
        );
        let BackupMcpSecret::Bearer { credential, .. } = &decrypted.mcp_connections[1] else {
            panic!("sorted bearer MCP connection");
        };
        assert_eq!(credential.expose_to_host_executor(), "private-mcp-token");
    }

    #[test]
    fn wrong_passphrase_ciphertext_manifest_binding_and_metadata_tampering_fail() {
        let associated_data = b"bound manifest fixture";
        let (metadata, ciphertext) =
            encrypt_backup_secrets(&fixture_snapshot(), &passphrase(), associated_data)
                .expect("encrypt");
        let wrong = BackupPassphrase::new("this is the wrong passphrase").expect("wrong");
        assert!(decrypt_backup_secrets(&metadata, &ciphertext, &wrong, associated_data).is_err());

        let mut tampered_ciphertext = ciphertext.clone();
        tampered_ciphertext[0] ^= 1;
        assert!(
            decrypt_backup_secrets(
                &metadata,
                &tampered_ciphertext,
                &passphrase(),
                associated_data,
            )
            .is_err()
        );
        assert!(
            decrypt_backup_secrets(
                &metadata,
                &ciphertext,
                &passphrase(),
                b"different manifest fixture",
            )
            .is_err()
        );
        let mut unsupported = metadata.clone();
        unsupported.memory_kib /= 2;
        assert!(
            decrypt_backup_secrets(&unsupported, &ciphertext, &passphrase(), associated_data,)
                .is_err()
        );
    }

    #[test]
    fn passphrase_and_snapshot_contracts_reject_weak_or_ambiguous_input() {
        assert!(BackupPassphrase::new("too-short").is_err());
        assert!(
            BackupSecretSnapshot::new(
                vec![
                    BackupProviderSecret::None {
                        connection_id: "duplicate".into(),
                    },
                    BackupProviderSecret::None {
                        connection_id: "duplicate".into(),
                    },
                ],
                Vec::new(),
            )
            .is_err()
        );
        assert!(
            BackupSecretSnapshot::new(
                vec![BackupProviderSecret::Primary {
                    connection_id: "mismatch".into(),
                    auth_mode: ProviderAuthMode::ChatGptBrowserOauth,
                    credential: ProviderCredential::new("private").expect("credential"),
                }],
                Vec::new(),
            )
            .is_err()
        );
    }
}
