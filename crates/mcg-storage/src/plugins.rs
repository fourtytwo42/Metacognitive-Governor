use chrono::{DateTime, Utc};
use mcg_domain::{
    PLUGIN_CATALOG_SCHEMA_VERSION, PluginCapabilityGrant, PluginCapabilityScope,
    PluginInstallSource, PluginInstallation, PluginInvocationRecord, PluginInvocationState,
    PluginRevision, PluginRevisionId, PluginSignatureState,
};
use mcg_plugin_protocol::{PluginCapability, PluginClass};
use mcg_plugin_trust::{PLUGIN_SIGNATURE_SCHEME, VerifiedPluginRevision};
use rusqlite::{OptionalExtension, Transaction, params};

use crate::{SqliteEventStore, StorageError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginRecoveryReport {
    pub invocations_marked_uncertain: u64,
}

impl SqliteEventStore {
    /// Installs an explicitly local unsigned module as a disabled revision.
    ///
    /// # Errors
    /// Rejects registry provenance, signature claims, invalid identity, and
    /// storage conflicts. Enabling always requires a separate owner mutation.
    pub fn install_unsigned_local_plugin_revision(
        &mut self,
        revision: &PluginRevision,
    ) -> Result<PluginInstallation, StorageError> {
        if revision.source != PluginInstallSource::LocalImport
            || revision.signature.state != PluginSignatureState::Unsigned
        {
            return Err(StorageError::InvalidData(
                "unsigned installation is restricted to explicit local imports".into(),
            ));
        }
        self.install_plugin_revision(revision, false)
    }

    /// Installs one cryptographically sealed registry revision as disabled.
    ///
    /// # Errors
    /// Returns catalog validation, conflict, serialization, or `SQLite` failures.
    pub fn install_verified_plugin_revision(
        &mut self,
        verified: &VerifiedPluginRevision,
    ) -> Result<PluginInstallation, StorageError> {
        self.install_plugin_revision(verified.revision(), false)
    }

    /// Installs one immutable sandboxed plugin revision and atomically advances
    /// its mutable installation head.
    ///
    /// # Errors
    /// Returns validation, conflict, serialization, or `SQLite` failures without
    /// committing a partial revision/head pair.
    fn install_plugin_revision(
        &mut self,
        revision: &PluginRevision,
        enabled: bool,
    ) -> Result<PluginInstallation, StorageError> {
        validate_plugin_revision(revision)?;
        let transaction = self.connection.transaction()?;
        if let Some(stored) = load_plugin_revision(&transaction, revision.id)? {
            if stored != *revision {
                return Err(StorageError::Conflict(format!(
                    "plugin revision {} was reused with different content",
                    revision.id
                )));
            }
            let installation = load_plugin_installation(&transaction, &revision.manifest.id)?
                .ok_or_else(|| {
                    StorageError::InvalidData("plugin revision has no installation head".into())
                })?;
            if installation.active_revision_id != revision.id {
                return Err(StorageError::Conflict(
                    "plugin revision is no longer the active installation head".into(),
                ));
            }
            return Ok(installation);
        }

        let previous = load_plugin_installation(&transaction, &revision.manifest.id)?;
        if previous
            .as_ref()
            .is_some_and(|installation| revision.installed_at < installation.updated_at)
        {
            return Err(StorageError::Conflict(
                "plugin revision timestamp precedes its installation head".into(),
            ));
        }
        let head_revision = previous
            .as_ref()
            .map_or(1, |installation| installation.revision.saturating_add(1));
        if head_revision == u64::MAX {
            return Err(StorageError::Conflict(
                "plugin installation revision is exhausted".into(),
            ));
        }
        let installation = PluginInstallation {
            schema_version: PLUGIN_CATALOG_SCHEMA_VERSION,
            plugin_id: revision.manifest.id.clone(),
            active_revision_id: revision.id,
            enabled,
            capability_grants: Vec::new(),
            revision: head_revision,
            updated_at: revision.installed_at,
        };
        transaction.execute(
            "INSERT INTO plugin_revisions(
                revision_id, plugin_id, module_sha256, revision_json, installed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                revision.id.to_string(),
                revision.manifest.id,
                revision.module_sha256,
                serde_json::to_string(revision)?,
                revision.installed_at.to_rfc3339(),
            ],
        )?;
        transaction.execute(
            "INSERT INTO plugin_installations(
                plugin_id, active_revision_id, enabled, revision, installation_json, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(plugin_id) DO UPDATE SET
                active_revision_id = excluded.active_revision_id,
                enabled = excluded.enabled,
                revision = excluded.revision,
                installation_json = excluded.installation_json,
                updated_at = excluded.updated_at",
            params![
                installation.plugin_id,
                installation.active_revision_id.to_string(),
                installation.enabled,
                to_sql_revision(installation.revision)?,
                serde_json::to_string(&installation)?,
                installation.updated_at.to_rfc3339(),
            ],
        )?;
        transaction.commit()?;
        Ok(installation)
    }

    /// Enables or disables an installed plugin with optimistic revision control.
    ///
    /// # Errors
    /// Returns not-found, stale-revision, validation, or `SQLite` failures.
    pub fn set_plugin_enabled(
        &mut self,
        plugin_id: &str,
        expected_revision: u64,
        enabled: bool,
        changed_at: DateTime<Utc>,
    ) -> Result<PluginInstallation, StorageError> {
        let transaction = self.connection.transaction()?;
        let mut installation = load_plugin_installation(&transaction, plugin_id)?
            .ok_or_else(|| StorageError::PluginNotFound(plugin_id.into()))?;
        if installation.revision != expected_revision {
            return Err(StorageError::Conflict(format!(
                "plugin installation revision is {}, expected {expected_revision}",
                installation.revision
            )));
        }
        if installation.enabled == enabled {
            return Ok(installation);
        }
        if enabled {
            let revision = load_plugin_revision(&transaction, installation.active_revision_id)?
                .ok_or(StorageError::PluginRevisionNotFound(
                    installation.active_revision_id,
                ))?;
            validate_grants_cover_manifest(&installation.capability_grants, &revision)?;
        }
        installation.revision = installation
            .revision
            .checked_add(1)
            .ok_or_else(|| StorageError::Conflict("plugin revision is exhausted".into()))?;
        installation.enabled = enabled;
        installation.updated_at = changed_at;
        transaction.execute(
            "UPDATE plugin_installations SET
                enabled = ?1, revision = ?2, installation_json = ?3, updated_at = ?4
             WHERE plugin_id = ?5 AND revision = ?6",
            params![
                enabled,
                to_sql_revision(installation.revision)?,
                serde_json::to_string(&installation)?,
                changed_at.to_rfc3339(),
                plugin_id,
                to_sql_revision(expected_revision)?,
            ],
        )?;
        transaction.commit()?;
        Ok(installation)
    }

    /// Replaces the complete owner-issued capability grant set using optimistic
    /// revision control. Any change disables the plugin atomically.
    ///
    /// # Errors
    /// Rejects stale writers, malformed or excessive scopes, capabilities not
    /// requested by the active manifest, and storage failures.
    pub fn set_plugin_capability_grants(
        &mut self,
        plugin_id: &str,
        expected_revision: u64,
        mut grants: Vec<PluginCapabilityGrant>,
        changed_at: DateTime<Utc>,
    ) -> Result<PluginInstallation, StorageError> {
        let transaction = self.connection.transaction()?;
        let mut installation = load_plugin_installation(&transaction, plugin_id)?
            .ok_or_else(|| StorageError::PluginNotFound(plugin_id.into()))?;
        if installation.revision != expected_revision {
            return Err(StorageError::Conflict(format!(
                "plugin installation revision is {}, expected {expected_revision}",
                installation.revision
            )));
        }
        let revision = load_plugin_revision(&transaction, installation.active_revision_id)?.ok_or(
            StorageError::PluginRevisionNotFound(installation.active_revision_id),
        )?;
        normalize_and_validate_plugin_grants(&mut grants, &revision)?;
        if installation.capability_grants == grants {
            return Ok(installation);
        }
        installation.revision = installation
            .revision
            .checked_add(1)
            .ok_or_else(|| StorageError::Conflict("plugin revision is exhausted".into()))?;
        installation.capability_grants = grants;
        installation.enabled = false;
        installation.updated_at = changed_at;
        transaction.execute(
            "UPDATE plugin_installations SET
                enabled = 0, revision = ?1, installation_json = ?2, updated_at = ?3
             WHERE plugin_id = ?4 AND revision = ?5",
            params![
                to_sql_revision(installation.revision)?,
                serde_json::to_string(&installation)?,
                changed_at.to_rfc3339(),
                plugin_id,
                to_sql_revision(expected_revision)?,
            ],
        )?;
        transaction.commit()?;
        Ok(installation)
    }

    /// Returns one installed plugin head.
    ///
    /// # Errors
    /// Returns invalid-data or `SQLite` failures.
    pub fn plugin_installation(
        &self,
        plugin_id: &str,
    ) -> Result<Option<PluginInstallation>, StorageError> {
        load_plugin_installation(&self.connection, plugin_id)
    }

    /// Returns all plugin heads in stable identity order.
    ///
    /// # Errors
    /// Returns invalid-data or `SQLite` failures.
    pub fn plugin_installations(&self) -> Result<Vec<PluginInstallation>, StorageError> {
        let mut statement = self
            .connection
            .prepare("SELECT installation_json FROM plugin_installations ORDER BY plugin_id")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        decode_rows(rows)
    }

    /// Returns one immutable plugin revision.
    ///
    /// # Errors
    /// Returns invalid-data or `SQLite` failures.
    pub fn plugin_revision(
        &self,
        revision_id: PluginRevisionId,
    ) -> Result<Option<PluginRevision>, StorageError> {
        load_plugin_revision(&self.connection, revision_id)
    }

    /// Atomically records one running invocation after proving the exact enabled
    /// revision is still active. A repeated identical identity is observation-only.
    ///
    /// # Errors
    /// Returns disabled/stale identity, conflict, validation, or `SQLite` failures.
    pub fn begin_plugin_invocation(
        &mut self,
        invocation: &PluginInvocationRecord,
    ) -> Result<PluginInvocationRecord, StorageError> {
        validate_running_invocation(invocation)?;
        let transaction = self.connection.transaction()?;
        if let Some(stored) = load_plugin_invocation(&transaction, invocation.id)? {
            if stored == *invocation {
                return Ok(stored);
            }
            return Err(StorageError::Conflict(format!(
                "plugin invocation {} was reused with different content",
                invocation.id
            )));
        }
        let installation = load_plugin_installation(&transaction, &invocation.plugin_id)?
            .ok_or_else(|| StorageError::PluginNotFound(invocation.plugin_id.clone()))?;
        if !installation.enabled || installation.active_revision_id != invocation.plugin_revision_id
        {
            return Err(StorageError::Conflict(
                "plugin invocation does not target the active enabled revision".into(),
            ));
        }
        let revision = load_plugin_revision(&transaction, invocation.plugin_revision_id)?.ok_or(
            StorageError::PluginRevisionNotFound(invocation.plugin_revision_id),
        )?;
        if revision.manifest.id != invocation.plugin_id {
            return Err(StorageError::InvalidData(
                "plugin invocation identity does not match its revision".into(),
            ));
        }
        insert_plugin_invocation(&transaction, invocation)?;
        insert_plugin_invocation_event(&transaction, invocation, invocation.started_at)?;
        transaction.commit()?;
        Ok(invocation.clone())
    }

    /// Atomically settles one running invocation and appends immutable terminal history.
    ///
    /// # Errors
    /// Returns illegal transition, identity, replay, validation, or `SQLite` failures.
    pub fn complete_plugin_invocation(
        &mut self,
        completed: &PluginInvocationRecord,
    ) -> Result<PluginInvocationRecord, StorageError> {
        validate_terminal_invocation(completed)?;
        let transaction = self.connection.transaction()?;
        let current = load_plugin_invocation(&transaction, completed.id)?.ok_or_else(|| {
            StorageError::InvalidData(format!("plugin invocation {} was not found", completed.id))
        })?;
        if current.state.is_terminal() {
            if current == *completed {
                return Ok(current);
            }
            return Err(StorageError::Conflict(
                "terminal plugin invocation is immutable".into(),
            ));
        }
        validate_same_invocation_identity(&current, completed)?;
        let completed_at = completed.completed_at.ok_or_else(|| {
            StorageError::InvalidData("terminal plugin invocation lacks completion time".into())
        })?;
        transaction.execute(
            "UPDATE plugin_invocations SET
                state = ?1, invocation_json = ?2, completed_at = ?3
             WHERE invocation_id = ?4 AND state = 'running'",
            params![
                invocation_state(completed.state),
                serde_json::to_string(completed)?,
                completed_at.to_rfc3339(),
                completed.id.to_string(),
            ],
        )?;
        insert_plugin_invocation_event(&transaction, completed, completed_at)?;
        transaction.commit()?;
        Ok(completed.clone())
    }

    /// Converts every crash-interrupted running plugin invocation to terminal
    /// uncertainty. Such work is never replayed automatically.
    ///
    /// # Errors
    /// Returns invalid-data, serialization, or `SQLite` failures atomically.
    pub fn recover_plugin_invocations(
        &mut self,
        recovered_at: DateTime<Utc>,
    ) -> Result<PluginRecoveryReport, StorageError> {
        let transaction = self.connection.transaction()?;
        let running: Vec<PluginInvocationRecord> = {
            let mut statement = transaction.prepare(
                "SELECT invocation_json FROM plugin_invocations
                 WHERE state = 'running' ORDER BY started_at, invocation_id",
            )?;
            let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
            decode_rows(rows)?
        };
        let count = u64::try_from(running.len())
            .map_err(|_| StorageError::InvalidData("plugin recovery count overflow".into()))?;
        for mut invocation in running {
            invocation.state = PluginInvocationState::Uncertain;
            invocation.result_sha256 = None;
            invocation.error_code = Some("runner_interrupted".into());
            invocation.completed_at = Some(recovered_at);
            transaction.execute(
                "UPDATE plugin_invocations SET
                    state = 'uncertain', invocation_json = ?1, completed_at = ?2
                 WHERE invocation_id = ?3 AND state = 'running'",
                params![
                    serde_json::to_string(&invocation)?,
                    recovered_at.to_rfc3339(),
                    invocation.id.to_string(),
                ],
            )?;
            insert_plugin_invocation_event(&transaction, &invocation, recovered_at)?;
        }
        transaction.commit()?;
        Ok(PluginRecoveryReport {
            invocations_marked_uncertain: count,
        })
    }

    /// Returns one current invocation projection.
    ///
    /// # Errors
    /// Returns invalid-data or `SQLite` failures.
    pub fn plugin_invocation(
        &self,
        invocation_id: mcg_domain::PluginInvocationId,
    ) -> Result<Option<PluginInvocationRecord>, StorageError> {
        load_plugin_invocation(&self.connection, invocation_id)
    }

    /// Returns immutable invocation snapshots in recorded order.
    ///
    /// # Errors
    /// Returns invalid-data or `SQLite` failures.
    pub fn plugin_invocation_history(
        &self,
        invocation_id: mcg_domain::PluginInvocationId,
    ) -> Result<Vec<PluginInvocationRecord>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT invocation_json FROM plugin_invocation_events
             WHERE invocation_id = ?1 ORDER BY sequence",
        )?;
        let rows =
            statement.query_map([invocation_id.to_string()], |row| row.get::<_, String>(0))?;
        decode_rows(rows)
    }
}

