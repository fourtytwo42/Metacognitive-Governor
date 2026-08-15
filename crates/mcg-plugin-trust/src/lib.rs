//! Cryptographic trust boundary for third-party plugin packages.

use std::collections::{BTreeMap, BTreeSet};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, VerifyingKey};
use mcg_domain::{PluginInstallSource, PluginRevision, PluginSignature, PluginSignatureState};
use mcg_plugin_protocol::{PluginCapability, PluginClass};
use sha2::{Digest as _, Sha256};
use thiserror::Error;
use url::Url;

pub const PLUGIN_SIGNATURE_SCHEME: &str = "ed25519-v1";
pub const PLUGIN_TRUST_SCHEMA_VERSION: u16 = 1;
const SIGNATURE_DOMAIN: &[u8] = b"MCG-PLUGIN-PACKAGE-SIGNATURE\0\x01";
const MAX_SIGNERS: usize = 128;
const MAX_REGISTRIES_PER_SIGNER: usize = 32;

/// One public signing identity and the exact registries it may authenticate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedPluginSigner {
    pub id: String,
    pub public_key_base64: String,
    pub registries: Vec<String>,
}

/// Detached package proof supplied by a registry package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginSignatureProof {
    pub schema_version: u16,
    pub scheme: String,
    pub signer: String,
    pub signature_base64: String,
}

/// A revision whose registry identity and complete immutable package identity
/// have been authenticated by [`PluginTrustStore`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPluginRevision(PluginRevision);

impl VerifiedPluginRevision {
    #[must_use]
    pub const fn revision(&self) -> &PluginRevision {
        &self.0
    }
}

#[derive(Debug, Error)]
pub enum PluginTrustError {
    #[error("plugin trust configuration is invalid: {0}")]
    InvalidConfiguration(String),
    #[error("plugin signature proof is invalid: {0}")]
    InvalidProof(String),
    #[error("plugin signer is not trusted: {0}")]
    UntrustedSigner(String),
    #[error("plugin signer is not trusted for registry: {0}")]
    RegistryNotTrusted(String),
    #[error("plugin signature verification failed")]
    VerificationFailed,
}

#[derive(Clone)]
struct SignerEntry {
    key: VerifyingKey,
    registries: BTreeSet<String>,
}

/// Immutable collection of owner-selected plugin package trust roots.
#[derive(Clone, Default)]
pub struct PluginTrustStore {
    signers: BTreeMap<String, SignerEntry>,
}

impl std::fmt::Debug for PluginTrustStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PluginTrustStore")
            .field("signer_count", &self.signers.len())
            .finish()
    }
}

impl PluginTrustStore {
    /// Builds a fail-closed trust store from bounded canonical public-key entries.
    ///
    /// # Errors
    /// Rejects duplicate identities, malformed keys, unsafe registry URLs, and
    /// empty or excessive authority sets.
    pub fn new(
        signers: impl IntoIterator<Item = TrustedPluginSigner>,
    ) -> Result<Self, PluginTrustError> {
        let mut entries = BTreeMap::new();
        let mut public_keys = BTreeSet::new();
        for signer in signers {
            if entries.len() >= MAX_SIGNERS || !valid_identifier(&signer.id) {
                return Err(PluginTrustError::InvalidConfiguration(
                    "signer identity is invalid or the signer limit was exceeded".into(),
                ));
            }
            let key_bytes = canonical_base64(&signer.public_key_base64, 32).map_err(|()| {
                PluginTrustError::InvalidConfiguration("signer public key is invalid".into())
            })?;
            let key_array: [u8; 32] = key_bytes.try_into().map_err(|_| {
                PluginTrustError::InvalidConfiguration("signer public key length is invalid".into())
            })?;
            let key = VerifyingKey::from_bytes(&key_array).map_err(|_| {
                PluginTrustError::InvalidConfiguration("signer public key is invalid".into())
            })?;
            if !public_keys.insert(key_array) {
                return Err(PluginTrustError::InvalidConfiguration(
                    "plugin public keys must not be assigned to multiple signer identities".into(),
                ));
            }
            if signer.registries.is_empty() || signer.registries.len() > MAX_REGISTRIES_PER_SIGNER {
                return Err(PluginTrustError::InvalidConfiguration(
                    "signer registry authority is empty or excessive".into(),
                ));
            }
            let mut registries = BTreeSet::new();
            for registry in signer.registries {
                let canonical = canonical_registry(&registry).map_err(|()| {
                    PluginTrustError::InvalidConfiguration("signer registry URL is invalid".into())
                })?;
                if canonical != registry || !registries.insert(canonical) {
                    return Err(PluginTrustError::InvalidConfiguration(
                        "signer registry URLs must be canonical and unique".into(),
                    ));
                }
            }
            if entries
                .insert(signer.id, SignerEntry { key, registries })
                .is_some()
            {
                return Err(PluginTrustError::InvalidConfiguration(
                    "plugin signer identities must be unique".into(),
                ));
            }
        }
        Ok(Self { signers: entries })
    }

