use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use chrono::Utc;
use futures_util::StreamExt;
use mcg_domain::ToolExecutionId;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue as ReqwestHeaderValue, LOCATION};
use reqwest::{Method, StatusCode, Url};
use sha2::{Digest, Sha256};

use crate::{
    ArtifactStore, CredentialResolver, FilesystemExecutor, HeaderValue, OutputRedactor,
    RequestHeader, SecretMaterial, TOOL_SCHEMA_VERSION, ToolError, ToolExecutor, ToolOutcome,
    ToolOutcomeStatus, ToolRequest, ToolRequestKind, VerificationEvidence,
};

const MAX_REDIRECTS: usize = 5;
const ABSOLUTE_MAX_BODY_BYTES: u64 = 1_073_741_824;

pub struct HttpExecutor {
    client: reqwest::Client,
    credentials: Arc<dyn CredentialResolver>,
    filesystem: Arc<FilesystemExecutor>,
    artifacts: ArtifactStore,
}

struct BufferedResponse {
    status: StatusCode,
    bytes: Vec<u8>,
    content_type: String,
    final_url: Url,
    truncated: bool,
}

impl HttpExecutor {
    /// Creates an HTTP executor with automatic redirects disabled.
    ///
    /// # Errors
    ///
    /// Returns an error if the TLS client or artifact store cannot be created.
    pub fn new(
        credentials: Arc<dyn CredentialResolver>,
        filesystem: Arc<FilesystemExecutor>,
        artifact_root: impl AsRef<std::path::Path>,
    ) -> Result<Self, ToolError> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| ToolError::Execution(format!("build HTTP client: {error}")))?;
        Ok(Self {
            client,
            credentials,
            filesystem,
            artifacts: ArtifactStore::open(artifact_root)?,
        })
    }

    async fn request(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
        parameters: RequestParameters<'_>,
    ) -> Result<ToolOutcome, ToolError> {
        let started_at = Utc::now();
        let method = Method::from_bytes(parameters.method.as_bytes())
            .map_err(|error| ToolError::Invalid(format!("invalid HTTP method: {error}")))?;
        let (headers, secrets) = self.resolve_headers(parameters.headers).await?;
        let response = self
            .send_bounded(
                method,
                parameters.url,
                headers,
                parameters.body,
                parameters.max_bytes,
                request.timeout_seconds,
            )
            .await?;
        let artifact = self
            .artifacts
            .capture(&response.bytes, response.content_type.clone())?;
        let redactor = OutputRedactor::new(secrets);
        let stdout = redactor.redact(&String::from_utf8_lossy(&response.bytes));
        let successful = response.status.is_success();
        Ok(ToolOutcome {
            schema_version: TOOL_SCHEMA_VERSION,
            execution_id,
            request_id: request.id,
            status: if successful {
                ToolOutcomeStatus::Succeeded
            } else {
                ToolOutcomeStatus::Failed
            },
            summary: format!(
                "HTTP {} from {}{}",
                response.status.as_u16(),
                safe_url(&response.final_url),
                if response.truncated {
                    "; body truncated"
                } else {
                    ""
                }
            ),
            exit_code: None,
            stdout: Some(stdout),
            stderr: None,
            artifacts: vec![artifact],
            verification: vec![VerificationEvidence {
                method: "http.status".into(),
                passed: successful,
                detail: response.status.as_u16().to_string(),
            }],
            started_at,
            finished_at: Utc::now(),
        })
    }

    async fn download(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
        parameters: DownloadParameters<'_>,
    ) -> Result<ToolOutcome, ToolError> {
        let started_at = Utc::now();
        let (headers, secrets) = self.resolve_headers(parameters.headers).await?;
        let response = self
            .send_bounded(
                Method::GET,
                parameters.url,
                headers,
                None,
                parameters.max_bytes,
                request.timeout_seconds,
            )
            .await?;
        if !response.status.is_success() {
            return Err(ToolError::Execution(format!(
                "download returned HTTP {}",
                response.status.as_u16()
            )));
        }
        if response.truncated {
            return Err(ToolError::Execution(
                "download exceeded its byte limit; no file was written".into(),
            ));
        }
        let observed_sha = format!("{:x}", Sha256::digest(&response.bytes));
        if let Some(expected) = parameters.expected_sha256 {
            validate_sha256(expected)?;
            if !observed_sha.eq_ignore_ascii_case(expected) {
                return Err(ToolError::Execution(format!(
                    "download hash mismatch: expected {expected}, observed {observed_sha}"
                )));
            }
        }
        let file_verification = self
            .filesystem
            .write_download(parameters.destination, &response.bytes)?;
        let artifact = self
            .artifacts
            .capture(&response.bytes, response.content_type)?;
        let redactor = OutputRedactor::new(secrets);
        Ok(ToolOutcome {
            schema_version: TOOL_SCHEMA_VERSION,
            execution_id,
            request_id: request.id,
            status: ToolOutcomeStatus::Succeeded,
            summary: "downloaded and verified file".into(),
            exit_code: None,
            stdout: None,
            stderr: None,
            artifacts: vec![artifact],
            verification: vec![
                VerificationEvidence {
                    method: "sha256.download".into(),
                    passed: true,
                    detail: redactor.redact(&observed_sha),
                },
                file_verification,
            ],
            started_at,
            finished_at: Utc::now(),
        })
    }

    async fn tcp_connect(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
        endpoint: &str,
    ) -> Result<ToolOutcome, ToolError> {
        let started_at = Utc::now();
        let parsed = parse_tcp_endpoint(endpoint)?;
        let host = parsed
            .host_str()
            .ok_or_else(|| ToolError::Invalid("TCP endpoint requires a host".into()))?;
        let port = parsed
            .port()
            .ok_or_else(|| ToolError::Invalid("TCP endpoint requires an explicit port".into()))?;
        tokio::time::timeout(
            Duration::from_secs(request.timeout_seconds),
            tokio::net::TcpStream::connect((host, port)),
        )
        .await
        .map_err(|_| ToolError::Execution("TCP connection timed out".into()))?
        .map_err(|error| ToolError::Execution(format!("TCP connection failed: {error}")))?;
        Ok(ToolOutcome {
            schema_version: TOOL_SCHEMA_VERSION,
            execution_id,
            request_id: request.id,
            status: ToolOutcomeStatus::Succeeded,
            summary: format!("TCP connection succeeded to {}", safe_tcp_endpoint(&parsed)),
            exit_code: None,
            stdout: None,
            stderr: None,
            artifacts: Vec::new(),
            verification: vec![VerificationEvidence {
                method: "tcp.connect".into(),
                passed: true,
                detail: safe_tcp_endpoint(&parsed),
            }],
            started_at,
            finished_at: Utc::now(),
        })
    }

    async fn resolve_headers(
        &self,
        headers: &[RequestHeader],
    ) -> Result<(HeaderMap, Vec<SecretMaterial>), ToolError> {
        let mut resolved = HeaderMap::new();
        let mut secrets = Vec::new();
        for header in headers {
            let name = HeaderName::from_bytes(header.name.as_bytes())
                .map_err(|error| ToolError::Invalid(format!("invalid header name: {error}")))?;
            validate_request_header(&name, &header.value)?;
            let value = match &header.value {
                HeaderValue::Literal(value) => {
                    ReqwestHeaderValue::from_str(value).map_err(|error| {
                        ToolError::Invalid(format!("invalid header value: {error}"))
                    })?
                }
                HeaderValue::Credential(reference) => {
                    let secret = self.credentials.resolve(reference).await?;
                    let value = ReqwestHeaderValue::from_str(secret.expose_to_executor()).map_err(
                        |_| ToolError::Invalid("credential is not a valid header value".into()),
                    )?;
                    secrets.push(secret);
                    value
                }
            };
            resolved.append(name, value);
        }
        Ok((resolved, secrets))
    }

    #[allow(clippy::too_many_arguments)]
    async fn send_bounded(
        &self,
        method: Method,
        url: &str,
        headers: HeaderMap,
        body: Option<&str>,
        max_bytes: u64,
        timeout_seconds: u64,
    ) -> Result<BufferedResponse, ToolError> {
        if max_bytes == 0 || max_bytes > ABSOLUTE_MAX_BODY_BYTES {
            return Err(ToolError::Invalid(format!(
                "HTTP body limit must be between 1 and {ABSOLUTE_MAX_BODY_BYTES} bytes"
            )));
        }
        let limit = usize::try_from(max_bytes)
            .map_err(|_| ToolError::Invalid("HTTP body limit is too large".into()))?;
        let initial = parse_http_url(url)?;
        let origin = initial.origin().ascii_serialization();
        let mut current = initial;
        for redirect_count in 0..=MAX_REDIRECTS {
            let mut builder = self
                .client
                .request(method.clone(), current.clone())
                .headers(headers.clone())
                .timeout(std::time::Duration::from_secs(timeout_seconds));
            if let Some(body) = body {
                builder = builder.body(body.to_owned());
            }
            let response = builder
                .send()
                .await
                .map_err(|error| ToolError::Execution(format!("HTTP request failed: {error}")))?;
            if response.status().is_redirection() {
                if redirect_count == MAX_REDIRECTS {
                    return Err(ToolError::Execution("HTTP redirect limit exceeded".into()));
                }
                let location = response
                    .headers()
                    .get(LOCATION)
                    .ok_or_else(|| ToolError::Execution("redirect omitted Location header".into()))?
                    .to_str()
                    .map_err(|_| {
                        ToolError::Execution("redirect Location is not valid text".into())
                    })?;
                let next = current.join(location).map_err(|error| {
                    ToolError::Execution(format!("invalid redirect URL: {error}"))
                })?;
                if next.origin().ascii_serialization() != origin {
                    return Err(ToolError::Denied(
                        "cross-origin redirects require a separate capability decision".into(),
                    ));
                }
                current = next;
                continue;
            }
            let status = response.status();
            let content_type = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("application/octet-stream")
                .to_owned();
            let mut bytes = Vec::new();
            let mut truncated = false;
            let mut stream = response.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk
                    .map_err(|error| ToolError::Execution(format!("read HTTP body: {error}")))?;
                let remaining = limit.saturating_sub(bytes.len());
                let retained = remaining.min(chunk.len());
                bytes.extend_from_slice(&chunk[..retained]);
                if retained < chunk.len() {
                    truncated = true;
                    break;
                }
                if bytes.len() == limit {
                    if stream.next().await.is_some() {
                        truncated = true;
                    }
                    break;
                }
            }
            return Ok(BufferedResponse {
                status,
                bytes,
                content_type,
                final_url: current,
                truncated,
            });
        }
        Err(ToolError::Execution("unreachable redirect state".into()))
    }
}

