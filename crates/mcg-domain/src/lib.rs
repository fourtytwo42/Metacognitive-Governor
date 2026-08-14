//! Stable domain types shared by every MCG interface and implementation layer.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! domain_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    };
}

domain_id!(ProjectId);
domain_id!(ThreadId);
domain_id!(TaskId);
domain_id!(RunId);

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TaskBudget {
    pub max_tokens: Option<u64>,
    pub max_cost_usd: Option<f64>,
    pub max_duration_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskEnvelope {
    pub id: TaskId,
    pub project_id: Option<ProjectId>,
    pub thread_id: Option<ThreadId>,
    pub goal: String,
    pub constraints: Vec<String>,
    pub success_conditions: Vec<String>,
    pub risk: RiskLevel,
    pub budget: TaskBudget,
    pub state: TaskState,
}

impl TaskEnvelope {
    #[must_use]
    pub fn new(goal: impl Into<String>) -> Self {
        Self {
            id: TaskId::new(),
            project_id: None,
            thread_id: None,
            goal: goal.into(),
            constraints: Vec::new(),
            success_conditions: Vec::new(),
            risk: RiskLevel::Medium,
            budget: TaskBudget::default(),
            state: TaskState::Pending,
        }
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
    }
}
