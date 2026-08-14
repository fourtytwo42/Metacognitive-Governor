use mcg_api::StartOpenAiOAuthRequest;
use mcg_config::AppConfig;
use mcg_events::EventEnvelope;
use mcg_runtime::{OAuthLoginStart, ProviderMutation};
use uuid::Uuid;

const LOCAL_DAEMON: &str = "http://127.0.0.1:7777";

#[tauri::command]
const fn runtime_label() -> &'static str {
    "MCG desktop scaffold"
}

fn daemon_token() -> Result<String, String> {
    let path = AppConfig::default().auth_token_path();
    std::fs::read_to_string(&path)
        .map(|token| token.trim().to_owned())
        .map_err(|error| {
            format!(
                "Cannot connect to the local MCG service at {}: {error}",
                path.display()
            )
        })
}

async fn response_error(response: reqwest::Response, action: &str) -> String {
    let status = response.status();
    let detail = response
        .text()
        .await
        .unwrap_or_else(|_| "No error details were returned".into());
    format!("{action} failed ({status}): {detail}")
}

#[tauri::command]
async fn start_openai_oauth(request: StartOpenAiOAuthRequest) -> Result<OAuthLoginStart, String> {
    let response = reqwest::Client::new()
        .post(format!("{LOCAL_DAEMON}/api/v1/oauth/openai/start"))
        .bearer_auth(daemon_token()?)
        .json(&request)
        .send()
        .await
        .map_err(|error| format!("OpenAI login could not start: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "OpenAI login start").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("OpenAI login response was invalid: {error}"))
}

#[tauri::command]
async fn complete_openai_oauth(session_id: Uuid) -> Result<ProviderMutation, String> {
    let response = reqwest::Client::new()
        .post(format!(
            "{LOCAL_DAEMON}/api/v1/oauth/openai/{session_id}/complete"
        ))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("OpenAI login could not complete: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "OpenAI login completion").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("OpenAI login response was invalid: {error}"))
}

#[tauri::command]
async fn cancel_openai_oauth(session_id: Uuid) -> Result<(), String> {
    let response = reqwest::Client::new()
        .post(format!(
            "{LOCAL_DAEMON}/api/v1/oauth/openai/{session_id}/cancel"
        ))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("OpenAI login could not be cancelled: {error}"))?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(response_error(response, "OpenAI login cancellation").await)
    }
}

#[tauri::command]
async fn refresh_openai_oauth(connection_id: String) -> Result<EventEnvelope, String> {
    let response = reqwest::Client::new()
        .post(format!(
            "{LOCAL_DAEMON}/api/v1/providers/{connection_id}/oauth/refresh"
        ))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("OpenAI login could not be refreshed: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "OpenAI token refresh").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("OpenAI refresh response was invalid: {error}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Starts the native desktop application.
///
/// # Panics
///
/// Panics when Tauri cannot initialize or run the application event loop.
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            runtime_label,
            start_openai_oauth,
            complete_openai_oauth,
            cancel_openai_oauth,
            refresh_openai_oauth
        ])
        .run(tauri::generate_context!())
        .expect("error while running MCG desktop application");
}
