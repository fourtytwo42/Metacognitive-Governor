use std::{collections::BTreeMap, sync::Arc};

use crate::{
    CHATGPT_CODEX_BASE_URL, ModelDescriptor, ModelProbeReport, ModelProvider, ModelRequest,
    ModelResponse, OAuthTokenSet, OpenAiCompatibleProviderBuilder, ProviderAuthMode,
    ProviderConnection, ProviderCredentialStore, ProviderError, ProviderKind, ProviderModelRef,
    ProviderSecretSlot, ProviderStream, RouteCandidate, probe_model,
};

#[derive(Default)]
pub struct ProviderRegistry {
    providers: BTreeMap<String, Arc<dyn ModelProvider>>,
    discovered: BTreeMap<String, Vec<ModelDescriptor>>,
}

impl std::fmt::Debug for ProviderRegistry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProviderRegistry")
            .field("provider_ids", &self.providers.keys().collect::<Vec<_>>())
            .field("discovered_provider_count", &self.discovered.len())
            .finish()
    }
}

impl ProviderRegistry {
    /// Builds and registers a provider from secret-free metadata plus credentials
    /// loaded from the supplied protected store.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid metadata, missing credentials, billing-mode
    /// boundary violations, or duplicate identities.
    pub fn register_connection<S: ProviderCredentialStore + ?Sized>(
        &mut self,
        connection: ProviderConnection,
        credentials: &S,
    ) -> Result<(), ProviderError> {
        connection.validate()?;
        let provider: Arc<dyn ModelProvider> = match connection.auth_mode {
            ProviderAuthMode::None => {
                Arc::new(OpenAiCompatibleProviderBuilder::new(connection).build()?)
            }
            ProviderAuthMode::ApiKey | ProviderAuthMode::BearerToken => {
                let credential = credentials
                    .load(&connection.id, ProviderSecretSlot::Primary)?
                    .ok_or_else(|| {
                        ProviderError::Authentication(format!(
                            "provider connection {} has no protected credential",
                            connection.id
                        ))
                    })?;
                Arc::new(
                    OpenAiCompatibleProviderBuilder::new(connection)
                        .credential(credential)
                        .build()?,
                )
            }
            ProviderAuthMode::ChatGptBrowserOauth | ProviderAuthMode::ChatGptDeviceCode => {
                if connection.kind != ProviderKind::OpenAi
                    || connection.base_url.trim_end_matches('/') != CHATGPT_CODEX_BASE_URL
                {
                    return Err(ProviderError::Configuration(
                        "ChatGPT OAuth connection must use the OpenAI subscription backend".into(),
                    ));
                }
                let load_required = |slot| {
                    credentials.load(&connection.id, slot)?.ok_or_else(|| {
                        ProviderError::Authentication(format!(
                            "provider connection {} has incomplete OAuth credentials",
                            connection.id
                        ))
                    })
                };
                let tokens = OAuthTokenSet {
                    access_token: load_required(ProviderSecretSlot::Primary)?,
                    id_token: load_required(ProviderSecretSlot::OAuthIdToken)?,
                    refresh_token: load_required(ProviderSecretSlot::OAuthRefreshToken)?,
                };
                Arc::new(OpenAiCompatibleProviderBuilder::openai_subscription(
                    connection.id,
                    connection.display_name,
                    connection.auth_mode,
                    &tokens,
                )?)
            }
        };
        self.register(provider)
    }

    /// Registers one already-authenticated provider without exposing its credential.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid metadata or a duplicate identity.
    pub fn register(&mut self, provider: Arc<dyn ModelProvider>) -> Result<(), ProviderError> {
        provider.connection().validate()?;
        let provider_id = provider.connection().id.clone();
        if self.providers.contains_key(&provider_id) {
            return Err(ProviderError::Configuration(format!(
                "provider connection {provider_id} is already registered"
            )));
        }
        self.providers.insert(provider_id, provider);
        Ok(())
    }

