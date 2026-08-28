use std::{
    env, fs,
    path::{Path, PathBuf},
};

const PINNED_CODEX_VERSION: &str = "0.150.1";

fn main() {
    if env::var_os("CARGO_FEATURE_SINGLE_FILE_PORTABLE").is_some() {
        configure_codex_payload();
    }
    tauri_build::build();
}

fn configure_codex_payload() {
    let package_root = env::var_os("MCG_CODEX_PACKAGE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let appdata = env::var_os("APPDATA")
                .expect("APPDATA is required to locate the pinned Codex package");
            PathBuf::from(appdata).join("npm/node_modules/@openai/codex")
        });
    let package = fs::read_to_string(package_root.join("package.json"))
        .expect("the pinned @openai/codex package.json is required for a portable build");
    assert!(
        package.contains(&format!("\"version\": \"{PINNED_CODEX_VERSION}\"")),
        "portable packaging requires @openai/codex {PINNED_CODEX_VERSION}"
    );
    let vendor =
        package_root.join("node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc");
    register("MCG_CODEX_EXE", &vendor.join("bin/codex.exe"));
    register(
        "MCG_CODEX_CODE_MODE_HOST_EXE",
        &vendor.join("bin/codex-code-mode-host.exe"),
    );
    register("MCG_CODEX_RG_EXE", &vendor.join("codex-path/rg.exe"));
    register(
        "MCG_CODEX_COMMAND_RUNNER_EXE",
        &vendor.join("codex-resources/codex-command-runner.exe"),
    );
    register(
        "MCG_CODEX_SANDBOX_SETUP_EXE",
        &vendor.join("codex-resources/codex-windows-sandbox-setup.exe"),
    );
    register("MCG_CODEX_README", &package_root.join("README.md"));
}

fn register(name: &str, path: &Path) {
    let canonical = path.canonicalize().unwrap_or_else(|error| {
        panic!(
            "required Codex payload {} is unavailable: {error}",
            path.display()
        )
    });
    println!("cargo:rerun-if-changed={}", canonical.display());
    println!("cargo:rustc-env={name}={}", canonical.display());
}
