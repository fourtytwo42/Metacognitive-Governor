use std::{
    fs,
    io::{Read, Write},
    net::{IpAddr, SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

use mcg_config::AppConfig;
use semver::Version;
use serde::{Deserialize, Serialize};

use super::portable_runtime;

const CONNECT_TIMEOUT: Duration = Duration::from_millis(250);
const STARTUP_TIMEOUT: Duration = Duration::from_secs(8);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
const SHUTDOWN_REQUEST_TIMEOUT: Duration = Duration::from_secs(1);
const RETRY_DELAY: Duration = Duration::from_millis(100);
const MAX_HEALTH_RESPONSE_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LocalServiceStatus {
    pub ready: bool,
    pub launched: bool,
    pub service: String,
    pub version: String,
    pub detail: String,
}

impl LocalServiceStatus {
    pub fn failed(detail: String) -> Self {
        Self {
            ready: false,
            launched: false,
            service: "mcg-daemon".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            detail,
        }
    }
}

#[derive(Debug, Deserialize)]
struct HealthDocument {
    status: String,
    service: String,
    version: String,
}

#[derive(Debug, Deserialize)]
struct RuntimeStatusDocument {
    service: String,
    version: String,
}

enum ProbeResult {
    Ready(HealthDocument),
    Unreachable,
}

/// Ensures the exact-version packaged daemon is available on the configured loopback endpoint.
///
/// # Errors
/// Returns an error for unsafe endpoints, foreign or incompatible services, missing packaged
/// binaries, process-launch failures, missing authentication state, or startup timeout.
pub fn ensure_local_service(endpoint: &str) -> Result<LocalServiceStatus, String> {
    let config = AppConfig::default();
    let address = endpoint_address(endpoint)?;
    match probe(address)? {
        ProbeResult::Ready(health) => {
            let token = authenticate_running_service(&health, &config, address)?;
            match compare_service_version(&health.version)? {
                std::cmp::Ordering::Equal => {
                    return Ok(ready_status(health, false));
                }
                std::cmp::Ordering::Less => {
                    request_authenticated_shutdown(address, &token)?;
                    wait_for_service_stop(address, SHUTDOWN_TIMEOUT).map_err(|error| {
                        format!(
                            "The authenticated MCG service version {} could not be replaced: {error}",
                            health.version
                        )
                    })?;
                }
                std::cmp::Ordering::Greater => {
                    return Err(format!(
                        "The running MCG service is version {}, which is newer than this desktop version {}. Open the newer MCG desktop instead.",
                        health.version,
                        env!("CARGO_PKG_VERSION")
                    ));
                }
            }
        }
        ProbeResult::Unreachable => {}
    }

    let executable = portable_runtime::packaged_executable(
        &config.data_directory,
        if cfg!(windows) {
            "mcg-service.exe"
        } else {
            "mcg-service"
        },
    )?
    .map_or_else(
        || {
            packaged_daemon_path(&std::env::current_exe().map_err(|error| {
                format!("The MCG desktop executable location could not be determined: {error}")
            })?)
        },
        Ok,
    )?;
    let codex_executable = portable_runtime::packaged_executable(
        &config.data_directory,
        "codex-runtime/bin/codex.exe",
    )?;
    let agent_mcp_executable = portable_runtime::packaged_executable(
        &config.data_directory,
        if cfg!(windows) {
            "mcg-agent-mcp.exe"
        } else {
            "mcg-agent-mcp"
        },
    )?;
    launch_daemon(
        &executable,
        address,
        &config,
        codex_executable.as_deref(),
        agent_mcp_executable.as_deref(),
    )?;

    let started = Instant::now();
    while started.elapsed() < STARTUP_TIMEOUT {
        thread::sleep(RETRY_DELAY);
        match probe(address)? {
            ProbeResult::Ready(health) => {
                authenticate_running_service(&health, &config, address)?;
                if compare_service_version(&health.version)? != std::cmp::Ordering::Equal {
                    return Err(format!(
                        "The packaged MCG service started as version {}, but desktop version {} was required.",
                        health.version,
                        env!("CARGO_PKG_VERSION")
                    ));
                }
                return Ok(ready_status(health, true));
            }
            ProbeResult::Unreachable => {}
        }
    }
    Err(format!(
        "The packaged MCG service did not become ready within {} seconds. Run {} directly for diagnostics.",
        STARTUP_TIMEOUT.as_secs(),
        executable.display()
    ))
}

fn authenticate_running_service(
    health: &HealthDocument,
    config: &AppConfig,
    address: SocketAddr,
) -> Result<String, String> {
    if health.status != "ok" || health.service != "mcg-daemon" {
        return Err("The loopback port answered with an invalid MCG health contract".into());
    }
    let token_path = config.auth_token_path();
    let token = fs::read_to_string(&token_path).map_err(|error| {
        format!(
            "The local MCG service is running, but its authentication token cannot be read at {}: {error}",
            token_path.display()
        )
    })?;
    if token.trim().is_empty() {
        return Err("The local MCG service authentication token is empty".into());
    }
    let token = token.trim();
    if token.len() > 4096
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~'))
    {
        return Err("The local MCG service authentication token is not header-safe".into());
    }
    let authenticated = probe_authenticated_status(address, token)?;
    if authenticated.service != health.service || authenticated.version != health.version {
        return Err(
            "The authenticated local MCG status does not match its public health identity".into(),
        );
    }
    Ok(token.to_owned())
}

fn ready_status(health: HealthDocument, launched: bool) -> LocalServiceStatus {
    LocalServiceStatus {
        ready: true,
        launched,
        service: health.service,
        version: health.version,
        detail: if launched {
            "The packaged local service started successfully.".into()
        } else {
            "The existing local service passed identity and version checks.".into()
        },
    }
}

fn compare_service_version(version: &str) -> Result<std::cmp::Ordering, String> {
    let running = Version::parse(version).map_err(|_| {
        format!("The authenticated MCG service reported an invalid version: {version}")
    })?;
    let desktop = Version::parse(env!("CARGO_PKG_VERSION"))
        .map_err(|_| "The MCG desktop has an invalid build version".to_owned())?;
    Ok(running.cmp(&desktop))
}

/// Stops the authenticated local service and waits briefly for its loopback listener to close.
///
/// # Errors
/// Returns an error when the endpoint is unsafe, the listener cannot prove its MCG identity, the
/// authenticated shutdown request is rejected, or the listener remains active past the timeout.
pub fn shutdown_local_service(endpoint: &str) -> Result<(), String> {
    let config = AppConfig::default();
    let address = endpoint_address(endpoint)?;
    let ProbeResult::Ready(health) = probe(address)? else {
        return Ok(());
    };
    let token = authenticate_running_service(&health, &config, address)?;
    request_authenticated_shutdown(address, &token)?;
    wait_for_service_stop(address, SHUTDOWN_TIMEOUT)
}

fn request_authenticated_shutdown(address: SocketAddr, token: &str) -> Result<(), String> {
    let mut stream = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT)
        .map_err(|error| format!("The local MCG shutdown request could not connect: {error}"))?;
    stream
        .set_read_timeout(Some(SHUTDOWN_REQUEST_TIMEOUT))
        .map_err(|error| format!("The local MCG shutdown timeout could not be set: {error}"))?;
    stream
        .set_write_timeout(Some(SHUTDOWN_REQUEST_TIMEOUT))
        .map_err(|error| format!("The local MCG shutdown timeout could not be set: {error}"))?;
    write!(
        stream,
        "POST /api/v1/system/shutdown HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {token}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )
    .map_err(|error| format!("The local MCG shutdown request failed: {error}"))?;
    let mut response = Vec::new();
    stream
        .take(MAX_HEALTH_RESPONSE_BYTES)
        .read_to_end(&mut response)
        .map_err(|error| format!("The local MCG shutdown response failed: {error}"))?;
    parse_success_response(&response, "shutdown")
}

fn parse_success_response(response: &[u8], operation: &str) -> Result<(), String> {
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| format!("The local MCG {operation} response was malformed"))?;
    let headers = std::str::from_utf8(&response[..header_end])
        .map_err(|_| format!("The local MCG {operation} response headers were not UTF-8"))?;
    let successful = headers.lines().next().is_some_and(|line| {
        line.split_whitespace()
            .nth(1)
            .and_then(|status| status.parse::<u16>().ok())
            .is_some_and(|status| (200..300).contains(&status))
    });
    if successful {
        Ok(())
    } else {
        Err(format!(
            "The authenticated local MCG {operation} request was rejected"
        ))
    }
}

fn wait_for_service_stop(address: SocketAddr, timeout: Duration) -> Result<(), String> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        thread::sleep(RETRY_DELAY);
        if TcpStream::connect_timeout(&address, CONNECT_TIMEOUT).is_err() {
            return Ok(());
        }
    }
    Err(format!(
        "The local MCG service did not stop within {} seconds",
        timeout.as_secs()
    ))
}

