//! Durable, trust-boundary-safe contracts for the Adaptive Cognitive Agent Runtime.
//!
//! These types deliberately avoid depending on runtime, storage, provider, tool,
//! policy, or evaluator implementations. Stable identifiers and canonical bytes
//! are protocol—not Rust layout or Serde implementation details.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

pub const PROTOCOL_V1: u16 = 1;
pub const MAX_CANONICAL_PAYLOAD_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_MANIFEST_ITEMS: usize = 4_096;
pub const MAX_CONTEXT_TRANSFORMATIONS: usize = 64;
pub const MAX_CONTEXT_EXPANSION_STEPS: usize = 256;
pub const MAX_REFERENCE_LOCATOR_BYTES: usize = 4_096;
pub const MAX_MANIFEST_TEXT_BYTES: usize = 4_096;
pub const RUNTIME_INVOCATION_REQUEST_HASH_DOMAIN: &str = "MetaCog.RuntimeInvocationRequest";
pub const RUNTIME_INVOCATION_RESPONSE_HASH_DOMAIN: &str = "MetaCog.RuntimeInvocationResponse";
pub const RUNTIME_WORKER_HELLO_MESSAGE_TYPE: &str = "metacog.runtime.hello.v1";
pub const RUNTIME_WORKER_INVOKE_MESSAGE_TYPE: &str = "metacog.runtime.invoke.v1";
pub const RUNTIME_WORKER_RESULT_MESSAGE_TYPE: &str = "metacog.runtime.result.v1";
pub const RUNTIME_WORKER_CANCEL_MESSAGE_TYPE: &str = "metacog.runtime.cancel.v1";
pub const RUNTIME_WORKER_SHUTDOWN_MESSAGE_TYPE: &str = "metacog.runtime.shutdown.v1";
pub const RUNTIME_WORKER_ACK_MESSAGE_TYPE: &str = "metacog.runtime.ack.v1";

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
stable_uuid!(ModelRunId);
stable_uuid!(RecordId);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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

    /// Validates a payload received across a trust boundary without interpreting its semantics.
    ///
    /// # Errors
    /// Returns a contract error for an invalid discriminator, version, or canonical payload.
    pub fn validate(&self) -> Result<(), ContractError> {
        validate_stable_type_id(&self.type_id)?;
        if self.schema_version == 0 {
            return Err(ContractError::InvalidContract(
                "payload schema version is zero",
            ));
        }
        decode_canonical_cbor_v1(&self.bytes)?;
        Ok(())
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

/// Produces a canonical, domain-separated hash for one typed contract value.
///
/// # Errors
/// Returns a contract error when the type ID, value, or semantic hash domain is invalid.
pub fn canonical_contract_hash<T: Serialize>(
    type_id: &str,
    semantic_domain: &str,
    value: &T,
) -> Result<ContentHash, ContractError> {
    VersionedPayload::canonical_v1(
        StableTypeId::parse(type_id)?,
        &serde_json::to_value(value).map_err(|_| ContractError::MalformedPayload)?,
    )?
    .content_hash(semantic_domain)
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

fn validate_stable_type_id(type_id: &StableTypeId) -> Result<(), ContractError> {
    let reparsed = StableTypeId::parse(type_id.as_str())?;
    (reparsed == *type_id)
        .then_some(())
        .ok_or(ContractError::InvalidTypeId)
}

fn validate_text(
    value: &str,
    maximum_bytes: usize,
    error: &'static str,
) -> Result<(), ContractError> {
    (!value.is_empty() && value.len() <= maximum_bytes && !value.chars().any(char::is_control))
        .then_some(())
        .ok_or(ContractError::InvalidContract(error))
}

fn validate_item_count(length: usize, error: &'static str) -> Result<(), ContractError> {
    (length <= MAX_MANIFEST_ITEMS)
        .then_some(())
        .ok_or(ContractError::InvalidContract(error))
}

fn validate_hashes(
    hashes: &[ContentHash],
    duplicate_error: &'static str,
) -> Result<(), ContractError> {
    validate_item_count(hashes.len(), "manifest hash collection exceeds its bound")?;
    let mut unique = BTreeSet::new();
    for hash in hashes {
        hash.validate()?;
        if !unique.insert((
            hash.algorithm,
            hash.domain.as_str(),
            hash.digest_hex.as_str(),
        )) {
            return Err(ContractError::InvalidContract(duplicate_error));
        }
    }
    Ok(())
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

impl ContentHash {
    /// Validates the algorithm, semantic domain, and lowercase fixed-width digest.
    ///
    /// # Errors
    /// Returns a contract error when this value is not a canonical 256-bit content hash.
    pub fn validate(&self) -> Result<(), ContractError> {
        validate_domain(&self.domain)?;
        if self.digest_hex.len() != 64
            || !self
                .digest_hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return Err(ContractError::InvalidContract(
                "content hash digest is not lowercase 256-bit hexadecimal",
            ));
        }
        Ok(())
    }
}

impl SourceRef {
    /// Validates a content-minimized immutable source reference.
    ///
    /// # Errors
    /// Returns a contract error for an invalid type, locator, or content hash.
    pub fn validate(&self) -> Result<(), ContractError> {
        validate_stable_type_id(&self.source_type)?;
        validate_text(
            &self.locator,
            MAX_REFERENCE_LOCATOR_BYTES,
            "source locator is empty, oversized, or contains control characters",
        )?;
        self.content_hash.validate()
    }
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

/// Immutable links from an action to the owner-visible records that caused it.
///
/// Older and non-interactive actions may omit this structure. When present, each source
/// reference binds both a stable locator and the exact content hash observed at authorization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionOrigin {
    pub conversation: Option<SourceRef>,
    pub invocation: Option<SourceRef>,
    pub command: Option<SourceRef>,
    pub approval: Option<SourceRef>,
    pub agent: Option<SourceRef>,
    #[serde(default)]
    pub evidence: Vec<SourceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionEnvelope {
    pub id: ActionId,
    pub workflow_id: WorkflowId,
    pub attempt_id: AttemptId,
    pub task_contract: VersionedRef<TaskContractId>,
    pub actor: RuntimeActorId,
    #[serde(default)]
    pub origin: Option<ActionOrigin>,
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
    /// Newer actions that causally compensate this action, in creation order.
    #[serde(default)]
    pub compensation_actions: Vec<ActionId>,
    pub executions: Vec<ActionExecution>,
    pub receipts: Vec<ActionReceipt>,
    pub reconciliation_jobs: Vec<ReconciliationJob>,
    pub events: Vec<ActionEvent>,
}

/// Owner-confirmed request to initiate the exact compensation plan already bound to an action.
///
/// The request intentionally contains no operation, actor, capability, authority, or filesystem
/// location. Protected Root derives those fields from the committed original envelope and refuses
/// stale identities before creating a new causal action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerActionCompensationRequest {
    pub action_id: ActionId,
    pub execution_id: ActionExecutionId,
    pub expected_action_revision: u64,
    pub expected_action_generation: u64,
    pub expected_workflow_revision: u64,
    pub expected_workflow_generation: u64,
    pub reason: String,
    pub confirmed: bool,
    pub idempotency_key: String,
}

impl OwnerActionCompensationRequest {
    pub const SCHEMA_VERSION: u32 = 1;
    pub const TYPE_ID: &'static str = "mcg.owner_action_compensation_request.v1";
    pub const HASH_DOMAIN: &'static str = "MetaCog.OwnerActionCompensationRequest";

    /// Validates the bounded owner intent before it crosses into Root authority derivation.
    ///
    /// # Errors
    /// Returns an invalid-contract error unless confirmation is explicit and the reason and
    /// idempotency key are nonempty bounded strings.
    pub fn validate(&self) -> Result<(), ContractError> {
        let reason = self.reason.trim();
        let idempotency_key = self.idempotency_key.trim();
        if !self.confirmed {
            return Err(ContractError::InvalidContract(
                "owner compensation confirmation is required",
            ));
        }
        if reason.is_empty() || reason.len() > MAX_MANIFEST_TEXT_BYTES {
            return Err(ContractError::InvalidContract(
                "owner compensation reason is empty or oversized",
            ));
        }
        if idempotency_key.is_empty() || idempotency_key.len() > MAX_MANIFEST_TEXT_BYTES {
            return Err(ContractError::InvalidContract(
                "owner compensation idempotency key is empty or oversized",
            ));
        }
        Ok(())
    }
}

/// Root-owned result of initiating or replaying one exact causal compensation action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionCompensationReceipt {
    pub original_action_id: ActionId,
    pub original_execution: ActionExecution,
    pub compensation: ActionInspection,
    pub replayed: bool,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvocationRole {
    Work,
    Verify,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextSelection {
    pub source: SourceRef,
    pub perspective: Option<SourceRef>,
    pub influence: VersionedPayload,
    pub transformations: Vec<SourceRef>,
    pub placement: u32,
    pub approximate_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextExpansionStep {
    pub sequence: u32,
    pub reason: StableTypeId,
    pub added_sources: Vec<SourceRef>,
}

/// Content-minimized evidence of exactly what information reached one material invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextManifest {
    pub id: ContextManifestId,
    pub attempt_id: AttemptId,
    pub selected_sources: Vec<ContextSelection>,
    pub selection_policy: VersionedPayload,
    pub expansion_steps: Vec<ContextExpansionStep>,
    pub approximate_tokens: u64,
}

impl ContextManifest {
    pub const SCHEMA_VERSION: u32 = 1;
    pub const TYPE_ID: &'static str = "mcg.context_manifest.v1";

    /// Validates ordering, bounds, exact token accounting, and every referenced payload.
    ///
    /// # Errors
    /// Returns a contract error when this manifest cannot safely reconstruct supplied context.
    pub fn validate(&self) -> Result<(), ContractError> {
        validate_item_count(
            self.selected_sources.len(),
            "context manifest contains too many selected sources",
        )?;
        if self.expansion_steps.len() > MAX_CONTEXT_EXPANSION_STEPS {
            return Err(ContractError::InvalidContract(
                "context manifest contains too many expansion steps",
            ));
        }
        self.selection_policy.validate()?;

        let mut total_tokens = 0_u64;
        for (index, selection) in self.selected_sources.iter().enumerate() {
            let expected = u32::try_from(index).map_err(|_| {
                ContractError::InvalidContract("context placement exceeds its wire range")
            })?;
            if selection.placement != expected {
                return Err(ContractError::InvalidContract(
                    "context placements are not contiguous and ordered",
                ));
            }
            selection.source.validate()?;
            if let Some(perspective) = &selection.perspective {
                perspective.validate()?;
            }
            selection.influence.validate()?;
            if selection.transformations.len() > MAX_CONTEXT_TRANSFORMATIONS {
                return Err(ContractError::InvalidContract(
                    "context selection contains too many transformations",
                ));
            }
            for transformation in &selection.transformations {
                transformation.validate()?;
            }
            total_tokens = total_tokens
                .checked_add(selection.approximate_tokens)
                .ok_or(ContractError::InvalidContract(
                    "context approximate token total overflows",
                ))?;
        }
        if total_tokens != self.approximate_tokens {
            return Err(ContractError::InvalidContract(
                "context approximate token total does not match its selections",
            ));
        }

        for (index, step) in self.expansion_steps.iter().enumerate() {
            let expected = u32::try_from(index).map_err(|_| {
                ContractError::InvalidContract("context expansion sequence exceeds its wire range")
            })?;
            if step.sequence != expected || step.added_sources.is_empty() {
                return Err(ContractError::InvalidContract(
                    "context expansion steps must be ordered and nonempty",
                ));
            }
            validate_stable_type_id(&step.reason)?;
            validate_item_count(
                step.added_sources.len(),
                "context expansion step contains too many sources",
            )?;
            for source in &step.added_sources {
                source.validate()?;
            }
        }
        Ok(())
    }
}

/// Immutable reconstruction evidence for one material model invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvocationManifest {
    pub id: InvocationId,
    pub role: InvocationRole,
    pub attempt_id: AttemptId,
    pub task_contract: VersionedRef<TaskContractId>,
    pub runtime_identity: RuntimeIdentity,
    pub cognitive_configuration: ContentHash,
    pub role_manifest: ContentHash,
    pub prompt_bundle: ContentHash,
    pub context_manifest: ContentHash,
    pub model_route: VersionedPayload,
    pub resolved_provider: SourceRef,
    pub resolved_model: SourceRef,
    pub observed_model_fingerprint: Option<String>,
    pub sampling: VersionedPayload,
    pub reasoning: Option<VersionedPayload>,
    pub available_tool_manifests: Vec<ContentHash>,
    pub selected_skill_manifests: Vec<ContentHash>,
    pub authority_envelope: SourceRef,
    pub egress_permit: Option<SourceRef>,
    pub repository_revision: Option<ContentHash>,
    pub environment: SourceRef,
    pub runtime_build: ContentHash,
    pub request_hash: ContentHash,
    pub response_hash: Option<ContentHash>,
}

impl InvocationManifest {
    pub const SCHEMA_VERSION: u32 = 1;
    pub const TYPE_ID: &'static str = "mcg.invocation_manifest.v1";

    /// Validates every immutable condition required to reconstruct a material invocation.
    ///
    /// # Errors
    /// Returns a contract error for missing, malformed, duplicate, or unbounded bindings.
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.task_contract.version == 0 {
            return Err(ContractError::InvalidContract(
                "invocation task contract version is zero",
            ));
        }
        self.task_contract.content_hash.validate()?;
        negotiate_protocol(&[self.runtime_identity.protocol_version])?;
        self.runtime_identity.build_hash.validate()?;
        for hash in [
            &self.cognitive_configuration,
            &self.role_manifest,
            &self.prompt_bundle,
            &self.context_manifest,
            &self.runtime_build,
            &self.request_hash,
        ] {
            hash.validate()?;
        }
        self.model_route.validate()?;
        self.resolved_provider.validate()?;
        self.resolved_model.validate()?;
        if let Some(fingerprint) = &self.observed_model_fingerprint {
            validate_text(
                fingerprint,
                MAX_MANIFEST_TEXT_BYTES,
                "observed model fingerprint is empty, oversized, or contains controls",
            )?;
        }
        self.sampling.validate()?;
        if let Some(reasoning) = &self.reasoning {
            reasoning.validate()?;
        }
        validate_hashes(
            &self.available_tool_manifests,
            "available tool manifest is duplicated",
        )?;
        validate_hashes(
            &self.selected_skill_manifests,
            "selected skill manifest is duplicated",
        )?;
        self.authority_envelope.validate()?;
        if let Some(permit) = &self.egress_permit {
            permit.validate()?;
        }
        if let Some(revision) = &self.repository_revision {
            revision.validate()?;
        }
        self.environment.validate()?;
        if let Some(response_hash) = &self.response_hash {
            response_hash.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelRunState {
    Planned,
    Dispatched,
    Completed,
    Failed,
    Cancelled,
    OutcomeUnknown,
}

impl ModelRunState {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Cancelled | Self::OutcomeUnknown
        )
    }

    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Planned, Self::Dispatched | Self::Cancelled)
                | (
                    Self::Dispatched,
                    Self::Completed | Self::Failed | Self::Cancelled | Self::OutcomeUnknown
                )
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
    pub reasoning_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRun {
    pub id: ModelRunId,
    pub invocation_id: InvocationId,
    pub role: InvocationRole,
    pub state: ModelRunState,
    pub provider_request_id: Option<String>,
    pub response_artifact: Option<ArtifactRef>,
    pub usage: Option<ModelUsage>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

impl ModelRun {
    pub const SCHEMA_VERSION: u32 = 1;
    pub const TYPE_ID: &'static str = "mcg.model_run.v1";

    /// Validates bounded provider metadata and the run-state timestamp invariant.
    ///
    /// # Errors
    /// Returns a contract error for malformed metadata or an inconsistent run state.
    pub fn validate(&self) -> Result<(), ContractError> {
        if let Some(provider_request_id) = &self.provider_request_id {
            validate_text(
                provider_request_id,
                MAX_MANIFEST_TEXT_BYTES,
                "provider request id is empty, oversized, or contains controls",
            )?;
        }
        if let Some(artifact) = &self.response_artifact {
            artifact.content_hash.validate()?;
            validate_text(
                &artifact.media_type,
                255,
                "artifact media type is empty, oversized, or contains controls",
            )?;
        }
        if self.state == ModelRunState::Planned
            && (self.provider_request_id.is_some()
                || self.response_artifact.is_some()
                || self.usage.is_some())
            || self.state == ModelRunState::Dispatched
                && (self.response_artifact.is_some() || self.usage.is_some())
        {
            return Err(ContractError::InvalidContract(
                "model run metadata appears before its lifecycle state permits it",
            ));
        }
        let terminal = matches!(
            self.state,
            ModelRunState::Completed
                | ModelRunState::Failed
                | ModelRunState::Cancelled
                | ModelRunState::OutcomeUnknown
        );
        if terminal != self.finished_at.is_some()
            || self
                .finished_at
                .is_some_and(|finished_at| finished_at < self.started_at)
        {
            return Err(ContractError::InvalidContract(
                "model run state and timestamps are inconsistent",
            ));
        }
        Ok(())
    }
}

/// One immutable revision in the authoritative model-run history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRunRecord {
    pub revision: u64,
    pub content_hash: ContentHash,
    pub run: ModelRun,
    pub recorded_at: DateTime<Utc>,
}

/// Bounded content-minimized projection for the global invocation browser.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvocationSummary {
    pub invocation_id: InvocationId,
    pub workflow_id: WorkflowId,
    pub attempt_id: AttemptId,
    pub role: InvocationRole,
    pub task_contract: VersionedRef<TaskContractId>,
    pub model_run_id: Option<ModelRunId>,
    pub model_run_state: Option<ModelRunState>,
    pub model_run_revision: Option<u64>,
    pub persisted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Canonical bounded reconstruction view for one material invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvocationInspection {
    pub manifest: InvocationManifest,
    pub manifest_hash: ContentHash,
    pub context_manifest: ContextManifest,
    pub context_manifest_hash: ContentHash,
    pub model_runs: Vec<ModelRunRecord>,
    pub model_runs_truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeWorkerRole {
    Cognition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeWorkerHello {
    pub schema_version: u32,
    pub role: RuntimeWorkerRole,
    pub worker_generation: u64,
    pub process_id: u32,
    pub runtime_identity: RuntimeIdentity,
    pub executable_hash: ContentHash,
}

impl RuntimeWorkerHello {
    pub const SCHEMA_VERSION: u32 = 1;
    pub const TYPE_ID: &'static str = "mcg.runtime_worker_hello.v1";

    /// Validates one executable- and generation-bound worker identity assertion.
    ///
    /// # Errors
    /// Returns a contract error for an invalid schema, generation, process, protocol, or hash.
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.schema_version != Self::SCHEMA_VERSION
            || self.worker_generation == 0
            || self.process_id == 0
        {
            return Err(ContractError::InvalidContract(
                "runtime worker hello has an invalid schema, generation, or process id",
            ));
        }
        negotiate_protocol(&[self.runtime_identity.protocol_version])?;
        self.runtime_identity.build_hash.validate()?;
        self.executable_hash.validate()?;
        if self.runtime_identity.build_hash != self.executable_hash {
            return Err(ContractError::InvalidContract(
                "runtime worker identity does not bind its executable hash",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeInvocationRequest {
    pub worker_generation: u64,
    pub attempt_revision: u64,
    pub lease_generation: u64,
    pub context_manifest: ContextManifest,
    pub invocation_manifest: InvocationManifest,
    pub model_run_id: ModelRunId,
    pub input: VersionedPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeterministicCriterionEvidence {
    pub clause_id: String,
    pub passed: bool,
    pub evidence_refs: Vec<SourceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeterministicVerifyInput {
    pub work_response_hash: ContentHash,
    pub criteria: Vec<DeterministicCriterionEvidence>,
}

impl DeterministicVerifyInput {
    pub const SCHEMA_VERSION: u32 = 1;
    pub const TYPE_ID: &'static str = "mcg.deterministic_verify_input.v1";

    /// Validates bounded independent verification input.
    ///
    /// # Errors
    /// Returns a contract error for invalid work evidence, duplicate criteria, or bad refs.
    pub fn validate(&self) -> Result<(), ContractError> {
        self.work_response_hash.validate()?;
        validate_item_count(
            self.criteria.len(),
            "deterministic Verify input contains too many criteria",
        )?;
        let mut clauses = BTreeSet::new();
        for criterion in &self.criteria {
            validate_text(
                &criterion.clause_id,
                MAX_MANIFEST_TEXT_BYTES,
                "deterministic Verify criterion id is invalid",
            )?;
            if !clauses.insert(criterion.clause_id.as_str()) {
                return Err(ContractError::InvalidContract(
                    "deterministic Verify criterion is duplicated",
                ));
            }
            validate_item_count(
                criterion.evidence_refs.len(),
                "deterministic Verify criterion has too many evidence refs",
            )?;
            for evidence in &criterion.evidence_refs {
                evidence.validate()?;
            }
        }
        Ok(())
    }
}

impl RuntimeInvocationRequest {
    pub const SCHEMA_VERSION: u32 = 1;
    pub const TYPE_ID: &'static str = "mcg.runtime_invocation_request.v1";

    /// Validates a fully reconstructable and workflow-fenced Runtime request.
    ///
    /// # Errors
    /// Returns a contract error for invalid generations, manifest substitution, or request hash.
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.worker_generation == 0 || self.attempt_revision == 0 {
            return Err(ContractError::InvalidContract(
                "runtime invocation request has an invalid generation or attempt revision",
            ));
        }
        self.context_manifest.validate()?;
        self.invocation_manifest.validate()?;
        if self.context_manifest.attempt_id != self.invocation_manifest.attempt_id {
            return Err(ContractError::InvalidContract(
                "runtime invocation context belongs to another attempt",
            ));
        }
        let context_hash = canonical_contract_hash(
            ContextManifest::TYPE_ID,
            "MetaCog.ContextManifest",
            &self.context_manifest,
        )?;
        if context_hash != self.invocation_manifest.context_manifest {
            return Err(ContractError::InvalidContract(
                "runtime invocation context hash does not match its manifest",
            ));
        }
        self.input.validate()?;
        if self
            .input
            .content_hash(RUNTIME_INVOCATION_REQUEST_HASH_DOMAIN)?
            != self.invocation_manifest.request_hash
        {
            return Err(ContractError::InvalidContract(
                "runtime invocation input does not match its request hash",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeInvocationResult {
    pub worker_generation: u64,
    pub invocation_id: InvocationId,
    pub model_run_id: ModelRunId,
    pub role: InvocationRole,
    pub request_hash: ContentHash,
    pub response: VersionedPayload,
    pub response_hash: ContentHash,
    pub criterion_results: Vec<CriterionResult>,
    pub verdict: Option<CompletionVerdict>,
}

impl RuntimeInvocationResult {
    pub const SCHEMA_VERSION: u32 = 1;
    pub const TYPE_ID: &'static str = "mcg.runtime_invocation_result.v1";

    /// Validates deterministic Runtime output without granting it commit authority.
    ///
    /// # Errors
    /// Returns a contract error for invalid hashes, bounds, or Work/Verify result shape.
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.worker_generation == 0 {
            return Err(ContractError::InvalidContract(
                "runtime invocation result generation is zero",
            ));
        }
        self.request_hash.validate()?;
        self.response.validate()?;
        if self
            .response
            .content_hash(RUNTIME_INVOCATION_RESPONSE_HASH_DOMAIN)?
            != self.response_hash
        {
            return Err(ContractError::InvalidContract(
                "runtime invocation response hash does not match its payload",
            ));
        }
        validate_item_count(
            self.criterion_results.len(),
            "runtime result contains too many criterion results",
        )?;
        for criterion in &self.criterion_results {
            validate_text(
                &criterion.clause_id,
                MAX_MANIFEST_TEXT_BYTES,
                "runtime criterion id is empty, oversized, or contains controls",
            )?;
            validate_item_count(
                criterion.evidence_refs.len(),
                "runtime criterion contains too many evidence references",
            )?;
            for evidence in &criterion.evidence_refs {
                evidence.validate()?;
            }
            criterion.detail_hash.validate()?;
        }
        match self.role {
            InvocationRole::Work
                if !self.criterion_results.is_empty() || self.verdict.is_some() =>
            {
                Err(ContractError::InvalidContract(
                    "Work result cannot assert independent verification",
                ))
            }
            InvocationRole::Verify if self.verdict.is_none() => Err(
                ContractError::InvalidContract("Verify result must propose a completion verdict"),
            ),
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeWorkerControl {
    pub worker_generation: u64,
}

impl RuntimeWorkerControl {
    pub const SCHEMA_VERSION: u32 = 1;
    pub const TYPE_ID: &'static str = "mcg.runtime_worker_control.v1";

    /// Validates a generation-fenced worker control message.
    ///
    /// # Errors
    /// Returns a contract error when the generation is zero.
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.worker_generation == 0 {
            return Err(ContractError::InvalidContract(
                "runtime worker control generation is zero",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CriterionResult {
    pub clause_id: String,
    pub passed: bool,
    pub evidence_refs: Vec<SourceRef>,
    pub detail_hash: ContentHash,
}

/// A test execution is retained through its governed immutable source reference.
pub type TestRunRef = SourceRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionVerdict {
    Satisfied,
    NotSatisfied,
    Inconclusive,
    Cancelled,
}

/// Root-owned closure evidence produced only after an independent Verify invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionCertificate {
    pub id: CompletionCertificateId,
    pub workflow_id: WorkflowId,
    pub attempt_id: AttemptId,
    pub task_contract: VersionedRef<TaskContractId>,
    pub work_invocations: Vec<InvocationId>,
    pub verify_invocations: Vec<InvocationId>,
    pub criterion_results: Vec<CriterionResult>,
    pub evidence_refs: Vec<SourceRef>,
    pub tests_run: Vec<TestRunRef>,
    pub artifacts: Vec<ArtifactRef>,
    pub known_untested_paths: Vec<String>,
    pub unresolved_assumptions: Vec<RecordId>,
    pub remaining_risks: Vec<RecordId>,
    pub verdict: CompletionVerdict,
    pub issued_at: DateTime<Utc>,
}

impl CompletionCertificate {
    pub const SCHEMA_VERSION: u32 = 1;
    pub const TYPE_ID: &'static str = "mcg.completion_certificate.v1";

    /// Validates independent Work/Verify lineage and bounded completion evidence.
    ///
    /// # Errors
    /// Returns a contract error for missing verification, duplicate lineage, malformed evidence,
    /// or a satisfied verdict that contradicts a failed criterion.
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.task_contract.version == 0 {
            return Err(ContractError::InvalidContract(
                "completion task contract version is zero",
            ));
        }
        self.task_contract.content_hash.validate()?;
        self.validate_invocation_lineage()?;
        self.validate_criteria()?;
        self.validate_evidence()?;
        self.validate_open_items()
    }

    fn validate_invocation_lineage(&self) -> Result<(), ContractError> {
        if self.work_invocations.is_empty() || self.verify_invocations.is_empty() {
            return Err(ContractError::InvalidContract(
                "completion requires both Work and independent Verify invocations",
            ));
        }
        validate_item_count(
            self.work_invocations.len(),
            "completion contains too many Work invocations",
        )?;
        validate_item_count(
            self.verify_invocations.len(),
            "completion contains too many Verify invocations",
        )?;
        let mut invocations = BTreeSet::new();
        for invocation in self.work_invocations.iter().chain(&self.verify_invocations) {
            if !invocations.insert(*invocation) {
                return Err(ContractError::InvalidContract(
                    "completion invocation lineage is duplicated or not independent",
                ));
            }
        }
        Ok(())
    }

    fn validate_criteria(&self) -> Result<(), ContractError> {
        validate_item_count(
            self.criterion_results.len(),
            "completion contains too many criterion results",
        )?;
        let mut clauses = BTreeSet::new();
        for criterion in &self.criterion_results {
            validate_text(
                &criterion.clause_id,
                MAX_MANIFEST_TEXT_BYTES,
                "criterion id is empty, oversized, or contains controls",
            )?;
            if !clauses.insert(criterion.clause_id.as_str()) {
                return Err(ContractError::InvalidContract(
                    "completion criterion result is duplicated",
                ));
            }
            validate_item_count(
                criterion.evidence_refs.len(),
                "criterion contains too many evidence references",
            )?;
            for source in &criterion.evidence_refs {
                source.validate()?;
            }
            criterion.detail_hash.validate()?;
        }
        if self.verdict == CompletionVerdict::Satisfied
            && self
                .criterion_results
                .iter()
                .any(|criterion| !criterion.passed)
        {
            return Err(ContractError::InvalidContract(
                "satisfied completion contains a failed criterion",
            ));
        }
        Ok(())
    }

    fn validate_evidence(&self) -> Result<(), ContractError> {
        validate_item_count(
            self.evidence_refs.len(),
            "completion contains too many evidence references",
        )?;
        validate_item_count(
            self.tests_run.len(),
            "completion contains too many test runs",
        )?;
        validate_item_count(
            self.artifacts.len(),
            "completion contains too many artifacts",
        )?;
        for source in self.evidence_refs.iter().chain(&self.tests_run) {
            source.validate()?;
        }
        for artifact in &self.artifacts {
            artifact.content_hash.validate()?;
            validate_text(
                &artifact.media_type,
                255,
                "artifact media type is empty, oversized, or contains controls",
            )?;
        }
        Ok(())
    }

    fn validate_open_items(&self) -> Result<(), ContractError> {
        validate_item_count(
            self.known_untested_paths.len(),
            "completion contains too many untested paths",
        )?;
        let mut untested_paths = BTreeSet::new();
        for path in &self.known_untested_paths {
            validate_text(
                path,
                MAX_MANIFEST_TEXT_BYTES,
                "untested path is empty, oversized, or contains controls",
            )?;
            if !untested_paths.insert(path.as_str()) {
                return Err(ContractError::InvalidContract(
                    "completion untested path is duplicated",
                ));
            }
        }
        validate_item_count(
            self.unresolved_assumptions.len(),
            "completion contains too many unresolved assumptions",
        )?;
        validate_item_count(
            self.remaining_risks.len(),
            "completion contains too many remaining risks",
        )?;
        if self
            .unresolved_assumptions
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            != self.unresolved_assumptions.len()
            || self.remaining_risks.iter().collect::<BTreeSet<_>>().len()
                != self.remaining_risks.len()
        {
            return Err(ContractError::InvalidContract(
                "completion assumption or risk record is duplicated",
            ));
        }
        Ok(())
    }
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
    #[error("contract invariant failed: {0}")]
    InvalidContract(&'static str),
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

    #[test]
    fn stage_five_reconstruction_contracts_round_trip_exactly() {
        let attempt_id = AttemptId(fixture_uuid("00000000-0000-4000-8000-000000000101"));
        let context = fixture_context_manifest(attempt_id);
        let invocation = fixture_invocation_manifest(attempt_id, &context);
        let run = ModelRun {
            id: ModelRunId(fixture_uuid("00000000-0000-4000-8000-000000000301")),
            invocation_id: invocation.id,
            role: InvocationRole::Work,
            state: ModelRunState::Completed,
            provider_request_id: Some("fake-provider-request-1".to_owned()),
            response_artifact: Some(ArtifactRef {
                content_hash: fixture_hash("response-artifact"),
                media_type: "application/json".to_owned(),
                byte_length: 128,
            }),
            usage: Some(ModelUsage {
                input_tokens: 21,
                output_tokens: 8,
                cached_input_tokens: 0,
                reasoning_tokens: 2,
            }),
            started_at: fixture_time("2026-08-30T12:00:00Z"),
            finished_at: Some(fixture_time("2026-08-30T12:00:01Z")),
        };
        let certificate = fixture_completion_certificate(attempt_id, invocation.id);

        context.validate().unwrap();
        invocation.validate().unwrap();
        run.validate().unwrap();
        certificate.validate().unwrap();

        let expected: Value = serde_json::from_str(include_str!(
            "../fixtures/stage-five-reconstruction-v1.expected.json"
        ))
        .expect("checked-in Stage 5 reconstruction fixture hashes");
        for (domain, expected_key, value) in [
            (
                "ContextManifest",
                "context_manifest_blake3_256",
                serde_json::to_value(&context).unwrap(),
            ),
            (
                "InvocationManifest",
                "invocation_manifest_blake3_256",
                serde_json::to_value(&invocation).unwrap(),
            ),
            (
                "ModelRun",
                "model_run_blake3_256",
                serde_json::to_value(&run).unwrap(),
            ),
            (
                "CompletionCertificate",
                "completion_certificate_blake3_256",
                serde_json::to_value(&certificate).unwrap(),
            ),
        ] {
            let hash = VersionedPayload::canonical_v1(
                StableTypeId::parse("test.stage_five_fixture.v1").unwrap(),
                &value,
            )
            .unwrap()
            .content_hash(domain)
            .unwrap();
            assert_eq!(hash.digest_hex, expected[expected_key].as_str().unwrap());
        }

        for value in [
            serde_json::to_value(&context).unwrap(),
            serde_json::to_value(&invocation).unwrap(),
            serde_json::to_value(&run).unwrap(),
            serde_json::to_value(&certificate).unwrap(),
        ] {
            let bytes = canonical_cbor_v1(&value).unwrap();
            let decoded = decode_canonical_cbor_v1(&bytes).unwrap();
            assert_eq!(decoded, value);
            assert_eq!(canonical_cbor_v1(&decoded).unwrap(), bytes);
        }
    }

    #[test]
    fn context_manifest_rejects_inexact_order_tokens_and_unbounded_transformations() {
        let attempt_id = AttemptId::new();
        let mut context = fixture_context_manifest(attempt_id);
        context.selected_sources[0].placement = 1;
        assert_eq!(
            context.validate(),
            Err(ContractError::InvalidContract(
                "context placements are not contiguous and ordered"
            ))
        );

        let mut context = fixture_context_manifest(attempt_id);
        context.approximate_tokens += 1;
        assert_eq!(
            context.validate(),
            Err(ContractError::InvalidContract(
                "context approximate token total does not match its selections"
            ))
        );

        let mut context = fixture_context_manifest(attempt_id);
        context.selected_sources[0].transformations =
            vec![fixture_source("transform"); MAX_CONTEXT_TRANSFORMATIONS + 1];
        assert_eq!(
            context.validate(),
            Err(ContractError::InvalidContract(
                "context selection contains too many transformations"
            ))
        );
    }

    #[test]
    fn invocation_and_model_run_reject_ambiguous_or_unbounded_evidence() {
        let attempt_id = AttemptId::new();
        let context = fixture_context_manifest(attempt_id);
        let mut invocation = fixture_invocation_manifest(attempt_id, &context);
        invocation
            .available_tool_manifests
            .push(invocation.available_tool_manifests[0].clone());
        assert_eq!(
            invocation.validate(),
            Err(ContractError::InvalidContract(
                "available tool manifest is duplicated"
            ))
        );

        let mut invocation = fixture_invocation_manifest(attempt_id, &context);
        invocation.resolved_provider.locator.clear();
        assert_eq!(
            invocation.validate(),
            Err(ContractError::InvalidContract(
                "source locator is empty, oversized, or contains control characters"
            ))
        );

        let run = ModelRun {
            id: ModelRunId::new(),
            invocation_id: invocation.id,
            role: InvocationRole::Work,
            state: ModelRunState::Dispatched,
            provider_request_id: None,
            response_artifact: None,
            usage: None,
            started_at: fixture_time("2026-08-30T12:00:00Z"),
            finished_at: Some(fixture_time("2026-08-30T12:00:01Z")),
        };
        assert_eq!(
            run.validate(),
            Err(ContractError::InvalidContract(
                "model run state and timestamps are inconsistent"
            ))
        );

        assert!(ModelRunState::Planned.can_transition_to(ModelRunState::Dispatched));
        assert!(ModelRunState::Dispatched.can_transition_to(ModelRunState::Completed));
        assert!(!ModelRunState::Completed.can_transition_to(ModelRunState::Dispatched));
        assert!(ModelRunState::OutcomeUnknown.is_terminal());
    }

    #[test]
    fn completion_certificate_requires_independent_verify_and_truthful_verdict() {
        let attempt_id = AttemptId::new();
        let work_invocation = InvocationId::new();
        let mut certificate = fixture_completion_certificate(attempt_id, work_invocation);
        certificate.verify_invocations[0] = work_invocation;
        assert_eq!(
            certificate.validate(),
            Err(ContractError::InvalidContract(
                "completion invocation lineage is duplicated or not independent"
            ))
        );

        let mut certificate = fixture_completion_certificate(attempt_id, work_invocation);
        certificate.criterion_results[0].passed = false;
        assert_eq!(
            certificate.validate(),
            Err(ContractError::InvalidContract(
                "satisfied completion contains a failed criterion"
            ))
        );
    }

    #[test]
    fn runtime_worker_contracts_bind_generation_context_request_and_role_semantics() {
        let attempt_id = AttemptId::new();
        let context = fixture_context_manifest(attempt_id);
        let input = fixture_payload("test.runtime_input.v1", "deterministic");
        let mut invocation = fixture_invocation_manifest(attempt_id, &context);
        invocation.context_manifest = canonical_contract_hash(
            ContextManifest::TYPE_ID,
            "MetaCog.ContextManifest",
            &context,
        )
        .unwrap();
        invocation.request_hash = input
            .content_hash(RUNTIME_INVOCATION_REQUEST_HASH_DOMAIN)
            .unwrap();
        invocation.response_hash = None;
        let request = RuntimeInvocationRequest {
            worker_generation: 1,
            attempt_revision: 1,
            lease_generation: 0,
            context_manifest: context.clone(),
            invocation_manifest: invocation.clone(),
            model_run_id: ModelRunId::new(),
            input,
        };
        request.validate().unwrap();

        let mut substituted = request.clone();
        substituted.context_manifest.approximate_tokens += 1;
        assert!(substituted.validate().is_err());
        let mut stale = request.clone();
        stale.worker_generation = 0;
        assert!(stale.validate().is_err());

        let response = fixture_payload("test.runtime_response.v1", "complete");
        let mut result = RuntimeInvocationResult {
            worker_generation: 1,
            invocation_id: invocation.id,
            model_run_id: request.model_run_id,
            role: InvocationRole::Work,
            request_hash: invocation.request_hash,
            response_hash: response
                .content_hash(RUNTIME_INVOCATION_RESPONSE_HASH_DOMAIN)
                .unwrap(),
            response,
            criterion_results: Vec::new(),
            verdict: None,
        };
        result.validate().unwrap();
        result.verdict = Some(CompletionVerdict::Satisfied);
        assert!(result.validate().is_err());

        let evidence = DeterministicCriterionEvidence {
            clause_id: "criterion-1".to_owned(),
            passed: true,
            evidence_refs: vec![fixture_source("test-evidence")],
        };
        let verify = DeterministicVerifyInput {
            work_response_hash: result.response_hash,
            criteria: vec![evidence.clone(), evidence],
        };
        assert!(verify.validate().is_err());
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

    fn fixture_uuid(value: &str) -> Uuid {
        Uuid::parse_str(value).expect("fixed test UUID")
    }

    fn fixture_time(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("fixed RFC 3339 timestamp")
            .with_timezone(&Utc)
    }

    fn fixture_payload(type_id: &str, label: &str) -> VersionedPayload {
        VersionedPayload::canonical_v1(
            StableTypeId::parse(type_id).unwrap(),
            &json!({"label": label}),
        )
        .unwrap()
    }

    fn fixture_source(label: &str) -> SourceRef {
        SourceRef {
            source_type: StableTypeId::parse("test.source.v1").unwrap(),
            locator: format!("artifact://{label}"),
            content_hash: fixture_hash(label),
        }
    }

    fn fixture_context_manifest(attempt_id: AttemptId) -> ContextManifest {
        ContextManifest {
            id: ContextManifestId(fixture_uuid("00000000-0000-4000-8000-000000000201")),
            attempt_id,
            selected_sources: vec![
                ContextSelection {
                    source: fixture_source("task"),
                    perspective: Some(fixture_source("owner-perspective")),
                    influence: fixture_payload("test.influence.v1", "trusted-user-input"),
                    transformations: vec![fixture_source("task-summary")],
                    placement: 0,
                    approximate_tokens: 13,
                },
                ContextSelection {
                    source: fixture_source("repository-state"),
                    perspective: None,
                    influence: fixture_payload("test.influence.v1", "local-observation"),
                    transformations: Vec::new(),
                    placement: 1,
                    approximate_tokens: 8,
                },
            ],
            selection_policy: fixture_payload("test.context_policy.v1", "bounded-progressive"),
            expansion_steps: vec![ContextExpansionStep {
                sequence: 0,
                reason: StableTypeId::parse("test.expansion_reason.v1").unwrap(),
                added_sources: vec![fixture_source("repository-state")],
            }],
            approximate_tokens: 21,
        }
    }

    fn fixture_invocation_manifest(
        attempt_id: AttemptId,
        context: &ContextManifest,
    ) -> InvocationManifest {
        let context_hash = VersionedPayload::canonical_v1(
            StableTypeId::parse(ContextManifest::TYPE_ID).unwrap(),
            &serde_json::to_value(context).unwrap(),
        )
        .unwrap()
        .content_hash("ContextManifest")
        .unwrap();
        InvocationManifest {
            id: InvocationId(fixture_uuid("00000000-0000-4000-8000-000000000401")),
            role: InvocationRole::Work,
            attempt_id,
            task_contract: VersionedRef {
                id: TaskContractId(fixture_uuid("00000000-0000-4000-8000-000000000001")),
                version: 1,
                content_hash: fixture_hash("task-contract"),
            },
            runtime_identity: RuntimeIdentity {
                instance_id: RuntimeInstanceId(fixture_uuid(
                    "00000000-0000-4000-8000-000000000501",
                )),
                build_hash: fixture_hash("runtime-build"),
                protocol_version: PROTOCOL_V1,
                started_at: fixture_time("2026-08-30T11:59:00Z"),
            },
            cognitive_configuration: fixture_hash("cognitive-configuration"),
            role_manifest: fixture_hash("work-role"),
            prompt_bundle: fixture_hash("prompt-bundle"),
            context_manifest: context_hash,
            model_route: fixture_payload("test.model_route.v1", "deterministic-fake"),
            resolved_provider: fixture_source("fake-provider"),
            resolved_model: fixture_source("fake-model-v1"),
            observed_model_fingerprint: Some("fake-model-v1-build-1".to_owned()),
            sampling: fixture_payload("test.sampling.v1", "temperature-zero"),
            reasoning: Some(fixture_payload("test.reasoning.v1", "bounded")),
            available_tool_manifests: vec![fixture_hash("read-only-tool")],
            selected_skill_manifests: vec![fixture_hash("coding-skill")],
            authority_envelope: fixture_source("authority-envelope"),
            egress_permit: None,
            repository_revision: Some(fixture_hash("repository-revision")),
            environment: fixture_source("isolated-worktree"),
            runtime_build: fixture_hash("runtime-build"),
            request_hash: fixture_hash("provider-request"),
            response_hash: Some(fixture_hash("provider-response")),
        }
    }

    fn fixture_completion_certificate(
        attempt_id: AttemptId,
        work_invocation: InvocationId,
    ) -> CompletionCertificate {
        CompletionCertificate {
            id: CompletionCertificateId(fixture_uuid("00000000-0000-4000-8000-000000000601")),
            workflow_id: WorkflowId(fixture_uuid("00000000-0000-4000-8000-000000000701")),
            attempt_id,
            task_contract: VersionedRef {
                id: TaskContractId(fixture_uuid("00000000-0000-4000-8000-000000000001")),
                version: 1,
                content_hash: fixture_hash("task-contract"),
            },
            work_invocations: vec![work_invocation],
            verify_invocations: vec![InvocationId(fixture_uuid(
                "00000000-0000-4000-8000-000000000402",
            ))],
            criterion_results: vec![CriterionResult {
                clause_id: "acceptance-1".to_owned(),
                passed: true,
                evidence_refs: vec![fixture_source("verification-evidence")],
                detail_hash: fixture_hash("criterion-detail"),
            }],
            evidence_refs: vec![fixture_source("completion-evidence")],
            tests_run: vec![fixture_source("test-run")],
            artifacts: vec![ArtifactRef {
                content_hash: fixture_hash("delivered-artifact"),
                media_type: "application/octet-stream".to_owned(),
                byte_length: 64,
            }],
            known_untested_paths: vec!["provider rate-limit retry".to_owned()],
            unresolved_assumptions: vec![RecordId(fixture_uuid(
                "00000000-0000-4000-8000-000000000801",
            ))],
            remaining_risks: vec![RecordId(fixture_uuid(
                "00000000-0000-4000-8000-000000000802",
            ))],
            verdict: CompletionVerdict::Satisfied,
            issued_at: fixture_time("2026-08-30T12:00:02Z"),
        }
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

    #[test]
    fn owner_compensation_requires_bounded_explicit_confirmation() {
        let mut request = OwnerActionCompensationRequest {
            action_id: ActionId::new(),
            execution_id: ActionExecutionId::new(),
            expected_action_revision: 7,
            expected_action_generation: 6,
            expected_workflow_revision: 11,
            expected_workflow_generation: 9,
            reason: "Remove the exact resource using its pre-authorized compensation plan.".into(),
            confirmed: true,
            idempotency_key: "owner-compensation-contract-1".into(),
        };
        assert_eq!(request.validate(), Ok(()));
        request.confirmed = false;
        assert!(matches!(
            request.validate(),
            Err(ContractError::InvalidContract(
                "owner compensation confirmation is required"
            ))
        ));
        request.confirmed = true;
        request.reason = " ".into();
        assert!(request.validate().is_err());
        request.reason = "valid".into();
        request.idempotency_key = " ".into();
        assert!(request.validate().is_err());
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
