use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use mcg_domain::{RoleId, RunId, TaskBudget, TaskEnvelope, WorkItemId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    OrganizationError, OrganizationGenome, RoleKind, SuccessCondition, WorkItem, WorkItemState,
    WorkPlan,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskComplexity {
    Atomic,
    Compound,
}

/// Structured classification output. It is advisory work intent, never a tool
/// capability grant or an authorization boundary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanningRequest {
    pub run_id: RunId,
    pub task: TaskEnvelope,
    pub complexity: TaskComplexity,
    pub work: Vec<PlannedWork>,
    pub require_independent_evaluation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedWork {
    pub id: WorkItemId,
    pub goal: String,
    pub role: RoleKind,
    pub dependencies: Vec<WorkItemId>,
    pub success_conditions: Vec<SuccessCondition>,
    pub risk_notes: Vec<String>,
    pub budget_weight: u32,
    pub estimated_useful_seconds: u64,
}

impl PlannedWork {
    #[must_use]
    pub fn atomic(
        goal: impl Into<String>,
        role: RoleKind,
        success_conditions: Vec<SuccessCondition>,
    ) -> Self {
        Self {
            id: WorkItemId::new(),
            goal: goal.into(),
            role,
            dependencies: Vec::new(),
            success_conditions,
            risk_notes: Vec::new(),
            budget_weight: 1,
            estimated_useful_seconds: 60,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrganizationSelection {
    pub roles: Vec<RoleId>,
    pub role_by_kind: BTreeMap<RoleKind, RoleId>,
    pub reason: String,
    pub estimated_overhead_ratio: f64,
}

#[derive(Debug, Default)]
pub struct MinimalOrganizationSelector;

impl MinimalOrganizationSelector {
    /// Selects exactly one persistent role for each declared work capability,
    /// adding an Executive only for compound/dependency-managed work.
    ///
    /// # Errors
    ///
    /// Returns an invariant, missing-role, limit, or overhead error when the
    /// request cannot be safely represented by the selected genome.
    pub fn select(
        &self,
        genome: &OrganizationGenome,
        request: &PlanningRequest,
    ) -> Result<OrganizationSelection, PlanningError> {
        genome.validate()?;
        validate_request(request)?;

        let independent_evaluation = request.require_independent_evaluation
            || genome.evaluation.require_independent_evaluation;
        let needs_executive = request.complexity == TaskComplexity::Compound
            || request.work.len() > 1
            || request
                .work
                .iter()
                .any(|work| !work.dependencies.is_empty());
        let mut kinds = request
            .work
            .iter()
            .map(|work| work.role)
            .collect::<BTreeSet<_>>();
        if independent_evaluation {
            kinds.insert(RoleKind::Evaluator);
        }
        if needs_executive {
            kinds.insert(RoleKind::Executive);
        }

        let mut role_by_kind = BTreeMap::new();
        for kind in kinds {
            let role = genome
                .roles
                .iter()
                .filter(|role| role.kind == kind)
                .min_by_key(|role| role.id)
                .ok_or(PlanningError::MissingRole(kind))?;
            role_by_kind.insert(kind, role.id);
        }
        if role_by_kind.len() > usize::from(genome.policy.max_total_agents) {
            return Err(PlanningError::AgentLimitExceeded);
        }

        let useful_seconds = request
            .work
            .iter()
            .map(|work| work.estimated_useful_seconds)
            .sum::<u64>();
        let dependency_count = request
            .work
            .iter()
            .map(|work| work.dependencies.len() as u64)
            .sum::<u64>();
        let coordination_seconds = if needs_executive {
            5_u64
                .saturating_mul(request.work.len() as u64)
                .saturating_add(2_u64.saturating_mul(dependency_count))
        } else {
            0
        };
        let estimated_overhead_ratio = ratio(coordination_seconds, useful_seconds);
        if estimated_overhead_ratio > genome.policy.max_organizational_overhead_ratio {
            return Err(PlanningError::OrganizationalOverheadExceeded {
                estimated: estimated_overhead_ratio,
                maximum: genome.policy.max_organizational_overhead_ratio,
            });
        }

        let roles = role_by_kind.values().copied().collect();
        let reason = if needs_executive {
            format!(
                "compound work selected {} capability roles with dependency coordination",
                role_by_kind.len()
            )
        } else {
            "atomic work dispatched directly to one capability role".into()
        };
        Ok(OrganizationSelection {
            roles,
            role_by_kind,
            reason,
            estimated_overhead_ratio,
        })
    }
}

#[derive(Debug, Default)]
pub struct ExecutivePlanner {
    selector: MinimalOrganizationSelector,
}

impl ExecutivePlanner {
    /// Produces a dependency-aware plan whose aggregate child budgets never
    /// exceed the task envelope budget.
    ///
    /// # Errors
    ///
    /// Returns a planning error when selection or plan validation fails.
    pub fn plan(
        &self,
        genome: &OrganizationGenome,
        request: &PlanningRequest,
        created_at: DateTime<Utc>,
    ) -> Result<WorkPlan, PlanningError> {
        let selection = self.selector.select(genome, request)?;
        let independent_evaluation = request.require_independent_evaluation
            || genome.evaluation.require_independent_evaluation;
        let mut work = request.work.clone();
        if independent_evaluation && !work.iter().any(|item| item.role == RoleKind::Evaluator) {
            let dependencies = work.iter().map(|item| item.id).collect();
            let conditions = work
                .iter()
                .flat_map(|item| item.success_conditions.iter().cloned())
                .collect::<Vec<_>>();
            work.push(PlannedWork {
                id: WorkItemId::new(),
                goal: format!("Independently evaluate: {}", request.task.goal),
                role: RoleKind::Evaluator,
                dependencies,
                success_conditions: if conditions.is_empty() {
                    vec![SuccessCondition::Custom {
                        description: format!("Verify the completed goal: {}", request.task.goal),
                    }]
                } else {
                    conditions
                },
                risk_notes: vec!["Evaluation must use evidence independent of self-report.".into()],
                budget_weight: 1,
                estimated_useful_seconds: 60,
            });
        }
        let budgets = allocate_budgets(
            &request.task.budget,
            &work
                .iter()
                .map(|item| item.budget_weight)
                .collect::<Vec<_>>(),
        );
        let items = work
            .into_iter()
            .zip(budgets)
            .map(|(item, budget)| WorkItem {
                id: item.id,
                goal: item.goal,
                assigned_role: selection.role_by_kind[&item.role],
                state: if item.dependencies.is_empty() {
                    WorkItemState::Ready
                } else {
                    WorkItemState::Pending
                },
                dependencies: item.dependencies,
                success_conditions: item.success_conditions,
                budget,
                risk_notes: item.risk_notes,
            })
            .collect();
        let plan = WorkPlan {
            run_id: request.run_id,
            task_id: request.task.id,
            organization_revision_id: genome.revision_id,
            items,
            selected_roles: selection.roles,
            selection_reason: selection.reason,
            estimated_organizational_overhead: selection.estimated_overhead_ratio,
            created_at,
        };
        plan.validate(genome)?;
        Ok(plan)
    }
}

impl WorkPlan {
    /// Validates plan identity, selected roles, work completeness, and its
    /// dependency graph against a specific organization revision.
    ///
    /// # Errors
    ///
    /// Returns the first violated organization or plan invariant.
    pub fn validate(&self, genome: &OrganizationGenome) -> Result<(), PlanningError> {
        genome.validate()?;
        if self.organization_revision_id != genome.revision_id {
            return Err(PlanningError::OrganizationRevisionMismatch);
        }
        if self.items.is_empty() {
            return Err(PlanningError::EmptyWorkPlan);
        }
        if !self.estimated_organizational_overhead.is_finite()
            || !(0.0..=1.0).contains(&self.estimated_organizational_overhead)
            || self.estimated_organizational_overhead
                > genome.policy.max_organizational_overhead_ratio
        {
            return Err(PlanningError::InvalidEstimate);
        }
        let selected = self.selected_roles.iter().copied().collect::<BTreeSet<_>>();
        if selected.len() != self.selected_roles.len() {
            return Err(PlanningError::DuplicateSelectedRole);
        }
        if selected.len() > usize::from(genome.policy.max_total_agents) {
            return Err(PlanningError::AgentLimitExceeded);
        }
        let known_roles = genome
            .roles
            .iter()
            .map(|role| role.id)
            .collect::<BTreeSet<_>>();
        if !selected.is_subset(&known_roles) {
            return Err(PlanningError::UnknownSelectedRole);
        }
        let item_ids = self
            .items
            .iter()
            .map(|item| item.id)
            .collect::<BTreeSet<_>>();
        if item_ids.len() != self.items.len() {
            return Err(PlanningError::DuplicateWorkItem);
        }
        for item in &self.items {
            if item.goal.trim().is_empty() || item.success_conditions.is_empty() {
                return Err(PlanningError::IncompleteWorkItem);
            }
            if !selected.contains(&item.assigned_role) {
                return Err(PlanningError::UnselectedAssignedRole);
            }
            if item
                .dependencies
                .iter()
                .any(|dependency| *dependency == item.id || !item_ids.contains(dependency))
            {
                return Err(PlanningError::InvalidDependency);
            }
        }
        if has_work_cycle(&self.items) {
            return Err(PlanningError::DependencyCycle);
        }
        let used = self
            .items
            .iter()
            .map(|item| item.assigned_role)
            .collect::<BTreeSet<_>>();
        let executive = genome
            .roles
            .iter()
            .find(|role| role.kind == RoleKind::Executive)
            .map(|role| role.id);
        let needs_executive =
            self.items.len() > 1 || self.items.iter().any(|item| !item.dependencies.is_empty());
        if genome.policy.prefer_minimal_organization
            && selected
                .iter()
                .any(|role| !(used.contains(role) || needs_executive && Some(*role) == executive))
        {
            return Err(PlanningError::UnnecessarySelectedRole);
        }
        if genome.evaluation.require_independent_evaluation {
            let evaluators = genome
                .roles
                .iter()
                .filter(|role| role.kind == RoleKind::Evaluator)
                .map(|role| role.id)
                .collect::<BTreeSet<_>>();
            if !self
                .items
                .iter()
                .any(|item| evaluators.contains(&item.assigned_role))
            {
                return Err(PlanningError::RequiredEvaluationMissing);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum PlanningError {
    #[error(transparent)]
    Organization(#[from] OrganizationError),
    #[error("planning request must contain bounded, complete work")]
    InvalidRequest,
    #[error("organization has no role for {0:?}")]
    MissingRole(RoleKind),
    #[error("selected organization exceeds the configured agent limit")]
    AgentLimitExceeded,
    #[error("estimated organizational overhead {estimated:.3} exceeds maximum {maximum:.3}")]
    OrganizationalOverheadExceeded { estimated: f64, maximum: f64 },
    #[error("work plan organization revision does not match the selected genome")]
    OrganizationRevisionMismatch,
    #[error("work plan must contain at least one work item")]
    EmptyWorkPlan,
    #[error("work plan contains an invalid overhead estimate")]
    InvalidEstimate,
    #[error("work plan selected the same role more than once")]
    DuplicateSelectedRole,
    #[error("work plan selected an unknown role")]
    UnknownSelectedRole,
    #[error("work plan contains duplicate work item identities")]
    DuplicateWorkItem,
    #[error("work item must have a goal and at least one success condition")]
    IncompleteWorkItem,
    #[error("work item is assigned to a role outside the selected organization")]
    UnselectedAssignedRole,
    #[error("minimal organization plan selected a role with no required work")]
    UnnecessarySelectedRole,
    #[error("organization policy requires an independent evaluation work item")]
    RequiredEvaluationMissing,
    #[error("work item has an unknown or self dependency")]
    InvalidDependency,
    #[error("work plan dependency graph contains a cycle")]
    DependencyCycle,
}

fn validate_request(request: &PlanningRequest) -> Result<(), PlanningError> {
    if request.task.goal.trim().is_empty() || request.work.is_empty() {
        return Err(PlanningError::InvalidRequest);
    }
    let ids = request
        .work
        .iter()
        .map(|work| work.id)
        .collect::<BTreeSet<_>>();
    if ids.len() != request.work.len()
        || request.work.iter().any(|work| {
            work.goal.trim().is_empty()
                || work.success_conditions.is_empty()
                || work.budget_weight == 0
                || work.estimated_useful_seconds == 0
                || work.role == RoleKind::Executive
                || work
                    .dependencies
                    .iter()
                    .any(|dependency| *dependency == work.id || !ids.contains(dependency))
        })
    {
        return Err(PlanningError::InvalidRequest);
    }
    let fixture_items = request
        .work
        .iter()
        .map(|work| WorkItem {
            id: work.id,
            goal: work.goal.clone(),
            assigned_role: RoleId::new(),
            dependencies: work.dependencies.clone(),
            success_conditions: work.success_conditions.clone(),
            budget: TaskBudget::default(),
            risk_notes: Vec::new(),
            state: WorkItemState::Pending,
        })
        .collect::<Vec<_>>();
    if has_work_cycle(&fixture_items) {
        return Err(PlanningError::DependencyCycle);
    }
    Ok(())
}

fn has_work_cycle(items: &[WorkItem]) -> bool {
    let edges = items
        .iter()
        .map(|item| (item.id, item.dependencies.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    items
        .iter()
        .any(|item| visit_work(item.id, &edges, &mut visiting, &mut visited))
}

fn visit_work(
    id: WorkItemId,
    edges: &BTreeMap<WorkItemId, Vec<WorkItemId>>,
    visiting: &mut BTreeSet<WorkItemId>,
    visited: &mut BTreeSet<WorkItemId>,
) -> bool {
    if visited.contains(&id) {
        return false;
    }
    if !visiting.insert(id) {
        return true;
    }
    if edges.get(&id).is_some_and(|dependencies| {
        dependencies
            .iter()
            .any(|dependency| visit_work(*dependency, edges, visiting, visited))
    }) {
        return true;
    }
    visiting.remove(&id);
    visited.insert(id);
    false
}

#[allow(clippy::cast_precision_loss)] // Dollar apportionment is bounded by the parent total.
fn allocate_budgets(parent: &TaskBudget, weights: &[u32]) -> Vec<TaskBudget> {
    let total = weights.iter().map(|weight| u64::from(*weight)).sum::<u64>();
    let mut used_tokens = 0;
    let mut used_duration = 0;
    let mut used_cost = 0.0;
    weights
        .iter()
        .enumerate()
        .map(|(index, weight)| {
            let last = index + 1 == weights.len();
            let weight = u64::from(*weight);
            let max_tokens = parent.max_tokens.map(|limit| {
                let value = if last {
                    limit - used_tokens
                } else {
                    limit.saturating_mul(weight) / total
                };
                used_tokens += value;
                value
            });
            let max_duration_seconds = parent.max_duration_seconds.map(|limit| {
                let value = if last {
                    limit - used_duration
                } else {
                    limit.saturating_mul(weight) / total
                };
                used_duration += value;
                value
            });
            let max_cost_usd = parent.max_cost_usd.map(|limit| {
                let value = if last {
                    (limit - used_cost).max(0.0)
                } else {
                    limit * weight as f64 / total as f64
                };
                used_cost += value;
                value
            });
            TaskBudget {
                max_tokens,
                max_cost_usd,
                max_duration_seconds,
            }
        })
        .collect()
}

#[allow(clippy::cast_precision_loss)]
fn ratio(coordination: u64, useful: u64) -> f64 {
    let total = coordination.saturating_add(useful);
    if total == 0 {
        0.0
    } else {
        coordination as f64 / total as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcg_domain::RiskLevel;

    fn task(goal: &str) -> TaskEnvelope {
        let mut task = TaskEnvelope::new(goal);
        task.risk = RiskLevel::Low;
        task.budget = TaskBudget {
            max_tokens: Some(10_000),
            max_cost_usd: Some(4.0),
            max_duration_seconds: Some(600),
        };
        task
    }

    #[test]
    fn atomic_rename_uses_only_coder_without_coordination() {
        let genome = OrganizationGenome::consumer_default(Utc::now());
        let request = PlanningRequest {
            run_id: RunId::new(),
            task: task("Rename foo to bar"),
            complexity: TaskComplexity::Atomic,
            work: vec![PlannedWork::atomic(
                "Rename the symbol",
                RoleKind::Coder,
                vec![SuccessCondition::Test {
                    command: "cargo test".into(),
                }],
            )],
            require_independent_evaluation: false,
        };
        let plan = ExecutivePlanner::default()
            .plan(&genome, &request, Utc::now())
            .expect("minimal plan");
        assert_eq!(plan.selected_roles.len(), 1);
        assert_eq!(plan.items.len(), 1);
        assert!(plan.estimated_organizational_overhead.abs() < f64::EPSILON);
        let coder = genome
            .roles
            .iter()
            .find(|role| role.kind == RoleKind::Coder)
            .unwrap();
        assert_eq!(plan.selected_roles, vec![coder.id]);
    }

    #[test]
    fn compound_site_plan_tracks_dependencies_budgets_and_independent_evaluation() {
        let genome = OrganizationGenome::consumer_default(Utc::now());
        let implementation = PlannedWork::atomic(
            "Implement the site",
            RoleKind::Coder,
            vec![SuccessCondition::Build {
                command: "npm run build".into(),
            }],
        );
        let mut browser = PlannedWork::atomic(
            "Exercise the built site",
            RoleKind::ToolWorker,
            vec![SuccessCondition::BrowserWorkflow {
                name: "consumer journey".into(),
            }],
        );
        browser.dependencies.push(implementation.id);
        let request = PlanningRequest {
            run_id: RunId::new(),
            task: task("Build and verify a production website"),
            complexity: TaskComplexity::Compound,
            work: vec![implementation, browser],
            require_independent_evaluation: true,
        };
        let plan = ExecutivePlanner::default()
            .plan(&genome, &request, Utc::now())
            .expect("compound plan");
        let selected_kinds = plan
            .selected_roles
            .iter()
            .map(|id| {
                genome
                    .roles
                    .iter()
                    .find(|role| role.id == *id)
                    .unwrap()
                    .kind
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            selected_kinds,
            BTreeSet::from([
                RoleKind::Executive,
                RoleKind::Coder,
                RoleKind::ToolWorker,
                RoleKind::Evaluator
            ])
        );
        assert!(!selected_kinds.contains(&RoleKind::Researcher));
        assert_eq!(plan.items.len(), 3);
        assert_eq!(
            plan.items
                .iter()
                .filter(|item| item.state == WorkItemState::Ready)
                .count(),
            1
        );
        assert_eq!(
            plan.items
                .iter()
                .map(|item| item.budget.max_tokens.unwrap())
                .sum::<u64>(),
            10_000
        );
        assert!(
            (plan
                .items
                .iter()
                .map(|item| item.budget.max_cost_usd.unwrap())
                .sum::<f64>()
                - 4.0)
                .abs()
                < 1e-10
        );
    }

    #[test]
    fn dependency_cycles_are_rejected_before_selection() {
        let genome = OrganizationGenome::consumer_default(Utc::now());
        let mut first = PlannedWork::atomic(
            "first",
            RoleKind::Coder,
            vec![SuccessCondition::Custom {
                description: "first".into(),
            }],
        );
        let mut second = PlannedWork::atomic(
            "second",
            RoleKind::Coder,
            vec![SuccessCondition::Custom {
                description: "second".into(),
            }],
        );
        first.dependencies.push(second.id);
        second.dependencies.push(first.id);
        let request = PlanningRequest {
            run_id: RunId::new(),
            task: task("cyclic fixture"),
            complexity: TaskComplexity::Compound,
            work: vec![first, second],
            require_independent_evaluation: false,
        };
        assert_eq!(
            ExecutivePlanner::default().plan(&genome, &request, Utc::now()),
            Err(PlanningError::DependencyCycle)
        );
    }

    #[test]
    fn imported_plan_cannot_smuggle_unused_roles_or_skip_required_evaluation() {
        let mut genome = OrganizationGenome::consumer_default(Utc::now());
        let request = PlanningRequest {
            run_id: RunId::new(),
            task: task("Update one file"),
            complexity: TaskComplexity::Atomic,
            work: vec![PlannedWork::atomic(
                "Update the file",
                RoleKind::Coder,
                vec![SuccessCondition::Test {
                    command: "cargo test".into(),
                }],
            )],
            require_independent_evaluation: false,
        };
        let mut plan = ExecutivePlanner::default()
            .plan(&genome, &request, Utc::now())
            .expect("valid atomic plan");
        let unused = crate::RoleDefinition {
            id: RoleId::new(),
            kind: RoleKind::Researcher,
            name: "Researcher".into(),
            purpose: "Research".into(),
            prompt_policy: "bounded".into(),
            model_policy: crate::ModelPolicy {
                capability_class: "research".into(),
                preferred_models: Vec::new(),
                allow_fallback: true,
                max_escalation_tier: 1,
            },
            tool_capabilities: Vec::new(),
            memory_namespace: "role/researcher".into(),
        };
        plan.selected_roles.push(unused.id);
        genome.roles.push(unused);
        assert_eq!(
            plan.validate(&genome),
            Err(PlanningError::UnnecessarySelectedRole)
        );

        plan.selected_roles.pop();
        genome.evaluation.require_independent_evaluation = true;
        assert_eq!(
            plan.validate(&genome),
            Err(PlanningError::RequiredEvaluationMissing)
        );
    }
}
