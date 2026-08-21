//! Signed portable-organization sharing and owner-selected publisher trust.

use std::collections::{BTreeMap, BTreeSet};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use thiserror::Error;
use url::Url;

use crate::PortableOrganizationGenome;
use mcg_domain::{OrganizationId, OrganizationImportId, OrganizationShareReviewId};

pub const ORGANIZATION_SHARE_SCHEMA_VERSION: u16 = 1;
pub const ORGANIZATION_PUBLISHER_SCHEMA_VERSION: u16 = 1;
pub const ORGANIZATION_SHARE_SIGNATURE_SCHEME: &str = "ed25519-v1";

const SIGNATURE_DOMAIN: &[u8] = b"MCG-ORGANIZATION-SHARE\0\x01";
const MAX_PUBLISHERS: usize = 128;
const MAX_REGISTRIES_PER_PUBLISHER: usize = 32;
const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_DISPLAY_NAME_CHARS: usize = 100;
const MAX_REASON_CHARS: usize = 2_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationPublisherState {
    Trusted,
    Revoked,
}

/// One durable owner-selected publisher identity. Trust is scoped to exact
/// canonical HTTPS registry origins and can be revoked without deleting audit
/// history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustedOrganizationPublisher {
    pub schema_version: u16,
    pub id: String,
    pub display_name: String,
    pub public_key_base64: String,
    pub registries: Vec<String>,
    pub state: OrganizationPublisherState,
    pub revision: u64,
    pub reason: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TrustedOrganizationPublisher {
    /// Validates the bounded public trust record without conferring trust.
    ///
    /// # Errors
    /// Rejects malformed identifiers, keys, registry scopes, revisions, or text.
    pub fn validate(&self) -> Result<(), OrganizationShareError> {
        if self.schema_version != ORGANIZATION_PUBLISHER_SCHEMA_VERSION
            || !valid_identifier(&self.id)
            || self.display_name.trim() != self.display_name
            || self.display_name.is_empty()
            || self.display_name.chars().count() > MAX_DISPLAY_NAME_CHARS
            || self.revision == 0
            || self.reason.trim() != self.reason
            || self.reason.is_empty()
            || self.reason.chars().count() > MAX_REASON_CHARS
            || self.updated_at < self.created_at
            || self.registries.is_empty()
            || self.registries.len() > MAX_REGISTRIES_PER_PUBLISHER
        {
            return Err(OrganizationShareError::InvalidPublisher(
                "publisher metadata is outside the bounded contract".into(),
            ));
        }
        decode_public_key(&self.public_key_base64)?;
        let mut registries = BTreeSet::new();
        for registry in &self.registries {
            let canonical = canonical_registry(registry)?;
            if canonical != *registry || !registries.insert(canonical) {
                return Err(OrganizationShareError::InvalidPublisher(
                    "publisher registry scopes must be canonical and unique".into(),
                ));
            }
        }
        Ok(())
    }

    /// Stable SHA-256 fingerprint of the exact 32-byte Ed25519 public key.
    ///
    /// # Errors
    /// Returns a publisher validation error for a malformed key.
    pub fn public_key_sha256(&self) -> Result<String, OrganizationShareError> {
        Ok(format!(
            "{:x}",
            Sha256::digest(decode_canonical_base64(&self.public_key_base64, 32)?)
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrganizationShareSignature {
    pub scheme: String,
    pub signature_base64: String,
}

/// Marketplace/share envelope. The signed identity covers registry, package,
/// publisher, and the complete portable genome including source provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrganizationSharePackage {
    pub schema_version: u16,
    pub registry: String,
    pub package: String,
    pub publisher: String,
    pub portable: PortableOrganizationGenome,
    pub signature: OrganizationShareSignature,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifiedOrganizationShareEvidence {
    pub schema_version: u16,
    pub registry: String,
    pub package: String,
    pub publisher: String,
    pub publisher_key_sha256: String,
    pub portable_sha256: String,
    pub package_sha256: String,
    pub signature_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationShareReviewState {
    Pending,
    Imported,
    Rejected,
}

/// Durable owner-review record. Verification is evidence, not import authority;
/// only an explicit pending-to-imported transition may materialize the genome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrganizationShareReview {
    pub schema_version: u16,
    pub id: OrganizationShareReviewId,
    pub package: OrganizationSharePackage,
    pub evidence: VerifiedOrganizationShareEvidence,
    pub state: OrganizationShareReviewState,
    pub import_id: Option<OrganizationImportId>,
    pub organization_id: Option<OrganizationId>,
    pub decision_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl OrganizationShareReview {
    /// Creates an inspectable pending review from a cryptographically verified
    /// package. It has no execution or organization-import authority.
    #[must_use]
    pub fn pending(
        id: OrganizationShareReviewId,
        verified: &VerifiedOrganizationShare,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            schema_version: ORGANIZATION_SHARE_SCHEMA_VERSION,
            id,
            package: verified.package.clone(),
            evidence: verified.evidence.clone(),
            state: OrganizationShareReviewState::Pending,
            import_id: None,
            organization_id: None,
            decision_reason: None,
            created_at,
            updated_at: created_at,
        }
    }

    /// Validates structural, evidence-binding, and state invariants. Current
    /// publisher trust must still be checked separately at import time.
    ///
    /// # Errors
    /// Rejects changed packages, malformed evidence, or illegal state fields.
    pub fn validate(&self) -> Result<(), OrganizationShareError> {
        validate_package(&self.package)?;
        if self.schema_version != ORGANIZATION_SHARE_SCHEMA_VERSION
            || self.evidence.schema_version != ORGANIZATION_SHARE_SCHEMA_VERSION
            || self.evidence.registry != self.package.registry
            || self.evidence.package != self.package.package
            || self.evidence.publisher != self.package.publisher
            || self.evidence.portable_sha256 != self.package.portable.content_sha256
            || !valid_sha256(&self.evidence.publisher_key_sha256)
            || !valid_sha256(&self.evidence.package_sha256)
            || !valid_sha256(&self.evidence.signature_sha256)
            || self.updated_at < self.created_at
        {
            return Err(OrganizationShareError::InvalidPackage(
                "review evidence or timestamps are invalid".into(),
            ));
        }
        let package_bytes = serde_json::to_vec(&self.package).map_err(|error| {
            OrganizationShareError::InvalidPackage(format!("package encoding failed: {error}"))
        })?;
        let signature_bytes = decode_canonical_base64(&self.package.signature.signature_base64, 64)
            .map_err(|_| {
                OrganizationShareError::InvalidPackage("signature bytes are invalid".into())
            })?;
        if self.evidence.package_sha256 != format!("{:x}", Sha256::digest(package_bytes))
            || self.evidence.signature_sha256 != format!("{:x}", Sha256::digest(signature_bytes))
        {
            return Err(OrganizationShareError::InvalidPackage(
                "review evidence no longer matches the package".into(),
            ));
        }
        let valid_state = match self.state {
            OrganizationShareReviewState::Pending => {
                self.import_id.is_none()
                    && self.organization_id.is_none()
                    && self.decision_reason.is_none()
                    && self.updated_at == self.created_at
            }
            OrganizationShareReviewState::Imported => {
                self.import_id.is_some()
                    && self.organization_id.is_some()
                    && self.decision_reason.as_deref().is_some_and(valid_reason)
            }
            OrganizationShareReviewState::Rejected => {
                self.import_id.is_none()
                    && self.organization_id.is_none()
                    && self.decision_reason.as_deref().is_some_and(valid_reason)
            }
        };
        if !valid_state {
            return Err(OrganizationShareError::InvalidPackage(
                "review state fields are inconsistent".into(),
            ));
        }
        Ok(())
    }

    /// Creates the only legal imported successor.
    ///
    /// # Errors
    /// Rejects non-pending reviews or invalid decision metadata.
    pub fn imported(
        &self,
        import_id: OrganizationImportId,
        organization_id: OrganizationId,
        reason: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<Self, OrganizationShareError> {
        if self.state != OrganizationShareReviewState::Pending
            || !valid_reason(reason)
            || updated_at < self.updated_at
        {
            return Err(OrganizationShareError::InvalidPackage(
                "review cannot advance to imported".into(),
            ));
        }
        let mut next = self.clone();
        next.state = OrganizationShareReviewState::Imported;
        next.import_id = Some(import_id);
        next.organization_id = Some(organization_id);
        next.decision_reason = Some(reason.into());
        next.updated_at = updated_at;
        next.validate()?;
        Ok(next)
    }

    /// Creates the only legal rejected successor.
    ///
    /// # Errors
    /// Rejects non-pending reviews or invalid decision metadata.
    pub fn rejected(
        &self,
        reason: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<Self, OrganizationShareError> {
        if self.state != OrganizationShareReviewState::Pending
            || !valid_reason(reason)
            || updated_at < self.updated_at
        {
            return Err(OrganizationShareError::InvalidPackage(
                "review cannot advance to rejected".into(),
            ));
        }
        let mut next = self.clone();
        next.state = OrganizationShareReviewState::Rejected;
        next.decision_reason = Some(reason.into());
        next.updated_at = updated_at;
        next.validate()?;
        Ok(next)
    }
}

/// A package that has crossed the local trust and cryptographic boundary.
#[derive(Debug, Clone, PartialEq)]
pub struct VerifiedOrganizationShare {
    package: OrganizationSharePackage,
    evidence: VerifiedOrganizationShareEvidence,
}

impl VerifiedOrganizationShare {
    #[must_use]
    pub const fn package(&self) -> &OrganizationSharePackage {
        &self.package
    }

    #[must_use]
    pub const fn evidence(&self) -> &VerifiedOrganizationShareEvidence {
        &self.evidence
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum OrganizationShareError {
    #[error("organization publisher record is invalid: {0}")]
    InvalidPublisher(String),
    #[error("organization share package is invalid: {0}")]
    InvalidPackage(String),
    #[error("organization publisher is not trusted: {0}")]
    UntrustedPublisher(String),
    #[error("organization publisher is revoked: {0}")]
    RevokedPublisher(String),
    #[error("organization publisher is not trusted for registry: {0}")]
    RegistryNotTrusted(String),
    #[error("organization share signature verification failed")]
    VerificationFailed,
}

#[derive(Clone)]
struct PublisherEntry {
    publisher: TrustedOrganizationPublisher,
    key: VerifyingKey,
    registries: BTreeSet<String>,
}

/// Immutable verification view constructed from durable owner trust records.
#[derive(Clone, Default)]
pub struct OrganizationShareTrustStore {
    publishers: BTreeMap<String, PublisherEntry>,
}

impl std::fmt::Debug for OrganizationShareTrustStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OrganizationShareTrustStore")
            .field("publisher_count", &self.publishers.len())
            .finish()
    }
}

impl OrganizationShareTrustStore {
    /// Builds a verification view from bounded durable publisher records.
    ///
    /// # Errors
    /// Rejects duplicate identities or keys and malformed authority records.
    pub fn new(
        publishers: impl IntoIterator<Item = TrustedOrganizationPublisher>,
    ) -> Result<Self, OrganizationShareError> {
        let mut entries = BTreeMap::new();
        let mut keys = BTreeSet::new();
        for publisher in publishers {
            if entries.len() >= MAX_PUBLISHERS {
                return Err(OrganizationShareError::InvalidPublisher(
                    "publisher limit was exceeded".into(),
                ));
            }
            publisher.validate()?;
            let key_bytes = decode_canonical_base64(&publisher.public_key_base64, 32)?;
            let key_array: [u8; 32] = key_bytes.try_into().map_err(|_| {
                OrganizationShareError::InvalidPublisher(
                    "publisher public key length is invalid".into(),
                )
            })?;
            if !keys.insert(key_array) {
                return Err(OrganizationShareError::InvalidPublisher(
                    "publisher public keys must be unique".into(),
                ));
            }
            let key = VerifyingKey::from_bytes(&key_array).map_err(|_| {
                OrganizationShareError::InvalidPublisher("publisher public key is invalid".into())
            })?;
            let registries = publisher.registries.iter().cloned().collect();
            let id = publisher.id.clone();
            if entries
                .insert(
                    id,
                    PublisherEntry {
                        publisher,
                        key,
                        registries,
                    },
                )
                .is_some()
            {
                return Err(OrganizationShareError::InvalidPublisher(
                    "publisher identities must be unique".into(),
                ));
            }
        }
        Ok(Self {
            publishers: entries,
        })
    }

    #[must_use]
    pub fn publisher_count(&self) -> usize {
        self.publishers.len()
    }

    /// Verifies the complete package against current owner trust. Verification
    /// does not import, activate, or grant authority to the organization.
    ///
    /// # Errors
    /// Rejects malformed, untrusted, revoked, out-of-scope, or tampered packages.
    pub fn verify(
        &self,
        package: &OrganizationSharePackage,
    ) -> Result<VerifiedOrganizationShare, OrganizationShareError> {
        validate_package(package)?;
        let entry = self
            .publishers
            .get(&package.publisher)
            .ok_or_else(|| OrganizationShareError::UntrustedPublisher(package.publisher.clone()))?;
        if entry.publisher.state == OrganizationPublisherState::Revoked {
            return Err(OrganizationShareError::RevokedPublisher(
                package.publisher.clone(),
            ));
        }
        if !entry.registries.contains(&package.registry) {
            return Err(OrganizationShareError::RegistryNotTrusted(
                package.registry.clone(),
            ));
        }
        let signature_bytes = decode_canonical_base64(&package.signature.signature_base64, 64)
            .map_err(|_| {
                OrganizationShareError::InvalidPackage("signature bytes are invalid".into())
            })?;
        let signature = Signature::from_slice(&signature_bytes).map_err(|_| {
            OrganizationShareError::InvalidPackage("signature length is invalid".into())
        })?;
        entry
            .key
            .verify_strict(&organization_share_signing_payload(package)?, &signature)
            .map_err(|_| OrganizationShareError::VerificationFailed)?;
        let package_bytes = serde_json::to_vec(package).map_err(|error| {
            OrganizationShareError::InvalidPackage(format!("package encoding failed: {error}"))
        })?;
        let evidence = VerifiedOrganizationShareEvidence {
            schema_version: ORGANIZATION_SHARE_SCHEMA_VERSION,
            registry: package.registry.clone(),
            package: package.package.clone(),
            publisher: package.publisher.clone(),
            publisher_key_sha256: entry.publisher.public_key_sha256()?,
            portable_sha256: package.portable.content_sha256.clone(),
            package_sha256: format!("{:x}", Sha256::digest(package_bytes)),
            signature_sha256: format!("{:x}", Sha256::digest(signature_bytes)),
        };
        Ok(VerifiedOrganizationShare {
            package: package.clone(),
            evidence,
        })
    }
}

/// Produces canonical domain-separated bytes for publisher signing.
///
/// # Errors
/// Rejects malformed package identity or portable content. The signature field
/// itself is intentionally excluded.
pub fn organization_share_signing_payload(
    package: &OrganizationSharePackage,
) -> Result<Vec<u8>, OrganizationShareError> {
    validate_package_identity(package)?;
    let portable = serde_json::to_vec(&package.portable).map_err(|error| {
        OrganizationShareError::InvalidPackage(format!("portable encoding failed: {error}"))
    })?;
    let mut payload = Vec::with_capacity(portable.len() + 512);
    payload.extend_from_slice(SIGNATURE_DOMAIN);
    push_u16(&mut payload, package.schema_version);
    push_text(&mut payload, &package.registry)?;
    push_text(&mut payload, &package.package)?;
    push_text(&mut payload, &package.publisher)?;
    push_text(&mut payload, &package.portable.content_sha256)?;
    payload.extend_from_slice(&Sha256::digest(portable));
    Ok(payload)
}

fn validate_package(package: &OrganizationSharePackage) -> Result<(), OrganizationShareError> {
    validate_package_identity(package)?;
    if package.signature.scheme != ORGANIZATION_SHARE_SIGNATURE_SCHEME {
        return Err(OrganizationShareError::InvalidPackage(
            "signature scheme is invalid".into(),
        ));
    }
    let _ = decode_canonical_base64(&package.signature.signature_base64, 64).map_err(|_| {
        OrganizationShareError::InvalidPackage("signature bytes are invalid".into())
    })?;
    Ok(())
}

fn validate_package_identity(
    package: &OrganizationSharePackage,
) -> Result<(), OrganizationShareError> {
    if package.schema_version != ORGANIZATION_SHARE_SCHEMA_VERSION
        || !valid_identifier(&package.package)
        || !valid_identifier(&package.publisher)
        || canonical_registry(&package.registry)? != package.registry
    {
        return Err(OrganizationShareError::InvalidPackage(
            "package identity is invalid".into(),
        ));
    }
    package.portable.validate().map_err(|error| {
        OrganizationShareError::InvalidPackage(format!("portable definition is invalid: {error}"))
    })
}

fn canonical_registry(value: &str) -> Result<String, OrganizationShareError> {
    let url = Url::parse(value)
        .map_err(|_| OrganizationShareError::InvalidPackage("registry URL is invalid".into()))?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(OrganizationShareError::InvalidPackage(
            "registry URL must be a credential-free canonical HTTPS URL".into(),
        ));
    }
    Ok(url.to_string())
}

fn decode_public_key(value: &str) -> Result<VerifyingKey, OrganizationShareError> {
    let bytes = decode_canonical_base64(value, 32)?;
    let array: [u8; 32] = bytes.try_into().map_err(|_| {
        OrganizationShareError::InvalidPublisher("publisher public key length is invalid".into())
    })?;
    VerifyingKey::from_bytes(&array).map_err(|_| {
        OrganizationShareError::InvalidPublisher("publisher public key is invalid".into())
    })
}

fn decode_canonical_base64(
    value: &str,
    exact_len: usize,
) -> Result<Vec<u8>, OrganizationShareError> {
    let bytes = STANDARD
        .decode(value)
        .map_err(|_| OrganizationShareError::InvalidPublisher("base64 value is invalid".into()))?;
    if bytes.len() != exact_len || STANDARD.encode(&bytes) != value {
        return Err(OrganizationShareError::InvalidPublisher(
            "base64 value is not canonical or has the wrong length".into(),
        ));
    }
    Ok(bytes)
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTIFIER_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
        })
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn valid_reason(value: &str) -> bool {
    !value.is_empty() && value.trim() == value && value.chars().count() <= MAX_REASON_CHARS
}

fn push_text(payload: &mut Vec<u8>, value: &str) -> Result<(), OrganizationShareError> {
    let length = u32::try_from(value.len())
        .map_err(|_| OrganizationShareError::InvalidPackage("signed field is too large".into()))?;
    payload.extend_from_slice(&length.to_be_bytes());
    payload.extend_from_slice(value.as_bytes());
    Ok(())
}

fn push_u16(payload: &mut Vec<u8>, value: u16) {
    payload.extend_from_slice(&value.to_be_bytes());
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer as _, SigningKey};

    use super::*;
    use crate::OrganizationGenome;

    fn publisher(
        key: &SigningKey,
        state: OrganizationPublisherState,
    ) -> TrustedOrganizationPublisher {
        let now = Utc::now();
        TrustedOrganizationPublisher {
            schema_version: ORGANIZATION_PUBLISHER_SCHEMA_VERSION,
            id: "publisher.example".into(),
            display_name: "Example Publisher".into(),
            public_key_base64: STANDARD.encode(key.verifying_key().to_bytes()),
            registries: vec!["https://registry.example/".into()],
            state,
            revision: 1,
            reason: "Owner reviewed publisher identity.".into(),
            created_at: now,
            updated_at: now,
        }
    }

    fn package(key: &SigningKey) -> OrganizationSharePackage {
        let portable = PortableOrganizationGenome::export(
            &OrganizationGenome::consumer_default(Utc::now()),
            Utc::now(),
        )
        .expect("portable organization");
        let mut package = OrganizationSharePackage {
            schema_version: ORGANIZATION_SHARE_SCHEMA_VERSION,
            registry: "https://registry.example/".into(),
            package: "consumer-default".into(),
            publisher: "publisher.example".into(),
            portable,
            signature: OrganizationShareSignature {
                scheme: ORGANIZATION_SHARE_SIGNATURE_SCHEME.into(),
                signature_base64: STANDARD.encode([0_u8; 64]),
            },
        };
        let payload = organization_share_signing_payload(&package).expect("signing payload");
        package.signature.signature_base64 = STANDARD.encode(key.sign(&payload).to_bytes());
        package
    }

    #[test]
    fn trusted_package_verifies_with_content_free_evidence() {
        let key = SigningKey::from_bytes(&[31; 32]);
        let store = OrganizationShareTrustStore::new([publisher(
            &key,
            OrganizationPublisherState::Trusted,
        )])
        .expect("trust store");
        let verified = store.verify(&package(&key)).expect("verified package");
        assert_eq!(verified.evidence().publisher, "publisher.example");
        assert_eq!(verified.evidence().package_sha256.len(), 64);
        assert_eq!(verified.evidence().signature_sha256.len(), 64);
        assert_eq!(verified.evidence().portable_sha256.len(), 64);
        let review = OrganizationShareReview::pending(
            OrganizationShareReviewId::new(),
            &verified,
            Utc::now(),
        );
        review.validate().expect("pending review");
        let imported = review
            .imported(
                OrganizationImportId::new(),
                OrganizationId::new(),
                "Owner reviewed the organization definition.",
                Utc::now(),
            )
            .expect("imported review");
        imported.validate().expect("imported review validates");
    }

    #[test]
    fn tampering_and_wrong_registry_fail_closed() {
        let key = SigningKey::from_bytes(&[37; 32]);
        let store = OrganizationShareTrustStore::new([publisher(
            &key,
            OrganizationPublisherState::Trusted,
        )])
        .expect("trust store");
        let mut tampered = package(&key);
        tampered.portable.definition.name = "Tampered".into();
        assert!(matches!(
            store.verify(&tampered),
            Err(OrganizationShareError::InvalidPackage(_))
        ));
        let mut wrong_registry = package(&key);
        wrong_registry.registry = "https://other.example/".into();
        assert_eq!(
            store.verify(&wrong_registry),
            Err(OrganizationShareError::RegistryNotTrusted(
                "https://other.example/".into()
            ))
        );
    }

    #[test]
    fn revoked_or_untrusted_publishers_never_verify() {
        let key = SigningKey::from_bytes(&[41; 32]);
        let package = package(&key);
        let revoked = OrganizationShareTrustStore::new([publisher(
            &key,
            OrganizationPublisherState::Revoked,
        )])
        .expect("revoked store");
        assert_eq!(
            revoked.verify(&package),
            Err(OrganizationShareError::RevokedPublisher(
                "publisher.example".into()
            ))
        );
        assert_eq!(
            OrganizationShareTrustStore::default().verify(&package),
            Err(OrganizationShareError::UntrustedPublisher(
                "publisher.example".into()
            ))
        );
    }

    #[test]
    fn duplicate_keys_and_noncanonical_registries_are_rejected() {
        let key = SigningKey::from_bytes(&[43; 32]);
        let first = publisher(&key, OrganizationPublisherState::Trusted);
        let mut second = first.clone();
        second.id = "publisher.other".into();
        assert!(matches!(
            OrganizationShareTrustStore::new([first.clone(), second]),
            Err(OrganizationShareError::InvalidPublisher(_))
        ));
        let mut malformed = first;
        malformed.registries = vec!["https://registry.example".into()];
        assert!(matches!(
            malformed.validate(),
            Err(OrganizationShareError::InvalidPublisher(_)
                | OrganizationShareError::InvalidPackage(_))
        ));
    }
}
