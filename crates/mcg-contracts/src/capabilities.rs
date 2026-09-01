use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    ActionId, ArtifactRef, ContentHash, ContractError, EffectClass, PermitId, PolicyBinding,
    RuntimeActorId, RuntimeIdentity, SecretHandleId, SkillManifestId, SourceRef, StableTypeId,
    TaskContractId, ToolManifestId, VersionedPayload, VersionedRef, canonical_contract_hash,
};

pub const TOOL_MANIFEST_HASH_DOMAIN: &str = "MetaCog.ToolManifest";
pub const SKILL_MANIFEST_HASH_DOMAIN: &str = "MetaCog.SkillManifest";
pub const CAPABILITY_PROFILE_HASH_DOMAIN: &str = "MetaCog.CapabilityProfile";
pub const TOOL_INVOCATION_PROPOSAL_HASH_DOMAIN: &str = "MetaCog.ToolInvocationProposal";
pub const TOOL_INVOCATION_PERMIT_HASH_DOMAIN: &str = "MetaCog.ToolInvocationPermit";
pub const TOOL_POLICY_HASH_DOMAIN: &str = "MetaCog.ToolPolicy";

const MAX_CAPABILITY_ITEMS: usize = 256;
const MAX_TEXT_BYTES: usize = 4_096;
const MAX_RESOURCE_LOCATOR_BYTES: usize = 4_096;
const MAX_TOOL_PERMIT_LIFETIME_SECONDS: u64 = 24 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IsolationGuarantee {
    Unavailable,
    BestEffort,
    Hard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxCapabilityProfile {
    pub filesystem_scope: IsolationGuarantee,
    pub network: IsolationGuarantee,
    pub process_tree: IsolationGuarantee,
    pub secrets: IsolationGuarantee,
    pub environment: IsolationGuarantee,
    pub resources: IsolationGuarantee,
    pub protected_paths: IsolationGuarantee,
}

impl SandboxCapabilityProfile {
    #[must_use]
    pub const fn unavailable() -> Self {
        Self {
            filesystem_scope: IsolationGuarantee::Unavailable,
            network: IsolationGuarantee::Unavailable,
            process_tree: IsolationGuarantee::Unavailable,
            secrets: IsolationGuarantee::Unavailable,
            environment: IsolationGuarantee::Unavailable,
            resources: IsolationGuarantee::Unavailable,
            protected_paths: IsolationGuarantee::Unavailable,
        }
    }

    #[must_use]
    pub const fn satisfies(self, required: Self) -> bool {
        self.filesystem_scope as u8 >= required.filesystem_scope as u8
            && self.network as u8 >= required.network as u8
            && self.process_tree as u8 >= required.process_tree as u8
            && self.secrets as u8 >= required.secrets as u8
            && self.environment as u8 >= required.environment as u8
            && self.resources as u8 >= required.resources as u8
            && self.protected_paths as u8 >= required.protected_paths as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPlatform {
    Windows,
    Macos,
    Linux,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilesystemAccess {
    Read,
    Write,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilesystemCapability {
    pub root: String,
    pub access: FilesystemAccess,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkCapability {
    pub destination: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ToolCapabilitySet {
    #[serde(default)]
    pub filesystem: Vec<FilesystemCapability>,
    #[serde(default)]
    pub network: Vec<NetworkCapability>,
    #[serde(default)]
    pub secret_slots: Vec<StableTypeId>,
    #[serde(default)]
    pub process_spawn: bool,
}

impl ToolCapabilitySet {
    /// Validates bounded, duplicate-free, content-minimized capability scopes.
    ///
    /// # Errors
    /// Returns an invalid-contract error for malformed or duplicate scopes.
    pub fn validate(&self) -> Result<(), ContractError> {
        validate_count(self.filesystem.len(), "too many filesystem capabilities")?;
        validate_count(self.network.len(), "too many network capabilities")?;
        validate_count(self.secret_slots.len(), "too many secret-slot capabilities")?;

        let mut filesystem = BTreeSet::new();
        for capability in &self.filesystem {
            validate_text(
                &capability.root,
                MAX_RESOURCE_LOCATOR_BYTES,
                "filesystem capability root is invalid",
            )?;
            if !filesystem.insert((capability.root.as_str(), capability.access)) {
                return Err(ContractError::InvalidContract(
                    "filesystem capability is duplicated",
                ));
            }
        }

        let mut network = BTreeSet::new();
        for capability in &self.network {
            validate_text(
                &capability.destination,
                MAX_RESOURCE_LOCATOR_BYTES,
                "network capability destination is invalid",
            )?;
            if !network.insert(capability.destination.as_str()) {
                return Err(ContractError::InvalidContract(
                    "network capability is duplicated",
                ));
            }
        }

        let mut secrets = BTreeSet::new();
        for slot in &self.secret_slots {
            StableTypeId::parse(slot.as_str())?;
            if !secrets.insert(slot.as_str()) {
                return Err(ContractError::InvalidContract(
                    "secret-slot capability is duplicated",
                ));
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn is_subset_of(&self, maximum: &Self) -> bool {
        self.filesystem.iter().all(|requested| {
            maximum.filesystem.iter().any(|allowed| {
                allowed.root == requested.root
                    && (allowed.access == FilesystemAccess::Write
                        || allowed.access == requested.access)
            })
        }) && self
            .network
            .iter()
            .all(|requested| maximum.network.iter().any(|allowed| allowed == requested))
            && self
                .secret_slots
                .iter()
                .all(|requested| maximum.secret_slots.contains(requested))
            && (!self.process_spawn || maximum.process_spawn)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResourceLimits {
    pub timeout_ms: u64,
    pub max_output_bytes: u64,
    pub max_memory_bytes: u64,
    pub max_child_processes: u32,
}

impl ToolResourceLimits {
    /// Validates non-empty execution and output bounds.
    ///
    /// # Errors
    /// Returns an invalid-contract error for zero timeout or output bounds.
    pub fn validate(self) -> Result<(), ContractError> {
        if self.timeout_ms == 0 || self.max_output_bytes == 0 {
            return Err(ContractError::InvalidContract(
                "tool timeout and output limits must be nonzero",
            ));
        }
        Ok(())
    }

    #[must_use]
    pub const fn fits_within(self, maximum: Self) -> bool {
        self.timeout_ms <= maximum.timeout_ms
            && self.max_output_bytes <= maximum.max_output_bytes
            && self.max_memory_bytes <= maximum.max_memory_bytes
            && self.max_child_processes <= maximum.max_child_processes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolManifest {
    pub id: ToolManifestId,
    pub version: u64,
    pub name: String,
    pub summary: String,
    pub runner: SourceRef,
    pub input_schema: SourceRef,
    pub output_schema: SourceRef,
    pub maximum_effect_class: EffectClass,
    pub required_capabilities: ToolCapabilitySet,
    pub resource_ceiling: ToolResourceLimits,
    pub sandbox_requirement: SandboxCapabilityProfile,
    pub platform_support: Vec<ExecutionPlatform>,
    pub receipt_semantics: StableTypeId,
    pub reconciler: Option<SourceRef>,
}

impl ToolManifest {
    pub const SCHEMA_VERSION: u32 = 1;
    pub const TYPE_ID: &'static str = "mcg.tool_manifest.v1";

    /// Validates the exact executable, schemas, scopes, ceilings, platforms, and receipt model.
    ///
    /// # Errors
    /// Returns an invalid-contract error for an unsafe, malformed, duplicate, or unbounded field.
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.version == 0 {
            return Err(ContractError::InvalidContract(
                "tool manifest version must be nonzero",
            ));
        }
        validate_text(&self.name, 256, "tool manifest name is invalid")?;
        validate_text(
            &self.summary,
            MAX_TEXT_BYTES,
            "tool manifest summary is invalid",
        )?;
        self.runner.validate()?;
        self.input_schema.validate()?;
        self.output_schema.validate()?;
        self.required_capabilities.validate()?;
        self.resource_ceiling.validate()?;
        StableTypeId::parse(self.receipt_semantics.as_str())?;
        if self.platform_support.is_empty() {
            return Err(ContractError::InvalidContract(
                "tool manifest has no supported platform",
            ));
        }
        validate_count(self.platform_support.len(), "too many supported platforms")?;
        let mut platforms = BTreeSet::new();
        if self
            .platform_support
            .iter()
            .any(|platform| !platforms.insert(*platform))
        {
            return Err(ContractError::InvalidContract(
                "tool manifest platform is duplicated",
            ));
        }
        if let Some(reconciler) = &self.reconciler {
            reconciler.validate()?;
        }
        Ok(())
    }

    /// Returns the immutable content hash for this exact manifest version.
    ///
    /// # Errors
    /// Returns a contract error when validation or canonical encoding fails.
    pub fn content_hash(&self) -> Result<ContentHash, ContractError> {
        self.validate()?;
        canonical_contract_hash(Self::TYPE_ID, TOOL_MANIFEST_HASH_DOMAIN, self)
    }

    /// Returns an immutable reference to this exact manifest version.
    ///
    /// # Errors
    /// Returns a contract error when validation or hashing fails.
    pub fn versioned_ref(&self) -> Result<VersionedRef<ToolManifestId>, ContractError> {
        Ok(VersionedRef {
            id: self.id,
            version: self.version,
            content_hash: self.content_hash()?,
        })
    }

    #[must_use]
    pub const fn allows_effect(&self, requested: EffectClass) -> bool {
        effect_rank(requested) <= effect_rank(self.maximum_effect_class)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillStatus {
    Candidate,
    Active,
    Quarantined,
    Superseded,
    Retired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillManifest {
    pub id: SkillManifestId,
    pub version: u64,
    pub name: String,
    pub summary: String,
    pub input_schema: SourceRef,
    pub output_schema: SourceRef,
    pub preconditions: Vec<SourceRef>,
    pub required_capabilities: ToolCapabilitySet,
    pub required_tools: Vec<VersionedRef<ToolManifestId>>,
    pub implementation: VersionedPayload,
    pub verification_procedure: VersionedPayload,
    pub tested_conditions: Vec<SourceRef>,
    pub known_exclusions: Vec<SourceRef>,
    pub fallback: Option<SourceRef>,
    pub rollback_policy: VersionedPayload,
    pub provenance: Vec<SourceRef>,
    pub status: SkillStatus,
}

impl SkillManifest {
    pub const SCHEMA_VERSION: u32 = 1;
    pub const TYPE_ID: &'static str = "mcg.skill_manifest.v1";

    /// Validates the procedure, exact tool dependencies, evidence, fallback, and lifecycle state.
    ///
    /// # Errors
    /// Returns an invalid-contract error for malformed, duplicate, or unbounded evidence.
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.version == 0 {
            return Err(ContractError::InvalidContract(
                "skill manifest version must be nonzero",
            ));
        }
        validate_text(&self.name, 256, "skill manifest name is invalid")?;
        validate_text(
            &self.summary,
            MAX_TEXT_BYTES,
            "skill manifest summary is invalid",
        )?;
        self.input_schema.validate()?;
        self.output_schema.validate()?;
        self.required_capabilities.validate()?;
        self.implementation.validate()?;
        self.verification_procedure.validate()?;
        self.rollback_policy.validate()?;
        validate_sources(&self.preconditions, "skill precondition is duplicated")?;
        validate_sources(
            &self.tested_conditions,
            "skill tested condition is duplicated",
        )?;
        validate_sources(&self.known_exclusions, "skill exclusion is duplicated")?;
        validate_sources(&self.provenance, "skill provenance is duplicated")?;
        if let Some(fallback) = &self.fallback {
            fallback.validate()?;
        }
        validate_count(self.required_tools.len(), "too many required tools")?;
        let mut tools = BTreeSet::new();
        for tool in &self.required_tools {
            if tool.version == 0 {
                return Err(ContractError::InvalidContract(
                    "skill required-tool version must be nonzero",
                ));
            }
            tool.content_hash.validate()?;
            if !tools.insert((tool.id, tool.version)) {
                return Err(ContractError::InvalidContract(
                    "skill required tool is duplicated",
                ));
            }
        }
        Ok(())
    }

    /// Returns the immutable content hash for this exact Skill version.
    ///
    /// # Errors
    /// Returns a contract error when validation or canonical encoding fails.
    pub fn content_hash(&self) -> Result<ContentHash, ContractError> {
        self.validate()?;
        canonical_contract_hash(Self::TYPE_ID, SKILL_MANIFEST_HASH_DOMAIN, self)
    }

    /// Returns an immutable reference to this exact Skill version.
    ///
    /// # Errors
    /// Returns a contract error when validation or hashing fails.
    pub fn versioned_ref(&self) -> Result<VersionedRef<SkillManifestId>, ContractError> {
        Ok(VersionedRef {
            id: self.id,
            version: self.version,
            content_hash: self.content_hash()?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityKind {
    Skill,
    Tool,
    Probe,
    TemporaryRole,
    ModelRoute,
    CoordinationPolicy,
    RuntimeExtension,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityEstimate {
    pub observations: u64,
    pub success_basis_points: u16,
    pub cost_microusd: u64,
    pub latency_ms: u64,
}

impl CapabilityEstimate {
    fn validate(self) -> Result<(), ContractError> {
        if self.success_basis_points > 10_000 {
            return Err(ContractError::InvalidContract(
                "capability success estimate exceeds 100 percent",
            ));
        }
        Ok(())
    }
}

/// Rebuildable selection index. This projection is evidence, never execution authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityProfile {
    pub capability_ref: SourceRef,
    pub kind: CapabilityKind,
    pub summary: String,
    pub inputs: Vec<SourceRef>,
    pub outputs: Vec<SourceRef>,
    pub preconditions: Vec<SourceRef>,
    pub required_authority: ToolCapabilitySet,
    pub tested_environments: Vec<SourceRef>,
    pub incompatibilities: Vec<SourceRef>,
    pub competence_envelope: Option<SourceRef>,
    pub historical: CapabilityEstimate,
    pub version: u64,
}

impl CapabilityProfile {
    pub const SCHEMA_VERSION: u32 = 1;
    pub const TYPE_ID: &'static str = "mcg.capability_profile.v1";

    /// Validates the bounded, rebuildable selection evidence.
    ///
    /// # Errors
    /// Returns an invalid-contract error for malformed or duplicate inputs.
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.version == 0 {
            return Err(ContractError::InvalidContract(
                "capability profile version must be nonzero",
            ));
        }
        self.capability_ref.validate()?;
        validate_text(
            &self.summary,
            MAX_TEXT_BYTES,
            "capability summary is invalid",
        )?;
        validate_sources(&self.inputs, "capability input is duplicated")?;
        validate_sources(&self.outputs, "capability output is duplicated")?;
        validate_sources(&self.preconditions, "capability precondition is duplicated")?;
        validate_sources(
            &self.tested_environments,
            "capability environment is duplicated",
        )?;
        validate_sources(
            &self.incompatibilities,
            "capability incompatibility is duplicated",
        )?;
        self.required_authority.validate()?;
        if let Some(envelope) = &self.competence_envelope {
            envelope.validate()?;
        }
        self.historical.validate()
    }

    /// Returns the immutable hash of this rebuildable profile revision.
    ///
    /// # Errors
    /// Returns a contract error when validation or canonical encoding fails.
    pub fn content_hash(&self) -> Result<ContentHash, ContractError> {
        self.validate()?;
        canonical_contract_hash(Self::TYPE_ID, CAPABILITY_PROFILE_HASH_DOMAIN, self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolInvocationProposal {
    pub action_id: ActionId,
    pub attempt_id: crate::AttemptId,
    pub attempt_revision: u64,
    pub lease_generation: u64,
    pub runtime_actor: RuntimeActorId,
    pub runtime_identity: RuntimeIdentity,
    pub task_contract: VersionedRef<TaskContractId>,
    pub tool_manifest: VersionedRef<ToolManifestId>,
    pub arguments: ArtifactRef,
    pub argument_hash: ContentHash,
    pub requested_effect_class: EffectClass,
    pub requested_capabilities: ToolCapabilitySet,
    pub requested_limits: ToolResourceLimits,
    pub required_sandbox: SandboxCapabilityProfile,
    pub platform: ExecutionPlatform,
    pub deadline: DateTime<Utc>,
}

impl ToolInvocationProposal {
    pub const SCHEMA_VERSION: u32 = 1;
    pub const TYPE_ID: &'static str = "mcg.tool_invocation_proposal.v1";

    /// Validates immutable action, workflow, manifest, arguments, scope, and budget evidence.
    ///
    /// # Errors
    /// Returns an invalid-contract error for malformed or incomplete authority evidence.
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.attempt_revision == 0 || self.lease_generation == 0 {
            return Err(ContractError::InvalidContract(
                "tool proposal workflow fences must be nonzero",
            ));
        }
        self.runtime_identity.validate()?;
        validate_versioned_ref(&self.task_contract, "tool proposal task version is invalid")?;
        validate_versioned_ref(
            &self.tool_manifest,
            "tool proposal manifest version is invalid",
        )?;
        self.arguments.validate()?;
        self.argument_hash.validate()?;
        if self.arguments.byte_length == 0 || self.arguments.content_hash != self.argument_hash {
            return Err(ContractError::InvalidContract(
                "tool proposal arguments do not bind the exact artifact",
            ));
        }
        self.requested_capabilities.validate()?;
        self.requested_limits.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolPolicy {
    pub allowed_manifests: Vec<VersionedRef<ToolManifestId>>,
    pub maximum_effect_class: EffectClass,
    pub maximum_capabilities: ToolCapabilitySet,
    pub maximum_limits: ToolResourceLimits,
    pub maximum_permit_lifetime_seconds: u64,
}

impl ToolPolicy {
    pub const SCHEMA_VERSION: u32 = 1;
    pub const TYPE_ID: &'static str = "mcg.tool_policy.v1";

    /// Validates the Root-owned tool allowlist, scopes, effects, limits, and lifetime.
    ///
    /// # Errors
    /// Returns an invalid-contract error for empty, duplicate, or unbounded policy.
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.allowed_manifests.is_empty() {
            return Err(ContractError::InvalidContract(
                "tool policy manifest allowlist is empty",
            ));
        }
        validate_count(
            self.allowed_manifests.len(),
            "too many allowed tool manifests",
        )?;
        let mut manifests = BTreeSet::new();
        for manifest in &self.allowed_manifests {
            validate_versioned_ref(manifest, "tool policy manifest version is invalid")?;
            if !manifests.insert((manifest.id, manifest.version)) {
                return Err(ContractError::InvalidContract(
                    "tool policy manifest is duplicated",
                ));
            }
        }
        self.maximum_capabilities.validate()?;
        self.maximum_limits.validate()?;
        if self.maximum_permit_lifetime_seconds == 0
            || self.maximum_permit_lifetime_seconds > MAX_TOOL_PERMIT_LIFETIME_SECONDS
        {
            return Err(ContractError::InvalidContract(
                "tool permit lifetime is outside the supported range",
            ));
        }
        Ok(())
    }

    #[must_use]
    pub const fn allows_effect(&self, requested: EffectClass) -> bool {
        effect_rank(requested) <= effect_rank(self.maximum_effect_class)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolInvocationPermit {
    pub id: PermitId,
    pub action_id: ActionId,
    pub attempt_id: crate::AttemptId,
    pub attempt_revision: u64,
    pub lease_generation: u64,
    pub runtime_actor: RuntimeActorId,
    pub runtime_identity: RuntimeIdentity,
    pub task_contract: VersionedRef<TaskContractId>,
    pub tool_manifest: VersionedRef<ToolManifestId>,
    pub proposal_hash: ContentHash,
    pub argument_hash: ContentHash,
    pub allowed_effect_class: EffectClass,
    pub capabilities: ToolCapabilitySet,
    pub limits: ToolResourceLimits,
    pub sandbox: SandboxCapabilityProfile,
    pub platform: ExecutionPlatform,
    pub policy: PolicyBinding,
    pub expires_at: DateTime<Utc>,
    pub secret_handles: Vec<SecretHandleId>,
    pub issued_at: DateTime<Utc>,
}

impl ToolInvocationPermit {
    pub const SCHEMA_VERSION: u32 = 1;
    pub const TYPE_ID: &'static str = "mcg.tool_invocation_permit.v1";

    /// Validates bounded, content-minimized Root-issued execution authority.
    ///
    /// # Errors
    /// Returns an invalid-contract error for malformed hashes, references, scopes, or lifetime.
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.attempt_revision == 0 || self.lease_generation == 0 {
            return Err(ContractError::InvalidContract(
                "tool permit workflow fences must be nonzero",
            ));
        }
        self.runtime_identity.validate()?;
        validate_versioned_ref(&self.task_contract, "tool permit task version is invalid")?;
        validate_versioned_ref(
            &self.tool_manifest,
            "tool permit manifest version is invalid",
        )?;
        self.proposal_hash.validate()?;
        self.argument_hash.validate()?;
        self.capabilities.validate()?;
        self.limits.validate()?;
        self.policy.validate()?;
        if self.expires_at <= self.issued_at {
            return Err(ContractError::InvalidContract(
                "tool permit expiry is not after issuance",
            ));
        }
        validate_count(self.secret_handles.len(), "too many tool secret handles")?;
        let mut handles = BTreeSet::new();
        if self
            .secret_handles
            .iter()
            .any(|handle| !handles.insert(*handle))
        {
            return Err(ContractError::InvalidContract(
                "tool permit secret handle is duplicated",
            ));
        }
        Ok(())
    }
}

/// Hashes one validated Tool manifest.
///
/// # Errors
/// Returns a contract error when validation or canonical encoding fails.
pub fn tool_manifest_hash(manifest: &ToolManifest) -> Result<ContentHash, ContractError> {
    manifest.content_hash()
}

/// Hashes one validated Skill manifest.
///
/// # Errors
/// Returns a contract error when validation or canonical encoding fails.
pub fn skill_manifest_hash(manifest: &SkillManifest) -> Result<ContentHash, ContractError> {
    manifest.content_hash()
}

/// Hashes one validated derived capability profile.
///
/// # Errors
/// Returns a contract error when validation or canonical encoding fails.
pub fn capability_profile_hash(profile: &CapabilityProfile) -> Result<ContentHash, ContractError> {
    profile.content_hash()
}

/// Hashes one exact tool invocation proposal.
///
/// # Errors
/// Returns a contract error when validation or canonical encoding fails.
pub fn tool_invocation_proposal_hash(
    proposal: &ToolInvocationProposal,
) -> Result<ContentHash, ContractError> {
    proposal.validate()?;
    canonical_contract_hash(
        ToolInvocationProposal::TYPE_ID,
        TOOL_INVOCATION_PROPOSAL_HASH_DOMAIN,
        proposal,
    )
}

/// Hashes one exact Root-issued tool invocation permit.
///
/// # Errors
/// Returns a contract error when validation or canonical encoding fails.
pub fn tool_invocation_permit_hash(
    permit: &ToolInvocationPermit,
) -> Result<ContentHash, ContractError> {
    permit.validate()?;
    canonical_contract_hash(
        ToolInvocationPermit::TYPE_ID,
        TOOL_INVOCATION_PERMIT_HASH_DOMAIN,
        permit,
    )
}

const fn effect_rank(effect: EffectClass) -> u8 {
    match effect {
        EffectClass::Observational => 0,
        EffectClass::Reversible => 1,
        EffectClass::Consequential => 2,
        EffectClass::Irreversible => 3,
    }
}

fn validate_count(length: usize, error: &'static str) -> Result<(), ContractError> {
    if length > MAX_CAPABILITY_ITEMS {
        return Err(ContractError::InvalidContract(error));
    }
    Ok(())
}

fn validate_text(
    value: &str,
    maximum_bytes: usize,
    error: &'static str,
) -> Result<(), ContractError> {
    if value.is_empty() || value.len() > maximum_bytes || value.chars().any(char::is_control) {
        return Err(ContractError::InvalidContract(error));
    }
    Ok(())
}

fn validate_versioned_ref<T>(
    reference: &VersionedRef<T>,
    error: &'static str,
) -> Result<(), ContractError> {
    if reference.version == 0 {
        return Err(ContractError::InvalidContract(error));
    }
    reference.content_hash.validate()
}

fn validate_sources(sources: &[SourceRef], duplicate: &'static str) -> Result<(), ContractError> {
    validate_count(sources.len(), "too many capability source references")?;
    let mut unique = BTreeSet::new();
    for source in sources {
        source.validate()?;
        if !unique.insert((
            source.source_type.as_str(),
            source.locator.as_str(),
            source.content_hash.algorithm,
            source.content_hash.domain.as_str(),
            source.content_hash.digest_hex.as_str(),
        )) {
            return Err(ContractError::InvalidContract(duplicate));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use serde_json::json;

    use super::*;
    use crate::{HashAlgorithm, RuntimeInstanceId};

    fn hash(label: &str) -> ContentHash {
        canonical_contract_hash("mcg.test_fixture.v1", label, &json!({"label": label}))
            .expect("fixture hash")
    }

    fn source(label: &str) -> SourceRef {
        SourceRef {
            source_type: StableTypeId::parse("mcg.test_source.v1").expect("source type"),
            locator: label.into(),
            content_hash: hash("MetaCog.TestSource"),
        }
    }

    fn limits() -> ToolResourceLimits {
        ToolResourceLimits {
            timeout_ms: 30_000,
            max_output_bytes: 1_048_576,
            max_memory_bytes: 256 * 1_048_576,
            max_child_processes: 4,
        }
    }

    fn sandbox(level: IsolationGuarantee) -> SandboxCapabilityProfile {
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
                destination: "https://example.test".into(),
            }],
            secret_slots: vec![StableTypeId::parse("mcg.secret.api.v1").expect("secret slot")],
            process_spawn: false,
        }
    }

    fn tool_manifest() -> ToolManifest {
        ToolManifest {
            id: ToolManifestId::new(),
            version: 1,
            name: "fixture.tool".into(),
            summary: "Fixture governed tool".into(),
            runner: source("runner"),
            input_schema: source("input-schema"),
            output_schema: source("output-schema"),
            maximum_effect_class: EffectClass::Observational,
            required_capabilities: capabilities(),
            resource_ceiling: limits(),
            sandbox_requirement: sandbox(IsolationGuarantee::BestEffort),
            platform_support: vec![ExecutionPlatform::Windows, ExecutionPlatform::Linux],
            receipt_semantics: StableTypeId::parse("mcg.tool_receipt.v1")
                .expect("receipt semantics"),
            reconciler: None,
        }
    }

    #[test]
    fn tool_manifest_hash_binds_every_authority_dimension() {
        let manifest = tool_manifest();
        let baseline = manifest.content_hash().expect("manifest hash");
        assert_eq!(manifest.validate(), Ok(()));

        let mut changed = manifest.clone();
        changed.required_capabilities.process_spawn = true;
        assert_ne!(changed.content_hash().expect("changed hash"), baseline);

        changed = manifest.clone();
        changed.sandbox_requirement.network = IsolationGuarantee::Hard;
        assert_ne!(changed.content_hash().expect("changed hash"), baseline);

        changed = manifest;
        changed.resource_ceiling.max_output_bytes += 1;
        assert_ne!(changed.content_hash().expect("changed hash"), baseline);
    }

    #[test]
    fn capability_intersection_and_sandbox_order_fail_closed() {
        let maximum = capabilities();
        let mut requested = maximum.clone();
        assert!(requested.is_subset_of(&maximum));
        requested.filesystem[0].access = FilesystemAccess::Write;
        assert!(!requested.is_subset_of(&maximum));
        assert!(
            sandbox(IsolationGuarantee::Hard).satisfies(sandbox(IsolationGuarantee::BestEffort))
        );
        assert!(
            !sandbox(IsolationGuarantee::BestEffort).satisfies(sandbox(IsolationGuarantee::Hard))
        );
    }

    #[test]
    fn duplicate_and_unbounded_manifest_inputs_are_rejected() {
        let mut manifest = tool_manifest();
        manifest.platform_support.push(ExecutionPlatform::Windows);
        assert!(manifest.validate().is_err());

        manifest = tool_manifest();
        manifest
            .required_capabilities
            .filesystem
            .push(manifest.required_capabilities.filesystem[0].clone());
        assert!(manifest.validate().is_err());

        manifest = tool_manifest();
        manifest.resource_ceiling.timeout_ms = 0;
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn tool_proposal_and_permit_bind_exact_arguments_and_authority() {
        let manifest = tool_manifest();
        let argument_hash = hash("MetaCog.ToolArguments");
        let now = Utc
            .with_ymd_and_hms(2026, 9, 1, 12, 0, 0)
            .single()
            .expect("fixture time");
        let proposal = ToolInvocationProposal {
            action_id: ActionId::new(),
            attempt_id: crate::AttemptId::new(),
            attempt_revision: 2,
            lease_generation: 3,
            runtime_actor: RuntimeActorId::new(),
            runtime_identity: RuntimeIdentity {
                instance_id: RuntimeInstanceId::new(),
                build_hash: ContentHash {
                    algorithm: HashAlgorithm::Blake3_256,
                    domain: "MetaCog.RuntimeBuild".into(),
                    digest_hex: "11".repeat(32),
                },
                protocol_version: crate::PROTOCOL_V1,
                started_at: now,
            },
            task_contract: VersionedRef {
                id: TaskContractId::new(),
                version: 1,
                content_hash: hash("MetaCog.TaskContract"),
            },
            tool_manifest: manifest.versioned_ref().expect("manifest ref"),
            arguments: ArtifactRef {
                content_hash: argument_hash.clone(),
                media_type: "application/json".into(),
                byte_length: 2,
            },
            argument_hash,
            requested_effect_class: EffectClass::Observational,
            requested_capabilities: manifest.required_capabilities.clone(),
            requested_limits: limits(),
            required_sandbox: manifest.sandbox_requirement,
            platform: ExecutionPlatform::Windows,
            deadline: now + chrono::Duration::minutes(1),
        };
        let proposal_hash = tool_invocation_proposal_hash(&proposal).expect("proposal hash");
        let policy_payload = VersionedPayload::canonical_v1(
            StableTypeId::parse("mcg.tool_policy.v1").expect("policy type"),
            &json!({"allow": true}),
        )
        .expect("policy payload");
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
            policy: PolicyBinding {
                content_hash: policy_payload
                    .content_hash("MetaCog.ToolPolicy")
                    .expect("policy hash"),
                payload: policy_payload,
            },
            expires_at: proposal.deadline,
            secret_handles: vec![SecretHandleId::new()],
            issued_at: now,
        };
        assert_eq!(permit.validate(), Ok(()));
        tool_invocation_permit_hash(&permit).expect("permit hash");

        let mut substituted = proposal;
        substituted.arguments.content_hash = hash("MetaCog.SubstitutedArguments");
        assert!(substituted.validate().is_err());
    }

    #[test]
    fn skill_manifest_and_capability_profile_are_bounded_evidence_not_authority() {
        let tool = tool_manifest();
        let implementation = VersionedPayload::canonical_v1(
            StableTypeId::parse("mcg.skill_implementation.v1").expect("implementation type"),
            &json!({"kind": "tool_sequence", "steps": ["fixture.tool"]}),
        )
        .expect("implementation");
        let verification = VersionedPayload::canonical_v1(
            StableTypeId::parse("mcg.skill_verification.v1").expect("verification type"),
            &json!({"checks": ["output_schema"]}),
        )
        .expect("verification");
        let rollback = VersionedPayload::canonical_v1(
            StableTypeId::parse("mcg.skill_rollback.v1").expect("rollback type"),
            &json!({"fallback": "reason"}),
        )
        .expect("rollback");
        let skill = SkillManifest {
            id: SkillManifestId::new(),
            version: 1,
            name: "fixture.skill".into(),
            summary: "A validated fixture procedure".into(),
            input_schema: source("skill-input"),
            output_schema: source("skill-output"),
            preconditions: vec![source("skill-precondition")],
            required_capabilities: tool.required_capabilities.clone(),
            required_tools: vec![tool.versioned_ref().expect("tool ref")],
            implementation,
            verification_procedure: verification,
            tested_conditions: vec![source("tested-condition")],
            known_exclusions: vec![source("known-exclusion")],
            fallback: Some(source("fallback")),
            rollback_policy: rollback,
            provenance: vec![source("evaluation-evidence")],
            status: SkillStatus::Active,
        };
        let skill_ref = skill.versioned_ref().expect("skill ref");
        assert_eq!(
            skill_ref.content_hash,
            skill_manifest_hash(&skill).expect("skill hash")
        );

        let profile = CapabilityProfile {
            capability_ref: source("skill-ref"),
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
                observations: 10,
                success_basis_points: 9_000,
                cost_microusd: 20,
                latency_ms: 15,
            },
            version: 1,
        };
        capability_profile_hash(&profile).expect("capability profile hash");
        let mut invalid = profile;
        invalid.historical.success_basis_points = 10_001;
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn deserialized_invalid_stable_secret_slot_fails_validation() {
        let invalid: StableTypeId =
            serde_json::from_str("\"NOT_A_VERSIONED_TYPE\"").expect("deserialize fixture");
        let mut set = capabilities();
        set.secret_slots = vec![invalid];
        assert!(set.validate().is_err());
    }
}
