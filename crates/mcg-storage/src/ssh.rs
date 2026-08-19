use chrono::{DateTime, Utc};
use mcg_domain::{
    SSH_SCHEMA_VERSION, SshHost, SshHostId, SshHostRevision, SshHostState, SshOperation,
    SshOperationId, SshOperationState,
};
use rusqlite::{Transaction, params};

use crate::{SqliteEventStore, StorageError};

impl SqliteEventStore {
    /// Creates one pinned SSH host and its immutable initial revision atomically.
    /// Exact caller-ID replay is observation-only.
    ///
    /// # Errors
    /// Returns an error for invalid definitions, conflicts, or persistence failure.
    pub fn create_ssh_host(
        &mut self,
        host: &SshHost,
        revision: &SshHostRevision,
    ) -> Result<(SshHost, bool), StorageError> {
        validate_host(host)?;
        validate_revision(host, revision)?;
        if let Some(existing) = self.ssh_host(host.id)? {
            if existing.definition_hash == host.definition_hash {
                return Ok((existing, false));
            }
            return Err(StorageError::Conflict(format!(
                "SSH host {} already exists with a different definition",
                host.id
            )));
        }
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO ssh_hosts(host_id, state, revision, definition_hash, host_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                host.id.to_string(),
                enum_json(&host.state)?,
                i64::from(host.revision),
                host.definition_hash,
                serde_json::to_string(host)?,
                host.created_at.to_rfc3339(),
                host.updated_at.to_rfc3339(),
            ],
        )?;
        insert_revision(&transaction, revision)?;
        transaction.commit()?;
        Ok((host.clone(), true))
    }

    /// Advances a host only from its expected revision and retains immutable history.
    ///
    /// # Errors
    /// Returns an error for stale authority, invalid definitions, or persistence failure.
    pub fn update_ssh_host(
        &mut self,
        expected_revision: u32,
        host: &SshHost,
        revision: &SshHostRevision,
    ) -> Result<SshHost, StorageError> {
        validate_host(host)?;
        validate_revision(host, revision)?;
        if host.revision != expected_revision.saturating_add(1) {
            return Err(StorageError::InvalidData(
                "SSH host revision does not follow expected revision".into(),
            ));
        }
        let existing = self
            .ssh_host(host.id)?
            .ok_or(StorageError::SshHostNotFound(host.id))?;
        if existing.state == SshHostState::Archived {
            return Err(StorageError::Conflict(
                "archived SSH hosts cannot be changed".into(),
            ));
        }
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE ssh_hosts
             SET state = ?1, revision = ?2, definition_hash = ?3, host_json = ?4, updated_at = ?5
             WHERE host_id = ?6 AND revision = ?7",
            params![
                enum_json(&host.state)?,
                i64::from(host.revision),
                host.definition_hash,
                serde_json::to_string(host)?,
                host.updated_at.to_rfc3339(),
                host.id.to_string(),
                i64::from(expected_revision),
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::Conflict(
                "SSH host revision authority changed".into(),
            ));
        }
        insert_revision(&transaction, revision)?;
        transaction.commit()?;
        Ok(host.clone())
    }

    /// Loads one current SSH host projection.
    ///
    /// # Errors
    /// Returns an error when storage cannot be read or decoded.
    pub fn ssh_host(&self, host_id: SshHostId) -> Result<Option<SshHost>, StorageError> {
        self.read_optional_json(
            "SELECT host_json FROM ssh_hosts WHERE host_id = ?1",
            &host_id.to_string(),
        )
    }

    /// Lists current SSH host projections.
    ///
    /// # Errors
    /// Returns an error when storage cannot be read or decoded.
    pub fn ssh_hosts(&self) -> Result<Vec<SshHost>, StorageError> {
        let mut statement = self
            .connection
            .prepare("SELECT host_json FROM ssh_hosts ORDER BY created_at, host_id")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.map(|row| serde_json::from_str(&row?).map_err(StorageError::from))
            .collect()
    }

    /// Lists immutable revision history for one SSH host.
    ///
    /// # Errors
    /// Returns an error when storage cannot be read or decoded.
    pub fn ssh_host_revisions(
        &self,
        host_id: SshHostId,
    ) -> Result<Vec<SshHostRevision>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT revision_json FROM ssh_host_revisions
             WHERE host_id = ?1 ORDER BY revision",
        )?;
        let rows = statement.query_map([host_id.to_string()], |row| row.get::<_, String>(0))?;
        rows.map(|row| serde_json::from_str(&row?).map_err(StorageError::from))
            .collect()
    }

    /// Claims one remote operation before a process or transfer starts.
    /// Exact replay returns the existing operation.
    ///
    /// # Errors
    /// Returns an error for identity conflicts, inactive hosts, or persistence failure.
    pub fn claim_ssh_operation(
        &mut self,
        operation: &SshOperation,
    ) -> Result<(SshOperation, bool), StorageError> {
        validate_operation(operation)?;
        let host = self
            .ssh_host(operation.host_id)?
            .ok_or(StorageError::SshHostNotFound(operation.host_id))?;
        if host.state != SshHostState::Active || host.revision != operation.host_revision {
            return Err(StorageError::Conflict(
                "SSH host is inactive or changed after authority was issued".into(),
            ));
        }
        if let Some(existing) = self.ssh_operation(operation.id)? {
            if existing.request_hash == operation.request_hash {
                return Ok((existing, false));
            }
            return Err(StorageError::Conflict(
                "SSH operation identity was reused with a different request".into(),
            ));
        }
        self.connection.execute(
            "INSERT INTO ssh_operations(operation_id, host_id, state, request_hash, operation_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                operation.id.to_string(),
                operation.host_id.to_string(),
                enum_json(&operation.state)?,
                operation.request_hash,
                serde_json::to_string(operation)?,
                operation.created_at.to_rfc3339(),
                operation.updated_at.to_rfc3339(),
            ],
        )?;
        Ok((operation.clone(), true))
    }

    /// Loads one SSH operation by caller-owned identity.
    ///
    /// # Errors
    /// Returns an error when storage cannot be read or decoded.
    pub fn ssh_operation(
        &self,
        operation_id: SshOperationId,
    ) -> Result<Option<SshOperation>, StorageError> {
        self.read_optional_json(
            "SELECT operation_json FROM ssh_operations WHERE operation_id = ?1",
            &operation_id.to_string(),
        )
    }

    /// Lists retained SSH operation history for one host.
    ///
    /// # Errors
    /// Returns an error when storage cannot be read or decoded.
    pub fn ssh_operations(&self, host_id: SshHostId) -> Result<Vec<SshOperation>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT operation_json FROM ssh_operations
             WHERE host_id = ?1 ORDER BY created_at DESC, operation_id DESC",
        )?;
        let rows = statement.query_map([host_id.to_string()], |row| row.get::<_, String>(0))?;
        rows.map(|row| serde_json::from_str(&row?).map_err(StorageError::from))
            .collect()
    }

    /// Settles one claimed operation exactly once.
    ///
    /// # Errors
    /// Returns an error for missing or already-settled operations.
    pub fn finish_ssh_operation(
        &mut self,
        operation_id: SshOperationId,
        state: SshOperationState,
        detail: &str,
        exit_code: Option<i32>,
        artifact_sha256: Vec<String>,
        now: DateTime<Utc>,
    ) -> Result<SshOperation, StorageError> {
        if !matches!(
            state,
            SshOperationState::Succeeded | SshOperationState::Failed
        ) {
            return Err(StorageError::InvalidData(
                "SSH operation settlement must be terminal".into(),
            ));
        }
        let mut operation = self
            .ssh_operation(operation_id)?
            .ok_or_else(|| StorageError::InvalidData("SSH operation was not found".into()))?;
        if operation.state != SshOperationState::Executing {
            return Err(StorageError::Conflict(
                "SSH operation is already settled".into(),
            ));
        }
        operation.state = state;
        operation.detail = detail.chars().take(1_024).collect();
        operation.exit_code = exit_code;
        operation.artifact_sha256 = artifact_sha256;
        operation.updated_at = now;
        let changed = self.connection.execute(
            "UPDATE ssh_operations SET state = ?1, operation_json = ?2, updated_at = ?3
             WHERE operation_id = ?4 AND state = 'executing'",
            params![
                enum_json(&state)?,
                serde_json::to_string(&operation)?,
                now.to_rfc3339(),
                operation_id.to_string(),
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::Conflict(
                "SSH operation settlement authority changed".into(),
            ));
        }
        Ok(operation)
    }

    /// Converts crash-interrupted operations to non-replayable uncertainty.
    ///
    /// # Errors
    /// Returns an error when recovery cannot commit.
    pub fn recover_ssh_operations(&mut self, now: DateTime<Utc>) -> Result<u64, StorageError> {
        let ids = {
            let mut statement = self
                .connection
                .prepare("SELECT operation_id FROM ssh_operations WHERE state = 'executing'")?;
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?
        };
        let mut recovered = 0_u64;
        for value in ids {
            let id = value
                .parse::<SshOperationId>()
                .map_err(|_| StorageError::InvalidData("invalid SSH operation UUID".into()))?;
            let mut operation = self
                .ssh_operation(id)?
                .ok_or_else(|| StorageError::InvalidData("SSH operation disappeared".into()))?;
            operation.state = SshOperationState::Uncertain;
            operation.detail = "daemon restarted before remote completion was verified".into();
            operation.updated_at = now;
            let changed = self.connection.execute(
                "UPDATE ssh_operations SET state = 'uncertain', operation_json = ?1, updated_at = ?2
                 WHERE operation_id = ?3 AND state = 'executing'",
                params![
                    serde_json::to_string(&operation)?,
                    now.to_rfc3339(),
                    id.to_string(),
                ],
            )?;
            recovered = recovered.saturating_add(u64::try_from(changed).map_err(|_| {
                StorageError::InvalidData("SSH recovery count exceeds database range".into())
            })?);
        }
        Ok(recovered)
    }
}

