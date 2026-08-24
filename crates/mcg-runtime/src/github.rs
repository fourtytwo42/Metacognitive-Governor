use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use mcg_domain::ProjectId;
use mcg_providers::{ProviderCredential, ProviderCredentialStore, ProviderSecretSlot};
use reqwest::{
    Client, StatusCode, Url,
    header::{ACCEPT, AUTHORIZATION, USER_AGENT},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Mutex,
    time::timeout,
};
use uuid::Uuid;

use crate::RuntimeError;

const GITHUB_CREDENTIAL_ID: &str = "github-owner";
const GITHUB_AUTHORIZE_URL: &str = "https://github.com/login/oauth/authorize";
const GITHUB_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const GITHUB_API_URL: &str = "https://api.github.com/";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubAccount {
    pub login: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub html_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubConnectionStatus {
    pub configured: bool,
    pub connected: bool,
    pub account: Option<GitHubAccount>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubLoginStart {
    pub session_id: Uuid,
    pub authorization_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubRepository {
    pub id: u64,
    pub full_name: String,
    pub private: bool,
    pub html_url: String,
    pub clone_url: String,
    pub default_branch: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubRepositoryList {
    pub repositories: Vec<GitHubRepository>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateGitHubRepository {
    pub name: String,
    pub description: Option<String>,
    pub private: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkGitHubRepository {
    pub project_id: ProjectId,
    pub clone_url: String,
}

struct PendingLogin {
    listener: TcpListener,
    state: String,
    verifier: String,
    redirect_uri: String,
}

pub(crate) struct GitHubIntegration {
    client_id: Option<String>,
    profile_path: PathBuf,
    credentials: Arc<dyn ProviderCredentialStore>,
    http: Client,
    pending: Mutex<BTreeMap<Uuid, PendingLogin>>,
}

impl GitHubIntegration {
    pub(crate) fn new(
        data_directory: &Path,
        credentials: Arc<dyn ProviderCredentialStore>,
    ) -> Result<Self, RuntimeError> {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| RuntimeError::GitHub(error.to_string()))?;
        Ok(Self {
            client_id: std::env::var("MCG_GITHUB_CLIENT_ID")
                .ok()
                .or_else(|| option_env!("MCG_GITHUB_CLIENT_ID").map(str::to_owned))
                .filter(|value| !value.trim().is_empty()),
            profile_path: data_directory.join("github-account.json"),
            credentials,
            http,
            pending: Mutex::new(BTreeMap::new()),
        })
    }

    pub(crate) fn status(&self) -> Result<GitHubConnectionStatus, RuntimeError> {
        let account = if self.profile_path.is_file() {
            Some(
                serde_json::from_slice(&fs::read(&self.profile_path)?).map_err(|error| {
                    RuntimeError::GitHub(format!("stored GitHub account is invalid: {error}"))
                })?,
            )
        } else {
            None
        };
        let connected = self
            .credentials
            .load(GITHUB_CREDENTIAL_ID, ProviderSecretSlot::Primary)?
            .is_some();
        Ok(GitHubConnectionStatus {
            configured: self.client_id.is_some(),
            connected,
            account: if connected { account } else { None },
        })
    }

    pub(crate) async fn start_login(&self) -> Result<GitHubLoginStart, RuntimeError> {
        let client_id = self.client_id.as_ref().ok_or_else(|| RuntimeError::GitHub(
            "GitHub sign-in is not configured in this build. Set MCG_GITHUB_CLIENT_ID to the public client ID of the MCG GitHub App.".into()
        ))?;
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        let session_id = Uuid::new_v4();
        let state = random_secret();
        let verifier = format!("{}{}", random_secret(), random_secret());
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let redirect_uri = format!("http://127.0.0.1:{port}/callback");
        let mut url = Url::parse(GITHUB_AUTHORIZE_URL)
            .map_err(|error| RuntimeError::GitHub(error.to_string()))?;
        url.query_pairs_mut()
            .append_pair("client_id", client_id)
            .append_pair("redirect_uri", &redirect_uri)
            .append_pair("state", &state)
            .append_pair("code_challenge", &challenge)
            .append_pair("code_challenge_method", "S256");
        self.pending.lock().await.insert(
            session_id,
            PendingLogin {
                listener,
                state,
                verifier,
                redirect_uri,
            },
        );
        Ok(GitHubLoginStart {
            session_id,
            authorization_url: url.into(),
        })
    }

    pub(crate) async fn complete_login(
        &self,
        session_id: Uuid,
    ) -> Result<GitHubConnectionStatus, RuntimeError> {
        let pending = self
            .pending
            .lock()
            .await
            .remove(&session_id)
            .ok_or_else(|| {
                RuntimeError::GitHub(
                    "GitHub sign-in session was not found or already completed".into(),
                )
            })?;
        let (mut socket, _) = timeout(Duration::from_secs(180), pending.listener.accept())
            .await
            .map_err(|_| RuntimeError::GitHub("GitHub sign-in timed out".into()))??;
        let mut bytes = vec![0_u8; 16 * 1024];
        let read = socket.read(&mut bytes).await?;
        let request = String::from_utf8_lossy(&bytes[..read]);
        let target = request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .ok_or_else(|| RuntimeError::GitHub("GitHub callback was malformed".into()))?;
        let callback = Url::parse(&format!("http://127.0.0.1{target}"))
            .map_err(|error| RuntimeError::GitHub(error.to_string()))?;
        let query: BTreeMap<_, _> = callback.query_pairs().into_owned().collect();
        let returned_state = query.get("state").map(String::as_str).unwrap_or_default();
        if !bool::from(returned_state.as_bytes().ct_eq(pending.state.as_bytes())) {
            send_callback(&mut socket, 400, "GitHub sign-in state mismatch").await;
            return Err(RuntimeError::GitHub(
                "GitHub callback state mismatch".into(),
            ));
        }
        let code = query.get("code").ok_or_else(|| {
            RuntimeError::GitHub(
                query
                    .get("error_description")
                    .cloned()
                    .unwrap_or_else(|| "GitHub sign-in was not completed".into()),
            )
        })?;
        let token: TokenResponse = self
            .http
            .post(GITHUB_TOKEN_URL)
            .header(ACCEPT, "application/json")
            .form(&[
                ("client_id", self.client_id.as_deref().unwrap_or_default()),
                ("code", code),
                ("redirect_uri", &pending.redirect_uri),
                ("code_verifier", &pending.verifier),
            ])
            .send()
            .await
            .map_err(github_transport)?
            .error_for_status()
            .map_err(github_transport)?
            .json()
            .await
            .map_err(github_transport)?;
        let refresh_token = token.refresh_token;
        let credential = ProviderCredential::new(token.access_token)?;
        let account: GitHubAccount = self.authorized_get("user", &credential).await?;
        self.credentials.save(
            GITHUB_CREDENTIAL_ID,
            ProviderSecretSlot::Primary,
            &credential,
        )?;
        if let Some(refresh_token) = refresh_token {
            self.credentials.save(
                GITHUB_CREDENTIAL_ID,
                ProviderSecretSlot::OAuthRefreshToken,
                &ProviderCredential::new(refresh_token)?,
            )?;
        }
        write_private_json(&self.profile_path, &account)?;
        send_callback(
            &mut socket,
            200,
            "GitHub is connected to MCG. You can close this tab.",
        )
        .await;
        self.status()
    }

    pub(crate) fn disconnect(&self) -> Result<GitHubConnectionStatus, RuntimeError> {
        self.credentials
            .delete(GITHUB_CREDENTIAL_ID, ProviderSecretSlot::Primary)?;
        self.credentials
            .delete(GITHUB_CREDENTIAL_ID, ProviderSecretSlot::OAuthRefreshToken)?;
        match fs::remove_file(&self.profile_path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        self.status()
    }

    pub(crate) async fn repositories(&self) -> Result<GitHubRepositoryList, RuntimeError> {
        let credential = self.credential()?;
        let mut repositories: Vec<GitHubRepository> = self.authorized_get("user/repos?per_page=100&sort=updated&affiliation=owner,collaborator,organization_member", &credential).await?;
        repositories.sort_by(|left, right| left.full_name.cmp(&right.full_name));
        Ok(GitHubRepositoryList { repositories })
    }

    pub(crate) async fn create_repository(
        &self,
        request: CreateGitHubRepository,
    ) -> Result<GitHubRepository, RuntimeError> {
        let name = request.name.trim();
        if name.is_empty()
            || name.len() > 100
            || !name
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
        {
            return Err(RuntimeError::GitHub(
                "repository name must be 1-100 letters, digits, dots, dashes, or underscores"
                    .into(),
            ));
        }
        let credential = self.credential()?;
        let url = Url::parse(GITHUB_API_URL)
            .unwrap()
            .join("user/repos")
            .unwrap();
        let body = serde_json::json!({"name": name, "description": request.description, "private": request.private, "auto_init": false});
        let mut response = self
            .http
            .post(url)
            .header(USER_AGENT, "MCG")
            .header(ACCEPT, "application/vnd.github+json")
            .header(
                AUTHORIZATION,
                format!("Bearer {}", credential.expose_to_host_executor()),
            )
            .json(&body)
            .send()
            .await
            .map_err(github_transport)?;
        if response.status() == StatusCode::UNAUTHORIZED {
            let refreshed = self.refresh_credential().await?;
            response = self
                .http
                .post(
                    Url::parse(GITHUB_API_URL)
                        .unwrap()
                        .join("user/repos")
                        .unwrap(),
                )
                .header(USER_AGENT, "MCG")
                .header(ACCEPT, "application/vnd.github+json")
                .header(
                    AUTHORIZATION,
                    format!("Bearer {}", refreshed.expose_to_host_executor()),
                )
                .json(&body)
                .send()
                .await
                .map_err(github_transport)?;
        }
        response
            .error_for_status()
            .map_err(github_transport)?
            .json()
            .await
            .map_err(github_transport)
    }

    pub(crate) fn link_repository(&self, root: &Path, clone_url: &str) -> Result<(), RuntimeError> {
        let root = fs::canonicalize(root)?;
        let clone_url = Url::parse(clone_url.trim())
            .map_err(|error| RuntimeError::GitHub(error.to_string()))?;
        if clone_url.scheme() != "https" || clone_url.host_str() != Some("github.com") {
            return Err(RuntimeError::GitHub(
                "GitHub remotes must use an https://github.com URL".into(),
            ));
        }
        let inside = Command::new("git")
            .args([
                "-C",
                root.to_string_lossy().as_ref(),
                "rev-parse",
                "--show-toplevel",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        if !inside.success() {
            return Err(RuntimeError::GitHub(
                "project must be a local Git repository before it can be linked".into(),
            ));
        }
        let existing = Command::new("git")
            .args([
                "-C",
                root.to_string_lossy().as_ref(),
                "remote",
                "get-url",
                "origin",
            ])
            .output()?;
        if existing.status.success() {
            let current = String::from_utf8_lossy(&existing.stdout).trim().to_owned();
            if current == clone_url.as_str() {
                return Ok(());
            }
            return Err(RuntimeError::GitHub(format!(
                "project already has an origin remote ({current}); MCG will not replace it automatically"
            )));
        }
        let added = Command::new("git")
            .args([
                "-C",
                root.to_string_lossy().as_ref(),
                "remote",
                "add",
                "origin",
                clone_url.as_str(),
            ])
            .output()?;
        if !added.status.success() {
            return Err(RuntimeError::GitHub(format!(
                "GitHub remote could not be added: {}",
                String::from_utf8_lossy(&added.stderr).trim()
            )));
        }
        Ok(())
    }

    fn credential(&self) -> Result<ProviderCredential, RuntimeError> {
        self.credentials
            .load(GITHUB_CREDENTIAL_ID, ProviderSecretSlot::Primary)?
            .ok_or_else(|| RuntimeError::GitHub("GitHub is not connected".into()))
    }

    async fn authorized_get<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        credential: &ProviderCredential,
    ) -> Result<T, RuntimeError> {
        let url = Url::parse(GITHUB_API_URL)
            .unwrap()
            .join(path)
            .map_err(|error| RuntimeError::GitHub(error.to_string()))?;
        let mut response = self
            .http
            .get(url)
            .header(USER_AGENT, "MCG")
            .header(ACCEPT, "application/vnd.github+json")
            .header(
                AUTHORIZATION,
                format!("Bearer {}", credential.expose_to_host_executor()),
            )
            .send()
            .await
            .map_err(github_transport)?;
        if response.status() == StatusCode::UNAUTHORIZED {
            let refreshed = self.refresh_credential().await?;
            let retry_url = Url::parse(GITHUB_API_URL)
                .unwrap()
                .join(path)
                .map_err(|error| RuntimeError::GitHub(error.to_string()))?;
            response = self
                .http
                .get(retry_url)
                .header(USER_AGENT, "MCG")
                .header(ACCEPT, "application/vnd.github+json")
                .header(
                    AUTHORIZATION,
                    format!("Bearer {}", refreshed.expose_to_host_executor()),
                )
                .send()
                .await
                .map_err(github_transport)?;
        }
        response
            .error_for_status()
            .map_err(github_transport)?
            .json()
            .await
            .map_err(github_transport)
    }

    async fn refresh_credential(&self) -> Result<ProviderCredential, RuntimeError> {
        let refresh = self
            .credentials
            .load(GITHUB_CREDENTIAL_ID, ProviderSecretSlot::OAuthRefreshToken)?
            .ok_or_else(|| {
                RuntimeError::GitHub("GitHub authorization expired; reconnect from Settings".into())
            })?;
        let token: TokenResponse = self
            .http
            .post(GITHUB_TOKEN_URL)
            .header(ACCEPT, "application/json")
            .form(&[
                ("client_id", self.client_id.as_deref().unwrap_or_default()),
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh.expose_to_host_executor()),
            ])
            .send()
            .await
            .map_err(github_transport)?
            .error_for_status()
            .map_err(github_transport)?
            .json()
            .await
            .map_err(github_transport)?;
        let access = ProviderCredential::new(token.access_token)?;
        self.credentials
            .save(GITHUB_CREDENTIAL_ID, ProviderSecretSlot::Primary, &access)?;
        if let Some(refresh_token) = token.refresh_token {
            self.credentials.save(
                GITHUB_CREDENTIAL_ID,
                ProviderSecretSlot::OAuthRefreshToken,
                &ProviderCredential::new(refresh_token)?,
            )?;
        }
        Ok(access)
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
}

fn random_secret() -> String {
    Uuid::new_v4().simple().to_string()
}
fn github_transport(error: reqwest::Error) -> RuntimeError {
    RuntimeError::GitHub(format!("GitHub request failed: {error}"))
}

async fn send_callback(socket: &mut tokio::net::TcpStream, status: u16, message: &str) {
    let body =
        format!("<!doctype html><meta charset=utf-8><title>MCG GitHub</title><p>{message}</p>");
    let response = format!(
        "HTTP/1.1 {status} OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = socket.write_all(response.as_bytes()).await;
}

fn write_private_json(path: &Path, value: &impl Serialize) -> Result<(), RuntimeError> {
    let temporary = path.with_extension("json.tmp");
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(value)
            .map_err(|error| RuntimeError::GitHub(error.to_string()))?,
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))?;
    }
    fs::rename(temporary, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use mcg_providers::MemoryProviderCredentialStore;

    use super::*;

    fn integration(root: &Path) -> GitHubIntegration {
        GitHubIntegration::new(root, Arc::new(MemoryProviderCredentialStore::default()))
            .expect("integration")
    }

    #[test]
    fn repository_link_requires_github_https_and_preserves_existing_origin() {
        let project = tempfile::tempdir().expect("project");
        let initialized = Command::new("git")
            .args(["-C", project.path().to_string_lossy().as_ref(), "init"])
            .output()
            .expect("git init");
        assert!(initialized.status.success());
        let github = integration(project.path());

        assert!(
            github
                .link_repository(project.path(), "https://example.com/owner/repo.git")
                .is_err()
        );
        github
            .link_repository(project.path(), "https://github.com/owner/first.git")
            .expect("first origin");
        assert!(
            github
                .link_repository(project.path(), "https://github.com/owner/second.git")
                .is_err()
        );

        let origin = Command::new("git")
            .args([
                "-C",
                project.path().to_string_lossy().as_ref(),
                "remote",
                "get-url",
                "origin",
            ])
            .output()
            .expect("origin");
        assert_eq!(
            String::from_utf8_lossy(&origin.stdout).trim(),
            "https://github.com/owner/first.git"
        );
    }
}
