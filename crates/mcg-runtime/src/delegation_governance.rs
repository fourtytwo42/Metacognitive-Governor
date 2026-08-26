//! Deterministic governance for conversation coding delegations.
//!
//! This module deliberately contains no model calls. It compiles typed contracts, performs
//! preflight checks, classifies failures, records bounded context receipts, and produces outcome
//! telemetry that the dormant metacognitive governor may later evaluate in shadow mode.

use chrono::{DateTime, Utc};
use mcg_domain::{
    AgentContextReceipt, AgentDelegation, AgentDelegationState, CodingDelegationBrief,
    DelegationCompletionContract, DelegationFailureClass, DelegationGovernance,
    DelegationOutcomeMetrics, DelegationPreflightReceipt, DelegationRecoveryDirective,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::Path};

pub const DEFAULT_CODER_CONTEXT_BUDGET: u64 = 12_000;
pub const DEFAULT_MAX_ATTEMPTS: u32 = 2;

/// Returns a bounded brief only when deterministic routing can prove that one Coder is the
/// appropriate shape. Ambiguous, architectural, research-heavy, or explicitly parallel work
/// continues through the Coordinator model.
#[must_use]
pub fn deterministic_single_coder_brief(request: &str) -> Option<CodingDelegationBrief> {
    let trimmed = request.trim();
    if trimmed.is_empty() || trimmed.len() > 6_000 {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    let coordinator_signals = [
        "multiple agents",
        "parallel agents",
        "split the work",
        "research online",
        "plan out",
        "architecture",
        "migration strategy",
        "compare approaches",
        "investigate first",
        "acceptance criteria",
        "requirements:",
        "uat",
        "user acceptance test",
        "acceptance test",
        "/browser",
        "in-app browser",
        "test in the browser",
        "browser testing",
        "visual testing",
        "accessibility testing",
    ];
    if coordinator_signals
        .iter()
        .any(|signal| lower.contains(signal))
    {
        return None;
    }
    let meaningful_lines = trimmed
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    let sentence_count = trimmed
        .split(['.', '!', '?'])
        .filter(|sentence| sentence.split_whitespace().count() >= 3)
        .count();
    let list_markers = trimmed
        .lines()
        .filter(|line| {
            let line = line.trim_start();
            line.starts_with("- ")
                || line.starts_with("* ")
                || line
                    .split_once('.')
                    .is_some_and(|(head, _)| head.chars().all(|ch| ch.is_ascii_digit()))
        })
        .count();
    if meaningful_lines > 8 || sentence_count > 4 || list_markers > 2 {
        return None;
    }
    Some(CodingDelegationBrief {
        objective: trimmed.to_owned(),
        scope: Vec::new(),
        constraints: vec![
            "Keep changes within the linked project and do not expand authority".into(),
            "Stop and report a blocker when requirements become ambiguous or unsafe".into(),
        ],
        success_conditions: vec![
            format!("Satisfy the complete owner request without dropping requirements: {trimmed}"),
            "Run relevant verification".into(),
            "Provide a requirement-to-evidence matrix covering every pinned success condition"
                .into(),
        ],
        context_references: Vec::new(),
    })
}

#[derive(Debug, Clone, Copy)]
pub struct PreflightInput<'a> {
    pub project_root: &'a Path,
    pub project_is_git: bool,
    pub parallel_limit: usize,
    pub active_coders: usize,
}

#[must_use]
pub fn compile_governance(brief: &CodingDelegationBrief) -> DelegationGovernance {
    let objective_sha256 = sha256(brief.objective.trim().as_bytes());
    let required_checks = brief
        .success_conditions
        .iter()
        .filter(|item| !item.trim().is_empty())
        .cloned()
        .collect();
    let reserved_scopes = normalized_scopes(&brief.scope);
    DelegationGovernance {
        contract: DelegationCompletionContract {
            objective_sha256,
            allowed_scopes: reserved_scopes.clone(),
            required_checks,
            required_evidence: vec![
                "changed files".into(),
                "checks performed".into(),
                "requirement-to-evidence matrix".into(),
            ],
            require_integration_ready: true,
            context_token_budget: DEFAULT_CODER_CONTEXT_BUDGET,
            max_attempts: DEFAULT_MAX_ATTEMPTS,
        },
        reserved_scopes,
        outcome: DelegationOutcomeMetrics {
            attempts: 1,
            ..DelegationOutcomeMetrics::default()
        },
        ..DelegationGovernance::default()
    }
}

