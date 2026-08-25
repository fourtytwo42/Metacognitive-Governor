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
    ConversationCreated,
    ConversationUpdated,
    ConversationMessageRecorded,
    ConversationTurnUpdated,
    ConversationActivityUpdated,
    ConversationAgentSessionUpdated,
    AgentDelegationUpdated,
    AgentMessageRecorded,
    AgentContextCheckpointUpdated,
    CoordinationBatchUpdated,
    AgentWakeUpdated,
    ConversationPreviewUpdated,
    BlackboardEntryRecorded,
    ManagedWorktreeUpdated,
    CodexHarnessEvent,
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
    ProviderConnectionRemoved,
    ProviderCatalogDiscovered,
    ProviderCredentialsRefreshed,
    McpConnectionRecorded,
    McpCatalogDiscovered,
    ToolRequested,
    ToolCompleted,
    ProjectDiscovered,
    ReviewedTopologyPlanCreated,
    ReviewedTopologyPlanStateChanged,
    MemoryClaimRecorded,
    MemoryClaimCorrected,
    MemoryClaimStateChanged,
    MemoryClaimTransferred,
    OrganizationRevisionRecorded,
    OrganizationPublisherRecorded,
    OrganizationShareReviewed,
    OrganizationShareImported,
    OrganizationShareRejected,
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
    SupervisionObserved,
    ExperimentCreated,
    ExperimentStateChanged,
    ShadowExecutionRecorded,
    ExperimentBenchmarkRecorded,
    ExperimentDecisionRecorded,
    GovernorProposalCreated,
    GovernorProposalStateChanged,
    ShadowWorkspacePrepared,
    ShadowWorkspaceStateChanged,
    SshWorkflowCreated,
    SshWorkflowStateChanged,
}