#[async_trait]
impl ToolExecutor for HttpExecutor {
    async fn execute(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
    ) -> Result<ToolOutcome, ToolError> {
        match &request.kind {
            ToolRequestKind::HttpRequest {
                method,
                url,
                headers,
                body,
                max_response_bytes,
            } => {
                self.request(
                    execution_id,
                    request,
                    RequestParameters {
                        method,
                        url,
                        headers,
                        body: body.as_deref(),
                        max_bytes: *max_response_bytes,
                    },
                )
                .await
            }
            ToolRequestKind::HttpDownload {
                url,
                destination,
                headers,
                max_bytes,
                expected_sha256,
            } => {
                self.download(
                    execution_id,
                    request,
                    DownloadParameters {
                        url,
                        destination,
                        headers,
                        max_bytes: *max_bytes,
                        expected_sha256: expected_sha256.as_deref(),
                    },
                )
                .await
            }
            ToolRequestKind::TcpConnect { endpoint } => {
                self.tcp_connect(execution_id, request, endpoint).await
            }
            _ => Err(ToolError::Invalid(
                "HTTP executor received an unsupported tool".into(),
            )),
        }
    }
}

fn parse_tcp_endpoint(endpoint: &str) -> Result<Url, ToolError> {
    let parsed = Url::parse(endpoint)
        .map_err(|error| ToolError::Invalid(format!("invalid TCP endpoint: {error}")))?;
    if parsed.scheme() != "tcp"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_none()
        || parsed.path() != ""
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(ToolError::Invalid(
            "TCP endpoint must be tcp://host:port without credentials, path, query, or fragment"
                .into(),
        ));
    }
    Ok(parsed)
}