#[must_use]
pub fn run_preflight(
    governance: &DelegationGovernance,
    input: PreflightInput<'_>,
    active: &[AgentDelegation],
    now: DateTime<Utc>,
) -> DelegationPreflightReceipt {
    let mut checks = Vec::new();
    let mut blockers = Vec::new();
    if input.project_root.is_dir() {
        checks.push("project root exists and is a directory".into());
    } else {
        blockers.push("project root is missing or is not a directory".into());
    }
    if governance.contract.objective_sha256.is_empty() {
        blockers.push("delegation objective contract is empty".into());
    } else {
        checks.push("objective is pinned by SHA-256".into());
    }
    if input.parallel_limit == 0 || input.active_coders >= input.parallel_limit {
        blockers.push(format!(
            "active Coding Agents ({}) reached the organization limit ({})",
            input.active_coders, input.parallel_limit
        ));
    } else {
        checks.push(format!(
            "organization capacity available ({}/{})",
            input.active_coders, input.parallel_limit
        ));
    }
    if !input.project_is_git && input.active_coders > 0 {
        blockers
            .push("non-Git projects serialize coding work and already have an active Coder".into());
    } else if input.project_is_git {
        checks.push("Git isolation is available".into());
    } else {
        checks.push("direct-workspace serialization is available".into());
    }
    let requested = &governance.reserved_scopes;
    if input.active_coders > 0 && requested.is_empty() {
        blockers.push("parallel work requires explicit non-overlapping scope reservations".into());
    }
    for delegation in active.iter().filter(|item| !item.state.is_terminal()) {
        let occupied = &delegation.governance.reserved_scopes;
        if requested
            .iter()
            .any(|left| occupied.iter().any(|right| scopes_overlap(left, right)))
        {
            blockers.push(format!(
                "requested scope overlaps active delegation {}",
                delegation.id
            ));
        }
    }
    if blockers.is_empty() {
        checks.push("no declared scope reservation conflicts".into());
    }
    let canonical = serde_json::to_vec(&(checks.clone(), blockers.clone())).unwrap_or_default();
    DelegationPreflightReceipt {
        passed: blockers.is_empty(),
        checks,
        blockers,
        receipt_sha256: Some(sha256(&canonical)),
        completed_at: Some(now),
    }
}

#[must_use]
pub fn context_receipt(
    source_references: &[String],
    context: &str,
    token_budget: u64,
    now: DateTime<Utc>,
) -> AgentContextReceipt {
    let approximate_tokens = context.chars().count().div_ceil(4) as u64;
    AgentContextReceipt {
        source_count: u32::try_from(source_references.len()).unwrap_or(u32::MAX),
        approximate_tokens: approximate_tokens.min(token_budget),
        token_budget,
        truncated: approximate_tokens > token_budget,
        source_sha256: sha256(context.as_bytes()),
        created_at: Some(now),
    }
}

