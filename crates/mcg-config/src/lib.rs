//! Layered user and runtime configuration.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const USER_SETTINGS_SCHEMA_VERSION: u16 = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliTuiPreferences {
    pub keymap: String,
    pub vim_mode: bool,
    pub theme: String,
    pub status_line: Vec<String>,
    pub title_fields: Vec<String>,
    pub raw_mode: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pet: Option<String>,
}

impl Default for CliTuiPreferences {
    fn default() -> Self {
        Self {
            keymap: "default".into(),
            vim_mode: false,
            theme: "system".into(),
            status_line: vec![
                "project".into(),
                "branch".into(),
                "model".into(),
                "effort".into(),
                "permissions".into(),
                "goal".into(),
                "usage".into(),
            ],
            title_fields: vec!["project".into(), "conversation".into()],
            raw_mode: false,
            pet: None,
        }
    }
}

impl CliTuiPreferences {
    #[must_use]
    pub fn is_valid(&self) -> bool {
        ["default", "vim"].contains(&self.keymap.as_str())
            && ["system", "dark", "light"].contains(&self.theme.as_str())
            && self.status_line.len() <= 16
            && self.title_fields.len() <= 8
            && self
                .status_line
                .iter()
                .chain(self.title_fields.iter())
                .all(|field| !field.is_empty() && field.len() <= 32)
            && self.pet.as_ref().is_none_or(|pet| pet.len() <= 32)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutonomyPreset {
    Guided,
    Balanced,
    Autonomous,
}

/// Presentation density for every `MetaCog` interface.
///
/// This preference never grants authority or changes runtime semantics. Higher
/// levels only reveal progressively deeper controls and evidence.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterfaceComplexity {
    #[default]
    Simple,
    Advanced,
    Developer,
}

impl InterfaceComplexity {
    #[must_use]
    pub const fn exposes_advanced(self) -> bool {
        matches!(self, Self::Advanced | Self::Developer)
    }

    #[must_use]
    pub const fn exposes_developer(self) -> bool {
        matches!(self, Self::Developer)
    }
}

/// One owner-configured public key allowed to authenticate packages from exact
/// registry identities. Public keys are configuration, never credentials.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustedPluginSignerConfig {
    pub id: String,
    pub public_key_base64: String,
    pub registries: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppConfig {
    pub bind_address: String,
    pub autonomy: AutonomyPreset,
    pub advanced_mode: bool,
    pub data_directory: PathBuf,
    #[serde(default)]
    pub browser_executable: Option<PathBuf>,
    #[serde(default = "default_shadow_experiments")]
    pub shadow_experiments: bool,
    /// Coordinator-led conversation rollout. Release builds enable it by default after the
    /// schema-42 security/E2E gate; debug builds stay opt-in for deterministic unit fixtures.
    #[serde(default)]
    pub two_agent_conversations: bool,
    #[serde(default)]
    pub trusted_plugin_signers: Vec<TrustedPluginSignerConfig>,
}

/// Owner-controlled behavior that may be changed safely while the daemon runs.
/// Network binding, data paths, and executable trust roots remain startup-only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserSettings {
    pub schema_version: u16,
    pub revision: u64,
    pub interface_complexity: InterfaceComplexity,
    pub autonomy: AutonomyPreset,
    pub shadow_experiments: bool,
    /// Optional experimental Codex app-server WebSocket endpoint. `None` keeps the
    /// bundled local stdio server. Credentials are never persisted here.
    pub codex_remote_endpoint: Option<String>,
    /// Name of the daemon environment variable containing the remote bearer token.
    /// The token value itself is never stored in settings or audit revisions.
    pub codex_remote_auth_token_env: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize)]
struct UserSettingsApiWire<'a> {
    schema_version: u16,
    revision: u64,
    interface_complexity: InterfaceComplexity,
    /// Response-only compatibility projection for pre-v2 clients.
    advanced_mode: bool,
    autonomy: AutonomyPreset,
    shadow_experiments: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    codex_remote_endpoint: &'a Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    codex_remote_auth_token_env: &'a Option<String>,
    updated_at: DateTime<Utc>,
}

