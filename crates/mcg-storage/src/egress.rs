use std::collections::BTreeSet;

use chrono::{DateTime, Duration, Utc};
use mcg_contracts::{
    ContentHash, ContextManifest, InvocationId, MODEL_DISCLOSURE_HASH_DOMAIN,
    MODEL_EGRESS_POLICY_HASH_DOMAIN, MODEL_INVOCATION_PERMIT_HASH_DOMAIN,
    MODEL_INVOCATION_PROPOSAL_HASH_DOMAIN, ModelDisclosureManifest, ModelEgressPolicy,
    ModelEgressRoute, ModelInvocationPermit, ModelInvocationProposal, PermitId, PolicyBinding,
    SecretHandleId, SourceRef, StableTypeId, VersionedPayload, WorkflowAttempt, WorkflowState,
    canonical_contract_hash, decode_canonical_cbor_v1,
};
use mcg_providers::{
    ModelCapabilities, ModelDescriptor, ModelPrice, PriceProvenance, ProviderAuthMode,
    ProviderConnection,
};
use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};

use crate::{SqliteEventStore, StorageError};

const PROVIDER_CONNECTION_TYPE: &str = "mcg.provider_connection.v1";
const PROVIDER_CONNECTION_HASH_DOMAIN: &str = "MetaCog.ProviderConnection";
const PROVIDER_MODEL_TYPE: &str = "mcg.provider_model.v1";
const PROVIDER_MODEL_HASH_DOMAIN: &str = "MetaCog.ProviderModel";
const PROVIDER_DESTINATION_TYPE: &str = "mcg.provider_destination.v1";
const PROVIDER_DESTINATION_HASH_DOMAIN: &str = "MetaCog.ProviderDestination";
const MODEL_PERMIT_SOURCE_TYPE: &str = "mcg.model_invocation_permit.v2";

#[derive(Serialize)]
struct CanonicalModelDescriptor<'a> {
    id: &'a str,
    display_name: &'a str,
    owned_by: &'a Option<String>,
    context_window: Option<u64>,
    capabilities: &'a ModelCapabilities,
    price: Option<CanonicalModelPrice<'a>>,
    metadata: &'a serde_json::Value,
}

#[derive(Serialize)]
struct CanonicalModelPrice<'a> {
    input_per_million: Option<String>,
    cached_input_per_million: Option<String>,
    output_per_million: Option<String>,
    currency: &'a str,
    provenance: &'a PriceProvenance,
}

fn canonical_price_rate(rate: Option<f64>) -> Result<Option<String>, StorageError> {
    rate.map(|rate| {
        if !rate.is_finite() || rate < 0.0 {
            return Err(StorageError::InvalidData(
                "provider model price is negative or non-finite".into(),
            ));
        }
        Ok(if rate == 0.0 {
            "0".to_owned()
        } else {
            rate.to_string()
        })
    })
    .transpose()
}

fn canonical_model_price(
    price: Option<&ModelPrice>,
) -> Result<Option<CanonicalModelPrice<'_>>, StorageError> {
    price
        .map(|price| {
            Ok(CanonicalModelPrice {
                input_per_million: canonical_price_rate(price.input_per_million)?,
                cached_input_per_million: canonical_price_rate(price.cached_input_per_million)?,
                output_per_million: canonical_price_rate(price.output_per_million)?,
                currency: &price.currency,
                provenance: &price.provenance,
            })
        })
        .transpose()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelSecretSlot {
    Primary,
    OAuthIdToken,
    OAuthRefreshToken,
}