fn safe_tcp_endpoint(endpoint: &Url) -> String {
    endpoint.to_string()
}

struct RequestParameters<'a> {
    method: &'a str,
    url: &'a str,
    headers: &'a [RequestHeader],
    body: Option<&'a str>,
    max_bytes: u64,
}

struct DownloadParameters<'a> {
    url: &'a str,
    destination: &'a str,
    headers: &'a [RequestHeader],
    max_bytes: u64,
    expected_sha256: Option<&'a str>,
}

fn parse_http_url(value: &str) -> Result<Url, ToolError> {
    let url = Url::parse(value)
        .map_err(|error| ToolError::Invalid(format!("invalid HTTP URL: {error}")))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(ToolError::Invalid(
            "only credential-free HTTP(S) URLs are supported".into(),
        ));
    }
    Ok(url)
}

fn validate_request_header(name: &HeaderName, value: &HeaderValue) -> Result<(), ToolError> {
    if matches!(
        name.as_str(),
        "connection" | "content-length" | "host" | "transfer-encoding"
    ) {
        return Err(ToolError::Denied(format!(
            "header {name} is controlled by the HTTP executor"
        )));
    }
    if matches!(
        name.as_str(),
        "authorization" | "cookie" | "proxy-authorization"
    ) && matches!(value, HeaderValue::Literal(_))
    {
        return Err(ToolError::Denied(format!(
            "sensitive header {name} must use a credential reference"
        )));
    }
    Ok(())
}

