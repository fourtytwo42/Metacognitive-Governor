use std::{
    fs::OpenOptions,
    io::Write as _,
    process::{Command, Stdio},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use mcg_plugin_protocol::{PLUGIN_SCHEMA_VERSION, PluginClass, PluginManifest, WasiPluginLimits};
use mcg_plugin_runner::{
    PLUGIN_RUNNER_EXECUTABLE_HASH_DOMAIN, PluginRunnerClient, PluginRunnerClientError,
    PluginRunnerRequest, PluginRunnerResponse, PluginRunnerState, sha256_hex,
};
use serde_json::json;

#[test]
fn process_executes_one_digest_bound_request_and_exits() {
    let module = br#"
        (module
          (import "wasi_snapshot_preview1" "fd_write"
            (func $fd_write (param i32 i32 i32 i32) (result i32)))
          (memory 1)
          (export "memory" (memory 0))
          (data (i32.const 8) "{\22runner\22:true}")
          (func (export "_start")
            (i32.store (i32.const 0) (i32.const 8))
            (i32.store (i32.const 4) (i32.const 15))
            (drop (call $fd_write
              (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 24)))))
    "#;
    let request = PluginRunnerRequest {
        schema_version: PLUGIN_SCHEMA_VERSION,
        manifest: PluginManifest {
            schema_version: PLUGIN_SCHEMA_VERSION,
            id: "example.process".into(),
            display_name: "Process fixture".into(),
            version: "1.0.0".into(),
            class: PluginClass::SandboxedWasi,
            requested_capabilities: Vec::new(),
        },
        module_sha256: sha256_hex(module),
        module_base64: STANDARD.encode(module),
        input: json!({}),
        limits: WasiPluginLimits::default(),
    };
    let mut child = Command::new(env!("CARGO_BIN_EXE_mcg-plugin-runner"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn runner");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(&serde_json::to_vec(&request).expect("request"))
        .expect("write request");
    let output = child.wait_with_output().expect("runner output");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let response: PluginRunnerResponse =
        serde_json::from_slice(&output.stdout).expect("one JSON response");
    assert_eq!(response.state, PluginRunnerState::Succeeded);
    assert_eq!(
        response.result.expect("result").output,
        json!({"runner": true})
    );
}

#[tokio::test]
async fn bounded_parent_client_executes_and_validates_the_runner_contract() {
    let module = br#"
        (module
          (import "wasi_snapshot_preview1" "fd_write"
            (func $fd_write (param i32 i32 i32 i32) (result i32)))
          (memory 1)
          (export "memory" (memory 0))
          (data (i32.const 8) "{\22client\22:true}")
          (func (export "_start")
            (i32.store (i32.const 0) (i32.const 8))
            (i32.store (i32.const 4) (i32.const 15))
            (drop (call $fd_write
              (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 24)))))
    "#;
    let request = PluginRunnerRequest {
        schema_version: PLUGIN_SCHEMA_VERSION,
        manifest: PluginManifest {
            schema_version: PLUGIN_SCHEMA_VERSION,
            id: "example.client".into(),
            display_name: "Client fixture".into(),
            version: "1.0.0".into(),
            class: PluginClass::SandboxedWasi,
            requested_capabilities: Vec::new(),
        },
        module_sha256: sha256_hex(module),
        module_base64: STANDARD.encode(module),
        input: json!({}),
        limits: WasiPluginLimits::default(),
    };
    let client = PluginRunnerClient::with_executable(
        env!("CARGO_BIN_EXE_mcg-plugin-runner"),
        std::time::Duration::from_secs(10),
    )
    .expect("trusted runner path");

    let response = client.invoke(&request).await.expect("bounded invocation");

    assert_eq!(response.state, PluginRunnerState::Succeeded);
    assert_eq!(
        response.result.expect("result").output,
        json!({"client": true})
    );
}

#[tokio::test]
async fn parent_deadline_covers_request_pipe_and_child_lifetime() {
    let module = br#"(module (func (export "_start")))"#;
    let request = PluginRunnerRequest {
        schema_version: PLUGIN_SCHEMA_VERSION,
        manifest: PluginManifest {
            schema_version: PLUGIN_SCHEMA_VERSION,
            id: "example.timeout".into(),
            display_name: "Timeout fixture".into(),
            version: "1.0.0".into(),
            class: PluginClass::SandboxedWasi,
            requested_capabilities: Vec::new(),
        },
        module_sha256: sha256_hex(module),
        module_base64: STANDARD.encode(module),
        input: json!({}),
        limits: WasiPluginLimits::default(),
    };
    let client = PluginRunnerClient::with_executable(
        env!("CARGO_BIN_EXE_mcg-plugin-runner"),
        std::time::Duration::from_nanos(1),
    )
    .expect("trusted runner path");
    let started = std::time::Instant::now();

    let result = client.invoke(&request).await;

    assert!(matches!(result, Err(PluginRunnerClientError::Timeout)));
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
}

#[tokio::test]
async fn parent_rejects_runner_replacement_before_plugin_execution() {
    let temporary = tempfile::tempdir().expect("temporary runner directory");
    let runner_name = if cfg!(windows) {
        "mcg-plugin-runner.exe"
    } else {
        "mcg-plugin-runner"
    };
    let runner = temporary.path().join(runner_name);
    std::fs::copy(env!("CARGO_BIN_EXE_mcg-plugin-runner"), &runner)
        .expect("copy fixed plugin runner");
    let client = PluginRunnerClient::with_executable(&runner, std::time::Duration::from_secs(10))
        .expect("pin copied runner");
    assert_eq!(
        client.runner_source().content_hash.domain,
        PLUGIN_RUNNER_EXECUTABLE_HASH_DOMAIN
    );
    OpenOptions::new()
        .append(true)
        .open(&runner)
        .expect("open copied runner")
        .write_all(b"changed")
        .expect("replace runner identity");

    let module = br#"(module (func (export "_start")))"#;
    let request = PluginRunnerRequest {
        schema_version: PLUGIN_SCHEMA_VERSION,
        manifest: PluginManifest {
            schema_version: PLUGIN_SCHEMA_VERSION,
            id: "example.replaced".into(),
            display_name: "Replacement fixture".into(),
            version: "1.0.0".into(),
            class: PluginClass::SandboxedWasi,
            requested_capabilities: Vec::new(),
        },
        module_sha256: sha256_hex(module),
        module_base64: STANDARD.encode(module),
        input: json!({}),
        limits: WasiPluginLimits::default(),
    };
    assert!(matches!(
        client.invoke(&request).await,
        Err(PluginRunnerClientError::ExecutableChanged)
    ));
}