fn validate_plugin_revision(revision: &PluginRevision) -> Result<(), StorageError> {
    revision
        .manifest
        .validate()
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    if revision.schema_version != PLUGIN_CATALOG_SCHEMA_VERSION
        || revision.manifest.class != PluginClass::SandboxedWasi
        || !valid_digest(&revision.module_sha256)
        || revision.module_size_bytes == 0
        || revision.module_size_bytes > 64 * 1_024 * 1_024
    {
        return Err(StorageError::InvalidData(
            "plugin revision is outside the supported catalog contract".into(),
        ));
    }
    match &revision.source {
        PluginInstallSource::Registry { registry, package }
            if !bounded_text(registry, 2_048) || !bounded_text(package, 256) =>
        {
            return Err(StorageError::InvalidData(
                "plugin registry provenance is invalid".into(),
            ));
        }
        PluginInstallSource::LocalImport | PluginInstallSource::Registry { .. } => {}
    }
    match revision.signature.state {
        PluginSignatureState::Unsigned
            if revision.signature.scheme.is_some()
                || revision.signature.signer.is_some()
                || revision.signature.signature_sha256.is_some() =>
        {
            return Err(StorageError::InvalidData(
                "unsigned plugin cannot claim signature metadata".into(),
            ));
        }
        PluginSignatureState::Verified
            if !matches!(revision.source, PluginInstallSource::Registry { .. })
                || revision.signature.scheme.as_deref() != Some(PLUGIN_SIGNATURE_SCHEME)
                || !revision
                    .signature
                    .signer
                    .as_deref()
                    .is_some_and(valid_plugin_identifier)
                || !revision
                    .signature
                    .signature_sha256
                    .as_deref()
                    .is_some_and(valid_digest) =>
        {
            return Err(StorageError::InvalidData(
                "verified plugin signature evidence is invalid".into(),
            ));
        }
        PluginSignatureState::Unsigned | PluginSignatureState::Verified => {}
    }
    Ok(())
}

