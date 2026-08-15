use chrono::{DateTime, Utc};
use mcg_domain::{
    EvaluationId, ExperimentBenchmarkId, ExperimentDecisionId, ExperimentId, OrganizationId,
    OrganizationRevisionId, RunId, SupervisionObservationId, TaskId,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    AgentRunState, ConfidenceEstimate, EvaluationOutcome, EvaluationReport, OrganizationalMetrics,
};

pub const SUPERVISION_SCHEMA_VERSION: u16 = 1;
pub const EXPERIMENT_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupervisionLoop {
    Fast,
    Medium,
    Slow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterventionLevel {
    Observe,
    Prime,
    Nudge,
    Reframe,
    Redirect,
    Halt,
    Restructure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupervisionSignal {
    RepeatedFailure,
    AgentDisagreement,
    RunawayLoop,
    Overconfidence,
    Underconfidence,
    ToolFailure,
    BudgetPressure,
    ExcessiveOverhead,
    UserCorrection,
    Regression,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupervisionEvidence {
    pub method: String,
    pub artifact_sha256: Option<String>,
    pub event_sequence: Option<u64>,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunSupervisionSnapshot {
    pub task_id: TaskId,
    pub run_id: RunId,
    pub organization_revision_id: OrganizationRevisionId,
    pub run_state: AgentRunState,
    pub metrics: OrganizationalMetrics,
    pub failed_instances: u32,
    pub exhausted_leases: u32,
    pub evaluation: EvaluationReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupervisionFinding {
    pub loop_kind: SupervisionLoop,
    pub signal: SupervisionSignal,
    pub severity_basis_points: u16,
    pub recommended_intervention: InterventionLevel,
    pub summary: String,
    pub evidence: Vec<SupervisionEvidence>,
}

pub struct SupervisionAnalyzer;

impl SupervisionAnalyzer {
    /// Derives deterministic supervision findings from durable run evidence.
    #[must_use]
    pub fn analyze(snapshot: &RunSupervisionSnapshot) -> Vec<SupervisionFinding> {
        let mut findings = Vec::new();
        if let Some(finding) = fast_finding(snapshot) {
            findings.push(finding);
        }
        if let Some(finding) = medium_finding(snapshot) {
            findings.push(finding);
        }
        if let Some(finding) = slow_finding(snapshot) {
            findings.push(finding);
        }
        findings
    }
}

pub struct MeasuredCandidateDesigner;

impl MeasuredCandidateDesigner {
    /// Produces a bounded, inactive organization revision from one slow-loop finding.
    ///
    /// # Errors
    /// Returns an invariant error unless the observation requests organization restructuring
    /// against the supplied baseline revision.
    pub fn design(
        baseline: &crate::OrganizationGenome,
        observation: &SupervisionObservation,
        revision_id: OrganizationRevisionId,
        revision: u64,
        created_at: DateTime<Utc>,
    ) -> Result<crate::OrganizationGenome, ExperimentError> {
        if observation.loop_kind != SupervisionLoop::Slow
            || observation.recommended_intervention != InterventionLevel::Restructure
            || observation.organization_revision_id != Some(baseline.revision_id)
            || revision <= baseline.revision
        {
            return Err(ExperimentError::InvalidCandidateProposal);
        }
        let mut candidate = baseline.clone();
        candidate.revision_id = revision_id;
        candidate.revision = revision;
        candidate.parent_revision_id = Some(baseline.revision_id);
        candidate.name = format!("{} measured candidate r{revision}", baseline.name);
        candidate.created_at = created_at;
        if observation.signal == SupervisionSignal::ExcessiveOverhead {
            candidate.policy.max_parallel_agents = candidate
                .policy
                .max_parallel_agents
                .saturating_sub(1)
                .max(1);
            candidate.policy.max_total_agents = candidate
                .policy
                .max_total_agents
                .max(candidate.policy.max_parallel_agents);
            candidate.policy.max_organizational_overhead_ratio =
                candidate.policy.max_organizational_overhead_ratio.min(0.25);
        } else {
            candidate.evaluation.require_independent_evaluation = true;
            candidate.evaluation.minimum_confidence =
                candidate.evaluation.minimum_confidence.max(0.90);
            for role in &mut candidate.roles {
                if matches!(
                    role.kind,
                    crate::RoleKind::Coder | crate::RoleKind::Evaluator
                ) {
                    role.model_policy.max_escalation_tier = role
                        .model_policy
                        .max_escalation_tier
                        .saturating_add(1)
                        .min(4);
                }
            }
        }
        candidate
            .validate()
            .map_err(|_| ExperimentError::InvalidCandidateProposal)?;
        Ok(candidate)
    }
}

fn fast_finding(snapshot: &RunSupervisionSnapshot) -> Option<SupervisionFinding> {
    let (signal, severity, intervention, summary) = if snapshot.exhausted_leases > 0 {
        (
            SupervisionSignal::BudgetPressure,
            9_000,
            InterventionLevel::Halt,
            "One or more agent leases exhausted a configured budget",
        )
    } else if snapshot.failed_instances > 0 {
        (
            SupervisionSignal::ToolFailure,
            8_000,
            InterventionLevel::Redirect,
            "One or more agent instances failed during the run",
        )
    } else {
        match snapshot.evaluation.outcome {
            EvaluationOutcome::Failed => (
                SupervisionSignal::RepeatedFailure,
                8_500,
                InterventionLevel::Redirect,
                "Objective evaluation failed",
            ),
            EvaluationOutcome::Inconclusive => (
                SupervisionSignal::Underconfidence,
                6_000,
                InterventionLevel::Nudge,
                "Objective evaluation was inconclusive",
            ),
            EvaluationOutcome::Passed => return None,
        }
    };
    Some(finding(
        snapshot,
        SupervisionLoop::Fast,
        signal,
        severity,
        intervention,
        summary,
    ))
}

fn medium_finding(snapshot: &RunSupervisionSnapshot) -> Option<SupervisionFinding> {
    let evaluation = &snapshot.evaluation;
    let (signal, severity, intervention, summary) = if !evaluation.regressions.is_empty() {
        (
            SupervisionSignal::Regression,
            9_000,
            InterventionLevel::Halt,
            "Objective evaluation reported one or more regressions",
        )
    } else if evaluation.outcome == EvaluationOutcome::Failed
        && evaluation.confidence.basis_points >= 8_000
    {
        (
            SupervisionSignal::Overconfidence,
            8_500,
            InterventionLevel::Reframe,
            "High-confidence work failed objective evaluation",
        )
    } else if evaluation.outcome == EvaluationOutcome::Passed
        && evaluation.confidence.basis_points <= 6_000
    {
        (
            SupervisionSignal::Underconfidence,
            6_500,
            InterventionLevel::Prime,
            "Low-confidence work passed objective evaluation",
        )
    } else if snapshot.metrics.coordination_seconds >= 30 && snapshot.metrics.overhead_ratio() > 0.4
    {
        (
            SupervisionSignal::ExcessiveOverhead,
            7_000,
            InterventionLevel::Reframe,
            "Coordination consumed more than forty percent of measured run time",
        )
    } else {
        return None;
    };
    Some(finding(
        snapshot,
        SupervisionLoop::Medium,
        signal,
        severity,
        intervention,
        summary,
    ))
}

fn slow_finding(snapshot: &RunSupervisionSnapshot) -> Option<SupervisionFinding> {
    if snapshot.evaluation.regressions.is_empty() {
        return None;
    }
    Some(finding(
        snapshot,
        SupervisionLoop::Slow,
        SupervisionSignal::Regression,
        9_500,
        InterventionLevel::Restructure,
        "Measured regression requires organization-level review before further promotion",
    ))
}

fn finding(
    snapshot: &RunSupervisionSnapshot,
    loop_kind: SupervisionLoop,
    signal: SupervisionSignal,
    severity_basis_points: u16,
    recommended_intervention: InterventionLevel,
    summary: &str,
) -> SupervisionFinding {
    let source = snapshot.evaluation.evidence.first();
    SupervisionFinding {
        loop_kind,
        signal,
        severity_basis_points,
        recommended_intervention,
        summary: summary.into(),
        evidence: vec![SupervisionEvidence {
            method: "objective_evaluation".into(),
            artifact_sha256: source
                .and_then(|value| value.artifact_sha256.as_ref())
                .filter(|digest| valid_sha256(digest))
                .cloned(),
            event_sequence: source.and_then(|value| value.event_sequence),
            detail: format!(
                "evaluation {} outcome {:?}; {} failed instances; {} exhausted leases",
                snapshot.evaluation.id,
                snapshot.evaluation.outcome,
                snapshot.failed_instances,
                snapshot.exhausted_leases
            ),
        }],
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupervisionObservation {
    pub schema_version: u16,
    pub id: SupervisionObservationId,
    pub loop_kind: SupervisionLoop,
    pub signal: SupervisionSignal,
    pub severity_basis_points: u16,
    pub recommended_intervention: InterventionLevel,
    pub task_id: Option<TaskId>,
    pub run_id: Option<RunId>,
    pub organization_revision_id: Option<OrganizationRevisionId>,
    pub summary: String,
    pub evidence: Vec<SupervisionEvidence>,
    pub observed_at: DateTime<Utc>,
}

impl SupervisionObservation {
    /// Validates a durable observation before it can influence an experiment.
    ///
    /// # Errors
    ///
    /// Returns an invariant error for unsupported versions, unbounded values,
    /// missing scope/evidence, or malformed artifact digests.
    pub fn validate(&self) -> Result<(), ExperimentError> {
        if self.schema_version != SUPERVISION_SCHEMA_VERSION {
            return Err(ExperimentError::UnsupportedSupervisionSchema(
                self.schema_version,
            ));
        }
        if self.severity_basis_points > 10_000 {
            return Err(ExperimentError::InvalidBasisPoints);
        }
        if self.summary.trim().is_empty() || self.summary.len() > 4_096 {
            return Err(ExperimentError::InvalidText);
        }
        if self.task_id.is_none()
            && self.run_id.is_none()
            && self.organization_revision_id.is_none()
        {
            return Err(ExperimentError::MissingObservationScope);
        }
        if self.evidence.is_empty() || self.evidence.len() > 64 {
            return Err(ExperimentError::MissingEvidence);
        }
        for evidence in &self.evidence {
            if evidence.method.trim().is_empty()
                || evidence.method.len() > 128
                || evidence.detail.len() > 4_096
                || evidence
                    .artifact_sha256
                    .as_deref()
                    .is_some_and(|digest| !valid_sha256(digest))
            {
                return Err(ExperimentError::InvalidEvidence);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentMode {
    Shadow,
    Scenario,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentState {
    Draft,
    Ready,
    Running,
    AwaitingDecision,
    Promoted,
    Rejected,
    Reverted,
}

impl ExperimentState {
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Draft, Self::Ready | Self::Rejected)
                | (Self::Ready, Self::Running | Self::Rejected)
                | (Self::Running, Self::AwaitingDecision | Self::Rejected)
                | (
                    Self::AwaitingDecision,
                    Self::Promoted | Self::Rejected | Self::Reverted
                )
                | (Self::Promoted, Self::Reverted)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentDefinition {
    pub schema_version: u16,
    pub id: ExperimentId,
    pub organization_id: OrganizationId,
    pub baseline_revision_id: OrganizationRevisionId,
    pub candidate_revision_id: OrganizationRevisionId,
    pub mode: ExperimentMode,
    pub state: ExperimentState,
    pub hypothesis: String,
    pub task_classes: Vec<String>,
    pub required_benchmarks: u16,
    pub maximum_regressions: u16,
    pub minimum_confidence: ConfidenceEstimate,
    pub observation_ids: Vec<SupervisionObservationId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ExperimentDefinition {
    /// Validates immutable experiment identity and bounded promotion criteria.
    ///
    /// # Errors
    ///
    /// Returns an invariant error when the candidate cannot be evaluated safely.
    pub fn validate(&self) -> Result<(), ExperimentError> {
        if self.schema_version != EXPERIMENT_SCHEMA_VERSION {
            return Err(ExperimentError::UnsupportedExperimentSchema(
                self.schema_version,
            ));
        }
        if self.baseline_revision_id == self.candidate_revision_id {
            return Err(ExperimentError::IdenticalRevisions);
        }
        if self.hypothesis.trim().is_empty() || self.hypothesis.len() > 8_192 {
            return Err(ExperimentError::InvalidText);
        }
        if self.task_classes.is_empty()
            || self.task_classes.len() > 32
            || self
                .task_classes
                .iter()
                .any(|value| value.trim().is_empty() || value.len() > 128)
            || self.required_benchmarks == 0
            || self.required_benchmarks > 1_024
            || self.maximum_regressions > self.required_benchmarks
            || self.minimum_confidence.basis_points > 10_000
            || self.observation_ids.len() > 1_024
        {
            return Err(ExperimentError::InvalidExperimentPolicy);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShadowAuthority {
    EvaluationOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShadowExecutionReceipt {
    pub schema_version: u16,
    pub experiment_id: ExperimentId,
    pub source_task_id: TaskId,
    pub candidate_run_id: RunId,
    pub authority: ShadowAuthority,
    pub output_artifact_sha256: String,
    pub completed_at: DateTime<Utc>,
}

impl ShadowExecutionReceipt {
    /// Validates that shadow work has evidence but no representable production authority.
    ///
    /// # Errors
    ///
    /// Returns an invariant error for unsupported versions or malformed evidence.
    pub fn validate(&self) -> Result<(), ExperimentError> {
        if self.schema_version != EXPERIMENT_SCHEMA_VERSION {
            return Err(ExperimentError::UnsupportedExperimentSchema(
                self.schema_version,
            ));
        }
        if !valid_sha256(&self.output_artifact_sha256) {
            return Err(ExperimentError::InvalidEvidence);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkArmResult {
    pub organization_revision_id: OrganizationRevisionId,
    pub run_id: RunId,
    pub evaluation_id: EvaluationId,
    pub outcome: EvaluationOutcome,
    pub confidence: ConfidenceEstimate,
    pub metrics: OrganizationalMetrics,
    pub evidence_sha256: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExperimentBenchmark {
    pub schema_version: u16,
    pub id: ExperimentBenchmarkId,
    pub experiment_id: ExperimentId,
    pub scenario_key: String,
    pub baseline: BenchmarkArmResult,
    pub candidate: BenchmarkArmResult,
    pub regressions: Vec<String>,
    pub measured_at: DateTime<Utc>,
}

impl ExperimentBenchmark {
    /// Validates paired, independently evidenced benchmark results.
    ///
    /// # Errors
    ///
    /// Returns an invariant error for duplicated arms or missing evidence.
    pub fn validate(&self) -> Result<(), ExperimentError> {
        if self.schema_version != EXPERIMENT_SCHEMA_VERSION {
            return Err(ExperimentError::UnsupportedExperimentSchema(
                self.schema_version,
            ));
        }
        if self.scenario_key.trim().is_empty()
            || self.scenario_key.len() > 256
            || self.baseline.organization_revision_id == self.candidate.organization_revision_id
            || self.baseline.run_id == self.candidate.run_id
            || self.regressions.len() > 128
        {
            return Err(ExperimentError::InvalidBenchmark);
        }
        for arm in [&self.baseline, &self.candidate] {
            if arm.confidence.basis_points > 10_000
                || arm.evidence_sha256.is_empty()
                || arm.evidence_sha256.len() > 64
                || arm
                    .evidence_sha256
                    .iter()
                    .any(|digest| !valid_sha256(digest))
                || !arm.metrics.cost_usd.is_finite()
                || arm.metrics.cost_usd < 0.0
            {
                return Err(ExperimentError::InvalidBenchmark);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentDecisionKind {
    Promote,
    Reject,
    Revert,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProspectiveAnalysis {
    pub expected_benefits: Vec<String>,
    pub expected_risks: Vec<String>,
    pub historical_evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentDecision {
    pub schema_version: u16,
    pub id: ExperimentDecisionId,
    pub experiment_id: ExperimentId,
    pub kind: ExperimentDecisionKind,
    pub from_revision_id: OrganizationRevisionId,
    pub to_revision_id: OrganizationRevisionId,
    pub benchmark_ids: Vec<ExperimentBenchmarkId>,
    #[serde(default)]
    pub trigger_evaluation_ids: Vec<EvaluationId>,
    pub prospective_analysis: ProspectiveAnalysis,
    pub reason: String,
    pub decided_at: DateTime<Utc>,
}

impl ExperimentDecision {
    /// Validates evidence-linked promotion or rollback intent before persistence.
    ///
    /// # Errors
    ///
    /// Returns an invariant error when evidence, direction, or rationale is missing.
    pub fn validate(&self) -> Result<(), ExperimentError> {
        if self.schema_version != EXPERIMENT_SCHEMA_VERSION {
            return Err(ExperimentError::UnsupportedExperimentSchema(
                self.schema_version,
            ));
        }
        if self.from_revision_id == self.to_revision_id
            || self.benchmark_ids.is_empty()
            || self.benchmark_ids.len() > 1_024
            || self.trigger_evaluation_ids.len() > 1_024
            || (self.kind == ExperimentDecisionKind::Revert
                && self.trigger_evaluation_ids.is_empty())
            || (self.kind != ExperimentDecisionKind::Revert
                && !self.trigger_evaluation_ids.is_empty())
            || self.reason.trim().is_empty()
            || self.reason.len() > 8_192
            || self.prospective_analysis.expected_benefits.len() > 128
            || self.prospective_analysis.expected_risks.len() > 128
            || self.prospective_analysis.historical_evidence.len() > 128
        {
            return Err(ExperimentError::InvalidDecision);
        }
        Ok(())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExperimentError {
    #[error("unsupported supervision schema version {0}")]
    UnsupportedSupervisionSchema(u16),
    #[error("unsupported experiment schema version {0}")]
    UnsupportedExperimentSchema(u16),
    #[error("basis points must be between 0 and 10000")]
    InvalidBasisPoints,
    #[error("text is empty or exceeds its bound")]
    InvalidText,
    #[error("a supervision observation requires task, run, or organization scope")]
    MissingObservationScope,
    #[error("objective evidence is required")]
    MissingEvidence,
    #[error("evidence is malformed or exceeds its bound")]
    InvalidEvidence,
    #[error("baseline and candidate revisions must differ")]
    IdenticalRevisions,
    #[error("experiment policy is inconsistent or out of bounds")]
    InvalidExperimentPolicy,
    #[error("benchmark arms or evidence are inconsistent")]
    InvalidBenchmark,
    #[error("experiment decision is missing evidence, direction, or rationale")]
    InvalidDecision,
    #[error("automatic candidate proposal is inconsistent with its slow-loop evidence")]
    InvalidCandidateProposal,
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn confidence(basis_points: u16) -> ConfidenceEstimate {
        ConfidenceEstimate::new(basis_points, 10).expect("confidence")
    }

    fn arm(revision_id: OrganizationRevisionId) -> BenchmarkArmResult {
        BenchmarkArmResult {
            organization_revision_id: revision_id,
            run_id: RunId::new(),
            evaluation_id: EvaluationId::new(),
            outcome: EvaluationOutcome::Passed,
            confidence: confidence(9_000),
            metrics: OrganizationalMetrics::default(),
            evidence_sha256: vec!["a".repeat(64)],
        }
    }

    #[test]
    fn graduated_intervention_and_experiment_transitions_are_explicit() {
        assert!(InterventionLevel::Observe < InterventionLevel::Restructure);
        assert!(ExperimentState::Draft.can_transition_to(ExperimentState::Ready));
        assert!(ExperimentState::Promoted.can_transition_to(ExperimentState::Reverted));
        assert!(!ExperimentState::Running.can_transition_to(ExperimentState::Promoted));
        assert!(!ExperimentState::Rejected.can_transition_to(ExperimentState::Running));
    }

    #[test]
    fn shadow_receipt_has_no_authoritative_output_state() {
        let receipt = ShadowExecutionReceipt {
            schema_version: EXPERIMENT_SCHEMA_VERSION,
            experiment_id: ExperimentId::new(),
            source_task_id: TaskId::new(),
            candidate_run_id: RunId::new(),
            authority: ShadowAuthority::EvaluationOnly,
            output_artifact_sha256: "b".repeat(64),
            completed_at: Utc::now(),
        };
        receipt.validate().expect("valid shadow receipt");
        let serialized = serde_json::to_value(receipt).expect("serialize receipt");
        assert_eq!(serialized["authority"], "evaluation_only");
        assert!(serialized.get("authoritative_output").is_none());
    }

    #[test]
    fn benchmark_requires_distinct_evidenced_arms() {
        let revision = OrganizationRevisionId::new();
        let mut benchmark = ExperimentBenchmark {
            schema_version: EXPERIMENT_SCHEMA_VERSION,
            id: ExperimentBenchmarkId::new(),
            experiment_id: ExperimentId::new(),
            scenario_key: "coding/build".into(),
            baseline: arm(revision),
            candidate: arm(OrganizationRevisionId::new()),
            regressions: Vec::new(),
            measured_at: Utc::now(),
        };
        benchmark.validate().expect("valid benchmark");
        benchmark.candidate.organization_revision_id = revision;
        assert_eq!(benchmark.validate(), Err(ExperimentError::InvalidBenchmark));
    }

    #[test]
    fn promotion_requires_measured_evidence_and_distinct_revisions() {
        let revision = OrganizationRevisionId::new();
        let mut decision = ExperimentDecision {
            schema_version: EXPERIMENT_SCHEMA_VERSION,
            id: ExperimentDecisionId::new(),
            experiment_id: ExperimentId::new(),
            kind: ExperimentDecisionKind::Promote,
            from_revision_id: revision,
            to_revision_id: OrganizationRevisionId::new(),
            benchmark_ids: vec![ExperimentBenchmarkId::new()],
            trigger_evaluation_ids: Vec::new(),
            prospective_analysis: ProspectiveAnalysis {
                expected_benefits: vec!["fewer retries".into()],
                expected_risks: vec!["higher token cost".into()],
                historical_evidence: vec!["coding/build benchmark".into()],
            },
            reason: "candidate passed the required scenario set".into(),
            decided_at: Utc::now(),
        };
        decision.validate().expect("valid decision");
        decision.to_revision_id = revision;
        assert_eq!(decision.validate(), Err(ExperimentError::InvalidDecision));
    }

    #[test]
    fn durable_run_evidence_drives_graduated_deterministic_findings() {
        let task_id = TaskId::new();
        let run_id = RunId::new();
        let snapshot = RunSupervisionSnapshot {
            task_id,
            run_id,
            organization_revision_id: OrganizationRevisionId::new(),
            run_state: AgentRunState::Failed,
            metrics: OrganizationalMetrics {
                useful_work_seconds: 30,
                coordination_seconds: 45,
                ..OrganizationalMetrics::default()
            },
            failed_instances: 1,
            exhausted_leases: 0,
            evaluation: EvaluationReport {
                schema_version: crate::AGENT_RUN_SCHEMA_VERSION,
                id: EvaluationId::new(),
                run_id,
                task_id,
                evaluator_instance_id: None,
                conditions: Vec::new(),
                outcome: EvaluationOutcome::Failed,
                confidence: confidence(9_000),
                regressions: vec!["seeded accessibility regression".into()],
                evidence: vec![crate::EvaluationEvidence {
                    method: "fixture".into(),
                    artifact_sha256: Some("d".repeat(64)),
                    event_sequence: Some(42),
                    detail: "seeded objective evidence".into(),
                }],
                evaluated_at: Utc::now(),
            },
        };
        let first = SupervisionAnalyzer::analyze(&snapshot);
        let second = SupervisionAnalyzer::analyze(&snapshot);
        assert_eq!(first, second);
        assert_eq!(first.len(), 3);
        assert_eq!(first[0].loop_kind, SupervisionLoop::Fast);
        assert_eq!(first[0].signal, SupervisionSignal::ToolFailure);
        assert_eq!(first[1].loop_kind, SupervisionLoop::Medium);
        assert_eq!(first[1].signal, SupervisionSignal::Regression);
        assert_eq!(first[2].loop_kind, SupervisionLoop::Slow);
        assert_eq!(
            first[2].recommended_intervention,
            InterventionLevel::Restructure
        );
        assert_eq!(first[2].evidence[0].event_sequence, Some(42));
    }

    #[test]
    fn slow_regression_produces_a_bounded_inactive_candidate() {
        let now = Utc::now();
        let baseline = crate::OrganizationGenome::consumer_default(now);
        let observation = SupervisionObservation {
            schema_version: SUPERVISION_SCHEMA_VERSION,
            id: SupervisionObservationId::new(),
            loop_kind: SupervisionLoop::Slow,
            signal: SupervisionSignal::Regression,
            severity_basis_points: 9_500,
            recommended_intervention: InterventionLevel::Restructure,
            task_id: Some(TaskId::new()),
            run_id: Some(RunId::new()),
            organization_revision_id: Some(baseline.revision_id),
            summary: "seeded regression".into(),
            evidence: vec![SupervisionEvidence {
                method: "fixture".into(),
                artifact_sha256: Some("a".repeat(64)),
                event_sequence: Some(1),
                detail: "objective regression".into(),
            }],
            observed_at: now,
        };
        let revision_id = OrganizationRevisionId::new();
        let candidate = MeasuredCandidateDesigner::design(
            &baseline,
            &observation,
            revision_id,
            2,
            now + chrono::Duration::seconds(1),
        )
        .expect("candidate");
        assert_eq!(candidate.revision_id, revision_id);
        assert_eq!(candidate.parent_revision_id, Some(baseline.revision_id));
        assert!(candidate.evaluation.require_independent_evaluation);
        assert!(candidate.evaluation.minimum_confidence >= 0.90);
        assert!(
            candidate
                .roles
                .iter()
                .filter(|role| matches!(
                    role.kind,
                    crate::RoleKind::Coder | crate::RoleKind::Evaluator
                ))
                .all(|role| role.model_policy.max_escalation_tier == 3)
        );
        assert!(!baseline.evaluation.require_independent_evaluation);
    }
}