    #[must_use]
    pub fn provider_ids(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }

    pub fn unregister(&mut self, provider_id: &str) {
        self.providers.remove(provider_id);
        self.discovered.remove(provider_id);
    }

    /// Discovers and replaces the cached model snapshot for one provider.
    ///
    /// # Errors
    ///
    /// Returns provider lookup, authentication, transport, or conformance errors.
    pub async fn discover(
        &mut self,
        provider_id: &str,
    ) -> Result<Vec<ModelDescriptor>, ProviderError> {
        let provider = self.provider(provider_id)?;
        let mut models = provider.list_models().await?;
        models.sort_by(|left, right| left.id.cmp(&right.id));
        models.dedup_by(|left, right| left.id == right.id);
        if models.iter().any(|model| model.id.trim().is_empty()) {
            return Err(ProviderError::Incompatible(
                "model discovery returned an empty model id".into(),
            ));
        }
        self.discovered
            .insert(provider_id.to_owned(), models.clone());
        Ok(models)
    }

    /// Runs explicit live capability probes for one discovered model and replaces
    /// that model's in-memory descriptor with the observed result.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown provider/model or failed discovery.
    pub async fn probe_model(
        &mut self,
        provider_id: &str,
        model_id: &str,
    ) -> Result<ModelProbeReport, ProviderError> {
        let provider = self.provider(provider_id)?;
        if !self
            .discovered
            .get(provider_id)
            .is_some_and(|models| models.iter().any(|model| model.id == model_id))
        {
            return Err(ProviderError::Configuration(format!(
                "model {model_id} has not been discovered for provider {provider_id}"
            )));
        }
        let report = probe_model(provider.as_ref(), model_id).await?;
        let models = self
            .discovered
            .get_mut(provider_id)
            .ok_or_else(|| ProviderError::Configuration("provider has no model catalog".into()))?;
        let model = models
            .iter_mut()
            .find(|model| model.id == model_id)
            .ok_or_else(|| ProviderError::Configuration("probed model disappeared".into()))?;
        model.clone_from(&report.model);
        Ok(report)
    }

    #[must_use]
    pub fn models(&self, provider_id: &str) -> Option<Vec<ModelDescriptor>> {
        self.discovered.get(provider_id).cloned()
    }

    /// Restores a previously validated, secret-free durable model catalog for
    /// an already registered provider without making a network request.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown provider, mismatched identity, or
    /// malformed/duplicate model identifiers.
    pub fn restore_catalog(
        &mut self,
        provider_id: &str,
        mut models: Vec<ModelDescriptor>,
    ) -> Result<(), ProviderError> {
        let Some(provider) = self.providers.get(provider_id) else {
            return Err(ProviderError::Configuration(format!(
                "provider connection {provider_id} is not registered"
            )));
        };
        let subscription = matches!(
            provider.connection().auth_mode,
            ProviderAuthMode::ChatGptBrowserOauth | ProviderAuthMode::ChatGptDeviceCode
        );
        if subscription {
            for model in &mut models {
                if model.capabilities.responses == Some(true) && model.capabilities.tools.is_none()
                {
                    model.capabilities.tools = Some(true);
                }
            }
        }
        models.sort_by(|left, right| left.id.cmp(&right.id));
        if models.iter().any(|model| model.id.trim().is_empty())
            || models.windows(2).any(|pair| pair[0].id == pair[1].id)
        {
            return Err(ProviderError::Incompatible(
                "durable model catalog contains duplicate or empty identities".into(),
            ));
        }
        self.discovered.insert(provider_id.to_owned(), models);
        Ok(())
    }

    #[must_use]
    pub fn route_candidates(&self) -> Vec<RouteCandidate> {
        self.discovered
            .iter()
            .flat_map(|(provider_id, models)| {
                models.iter().cloned().map(|model| RouteCandidate {
                    target: ProviderModelRef {
                        provider_id: provider_id.clone(),
                        model_id: model.id.clone(),
                    },
                    model,
                    preference: 0,
                })
            })
            .collect()
    }