fn valid_plugin_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
        })
}

fn validate_running_invocation(invocation: &PluginInvocationRecord) -> Result<(), StorageError> {
    if invocation.schema_version != PLUGIN_CATALOG_SCHEMA_VERSION
        || invocation.state != PluginInvocationState::Running
        || invocation.result_sha256.is_some()
        || invocation.error_code.is_some()
        || invocation.completed_at.is_some()
        || !valid_digest(&invocation.input_sha256)
    {
        return Err(StorageError::InvalidData(
            "running plugin invocation is invalid".into(),
        ));
    }
    Ok(())
}

fn normalize_and_validate_plugin_grants(
    grants: &mut [PluginCapabilityGrant],
    revision: &PluginRevision,
) -> Result<(), StorageError> {
    if grants.len() > 5 {
        return Err(StorageError::InvalidData(
            "plugin capability grant count is excessive".into(),
        ));
    }
    grants.sort_by_key(|grant| grant.capability);
    let mut capabilities = std::collections::BTreeSet::new();
    for grant in grants {
        grant.scopes.sort();
        if grant.schema_version != PLUGIN_CATALOG_SCHEMA_VERSION
            || !revision
                .manifest
                .requested_capabilities
                .contains(&grant.capability)
            || !capabilities.insert(grant.capability)
            || grant.scopes.is_empty()
            || grant.scopes.len() > 32
        {
            return Err(StorageError::InvalidData(
                "plugin capability grant identity is invalid".into(),
            ));
        }
        let mut scopes = std::collections::BTreeSet::new();
        for scope in &grant.scopes {
            if !scopes.insert(scope.clone()) || !valid_plugin_scope(grant.capability, scope) {
                return Err(StorageError::InvalidData(
                    "plugin capability grant scope is invalid".into(),
                ));
            }
        }
    }
    Ok(())
}

