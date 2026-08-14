use std::fmt::Write as _;

use chrono::{DateTime, Utc};
use mcg_domain::{MemoryClaimId, ProjectId};
use thiserror::Error;

use crate::{
    ClaimProvenance, MEMORY_CLAIM_SCHEMA_VERSION, MemoryClaim, MemoryState, ProvenanceKind,
};

#[derive(Debug, Error, PartialEq)]
pub enum MemoryError {
    #[error("memory namespace is reserved for secrets")]
    SecretNamespace,
    #[error("memory confidence must be finite and between zero and one")]
    InvalidConfidence,
    #[error("memory key cannot be empty")]
    EmptyKey,
    #[error("claim {0} cannot transition from {1:?} to {2:?}")]
    InvalidTransition(MemoryClaimId, MemoryState, MemoryState),
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
    let mut active = claims
        .iter()
        .filter(|claim| {
            claim.project_id == project_id
                && matches!(claim.state, MemoryState::Working | MemoryState::Durable)
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
}