    #[must_use]
    pub fn signer_count(&self) -> usize {
        self.signers.len()
    }

    /// Verifies a detached Ed25519 proof and returns a sealed revision that can
    /// cross the durable verified-install boundary.
    ///
    /// # Errors
    /// Rejects unsigned/local or already-claimed revisions, untrusted signers or
    /// registries, malformed proof bytes, and cryptographic mismatch.
    pub fn verify_registry_revision(
        &self,
        revision: &PluginRevision,
        proof: &PluginSignatureProof,
    ) -> Result<VerifiedPluginRevision, PluginTrustError> {
        validate_unverified_revision(revision)?;
        if proof.schema_version != PLUGIN_TRUST_SCHEMA_VERSION
            || proof.scheme != PLUGIN_SIGNATURE_SCHEME
            || !valid_identifier(&proof.signer)
        {
            return Err(PluginTrustError::InvalidProof(
                "signature scheme or signer identity is invalid".into(),
            ));
        }
        let PluginInstallSource::Registry { registry, .. } = &revision.source else {
            return Err(PluginTrustError::InvalidProof(
                "only registry packages may claim verified signatures".into(),
            ));
        };
        let canonical = canonical_registry(registry).map_err(|()| {
            PluginTrustError::InvalidProof("package registry URL is invalid".into())
        })?;
        if canonical != *registry {
            return Err(PluginTrustError::InvalidProof(
                "package registry URL is not canonical".into(),
            ));
        }
        let signer = self
            .signers
            .get(&proof.signer)
            .ok_or_else(|| PluginTrustError::UntrustedSigner(proof.signer.clone()))?;
        if !signer.registries.contains(registry) {
            return Err(PluginTrustError::RegistryNotTrusted(registry.clone()));
        }
        let signature_bytes = canonical_base64(&proof.signature_base64, 64)
            .map_err(|()| PluginTrustError::InvalidProof("signature bytes are invalid".into()))?;
        let signature = Signature::from_slice(&signature_bytes)
            .map_err(|_| PluginTrustError::InvalidProof("signature length is invalid".into()))?;
        signer
            .key
            .verify_strict(
                &plugin_signing_payload(revision, &proof.signer)?,
                &signature,
            )
            .map_err(|_| PluginTrustError::VerificationFailed)?;

        let mut verified = revision.clone();
        verified.signature = PluginSignature {
            state: PluginSignatureState::Verified,
            scheme: Some(PLUGIN_SIGNATURE_SCHEME.into()),
            signer: Some(proof.signer.clone()),
            signature_sha256: Some(format!("{:x}", Sha256::digest(signature_bytes))),
        };
        Ok(VerifiedPluginRevision(verified))
    }
}

/// Produces the canonical, domain-separated bytes signed by plugin publishers.
///
/// # Errors
/// Rejects invalid manifests, digests, sizes, sources, or signature metadata.
pub fn plugin_signing_payload(
    revision: &PluginRevision,
    signer: &str,
) -> Result<Vec<u8>, PluginTrustError> {
    validate_unverified_revision(revision)?;
    if !valid_identifier(signer) {
        return Err(PluginTrustError::InvalidProof(
            "signer identity is invalid".into(),
        ));
    }
    let PluginInstallSource::Registry { registry, package } = &revision.source else {
        return Err(PluginTrustError::InvalidProof(
            "local imports do not have registry signing payloads".into(),
        ));
    };
    let mut payload = Vec::with_capacity(512);
    payload.extend_from_slice(SIGNATURE_DOMAIN);
    push_u16(&mut payload, PLUGIN_TRUST_SCHEMA_VERSION);
    push_text(&mut payload, signer)?;
    push_u16(&mut payload, revision.schema_version);
    push_u16(&mut payload, revision.manifest.schema_version);
    push_text(&mut payload, &revision.manifest.id)?;
    push_text(&mut payload, &revision.manifest.display_name)?;
    push_text(&mut payload, &revision.manifest.version)?;
    payload.push(match revision.manifest.class {
        PluginClass::TrustedNative => 0,
        PluginClass::SandboxedWasi => 1,
    });
    let mut capabilities = revision.manifest.requested_capabilities.clone();
    capabilities.sort_unstable();
    push_u32(
        &mut payload,
        u32::try_from(capabilities.len())
            .map_err(|_| PluginTrustError::InvalidProof("capability count is excessive".into()))?,
    );
    for capability in capabilities {
        payload.push(match capability {
            PluginCapability::FilesystemRead => 0,
            PluginCapability::FilesystemWrite => 1,
            PluginCapability::Network => 2,
            PluginCapability::Process => 3,
            PluginCapability::Credentials => 4,
        });
    }
    push_text(&mut payload, &revision.module_sha256)?;
    payload.extend_from_slice(&revision.module_size_bytes.to_be_bytes());
    push_text(&mut payload, registry)?;
    push_text(&mut payload, package)?;
    Ok(payload)
}

