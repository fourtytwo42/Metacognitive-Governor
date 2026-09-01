use chrono::{DateTime, Utc};
use mcg_contracts::{
    ActionExecutionState, CapabilityProfile, ContentHash, EffectClass,
    MAX_TOOL_CAPABILITY_SUMMARIES, PermitId, PolicyBinding, ReconciliationJobId,
    ReconciliationJobState, SecretHandleId, SkillManifest, SkillManifestId, SourceRef,
    StableTypeId, TOOL_INVOCATION_PERMIT_HASH_DOMAIN, TOOL_POLICY_HASH_DOMAIN,
    ToolAuthorizationInspection, ToolAuthorizationState, ToolAuthorizationSummary,
    ToolCapabilityScopePreview, ToolInvocationPermit, ToolInvocationProposal, ToolManifest,
    ToolManifestId, ToolPolicy, VersionedPayload, VersionedRef, WaitReason, WorkflowState,
    capability_profile_hash, decode_canonical_cbor_v1, skill_manifest_hash,
    tool_capability_set_hash, tool_invocation_permit_hash, tool_invocation_proposal_hash,
    tool_manifest_hash,
};
use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{SqliteEventStore, StorageError};

const TOOL_PERMIT_SOURCE_TYPE: &str = "mcg.tool_invocation_permit.v1";
const RECONCILIATION_PROBE_BINDING_TYPE: &str = "mcg.reconciliation_probe_binding.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolSecretHandleBinding {
    pub handle_id: SecretHandleId,
    pub permit_id: PermitId,
    pub slot: StableTypeId,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCapabilityAuthorizationReceipt {
    pub proposal: ToolInvocationProposal,
    pub proposal_hash: ContentHash,
    pub permit: ToolInvocationPermit,
    pub permit_hash: ContentHash,
    pub permit_ref: SourceRef,
    pub secret_handles: Vec<ToolSecretHandleBinding>,
}

