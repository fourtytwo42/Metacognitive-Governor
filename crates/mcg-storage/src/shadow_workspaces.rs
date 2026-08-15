use chrono::{DateTime, Utc};
use mcg_agents::{
    ExperimentState, SHADOW_WORKSPACE_SCHEMA_VERSION, ShadowWorkspace, ShadowWorkspaceState,
};
use mcg_domain::ExperimentId;
use mcg_events::{EventEnvelope, EventKind};
use rusqlite::{OptionalExtension, params};

use crate::{SqliteEventStore, StorageError};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ShadowWorkspaceMutation {
    pub workspace: ShadowWorkspace,
    pub replayed: bool,
    pub events: Vec<EventEnvelope>,
}

impl SqliteEventStore {
    /// Claims one stable private workspace before any project bytes are copied.
    ///
    /// # Errors
    /// Returns an error for invalid evidence, missing scope, or conflicting identity reuse.
    pub fn create_shadow_workspace(
        &mut self,
        workspace: &ShadowWorkspace,
    ) -> Result<ShadowWorkspaceMutation, StorageError> {
        workspace
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        if workspace.schema_version != SHADOW_WORKSPACE_SCHEMA_VERSION
            || workspace.state != ShadowWorkspaceState::Preparing
        {
            return Err(StorageError::InvalidData(
                "new shadow workspace must be preparing".into(),
            ));
        }
        if let Some(existing) = self.shadow_workspace(workspace.experiment_id)? {
            if existing == *workspace {
                return Ok(ShadowWorkspaceMutation {
                    workspace: existing,
                    replayed: true,
                    events: Vec::new(),
                });
            }
            return Err(StorageError::Conflict(
                "shadow workspace identity was reused with different scope".into(),
            ));
        }
        let experiment = self
            .experiment(workspace.experiment_id)?
            .ok_or_else(|| StorageError::Conflict("shadow experiment was not found".into()))?;
        if experiment.state != ExperimentState::Running {
            return Err(StorageError::Conflict(
                "shadow workspace requires a running experiment".into(),
            ));
        }
        if self.project(workspace.project_id)?.is_none() {
            return Err(StorageError::ProjectNotFound(workspace.project_id));
        }
        let event = EventEnvelope::new(
            EventKind::ShadowWorkspacePrepared,
            None,
            serde_json::to_value(workspace)?,
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO shadow_workspaces(
                experiment_id, project_id, state, definition_hash, relative_path,
                workspace_json, created_at, updated_at
             ) VALUES (?1, ?2, 'preparing', ?3, ?4, ?5, ?6, ?7)",
            params![
                workspace.experiment_id.to_string(),
                workspace.project_id.to_string(),
                &workspace.definition_hash,
                &workspace.relative_path,
                serde_json::to_string(workspace)?,
                workspace.created_at.to_rfc3339(),
                workspace.updated_at.to_rfc3339()
            ],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(ShadowWorkspaceMutation {
            workspace: workspace.clone(),
            replayed: false,
            events: vec![event],
        })
    }

    /// Commits bounded content evidence after the complete copy is fsynced and verified.
    ///
    /// # Errors
    /// Returns an error for stale preparation, changed definition, or malformed evidence.
    pub fn complete_shadow_workspace(
        &mut self,
        experiment_id: ExperimentId,
        expected_definition_hash: &str,
        file_count: u64,
        total_bytes: u64,
        content_sha256: String,
        at: DateTime<Utc>,
    ) -> Result<ShadowWorkspaceMutation, StorageError> {
        let mut workspace = self
            .shadow_workspace(experiment_id)?
            .ok_or_else(|| StorageError::Conflict("shadow workspace was not found".into()))?;
        if workspace.state != ShadowWorkspaceState::Preparing
            || workspace.definition_hash != expected_definition_hash
        {
            return Err(StorageError::Conflict(
                "shadow workspace preparation is stale".into(),
            ));
        }
        let previous = workspace.state;
        workspace.state = ShadowWorkspaceState::Ready;
        workspace.file_count = file_count;
        workspace.total_bytes = total_bytes;
        workspace.content_sha256 = Some(content_sha256);
        workspace.updated_at = at;
        workspace
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let event = EventEnvelope::new(
            EventKind::ShadowWorkspaceStateChanged,
            None,
            serde_json::json!({"from": previous, "to": workspace.state, "workspace": &workspace}),
        );
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE shadow_workspaces SET state = 'ready', workspace_json = ?1, updated_at = ?2
             WHERE experiment_id = ?3 AND state = 'preparing' AND definition_hash = ?4",
            params![
                serde_json::to_string(&workspace)?,
                at.to_rfc3339(),
                experiment_id.to_string(),
                expected_definition_hash
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::Conflict(
                "shadow workspace changed concurrently".into(),
            ));
        }
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(ShadowWorkspaceMutation {
            workspace,
            replayed: false,
            events: vec![event],
        })
    }

