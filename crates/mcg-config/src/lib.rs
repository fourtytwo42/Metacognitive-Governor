//! Layered user and runtime configuration.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

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

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1:7777".to_owned(),
            autonomy: AutonomyPreset::Autonomous,
            advanced_mode: false,
            data_directory: default_data_directory(),
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
