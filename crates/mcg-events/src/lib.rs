//! Versioned events form MCG's audit and integration backbone.

use chrono::{DateTime, Utc};
use mcg_domain::{TaskEnvelope, TaskId, TaskState};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub const EVENT_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    TaskCreated,
    TaskStarted,
    TaskStateChanged,
    TaskCompleted,
    TaskFailed,
    ApprovalRequested,
    ApprovalResolved,
    ProviderCallStarted,
    ProviderCallCompleted,
    ProviderConnectionRecorded,
    ProviderCatalogDiscovered,
    ProviderCredentialsRefreshed,
    McpConnectionRecorded,
    McpCatalogDiscovered,
    ToolRequested,
    ToolCompleted,
    ProjectDiscovered,
    MemoryClaimRecorded,
    MemoryClaimCorrected,
    MemoryClaimStateChanged,
    OrganizationRevisionRecorded,
    AgentRunPlanned,
    AgentRunStateChanged,
    AgentInstanceRecorded,
    AgentInstanceStateChanged,
    AgentLeaseRecorded,
    AgentLeaseStateChanged,
    AgentUsageRecorded,
    EvaluationCheckpointed,
    EvaluationRecorded,
    AgentRunMetricsUpdated,
    AgentTurnCompleted,
    AgentTurnWaiting,
    AgentTurnFailed,
    McgIntervention,
}