impl SqliteEventStore {
    /// Registers one immutable Tool manifest version and advances its head by exactly one.
    /// Exact replay is idempotent; skipped versions or substituted content fail closed.
    ///
    /// # Errors
    /// Returns a storage error for malformed, conflicting, or non-monotonic manifests.
    pub fn register_tool_manifest(
        &mut self,
        manifest: &ToolManifest,
        registered_at: DateTime<Utc>,
    ) -> Result<VersionedRef<ToolManifestId>, StorageError> {
        manifest
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let reference = manifest
            .versioned_ref()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        if let Some(existing) = self.tool_manifest(manifest.id, manifest.version)? {
            if existing != *manifest {
                return Err(StorageError::Conflict(
                    "Tool manifest version was replayed with substituted content".into(),
                ));
            }
            let head = self.current_tool_manifest(manifest.id)?.ok_or_else(|| {
                StorageError::InvalidData("Tool manifest version has no durable head".into())
            })?;
            if head.version < manifest.version {
                return Err(StorageError::InvalidData(
                    "Tool manifest head precedes a registered version".into(),
                ));
            }
            return Ok(reference);
        }

        let current = self
            .connection
            .query_row(
                "SELECT current_version FROM tool_manifest_heads WHERE tool_manifest_id = ?1",
                [manifest.id.to_string()],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .map(db_version_from_i64)
            .transpose()?;
        match current {
            None if manifest.version != 1 => {
                return Err(StorageError::Conflict(
                    "first Tool manifest version must be one".into(),
                ));
            }
            Some(current) if manifest.version != current + 1 => {
                return Err(StorageError::Conflict(
                    "Tool manifest head must advance exactly one version".into(),
                ));
            }
            _ => {}
        }

        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO tool_manifest_versions(
                tool_manifest_id, version, manifest_hash_json, manifest_json, registered_at
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                manifest.id.to_string(),
                db_version(manifest.version)?,
                serde_json::to_string(&reference.content_hash)?,
                serde_json::to_string(manifest)?,
                registered_at.to_rfc3339(),
            ],
        )?;
        if current.is_some() {
            transaction.execute(
                "UPDATE tool_manifest_heads
                 SET current_version = ?2, current_hash_json = ?3, updated_at = ?4
                 WHERE tool_manifest_id = ?1",
                params![
                    manifest.id.to_string(),
                    db_version(manifest.version)?,
                    serde_json::to_string(&reference.content_hash)?,
                    registered_at.to_rfc3339(),
                ],
            )?;
        } else {
            transaction.execute(
                "INSERT INTO tool_manifest_heads(
                    tool_manifest_id, current_version, current_hash_json, updated_at
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    manifest.id.to_string(),
                    db_version(manifest.version)?,
                    serde_json::to_string(&reference.content_hash)?,
                    registered_at.to_rfc3339(),
                ],
            )?;
        }
        transaction.commit()?;
        Ok(reference)
    }

    /// Loads and revalidates one exact immutable Tool manifest version.
    ///
    /// # Errors
    /// Returns a storage error for malformed or hash-inconsistent durable evidence.
    pub fn tool_manifest(
        &self,
        id: ToolManifestId,
        version: u64,
    ) -> Result<Option<ToolManifest>, StorageError> {
        let row = self
            .connection
            .query_row(
                "SELECT manifest_hash_json, manifest_json
                 FROM tool_manifest_versions
                 WHERE tool_manifest_id = ?1 AND version = ?2",
                params![id.to_string(), db_version(version)?],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        row.map(|(hash, manifest)| decode_tool_manifest(id, version, &hash, &manifest))
            .transpose()
    }

    /// Loads the current immutable Tool manifest head and revalidates its exact hash.
    ///
    /// # Errors
    /// Returns a storage error for missing or inconsistent head evidence.
    pub fn current_tool_manifest(
        &self,
        id: ToolManifestId,
    ) -> Result<Option<ToolManifest>, StorageError> {
        let head = self
            .connection
            .query_row(
                "SELECT current_version, current_hash_json
                 FROM tool_manifest_heads WHERE tool_manifest_id = ?1",
                [id.to_string()],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        let Some((version, expected_hash)) = head else {
            return Ok(None);
        };
        let version = db_version_from_i64(version)?;
        let manifest = self.tool_manifest(id, version)?.ok_or_else(|| {
            StorageError::InvalidData("Tool manifest head has no immutable version".into())
        })?;
        if serde_json::from_str::<ContentHash>(&expected_hash)?
            != tool_manifest_hash(&manifest)
                .map_err(|error| StorageError::InvalidData(error.to_string()))?
        {
            return Err(StorageError::InvalidData(
                "Tool manifest head hash differs from its version".into(),
            ));
        }
        Ok(Some(manifest))
    }

    /// Registers one immutable Skill manifest after resolving every exact Tool dependency.
    ///
    /// # Errors
    /// Returns a storage error for malformed, missing, substituted, or non-monotonic evidence.
    pub fn register_skill_manifest(
        &mut self,
        manifest: &SkillManifest,
        registered_at: DateTime<Utc>,
    ) -> Result<VersionedRef<SkillManifestId>, StorageError> {
        manifest
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        for required in &manifest.required_tools {
            let tool = self
                .tool_manifest(required.id, required.version)?
                .ok_or_else(|| {
                    StorageError::InvalidData("required Tool is not registered".into())
                })?;
            if tool.versioned_ref().map_err(contract_error)? != *required {
                return Err(StorageError::Conflict(
                    "Skill required Tool reference was substituted".into(),
                ));
            }
        }
        let reference = manifest.versioned_ref().map_err(contract_error)?;
        if let Some(existing) = self.skill_manifest(manifest.id, manifest.version)? {
            if existing != *manifest {
                return Err(StorageError::Conflict(
                    "Skill manifest version was replayed with substituted content".into(),
                ));
            }
            let head = self.current_skill_manifest(manifest.id)?.ok_or_else(|| {
                StorageError::InvalidData("Skill manifest version has no durable head".into())
            })?;
            if head.version < manifest.version {
                return Err(StorageError::InvalidData(
                    "Skill manifest head precedes a registered version".into(),
                ));
            }
            return Ok(reference);
        }
        let current = self
            .connection
            .query_row(
                "SELECT current_version FROM skill_manifest_heads WHERE skill_manifest_id = ?1",
                [manifest.id.to_string()],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .map(db_version_from_i64)
            .transpose()?;
        match current {
            None if manifest.version != 1 => {
                return Err(StorageError::Conflict(
                    "first Skill manifest version must be one".into(),
                ));
            }
            Some(current) if manifest.version != current + 1 => {
                return Err(StorageError::Conflict(
                    "Skill manifest head must advance exactly one version".into(),
                ));
            }
            _ => {}
        }

        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO skill_manifest_versions(
                skill_manifest_id, version, manifest_hash_json, manifest_json, registered_at
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                manifest.id.to_string(),
                db_version(manifest.version)?,
                serde_json::to_string(&reference.content_hash)?,
                serde_json::to_string(manifest)?,
                registered_at.to_rfc3339(),
            ],
        )?;
        if current.is_some() {
            transaction.execute(
                "UPDATE skill_manifest_heads
                 SET current_version = ?2, current_hash_json = ?3, updated_at = ?4
                 WHERE skill_manifest_id = ?1",
                params![
                    manifest.id.to_string(),
                    db_version(manifest.version)?,
                    serde_json::to_string(&reference.content_hash)?,
                    registered_at.to_rfc3339(),
                ],
            )?;
        } else {
            transaction.execute(
                "INSERT INTO skill_manifest_heads(
                    skill_manifest_id, current_version, current_hash_json, updated_at
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    manifest.id.to_string(),
                    db_version(manifest.version)?,
                    serde_json::to_string(&reference.content_hash)?,
                    registered_at.to_rfc3339(),
                ],
            )?;
        }
        transaction.commit()?;
        Ok(reference)
    }

    /// Loads and revalidates one exact immutable Skill manifest version and Tool lineage.
    ///
    /// # Errors
    /// Returns a storage error for malformed, hash-inconsistent, or missing Tool evidence.
    pub fn skill_manifest(
        &self,
        id: SkillManifestId,
        version: u64,
    ) -> Result<Option<SkillManifest>, StorageError> {
        let row = self
            .connection
            .query_row(
                "SELECT manifest_hash_json, manifest_json
                 FROM skill_manifest_versions
                 WHERE skill_manifest_id = ?1 AND version = ?2",
                params![id.to_string(), db_version(version)?],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        let Some((hash, manifest)) = row else {
            return Ok(None);
        };
        let manifest = decode_skill_manifest(id, version, &hash, &manifest)?;
        for required in &manifest.required_tools {
            let tool = self
                .tool_manifest(required.id, required.version)?
                .ok_or_else(|| StorageError::InvalidData("required Tool disappeared".into()))?;
            if tool.versioned_ref().map_err(contract_error)? != *required {
                return Err(StorageError::InvalidData(
                    "Skill required Tool hash differs from durable evidence".into(),
                ));
            }
        }
        Ok(Some(manifest))
    }

    /// Loads the current immutable Skill manifest head and revalidates its exact hash and Tool
    /// lineage.
    ///
    /// # Errors
    /// Returns a storage error for missing or inconsistent head evidence.
    pub fn current_skill_manifest(
        &self,
        id: SkillManifestId,
    ) -> Result<Option<SkillManifest>, StorageError> {
        let head = self
            .connection
            .query_row(
                "SELECT current_version, current_hash_json
                 FROM skill_manifest_heads WHERE skill_manifest_id = ?1",
                [id.to_string()],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        let Some((version, expected_hash)) = head else {
            return Ok(None);
        };
        let version = db_version_from_i64(version)?;
        let manifest = self.skill_manifest(id, version)?.ok_or_else(|| {
            StorageError::InvalidData("Skill manifest head has no immutable version".into())
        })?;
        if serde_json::from_str::<ContentHash>(&expected_hash)?
            != skill_manifest_hash(&manifest).map_err(contract_error)?
        {
            return Err(StorageError::InvalidData(
                "Skill manifest head hash differs from its version".into(),
            ));
        }
        Ok(Some(manifest))
    }

    /// Rebuilds one non-authoritative Capability Profile projection.
    ///
    /// # Errors
    /// Returns a storage error for malformed or unhashable profile evidence.
    pub fn rebuild_capability_profile(
        &mut self,
        profile: &CapabilityProfile,
        rebuilt_at: DateTime<Utc>,
    ) -> Result<ContentHash, StorageError> {
        profile.validate().map_err(contract_error)?;
        let hash = capability_profile_hash(profile).map_err(contract_error)?;
        self.connection.execute(
            "INSERT INTO capability_profiles(
                source_type, source_locator, source_hash_json,
                profile_hash_json, profile_json, rebuilt_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(source_type, source_locator) DO UPDATE SET
                source_hash_json = excluded.source_hash_json,
                profile_hash_json = excluded.profile_hash_json,
                profile_json = excluded.profile_json,
                rebuilt_at = excluded.rebuilt_at",
            params![
                profile.capability_ref.source_type.as_str(),
                profile.capability_ref.locator,
                serde_json::to_string(&profile.capability_ref.content_hash)?,
                serde_json::to_string(&hash)?,
                serde_json::to_string(profile)?,
                rebuilt_at.to_rfc3339(),
            ],
        )?;
        Ok(hash)
    }

    /// Lists and revalidates the bounded rebuildable Capability Profile projection.
    ///
    /// # Errors
    /// Returns a storage error for malformed or hash-inconsistent projection rows.
    pub fn capability_profiles(&self) -> Result<Vec<CapabilityProfile>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT source_type, source_locator, source_hash_json, profile_hash_json, profile_json
             FROM capability_profiles ORDER BY source_type, source_locator",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        rows.map(|row| {
            let (source_type, source_locator, source_hash, profile_hash, profile) = row?;
            let profile: CapabilityProfile = serde_json::from_str(&profile)?;
            profile.validate().map_err(contract_error)?;
            if profile.capability_ref.source_type.as_str() != source_type
                || profile.capability_ref.locator != source_locator
                || profile.capability_ref.content_hash
                    != serde_json::from_str::<ContentHash>(&source_hash)?
                || capability_profile_hash(&profile).map_err(contract_error)?
                    != serde_json::from_str::<ContentHash>(&profile_hash)?
            {
                return Err(StorageError::InvalidData(
                    "Capability Profile projection differs from its indexed evidence".into(),
                ));
            }
            Ok(profile)
        })
        .collect()
    }

    /// Atomically persists one exact Root-issued Tool proposal, permit, and opaque handle map.
    /// Exact replay returns the original receipt; any substitution fails closed.
    ///
    /// # Errors
    /// Returns a storage error for malformed, stale, unregistered, substituted, or inactive data.
    pub fn authorize_tool_capability(
        &mut self,
        proposal: &ToolInvocationProposal,
        policy: &ToolPolicy,
        permit: &ToolInvocationPermit,
        authorized_at: DateTime<Utc>,
    ) -> Result<ToolCapabilityAuthorizationReceipt, StorageError> {
        proposal.validate().map_err(contract_error)?;
        policy.validate().map_err(contract_error)?;
        permit.validate().map_err(contract_error)?;
        let proposal_hash = tool_invocation_proposal_hash(proposal).map_err(contract_error)?;
        let permit_hash = tool_invocation_permit_hash(permit).map_err(contract_error)?;
        let policy_binding = tool_policy_binding(policy)?;
        let bindings = tool_secret_bindings(proposal, permit)?;
        validate_tool_proposal_permit(
            proposal,
            policy,
            permit,
            &proposal_hash,
            &policy_binding,
            authorized_at,
        )?;

        if let Some(existing) = self.tool_capability_authorization(proposal.action_id)? {
            if existing.proposal != *proposal
                || existing.proposal_hash != proposal_hash
                || existing.permit != *permit
                || existing.permit_hash != permit_hash
                || existing.secret_handles != bindings
            {
                return Err(StorageError::Conflict(
                    "Tool capability authorization was replayed with substituted authority".into(),
                ));
            }
            return Ok(existing);
        }

        validate_tool_immutable_evidence(self, proposal, permit)?;
        validate_tool_active_authority(self, proposal, authorized_at)?;
        let receipt = ToolCapabilityAuthorizationReceipt {
            proposal: proposal.clone(),
            proposal_hash,
            permit: permit.clone(),
            permit_hash: permit_hash.clone(),
            permit_ref: tool_permit_source_ref(permit, &permit_hash)?,
            secret_handles: bindings,
        };
        persist_tool_authorization(&mut self.connection, &receipt, authorized_at)?;
        Ok(receipt)
    }

    /// Persists one observational Tool permit for an exact running reconciliation probe.
    ///
    /// Unlike ordinary capability authority, this narrow path is valid only while the owning
    /// workflow is fenced in `Waiting(Reconciliation)`. It binds an already-authorized
    /// observational probe to the exact running job and cannot authorize the original effect.
    ///
    /// # Errors
    /// Returns a storage error for malformed, substituted, expired, non-observational, or stale
    /// reconciliation evidence.
    pub fn authorize_tool_reconciliation_capability(
        &mut self,
        proposal: &ToolInvocationProposal,
        policy: &ToolPolicy,
        permit: &ToolInvocationPermit,
        job_id: ReconciliationJobId,
        authorized_at: DateTime<Utc>,
    ) -> Result<ToolCapabilityAuthorizationReceipt, StorageError> {
        proposal.validate().map_err(contract_error)?;
        policy.validate().map_err(contract_error)?;
        permit.validate().map_err(contract_error)?;
        let proposal_hash = tool_invocation_proposal_hash(proposal).map_err(contract_error)?;
        let permit_hash = tool_invocation_permit_hash(permit).map_err(contract_error)?;
        let policy_binding = tool_policy_binding(policy)?;
        let bindings = tool_secret_bindings(proposal, permit)?;
        validate_tool_proposal_permit(
            proposal,
            policy,
            permit,
            &proposal_hash,
            &policy_binding,
            authorized_at,
        )?;

        if let Some(existing) = self.tool_capability_authorization(proposal.action_id)? {
            if existing.proposal != *proposal
                || existing.proposal_hash != proposal_hash
                || existing.permit != *permit
                || existing.permit_hash != permit_hash
                || existing.secret_handles != bindings
            {
                return Err(StorageError::Conflict(
                    "reconciliation Tool authority was replayed with substituted evidence".into(),
                ));
            }
            validate_tool_reconciliation_authority(self, proposal, job_id, authorized_at)?;
            if authorized_at < existing.permit.issued_at
                || authorized_at >= existing.permit.expires_at
            {
                return Err(StorageError::Conflict(
                    "reconciliation Tool authority replay is outside its active time window".into(),
                ));
            }
            return Ok(existing);
        }

        validate_tool_immutable_evidence(self, proposal, permit)?;
        validate_tool_reconciliation_authority(self, proposal, job_id, authorized_at)?;
        let receipt = ToolCapabilityAuthorizationReceipt {
            proposal: proposal.clone(),
            proposal_hash,
            permit: permit.clone(),
            permit_hash: permit_hash.clone(),
            permit_ref: tool_permit_source_ref(permit, &permit_hash)?,
            secret_handles: bindings,
        };
        persist_tool_authorization(&mut self.connection, &receipt, authorized_at)?;
        Ok(receipt)
    }

    /// Loads and canonically revalidates one complete Tool authorization by action identity.
    ///
    /// # Errors
    /// Returns a storage error for malformed, missing, or inconsistent durable evidence.
    pub fn tool_capability_authorization(
        &self,
        action_id: mcg_contracts::ActionId,
    ) -> Result<Option<ToolCapabilityAuthorizationReceipt>, StorageError> {
        let row = self
            .connection
            .query_row(
                "SELECT p.workflow_attempt_id, p.tool_manifest_id, p.tool_manifest_version,
                        p.argument_hash_json, p.proposed_at,
                        p.proposal_json, p.proposal_hash_json,
                        m.permit_id, m.proposal_hash_json, m.issued_at, m.expires_at,
                        m.permit_json, m.permit_hash_json
                 FROM tool_invocation_proposals p
                 JOIN tool_invocation_permits m ON m.action_id = p.action_id
                 WHERE p.action_id = ?1",
                [action_id.to_string()],
                stored_tool_authorization_row,
            )
            .optional()?;
        let Some(row) = row else {
            return Ok(None);
        };
        let proposal: ToolInvocationProposal = serde_json::from_str(&row.proposal_json)?;
        let proposal_hash: ContentHash = serde_json::from_str(&row.proposal_hash_json)?;
        let permit: ToolInvocationPermit = serde_json::from_str(&row.permit_json)?;
        let permit_hash: ContentHash = serde_json::from_str(&row.permit_hash_json)?;
        let policy = decode_tool_policy(&permit.policy)?;
        let bindings = load_tool_secret_handles(&self.connection, permit.id)?;
        if action_id != proposal.action_id
            || proposal.attempt_id.to_string() != row.workflow_attempt_id
            || proposal.tool_manifest.id.to_string() != row.tool_manifest_id
            || proposal.tool_manifest.version != db_version_from_i64(row.tool_manifest_version)?
            || proposal.argument_hash
                != serde_json::from_str::<ContentHash>(&row.argument_hash_json)?
            || proposal.deadline != permit.expires_at
            || permit.issued_at != parse_time(&row.proposed_at)?
            || permit.id.to_string() != row.permit_id
            || proposal_hash != serde_json::from_str::<ContentHash>(&row.permit_proposal_hash_json)?
            || permit.issued_at != parse_time(&row.issued_at)?
            || permit.expires_at != parse_time(&row.expires_at)?
            || proposal_hash != tool_invocation_proposal_hash(&proposal).map_err(contract_error)?
            || permit_hash != tool_invocation_permit_hash(&permit).map_err(contract_error)?
            || tool_policy_binding(&policy)? != permit.policy
            || bindings != tool_secret_bindings(&proposal, &permit)?
        {
            return Err(StorageError::InvalidData(
                "stored Tool capability authorization failed canonical replay".into(),
            ));
        }
        validate_tool_proposal_permit(
            &proposal,
            &policy,
            &permit,
            &proposal_hash,
            &permit.policy,
            permit.issued_at,
        )?;
        validate_tool_immutable_evidence(self, &proposal, &permit)?;
        Ok(Some(ToolCapabilityAuthorizationReceipt {
            proposal,
            proposal_hash,
            permit: permit.clone(),
            permit_hash: permit_hash.clone(),
            permit_ref: tool_permit_source_ref(&permit, &permit_hash)?,
            secret_handles: bindings,
        }))
    }

    /// Loads a Tool authorization only while its workflow lease and permit remain active.
    ///
    /// # Errors
    /// Returns a storage error for stale, expired, reassigned, or malformed authority.
    pub fn active_tool_capability_authorization(
        &self,
        action_id: mcg_contracts::ActionId,
        now: DateTime<Utc>,
    ) -> Result<Option<ToolCapabilityAuthorizationReceipt>, StorageError> {
        let Some(receipt) = self.tool_capability_authorization(action_id)? else {
            return Ok(None);
        };
        validate_tool_active_authority(self, &receipt.proposal, now)?;
        if now < receipt.permit.issued_at || now >= receipt.permit.expires_at {
            return Err(StorageError::Conflict(
                "Tool capability authorization is outside its active time window".into(),
            ));
        }
        Ok(Some(receipt))
    }

    /// Lists a bounded newest-first, content-minimized projection of persisted Tool authority.
    /// Every row is reloaded through the canonical authorization path before projection.
    ///
    /// # Errors
    /// Returns a storage error for invalid bounds or malformed authoritative evidence.
    pub fn tool_authorization_summaries(
        &self,
        limit: usize,
        now: DateTime<Utc>,
    ) -> Result<Vec<ToolAuthorizationSummary>, StorageError> {
        if limit == 0 || limit > MAX_TOOL_CAPABILITY_SUMMARIES {
            return Err(StorageError::InvalidData(
                "Tool authorization summary limit is outside the supported range".into(),
            ));
        }
        let sql_limit = i64::try_from(limit).map_err(|_| {
            StorageError::InvalidData("Tool authorization summary limit overflowed".into())
        })?;
        let action_ids = {
            let mut statement = self.connection.prepare(
                "SELECT action_id FROM tool_invocation_permits
                 ORDER BY issued_at DESC, permit_id DESC LIMIT ?1",
            )?;
            let rows = statement.query_map([sql_limit], |row| row.get::<_, String>(0))?;
            rows.map(|row| {
                let value = row?;
                Ok(mcg_contracts::ActionId(parse_uuid(
                    &value,
                    "Tool authorization action id",
                )?))
            })
            .collect::<Result<Vec<_>, StorageError>>()?
        };
        action_ids
            .into_iter()
            .map(|action_id| {
                self.tool_authorization_inspection(action_id, now)?
                    .map(|inspection| inspection.summary)
                    .ok_or_else(|| {
                        StorageError::InvalidData(
                            "indexed Tool authorization is missing its canonical evidence".into(),
                        )
                    })
            })
            .collect()
    }

    /// Loads one bounded Developer-safe Tool authorization projection by action identity.
    /// Raw arguments, secret values, and opaque handle identifiers are intentionally excluded.
    ///
    /// # Errors
    /// Returns a storage error for malformed, substituted, or unreadable authoritative evidence.
    pub fn tool_authorization_inspection(
        &self,
        action_id: mcg_contracts::ActionId,
        now: DateTime<Utc>,
    ) -> Result<Option<ToolAuthorizationInspection>, StorageError> {
        let Some(receipt) = self.tool_capability_authorization(action_id)? else {
            return Ok(None);
        };
        let manifest = self
            .tool_manifest(
                receipt.proposal.tool_manifest.id,
                receipt.proposal.tool_manifest.version,
            )?
            .ok_or_else(|| {
                StorageError::InvalidData("Tool authorization manifest version is missing".into())
            })?;
        let state = if now < receipt.permit.issued_at {
            ToolAuthorizationState::NotYetActive
        } else if now >= receipt.permit.expires_at {
            ToolAuthorizationState::Expired
        } else {
            let attempt = self.workflow_attempt(receipt.proposal.attempt_id)?.ok_or(
                StorageError::WorkflowAttemptNotFound(receipt.proposal.attempt_id),
            )?;
            if attempt.revision == receipt.proposal.attempt_revision
                && attempt.lease_generation == receipt.proposal.lease_generation
                && attempt.lease_owner == Some(receipt.proposal.runtime_actor)
                && attempt.task_contract == receipt.proposal.task_contract
                && attempt.state == WorkflowState::Running
                && attempt.lease_expires_at.is_some_and(|expiry| expiry > now)
            {
                ToolAuthorizationState::Active
            } else {
                ToolAuthorizationState::WorkflowInactive
            }
        };
        let capabilities = &receipt.permit.capabilities;
        let summary = ToolAuthorizationSummary {
            action_id,
            permit_id: receipt.permit.id,
            attempt_id: receipt.proposal.attempt_id,
            tool_manifest: receipt.proposal.tool_manifest.clone(),
            tool_name: manifest.name.clone(),
            effect_class: receipt.permit.allowed_effect_class,
            filesystem_scope_count: u32::try_from(capabilities.filesystem.len()).map_err(|_| {
                StorageError::InvalidData("filesystem capability count overflowed".into())
            })?,
            network_scope_count: u32::try_from(capabilities.network.len()).map_err(|_| {
                StorageError::InvalidData("network capability count overflowed".into())
            })?,
            secret_slot_count: u32::try_from(capabilities.secret_slots.len()).map_err(|_| {
                StorageError::InvalidData("secret-slot capability count overflowed".into())
            })?,
            process_spawn: capabilities.process_spawn,
            state,
            issued_at: receipt.permit.issued_at,
            expires_at: receipt.permit.expires_at,
        };
        let inspection = ToolAuthorizationInspection {
            summary,
            runner: manifest.runner,
            input_schema: manifest.input_schema,
            output_schema: manifest.output_schema,
            receipt_semantics: manifest.receipt_semantics,
            reconciler: manifest.reconciler,
            platform_support: manifest.platform_support,
            runtime_identity: receipt.permit.runtime_identity,
            runtime_actor: receipt.permit.runtime_actor,
            task_contract: receipt.permit.task_contract,
            attempt_revision: receipt.permit.attempt_revision,
            lease_generation: receipt.permit.lease_generation,
            argument_hash: receipt.permit.argument_hash,
            proposal_hash: receipt.proposal_hash,
            permit_hash: receipt.permit_hash,
            policy: receipt.permit.policy,
            capability_scope_hash: tool_capability_set_hash(capabilities)
                .map_err(contract_error)?,
            scopes: ToolCapabilityScopePreview::from_capabilities(capabilities),
            limits: receipt.permit.limits,
            sandbox: receipt.permit.sandbox,
            platform: receipt.permit.platform,
        };
        inspection.validate().map_err(contract_error)?;
        Ok(Some(inspection))
    }

    /// Resolves one opaque Tool secret handle only for its exact active permit.
    ///
    /// # Errors
    /// Returns a storage error for a missing, wrong-pair, expired, or stale handle.
    pub fn resolve_tool_secret_handle_binding(
        &self,
        permit_id: PermitId,
        handle_id: SecretHandleId,
        now: DateTime<Utc>,
    ) -> Result<ToolSecretHandleBinding, StorageError> {
        let row = self
            .connection
            .query_row(
                "SELECT h.slot_type, h.expires_at, p.action_id
                 FROM tool_secret_handle_bindings h
                 JOIN tool_invocation_permits p ON p.permit_id = h.permit_id
                 WHERE h.secret_handle_id = ?1 AND h.permit_id = ?2",
                params![handle_id.to_string(), permit_id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| {
                StorageError::InvalidData(
                    "Tool secret handle does not belong to the requested permit".into(),
                )
            })?;
        let action_id = mcg_contracts::ActionId(parse_uuid(&row.2, "Tool action id")?);
        let receipt = self
            .active_tool_capability_authorization(action_id, now)?
            .ok_or_else(|| StorageError::InvalidData("Tool permit is missing".into()))?;
        let binding = ToolSecretHandleBinding {
            handle_id,
            permit_id,
            slot: StableTypeId::parse(row.0).map_err(contract_error)?,
            expires_at: parse_time(&row.1)?,
        };
        if receipt.permit.id != permit_id || !receipt.secret_handles.contains(&binding) {
            return Err(StorageError::Conflict(
                "Tool secret handle is inactive or does not match its exact permit".into(),
            ));
        }
        Ok(binding)
    }
}