fn validate_sha256(value: &str) -> Result<(), ToolError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ToolError::Invalid(
            "expected_sha256 must contain exactly 64 hexadecimal characters".into(),
        ));
    }
    Ok(())
}

fn safe_url(url: &Url) -> String {
    let mut safe = url.clone();
    safe.set_query(None);
    safe.set_fragment(None);
    safe.to_string()
}

#[cfg(test)]
mod tests {
    use axum::Router;
    use axum::http::{HeaderMap as AxumHeaderMap, StatusCode as AxumStatusCode};
    use axum::response::Redirect;
    use axum::routing::get;
    use tempfile::tempdir;

    use super::*;
    use crate::{CredentialReference, RejectingCredentialResolver};

    async fn server() -> (String, tokio::task::JoinHandle<()>) {
        let app = Router::new()
            .route("/data", get(|| async { "download fixture" }))
            .route("/large", get(|| async { "x".repeat(256) }))
            .route(
                "/cross-origin",
                get(|| async { Redirect::temporary("http://example.com/elsewhere") }),
            )
            .route(
                "/auth",
                get(|headers: AxumHeaderMap| async move {
                    headers
                        .get("authorization")
                        .and_then(|value| value.to_str().ok())
                        .map_or((AxumStatusCode::BAD_REQUEST, "missing".into()), |value| {
                            (AxumStatusCode::OK, value.to_owned())
                        })
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let address = listener.local_addr().expect("address");
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("server");
        });
        (format!("http://{address}"), task)
    }

    fn executor(
        project: &std::path::Path,
        artifacts: &std::path::Path,
        credentials: Arc<dyn CredentialResolver>,
    ) -> HttpExecutor {
        let filesystem = Arc::new(
            FilesystemExecutor::open([project.to_path_buf()], artifacts).expect("filesystem"),
        );
        HttpExecutor::new(credentials, filesystem, artifacts).expect("http")
    }

    #[tokio::test]
    async fn request_is_bounded_and_artifact_backed() {
        let (base, server) = server().await;
        let project = tempdir().expect("project");
        let artifacts = tempdir().expect("artifacts");
        let request = ToolRequest::new(
            mcg_domain::TaskId::new(),
            "bounded request",
            ToolRequestKind::HttpRequest {
                method: "GET".into(),
                url: format!("{base}/large"),
                headers: Vec::new(),
                body: None,
                max_response_bytes: 16,
            },
        );
        let outcome = executor(
            project.path(),
            artifacts.path(),
            Arc::new(RejectingCredentialResolver),
        )
        .execute(ToolExecutionId::new(), &request)
        .await
        .expect("request");
        assert_eq!(outcome.stdout.as_deref().map(str::len), Some(16));
        assert!(outcome.summary.contains("truncated"));
        assert_eq!(outcome.artifacts.len(), 1);
        server.abort();
    }

    #[tokio::test]
    async fn tcp_connect_is_bounded_typed_and_credential_free() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let endpoint = format!("tcp://{}", listener.local_addr().expect("address"));
        let project = tempdir().expect("project");
        let artifacts = tempdir().expect("artifacts");
        let mut request = ToolRequest::new(
            mcg_domain::TaskId::new(),
            "probe TCP listener",
            ToolRequestKind::TcpConnect {
                endpoint: endpoint.clone(),
            },
        );
        request.timeout_seconds = 1;
        let outcome = executor(
            project.path(),
            artifacts.path(),
            Arc::new(RejectingCredentialResolver),
        )
        .execute(ToolExecutionId::new(), &request)
        .await
        .expect("TCP probe");
        assert_eq!(outcome.status, ToolOutcomeStatus::Succeeded);
        assert!(outcome.artifacts.is_empty());
        assert_eq!(outcome.stdout, None);
        assert_eq!(outcome.stderr, None);
        assert_eq!(
            outcome.verification,
            vec![VerificationEvidence {
                method: "tcp.connect".into(),
                passed: true,
                detail: endpoint,
            }]
        );
        for invalid in [
            "http://127.0.0.1:80",
            "tcp://user@127.0.0.1:80",
            "tcp://127.0.0.1",
            "tcp://127.0.0.1:80/path",
            "tcp://127.0.0.1:80?secret=value",
        ] {
            assert!(parse_tcp_endpoint(invalid).is_err(), "accepted {invalid}");
        }

        let closed = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("closed-port reservation");
        let closed_endpoint = format!("tcp://{}", closed.local_addr().expect("closed address"));
        drop(closed);
        let mut refused = ToolRequest::new(
            mcg_domain::TaskId::new(),
            "reject closed TCP port",
            ToolRequestKind::TcpConnect {
                endpoint: closed_endpoint,
            },
        );
        refused.timeout_seconds = 1;
        assert!(matches!(
            executor(
                project.path(),
                artifacts.path(),
                Arc::new(RejectingCredentialResolver),
            )
            .execute(ToolExecutionId::new(), &refused)
            .await,
            Err(ToolError::Execution(_))
        ));
    }

