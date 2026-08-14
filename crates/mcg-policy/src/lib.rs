//! Capability decisions and owner-controlled autonomy presets.

use mcg_config::AutonomyPreset;
use mcg_domain::TaskId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionClass {
    ProjectRead,
    ReversibleProjectWrite,
    BuildOrTest,
    DestructiveDelete,
    CredentialAccess,
    ExternalPublication,
    Purchase,
    PrivilegeElevation,
    ConstitutionChange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyRequest {
    pub task_id: TaskId,
    pub action: ActionClass,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum PolicyDecision {
    Allow { reason: String },
    Deny { reason: String },
    RequireApproval { reason: String },
}

#[must_use]
pub fn evaluate(preset: AutonomyPreset, request: &PolicyRequest) -> PolicyDecision {
    use ActionClass::{
        BuildOrTest, ConstitutionChange, CredentialAccess, DestructiveDelete, ExternalPublication,
        PrivilegeElevation, ProjectRead, Purchase, ReversibleProjectWrite,
    };

    match request.action {
        ConstitutionChange => PolicyDecision::Deny {
            reason: "MCG cannot change the owner constitution".to_owned(),
        },
        DestructiveDelete | CredentialAccess | ExternalPublication | Purchase
        | PrivilegeElevation => PolicyDecision::RequireApproval {
            reason: "the action is consequential and requires owner approval".to_owned(),
        },
        ProjectRead => PolicyDecision::Allow {
            reason: "project-scoped reads are allowed".to_owned(),
        },
        ReversibleProjectWrite | BuildOrTest if preset == AutonomyPreset::Guided => {
            PolicyDecision::RequireApproval {
                reason: "guided autonomy asks before project mutations".to_owned(),
            }
        }
        ReversibleProjectWrite | BuildOrTest => PolicyDecision::Allow {
            reason: "the selected autonomy preset permits reversible project work".to_owned(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(action: ActionClass) -> PolicyRequest {
        PolicyRequest {
            task_id: TaskId::new(),
            action,
            summary: "test action".to_owned(),
        }
    }

    #[test]
    fn autonomous_mode_allows_reversible_project_work() {
        let decision = evaluate(
            AutonomyPreset::Autonomous,
            &request(ActionClass::ReversibleProjectWrite),
        );

        assert!(matches!(decision, PolicyDecision::Allow { .. }));
    }

    #[test]
    fn consequential_actions_always_require_approval() {
        let decision = evaluate(
            AutonomyPreset::Autonomous,
            &request(ActionClass::ExternalPublication),
        );

        assert!(matches!(decision, PolicyDecision::RequireApproval { .. }));
    }

    #[test]
    fn constitution_changes_are_denied() {
        let decision = evaluate(
            AutonomyPreset::Autonomous,
            &request(ActionClass::ConstitutionChange),
        );

        assert!(matches!(decision, PolicyDecision::Deny { .. }));
    }
}
