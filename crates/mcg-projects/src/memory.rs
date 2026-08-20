use std::fmt::Write as _;

use chrono::{DateTime, Utc};
use mcg_domain::{MemoryClaimId, MemoryTransferId, ProjectId};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ClaimProvenance, MEMORY_CLAIM_SCHEMA_VERSION, MEMORY_TRANSFER_SCHEMA_VERSION, MemoryClaim,
    MemoryState, ProjectMemoryTransfer, ProvenanceKind,
};

const MAX_TRANSFER_CLAIM_BYTES: usize = 64 * 1024;
const MAX_TRANSFER_REASON_BYTES: usize = 512;

#[derive(Debug, Error, PartialEq)]
pub enum MemoryError {
    #[error("memory namespace is reserved for secrets")]
    SecretNamespace,
    #[error("memory confidence must be finite and between zero and one")]
    InvalidConfidence,
    #[error("memory key cannot be empty")]
    EmptyKey,
    #[error("cross-project memory transfer is invalid: {0}")]
    InvalidTransfer(String),
    #[error("claim {0} cannot transition from {1:?} to {2:?}")]
    InvalidTransition(MemoryClaimId, MemoryState, MemoryState),
}

/// Returns the lowercase SHA-256 of the exact versioned claim JSON reviewed for transfer.
///
/// # Errors
///
/// Returns an invalid-transfer error if the claim cannot be encoded.
pub fn memory_claim_sha256(claim: &MemoryClaim) -> Result<String, MemoryError> {
    let bytes = serde_json::to_vec(claim)
        .map_err(|error| MemoryError::InvalidTransfer(format!("claim encoding failed: {error}")))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

/// Returns the digest of a claim only when it is currently eligible for an
/// explicit cross-project transfer review.
///
/// # Errors
///
/// Rejects stale, non-durable, or oversized source claims.
pub fn transferable_memory_claim_sha256(claim: &MemoryClaim) -> Result<String, MemoryError> {
    transferable_memory_claim_sha256_at(claim, Utc::now())
}

fn transferable_memory_claim_sha256_at(
    claim: &MemoryClaim,
    now: DateTime<Utc>,
) -> Result<String, MemoryError> {
    if claim.state != MemoryState::Durable || !claim.is_fresh_at(now) {
        return Err(MemoryError::InvalidTransfer(
            "source claim must be fresh and durable".into(),
        ));
    }
    let source_bytes = serde_json::to_vec(claim)
        .map_err(|error| MemoryError::InvalidTransfer(format!("claim encoding failed: {error}")))?;
    if source_bytes.len() > MAX_TRANSFER_CLAIM_BYTES {
        return Err(MemoryError::InvalidTransfer(
            "source claim exceeds the 64 KiB transfer limit".into(),
        ));
    }
    Ok(format!("{:x}", Sha256::digest(source_bytes)))
}

/// Prepares one explicit, digest-bound cross-project memory transfer.
///
/// The source must be a fresh durable claim. The copied claim starts as `new`,
/// preserves confidence and freshness, and carries only transfer provenance.
/// No ambient cross-project lookup or automatic learning is granted.
///
/// # Errors
///
/// Rejects same-project, stale, non-durable, oversized, changed-digest, or
/// insufficiently reviewed transfers.
#[allow(clippy::too_many_arguments)]
pub fn prepare_project_memory_transfer(
    transfer_id: MemoryTransferId,
    source: &MemoryClaim,
    expected_source_sha256: &str,
    target_project_id: ProjectId,
    reason: &str,
) -> Result<(ProjectMemoryTransfer, MemoryClaim), MemoryError> {
    prepare_project_memory_transfer_at(
        transfer_id,
        source,
        expected_source_sha256,
        target_project_id,
        reason,
        Utc::now(),
    )
}

#[allow(clippy::too_many_arguments)]
fn prepare_project_memory_transfer_at(
    transfer_id: MemoryTransferId,
    source: &MemoryClaim,
    expected_source_sha256: &str,
    target_project_id: ProjectId,
    reason: &str,
    now: DateTime<Utc>,
) -> Result<(ProjectMemoryTransfer, MemoryClaim), MemoryError> {
    if source.project_id == target_project_id {
        return Err(MemoryError::InvalidTransfer(
            "source and target projects must differ".into(),
        ));
    }
    let reason = reason.trim();
    if reason.is_empty()
        || reason.len() > MAX_TRANSFER_REASON_BYTES
        || reason.chars().any(char::is_control)
    {
        return Err(MemoryError::InvalidTransfer(
            "owner review reason must contain 1 to 512 printable bytes".into(),
        ));
    }
    let actual_sha256 = transferable_memory_claim_sha256_at(source, now)?;
    if expected_source_sha256.len() != 64
        || !expected_source_sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || expected_source_sha256 != actual_sha256
    {
        return Err(MemoryError::InvalidTransfer(
            "source claim digest is malformed or no longer matches".into(),
        ));
    }
    let target_claim = MemoryLifecycle::create(
        target_project_id,
        source.namespace.clone(),
        source.key.clone(),
        source.value.clone(),
        source.confidence,
        ClaimProvenance {
            source_kind: ProvenanceKind::Imported,
            source: format!("cross-project-transfer:{transfer_id}"),
            task_id: None,
            artifact_sha256: Some(actual_sha256.clone()),
        },
        source.fresh_until,
    )?;
    let transfer = ProjectMemoryTransfer {
        schema_version: MEMORY_TRANSFER_SCHEMA_VERSION,
        id: transfer_id,
        source_project_id: source.project_id,
        source_claim_id: source.id,
        source_claim_sha256: actual_sha256,
        target_project_id,
        target_claim_id: target_claim.id,
        reason: reason.to_owned(),
        created_at: now,
    };
    Ok((transfer, target_claim))
}

pub struct MemoryLifecycle;

impl MemoryLifecycle {
    /// Creates a provenance-bearing claim after validating its scope and confidence.
    ///
    /// # Errors
    ///
    /// Secret-like namespaces, empty keys, and invalid confidence fail closed.
    pub fn create(
        project_id: ProjectId,
        namespace: impl Into<String>,
        key: impl Into<String>,
        value: serde_json::Value,
        confidence: f64,
        provenance: ClaimProvenance,
        fresh_until: Option<DateTime<Utc>>,
    ) -> Result<MemoryClaim, MemoryError> {
        let namespace = namespace.into();
        let key = key.into();
        validate(&namespace, &key, confidence)?;
        Ok(MemoryClaim {
            schema_version: MEMORY_CLAIM_SCHEMA_VERSION,
            id: MemoryClaimId::new(),
            project_id,
            namespace,
            key,
            value,
            state: MemoryState::New,
            confidence,
            provenance,
            observed_at: Utc::now(),
            fresh_until,
            supersedes: None,
            correction_count: 0,
            usefulness_count: 0,
        })
    }

    /// Reinforces useful memory and promotes recurring claims to durable state.
    ///
    /// # Errors
    ///
    /// Retracted or superseded claims cannot be reinforced.
    pub fn reinforce(claim: &mut MemoryClaim) -> Result<(), MemoryError> {
        if matches!(
            claim.state,
            MemoryState::Retracted | MemoryState::Superseded
        ) {
            return Err(MemoryError::InvalidTransition(
                claim.id,
                claim.state,
                MemoryState::Durable,
            ));
        }
        claim.usefulness_count = claim.usefulness_count.saturating_add(1);
        claim.state = if claim.usefulness_count >= 2 || claim.correction_count >= 2 {
            MemoryState::Durable
        } else {
            MemoryState::Working
        };
        Ok(())
    }

    /// Produces a new corrected version and marks the old version superseded.
    ///
    /// # Errors
    ///
    /// The replacement is subject to the same validation as a new claim.
    pub fn correct(
        previous: &mut MemoryClaim,
        value: serde_json::Value,
        confidence: f64,
        provenance: ClaimProvenance,
        fresh_until: Option<DateTime<Utc>>,
    ) -> Result<MemoryClaim, MemoryError> {
        if matches!(
            previous.state,
            MemoryState::Retracted | MemoryState::Superseded
        ) {
            return Err(MemoryError::InvalidTransition(
                previous.id,
                previous.state,
                MemoryState::Superseded,
            ));
        }
        validate(&previous.namespace, &previous.key, confidence)?;
        previous.state = MemoryState::Superseded;
        Ok(MemoryClaim {
            schema_version: MEMORY_CLAIM_SCHEMA_VERSION,
            id: MemoryClaimId::new(),
            project_id: previous.project_id,
            namespace: previous.namespace.clone(),
            key: previous.key.clone(),
            value,
            state: if previous.correction_count >= 1 {
                MemoryState::Durable
            } else {
                MemoryState::Working
            },
            confidence,
            provenance,
            observed_at: Utc::now(),
            fresh_until,
            supersedes: Some(previous.id),
            correction_count: previous.correction_count.saturating_add(1),
            usefulness_count: previous.usefulness_count,
        })
    }

    /// Marks unused or stale working memory for decay.
    ///
    /// # Errors
    ///
    /// Durable, superseded, and retracted claims cannot enter decay directly.
    pub fn decay(claim: &mut MemoryClaim) -> Result<(), MemoryError> {
        if !matches!(claim.state, MemoryState::New | MemoryState::Working) {
            return Err(MemoryError::InvalidTransition(
                claim.id,
                claim.state,
                MemoryState::Decaying,
            ));
        }
        claim.state = MemoryState::Decaying;
        Ok(())
    }

    pub fn retract(claim: &mut MemoryClaim) {
        claim.state = MemoryState::Retracted;
    }

    /// Produces a reinforced successor version and supersedes the prior head.
    ///
    /// # Errors
    ///
    /// Terminal claim versions cannot be reinforced.
    pub fn reinforced(
        previous: &mut MemoryClaim,
        provenance: ClaimProvenance,
    ) -> Result<MemoryClaim, MemoryError> {
        if matches!(
            previous.state,
            MemoryState::Retracted | MemoryState::Superseded
        ) {
            return Err(MemoryError::InvalidTransition(
                previous.id,
                previous.state,
                MemoryState::Durable,
            ));
        }
        let usefulness_count = previous.usefulness_count.saturating_add(1);
        let correction_count = previous.correction_count;
        let next_state = if usefulness_count >= 2 || correction_count >= 2 {
            MemoryState::Durable
        } else {
            MemoryState::Working
        };
        Ok(successor(
            previous,
            next_state,
            provenance,
            correction_count,
            usefulness_count,
        ))
    }

    /// Produces a decaying successor version for unused new/working memory.
    ///
    /// # Errors
    ///
    /// Durable and terminal versions cannot decay directly.
    pub fn decayed(
        previous: &mut MemoryClaim,
        provenance: ClaimProvenance,
    ) -> Result<MemoryClaim, MemoryError> {
        if !matches!(previous.state, MemoryState::New | MemoryState::Working) {
            return Err(MemoryError::InvalidTransition(
                previous.id,
                previous.state,
                MemoryState::Decaying,
            ));
        }
        let correction_count = previous.correction_count;
        let usefulness_count = previous.usefulness_count;
        Ok(successor(
            previous,
            MemoryState::Decaying,
            provenance,
            correction_count,
            usefulness_count,
        ))
    }

    /// Produces a decaying successor when an explicit freshness deadline has
    /// elapsed. Unlike an owner-requested decay, expiration also applies to a
    /// durable claim: durability records prior usefulness, not perpetual truth.
    ///
    /// # Errors
    ///
    /// Claims already decaying or in a terminal state cannot expire again.
    pub fn expired(
        previous: &mut MemoryClaim,
        provenance: ClaimProvenance,
    ) -> Result<MemoryClaim, MemoryError> {
        if !matches!(
            previous.state,
            MemoryState::New | MemoryState::Working | MemoryState::Durable
        ) {
            return Err(MemoryError::InvalidTransition(
                previous.id,
                previous.state,
                MemoryState::Decaying,
            ));
        }
        let correction_count = previous.correction_count;
        let usefulness_count = previous.usefulness_count;
        Ok(successor(
            previous,
            MemoryState::Decaying,
            provenance,
            correction_count,
            usefulness_count,
        ))
    }

    /// Produces a retracted terminal successor while retaining prior versions.
    ///
    /// # Errors
    ///
    /// Already terminal versions cannot be retracted again.
    pub fn retracted(
        previous: &mut MemoryClaim,
        provenance: ClaimProvenance,
    ) -> Result<MemoryClaim, MemoryError> {
        if matches!(
            previous.state,
            MemoryState::Retracted | MemoryState::Superseded
        ) {
            return Err(MemoryError::InvalidTransition(
                previous.id,
                previous.state,
                MemoryState::Retracted,
            ));
        }
        let correction_count = previous.correction_count;
        let usefulness_count = previous.usefulness_count;
        Ok(successor(
            previous,
            MemoryState::Retracted,
            provenance,
            correction_count,
            usefulness_count,
        ))
    }
}

fn successor(
    previous: &mut MemoryClaim,
    state: MemoryState,
    provenance: ClaimProvenance,
    correction_count: u32,
    usefulness_count: u32,
) -> MemoryClaim {
    let previous_id = previous.id;
    previous.state = MemoryState::Superseded;
    MemoryClaim {
        schema_version: MEMORY_CLAIM_SCHEMA_VERSION,
        id: MemoryClaimId::new(),
        project_id: previous.project_id,
        namespace: previous.namespace.clone(),
        key: previous.key.clone(),
        value: previous.value.clone(),
        state,
        confidence: previous.confidence,
        provenance,
        observed_at: Utc::now(),
        fresh_until: previous.fresh_until,
        supersedes: Some(previous_id),
        correction_count,
        usefulness_count,
    }
}

#[must_use]
pub fn render_managed_memory(project_id: ProjectId, claims: &[MemoryClaim]) -> String {
    render_managed_memory_at(project_id, claims, Utc::now())
}

#[must_use]
pub fn render_managed_memory_at(
    project_id: ProjectId,
    claims: &[MemoryClaim],
    now: DateTime<Utc>,
) -> String {
    let mut active = claims
        .iter()
        .filter(|claim| {
            claim.project_id == project_id
                && matches!(claim.state, MemoryState::Working | MemoryState::Durable)
                && claim.is_fresh_at(now)
        })
        .collect::<Vec<_>>();
    active.sort_by(|left, right| {
        left.namespace
            .cmp(&right.namespace)
            .then_with(|| left.key.cmp(&right.key))
            .then_with(|| left.observed_at.cmp(&right.observed_at))
    });
    let mut output = format!("# Managed Project Memory\n\nProject: `{project_id}`\n");
    let mut namespace = "";
    for claim in active {
        if namespace != claim.namespace {
            namespace = &claim.namespace;
            let _ = write!(output, "\n## {}\n", claim.namespace);
        }
        let _ = write!(
            output,
            "\n- **{}**: {} _(confidence {:.2}; source: {})_\n",
            claim.key, claim.value, claim.confidence, claim.provenance.source
        );
    }
    output
}

fn validate(namespace: &str, key: &str, confidence: f64) -> Result<(), MemoryError> {
    let namespace = namespace.to_ascii_lowercase();
    let key_lower = key.to_ascii_lowercase();
    let reserved = [
        "secret",
        "secrets",
        "credential",
        "credentials",
        "token",
        "tokens",
        "api_key",
        "private_key",
        "password",
    ];
    if reserved.iter().any(|reserved| {
        namespace == *reserved
            || namespace.starts_with(&format!("{reserved}."))
            || key_lower == *reserved
            || key_lower.ends_with(&format!(".{reserved}"))
            || key_lower.ends_with(&format!("_{reserved}"))
    }) {
        return Err(MemoryError::SecretNamespace);
    }
    if key.trim().is_empty() {
        return Err(MemoryError::EmptyKey);
    }
    if !confidence.is_finite() || !(0.0..=1.0).contains(&confidence) {
        return Err(MemoryError::InvalidConfidence);
    }
    Ok(())
}

#[must_use]
pub fn user_provenance(source: impl Into<String>) -> ClaimProvenance {
    ClaimProvenance {
        source_kind: ProvenanceKind::User,
        source: source.into(),
        task_id: None,
        artifact_sha256: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correction_versions_claim_and_repeated_correction_becomes_durable() {
        let project_id = ProjectId::new();
        let mut first = MemoryLifecycle::create(
            project_id,
            "project",
            "package_manager",
            serde_json::json!("npm"),
            0.7,
            user_provenance("initial setup"),
            None,
        )
        .expect("claim");
        let mut second = MemoryLifecycle::correct(
            &mut first,
            serde_json::json!("pnpm"),
            1.0,
            user_provenance("owner correction"),
            None,
        )
        .expect("correction");
        let third = MemoryLifecycle::correct(
            &mut second,
            serde_json::json!("bun"),
            1.0,
            user_provenance("owner correction"),
            None,
        )
        .expect("correction");
        assert_eq!(first.state, MemoryState::Superseded);
        assert_eq!(second.state, MemoryState::Superseded);
        assert_eq!(third.state, MemoryState::Durable);
        assert_eq!(third.correction_count, 2);
        assert_eq!(third.supersedes, Some(second.id));
    }

    #[test]
    fn secret_namespace_is_never_accepted_into_durable_memory() {
        let result = MemoryLifecycle::create(
            ProjectId::new(),
            "credentials.provider",
            "api_key",
            serde_json::json!("not-a-real-secret"),
            1.0,
            user_provenance("fixture"),
            None,
        );
        assert_eq!(result, Err(MemoryError::SecretNamespace));
        let disguised = MemoryLifecycle::create(
            ProjectId::new(),
            "project",
            "provider_api_key",
            serde_json::json!("not-a-real-secret"),
            1.0,
            user_provenance("fixture"),
            None,
        );
        assert_eq!(disguised, Err(MemoryError::SecretNamespace));
    }

    #[test]
    fn cross_project_transfer_is_explicit_digest_bound_and_provenance_preserving() {
        let now = Utc::now();
        let source_project_id = ProjectId::new();
        let target_project_id = ProjectId::new();
        let mut source = MemoryLifecycle::create(
            source_project_id,
            "project",
            "package_manager",
            serde_json::json!("pnpm"),
            0.9,
            user_provenance("owner-confirmed source"),
            Some(now + chrono::Duration::days(7)),
        )
        .expect("source claim");
        source.state = MemoryState::Durable;
        source.usefulness_count = 2;
        let digest = memory_claim_sha256(&source).expect("claim digest");
        let transfer_id = MemoryTransferId::new();
        let (transfer, target) = prepare_project_memory_transfer_at(
            transfer_id,
            &source,
            &digest,
            target_project_id,
            "The owner reviewed this project convention for reuse",
            now,
        )
        .expect("transfer");

        assert_eq!(transfer.source_project_id, source_project_id);
        assert_eq!(transfer.source_claim_id, source.id);
        assert_eq!(transfer.source_claim_sha256, digest);
        assert_eq!(transfer.target_project_id, target_project_id);
        assert_eq!(transfer.target_claim_id, target.id);
        assert_eq!(target.project_id, target_project_id);
        assert_eq!(target.namespace, source.namespace);
        assert_eq!(target.key, source.key);
        assert_eq!(target.value, source.value);
        assert_eq!(target.confidence.to_bits(), source.confidence.to_bits());
        assert_eq!(target.fresh_until, source.fresh_until);
        assert_eq!(target.state, MemoryState::New);
        assert_eq!(target.provenance.source_kind, ProvenanceKind::Imported);
        assert_eq!(
            target.provenance.source,
            format!("cross-project-transfer:{transfer_id}")
        );
        assert_eq!(
            target.provenance.artifact_sha256.as_deref(),
            Some(transfer.source_claim_sha256.as_str())
        );
    }

    #[test]
    fn cross_project_transfer_rejects_ambient_or_changed_authority() {
        let now = Utc::now();
        let source_project_id = ProjectId::new();
        let target_project_id = ProjectId::new();
        let mut source = MemoryLifecycle::create(
            source_project_id,
            "project",
            "test_command",
            serde_json::json!("cargo test"),
            1.0,
            user_provenance("fixture"),
            Some(now + chrono::Duration::hours(1)),
        )
        .expect("source");
        let digest = memory_claim_sha256(&source).expect("digest");
        for (target, expected_digest, reason) in [
            (target_project_id, digest.as_str(), "reviewed"),
            (source_project_id, digest.as_str(), "reviewed"),
            (target_project_id, "0", "reviewed"),
            (target_project_id, digest.as_str(), ""),
        ] {
            assert!(
                prepare_project_memory_transfer_at(
                    MemoryTransferId::new(),
                    &source,
                    expected_digest,
                    target,
                    reason,
                    now,
                )
                .is_err()
            );
        }
        source.state = MemoryState::Durable;
        source.fresh_until = Some(now - chrono::Duration::seconds(1));
        let digest = memory_claim_sha256(&source).expect("expired digest");
        assert!(
            prepare_project_memory_transfer_at(
                MemoryTransferId::new(),
                &source,
                &digest,
                target_project_id,
                "reviewed",
                now,
            )
            .is_err()
        );
    }

    #[test]
    fn explicit_expiration_can_decay_durable_memory_without_losing_evidence() {
        let project_id = ProjectId::new();
        let mut durable = MemoryLifecycle::create(
            project_id,
            "project",
            "deployment",
            serde_json::json!("staging"),
            0.9,
            user_provenance("owner"),
            None,
        )
        .expect("claim");
        durable.state = MemoryState::Durable;
        durable.usefulness_count = 3;
        let replacement = MemoryLifecycle::expired(
            &mut durable,
            ClaimProvenance {
                source_kind: ProvenanceKind::Derived,
                source: "automatic freshness consolidation".into(),
                task_id: None,
                artifact_sha256: None,
            },
        )
        .expect("expiration");

        assert_eq!(durable.state, MemoryState::Superseded);
        assert_eq!(replacement.state, MemoryState::Decaying);
        assert_eq!(replacement.value, serde_json::json!("staging"));
        assert_eq!(replacement.usefulness_count, 3);
        assert_eq!(replacement.supersedes, Some(durable.id));
    }
}