#[must_use]
pub fn classify_failure(message: &str) -> (DelegationFailureClass, DelegationRecoveryDirective) {
    let message = message.to_ascii_lowercase();
    if message.contains("cancel") {
        (
            DelegationFailureClass::Cancelled,
            DelegationRecoveryDirective::DoNotRetry,
        )
    } else if message.contains("permission")
        || message.contains("denied")
        || message.contains("authority")
    {
        (
            DelegationFailureClass::PolicyDenied,
            DelegationRecoveryDirective::AwaitOwner,
        )
    } else if message.contains("preflight")
        || message.contains("project root")
        || message.contains("requires a git")
    {
        (
            DelegationFailureClass::Precondition,
            DelegationRecoveryDirective::Replan,
        )
    } else if message.contains("conflict")
        || message.contains("stale")
        || message.contains("head changed")
    {
        (
            DelegationFailureClass::Conflict,
            DelegationRecoveryDirective::RetainForReview,
        )
    } else if message.contains("report_result")
        || message.contains("without an authenticated report")
    {
        (
            DelegationFailureClass::AgentOmission,
            DelegationRecoveryDirective::RetrySameThread,
        )
    } else if message.contains("check")
        || message.contains("validation")
        || message.contains("evidence")
    {
        (
            DelegationFailureClass::Verification,
            DelegationRecoveryDirective::RetryNewDelegation,
        )
    } else if message.contains("site")
        || message.contains("preview")
        || message.contains("delivery")
        || message.contains("deploy")
    {
        (
            DelegationFailureClass::Delivery,
            DelegationRecoveryDirective::RetryNewDelegation,
        )
    } else if message.contains("timeout")
        || message.contains("temporar")
        || message.contains("unavailable")
    {
        (
            DelegationFailureClass::TransientTool,
            DelegationRecoveryDirective::RetrySameThread,
        )
    } else {
        (
            DelegationFailureClass::Uncertain,
            DelegationRecoveryDirective::RetainForReview,
        )
    }
}

pub fn finalize_outcome(delegation: &mut AgentDelegation, now: DateTime<Utc>) {
    delegation.governance.outcome.latency_ms = now
        .signed_duration_since(delegation.created_at)
        .num_milliseconds()
        .max(0) as u64;
    delegation.governance.outcome.changed_file_count =
        u32::try_from(delegation.changed_files.len()).unwrap_or(u32::MAX);
    delegation.governance.outcome.check_count =
        u32::try_from(delegation.checks.len()).unwrap_or(u32::MAX);
    delegation.governance.outcome.approximate_tokens = delegation
        .governance
        .context_receipts
        .iter()
        .map(|receipt| receipt.approximate_tokens)
        .sum();
    delegation.governance.outcome.successful = delegation.state == AgentDelegationState::Completed
        || (delegation.state == AgentDelegationState::Reported
            && delegation.integration_ready
            && delegation.error.is_none());
    if let Some(error) = delegation.error.as_deref() {
        let (failure_class, recovery) = classify_failure(error);
        delegation.governance.failure_class = failure_class;
        delegation.governance.recovery = recovery;
    } else if delegation.state == AgentDelegationState::Cancelled {
        delegation.governance.failure_class = DelegationFailureClass::Cancelled;
        delegation.governance.recovery = DelegationRecoveryDirective::DoNotRetry;
    }
}