fn validate_grants_cover_manifest(
    grants: &[PluginCapabilityGrant],
    revision: &PluginRevision,
) -> Result<(), StorageError> {
    let mut normalized = grants.to_vec();
    normalize_and_validate_plugin_grants(&mut normalized, revision)?;
    if normalized != grants
        || revision
            .manifest
            .requested_capabilities
            .iter()
            .any(|capability| !grants.iter().any(|grant| grant.capability == *capability))
    {
        return Err(StorageError::Conflict(
            "plugin cannot be enabled until every requested capability has an exact grant".into(),
        ));
    }
    Ok(())
}

fn valid_plugin_scope(capability: PluginCapability, scope: &PluginCapabilityScope) -> bool {
    match (capability, scope) {
        (
            PluginCapability::FilesystemRead | PluginCapability::FilesystemWrite,
            PluginCapabilityScope::TaskProject,
        ) => true,
        (PluginCapability::Network, PluginCapabilityScope::NetworkOrigin { origin }) => {
            url::Url::parse(origin).is_ok_and(|url| {
                matches!(url.scheme(), "http" | "https")
                    && url.host_str().is_some()
                    && url.username().is_empty()
                    && url.password().is_none()
                    && url.query().is_none()
                    && url.fragment().is_none()
                    && url.path() == "/"
                    && url.origin().ascii_serialization() == *origin
            })
        }
        (PluginCapability::Process, PluginCapabilityScope::Executable { executable }) => {
            bounded_scope_text(executable, 1_024)
        }
        (PluginCapability::Credentials, PluginCapabilityScope::Credential { provider, key }) => {
            valid_credential_component(provider) && valid_credential_component(key)
        }
        _ => false,
    }
}