fn endpoint_address(endpoint: &str) -> Result<SocketAddr, String> {
    let url = reqwest::Url::parse(endpoint)
        .map_err(|error| format!("Invalid local MCG service endpoint: {error}"))?;
    let host = url
        .host_str()
        .and_then(|value| {
            value
                .trim_start_matches('[')
                .trim_end_matches(']')
                .parse::<IpAddr>()
                .ok()
        })
        .filter(IpAddr::is_loopback)
        .ok_or_else(|| {
            "The packaged service endpoint must use a numeric loopback address".to_owned()
        })?;
    let port = url
        .port()
        .ok_or_else(|| "The packaged service endpoint must include a port".to_owned())?;
    Ok(SocketAddr::new(host, port))
}

fn probe(address: SocketAddr) -> Result<ProbeResult, String> {
    let mut stream = match TcpStream::connect_timeout(&address, CONNECT_TIMEOUT) {
        Ok(stream) => stream,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionRefused
                    | std::io::ErrorKind::TimedOut
                    | std::io::ErrorKind::NotConnected
                    | std::io::ErrorKind::AddrNotAvailable
            ) =>
        {
            return Ok(ProbeResult::Unreachable);
        }
        Err(error) => {
            return Err(format!(
                "The local MCG service could not be probed: {error}"
            ));
        }
    };
    stream
        .set_read_timeout(Some(CONNECT_TIMEOUT))
        .map_err(|error| format!("The local MCG health timeout could not be set: {error}"))?;
    stream
        .set_write_timeout(Some(CONNECT_TIMEOUT))
        .map_err(|error| format!("The local MCG health timeout could not be set: {error}"))?;
    write!(
        stream,
        "GET /api/v1/system/health HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
    )
    .map_err(|error| format!("The local MCG health request could not be sent: {error}"))?;
    let mut response = Vec::new();
    stream
        .take(MAX_HEALTH_RESPONSE_BYTES)
        .read_to_end(&mut response)
        .map_err(|error| format!("The local MCG health response could not be read: {error}"))?;
    parse_health_response(&response).map(ProbeResult::Ready)
}

