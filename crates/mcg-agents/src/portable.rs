use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use mcg_domain::{OrganizationId, OrganizationImportId, OrganizationRevisionId, RoleId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    EvaluationPolicy, ORGANIZATION_SCHEMA_VERSION, OrganizationGenome, OrganizationPolicy,
    RelationshipKind, RoleDefinition, RoleKind,
};

pub const PORTABLE_ORGANIZATION_SCHEMA_VERSION: u16 = 1;
const MAX_PORTABLE_ORGANIZATION_BYTES: usize = 1024 * 1024;
const MAX_PORTABLE_ROLES: usize = 64;
const MAX_PORTABLE_RELATIONSHIPS: usize = 512;
const MAX_PORTABLE_TEXT_CHARS: usize = 16_384;
const MAX_DISPLAY_NAME_CHARS: usize = 100;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortableOrganizationGenome {
    pub schema_version: u16,
    pub source: PortableOrganizationSource,
    pub definition: PortableOrganizationDefinition,
    pub content_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortableOrganizationSource {
    pub organization_id: OrganizationId,
    pub revision_id: OrganizationRevisionId,
    pub revision: u64,
    pub exported_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortableOrganizationDefinition {
    pub name: String,
    pub roles: Vec<PortableRoleDefinition>,
    pub relationships: Vec<PortableRoleRelationship>,
    pub policy: OrganizationPolicy,
    pub evaluation: EvaluationPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortableRoleDefinition {
    pub key: String,
    pub kind: RoleKind,
    pub name: String,
    pub purpose: String,
    pub prompt_policy: String,
    pub model_policy: crate::ModelPolicy,
    pub tool_capabilities: Vec<String>,
    pub memory_namespace: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortableRoleRelationship {
    pub from: String,
    pub to: String,
    pub kind: RelationshipKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortableOrganizationComparison {
    pub equivalent: bool,
    pub left_sha256: String,
    pub right_sha256: String,
    pub sections_changed: Vec<PortableOrganizationSection>,
    pub roles_added: Vec<String>,
    pub roles_removed: Vec<String>,
    pub roles_changed: Vec<String>,
    pub relationships_added: Vec<String>,
    pub relationships_removed: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortableOrganizationSection {
    Name,
    Policy,
    Evaluation,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PortableOrganizationError {
    #[error("portable organization schema {0} is unsupported")]
    UnsupportedSchema(u16),
    #[error("portable organization digest is malformed or does not match its definition")]
    DigestMismatch,
    #[error("portable organization exceeds its bounded contract")]
    Bounds,
    #[error("portable organization contains an invalid role graph")]
    InvalidGraph,
    #[error("portable organization definition cannot be encoded: {0}")]
    Encoding(String),
    #[error("portable organization materialization is invalid: {0}")]
    Materialization(String),
}

impl PortableOrganizationGenome {
    /// Exports one validated immutable revision into an identity-independent,
    /// content-addressed definition. Role keys preserve declared order and are
    /// remapped to fresh local identities on import.
    ///
    /// # Errors
    ///
    /// Returns an error when the source genome is invalid or exceeds bounds.
    pub fn export(
        genome: &OrganizationGenome,
        exported_at: DateTime<Utc>,
    ) -> Result<Self, PortableOrganizationError> {
        genome
            .validate()
            .map_err(|error| PortableOrganizationError::Materialization(error.to_string()))?;
        let keys = genome
            .roles
            .iter()
            .enumerate()
            .map(|(index, role)| (role.id, format!("role-{index:03}")))
            .collect::<BTreeMap<_, _>>();
        let roles = genome
            .roles
            .iter()
            .map(|role| PortableRoleDefinition {
                key: keys[&role.id].clone(),
                kind: role.kind,
                name: role.name.clone(),
                purpose: role.purpose.clone(),
                prompt_policy: role.prompt_policy.clone(),
                model_policy: role.model_policy.clone(),
                tool_capabilities: role.tool_capabilities.clone(),
                memory_namespace: role.memory_namespace.clone(),
            })
            .collect();
        let relationships = genome
            .relationships
            .iter()
            .map(|relationship| PortableRoleRelationship {
                from: keys[&relationship.from].clone(),
                to: keys[&relationship.to].clone(),
                kind: relationship.kind,
            })
            .collect();
        let definition = PortableOrganizationDefinition {
            name: genome.name.clone(),
            roles,
            relationships,
            policy: genome.policy.clone(),
            evaluation: genome.evaluation.clone(),
        };
        let content_sha256 = definition_sha256(&definition)?;
        let portable = Self {
            schema_version: PORTABLE_ORGANIZATION_SCHEMA_VERSION,
            source: PortableOrganizationSource {
                organization_id: genome.organization_id,
                revision_id: genome.revision_id,
                revision: genome.revision,
                exported_at,
            },
            definition,
            content_sha256,
        };
        portable.validate()?;
        Ok(portable)
    }

    /// Validates schema, digest, size, text, and logical role references without
    /// trusting source installation identities.
    ///
    /// # Errors
    ///
    /// Returns a bounded-contract, graph, encoding, or digest error.
    pub fn validate(&self) -> Result<(), PortableOrganizationError> {
        if self.schema_version != PORTABLE_ORGANIZATION_SCHEMA_VERSION {
            return Err(PortableOrganizationError::UnsupportedSchema(
                self.schema_version,
            ));
        }
        let encoded = serde_json::to_vec(self)
            .map_err(|error| PortableOrganizationError::Encoding(error.to_string()))?;
        if encoded.len() > MAX_PORTABLE_ORGANIZATION_BYTES
            || self.definition.roles.is_empty()
            || self.definition.roles.len() > MAX_PORTABLE_ROLES
            || self.definition.relationships.len() > MAX_PORTABLE_RELATIONSHIPS
            || !bounded_display_name(&self.definition.name)
        {
            return Err(PortableOrganizationError::Bounds);
        }
        let keys = self
            .definition
            .roles
            .iter()
            .map(|role| role.key.as_str())
            .collect::<BTreeSet<_>>();
        if keys.len() != self.definition.roles.len()
            || self.definition.roles.iter().any(|role| {
                !valid_key(&role.key)
                    || !bounded_display_name(&role.name)
                    || !bounded_text(&role.purpose)
                    || !bounded_text(&role.prompt_policy)
                    || !bounded_text(&role.model_policy.capability_class)
                    || !bounded_text(&role.memory_namespace)
                    || role
                        .model_policy
                        .preferred_models
                        .iter()
                        .chain(role.tool_capabilities.iter())
                        .any(|value| !bounded_text(value))
            })
            || self.definition.relationships.iter().any(|relationship| {
                relationship.from == relationship.to
                    || !keys.contains(relationship.from.as_str())
                    || !keys.contains(relationship.to.as_str())
            })
        {
            return Err(PortableOrganizationError::InvalidGraph);
        }
        if definition_sha256(&self.definition)? != self.content_sha256 {
            return Err(PortableOrganizationError::DigestMismatch);
        }
        Ok(())
    }

    /// Materializes an imported definition as a fresh root organization. Source
    /// identifiers remain provenance only and never become local authority.
    ///
    /// # Errors
    ///
    /// Returns an error when the portable definition or resulting genome is invalid.
    pub fn materialize(
        &self,
        name: Option<&str>,
        created_at: DateTime<Utc>,
    ) -> Result<OrganizationGenome, PortableOrganizationError> {
        self.validate()?;
        let name = name.unwrap_or(&self.definition.name).trim();
        if !bounded_display_name(name) {
            return Err(PortableOrganizationError::Bounds);
        }
        let identities = self
            .definition
            .roles
            .iter()
            .map(|role| (role.key.clone(), RoleId::new()))
            .collect::<BTreeMap<_, _>>();
        let roles = self
            .definition
            .roles
            .iter()
            .map(|role| RoleDefinition {
                id: identities[&role.key],
                kind: role.kind,
                name: role.name.clone(),
                purpose: role.purpose.clone(),
                prompt_policy: role.prompt_policy.clone(),
                model_policy: role.model_policy.clone(),
                tool_capabilities: role.tool_capabilities.clone(),
                memory_namespace: role.memory_namespace.clone(),
            })
            .collect();
        let relationships = self
            .definition
            .relationships
            .iter()
            .map(|relationship| crate::RoleRelationship {
                from: identities[&relationship.from],
                to: identities[&relationship.to],
                kind: relationship.kind,
            })
            .collect();
        let genome = OrganizationGenome {
            schema_version: ORGANIZATION_SCHEMA_VERSION,
            organization_id: OrganizationId::new(),
            revision_id: OrganizationRevisionId::new(),
            revision: 1,
            parent_revision_id: None,
            inherited_from_revision_id: None,
            name: name.to_owned(),
            roles,
            relationships,
            policy: self.definition.policy.clone(),
            evaluation: self.definition.evaluation.clone(),
            created_at,
        };
        genome
            .validate()
            .map_err(|error| PortableOrganizationError::Materialization(error.to_string()))?;
        Ok(genome)
    }

    /// Materializes an import with deterministic local identities derived from
    /// a caller-owned request id. Retries can therefore observe the same durable
    /// organization instead of creating duplicates.
    ///
    /// # Errors
    ///
    /// Returns an error when the definition or resulting genome is invalid.
    pub fn materialize_for_import(
        &self,
        import_id: OrganizationImportId,
        name: Option<&str>,
        created_at: DateTime<Utc>,
    ) -> Result<OrganizationGenome, PortableOrganizationError> {
        self.validate()?;
        let name = name.unwrap_or(&self.definition.name).trim();
        if !bounded_display_name(name) {
            return Err(PortableOrganizationError::Bounds);
        }
        let identities = self
            .definition
            .roles
            .iter()
            .map(|role| {
                (
                    role.key.clone(),
                    RoleId::from_stable_bytes(stable_import_bytes(
                        import_id,
                        &format!("role:{}", role.key),
                    )),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let roles = self
            .definition
            .roles
            .iter()
            .map(|role| RoleDefinition {
                id: identities[&role.key],
                kind: role.kind,
                name: role.name.clone(),
                purpose: role.purpose.clone(),
                prompt_policy: role.prompt_policy.clone(),
                model_policy: role.model_policy.clone(),
                tool_capabilities: role.tool_capabilities.clone(),
                memory_namespace: role.memory_namespace.clone(),
            })
            .collect();
        let relationships = self
            .definition
            .relationships
            .iter()
            .map(|relationship| crate::RoleRelationship {
                from: identities[&relationship.from],
                to: identities[&relationship.to],
                kind: relationship.kind,
            })
            .collect();
        let genome = OrganizationGenome {
            schema_version: ORGANIZATION_SCHEMA_VERSION,
            organization_id: OrganizationId::from_stable_bytes(stable_import_bytes(
                import_id,
                "organization",
            )),
            revision_id: OrganizationRevisionId::from_stable_bytes(stable_import_bytes(
                import_id, "revision",
            )),
            revision: 1,
            parent_revision_id: None,
            inherited_from_revision_id: None,
            name: name.to_owned(),
            roles,
            relationships,
            policy: self.definition.policy.clone(),
            evaluation: self.definition.evaluation.clone(),
            created_at,
        };
        genome
            .validate()
            .map_err(|error| PortableOrganizationError::Materialization(error.to_string()))?;
        Ok(genome)
    }

    #[must_use]
    pub fn compare(&self, other: &Self) -> PortableOrganizationComparison {
        let left_roles = role_keys(&self.definition.roles);
        let right_roles = role_keys(&other.definition.roles);
        let left_relationships = relationship_summaries(&self.definition.relationships);
        let right_relationships = relationship_summaries(&other.definition.relationships);
        let sections_changed = [
            (
                PortableOrganizationSection::Name,
                self.definition.name != other.definition.name,
            ),
            (
                PortableOrganizationSection::Policy,
                self.definition.policy != other.definition.policy,
            ),
            (
                PortableOrganizationSection::Evaluation,
                self.definition.evaluation != other.definition.evaluation,
            ),
        ]
        .into_iter()
        .filter_map(|(section, changed)| changed.then_some(section))
        .collect();
        PortableOrganizationComparison {
            equivalent: self.content_sha256 == other.content_sha256,
            left_sha256: self.content_sha256.clone(),
            right_sha256: other.content_sha256.clone(),
            sections_changed,
            roles_added: right_roles.difference(&left_roles).cloned().collect(),
            roles_removed: left_roles.difference(&right_roles).cloned().collect(),
            roles_changed: changed_role_keys(&self.definition.roles, &other.definition.roles),
            relationships_added: right_relationships
                .difference(&left_relationships)
                .cloned()
                .collect(),
            relationships_removed: left_relationships
                .difference(&right_relationships)
                .cloned()
                .collect(),
        }
    }
}

fn definition_sha256(
    definition: &PortableOrganizationDefinition,
) -> Result<String, PortableOrganizationError> {
    let bytes = serde_json::to_vec(definition)
        .map_err(|error| PortableOrganizationError::Encoding(error.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn stable_import_bytes(import_id: OrganizationImportId, label: &str) -> [u8; 16] {
    let digest = Sha256::digest(format!("{import_id}:{label}").as_bytes());
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes
}

fn bounded_text(value: &str) -> bool {
    let length = value.chars().count();
    value.trim() == value && length > 0 && length <= MAX_PORTABLE_TEXT_CHARS
}

fn bounded_display_name(value: &str) -> bool {
    bounded_text(value) && value.chars().count() <= MAX_DISPLAY_NAME_CHARS
}

fn valid_key(value: &str) -> bool {
    value.len() <= 64
        && !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn role_keys(roles: &[PortableRoleDefinition]) -> BTreeSet<String> {
    roles.iter().map(|role| role.key.clone()).collect()
}

fn changed_role_keys(
    left: &[PortableRoleDefinition],
    right: &[PortableRoleDefinition],
) -> Vec<String> {
    let left = left
        .iter()
        .map(|role| (&role.key, role))
        .collect::<BTreeMap<_, _>>();
    let right = right
        .iter()
        .map(|role| (&role.key, role))
        .collect::<BTreeMap<_, _>>();
    left.into_iter()
        .filter_map(|(key, role)| {
            right
                .get(key)
                .filter(|candidate| **candidate != role)
                .map(|_| key.clone())
        })
        .collect()
}

fn relationship_summaries(relationships: &[PortableRoleRelationship]) -> BTreeSet<String> {
    relationships
        .iter()
        .map(|relationship| {
            format!(
                "{} -> {:?} -> {}",
                relationship.from, relationship.kind, relationship.to
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_genome_round_trips_semantics_with_fresh_authority() {
        let source = OrganizationGenome::consumer_default(Utc::now());
        let portable =
            PortableOrganizationGenome::export(&source, Utc::now()).expect("export genome");
        portable.validate().expect("validate export");
        let imported = portable
            .materialize(Some("Imported consumer"), Utc::now())
            .expect("materialize genome");
        assert_ne!(imported.organization_id, source.organization_id);
        assert_ne!(imported.revision_id, source.revision_id);
        assert!(imported.roles.iter().all(|role| {
            source
                .roles
                .iter()
                .all(|source_role| source_role.id != role.id)
        }));
        assert_eq!(imported.inherited_from_revision_id, None);
        assert_eq!(imported.policy, source.policy);
        assert_eq!(imported.evaluation, source.evaluation);
        assert_eq!(imported.relationships.len(), source.relationships.len());

        let import_id = OrganizationImportId::new();
        let first = portable
            .materialize_for_import(import_id, None, Utc::now())
            .expect("deterministic import");
        let second = portable
            .materialize_for_import(import_id, None, first.created_at)
            .expect("deterministic retry");
        assert_eq!(first, second);
    }

    #[test]
    fn portable_genome_rejects_tamper_and_compares_definition_changes() {
        let source = OrganizationGenome::consumer_default(Utc::now());
        let portable =
            PortableOrganizationGenome::export(&source, Utc::now()).expect("export genome");
        let mut changed = portable.clone();
        changed.definition.policy.max_parallel_agents = 2;
        assert_eq!(
            changed.validate(),
            Err(PortableOrganizationError::DigestMismatch)
        );
        changed.content_sha256 =
            definition_sha256(&changed.definition).expect("changed definition digest");
        changed.validate().expect("valid changed genome");
        let comparison = portable.compare(&changed);
        assert!(!comparison.equivalent);
        assert_eq!(
            comparison.sections_changed,
            [PortableOrganizationSection::Policy]
        );

        let mut oversized_name = portable;
        oversized_name.definition.name = "x".repeat(MAX_DISPLAY_NAME_CHARS + 1);
        oversized_name.content_sha256 =
            definition_sha256(&oversized_name.definition).expect("oversized definition digest");
        assert_eq!(
            oversized_name.validate(),
            Err(PortableOrganizationError::Bounds)
        );
    }
}