fn bounded_scope_text(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty()
        && value.len() <= maximum
        && !value.contains('\0')
        && !value.chars().any(char::is_control)
}

fn valid_credential_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b':' | b'/')
        })
}

fn validate_terminal_invocation(invocation: &PluginInvocationRecord) -> Result<(), StorageError> {
    if invocation.schema_version != PLUGIN_CATALOG_SCHEMA_VERSION
        || !invocation.state.is_terminal()
        || invocation.completed_at.is_none()
        || invocation.completed_at < Some(invocation.started_at)
        || !valid_digest(&invocation.input_sha256)
    {
        return Err(StorageError::InvalidData(
            "terminal plugin invocation is invalid".into(),
        ));
    }
    let shape_is_valid = match invocation.state {
        PluginInvocationState::Succeeded => {
            invocation
                .result_sha256
                .as_deref()
                .is_some_and(valid_digest)
                && invocation.error_code.is_none()
        }
        PluginInvocationState::Failed | PluginInvocationState::Uncertain => {
            invocation.result_sha256.is_none()
                && invocation
                    .error_code
                    .as_deref()
                    .is_some_and(bounded_error_code)
        }
        PluginInvocationState::Running => false,
    };
    if !shape_is_valid {
        return Err(StorageError::InvalidData(
            "plugin invocation terminal evidence is inconsistent".into(),
        ));
    }
    Ok(())
}

fn validate_same_invocation_identity(
    current: &PluginInvocationRecord,
    completed: &PluginInvocationRecord,
) -> Result<(), StorageError> {
    if current.schema_version != completed.schema_version
        || current.id != completed.id
        || current.plugin_id != completed.plugin_id
        || current.plugin_revision_id != completed.plugin_revision_id
        || current.task_id != completed.task_id
        || current.input_sha256 != completed.input_sha256
        || current.started_at != completed.started_at
    {
        return Err(StorageError::Conflict(
            "plugin invocation completion changed immutable identity".into(),
        ));
    }
    Ok(())
}

