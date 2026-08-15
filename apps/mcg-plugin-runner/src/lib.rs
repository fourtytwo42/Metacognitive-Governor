//! One-shot process contract for sandboxed plugin execution.

use std::fmt::Write as _;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use mcg_plugins::{
    PLUGIN_SCHEMA_VERSION, PluginManifest, PluginResult, WasiPluginExecutor, WasiPluginLimits,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const RUNNER_REQUEST_MAX_BYTES: usize = 32 * 1_024 * 1_024;
const RUNNER_DETAIL_MAX_BYTES: usize = 2_048;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginRunnerRequest {
    pub schema_version: u16,
    pub manifest: PluginManifest,
    pub module_sha256: String,
    pub module_base64: String,
    pub input: Value,
    #[serde(default)]
    pub limits: WasiPluginLimits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginRunnerState {
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginRunnerResponse {
    pub schema_version: u16,
    pub plugin_id: Option<String>,
    pub state: PluginRunnerState,
    pub result: Option<PluginResult>,
    pub error_code: Option<String>,
    pub detail: Option<String>,
}

impl PluginRunnerResponse {
    #[must_use]
    pub fn failed(
        plugin_id: Option<String>,
        error_code: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: PLUGIN_SCHEMA_VERSION,
            plugin_id,
            state: PluginRunnerState::Failed,
            result: None,
            error_code: Some(error_code.into()),
            detail: Some(bounded_detail(detail.into())),
        }
    }
}

/// Parses, verifies, and executes one complete runner request.
///
/// # Errors
/// This function represents failures in the returned versioned response rather
/// than exposing an unstructured error channel.
pub async fn execute_request(request_bytes: &[u8]) -> PluginRunnerResponse {
    if request_bytes.len() > RUNNER_REQUEST_MAX_BYTES {
        return PluginRunnerResponse::failed(
            None,
            "request_too_large",
            "runner request is too large",
        );
    }
    let Ok(request) = serde_json::from_slice::<PluginRunnerRequest>(request_bytes) else {
        return PluginRunnerResponse::failed(
            None,
            "invalid_request",
            "runner request is not one valid JSON document",
        );
    };
    let plugin_id = Some(request.manifest.id.clone());
    if request.schema_version != PLUGIN_SCHEMA_VERSION {
        return PluginRunnerResponse::failed(
            plugin_id,
            "unsupported_schema",
            "runner request schema is unsupported",
        );
    }
    let Ok(module) = STANDARD.decode(request.module_base64.as_bytes()) else {
        return PluginRunnerResponse::failed(
            plugin_id,
            "invalid_module_encoding",
            "plugin module is not canonical base64",
        );
    };
    if STANDARD.encode(&module) != request.module_base64 {
        return PluginRunnerResponse::failed(
            plugin_id,
            "invalid_module_encoding",
            "plugin module is not canonical base64",
        );
    }
    if request.module_sha256.len() != 64
        || !request
            .module_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || sha256_hex(&module) != request.module_sha256.to_ascii_lowercase()
    {
        return PluginRunnerResponse::failed(
            plugin_id,
            "module_digest_mismatch",
            "plugin module digest does not match its request",
        );
    }
    let executor = match WasiPluginExecutor::new(request.limits) {
        Ok(executor) => executor,
        Err(error) => {
            return PluginRunnerResponse::failed(plugin_id, "invalid_limits", error.to_string());
        }
    };
    match executor
        .invoke_json(&request.manifest, &module, &request.input)
        .await
    {
        Ok(result) => PluginRunnerResponse {
            schema_version: PLUGIN_SCHEMA_VERSION,
            plugin_id,
            state: PluginRunnerState::Succeeded,
            result: Some(result),
            error_code: None,
            detail: None,
        },
        Err(error) => {
            PluginRunnerResponse::failed(plugin_id, "plugin_execution_failed", error.to_string())
        }
    }
}

#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(64);
    for byte in digest {
        write!(&mut encoded, "{byte:02x}").expect("write to string");
    }
    encoded
}

fn bounded_detail(mut detail: String) -> String {
    if detail.len() <= RUNNER_DETAIL_MAX_BYTES {
        return detail;
    }
    let mut boundary = RUNNER_DETAIL_MAX_BYTES;
    while !detail.is_char_boundary(boundary) {
        boundary -= 1;
    }
    detail.truncate(boundary);
    detail
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use mcg_plugins::{PluginClass, PluginManifest};

    use super::*;

    fn request(module: &[u8]) -> PluginRunnerRequest {
        PluginRunnerRequest {
            schema_version: PLUGIN_SCHEMA_VERSION,
            manifest: PluginManifest {
                schema_version: PLUGIN_SCHEMA_VERSION,
                id: "example.runner".into(),
                display_name: "Runner fixture".into(),
                version: "1.0.0".into(),
                class: PluginClass::SandboxedWasi,
                requested_capabilities: Vec::new(),
            },
            module_sha256: sha256_hex(module),
            module_base64: STANDARD.encode(module),
            input: json!({}),
            limits: WasiPluginLimits::default(),
        }
    }

    #[tokio::test]
    async fn digest_mismatch_fails_before_execution() {
        let module = br#"(module (func (export "_start")))"#;
        let mut request = request(module);
        request.module_sha256 = "0".repeat(64);
        let response = execute_request(&serde_json::to_vec(&request).expect("request")).await;
        assert_eq!(response.state, PluginRunnerState::Failed);
        assert_eq!(
            response.error_code.as_deref(),
            Some("module_digest_mismatch")
        );
    }

    #[tokio::test]
    async fn malformed_and_oversized_requests_are_bounded() {
        let malformed = execute_request(b"not-json").await;
        assert_eq!(malformed.error_code.as_deref(), Some("invalid_request"));
        let oversized = execute_request(&vec![b'x'; RUNNER_REQUEST_MAX_BYTES + 1]).await;
        assert_eq!(oversized.error_code.as_deref(), Some("request_too_large"));
    }
}
