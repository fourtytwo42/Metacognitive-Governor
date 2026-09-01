//! Lightweight, versioned protocol shared by the daemon and isolated plugin runner.

use mcg_browser_contracts::BrowserWorkflowDocument;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PLUGIN_CAPABILITY_TURN_LIMIT: usize = 8;

pub const PLUGIN_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginClass {
    TrustedNative,
    SandboxedWasi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginCapability {
    FilesystemRead,
    FilesystemWrite,
    Network,
    Process,
    Credentials,
    Mcp,
    Browser,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginCredentialReference {
    pub provider: String,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", content = "value", rename_all = "snake_case")]
pub enum PluginValueSource {
    Literal(String),
    Credential(PluginCredentialReference),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginHeader {
    pub name: String,
    #[serde(flatten)]
    pub value: PluginValueSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginEnvironmentVariable {
    pub name: String,
    #[serde(flatten)]
    pub value: PluginValueSource,
}

/// Bounded operations that remain requests until the daemon's normal policy
/// broker authorizes and executes them. The WASI guest receives no authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum PluginCapabilityOperation {
    FilesystemRead {
        path: String,
        max_bytes: u64,
    },
    FilesystemWrite {
        path: String,
        content: String,
        create_new: bool,
    },
    HttpRequest {
        method: String,
        url: String,
        #[serde(default)]
        headers: Vec<PluginHeader>,
        body: Option<String>,
        max_response_bytes: u64,
    },
    ProcessExec {
        executable: String,
        #[serde(default)]
        argv: Vec<String>,
        cwd: String,
        #[serde(default)]
        env: Vec<PluginEnvironmentVariable>,
        stdin: Option<String>,
    },
    McpCall {
        connection_id: String,
        tool_name: String,
        arguments: Value,
        max_response_bytes: u64,
    },
    BrowserWorkflow {
        document: BrowserWorkflowDocument,
        viewport_width: u32,
        viewport_height: u32,
        timeout_ms: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginCapabilityRequest {
    pub schema_version: u16,
    pub id: String,
    pub operation: PluginCapabilityOperation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum PluginCapabilityOutcome {
    Succeeded {
        request_id: String,
        outcome: Value,
    },
    Failed {
        request_id: String,
        error_code: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginHostTurn {
    pub schema_version: u16,
    pub original_input: Value,
    #[serde(default)]
    pub outcomes: Vec<PluginCapabilityOutcome>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PluginGuestTurn {
    Completed { output: Value },
    CapabilityRequest { request: PluginCapabilityRequest },
}

impl PluginCapabilityOperation {
    #[must_use]
    pub fn required_capabilities(&self) -> Vec<PluginCapability> {
        let mut capabilities = vec![match self {
            Self::FilesystemRead { .. } => PluginCapability::FilesystemRead,
            Self::FilesystemWrite { .. } => PluginCapability::FilesystemWrite,
            Self::HttpRequest { .. } => PluginCapability::Network,
            Self::ProcessExec { .. } => PluginCapability::Process,
            Self::McpCall { .. } => PluginCapability::Mcp,
            Self::BrowserWorkflow { .. } => PluginCapability::Browser,
        }];
        let uses_credentials = match self {
            Self::HttpRequest { headers, .. } => headers
                .iter()
                .any(|header| matches!(header.value, PluginValueSource::Credential(_))),
            Self::ProcessExec { env, .. } => env
                .iter()
                .any(|variable| matches!(variable.value, PluginValueSource::Credential(_))),
            Self::FilesystemRead { .. }
            | Self::FilesystemWrite { .. }
            | Self::McpCall { .. }
            | Self::BrowserWorkflow { .. } => false,
        };
        if uses_credentials {
            capabilities.push(PluginCapability::Credentials);
        }
        capabilities
    }

    /// Validates operation-local size and structured-argument bounds.
    ///
    /// # Errors
    /// Rejects empty, excessive, control-bearing, or ambiguous operation data.
    pub fn validate(&self) -> Result<(), PluginError> {
        match self {
            Self::FilesystemRead { path, max_bytes } => {
                bounded_text(path, 4_096)?;
                if *max_bytes == 0 || *max_bytes > 8 * 1_024 * 1_024 {
                    return Err(PluginError::Manifest(
                        "plugin filesystem read bound is invalid".into(),
                    ));
                }
            }
            Self::FilesystemWrite { path, content, .. } => {
                bounded_text(path, 4_096)?;
                if content.len() > 8 * 1_024 * 1_024 || content.contains('\0') {
                    return Err(PluginError::Manifest(
                        "plugin filesystem write content is invalid".into(),
                    ));
                }
            }
            Self::HttpRequest {
                method,
                url,
                headers,
                body,
                max_response_bytes,
            } => {
                if !matches!(
                    method.as_str(),
                    "GET" | "HEAD" | "POST" | "PUT" | "PATCH" | "DELETE"
                ) || headers.len() > 32
                    || *max_response_bytes == 0
                    || *max_response_bytes > 8 * 1_024 * 1_024
                {
                    return Err(PluginError::Manifest(
                        "plugin HTTP request shape is invalid".into(),
                    ));
                }
                bounded_text(url, 4_096)?;
                if body
                    .as_ref()
                    .is_some_and(|body| body.len() > 8 * 1_024 * 1_024 || body.contains('\0'))
                {
                    return Err(PluginError::Manifest(
                        "plugin HTTP request body is invalid".into(),
                    ));
                }
                for header in headers {
                    if !valid_header_name(&header.name) {
                        return Err(PluginError::Manifest(
                            "plugin HTTP header name is invalid".into(),
                        ));
                    }
                    validate_value_source(&header.value)?;
                }
            }
            Self::ProcessExec {
                executable,
                argv,
                cwd,
                env,
                stdin,
            } => {
                bounded_text(executable, 1_024)?;
                bounded_text(cwd, 4_096)?;
                if argv.len() > 128
                    || env.len() > 64
                    || argv.iter().any(|value| bounded_text(value, 8_192).is_err())
                    || stdin.as_ref().is_some_and(|value| {
                        value.len() > 8 * 1_024 * 1_024 || value.contains('\0')
                    })
                {
                    return Err(PluginError::Manifest(
                        "plugin process request shape is invalid".into(),
                    ));
                }
                for variable in env {
                    if !valid_environment_name(&variable.name) {
                        return Err(PluginError::Manifest(
                            "plugin process environment name is invalid".into(),
                        ));
                    }
                    validate_value_source(&variable.value)?;
                }
            }
            Self::McpCall {
                connection_id,
                tool_name,
                arguments,
                max_response_bytes,
            } => validate_mcp_call(connection_id, tool_name, arguments, *max_response_bytes)?,
            Self::BrowserWorkflow { .. } => validate_browser_workflow(self)?,
        }
        Ok(())
    }
}

fn validate_browser_workflow(operation: &PluginCapabilityOperation) -> Result<(), PluginError> {
    let PluginCapabilityOperation::BrowserWorkflow {
        document,
        viewport_width,
        viewport_height,
        timeout_ms,
    } = operation
    else {
        return Err(PluginError::Manifest(
            "plugin operation is not a browser workflow".into(),
        ));
    };
    document.validate().map_err(PluginError::Manifest)?;
    let document_bytes = serde_json::to_vec(document)
        .map_err(|_| PluginError::Manifest("plugin browser workflow is not serializable".into()))?;
    if document_bytes.len() > 512 * 1_024
        || !(320..=7_680).contains(viewport_width)
        || !(240..=4_320).contains(viewport_height)
        || *timeout_ms == 0
        || *timeout_ms > 120_000
    {
        return Err(PluginError::Manifest(
            "plugin browser workflow bounds are invalid".into(),
        ));
    }
    Ok(())
}

fn validate_mcp_call(
    connection_id: &str,
    tool_name: &str,
    arguments: &Value,
    max_response_bytes: u64,
) -> Result<(), PluginError> {
    bounded_text(connection_id, 128)?;
    bounded_text(tool_name, 1_024)?;
    let argument_bytes = serde_json::to_vec(arguments)
        .map_err(|_| PluginError::Manifest("plugin MCP arguments are not serializable".into()))?;
    if !arguments.is_object()
        || argument_bytes.len() > 1_024 * 1_024
        || max_response_bytes == 0
        || max_response_bytes > 8 * 1_024 * 1_024
    {
        return Err(PluginError::Manifest(
            "plugin MCP call shape is invalid".into(),
        ));
    }
    Ok(())
}

impl PluginCapabilityRequest {
    /// Validates identity, bounds, and that the manifest declared every class
    /// needed by this request.
    ///
    /// # Errors
    /// Rejects malformed identity/operation data or undeclared capabilities.
    pub fn validate(&self, manifest: &PluginManifest) -> Result<(), PluginError> {
        if self.schema_version != PLUGIN_SCHEMA_VERSION || !valid_identifier(&self.id) {
            return Err(PluginError::Manifest(
                "plugin capability request identity is invalid".into(),
            ));
        }
        self.operation.validate()?;
        for capability in self.operation.required_capabilities() {
            if !manifest.requested_capabilities.contains(&capability) {
                return Err(PluginError::CapabilityDenied(format!(
                    "{} did not declare {capability:?}",
                    manifest.id
                )));
            }
        }
        Ok(())
    }
}

impl PluginHostTurn {
    /// Validates bounded unique capability history before it reaches a guest.
    ///
    /// # Errors
    /// Rejects unsupported schema, excessive history, or duplicate/bad IDs.
    pub fn validate(&self) -> Result<(), PluginError> {
        if self.schema_version != PLUGIN_SCHEMA_VERSION
            || self.outcomes.len() > PLUGIN_CAPABILITY_TURN_LIMIT
        {
            return Err(PluginError::Manifest(
                "plugin host turn shape is invalid".into(),
            ));
        }
        let mut ids = std::collections::BTreeSet::new();
        for outcome in &self.outcomes {
            let (request_id, error_code) = match outcome {
                PluginCapabilityOutcome::Succeeded { request_id, .. } => (request_id, None),
                PluginCapabilityOutcome::Failed {
                    request_id,
                    error_code,
                } => (request_id, Some(error_code)),
            };
            if !valid_identifier(request_id)
                || !ids.insert(request_id)
                || error_code.is_some_and(|code| !valid_error_code(code))
            {
                return Err(PluginError::Manifest(
                    "plugin capability outcome history is invalid".into(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    pub schema_version: u16,
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub class: PluginClass,
    #[serde(default)]
    pub requested_capabilities: Vec<PluginCapability>,
}

impl PluginManifest {
    /// Validates the stable, portable identity and capability declaration.
    ///
    /// # Errors
    /// Returns a manifest error for unsupported schemas, unsafe identifiers,
    /// empty user-facing fields, duplicates, or capabilities on the wrong class.
    pub fn validate(&self) -> Result<(), PluginError> {
        if self.schema_version != PLUGIN_SCHEMA_VERSION {
            return Err(PluginError::Manifest("unsupported plugin schema".into()));
        }
        if self.id.is_empty()
            || self.id.len() > 128
            || !self.id.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'-' | b'_')
            })
        {
            return Err(PluginError::Manifest(
                "plugin id must use 1-128 lowercase ASCII identifier characters".into(),
            ));
        }
        if self.display_name.trim().is_empty()
            || self.display_name.len() > 256
            || self.version.trim().is_empty()
            || self.version.len() > 64
        {
            return Err(PluginError::Manifest(
                "plugin display name and version must be bounded and non-empty".into(),
            ));
        }
        let mut capabilities = self.requested_capabilities.clone();
        capabilities.sort_unstable();
        capabilities.dedup();
        if capabilities.len() != self.requested_capabilities.len() {
            return Err(PluginError::Manifest(
                "plugin capabilities must not contain duplicates".into(),
            ));
        }
        if self.class == PluginClass::TrustedNative && !capabilities.is_empty() {
            return Err(PluginError::Manifest(
                "trusted native authority is declared by the host integration, not a manifest"
                    .into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginInvocation {
    pub schema_version: u16,
    pub plugin_id: String,
    pub input: Value,
}

impl PluginInvocation {
    /// Validates a bounded invocation envelope.
    ///
    /// # Errors
    /// Returns a manifest error when the schema or plugin identity is invalid.
    pub fn validate(&self) -> Result<(), PluginError> {
        PluginManifest {
            schema_version: self.schema_version,
            id: self.plugin_id.clone(),
            display_name: "invocation".into(),
            version: "1".into(),
            class: PluginClass::TrustedNative,
            requested_capabilities: Vec::new(),
        }
        .validate()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginResult {
    pub schema_version: u16,
    pub plugin_id: String,
    pub output: Value,
}

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("invalid plugin manifest: {0}")]
    Manifest(String),
    #[error("plugin was not found: {0}")]
    NotFound(String),
    #[error("plugin identity is already registered: {0}")]
    Duplicate(String),
    #[error("plugin capability is not granted: {0}")]
    CapabilityDenied(String),
    #[error("plugin resource limit was exceeded: {0}")]
    ResourceLimit(String),
    #[error("plugin execution failed: {0}")]
    Execution(String),
    #[error("plugin returned an invalid result: {0}")]
    InvalidResult(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WasiPluginLimits {
    pub maximum_module_bytes: usize,
    pub maximum_input_bytes: usize,
    pub maximum_output_bytes: usize,
    pub maximum_memory_bytes: usize,
    pub maximum_table_elements: usize,
    pub maximum_instances: usize,
    pub fuel: u64,
    pub maximum_wall_time_ms: u64,
}

impl Default for WasiPluginLimits {
    fn default() -> Self {
        Self {
            maximum_module_bytes: 16 * 1_024 * 1_024,
            maximum_input_bytes: 1_024 * 1_024,
            maximum_output_bytes: 1_024 * 1_024,
            maximum_memory_bytes: 64 * 1_024 * 1_024,
            maximum_table_elements: 10_000,
            maximum_instances: 4,
            fuel: 10_000_000,
            maximum_wall_time_ms: 5_000,
        }
    }
}

impl WasiPluginLimits {
    /// Validates every owner-controlled limit against the hard safety envelope.
    ///
    /// # Errors
    /// Returns a resource-limit error when any value is zero, insufficient, or
    /// exceeds the compiled hard ceiling.
    pub fn validate(&self) -> Result<(), PluginError> {
        if self.maximum_module_bytes == 0
            || self.maximum_module_bytes > 64 * 1_024 * 1_024
            || self.maximum_input_bytes == 0
            || self.maximum_input_bytes > 8 * 1_024 * 1_024
            || self.maximum_output_bytes == 0
            || self.maximum_output_bytes > 8 * 1_024 * 1_024
            || self.maximum_memory_bytes < 64 * 1_024
            || self.maximum_memory_bytes > 512 * 1_024 * 1_024
            || self.maximum_table_elements == 0
            || self.maximum_table_elements > 1_000_000
            || self.maximum_instances == 0
            || self.maximum_instances > 64
            || self.fuel == 0
            || self.fuel > 1_000_000_000
            || self.maximum_wall_time_ms == 0
            || self.maximum_wall_time_ms > 120_000
        {
            return Err(PluginError::ResourceLimit(
                "WASI plugin limits are outside supported safety bounds".into(),
            ));
        }
        Ok(())
    }
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
        })
}

fn bounded_text(value: &str, maximum: usize) -> Result<(), PluginError> {
    if value.is_empty()
        || value.len() > maximum
        || value.contains('\0')
        || value.chars().any(char::is_control)
    {
        return Err(PluginError::Manifest(
            "plugin capability text is invalid".into(),
        ));
    }
    Ok(())
}

fn validate_value_source(source: &PluginValueSource) -> Result<(), PluginError> {
    match source {
        PluginValueSource::Literal(value)
            if value.len() <= 8_192 && !value.contains(['\0', '\r', '\n']) =>
        {
            Ok(())
        }
        PluginValueSource::Credential(reference)
            if valid_credential_component(&reference.provider)
                && valid_credential_component(&reference.key) =>
        {
            Ok(())
        }
        PluginValueSource::Literal(_) | PluginValueSource::Credential(_) => Err(
            PluginError::Manifest("plugin literal or credential reference is invalid".into()),
        ),
    }
}

fn valid_credential_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b':' | b'/')
        })
}

fn valid_header_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn valid_environment_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !value.as_bytes()[0].is_ascii_digit()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn valid_error_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_and_limit_contracts_fail_closed() {
        let manifest = PluginManifest {
            schema_version: PLUGIN_SCHEMA_VERSION,
            id: "example.plugin".into(),
            display_name: "Example".into(),
            version: "1.0.0".into(),
            class: PluginClass::SandboxedWasi,
            requested_capabilities: vec![PluginCapability::Network],
        };
        assert!(manifest.validate().is_ok());
        let mut invalid = manifest;
        invalid.id = "Unsafe/Plugin".into();
        assert!(matches!(invalid.validate(), Err(PluginError::Manifest(_))));

        let limits = WasiPluginLimits {
            maximum_wall_time_ms: 0,
            ..WasiPluginLimits::default()
        };
        assert!(matches!(
            limits.validate(),
            Err(PluginError::ResourceLimit(_))
        ));
    }

    #[test]
    fn capability_turns_require_declared_classes_and_bounded_history() {
        let manifest = PluginManifest {
            schema_version: PLUGIN_SCHEMA_VERSION,
            id: "example.capabilities".into(),
            display_name: "Capability example".into(),
            version: "1.0.0".into(),
            class: PluginClass::SandboxedWasi,
            requested_capabilities: vec![PluginCapability::Network, PluginCapability::Credentials],
        };
        let request = PluginCapabilityRequest {
            schema_version: PLUGIN_SCHEMA_VERSION,
            id: "fetch_config".into(),
            operation: PluginCapabilityOperation::HttpRequest {
                method: "GET".into(),
                url: "https://api.example/config".into(),
                headers: vec![PluginHeader {
                    name: "Authorization".into(),
                    value: PluginValueSource::Credential(PluginCredentialReference {
                        provider: "example".into(),
                        key: "api-token".into(),
                    }),
                }],
                body: None,
                max_response_bytes: 1_024,
            },
        };
        assert!(request.validate(&manifest).is_ok());
        assert_eq!(
            request.operation.required_capabilities(),
            vec![PluginCapability::Network, PluginCapability::Credentials]
        );
        let mut missing_credential = manifest;
        missing_credential
            .requested_capabilities
            .retain(|capability| *capability != PluginCapability::Credentials);
        assert!(matches!(
            request.validate(&missing_credential),
            Err(PluginError::CapabilityDenied(_))
        ));

        let outcome = PluginCapabilityOutcome::Failed {
            request_id: "fetch_config".into(),
            error_code: "denied".into(),
        };
        let duplicate = PluginHostTurn {
            schema_version: PLUGIN_SCHEMA_VERSION,
            original_input: serde_json::json!({}),
            outcomes: vec![outcome.clone(), outcome],
        };
        assert!(duplicate.validate().is_err());
    }

    #[test]
    fn mcp_calls_require_a_declared_capability_and_bounded_object_arguments() {
        let manifest = PluginManifest {
            schema_version: PLUGIN_SCHEMA_VERSION,
            id: "example.mcp".into(),
            display_name: "MCP example".into(),
            version: "1.0.0".into(),
            class: PluginClass::SandboxedWasi,
            requested_capabilities: vec![PluginCapability::Mcp],
        };
        let mut request = PluginCapabilityRequest {
            schema_version: PLUGIN_SCHEMA_VERSION,
            id: "lookup".into(),
            operation: PluginCapabilityOperation::McpCall {
                connection_id: "fixture_connection".into(),
                tool_name: "lookup".into(),
                arguments: serde_json::json!({"query": "bounded"}),
                max_response_bytes: 64 * 1_024,
            },
        };
        assert!(request.validate(&manifest).is_ok());
        assert_eq!(
            request.operation.required_capabilities(),
            vec![PluginCapability::Mcp]
        );

        let missing = PluginManifest {
            requested_capabilities: Vec::new(),
            ..manifest.clone()
        };
        assert!(matches!(
            request.validate(&missing),
            Err(PluginError::CapabilityDenied(_))
        ));
        let PluginCapabilityOperation::McpCall { arguments, .. } = &mut request.operation else {
            unreachable!("MCP fixture operation")
        };
        *arguments = serde_json::json!(["not", "an", "object"]);
        assert!(matches!(
            request.validate(&manifest),
            Err(PluginError::Manifest(_))
        ));
    }

    #[test]
    fn browser_workflows_require_a_distinct_capability_and_bounded_safe_language() {
        let manifest = PluginManifest {
            schema_version: PLUGIN_SCHEMA_VERSION,
            id: "example.browser".into(),
            display_name: "Browser example".into(),
            version: "1.0.0".into(),
            class: PluginClass::SandboxedWasi,
            requested_capabilities: vec![PluginCapability::Browser],
        };
        let mut request = PluginCapabilityRequest {
            schema_version: PLUGIN_SCHEMA_VERSION,
            id: "browser-proof".into(),
            operation: PluginCapabilityOperation::BrowserWorkflow {
                document: BrowserWorkflowDocument {
                    schema_version: mcg_browser_contracts::BROWSER_WORKFLOW_SCHEMA_VERSION,
                    name: "same-origin proof".into(),
                    start_url: "https://app.example.test/start".into(),
                    steps: vec![mcg_browser_contracts::BrowserWorkflowStep::AssertElement {
                        selector: "main".into(),
                    }],
                },
                viewport_width: 1_280,
                viewport_height: 720,
                timeout_ms: 30_000,
            },
        };
        assert!(request.validate(&manifest).is_ok());
        assert_eq!(
            request.operation.required_capabilities(),
            vec![PluginCapability::Browser]
        );
        let network_only = PluginManifest {
            requested_capabilities: vec![PluginCapability::Network],
            ..manifest.clone()
        };
        assert!(matches!(
            request.validate(&network_only),
            Err(PluginError::CapabilityDenied(_))
        ));
        let PluginCapabilityOperation::BrowserWorkflow { document, .. } = &mut request.operation
        else {
            unreachable!("browser fixture operation")
        };
        document.steps = vec![mcg_browser_contracts::BrowserWorkflowStep::Press {
            selector: "body".into(),
            key: "F12".into(),
        }];
        assert!(matches!(
            request.validate(&manifest),
            Err(PluginError::Manifest(_))
        ));
    }
}