struct StoredToolAuthorizationRow {
    workflow_attempt_id: String,
    tool_manifest_id: String,
    tool_manifest_version: i64,
    argument_hash_json: String,
    proposed_at: String,
    proposal_json: String,
    proposal_hash_json: String,
    permit_id: String,
    permit_proposal_hash_json: String,
    issued_at: String,
    expires_at: String,
    permit_json: String,
    permit_hash_json: String,
}

fn stored_tool_authorization_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<StoredToolAuthorizationRow> {
    Ok(StoredToolAuthorizationRow {
        workflow_attempt_id: row.get(0)?,
        tool_manifest_id: row.get(1)?,
        tool_manifest_version: row.get(2)?,
        argument_hash_json: row.get(3)?,
        proposed_at: row.get(4)?,
        proposal_json: row.get(5)?,
        proposal_hash_json: row.get(6)?,
        permit_id: row.get(7)?,
        permit_proposal_hash_json: row.get(8)?,
        issued_at: row.get(9)?,
        expires_at: row.get(10)?,
        permit_json: row.get(11)?,
        permit_hash_json: row.get(12)?,
    })
}

/// Encodes and hashes one Root Tool policy as an exact immutable binding.
///
/// # Errors
/// Returns a storage error when validation, encoding, or hashing fails.
pub fn tool_policy_binding(policy: &ToolPolicy) -> Result<PolicyBinding, StorageError> {
    policy.validate().map_err(contract_error)?;
    let payload = VersionedPayload::canonical_v1(
        StableTypeId::parse(ToolPolicy::TYPE_ID).map_err(contract_error)?,
        &serde_json::to_value(policy)?,
    )
    .map_err(contract_error)?;
    let content_hash = payload
        .content_hash(TOOL_POLICY_HASH_DOMAIN)
        .map_err(contract_error)?;
    Ok(PolicyBinding {
        payload,
        content_hash,
    })
}

