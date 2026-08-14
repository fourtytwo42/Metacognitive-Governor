//! Layered user and runtime configuration.

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
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1:7777".to_owned(),
            autonomy: AutonomyPreset::Autonomous,
            advanced_mode: false,
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
    }
}