impl EventKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TaskCreated => "task_created",
            Self::TaskStarted => "task_started",
            Self::TaskStateChanged => "task_state_changed",
            Self::TaskCompleted => "task_completed",
            Self::TaskFailed => "task_failed",
            Self::ApprovalRequested => "approval_requested",
            Self::ApprovalResolved => "approval_resolved",
            Self::ProviderCallStarted => "provider_call_started",
            Self::ProviderCallCompleted => "provider_call_completed",
            Self::ProviderConnectionRecorded => "provider_connection_recorded",
            Self::ProviderCatalogDiscovered => "provider_catalog_discovered",
            Self::ProviderCredentialsRefreshed => "provider_credentials_refreshed",
            Self::McpConnectionRecorded => "mcp_connection_recorded",
            Self::McpCatalogDiscovered => "mcp_catalog_discovered",
            Self::ToolRequested => "tool_requested",
            Self::ToolCompleted => "tool_completed",
            Self::ProjectDiscovered => "project_discovered",
            Self::MemoryClaimRecorded => "memory_claim_recorded",
            Self::MemoryClaimCorrected => "memory_claim_corrected",
            Self::MemoryClaimStateChanged => "memory_claim_state_changed",
            Self::OrganizationRevisionRecorded => "organization_revision_recorded",
            Self::AgentRunPlanned => "agent_run_planned",
            Self::AgentRunStateChanged => "agent_run_state_changed",
            Self::AgentInstanceRecorded => "agent_instance_recorded",
            Self::AgentInstanceStateChanged => "agent_instance_state_changed",
            Self::AgentLeaseRecorded => "agent_lease_recorded",
            Self::AgentLeaseStateChanged => "agent_lease_state_changed",
            Self::AgentUsageRecorded => "agent_usage_recorded",
            Self::EvaluationCheckpointed => "evaluation_checkpointed",
            Self::EvaluationRecorded => "evaluation_recorded",
            Self::AgentRunMetricsUpdated => "agent_run_metrics_updated",
            Self::AgentTurnCompleted => "agent_turn_completed",
            Self::AgentTurnWaiting => "agent_turn_waiting",
            Self::AgentTurnFailed => "agent_turn_failed",
            Self::McgIntervention => "mcg_intervention",
        }
    }

    /// Parses the stable persisted representation of an event kind.
    ///
    /// # Errors
    ///
    /// Returns the unknown value when no schema variant matches.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "task_created" | "TaskCreated" => Ok(Self::TaskCreated),
            "task_started" | "TaskStarted" => Ok(Self::TaskStarted),
            "task_state_changed" | "TaskStateChanged" => Ok(Self::TaskStateChanged),
            "task_completed" | "TaskCompleted" => Ok(Self::TaskCompleted),
            "task_failed" | "TaskFailed" => Ok(Self::TaskFailed),
            "approval_requested" | "ApprovalRequested" => Ok(Self::ApprovalRequested),
            "approval_resolved" | "ApprovalResolved" => Ok(Self::ApprovalResolved),
            "provider_call_started" | "ProviderCallStarted" => Ok(Self::ProviderCallStarted),
            "provider_call_completed" | "ProviderCallCompleted" => Ok(Self::ProviderCallCompleted),
            "provider_connection_recorded" | "ProviderConnectionRecorded" => {
                Ok(Self::ProviderConnectionRecorded)
            }
            "provider_catalog_discovered" | "ProviderCatalogDiscovered" => {
                Ok(Self::ProviderCatalogDiscovered)
            }
            "provider_credentials_refreshed" | "ProviderCredentialsRefreshed" => {
                Ok(Self::ProviderCredentialsRefreshed)
            }
            "mcp_connection_recorded" | "McpConnectionRecorded" => Ok(Self::McpConnectionRecorded),
            "mcp_catalog_discovered" | "McpCatalogDiscovered" => Ok(Self::McpCatalogDiscovered),
            "tool_requested" | "ToolRequested" => Ok(Self::ToolRequested),
            "tool_completed" | "ToolCompleted" => Ok(Self::ToolCompleted),
            "project_discovered" | "ProjectDiscovered" => Ok(Self::ProjectDiscovered),
            "memory_claim_recorded" | "MemoryClaimRecorded" => Ok(Self::MemoryClaimRecorded),
            "memory_claim_corrected" | "MemoryClaimCorrected" => Ok(Self::MemoryClaimCorrected),
            "memory_claim_state_changed" | "MemoryClaimStateChanged" => {
                Ok(Self::MemoryClaimStateChanged)
            }
            "organization_revision_recorded" | "OrganizationRevisionRecorded" => {
                Ok(Self::OrganizationRevisionRecorded)
            }
            "agent_run_planned" | "AgentRunPlanned" => Ok(Self::AgentRunPlanned),
            "agent_run_state_changed" | "AgentRunStateChanged" => Ok(Self::AgentRunStateChanged),
            "agent_instance_recorded" | "AgentInstanceRecorded" => Ok(Self::AgentInstanceRecorded),
            "agent_instance_state_changed" | "AgentInstanceStateChanged" => {
                Ok(Self::AgentInstanceStateChanged)
            }
            "agent_lease_recorded" | "AgentLeaseRecorded" => Ok(Self::AgentLeaseRecorded),
            "agent_lease_state_changed" | "AgentLeaseStateChanged" => {
                Ok(Self::AgentLeaseStateChanged)
            }
            "agent_usage_recorded" | "AgentUsageRecorded" => Ok(Self::AgentUsageRecorded),
            "evaluation_checkpointed" | "EvaluationCheckpointed" => {
                Ok(Self::EvaluationCheckpointed)
            }
            "evaluation_recorded" | "EvaluationRecorded" => Ok(Self::EvaluationRecorded),
            "agent_run_metrics_updated" | "AgentRunMetricsUpdated" => {
                Ok(Self::AgentRunMetricsUpdated)
            }
            "agent_turn_completed" | "AgentTurnCompleted" => Ok(Self::AgentTurnCompleted),
            "agent_turn_waiting" | "AgentTurnWaiting" => Ok(Self::AgentTurnWaiting),
            "agent_turn_failed" | "AgentTurnFailed" => Ok(Self::AgentTurnFailed),
            "mcg_intervention" | "McgIntervention" => Ok(Self::McgIntervention),
            other => Err(other.to_owned()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskCreatedPayload {
    pub task: TaskEnvelope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskStateChangedPayload {
    pub from: TaskState,
    pub to: TaskState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub id: Uuid,
    pub schema_version: u16,
    pub sequence: u64,
    pub occurred_at: DateTime<Utc>,
    pub task_id: Option<TaskId>,
    pub kind: EventKind,
    pub payload: Value,
}

impl EventEnvelope {
    #[must_use]
    pub fn new(kind: EventKind, task_id: Option<TaskId>, payload: Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            schema_version: EVENT_SCHEMA_VERSION,
            sequence: 0,
            occurred_at: Utc::now(),
            task_id,
            kind,
            payload,
        }
    }

    #[must_use]
    pub const fn with_sequence(mut self, sequence: u64) -> Self {
        self.sequence = sequence;
        self
    }
}