    #[tokio::test]
    async fn hash_mismatch_prevents_download_side_effect() {
        let (base, server) = server().await;
        let project = tempdir().expect("project");
        let artifacts = tempdir().expect("artifacts");
        let destination = project.path().join("download.txt");
        let request = ToolRequest::new(
            mcg_domain::TaskId::new(),
            "download",
            ToolRequestKind::HttpDownload {
                url: format!("{base}/data"),
                destination: destination.to_string_lossy().into(),
                headers: Vec::new(),
                max_bytes: 1_024,
                expected_sha256: Some("0".repeat(64)),
            },
        );
        assert!(
            executor(
                project.path(),
                artifacts.path(),
                Arc::new(RejectingCredentialResolver),
            )
            .execute(ToolExecutionId::new(), &request)
            .await
            .is_err()
        );
        assert!(!destination.exists());
        server.abort();
    }

    #[tokio::test]
    async fn cross_origin_redirect_is_denied() {
        let (base, server) = server().await;
        let project = tempdir().expect("project");
        let artifacts = tempdir().expect("artifacts");
        let request = ToolRequest::new(
            mcg_domain::TaskId::new(),
            "redirect",
            ToolRequestKind::HttpRequest {
                method: "GET".into(),
                url: format!("{base}/cross-origin"),
                headers: Vec::new(),
                body: None,
                max_response_bytes: 1_024,
            },
        );
        assert!(matches!(
            executor(
                project.path(),
                artifacts.path(),
                Arc::new(RejectingCredentialResolver),
            )
            .execute(ToolExecutionId::new(), &request)
            .await,
            Err(ToolError::Denied(_))
        ));
        server.abort();
    }

    struct FixtureResolver;

    #[async_trait]
    impl CredentialResolver for FixtureResolver {
        async fn resolve(
            &self,
            _reference: &CredentialReference,
        ) -> Result<SecretMaterial, ToolError> {
            SecretMaterial::new("Bearer credential-fixture")
        }
    }

    #[tokio::test]
    async fn credential_header_echo_is_redacted() {
        let (base, server) = server().await;
        let project = tempdir().expect("project");
        let artifacts = tempdir().expect("artifacts");
        let request = ToolRequest::new(
            mcg_domain::TaskId::new(),
            "auth",
            ToolRequestKind::HttpRequest {
                method: "GET".into(),
                url: format!("{base}/auth"),
                headers: vec![RequestHeader {
                    name: "Authorization".into(),
                    value: HeaderValue::Credential(CredentialReference {
                        provider: "fixture".into(),
                        key: "test".into(),
                    }),
                }],
                body: None,
                max_response_bytes: 1_024,
            },
        );
        let outcome = executor(project.path(), artifacts.path(), Arc::new(FixtureResolver))
            .execute(ToolExecutionId::new(), &request)
            .await
            .expect("request");
        assert_eq!(outcome.stdout.as_deref(), Some("[REDACTED]"));
        server.abort();
    }
}
