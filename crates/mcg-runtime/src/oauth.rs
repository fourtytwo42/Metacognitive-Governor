use std::{collections::BTreeMap, time::Instant};

use chrono::Utc;
use mcg_events::EventEnvelope;
use mcg_providers::{
    BrowserAuthorization, BrowserAuthorizationClient, BrowserAuthorizationConfig,
    CHATGPT_CODEX_BASE_URL, DeviceAuthorization, DeviceAuthorizationClient,
    DeviceAuthorizationConfig, OAuthTokenSet, ProviderAuthMode, ProviderConnection, ProviderKind,
    ProviderModelCatalog, clear_provider_credentials, load_oauth_credentials,
    save_provider_credentials,
};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;

use crate::{AppRuntime, ProviderMutation, RuntimeError, StorageError};

const MAX_PENDING_OAUTH_LOGINS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenAiOAuthMethod {
    Browser,
    DeviceCode,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OAuthLoginStart {
    pub session_id: Uuid,
    pub connection_id: String,
    pub auth_mode: ProviderAuthMode,
    pub verification_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_code: Option<String>,
}

impl std::fmt::Debug for OAuthLoginStart {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OAuthLoginStart")
            .field("session_id", &self.session_id)
            .field("connection_id", &self.connection_id)
            .field("auth_mode", &self.auth_mode)
            .field("verification_url", &"[REDACTED]")
            .field("user_code", &self.user_code.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

enum PendingAuthorization {
    Browser {
        client: BrowserAuthorizationClient,
        authorization: BrowserAuthorization,
    },
    DeviceCode {
        client: DeviceAuthorizationClient,
        authorization: DeviceAuthorization,
    },
}

struct PendingOAuthLogin {
    connection: ProviderConnection,
    authorization: PendingAuthorization,
    expires_at: Instant,
}

pub(crate) struct OAuthRuntimeState {
    sessions: AsyncMutex<BTreeMap<Uuid, PendingOAuthLogin>>,
    browser_config: BrowserAuthorizationConfig,
    device_config: DeviceAuthorizationConfig,
}

impl OAuthRuntimeState {
    pub(crate) fn new(
        browser_config: BrowserAuthorizationConfig,
        device_config: DeviceAuthorizationConfig,
    ) -> Self {
        Self {
            sessions: AsyncMutex::new(BTreeMap::new()),
            browser_config,
            device_config,
        }
    }
}

impl AppRuntime {
    /// Starts a short-lived standalone `ChatGPT` OAuth flow. Only the authorization
    /// handle is retained in memory; no provider metadata or token is persisted yet.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid/duplicate identities, exhausted pending-session
    /// capacity, unavailable callback ports, or OAuth transport failure.
    pub async fn start_openai_oauth(
        &self,
        connection_id: String,
        display_name: String,
        method: OpenAiOAuthMethod,
    ) -> Result<OAuthLoginStart, RuntimeError> {
        let auth_mode = match method {
            OpenAiOAuthMethod::Browser => ProviderAuthMode::ChatGptBrowserOauth,
            OpenAiOAuthMethod::DeviceCode => ProviderAuthMode::ChatGptDeviceCode,
        };
        let connection = ProviderConnection {
            contract_version: mcg_providers::CONTRACT_VERSION,
            id: connection_id,
            display_name,
            kind: ProviderKind::OpenAi,
            base_url: CHATGPT_CODEX_BASE_URL.into(),
            auth_mode,
            organization: None,
            project: None,
        };
        connection.validate()?;
        if self.store()?.provider_connection(&connection.id)?.is_some() {
            return Err(StorageError::Conflict(format!(
                "provider connection {} already exists",
                connection.id
            ))
            .into());
        }
        self.ensure_oauth_session_capacity(&connection.id).await?;

        let (authorization, verification_url, user_code, timeout) = match method {
            OpenAiOAuthMethod::Browser => {
                let client =
                    BrowserAuthorizationClient::new(self.oauth_state.browser_config.clone())?;
                let authorization = client.start().await?;
                let verification_url = authorization.authorization_url.clone();
                (
                    PendingAuthorization::Browser {
                        client,
                        authorization,
                    },
                    verification_url,
                    None,
                    self.oauth_state.browser_config.timeout,
                )
            }
            OpenAiOAuthMethod::DeviceCode => {
                let client =
                    DeviceAuthorizationClient::new(self.oauth_state.device_config.clone())?;
                let authorization = client.start().await?;
                let verification_url = authorization.verification_url.clone();
                let user_code = Some(authorization.user_code.clone());
                (
                    PendingAuthorization::DeviceCode {
                        client,
                        authorization,
                    },
                    verification_url,
                    user_code,
                    self.oauth_state.device_config.timeout,
                )
            }
        };

        let session_id = Uuid::new_v4();
        let mut sessions = self.oauth_state.sessions.lock().await;
        if sessions.len() >= MAX_PENDING_OAUTH_LOGINS {
            return Err(
                StorageError::Conflict("too many OpenAI OAuth logins are pending".into()).into(),
            );
        }
        if sessions
            .values()
            .any(|pending| pending.connection.id == connection.id)
            || self.store()?.provider_connection(&connection.id)?.is_some()
        {
            return Err(StorageError::Conflict(format!(
                "provider connection {} already exists or is pending",
                connection.id
            ))
            .into());
        }
        sessions.insert(
            session_id,
            PendingOAuthLogin {
                connection: connection.clone(),
                authorization,
                expires_at: Instant::now() + timeout,
            },
        );
        Ok(OAuthLoginStart {
            session_id,
            connection_id: connection.id,
            auth_mode,
            verification_url,
            user_code,
        })
    }

    /// Completes one in-memory OAuth flow and atomically creates its secret-free
    /// subscription provider projection after storing tokens in the protected store.
    ///
    /// Model discovery remains explicit because the subscription backend may use
    /// an account-specific catalog surface; login success never selects API billing.
    ///
    /// # Errors
    ///
    /// Returns an error for unknown/consumed sessions, authentication failure,
    /// protected-store failure, or failed durable provider persistence.
    pub async fn complete_openai_oauth(
        &self,
        session_id: Uuid,
    ) -> Result<ProviderMutation, RuntimeError> {
        let pending = self
            .oauth_state
            .sessions
            .lock()
            .await
            .remove(&session_id)
            .ok_or_else(|| {
                StorageError::InvalidData(
                    "OAuth login session was not found or was consumed".into(),
                )
            })?;
        if pending.expires_at <= Instant::now() {
            return Err(StorageError::InvalidData("OAuth login session expired".into()).into());
        }
        let tokens = match pending.authorization {
            PendingAuthorization::Browser {
                client,
                authorization,
            } => client.complete(authorization).await?,
            PendingAuthorization::DeviceCode {
                client,
                authorization,
            } => client.complete(&authorization).await?,
        };
        self.register_oauth_connection(pending.connection, &tokens)
            .await
    }

    /// Cancels one not-yet-completed OAuth flow and drops its callback/code state.
    ///
    /// # Errors
    ///
    /// Returns an error when the session is unknown or already consumed.
    pub async fn cancel_openai_oauth(&self, session_id: Uuid) -> Result<(), RuntimeError> {
        if self
            .oauth_state
            .sessions
            .lock()
            .await
            .remove(&session_id)
            .is_none()
        {
            return Err(StorageError::InvalidData(
                "OAuth login session was not found or was consumed".into(),
            )
            .into());
        }
        Ok(())
    }

    /// Refreshes one OAuth connection without changing its billing/authentication
    /// mode, then appends secret-free durable rotation evidence.
    ///
    /// # Errors
    ///
    /// Returns not-found, non-OAuth, protected-store, issuer, registry, or durable
    /// event failures. A failed replacement restores the previous token set.
    pub async fn refresh_openai_oauth(
        &self,
        connection_id: &str,
    ) -> Result<EventEnvelope, RuntimeError> {
        let connection = self
            .store()?
            .provider_connection(connection_id)?
            .ok_or_else(|| {
                StorageError::InvalidData(format!(
                    "provider connection {connection_id} was not found"
                ))
            })?;
        let previous = load_oauth_credentials(
            self.provider_credentials.as_ref(),
            connection_id,
            connection.auth_mode,
        )?;
        let refreshed = previous.refresh(&self.oauth_state.device_config).await?;

        let mut registry = self.provider_registry.write().await;
        registry.unregister(connection_id);
        if let Err(error) = save_provider_credentials(
            self.provider_credentials.as_ref(),
            connection_id,
            connection.auth_mode,
            None,
            Some(&refreshed),
        ) {
            let _ = registry.register_connection(connection, self.provider_credentials.as_ref());
            return Err(error.into());
        }
        if let Err(error) =
            registry.register_connection(connection.clone(), self.provider_credentials.as_ref())
        {
            let _ = save_provider_credentials(
                self.provider_credentials.as_ref(),
                connection_id,
                connection.auth_mode,
                None,
                Some(&previous),
            );
            let _ = registry.register_connection(connection, self.provider_credentials.as_ref());
            return Err(error.into());
        }
        let event = match self
            .store()?
            .record_provider_credentials_refreshed(connection_id, Utc::now())
        {
            Ok(event) => event,
            Err(error) => {
                registry.unregister(connection_id);
                let _ = save_provider_credentials(
                    self.provider_credentials.as_ref(),
                    connection_id,
                    connection.auth_mode,
                    None,
                    Some(&previous),
                );
                let _ =
                    registry.register_connection(connection, self.provider_credentials.as_ref());
                return Err(error.into());
            }
        };
        drop(registry);
        let _ = self.event_sender.send(event.clone());
        Ok(event)
    }

    async fn ensure_oauth_session_capacity(&self, connection_id: &str) -> Result<(), RuntimeError> {
        let mut sessions = self.oauth_state.sessions.lock().await;
        let now = Instant::now();
        sessions.retain(|_, pending| pending.expires_at > now);
        if sessions.len() >= MAX_PENDING_OAUTH_LOGINS {
            return Err(
                StorageError::Conflict("too many OpenAI OAuth logins are pending".into()).into(),
            );
        }
        if sessions
            .values()
            .any(|pending| pending.connection.id == connection_id)
        {
            return Err(StorageError::Conflict(format!(
                "provider connection {connection_id} already exists or is pending"
            ))
            .into());
        }
        Ok(())
    }

    async fn register_oauth_connection(
        &self,
        connection: ProviderConnection,
        tokens: &OAuthTokenSet,
    ) -> Result<ProviderMutation, RuntimeError> {
        save_provider_credentials(
            self.provider_credentials.as_ref(),
            &connection.id,
            connection.auth_mode,
            None,
            Some(tokens),
        )?;
        let registration = self
            .provider_registry
            .write()
            .await
            .register_connection(connection.clone(), self.provider_credentials.as_ref());
        if let Err(error) = registration {
            let _ = clear_provider_credentials(self.provider_credentials.as_ref(), &connection.id);
            return Err(error.into());
        }
        let catalog = ProviderModelCatalog {
            connection_id: connection.id.clone(),
            models: Vec::new(),
            discovered_at: Utc::now(),
        };
        let persistence =
            self.store()?
                .record_provider_configuration(&connection, &catalog, Utc::now());
        let events = match persistence {
            Ok(events) => events,
            Err(error) => {
                self.provider_registry
                    .write()
                    .await
                    .unregister(&connection.id);
                let _ =
                    clear_provider_credentials(self.provider_credentials.as_ref(), &connection.id);
                return Err(error.into());
            }
        };
        for event in &events {
            let _ = self.event_sender.send(event.clone());
        }
        Ok(ProviderMutation {
            connection,
            catalog,
            events,
        })
    }
}