fn insert_plugin_invocation(
    transaction: &Transaction<'_>,
    invocation: &PluginInvocationRecord,
) -> Result<(), StorageError> {
    transaction.execute(
        "INSERT INTO plugin_invocations(
            invocation_id, plugin_id, plugin_revision_id, task_id, input_sha256,
            state, invocation_json, started_at, completed_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, 'running', ?6, ?7, NULL)",
        params![
            invocation.id.to_string(),
            invocation.plugin_id,
            invocation.plugin_revision_id.to_string(),
            invocation.task_id.map(|id| id.to_string()),
            invocation.input_sha256,
            serde_json::to_string(invocation)?,
            invocation.started_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

fn insert_plugin_invocation_event(
    transaction: &Transaction<'_>,
    invocation: &PluginInvocationRecord,
    recorded_at: DateTime<Utc>,
) -> Result<(), StorageError> {
    transaction.execute(
        "INSERT INTO plugin_invocation_events(
            invocation_id, state, invocation_json, recorded_at
         ) VALUES (?1, ?2, ?3, ?4)",
        params![
            invocation.id.to_string(),
            invocation_state(invocation.state),
            serde_json::to_string(invocation)?,
            recorded_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

fn load_plugin_installation(
    connection: &rusqlite::Connection,
    plugin_id: &str,
) -> Result<Option<PluginInstallation>, StorageError> {
    let json = connection
        .query_row(
            "SELECT installation_json FROM plugin_installations WHERE plugin_id = ?1",
            [plugin_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    decode_optional(json)
}

fn load_plugin_revision(
    connection: &rusqlite::Connection,
    revision_id: PluginRevisionId,
) -> Result<Option<PluginRevision>, StorageError> {
    let json = connection
        .query_row(
            "SELECT revision_json FROM plugin_revisions WHERE revision_id = ?1",
            [revision_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    decode_optional(json)
}

fn load_plugin_invocation(
    connection: &rusqlite::Connection,
    invocation_id: mcg_domain::PluginInvocationId,
) -> Result<Option<PluginInvocationRecord>, StorageError> {
    let json = connection
        .query_row(
            "SELECT invocation_json FROM plugin_invocations WHERE invocation_id = ?1",
            [invocation_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    decode_optional(json)
}

fn decode_optional<T: serde::de::DeserializeOwned>(
    json: Option<String>,
) -> Result<Option<T>, StorageError> {
    json.map(|value| serde_json::from_str(&value).map_err(StorageError::from))
        .transpose()
}

fn decode_rows<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<String>>,
) -> Result<Vec<T>, StorageError>
where
    T: serde::de::DeserializeOwned,
{
    rows.map(|row| {
        let json = row?;
        serde_json::from_str(&json).map_err(StorageError::from)
    })
    .collect()
}

const fn invocation_state(state: PluginInvocationState) -> &'static str {
    match state {
        PluginInvocationState::Running => "running",
        PluginInvocationState::Succeeded => "succeeded",
        PluginInvocationState::Failed => "failed",
        PluginInvocationState::Uncertain => "uncertain",
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn bounded_text(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty() && value.len() <= maximum && !value.contains('\0')
}

fn bounded_error_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn to_sql_revision(revision: u64) -> Result<i64, StorageError> {
    i64::try_from(revision)
        .map_err(|_| StorageError::InvalidData("plugin revision exceeds SQLite range".into()))
}

#[cfg(test)]
mod tests {
    use chrono::Duration;
    use mcg_domain::{
        PLUGIN_CATALOG_SCHEMA_VERSION, PluginInstallSource, PluginInvocationId,
        PluginInvocationRecord, PluginInvocationState, PluginRevision, PluginRevisionId,
        PluginSignature, PluginSignatureState,
    };
    use mcg_plugin_protocol::{PLUGIN_SCHEMA_VERSION, PluginClass, PluginManifest};
    use tempfile::tempdir;

    use super::*;

    fn revision(at: DateTime<Utc>) -> PluginRevision {
        PluginRevision {
            schema_version: PLUGIN_CATALOG_SCHEMA_VERSION,
            id: PluginRevisionId::new(),
            manifest: PluginManifest {
                schema_version: PLUGIN_SCHEMA_VERSION,
                id: "example.catalog".into(),
                display_name: "Catalog fixture".into(),
                version: "1.0.0".into(),
                class: PluginClass::SandboxedWasi,
                requested_capabilities: Vec::new(),
            },
            module_sha256: "a".repeat(64),
            module_size_bytes: 1_024,
            source: PluginInstallSource::LocalImport,
            signature: PluginSignature {
                state: PluginSignatureState::Unsigned,
                scheme: None,
                signer: None,
                signature_sha256: None,
            },
            installed_at: at,
        }
    }

    fn running(revision: &PluginRevision, at: DateTime<Utc>) -> PluginInvocationRecord {
        PluginInvocationRecord {
            schema_version: PLUGIN_CATALOG_SCHEMA_VERSION,
            id: PluginInvocationId::new(),
            plugin_id: revision.manifest.id.clone(),
            plugin_revision_id: revision.id,
            task_id: None,
            input_sha256: "b".repeat(64),
            state: PluginInvocationState::Running,
            result_sha256: None,
            error_code: None,
            started_at: at,
            completed_at: None,
        }
    }

    #[test]
    fn catalog_revisions_are_atomic_immutable_and_restart_durable() {
        let directory = tempdir().expect("tempdir");
        let database = directory.path().join("mcg.sqlite3");
        let at = Utc::now();
        let plugin = revision(at);
        let mut updated_plugin = plugin.clone();
        updated_plugin.id = PluginRevisionId::new();
        updated_plugin.manifest.version = "1.1.0".into();
        updated_plugin.module_sha256 = "e".repeat(64);
        updated_plugin.installed_at = at + Duration::seconds(2);
        {
            let mut store = SqliteEventStore::open(&database).expect("store");
            let installation = store
                .install_plugin_revision(&plugin, false)
                .expect("install");
            assert_eq!(installation.revision, 1);
            assert!(!installation.enabled);
            let mut unverified_claim = plugin.clone();
            unverified_claim.id = PluginRevisionId::new();
            unverified_claim.signature = PluginSignature {
                state: PluginSignatureState::Verified,
                scheme: Some("fixture".into()),
                signer: Some("untrusted caller".into()),
                signature_sha256: Some("d".repeat(64)),
            };
            assert!(matches!(
                store.install_plugin_revision(&unverified_claim, false),
                Err(StorageError::InvalidData(_))
            ));
            assert_eq!(
                store
                    .install_plugin_revision(&plugin, true)
                    .expect("exact replay"),
                installation
            );
            assert!(
                store
                    .connection
                    .execute(
                        "UPDATE plugin_revisions SET module_sha256 = ?1 WHERE revision_id = ?2",
                        params!["c".repeat(64), plugin.id.to_string()],
                    )
                    .is_err()
            );
            let enabled = store
                .set_plugin_enabled(&plugin.manifest.id, 1, true, at + Duration::seconds(1))
                .expect("enable");
            assert_eq!(enabled.revision, 2);
            assert!(enabled.enabled);
            assert!(matches!(
                store.set_plugin_enabled(&plugin.manifest.id, 1, false, at),
                Err(StorageError::Conflict(_))
            ));
            let updated = store
                .install_plugin_revision(&updated_plugin, true)
                .expect("install update");
            assert_eq!(updated.revision, 3);
            assert_eq!(updated.active_revision_id, updated_plugin.id);
            assert!(matches!(
                store.install_plugin_revision(&plugin, true),
                Err(StorageError::Conflict(_))
            ));
        }
        let store = SqliteEventStore::open(&database).expect("reopen");
        assert_eq!(
            store.plugin_revision(plugin.id).expect("revision"),
            Some(plugin)
        );
        assert!(
            store
                .plugin_installation("example.catalog")
                .expect("installation")
                .expect("installed")
                .active_revision_id
                == updated_plugin.id
        );
        assert_eq!(store.plugin_installations().expect("catalog").len(), 1);
        assert_eq!(
            store
                .plugin_revision(updated_plugin.id)
                .expect("updated revision"),
            Some(updated_plugin)
        );
    }

    #[test]
    fn invocation_terminal_replay_and_crash_recovery_are_consistent() {
        let directory = tempdir().expect("tempdir");
        let database = directory.path().join("mcg.sqlite3");
        let at = Utc::now();
        let plugin = revision(at);
        let first = running(&plugin, at + Duration::seconds(1));
        let second = running(&plugin, at + Duration::seconds(2));
        {
            let mut store = SqliteEventStore::open(&database).expect("store");
            store
                .install_plugin_revision(&plugin, true)
                .expect("install");
            store.begin_plugin_invocation(&first).expect("begin first");
            let mut completed = first.clone();
            completed.state = PluginInvocationState::Succeeded;
            completed.result_sha256 = Some("c".repeat(64));
            completed.completed_at = Some(at + Duration::seconds(3));
            assert_eq!(
                store
                    .complete_plugin_invocation(&completed)
                    .expect("complete"),
                completed
            );
            assert_eq!(
                store
                    .complete_plugin_invocation(&completed)
                    .expect("terminal replay"),
                completed
            );
            assert_eq!(
                store
                    .plugin_invocation_history(first.id)
                    .expect("history")
                    .len(),
                2
            );
            store
                .begin_plugin_invocation(&second)
                .expect("begin second");
        }

        let recovered_at = at + Duration::seconds(10);
        let mut store = SqliteEventStore::open(&database).expect("reopen");
        assert_eq!(
            store
                .recover_plugin_invocations(recovered_at)
                .expect("recover")
                .invocations_marked_uncertain,
            1
        );
        let recovered = store
            .plugin_invocation(second.id)
            .expect("invocation")
            .expect("stored");
        assert_eq!(recovered.state, PluginInvocationState::Uncertain);
        assert_eq!(recovered.error_code.as_deref(), Some("runner_interrupted"));
        assert_eq!(
            store
                .plugin_invocation_history(second.id)
                .expect("history")
                .len(),
            2
        );
        assert_eq!(
            store
                .recover_plugin_invocations(recovered_at + Duration::seconds(1))
                .expect("idempotent recovery")
                .invocations_marked_uncertain,
            0
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One lifecycle proves grant, enable, revoke, conflict, and reopen atomically.
    fn capability_grants_are_scoped_optimistic_and_disable_on_change() {
        let directory = tempdir().expect("tempdir");
        let database = directory.path().join("mcg.sqlite3");
        let at = Utc::now();
        let mut plugin = revision(at);
        plugin.manifest.requested_capabilities =
            vec![PluginCapability::Network, PluginCapability::FilesystemRead];
        let mut store = SqliteEventStore::open(&database).expect("store");
        let installed = store
            .install_unsigned_local_plugin_revision(&plugin)
            .expect("install");
        assert!(matches!(
            store.set_plugin_enabled(&plugin.manifest.id, installed.revision, true, at),
            Err(StorageError::Conflict(_))
        ));

        let invalid = vec![PluginCapabilityGrant {
            schema_version: PLUGIN_CATALOG_SCHEMA_VERSION,
            capability: PluginCapability::Network,
            scopes: vec![PluginCapabilityScope::TaskProject],
        }];
        assert!(matches!(
            store.set_plugin_capability_grants(
                &plugin.manifest.id,
                installed.revision,
                invalid,
                at + Duration::seconds(1),
            ),
            Err(StorageError::InvalidData(_))
        ));

        let grants = vec![
            PluginCapabilityGrant {
                schema_version: PLUGIN_CATALOG_SCHEMA_VERSION,
                capability: PluginCapability::Network,
                scopes: vec![PluginCapabilityScope::NetworkOrigin {
                    origin: "http://127.0.0.1:3000".into(),
                }],
            },
            PluginCapabilityGrant {
                schema_version: PLUGIN_CATALOG_SCHEMA_VERSION,
                capability: PluginCapability::FilesystemRead,
                scopes: vec![PluginCapabilityScope::TaskProject],
            },
        ];
        let granted = store
            .set_plugin_capability_grants(
                &plugin.manifest.id,
                installed.revision,
                grants.clone(),
                at + Duration::seconds(2),
            )
            .expect("grant");
        assert_eq!(granted.revision, 2);
        assert!(!granted.enabled);
        assert_eq!(
            granted.capability_grants[0].capability,
            PluginCapability::FilesystemRead
        );
        let enabled = store
            .set_plugin_enabled(
                &plugin.manifest.id,
                granted.revision,
                true,
                at + Duration::seconds(3),
            )
            .expect("enable");
        assert!(enabled.enabled);
        assert_eq!(enabled.revision, 3);
        assert!(matches!(
            store.set_plugin_capability_grants(
                &plugin.manifest.id,
                granted.revision,
                grants.clone(),
                at + Duration::seconds(4),
            ),
            Err(StorageError::Conflict(_))
        ));

        let changed = vec![
            PluginCapabilityGrant {
                schema_version: PLUGIN_CATALOG_SCHEMA_VERSION,
                capability: PluginCapability::FilesystemRead,
                scopes: vec![PluginCapabilityScope::TaskProject],
            },
            PluginCapabilityGrant {
                schema_version: PLUGIN_CATALOG_SCHEMA_VERSION,
                capability: PluginCapability::Network,
                scopes: vec![PluginCapabilityScope::NetworkOrigin {
                    origin: "https://api.example".into(),
                }],
            },
        ];
        let disabled = store
            .set_plugin_capability_grants(
                &plugin.manifest.id,
                enabled.revision,
                changed,
                at + Duration::seconds(5),
            )
            .expect("change grants");
        assert_eq!(disabled.revision, 4);
        assert!(!disabled.enabled);
        drop(store);

        let reopened = SqliteEventStore::open(database).expect("reopen");
        assert_eq!(
            reopened
                .plugin_installation(&plugin.manifest.id)
                .expect("installation")
                .expect("installed"),
            disabled
        );
    }
}