    /// Converts crash-interrupted preparation into explicit non-runnable uncertainty.
    ///
    /// # Errors
    /// Returns an error when persisted workspace evidence cannot be decoded or updated atomically.
    pub fn recover_shadow_workspaces(&mut self, at: DateTime<Utc>) -> Result<u64, StorageError> {
        let preparing = {
            let mut statement = self.connection.prepare(
                "SELECT workspace_json FROM shadow_workspaces WHERE state = 'preparing' ORDER BY created_at, experiment_id")?;
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .map(|row| Ok(serde_json::from_str::<ShadowWorkspace>(&row?)?))
                .collect::<Result<Vec<_>, StorageError>>()?
        };
        let count = u64::try_from(preparing.len())
            .map_err(|_| StorageError::InvalidData("too many shadow workspaces".into()))?;
        for mut workspace in preparing {
            let previous = workspace.state;
            workspace.state = ShadowWorkspaceState::Uncertain;
            workspace.updated_at = at;
            let event = EventEnvelope::new(
                EventKind::ShadowWorkspaceStateChanged,
                None,
                serde_json::json!({"from": previous, "to": workspace.state, "workspace": &workspace, "reason": "startup_recovery"}),
            );
            let transaction = self.connection.transaction()?;
            let changed = transaction.execute(
                "UPDATE shadow_workspaces SET state = 'uncertain', workspace_json = ?1, updated_at = ?2
                 WHERE experiment_id = ?3 AND state = 'preparing'",
                params![serde_json::to_string(&workspace)?, at.to_rfc3339(), workspace.experiment_id.to_string()],
            )?;
            if changed != 1 {
                return Err(StorageError::Conflict(
                    "shadow workspace changed during recovery".into(),
                ));
            }
            Self::insert_event(&transaction, &event)?;
            transaction.commit()?;
        }
        Ok(count)
    }

