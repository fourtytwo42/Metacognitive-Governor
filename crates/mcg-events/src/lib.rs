//! Versioned events form MCG's audit and integration backbone.

use chrono::{DateTime, Utc};
use mcg_domain::TaskId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub const EVENT_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    TaskCreated,
    TaskStarted,
    TaskCompleted,
    TaskFailed,
    ApprovalRequested,
    ApprovalResolved,
    ProviderCallStarted,
    ProviderCallCompleted,
    ToolRequested,
    ToolCompleted,
    McgIntervention,
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
}
