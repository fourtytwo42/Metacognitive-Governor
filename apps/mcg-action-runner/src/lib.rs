use std::io::{Read as _, Write as _};
use std::path::{Component, Path, PathBuf};

use base64::Engine as _;
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt as _};
use cap_std::ambient_authority;
use cap_std::fs::{Dir, OpenOptions};
use chrono::{DateTime, Utc};
use mcg_contracts::{
    ActionExecutionId, ActionId, ContentHash, StableTypeId, VersionedPayload,
    decode_canonical_cbor_v1,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use uuid::Uuid;

pub const ACTION_RUNNER_PROTOCOL_VERSION: u32 = 1;
pub const ACTION_RUNNER_REQUEST_MAX_BYTES: usize = 2 * 1024 * 1024;
pub const ACTION_RUNNER_CONTENT_MAX_BYTES: u64 = 1024 * 1024;
pub const GOVERNED_ACTION_OPERATION_HASH_DOMAIN: &str = "MetaCog.ActionOperation";
pub const GOVERNED_FILESYSTEM_OPERATION_TYPE: &str = "mcg.governed_filesystem_operation.v1";
pub const GOVERNED_FILESYSTEM_OBSERVATION_TYPE: &str = "mcg.governed_filesystem_observation.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionRunnerRequest {
    pub protocol_version: u32,
    pub request_id: Uuid,
    pub action_id: ActionId,
    pub execution_id: ActionExecutionId,
    pub capability_root: PathBuf,
    pub operation: VersionedPayload,
    pub operation_hash: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum GovernedFilesystemOperation {
    ReadFile {
        relative_path: String,
        max_bytes: u64,
    },
    CreateFile {
        relative_path: String,
        content_base64: String,
    },
    RemoveCreatedFile {
        relative_path: String,
        expected_sha256: String,
    },
    InspectFile {
        relative_path: String,
        max_bytes: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunnerEffectOutcome {
    Observational,
    NoEffectConfirmed,
    EffectConfirmed,
    OutcomeUnknown,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernedFilesystemObservation {
    pub relative_path: String,
    pub exists: bool,
    pub sha256: Option<String>,
    pub size_bytes: Option<u64>,
    pub content_base64: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionRunnerResponse {
    pub protocol_version: u32,
    pub request_id: Option<Uuid>,
    pub action_id: Option<ActionId>,
    pub execution_id: Option<ActionExecutionId>,
    pub outcome: RunnerEffectOutcome,
    pub observation: Option<VersionedPayload>,
    pub error_code: Option<String>,
    pub detail: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilesystemRunnerQualification {
    pub protocol_version: u32,
    pub created: RunnerEffectOutcome,
    pub reconciled: RunnerEffectOutcome,
    pub duplicate: RunnerEffectOutcome,
    pub compensated: RunnerEffectOutcome,
    pub final_absent: bool,
}

/// Canonically encodes a governed filesystem operation and binds its fixed hash domain.
///
/// # Errors
///
/// Returns an error when the operation cannot be serialized, canonically encoded, or hashed.
pub fn governed_filesystem_operation_payload(
    operation: &GovernedFilesystemOperation,
) -> Result<(VersionedPayload, ContentHash), String> {
    let value = serde_json::to_value(operation).map_err(|error| error.to_string())?;
    let payload = VersionedPayload::canonical_v1(
        StableTypeId::parse(GOVERNED_FILESYSTEM_OPERATION_TYPE)
            .map_err(|error| error.to_string())?,
        &value,
    )
    .map_err(|error| error.to_string())?;
    let hash = payload
        .content_hash(GOVERNED_ACTION_OPERATION_HASH_DOMAIN)
        .map_err(|error| error.to_string())?;
    Ok((payload, hash))
}

/// Exercises the packaged runner in an explicitly supplied empty real directory.
///
/// The qualification creates one unique file, reconciles a deliberately discarded
/// response by inspection, proves duplicate dispatch has no effect, compensates the
/// exact bytes, and verifies that the directory is empty again.
///
/// # Errors
///
/// Returns an error when the root is not an empty real directory, any governed step
/// fails, an observation differs from the expected bytes, or cleanup cannot be proven.
pub fn qualify_filesystem_runner(root: &Path) -> Result<FilesystemRunnerQualification, String> {
    CapabilityRoot::open(root).map_err(|error| error.to_string())?;
    let mut entries = std::fs::read_dir(root).map_err(|error| error.to_string())?;
    if entries
        .next()
        .transpose()
        .map_err(|error| error.to_string())?
        .is_some()
    {
        return Err("qualification root must be empty".into());
    }

    let relative_path = format!("mcg-action-runner-{}.txt", Uuid::new_v4());
    let content = b"MetaCog governed filesystem runner qualification\n";
    let expected_sha256 = sha256(content);
    let create = qualification_request(
        root,
        &GovernedFilesystemOperation::CreateFile {
            relative_path: relative_path.clone(),
            content_base64: base64::engine::general_purpose::STANDARD.encode(content),
        },
    )?;
    let inspect = qualification_request(
        root,
        &GovernedFilesystemOperation::InspectFile {
            relative_path: relative_path.clone(),
            max_bytes: ACTION_RUNNER_CONTENT_MAX_BYTES,
        },
    )?;
    let compensate = qualification_request(
        root,
        &GovernedFilesystemOperation::RemoveCreatedFile {
            relative_path: relative_path.clone(),
            expected_sha256: expected_sha256.clone(),
        },
    )?;

    let qualified = (|| {
        let created = invoke_request(&create)?;
        require_outcome(&created, RunnerEffectOutcome::EffectConfirmed, "create")?;

        let reconciled = invoke_request(&inspect)?;
        require_outcome(
            &reconciled,
            RunnerEffectOutcome::Observational,
            "reconciliation inspection",
        )?;
        let observed = decode_observation(&reconciled)?;
        if !observed.exists || observed.sha256.as_deref() != Some(expected_sha256.as_str()) {
            return Err(
                "qualification reconciliation did not observe the exact created file".into(),
            );
        }

        let duplicate = invoke_request(&create)?;
        require_outcome(
            &duplicate,
            RunnerEffectOutcome::NoEffectConfirmed,
            "duplicate create",
        )?;

        let compensated = invoke_request(&compensate)?;
        require_outcome(
            &compensated,
            RunnerEffectOutcome::EffectConfirmed,
            "compensation",
        )?;
        if decode_observation(&compensated)?.exists {
            return Err("qualification compensation did not leave the target absent".into());
        }

        Ok(FilesystemRunnerQualification {
            protocol_version: ACTION_RUNNER_PROTOCOL_VERSION,
            created: created.outcome,
            reconciled: reconciled.outcome,
            duplicate: duplicate.outcome,
            compensated: compensated.outcome,
            final_absent: true,
        })
    })();

    if qualified.is_err() && root.join(&relative_path).exists() {
        let _ = invoke_request(&compensate);
    }
    if root.join(&relative_path).exists() {
        return Err("qualification could not restore its empty root".into());
    }
    if std::fs::read_dir(root)
        .map_err(|error| error.to_string())?
        .next()
        .transpose()
        .map_err(|error| error.to_string())?
        .is_some()
    {
        return Err("qualification root is not empty after cleanup".into());
    }
    qualified
}

impl ActionRunnerResponse {
    fn malformed(started_at: DateTime<Utc>, code: &str, detail: impl Into<String>) -> Self {
        Self {
            protocol_version: ACTION_RUNNER_PROTOCOL_VERSION,
            request_id: None,
            action_id: None,
            execution_id: None,
            outcome: RunnerEffectOutcome::Denied,
            observation: None,
            error_code: Some(code.into()),
            detail: detail.into(),
            started_at,
            finished_at: Utc::now(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum RunnerFailure {
    #[error("{detail}")]
    Denied { code: &'static str, detail: String },
    #[error("{detail}")]
    NoEffect { code: &'static str, detail: String },
    #[error("{detail}")]
    Unknown { code: &'static str, detail: String },
}

impl RunnerFailure {
    const fn outcome(&self) -> RunnerEffectOutcome {
        match self {
            Self::Denied { .. } => RunnerEffectOutcome::Denied,
            Self::NoEffect { .. } => RunnerEffectOutcome::NoEffectConfirmed,
            Self::Unknown { .. } => RunnerEffectOutcome::OutcomeUnknown,
        }
    }

    const fn code(&self) -> &'static str {
        match self {
            Self::Denied { code, .. }
            | Self::NoEffect { code, .. }
            | Self::Unknown { code, .. } => code,
        }
    }
}

struct CapabilityRoot {
    canonical: PathBuf,
    directory: Dir,
}

/// Executes one bounded request without retaining authority between invocations.
#[must_use]
pub fn execute_request(bytes: &[u8]) -> ActionRunnerResponse {
    let started_at = Utc::now();
    if bytes.len() > ACTION_RUNNER_REQUEST_MAX_BYTES {
        return ActionRunnerResponse::malformed(
            started_at,
            "request_too_large",
            "action runner request exceeds its byte limit",
        );
    }
    let request: ActionRunnerRequest = match serde_json::from_slice(bytes) {
        Ok(request) => request,
        Err(error) => {
            return ActionRunnerResponse::malformed(
                started_at,
                "request_invalid",
                format!("decode action runner request: {error}"),
            );
        }
    };
    let result = execute(&request);
    let (outcome, observation, error_code, detail) = match result {
        Ok((outcome, observation, detail)) => (outcome, observation, None, detail),
        Err(error) => (
            error.outcome(),
            None,
            Some(error.code().into()),
            error.to_string(),
        ),
    };
    ActionRunnerResponse {
        protocol_version: ACTION_RUNNER_PROTOCOL_VERSION,
        request_id: Some(request.request_id),
        action_id: Some(request.action_id),
        execution_id: Some(request.execution_id),
        outcome,
        observation,
        error_code,
        detail,
        started_at,
        finished_at: Utc::now(),
    }
}

fn execute(
    request: &ActionRunnerRequest,
) -> Result<(RunnerEffectOutcome, Option<VersionedPayload>, String), RunnerFailure> {
    if request.protocol_version != ACTION_RUNNER_PROTOCOL_VERSION {
        return Err(denied(
            "protocol_unsupported",
            "action runner protocol version is unsupported",
        ));
    }
    if request.operation.type_id.as_str() != GOVERNED_FILESYSTEM_OPERATION_TYPE {
        return Err(denied(
            "operation_type_denied",
            "operation is not a governed filesystem operation",
        ));
    }
    validate_operation_binding(request)?;
    let value = decode_canonical_cbor_v1(&request.operation.bytes)
        .map_err(|error| denied("operation_invalid", error.to_string()))?;
    let operation: GovernedFilesystemOperation = serde_json::from_value(value)
        .map_err(|error| denied("operation_invalid", error.to_string()))?;
    let root = CapabilityRoot::open(&request.capability_root)?;
    let (outcome, observation, detail) = execute_operation(&root, operation)?;
    encode_observation(outcome, &observation, detail)
}

fn validate_operation_binding(request: &ActionRunnerRequest) -> Result<(), RunnerFailure> {
    if request.operation_hash.domain != GOVERNED_ACTION_OPERATION_HASH_DOMAIN {
        return Err(denied(
            "operation_hash_domain_mismatch",
            "operation hash does not use the governed action domain",
        ));
    }
    let actual_hash = request
        .operation
        .content_hash(&request.operation_hash.domain)
        .map_err(|error| denied("operation_invalid", error.to_string()))?;
    if actual_hash != request.operation_hash {
        return Err(denied(
            "operation_hash_mismatch",
            "operation bytes do not match the authorized hash",
        ));
    }
    Ok(())
}

fn execute_operation(
    root: &CapabilityRoot,
    operation: GovernedFilesystemOperation,
) -> Result<(RunnerEffectOutcome, GovernedFilesystemObservation, String), RunnerFailure> {
    Ok(match operation {
        GovernedFilesystemOperation::ReadFile {
            relative_path,
            max_bytes,
        } => {
            let observation = root.inspect(&relative_path, max_bytes, true)?;
            if !observation.exists {
                return Err(no_effect(
                    "file_not_found",
                    "governed read target does not exist",
                ));
            }
            (
                RunnerEffectOutcome::Observational,
                observation,
                "read bounded regular file".into(),
            )
        }
        GovernedFilesystemOperation::InspectFile {
            relative_path,
            max_bytes,
        } => (
            RunnerEffectOutcome::Observational,
            root.inspect(&relative_path, max_bytes, false)?,
            "inspected governed file state".into(),
        ),
        GovernedFilesystemOperation::CreateFile {
            relative_path,
            content_base64,
        } => {
            let content = base64::engine::general_purpose::STANDARD
                .decode(content_base64)
                .map_err(|error| denied("content_invalid", error.to_string()))?;
            if u64::try_from(content.len()).unwrap_or(u64::MAX) > ACTION_RUNNER_CONTENT_MAX_BYTES {
                return Err(denied(
                    "content_too_large",
                    "governed file content exceeds its byte limit",
                ));
            }
            let observation = root.create_new(&relative_path, &content)?;
            (
                RunnerEffectOutcome::EffectConfirmed,
                observation,
                "created governed file exactly once".into(),
            )
        }
        GovernedFilesystemOperation::RemoveCreatedFile {
            relative_path,
            expected_sha256,
        } => {
            validate_sha256(&expected_sha256)?;
            let observation = root.remove_exact(&relative_path, &expected_sha256)?;
            (
                RunnerEffectOutcome::EffectConfirmed,
                observation,
                "removed the exact governed file as compensation".into(),
            )
        }
    })
}

fn encode_observation(
    outcome: RunnerEffectOutcome,
    observation: &GovernedFilesystemObservation,
    detail: String,
) -> Result<(RunnerEffectOutcome, Option<VersionedPayload>, String), RunnerFailure> {
    let observation_value = serde_json::to_value(observation)
        .map_err(|error| denied("observation_invalid", error.to_string()))?;
    let payload = VersionedPayload::canonical_v1(
        StableTypeId::parse(GOVERNED_FILESYSTEM_OBSERVATION_TYPE)
            .map_err(|error| denied("observation_invalid", error.to_string()))?,
        &observation_value,
    )
    .map_err(|error| denied("observation_invalid", error.to_string()))?;
    Ok((outcome, Some(payload), detail))
}

impl CapabilityRoot {
    fn open(path: &Path) -> Result<Self, RunnerFailure> {
        if !path.is_absolute() {
            return Err(denied(
                "capability_root_invalid",
                "capability root must be absolute",
            ));
        }
        let canonical = path
            .canonicalize()
            .map_err(|error| denied("capability_root_invalid", error.to_string()))?;
        let metadata = std::fs::symlink_metadata(path)
            .map_err(|error| denied("capability_root_invalid", error.to_string()))?;
        if !metadata.is_dir() || is_link_or_reparse(&metadata) {
            return Err(denied(
                "capability_root_invalid",
                "capability root must be a real directory, not a link or reparse point",
            ));
        }
        let directory = Dir::open_ambient_dir(&canonical, ambient_authority())
            .map_err(|error| denied("capability_root_invalid", error.to_string()))?;
        Ok(Self {
            canonical,
            directory,
        })
    }

    fn relative(&self, value: &str) -> Result<PathBuf, RunnerFailure> {
        let path = Path::new(value);
        if value.trim().is_empty()
            || path.is_absolute()
            || path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(denied(
                "path_denied",
                "governed filesystem path must be normalized and relative",
            ));
        }
        let mut current = self.canonical.clone();
        let components = path.components().collect::<Vec<_>>();
        for (index, component) in components.iter().enumerate() {
            let Component::Normal(component) = component else {
                unreachable!("validated path component");
            };
            current.push(component);
            match std::fs::symlink_metadata(&current) {
                Ok(metadata) => {
                    if is_link_or_reparse(&metadata) {
                        return Err(denied(
                            "path_link_denied",
                            "governed path traverses a symlink or reparse point",
                        ));
                    }
                    if index + 1 < components.len() && !metadata.is_dir() {
                        return Err(denied(
                            "path_parent_invalid",
                            "governed path parent is not a directory",
                        ));
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    if index + 1 < components.len() {
                        return Err(denied(
                            "path_parent_missing",
                            "governed path parent does not exist",
                        ));
                    }
                }
                Err(error) => return Err(denied("path_inspection_failed", error.to_string())),
            }
        }
        Ok(path.to_path_buf())
    }

    fn inspect(
        &self,
        relative_path: &str,
        max_bytes: u64,
        include_content: bool,
    ) -> Result<GovernedFilesystemObservation, RunnerFailure> {
        if max_bytes == 0 || max_bytes > ACTION_RUNNER_CONTENT_MAX_BYTES {
            return Err(denied(
                "read_limit_invalid",
                "read limit must be within the governed content bound",
            ));
        }
        let relative = self.relative(relative_path)?;
        let metadata = match self.directory.symlink_metadata(&relative) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(GovernedFilesystemObservation {
                    relative_path: relative_path.into(),
                    exists: false,
                    sha256: None,
                    size_bytes: None,
                    content_base64: None,
                });
            }
            Err(error) => return Err(no_effect("file_inspection_failed", error.to_string())),
        };
        if !metadata.is_file() || metadata.is_symlink() {
            return Err(denied(
                "file_type_denied",
                "governed target must be a regular non-link file",
            ));
        }
        if metadata.len() > max_bytes {
            return Err(denied(
                "file_too_large",
                "governed target exceeds the authorized read limit",
            ));
        }
        let mut options = OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        let file = self
            .directory
            .open_with(&relative, &options)
            .map_err(|error| no_effect("file_open_failed", error.to_string()))?;
        let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(0));
        file.take(max_bytes.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|error| no_effect("file_read_failed", error.to_string()))?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
            return Err(denied(
                "file_too_large",
                "governed target changed beyond the authorized read limit",
            ));
        }
        Ok(observation(relative_path, Some(&bytes), include_content))
    }

    fn create_new(
        &self,
        relative_path: &str,
        bytes: &[u8],
    ) -> Result<GovernedFilesystemObservation, RunnerFailure> {
        let relative = self.relative(relative_path)?;
        if self.directory.symlink_metadata(&relative).is_ok() {
            return Err(no_effect(
                "destination_exists",
                "create-new destination already exists",
            ));
        }
        let parent = relative
            .parent()
            .ok_or_else(|| denied("path_parent_invalid", "governed file has no parent"))?;
        let name = relative
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| denied("path_name_invalid", "governed file name is invalid"))?;
        let temporary = parent.join(format!(".{name}.{}.mcg-action", Uuid::new_v4()));
        let mut options = OpenOptions::new();
        options
            .write(true)
            .create_new(true)
            .follow(FollowSymlinks::No);
        let mut file = self
            .directory
            .open_with(&temporary, &options)
            .map_err(|error| no_effect("temporary_create_failed", error.to_string()))?;
        if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
            let _ = self.directory.remove_file(&temporary);
            return Err(no_effect("temporary_write_failed", error.to_string()));
        }
        drop(file);
        if let Err(error) = self
            .directory
            .hard_link(&temporary, &self.directory, &relative)
        {
            let _ = self.directory.remove_file(&temporary);
            return if self.directory.symlink_metadata(&relative).is_ok() {
                Err(no_effect(
                    "destination_exists",
                    "create-new destination appeared before commit",
                ))
            } else {
                Err(no_effect("file_publish_failed", error.to_string()))
            };
        }
        let _ = self.directory.remove_file(&temporary);
        match self.inspect(relative_path, ACTION_RUNNER_CONTENT_MAX_BYTES, false) {
            Ok(observed) if observed.sha256.as_deref() == Some(&sha256(bytes)) => Ok(observed),
            Ok(_) => Err(unknown(
                "post_write_mismatch",
                "published file does not match authorized content",
            )),
            Err(error) => Err(unknown("post_write_unknown", error.to_string())),
        }
    }

    fn remove_exact(
        &self,
        relative_path: &str,
        expected_sha256: &str,
    ) -> Result<GovernedFilesystemObservation, RunnerFailure> {
        let relative = self.relative(relative_path)?;
        let before = self.inspect(relative_path, ACTION_RUNNER_CONTENT_MAX_BYTES, false)?;
        if !before.exists {
            return Err(no_effect(
                "file_not_found",
                "compensation target is already absent",
            ));
        }
        if before.sha256.as_deref() != Some(expected_sha256) {
            return Err(no_effect(
                "precondition_failed",
                "compensation target hash changed",
            ));
        }
        let parent = relative
            .parent()
            .ok_or_else(|| denied("path_parent_invalid", "governed file has no parent"))?;
        let name = relative
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| denied("path_name_invalid", "governed file name is invalid"))?;
        let quarantine = parent.join(format!(".{name}.{}.mcg-remove", Uuid::new_v4()));
        self.directory
            .rename(&relative, &self.directory, &quarantine)
            .map_err(|error| no_effect("compensation_rename_failed", error.to_string()))?;
        let quarantined = inspect_relative(
            &self.directory,
            &quarantine,
            ACTION_RUNNER_CONTENT_MAX_BYTES,
        );
        match quarantined {
            Ok(bytes) if sha256(&bytes) == expected_sha256 => {}
            Ok(_) => {
                if self
                    .directory
                    .rename(&quarantine, &self.directory, &relative)
                    .is_err()
                {
                    return Err(unknown(
                        "compensation_restore_failed",
                        "changed target was quarantined but could not be restored",
                    ));
                }
                return Err(no_effect(
                    "precondition_raced",
                    "compensation target changed before removal",
                ));
            }
            Err(error) => {
                let _ = self
                    .directory
                    .rename(&quarantine, &self.directory, &relative);
                return Err(unknown("compensation_inspection_failed", error.clone()));
            }
        }
        self.directory
            .remove_file(&quarantine)
            .map_err(|error| unknown("compensation_remove_failed", error.to_string()))?;
        let after = self.inspect(relative_path, ACTION_RUNNER_CONTENT_MAX_BYTES, false)?;
        if after.exists {
            return Err(unknown(
                "compensation_postcondition_unknown",
                "a file exists at the compensated path after exact removal",
            ));
        }
        Ok(after)
    }
}

fn inspect_relative(directory: &Dir, relative: &Path, max_bytes: u64) -> Result<Vec<u8>, String> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let file = directory
        .open_with(relative, &options)
        .map_err(|error| error.to_string())?;
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() > max_bytes {
        return Err("governed file is not a bounded regular file".into());
    }
    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(0));
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err("governed file exceeded its bound while reading".into());
    }
    Ok(bytes)
}

fn observation(
    relative_path: &str,
    bytes: Option<&[u8]>,
    include_content: bool,
) -> GovernedFilesystemObservation {
    GovernedFilesystemObservation {
        relative_path: relative_path.into(),
        exists: bytes.is_some(),
        sha256: bytes.map(sha256),
        size_bytes: bytes.and_then(|bytes| u64::try_from(bytes.len()).ok()),
        content_base64: (include_content)
            .then(|| bytes.map(|bytes| base64::engine::general_purpose::STANDARD.encode(bytes)))
            .flatten(),
    }
}

fn qualification_request(
    root: &Path,
    operation: &GovernedFilesystemOperation,
) -> Result<ActionRunnerRequest, String> {
    let (operation, operation_hash) = governed_filesystem_operation_payload(operation)?;
    Ok(ActionRunnerRequest {
        protocol_version: ACTION_RUNNER_PROTOCOL_VERSION,
        request_id: Uuid::new_v4(),
        action_id: ActionId::new(),
        execution_id: ActionExecutionId::new(),
        capability_root: root.into(),
        operation,
        operation_hash,
    })
}

fn invoke_request(request: &ActionRunnerRequest) -> Result<ActionRunnerResponse, String> {
    let bytes = serde_json::to_vec(request).map_err(|error| error.to_string())?;
    Ok(execute_request(&bytes))
}

fn require_outcome(
    response: &ActionRunnerResponse,
    expected: RunnerEffectOutcome,
    step: &str,
) -> Result<(), String> {
    if response.outcome == expected {
        Ok(())
    } else {
        Err(format!(
            "qualification {step} returned {:?}: {}",
            response.outcome, response.detail
        ))
    }
}

fn decode_observation(
    response: &ActionRunnerResponse,
) -> Result<GovernedFilesystemObservation, String> {
    let payload = response
        .observation
        .as_ref()
        .ok_or_else(|| "qualification response omitted its observation".to_string())?;
    let value = decode_canonical_cbor_v1(&payload.bytes).map_err(|error| error.to_string())?;
    serde_json::from_value(value).map_err(|error| error.to_string())
}

fn validate_sha256(value: &str) -> Result<(), RunnerFailure> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(denied(
            "sha256_invalid",
            "expected SHA-256 must be 64 lowercase hexadecimal characters",
        ));
    }
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn denied(code: &'static str, detail: impl Into<String>) -> RunnerFailure {
    RunnerFailure::Denied {
        code,
        detail: detail.into(),
    }
}

fn no_effect(code: &'static str, detail: impl Into<String>) -> RunnerFailure {
    RunnerFailure::NoEffect {
        code,
        detail: detail.into(),
    }
}

fn unknown(code: &'static str, detail: impl Into<String>) -> RunnerFailure {
    RunnerFailure::Unknown {
        code,
        detail: detail.into(),
    }
}

#[cfg(windows)]
fn is_link_or_reparse(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_link_or_reparse(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::needless_pass_by_value)]
    fn request(root: &Path, operation: GovernedFilesystemOperation) -> ActionRunnerRequest {
        let (operation, operation_hash) =
            governed_filesystem_operation_payload(&operation).unwrap();
        ActionRunnerRequest {
            protocol_version: ACTION_RUNNER_PROTOCOL_VERSION,
            request_id: Uuid::new_v4(),
            action_id: ActionId::new(),
            execution_id: ActionExecutionId::new(),
            capability_root: root.into(),
            operation,
            operation_hash,
        }
    }

    fn run(request: &ActionRunnerRequest) -> ActionRunnerResponse {
        execute_request(&serde_json::to_vec(request).unwrap())
    }

    fn decoded(response: &ActionRunnerResponse) -> GovernedFilesystemObservation {
        let payload = response.observation.as_ref().unwrap();
        serde_json::from_value(decode_canonical_cbor_v1(&payload.bytes).unwrap()).unwrap()
    }

    #[test]
    fn create_lost_response_reconcile_and_compensate_exactly_once() {
        let root = tempfile::tempdir().unwrap();
        let content = b"governed effect";
        let create = request(
            root.path(),
            GovernedFilesystemOperation::CreateFile {
                relative_path: "result.txt".into(),
                content_base64: base64::engine::general_purpose::STANDARD.encode(content),
            },
        );
        let created = run(&create);
        assert_eq!(created.outcome, RunnerEffectOutcome::EffectConfirmed);
        assert_eq!(
            std::fs::read(root.path().join("result.txt")).unwrap(),
            content
        );

        let inspect = request(
            root.path(),
            GovernedFilesystemOperation::InspectFile {
                relative_path: "result.txt".into(),
                max_bytes: 100,
            },
        );
        let reconciled = run(&inspect);
        assert_eq!(reconciled.outcome, RunnerEffectOutcome::Observational);
        let expected_sha256 = sha256(content);
        assert_eq!(
            decoded(&reconciled).sha256.as_deref(),
            Some(expected_sha256.as_str())
        );

        let duplicate = run(&create);
        assert_eq!(duplicate.outcome, RunnerEffectOutcome::NoEffectConfirmed);
        assert_eq!(duplicate.error_code.as_deref(), Some("destination_exists"));
        assert_eq!(
            std::fs::read(root.path().join("result.txt")).unwrap(),
            content
        );

        let compensate = request(
            root.path(),
            GovernedFilesystemOperation::RemoveCreatedFile {
                relative_path: "result.txt".into(),
                expected_sha256: sha256(content),
            },
        );
        let removed = run(&compensate);
        assert_eq!(removed.outcome, RunnerEffectOutcome::EffectConfirmed);
        assert!(!decoded(&removed).exists);
        assert!(!root.path().join("result.txt").exists());
        let repeated = run(&compensate);
        assert_eq!(repeated.outcome, RunnerEffectOutcome::NoEffectConfirmed);
    }

    #[test]
    fn traversal_absolute_paths_hash_substitution_and_changed_compensation_are_denied() {
        let root = tempfile::tempdir().unwrap();
        for relative_path in ["../escape.txt", ".", "nested/../escape.txt"] {
            let response = run(&request(
                root.path(),
                GovernedFilesystemOperation::InspectFile {
                    relative_path: relative_path.into(),
                    max_bytes: 100,
                },
            ));
            assert_eq!(response.outcome, RunnerEffectOutcome::Denied);
        }
        let mut substituted = request(
            root.path(),
            GovernedFilesystemOperation::InspectFile {
                relative_path: "missing.txt".into(),
                max_bytes: 100,
            },
        );
        substituted.operation.bytes.push(0);
        assert_eq!(run(&substituted).outcome, RunnerEffectOutcome::Denied);
        let mut wrong_domain = request(
            root.path(),
            GovernedFilesystemOperation::InspectFile {
                relative_path: "missing.txt".into(),
                max_bytes: 100,
            },
        );
        wrong_domain.operation_hash = wrong_domain
            .operation
            .content_hash("AttackerSelectedDomain")
            .unwrap();
        let response = run(&wrong_domain);
        assert_eq!(response.outcome, RunnerEffectOutcome::Denied);
        assert_eq!(
            response.error_code.as_deref(),
            Some("operation_hash_domain_mismatch")
        );

        std::fs::write(root.path().join("changed.txt"), "new owner content").unwrap();
        let response = run(&request(
            root.path(),
            GovernedFilesystemOperation::RemoveCreatedFile {
                relative_path: "changed.txt".into(),
                expected_sha256: sha256(b"old content"),
            },
        ));
        assert_eq!(response.outcome, RunnerEffectOutcome::NoEffectConfirmed);
        assert_eq!(
            std::fs::read_to_string(root.path().join("changed.txt")).unwrap(),
            "new owner content"
        );
    }

    #[test]
    fn package_qualification_restores_an_empty_root() {
        let root = tempfile::tempdir().unwrap();
        let report = qualify_filesystem_runner(root.path()).unwrap();
        assert_eq!(report.created, RunnerEffectOutcome::EffectConfirmed);
        assert_eq!(report.reconciled, RunnerEffectOutcome::Observational);
        assert_eq!(report.duplicate, RunnerEffectOutcome::NoEffectConfirmed);
        assert_eq!(report.compensated, RunnerEffectOutcome::EffectConfirmed);
        assert!(report.final_absent);
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);

        std::fs::write(root.path().join("owner-file"), b"preserve").unwrap();
        assert!(qualify_filesystem_runner(root.path()).is_err());
        assert_eq!(
            std::fs::read(root.path().join("owner-file")).unwrap(),
            b"preserve"
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_targets_and_ancestors_are_denied() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret.txt"), "secret").unwrap();
        symlink(outside.path(), root.path().join("linked")).unwrap();
        let response = run(&request(
            root.path(),
            GovernedFilesystemOperation::ReadFile {
                relative_path: "linked/secret.txt".into(),
                max_bytes: 100,
            },
        ));
        assert_eq!(response.outcome, RunnerEffectOutcome::Denied);
        assert_eq!(response.error_code.as_deref(), Some("path_link_denied"));
    }
}
