use std::{
    io::Write as _,
    process::{Command, Stdio},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use mcg_plugin_runner::{PluginRunnerRequest, PluginRunnerResponse, PluginRunnerState, sha256_hex};
use mcg_plugins::{PLUGIN_SCHEMA_VERSION, PluginClass, PluginManifest, WasiPluginLimits};
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
