use chrono::{DateTime, Duration, Utc};
use mcg_config::{
    AppConfig, InterfaceComplexity, USER_SETTINGS_SCHEMA_VERSION, UserSettings,
    UserSettingsRevision,
};
use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use crate::{SqliteEventStore, StorageError};

const OWNER_KEY: &str = "local-owner";
const MAX_REASON_BYTES: usize = 512;

impl SqliteEventStore {
    /// Loads durable settings or atomically creates revision one from startup defaults.
    ///
    /// # Errors
    /// Returns validation, serialization, or `SQLite` failures.
    pub fn initialize_user_settings(
        &mut self,
        config: &AppConfig,
        now: DateTime<Utc>,
    ) -> Result<UserSettings, StorageError> {
        if let Some(settings) = self.user_settings()? {
            return Ok(settings);
        }
        let settings = UserSettings::initial(config, now);
        let json = persisted_settings_json(&settings)?;
        let revision = revision_i64(settings.revision)?;
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO user_settings(owner_key, revision, settings_json, updated_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![OWNER_KEY, revision, &json, now.to_rfc3339()],
        )?;
        transaction.execute(
            "INSERT INTO user_settings_revisions(revision, settings_json, reason, changed_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![revision, &json, "initial settings", now.to_rfc3339()],
        )?;
        transaction.commit()?;
        Ok(settings)
    }

    /// Loads the current owner settings projection.
    ///
    /// # Errors
    /// Returns validation, decoding, or `SQLite` failures.
    pub fn user_settings(&self) -> Result<Option<UserSettings>, StorageError> {
        let json = self
            .connection
            .query_row(
                "SELECT settings_json FROM user_settings WHERE owner_key = ?1",
                [OWNER_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        json.map(|json| {
            let settings: UserSettings = serde_json::from_str(&json)?;
            validate_settings(&settings)?;
            Ok(settings)
        })
        .transpose()
    }

    /// Atomically replaces current settings and appends an immutable revision.
    ///
    /// # Errors
    /// Returns stale-revision, validation, serialization, or `SQLite` failures.
    pub fn update_user_settings(
        &mut self,
        expected_revision: u64,
        interface_complexity: InterfaceComplexity,
        autonomy: mcg_config::AutonomyPreset,
        shadow_experiments: bool,
        reason: &str,
        now: DateTime<Utc>,
    ) -> Result<UserSettings, StorageError> {
        validate_reason(reason)?;
        let current = self.user_settings()?.ok_or_else(|| {
            StorageError::InvalidData("user settings were not initialized".into())
        })?;
        if current.revision != expected_revision {
            return Err(StorageError::Conflict(format!(
                "user settings revision is {}; expected {expected_revision}",
                current.revision
            )));
        }
        let updated_at = if now > current.updated_at {
            now
        } else {
            current.updated_at + Duration::nanoseconds(1)
        };
        let settings = UserSettings {
            schema_version: USER_SETTINGS_SCHEMA_VERSION,
            revision: current.revision.checked_add(1).ok_or_else(|| {
                StorageError::InvalidData("user settings revision overflow".into())
            })?,
            interface_complexity,
            autonomy,
            shadow_experiments,
            codex_remote_endpoint: current.codex_remote_endpoint,
            codex_remote_auth_token_env: current.codex_remote_auth_token_env,
            updated_at,
        };
        validate_settings(&settings)?;
        let json = persisted_settings_json(&settings)?;
        let revision = revision_i64(settings.revision)?;
        let expected_revision = revision_i64(expected_revision)?;
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE user_settings SET revision = ?1, settings_json = ?2, updated_at = ?3
             WHERE owner_key = ?4 AND revision = ?5",
            params![
                revision,
                &json,
                settings.updated_at.to_rfc3339(),
                OWNER_KEY,
                expected_revision,
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::Conflict(
                "user settings changed concurrently".into(),
            ));
        }
        transaction.execute(
            "INSERT INTO user_settings_revisions(revision, settings_json, reason, changed_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                revision,
                &json,
                reason.trim(),
                settings.updated_at.to_rfc3339(),
            ],
        )?;
        transaction.commit()?;
        Ok(settings)
    }