fn probe_authenticated_status(
    address: SocketAddr,
    token: &str,
) -> Result<RuntimeStatusDocument, String> {
    let mut stream = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT).map_err(|error| {
        format!("The authenticated local MCG status could not connect: {error}")
    })?;
    stream
        .set_read_timeout(Some(CONNECT_TIMEOUT))
        .map_err(|error| {
            format!("The authenticated local MCG timeout could not be set: {error}")
        })?;
    stream
        .set_write_timeout(Some(CONNECT_TIMEOUT))
        .map_err(|error| {
            format!("The authenticated local MCG timeout could not be set: {error}")
        })?;
    write!(
        stream,
        "GET /api/v1/system/status HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {token}\r\nConnection: close\r\n\r\n"
    )
    .map_err(|error| format!("The authenticated local MCG status request failed: {error}"))?;
    let mut response = Vec::new();
    stream
        .take(MAX_HEALTH_RESPONSE_BYTES)
        .read_to_end(&mut response)
        .map_err(|error| format!("The authenticated local MCG status response failed: {error}"))?;
    parse_authenticated_status_response(&response)
}

fn parse_health_response(response: &[u8]) -> Result<HealthDocument, String> {
    let separator = b"\r\n\r\n";
    let header_end = response
        .windows(separator.len())
        .position(|window| window == separator)
        .ok_or_else(|| "The loopback port returned a malformed HTTP response".to_owned())?;
    let headers = std::str::from_utf8(&response[..header_end])
        .map_err(|_| "The loopback port returned non-UTF-8 HTTP headers".to_owned())?;
    if !headers
        .lines()
        .next()
        .is_some_and(|line| line.starts_with("HTTP/1.1 200 ") || line.starts_with("HTTP/1.0 200 "))
    {
        return Err("The loopback port did not return a successful MCG health response".into());
    }
    serde_json::from_slice(&response[header_end + separator.len()..]).map_err(|error| {
        format!("The loopback port returned an invalid MCG health document: {error}")
    })
}

