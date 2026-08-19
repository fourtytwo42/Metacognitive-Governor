use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream},
    path::{Component, Path},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

const REQUEST_SCHEMA_VERSION: u16 = 3;
const SETUP_FAILURE_EXIT_CODE: i32 = 125;
const COMMAND_FAILURE_EXIT_CODE: i32 = 1;
const MAX_REQUEST_BYTES: u64 = 64 * 1024;
const MAX_COMMAND_BYTES: usize = 32 * 1024;
const MAX_ARGUMENTS: usize = 128;
const MAX_FILES: u64 = 50_000;
const MAX_TOTAL_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_FILE_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_DEPTH: usize = 32;
const MAX_PROBE_URL_BYTES: usize = 2 * 1024;
const MAX_HTTP_HEADER_BYTES: usize = 8 * 1024;
const SERVICE_STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
const HTTP_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(1);
const SERVICE_POLL_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RunnerRequest {
    schema_version: u16,
    source_root: String,
    workspace_root: String,
    executable: String,
    argv: Vec<String>,
    toolchain_root: Option<String>,
    probe: Option<RunnerProbe>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
enum RunnerProbe {
    HttpStatus {
        url: String,
        expected_status: Option<u16>,
    },
}

fn main() {
    let argument = std::env::args_os().nth(1);
    let exit_code = if argument.as_deref() == Some(std::ffi::OsStr::new("--probe")) {
        0
    } else {
        argument
            .ok_or("request path is required")
            .and_then(|path| load_request(Path::new(&path)))
            .and_then(|request| execute_request(&request, true))
            .unwrap_or(SETUP_FAILURE_EXIT_CODE)
    };
    std::process::exit(exit_code);
}

fn load_request(path: &Path) -> Result<RunnerRequest, &'static str> {
    let metadata = fs::symlink_metadata(path).map_err(|_| "inspect request")?;
    if path_is_link_or_reparse(path, &metadata) || !metadata.is_file() {
        return Err("request must be a regular file");
    }
    if metadata.len() == 0 || metadata.len() > MAX_REQUEST_BYTES {
        return Err("request size is invalid");
    }
    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).map_err(|_| "size")?);
    fs::File::open(path)
        .map_err(|_| "open request")?
        .take(MAX_REQUEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "read request")?;
    serde_json::from_slice(&bytes).map_err(|_| "parse request")
}

fn execute_request(
    request: &RunnerRequest,
    enforce_sandbox_paths: bool,
) -> Result<i32, &'static str> {
    validate_request(request, enforce_sandbox_paths)?;
    let source = Path::new(&request.source_root);
    let workspace = Path::new(&request.workspace_root);
    copy_workspace(source, workspace)?;
    if let Some(probe) = request.probe.as_ref() {
        execute_service_probe(request, workspace, probe)
    } else {
        let mut command = prepared_command(request, workspace)?;
        let status = command.status().map_err(|_| "spawn command")?;
        Ok(if status.success() {
            0
        } else {
            COMMAND_FAILURE_EXIT_CODE
        })
    }
}

fn prepared_command(request: &RunnerRequest, workspace: &Path) -> Result<Command, &'static str> {
    let mut command = Command::new(&request.executable);
    command
        .args(&request.argv)
        .current_dir(workspace)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    preserve_runtime_environment(&mut command, request.toolchain_root.as_deref())?;
    Ok(command)
}

fn execute_service_probe(
    request: &RunnerRequest,
    workspace: &Path,
    probe: &RunnerProbe,
) -> Result<i32, &'static str> {
    let RunnerProbe::HttpStatus {
        url,
        expected_status,
    } = probe;
    let target = LoopbackHttpTarget::parse(url)?;
    let mut command = prepared_command(request, workspace)?;
    command
        .env("HOST", target.service_host())
        .env("PORT", target.port().to_string());
    let mut child = command.spawn().map_err(|_| "spawn service command")?;
    let deadline = Instant::now() + SERVICE_STARTUP_TIMEOUT;
    let disposition = loop {
        if let Some(status) = target.request_status() {
            let passed = expected_status.map_or_else(
                || (200..300).contains(&status),
                |expected| status == expected,
            );
            break if passed { 0 } else { COMMAND_FAILURE_EXIT_CODE };
        }
        if child
            .try_wait()
            .map_err(|_| "inspect service command")?
            .is_some()
            || Instant::now() >= deadline
        {
            break COMMAND_FAILURE_EXIT_CODE;
        }
        thread::sleep(SERVICE_POLL_INTERVAL);
    };
    let _ = child.kill();
    let _ = child.wait();
    Ok(disposition)
}

