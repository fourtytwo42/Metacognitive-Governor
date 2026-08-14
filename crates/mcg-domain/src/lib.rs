//! Stable, versioned domain types shared by every MCG interface.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const TASK_SCHEMA_VERSION: u16 = 1;

macro_rules! domain_id {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl std::str::FromStr for $name {
            type Err = uuid::Error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(value).map(Self)
            }
        }
    };
}

domain_id!(ProjectId);
domain_id!(ThreadId);
domain_id!(TaskId);
domain_id!(RunId);
domain_id!(ToolRequestId);
domain_id!(ToolExecutionId);
domain_id!(ApprovalId);
domain_id!(LeaseId);
domain_id!(ArtifactId);
domain_id!(ManagedProcessId);
domain_id!(ProjectSnapshotId);
domain_id!(MemoryClaimId);
domain_id!(OrganizationId);
domain_id!(OrganizationRevisionId);
domain_id!(RoleId);
domain_id!(AgentInstanceId);
domain_id!(AgentLeaseId);
domain_id!(WorkItemId);
domain_id!(EvaluationId);
domain_id!(UsageRecordId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Pending,
    Active,
    WaitingForApproval,
    Blocked,
    Completed,
    Failed,
    Cancelled,
}

impl TaskState {
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Pending, Self::Active | Self::Cancelled)
                | (
                    Self::Active,
                    Self::WaitingForApproval
                        | Self::Blocked
                        | Self::Completed
                        | Self::Failed
                        | Self::Cancelled
                )
                | (
                    Self::WaitingForApproval | Self::Blocked,
                    Self::Active | Self::Failed | Self::Cancelled
                )
        )
    }
}

impl std::fmt::Display for TaskState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = serde_json::to_value(self).map_err(|_| std::fmt::Error)?;
        formatter.write_str(value.as_str().ok_or(std::fmt::Error)?)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("invalid task transition from {from} to {to}")]
pub struct InvalidTaskTransition {
    pub from: TaskState,
    pub to: TaskState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TaskBudget {
    pub max_tokens: Option<u64>,
    pub max_cost_usd: Option<f64>,
    pub max_duration_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskEnvelope {
    pub schema_version: u16,
    pub id: TaskId,
    pub project_id: Option<ProjectId>,
    pub thread_id: Option<ThreadId>,
    pub goal: String,
    pub constraints: Vec<String>,
    pub success_conditions: Vec<String>,
    pub risk: RiskLevel,
    pub budget: TaskBudget,
    pub state: TaskState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TaskEnvelope {
    #[must_use]
    pub fn new(goal: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            schema_version: TASK_SCHEMA_VERSION,
            id: TaskId::new(),
            project_id: None,
            thread_id: None,
            goal: goal.into(),
            constraints: Vec::new(),
            success_conditions: Vec::new(),
            risk: RiskLevel::Medium,
            budget: TaskBudget::default(),
            state: TaskState::Pending,
            created_at: now,
            updated_at: now,
        }
    }

    /// Applies a legal state transition.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidTaskTransition`] when the state machine forbids the edge.
    pub fn transition_to(&mut self, next: TaskState) -> Result<(), InvalidTaskTransition> {
        if !self.state.can_transition_to(next) {
            return Err(InvalidTaskTransition {
                from: self.state,
                to: next,
            });
        }
        self.state = next;
        self.updated_at = Utc::now();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_envelope_round_trips_through_json() {
        let task = TaskEnvelope::new("Build a verified website");
        let json = serde_json::to_string(&task).expect("task should serialize");
        let decoded: TaskEnvelope = serde_json::from_str(&json).expect("task should deserialize");

        assert_eq!(decoded, task);
        assert_eq!(decoded.schema_version, TASK_SCHEMA_VERSION);
    }

    #[test]
    fn terminal_task_states_cannot_restart() {
        assert!(!TaskState::Completed.can_transition_to(TaskState::Active));
        assert!(TaskState::Pending.can_transition_to(TaskState::Active));
    }
}