fn parse_authenticated_status_response(response: &[u8]) -> Result<RuntimeStatusDocument, String> {
    let separator = b"\r\n\r\n";
    let header_end = response
        .windows(separator.len())
        .position(|window| window == separator)
        .ok_or_else(|| "The authenticated local MCG status was malformed".to_owned())?;
    let headers = std::str::from_utf8(&response[..header_end])
        .map_err(|_| "The authenticated local MCG status headers were not UTF-8".to_owned())?;
    if !headers
        .lines()
        .next()
        .is_some_and(|line| line.starts_with("HTTP/1.1 200 ") || line.starts_with("HTTP/1.0 200 "))
    {
        return Err("The local MCG service rejected its installation authentication token".into());
    }
    serde_json::from_slice(&response[header_end + separator.len()..])
        .map_err(|error| format!("The authenticated local MCG status was invalid: {error}"))
}

fn packaged_daemon_path(current_executable: &Path) -> Result<PathBuf, String> {
    let parent = current_executable.parent().ok_or_else(|| {
        "The MCG desktop executable has no containing installation directory".to_owned()
    })?;
    let candidate = parent.join(if cfg!(windows) {
        "mcg-service.exe"
    } else {
        "mcg-service"
    });
    let metadata = fs::symlink_metadata(&candidate).map_err(|error| {
        format!(
            "The packaged local service is missing at {}: {error}. Repair or reinstall MCG.",
            candidate.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "The packaged local service at {} is not a regular installation file",
            candidate.display()
        ));
    }
    Ok(candidate)
}