    /// Calls the exact registered provider/model target selected by the router.
    ///
    /// # Errors
    ///
    /// Returns an error for unknown targets or provider call failure.
    pub async fn create_response(
        &self,
        target: &ProviderModelRef,
        mut request: ModelRequest,
    ) -> Result<ModelResponse, ProviderError> {
        let provider = self.provider(&target.provider_id)?;
        let known_model = self
            .discovered
            .get(&target.provider_id)
            .is_some_and(|models| models.iter().any(|model| model.id == target.model_id));
        if !known_model {
            return Err(ProviderError::Configuration(format!(
                "model {} has not been discovered for provider {}",
                target.model_id, target.provider_id
            )));
        }
        request.model.clone_from(&target.model_id);
        provider.create_response(request).await
    }

    /// Streams a response from the exact registered and discovered target.
    ///
    /// # Errors
    ///
    /// Returns an error for unknown targets or provider stream failure.
    pub async fn stream_response(
        &self,
        target: &ProviderModelRef,
        mut request: ModelRequest,
    ) -> Result<ProviderStream, ProviderError> {
        let provider = self.provider(&target.provider_id)?;
        let known_model = self
            .discovered
            .get(&target.provider_id)
            .is_some_and(|models| models.iter().any(|model| model.id == target.model_id));
        if !known_model {
            return Err(ProviderError::Configuration(format!(
                "model {} has not been discovered for provider {}",
                target.model_id, target.provider_id
            )));
        }
        request.model.clone_from(&target.model_id);
        provider.stream_response(request).await
    }

