//! Lightweight, versioned protocol shared by the daemon and isolated plugin runner.

use serde::{Deserialize, Serialize};
use serde_json::Value;

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
}