fn insert_revision(
    transaction: &Transaction<'_>,
    revision: &SshHostRevision,
) -> Result<(), StorageError> {
    transaction.execute(
        "INSERT INTO ssh_host_revisions(revision_id, host_id, revision, definition_hash, revision_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            revision.id.to_string(),
            revision.host_id.to_string(),
            i64::from(revision.revision),
            revision.definition_hash,
            serde_json::to_string(revision)?,
            revision.created_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

fn validate_host(host: &SshHost) -> Result<(), StorageError> {
    let expected_target = if host.port == 22 {
        host.hostname.clone()
    } else {
        format!("[{}]:{}", host.hostname, host.port)
    };
    let mut fields = host.known_hosts_entry.split_whitespace();
    let targets = fields.next().unwrap_or_default();
    let algorithm = fields.next().unwrap_or_default();
    let key = fields.next().unwrap_or_default();
    let valid = host.schema_version == SSH_SCHEMA_VERSION
        && host.revision > 0
        && host.definition_hash.len() == 64
        && host
            .definition_hash
            .bytes()
            .all(|value| value.is_ascii_hexdigit())
        && !host.display_name.trim().is_empty()
        && host.display_name.chars().count() <= 160
        && valid_atom(&host.hostname, 253, ".-:")
        && host.port > 0
        && valid_atom(&host.username, 128, "._-")
        && targets.split(',').any(|target| target == expected_target)
        && matches!(
            algorithm,
            "ssh-ed25519"
                | "ssh-rsa"
                | "ecdsa-sha2-nistp256"
                | "ecdsa-sha2-nistp384"
                | "ecdsa-sha2-nistp521"
        )
        && !key.is_empty()
        && fields.next().is_none()
        && host.known_hosts_entry.chars().count() <= 8_192
        && host.host_key_sha256.starts_with("SHA256:")
        && host.host_key_sha256.chars().count() <= 96
        && valid_capabilities(&host.capabilities)
        && host.created_at <= host.updated_at;
    if !valid {
        return Err(StorageError::InvalidData(
            "invalid pinned SSH host definition".into(),
        ));
    }
    Ok(())
}

fn valid_capabilities(values: &[String]) -> bool {
    values.len() <= 64
        && values.windows(2).all(|pair| pair[0] < pair[1])
        && values.iter().all(|value| {
            !value.is_empty()
                && value.chars().count() <= 96
                && value.chars().all(|character| {
                    character.is_ascii_lowercase()
                        || character.is_ascii_digit()
                        || matches!(character, ':' | '.' | '_' | '-')
                })
        })
}

fn validate_revision(host: &SshHost, revision: &SshHostRevision) -> Result<(), StorageError> {
    if revision.schema_version != SSH_SCHEMA_VERSION
        || revision.host_id != host.id
        || revision.revision != host.revision
        || revision.definition_hash != host.definition_hash
        || revision.host != *host
        || revision.reason.trim().is_empty()
        || revision.reason.chars().count() > 512
    {
        return Err(StorageError::InvalidData(
            "invalid SSH host revision".into(),
        ));
    }
    Ok(())
}

fn validate_operation(operation: &SshOperation) -> Result<(), StorageError> {
    if operation.schema_version != SSH_SCHEMA_VERSION
        || operation.host_revision == 0
        || operation.request_hash.len() != 64
        || !operation
            .request_hash
            .bytes()
            .all(|value| value.is_ascii_hexdigit())
        || operation.state != SshOperationState::Executing
        || operation.created_at > operation.updated_at
    {
        return Err(StorageError::InvalidData("invalid SSH operation".into()));
    }
    Ok(())
}

fn valid_atom(value: &str, max: usize, punctuation: &str) -> bool {
    !value.is_empty()
        && value.chars().count() <= max
        && !value.starts_with('-')
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || punctuation.contains(character))
}

fn enum_json<T: serde::Serialize>(value: &T) -> Result<String, StorageError> {
    Ok(serde_json::to_string(value)?.trim_matches('"').to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcg_domain::{SshAuthentication, SshHostPlatform, SshHostRevisionId, SshOperationKind};

    fn fixture_host(now: DateTime<Utc>) -> (SshHost, SshHostRevision) {
        let host = SshHost {
            schema_version: SSH_SCHEMA_VERSION,
            id: SshHostId::new(),
            definition_hash: "a".repeat(64),
            revision: 1,
            display_name: "Ubuntu builder".into(),
            hostname: "192.0.2.10".into(),
            port: 22,
            username: "builder".into(),
            platform: SshHostPlatform::Posix,
            authentication: SshAuthentication::Agent,
            known_hosts_entry: "192.0.2.10 ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIFixture".into(),
            host_key_sha256: "SHA256:fixtureFingerprint".into(),
            capabilities: vec!["os:linux".into()],
            state: SshHostState::Active,
            created_at: now,
            updated_at: now,
        };
        let revision = SshHostRevision {
            schema_version: SSH_SCHEMA_VERSION,
            id: SshHostRevisionId::new(),
            host_id: host.id,
            revision: 1,
            definition_hash: host.definition_hash.clone(),
            host: host.clone(),
            reason: "initial registration".into(),
            created_at: now,
        };
        (host, revision)
    }

    #[test]
    fn host_revision_and_operation_recovery_are_durable() {
        let directory = tempfile::tempdir().expect("directory");
        let database = directory.path().join("ssh.sqlite3");
        let now = Utc::now();
        let (host, revision) = fixture_host(now);
        let operation = SshOperation {
            schema_version: SSH_SCHEMA_VERSION,
            id: SshOperationId::new(),
            host_id: host.id,
            host_revision: 1,
            task_id: None,
            request_hash: "b".repeat(64),
            kind: SshOperationKind::Inspect,
            state: SshOperationState::Executing,
            detail: String::new(),
            exit_code: None,
            artifact_sha256: Vec::new(),
            created_at: now,
            updated_at: now,
        };
        {
            let mut store = SqliteEventStore::open(&database).expect("store");
            assert!(store.create_ssh_host(&host, &revision).expect("host").1);
            assert!(!store.create_ssh_host(&host, &revision).expect("replay").1);
            assert!(store.claim_ssh_operation(&operation).expect("claim").1);
        }
        let mut reopened = SqliteEventStore::open(&database).expect("reopen");
        assert_eq!(reopened.recover_ssh_operations(now).expect("recover"), 1);
        assert_eq!(
            reopened
                .ssh_operation(operation.id)
                .expect("operation")
                .expect("present")
                .state,
            SshOperationState::Uncertain
        );
        assert!(
            reopened
                .connection
                .execute("DELETE FROM ssh_host_revisions", [])
                .is_err()
        );
        assert!(
            reopened
                .connection
                .execute("DELETE FROM ssh_operations", [])
                .is_err()
        );
    }

    #[test]
    fn host_key_target_and_operation_identity_fail_closed() {
        let now = Utc::now();
        let mut store = SqliteEventStore::open_in_memory().expect("store");
        let (host, revision) = fixture_host(now);
        store.create_ssh_host(&host, &revision).expect("host");
        let mut wrong_key = host.clone();
        wrong_key.id = SshHostId::new();
        wrong_key.known_hosts_entry =
            "attacker.example ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIFixture".into();
        let mut wrong_revision = revision.clone();
        wrong_revision.id = SshHostRevisionId::new();
        wrong_revision.host_id = wrong_key.id;
        wrong_revision.host = wrong_key.clone();
        assert!(matches!(
            store.create_ssh_host(&wrong_key, &wrong_revision),
            Err(StorageError::InvalidData(_))
        ));

        let operation = SshOperation {
            schema_version: SSH_SCHEMA_VERSION,
            id: SshOperationId::new(),
            host_id: host.id,
            host_revision: 1,
            task_id: None,
            request_hash: "b".repeat(64),
            kind: SshOperationKind::Inspect,
            state: SshOperationState::Executing,
            detail: String::new(),
            exit_code: None,
            artifact_sha256: Vec::new(),
            created_at: now,
            updated_at: now,
        };
        store.claim_ssh_operation(&operation).expect("claim");
        let mut conflict = operation.clone();
        conflict.request_hash = "c".repeat(64);
        assert!(matches!(
            store.claim_ssh_operation(&conflict),
            Err(StorageError::Conflict(_))
        ));
    }
}