fn decode_tool_manifest(
    id: ToolManifestId,
    version: u64,
    hash: &str,
    manifest: &str,
) -> Result<ToolManifest, StorageError> {
    let manifest: ToolManifest = serde_json::from_str(manifest)?;
    let expected: ContentHash = serde_json::from_str(hash)?;
    if manifest.id != id
        || manifest.version != version
        || tool_manifest_hash(&manifest).map_err(contract_error)? != expected
    {
        return Err(StorageError::InvalidData(
            "Tool manifest row differs from its immutable identity or hash".into(),
        ));
    }
    Ok(manifest)
}

fn decode_skill_manifest(
    id: SkillManifestId,
    version: u64,
    hash: &str,
    manifest: &str,
) -> Result<SkillManifest, StorageError> {
    let manifest: SkillManifest = serde_json::from_str(manifest)?;
    let expected: ContentHash = serde_json::from_str(hash)?;
    if manifest.id != id
        || manifest.version != version
        || skill_manifest_hash(&manifest).map_err(contract_error)? != expected
    {
        return Err(StorageError::InvalidData(
            "Skill manifest row differs from its immutable identity or hash".into(),
        ));
    }
    Ok(manifest)
}

fn validate_tool_proposal_permit(
    proposal: &ToolInvocationProposal,
    policy: &ToolPolicy,
    permit: &ToolInvocationPermit,
    proposal_hash: &ContentHash,
    policy_binding: &PolicyBinding,
    authorized_at: DateTime<Utc>,
) -> Result<(), StorageError> {
    if permit.action_id != proposal.action_id
        || permit.attempt_id != proposal.attempt_id
        || permit.attempt_revision != proposal.attempt_revision
        || permit.lease_generation != proposal.lease_generation
        || permit.runtime_actor != proposal.runtime_actor
        || permit.runtime_identity != proposal.runtime_identity
        || permit.task_contract != proposal.task_contract
        || permit.tool_manifest != proposal.tool_manifest
        || permit.proposal_hash != *proposal_hash
        || permit.argument_hash != proposal.argument_hash
        || permit.allowed_effect_class != proposal.requested_effect_class
        || permit.capabilities != proposal.requested_capabilities
        || permit.limits != proposal.requested_limits
        || permit.platform != proposal.platform
        || permit.policy != *policy_binding
        || permit.issued_at != authorized_at
        || permit.expires_at != proposal.deadline
        || !permit.sandbox.satisfies(proposal.required_sandbox)
        || !policy.allowed_manifests.contains(&proposal.tool_manifest)
        || !policy.allows_effect(proposal.requested_effect_class)
        || !proposal
            .requested_capabilities
            .is_subset_of(&policy.maximum_capabilities)
        || !proposal.requested_limits.fits_within(policy.maximum_limits)
    {
        return Err(StorageError::Conflict(
            "Tool permit does not bind the exact proposal and Root policy".into(),
        ));
    }
    Ok(())
}

