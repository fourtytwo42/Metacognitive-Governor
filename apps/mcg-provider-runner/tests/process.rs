use std::process::Command;

#[test]
fn packaged_qualification_is_bounded_and_content_free() {
    let output = Command::new(env!("CARGO_BIN_EXE_mcg-provider-runner"))
        .arg("--qualify-provider-runner")
        .output()
        .expect("launch provider runner qualification");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("qualification JSON");
    assert_eq!(report["protocol_version"], 1);
    assert_eq!(report["role"], "provider_adapter");
    assert_eq!(report["storage_linked"], false);
    assert_eq!(report["credential_store"], "operating_system");
    assert_eq!(report["physical_retry_attempts"], 1);
}

#[test]
fn dependency_tree_excludes_root_storage_runtime_and_tool_authority() {
    let output = Command::new(env!("CARGO"))
        .args([
            "tree",
            "-p",
            "mcg-provider-runner",
            "--edges",
            "normal",
            "--prefix",
            "none",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("provider runner dependency tree");
    assert!(output.status.success());
    let tree = String::from_utf8(output.stdout).expect("dependency tree text");
    for forbidden in [
        "mcg-storage",
        "mcg-protected-root",
        "mcg-runtime ",
        "mcg-tools",
        "mcg-mcp",
        "mcg-api",
        "rusqlite",
        "chromiumoxide",
    ] {
        assert!(
            !tree.contains(forbidden),
            "forbidden dependency: {forbidden}"
        );
    }
}
