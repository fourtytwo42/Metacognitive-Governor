use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use mcg_domain::{MemoryClaimId, ProjectId, ProjectSnapshotId};
use serde::{Deserialize, Serialize};

pub const PROJECT_MODEL_SCHEMA_VERSION: u16 = 1;
pub const MEMORY_CLAIM_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectModel {
    pub schema_version: u16,
    pub id: ProjectSnapshotId,
    pub project_id: ProjectId,
    pub root: String,
    pub fingerprint: String,
    pub observed_at: DateTime<Utc>,
    pub repository: RepositoryModel,
    pub languages: Vec<String>,
    pub build_systems: Vec<String>,
    pub test_systems: Vec<String>,
    pub package_managers: Vec<String>,
    pub services: Vec<String>,
    pub ci_systems: Vec<String>,
    pub important_directories: Vec<String>,
    pub commands: Vec<ProjectCommand>,
    pub instructions: InstructionResolution,
    pub existing_memory: Option<String>,
    pub world: ProjectWorldModel,
    pub watched_inputs: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RepositoryModel {
    pub is_git: bool,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub dirty: bool,
    pub recent_commits: Vec<RecentCommit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentCommit {
    pub id: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectCommand {
    pub name: String,
    pub command: String,
    pub source: String,
    pub confidence_basis: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct InstructionResolution {
    pub documents: Vec<InstructionDocument>,
    pub effective: Vec<EffectiveInstruction>,
    pub conflicts: Vec<InstructionConflict>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstructionDocument {
    pub path: String,
    pub kind: InstructionKind,
    pub scope: String,
    pub priority: u16,
    pub sha256: String,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstructionKind {
    Agents,
    AgentsOverride,
    Claude,
    ClaudeLocal,
    ClaudeRule,
    CursorRule,
    Readme,
    Contributing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectiveInstruction {
    pub key: String,
    pub value: String,
    pub source_path: String,
    pub priority: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstructionConflict {
    pub key: String,
    pub values: Vec<String>,
    pub source_paths: Vec<String>,
    pub resolution: ConflictResolution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolution {
    MoreSpecific,
    HigherPrecedence,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProjectWorldModel {
    pub nodes: Vec<WorldNode>,
    pub relationships: Vec<WorldRelationship>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldNode {
    pub id: String,
    pub kind: WorldNodeKind,
    pub label: String,
    pub source: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorldNodeKind {
    Service,
    Database,
    Domain,
    Api,
    Deployment,
    Dependency,
    Repository,
    Runtime,
    Command,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldRelationship {
    pub from: String,
    pub relation: String,
    pub to: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryClaim {
    pub schema_version: u16,
    pub id: MemoryClaimId,
    pub project_id: ProjectId,
    pub namespace: String,
    pub key: String,
    pub value: serde_json::Value,
    pub state: MemoryState,
    pub confidence: f64,
    pub provenance: ClaimProvenance,
    pub observed_at: DateTime<Utc>,
    pub fresh_until: Option<DateTime<Utc>>,
    pub supersedes: Option<MemoryClaimId>,
    pub correction_count: u32,
    pub usefulness_count: u32,
}

impl MemoryClaim {
    #[must_use]
    pub fn is_fresh_at(&self, now: DateTime<Utc>) -> bool {
        self.state != MemoryState::Retracted
            && self
                .fresh_until
                .is_none_or(|fresh_until| fresh_until >= now)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryState {
    New,
    Working,
    Durable,
    Decaying,
    Superseded,
    Retracted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimProvenance {
    pub source_kind: ProvenanceKind,
    pub source: String,
    pub task_id: Option<String>,
    pub artifact_sha256: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceKind {
    User,
    File,
    ToolObservation,
    ProviderObservation,
    Derived,
    Imported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextItem {
    pub category: ContextCategory,
    pub content: String,
    pub source: String,
    pub priority: u16,
    pub estimated_tokens: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextCategory {
    Goal,
    Constraint,
    Decision,
    OpenQuestion,
    Evidence,
    Plan,
    Instruction,
    Memory,
    ProjectFact,
    Transcript,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextAssembly {
    pub items: Vec<ContextItem>,
    pub omitted_items: u32,
    pub estimated_tokens: u32,
    pub compression_round: u16,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_and_memory_contracts_round_trip() {
        let project_id = ProjectId::new();
        let claim = MemoryClaim {
            schema_version: MEMORY_CLAIM_SCHEMA_VERSION,
            id: MemoryClaimId::new(),
            project_id,
            namespace: "project".into(),
            key: "package_manager".into(),
            value: serde_json::json!("npm"),
            state: MemoryState::Durable,
            confidence: 1.0,
            provenance: ClaimProvenance {
                source_kind: ProvenanceKind::File,
                source: "package-lock.json".into(),
                task_id: None,
                artifact_sha256: None,
            },
            observed_at: Utc::now(),
            fresh_until: None,
            supersedes: None,
            correction_count: 0,
            usefulness_count: 1,
        };
        let encoded = serde_json::to_string(&claim).expect("serialize");
        let decoded: MemoryClaim = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(decoded, claim);
        assert!(decoded.is_fresh_at(Utc::now()));
    }
}
