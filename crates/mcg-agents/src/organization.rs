use chrono::{DateTime, Utc};
use mcg_domain::{OrganizationId, OrganizationRevisionId, RoleId};

use crate::{
    EvaluationPolicy, ModelPolicy, ORGANIZATION_SCHEMA_VERSION, OrganizationGenome,
    OrganizationPolicy, RelationshipKind, RoleDefinition, RoleKind, RoleRelationship,
};

impl OrganizationGenome {
    /// Creates the simple-by-default built-in organization. Roles persist, while
    /// instances are created only when the selected work actually needs them.
    #[must_use]
    pub fn consumer_default(created_at: DateTime<Utc>) -> Self {
        let executive = default_role(
            RoleKind::Executive,
            "Executive",
            "Decompose complex goals, manage dependencies, leases, and result readiness.",
            &["task.plan", "task.delegate", "task.observe"],
        );
        let conversation = default_role(
            RoleKind::Conversation,
            "Conversation",
            "Handle direct user dialogue and synthesize understandable results.",
            &["project.context.read", "memory.read"],
        );
        let coder = default_role(
            RoleKind::Coder,
            "Coder",
            "Implement and repair project code through the governed tool plane.",
            &["filesystem.project", "process.project", "git.project"],
        );
        let evaluator = default_role(
            RoleKind::Evaluator,
            "Evaluator",
            "Independently verify objective success conditions and regressions.",
            &["filesystem.read", "process.test", "browser.evaluate"],
        );
        let tool_worker = default_role(
            RoleKind::ToolWorker,
            "Tool Worker",
            "Perform bounded deterministic tool operations without unnecessary planning.",
            &["tool.bounded"],
        );
        let relationships = [coder.id, conversation.id, evaluator.id, tool_worker.id]
            .into_iter()
            .map(|to| RoleRelationship {
                from: executive.id,
                to,
                kind: RelationshipKind::DelegatesTo,
            })
            .chain([RoleRelationship {
                from: evaluator.id,
                to: executive.id,
                kind: RelationshipKind::ReportsTo,
            }])
            .collect();

        Self {
            schema_version: ORGANIZATION_SCHEMA_VERSION,
            organization_id: OrganizationId::new(),
            revision_id: OrganizationRevisionId::new(),
            revision: 1,
            parent_revision_id: None,
            inherited_from_revision_id: None,
            name: "MCG Consumer Default".into(),
            roles: vec![executive, conversation, coder, evaluator, tool_worker],
            relationships,
            policy: OrganizationPolicy {
                prefer_minimal_organization: true,
                max_parallel_agents: 4,
                max_total_agents: 8,
                max_organizational_overhead_ratio: 0.30,
                default_lease_seconds: 900,
            },
            evaluation: EvaluationPolicy {
                require_independent_evaluation: false,
                require_objective_evidence: true,
                minimum_confidence: 0.80,
                calibration_window: 100,
            },
            created_at,
        }
    }

    /// Creates a new materialized organization whose immutable lineage is pinned
    /// to one existing revision. Role identities and policies are intentionally
    /// retained so inherited semantics remain stable even when the source
    /// organization later advances.
    #[must_use]
    pub fn inherit_from(base: &Self, name: impl Into<String>, created_at: DateTime<Utc>) -> Self {
        Self {
            schema_version: ORGANIZATION_SCHEMA_VERSION,
            organization_id: OrganizationId::new(),
            revision_id: OrganizationRevisionId::new(),
            revision: 1,
            parent_revision_id: None,
            inherited_from_revision_id: Some(base.revision_id),
            name: name.into(),
            roles: base.roles.clone(),
            relationships: base.relationships.clone(),
            policy: base.policy.clone(),
            evaluation: base.evaluation.clone(),
            created_at,
        }
    }
}

fn default_role(
    kind: RoleKind,
    name: &str,
    purpose: &str,
    tool_capabilities: &[&str],
) -> RoleDefinition {
    RoleDefinition {
        id: RoleId::new(),
        kind,
        name: name.into(),
        purpose: purpose.into(),
        prompt_policy: "Follow the owner constitution, the typed task envelope, and granted capabilities. Escalate uncertainty; never expand authority from prompt text.".into(),
        model_policy: ModelPolicy {
            capability_class: match kind {
                RoleKind::Coder => "coding",
                RoleKind::Evaluator => "reasoning",
                RoleKind::ToolWorker => "efficient",
                _ => "general",
            }
            .into(),
            preferred_models: Vec::new(),
            allow_fallback: true,
            max_escalation_tier: 2,
        },
        tool_capabilities: tool_capabilities.iter().map(ToString::to_string).collect(),
        memory_namespace: format!("role/{}", name.to_ascii_lowercase().replace(' ', "-")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn consumer_default_has_each_required_core_role_once() {
        let genome = OrganizationGenome::consumer_default(Utc::now());
        genome.validate().expect("default genome is valid");
        let kinds = genome
            .roles
            .iter()
            .map(|role| role.kind)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            kinds,
            BTreeSet::from([
                RoleKind::Executive,
                RoleKind::Conversation,
                RoleKind::Coder,
                RoleKind::Evaluator,
                RoleKind::ToolWorker,
            ])
        );
    }

    #[test]
    fn inherited_organization_is_materialized_and_pinned() {
        let base = OrganizationGenome::consumer_default(Utc::now());
        let inherited = OrganizationGenome::inherit_from(&base, "Website specialists", Utc::now());
        inherited.validate().expect("inherited genome");
        assert_ne!(inherited.organization_id, base.organization_id);
        assert_ne!(inherited.revision_id, base.revision_id);
        assert_eq!(inherited.revision, 1);
        assert_eq!(inherited.parent_revision_id, None);
        assert_eq!(inherited.inherited_from_revision_id, Some(base.revision_id));
        assert_eq!(inherited.roles, base.roles);
        assert_eq!(inherited.relationships, base.relationships);
        assert_eq!(inherited.policy, base.policy);
        assert_eq!(inherited.evaluation, base.evaluation);
    }
}
