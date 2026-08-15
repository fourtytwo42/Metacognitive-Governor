use std::{
    fs,
    io::{Read, Write},
    net::{IpAddr, SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use mcg_config::AppConfig;
use serde::{Deserialize, Serialize};

const CONNECT_TIMEOUT: Duration = Duration::from_millis(250);
const STARTUP_TIMEOUT: Duration = Duration::from_secs(8);
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
        ProbeResult::Ready(health) => return ready_status(health, false, &config, address),
        ProbeResult::Unreachable => {}
    }

    let executable = packaged_daemon_path(&std::env::current_exe().map_err(|error| {
        format!("The MCG desktop executable location could not be determined: {error}")
    })?)?;
    launch_daemon(&executable, address, &config)?;

    let started = Instant::now();
    while started.elapsed() < STARTUP_TIMEOUT {
        thread::sleep(RETRY_DELAY);
        match probe(address)? {
            ProbeResult::Ready(health) => return ready_status(health, true, &config, address),
            ProbeResult::Unreachable => {}
        }
    }
    Err(format!(
        "The packaged MCG service did not become ready within {} seconds. Run {} directly for diagnostics.",
        STARTUP_TIMEOUT.as_secs(),
        executable.display()
    ))
}

fn ready_status(
    health: HealthDocument,
    launched: bool,
    config: &AppConfig,
    address: SocketAddr,
) -> Result<LocalServiceStatus, String> {
    if health.status != "ok" || health.service != "mcg-daemon" {
        return Err("The loopback port answered with an invalid MCG health contract".into());
    }
    if health.version != env!("CARGO_PKG_VERSION") {
        return Err(format!(
            "The running MCG service is version {}, but this desktop is version {}. Stop the old service and reopen MCG to finish the upgrade.",
            health.version,
            env!("CARGO_PKG_VERSION")
        ));
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
    Ok(LocalServiceStatus {
        ready: true,
        launched,
        service: health.service,
        version: health.version,
        detail: if launched {
            "The packaged local service started successfully.".into()
        } else {
            "The existing local service passed identity and version checks.".into()
        },
    })
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

fn launch_daemon(executable: &Path, address: SocketAddr, config: &AppConfig) -> Result<(), String> {
    let mut command = Command::new(executable);
    command
        .env("MCG_BIND_ADDRESS", address.to_string())
        .env("MCG_DATA_DIR", &config.data_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command.spawn().map(drop).map_err(|error| {
        format!(
            "The packaged local MCG service could not be started from {}: {error}",
            executable.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