fn validate_unverified_revision(revision: &PluginRevision) -> Result<(), PluginTrustError> {
    revision
        .manifest
        .validate()
        .map_err(|error| PluginTrustError::InvalidProof(error.to_string()))?;
    if revision.manifest.class != PluginClass::SandboxedWasi
        || revision.module_size_bytes == 0
        || revision.module_size_bytes > 64 * 1_024 * 1_024
        || revision.module_sha256.len() != 64
        || !revision
            .module_sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        || revision.signature.state != PluginSignatureState::Unsigned
        || revision.signature.scheme.is_some()
        || revision.signature.signer.is_some()
        || revision.signature.signature_sha256.is_some()
    {
        return Err(PluginTrustError::InvalidProof(
            "plugin revision is not an unsigned sandboxed package identity".into(),
        ));
    }
    if let PluginInstallSource::Registry { package, .. } = &revision.source
        && (!valid_package_name(package) || package != &revision.manifest.id)
    {
        return Err(PluginTrustError::InvalidProof(
            "registry package identity is invalid".into(),
        ));
    }
    Ok(())
}

fn canonical_registry(value: &str) -> Result<String, ()> {
    let url = Url::parse(value).map_err(|_| ())?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(());
    }
    Ok(url.to_string())
}

fn canonical_base64(value: &str, exact_len: usize) -> Result<Vec<u8>, ()> {
    let bytes = STANDARD.decode(value).map_err(|_| ())?;
    if bytes.len() != exact_len || STANDARD.encode(&bytes) != value {
        return Err(());
    }
    Ok(bytes)
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
        })
}

fn valid_package_name(value: &str) -> bool {
    valid_identifier(value)
}

fn push_text(payload: &mut Vec<u8>, value: &str) -> Result<(), PluginTrustError> {
    let length = u32::try_from(value.len())
        .map_err(|_| PluginTrustError::InvalidProof("signed field is too large".into()))?;
    push_u32(payload, length);
    payload.extend_from_slice(value.as_bytes());
    Ok(())
}

fn push_u16(payload: &mut Vec<u8>, value: u16) {
    payload.extend_from_slice(&value.to_be_bytes());
}

