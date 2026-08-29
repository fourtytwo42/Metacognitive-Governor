use std::time::Duration;

#[cfg(windows)]
use std::{
    fs,
    os::windows::process::CommandExt,
    process::{Command, Stdio},
};

use chrono::{DateTime, TimeDelta, Utc};
use semver::Version;
use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::{Update, UpdaterExt};
use uuid::Uuid;

use super::{LOCAL_DAEMON, daemon_token, portable_runtime, response_error, service_bootstrap};

const UPDATE_ENDPOINT: Option<&str> = option_env!("MCG_UPDATER_ENDPOINT");
const UPDATE_PUBLIC_KEY: Option<&str> = option_env!("MCG_UPDATER_PUBKEY");
const MAX_RELEASE_NOTES_CHARS: usize = 8_192;
const BACKUP_CLOCK_SKEW_SECONDS: i64 = 5;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct UpdateConfiguration {
    schema_version: u16,
    configured: bool,
    current_version: String,
    trusted_origin: Option<String>,
    detail: String,
    recovery_notice: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct UpdateCheck {
    schema_version: u16,
    configured: bool,
    current_version: String,
    available_version: Option<String>,
    notes: Option<String>,
    published_at: Option<String>,
    target: Option<String>,
    detail: String,
}

fn compiled_updater_config() -> Result<mcg_updater::TrustedUpdaterConfig, String> {
    mcg_updater::TrustedUpdaterConfig::new(UPDATE_ENDPOINT, UPDATE_PUBLIC_KEY)
}

#[tauri::command]
pub(crate) fn update_configuration() -> UpdateConfiguration {
    let recovery_notice = std::env::var("MCG_UPDATE_RECOVERY_NOTICE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.chars().take(512).collect());
    match compiled_updater_config() {
        Ok(config) => UpdateConfiguration {
            schema_version: 1,
            configured: true,
            current_version: env!("CARGO_PKG_VERSION").into(),
            trusted_origin: Some(config.trusted_origin()),
            detail: "MetaCog will accept only packages signed by this build's trusted update key."
                .into(),
            recovery_notice,
        },
        Err(detail) => UpdateConfiguration {
            schema_version: 1,
            configured: false,
            current_version: env!("CARGO_PKG_VERSION").into(),
            trusted_origin: None,
            detail,
            recovery_notice,
        },
    }
}

fn updater(app: &AppHandle) -> Result<tauri_plugin_updater::Updater, String> {
    let config = compiled_updater_config()?;
    app.updater_builder()
        .endpoints(vec![config.endpoint().clone()])
        .map_err(|error| format!("The trusted update endpoint was rejected: {error}"))?
        .pubkey(config.public_key().to_owned())
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| format!("The signed updater could not start: {error}"))
}

fn bounded_notes(notes: Option<String>) -> Option<String> {
    notes.map(|value| value.chars().take(MAX_RELEASE_NOTES_CHARS).collect())
}

fn validate_update(update: &Update) -> Result<(), String> {
    compiled_updater_config()?.validate_release(update.download_url.as_str(), &update.signature)
}

fn update_check(update: Option<&Update>) -> UpdateCheck {
    match update {
        Some(update) => UpdateCheck {
            schema_version: 1,
            configured: true,
            current_version: update.current_version.clone(),
            available_version: Some(update.version.clone()),
            notes: bounded_notes(update.body.clone()),
            published_at: update.date.map(|date| date.to_string()),
            target: Some(update.target.clone()),
            detail: "A newer owner-signed MetaCog release is available.".into(),
        },
        None => UpdateCheck {
            schema_version: 1,
            configured: true,
            current_version: env!("CARGO_PKG_VERSION").into(),
            available_version: None,
            notes: None,
            published_at: None,
            target: tauri_plugin_updater::target(),
            detail: "MetaCog is up to date.".into(),
        },
    }
}

#[tauri::command]
pub(crate) async fn check_for_update(app: AppHandle) -> Result<UpdateCheck, String> {
    let update = updater(&app)?
        .check()
        .await
        .map_err(|error| format!("The signed update check failed: {error}"))?;
    if let Some(update) = &update {
        validate_update(update)?;
    }
    Ok(update_check(update.as_ref()))
}

async fn create_and_verify_backup(backup_id: Uuid) -> Result<(), String> {
    let requested_at = Utc::now();
    let token = daemon_token()?;
    let client = reqwest::Client::new();
    let created = client
        .post(format!("{LOCAL_DAEMON}/api/v1/backups/{backup_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|error| format!("The pre-update backup could not be created: {error}"))?;
    if !created.status().is_success() {
        return Err(response_error(created, "Pre-update backup creation").await);
    }
    let created: mcg_api::BackupSummaryResponse = created
        .json()
        .await
        .map_err(|error| format!("The pre-update backup response was invalid: {error}"))?;
    let verified = client
        .get(format!("{LOCAL_DAEMON}/api/v1/backups/{backup_id}/verify"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|error| format!("The pre-update backup could not be verified: {error}"))?;
    if !verified.status().is_success() {
        return Err(response_error(verified, "Pre-update backup verification").await);
    }
    let verified: mcg_api::BackupSummaryResponse = verified
        .json()
        .await
        .map_err(|error| format!("The pre-update verification response was invalid: {error}"))?;
    if created != verified || verified.backup_id != backup_id {
        return Err("The pre-update backup changed between creation and verification.".into());
    }
    validate_pre_update_backup(&verified, backup_id, requested_at, Utc::now())?;
    Ok(())
}

fn validate_pre_update_backup(
    summary: &mcg_api::BackupSummaryResponse,
    backup_id: Uuid,
    requested_at: DateTime<Utc>,
    verified_at: DateTime<Utc>,
) -> Result<(), String> {
    if summary.backup_id != backup_id || summary.file_name != format!("{backup_id}.mcgb") {
        return Err("The pre-update backup identity is not canonical.".into());
    }
    if summary.size_bytes == 0
        || summary.sha256.len() != 64
        || !summary
            .sha256
            .bytes()
            .all(|value: u8| value.is_ascii_digit() || (b'a'..=b'f').contains(&value))
    {
        return Err("The pre-update backup digest or size is invalid.".into());
    }
    if summary.manifest.application_version != env!("CARGO_PKG_VERSION") {
        return Err("The pre-update backup came from a different MetaCog version.".into());
    }
    if summary.manifest.secrets_included || summary.manifest.secret_encryption.is_some() {
        return Err("Automatic pre-update backups must not contain credentials.".into());
    }
    if !summary
        .manifest
        .entries
        .iter()
        .any(|entry| entry.path == "state/mcg.sqlite3" && entry.size_bytes > 0)
    {
        return Err("The pre-update backup does not contain the MetaCog database.".into());
    }
    let skew = TimeDelta::seconds(BACKUP_CLOCK_SKEW_SECONDS);
    if summary.manifest.created_at < requested_at - skew
        || summary.manifest.created_at > verified_at + skew
    {
        return Err("The pre-update backup is not fresh for this installation attempt.".into());
    }
    Ok(())
}

fn canonical_expected_version(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 128 {
        return Err("The expected update version is invalid.".into());
    }
    let version = Version::parse(value)
        .map_err(|_| "The expected update version is not canonical semantic version text.")?;
    if version.to_string() != value {
        return Err("The expected update version is not canonical semantic version text.".into());
    }
    Ok(value.to_owned())
}

async fn stop_local_service() -> Result<(), String> {
    let token = daemon_token()?;
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{LOCAL_DAEMON}/api/v1/system/shutdown"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|error| format!("The local service could not be stopped for update: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Pre-update service shutdown").await);
    }
    for _ in 0..100 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if client
            .get(format!("{LOCAL_DAEMON}/health"))
            .timeout(Duration::from_millis(200))
            .send()
            .await
            .is_err()
        {
            return Ok(());
        }
    }
    Err("The local service did not stop cleanly before the update timeout.".into())
}

fn restart_service_after_failed_install() -> String {
    match service_bootstrap::ensure_local_service(LOCAL_DAEMON) {
        Ok(status) if status.ready => " The local service was restarted.".into(),
        Ok(status) => format!(" The local service also needs attention: {}", status.detail),
        Err(error) => format!(" The local service also could not restart: {error}"),
    }
}

#[cfg(windows)]
fn launch_windows_update_supervisor(
    app: &AppHandle,
    request_id: Uuid,
    expected_version: &str,
    installer: &[u8],
) -> Result<(), String> {
    let config = mcg_config::AppConfig::default();
    let current = std::env::current_exe().map_err(|error| {
        format!("The MetaCog desktop executable location could not be determined: {error}")
    })?;
    let parent = current.parent().ok_or_else(|| {
        "The MetaCog desktop executable has no containing installation directory.".to_owned()
    })?;
    let supervisor =
        portable_runtime::packaged_executable(&config.data_directory, "mcg-updater.exe")?
            .unwrap_or_else(|| parent.join("mcg-updater.exe"));
    let metadata = fs::symlink_metadata(&supervisor).map_err(|error| {
        format!("The packaged update supervisor is missing. Repair or reinstall MetaCog: {error}")
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("The packaged update supervisor is not a regular installation file.".into());
    }
    let supervisor_bytes = fs::read(&supervisor)
        .map_err(|error| format!("The packaged update supervisor could not be staged: {error}"))?;
    let staged = mcg_updater::stage_verified_windows_installer(
        &config.data_directory,
        parent,
        request_id,
        std::process::id(),
        expected_version,
        installer,
        &supervisor_bytes,
    )?;
    let mut command = Command::new(&staged.supervisor_path);
    command
        .arg("--apply")
        .arg(&staged.request_path)
        .env("MCG_DATA_DIR", &config.data_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command.creation_flags(CREATE_NO_WINDOW);
    if let Err(error) = command.spawn() {
        let _ = fs::remove_file(&staged.request_path);
        let _ = fs::remove_file(&staged.installer_path);
        let _ = fs::remove_file(&staged.supervisor_path);
        return Err(format!(
            "The supervised Windows updater could not start: {error}"
        ));
    }
    app.exit(0);
    Ok(())
}

#[tauri::command]
pub(crate) async fn install_update(
    app: AppHandle,
    expected_version: String,
    backup_id: String,
) -> Result<(), String> {
    let expected_version = canonical_expected_version(&expected_version)?;
    let backup_id = Uuid::parse_str(&backup_id)
        .map_err(|_| "The pre-update backup identity must be a UUID.".to_owned())?;
    let update = updater(&app)?
        .check()
        .await
        .map_err(|error| format!("The signed update could not be rechecked: {error}"))?
        .ok_or_else(|| "The checked update is no longer available.".to_owned())?;
    validate_update(&update)?;
    if update.version != expected_version {
        return Err(format!(
            "The available update changed from {expected_version} to {}. Review it again before installing.",
            update.version
        ));
    }
    let bytes = update.download(|_, _| {}, || {}).await.map_err(|error| {
        format!("The signed update could not be downloaded and verified: {error}")
    })?;
    #[cfg(windows)]
    {
        let product_version = mcg_updater::windows_installer_product_version(&bytes)?;
        if product_version != expected_version {
            return Err(format!(
                "The signed Windows installer version {product_version} does not match the reviewed update version {expected_version}."
            ));
        }
    }
    create_and_verify_backup(backup_id).await?;
    if let Err(error) = stop_local_service().await {
        let recovery = restart_service_after_failed_install();
        return Err(format!("{error}.{recovery}"));
    }
    #[cfg(windows)]
    {
        launch_windows_update_supervisor(&app, backup_id, &expected_version, &bytes)
    }
    #[cfg(not(windows))]
    {
        if let Err(error) = update.install(bytes) {
            let recovery = restart_service_after_failed_install();
            return Err(format!(
                "The verified update could not be installed: {error}.{recovery}"
            ));
        }
        app.restart();
    }
}

#[cfg(test)]
mod tests {
    use base64::Engine;
    use chrono::{TimeDelta, Utc};
    use mcg_storage::{BackupBundleEntry, BackupBundleManifest};
    use uuid::Uuid;

    use super::{canonical_expected_version, update_check, validate_pre_update_backup};

    fn public_key() -> String {
        base64::engine::general_purpose::STANDARD.encode(
            "untrusted comment: minisign public key: 0000000000000000\nRWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3\n",
        )
    }

    fn signature() -> String {
        base64::engine::general_purpose::STANDARD.encode(
            "untrusted comment: signature from minisign secret key\nRUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=\ntrusted comment: timestamp:1633700835\tfile:test\tprehashed\nwLMDjy9FLAuxZ3q4NlEvkgtyhrr0gtTu6KC4KBJdITbbOeAi1zBIYo0v4iTgt8jJpIidRJnp94ABQkJAgAooBQ==\n",
        )
    }

    #[test]
    fn updater_configuration_is_explicit_https_and_minisign_only() {
        let key = public_key();
        assert!(
            mcg_updater::TrustedUpdaterConfig::new(
                Some("https://updates.example.test/latest.json"),
                Some(&key)
            )
            .is_ok()
        );
        assert!(mcg_updater::TrustedUpdaterConfig::new(None, Some(&key)).is_err());
        assert!(
            mcg_updater::TrustedUpdaterConfig::new(
                Some("http://updates.example.test/latest.json"),
                Some(&key)
            )
            .is_err()
        );
        assert!(
            mcg_updater::TrustedUpdaterConfig::new(
                Some("https://user:secret@updates.example.test/latest.json"),
                Some(&key)
            )
            .is_err()
        );
        assert!(
            mcg_updater::TrustedUpdaterConfig::new(
                Some("https://updates.example.test/latest.json#fragment"),
                Some(&key)
            )
            .is_err()
        );
        assert!(
            mcg_updater::TrustedUpdaterConfig::new(
                Some("https://updates.example.test/latest.json"),
                Some("not-base64")
            )
            .is_err()
        );
    }

    #[test]
    fn updater_manifest_signature_must_be_a_canonical_tauri_envelope() {
        let config = mcg_updater::TrustedUpdaterConfig::new(
            Some("https://updates.example.test/latest.json"),
            Some(&public_key()),
        )
        .unwrap();
        assert!(
            config
                .validate_release("https://updates.example.test/update.exe", &signature())
                .is_ok()
        );
        assert!(
            config
                .validate_release("https://updates.example.test/update.exe", "not-base64")
                .is_err()
        );
        assert!(
            config
                .validate_release(
                    "https://updates.example.test/update.exe",
                    &base64::engine::general_purpose::STANDARD.encode("not a minisign signature")
                )
                .is_err()
        );
    }

    #[test]
    fn no_update_result_is_explicit_and_never_a_downgrade() {
        let result = update_check(None);
        assert!(result.configured);
        assert!(result.available_version.is_none());
        assert_eq!(result.detail, "MetaCog is up to date.");
    }

    fn backup_summary(backup_id: Uuid) -> mcg_api::BackupSummaryResponse {
        mcg_api::BackupSummaryResponse {
            backup_id,
            file_name: format!("{backup_id}.mcgb"),
            sha256: "a".repeat(64),
            size_bytes: 1_024,
            manifest: BackupBundleManifest {
                schema_version: 2,
                application_version: env!("CARGO_PKG_VERSION").into(),
                created_at: Utc::now(),
                secrets_included: false,
                secret_encryption: None,
                entries: vec![BackupBundleEntry {
                    path: "state/mcg.sqlite3".into(),
                    size_bytes: 512,
                    sha256: "b".repeat(64),
                }],
            },
        }
    }

    #[test]
    fn updater_install_requires_canonical_semver_review_identity() {
        assert_eq!(canonical_expected_version("1.2.3").unwrap(), "1.2.3");
        assert_eq!(
            canonical_expected_version("1.2.3-rc.1+qualified").unwrap(),
            "1.2.3-rc.1+qualified"
        );
        assert!(canonical_expected_version("v1.2.3").is_err());
        assert!(canonical_expected_version("01.2.3").is_err());
        assert!(canonical_expected_version("1.2.3\nchanged").is_err());
    }

    #[test]
    fn updater_backup_must_be_fresh_current_and_credential_free() {
        let backup_id = Uuid::new_v4();
        let now = Utc::now();
        let valid = backup_summary(backup_id);
        assert!(validate_pre_update_backup(&valid, backup_id, now, now).is_ok());

        let mut stale = valid.clone();
        stale.manifest.created_at = now - TimeDelta::minutes(10);
        assert!(validate_pre_update_backup(&stale, backup_id, now, now).is_err());

        let mut secret = valid.clone();
        secret.manifest.secrets_included = true;
        assert!(validate_pre_update_backup(&secret, backup_id, now, now).is_err());

        let mut wrong_version = valid.clone();
        wrong_version.manifest.application_version = "999.0.0".into();
        assert!(validate_pre_update_backup(&wrong_version, backup_id, now, now).is_err());

        let mut missing_database = valid;
        missing_database.manifest.entries.clear();
        assert!(validate_pre_update_backup(&missing_database, backup_id, now, now).is_err());
    }
}
