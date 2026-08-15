//! Capability decisions and owner-controlled autonomy presets.
//!
//! Policy evaluates already-resolved resources. Filesystem canonicalization and
//! executable resolution belong to the tool broker; prompt text is never an
//! authority boundary.

use std::path::{Component, Path, PathBuf};

use mcg_config::AutonomyPreset;
use mcg_domain::TaskId;
use serde::{Deserialize, Serialize};

pub const POLICY_SCHEMA_VERSION: u16 = 1;

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
    ComputerInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyRequest {
    pub task_id: TaskId,
    pub action: ActionClass,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityEffect {
    Read,
    Write,
    Execute,
    Delete,
    Publish,
    Administer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    File,
    Directory,
    Process,
    ManagedProcess,
    System,
    Shell,
    NetworkOrigin,
    EnvironmentVariable,
    GitRepository,
    Memory,
    Constitution,
    Credential,
    ComputerSession,
    Site,
    SshHost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityAccess {
    Allowed,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityRequest {
    pub task_id: TaskId,
    pub action: ActionClass,
    pub effect: CapabilityEffect,
    pub kind: ResourceKind,
    /// Canonical path, normalized network origin, executable name, or logical key.
    pub resource: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityPolicy {
    pub schema_version: u16,
    #[serde(default)]
    pub filesystem_read_roots: Vec<PathBuf>,
    #[serde(default)]
    pub filesystem_write_roots: Vec<PathBuf>,
    #[serde(default)]
    pub allowed_executables: Vec<String>,
    #[serde(default)]
    pub allowed_shells: Vec<String>,
    #[serde(default)]
    pub allowed_network_origins: Vec<String>,
    #[serde(default)]
    pub allowed_environment_variables: Vec<String>,
    #[serde(default)]
    pub allowed_computer_sessions: Vec<String>,
    #[serde(default)]
    pub allowed_sites: Vec<String>,
    #[serde(default)]
    pub allowed_ssh_hosts: Vec<String>,
    #[serde(default)]
    pub memory_read_prefixes: Vec<String>,
    #[serde(default)]
    pub memory_write_prefixes: Vec<String>,
    pub allow_shell: bool,
    pub allow_git_mutation: bool,
    pub managed_process_control: CapabilityAccess,
    pub system_inspection: CapabilityAccess,
}

impl CapabilityPolicy {
    #[must_use]
    pub fn project(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            schema_version: POLICY_SCHEMA_VERSION,
            filesystem_read_roots: vec![root.clone()],
            filesystem_write_roots: vec![root],
            allowed_executables: vec![
                "cargo".into(),
                "git".into(),
                "node".into(),
                "npm".into(),
                "python".into(),
            ],
            allowed_shells: vec![
                "bash".into(),
                "cmd".into(),
                "powershell".into(),
                "pwsh".into(),
                "sh".into(),
            ],
            allowed_network_origins: Vec::new(),
            allowed_environment_variables: Vec::new(),
            allowed_computer_sessions: Vec::new(),
            allowed_sites: Vec::new(),
            allowed_ssh_hosts: Vec::new(),
            memory_read_prefixes: vec!["project/".into(), "task/".into()],
            memory_write_prefixes: vec!["project/".into(), "task/".into()],
            allow_shell: false,
            allow_git_mutation: true,
            managed_process_control: CapabilityAccess::Allowed,
            system_inspection: CapabilityAccess::Allowed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstitutionalRequirement {
    Required,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerConstitution {
    pub schema_version: u16,
    pub audit: ConstitutionalRequirement,
    pub secret_isolation: ConstitutionalRequirement,
    pub destructive_approval: ConstitutionalRequirement,
    pub publication_approval: ConstitutionalRequirement,
    pub privilege_elevation_approval: ConstitutionalRequirement,
    #[serde(default)]
    pub secret_paths: Vec<PathBuf>,
    #[serde(default)]
    pub protected_paths: Vec<PathBuf>,
}

impl OwnerConstitution {
    #[must_use]
    pub fn secure_defaults() -> Self {
        Self {
            schema_version: POLICY_SCHEMA_VERSION,
            audit: ConstitutionalRequirement::Required,
            secret_isolation: ConstitutionalRequirement::Required,
            destructive_approval: ConstitutionalRequirement::Required,
            publication_approval: ConstitutionalRequirement::Required,
            privilege_elevation_approval: ConstitutionalRequirement::Required,
            secret_paths: Vec::new(),
            protected_paths: Vec::new(),
        }
    }

    /// Rejects any representation that weakens non-negotiable owner invariants.
    /// This is deliberately re-run at each broker decision so deserialization or
    /// in-memory mutation cannot bypass construction-time checks.
    ///
    /// # Errors
    ///
    /// Returns the first non-negotiable invariant that is missing or disabled.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema_version != POLICY_SCHEMA_VERSION {
            return Err("unsupported constitution schema version");
        }
        if self.audit != ConstitutionalRequirement::Required {
            return Err("audit logging cannot be disabled");
        }
        if self.secret_isolation != ConstitutionalRequirement::Required {
            return Err("secret isolation cannot be disabled");
        }
        if self.destructive_approval != ConstitutionalRequirement::Required
            || self.publication_approval != ConstitutionalRequirement::Required
            || self.privilege_elevation_approval != ConstitutionalRequirement::Required
        {
            return Err("constitutional approval gates cannot be weakened");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum PolicyDecision {
    Allow {
        reason: String,
        #[serde(default)]
        matched_rules: Vec<String>,
    },
    Deny {
        reason: String,
        #[serde(default)]
        matched_rules: Vec<String>,
    },
    RequireApproval {
        reason: String,
        #[serde(default)]
        matched_rules: Vec<String>,
    },
}

impl PolicyDecision {
    fn allow(reason: impl Into<String>, rule: impl Into<String>) -> Self {
        Self::Allow {
            reason: reason.into(),
            matched_rules: vec![rule.into()],
        }
    }

    fn deny(reason: impl Into<String>, rule: impl Into<String>) -> Self {
        Self::Deny {
            reason: reason.into(),
            matched_rules: vec![rule.into()],
        }
    }

    fn approval(reason: impl Into<String>, rule: impl Into<String>) -> Self {
        Self::RequireApproval {
            reason: reason.into(),
            matched_rules: vec![rule.into()],
        }
    }
}

#[must_use]
pub fn evaluate(preset: AutonomyPreset, request: &PolicyRequest) -> PolicyDecision {
    use ActionClass::{
        BuildOrTest, ComputerInput, ConstitutionChange, CredentialAccess, DestructiveDelete,
        ExternalPublication, PrivilegeElevation, ProjectRead, Purchase, ReversibleProjectWrite,
    };

    match request.action {
        ConstitutionChange => PolicyDecision::deny(
            "MCG cannot change the owner constitution",
            "constitution.immutable",
        ),
        DestructiveDelete | CredentialAccess | ExternalPublication | Purchase
        | PrivilegeElevation => PolicyDecision::approval(
            "the action is consequential and requires owner approval",
            "constitution.consequential_approval",
        ),
        ProjectRead => {
            PolicyDecision::allow("project-scoped reads are allowed", "preset.project_read")
        }
        ComputerInput if preset == AutonomyPreset::Guided => PolicyDecision::approval(
            "guided autonomy asks before bounded computer input",
            "preset.guided_computer_input",
        ),
        ComputerInput => PolicyDecision::allow(
            "the selected autonomy preset permits bounded computer input",
            "preset.bounded_computer_input",
        ),
        ReversibleProjectWrite | BuildOrTest if preset == AutonomyPreset::Guided => {
            PolicyDecision::approval(
                "guided autonomy asks before project mutations",
                "preset.guided_mutation",
            )
        }
        ReversibleProjectWrite | BuildOrTest => PolicyDecision::allow(
            "the selected autonomy preset permits reversible project work",
            "preset.reversible_project_work",
        ),
    }
}

#[must_use]
pub fn evaluate_capability(
    preset: AutonomyPreset,
    constitution: &OwnerConstitution,
    capabilities: &CapabilityPolicy,
    request: &CapabilityRequest,
) -> PolicyDecision {
    if let Err(reason) = constitution.validate() {
        return PolicyDecision::deny(reason, "constitution.invalid");
    }
    if capabilities.schema_version != POLICY_SCHEMA_VERSION {
        return PolicyDecision::deny(
            "unsupported capability policy schema version",
            "capability.invalid",
        );
    }
    if request.kind == ResourceKind::Constitution
        || request.action == ActionClass::ConstitutionChange
    {
        return PolicyDecision::deny(
            "MCG cannot modify or administer the owner constitution",
            "constitution.immutable",
        );
    }
    if request.kind == ResourceKind::Credential {
        return PolicyDecision::deny(
            "stored credential values cannot enter tool or model context",
            "constitution.secret_isolation",
        );
    }

    if matches!(request.kind, ResourceKind::File | ResourceKind::Directory) {
        let resource = Path::new(&request.resource);
        if constitution
            .secret_paths
            .iter()
            .any(|scope| resource.starts_with(scope))
        {
            return PolicyDecision::deny(
                "the resource is within a secret-isolation scope",
                "constitution.secret_path",
            );
        }
    }

    if !capability_contains(capabilities, request) {
        return PolicyDecision::deny(
            "the requested resource is outside the caller capability scope",
            "capability.scope",
        );
    }

    if matches!(request.kind, ResourceKind::File | ResourceKind::Directory)
        && constitution
            .protected_paths
            .iter()
            .any(|scope| Path::new(&request.resource).starts_with(scope))
        && request.effect != CapabilityEffect::Read
    {
        return PolicyDecision::approval(
            "modifying a protected path requires owner approval",
            "constitution.protected_path",
        );
    }

    if request.effect == CapabilityEffect::Delete {
        return PolicyDecision::approval(
            "destructive actions require owner approval",
            "constitution.destructive_approval",
        );
    }
    if request.effect == CapabilityEffect::Publish {
        return PolicyDecision::approval(
            "external publication requires owner approval",
            "constitution.publication_approval",
        );
    }
    if request.action == ActionClass::PrivilegeElevation {
        return PolicyDecision::approval(
            "privilege elevation requires owner approval",
            "constitution.elevation_approval",
        );
    }

    evaluate(
        preset,
        &PolicyRequest {
            task_id: request.task_id,
            action: request.action,
            summary: request.summary.clone(),
        },
    )
}

fn capability_contains(policy: &CapabilityPolicy, request: &CapabilityRequest) -> bool {
    match request.kind {
        ResourceKind::File | ResourceKind::Directory | ResourceKind::GitRepository => {
            if Path::new(&request.resource)
                .components()
                .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
            {
                return false;
            }
            let roots = if request.effect == CapabilityEffect::Read {
                &policy.filesystem_read_roots
            } else {
                &policy.filesystem_write_roots
            };
            roots
                .iter()
                .any(|root| Path::new(&request.resource).starts_with(root))
                && (request.kind != ResourceKind::GitRepository
                    || request.effect == CapabilityEffect::Read
                    || policy.allow_git_mutation)
        }
        ResourceKind::Process => policy
            .allowed_executables
            .iter()
            .any(|item| item.eq_ignore_ascii_case(&request.resource)),
        ResourceKind::ManagedProcess => policy.managed_process_control == CapabilityAccess::Allowed,
        ResourceKind::System => policy.system_inspection == CapabilityAccess::Allowed,
        ResourceKind::Shell => {
            policy.allow_shell
                && policy
                    .allowed_shells
                    .iter()
                    .any(|shell| shell.eq_ignore_ascii_case(&request.resource))
        }
        ResourceKind::NetworkOrigin => policy
            .allowed_network_origins
            .iter()
            .any(|origin| origin.eq_ignore_ascii_case(&request.resource)),
        ResourceKind::EnvironmentVariable => policy
            .allowed_environment_variables
            .iter()
            .any(|name| name.eq_ignore_ascii_case(&request.resource)),
        ResourceKind::Memory => {
            let prefixes = if request.effect == CapabilityEffect::Read {
                &policy.memory_read_prefixes
            } else {
                &policy.memory_write_prefixes
            };
            prefixes
                .iter()
                .any(|prefix| request.resource.starts_with(prefix))
        }
        ResourceKind::ComputerSession => policy
            .allowed_computer_sessions
            .iter()
            .any(|session| session == &request.resource),
        ResourceKind::Site => policy
            .allowed_sites
            .iter()
            .any(|site| site == &request.resource),
        ResourceKind::SshHost => policy
            .allowed_ssh_hosts
            .iter()
            .any(|host| host == &request.resource),
        ResourceKind::Constitution | ResourceKind::Credential => false,
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

    fn capability_request(
        action: ActionClass,
        effect: CapabilityEffect,
        kind: ResourceKind,
        resource: impl Into<String>,
    ) -> CapabilityRequest {
        CapabilityRequest {
            task_id: TaskId::new(),
            action,
            effect,
            kind,
            resource: resource.into(),
            summary: "test capability".into(),
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

    #[test]
    fn invalid_deserialized_constitution_fails_closed() {
        let root = PathBuf::from("C:/project");
        let capabilities = CapabilityPolicy::project(&root);
        let mut constitution = OwnerConstitution::secure_defaults();
        constitution.audit = ConstitutionalRequirement::Disabled;
        let decision = evaluate_capability(
            AutonomyPreset::Autonomous,
            &constitution,
            &capabilities,
            &capability_request(
                ActionClass::ProjectRead,
                CapabilityEffect::Read,
                ResourceKind::File,
                root.join("README.md").to_string_lossy(),
            ),
        );
        assert!(matches!(decision, PolicyDecision::Deny { .. }));
    }

    #[test]
    fn secret_paths_are_denied_even_when_project_scope_allows_them() {
        let root = PathBuf::from("/project");
        let capabilities = CapabilityPolicy::project(&root);
        let mut constitution = OwnerConstitution::secure_defaults();
        constitution.secret_paths.push(root.join(".secrets"));
        let decision = evaluate_capability(
            AutonomyPreset::Autonomous,
            &constitution,
            &capabilities,
            &capability_request(
                ActionClass::ProjectRead,
                CapabilityEffect::Read,
                ResourceKind::File,
                "/project/.secrets/token",
            ),
        );
        assert!(matches!(decision, PolicyDecision::Deny { .. }));
    }

    #[test]
    fn protected_path_write_requires_approval() {
        let root = PathBuf::from("/project");
        let capabilities = CapabilityPolicy::project(&root);
        let mut constitution = OwnerConstitution::secure_defaults();
        constitution.protected_paths.push(root.join("release"));
        let decision = evaluate_capability(
            AutonomyPreset::Autonomous,
            &constitution,
            &capabilities,
            &capability_request(
                ActionClass::ReversibleProjectWrite,
                CapabilityEffect::Write,
                ResourceKind::File,
                "/project/release/app",
            ),
        );
        assert!(matches!(decision, PolicyDecision::RequireApproval { .. }));
    }

    #[test]
    fn sibling_prefix_does_not_escape_path_capability() {
        let capabilities = CapabilityPolicy::project("/project");
        let decision = evaluate_capability(
            AutonomyPreset::Autonomous,
            &OwnerConstitution::secure_defaults(),
            &capabilities,
            &capability_request(
                ActionClass::ProjectRead,
                CapabilityEffect::Read,
                ResourceKind::File,
                "/project-secrets/token",
            ),
        );
        assert!(matches!(decision, PolicyDecision::Deny { .. }));
    }

    #[test]
    fn executable_and_shell_paths_require_exact_capability_entries() {
        let mut capabilities = CapabilityPolicy::project("/project");
        capabilities.allow_shell = true;
        for (kind, resource) in [
            (ResourceKind::Process, "/untrusted/git"),
            (ResourceKind::Shell, "/untrusted/bash"),
        ] {
            let decision = evaluate_capability(
                AutonomyPreset::Autonomous,
                &OwnerConstitution::secure_defaults(),
                &capabilities,
                &capability_request(
                    ActionClass::BuildOrTest,
                    CapabilityEffect::Execute,
                    kind,
                    resource,
                ),
            );
            assert!(matches!(decision, PolicyDecision::Deny { .. }));
        }
    }

    #[test]
    fn computer_input_requires_exact_session_capability_and_respects_autonomy() {
        let resource = "computer://session/018f7f18-76e3-7b2a-9cc6-5bf2518f5750";
        let request = capability_request(
            ActionClass::ComputerInput,
            CapabilityEffect::Execute,
            ResourceKind::ComputerSession,
            resource,
        );
        let mut capabilities = CapabilityPolicy::project("/project");
        let constitution = OwnerConstitution::secure_defaults();

        assert!(matches!(
            evaluate_capability(
                AutonomyPreset::Autonomous,
                &constitution,
                &capabilities,
                &request,
            ),
            PolicyDecision::Deny { .. }
        ));

        capabilities.allowed_computer_sessions.push(resource.into());
        assert!(matches!(
            evaluate_capability(
                AutonomyPreset::Autonomous,
                &constitution,
                &capabilities,
                &request,
            ),
            PolicyDecision::Allow { .. }
        ));
        assert!(matches!(
            evaluate_capability(
                AutonomyPreset::Guided,
                &constitution,
                &capabilities,
                &request,
            ),
            PolicyDecision::RequireApproval { .. }
        ));
    }

    #[test]
    fn approval_class_effects_cannot_bypass_capability_scope() {
        let capabilities = CapabilityPolicy::project("/project");
        let constitution = OwnerConstitution::secure_defaults();
        for effect in [CapabilityEffect::Delete, CapabilityEffect::Publish] {
            let decision = evaluate_capability(
                AutonomyPreset::Autonomous,
                &constitution,
                &capabilities,
                &capability_request(
                    ActionClass::ExternalPublication,
                    effect,
                    ResourceKind::File,
                    "/outside/release",
                ),
            );
            assert!(matches!(decision, PolicyDecision::Deny { .. }));
        }
    }
}