    /// Marks a live preparation failure as explicit non-runnable uncertainty.
    ///
    /// # Errors
    /// Returns an error when the claim is missing, stale, or cannot be updated atomically.
    pub fn fail_shadow_workspace(
        &mut self,
        experiment_id: ExperimentId,
        reason: &str,
        at: DateTime<Utc>,
    ) -> Result<ShadowWorkspaceMutation, StorageError> {
        if reason.trim().is_empty() || reason.len() > 1_024 {
            return Err(StorageError::InvalidData(
                "shadow workspace failure reason is invalid".into(),
            ));
        }
        let mut workspace = self
            .shadow_workspace(experiment_id)?
            .ok_or_else(|| StorageError::Conflict("shadow workspace was not found".into()))?;
        if workspace.state != ShadowWorkspaceState::Preparing {
            return Err(StorageError::Conflict(
                "shadow workspace is not preparing".into(),
            ));
        }
        let previous = workspace.state;
        workspace.state = ShadowWorkspaceState::Uncertain;
        workspace.updated_at = at;
        let event = EventEnvelope::new(
            EventKind::ShadowWorkspaceStateChanged,
            None,
            serde_json::json!({"from": previous, "to": workspace.state, "workspace": &workspace, "reason": reason}),
        );
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE shadow_workspaces SET state = 'uncertain', workspace_json = ?1, updated_at = ?2
             WHERE experiment_id = ?3 AND state = 'preparing'",
            params![
                serde_json::to_string(&workspace)?,
                at.to_rfc3339(),
                workspace.experiment_id.to_string(),
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::Conflict(
                "shadow workspace changed during failure settlement".into(),
            ));
        }
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(ShadowWorkspaceMutation {
            workspace,
            replayed: false,
            events: vec![event],
        })
    }

    /// Loads one durable workspace projection.
    ///
    /// # Errors
    /// Returns an error when persisted JSON is invalid.
    pub fn shadow_workspace(
        &self,
        experiment_id: ExperimentId,
    ) -> Result<Option<ShadowWorkspace>, StorageError> {
        let json = self
            .connection
            .query_row(
                "SELECT workspace_json FROM shadow_workspaces WHERE experiment_id = ?1",
                [experiment_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        json.map(|value| serde_json::from_str(&value).map_err(StorageError::from))
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};
    use mcg_agents::{
        ConfidenceEstimate, EXPERIMENT_SCHEMA_VERSION, ExperimentDefinition, ExperimentMode,
        ExperimentState, InterventionLevel, SHADOW_WORKSPACE_SCHEMA_VERSION,
        SUPERVISION_SCHEMA_VERSION, ShadowWorkspace, ShadowWorkspaceState, SupervisionEvidence,
        SupervisionLoop, SupervisionObservation, SupervisionSignal,
    };
    use mcg_domain::{ExperimentId, OrganizationRevisionId, ProjectId, SupervisionObservationId};
    use mcg_projects::ProjectDiscoverer;

    use super::*;

    fn seed_experiment(
        store: &mut SqliteEventStore,
        baseline: &mcg_agents::OrganizationGenome,
        candidate: &mcg_agents::OrganizationGenome,
        at: chrono::DateTime<Utc>,
    ) -> ExperimentDefinition {
        let observation = SupervisionObservation {
            schema_version: SUPERVISION_SCHEMA_VERSION,
            id: SupervisionObservationId::new(),
            loop_kind: SupervisionLoop::Slow,
            signal: SupervisionSignal::Regression,
            severity_basis_points: 9_000,
            recommended_intervention: InterventionLevel::Restructure,
            task_id: None,
            run_id: None,
            organization_revision_id: Some(baseline.revision_id),
            summary: "seed isolated workspace".into(),
            evidence: vec![SupervisionEvidence {
                method: "fixture".into(),
                artifact_sha256: Some("a".repeat(64)),
                event_sequence: None,
                detail: "durable fixture".into(),
            }],
            observed_at: at,
        };
        store
            .record_supervision_observation(&observation)
            .expect("observation");
        let experiment = ExperimentDefinition {
            schema_version: EXPERIMENT_SCHEMA_VERSION,
            id: ExperimentId::new(),
            organization_id: baseline.organization_id,
            baseline_revision_id: baseline.revision_id,
            candidate_revision_id: candidate.revision_id,
            mode: ExperimentMode::Shadow,
            state: ExperimentState::Draft,
            hypothesis: "isolated execution prevents production mutation".into(),
            task_classes: vec!["fixture".into()],
            required_benchmarks: 1,
            maximum_regressions: 0,
            minimum_confidence: ConfidenceEstimate::new(8_000, 1).expect("confidence"),
            observation_ids: vec![observation.id],
            created_at: at,
            updated_at: at,
        };
        store.create_experiment(&experiment).expect("experiment");
        store
            .transition_experiment(
                experiment.id,
                ExperimentState::Draft,
                ExperimentState::Ready,
                at + Duration::seconds(1),
            )
            .expect("ready");
        store
            .transition_experiment(
                experiment.id,
                ExperimentState::Ready,
                ExperimentState::Running,
                at + Duration::seconds(2),
            )
            .expect("running");
        experiment
    }

    fn preparing(
        experiment_id: ExperimentId,
        project_id: ProjectId,
        at: chrono::DateTime<Utc>,
    ) -> ShadowWorkspace {
        ShadowWorkspace {
            schema_version: SHADOW_WORKSPACE_SCHEMA_VERSION,
            experiment_id,
            project_id,
            state: ShadowWorkspaceState::Preparing,
            definition_hash: "b".repeat(64),
            source_fingerprint: "c".repeat(64),
            relative_path: format!("shadow-workspaces/{experiment_id}"),
            file_count: 0,
            total_bytes: 0,
            content_sha256: None,
            created_at: at,
            updated_at: at,
        }
    }

    #[test]
    fn workspace_claim_completion_and_crash_uncertainty_are_durable() {
        let directory = tempfile::tempdir().expect("directory");
        let database = directory.path().join("mcg.sqlite3");
        let project_root = tempfile::tempdir().expect("project");
        std::fs::write(project_root.path().join("package.json"), "{}").expect("manifest");
        let project_id = ProjectId::new();
        let project =
            ProjectDiscoverer::discover(project_root.path(), project_id).expect("discover project");
        let at = Utc::now();
        let baseline = mcg_agents::OrganizationGenome::consumer_default(at);
        let mut candidate = baseline.clone();
        candidate.revision_id = OrganizationRevisionId::new();
        candidate.revision += 1;
        candidate.parent_revision_id = Some(baseline.revision_id);
        candidate.created_at += Duration::seconds(1);
        let mut store = SqliteEventStore::open(&database).expect("store");
        store.record_project_snapshot(&project).expect("project");
        store
            .record_organization_revision(&baseline)
            .expect("baseline");
        store
            .record_candidate_organization_revision(&candidate)
            .expect("candidate");
        let completed_experiment = seed_experiment(&mut store, &baseline, &candidate, at);
        let interrupted_experiment = seed_experiment(
            &mut store,
            &baseline,
            &candidate,
            at + Duration::seconds(10),
        );
        let completed_claim = preparing(completed_experiment.id, project_id, at);
        let created = store
            .create_shadow_workspace(&completed_claim)
            .expect("claim");
        assert!(!created.replayed);
        assert!(
            store
                .create_shadow_workspace(&completed_claim)
                .expect("exact replay")
                .replayed
        );
        let ready = store
            .complete_shadow_workspace(
                completed_experiment.id,
                &completed_claim.definition_hash,
                2,
                42,
                "d".repeat(64),
                at + Duration::seconds(3),
            )
            .expect("complete");
        assert_eq!(ready.workspace.state, ShadowWorkspaceState::Ready);
        store
            .create_shadow_workspace(&preparing(interrupted_experiment.id, project_id, at))
            .expect("interrupted claim");
        drop(store);

        let mut reopened = SqliteEventStore::open(&database).expect("reopen");
        assert_eq!(
            reopened
                .recover_shadow_workspaces(at + Duration::seconds(20))
                .expect("recover"),
            1
        );
        assert_eq!(
            reopened
                .shadow_workspace(completed_experiment.id)
                .expect("ready")
                .expect("present")
                .state,
            ShadowWorkspaceState::Ready
        );
        assert_eq!(
            reopened
                .shadow_workspace(interrupted_experiment.id)
                .expect("uncertain")
                .expect("present")
                .state,
            ShadowWorkspaceState::Uncertain
        );
        assert_eq!(
            reopened
                .recover_shadow_workspaces(at + Duration::seconds(21))
                .expect("replay"),
            0
        );
    }
}