    fn provider(&self, provider_id: &str) -> Result<Arc<dyn ModelProvider>, ProviderError> {
        self.providers.get(provider_id).cloned().ok_or_else(|| {
            ProviderError::Configuration(format!(
                "provider connection {provider_id} is not registered"
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use futures_util::stream;
    use serde_json::Value;

    use super::*;
    use crate::{
        CONTRACT_VERSION, MemoryProviderCredentialStore, ModelCapabilities, ModelStreamEvent,
        OutputItem, ProviderAuthMode, ProviderConnection, ProviderKind, ProviderStream, Usage,
    };

    struct FixtureProvider {
        connection: ProviderConnection,
    }

    #[async_trait]
    impl ModelProvider for FixtureProvider {
        fn connection(&self) -> &ProviderConnection {
            &self.connection
        }

        async fn list_models(&self) -> Result<Vec<ModelDescriptor>, ProviderError> {
            Ok(vec![ModelDescriptor {
                id: "fixture-model".into(),
                display_name: "Fixture".into(),
                owned_by: None,
                context_window: Some(16_384),
                capabilities: ModelCapabilities {
                    responses: Some(true),
                    tools: Some(true),
                    ..ModelCapabilities::default()
                },
                price: None,
                metadata: Value::Null,
            }])
        }

        async fn create_response(
            &self,
            request: ModelRequest,
        ) -> Result<ModelResponse, ProviderError> {
            Ok(ModelResponse {
                contract_version: CONTRACT_VERSION,
                id: "fixture-response".into(),
                model: request.model,
                status: Some("completed".into()),
                output: Vec::<OutputItem>::new(),
                usage: Usage::default(),
                provider_metadata: Value::Null,
            })
        }

        async fn stream_response(
            &self,
            _request: ModelRequest,
        ) -> Result<ProviderStream, ProviderError> {
            Ok(Box::pin(stream::empty::<
                Result<ModelStreamEvent, ProviderError>,
            >()))
        }
    }

    fn fixture_provider(id: &str) -> Arc<dyn ModelProvider> {
        Arc::new(FixtureProvider {
            connection: ProviderConnection {
                contract_version: CONTRACT_VERSION,
                id: id.into(),
                display_name: "Fixture".into(),
                kind: ProviderKind::OpenAiCompatible,
                base_url: "http://127.0.0.1:9999/v1".into(),
                auth_mode: ProviderAuthMode::None,
                organization: None,
                project: None,
            },
        })
    }

    #[tokio::test]
    async fn discovery_is_deterministic_and_calls_only_exact_registered_target() {
        let mut registry = ProviderRegistry::default();
        registry
            .register(fixture_provider("fixture"))
            .expect("register");
        assert!(registry.register(fixture_provider("fixture")).is_err());
        let models = registry.discover("fixture").await.expect("discover");
        assert_eq!(models.len(), 1);
        let candidates = registry.route_candidates();
        assert_eq!(candidates.len(), 1);
        let response = registry
            .create_response(
                &candidates[0].target,
                ModelRequest {
                    contract_version: CONTRACT_VERSION,
                    model: "ignored-by-registry".into(),
                    client_request_id: None,
                    instructions: None,
                    messages: Vec::new(),
                    tools: Vec::new(),
                    tool_choice: None,
                    structured_output: None,
                    reasoning: None,
                    temperature: None,
                    top_p: None,
                    max_output_tokens: None,
                    metadata: Value::Null,
                },
            )
            .await
            .expect("response");
        assert_eq!(response.model, "fixture-model");
        assert!(
            registry
                .create_response(
                    &ProviderModelRef {
                        provider_id: "fixture".into(),
                        model_id: "not-discovered".into(),
                    },
                    ModelRequest {
                        contract_version: CONTRACT_VERSION,
                        model: "not-discovered".into(),
                        client_request_id: None,
                        instructions: None,
                        messages: Vec::new(),
                        tools: Vec::new(),
                        tool_choice: None,
                        structured_output: None,
                        reasoning: None,
                        temperature: None,
                        top_p: None,
                        max_output_tokens: None,
                        metadata: Value::Null,
                    },
                )
                .await
                .is_err()
        );
    }

    #[test]
    fn configured_registration_requires_the_exact_auth_shape() {
        let credentials = MemoryProviderCredentialStore::default();
        let mut registry = ProviderRegistry::default();
        let mut connection = fixture_provider("unused").connection().clone();
        connection.id = "local".into();
        registry
            .register_connection(connection.clone(), &credentials)
            .expect("anonymous local connection");
        connection.id = "paid".into();
        connection.auth_mode = ProviderAuthMode::ApiKey;
        assert!(
            registry
                .register_connection(connection, &credentials)
                .is_err()
        );
        assert_eq!(registry.provider_ids(), vec!["local"]);
    }

    #[test]
    fn restored_subscription_catalogs_gain_the_implicit_codex_tool_capability() {
        let mut registry = ProviderRegistry::default();
        registry
            .register(Arc::new(FixtureProvider {
                connection: ProviderConnection {
                    contract_version: CONTRACT_VERSION,
                    id: "subscription".into(),
                    display_name: "OpenAI Subscription".into(),
                    kind: ProviderKind::OpenAi,
                    base_url: CHATGPT_CODEX_BASE_URL.into(),
                    auth_mode: ProviderAuthMode::ChatGptBrowserOauth,
                    organization: None,
                    project: None,
                },
            }))
            .expect("register subscription fixture");
        registry
            .restore_catalog(
                "subscription",
                vec![ModelDescriptor {
                    id: "gpt-codex".into(),
                    display_name: "GPT Codex".into(),
                    owned_by: Some("openai".into()),
                    context_window: Some(272_000),
                    capabilities: ModelCapabilities {
                        responses: Some(true),
                        tools: None,
                        ..ModelCapabilities::default()
                    },
                    price: None,
                    metadata: Value::Null,
                }],
            )
            .expect("restore old subscription catalog");

        assert_eq!(
            registry.route_candidates()[0].model.capabilities.tools,
            Some(true)
        );
    }
}
