//! Durable, trust-boundary-safe contracts for the Adaptive Cognitive Agent Runtime.
//!
//! These types deliberately avoid depending on runtime, storage, provider, tool,
//! policy, or evaluator implementations. Stable identifiers and canonical bytes
//! are protocol—not Rust layout or Serde implementation details.

use std::{collections::BTreeMap, fmt};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

pub const PROTOCOL_V1: u16 = 1;
pub const MAX_CANONICAL_PAYLOAD_BYTES: usize = 16 * 1024 * 1024;

macro_rules! stable_uuid {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

stable_uuid!(RuntimeInstanceId);
stable_uuid!(RuntimeActorId);
stable_uuid!(TaskContractId);
stable_uuid!(WorkflowId);
stable_uuid!(AttemptId);
stable_uuid!(EventId);
stable_uuid!(ConfigurationId);
stable_uuid!(ActionId);
stable_uuid!(ActionExecutionId);
stable_uuid!(ActionReceiptId);
stable_uuid!(ReconciliationJobId);
stable_uuid!(ContextManifestId);
stable_uuid!(InvocationId);
stable_uuid!(CompletionCertificateId);
stable_uuid!(EvaluationSeedId);
stable_uuid!(ExperimentSpecId);
stable_uuid!(RequestId);
stable_uuid!(CorrelationId);
stable_uuid!(SessionId);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StableTypeId(String);

impl StableTypeId {
    /// Validates and constructs a stable, explicitly versioned discriminator.
    ///
    /// # Errors
    /// Returns [`ContractError::InvalidTypeId`] for unsafe or unversioned values.
    pub fn parse(value: impl Into<String>) -> Result<Self, ContractError> {
        let value = value.into();
        let version = value.rsplit_once(".v").map(|(_, version)| version);
        let valid = !value.is_empty()
            && value.len() <= 128
            && version.is_some_and(|version| {
                !version.is_empty() && version.bytes().all(|byte| byte.is_ascii_digit())
            })
            && value.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'.' || byte == b'_'
            });
        valid
            .then_some(Self(value))
            .ok_or(ContractError::InvalidTypeId)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CanonicalEncodingVersion {
    CanonicalCborV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CanonicalNormalizationVersion {
    AcarNormalizationV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HashAlgorithm {
    Blake3_256,
    Sha256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionedPayload {
    pub type_id: StableTypeId,
    pub schema_version: u32,
    pub canonical_encoding: CanonicalEncodingVersion,
    pub normalization: CanonicalNormalizationVersion,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentHash {
    pub algorithm: HashAlgorithm,
    pub domain: String,
    pub digest_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcEnvelope {
    pub protocol_version: u16,
    pub request_id: RequestId,
    pub correlation_id: CorrelationId,
    pub session_id: SessionId,
    pub caller_instance: RuntimeInstanceId,
    pub message_type: StableTypeId,
    pub expires_at: Option<DateTime<Utc>>,
    pub payload: VersionedPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthenticatedRpcRequest {
    pub envelope: RpcEnvelope,
    pub session_capability: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootMigrationReceipt {
    pub schema_version: u16,
    pub from_storage_schema: i64,
    pub to_storage_schema: i64,
    pub backup_path: Option<String>,
    pub backup_sha256: Option<String>,
    pub migrated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootStatus {
    pub schema_version: u16,
    pub service: String,
    pub process_id: u32,
    pub protocol_version: u16,
    pub session_id: SessionId,
    pub runtime_identity: RuntimeIdentity,
    pub storage_schema_version: i64,
    pub writer_queue_capacity: u32,
    pub migration_receipt: RootMigrationReceipt,
}

impl RpcEnvelope {
    /// Validates bounded framing and supported privileged protocol versions.
    ///
    /// # Errors
    /// Returns a contract error for unsupported protocol or oversized content.
    pub fn validate(&self) -> Result<(), ContractError> {
        negotiate_protocol(&[self.protocol_version])?;
        if self.payload.bytes.len() > MAX_CANONICAL_PAYLOAD_BYTES {
            return Err(ContractError::PayloadTooLarge);
        }
        decode_canonical_cbor_v1(&self.payload.bytes)?;
        Ok(())
    }
}

impl VersionedPayload {
    /// Builds the immutable v1 envelope from a structured JSON-compatible value.
    ///
    /// # Errors
    /// Returns a canonicalization error for unsupported values or oversized payloads.
    pub fn canonical_v1(type_id: StableTypeId, value: &Value) -> Result<Self, ContractError> {
        Ok(Self {
            type_id,
            schema_version: 1,
            canonical_encoding: CanonicalEncodingVersion::CanonicalCborV1,
            normalization: CanonicalNormalizationVersion::AcarNormalizationV1,
            bytes: canonical_cbor_v1(value)?,
        })
    }

    /// Hashes this exact historical payload with explicit semantic separation.
    ///
    /// # Errors
    /// Returns [`ContractError::InvalidHashDomain`] for an unsafe domain label.
    pub fn content_hash(&self, semantic_domain: &str) -> Result<ContentHash, ContractError> {
        validate_domain(semantic_domain)?;
        let prefix = format!("ACAR:{semantic_domain}:v{}\0", self.schema_version);
        let mut hasher = blake3::Hasher::new();
        hasher.update(prefix.as_bytes());
        hasher.update(&self.bytes);
        Ok(ContentHash {
            algorithm: HashAlgorithm::Blake3_256,
            domain: semantic_domain.to_owned(),
            digest_hex: hasher.finalize().to_hex().to_string(),
        })
    }
}

/// One explicit historical decoder/upcaster in a compatibility chain.
pub trait PayloadUpcaster<T> {
    fn type_id(&self) -> &StableTypeId;
    fn schema_version(&self) -> u32;
    /// Decodes this exact historical schema and returns the current semantic form.
    ///
    /// # Errors
    /// Returns a contract error when historical bytes cannot be safely interpreted.
    fn decode_and_upcast(&self, payload: &VersionedPayload) -> Result<T, ContractError>;
}

/// Selects an exact discriminator/version decoder. Unknown versions fail closed.
///
/// # Errors
/// Returns a contract error for malformed, noncanonical, or unsupported payloads.
pub fn decode_with_upcasters<T>(
    payload: &VersionedPayload,
    upcasters: &[&dyn PayloadUpcaster<T>],
) -> Result<T, ContractError> {
    if payload.canonical_encoding != CanonicalEncodingVersion::CanonicalCborV1
        || payload.normalization != CanonicalNormalizationVersion::AcarNormalizationV1
    {
        return Err(ContractError::UnsupportedCanonicalVersion);
    }
    decode_canonical_cbor_v1(&payload.bytes)?;
    upcasters
        .iter()
        .find(|decoder| {
            decoder.type_id() == &payload.type_id
                && decoder.schema_version() == payload.schema_version
        })
        .ok_or(ContractError::UnsupportedPayloadVersion)?
        .decode_and_upcast(payload)
}

/// Encodes the accepted ACAR v1 canonical-CBOR subset.
///
/// Maps use RFC 8949 deterministic key order. Text is NFC-normalized. Floats
/// are rejected because v1 contracts use integers or explicit decimal strings.
///
/// # Errors
/// Returns a contract error for unsupported numbers or oversized output.
pub fn canonical_cbor_v1(value: &Value) -> Result<Vec<u8>, ContractError> {
    let mut output = Vec::new();
    encode_value(value, &mut output)?;
    if output.len() > MAX_CANONICAL_PAYLOAD_BYTES {
        return Err(ContractError::PayloadTooLarge);
    }
    Ok(output)
}

/// Decodes only the canonical v1 subset and rejects alternate encodings.
///
/// # Errors
/// Returns a contract error for malformed, oversized, or noncanonical bytes.
pub fn decode_canonical_cbor_v1(bytes: &[u8]) -> Result<Value, ContractError> {
    if bytes.len() > MAX_CANONICAL_PAYLOAD_BYTES {
        return Err(ContractError::PayloadTooLarge);
    }
    let value: Value = ciborium::from_reader(bytes).map_err(|_| ContractError::MalformedPayload)?;
    let canonical = canonical_cbor_v1(&value)?;
    if canonical != bytes {
        return Err(ContractError::NonCanonicalPayload);
    }
    Ok(value)
}

fn encode_value(value: &Value, output: &mut Vec<u8>) -> Result<(), ContractError> {
    match value {
        Value::Null => output.push(0xf6),
        Value::Bool(false) => output.push(0xf4),
        Value::Bool(true) => output.push(0xf5),
        Value::Number(number) => {
            if let Some(unsigned) = number.as_u64() {
                encode_head(0, unsigned, output);
            } else if let Some(signed) = number.as_i64() {
                let magnitude = u64::try_from(-1_i128 - i128::from(signed))
                    .map_err(|_| ContractError::UnsupportedNumber)?;
                encode_head(1, magnitude, output);
            } else {
                return Err(ContractError::UnsupportedNumber);
            }
        }
        Value::String(text) => {
            let normalized: String = text.nfc().collect();
            encode_head(3, normalized.len() as u64, output);
            output.extend_from_slice(normalized.as_bytes());
        }
        Value::Array(items) => {
            encode_head(4, items.len() as u64, output);
            for item in items {
                encode_value(item, output)?;
            }
        }
        Value::Object(map) => {
            let mut entries = Vec::with_capacity(map.len());
            for (key, value) in map {
                let mut encoded_key = Vec::new();
                encode_value(&Value::String(key.clone()), &mut encoded_key)?;
                entries.push((encoded_key, value));
            }
            entries.sort_by(|left, right| {
                left.0
                    .len()
                    .cmp(&right.0.len())
                    .then_with(|| left.0.cmp(&right.0))
            });
            if entries.windows(2).any(|pair| pair[0].0 == pair[1].0) {
                return Err(ContractError::DuplicateNormalizedMapKey);
            }
            encode_head(5, entries.len() as u64, output);
            for (key, value) in entries {
                output.extend_from_slice(&key);
                encode_value(value, output)?;
            }
        }
    }
    Ok(())
}

fn encode_head(major: u8, argument: u64, output: &mut Vec<u8>) {
    let prefix = major << 5;
    match argument {
        0..=23 => output.push(prefix | u8::try_from(argument).expect("matched u8 range")),
        24..=0xff => output.extend_from_slice(&[
            prefix | 24,
            u8::try_from(argument).expect("matched u8 range"),
        ]),
        0x100..=0xffff => {
            output.push(prefix | 25);
            output.extend_from_slice(
                &u16::try_from(argument)
                    .expect("matched u16 range")
                    .to_be_bytes(),
            );
        }
        0x1_0000..=0xffff_ffff => {
            output.push(prefix | 26);
            output.extend_from_slice(
                &u32::try_from(argument)
                    .expect("matched u32 range")
                    .to_be_bytes(),
            );
        }
        _ => {
            output.push(prefix | 27);
            output.extend_from_slice(&argument.to_be_bytes());
        }
    }
}

fn validate_domain(domain: &str) -> Result<(), ContractError> {
    let valid = !domain.is_empty()
        && domain.len() <= 96
        && domain
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
    valid.then_some(()).ok_or(ContractError::InvalidHashDomain)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeIdentity {
    pub instance_id: RuntimeInstanceId,
    pub build_hash: ContentHash,
    pub protocol_version: u16,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionedRef<T> {
    pub id: T,
    pub version: u64,
    pub content_hash: ContentHash,
}

/// Immutable reference to evidence or user intent stored outside the contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRef {
    pub source_type: StableTypeId,
    pub locator: String,
    pub content_hash: ContentHash,
}

/// A stable, source-backed statement in a task contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractClause {
    pub clause_id: String,
    pub statement: String,
    pub source_refs: Vec<SourceRef>,
}

/// A versioned policy/configuration payload whose implementation belongs to a later trust layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyBinding {
    pub payload: VersionedPayload,
    pub content_hash: ContentHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskContractStatus {
    Draft,
    Active,
    Superseded,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskContract {
    pub id: TaskContractId,
    pub version: u64,
    pub previous_version: Option<VersionedRef<TaskContractId>>,
    pub user_intent_ref: SourceRef,
    pub objective: String,
    pub non_goals: Vec<String>,
    pub requirements: Vec<ContractClause>,
    pub constraints: Vec<ContractClause>,
    pub assumptions: Vec<ContractClause>,
    pub acceptance_criteria: Vec<ContractClause>,
    pub dependencies: Vec<VersionedRef<TaskContractId>>,
    pub permitted_side_effects: PolicyBinding,
    pub authority: PolicyBinding,
    pub environment: PolicyBinding,
    pub risk: PolicyBinding,
    pub budgets: PolicyBinding,
    pub rollback: PolicyBinding,
    pub status: TaskContractStatus,
    pub created_at: DateTime<Utc>,
}

impl TaskContract {
    pub const SCHEMA_VERSION: u32 = 1;
    pub const TYPE_ID: &'static str = "mcg.task_contract.v1";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitReason {
    User,
    Approval,
    External,
    RetryBackoff,
    Dependency,
    Reconciliation,
    ProtectedComponent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowState {
    Ready,
    Running,
    Waiting(WaitReason),
    Verifying,
    Succeeded,
    Failed,
    CancelRequested,
    Cancelled,
    Quarantined,
}

impl WorkflowState {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::Quarantined
        )
    }

    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Ready, Self::Running)
                | (
                    Self::Running,
                    Self::Waiting(_)
                        | Self::Verifying
                        | Self::Failed
                        | Self::CancelRequested
                        | Self::Quarantined
                )
                | (
                    Self::Waiting(_),
                    Self::Ready | Self::Failed | Self::CancelRequested | Self::Quarantined
                )
                | (Self::Waiting(WaitReason::Reconciliation), Self::Cancelled)
                | (
                    Self::Verifying,
                    Self::Succeeded
                        | Self::Ready
                        | Self::Failed
                        | Self::CancelRequested
                        | Self::Quarantined
                )
                | (
                    Self::CancelRequested,
                    Self::Cancelled | Self::Waiting(WaitReason::Reconciliation) | Self::Quarantined
                )
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRef {
    pub content_hash: ContentHash,
    pub media_type: String,
    pub byte_length: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub initial_backoff_ms: u64,
    pub maximum_backoff_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowAttempt {
    pub workflow_id: WorkflowId,
    pub attempt_id: AttemptId,
    pub task_contract: VersionedRef<TaskContractId>,
    pub parent_attempt_id: Option<AttemptId>,
    pub attempt_number: u32,
    pub state: WorkflowState,
    pub revision: u64,
    pub lease_owner: Option<RuntimeActorId>,
    pub lease_generation: u64,
    pub lease_expires_at: Option<DateTime<Utc>>,
    pub heartbeat_at: Option<DateTime<Utc>>,
    pub checkpoint: Option<ArtifactRef>,
    pub environment_snapshot: ContentHash,
    pub configuration: VersionedRef<ConfigurationId>,
    pub idempotency_key: String,
    pub retry_policy: RetryPolicy,
    pub compensation_policy: PolicyBinding,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl WorkflowAttempt {
    pub const SCHEMA_VERSION: u32 = 1;
    pub const TYPE_ID: &'static str = "mcg.workflow_attempt.v1";

    /// Returns whether a worker result still belongs to this authoritative attempt generation.
    #[must_use]
    pub fn accepts_worker_result(&self, attempt_id: AttemptId, lease_generation: u64) -> bool {
        matches!(
            self.state,
            WorkflowState::Running | WorkflowState::Verifying
        ) && self.attempt_id == attempt_id
            && self.lease_generation == lease_generation
            && self.lease_owner.is_some()
    }
}

/// Ordered Root-owned workflow evidence. Projection consumers never author these events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowEvent {
    pub id: EventId,
    pub type_id: StableTypeId,
    pub schema_version: u32,
    pub sequence: u64,
    pub workflow_id: WorkflowId,
    pub attempt_id: AttemptId,
    pub attempt_revision: u64,
    pub lease_generation: u64,
    pub correlation_id: CorrelationId,
    pub causation_id: Option<EventId>,
    pub idempotency_key: String,
    pub occurred_at: DateTime<Utc>,
    pub payload: VersionedPayload,
}

impl WorkflowEvent {
    pub const SCHEMA_VERSION: u32 = 1;
    pub const TYPE_ID: &'static str = "mcg.workflow_event.v1";
}

/// Canonical Root inspection of one workflow's current attempt and immutable history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowInspection {
    pub attempt: WorkflowAttempt,
    pub task_contract: TaskContract,
    pub events: Vec<WorkflowEvent>,
}

/// Revision/generation-bound request to durably begin cancellation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowCancelRequest {
    pub attempt_id: AttemptId,
    pub expected_revision: u64,
    pub expected_generation: u64,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub id: EventId,
    pub type_id: StableTypeId,
    pub occurred_at: DateTime<Utc>,
    pub payload: VersionedPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectClass {
    Observational,
    Reversible,
    Consequential,
    Irreversible,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedVersion {
    pub resource: SourceRef,
    pub expected_hash: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionEnvelope {
    pub id: ActionId,
    pub workflow_id: WorkflowId,
    pub attempt_id: AttemptId,
    pub task_contract: VersionedRef<TaskContractId>,
    pub actor: RuntimeActorId,
    pub authority: PolicyBinding,
    #[serde(default)]
    pub elevated_authority: Option<PolicyBinding>,
    pub expected_versions: Vec<ExpectedVersion>,
    pub operation: VersionedPayload,
    pub operation_hash: ContentHash,
    pub effect_class: EffectClass,
    pub preconditions: Vec<VersionedPayload>,
    pub budget_reservation: PolicyBinding,
    pub idempotency_key: String,
    pub compensation: Option<VersionedPayload>,
    #[serde(default)]
    pub compensates_action: Option<ActionId>,
    pub reconciler: Option<VersionedPayload>,
    pub reconciliation_deadline: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl ActionEnvelope {
    pub const SCHEMA_VERSION: u32 = 1;
    pub const TYPE_ID: &'static str = "mcg.action_envelope.v1";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionExecutionState {
    Proposed,
    Authorized,
    Executing,
    Observed,
    NoEffectConfirmed,
    EffectConfirmed,
    OutcomeUnknown,
    Reconciling,
    Validated,
    Committed,
    Denied,
    Quarantined,
    Compensated,
}

impl ActionExecutionState {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::NoEffectConfirmed
                | Self::Committed
                | Self::Denied
                | Self::Quarantined
                | Self::Compensated
        )
    }

    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (
                Self::Proposed,
                Self::Authorized | Self::Denied | Self::Quarantined
            ) | (
                Self::Authorized,
                Self::Executing | Self::NoEffectConfirmed | Self::Quarantined
            ) | (
                Self::Executing,
                Self::Observed | Self::OutcomeUnknown | Self::Quarantined
            ) | (
                Self::Observed,
                Self::NoEffectConfirmed | Self::EffectConfirmed | Self::OutcomeUnknown
            ) | (Self::EffectConfirmed, Self::Validated | Self::Quarantined)
                | (Self::Validated, Self::Committed | Self::Quarantined)
                | (Self::OutcomeUnknown, Self::Reconciling | Self::Quarantined)
                | (
                    Self::Reconciling,
                    Self::NoEffectConfirmed
                        | Self::EffectConfirmed
                        | Self::OutcomeUnknown
                        | Self::Quarantined
                )
                | (Self::Committed, Self::Compensated)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum EffectOutcome {
    NoEffectConfirmed,
    EffectConfirmed { evidence: Vec<ArtifactRef> },
    OutcomeUnknown { hint: VersionedPayload },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionExecution {
    pub id: ActionExecutionId,
    pub action_id: ActionId,
    pub attempt_number: u32,
    pub state: ActionExecutionState,
    pub revision: u64,
    pub lease_generation: u64,
    pub executor: Option<RuntimeActorId>,
    pub outcome: Option<EffectOutcome>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ActionExecution {
    pub const SCHEMA_VERSION: u32 = 1;
    pub const TYPE_ID: &'static str = "mcg.action_execution.v1";
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionReceipt {
    pub id: ActionReceiptId,
    pub action_id: ActionId,
    pub execution_id: ActionExecutionId,
    pub observation: VersionedPayload,
    pub observed_at: DateTime<Utc>,
}

impl ActionReceipt {
    pub const SCHEMA_VERSION: u32 = 1;
    pub const TYPE_ID: &'static str = "mcg.action_receipt.v1";
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionValidation {
    pub action_id: ActionId,
    pub execution_id: ActionExecutionId,
    pub validator: RuntimeActorId,
    pub evidence: Vec<ArtifactRef>,
    pub postconditions: Vec<VersionedPayload>,
    pub validated_at: DateTime<Utc>,
}

impl ActionValidation {
    pub const SCHEMA_VERSION: u32 = 1;
    pub const TYPE_ID: &'static str = "mcg.action_validation.v1";
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionEvent {
    pub id: EventId,
    pub type_id: StableTypeId,
    pub schema_version: u32,
    pub sequence: u64,
    pub action_id: ActionId,
    pub execution_id: Option<ActionExecutionId>,
    pub correlation_id: CorrelationId,
    pub causation_id: Option<EventId>,
    pub idempotency_key: String,
    pub occurred_at: DateTime<Utc>,
    pub payload: VersionedPayload,
}

impl ActionEvent {
    pub const SCHEMA_VERSION: u32 = 1;
    pub const TYPE_ID: &'static str = "mcg.action_event.v1";
}

/// Bounded owner-facing projection of one current canonical action execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionSummary {
    pub action_id: ActionId,
    pub execution_id: ActionExecutionId,
    pub workflow_id: WorkflowId,
    pub attempt_id: AttemptId,
    pub actor: RuntimeActorId,
    pub effect_class: EffectClass,
    pub state: ActionExecutionState,
    pub outcome: Option<EffectOutcome>,
    pub revision: u64,
    pub lease_generation: u64,
    pub updated_at: DateTime<Utc>,
}

/// Canonical Root inspection of immutable action intent, observations, and ordered history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionInspection {
    pub envelope: ActionEnvelope,
    pub executions: Vec<ActionExecution>,
    pub receipts: Vec<ActionReceipt>,
    pub reconciliation_jobs: Vec<ReconciliationJob>,
    pub events: Vec<ActionEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconciliationJobState {
    Pending,
    Running,
    Resolved,
    UserActionRequired,
    Quarantined,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconciliationJob {
    pub id: ReconciliationJobId,
    pub action_id: ActionId,
    pub execution_id: ActionExecutionId,
    pub adapter: VersionedPayload,
    pub state: ReconciliationJobState,
    pub revision: u64,
    pub lease_generation: u64,
    pub lease_owner: Option<RuntimeActorId>,
    pub lease_expires_at: Option<DateTime<Utc>>,
    pub next_attempt_at: DateTime<Utc>,
    pub attempts: u32,
    pub deadline: DateTime<Utc>,
    pub last_hint: Option<VersionedPayload>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ReconciliationJob {
    pub const SCHEMA_VERSION: u32 = 1;
    pub const TYPE_ID: &'static str = "mcg.reconciliation_job.v1";
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum ReconciliationObservation {
    ConfirmedNotOccurred,
    ConfirmedOccurred {
        receipt: ActionReceipt,
        evidence: Vec<ArtifactRef>,
    },
    StillUnknown {
        hint: VersionedPayload,
        next_attempt_at: DateTime<Utc>,
    },
}

impl ReconciliationObservation {
    pub const SCHEMA_VERSION: u32 = 1;
    pub const TYPE_ID: &'static str = "mcg.reconciliation_observation.v1";
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextManifest {
    pub id: ContextManifestId,
    pub selected_sources: Vec<ContentHash>,
    pub selection_policy: VersionedPayload,
    pub approximate_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvocationManifest {
    pub id: InvocationId,
    pub attempt_id: AttemptId,
    pub runtime_identity: RuntimeIdentity,
    pub context_manifest: ContentHash,
    pub request_hash: ContentHash,
    pub response_hash: Option<ContentHash>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionCertificate {
    pub id: CompletionCertificateId,
    pub workflow_id: WorkflowId,
    pub attempt_id: AttemptId,
    pub verification_evidence: Vec<ContentHash>,
    pub issued_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluationSeed {
    pub id: EvaluationSeedId,
    pub seed: u64,
    pub fixture_hashes: Vec<ContentHash>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentSpec {
    pub id: ExperimentSpecId,
    pub version: u64,
    pub evaluation_seed: VersionedRef<EvaluationSeedId>,
    pub conditions: BTreeMap<String, VersionedPayload>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeErrorClass {
    PolicyDenied,
    VersionConflict,
    LeaseLost,
    InvalidInput,
    BudgetExceeded,
    ProviderUnavailable,
    ProviderRejected,
    ProviderInvalidResponse,
    ToolExecutionFailed,
    ToolOutcomeUnknown,
    VerificationFailed,
    StateUnavailable,
    StateCorruption,
    ProtectedBoundaryViolation,
    ExperimentInvalid,
    EvaluationContaminated,
    Cancelled,
    Timeout,
    InternalBug,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorDisposition {
    Retryable,
    NonRetryable,
    Reconcilable,
    UserActionRequired,
    SecurityQuarantine,
    ProgrammingBug,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicRuntimeError {
    pub class: RuntimeErrorClass,
    pub disposition: ErrorDisposition,
    pub code: String,
    pub safe_message: String,
    pub correlation_id: Option<Uuid>,
}

/// Replaces sensitive values in string-keyed diagnostic data before it crosses an interface.
#[must_use]
pub fn redact_diagnostic_fields(fields: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    fields
        .iter()
        .map(|(key, value)| {
            let lower = key.to_ascii_lowercase();
            let sensitive = [
                "secret",
                "token",
                "password",
                "authorization",
                "credential",
                "private_key",
            ]
            .iter()
            .any(|marker| lower.contains(marker));
            (
                key.clone(),
                if sensitive {
                    "[REDACTED]".to_owned()
                } else {
                    value.clone()
                },
            )
        })
        .collect()
}

/// Chooses the highest mutually supported protected protocol version.
///
/// # Errors
/// Returns [`ContractError::UnsupportedProtocol`] when v1 was not offered.
pub fn negotiate_protocol(peer_versions: &[u16]) -> Result<u16, ContractError> {
    peer_versions
        .contains(&PROTOCOL_V1)
        .then_some(PROTOCOL_V1)
        .ok_or(ContractError::UnsupportedProtocol)
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ContractError {
    #[error("stable type id is invalid or lacks an explicit numeric version suffix")]
    InvalidTypeId,
    #[error("canonical hash domain is invalid")]
    InvalidHashDomain,
    #[error("floating-point or out-of-range number is not accepted by normalization v1")]
    UnsupportedNumber,
    #[error("map keys collide after canonical text normalization")]
    DuplicateNormalizedMapKey,
    #[error("canonical payload exceeds the v1 size limit")]
    PayloadTooLarge,
    #[error("payload is not valid canonical CBOR v1")]
    MalformedPayload,
    #[error("payload uses a valid but non-canonical CBOR representation")]
    NonCanonicalPayload,
    #[error("no supported protocol version was offered")]
    UnsupportedProtocol,
    #[error("canonical encoding or normalization version is unsupported")]
    UnsupportedCanonicalVersion,
    #[error("payload discriminator or schema version is unsupported")]
    UnsupportedPayloadVersion,
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use serde_json::json;

    fn hex(bytes: &[u8]) -> String {
        use std::fmt::Write as _;
        bytes.iter().fold(String::new(), |mut output, byte| {
            write!(output, "{byte:02x}").expect("writing to String cannot fail");
            output
        })
    }

    #[test]
    fn canonical_fixture_v1_is_stable() {
        let value: Value = serde_json::from_str(include_str!("../fixtures/canonical-v1.json"))
            .expect("checked-in canonical fixture");
        let expected: Value =
            serde_json::from_str(include_str!("../fixtures/canonical-v1.expected.json"))
                .expect("checked-in expected fixture");
        let payload =
            VersionedPayload::canonical_v1(StableTypeId::parse("test.fixture.v1").unwrap(), &value)
                .unwrap();
        assert_eq!(
            hex(&payload.bytes),
            expected["canonical_cbor_hex"].as_str().unwrap()
        );
        assert_eq!(
            payload.content_hash("TestFixture").unwrap().digest_hex,
            expected["blake3_256"].as_str().unwrap()
        );
    }

    #[test]
    fn canonical_equivalence_normalizes_text_and_map_order() {
        let left = json!({"b": "e\u{301}", "a": -1});
        let right = json!({"a": -1, "b": "é"});
        assert_eq!(
            canonical_cbor_v1(&left).unwrap(),
            canonical_cbor_v1(&right).unwrap()
        );
    }

    #[test]
    fn canonical_round_trip_and_noncanonical_rejection_are_stable() {
        let values = [
            json!(null),
            json!(0),
            json!(-24),
            json!("é"),
            json!([1, 2, 3]),
            json!({"a": 1}),
        ];
        for value in values {
            let bytes = canonical_cbor_v1(&value).unwrap();
            assert_eq!(decode_canonical_cbor_v1(&bytes).unwrap(), value);
        }
        // Integer zero encoded with an unnecessary extra byte.
        assert_eq!(
            decode_canonical_cbor_v1(&[0x18, 0x00]),
            Err(ContractError::NonCanonicalPayload)
        );
        assert_eq!(
            decode_canonical_cbor_v1(&[0xff]),
            Err(ContractError::MalformedPayload)
        );
    }

    #[test]
    fn malformed_contract_values_fail_closed() {
        assert_eq!(
            canonical_cbor_v1(&json!(1.5)),
            Err(ContractError::UnsupportedNumber)
        );
        assert_eq!(
            StableTypeId::parse("RustEnum::Variant"),
            Err(ContractError::InvalidTypeId)
        );
        assert_eq!(
            negotiate_protocol(&[2, 3]),
            Err(ContractError::UnsupportedProtocol)
        );
        assert_eq!(
            canonical_cbor_v1(&json!({"e\u{301}": 1, "é": 2})),
            Err(ContractError::DuplicateNormalizedMapKey)
        );
    }

    #[test]
    fn workflow_state_machine_rejects_unspecified_transitions() {
        assert!(WorkflowState::Ready.can_transition_to(WorkflowState::Running));
        assert!(WorkflowState::Verifying.can_transition_to(WorkflowState::Ready));
        assert!(!WorkflowState::Ready.can_transition_to(WorkflowState::Succeeded));
        assert!(!WorkflowState::Succeeded.can_transition_to(WorkflowState::Running));
        assert!(
            WorkflowState::CancelRequested
                .can_transition_to(WorkflowState::Waiting(WaitReason::Reconciliation))
        );
        assert!(
            !WorkflowState::CancelRequested
                .can_transition_to(WorkflowState::Waiting(WaitReason::User))
        );
    }

    #[test]
    fn workflow_transition_table_is_exhaustive_and_terminals_are_monotonic() {
        let states = [
            WorkflowState::Ready,
            WorkflowState::Running,
            WorkflowState::Waiting(WaitReason::User),
            WorkflowState::Waiting(WaitReason::Approval),
            WorkflowState::Waiting(WaitReason::External),
            WorkflowState::Waiting(WaitReason::RetryBackoff),
            WorkflowState::Waiting(WaitReason::Dependency),
            WorkflowState::Waiting(WaitReason::Reconciliation),
            WorkflowState::Waiting(WaitReason::ProtectedComponent),
            WorkflowState::Verifying,
            WorkflowState::Succeeded,
            WorkflowState::Failed,
            WorkflowState::CancelRequested,
            WorkflowState::Cancelled,
            WorkflowState::Quarantined,
        ];
        for from in states {
            for to in states {
                let expected = matches!(
                    (from, to),
                    (WorkflowState::Ready, WorkflowState::Running)
                        | (
                            WorkflowState::Running,
                            WorkflowState::Waiting(_)
                                | WorkflowState::Verifying
                                | WorkflowState::Failed
                                | WorkflowState::CancelRequested
                                | WorkflowState::Quarantined
                        )
                        | (
                            WorkflowState::Waiting(_),
                            WorkflowState::Ready
                                | WorkflowState::Failed
                                | WorkflowState::CancelRequested
                                | WorkflowState::Quarantined
                        )
                        | (
                            WorkflowState::Waiting(WaitReason::Reconciliation),
                            WorkflowState::Cancelled
                        )
                        | (
                            WorkflowState::Verifying,
                            WorkflowState::Succeeded
                                | WorkflowState::Ready
                                | WorkflowState::Failed
                                | WorkflowState::CancelRequested
                                | WorkflowState::Quarantined
                        )
                        | (
                            WorkflowState::CancelRequested,
                            WorkflowState::Cancelled
                                | WorkflowState::Waiting(WaitReason::Reconciliation)
                                | WorkflowState::Quarantined
                        )
                );
                assert_eq!(
                    from.can_transition_to(to),
                    expected,
                    "unexpected transition result for {from:?} -> {to:?}"
                );
                if from.is_terminal() {
                    assert!(!from.can_transition_to(to));
                }
            }
        }
    }

    #[test]
    fn action_execution_transition_table_separates_effect_observation_and_commit() {
        let states = [
            ActionExecutionState::Proposed,
            ActionExecutionState::Authorized,
            ActionExecutionState::Executing,
            ActionExecutionState::Observed,
            ActionExecutionState::NoEffectConfirmed,
            ActionExecutionState::EffectConfirmed,
            ActionExecutionState::OutcomeUnknown,
            ActionExecutionState::Reconciling,
            ActionExecutionState::Validated,
            ActionExecutionState::Committed,
            ActionExecutionState::Denied,
            ActionExecutionState::Quarantined,
            ActionExecutionState::Compensated,
        ];
        for from in states {
            for to in states {
                if from.is_terminal() && from != ActionExecutionState::Committed {
                    assert!(!from.can_transition_to(to));
                }
            }
        }
        assert!(ActionExecutionState::Proposed.can_transition_to(ActionExecutionState::Authorized));
        assert!(
            ActionExecutionState::Executing.can_transition_to(ActionExecutionState::OutcomeUnknown)
        );
        assert!(
            ActionExecutionState::Reconciling
                .can_transition_to(ActionExecutionState::EffectConfirmed)
        );
        assert!(
            ActionExecutionState::EffectConfirmed
                .can_transition_to(ActionExecutionState::Validated)
        );
        assert!(ActionExecutionState::Validated.can_transition_to(ActionExecutionState::Committed));
        assert!(
            !ActionExecutionState::Executing.can_transition_to(ActionExecutionState::Committed)
        );
        assert!(
            !ActionExecutionState::OutcomeUnknown
                .can_transition_to(ActionExecutionState::Executing)
        );
        assert_eq!(ActionEnvelope::TYPE_ID, "mcg.action_envelope.v1");
        assert_eq!(ActionExecution::TYPE_ID, "mcg.action_execution.v1");
        assert_eq!(ActionReceipt::TYPE_ID, "mcg.action_receipt.v1");
        assert_eq!(ActionValidation::TYPE_ID, "mcg.action_validation.v1");
        assert_eq!(ReconciliationJob::TYPE_ID, "mcg.reconciliation_job.v1");
        assert_eq!(
            ReconciliationObservation::TYPE_ID,
            "mcg.reconciliation_observation.v1"
        );
    }

    #[test]
    fn workflow_worker_results_are_generation_and_attempt_fenced() {
        let attempt_id = AttemptId::new();
        let attempt = WorkflowAttempt {
            workflow_id: WorkflowId::new(),
            attempt_id,
            task_contract: VersionedRef {
                id: TaskContractId::new(),
                version: 1,
                content_hash: fixture_hash("task-contract"),
            },
            parent_attempt_id: None,
            attempt_number: 1,
            state: WorkflowState::Running,
            revision: 2,
            lease_owner: Some(RuntimeActorId::new()),
            lease_generation: 7,
            lease_expires_at: None,
            heartbeat_at: None,
            checkpoint: None,
            environment_snapshot: fixture_hash("environment"),
            configuration: VersionedRef {
                id: ConfigurationId::new(),
                version: 1,
                content_hash: fixture_hash("configuration"),
            },
            idempotency_key: "workflow-attempt-1".to_owned(),
            retry_policy: RetryPolicy {
                max_attempts: 3,
                initial_backoff_ms: 100,
                maximum_backoff_ms: 1_000,
            },
            compensation_policy: fixture_policy("compensation"),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert!(attempt.accepts_worker_result(attempt_id, 7));
        assert!(!attempt.accepts_worker_result(attempt_id, 6));
        assert!(!attempt.accepts_worker_result(AttemptId::new(), 7));

        let mut terminal = attempt;
        terminal.state = WorkflowState::Failed;
        assert!(!terminal.accepts_worker_result(attempt_id, 7));
        terminal.state = WorkflowState::CancelRequested;
        assert!(!terminal.accepts_worker_result(attempt_id, 7));

        assert_eq!(TaskContract::SCHEMA_VERSION, 1);
        assert_eq!(TaskContract::TYPE_ID, "mcg.task_contract.v1");
        assert_eq!(WorkflowAttempt::TYPE_ID, "mcg.workflow_attempt.v1");
        assert_eq!(WorkflowEvent::TYPE_ID, "mcg.workflow_event.v1");
    }

    #[test]
    fn redaction_is_case_insensitive_and_preserves_safe_fields() {
        let fields = BTreeMap::from([
            ("Authorization".to_owned(), "Bearer usable".to_owned()),
            ("workflow_id".to_owned(), "abc".to_owned()),
        ]);
        let redacted = redact_diagnostic_fields(&fields);
        assert_eq!(redacted["Authorization"], "[REDACTED]");
        assert_eq!(redacted["workflow_id"], "abc");
    }

    #[test]
    fn contracts_crate_has_no_forbidden_runtime_dependencies() {
        let manifest = include_str!("../Cargo.toml");
        for forbidden in [
            "mcg-runtime",
            "mcg-storage",
            "mcg-policy",
            "mcg-providers",
            "mcg-tools",
            "rusqlite",
            "reqwest",
            "tauri",
        ] {
            assert!(
                !manifest.contains(forbidden),
                "forbidden dependency: {forbidden}"
            );
        }
    }

    fn fixture_hash(label: &str) -> ContentHash {
        VersionedPayload::canonical_v1(
            StableTypeId::parse("test.fixture_hash.v1").unwrap(),
            &json!({"label": label}),
        )
        .unwrap()
        .content_hash("TestFixture")
        .unwrap()
    }

    fn fixture_policy(label: &str) -> PolicyBinding {
        let payload = VersionedPayload::canonical_v1(
            StableTypeId::parse("test.policy.v1").unwrap(),
            &json!({"policy": label}),
        )
        .unwrap();
        let content_hash = payload.content_hash("TestPolicy").unwrap();
        PolicyBinding {
            payload,
            content_hash,
        }
    }

    proptest! {
        #[test]
        fn canonical_map_roundtrip_property(entries in prop::collection::btree_map("[a-z]{1,8}", any::<i64>(), 0..32)) {
            let value = serde_json::to_value(entries).unwrap();
            let bytes = canonical_cbor_v1(&value).unwrap();
            prop_assert_eq!(decode_canonical_cbor_v1(&bytes).unwrap(), value);
        }

        #[test]
        fn arbitrary_bytes_never_bypass_canonical_reencoding(bytes in prop::collection::vec(any::<u8>(), 0..512)) {
            if let Ok(value) = decode_canonical_cbor_v1(&bytes) {
                prop_assert_eq!(canonical_cbor_v1(&value).unwrap(), bytes);
            }
        }
    }
}