impl ModelSecretSlot {
    const fn label(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::OAuthIdToken => "oauth_id_token",
            Self::OAuthRefreshToken => "oauth_refresh_token",
        }
    }

    fn parse(value: &str) -> Result<Self, StorageError> {
        match value {
            "primary" => Ok(Self::Primary),
            "oauth_id_token" => Ok(Self::OAuthIdToken),
            "oauth_refresh_token" => Ok(Self::OAuthRefreshToken),
            _ => Err(StorageError::InvalidData(
                "model secret-handle slot is invalid".into(),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelSecretHandleBinding {
    pub handle_id: SecretHandleId,
    pub permit_id: PermitId,
    pub connection_id: String,
    pub slot: ModelSecretSlot,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelEgressAuthorizationReceipt {
    pub proposal: ModelInvocationProposal,
    pub proposal_hash: ContentHash,
    pub permit: ModelInvocationPermit,
    pub permit_hash: ContentHash,
    pub permit_ref: SourceRef,
    pub secret_handles: Vec<ModelSecretHandleBinding>,
}

impl SqliteEventStore {
    /// Atomically records one immutable Root-issued model egress permit and opaque handle map.
    ///
    /// Exact replay returns the original receipt. Invocation, route, policy, budget, disclosure,
    /// permit, or handle substitution fails closed.
    ///
    /// # Errors
    /// Returns a storage error for malformed, stale, unauthorized, substituted, or missing data.
    pub fn authorize_model_egress(
        &mut self,
        proposal: &ModelInvocationProposal,
        policy: &ModelEgressPolicy,
        permit: &ModelInvocationPermit,
        secret_handles: &[ModelSecretHandleBinding],
        authorized_at: DateTime<Utc>,
    ) -> Result<ModelEgressAuthorizationReceipt, StorageError> {
        proposal
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        policy
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        permit
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let proposal_hash = model_proposal_hash(proposal)?;
        let disclosure_hash = model_disclosure_hash(&proposal.disclosure)?;
        let policy_binding = model_egress_policy_binding(policy)?;
        let permit_hash = model_permit_hash(permit)?;
        validate_proposal_permit_binding(
            proposal,
            policy,
            permit,
            &proposal_hash,
            &disclosure_hash,
            &policy_binding,
            authorized_at,
        )?;

        if let Some(existing) = self.model_egress_authorization(proposal.invocation_id)? {
            if existing.proposal != *proposal
                || existing.proposal_hash != proposal_hash
                || existing.permit != *permit
                || existing.permit_hash != permit_hash
                || existing.secret_handles != secret_handles
            {
                return Err(StorageError::Conflict(
                    "model egress invocation was replayed with substituted authority".into(),
                ));
            }
            return Ok(existing);
        }

        let attempt = self
            .workflow_attempt(proposal.attempt_id)?
            .ok_or(StorageError::WorkflowAttemptNotFound(proposal.attempt_id))?;
        validate_active_attempt_binding(&attempt, proposal, permit)?;
        validate_active_attempt_time(&attempt, authorized_at)?;
        let context =
            load_context_by_hash(&self.connection, &proposal.disclosure.context_manifest)?
                .ok_or_else(|| {
                    StorageError::Conflict(
                        "model disclosure context does not resolve to persisted evidence".into(),
                    )
                })?;
        validate_context_disclosure(&context, proposal)?;

        let connection = self
            .provider_connection(&proposal.route.provider_id)?
            .ok_or_else(|| {
                StorageError::InvalidData("model egress provider was not found".into())
            })?;
        let catalog = self
            .provider_catalog(&proposal.route.provider_id)?
            .ok_or_else(|| {
                StorageError::InvalidData("model egress catalog was not found".into())
            })?;
        let model = catalog
            .models
            .iter()
            .find(|model| model.id == proposal.route.model_id)
            .ok_or_else(|| StorageError::InvalidData("model egress model was not found".into()))?;
        validate_resolved_route(proposal, permit, &connection, model)?;
        validate_secret_handles(permit, secret_handles, &connection)?;

        let authorization = ModelEgressAuthorizationReceipt {
            proposal: proposal.clone(),
            proposal_hash,
            permit: permit.clone(),
            permit_hash: permit_hash.clone(),
            permit_ref: model_permit_source_ref(permit, &permit_hash)?,
            secret_handles: secret_handles.to_vec(),
        };
        persist_model_egress_authorization(
            &mut self.connection,
            &authorization,
            &disclosure_hash,
            authorized_at,
        )?;
        Ok(authorization)
    }

    /// Loads one complete content-minimized egress authorization by invocation identity.
    ///
    /// # Errors
    /// Returns a storage error for malformed or inconsistent durable evidence.
    pub fn model_egress_authorization(
        &self,
        invocation_id: InvocationId,
    ) -> Result<Option<ModelEgressAuthorizationReceipt>, StorageError> {
        let row = self
            .connection
            .query_row(
                "SELECT p.proposal_json, p.proposal_hash_json,
                        m.permit_json, m.permit_hash_json
                 FROM model_invocation_proposals p
                 JOIN model_invocation_permits m ON m.invocation_id = p.invocation_id
                 WHERE p.invocation_id = ?1",
                [invocation_id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?;
        let Some((proposal, proposal_hash, permit, permit_hash)) = row else {
            return Ok(None);
        };
        let proposal: ModelInvocationProposal = serde_json::from_str(&proposal)?;
        let proposal_hash: ContentHash = serde_json::from_str(&proposal_hash)?;
        let permit: ModelInvocationPermit = serde_json::from_str(&permit)?;
        let permit_hash: ContentHash = serde_json::from_str(&permit_hash)?;
        let secret_handles = load_secret_handles(&self.connection, permit.id)?;
        validate_loaded_authorization(
            self,
            invocation_id,
            &proposal,
            &proposal_hash,
            &permit,
            &permit_hash,
            &secret_handles,
        )?;
        Ok(Some(ModelEgressAuthorizationReceipt {
            proposal,
            proposal_hash,
            permit: permit.clone(),
            permit_hash: permit_hash.clone(),
            permit_ref: model_permit_source_ref(&permit, &permit_hash)?,
            secret_handles,
        }))
    }

    /// Loads an authorization only while its exact workflow lease and permit remain active.
    ///
    /// # Errors
    /// Returns a storage error for stale identity, revision, generation, owner, lease, or time.
    pub fn active_model_egress_authorization(
        &self,
        invocation_id: InvocationId,
        now: DateTime<Utc>,
    ) -> Result<Option<ModelEgressAuthorizationReceipt>, StorageError> {
        let Some(authorization) = self.model_egress_authorization(invocation_id)? else {
            return Ok(None);
        };
        let attempt = self
            .workflow_attempt(authorization.proposal.attempt_id)?
            .ok_or(StorageError::WorkflowAttemptNotFound(
                authorization.proposal.attempt_id,
            ))?;
        validate_active_attempt_time(&attempt, now)?;
        if now < authorization.permit.issued_at || now >= authorization.permit.expires_at {
            return Err(StorageError::Conflict(
                "model egress authorization is outside its active time window".into(),
            ));
        }
        Ok(Some(authorization))
    }

    /// Loads one opaque handle binding only when it belongs to the exact unexpired permit.
    ///
    /// # Errors
    /// Returns a storage error for malformed evidence or an expired/mismatched binding.
    pub fn resolve_model_secret_handle_binding(
        &self,
        permit_id: PermitId,
        handle_id: SecretHandleId,
        now: DateTime<Utc>,
    ) -> Result<ModelSecretHandleBinding, StorageError> {
        let binding = self
            .connection
            .query_row(
                "SELECT h.connection_id, h.slot, h.expires_at, m.invocation_id
                 FROM model_secret_handle_bindings h
                 JOIN model_invocation_permits m ON m.permit_id = h.permit_id
                 WHERE h.secret_handle_id = ?1 AND h.permit_id = ?2",
                params![handle_id.to_string(), permit_id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| {
                StorageError::InvalidData(
                    "model secret handle does not belong to the requested permit".into(),
                )
            })?;
        let expires_at = parse_time(&binding.2)?;
        let invocation_id = InvocationId(uuid::Uuid::parse_str(&binding.3).map_err(|_| {
            StorageError::InvalidData("invalid model secret-handle invocation id".into())
        })?);
        let authorization = self
            .active_model_egress_authorization(invocation_id, now)?
            .ok_or_else(|| {
                StorageError::InvalidData("model secret-handle permit is missing".into())
            })?;
        if authorization.permit.id != permit_id
            || !authorization.permit.secret_handles.contains(&handle_id)
            || authorization.permit.issued_at > now
            || authorization.permit.expires_at <= now
            || expires_at != authorization.permit.expires_at
        {
            return Err(StorageError::Conflict(
                "model secret handle is inactive or does not match its exact permit".into(),
            ));
        }
        Ok(ModelSecretHandleBinding {
            handle_id,
            permit_id,
            connection_id: binding.0,
            slot: ModelSecretSlot::parse(&binding.1)?,
            expires_at,
        })
    }
}

fn persist_model_egress_authorization(
    connection: &mut rusqlite::Connection,
    authorization: &ModelEgressAuthorizationReceipt,
    disclosure_hash: &ContentHash,
    authorized_at: DateTime<Utc>,
) -> Result<(), StorageError> {
    let proposal = &authorization.proposal;
    let permit = &authorization.permit;
    let transaction = connection.transaction()?;
    transaction.execute(
        "INSERT INTO model_invocation_proposals(
                invocation_id, workflow_attempt_id, request_hash_json, disclosure_hash_json,
                proposal_hash_json, proposal_json, proposed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            proposal.invocation_id.to_string(),
            proposal.attempt_id.to_string(),
            serde_json::to_string(&proposal.request_hash)?,
            serde_json::to_string(&disclosure_hash)?,
            serde_json::to_string(&authorization.proposal_hash)?,
            serde_json::to_string(proposal)?,
            authorized_at.to_rfc3339(),
        ],
    )?;
    transaction.execute(
        "INSERT INTO model_invocation_permits(
                permit_id, invocation_id, proposal_hash_json, permit_hash_json,
                permit_json, issued_at, expires_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            permit.id.to_string(),
            permit.invocation_id.to_string(),
            serde_json::to_string(&authorization.proposal_hash)?,
            serde_json::to_string(&authorization.permit_hash)?,
            serde_json::to_string(permit)?,
            permit.issued_at.to_rfc3339(),
            permit.expires_at.to_rfc3339(),
        ],
    )?;
    persist_secret_handles(&transaction, &authorization.secret_handles)?;
    transaction.commit()?;
    Ok(())
}

/// Computes the canonical, domain-separated hash of a model invocation proposal.
///
/// # Errors
/// Returns a storage error when canonical encoding or hashing fails.
pub fn model_proposal_hash(
    proposal: &ModelInvocationProposal,
) -> Result<ContentHash, StorageError> {
    contract_hash(
        ModelInvocationProposal::TYPE_ID,
        MODEL_INVOCATION_PROPOSAL_HASH_DOMAIN,
        proposal,
    )
}

/// Computes the canonical, domain-separated hash of a disclosure manifest.
///
/// # Errors
/// Returns a storage error when canonical encoding or hashing fails.
pub fn model_disclosure_hash(
    disclosure: &ModelDisclosureManifest,
) -> Result<ContentHash, StorageError> {
    contract_hash(
        ModelDisclosureManifest::TYPE_ID,
        MODEL_DISCLOSURE_HASH_DOMAIN,
        disclosure,
    )
}

/// Encodes and hashes one Root model-egress policy as an exact policy binding.
///
/// # Errors
/// Returns a storage error when the type, encoding, or hash cannot be constructed.
pub fn model_egress_policy_binding(
    policy: &ModelEgressPolicy,
) -> Result<PolicyBinding, StorageError> {
    let payload = VersionedPayload::canonical_v1(
        StableTypeId::parse(ModelEgressPolicy::TYPE_ID)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?,
        &serde_json::to_value(policy)?,
    )
    .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    let content_hash = payload
        .content_hash(MODEL_EGRESS_POLICY_HASH_DOMAIN)
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    Ok(PolicyBinding {
        payload,
        content_hash,
    })
}

/// Canonically encodes the exact provider/model/destination route used by an invocation.
///
/// # Errors
/// Returns a storage error when the route is invalid or cannot be canonically encoded.
pub fn model_egress_route_payload(
    route: &ModelEgressRoute,
) -> Result<VersionedPayload, StorageError> {
    route
        .validate()
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    VersionedPayload::canonical_v1(
        StableTypeId::parse(ModelEgressRoute::TYPE_ID)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?,
        &serde_json::to_value(route)?,
    )
    .map_err(|error| StorageError::InvalidData(error.to_string()))
}

/// Computes the canonical, domain-separated hash of a Root-issued permit.
///
/// # Errors
/// Returns a storage error when canonical encoding or hashing fails.
pub fn model_permit_hash(permit: &ModelInvocationPermit) -> Result<ContentHash, StorageError> {
    contract_hash(
        ModelInvocationPermit::TYPE_ID,
        MODEL_INVOCATION_PERMIT_HASH_DOMAIN,
        permit,
    )
}

/// Creates the exact immutable source reference for a persisted provider connection.
///
/// # Errors
/// Returns a storage error when the reference cannot be validated or hashed.
pub fn model_provider_source_ref(
    connection: &ProviderConnection,
) -> Result<SourceRef, StorageError> {
    source_ref(
        PROVIDER_CONNECTION_TYPE,
        &connection.id,
        PROVIDER_CONNECTION_HASH_DOMAIN,
        connection,
    )
}

/// Creates the exact immutable source reference for a provider catalog model.
///
/// # Errors
/// Returns a storage error when the reference cannot be validated or hashed.
pub fn model_descriptor_source_ref(
    provider_id: &str,
    model: &ModelDescriptor,
) -> Result<SourceRef, StorageError> {
    let canonical = CanonicalModelDescriptor {
        id: &model.id,
        display_name: &model.display_name,
        owned_by: &model.owned_by,
        context_window: model.context_window,
        capabilities: &model.capabilities,
        price: canonical_model_price(model.price.as_ref())?,
        metadata: &model.metadata,
    };
    source_ref(
        PROVIDER_MODEL_TYPE,
        &format!("{provider_id}/{}", model.id),
        PROVIDER_MODEL_HASH_DOMAIN,
        &canonical,
    )
}

/// Creates the exact immutable source reference for a provider destination.
///
/// # Errors
/// Returns a storage error when the reference cannot be validated or hashed.
pub fn model_destination_source_ref(destination: &str) -> Result<SourceRef, StorageError> {
    source_ref(
        PROVIDER_DESTINATION_TYPE,
        destination,
        PROVIDER_DESTINATION_HASH_DOMAIN,
        &destination,
    )
}

/// Creates the exact immutable source reference for a Root-issued permit.
///
/// # Errors
/// Returns a storage error when the reference cannot be validated.
pub fn model_permit_source_ref(
    permit: &ModelInvocationPermit,
    permit_hash: &ContentHash,
) -> Result<SourceRef, StorageError> {
    let source = SourceRef {
        source_type: StableTypeId::parse(MODEL_PERMIT_SOURCE_TYPE)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?,
        locator: permit.id.to_string(),
        content_hash: permit_hash.clone(),
    };
    source
        .validate()
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    Ok(source)
}

fn validate_proposal_permit_binding(
    proposal: &ModelInvocationProposal,
    policy: &ModelEgressPolicy,
    permit: &ModelInvocationPermit,
    proposal_hash: &ContentHash,
    disclosure_hash: &ContentHash,
    policy_binding: &PolicyBinding,
    authorized_at: DateTime<Utc>,
) -> Result<(), StorageError> {
    if permit.invocation_id != proposal.invocation_id
        || permit.attempt_id != proposal.attempt_id
        || permit.attempt_revision != proposal.attempt_revision
        || permit.lease_generation != proposal.lease_generation
        || permit.runtime_actor != proposal.runtime_actor
        || permit.runtime_identity != proposal.runtime_identity
        || permit.role != proposal.role
        || permit.task_contract != proposal.task_contract
        || permit.request_hash != proposal.request_hash
        || permit.proposal_hash != *proposal_hash
        || permit.disclosure_hash != *disclosure_hash
        || permit.destination_class != proposal.route.destination_class
        || permit.policy != *policy_binding
        || permit.issued_at != authorized_at
        || permit.expires_at != proposal.deadline
    {
        return Err(StorageError::Conflict(
            "model invocation permit does not bind the exact proposal".into(),
        ));
    }
    if !policy.allowed_routes.contains(&proposal.route)
        || proposal.data_classification > policy.maximum_data_classification
        || permit.allowed_data_classification
            != proposal
                .data_classification
                .max(policy.minimum_data_classification)
        || permit.allowed_data_classification > policy.maximum_data_classification
        || permit.limits != proposal.requested_limits
        || !permit.limits.fits_within(policy.maximum_limits)
    {
        return Err(StorageError::Conflict(
            "model invocation proposal exceeds Root egress policy".into(),
        ));
    }
    let lifetime = permit.expires_at - permit.issued_at;
    let maximum_lifetime = i64::try_from(policy.maximum_permit_lifetime_seconds)
        .map(Duration::seconds)
        .map_err(|_| StorageError::InvalidData("model permit lifetime overflows".into()))?;
    if permit.expires_at <= authorized_at || lifetime > maximum_lifetime {
        return Err(StorageError::Conflict(
            "model invocation permit deadline exceeds Root policy".into(),
        ));
    }
    Ok(())
}

fn validate_loaded_authorization(
    store: &SqliteEventStore,
    invocation_id: InvocationId,
    proposal: &ModelInvocationProposal,
    stored_proposal_hash: &ContentHash,
    permit: &ModelInvocationPermit,
    stored_permit_hash: &ContentHash,
    secret_handles: &[ModelSecretHandleBinding],
) -> Result<(), StorageError> {
    proposal
        .validate()
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    permit
        .validate()
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    let proposal_hash = model_proposal_hash(proposal)?;
    let permit_hash = model_permit_hash(permit)?;
    let disclosure_hash = model_disclosure_hash(&proposal.disclosure)?;
    let policy = model_egress_policy_from_binding(&permit.policy)?;
    if proposal.invocation_id != invocation_id
        || permit.invocation_id != invocation_id
        || proposal_hash != *stored_proposal_hash
        || permit_hash != *stored_permit_hash
    {
        return Err(StorageError::InvalidData(
            "stored model egress authorization fails canonical identity verification".into(),
        ));
    }
    validate_proposal_permit_binding(
        proposal,
        &policy,
        permit,
        &proposal_hash,
        &disclosure_hash,
        &permit.policy,
        permit.issued_at,
    )?;
    let attempt = store
        .workflow_attempt(proposal.attempt_id)?
        .ok_or(StorageError::WorkflowAttemptNotFound(proposal.attempt_id))?;
    validate_active_attempt_binding(&attempt, proposal, permit)?;
    let context = load_context_by_hash(&store.connection, &proposal.disclosure.context_manifest)?
        .ok_or_else(|| {
        StorageError::InvalidData(
            "stored model egress authorization references missing context".into(),
        )
    })?;
    validate_context_disclosure(&context, proposal)?;
    let connection = store
        .provider_connection(&proposal.route.provider_id)?
        .ok_or_else(|| {
            StorageError::InvalidData("stored model egress provider is missing".into())
        })?;
    let catalog = store
        .provider_catalog(&proposal.route.provider_id)?
        .ok_or_else(|| {
            StorageError::InvalidData("stored model egress catalog is missing".into())
        })?;
    let model = catalog
        .models
        .iter()
        .find(|model| model.id == proposal.route.model_id)
        .ok_or_else(|| StorageError::InvalidData("stored model egress model is missing".into()))?;
    validate_resolved_route(proposal, permit, &connection, model)?;
    validate_secret_handles(permit, secret_handles, &connection)
}

fn validate_active_attempt_binding(
    attempt: &WorkflowAttempt,
    proposal: &ModelInvocationProposal,
    permit: &ModelInvocationPermit,
) -> Result<(), StorageError> {
    let role_matches_state = matches!(
        (proposal.role, attempt.state),
        (mcg_contracts::InvocationRole::Work, WorkflowState::Running)
            | (
                mcg_contracts::InvocationRole::Verify,
                WorkflowState::Verifying
            )
    );
    if attempt.attempt_id != proposal.attempt_id
        || attempt.attempt_id != permit.attempt_id
        || attempt.revision != proposal.attempt_revision
        || attempt.revision != permit.attempt_revision
        || attempt.lease_generation != proposal.lease_generation
        || attempt.lease_generation != permit.lease_generation
        || attempt.lease_owner != Some(proposal.runtime_actor)
        || attempt.lease_owner != Some(permit.runtime_actor)
        || attempt.task_contract != proposal.task_contract
        || attempt.task_contract != permit.task_contract
        || !role_matches_state
        || !matches!(
            attempt.state,
            WorkflowState::Running | WorkflowState::Verifying
        )
    {
        return Err(StorageError::Conflict(
            "model egress permit does not belong to the exact active workflow lease".into(),
        ));
    }
    Ok(())
}

fn validate_active_attempt_time(
    attempt: &WorkflowAttempt,
    now: DateTime<Utc>,
) -> Result<(), StorageError> {
    if attempt
        .lease_expires_at
        .is_none_or(|expires_at| expires_at <= now)
    {
        return Err(StorageError::Conflict(
            "model egress authorization workflow lease is expired".into(),
        ));
    }
    Ok(())
}

fn model_egress_policy_from_binding(
    binding: &PolicyBinding,
) -> Result<ModelEgressPolicy, StorageError> {
    binding
        .validate()
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    let expected_type = StableTypeId::parse(ModelEgressPolicy::TYPE_ID)
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    let expected_hash = binding
        .payload
        .content_hash(MODEL_EGRESS_POLICY_HASH_DOMAIN)
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    if binding.payload.type_id != expected_type
        || binding.payload.schema_version != ModelEgressPolicy::SCHEMA_VERSION
        || binding.content_hash != expected_hash
    {
        return Err(StorageError::InvalidData(
            "stored model egress policy binding is not canonical".into(),
        ));
    }
    let value = decode_canonical_cbor_v1(&binding.payload.bytes)
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    let policy: ModelEgressPolicy = serde_json::from_value(value)?;
    policy
        .validate()
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    if model_egress_policy_binding(&policy)? != *binding {
        return Err(StorageError::InvalidData(
            "stored model egress policy content is not canonical".into(),
        ));
    }
    Ok(policy)
}

fn validate_context_disclosure(
    context: &ContextManifest,
    proposal: &ModelInvocationProposal,
) -> Result<(), StorageError> {
    let sources = context
        .selected_sources
        .iter()
        .map(|selection| selection.source.clone())
        .collect::<Vec<_>>();
    if context.attempt_id != proposal.attempt_id
        || context.approximate_tokens != proposal.disclosure.approximate_input_tokens
        || sources != proposal.disclosure.disclosed_sources
    {
        return Err(StorageError::Conflict(
            "model disclosure does not match the exact persisted context manifest".into(),
        ));
    }
    Ok(())
}

fn validate_resolved_route(
    proposal: &ModelInvocationProposal,
    permit: &ModelInvocationPermit,
    connection: &ProviderConnection,
    model: &ModelDescriptor,
) -> Result<(), StorageError> {
    if proposal.route.destination != connection.base_url
        || permit.resolved_provider != model_provider_source_ref(connection)?
        || permit.resolved_model != model_descriptor_source_ref(&connection.id, model)?
        || permit.destination != model_destination_source_ref(&connection.base_url)?
    {
        return Err(StorageError::Conflict(
            "model invocation permit substituted its persisted provider route".into(),
        ));
    }
    Ok(())
}

fn validate_secret_handles(
    permit: &ModelInvocationPermit,
    bindings: &[ModelSecretHandleBinding],
    connection: &ProviderConnection,
) -> Result<(), StorageError> {
    let expected_slots = model_secret_slots_for_auth(connection.auth_mode);
    if bindings.len() != expected_slots.len() || permit.secret_handles.len() != expected_slots.len()
    {
        return Err(StorageError::Conflict(
            "model invocation permit secret-handle shape does not match provider authentication"
                .into(),
        ));
    }
    let mut handles = BTreeSet::new();
    let mut slots = BTreeSet::new();
    for binding in bindings {
        if binding.permit_id != permit.id
            || binding.connection_id != connection.id
            || binding.expires_at != permit.expires_at
            || !handles.insert(binding.handle_id)
            || !slots.insert(binding.slot.label())
        {
            return Err(StorageError::Conflict(
                "model secret-handle binding is duplicated or substituted".into(),
            ));
        }
    }
    let permitted = permit
        .secret_handles
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let expected = expected_slots
        .iter()
        .map(|slot| slot.label())
        .collect::<BTreeSet<_>>();
    if handles != permitted || slots != expected {
        return Err(StorageError::Conflict(
            "model secret-handle bindings do not match the exact permit".into(),
        ));
    }
    Ok(())
}

/// Returns the canonical least-authority credential slots for one provider execution.
/// OAuth execution receives access and identity credentials but never the renewable token.
#[must_use]
pub fn model_secret_slots_for_auth(auth_mode: ProviderAuthMode) -> Vec<ModelSecretSlot> {
    match auth_mode {
        ProviderAuthMode::None => Vec::new(),
        ProviderAuthMode::ApiKey | ProviderAuthMode::BearerToken => {
            vec![ModelSecretSlot::Primary]
        }
        ProviderAuthMode::ChatGptBrowserOauth | ProviderAuthMode::ChatGptDeviceCode => {
            vec![ModelSecretSlot::Primary, ModelSecretSlot::OAuthIdToken]
        }
    }
}

fn persist_secret_handles(
    transaction: &Transaction<'_>,
    bindings: &[ModelSecretHandleBinding],
) -> Result<(), StorageError> {
    for binding in bindings {
        transaction.execute(
            "INSERT INTO model_secret_handle_bindings(
                secret_handle_id, permit_id, connection_id, slot, expires_at
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                binding.handle_id.to_string(),
                binding.permit_id.to_string(),
                binding.connection_id,
                binding.slot.label(),
                binding.expires_at.to_rfc3339(),
            ],
        )?;
    }
    Ok(())
}

fn load_secret_handles(
    connection: &rusqlite::Connection,
    permit_id: PermitId,
) -> Result<Vec<ModelSecretHandleBinding>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT secret_handle_id, connection_id, slot, expires_at
         FROM model_secret_handle_bindings WHERE permit_id = ?1 ORDER BY slot",
    )?;
    let rows = statement.query_map([permit_id.to_string()], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    rows.map(|row| {
        let (handle_id, connection_id, slot, expires_at) = row?;
        Ok(ModelSecretHandleBinding {
            handle_id: SecretHandleId(
                uuid::Uuid::parse_str(&handle_id).map_err(|_| {
                    StorageError::InvalidData("invalid model secret-handle id".into())
                })?,
            ),
            permit_id,
            connection_id,
            slot: ModelSecretSlot::parse(&slot)?,
            expires_at: parse_time(&expires_at)?,
        })
    })
    .collect()
}

fn load_context_by_hash(
    connection: &rusqlite::Connection,
    hash: &ContentHash,
) -> Result<Option<ContextManifest>, StorageError> {
    let json = connection
        .query_row(
            "SELECT manifest_json FROM context_manifests WHERE content_hash_json = ?1",
            [serde_json::to_string(hash)?],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    json.map(|value| serde_json::from_str(&value).map_err(StorageError::from))
        .transpose()
}

fn source_ref<T: Serialize>(
    source_type: &str,
    locator: &str,
    domain: &str,
    value: &T,
) -> Result<SourceRef, StorageError> {
    let source = SourceRef {
        source_type: StableTypeId::parse(source_type)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?,
        locator: locator.to_owned(),
        content_hash: contract_hash(source_type, domain, value)?,
    };
    source
        .validate()
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    Ok(source)
}

fn contract_hash<T: Serialize>(
    type_id: &str,
    domain: &str,
    value: &T,
) -> Result<ContentHash, StorageError> {
    canonical_contract_hash(type_id, domain, value)
        .map_err(|error| StorageError::InvalidData(error.to_string()))
}

fn parse_time(value: &str) -> Result<DateTime<Utc>, StorageError> {
    Ok(DateTime::parse_from_rfc3339(value)
        .map_err(|error| StorageError::InvalidData(error.to_string()))?
        .with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use mcg_contracts::{
        ArtifactRef, AttemptId, ConfigurationId, ContextManifestId, ContextSelection,
        ContractClause, CorrelationId, HashAlgorithm, InvocationManifest, InvocationRole,
        ModelBudget, ModelDataClassification, ModelDestinationClass, ModelEgressRoute,
        ModelInvocationPermit, ModelInvocationProposal, PROTOCOL_V1, PolicyBinding, RetryPolicy,
        RuntimeActorId, RuntimeIdentity, RuntimeInstanceId, TaskContract, TaskContractId,
        TaskContractStatus, VersionedRef, WorkflowAttempt, WorkflowId, WorkflowState,
    };
    use mcg_providers::{
        CONTRACT_VERSION, ModelCapabilities, ProviderAuthMode, ProviderConnection, ProviderKind,
        ProviderModelCatalog,
    };

    use super::*;
    use crate::workflows::task_contract_content_hash;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 31, 12, 0, 0).unwrap()
    }

    fn hash(label: &str) -> ContentHash {
        VersionedPayload::canonical_v1(
            StableTypeId::parse("test.egress_ref.v1").unwrap(),
            &serde_json::json!({"label": label}),
        )
        .unwrap()
        .content_hash("TestEgressRef")
        .unwrap()
    }

    fn payload(type_id: &str, label: &str) -> VersionedPayload {
        VersionedPayload::canonical_v1(
            StableTypeId::parse(type_id).unwrap(),
            &serde_json::json!({"label": label}),
        )
        .unwrap()
    }

    fn source(label: &str) -> SourceRef {
        SourceRef {
            source_type: StableTypeId::parse("test.egress_source.v1").unwrap(),
            locator: format!("artifact://{label}"),
            content_hash: hash(label),
        }
    }

    fn policy_binding(label: &str) -> PolicyBinding {
        let payload = payload("test.egress_policy.v1", label);
        PolicyBinding {
            content_hash: payload.content_hash("TestEgressPolicy").unwrap(),
            payload,
        }
    }

    fn workflow_fixture() -> (TaskContract, WorkflowAttempt) {
        let task_contract_id = TaskContractId::new();
        let clause = ContractClause {
            clause_id: "criterion-1".into(),
            statement: "return one governed model response".into(),
            source_refs: vec![source("criterion")],
        };
        let contract = TaskContract {
            id: task_contract_id,
            version: 1,
            previous_version: None,
            user_intent_ref: source("intent"),
            objective: "exercise Root model egress authorization".into(),
            non_goals: Vec::new(),
            requirements: vec![clause.clone()],
            constraints: Vec::new(),
            assumptions: Vec::new(),
            acceptance_criteria: vec![clause],
            dependencies: Vec::new(),
            permitted_side_effects: policy_binding("model-egress"),
            authority: policy_binding("root"),
            environment: policy_binding("isolated"),
            risk: policy_binding("bounded"),
            budgets: policy_binding("bounded"),
            rollback: policy_binding("none"),
            status: TaskContractStatus::Active,
            created_at: now(),
        };
        let mut attempt = WorkflowAttempt {
            workflow_id: WorkflowId::new(),
            attempt_id: AttemptId::new(),
            task_contract: VersionedRef {
                id: task_contract_id,
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
            idempotency_key: "egress-workflow-1".into(),
            retry_policy: RetryPolicy {
                max_attempts: 2,
                initial_backoff_ms: 100,
                maximum_backoff_ms: 1_000,
            },
            compensation_policy: policy_binding("none"),
            created_at: now(),
            updated_at: now(),
        };
        attempt.task_contract.content_hash = task_contract_content_hash(&contract).unwrap();
        (contract, attempt)
    }

    #[test]
    fn oauth_execution_authority_excludes_the_renewable_credential() {
        for auth_mode in [
            ProviderAuthMode::ChatGptBrowserOauth,
            ProviderAuthMode::ChatGptDeviceCode,
        ] {
            assert_eq!(
                model_secret_slots_for_auth(auth_mode),
                vec![ModelSecretSlot::Primary, ModelSecretSlot::OAuthIdToken]
            );
        }
        assert!(
            !model_secret_slots_for_auth(ProviderAuthMode::ChatGptDeviceCode)
                .contains(&ModelSecretSlot::OAuthRefreshToken)
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn root_egress_authorization_is_exact_replay_safe_and_secret_free() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (contract, attempt) = workflow_fixture();
        store
            .create_workflow(&contract, &attempt, CorrelationId::new(), now())
            .unwrap();
        let runtime_actor = RuntimeActorId::new();
        let attempt = store
            .claim_workflow_attempt(
                attempt.attempt_id,
                attempt.revision,
                attempt.lease_generation,
                runtime_actor,
                now() + Duration::minutes(10),
                CorrelationId::new(),
                "egress-workflow-claim-1",
                now(),
            )
            .unwrap();
        let runtime_identity = RuntimeIdentity {
            instance_id: RuntimeInstanceId::new(),
            build_hash: hash("runtime-build"),
            protocol_version: PROTOCOL_V1,
            started_at: now(),
        };
        let context = ContextManifest {
            id: ContextManifestId::new(),
            attempt_id: attempt.attempt_id,
            selected_sources: vec![ContextSelection {
                source: source("project-context"),
                perspective: None,
                influence: payload("test.egress_influence.v1", "selected"),
                transformations: Vec::new(),
                placement: 0,
                approximate_tokens: 5,
            }],
            selection_policy: payload("test.egress_selection.v1", "bounded"),
            expansion_steps: Vec::new(),
            approximate_tokens: 5,
        };
        let context_hash = store.persist_context_manifest(&context, now()).unwrap();
        let connection = ProviderConnection {
            contract_version: CONTRACT_VERSION,
            id: "fixture-provider".into(),
            display_name: "Fixture Provider".into(),
            kind: ProviderKind::OpenAiCompatible,
            base_url: "https://provider.example/v1".into(),
            auth_mode: ProviderAuthMode::ApiKey,
            organization: None,
            project: None,
        };
        let model = ModelDescriptor {
            id: "fixture-model".into(),
            display_name: "Fixture Model".into(),
            owned_by: Some("fixture".into()),
            context_window: Some(16_384),
            capabilities: ModelCapabilities {
                responses: Some(true),
                ..ModelCapabilities::default()
            },
            price: None,
            metadata: serde_json::Value::Null,
        };
        assert_eq!(
            model_descriptor_source_ref(&connection.id, &model).unwrap(),
            source_ref(
                PROVIDER_MODEL_TYPE,
                &format!("{}/{}", connection.id, model.id),
                PROVIDER_MODEL_HASH_DOMAIN,
                &model,
            )
            .unwrap(),
            "price-less model authority hashes must remain backward compatible"
        );
        let mut priced_model = model.clone();
        priced_model.price = Some(ModelPrice {
            input_per_million: Some(1.0),
            cached_input_per_million: Some(0.5),
            output_per_million: Some(2.0),
            currency: "USD".into(),
            provenance: PriceProvenance::McgCatalog {
                catalog_date: "2026-09-01".into(),
            },
        });
        assert_ne!(
            model_descriptor_source_ref(&connection.id, &priced_model).unwrap(),
            model_descriptor_source_ref(&connection.id, &model).unwrap()
        );
        priced_model
            .price
            .as_mut()
            .expect("priced model")
            .input_per_million = Some(-1.0);
        assert!(model_descriptor_source_ref(&connection.id, &priced_model).is_err());
        let catalog = ProviderModelCatalog {
            connection_id: connection.id.clone(),
            models: vec![model.clone()],
            discovered_at: now(),
        };
        store
            .record_provider_configuration(&connection, &catalog, now())
            .unwrap();
        let route = ModelEgressRoute {
            provider_id: connection.id.clone(),
            model_id: model.id.clone(),
            destination: connection.base_url.clone(),
            destination_class: ModelDestinationClass::ThirdPartyRemote,
        };
        let request_hash = hash("request");
        let proposal = ModelInvocationProposal {
            invocation_id: InvocationId::new(),
            attempt_id: attempt.attempt_id,
            attempt_revision: attempt.revision,
            lease_generation: attempt.lease_generation,
            runtime_actor,
            runtime_identity: runtime_identity.clone(),
            role: InvocationRole::Work,
            task_contract: attempt.task_contract.clone(),
            request_artifact: ArtifactRef {
                content_hash: request_hash.clone(),
                media_type: "application/vnd.metacog.model-request+cbor".into(),
                byte_length: 128,
            },
            request_hash: request_hash.clone(),
            route: route.clone(),
            data_classification: ModelDataClassification::Confidential,
            disclosure: ModelDisclosureManifest {
                context_manifest: context_hash,
                disclosed_sources: vec![source("project-context")],
                approximate_input_tokens: 5,
            },
            requested_limits: ModelBudget {
                max_input_tokens: 128,
                max_output_tokens: 64,
                max_cost_microusd: 25_000,
            },
            deadline: now() + Duration::minutes(5),
        };
        let policy = ModelEgressPolicy {
            allowed_routes: vec![route],
            minimum_data_classification: ModelDataClassification::Internal,
            maximum_data_classification: ModelDataClassification::Confidential,
            maximum_limits: ModelBudget {
                max_input_tokens: 256,
                max_output_tokens: 128,
                max_cost_microusd: 50_000,
            },
            maximum_permit_lifetime_seconds: 600,
        };
        let proposal_hash = model_proposal_hash(&proposal).unwrap();
        let permit_id = PermitId::new();
        let handle_id = SecretHandleId::new();
        let permit = ModelInvocationPermit {
            id: permit_id,
            invocation_id: proposal.invocation_id,
            attempt_id: proposal.attempt_id,
            attempt_revision: proposal.attempt_revision,
            lease_generation: proposal.lease_generation,
            runtime_actor: proposal.runtime_actor,
            runtime_identity: proposal.runtime_identity.clone(),
            role: proposal.role,
            task_contract: proposal.task_contract.clone(),
            proposal_hash,
            request_hash,
            resolved_provider: model_provider_source_ref(&connection).unwrap(),
            resolved_model: model_descriptor_source_ref(&connection.id, &model).unwrap(),
            destination: model_destination_source_ref(&connection.base_url).unwrap(),
            destination_class: proposal.route.destination_class,
            allowed_data_classification: proposal.data_classification,
            disclosure_hash: model_disclosure_hash(&proposal.disclosure).unwrap(),
            limits: proposal.requested_limits,
            policy: model_egress_policy_binding(&policy).unwrap(),
            expires_at: proposal.deadline,
            secret_handles: vec![handle_id],
            issued_at: now(),
        };
        let binding = ModelSecretHandleBinding {
            handle_id,
            permit_id,
            connection_id: connection.id.clone(),
            slot: ModelSecretSlot::Primary,
            expires_at: proposal.deadline,
        };
        let receipt = store
            .authorize_model_egress(
                &proposal,
                &policy,
                &permit,
                std::slice::from_ref(&binding),
                now(),
            )
            .unwrap();
        assert_eq!(
            store
                .authorize_model_egress(
                    &proposal,
                    &policy,
                    &permit,
                    std::slice::from_ref(&binding),
                    now(),
                )
                .unwrap(),
            receipt
        );
        assert_eq!(
            store
                .resolve_model_secret_handle_binding(permit_id, handle_id, now())
                .unwrap(),
            binding
        );
        assert!(
            store
                .resolve_model_secret_handle_binding(permit_id, handle_id, proposal.deadline)
                .is_err()
        );
        let encoded = serde_json::to_string(&receipt).unwrap();
        assert!(!encoded.contains("api-key"));

        let runtime_build = runtime_identity.build_hash.clone();
        let manifest = InvocationManifest {
            id: proposal.invocation_id,
            role: InvocationRole::Work,
            attempt_id: attempt.attempt_id,
            task_contract: attempt.task_contract.clone(),
            runtime_identity,
            cognitive_configuration: hash("cognitive-configuration"),
            role_manifest: hash("work-role"),
            prompt_bundle: hash("prompt-bundle"),
            context_manifest: proposal.disclosure.context_manifest.clone(),
            model_route: model_egress_route_payload(&proposal.route).unwrap(),
            resolved_provider: permit.resolved_provider.clone(),
            resolved_model: permit.resolved_model.clone(),
            observed_model_fingerprint: None,
            sampling: payload("test.egress_sampling.v1", "bounded"),
            reasoning: None,
            available_tool_manifests: Vec::new(),
            selected_skill_manifests: Vec::new(),
            authority_envelope: source("root-authority"),
            egress_permit: Some(receipt.permit_ref.clone()),
            repository_revision: Some(hash("repository-revision")),
            environment: source("isolated-environment"),
            runtime_build,
            request_hash: proposal.request_hash.clone(),
            response_hash: None,
        };
        store
            .persist_invocation_manifest(&manifest, now() + Duration::seconds(1))
            .unwrap();
        let mut substituted_manifest = manifest;
        substituted_manifest.resolved_model = source("substituted-model");
        assert!(
            store
                .persist_invocation_manifest(&substituted_manifest, now() + Duration::seconds(1))
                .is_err()
        );

        let mut substituted = proposal.clone();
        substituted.route.model_id = "other-model".into();
        assert!(
            store
                .authorize_model_egress(
                    &substituted,
                    &policy,
                    &permit,
                    std::slice::from_ref(&binding),
                    now(),
                )
                .is_err()
        );

        let mut wrong_role_proposal = proposal.clone();
        wrong_role_proposal.invocation_id = InvocationId::new();
        wrong_role_proposal.role = InvocationRole::Verify;
        let mut wrong_role_permit = permit.clone();
        wrong_role_permit.id = PermitId::new();
        wrong_role_permit.invocation_id = wrong_role_proposal.invocation_id;
        wrong_role_permit.role = InvocationRole::Verify;
        wrong_role_permit.proposal_hash = model_proposal_hash(&wrong_role_proposal).unwrap();
        let wrong_role_handle = SecretHandleId::new();
        wrong_role_permit.secret_handles = vec![wrong_role_handle];
        let wrong_role_binding = ModelSecretHandleBinding {
            handle_id: wrong_role_handle,
            permit_id: wrong_role_permit.id,
            connection_id: connection.id.clone(),
            slot: ModelSecretSlot::Primary,
            expires_at: wrong_role_permit.expires_at,
        };
        assert!(
            store
                .authorize_model_egress(
                    &wrong_role_proposal,
                    &policy,
                    &wrong_role_permit,
                    &[wrong_role_binding],
                    now(),
                )
                .is_err(),
            "a Verify permit must not be issued for a Running workflow"
        );

        store
            .heartbeat_workflow_attempt(
                attempt.attempt_id,
                attempt.revision,
                attempt.lease_generation,
                runtime_actor,
                now() + Duration::minutes(11),
                CorrelationId::new(),
                "egress-workflow-heartbeat-1",
                now() + Duration::seconds(2),
            )
            .unwrap();
        assert!(
            store
                .model_egress_authorization(receipt.proposal.invocation_id)
                .is_err(),
            "a workflow revision change must invalidate the old permit"
        );
    }
}
