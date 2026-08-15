use std::path::Path;

use async_trait::async_trait;
use mcg_agents::SuccessCondition;
use mcg_tools::{ArtifactReference, ArtifactStore};
use serde::{Deserialize, Serialize};

pub(crate) const SHADOW_COMMAND_TRANSCRIPT_SCHEMA_VERSION: u16 = 1;

/// Credential-free, bounded command result interpreted by the trusted runtime.
/// Output bodies belong in separate bounded artifacts; this record binds the
/// exact condition to the process disposition used for pass/fail derivation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ShadowCommandTranscript {
    pub schema_version: u16,
    pub condition: SuccessCondition,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub stdout_sha256: Option<String>,
    pub stderr_sha256: Option<String>,
}

/// Outcome returned by an operating-system-level shadow command sandbox.
///
/// A completed result is trusted only when it carries an immutable transcript
/// artifact digest. `Unavailable` is deliberately distinct from command
/// failure: it keeps the objective inconclusive instead of pretending that a
/// host process launched with a restricted working directory was isolated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ShadowCommandVerification {
    Unavailable,
    #[allow(
        dead_code,
        reason = "constructed by qualified host backends; the safe default is unavailable"
    )]
    Completed {
        transcript: ArtifactReference,
    },
}

/// Capability boundary for build, test, and static-analysis verification in a
/// retained shadow workspace.
///
/// Implementations must provide operating-system enforcement for filesystem,
/// process, credential, and network isolation. A working-directory check alone
/// does not satisfy this contract.
#[async_trait]
pub(crate) trait ShadowCommandVerifier: Send + Sync {
    async fn verify(
        &self,
        workspace_root: &Path,
        condition: &SuccessCondition,
        artifacts: &ArtifactStore,
    ) -> ShadowCommandVerification;
}

/// Production-safe default until a qualified host sandbox backend is present.
#[derive(Debug, Default)]
pub(crate) struct UnavailableShadowCommandVerifier;

#[async_trait]
impl ShadowCommandVerifier for UnavailableShadowCommandVerifier {
    async fn verify(
        &self,
        _workspace_root: &Path,
        _condition: &SuccessCondition,
        _artifacts: &ArtifactStore,
    ) -> ShadowCommandVerification {
        ShadowCommandVerification::Unavailable
    }
}
