use chrono::{DateTime, Utc};
use mcg_contracts::{
    AttemptId, CompletionCertificate, CompletionCertificateId, CompletionVerdict, ContentHash,
    ContextManifest, InvocationId, InvocationInspection, InvocationManifest, InvocationRole,
    InvocationSummary, ModelRun, ModelRunId, ModelRunRecord, ModelRunState,
    RuntimeInvocationResult, StableTypeId, VersionedPayload, WorkflowAttempt, WorkflowId,
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Serialize;

use crate::{SqliteEventStore, StorageError};

const CONTEXT_MANIFEST_HASH_DOMAIN: &str = "MetaCog.ContextManifest";
const INVOCATION_MANIFEST_HASH_DOMAIN: &str = "MetaCog.InvocationManifest";
const MODEL_RUN_HASH_DOMAIN: &str = "MetaCog.ModelRun";
const COMPLETION_CERTIFICATE_HASH_DOMAIN: &str = "MetaCog.CompletionCertificate";
pub const MAX_INVOCATION_INSPECTION_ITEMS: usize = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RuntimeResponseArtifact {
    pub model_run_id: ModelRunId,
    pub invocation_id: InvocationId,
    pub response_hash: ContentHash,
    pub response: VersionedPayload,
    pub recorded_at: DateTime<Utc>,
}

impl SqliteEventStore {
    /// Allocates the next durable cognition-worker generation under Root's single writer.
    /// The value survives restart and can never be reset or skipped by ordinary storage APIs.
    ///
    /// # Errors
    /// Returns a storage error for overflow, conflict, or transaction failure.
    pub fn allocate_runtime_worker_generation(
        &mut self,
        now: DateTime<Utc>,
    ) -> Result<u64, StorageError> {
        let transaction = self.connection.transaction()?;
        let current = transaction
            .query_row(
                "SELECT generation FROM runtime_worker_generations WHERE role = 'cognition'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        let generation = match current {
            None => {
                transaction.execute(
                    "INSERT INTO runtime_worker_generations(role, generation, updated_at)
                     VALUES ('cognition', 1, ?1)",
                    [now.to_rfc3339()],
                )?;
                1
            }
            Some(current) => {
                let current = u64::try_from(current).map_err(|_| {
                    StorageError::InvalidData("negative Runtime worker generation".into())
                })?;
                let generation = current.checked_add(1).ok_or_else(|| {
                    StorageError::InvalidData("Runtime worker generation overflow".into())
                })?;
                let changed = transaction.execute(
                    "UPDATE runtime_worker_generations SET generation = ?1, updated_at = ?2
                     WHERE role = 'cognition' AND generation = ?3",
                    params![
                        sql_u64(generation, "Runtime worker generation")?,
                        now.to_rfc3339(),
                        sql_u64(current, "current Runtime worker generation")?,
                    ],
                )?;
                if changed != 1 {
                    return Err(StorageError::Conflict(
                        "Runtime worker generation changed before allocation".into(),
                    ));
                }
                generation
            }
        };
        transaction.commit()?;
        Ok(generation)
    }

    /// Returns the last allocated cognition-worker generation.
    ///
    /// # Errors
    /// Returns a storage error for query or invalid persisted data.
    pub fn runtime_worker_generation(&self) -> Result<Option<u64>, StorageError> {
        self.connection
            .query_row(
                "SELECT generation FROM runtime_worker_generations WHERE role = 'cognition'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .map(|generation| {
                u64::try_from(generation).map_err(|_| {
                    StorageError::InvalidData("negative Runtime worker generation".into())
                })
            })
            .transpose()
    }

    /// Persists the exact immutable Runtime response behind a completed model-run artifact ref.
    /// Exact replay is idempotent; identity or payload substitution fails closed.
    ///
    /// # Errors
    /// Returns a storage error for malformed results, missing lineage, substitution, or I/O.
    pub fn persist_runtime_response_artifact(
        &mut self,
        result: &RuntimeInvocationResult,
        recorded_at: DateTime<Utc>,
    ) -> Result<RuntimeResponseArtifact, StorageError> {
        result
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let run = load_model_run_head(&self.connection, result.model_run_id)?.ok_or_else(|| {
            StorageError::Conflict("runtime response model run does not exist".into())
        })?;
        if run.run.invocation_id != result.invocation_id || run.run.role != result.role {
            return Err(StorageError::Conflict(
                "runtime response does not bind the model-run invocation and role".into(),
            ));
        }
        let manifest =
            load_invocation(&self.connection, result.invocation_id)?.ok_or_else(|| {
                StorageError::Conflict("runtime response invocation does not exist".into())
            })?;
        if manifest.request_hash != result.request_hash || manifest.role != result.role {
            return Err(StorageError::Conflict(
                "runtime response does not bind the exact invocation request".into(),
            ));
        }
        let artifact = RuntimeResponseArtifact {
            model_run_id: result.model_run_id,
            invocation_id: result.invocation_id,
            response_hash: result.response_hash.clone(),
            response: result.response.clone(),
            recorded_at,
        };
        if let Some(existing) = self.runtime_response_artifact(result.model_run_id)? {
            if existing.model_run_id != artifact.model_run_id
                || existing.invocation_id != artifact.invocation_id
                || existing.response_hash != artifact.response_hash
                || existing.response != artifact.response
            {
                return Err(StorageError::Conflict(
                    "runtime response identity was replayed with different content".into(),
                ));
            }
            return Ok(existing);
        }
        self.connection.execute(
            "INSERT INTO runtime_response_artifacts(
                model_run_id, invocation_id, response_hash_json, response_payload_json, recorded_at
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                result.model_run_id.to_string(),
                result.invocation_id.to_string(),
                serde_json::to_string(&result.response_hash)?,
                serde_json::to_string(&result.response)?,
                recorded_at.to_rfc3339(),
            ],
        )?;
        Ok(artifact)
    }

    /// Loads the exact response retained for one model run.
    ///
    /// # Errors
    /// Returns a storage error for query or stored-data failures.
    pub fn runtime_response_artifact(
        &self,
        model_run_id: ModelRunId,
    ) -> Result<Option<RuntimeResponseArtifact>, StorageError> {
        self.connection
            .query_row(
                "SELECT invocation_id, response_hash_json, response_payload_json, recorded_at
                 FROM runtime_response_artifacts WHERE model_run_id = ?1",
                [model_run_id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?
            .map(|(invocation_id, response_hash, response, recorded_at)| {
                Ok(RuntimeResponseArtifact {
                    model_run_id,
                    invocation_id: InvocationId(uuid::Uuid::parse_str(&invocation_id).map_err(
                        |_| StorageError::InvalidData("invalid response invocation id".into()),
                    )?),
                    response_hash: serde_json::from_str(&response_hash)?,
                    response: serde_json::from_str(&response)?,
                    recorded_at: DateTime::parse_from_rfc3339(&recorded_at)
                        .map_err(|error| StorageError::InvalidData(error.to_string()))?
                        .with_timezone(&Utc),
                })
            })
            .transpose()
    }

    /// Persists one immutable, content-addressed record of exactly what reached a model.
    ///
    /// Exact replay returns the original hash. Reusing an identity with different content fails.
    ///
    /// # Errors
    /// Returns a storage error for invalid content, missing workflow lineage, conflicting replay,
    /// or a failed transaction.
    pub fn persist_context_manifest(
        &mut self,
        manifest: &ContextManifest,
        persisted_at: DateTime<Utc>,
    ) -> Result<ContentHash, StorageError> {
        manifest
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        load_attempt(&self.connection, manifest.attempt_id)?
            .ok_or(StorageError::WorkflowAttemptNotFound(manifest.attempt_id))?;
        let hash = contract_hash(
            ContextManifest::TYPE_ID,
            CONTEXT_MANIFEST_HASH_DOMAIN,
            manifest,
        )?;
        if let Some((existing_json, existing_hash_json)) = self
            .connection
            .query_row(
                "SELECT manifest_json, content_hash_json FROM context_manifests
                 WHERE context_manifest_id = ?1",
                [manifest.id.to_string()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
        {
            let existing: ContextManifest = serde_json::from_str(&existing_json)?;
            let existing_hash: ContentHash = serde_json::from_str(&existing_hash_json)?;
            if existing != *manifest || existing_hash != hash {
                return Err(StorageError::Conflict(
                    "context-manifest identity was replayed with different content".into(),
                ));
            }
            return Ok(existing_hash);
        }
        self.connection.execute(
            "INSERT INTO context_manifests(
                context_manifest_id, workflow_attempt_id, type_id, schema_version,
                content_hash_json, manifest_json, persisted_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                manifest.id.to_string(),
                manifest.attempt_id.to_string(),
                ContextManifest::TYPE_ID,
                i64::from(ContextManifest::SCHEMA_VERSION),
                serde_json::to_string(&hash)?,
                serde_json::to_string(manifest)?,
                persisted_at.to_rfc3339(),
            ],
        )?;
        Ok(hash)
    }

    /// Persists one immutable invocation reconstruction manifest after verifying all Root-owned
    /// workflow, task-contract, context, runtime-build, and role bindings.
    ///
    /// # Errors
    /// Returns a storage error for invalid or missing lineage, hash substitution, conflicting
    /// replay, or persistence failure.
    pub fn persist_invocation_manifest(
        &mut self,
        manifest: &InvocationManifest,
        persisted_at: DateTime<Utc>,
    ) -> Result<ContentHash, StorageError> {
        manifest
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let attempt = load_attempt(&self.connection, manifest.attempt_id)?
            .ok_or(StorageError::WorkflowAttemptNotFound(manifest.attempt_id))?;
        if attempt.task_contract != manifest.task_contract {
            return Err(StorageError::Conflict(
                "invocation does not bind the attempt's exact task contract".into(),
            ));
        }
        if manifest.runtime_identity.build_hash != manifest.runtime_build {
            return Err(StorageError::Conflict(
                "invocation runtime identity and runtime build hashes differ".into(),
            ));
        }
        let context = load_context_by_hash(&self.connection, &manifest.context_manifest)?
            .ok_or_else(|| {
                StorageError::Conflict(
                    "invocation context hash does not resolve to a persisted manifest".into(),
                )
            })?;
        if context.attempt_id != manifest.attempt_id {
            return Err(StorageError::Conflict(
                "invocation context belongs to a different workflow attempt".into(),
            ));
        }
        if let Some(permit_ref) = &manifest.egress_permit {
            let authorization = self
                .model_egress_authorization(manifest.id)?
                .ok_or_else(|| {
                    StorageError::Conflict(
                        "invocation egress permit does not resolve to Root authority".into(),
                    )
                })?;
            if authorization.permit_ref != *permit_ref
                || authorization.permit.invocation_id != manifest.id
                || authorization.permit.attempt_id != manifest.attempt_id
                || authorization.permit.runtime_identity != manifest.runtime_identity
                || authorization.permit.role != manifest.role
                || authorization.permit.task_contract != manifest.task_contract
                || authorization.permit.request_hash != manifest.request_hash
                || authorization.proposal.disclosure.context_manifest != manifest.context_manifest
                || authorization.permit.resolved_provider != manifest.resolved_provider
                || authorization.permit.resolved_model != manifest.resolved_model
                || crate::model_egress_route_payload(&authorization.proposal.route)?
                    != manifest.model_route
                || persisted_at < authorization.permit.issued_at
                || persisted_at >= authorization.permit.expires_at
            {
                return Err(StorageError::Conflict(
                    "invocation manifest does not bind the exact active Root egress permit".into(),
                ));
            }
        }
        let hash = contract_hash(
            InvocationManifest::TYPE_ID,
            INVOCATION_MANIFEST_HASH_DOMAIN,
            manifest,
        )?;
        if let Some((existing_json, existing_hash_json)) = self
            .connection
            .query_row(
                "SELECT manifest_json, content_hash_json FROM invocation_manifests
                 WHERE invocation_id = ?1",
                [manifest.id.to_string()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
        {
            let existing: InvocationManifest = serde_json::from_str(&existing_json)?;
            let existing_hash: ContentHash = serde_json::from_str(&existing_hash_json)?;
            if existing != *manifest || existing_hash != hash {
                return Err(StorageError::Conflict(
                    "invocation identity was replayed with different content".into(),
                ));
            }
            return Ok(existing_hash);
        }
        self.connection.execute(
            "INSERT INTO invocation_manifests(
                invocation_id, role, workflow_attempt_id, task_contract_id,
                task_contract_version, context_manifest_hash_json, type_id, schema_version,
                content_hash_json, manifest_json, persisted_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                manifest.id.to_string(),
                invocation_role_label(manifest.role),
                manifest.attempt_id.to_string(),
                manifest.task_contract.id.to_string(),
                sql_u64(
                    manifest.task_contract.version,
                    "invocation task-contract version"
                )?,
                serde_json::to_string(&manifest.context_manifest)?,
                InvocationManifest::TYPE_ID,
                i64::from(InvocationManifest::SCHEMA_VERSION),
                serde_json::to_string(&hash)?,
                serde_json::to_string(manifest)?,
                persisted_at.to_rfc3339(),
            ],
        )?;
        Ok(hash)
    }

    /// Creates the immutable first revision of a planned model run.
    ///
    /// # Errors
    /// Returns a storage error for invalid state, missing invocation lineage, replay substitution,
    /// or persistence failure.
    pub fn create_model_run(
        &mut self,
        run: &ModelRun,
        recorded_at: DateTime<Utc>,
    ) -> Result<ModelRunRecord, StorageError> {
        run.validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        if run.state != ModelRunState::Planned {
            return Err(StorageError::InvalidData(
                "a model run must begin in planned state".into(),
            ));
        }
        let manifest = load_invocation(&self.connection, run.invocation_id)?.ok_or_else(|| {
            StorageError::Conflict("model run invocation manifest does not exist".into())
        })?;
        if manifest.role != run.role {
            return Err(StorageError::Conflict(
                "model run role differs from its invocation manifest".into(),
            ));
        }
        if let Some(existing) = load_model_run_head(&self.connection, run.id)? {
            if existing.revision != 1 || existing.run != *run {
                return Err(StorageError::Conflict(
                    "model-run identity was replayed with different content".into(),
                ));
            }
            return Ok(existing);
        }
        let record = model_run_record(1, run, recorded_at)?;
        let transaction = self.connection.transaction()?;
        insert_model_run_version(&transaction, &record)?;
        transaction.execute(
            "INSERT INTO model_run_heads(model_run_id, invocation_id, current_revision)
             VALUES (?1, ?2, 1)",
            params![run.id.to_string(), run.invocation_id.to_string()],
        )?;
        transaction.commit()?;
        Ok(record)
    }

    /// Loads the current authoritative revision for one exact model-run identity.
    ///
    /// # Errors
    /// Returns a storage error when durable evidence cannot be decoded or validated.
    pub fn model_run(
        &self,
        model_run_id: ModelRunId,
    ) -> Result<Option<ModelRunRecord>, StorageError> {
        load_model_run_head(&self.connection, model_run_id)
    }

    /// Appends one generation-fenced model-run state revision and advances its head with CAS.
    ///
    /// # Errors
    /// Returns a storage error for stale revision, illegal transition, mutated historical fields,
    /// invalid content, or transaction failure.
    pub fn transition_model_run(
        &mut self,
        next: &ModelRun,
        expected_revision: u64,
        recorded_at: DateTime<Utc>,
    ) -> Result<ModelRunRecord, StorageError> {
        next.validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let transaction = self.connection.transaction()?;
        let current = load_model_run_head(&transaction, next.id)?
            .ok_or_else(|| StorageError::Conflict("model-run head does not exist".into()))?;
        if current.revision != expected_revision {
            return Err(StorageError::Conflict(format!(
                "stale model-run revision: expected {expected_revision}, current {}",
                current.revision
            )));
        }
        if !current.run.state.can_transition_to(next.state) {
            return Err(StorageError::Conflict(format!(
                "illegal model-run transition {:?} -> {:?}",
                current.run.state, next.state
            )));
        }
        validate_model_run_continuity(&current.run, next)?;
        let revision = expected_revision
            .checked_add(1)
            .ok_or_else(|| StorageError::InvalidData("model-run revision overflow".into()))?;
        let record = model_run_record(revision, next, recorded_at)?;
        insert_model_run_version(&transaction, &record)?;
        let changed = transaction.execute(
            "UPDATE model_run_heads SET current_revision = ?1
             WHERE model_run_id = ?2 AND invocation_id = ?3 AND current_revision = ?4",
            params![
                sql_u64(revision, "model-run revision")?,
                next.id.to_string(),
                next.invocation_id.to_string(),
                sql_u64(expected_revision, "expected model-run revision")?,
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::Conflict(
                "model-run head changed before commit".into(),
            ));
        }
        transaction.commit()?;
        Ok(record)
    }

    /// Loads one bounded exact reconstruction view for an invocation.
    ///
    /// # Errors
    /// Returns a storage error for an invalid bound, query failure, or corrupt stored lineage.
    pub fn invocation_inspection(
        &self,
        invocation_id: InvocationId,
        maximum_model_run_versions: usize,
    ) -> Result<Option<InvocationInspection>, StorageError> {
        validate_inspection_limit(maximum_model_run_versions)?;
        let Some((manifest, manifest_hash)) =
            load_invocation_with_hash(&self.connection, invocation_id)?
        else {
            return Ok(None);
        };
        let context_manifest = load_context_by_hash(&self.connection, &manifest.context_manifest)?
            .ok_or_else(|| {
                StorageError::InvalidData("invocation references a missing context manifest".into())
            })?;
        let context_manifest_hash = contract_hash(
            ContextManifest::TYPE_ID,
            CONTEXT_MANIFEST_HASH_DOMAIN,
            &context_manifest,
        )?;
        if context_manifest_hash != manifest.context_manifest {
            return Err(StorageError::InvalidData(
                "stored context manifest no longer matches its invocation hash".into(),
            ));
        }
        let fetch_limit = maximum_model_run_versions.checked_add(1).ok_or_else(|| {
            StorageError::InvalidData("model-run inspection bound overflow".into())
        })?;
        let mut statement = self.connection.prepare(
            "SELECT revision, content_hash_json, run_json, recorded_at
             FROM model_run_versions WHERE invocation_id = ?1
             ORDER BY revision ASC LIMIT ?2",
        )?;
        let rows = statement.query_map(
            params![
                invocation_id.to_string(),
                sql_usize(fetch_limit, "model-run inspection limit")?,
            ],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )?;
        let mut model_runs = rows
            .map(|row| decode_model_run_record(row?))
            .collect::<Result<Vec<_>, _>>()?;
        let model_runs_truncated = model_runs.len() > maximum_model_run_versions;
        model_runs.truncate(maximum_model_run_versions);
        Ok(Some(InvocationInspection {
            manifest,
            manifest_hash,
            context_manifest,
            context_manifest_hash,
            model_runs,
            model_runs_truncated,
        }))
    }

    /// Lists a bounded chronological set of invocation manifests for one workflow attempt.
    ///
    /// # Errors
    /// Returns a storage error for an invalid limit, query failure, or malformed stored JSON.
    pub fn invocation_manifests_for_attempt(
        &self,
        attempt_id: AttemptId,
        limit: usize,
    ) -> Result<Vec<InvocationManifest>, StorageError> {
        validate_inspection_limit(limit)?;
        let mut statement = self.connection.prepare(
            "SELECT manifest_json FROM invocation_manifests
             WHERE workflow_attempt_id = ?1
             ORDER BY persisted_at ASC,
                      CASE role WHEN 'work' THEN 0 ELSE 1 END ASC,
                      invocation_id ASC
             LIMIT ?2",
        )?;
        statement
            .query_map(
                params![
                    attempt_id.to_string(),
                    sql_usize(limit, "invocation inspection limit")?,
                ],
                |row| row.get::<_, String>(0),
            )?
            .map(|row| Ok(serde_json::from_str(&row?)?))
            .collect()
    }

    /// Lists a bounded newest-first projection of all authoritative invocations.
    ///
    /// # Errors
    /// Returns a storage error for an invalid limit, malformed history, or query failure.
    pub fn invocation_summaries(
        &self,
        limit: usize,
    ) -> Result<Vec<InvocationSummary>, StorageError> {
        validate_inspection_limit(limit)?;
        let mut statement = self.connection.prepare(
            "SELECT invocation.manifest_json, attempt.workflow_id,
                    invocation.persisted_at, head.current_revision,
                    version.run_json, version.recorded_at
             FROM invocation_manifests AS invocation
             JOIN workflow_attempts AS attempt
               ON attempt.attempt_id = invocation.workflow_attempt_id
             LEFT JOIN model_run_heads AS head
               ON head.invocation_id = invocation.invocation_id
             LEFT JOIN model_run_versions AS version
               ON version.model_run_id = head.model_run_id
              AND version.revision = head.current_revision
             ORDER BY invocation.persisted_at DESC, invocation.invocation_id DESC
             LIMIT ?1",
        )?;
        statement
            .query_map([sql_usize(limit, "invocation summary limit")?], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            })?
            .map(decode_invocation_summary)
            .collect()
    }

    /// Lists a bounded chronological projection of authoritative invocations for one attempt.
    ///
    /// # Errors
    /// Returns a storage error for an invalid limit, malformed history, or query failure.
    pub fn invocation_summaries_for_attempt(
        &self,
        attempt_id: AttemptId,
        limit: usize,
    ) -> Result<Vec<InvocationSummary>, StorageError> {
        validate_inspection_limit(limit)?;
        let mut statement = self.connection.prepare(
            "SELECT invocation.manifest_json, attempt.workflow_id,
                    invocation.persisted_at, head.current_revision,
                    version.run_json, version.recorded_at
             FROM invocation_manifests AS invocation
             JOIN workflow_attempts AS attempt
               ON attempt.attempt_id = invocation.workflow_attempt_id
             LEFT JOIN model_run_heads AS head
               ON head.invocation_id = invocation.invocation_id
             LEFT JOIN model_run_versions AS version
               ON version.model_run_id = head.model_run_id
              AND version.revision = head.current_revision
             WHERE invocation.workflow_attempt_id = ?1
             ORDER BY invocation.persisted_at ASC,
                      CASE invocation.role WHEN 'work' THEN 0 ELSE 1 END ASC,
                      invocation.invocation_id ASC
             LIMIT ?2",
        )?;
        statement
            .query_map(
                params![
                    attempt_id.to_string(),
                    sql_usize(limit, "workflow activity invocation limit")?,
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<i64>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                    ))
                },
            )?
            .map(decode_invocation_summary)
            .collect()
    }

    /// Persists one immutable `CompletionCertificate` after resolving every Work/Verify invocation
    /// to a completed, role-correct model run under the same attempt and task contract.
    ///
    /// # Errors
    /// Returns a storage error for invalid or missing lineage, incomplete runs, replay
    /// substitution, or transaction failure.
    pub fn persist_completion_certificate(
        &mut self,
        certificate: &CompletionCertificate,
        persisted_at: DateTime<Utc>,
    ) -> Result<ContentHash, StorageError> {
        certificate
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let attempt = load_attempt(&self.connection, certificate.attempt_id)?.ok_or(
            StorageError::WorkflowAttemptNotFound(certificate.attempt_id),
        )?;
        if attempt.workflow_id != certificate.workflow_id
            || attempt.task_contract != certificate.task_contract
        {
            return Err(StorageError::Conflict(
                "completion certificate does not bind its exact workflow attempt".into(),
            ));
        }
        validate_completion_invocations(&self.connection, certificate)?;
        let hash = contract_hash(
            CompletionCertificate::TYPE_ID,
            COMPLETION_CERTIFICATE_HASH_DOMAIN,
            certificate,
        )?;
        if let Some((existing_json, existing_hash_json)) = self
            .connection
            .query_row(
                "SELECT certificate_json, content_hash_json FROM completion_certificates
                 WHERE completion_certificate_id = ?1",
                [certificate.id.to_string()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
        {
            let existing: CompletionCertificate = serde_json::from_str(&existing_json)?;
            let existing_hash: ContentHash = serde_json::from_str(&existing_hash_json)?;
            if existing != *certificate || existing_hash != hash {
                return Err(StorageError::Conflict(
                    "completion-certificate identity was replayed with different content".into(),
                ));
            }
            return Ok(existing_hash);
        }
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO completion_certificates(
                completion_certificate_id, workflow_id, workflow_attempt_id, task_contract_id,
                task_contract_version, verdict, type_id, schema_version, content_hash_json,
                certificate_json, issued_at, persisted_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                certificate.id.to_string(),
                certificate.workflow_id.to_string(),
                certificate.attempt_id.to_string(),
                certificate.task_contract.id.to_string(),
                sql_u64(
                    certificate.task_contract.version,
                    "completion task-contract version"
                )?,
                completion_verdict_label(certificate.verdict),
                CompletionCertificate::TYPE_ID,
                i64::from(CompletionCertificate::SCHEMA_VERSION),
                serde_json::to_string(&hash)?,
                serde_json::to_string(certificate)?,
                certificate.issued_at.to_rfc3339(),
                persisted_at.to_rfc3339(),
            ],
        )?;
        insert_completion_invocations(
            &transaction,
            certificate.id,
            InvocationRole::Work,
            &certificate.work_invocations,
        )?;
        insert_completion_invocations(
            &transaction,
            certificate.id,
            InvocationRole::Verify,
            &certificate.verify_invocations,
        )?;
        transaction.commit()?;
        Ok(hash)
    }

    /// Loads one exact immutable completion certificate.
    ///
    /// # Errors
    /// Returns a storage error for query or stored-data failures.
    pub fn completion_certificate(
        &self,
        id: CompletionCertificateId,
    ) -> Result<Option<CompletionCertificate>, StorageError> {
        self.connection
            .query_row(
                "SELECT certificate_json FROM completion_certificates
                 WHERE completion_certificate_id = ?1",
                [id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|json| Ok(serde_json::from_str(&json)?))
            .transpose()
    }
}

type InvocationSummaryRow = (
    String,
    String,
    String,
    Option<i64>,
    Option<String>,
    Option<String>,
);

fn decode_invocation_summary(
    row: Result<InvocationSummaryRow, rusqlite::Error>,
) -> Result<InvocationSummary, StorageError> {
    let (manifest, workflow_id, persisted_at, revision, run, recorded_at) = row?;
    let manifest: InvocationManifest = serde_json::from_str(&manifest)?;
    let workflow_id = WorkflowId(
        uuid::Uuid::parse_str(&workflow_id)
            .map_err(|_| StorageError::InvalidData("invalid invocation workflow id".into()))?,
    );
    let persisted_at = DateTime::parse_from_rfc3339(&persisted_at)
        .map_err(|error| StorageError::InvalidData(error.to_string()))?
        .with_timezone(&Utc);
    let revision = revision
        .map(|revision| {
            u64::try_from(revision)
                .map_err(|_| StorageError::InvalidData("negative model-run revision".into()))
        })
        .transpose()?;
    let run = run
        .map(|run| serde_json::from_str::<ModelRun>(&run))
        .transpose()?;
    let updated_at = recorded_at
        .map(|recorded_at| {
            DateTime::parse_from_rfc3339(&recorded_at)
                .map(|value| value.with_timezone(&Utc))
                .map_err(|error| StorageError::InvalidData(error.to_string()))
        })
        .transpose()?
        .unwrap_or(persisted_at);
    if revision.is_some() != run.is_some() {
        return Err(StorageError::InvalidData(
            "invocation summary has incomplete model-run head evidence".into(),
        ));
    }
    Ok(InvocationSummary {
        invocation_id: manifest.id,
        workflow_id,
        attempt_id: manifest.attempt_id,
        role: manifest.role,
        task_contract: manifest.task_contract,
        model_run_id: run.as_ref().map(|run| run.id),
        model_run_state: run.as_ref().map(|run| run.state),
        model_run_revision: revision,
        persisted_at,
        updated_at,
    })
}

fn contract_hash<T: Serialize>(
    type_id: &str,
    domain: &str,
    value: &T,
) -> Result<ContentHash, StorageError> {
    let value = serde_json::to_value(value)?;
    let payload = VersionedPayload::canonical_v1(
        StableTypeId::parse(type_id)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?,
        &value,
    )
    .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    payload
        .content_hash(domain)
        .map_err(|error| StorageError::InvalidData(error.to_string()))
}

fn load_attempt(
    connection: &Connection,
    attempt_id: AttemptId,
) -> Result<Option<WorkflowAttempt>, StorageError> {
    connection
        .query_row(
            "SELECT attempt_json FROM workflow_attempts WHERE attempt_id = ?1",
            [attempt_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|json| Ok(serde_json::from_str(&json)?))
        .transpose()
}

fn load_context_by_hash(
    connection: &Connection,
    hash: &ContentHash,
) -> Result<Option<ContextManifest>, StorageError> {
    connection
        .query_row(
            "SELECT manifest_json FROM context_manifests WHERE content_hash_json = ?1",
            [serde_json::to_string(hash)?],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|json| Ok(serde_json::from_str(&json)?))
        .transpose()
}

fn load_invocation(
    connection: &Connection,
    invocation_id: InvocationId,
) -> Result<Option<InvocationManifest>, StorageError> {
    Ok(load_invocation_with_hash(connection, invocation_id)?.map(|(manifest, _)| manifest))
}

fn load_invocation_with_hash(
    connection: &Connection,
    invocation_id: InvocationId,
) -> Result<Option<(InvocationManifest, ContentHash)>, StorageError> {
    connection
        .query_row(
            "SELECT manifest_json, content_hash_json FROM invocation_manifests
             WHERE invocation_id = ?1",
            [invocation_id.to_string()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .map(|(manifest, hash)| {
            Ok((
                serde_json::from_str(&manifest)?,
                serde_json::from_str(&hash)?,
            ))
        })
        .transpose()
}

fn model_run_record(
    revision: u64,
    run: &ModelRun,
    recorded_at: DateTime<Utc>,
) -> Result<ModelRunRecord, StorageError> {
    Ok(ModelRunRecord {
        revision,
        content_hash: contract_hash(ModelRun::TYPE_ID, MODEL_RUN_HASH_DOMAIN, run)?,
        run: run.clone(),
        recorded_at,
    })
}

fn insert_model_run_version(
    transaction: &Transaction<'_>,
    record: &ModelRunRecord,
) -> Result<(), StorageError> {
    transaction.execute(
        "INSERT INTO model_run_versions(
            model_run_id, revision, invocation_id, role, state, content_hash_json,
            run_json, recorded_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            record.run.id.to_string(),
            sql_u64(record.revision, "model-run revision")?,
            record.run.invocation_id.to_string(),
            invocation_role_label(record.run.role),
            model_run_state_label(record.run.state),
            serde_json::to_string(&record.content_hash)?,
            serde_json::to_string(&record.run)?,
            record.recorded_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

fn load_model_run_head(
    connection: &Connection,
    run_id: ModelRunId,
) -> Result<Option<ModelRunRecord>, StorageError> {
    connection
        .query_row(
            "SELECT version.revision, version.content_hash_json, version.run_json,
                    version.recorded_at
             FROM model_run_heads AS head
             JOIN model_run_versions AS version
               ON version.model_run_id = head.model_run_id
              AND version.revision = head.current_revision
             WHERE head.model_run_id = ?1",
            [run_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?
        .map(decode_model_run_record)
        .transpose()
}

fn decode_model_run_record(
    row: (i64, String, String, String),
) -> Result<ModelRunRecord, StorageError> {
    let (revision, hash, run, recorded_at) = row;
    Ok(ModelRunRecord {
        revision: u64::try_from(revision)
            .map_err(|_| StorageError::InvalidData("negative model-run revision".into()))?,
        content_hash: serde_json::from_str(&hash)?,
        run: serde_json::from_str(&run)?,
        recorded_at: DateTime::parse_from_rfc3339(&recorded_at)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?
            .with_timezone(&Utc),
    })
}

fn validate_model_run_continuity(current: &ModelRun, next: &ModelRun) -> Result<(), StorageError> {
    let provider_preserved = current.provider_request_id.is_none()
        || current.provider_request_id == next.provider_request_id;
    let artifact_preserved =
        current.response_artifact.is_none() || current.response_artifact == next.response_artifact;
    let usage_preserved = current.usage.is_none() || current.usage == next.usage;
    if current.id != next.id
        || current.invocation_id != next.invocation_id
        || current.role != next.role
        || current.started_at != next.started_at
        || !provider_preserved
        || !artifact_preserved
        || !usage_preserved
    {
        return Err(StorageError::Conflict(
            "model-run transition rewrites immutable historical fields".into(),
        ));
    }
    Ok(())
}

fn validate_completion_invocations(
    connection: &Connection,
    certificate: &CompletionCertificate,
) -> Result<(), StorageError> {
    for (expected_role, invocation_ids) in [
        (InvocationRole::Work, &certificate.work_invocations),
        (InvocationRole::Verify, &certificate.verify_invocations),
    ] {
        for invocation_id in invocation_ids {
            let manifest = load_invocation(connection, *invocation_id)?.ok_or_else(|| {
                StorageError::Conflict(
                    "completion certificate references a missing invocation".into(),
                )
            })?;
            if manifest.role != expected_role
                || manifest.attempt_id != certificate.attempt_id
                || manifest.task_contract != certificate.task_contract
            {
                return Err(StorageError::Conflict(
                    "completion invocation has the wrong role, attempt, or task contract".into(),
                ));
            }
            let state = connection
                .query_row(
                    "SELECT version.state
                     FROM model_run_heads AS head
                     JOIN model_run_versions AS version
                       ON version.model_run_id = head.model_run_id
                      AND version.revision = head.current_revision
                     WHERE head.invocation_id = ?1",
                    [invocation_id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            if state.as_deref() != Some("completed") {
                return Err(StorageError::Conflict(
                    "completion invocation has no completed model run".into(),
                ));
            }
        }
    }
    Ok(())
}

fn insert_completion_invocations(
    transaction: &Transaction<'_>,
    certificate_id: CompletionCertificateId,
    role: InvocationRole,
    invocation_ids: &[InvocationId],
) -> Result<(), StorageError> {
    for (ordinal, invocation_id) in invocation_ids.iter().enumerate() {
        transaction.execute(
            "INSERT INTO completion_certificate_invocations(
                completion_certificate_id, invocation_id, role, ordinal
             ) VALUES (?1, ?2, ?3, ?4)",
            params![
                certificate_id.to_string(),
                invocation_id.to_string(),
                invocation_role_label(role),
                sql_usize(ordinal, "completion invocation ordinal")?,
            ],
        )?;
    }
    Ok(())
}

fn validate_inspection_limit(limit: usize) -> Result<(), StorageError> {
    if limit == 0 || limit > MAX_INVOCATION_INSPECTION_ITEMS {
        return Err(StorageError::InvalidData(format!(
            "inspection limit must be between 1 and {MAX_INVOCATION_INSPECTION_ITEMS}"
        )));
    }
    Ok(())
}

const fn invocation_role_label(role: InvocationRole) -> &'static str {
    match role {
        InvocationRole::Work => "work",
        InvocationRole::Verify => "verify",
    }
}

const fn model_run_state_label(state: ModelRunState) -> &'static str {
    match state {
        ModelRunState::Planned => "planned",
        ModelRunState::Dispatched => "dispatched",
        ModelRunState::Completed => "completed",
        ModelRunState::Failed => "failed",
        ModelRunState::Cancelled => "cancelled",
        ModelRunState::OutcomeUnknown => "outcome_unknown",
    }
}

const fn completion_verdict_label(verdict: CompletionVerdict) -> &'static str {
    match verdict {
        CompletionVerdict::Satisfied => "satisfied",
        CompletionVerdict::NotSatisfied => "not_satisfied",
        CompletionVerdict::Inconclusive => "inconclusive",
        CompletionVerdict::Cancelled => "cancelled",
    }
}

fn sql_u64(value: u64, field: &str) -> Result<i64, StorageError> {
    i64::try_from(value)
        .map_err(|_| StorageError::InvalidData(format!("{field} exceeds SQLite integer range")))
}

fn sql_usize(value: usize, field: &str) -> Result<i64, StorageError> {
    i64::try_from(value)
        .map_err(|_| StorageError::InvalidData(format!("{field} exceeds SQLite integer range")))
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use mcg_contracts::{
        ArtifactRef, ConfigurationId, ContextSelection, ContractClause, CorrelationId,
        CriterionResult, HashAlgorithm, ModelUsage, PolicyBinding, RecordId, RetryPolicy,
        RuntimeIdentity, RuntimeInstanceId, SourceRef, TaskContract, TaskContractId,
        TaskContractStatus, VersionedRef, WorkflowId, WorkflowState,
    };
    use tempfile::tempdir;

    use super::*;
    use crate::workflows::task_contract_content_hash;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 30, 12, 0, 0).unwrap()
    }

    fn hash(label: &str) -> ContentHash {
        VersionedPayload::canonical_v1(
            StableTypeId::parse("test.invocation_ref.v1").unwrap(),
            &serde_json::json!({"label": label}),
        )
        .unwrap()
        .content_hash("TestInvocationRef")
        .unwrap()
    }

    fn payload(type_id: &str, label: &str) -> VersionedPayload {
        VersionedPayload::canonical_v1(
            StableTypeId::parse(type_id).unwrap(),
            &serde_json::json!({"label": label}),
        )
        .unwrap()
    }

    fn source(label: &str) -> SourceRef {
        SourceRef {
            source_type: StableTypeId::parse("test.invocation_source.v1").unwrap(),
            locator: format!("artifact://{label}"),
            content_hash: hash(label),
        }
    }

    fn policy(label: &str) -> PolicyBinding {
        let payload = payload("test.invocation_policy.v1", label);
        PolicyBinding {
            content_hash: payload.content_hash("TestInvocationPolicy").unwrap(),
            payload,
        }
    }

    fn workflow_fixture() -> (TaskContract, WorkflowAttempt) {
        let task_id = TaskContractId::new();
        let clause = ContractClause {
            clause_id: "criterion-1".to_owned(),
            statement: "deterministic result is verified".to_owned(),
            source_refs: vec![source("criterion")],
        };
        let contract = TaskContract {
            id: task_id,
            version: 1,
            previous_version: None,
            user_intent_ref: source("intent"),
            objective: "run deterministic Work and Verify".to_owned(),
            non_goals: Vec::new(),
            requirements: vec![clause.clone()],
            constraints: Vec::new(),
            assumptions: Vec::new(),
            acceptance_criteria: vec![clause],
            dependencies: Vec::new(),
            permitted_side_effects: policy("none"),
            authority: policy("root"),
            environment: policy("isolated"),
            risk: policy("low"),
            budgets: policy("bounded"),
            rollback: policy("none"),
            status: TaskContractStatus::Active,
            created_at: now(),
        };
        let mut attempt = WorkflowAttempt {
            workflow_id: WorkflowId::new(),
            attempt_id: AttemptId::new(),
            task_contract: VersionedRef {
                id: task_id,
                version: 1,
                content_hash: ContentHash {
                    algorithm: HashAlgorithm::Blake3_256,
                    domain: "placeholder".to_owned(),
                    digest_hex: "0".repeat(64),
                },
            },
            parent_attempt_id: None,
            attempt_number: 1,
            state: WorkflowState::Ready,
            revision: 1,
            lease_owner: None,
            lease_generation: 0,
            lease_expires_at: None,
            heartbeat_at: None,
            checkpoint: None,
            environment_snapshot: hash("environment"),
            configuration: VersionedRef {
                id: ConfigurationId::new(),
                version: 1,
                content_hash: hash("configuration"),
            },
            idempotency_key: "invocation-workflow-1".to_owned(),
            retry_policy: RetryPolicy {
                max_attempts: 2,
                initial_backoff_ms: 100,
                maximum_backoff_ms: 1_000,
            },
            compensation_policy: policy("reconcile"),
            created_at: now(),
            updated_at: now(),
        };
        attempt.task_contract.content_hash = task_contract_content_hash(&contract).unwrap();
        (contract, attempt)
    }

    fn context(attempt_id: AttemptId, label: &str) -> ContextManifest {
        ContextManifest {
            id: mcg_contracts::ContextManifestId::new(),
            attempt_id,
            selected_sources: vec![ContextSelection {
                source: source(label),
                perspective: Some(source("owner-perspective")),
                influence: payload("test.influence.v1", "trusted"),
                transformations: vec![source(&format!("{label}-summary"))],
                placement: 0,
                approximate_tokens: 5,
            }],
            selection_policy: payload("test.context_policy.v1", "bounded"),
            expansion_steps: Vec::new(),
            approximate_tokens: 5,
        }
    }

    fn invocation(
        attempt: &WorkflowAttempt,
        role: InvocationRole,
        context_hash: ContentHash,
    ) -> InvocationManifest {
        let runtime_build = hash("runtime-build");
        InvocationManifest {
            id: InvocationId::new(),
            role,
            attempt_id: attempt.attempt_id,
            task_contract: attempt.task_contract.clone(),
            runtime_identity: RuntimeIdentity {
                instance_id: RuntimeInstanceId::new(),
                build_hash: runtime_build.clone(),
                protocol_version: mcg_contracts::PROTOCOL_V1,
                started_at: now(),
            },
            cognitive_configuration: hash("cognitive-configuration"),
            role_manifest: hash(invocation_role_label(role)),
            prompt_bundle: hash("prompt-bundle"),
            context_manifest: context_hash,
            model_route: payload("test.model_route.v1", "fake"),
            resolved_provider: source("fake-provider"),
            resolved_model: source("fake-model"),
            observed_model_fingerprint: Some("fake-v1".to_owned()),
            sampling: payload("test.sampling.v1", "zero"),
            reasoning: None,
            available_tool_manifests: Vec::new(),
            selected_skill_manifests: Vec::new(),
            authority_envelope: source("authority"),
            egress_permit: None,
            repository_revision: Some(hash("revision")),
            environment: source("worktree"),
            runtime_build,
            request_hash: hash(&format!("{}-request", invocation_role_label(role))),
            response_hash: None,
        }
    }

    fn run(invocation: &InvocationManifest) -> ModelRun {
        ModelRun {
            id: ModelRunId::new(),
            invocation_id: invocation.id,
            role: invocation.role,
            state: ModelRunState::Planned,
            provider_request_id: None,
            response_artifact: None,
            usage: None,
            started_at: now(),
            finished_at: None,
        }
    }

    fn completed_run(run: &ModelRun) -> ModelRun {
        ModelRun {
            state: ModelRunState::Completed,
            provider_request_id: Some(format!("fake-{}", run.id)),
            response_artifact: Some(ArtifactRef {
                content_hash: hash(&format!("response-{}", run.id)),
                media_type: "application/json".to_owned(),
                byte_length: 32,
            }),
            usage: Some(ModelUsage {
                input_tokens: 5,
                output_tokens: 3,
                cached_input_tokens: 0,
                reasoning_tokens: 0,
            }),
            finished_at: Some(now() + chrono::Duration::seconds(2)),
            ..run.clone()
        }
    }

    fn certificate(
        attempt: &WorkflowAttempt,
        work: InvocationId,
        verify: InvocationId,
    ) -> CompletionCertificate {
        CompletionCertificate {
            id: CompletionCertificateId::new(),
            workflow_id: attempt.workflow_id,
            attempt_id: attempt.attempt_id,
            task_contract: attempt.task_contract.clone(),
            work_invocations: vec![work],
            verify_invocations: vec![verify],
            criterion_results: vec![CriterionResult {
                clause_id: "criterion-1".to_owned(),
                passed: true,
                evidence_refs: vec![source("verify-result")],
                detail_hash: hash("criterion-detail"),
            }],
            evidence_refs: vec![source("completion-evidence")],
            tests_run: vec![source("deterministic-test")],
            artifacts: Vec::new(),
            known_untested_paths: Vec::new(),
            unresolved_assumptions: vec![RecordId::new()],
            remaining_risks: Vec::new(),
            verdict: CompletionVerdict::Satisfied,
            issued_at: now() + chrono::Duration::seconds(3),
        }
    }

    fn persist_completed_invocation(
        store: &mut SqliteEventStore,
        attempt: &WorkflowAttempt,
        role: InvocationRole,
    ) -> (ContextManifest, InvocationManifest, ModelRunRecord) {
        let context = context(attempt.attempt_id, invocation_role_label(role));
        let context_hash = store.persist_context_manifest(&context, now()).unwrap();
        let invocation = invocation(attempt, role, context_hash);
        store
            .persist_invocation_manifest(&invocation, now())
            .unwrap();
        let planned = run(&invocation);
        store.create_model_run(&planned, now()).unwrap();
        let mut dispatched = planned.clone();
        dispatched.state = ModelRunState::Dispatched;
        dispatched.provider_request_id = Some(format!("fake-{}", planned.id));
        store
            .transition_model_run(&dispatched, 1, now() + chrono::Duration::seconds(1))
            .unwrap();
        let completed = completed_run(&planned);
        let record = store
            .transition_model_run(&completed, 2, now() + chrono::Duration::seconds(2))
            .unwrap();
        (context, invocation, record)
    }

    #[test]
    fn reconstruction_history_and_completion_survive_restart() {
        let directory = tempdir().unwrap();
        let database = directory.path().join("state.db");
        let (contract, attempt) = workflow_fixture();
        let (work_context, work, work_record, verify, certificate, certificate_hash) = {
            let mut store = SqliteEventStore::open(&database).unwrap();
            store
                .create_workflow(&contract, &attempt, CorrelationId::new(), now())
                .unwrap();
            let (work_context, work, work_record) =
                persist_completed_invocation(&mut store, &attempt, InvocationRole::Work);
            let (_, verify, _) =
                persist_completed_invocation(&mut store, &attempt, InvocationRole::Verify);
            let certificate = certificate(&attempt, work.id, verify.id);
            let certificate_hash = store
                .persist_completion_certificate(&certificate, now())
                .unwrap();
            assert_eq!(
                store
                    .persist_completion_certificate(&certificate, now())
                    .unwrap(),
                certificate_hash
            );
            (
                work_context,
                work,
                work_record,
                verify,
                certificate,
                certificate_hash,
            )
        };

        let store = SqliteEventStore::open(&database).unwrap();
        let inspection = store.invocation_inspection(work.id, 10).unwrap().unwrap();
        assert_eq!(inspection.context_manifest, work_context);
        assert_eq!(inspection.manifest, work);
        assert_eq!(inspection.model_runs.len(), 3);
        assert_eq!(inspection.model_runs.last(), Some(&work_record));
        assert!(!inspection.model_runs_truncated);
        let summaries = store.invocation_summaries(10).unwrap();
        assert_eq!(summaries.len(), 2);
        let work_summary = summaries
            .iter()
            .find(|summary| summary.invocation_id == work.id)
            .expect("work invocation summary");
        assert_eq!(work_summary.workflow_id, attempt.workflow_id);
        assert_eq!(work_summary.attempt_id, attempt.attempt_id);
        assert_eq!(work_summary.role, InvocationRole::Work);
        assert_eq!(work_summary.model_run_id, Some(work_record.run.id));
        assert_eq!(work_summary.model_run_state, Some(ModelRunState::Completed));
        assert_eq!(work_summary.model_run_revision, Some(3));
        assert_eq!(store.invocation_summaries(1).unwrap().len(), 1);
        let attempt_summaries = store
            .invocation_summaries_for_attempt(attempt.attempt_id, 10)
            .unwrap();
        assert_eq!(attempt_summaries.len(), 2);
        assert_eq!(attempt_summaries[0].role, InvocationRole::Work);
        assert_eq!(attempt_summaries[1].role, InvocationRole::Verify);
        assert!(
            store
                .invocation_summaries_for_attempt(AttemptId::new(), 10)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            store
                .invocation_manifests_for_attempt(attempt.attempt_id, 10)
                .unwrap(),
            vec![work, verify]
        );
        assert_eq!(
            store.completion_certificate(certificate.id).unwrap(),
            Some(certificate)
        );
        assert_eq!(certificate_hash.domain, COMPLETION_CERTIFICATE_HASH_DOMAIN);
    }

    #[test]
    fn lineage_substitution_stale_runs_and_unbounded_inspection_fail_closed() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (contract, attempt) = workflow_fixture();
        store
            .create_workflow(&contract, &attempt, CorrelationId::new(), now())
            .unwrap();
        let context = context(attempt.attempt_id, "work");
        let context_hash = store.persist_context_manifest(&context, now()).unwrap();
        let mut invocation = invocation(&attempt, InvocationRole::Work, context_hash);
        store
            .persist_invocation_manifest(&invocation, now())
            .unwrap();
        invocation.request_hash = hash("substituted-request");
        assert!(matches!(
            store.persist_invocation_manifest(&invocation, now()),
            Err(StorageError::Conflict(_))
        ));

        let planned = run(&invocation);
        store.create_model_run(&planned, now()).unwrap();
        let mut dispatched = planned.clone();
        dispatched.state = ModelRunState::Dispatched;
        assert!(store.transition_model_run(&dispatched, 7, now()).is_err());
        store.transition_model_run(&dispatched, 1, now()).unwrap();
        let mut rewrite = completed_run(&planned);
        rewrite.started_at += chrono::Duration::seconds(1);
        assert!(matches!(
            store.transition_model_run(&rewrite, 2, now()),
            Err(StorageError::Conflict(_))
        ));
        assert!(store.invocation_inspection(invocation.id, 0).is_err());
        assert!(store.invocation_summaries(0).is_err());
        assert!(
            store
                .invocation_summaries_for_attempt(attempt.attempt_id, 0)
                .is_err()
        );
        assert!(
            store
                .invocation_inspection(invocation.id, MAX_INVOCATION_INSPECTION_ITEMS + 1)
                .is_err()
        );
    }

    #[test]
    fn exact_runtime_response_artifact_survives_restart_and_rejects_substitution() {
        let directory = tempdir().unwrap();
        let database = directory.path().join("state.db");
        let model_run_id;
        let expected;
        {
            let mut store = SqliteEventStore::open(&database).unwrap();
            let (contract, attempt) = workflow_fixture();
            store
                .create_workflow(&contract, &attempt, CorrelationId::new(), now())
                .unwrap();
            let context = context(attempt.attempt_id, "work");
            let context_hash = store.persist_context_manifest(&context, now()).unwrap();
            let invocation = invocation(&attempt, InvocationRole::Work, context_hash);
            store
                .persist_invocation_manifest(&invocation, now())
                .unwrap();
            let planned = run(&invocation);
            model_run_id = planned.id;
            store.create_model_run(&planned, now()).unwrap();
            let mut dispatched = planned;
            dispatched.state = ModelRunState::Dispatched;
            dispatched.provider_request_id = Some("runtime-worker:1".into());
            store.transition_model_run(&dispatched, 1, now()).unwrap();
            let response = payload("test.runtime_response.v1", "exact-output");
            let response_hash = response
                .content_hash(mcg_contracts::RUNTIME_INVOCATION_RESPONSE_HASH_DOMAIN)
                .unwrap();
            let result = RuntimeInvocationResult {
                worker_generation: 1,
                invocation_id: invocation.id,
                model_run_id,
                role: InvocationRole::Work,
                request_hash: invocation.request_hash,
                response,
                response_hash,
                criterion_results: Vec::new(),
                verdict: None,
            };
            expected = store
                .persist_runtime_response_artifact(&result, now())
                .unwrap();
            assert_eq!(
                store
                    .persist_runtime_response_artifact(
                        &result,
                        now() + chrono::Duration::seconds(1),
                    )
                    .unwrap(),
                expected
            );
            let mut substituted = result;
            substituted.response = payload("test.runtime_response.v1", "substituted-output");
            substituted.response_hash = substituted
                .response
                .content_hash(mcg_contracts::RUNTIME_INVOCATION_RESPONSE_HASH_DOMAIN)
                .unwrap();
            assert!(matches!(
                store.persist_runtime_response_artifact(&substituted, now()),
                Err(StorageError::Conflict(_))
            ));
            assert!(
                store
                    .connection
                    .execute(
                        "UPDATE runtime_response_artifacts SET recorded_at = recorded_at
                         WHERE model_run_id = ?1",
                        [model_run_id.to_string()],
                    )
                    .is_err()
            );
            assert!(
                store
                    .connection
                    .execute(
                        "DELETE FROM runtime_response_artifacts WHERE model_run_id = ?1",
                        [model_run_id.to_string()],
                    )
                    .is_err()
            );
        }
        let store = SqliteEventStore::open(&database).unwrap();
        assert_eq!(
            store.runtime_response_artifact(model_run_id).unwrap(),
            Some(expected)
        );
    }

    #[test]
    fn runtime_worker_generation_advances_monotonically_across_restart() {
        let directory = tempdir().unwrap();
        let database = directory.path().join("state.db");
        {
            let mut store = SqliteEventStore::open(&database).unwrap();
            assert_eq!(store.runtime_worker_generation().unwrap(), None);
            assert_eq!(store.allocate_runtime_worker_generation(now()).unwrap(), 1);
            assert_eq!(store.allocate_runtime_worker_generation(now()).unwrap(), 2);
            assert!(
                store
                    .connection
                    .execute(
                        "UPDATE runtime_worker_generations SET generation = 4
                         WHERE role = 'cognition'",
                        [],
                    )
                    .is_err()
            );
        }
        let mut reopened = SqliteEventStore::open(&database).unwrap();
        assert_eq!(reopened.runtime_worker_generation().unwrap(), Some(2));
        assert_eq!(
            reopened.allocate_runtime_worker_generation(now()).unwrap(),
            3
        );
    }

    #[test]
    fn completion_requires_fresh_completed_work_and_verify_runs() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (contract, attempt) = workflow_fixture();
        store
            .create_workflow(&contract, &attempt, CorrelationId::new(), now())
            .unwrap();
        let (_, work, _) = persist_completed_invocation(&mut store, &attempt, InvocationRole::Work);
        let verify_context = context(attempt.attempt_id, "verify");
        let verify_hash = store
            .persist_context_manifest(&verify_context, now())
            .unwrap();
        let verify = invocation(&attempt, InvocationRole::Verify, verify_hash);
        store.persist_invocation_manifest(&verify, now()).unwrap();
        store.create_model_run(&run(&verify), now()).unwrap();
        let certificate = certificate(&attempt, work.id, verify.id);
        assert!(matches!(
            store.persist_completion_certificate(&certificate, now()),
            Err(StorageError::Conflict(_))
        ));

        let mut wrong_role = certificate.clone();
        wrong_role.verify_invocations = vec![work.id];
        assert!(wrong_role.validate().is_err());
    }

    #[test]
    fn schema_fifty_six_tables_and_reconstruction_rows_are_immutable() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (contract, attempt) = workflow_fixture();
        store
            .create_workflow(&contract, &attempt, CorrelationId::new(), now())
            .unwrap();
        let (context, invocation, _) =
            persist_completed_invocation(&mut store, &attempt, InvocationRole::Work);
        assert!(
            store
                .connection
                .execute(
                    "UPDATE context_manifests SET persisted_at = persisted_at
                 WHERE context_manifest_id = ?1",
                    [context.id.to_string()],
                )
                .is_err()
        );
        assert!(
            store
                .connection
                .execute(
                    "UPDATE invocation_manifests SET persisted_at = persisted_at
                 WHERE invocation_id = ?1",
                    [invocation.id.to_string()],
                )
                .is_err()
        );
        assert!(
            store
                .connection
                .execute(
                    "DELETE FROM model_run_versions WHERE invocation_id = ?1",
                    [invocation.id.to_string()],
                )
                .is_err()
        );
        assert_eq!(
            store.diagnostics().unwrap().schema_version,
            crate::STORAGE_SCHEMA_VERSION
        );
    }

    #[test]
    fn schema_fifty_five_profile_migrates_to_reconstruction_storage() {
        let directory = tempdir().unwrap();
        let database = directory.path().join("state.db");
        drop(SqliteEventStore::open(&database).unwrap());
        let connection = Connection::open(&database).unwrap();
        connection
            .execute_batch(
                "PRAGMA foreign_keys = OFF;
                 DROP TABLE completion_certificate_invocations;
                 DROP TABLE completion_certificates;
                 DROP TABLE model_run_heads;
                 DROP TABLE model_run_versions;
                 DROP TABLE invocation_manifests;
                 DROP TABLE context_manifests;
                 DELETE FROM mcg_schema WHERE version = 56;",
            )
            .unwrap();
        drop(connection);

        let store = SqliteEventStore::open(&database).unwrap();
        assert_eq!(
            store.diagnostics().unwrap().schema_version,
            crate::STORAGE_SCHEMA_VERSION
        );
        let table_count: i64 = store
            .connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name IN (
                    'context_manifests', 'invocation_manifests', 'model_run_versions',
                    'model_run_heads', 'completion_certificates',
                    'completion_certificate_invocations'
                 )",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(table_count, 6);
    }
}