fn validate_tool_immutable_evidence(
    store: &SqliteEventStore,
    proposal: &ToolInvocationProposal,
    permit: &ToolInvocationPermit,
) -> Result<(), StorageError> {
    let manifest = store
        .tool_manifest(proposal.tool_manifest.id, proposal.tool_manifest.version)?
        .ok_or_else(|| StorageError::InvalidData("Tool manifest is not registered".into()))?;
    if manifest.versioned_ref().map_err(contract_error)? != proposal.tool_manifest
        || !manifest.platform_support.contains(&proposal.platform)
        || !manifest.allows_effect(proposal.requested_effect_class)
        || manifest.required_capabilities != proposal.requested_capabilities
        || !proposal
            .requested_limits
            .fits_within(manifest.resource_ceiling)
        || proposal.required_sandbox != manifest.sandbox_requirement
        || !permit.sandbox.satisfies(manifest.sandbox_requirement)
    {
        return Err(StorageError::Conflict(
            "Tool authorization differs from its registered manifest".into(),
        ));
    }

    let action = store
        .action_envelope(proposal.action_id)?
        .ok_or_else(|| StorageError::InvalidData("Tool action envelope is missing".into()))?;
    if action.attempt_id != proposal.attempt_id
        || action.task_contract != proposal.task_contract
        || action.actor != proposal.runtime_actor
        || action.operation_hash != proposal.argument_hash
        || action.effect_class != proposal.requested_effect_class
    {
        return Err(StorageError::Conflict(
            "Tool proposal does not bind the exact authoritative action".into(),
        ));
    }
    Ok(())
}

fn validate_tool_active_authority(
    store: &SqliteEventStore,
    proposal: &ToolInvocationProposal,
    now: DateTime<Utc>,
) -> Result<(), StorageError> {
    let attempt = store
        .workflow_attempt(proposal.attempt_id)?
        .ok_or(StorageError::WorkflowAttemptNotFound(proposal.attempt_id))?;
    let ordinary_active = attempt.revision == proposal.attempt_revision
        && attempt.lease_generation == proposal.lease_generation
        && attempt.lease_owner == Some(proposal.runtime_actor)
        && attempt.task_contract == proposal.task_contract
        && attempt.state == WorkflowState::Running
        && attempt.lease_expires_at.is_some_and(|expiry| expiry > now);
    if ordinary_active {
        return Ok(());
    }
    let action = store.action_envelope(proposal.action_id)?.ok_or_else(|| {
        StorageError::InvalidData("Tool action envelope is missing during active validation".into())
    })?;
    let reconciliation_job_id = action
        .preconditions
        .as_slice()
        .first()
        .filter(|_| action.preconditions.len() == 1)
        .filter(|binding| binding.type_id.as_str() == RECONCILIATION_PROBE_BINDING_TYPE)
        .and_then(|binding| decode_canonical_cbor_v1(&binding.bytes).ok())
        .and_then(|value| value.get("job_id").cloned())
        .and_then(|value| serde_json::from_value::<ReconciliationJobId>(value).ok());
    if let Some(job_id) = reconciliation_job_id {
        return validate_tool_reconciliation_authority(store, proposal, job_id, now);
    }
    Err(StorageError::Conflict(
        "Tool authorization does not belong to active workflow or reconciliation authority".into(),
    ))
}

fn validate_tool_reconciliation_authority(
    store: &SqliteEventStore,
    proposal: &ToolInvocationProposal,
    job_id: ReconciliationJobId,
    now: DateTime<Utc>,
) -> Result<(), StorageError> {
    let attempt = store
        .workflow_attempt(proposal.attempt_id)?
        .ok_or(StorageError::WorkflowAttemptNotFound(proposal.attempt_id))?;
    let job = store
        .reconciliation_job(job_id)?
        .ok_or_else(|| StorageError::Conflict("reconciliation Tool job was not found".into()))?;
    let target = store.action_execution(job.execution_id)?.ok_or_else(|| {
        StorageError::InvalidData("reconciliation Tool target execution disappeared".into())
    })?;
    let target_envelope = store.action_envelope(job.action_id)?.ok_or_else(|| {
        StorageError::InvalidData("reconciliation Tool target intent disappeared".into())
    })?;
    let probe = store.action_envelope(proposal.action_id)?.ok_or_else(|| {
        StorageError::InvalidData("reconciliation Tool probe intent disappeared".into())
    })?;
    let probe_execution = store
        .action_execution_for_action(proposal.action_id)?
        .ok_or_else(|| {
            StorageError::InvalidData("reconciliation Tool probe execution disappeared".into())
        })?;
    if attempt.revision != proposal.attempt_revision
        || attempt.lease_generation != proposal.lease_generation
        || attempt.task_contract != proposal.task_contract
        || attempt.state != WorkflowState::Waiting(WaitReason::Reconciliation)
        || attempt.lease_owner.is_some()
        || attempt.lease_expires_at.is_some()
        || job.state != ReconciliationJobState::Running
        || job.lease_owner != Some(proposal.runtime_actor)
        || job.lease_expires_at.is_none_or(|expiry| expiry <= now)
        || proposal.deadline > job.deadline
        || proposal.deadline
            > job.lease_expires_at.ok_or_else(|| {
                StorageError::Conflict("reconciliation Tool job has no lease".into())
            })?
        || target.state != ActionExecutionState::Reconciling
        || target.executor != Some(proposal.runtime_actor)
        || target.action_id != target_envelope.id
        || target_envelope.workflow_id != attempt.workflow_id
        || target_envelope.attempt_id != attempt.attempt_id
        || probe.workflow_id != attempt.workflow_id
        || probe.attempt_id != attempt.attempt_id
        || probe.actor != proposal.runtime_actor
        || probe.effect_class != EffectClass::Observational
        || probe.operation != job.adapter
        || probe.compensation.is_some()
        || probe.compensates_action.is_some()
        || probe.reconciler.is_some()
        || probe.reconciliation_deadline.is_some()
        || probe_execution.state != ActionExecutionState::Authorized
        || probe_execution.executor.is_some()
    {
        return Err(StorageError::Conflict(
            "Tool reconciliation authority does not bind the exact observational probe".into(),
        ));
    }
    Ok(())
}

fn tool_secret_bindings(
    proposal: &ToolInvocationProposal,
    permit: &ToolInvocationPermit,
) -> Result<Vec<ToolSecretHandleBinding>, StorageError> {
    if permit.secret_handles.len() != proposal.requested_capabilities.secret_slots.len() {
        return Err(StorageError::Conflict(
            "Tool permit secret-handle shape differs from requested slots".into(),
        ));
    }
    Ok(permit
        .secret_handles
        .iter()
        .copied()
        .zip(proposal.requested_capabilities.secret_slots.iter().cloned())
        .map(|(handle_id, slot)| ToolSecretHandleBinding {
            handle_id,
            permit_id: permit.id,
            slot,
            expires_at: permit.expires_at,
        })
        .collect())
}