fn push_u32(payload: &mut Vec<u8>, value: u32) {
    payload.extend_from_slice(&value.to_be_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use ed25519_dalek::{Signer as _, SigningKey};
    use mcg_domain::{PLUGIN_CATALOG_SCHEMA_VERSION, PluginRevisionId};
    use mcg_plugin_protocol::{PLUGIN_SCHEMA_VERSION, PluginManifest};

    fn revision(module: &[u8], registry: &str) -> PluginRevision {
        PluginRevision {
            schema_version: PLUGIN_CATALOG_SCHEMA_VERSION,
            id: PluginRevisionId::new(),
            manifest: PluginManifest {
                schema_version: PLUGIN_SCHEMA_VERSION,
                id: "example.signed".into(),
                display_name: "Signed example".into(),
                version: "1.2.3".into(),
                class: PluginClass::SandboxedWasi,
                requested_capabilities: vec![PluginCapability::Network],
            },
            module_sha256: format!("{:x}", Sha256::digest(module)),
            module_size_bytes: u64::try_from(module.len()).expect("module length"),
            source: PluginInstallSource::Registry {
                registry: registry.into(),
                package: "example.signed".into(),
            },
            signature: PluginSignature {
                state: PluginSignatureState::Unsigned,
                scheme: None,
                signer: None,
                signature_sha256: None,
            },
            installed_at: Utc::now(),
        }
    }

    fn proof(signing_key: &SigningKey, revision: &PluginRevision) -> PluginSignatureProof {
        let signature = signing_key
            .sign(&plugin_signing_payload(revision, "publisher.example").expect("payload"));
        PluginSignatureProof {
            schema_version: PLUGIN_TRUST_SCHEMA_VERSION,
            scheme: PLUGIN_SIGNATURE_SCHEME.into(),
            signer: "publisher.example".into(),
            signature_base64: STANDARD.encode(signature.to_bytes()),
        }
    }

    fn trust(signing_key: &SigningKey) -> PluginTrustStore {
        PluginTrustStore::new([TrustedPluginSigner {
            id: "publisher.example".into(),
            public_key_base64: STANDARD.encode(signing_key.verifying_key().to_bytes()),
            registries: vec!["https://plugins.example/".into()],
        }])
        .expect("trust store")
    }

    #[test]
    fn exact_registry_package_identity_verifies() {
        let key = SigningKey::from_bytes(&[7; 32]);
        let revision = revision(b"module", "https://plugins.example/");
        let verified = trust(&key)
            .verify_registry_revision(&revision, &proof(&key, &revision))
            .expect("verified package");
        assert_eq!(
            verified.revision().signature.state,
            PluginSignatureState::Verified
        );
        assert_eq!(
            verified.revision().signature.signer.as_deref(),
            Some("publisher.example")
        );
        assert!(verified.revision().signature.signature_sha256.is_some());
    }

    #[test]
    fn tampering_and_cross_registry_replay_fail() {
        let key = SigningKey::from_bytes(&[11; 32]);
        let package_revision = revision(b"module", "https://plugins.example/");
        let proof = proof(&key, &package_revision);

        let mut changed = package_revision;
        changed.module_sha256 = "a".repeat(64);
        assert!(matches!(
            trust(&key).verify_registry_revision(&changed, &proof),
            Err(PluginTrustError::VerificationFailed)
        ));

        let other_registry = revision(b"module", "https://other.example/");
        assert!(matches!(
            trust(&key).verify_registry_revision(&other_registry, &proof),
            Err(PluginTrustError::RegistryNotTrusted(_))
        ));
    }

    #[test]
    fn malformed_trust_and_local_signature_claims_fail_closed() {
        assert!(
            PluginTrustStore::new([TrustedPluginSigner {
                id: "publisher.example".into(),
                public_key_base64: "not-base64".into(),
                registries: vec!["http://plugins.example/".into()],
            }])
            .is_err()
        );

        let key = SigningKey::from_bytes(&[13; 32]);
        let encoded_key = STANDARD.encode(key.verifying_key().to_bytes());
        assert!(
            PluginTrustStore::new([TrustedPluginSigner {
                id: "publisher.example".into(),
                public_key_base64: encoded_key.trim_end_matches('=').into(),
                registries: vec!["https://plugins.example/".into()],
            }])
            .is_err()
        );
        assert!(
            PluginTrustStore::new([
                TrustedPluginSigner {
                    id: "publisher.example".into(),
                    public_key_base64: encoded_key.clone(),
                    registries: vec!["https://plugins.example/".into()],
                },
                TrustedPluginSigner {
                    id: "publisher.alias".into(),
                    public_key_base64: encoded_key,
                    registries: vec!["https://plugins.example/".into()],
                },
            ])
            .is_err()
        );
        let registry_revision = revision(b"module", "https://plugins.example/");
        let registry_proof = proof(&key, &registry_revision);
        let mut local = registry_revision;
        local.source = PluginInstallSource::LocalImport;
        assert!(matches!(
            trust(&key).verify_registry_revision(&local, &registry_proof),
            Err(PluginTrustError::InvalidProof(_))
        ));
    }

    #[test]
    fn signer_identity_cannot_be_relabelled() {
        let first = SigningKey::from_bytes(&[17; 32]);
        let second = SigningKey::from_bytes(&[19; 32]);
        let trust = PluginTrustStore::new([
            TrustedPluginSigner {
                id: "publisher.example".into(),
                public_key_base64: STANDARD.encode(first.verifying_key().to_bytes()),
                registries: vec!["https://plugins.example/".into()],
            },
            TrustedPluginSigner {
                id: "publisher.other".into(),
                public_key_base64: STANDARD.encode(second.verifying_key().to_bytes()),
                registries: vec!["https://plugins.example/".into()],
            },
        ])
        .expect("trust store");
        let revision = revision(b"module", "https://plugins.example/");
        let mut proof = proof(&first, &revision);
        proof.signer = "publisher.other".into();
        assert!(matches!(
            trust.verify_registry_revision(&revision, &proof),
            Err(PluginTrustError::VerificationFailed)
        ));
    }
}