impl Serialize for UserSettings {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        UserSettingsApiWire {
            schema_version: self.schema_version,
            revision: self.revision,
            interface_complexity: self.interface_complexity,
            advanced_mode: self.interface_complexity.exposes_advanced(),
            autonomy: self.autonomy,
            shadow_experiments: self.shadow_experiments,
            codex_remote_endpoint: &self.codex_remote_endpoint,
            codex_remote_auth_token_env: &self.codex_remote_auth_token_env,
            updated_at: self.updated_at,
        }
        .serialize(serializer)
    }
}

#[derive(Deserialize)]
struct UserSettingsWire {
    schema_version: u16,
    revision: u64,
    #[serde(default)]
    interface_complexity: Option<InterfaceComplexity>,
    #[serde(default)]
    advanced_mode: Option<bool>,
    autonomy: AutonomyPreset,
    shadow_experiments: bool,
    #[serde(default)]
    codex_remote_endpoint: Option<String>,
    #[serde(default)]
    codex_remote_auth_token_env: Option<String>,
    updated_at: DateTime<Utc>,
}

impl<'de> Deserialize<'de> for UserSettings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = UserSettingsWire::deserialize(deserializer)?;
        if !matches!(wire.schema_version, 1 | USER_SETTINGS_SCHEMA_VERSION) {
            return Err(serde::de::Error::custom(format!(
                "unsupported user settings schema version {}",
                wire.schema_version
            )));
        }
        if wire.schema_version == USER_SETTINGS_SCHEMA_VERSION
            && wire.interface_complexity.is_none()
        {
            return Err(serde::de::Error::custom(
                "user settings v2 requires interface_complexity",
            ));
        }
        let interface_complexity = match (wire.interface_complexity, wire.advanced_mode) {
            (Some(value), _) => value,
            (None, Some(true)) => InterfaceComplexity::Advanced,
            (None, Some(false) | None) => InterfaceComplexity::Simple,
        };
        Ok(Self {
            schema_version: USER_SETTINGS_SCHEMA_VERSION,
            revision: wire.revision,
            interface_complexity,
            autonomy: wire.autonomy,
            shadow_experiments: wire.shadow_experiments,
            codex_remote_endpoint: wire.codex_remote_endpoint,
            codex_remote_auth_token_env: wire.codex_remote_auth_token_env,
            updated_at: wire.updated_at,
        })
    }
}

/// One immutable owner-settings audit revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserSettingsRevision {
    pub settings: UserSettings,
    pub reason: String,
    pub changed_at: DateTime<Utc>,
}

impl UserSettings {
    #[must_use]
    pub fn initial(config: &AppConfig, now: DateTime<Utc>) -> Self {
        Self {
            schema_version: USER_SETTINGS_SCHEMA_VERSION,
            revision: 1,
            interface_complexity: if config.advanced_mode {
                InterfaceComplexity::Advanced
            } else {
                InterfaceComplexity::Simple
            },
            autonomy: config.autonomy,
            shadow_experiments: config.shadow_experiments,
            codex_remote_endpoint: None,
            codex_remote_auth_token_env: None,
            updated_at: now,
        }
    }

    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.schema_version == USER_SETTINGS_SCHEMA_VERSION && self.revision > 0
    }
}

impl AppConfig {
    #[must_use]
    pub fn database_path(&self) -> PathBuf {
        self.data_directory.join("mcg.sqlite3")
    }

    #[must_use]
    pub fn auth_token_path(&self) -> PathBuf {
        self.data_directory.join("local-auth-token")
    }

    #[must_use]
    pub fn with_data_directory(mut self, path: impl AsRef<Path>) -> Self {
        self.data_directory = path.as_ref().to_path_buf();
        self
    }
}