fn persist_tool_authorization(
    connection: &mut rusqlite::Connection,
    receipt: &ToolCapabilityAuthorizationReceipt,
    authorized_at: DateTime<Utc>,
) -> Result<(), StorageError> {
    let transaction = connection.transaction()?;
    transaction.execute(
        "INSERT INTO tool_invocation_proposals(
            action_id, workflow_attempt_id, tool_manifest_id, tool_manifest_version,
            argument_hash_json, proposal_hash_json, proposal_json, proposed_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            receipt.proposal.action_id.to_string(),
            receipt.proposal.attempt_id.to_string(),
            receipt.proposal.tool_manifest.id.to_string(),
            db_version(receipt.proposal.tool_manifest.version)?,
            serde_json::to_string(&receipt.proposal.argument_hash)?,
            serde_json::to_string(&receipt.proposal_hash)?,
            serde_json::to_string(&receipt.proposal)?,
            authorized_at.to_rfc3339(),
        ],
    )?;
    transaction.execute(
        "INSERT INTO tool_invocation_permits(
            permit_id, action_id, proposal_hash_json, permit_hash_json,
            permit_json, issued_at, expires_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            receipt.permit.id.to_string(),
            receipt.permit.action_id.to_string(),
            serde_json::to_string(&receipt.proposal_hash)?,
            serde_json::to_string(&receipt.permit_hash)?,
            serde_json::to_string(&receipt.permit)?,
            receipt.permit.issued_at.to_rfc3339(),
            receipt.permit.expires_at.to_rfc3339(),
        ],
    )?;
    persist_tool_secret_handles(&transaction, &receipt.secret_handles)?;
    transaction.commit()?;
    Ok(())
}

fn persist_tool_secret_handles(
    transaction: &Transaction<'_>,
    bindings: &[ToolSecretHandleBinding],
) -> Result<(), StorageError> {
    for binding in bindings {
        transaction.execute(
            "INSERT INTO tool_secret_handle_bindings(
                secret_handle_id, permit_id, slot_type, expires_at
             ) VALUES (?1, ?2, ?3, ?4)",
            params![
                binding.handle_id.to_string(),
                binding.permit_id.to_string(),
                binding.slot.as_str(),
                binding.expires_at.to_rfc3339(),
            ],
        )?;
    }
    Ok(())
}

fn load_tool_secret_handles(
    connection: &rusqlite::Connection,
    permit_id: PermitId,
) -> Result<Vec<ToolSecretHandleBinding>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT secret_handle_id, slot_type, expires_at
         FROM tool_secret_handle_bindings WHERE permit_id = ?1 ORDER BY rowid",
    )?;
    let rows = statement.query_map([permit_id.to_string()], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    rows.map(|row| {
        let (handle, slot, expiry) = row?;
        Ok(ToolSecretHandleBinding {
            handle_id: SecretHandleId(parse_uuid(&handle, "Tool secret handle")?),
            permit_id,
            slot: StableTypeId::parse(slot).map_err(contract_error)?,
            expires_at: parse_time(&expiry)?,
        })
    })
    .collect()
}

fn decode_tool_policy(binding: &PolicyBinding) -> Result<ToolPolicy, StorageError> {
    binding.validate().map_err(contract_error)?;
    if binding.payload.type_id.as_str() != ToolPolicy::TYPE_ID {
        return Err(StorageError::InvalidData(
            "Tool permit policy type is unsupported".into(),
        ));
    }
    let value = decode_canonical_cbor_v1(&binding.payload.bytes).map_err(contract_error)?;
    let policy: ToolPolicy = serde_json::from_value(value)?;
    policy.validate().map_err(contract_error)?;
    Ok(policy)
}

fn tool_permit_source_ref(
    permit: &ToolInvocationPermit,
    permit_hash: &ContentHash,
) -> Result<SourceRef, StorageError> {
    let source = SourceRef {
        source_type: StableTypeId::parse(TOOL_PERMIT_SOURCE_TYPE).map_err(contract_error)?,
        locator: permit.id.to_string(),
        content_hash: permit_hash.clone(),
    };
    source.validate().map_err(contract_error)?;
    if source.content_hash.domain != TOOL_INVOCATION_PERMIT_HASH_DOMAIN {
        return Err(StorageError::InvalidData(
            "Tool permit source hash domain is invalid".into(),
        ));
    }
    Ok(source)
}

fn parse_uuid(value: &str, label: &str) -> Result<Uuid, StorageError> {
    Uuid::parse_str(value).map_err(|_| StorageError::InvalidData(format!("invalid {label}")))
}

fn db_version(version: u64) -> Result<i64, StorageError> {
    i64::try_from(version)
        .map_err(|_| StorageError::InvalidData("manifest version exceeds SQLite range".into()))
}

fn db_version_from_i64(version: i64) -> Result<u64, StorageError> {
    u64::try_from(version)
        .map_err(|_| StorageError::InvalidData("manifest version is negative".into()))
}

fn parse_time(value: &str) -> Result<DateTime<Utc>, StorageError> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|error| StorageError::InvalidData(error.to_string()))
}