    /// Atomically switches the persisted Codex app-server transport without ever
    /// storing the bearer credential value.
    pub fn update_codex_transport_settings(
        &mut self,
        expected_revision: u64,
        endpoint: Option<String>,
        auth_token_env: Option<String>,
        reason: &str,
        now: DateTime<Utc>,
    ) -> Result<UserSettings, StorageError> {
        validate_reason(reason)?;
        let current = self.user_settings()?.ok_or_else(|| {
            StorageError::InvalidData("user settings were not initialized".into())
        })?;
        if current.revision != expected_revision {
            return Err(StorageError::Conflict(format!(
                "user settings revision is {}; expected {expected_revision}",
                current.revision
            )));
        }
        let updated_at = if now > current.updated_at {
            now
        } else {
            current.updated_at + Duration::nanoseconds(1)
        };
        let settings = UserSettings {
            schema_version: USER_SETTINGS_SCHEMA_VERSION,
            revision: current.revision.checked_add(1).ok_or_else(|| {
                StorageError::InvalidData("user settings revision overflow".into())
            })?,
            interface_complexity: current.interface_complexity,
            autonomy: current.autonomy,
            shadow_experiments: current.shadow_experiments,
            codex_remote_endpoint: endpoint,
            codex_remote_auth_token_env: auth_token_env,
            updated_at,
        };
        validate_settings(&settings)?;
        persist_settings_revision(self, expected_revision, &settings, reason)
    }

    /// Lists immutable settings history in revision order.
    ///
    /// # Errors
    /// Returns decoding or `SQLite` failures.
    pub fn user_settings_history(&self) -> Result<Vec<UserSettingsRevision>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT settings_json, reason, changed_at
             FROM user_settings_revisions ORDER BY revision",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        let mut revisions = Vec::new();
        for row in rows {
            let (settings_json, reason, changed_at) = row?;
            let settings: UserSettings = serde_json::from_str(&settings_json)?;
            validate_settings(&settings)?;
            validate_reason(&reason)?;
            let changed_at = DateTime::parse_from_rfc3339(&changed_at)
                .map_err(|error| StorageError::InvalidData(error.to_string()))?
                .with_timezone(&Utc);
            revisions.push(UserSettingsRevision {
                settings,
                reason,
                changed_at,
            });
        }
        Ok(revisions)
    }
}

fn persist_settings_revision(
    store: &mut SqliteEventStore,
    expected_revision: u64,
    settings: &UserSettings,
    reason: &str,
) -> Result<UserSettings, StorageError> {
    let json = persisted_settings_json(settings)?;
    let revision = revision_i64(settings.revision)?;
    let expected_revision = revision_i64(expected_revision)?;
    let transaction = store.connection.transaction()?;
    let changed = transaction.execute(
        "UPDATE user_settings SET revision = ?1, settings_json = ?2, updated_at = ?3
         WHERE owner_key = ?4 AND revision = ?5",
        params![
            revision,
            &json,
            settings.updated_at.to_rfc3339(),
            OWNER_KEY,
            expected_revision
        ],
    )?;
    if changed != 1 {
        return Err(StorageError::Conflict(
            "user settings changed concurrently".into(),
        ));
    }
    transaction.execute(
        "INSERT INTO user_settings_revisions(revision, settings_json, reason, changed_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            revision,
            &json,
            reason.trim(),
            settings.updated_at.to_rfc3339()
        ],
    )?;
    transaction.commit()?;
    Ok(settings.clone())
}

fn validate_settings(settings: &UserSettings) -> Result<(), StorageError> {
    if !settings.is_valid() || settings.revision > i64::MAX as u64 {
        return Err(StorageError::InvalidData(
            "user settings schema or revision is invalid".into(),
        ));
    }
    Ok(())
}

#[derive(Serialize)]
struct PersistedUserSettings<'a> {
    schema_version: u16,
    revision: u64,
    interface_complexity: InterfaceComplexity,
    autonomy: mcg_config::AutonomyPreset,
    shadow_experiments: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    codex_remote_endpoint: &'a Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    codex_remote_auth_token_env: &'a Option<String>,
    updated_at: DateTime<Utc>,
}

