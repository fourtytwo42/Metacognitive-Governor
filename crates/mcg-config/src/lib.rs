//! Layered user and runtime configuration.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const USER_SETTINGS_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutonomyPreset {
    Guided,
    Balanced,
    Autonomous,
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
}

/// Owner-controlled behavior that may be changed safely while the daemon runs.
/// Network binding, data paths, and executable trust roots remain startup-only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserSettings {
    pub schema_version: u16,
    pub revision: u64,
    pub advanced_mode: bool,
    pub autonomy: AutonomyPreset,
    pub shadow_experiments: bool,
    pub updated_at: DateTime<Utc>,
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
            advanced_mode: config.advanced_mode,
            autonomy: config.autonomy,
            shadow_experiments: config.shadow_experiments,
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
}