fn validate_request(
    request: &RunnerRequest,
    enforce_sandbox_paths: bool,
) -> Result<(), &'static str> {
    if request.schema_version != REQUEST_SCHEMA_VERSION
        || request.executable.trim().is_empty()
        || request.executable.contains('\0')
        || request.argv.len() > MAX_ARGUMENTS
        || request.argv.iter().any(|argument| argument.contains('\0'))
    {
        return Err("request contract is invalid");
    }
    if let Some(toolchain_root) = &request.toolchain_root {
        let path = Path::new(toolchain_root);
        if toolchain_root.contains('\0')
            || !path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
            || (enforce_sandbox_paths && !toolchain_root_is_exact(toolchain_root))
        {
            return Err("toolchain root escaped the sandbox contract");
        }
    }
    if let Some(RunnerProbe::HttpStatus {
        url,
        expected_status,
    }) = &request.probe
    {
        LoopbackHttpTarget::parse(url)?;
        if expected_status.is_some_and(|status| !(100..=599).contains(&status)) {
            return Err("expected HTTP status is invalid");
        }
    }
    let command_bytes = request
        .argv
        .iter()
        .try_fold(request.executable.len(), |total, argument| {
            total
                .checked_add(argument.len())
                .and_then(|value| value.checked_add(1))
        })
        .ok_or("command size overflow")?;
    if command_bytes > MAX_COMMAND_BYTES {
        return Err("command is too large");
    }
    if enforce_sandbox_paths && !sandbox_paths_are_exact(request) {
        return Err("request paths escaped the sandbox contract");
    }
    for value in [&request.source_root, &request.workspace_root] {
        let path = Path::new(value);
        if !path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        {
            return Err("request paths must be absolute and normalized");
        }
    }
    Ok(())
}

fn sandbox_paths_are_exact(request: &RunnerRequest) -> bool {
    #[cfg(windows)]
    {
        request.source_root.eq_ignore_ascii_case(r"C:\mcg-source")
            && request
                .workspace_root
                .eq_ignore_ascii_case(r"C:\mcg-workspace")
    }
    #[cfg(not(windows))]
    {
        request.source_root == "/mcg-source" && request.workspace_root == "/tmp/mcg-workspace"
    }
}

fn toolchain_root_is_exact(toolchain_root: &str) -> bool {
    #[cfg(windows)]
    {
        toolchain_root.eq_ignore_ascii_case(r"C:\mcg-toolchain")
    }
    #[cfg(not(windows))]
    {
        toolchain_root == "/mcg-toolchain"
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoopbackHttpTarget {
    addresses: Vec<SocketAddr>,
    authority: String,
    request_target: String,
    service_host: &'static str,
    port: u16,
}

impl LoopbackHttpTarget {
    fn parse(url: &str) -> Result<Self, &'static str> {
        if url.is_empty()
            || url.len() > MAX_PROBE_URL_BYTES
            || url
                .bytes()
                .any(|byte| byte.is_ascii_control() || byte == b' ')
            || url.contains('#')
        {
            return Err("HTTP probe URL is invalid");
        }
        let remainder = url
            .strip_prefix("http://")
            .ok_or("HTTP probe must use loopback plaintext HTTP")?;
        let (authority, request_target) = remainder
            .split_once('/')
            .map_or((remainder, "/".to_owned()), |(authority, path)| {
                (authority, format!("/{path}"))
            });
        if authority.is_empty() || authority.contains('@') {
            return Err("HTTP probe authority is invalid");
        }
        let (host, port) = if let Some(port) = authority.strip_prefix("[::1]:") {
            ("::1", parse_port(port)?)
        } else {
            let (host, port) = authority
                .split_once(':')
                .ok_or("HTTP probe requires an explicit port")?;
            if port.contains(':') {
                return Err("HTTP probe authority is ambiguous");
            }
            (host, parse_port(port)?)
        };
        let (addresses, service_host) = match host.to_ascii_lowercase().as_str() {
            "127.0.0.1" => (
                vec![SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)],
                "127.0.0.1",
            ),
            "::1" => (
                vec![SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), port)],
                "::1",
            ),
            "localhost" => (
                vec![
                    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
                    SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), port),
                ],
                "127.0.0.1",
            ),
            _ => return Err("HTTP probe must target loopback"),
        };
        Ok(Self {
            addresses,
            authority: authority.to_owned(),
            request_target,
            service_host,
            port,
        })
    }

    fn service_host(&self) -> &'static str {
        self.service_host
    }

    const fn port(&self) -> u16 {
        self.port
    }

    fn request_status(&self) -> Option<u16> {
        self.addresses
            .iter()
            .find_map(|address| self.request_status_from(*address))
    }

    fn request_status_from(&self, address: SocketAddr) -> Option<u16> {
        let mut stream = TcpStream::connect_timeout(&address, HTTP_ATTEMPT_TIMEOUT).ok()?;
        stream.set_read_timeout(Some(HTTP_ATTEMPT_TIMEOUT)).ok()?;
        stream.set_write_timeout(Some(HTTP_ATTEMPT_TIMEOUT)).ok()?;
        write!(
            stream,
            "GET {} HTTP/1.1\r\nHost: {}\r\nAccept: */*\r\nConnection: close\r\n\r\n",
            self.request_target, self.authority
        )
        .ok()?;
        stream.flush().ok()?;
        let mut response = Vec::with_capacity(512);
        let mut buffer = [0_u8; 512];
        while response.len() < MAX_HTTP_HEADER_BYTES {
            let read = stream.read(&mut buffer).ok()?;
            if read == 0 {
                break;
            }
            response.extend_from_slice(&buffer[..read]);
            if response.windows(2).any(|window| window == b"\r\n") {
                break;
            }
        }
        let first_line_end = response.windows(2).position(|window| window == b"\r\n")?;
        let first_line = std::str::from_utf8(&response[..first_line_end]).ok()?;
        let mut fields = first_line.split_ascii_whitespace();
        let version = fields.next()?;
        let status = fields.next()?.parse::<u16>().ok()?;
        if !version.starts_with("HTTP/1.") || !(100..=599).contains(&status) {
            return None;
        }
        Some(status)
    }
}