fn default_data_directory() -> PathBuf {
    if let Some(path) = std::env::var_os("MCG_DATA_DIR") {
        return PathBuf::from(path);
    }
    if cfg!(windows) {
        if let Some(path) = std::env::var_os("LOCALAPPDATA") {
            return PathBuf::from(path).join("MCG");
        }
    } else if let Some(path) = std::env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(path).join("mcg");
    } else if let Some(path) = std::env::var_os("HOME") {
        return PathBuf::from(path).join(".local").join("share").join("mcg");
    }
    PathBuf::from(".mcg")
}

const fn default_shadow_experiments() -> bool {
    cfg!(debug_assertions)
}

fn shadow_experiments_from_environment() -> bool {
    std::env::var("MCG_SHADOW_EXPERIMENTS").map_or_else(
        |_| default_shadow_experiments(),
        |value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        },
    )
}

const fn default_multi_agent_conversations() -> bool {
    !cfg!(debug_assertions)
}

fn two_agent_conversations_from_environment() -> bool {
    for key in [
        "MCG_MULTI_AGENT_CONVERSATIONS",
        "MCG_TWO_AGENT_CONVERSATIONS",
    ] {
        if let Ok(value) = std::env::var(key) {
            return matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            );
        }
    }
    default_multi_agent_conversations()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            bind_address: std::env::var("MCG_BIND_ADDRESS")
                .unwrap_or_else(|_| "127.0.0.1:7777".to_owned()),
            autonomy: AutonomyPreset::Autonomous,
            advanced_mode: false,
            data_directory: default_data_directory(),
            browser_executable: std::env::var_os("MCG_BROWSER_EXECUTABLE").map(PathBuf::from),
            shadow_experiments: shadow_experiments_from_environment(),
            two_agent_conversations: two_agent_conversations_from_environment(),
            trusted_plugin_signers: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consumer_default_is_simple_and_autonomous() {
        let config = AppConfig::default();

        assert_eq!(config.autonomy, AutonomyPreset::Autonomous);
        assert!(!config.advanced_mode);
        if std::env::var_os("MCG_MULTI_AGENT_CONVERSATIONS").is_none()
            && std::env::var_os("MCG_TWO_AGENT_CONVERSATIONS").is_none()
        {
            assert_eq!(
                config.two_agent_conversations,
                default_multi_agent_conversations()
            );
        }
        if std::env::var_os("MCG_SHADOW_EXPERIMENTS").is_none() {
            assert_eq!(config.shadow_experiments, cfg!(debug_assertions));
        }
        assert!(config.bind_address.starts_with("127.0.0.1:"));
        assert_eq!(
            config
                .database_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("mcg.sqlite3")
        );
    }

    #[test]
    fn legacy_settings_upcast_without_promoting_to_developer() {
        let base = serde_json::json!({
            "schema_version": 1,
            "revision": 7,
            "advanced_mode": true,
            "autonomy": "balanced",
            "shadow_experiments": false,
            "updated_at": "2026-08-29T00:00:00Z"
        });
        let settings: UserSettings = serde_json::from_value(base).expect("legacy settings");
        assert_eq!(settings.schema_version, USER_SETTINGS_SCHEMA_VERSION);
        assert_eq!(settings.interface_complexity, InterfaceComplexity::Advanced);
        let response = serde_json::to_value(settings).expect("v2 API settings");
        assert_eq!(response["interface_complexity"], "advanced");
        assert_eq!(response["advanced_mode"], true);
    }

    #[test]
    fn unknown_settings_schema_fails_closed() {
        let future = serde_json::json!({
            "schema_version": 99,
            "revision": 1,
            "interface_complexity": "developer",
            "autonomy": "balanced",
            "shadow_experiments": false,
            "updated_at": "2026-08-29T00:00:00Z"
        });
        assert!(serde_json::from_value::<UserSettings>(future).is_err());
    }
}
