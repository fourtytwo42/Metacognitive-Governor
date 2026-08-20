use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use mcg_domain::{
    AgentInstanceId, AgentLeaseId, EvaluationId, OrganizationId, OrganizationRevisionId, RoleId,
    RunId, TaskBudget, TaskId, ToolExecutionId, UsageRecordId, WorkItemId,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const ORGANIZATION_SCHEMA_VERSION: u16 = 2;
const LEGACY_ORGANIZATION_SCHEMA_VERSION: u16 = 1;
pub const AGENT_RUN_SCHEMA_VERSION: u16 = 1;
pub const PROVIDER_PROXY_TURN_SCHEMA_VERSION: u16 = 2;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrganizationGenome {
    pub schema_version: u16,
    pub organization_id: OrganizationId,
    pub revision_id: OrganizationRevisionId,
    pub revision: u64,
    pub parent_revision_id: Option<OrganizationRevisionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inherited_from_revision_id: Option<OrganizationRevisionId>,
    pub name: String,
    pub roles: Vec<RoleDefinition>,
    pub relationships: Vec<RoleRelationship>,
    pub policy: OrganizationPolicy,
    pub evaluation: EvaluationPolicy,
    pub created_at: DateTime<Utc>,
}

impl OrganizationGenome {
    /// Validates graph identity, required built-in roles, and delegation cycles.
    ///
    /// # Errors
    ///
    /// Returns a precise invariant violation for malformed imported genomes.
    pub fn validate(&self) -> Result<(), OrganizationError> {
        if !matches!(
            self.schema_version,
            LEGACY_ORGANIZATION_SCHEMA_VERSION | ORGANIZATION_SCHEMA_VERSION
        ) {
            return Err(OrganizationError::UnsupportedSchema(self.schema_version));
        }
        if (self.schema_version == LEGACY_ORGANIZATION_SCHEMA_VERSION
            && self.inherited_from_revision_id.is_some())
            || self.inherited_from_revision_id == Some(self.revision_id)
            || (self.inherited_from_revision_id.is_some()
                && self.inherited_from_revision_id == self.parent_revision_id)
        {
            return Err(OrganizationError::InvalidInheritance);
        }
        if self.name.trim().is_empty() || self.roles.is_empty() {
            return Err(OrganizationError::EmptyOrganization);
        }
        let role_ids = self
            .roles
            .iter()
            .map(|role| role.id)
            .collect::<BTreeSet<_>>();
        if role_ids.len() != self.roles.len() {
            return Err(OrganizationError::DuplicateRole);
        }
        for relationship in &self.relationships {
            if !role_ids.contains(&relationship.from) || !role_ids.contains(&relationship.to) {
                return Err(OrganizationError::UnknownRoleRelationship);
            }
            if relationship.from == relationship.to {
                return Err(OrganizationError::SelfRelationship);
            }
        }
        if has_delegation_cycle(&role_ids, &self.relationships) {
            return Err(OrganizationError::DelegationCycle);
        }
        if self.policy.max_parallel_agents == 0 || self.policy.max_total_agents == 0 {
            return Err(OrganizationError::InvalidAgentLimit);
        }
        if self.policy.max_parallel_agents > self.policy.max_total_agents
            || self.policy.default_lease_seconds == 0
            || !self.policy.max_organizational_overhead_ratio.is_finite()
            || !(0.0..=1.0).contains(&self.policy.max_organizational_overhead_ratio)
            || !self.evaluation.minimum_confidence.is_finite()
            || !(0.0..=1.0).contains(&self.evaluation.minimum_confidence)
            || self.evaluation.calibration_window == 0
        {
            return Err(OrganizationError::InvalidPolicy);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleDefinition {
    pub id: RoleId,
    pub kind: RoleKind,
    pub name: String,
    pub purpose: String,
    pub prompt_policy: String,
    pub model_policy: ModelPolicy,
    pub tool_capabilities: Vec<String>,
    pub memory_namespace: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoleKind {
    Executive,
    Conversation,
    Coder,
    Evaluator,
    ToolWorker,
    Researcher,
    Tester,
    TemporarySpecialist,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelPolicy {
    pub capability_class: String,
    pub preferred_models: Vec<String>,
    pub allow_fallback: bool,
    pub max_escalation_tier: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleRelationship {
    pub from: RoleId,
    pub to: RoleId,
    pub kind: RelationshipKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipKind {
    DelegatesTo,
    Reviews,
    EscalatesTo,
    ReportsTo,
    SharesInformationWith,
    ObservedBy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrganizationPolicy {
    pub prefer_minimal_organization: bool,
    pub max_parallel_agents: u16,
    pub max_total_agents: u16,
    pub max_organizational_overhead_ratio: f64,
    pub default_lease_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluationPolicy {
    pub require_independent_evaluation: bool,
    pub require_objective_evidence: bool,
    pub minimum_confidence: f64,
    pub calibration_window: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentInstance {
    pub schema_version: u16,
    pub id: AgentInstanceId,
    pub role_id: RoleId,
    pub run_id: RunId,
    pub task_id: TaskId,
    pub state: AgentState,
    pub model: Option<AssignedModel>,
    pub context_fingerprint: String,
    pub confidence: ConfidenceEstimate,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    Created,
    Running,
    Waiting,
    Blocked,
    Completed,
    Failed,
    Cancelled,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssignedModel {
    pub provider_connection_id: String,
    pub model_id: String,
    pub route_reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentLease {
    pub schema_version: u16,
    pub id: AgentLeaseId,
    pub instance_id: AgentInstanceId,
    pub work_item_id: WorkItemId,
    pub goal: String,
    pub budget: LeaseBudget,
    pub tool_permissions: Vec<String>,
    pub reporting_interval_seconds: u64,
    pub max_retries: u16,
    pub escalation_conditions: Vec<EscalationCondition>,
    pub state: AgentLeaseState,
    pub acquired_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub heartbeat_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LeaseBudget {
    pub limit: TaskBudget,
    pub consumed_tokens: u64,
    pub consumed_cost_usd: f64,
    pub elapsed_seconds: u64,
}

impl LeaseBudget {
    #[must_use]
    pub fn is_exhausted(&self) -> bool {
        self.limit
            .max_tokens
            .is_some_and(|limit| self.consumed_tokens >= limit)
            || self
                .limit
                .max_cost_usd
                .is_some_and(|limit| self.consumed_cost_usd >= limit)
            || self
                .limit
                .max_duration_seconds
                .is_some_and(|limit| self.elapsed_seconds >= limit)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentLeaseState {
    Active,
    Completed,
    Failed,
    Expired,
    Reassigned,
    Terminated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum EscalationCondition {
    LowConfidence { below_basis_points: u16 },
    RetryLimit,
    BudgetThreshold { basis_points: u16 },
    ToolFailure,
    ConflictingEvidence,
    NoProgress { seconds: u64 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkPlan {
    pub run_id: RunId,
    pub task_id: TaskId,
    pub organization_revision_id: OrganizationRevisionId,
    pub items: Vec<WorkItem>,
    pub selected_roles: Vec<RoleId>,
    pub selection_reason: String,
    pub estimated_organizational_overhead: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentRun {
    pub schema_version: u16,
    pub id: RunId,
    pub task_id: TaskId,
    pub organization_revision_id: OrganizationRevisionId,
    pub state: AgentRunState,
    pub metrics: OrganizationalMetrics,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRunState {
    Planned,
    Running,
    Waiting,
    Recovering,
    Completed,
    Failed,
    Cancelled,
}

impl AgentRunState {
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Planned, Self::Running | Self::Cancelled)
                | (
                    Self::Running,
                    Self::Waiting
                        | Self::Recovering
                        | Self::Completed
                        | Self::Failed
                        | Self::Cancelled
                )
                | (
                    Self::Waiting,
                    Self::Running | Self::Recovering | Self::Failed | Self::Cancelled
                )
                | (
                    Self::Recovering,
                    Self::Running | Self::Failed | Self::Cancelled
                )
        )
    }

    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentUsageRecord {
    pub schema_version: u16,
    pub id: UsageRecordId,
    pub run_id: RunId,
    pub task_id: TaskId,
    pub instance_id: Option<AgentInstanceId>,
    pub kind: UsageKind,
    pub provider_connection_id: Option<String>,
    pub model_id: Option<String>,
    pub tool_execution_id: Option<ToolExecutionId>,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: Option<f64>,
    pub duration_ms: u64,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageKind {
    ProviderModel,
    Tool,
    Coordination,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkItem {
    pub id: WorkItemId,
    pub goal: String,
    pub assigned_role: RoleId,
    pub dependencies: Vec<WorkItemId>,
    pub success_conditions: Vec<SuccessCondition>,
    pub budget: TaskBudget,
    pub risk_notes: Vec<String>,
    pub state: WorkItemState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkItemState {
    Pending,
    Ready,
    Running,
    Waiting,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum SuccessCondition {
    Build {
        command: String,
    },
    Test {
        command: String,
    },
    StaticAnalysis {
        command: String,
    },
    ConfigurationValid {
        command: String,
    },
    FileExists {
        path: String,
        min_bytes: u64,
        expected_sha256: Option<String>,
    },
    HttpResponds {
        url: String,
        status: u16,
    },
    PageRenders {
        url: String,
    },
    Accessibility {
        url: String,
    },
    BrowserWorkflow {
        name: String,
        path: String,
        expected_sha256: String,
    },
    ElementExists {
        url: String,
        selector: String,
    },
    ScreenshotMatches {
        baseline: String,
        expected_baseline_sha256: String,
        url: String,
        max_changed_basis_points: u16,
        per_channel_threshold: u8,
    },
    ServiceHealthy {
        endpoint: String,
    },
    PortListening {
        endpoint: String,
    },
    ResearchSources {
        evidence_path: String,
        minimum: u16,
        max_age_days: Option<u32>,
        required_scope: String,
    },
    Custom {
        description: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluationReport {
    pub schema_version: u16,
    pub id: EvaluationId,
    pub run_id: RunId,
    pub task_id: TaskId,
    pub evaluator_instance_id: Option<AgentInstanceId>,
    pub conditions: Vec<ConditionResult>,
    pub outcome: EvaluationOutcome,
    pub confidence: ConfidenceEstimate,
    pub regressions: Vec<String>,
    pub evidence: Vec<EvaluationEvidence>,
    pub evaluated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConditionResult {
    pub condition: SuccessCondition,
    pub passed: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationOutcome {
    Passed,
    Failed,
    Inconclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluationEvidence {
    pub method: String,
    pub artifact_sha256: Option<String>,
    pub event_sequence: Option<u64>,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfidenceEstimate {
    pub basis_points: u16,
    pub sample_size: u32,
}

impl ConfidenceEstimate {
    /// Constructs a bounded confidence estimate.
    ///
    /// # Errors
    ///
    /// Values above 10000 basis points are invalid.
    pub const fn new(basis_points: u16, sample_size: u32) -> Result<Self, OrganizationError> {
        if basis_points > 10_000 {
            return Err(OrganizationError::InvalidConfidence);
        }
        Ok(Self {
            basis_points,
            sample_size,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct OrganizationalMetrics {
    pub agent_instances: u32,
    pub model_calls: u32,
    pub tool_calls: u32,
    pub useful_work_seconds: u64,
    pub coordination_seconds: u64,
    pub tokens: u64,
    pub cost_usd: f64,
}

impl OrganizationalMetrics {
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // Ratios tolerate sub-nanosecond precision loss at u64 scale.
    pub fn overhead_ratio(&self) -> f64 {
        let total = self
            .useful_work_seconds
            .saturating_add(self.coordination_seconds);
        if total == 0 {
            0.0
        } else {
            self.coordination_seconds as f64 / total as f64
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalibrationBucket {
    pub lower_basis_points: u16,
    pub upper_basis_points: u16,
    pub predictions: u32,
    pub correct: u32,
}

impl CalibrationBucket {
    #[must_use]
    pub fn observed_accuracy(&self) -> Option<f64> {
        (self.predictions > 0).then(|| f64::from(self.correct) / f64::from(self.predictions))
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OrganizationError {
    #[error("unsupported organization schema version {0}")]
    UnsupportedSchema(u16),
    #[error("organization must have a name and at least one role")]
    EmptyOrganization,
    #[error("organization contains duplicate role identities")]
    DuplicateRole,
    #[error("organization relationship references an unknown role")]
    UnknownRoleRelationship,
    #[error("organization role cannot relate to itself")]
    SelfRelationship,
    #[error("organization delegation graph contains a cycle")]
    DelegationCycle,
    #[error("organization agent limits must be positive")]
    InvalidAgentLimit,
    #[error("organization policy values are inconsistent or out of bounds")]
    InvalidPolicy,
    #[error("organization inheritance identity is inconsistent")]
    InvalidInheritance,
    #[error("confidence must be between 0 and 10000 basis points")]
    InvalidConfidence,
}

fn has_delegation_cycle(roles: &BTreeSet<RoleId>, relationships: &[RoleRelationship]) -> bool {
    let mut edges = BTreeMap::<RoleId, Vec<RoleId>>::new();
    for relationship in relationships
        .iter()
        .filter(|relationship| relationship.kind == RelationshipKind::DelegatesTo)
    {
        edges
            .entry(relationship.from)
            .or_default()
            .push(relationship.to);
    }
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    roles
        .iter()
        .any(|role| visit(*role, &edges, &mut visiting, &mut visited))
}

fn visit(
    role: RoleId,
    edges: &BTreeMap<RoleId, Vec<RoleId>>,
    visiting: &mut BTreeSet<RoleId>,
    visited: &mut BTreeSet<RoleId>,
) -> bool {
    if visited.contains(&role) {
        return false;
    }
    if !visiting.insert(role) {
        return true;
    }
    if edges.get(&role).is_some_and(|children| {
        children
            .iter()
            .any(|child| visit(*child, edges, visiting, visited))
    }) {
        return true;
    }
    visiting.remove(&role);
    visited.insert(role);
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn role(kind: RoleKind, name: &str) -> RoleDefinition {
        RoleDefinition {
            id: RoleId::new(),
            kind,
            name: name.into(),
            purpose: name.into(),
            prompt_policy: "bounded".into(),
            model_policy: ModelPolicy {
                capability_class: "general".into(),
                preferred_models: Vec::new(),
                allow_fallback: true,
                max_escalation_tier: 2,
            },
            tool_capabilities: Vec::new(),
            memory_namespace: name.to_ascii_lowercase(),
        }
    }

    #[test]
    fn genome_round_trips_and_rejects_delegation_cycles() {
        let executive = role(RoleKind::Executive, "Executive");
        let coder = role(RoleKind::Coder, "Coder");
        let mut genome = OrganizationGenome {
            schema_version: ORGANIZATION_SCHEMA_VERSION,
            organization_id: OrganizationId::new(),
            revision_id: OrganizationRevisionId::new(),
            revision: 1,
            parent_revision_id: None,
            inherited_from_revision_id: None,
            name: "fixture".into(),
            roles: vec![executive.clone(), coder.clone()],
            relationships: vec![RoleRelationship {
                from: executive.id,
                to: coder.id,
                kind: RelationshipKind::DelegatesTo,
            }],
            policy: OrganizationPolicy {
                prefer_minimal_organization: true,
                max_parallel_agents: 2,
                max_total_agents: 4,
                max_organizational_overhead_ratio: 0.25,
                default_lease_seconds: 900,
            },
            evaluation: EvaluationPolicy {
                require_independent_evaluation: true,
                require_objective_evidence: true,
                minimum_confidence: 0.8,
                calibration_window: 100,
            },
            created_at: Utc::now(),
        };
        genome.validate().expect("valid genome");
        let json = serde_json::to_string(&genome).expect("serialize");
        assert_eq!(
            serde_json::from_str::<OrganizationGenome>(&json).expect("deserialize"),
            genome
        );
        let mut legacy = serde_json::to_value(&genome).expect("legacy JSON");
        legacy["schema_version"] = serde_json::json!(LEGACY_ORGANIZATION_SCHEMA_VERSION);
        legacy
            .as_object_mut()
            .expect("object")
            .remove("inherited_from_revision_id");
        let legacy: OrganizationGenome = serde_json::from_value(legacy).expect("legacy genome");
        assert_eq!(legacy.inherited_from_revision_id, None);
        legacy.validate().expect("legacy schema remains readable");
        genome.relationships.push(RoleRelationship {
            from: coder.id,
            to: executive.id,
            kind: RelationshipKind::DelegatesTo,
        });
        assert_eq!(genome.validate(), Err(OrganizationError::DelegationCycle));
    }

    #[test]
    fn lease_budget_and_overhead_are_objective() {
        let budget = LeaseBudget {
            limit: TaskBudget {
                max_tokens: Some(100),
                max_cost_usd: Some(1.0),
                max_duration_seconds: Some(60),
            },
            consumed_tokens: 100,
            consumed_cost_usd: 0.2,
            elapsed_seconds: 10,
        };
        assert!(budget.is_exhausted());
        let metrics = OrganizationalMetrics {
            useful_work_seconds: 75,
            coordination_seconds: 25,
            ..OrganizationalMetrics::default()
        };
        assert!((metrics.overhead_ratio() - 0.25).abs() < f64::EPSILON);
    }
}
