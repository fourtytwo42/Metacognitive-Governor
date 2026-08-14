use mcg_providers::{
    CONTRACT_VERSION, ModelProvider, OpenAiCompatibleProviderBuilder, ProbeSupport,
    ProviderAuthMode, ProviderConnection, ProviderKind, probe_model,
};
use serde_json::Value;

#[tokio::test]
async fn gpu45_discovers_active_model_and_completes_response() {
    if std::env::var("MCG_LIVE_GPU45").as_deref() != Ok("1") {
        eprintln!("skipped: set MCG_LIVE_GPU45=1 to run the LAN provider conformance test");
        return;
    }

    let console_url = std::env::var("MCG_GPU45_CONSOLE_URL")
        .unwrap_or_else(|_| "http://192-168-50-189.nip.io".into());
    let inference_url = std::env::var("MCG_GPU45_BASE_URL")
        .unwrap_or_else(|_| "http://192-168-50-189.nip.io:30001/v1".into());
    let health: Value = reqwest::get(format!("{console_url}/api/health/summary"))
        .await
        .expect("GPU45 health request")
        .error_for_status()
        .expect("GPU45 healthy HTTP status")
        .json()
        .await
        .expect("GPU45 health JSON");
    let active_model = health
        .pointer("/provider/model")
        .and_then(Value::as_str)
        .expect("GPU45 reports its active model")
        .to_owned();

    let provider = OpenAiCompatibleProviderBuilder::new(ProviderConnection {
        contract_version: CONTRACT_VERSION,
        id: "gpu45-live".into(),
        display_name: "GPU45 live conformance".into(),
        kind: ProviderKind::OpenAiCompatible,
        base_url: inference_url,
        auth_mode: ProviderAuthMode::None,
        organization: None,
        project: None,
    })
    .build()
    .expect("GPU45 provider configuration");
    let models = provider.list_models().await.expect("GPU45 model discovery");
    assert!(models.iter().any(|model| model.id == active_model));

    let report = probe_model(&provider, &active_model)
        .await
        .expect("GPU45 common provider conformance");
    assert_eq!(report.model.id, active_model);
    assert!(
        report
            .observations
            .iter()
            .all(|observation| observation.support == ProbeSupport::Supported)
    );
}