fn persisted_settings_json(settings: &UserSettings) -> Result<String, StorageError> {
    Ok(serde_json::to_string(&PersistedUserSettings {
        schema_version: settings.schema_version,
        revision: settings.revision,
        interface_complexity: settings.interface_complexity,
        autonomy: settings.autonomy,
        shadow_experiments: settings.shadow_experiments,
        codex_remote_endpoint: &settings.codex_remote_endpoint,
        codex_remote_auth_token_env: &settings.codex_remote_auth_token_env,
        updated_at: settings.updated_at,
    })?)
}

fn revision_i64(revision: u64) -> Result<i64, StorageError> {
    i64::try_from(revision)
        .map_err(|_| StorageError::InvalidData("user settings revision exceeds i64".into()))
}

fn validate_reason(reason: &str) -> Result<(), StorageError> {
    let reason = reason.trim();
    if reason.is_empty() || reason.len() > MAX_REASON_BYTES || reason.chars().any(char::is_control)
    {
        return Err(StorageError::InvalidData(
            "settings change reason must be 1-512 printable bytes".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use mcg_config::AutonomyPreset;

    use super::*;

    #[test]
    fn settings_are_revision_safe_and_survive_restart() {
        let temporary = tempfile::tempdir().expect("tempdir");
        let database = temporary.path().join("settings.sqlite3");
        let now = Utc::now();
        let config = AppConfig::default();
        let mut store = SqliteEventStore::open(&database).expect("store");
        let initial = store
            .initialize_user_settings(&config, now)
            .expect("initialize");
        assert_eq!(initial.revision, 1);
        let updated = store
            .update_user_settings(
                1,
                InterfaceComplexity::Advanced,
                AutonomyPreset::Guided,
                true,
                "owner enabled advanced controls",
                now,
            )
            .expect("update");
        assert_eq!(updated.revision, 2);
        assert_eq!(updated.interface_complexity, InterfaceComplexity::Advanced);
        assert_eq!(updated.autonomy, AutonomyPreset::Guided);
        assert!(updated.updated_at > initial.updated_at);
        let persisted: serde_json::Value = store
            .connection
            .query_row(
                "SELECT settings_json FROM user_settings WHERE owner_key = ?1",
                [OWNER_KEY],
                |row| row.get::<_, String>(0),
            )
            .map(|json| serde_json::from_str(&json).expect("persisted settings JSON"))
            .expect("persisted settings");
        assert_eq!(persisted["interface_complexity"], "advanced");
        assert!(persisted.get("advanced_mode").is_none());
        let transported = store
            .update_codex_transport_settings(
                2,
                Some("wss://codex.example.test/socket".into()),
                Some("CODEX_REMOTE_TOKEN".into()),
                "selected remote Codex transport",
                now,
            )
            .expect("transport update");
        assert_eq!(transported.revision, 3);
        assert_eq!(
            transported.codex_remote_endpoint.as_deref(),
            Some("wss://codex.example.test/socket")
        );
        assert!(matches!(
            store.update_user_settings(
                1,
                InterfaceComplexity::Simple,
                AutonomyPreset::Autonomous,
                false,
                "stale update",
                now,
            ),
            Err(StorageError::Conflict(_))
        ));
        assert!(
            store
                .connection
                .execute(
                    "UPDATE user_settings_revisions SET reason = 'rewritten' WHERE revision = 1",
                    [],
                )
                .is_err()
        );
        assert!(
            store
                .connection
                .execute("DELETE FROM user_settings_revisions WHERE revision = 1", [])
                .is_err()
        );
        drop(store);

        let mut reopened = SqliteEventStore::open(&database).expect("reopen");
        assert_eq!(
            reopened.user_settings().expect("read"),
            Some(transported.clone())
        );
        assert_eq!(
            reopened.user_settings_history().expect("history"),
            vec![
                UserSettingsRevision {
                    settings: initial,
                    reason: "initial settings".into(),
                    changed_at: now,
                },
                UserSettingsRevision {
                    settings: updated.clone(),
                    reason: "owner enabled advanced controls".into(),
                    changed_at: updated.updated_at,
                },
                UserSettingsRevision {
                    settings: transported.clone(),
                    reason: "selected remote Codex transport".into(),
                    changed_at: transported.updated_at,
                },
            ]
        );
        assert_eq!(
            reopened
                .initialize_user_settings(&AppConfig::default(), Utc::now())
                .expect("reinitialize"),
            transported
        );
    }
}