fn launch_daemon(
    executable: &Path,
    address: SocketAddr,
    config: &AppConfig,
    codex_executable: Option<&Path>,
    agent_mcp_executable: Option<&Path>,
) -> Result<(), String> {
    let mut command = Command::new(executable);
    command
        .env("MCG_BIND_ADDRESS", address.to_string())
        .env("MCG_DATA_DIR", &config.data_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(codex_executable) = codex_executable {
        command.env("MCG_CODEX_APP_SERVER_EXECUTABLE", codex_executable);
    }
    if let Some(agent_mcp_executable) = agent_mcp_executable {
        command.env("MCG_AGENT_MCP_EXECUTABLE", agent_mcp_executable);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    spawn_reaped_process(command, executable).map(drop)
}

fn spawn_reaped_process(
    mut command: Command,
    executable: &Path,
) -> Result<Receiver<Result<ExitStatus, String>>, String> {
    let executable = executable.display().to_string();
    let (started_tx, started_rx) = mpsc::sync_channel(1);
    let (completed_tx, completed_rx) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("mcg-service-reaper".into())
        .spawn(move || match command.spawn() {
            Ok(mut child) => {
                if started_tx.send(Ok(())).is_err() {
                    let _ = child.kill();
                    let _ = child.wait();
                    return;
                }
                let result = child.wait().map_err(|error| {
                    format!("The packaged local MCG service could not be reaped: {error}")
                });
                let _ = completed_tx.send(result);
            }
            Err(error) => {
                let _ = started_tx.send(Err(format!(
                    "The packaged local MCG service could not be started from {executable}: {error}"
                )));
            }
        })
        .map_err(|error| {
            format!("The packaged local MCG service reaper could not start: {error}")
        })?;
    started_rx.recv().map_err(|_| {
        "The packaged local MCG service reaper stopped before reporting startup".to_owned()
    })??;
    Ok(completed_rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_versions_allow_only_equal_or_older_daemons() {
        assert_eq!(
            compare_service_version(env!("CARGO_PKG_VERSION")).expect("current version"),
            std::cmp::Ordering::Equal
        );
        assert_eq!(
            compare_service_version("0.0.0").expect("older version"),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            compare_service_version("999.0.0").expect("newer version"),
            std::cmp::Ordering::Greater
        );
        assert!(compare_service_version("not-a-version").is_err());
    }

    #[test]
    fn shutdown_request_is_authenticated_and_requires_success() {
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").expect("loopback fixture");
        let address = listener.local_addr().expect("fixture address");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("shutdown connection");
            let mut request = Vec::new();
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let mut chunk = [0_u8; 512];
                let size = stream.read(&mut chunk).expect("shutdown request");
                assert!(size > 0, "shutdown request ended before its headers");
                request.extend_from_slice(&chunk[..size]);
            }
            let request = std::str::from_utf8(&request).expect("HTTP request");
            assert!(request.starts_with("POST /api/v1/system/shutdown HTTP/1.1\r\n"));
            assert!(request.contains("\r\nAuthorization: Bearer test-token\r\n"));
            stream
                .write_all(b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\n\r\n")
                .expect("shutdown response");
        });
        request_authenticated_shutdown(address, "test-token").expect("accepted shutdown");
        server.join().expect("fixture server");

        assert!(parse_success_response(b"HTTP/1.1 401 Nope\r\n\r\n", "shutdown").is_err());
    }

    #[test]
    fn health_parser_requires_exact_successful_mcg_document() {
        let valid = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\n\r\n{{\"status\":\"ok\",\"service\":\"mcg-daemon\",\"version\":\"{}\"}}",
            env!("CARGO_PKG_VERSION")
        );
        let parsed = parse_health_response(valid.as_bytes()).expect("valid health response");
        assert_eq!(parsed.service, "mcg-daemon");
        assert!(parse_health_response(b"HTTP/1.1 404 Nope\r\n\r\n{}").is_err());
        assert!(parse_health_response(b"HTTP/1.1 200 OK\r\n\r\nnot-json").is_err());
        assert!(parse_health_response(b"HTTP/1.1 200 OK\n\n{}").is_err());

        let status = format!(
            "HTTP/1.1 200 OK\r\n\r\n{{\"service\":\"mcg-daemon\",\"version\":\"{}\"}}",
            env!("CARGO_PKG_VERSION")
        );
        let status =
            parse_authenticated_status_response(status.as_bytes()).expect("authenticated status");
        assert_eq!(status.service, "mcg-daemon");
        assert!(parse_authenticated_status_response(b"HTTP/1.1 401 Nope\r\n\r\n{}").is_err());
    }

    #[test]
    fn endpoint_and_packaged_binary_are_strictly_local() {
        assert_eq!(
            endpoint_address("http://127.0.0.1:7777").expect("loopback endpoint"),
            "127.0.0.1:7777".parse().expect("socket address")
        );
        assert!(endpoint_address("http://example.com:7777").is_err());
        assert!(endpoint_address("http://127.0.0.1").is_err());

        let directory = tempfile::tempdir().expect("temporary installation");
        let desktop = directory.path().join(if cfg!(windows) {
            "mcg-desktop.exe"
        } else {
            "mcg-desktop"
        });
        fs::write(&desktop, b"desktop").expect("desktop fixture");
        assert!(packaged_daemon_path(&desktop).is_err());
        let daemon = directory.path().join(if cfg!(windows) {
            "mcg-service.exe"
        } else {
            "mcg-service"
        });
        fs::write(&daemon, b"daemon").expect("daemon fixture");
        assert_eq!(
            packaged_daemon_path(&desktop).expect("packaged daemon"),
            daemon
        );
    }

    #[test]
    fn launched_service_process_is_waited_by_a_dedicated_reaper() {
        let mut command = if cfg!(windows) {
            let mut command = Command::new("cmd.exe");
            command.args(["/D", "/C", "exit", "0"]);
            command
        } else {
            let mut command = Command::new("sh");
            command.args(["-c", "exit 0"]);
            command
        };
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let completion =
            spawn_reaped_process(command, Path::new("reaper-test")).expect("launch reaped process");
        let status = completion
            .recv_timeout(Duration::from_secs(5))
            .expect("reaper completion")
            .expect("process wait");
        assert!(status.success());

        let missing = Command::new("mcg-service-that-must-not-exist");
        assert!(spawn_reaped_process(missing, Path::new("missing-service")).is_err());
    }
}