fn parse_port(value: &str) -> Result<u16, &'static str> {
    value
        .parse::<u16>()
        .ok()
        .filter(|port| *port != 0)
        .ok_or("HTTP probe port is invalid")
}

fn copy_workspace(source: &Path, workspace: &Path) -> Result<(), &'static str> {
    let source = source.canonicalize().map_err(|_| "resolve source")?;
    let metadata = fs::symlink_metadata(&source).map_err(|_| "inspect source")?;
    if path_is_link_or_reparse(&source, &metadata) || !metadata.is_dir() {
        return Err("source must be a real directory");
    }
    if workspace.exists() {
        return Err("workspace destination already exists");
    }
    fs::create_dir(workspace).map_err(|_| "create workspace")?;
    let mut pending = vec![(source.clone(), workspace.to_path_buf(), 0_usize)];
    let mut files = 0_u64;
    let mut total_bytes = 0_u64;
    while let Some((from, to, depth)) = pending.pop() {
        if depth > MAX_DEPTH {
            return Err("source depth limit exceeded");
        }
        for entry in fs::read_dir(&from).map_err(|_| "read source")? {
            let entry = entry.map_err(|_| "read source entry")?;
            let name = entry.file_name();
            let source_path = entry.path();
            let destination_path = to.join(&name);
            let metadata =
                fs::symlink_metadata(&source_path).map_err(|_| "inspect source entry")?;
            if path_is_link_or_reparse(&source_path, &metadata) {
                return Err("source links and reparse points are forbidden");
            }
            if metadata.is_dir() {
                fs::create_dir(&destination_path).map_err(|_| "create destination directory")?;
                pending.push((source_path, destination_path, depth.saturating_add(1)));
            } else if metadata.is_file() {
                files = files.checked_add(1).ok_or("file count overflow")?;
                total_bytes = total_bytes
                    .checked_add(metadata.len())
                    .ok_or("source size overflow")?;
                if files > MAX_FILES
                    || metadata.len() > MAX_FILE_BYTES
                    || total_bytes > MAX_TOTAL_BYTES
                {
                    return Err("source file or byte limit exceeded");
                }
                copy_file(&source_path, &destination_path, metadata.len())?;
            } else {
                return Err("source contains a non-regular entry");
            }
        }
    }
    Ok(())
}

fn copy_file(source: &Path, destination: &Path, expected_bytes: u64) -> Result<(), &'static str> {
    let input = fs::File::open(source).map_err(|_| "open source file")?;
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)
        .map_err(|_| "create destination file")?;
    let copied = std::io::copy(&mut input.take(MAX_FILE_BYTES + 1), &mut output)
        .map_err(|_| "copy source file")?;
    if copied != expected_bytes || copied > MAX_FILE_BYTES {
        return Err("source changed while copying");
    }
    output.flush().map_err(|_| "flush destination file")?;
    Ok(())
}

fn path_is_link_or_reparse(_path: &Path, metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    false
}