impl EventKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConversationCreated => "conversation_created",
            Self::ConversationUpdated => "conversation_updated",
            Self::ConversationMessageRecorded => "conversation_message_recorded",
            Self::ConversationTurnUpdated => "conversation_turn_updated",
            Self::ConversationActivityUpdated => "conversation_activity_updated",
            Self::ConversationAgentSessionUpdated => "conversation_agent_session_updated",
            Self::AgentDelegationUpdated => "agent_delegation_updated",
            Self::AgentMessageRecorded => "agent_message_recorded",
            Self::AgentContextCheckpointUpdated => "agent_context_checkpoint_updated",
            Self::CoordinationBatchUpdated => "coordination_batch_updated",
            Self::AgentWakeUpdated => "agent_wake_updated",
            Self::ConversationPreviewUpdated => "conversation_preview_updated",
            Self::BlackboardEntryRecorded => "blackboard_entry_recorded",
            Self::ManagedWorktreeUpdated => "managed_worktree_updated",
            Self::CodexHarnessEvent => "codex_harness_event",
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
            Self::ProviderConnectionRemoved => "provider_connection_removed",
            Self::ProviderCatalogDiscovered => "provider_catalog_discovered",
            Self::ProviderCredentialsRefreshed => "provider_credentials_refreshed",
            Self::McpConnectionRecorded => "mcp_connection_recorded",
            Self::McpCatalogDiscovered => "mcp_catalog_discovered",
            Self::ToolRequested => "tool_requested",
            Self::ToolCompleted => "tool_completed",
            Self::ProjectDiscovered => "project_discovered",
            Self::ReviewedTopologyPlanCreated => "reviewed_topology_plan_created",
            Self::ReviewedTopologyPlanStateChanged => "reviewed_topology_plan_state_changed",
            Self::MemoryClaimRecorded => "memory_claim_recorded",
            Self::MemoryClaimCorrected => "memory_claim_corrected",
            Self::MemoryClaimStateChanged => "memory_claim_state_changed",
            Self::MemoryClaimTransferred => "memory_claim_transferred",
            Self::OrganizationRevisionRecorded => "organization_revision_recorded",
            Self::OrganizationPublisherRecorded => "organization_publisher_recorded",
            Self::OrganizationShareReviewed => "organization_share_reviewed",
            Self::OrganizationShareImported => "organization_share_imported",
            Self::OrganizationShareRejected => "organization_share_rejected",
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
            Self::SupervisionObserved => "supervision_observed",
            Self::ExperimentCreated => "experiment_created",
            Self::ExperimentStateChanged => "experiment_state_changed",
            Self::ShadowExecutionRecorded => "shadow_execution_recorded",
            Self::ExperimentBenchmarkRecorded => "experiment_benchmark_recorded",
            Self::ExperimentDecisionRecorded => "experiment_decision_recorded",
            Self::GovernorProposalCreated => "governor_proposal_created",
            Self::GovernorProposalStateChanged => "governor_proposal_state_changed",
            Self::ShadowWorkspacePrepared => "shadow_workspace_prepared",
            Self::ShadowWorkspaceStateChanged => "shadow_workspace_state_changed",
            Self::SshWorkflowCreated => "ssh_workflow_created",
            Self::SshWorkflowStateChanged => "ssh_workflow_state_changed",
        }
    }

    /// Parses the stable persisted representation of an event kind.
    ///
    /// # Errors
    ///
    /// Returns the unknown value when no schema variant matches.
    #[allow(clippy::too_many_lines)] // Exhaustive compatibility parser for the stable event registry.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "conversation_created" | "ConversationCreated" => Ok(Self::ConversationCreated),
            "conversation_updated" | "ConversationUpdated" => Ok(Self::ConversationUpdated),
            "conversation_message_recorded" | "ConversationMessageRecorded" => {
                Ok(Self::ConversationMessageRecorded)
            }
            "conversation_turn_updated" | "ConversationTurnUpdated" => {
                Ok(Self::ConversationTurnUpdated)
            }
            "conversation_activity_updated" | "ConversationActivityUpdated" => {
                Ok(Self::ConversationActivityUpdated)
            }
            "conversation_agent_session_updated" | "ConversationAgentSessionUpdated" => {
                Ok(Self::ConversationAgentSessionUpdated)
            }
            "agent_delegation_updated" | "AgentDelegationUpdated" => {
                Ok(Self::AgentDelegationUpdated)
            }
            "agent_message_recorded" | "AgentMessageRecorded" => Ok(Self::AgentMessageRecorded),
            "agent_context_checkpoint_updated" | "AgentContextCheckpointUpdated" => {
                Ok(Self::AgentContextCheckpointUpdated)
            }
            "coordination_batch_updated" | "CoordinationBatchUpdated" => {
                Ok(Self::CoordinationBatchUpdated)
            }
            "agent_wake_updated" | "AgentWakeUpdated" => Ok(Self::AgentWakeUpdated),
            "conversation_preview_updated" | "ConversationPreviewUpdated" => {
                Ok(Self::ConversationPreviewUpdated)
            }
            "blackboard_entry_recorded" | "BlackboardEntryRecorded" => {
                Ok(Self::BlackboardEntryRecorded)
            }
            "managed_worktree_updated" | "ManagedWorktreeUpdated" => {
                Ok(Self::ManagedWorktreeUpdated)
            }
            "codex_harness_event" | "CodexHarnessEvent" => Ok(Self::CodexHarnessEvent),
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
            "provider_connection_removed" | "ProviderConnectionRemoved" => {
                Ok(Self::ProviderConnectionRemoved)
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
            "reviewed_topology_plan_created" | "ReviewedTopologyPlanCreated" => {
                Ok(Self::ReviewedTopologyPlanCreated)
            }
            "reviewed_topology_plan_state_changed" | "ReviewedTopologyPlanStateChanged" => {
                Ok(Self::ReviewedTopologyPlanStateChanged)
            }
            "memory_claim_recorded" | "MemoryClaimRecorded" => Ok(Self::MemoryClaimRecorded),
            "memory_claim_corrected" | "MemoryClaimCorrected" => Ok(Self::MemoryClaimCorrected),
            "memory_claim_state_changed" | "MemoryClaimStateChanged" => {
                Ok(Self::MemoryClaimStateChanged)
            }
            "memory_claim_transferred" | "MemoryClaimTransferred" => {
                Ok(Self::MemoryClaimTransferred)
            }
            "organization_revision_recorded" | "OrganizationRevisionRecorded" => {
                Ok(Self::OrganizationRevisionRecorded)
            }
            "organization_publisher_recorded" | "OrganizationPublisherRecorded" => {
                Ok(Self::OrganizationPublisherRecorded)
            }
            "organization_share_reviewed" | "OrganizationShareReviewed" => {
                Ok(Self::OrganizationShareReviewed)
            }
            "organization_share_imported" | "OrganizationShareImported" => {
                Ok(Self::OrganizationShareImported)
            }
            "organization_share_rejected" | "OrganizationShareRejected" => {
                Ok(Self::OrganizationShareRejected)
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
            "supervision_observed" | "SupervisionObserved" => Ok(Self::SupervisionObserved),
            "experiment_created" | "ExperimentCreated" => Ok(Self::ExperimentCreated),
            "experiment_state_changed" | "ExperimentStateChanged" => {
                Ok(Self::ExperimentStateChanged)
            }
            "shadow_execution_recorded" | "ShadowExecutionRecorded" => {
                Ok(Self::ShadowExecutionRecorded)
            }
            "experiment_benchmark_recorded" | "ExperimentBenchmarkRecorded" => {
                Ok(Self::ExperimentBenchmarkRecorded)
            }
            "experiment_decision_recorded" | "ExperimentDecisionRecorded" => {
                Ok(Self::ExperimentDecisionRecorded)
            }
            "governor_proposal_created" | "GovernorProposalCreated" => {
                Ok(Self::GovernorProposalCreated)
            }
            "governor_proposal_state_changed" | "GovernorProposalStateChanged" => {
                Ok(Self::GovernorProposalStateChanged)
            }
            "shadow_workspace_prepared" | "ShadowWorkspacePrepared" => {
                Ok(Self::ShadowWorkspacePrepared)
            }
            "shadow_workspace_state_changed" | "ShadowWorkspaceStateChanged" => {
                Ok(Self::ShadowWorkspaceStateChanged)
            }
            "ssh_workflow_created" | "SshWorkflowCreated" => Ok(Self::SshWorkflowCreated),
            "ssh_workflow_state_changed" | "SshWorkflowStateChanged" => {
                Ok(Self::SshWorkflowStateChanged)
            }
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