#[allow(clippy::needless_pass_by_value)]
fn contract_error(error: mcg_contracts::ContractError) -> StorageError {
    StorageError::InvalidData(error.to_string())
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone};
    use mcg_contracts::{
        ActionEnvelope, ActionExecution, ActionExecutionId, ActionExecutionState, ActionId,
        ArtifactRef, AttemptId, CapabilityEstimate, CapabilityKind, ConfigurationId,
        ContractClause, CorrelationId, EffectClass, ExecutionPlatform, FilesystemAccess,
        FilesystemCapability, HashAlgorithm, IsolationGuarantee, NetworkCapability, PROTOCOL_V1,
        RetryPolicy, RuntimeActorId, RuntimeIdentity, RuntimeInstanceId, SandboxCapabilityProfile,
        SkillStatus, TaskContract, TaskContractId, TaskContractStatus, ToolCapabilitySet,
        ToolResourceLimits, VersionedRef, WorkflowAttempt, WorkflowId,
    };

    use super::*;
    use crate::task_contract_content_hash;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, 1, 12, 0, 0)
            .single()
            .expect("fixture time")
    }

    fn payload(type_id: &str, label: &str) -> VersionedPayload {
        VersionedPayload::canonical_v1(
            StableTypeId::parse(type_id).expect("fixture type"),
            &serde_json::json!({"label": label}),
        )
        .expect("fixture payload")
    }

    fn hash(label: &str) -> ContentHash {
        payload("test.tool_capability_ref.v1", label)
            .content_hash("TestToolCapabilityRef")
            .expect("fixture hash")
    }

    fn source(label: &str) -> SourceRef {
        SourceRef {
            source_type: StableTypeId::parse("test.tool_capability_source.v1")
                .expect("source type"),
            locator: format!("fixture://{label}"),
            content_hash: hash(label),
        }
    }

    fn policy(label: &str) -> PolicyBinding {
        let payload = payload("test.tool_capability_policy.v1", label);
        PolicyBinding {
            content_hash: payload
                .content_hash("TestToolCapabilityPolicy")
                .expect("policy hash"),
            payload,
        }
    }

    const fn limits() -> ToolResourceLimits {
        ToolResourceLimits {
            timeout_ms: 30_000,
            max_output_bytes: 1_048_576,
            max_memory_bytes: 256 * 1_048_576,
            max_child_processes: 1,
        }
    }

    const fn sandbox(level: IsolationGuarantee) -> SandboxCapabilityProfile {
        SandboxCapabilityProfile {
            filesystem_scope: level,
            network: level,
            process_tree: level,
            secrets: level,
            environment: level,
            resources: level,
            protected_paths: level,
        }
    }

    fn capabilities() -> ToolCapabilitySet {
        ToolCapabilitySet {
            filesystem: vec![FilesystemCapability {
                root: "workspace".into(),
                access: FilesystemAccess::Read,
            }],
            network: vec![NetworkCapability {
                destination: "https://tool.example.test".into(),
            }],
            secret_slots: vec![
                StableTypeId::parse("mcg.secret.tool_fixture.v1").expect("secret slot"),
            ],
            process_spawn: false,
        }
    }

    fn tool_manifest(id: ToolManifestId, version: u64) -> ToolManifest {
        ToolManifest {
            id,
            version,
            name: "fixture.read".into(),
            summary: format!("Fixture governed reader version {version}"),
            runner: source(&format!("runner-{version}")),
            input_schema: source("input-schema"),
            output_schema: source("output-schema"),
            maximum_effect_class: EffectClass::Observational,
            required_capabilities: capabilities(),
            resource_ceiling: limits(),
            sandbox_requirement: sandbox(IsolationGuarantee::BestEffort),
            platform_support: vec![ExecutionPlatform::Windows, ExecutionPlatform::Linux],
            receipt_semantics: StableTypeId::parse("mcg.tool_receipt.fixture.v1")
                .expect("receipt type"),
            reconciler: None,
        }
    }

    fn skill_manifest(tool: &ToolManifest) -> SkillManifest {
        SkillManifest {
            id: SkillManifestId::new(),
            version: 1,
            name: "fixture.skill".into(),
            summary: "A validated fixture procedure".into(),
            input_schema: source("skill-input"),
            output_schema: source("skill-output"),
            preconditions: vec![source("skill-precondition")],
            required_capabilities: tool.required_capabilities.clone(),
            required_tools: vec![tool.versioned_ref().expect("Tool reference")],
            implementation: payload("test.skill_implementation.v1", "one-tool"),
            verification_procedure: payload("test.skill_verification.v1", "schema"),
            tested_conditions: vec![source("tested-condition")],
            known_exclusions: vec![source("known-exclusion")],
            fallback: Some(source("fallback")),
            rollback_policy: payload("test.skill_rollback.v1", "reason"),
            provenance: vec![source("evaluation")],
            status: SkillStatus::Active,
        }
    }

    fn capability_profile(skill: &SkillManifest, observations: u64) -> CapabilityProfile {
        CapabilityProfile {
            capability_ref: SourceRef {
                source_type: StableTypeId::parse("mcg.skill_manifest.v1")
                    .expect("capability source type"),
                locator: skill.id.to_string(),
                content_hash: skill.content_hash().expect("Skill hash"),
            },
            kind: CapabilityKind::Skill,
            summary: skill.summary.clone(),
            inputs: vec![skill.input_schema.clone()],
            outputs: vec![skill.output_schema.clone()],
            preconditions: skill.preconditions.clone(),
            required_authority: skill.required_capabilities.clone(),
            tested_environments: vec![source("windows-fixture")],
            incompatibilities: Vec::new(),
            competence_envelope: Some(source("competence-envelope")),
            historical: CapabilityEstimate {
                observations,
                success_basis_points: 9_000,
                cost_microusd: 20,
                latency_ms: 15,
            },
            version: observations.max(1),
        }
    }

    fn running_workflow(
        store: &mut SqliteEventStore,
    ) -> (TaskContract, WorkflowAttempt, RuntimeActorId) {
        let task_id = TaskContractId::new();
        let clause = ContractClause {
            clause_id: "criterion-1".into(),
            statement: "read one governed resource".into(),
            source_refs: vec![source("criterion")],
        };
        let contract = TaskContract {
            id: task_id,
            version: 1,
            previous_version: None,
            user_intent_ref: source("intent"),
            objective: "exercise durable Tool authority".into(),
            non_goals: Vec::new(),
            requirements: vec![clause.clone()],
            constraints: Vec::new(),
            assumptions: Vec::new(),
            acceptance_criteria: vec![clause],
            dependencies: Vec::new(),
            permitted_side_effects: policy("observational"),
            authority: policy("root"),
            environment: policy("isolated"),
            risk: policy("bounded"),
            budgets: policy("bounded"),
            rollback: policy("none"),
            status: TaskContractStatus::Active,
            created_at: now(),
        };
        let mut attempt = WorkflowAttempt {
            workflow_id: WorkflowId::new(),
            attempt_id: AttemptId::new(),
            task_contract: VersionedRef {
                id: task_id,
                version: 1,
                content_hash: ContentHash {
                    algorithm: HashAlgorithm::Blake3_256,
                    domain: "placeholder".into(),
                    digest_hex: "0".repeat(64),
                },
            },
            parent_attempt_id: None,
            attempt_number: 1,
            state: WorkflowState::Ready,
            revision: 1,
            lease_owner: None,
            lease_generation: 0,
            lease_expires_at: None,
            heartbeat_at: None,
            checkpoint: None,
            environment_snapshot: hash("environment"),
            configuration: VersionedRef {
                id: ConfigurationId::new(),
                version: 1,
                content_hash: hash("configuration"),
            },
            idempotency_key: "tool-capability-workflow".into(),
            retry_policy: RetryPolicy {
                max_attempts: 2,
                initial_backoff_ms: 100,
                maximum_backoff_ms: 1_000,
            },
            compensation_policy: policy("none"),
            created_at: now(),
            updated_at: now(),
        };
        attempt.task_contract.content_hash = task_contract_content_hash(&contract).unwrap();
        store
            .create_workflow(&contract, &attempt, CorrelationId::new(), now())
            .expect("create workflow");
        let actor = RuntimeActorId::new();
        let running = store
            .claim_workflow_attempt(
                attempt.attempt_id,
                attempt.revision,
                attempt.lease_generation,
                actor,
                now() + Duration::minutes(10),
                CorrelationId::new(),
                "tool-capability-claim",
                now(),
            )
            .expect("claim workflow");
        (contract, running, actor)
    }

    fn authorize_action(
        store: &mut SqliteEventStore,
        attempt: &WorkflowAttempt,
        actor: RuntimeActorId,
    ) -> ActionEnvelope {
        let operation = payload("test.tool_arguments.v1", "read-workspace");
        let envelope = ActionEnvelope {
            id: ActionId::new(),
            workflow_id: attempt.workflow_id,
            attempt_id: attempt.attempt_id,
            task_contract: attempt.task_contract.clone(),
            actor,
            origin: None,
            authority: policy("exact-read"),
            elevated_authority: None,
            expected_versions: Vec::new(),
            operation_hash: operation
                .content_hash("TestToolArguments")
                .expect("argument hash"),
            operation,
            effect_class: EffectClass::Observational,
            preconditions: Vec::new(),
            budget_reservation: policy("one-read"),
            idempotency_key: "tool-capability-action".into(),
            compensation: None,
            compensates_action: None,
            reconciler: None,
            reconciliation_deadline: None,
            created_at: now(),
        };
        let execution = ActionExecution {
            id: ActionExecutionId::new(),
            action_id: envelope.id,
            attempt_number: 1,
            state: ActionExecutionState::Authorized,
            revision: 1,
            lease_generation: 0,
            executor: None,
            outcome: None,
            created_at: now(),
            updated_at: now(),
        };
        store
            .authorize_action(
                &envelope,
                &execution,
                attempt.revision,
                attempt.lease_generation,
                CorrelationId::new(),
                now(),
            )
            .expect("authorize action");
        envelope
    }

    fn tool_authority(
        attempt: &WorkflowAttempt,
        actor: RuntimeActorId,
        action: &ActionEnvelope,
        manifest: &ToolManifest,
    ) -> (ToolInvocationProposal, ToolPolicy, ToolInvocationPermit) {
        let runtime_identity = RuntimeIdentity {
            instance_id: RuntimeInstanceId::new(),
            build_hash: hash("runtime-build"),
            protocol_version: PROTOCOL_V1,
            started_at: now() - Duration::seconds(1),
        };
        let proposal = ToolInvocationProposal {
            action_id: action.id,
            attempt_id: attempt.attempt_id,
            attempt_revision: attempt.revision,
            lease_generation: attempt.lease_generation,
            runtime_actor: actor,
            runtime_identity,
            task_contract: attempt.task_contract.clone(),
            tool_manifest: manifest.versioned_ref().expect("Tool reference"),
            arguments: ArtifactRef {
                content_hash: action.operation_hash.clone(),
                media_type: "application/cbor".into(),
                byte_length: u64::try_from(action.operation.bytes.len()).expect("argument size"),
            },
            argument_hash: action.operation_hash.clone(),
            requested_effect_class: action.effect_class,
            requested_capabilities: manifest.required_capabilities.clone(),
            requested_limits: limits(),
            required_sandbox: manifest.sandbox_requirement,
            platform: ExecutionPlatform::Windows,
            deadline: now() + Duration::minutes(2),
        };
        let policy = ToolPolicy {
            allowed_manifests: vec![manifest.versioned_ref().expect("Tool reference")],
            maximum_effect_class: EffectClass::Consequential,
            maximum_capabilities: manifest.required_capabilities.clone(),
            maximum_limits: manifest.resource_ceiling,
            maximum_permit_lifetime_seconds: 300,
        };
        let proposal_hash = tool_invocation_proposal_hash(&proposal).expect("proposal hash");
        let permit = ToolInvocationPermit {
            id: PermitId::new(),
            action_id: proposal.action_id,
            attempt_id: proposal.attempt_id,
            attempt_revision: proposal.attempt_revision,
            lease_generation: proposal.lease_generation,
            runtime_actor: proposal.runtime_actor,
            runtime_identity: proposal.runtime_identity.clone(),
            task_contract: proposal.task_contract.clone(),
            tool_manifest: proposal.tool_manifest.clone(),
            proposal_hash,
            argument_hash: proposal.argument_hash.clone(),
            allowed_effect_class: proposal.requested_effect_class,
            capabilities: proposal.requested_capabilities.clone(),
            limits: proposal.requested_limits,
            sandbox: sandbox(IsolationGuarantee::Hard),
            platform: proposal.platform,
            policy: tool_policy_binding(&policy).expect("policy binding"),
            expires_at: proposal.deadline,
            secret_handles: vec![SecretHandleId::new()],
            issued_at: now(),
        };
        (proposal, policy, permit)
    }

    #[test]
    fn immutable_manifests_and_rebuildable_profiles_survive_reopen() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let database = directory.path().join("capabilities.db");
        let tool_id = ToolManifestId::new();
        let tool_v1 = tool_manifest(tool_id, 1);
        let skill;
        let replacement_profile;
        {
            let mut store = SqliteEventStore::open(&database).expect("open store");
            let tool_v1_ref = store
                .register_tool_manifest(&tool_v1, now())
                .expect("register Tool v1");
            assert_eq!(
                store
                    .register_tool_manifest(&tool_v1, now())
                    .expect("exact replay"),
                tool_v1_ref
            );
            let mut substituted = tool_v1.clone();
            substituted.summary.push_str(" substituted");
            assert!(store.register_tool_manifest(&substituted, now()).is_err());
            assert!(
                store
                    .register_tool_manifest(&tool_manifest(tool_id, 3), now())
                    .is_err()
            );
            let tool_v2 = tool_manifest(tool_id, 2);
            store
                .register_tool_manifest(&tool_v2, now())
                .expect("register Tool v2");
            assert_eq!(store.current_tool_manifest(tool_id).unwrap(), Some(tool_v2));

            skill = skill_manifest(&tool_v1);
            store
                .register_skill_manifest(&skill, now())
                .expect("register Skill");
            assert_eq!(
                store.current_skill_manifest(skill.id).unwrap(),
                Some(skill.clone())
            );
            let mut substituted_tool = skill.clone();
            substituted_tool.required_tools[0].content_hash = hash("wrong Tool");
            assert!(
                store
                    .register_skill_manifest(&substituted_tool, now())
                    .is_err()
            );

            let initial_profile = capability_profile(&skill, 1);
            store
                .rebuild_capability_profile(&initial_profile, now())
                .expect("initial profile");
            replacement_profile = capability_profile(&skill, 2);
            store
                .rebuild_capability_profile(&replacement_profile, now() + Duration::seconds(1))
                .expect("replace projection");
            assert_eq!(
                store.capability_profiles().unwrap(),
                vec![replacement_profile.clone()]
            );
        }
        let mut reopened = SqliteEventStore::open(&database).expect("reopen store");
        assert_eq!(reopened.tool_manifest(tool_id, 1).unwrap(), Some(tool_v1));
        assert_eq!(
            reopened.current_skill_manifest(skill.id).unwrap(),
            Some(skill)
        );
        assert_eq!(
            reopened.capability_profiles().unwrap(),
            vec![replacement_profile]
        );
        reopened
            .connection
            .execute_batch(
                "DROP TRIGGER tool_manifest_heads_no_delete;
                 DELETE FROM tool_manifest_heads;",
            )
            .expect("simulate damaged Tool head");
        assert!(
            reopened
                .register_tool_manifest(&tool_manifest(tool_id, 1), now())
                .is_err()
        );
    }

    #[test]
    fn root_tool_authority_is_exact_active_opaque_and_historically_replayable() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let database = directory.path().join("tool-authority.db");
        let action_id;
        let permit_id;
        let handle_id;
        let expected_receipt;
        {
            let mut store = SqliteEventStore::open(&database).expect("open store");
            let (_, attempt, actor) = running_workflow(&mut store);
            let action = authorize_action(&mut store, &attempt, actor);
            let manifest = tool_manifest(ToolManifestId::new(), 1);
            store
                .register_tool_manifest(&manifest, now())
                .expect("register Tool");
            let (proposal, policy, permit) = tool_authority(&attempt, actor, &action, &manifest);
            expected_receipt = store
                .authorize_tool_capability(&proposal, &policy, &permit, now())
                .expect("persist Root authority");
            assert_eq!(
                store
                    .authorize_tool_capability(&proposal, &policy, &permit, now())
                    .expect("exact replay"),
                expected_receipt
            );
            assert_eq!(
                store
                    .active_tool_capability_authorization(action.id, now())
                    .expect("active authority"),
                Some(expected_receipt.clone())
            );
            let summaries = store
                .tool_authorization_summaries(10, now())
                .expect("bounded Tool summaries");
            assert_eq!(summaries.len(), 1);
            assert_eq!(summaries[0].action_id, action.id);
            assert_eq!(summaries[0].state, ToolAuthorizationState::Active);
            let inspection = store
                .tool_authorization_inspection(action.id, now())
                .expect("Tool inspection")
                .expect("persisted Tool inspection");
            assert_eq!(inspection.summary, summaries[0]);
            assert_eq!(inspection.proposal_hash, expected_receipt.proposal_hash);
            assert_eq!(inspection.permit_hash, expected_receipt.permit_hash);
            assert_eq!(inspection.scopes.secret_slots.len(), 1);
            let serialized = serde_json::to_string(&inspection).expect("serialize inspection");
            assert!(!serialized.contains(&permit.secret_handles[0].to_string()));
            assert!(store.tool_authorization_summaries(0, now()).is_err());
            assert!(
                store
                    .tool_authorization_summaries(MAX_TOOL_CAPABILITY_SUMMARIES + 1, now())
                    .is_err()
            );
            action_id = action.id;
            permit_id = permit.id;
            handle_id = permit.secret_handles[0];
            assert_eq!(
                store
                    .resolve_tool_secret_handle_binding(permit_id, handle_id, now())
                    .expect("exact handle"),
                expected_receipt.secret_handles[0]
            );
            assert!(
                store
                    .resolve_tool_secret_handle_binding(permit_id, SecretHandleId::new(), now())
                    .is_err()
            );

            let mut substituted = proposal.clone();
            substituted.runtime_identity.instance_id = RuntimeInstanceId::new();
            assert!(
                store
                    .authorize_tool_capability(&substituted, &policy, &permit, now())
                    .is_err()
            );
        }

        let reopened = SqliteEventStore::open(&database).expect("reopen store");
        assert_eq!(
            reopened
                .tool_capability_authorization(action_id)
                .expect("historical replay"),
            Some(expected_receipt.clone())
        );
        assert!(
            reopened
                .active_tool_capability_authorization(action_id, now() + Duration::minutes(3))
                .is_err()
        );
        assert!(
            reopened
                .resolve_tool_secret_handle_binding(
                    permit_id,
                    handle_id,
                    now() + Duration::minutes(3),
                )
                .is_err()
        );
        assert_eq!(
            reopened
                .tool_capability_authorization(action_id)
                .expect("expired historical replay"),
            Some(expected_receipt)
        );
        assert_eq!(
            reopened
                .tool_authorization_inspection(action_id, now() + Duration::minutes(3))
                .expect("expired Tool inspection")
                .expect("persisted expired Tool inspection")
                .summary
                .state,
            ToolAuthorizationState::Expired
        );
        reopened
            .connection
            .execute_batch(
                "DROP TRIGGER tool_invocation_proposals_no_update;
                 UPDATE tool_invocation_proposals SET proposed_at = '2026-09-01T12:00:01Z';",
            )
            .expect("simulate damaged denormalized permit evidence");
        assert!(reopened.tool_capability_authorization(action_id).is_err());
    }
}
