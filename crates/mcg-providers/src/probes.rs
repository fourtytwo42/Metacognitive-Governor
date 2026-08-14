use std::time::Duration;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    CONTRACT_VERSION, ContentPart, Message, MessageRole, ModelDescriptor, ModelProvider,
    ModelRequest, ProviderError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeSupport {
    Supported,
    Unsupported,
    Indeterminate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeObservation {
    pub operation: String,
    pub support: ProbeSupport,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelProbeReport {
    pub provider_id: String,
    pub model: ModelDescriptor,
    pub observations: Vec<ProbeObservation>,
}

/// Discovers a model, verifies a minimal buffered Responses call, and verifies
/// that a streaming response yields at least one valid canonical event.
///
/// The probe does not infer tool or structured-output support from model quality;
/// those capabilities remain provider-declared until dedicated evals can separate
/// endpoint support from instruction-following quality.
///
/// # Errors
/// Returns an error when model discovery itself fails or the requested model is
/// absent. Individual inference probe failures are preserved as observations.
pub async fn probe_model(
    provider: &dyn ModelProvider,
    model_id: &str,
) -> Result<ModelProbeReport, ProviderError> {
    let mut model = provider
        .list_models()
        .await?
        .into_iter()
        .find(|model| model.id == model_id)
        .ok_or_else(|| {
            ProviderError::Configuration(format!(
                "model {model_id} was not returned by provider discovery"
            ))
        })?;
    let request = probe_request(model_id);
    let buffered = provider.create_response(request.clone()).await;
    let mut observations = vec![observation("responses", &buffered)];
    model.capabilities.responses = Some(buffered.is_ok());

    let streaming = match provider.stream_response(request).await {
        Ok(mut stream) => {
            match tokio::time::timeout(Duration::from_secs(30), stream.next()).await {
                Ok(Some(Ok(_))) => Ok(()),
                Ok(Some(Err(error))) => Err(error),
                Ok(None) => Err(ProviderError::Incompatible(
                    "provider stream ended without an event".into(),
                )),
                Err(_) => Err(ProviderError::Request {
                    message: "provider stream produced no event within 30 seconds".into(),
                    retriable: true,
                }),
            }
        }
        Err(error) => Err(error),
    };
    observations.push(observation("streaming", &streaming));
    model.capabilities.streaming = Some(streaming.is_ok());
    Ok(ModelProbeReport {
        provider_id: provider.connection().id.clone(),
        model,
        observations,
    })
}

fn observation<T>(operation: &str, result: &Result<T, ProviderError>) -> ProbeObservation {
    match result {
        Ok(_) => ProbeObservation {
            operation: operation.into(),
            support: ProbeSupport::Supported,
            detail: "probe passed".into(),
        },
        Err(error) => ProbeObservation {
            operation: operation.into(),
            support: if matches!(
                error,
                ProviderError::Http {
                    status: 400 | 404 | 405 | 422,
                    ..
                }
            ) {
                ProbeSupport::Unsupported
            } else {
                ProbeSupport::Indeterminate
            },
            detail: error.to_string(),
        },
    }
}

fn probe_request(model: &str) -> ModelRequest {
    ModelRequest {
        contract_version: CONTRACT_VERSION,
        model: model.into(),
        instructions: None,
        messages: vec![Message {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "Reply with OK.".into(),
            }],
            name: None,
        }],
        tools: vec![],
        tool_choice: None,
        structured_output: None,
        reasoning: None,
        temperature: Some(0.0),
        top_p: None,
        max_output_tokens: Some(16),
        metadata: Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use futures_util::stream;

    use super::*;
    use crate::{
        ModelCapabilities, ModelResponse, ModelStreamEvent, OutputItem, ProviderAuthMode,
        ProviderConnection, ProviderKind, ProviderStream, Usage,
    };

    struct FakeProvider {
        connection: ProviderConnection,
    }

    #[async_trait]
    impl ModelProvider for FakeProvider {
        fn connection(&self) -> &ProviderConnection {
            &self.connection
        }

        async fn list_models(&self) -> Result<Vec<ModelDescriptor>, ProviderError> {
            Ok(vec![ModelDescriptor {
                id: "fixture".into(),
                display_name: "Fixture".into(),
                owned_by: None,
                context_window: Some(8_192),
                capabilities: ModelCapabilities::default(),
                price: None,
                metadata: Value::Null,
            }])
        }

        async fn create_response(
            &self,
            _request: ModelRequest,
        ) -> Result<ModelResponse, ProviderError> {
            Ok(response())
        }

        async fn stream_response(
            &self,
            _request: ModelRequest,
        ) -> Result<ProviderStream, ProviderError> {
            Ok(Box::pin(stream::iter([Ok(ModelStreamEvent::Completed {
                response: response(),
            })])))
        }
    }

    fn response() -> ModelResponse {
        ModelResponse {
            contract_version: CONTRACT_VERSION,
            id: "response".into(),
            model: "fixture".into(),
            status: Some("completed".into()),
            output: vec![OutputItem::Message {
                role: MessageRole::Assistant,
                content: vec![ContentPart::Text { text: "OK".into() }],
            }],
            usage: Usage::default(),
            provider_metadata: Value::Null,
        }
    }

    #[tokio::test]
    async fn probe_combines_discovery_and_observed_capabilities() {
        let provider = FakeProvider {
            connection: ProviderConnection {
                contract_version: CONTRACT_VERSION,
                id: "fake".into(),
                display_name: "Fake".into(),
                kind: ProviderKind::OpenAiCompatible,
                base_url: "http://127.0.0.1/v1".into(),
                auth_mode: ProviderAuthMode::None,
                organization: None,
                project: None,
            },
        };
        let report = probe_model(&provider, "fixture").await.expect("probe");
        assert_eq!(report.model.capabilities.responses, Some(true));
        assert_eq!(report.model.capabilities.streaming, Some(true));
        assert!(
            report
                .observations
                .iter()
                .all(|observation| observation.support == ProbeSupport::Supported)
        );
    }
}
