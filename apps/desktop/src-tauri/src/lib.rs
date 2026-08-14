use mcg_api::{
    CreateTaskRequest, DiscoverProjectRequest, LaunchTaskRequest, LaunchTaskResponse,
    McpListResponse, ProjectListResponse, StartOpenAiOAuthRequest, TaskEventsResponse,
    TaskListResponse, TaskMutationResponse, TransitionTaskRequest,
};
use mcg_config::AppConfig;
use mcg_domain::TaskEnvelope;
use mcg_events::EventEnvelope;
use mcg_mcp::{McpConnection, McpToolCatalog};
use mcg_runtime::{McpMutation, OAuthLoginStart, ProjectMutation, ProviderMutation};
use serde::{Deserialize, Serialize};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;
use zeroize::Zeroize;

const LOCAL_DAEMON: &str = "http://127.0.0.1:7777";

#[derive(Deserialize, Serialize)]
struct ConfigureMcpCommand {
    connection: McpConnection,
    credential: Option<String>,
}

impl Drop for ConfigureMcpCommand {
    fn drop(&mut self) {
        if let Some(credential) = &mut self.credential {
            credential.zeroize();
        }
    }
}

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

#[tauri::command]
async fn list_mcp_connections() -> Result<McpListResponse, String> {
    let response = reqwest::Client::new()
        .get(format!("{LOCAL_DAEMON}/api/v1/mcp/connections"))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("MCP connections could not be loaded: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "MCP connection listing").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("MCP connection response was invalid: {error}"))
}

#[tauri::command]
async fn configure_mcp_connection(request: ConfigureMcpCommand) -> Result<McpMutation, String> {
    let response = reqwest::Client::new()
        .post(format!("{LOCAL_DAEMON}/api/v1/mcp/connections"))
        .bearer_auth(daemon_token()?)
        .json(&request)
        .send()
        .await
        .map_err(|error| format!("MCP connection could not be configured: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "MCP connection setup").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("MCP setup response was invalid: {error}"))
}

#[tauri::command]
async fn discover_mcp_connection(connection_id: String) -> Result<McpToolCatalog, String> {
    let response = reqwest::Client::new()
        .post(format!(
            "{LOCAL_DAEMON}/api/v1/mcp/connections/{connection_id}/discover"
        ))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("MCP discovery could not start: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "MCP discovery").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("MCP discovery response was invalid: {error}"))
}

#[tauri::command]
async fn list_tasks() -> Result<TaskListResponse, String> {
    let response = reqwest::Client::new()
        .get(format!("{LOCAL_DAEMON}/api/v1/tasks"))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("Tasks could not be loaded: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Task listing").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Task list response was invalid: {error}"))
}

#[tauri::command]
async fn create_task(request: CreateTaskRequest) -> Result<TaskMutationResponse, String> {
    let response = reqwest::Client::new()
        .post(format!("{LOCAL_DAEMON}/api/v1/tasks"))
        .bearer_auth(daemon_token()?)
        .json(&request)
        .send()
        .await
        .map_err(|error| format!("Task could not be created: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Task creation").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Task creation response was invalid: {error}"))
}

#[tauri::command]
async fn transition_task(
    task_id: String,
    request: TransitionTaskRequest,
) -> Result<TaskEnvelope, String> {
    let response = reqwest::Client::new()
        .post(format!("{LOCAL_DAEMON}/api/v1/tasks/{task_id}"))
        .bearer_auth(daemon_token()?)
        .json(&request)
        .send()
        .await
        .map_err(|error| format!("Task state could not be changed: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Task transition").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Task transition response was invalid: {error}"))
}

#[tauri::command]
async fn launch_task(
    task_id: String,
    request: LaunchTaskRequest,
) -> Result<LaunchTaskResponse, String> {
    let response = reqwest::Client::new()
        .post(format!("{LOCAL_DAEMON}/api/v1/tasks/{task_id}/launch"))
        .bearer_auth(daemon_token()?)
        .json(&request)
        .send()
        .await
        .map_err(|error| format!("Task could not be launched: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Task launch").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Task launch response was invalid: {error}"))
}

#[tauri::command]
async fn task_events(task_id: String) -> Result<TaskEventsResponse, String> {
    let response = reqwest::Client::new()
        .get(format!("{LOCAL_DAEMON}/api/v1/tasks/{task_id}/events"))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("Task history could not be loaded: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Task history").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Task history response was invalid: {error}"))
}

#[tauri::command]
async fn pick_project_directory(app: tauri::AppHandle) -> Result<Option<String>, String> {
    app.dialog()
        .file()
        .set_title("Choose a project folder")
        .blocking_pick_folder()
        .map(|path| {
            path.into_path()
                .map(|path| path.to_string_lossy().into_owned())
                .map_err(|_| "The selected folder is not a local filesystem path".to_owned())
        })
        .transpose()
}

#[tauri::command]
async fn discover_project(request: DiscoverProjectRequest) -> Result<ProjectMutation, String> {
    let response = reqwest::Client::new()
        .post(format!("{LOCAL_DAEMON}/api/v1/projects/discover"))
        .bearer_auth(daemon_token()?)
        .json(&request)
        .send()
        .await
        .map_err(|error| format!("Project discovery could not start: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Project discovery").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Project discovery response was invalid: {error}"))
}

#[tauri::command]
async fn list_projects() -> Result<ProjectListResponse, String> {
    let response = reqwest::Client::new()
        .get(format!("{LOCAL_DAEMON}/api/v1/projects"))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("Projects could not be loaded: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Project listing").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Project list response was invalid: {error}"))
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri deserializes owned command arguments.
fn open_external_url(url: String) -> Result<(), String> {
    let parsed = validate_external_url(&url)?;
    webbrowser::open(parsed.as_str())
        .map_err(|error| format!("The system browser could not be opened: {error}"))
}

fn validate_external_url(url: &str) -> Result<reqwest::Url, String> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|error| format!("The verification URL is invalid: {error}"))?;
    if parsed.scheme() != "https" || parsed.host_str() != Some("auth.openai.com") {
        return Err("Only the official OpenAI authorization site may be opened".into());
    }
    Ok(parsed)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Starts the native desktop application.
///
/// # Panics
///
/// Panics when Tauri cannot initialize or run the application event loop.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            runtime_label,
            start_openai_oauth,
            complete_openai_oauth,
            cancel_openai_oauth,
            refresh_openai_oauth,
            list_mcp_connections,
            configure_mcp_connection,
            discover_mcp_connection,
            list_tasks,
            create_task,
            launch_task,
            transition_task,
            task_events,
            pick_project_directory,
            discover_project,
            list_projects,
            open_external_url
        ])
        .run(tauri::generate_context!())
        .expect("error while running MCG desktop application");
}

#[cfg(test)]
mod tests {
    use super::validate_external_url;

    #[test]
    fn external_launcher_allows_only_official_openai_https() {
        assert!(validate_external_url("https://auth.openai.com/authorize?client_id=test").is_ok());
        assert!(validate_external_url("http://auth.openai.com/authorize").is_err());
        assert!(validate_external_url("https://auth.openai.com.evil.example/authorize").is_err());
        assert!(validate_external_url("file:///C:/secret").is_err());
    }
}