fn preserve_runtime_environment(
    command: &mut Command,
    toolchain_root: Option<&str>,
) -> Result<(), &'static str> {
    let inherited_path = std::env::var_os("PATH").unwrap_or_default();
    if let Some(root) = toolchain_root {
        let root = Path::new(root);
        if !root.is_absolute()
            || root
                .components()
                .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        {
            return Err("toolchain root must be absolute and normalized");
        }
        let bin = if cfg!(windows) {
            root.to_path_buf()
        } else {
            root.join("bin")
        };
        let mut values = vec![bin];
        values.extend(std::env::split_paths(&inherited_path));
        let path = std::env::join_paths(values).map_err(|_| "compose toolchain PATH")?;
        command.env("PATH", path);
    } else {
        command.env("PATH", inherited_path);
    }
    for name in ["PATHEXT", "SYSTEMROOT", "WINDIR", "COMSPEC", "TEMP", "TMP"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spawn_http_fixture(
        request_line: &'static [u8],
        response: &'static [u8],
    ) -> (u16, thread::JoinHandle<()>) {
        let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("listener");
        let port = listener.local_addr().expect("address").port();
        let server = thread::spawn(move || {
            loop {
                let (mut stream, _) = listener.accept().expect("accept");
                stream
                    .set_read_timeout(Some(Duration::from_millis(100)))
                    .expect("read timeout");
                let mut request = [0_u8; 1024];
                let mut read = 0;
                while read < request.len()
                    && !request[..read].windows(2).any(|window| window == b"\r\n")
                {
                    match stream.read(&mut request[read..]) {
                        Ok(0) | Err(_) => break,
                        Ok(count) => read += count,
                    }
                }
                stream.write_all(response).expect("response");
                if request[..read].starts_with(request_line) {
                    break;
                }
            }
        });
        (port, server)
    }

    fn command(success: bool) -> (String, Vec<String>) {
        #[cfg(windows)]
        {
            (
                "cmd.exe".into(),
                vec![
                    "/D".into(),
                    "/S".into(),
                    "/C".into(),
                    if success { "exit 0" } else { "exit 9" }.into(),
                ],
            )
        }
        #[cfg(not(windows))]
        {
            (
                "/bin/sh".into(),
                vec![
                    "-c".into(),
                    if success { "exit 0" } else { "exit 9" }.into(),
                ],
            )
        }
    }

    #[test]
    fn copies_regular_workspace_and_normalizes_command_failure() {
        let source = tempfile::tempdir().expect("source");
        let destination_parent = tempfile::tempdir().expect("destination parent");
        fs::create_dir(source.path().join("nested")).expect("nested");
        fs::write(source.path().join("nested/file.txt"), b"fixture").expect("fixture");
        let destination = destination_parent.path().join("workspace");
        let (executable, argv) = command(false);
        let request = RunnerRequest {
            schema_version: REQUEST_SCHEMA_VERSION,
            source_root: source.path().to_string_lossy().into_owned(),
            workspace_root: destination.to_string_lossy().into_owned(),
            executable,
            argv,
            toolchain_root: None,
            probe: None,
        };
        assert_eq!(
            execute_request(&request, false),
            Ok(COMMAND_FAILURE_EXIT_CODE)
        );
        assert_eq!(
            fs::read(destination.join("nested/file.txt")).expect("copied file"),
            b"fixture"
        );
    }

    #[test]
    fn rejects_replay_oversize_and_links_before_process_execution() {
        let source = tempfile::tempdir().expect("source");
        let destination_parent = tempfile::tempdir().expect("destination parent");
        let destination = destination_parent.path().join("workspace");
        let (executable, argv) = command(true);
        let request = RunnerRequest {
            schema_version: REQUEST_SCHEMA_VERSION,
            source_root: source.path().to_string_lossy().into_owned(),
            workspace_root: destination.to_string_lossy().into_owned(),
            executable,
            argv,
            toolchain_root: None,
            probe: None,
        };
        assert_eq!(execute_request(&request, false), Ok(0));
        assert!(execute_request(&request, false).is_err());

        let oversized_source = tempfile::tempdir().expect("oversized source");
        let oversized =
            fs::File::create(oversized_source.path().join("oversized")).expect("oversized file");
        oversized
            .set_len(MAX_TOTAL_BYTES + 1)
            .expect("sparse length");
        let oversized_destination = destination_parent.path().join("oversized-workspace");
        assert!(copy_workspace(oversized_source.path(), &oversized_destination).is_err());

        let link_source = tempfile::tempdir().expect("link source");
        let outside = tempfile::tempdir().expect("outside");
        fs::write(outside.path().join("secret"), b"secret").expect("secret");
        let link = link_source.path().join("escape");
        #[cfg(unix)]
        let linked = std::os::unix::fs::symlink(outside.path().join("secret"), &link).is_ok();
        #[cfg(windows)]
        let linked =
            std::os::windows::fs::symlink_file(outside.path().join("secret"), &link).is_ok();
        if linked {
            assert!(
                copy_workspace(
                    link_source.path(),
                    &destination_parent.path().join("link-workspace")
                )
                .is_err()
            );
        }
    }

    #[test]
    fn rejects_untrusted_schema_paths_and_oversized_argument_vectors() {
        let mut request = RunnerRequest {
            schema_version: 0,
            source_root: r"C:\mcg-source".into(),
            workspace_root: r"C:\mcg-workspace".into(),
            executable: "tool.exe".into(),
            argv: Vec::new(),
            toolchain_root: None,
            probe: None,
        };
        assert!(validate_request(&request, true).is_err());
        request.schema_version = REQUEST_SCHEMA_VERSION;
        request.argv = vec!["argument".into(); MAX_ARGUMENTS + 1];
        assert!(validate_request(&request, false).is_err());
        request.argv.clear();
        request.executable = "x".repeat(MAX_COMMAND_BYTES + 1);
        assert!(validate_request(&request, false).is_err());
        request.executable = "node.exe".into();
        request.toolchain_root = Some(r"C:\different-toolchain".into());
        assert!(validate_request(&request, true).is_err());
        request.toolchain_root = Some(if cfg!(windows) {
            r"C:\mcg-toolchain".into()
        } else {
            "/mcg-toolchain".into()
        });
        if !cfg!(windows) {
            request.source_root = "/mcg-source".into();
            request.workspace_root = "/tmp/mcg-workspace".into();
        }
        assert!(validate_request(&request, false).is_ok());
        #[cfg(windows)]
        assert!(validate_request(&request, true).is_ok());
        request.toolchain_root = Some(r"C:\mcg-toolchain\..\escape".into());
        assert!(validate_request(&request, false).is_err());
    }

    #[test]
    fn loopback_http_probe_rejects_authority_escape_and_observes_status() {
        for rejected in [
            "https://127.0.0.1:8443/",
            "http://192.168.1.10:8080/",
            "http://user@127.0.0.1:8080/",
            "http://127.0.0.1/",
            "http://127.0.0.1:8080/#fragment",
            "http://127.0.0.1:8080/\r\nInjected: true",
        ] {
            assert!(LoopbackHttpTarget::parse(rejected).is_err(), "{rejected}");
        }

        let (port, server) = spawn_http_fixture(
            b"GET /health HTTP/1.1\r\n",
            b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n",
        );
        let target =
            LoopbackHttpTarget::parse(&format!("http://127.0.0.1:{port}/health")).expect("target");
        assert_eq!(target.request_status(), Some(204));
        server.join().expect("server");
    }

    #[test]
    fn service_probe_runs_beside_the_service_and_normalizes_status_failure() {
        fn exercise(expected_status: u16) -> i32 {
            let (port, server) = spawn_http_fixture(
                b"GET /ready HTTP/1.1\r\n",
                b"HTTP/1.1 201 Created\r\nContent-Length: 0\r\n\r\n",
            );
            let source = tempfile::tempdir().expect("source");
            let destination_parent = tempfile::tempdir().expect("destination parent");
            let destination = destination_parent.path().join("workspace");
            #[cfg(windows)]
            let (executable, argv) = (
                "powershell.exe".into(),
                vec![
                    "-NoProfile".into(),
                    "-Command".into(),
                    "Start-Sleep -Seconds 10".into(),
                ],
            );
            #[cfg(not(windows))]
            let (executable, argv) = ("/bin/sh".into(), vec!["-c".into(), "sleep 10".into()]);
            let request = RunnerRequest {
                schema_version: REQUEST_SCHEMA_VERSION,
                source_root: source.path().to_string_lossy().into_owned(),
                workspace_root: destination.to_string_lossy().into_owned(),
                executable,
                argv,
                toolchain_root: None,
                probe: Some(RunnerProbe::HttpStatus {
                    url: format!("http://127.0.0.1:{port}/ready"),
                    expected_status: Some(expected_status),
                }),
            };
            let result = execute_request(&request, false).expect("service probe");
            server.join().expect("server");
            result
        }

        assert_eq!(exercise(201), 0);
        assert_eq!(exercise(204), COMMAND_FAILURE_EXIT_CODE);
    }
}
