use std::path::{Path, PathBuf};

#[cfg(all(windows, feature = "single-file-portable"))]
use std::{
    fs::{self, OpenOptions},
    io::Write,
};

#[cfg(all(windows, feature = "single-file-portable"))]
use sha2::{Digest, Sha256};

#[cfg(all(windows, feature = "single-file-portable"))]
struct Payload {
    name: &'static str,
    bytes: &'static [u8],
}

#[cfg(all(windows, feature = "single-file-portable"))]
const PAYLOADS: &[Payload] = &[
    Payload {
        name: "mcg-service.exe",
        bytes: include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../target/release/mcg-protected-rootd.exe"
        )),
    },
    Payload {
        name: "mcg-agent-mcp.exe",
        bytes: include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../target/release/mcg-agent-mcp.exe"
        )),
    },
    Payload {
        name: "mcg-action-runner.exe",
        bytes: include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../target/release/mcg-action-runner.exe"
        )),
    },
    Payload {
        name: "mcg-process-runner.exe",
        bytes: include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../target/release/mcg-process-runner.exe"
        )),
    },
    Payload {
        name: "mcg-provider-runner.exe",
        bytes: include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../target/release/mcg-provider-runner.exe"
        )),
    },
    Payload {
        name: "mcg-runtime-worker.exe",
        bytes: include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../target/release/mcg-runtime-worker.exe"
        )),
    },
    Payload {
        name: "mcg-plugin-runner.exe",
        bytes: include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../target/release/mcg-plugin-runner.exe"
        )),
    },
    Payload {
        name: "mcg-shadow-runner.exe",
        bytes: include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../target/release/mcg-shadow-runner.exe"
        )),
    },
    Payload {
        name: "mcg-updater.exe",
        bytes: include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../target/release/mcg-updater.exe"
        )),
    },
    Payload {
        name: "codex-runtime/bin/codex.exe",
        bytes: include_bytes!(env!("MCG_CODEX_EXE")),
    },
    Payload {
        name: "codex-runtime/bin/codex-code-mode-host.exe",
        bytes: include_bytes!(env!("MCG_CODEX_CODE_MODE_HOST_EXE")),
    },
    Payload {
        name: "codex-runtime/codex-path/rg.exe",
        bytes: include_bytes!(env!("MCG_CODEX_RG_EXE")),
    },
    Payload {
        name: "codex-runtime/codex-resources/codex-command-runner.exe",
        bytes: include_bytes!(env!("MCG_CODEX_COMMAND_RUNNER_EXE")),
    },
    Payload {
        name: "codex-runtime/codex-resources/codex-windows-sandbox-setup.exe",
        bytes: include_bytes!(env!("MCG_CODEX_SANDBOX_SETUP_EXE")),
    },
    Payload {
        name: "codex-runtime/README.md",
        bytes: include_bytes!(env!("MCG_CODEX_README")),
    },
    Payload {
        name: "codex-runtime/Apache-2.0.txt",
        bytes: include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../LICENSES/Apache-2.0.txt"
        )),
    },
];

/// Returns the verified, private runtime copy of an executable embedded in a one-file build.
/// Ordinary development and installed builds return `None` and keep using adjacent binaries.
pub(crate) fn packaged_executable(
    data_directory: &Path,
    name: &str,
) -> Result<Option<PathBuf>, String> {
    #[cfg(all(windows, feature = "single-file-portable"))]
    {
        let requested = PAYLOADS
            .iter()
            .find(|payload| payload.name == name)
            .ok_or_else(|| format!("The embedded MetaCog runtime does not contain {name}"))?;
        let runtime_root = data_directory
            .join("runtime")
            .join(format!("v{}", env!("CARGO_PKG_VERSION")));
        ensure_private_directory(data_directory)?;
        ensure_private_directory(&data_directory.join("runtime"))?;
        ensure_private_directory(&runtime_root)?;
        let mut requested_path = None;
        for payload in PAYLOADS {
            let path = ensure_payload(&runtime_root, payload)?;
            if payload.name == requested.name {
                requested_path = Some(path);
            }
        }
        Ok(requested_path)
    }
    #[cfg(not(all(windows, feature = "single-file-portable")))]
    {
        let _ = (data_directory, name);
        Ok(None)
    }
}

#[cfg(all(windows, feature = "single-file-portable"))]
fn ensure_payload(runtime_root: &Path, payload: &Payload) -> Result<PathBuf, String> {
    let destination = runtime_root.join(payload.name);
    if let Some(parent) = destination.parent() {
        ensure_private_directory(parent)?;
    }
    if verified_file(&destination, payload.bytes)? {
        return Ok(destination);
    }
    if destination.exists() {
        fs::remove_file(&destination).map_err(|error| {
            format!(
                "The outdated private runtime helper at {} could not be replaced: {error}",
                destination.display()
            )
        })?;
    }
    let temporary = runtime_root.join(format!("{}.tmp-{}", payload.name, std::process::id()));
    if temporary.exists() {
        fs::remove_file(&temporary).map_err(|error| {
            format!(
                "The stale private runtime staging file at {} could not be removed: {error}",
                temporary.display()
            )
        })?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| format!("The embedded {} could not be staged: {error}", payload.name))?;
    file.write_all(payload.bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| {
            format!(
                "The embedded {} could not be written: {error}",
                payload.name
            )
        })?;
    drop(file);
    fs::rename(&temporary, &destination).map_err(|error| {
        format!(
            "The embedded {} could not be activated: {error}",
            payload.name
        )
    })?;
    if !verified_file(&destination, payload.bytes)? {
        return Err(format!(
            "The extracted {} failed its embedded integrity check",
            payload.name
        ));
    }
    Ok(destination)
}

#[cfg(all(windows, feature = "single-file-portable"))]
fn ensure_private_directory(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|error| {
        format!(
            "The private MetaCog runtime directory at {} could not be created: {error}",
            path.display()
        )
    })?;
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "The private MetaCog runtime directory at {} could not be inspected: {error}",
            path.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "The private MetaCog runtime path at {} is not a regular directory",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(all(windows, feature = "single-file-portable"))]
fn verified_file(path: &Path, expected: &[u8]) -> Result<bool, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(format!(
                "The private runtime helper at {} could not be inspected: {error}",
                path.display()
            ));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Ok(false);
    }
    let actual = fs::read(path).map_err(|error| {
        format!(
            "The private runtime helper at {} could not be verified: {error}",
            path.display()
        )
    })?;
    Ok(Sha256::digest(actual) == Sha256::digest(expected))
}

#[cfg(all(test, windows, feature = "single-file-portable"))]
mod tests {
    use super::*;

    #[test]
    fn embedded_runtime_is_extracted_and_repairs_tampering() {
        let directory = tempfile::tempdir().expect("runtime fixture");
        let first = packaged_executable(directory.path(), "mcg-service.exe")
            .expect("extract runtime")
            .expect("embedded path");
        assert!(verified_file(&first, PAYLOADS[0].bytes).expect("verify runtime"));
        fs::write(&first, b"tampered").expect("tamper fixture");
        let repaired = packaged_executable(directory.path(), "mcg-service.exe")
            .expect("repair runtime")
            .expect("embedded path");
        assert_eq!(repaired, first);
        assert!(verified_file(&repaired, PAYLOADS[0].bytes).expect("verify repaired runtime"));
        assert!(packaged_executable(directory.path(), "not-embedded.exe").is_err());
    }
}
