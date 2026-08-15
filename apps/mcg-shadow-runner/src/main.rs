use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Component, Path},
    process::{Command, Stdio},
};

use serde::{Deserialize, Serialize};

const REQUEST_SCHEMA_VERSION: u16 = 1;
const SETUP_FAILURE_EXIT_CODE: i32 = 125;
const COMMAND_FAILURE_EXIT_CODE: i32 = 1;
const MAX_REQUEST_BYTES: u64 = 64 * 1024;
const MAX_COMMAND_BYTES: usize = 32 * 1024;
const MAX_ARGUMENTS: usize = 128;
const MAX_FILES: u64 = 50_000;
const MAX_TOTAL_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_FILE_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_DEPTH: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RunnerRequest {
    schema_version: u16,
    source_root: String,
    workspace_root: String,
    executable: String,
    argv: Vec<String>,
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
    let mut command = Command::new(&request.executable);
    command
        .args(&request.argv)
        .current_dir(workspace)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    preserve_runtime_environment(&mut command);
    let status = command.status().map_err(|_| "spawn command")?;
    Ok(if status.success() {
        0
    } else {
        COMMAND_FAILURE_EXIT_CODE
    })
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
        let _ = request;
        false
    }
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

fn preserve_runtime_environment(command: &mut Command) {
    for name in [
        "PATH",
        "PATHEXT",
        "SYSTEMROOT",
        "WINDIR",
        "COMSPEC",
        "TEMP",
        "TMP",
    ] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        };
        assert!(validate_request(&request, true).is_err());
        request.schema_version = REQUEST_SCHEMA_VERSION;
        request.argv = vec!["argument".into(); MAX_ARGUMENTS + 1];
        assert!(validate_request(&request, false).is_err());
        request.argv.clear();
        request.executable = "x".repeat(MAX_COMMAND_BYTES + 1);
        assert!(validate_request(&request, false).is_err());
    }
}