fn normalized_scopes(scopes: &[String]) -> Vec<String> {
    scopes
        .iter()
        .map(|scope| {
            scope
                .trim()
                .replace('\\', "/")
                .trim_start_matches("./")
                .to_owned()
        })
        .filter(|scope| !scope.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn scopes_overlap(left: &str, right: &str) -> bool {
    left == right
        || left
            .strip_prefix(right)
            .is_some_and(|tail| tail.starts_with('/'))
        || right
            .strip_prefix(left)
            .is_some_and(|tail| tail.starts_with('/'))
}

fn sha256(input: &[u8]) -> String {
    format!("{:x}", Sha256::digest(input))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcg_domain::{
        AgentDelegationId, ConversationAgentSessionId, ConversationId, OrganizationRevisionId,
        ProjectId,
    };
    use tempfile::tempdir;

    #[test]
    fn preflight_rejects_overlapping_declared_scope() {
        let root = tempdir().expect("tempdir");
        let brief = CodingDelegationBrief {
            objective: "Change the desktop UI".into(),
            scope: vec!["apps/desktop".into()],
            constraints: vec![],
            success_conditions: vec!["typecheck".into()],
            context_references: vec![],
        };
        let mut occupied = minimal_delegation();
        occupied.governance = compile_governance(&brief);
        let receipt = run_preflight(
            &compile_governance(&brief),
            PreflightInput {
                project_root: root.path(),
                project_is_git: true,
                parallel_limit: 8,
                active_coders: 1,
            },
            &[occupied],
            Utc::now(),
        );
        assert!(!receipt.passed);
        assert!(
            receipt
                .blockers
                .iter()
                .any(|item| item.contains("overlaps"))
        );
    }

    #[test]
    fn context_receipt_enforces_budget_without_losing_source_hash() {
        let receipt = context_receipt(&["message:1".into()], &"x".repeat(800), 100, Utc::now());
        assert!(receipt.truncated);
        assert_eq!(receipt.approximate_tokens, 100);
        assert_eq!(receipt.source_sha256.len(), 64);
    }

    #[test]
    fn failures_receive_deterministic_recovery_directives() {
        assert_eq!(
            classify_failure("live repository HEAD changed: conflict"),
            (
                DelegationFailureClass::Conflict,
                DelegationRecoveryDirective::RetainForReview
            )
        );
        assert_eq!(
            classify_failure("Coder finished without an authenticated report_result call"),
            (
                DelegationFailureClass::AgentOmission,
                DelegationRecoveryDirective::RetrySameThread
            )
        );
    }

    #[test]
    fn adaptive_router_bypasses_only_obvious_single_coder_work() {
        assert!(
            deterministic_single_coder_brief("Build a simple landing page in this project")
                .is_some()
        );
        assert!(
            deterministic_single_coder_brief(
                "Research online and plan out an architecture using multiple agents"
            )
            .is_none()
        );
        assert!(
            deterministic_single_coder_brief(
                "Build a React board. It must filter by status. It must filter by priority. It must clear filters. It must support full-field editing. It must persist after reload."
            )
            .is_none(),
            "multi-requirement work needs Coordinator requirement preservation"
        );
        assert!(
            deterministic_single_coder_brief("Can you do some UAT testing in the /browser?")
                .is_none(),
            "browser UAT must reach the Coordinator so it can assign a governed Tester"
        );
        assert!(
            deterministic_single_coder_brief("Use the in-app browser for accessibility testing")
                .is_none(),
            "browser verification must never be routed directly to a Coder"
        );
        let brief = deterministic_single_coder_brief("Fix the typo in README.md")
            .expect("simple bounded work should bypass planning");
        assert!(brief.success_conditions[0].contains("Fix the typo in README.md"));
        assert!(
            compile_governance(&brief)
                .contract
                .required_evidence
                .iter()
                .any(|item| item == "requirement-to-evidence matrix")
        );
    }

    fn minimal_delegation() -> AgentDelegation {
        let now = Utc::now();
        AgentDelegation {
            schema_version: 1,
            id: AgentDelegationId::new(),
            conversation_id: ConversationId::new(),
            source_session_id: ConversationAgentSessionId::new(),
            coder_session_id: None,
            project_id: ProjectId::new(),
            organization_revision_id: OrganizationRevisionId::new(),
            coordination_batch_id: None,
            origin_turn_id: None,
            revision: 1,
            prompt_revision: 1,
            permission_mode: None,
            state: AgentDelegationState::Running,
            brief: CodingDelegationBrief {
                objective: "test".into(),
                scope: vec![],
                constraints: vec![],
                success_conditions: vec![],
                context_references: vec![],
            },
            governance: DelegationGovernance::default(),
            task_id: None,
            run_id: None,
            agent_instance_id: None,
            agent_lease_id: None,
            worktree_id: None,
            retry_of_id: None,
            idempotency_key: None,
            summary: None,
            changed_files: vec![],
            checks: vec![],
            evidence: vec![],
            unresolved_issues: vec![],
            integration_ready: false,
            report_sha256: None,
            coordinator_decision: None,
            coordinator_rationale: None,
            coordinator_reviewed_at: None,
            accepted_codex_item_id: None,
            error: None,
            created_at: now,
            updated_at: now,
        }
    }
}
