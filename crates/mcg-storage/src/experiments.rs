use mcg_agents::{
    ExperimentBenchmark, ExperimentDecision, ExperimentDecisionKind, ExperimentDefinition,
    ExperimentMode, ExperimentState, ShadowExecutionReceipt, SupervisionLoop,
    SupervisionObservation, SupervisionSignal,
};
use mcg_domain::{
    EvaluationId, ExperimentBenchmarkId, ExperimentId, OrganizationId, OrganizationRevisionId,
    SupervisionObservationId,
};
use mcg_events::{EventEnvelope, EventKind};
use rusqlite::{OptionalExtension, params};

use crate::{SqliteEventStore, StorageError};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExperimentMutation {
    pub experiment: ExperimentDefinition,
    pub replayed: bool,
    pub events: Vec<EventEnvelope>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CandidateRevisionMutation {
    pub candidate: mcg_agents::OrganizationGenome,
    pub replayed: bool,
    pub events: Vec<EventEnvelope>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AutomaticSupervisionMutation {
    pub observations: Vec<SupervisionObservation>,
    pub replayed: bool,
    pub events: Vec<EventEnvelope>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomaticExperimentDisposition {
    Ready,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AutomaticExperimentOutcome {
    pub observation_id: SupervisionObservationId,
    pub disposition: AutomaticExperimentDisposition,
    pub candidate_revision_id: Option<OrganizationRevisionId>,
    pub experiment_id: Option<ExperimentId>,
    pub detail: String,
    pub processed_at: chrono::DateTime<chrono::Utc>,
    pub replayed: bool,
}

impl SqliteEventStore {
    /// Lists automatic experiments that are ready to start or running without a receipt.
    ///
    /// # Errors
    /// Returns an error for invalid bounds, malformed identities, or database failures.
    pub fn executable_automatic_experiment_ids(
        &self,
        limit: usize,
    ) -> Result<Vec<ExperimentId>, StorageError> {
        if limit == 0 || limit > 128 {
            return Err(StorageError::InvalidData(
                "automatic shadow batch must contain 1 to 128 experiments".into(),
            ));
        }
        let limit = i64::try_from(limit)
            .map_err(|_| StorageError::InvalidData("shadow batch is too large".into()))?;
        let mut statement = self.connection.prepare(
            "SELECT e.experiment_id
             FROM automatic_experiment_outcomes a
             JOIN experiments e ON e.experiment_id = a.experiment_id
             LEFT JOIN shadow_execution_receipts r ON r.experiment_id = e.experiment_id
             WHERE a.disposition = 'ready'
               AND e.state IN ('ready', 'running')
               AND r.experiment_id IS NULL
             ORDER BY e.created_at, e.experiment_id LIMIT ?1",
        )?;
        statement
            .query_map([limit], |row| row.get::<_, String>(0))?
            .map(|row| {
                row?.parse().map_err(|_| {
                    rusqlite::Error::InvalidColumnType(
                        0,
                        "experiment_id".into(),
                        rusqlite::types::Type::Text,
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    /// Lists slow-loop observations that have no durable orchestration outcome.
    ///
    /// # Errors
    /// Returns an error for invalid bounds, malformed identities, or database failures.
    pub fn pending_candidate_observation_ids(
        &self,
        limit: usize,
    ) -> Result<Vec<SupervisionObservationId>, StorageError> {
        if limit == 0 || limit > 1_024 {
            return Err(StorageError::InvalidData(
                "candidate orchestration batch must contain 1 to 1024 observations".into(),
            ));
        }
        let limit = i64::try_from(limit)
            .map_err(|_| StorageError::InvalidData("candidate batch is too large".into()))?;
        let mut statement = self.connection.prepare(
            "SELECT o.observation_id FROM supervision_observations o
             LEFT JOIN automatic_experiment_outcomes a
               ON a.observation_id = o.observation_id
             WHERE o.loop_kind = 'slow' AND a.observation_id IS NULL
             ORDER BY o.observed_at, o.observation_id LIMIT ?1",
        )?;
        statement
            .query_map([limit], |row| row.get::<_, String>(0))?
            .map(|row| {
                row?.parse().map_err(|_| {
                    rusqlite::Error::InvalidColumnType(
                        0,
                        "observation_id".into(),
                        rusqlite::types::Type::Text,
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    /// Loads one immutable automatic candidate-orchestration outcome.
    ///
    /// # Errors
    /// Returns an error when persisted identities, timestamps, or disposition are invalid.
    pub fn automatic_experiment_outcome(
        &self,
        observation_id: SupervisionObservationId,
    ) -> Result<Option<AutomaticExperimentOutcome>, StorageError> {
        self.connection
            .query_row(
                "SELECT disposition, candidate_revision_id, experiment_id, detail, processed_at
                 FROM automatic_experiment_outcomes WHERE observation_id = ?1",
                [observation_id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .optional()?
            .map(
                |(disposition, candidate, experiment, detail, processed_at)| {
                    Ok(AutomaticExperimentOutcome {
                        observation_id,
                        disposition: match disposition.as_str() {
                            "ready" => AutomaticExperimentDisposition::Ready,
                            "stale" => AutomaticExperimentDisposition::Stale,
                            _ => {
                                return Err(StorageError::InvalidData(
                                    "invalid automatic experiment disposition".into(),
                                ));
                            }
                        },
                        candidate_revision_id: candidate
                            .map(|value| value.parse())
                            .transpose()
                            .map_err(|_| {
                                StorageError::InvalidData("invalid candidate identity".into())
                            })?,
                        experiment_id: experiment.map(|value| value.parse()).transpose().map_err(
                            |_| StorageError::InvalidData("invalid experiment identity".into()),
                        )?,
                        detail,
                        processed_at: chrono::DateTime::parse_from_rfc3339(&processed_at)
                            .map_err(|_| {
                                StorageError::InvalidData("invalid orchestration timestamp".into())
                            })?
                            .with_timezone(&chrono::Utc),
                        replayed: true,
                    })
                },
            )
            .transpose()
    }

    /// Records an immutable outcome after automatic candidate orchestration.
    ///
    /// # Errors
    /// Returns an error for invalid ready/stale shape, missing linked evidence, or drift.
    pub fn record_automatic_experiment_outcome(
        &mut self,
        outcome: &AutomaticExperimentOutcome,
    ) -> Result<AutomaticExperimentOutcome, StorageError> {
        if outcome.detail.trim().is_empty() || outcome.detail.len() > 4_096 {
            return Err(StorageError::InvalidData(
                "automatic experiment outcome detail is invalid".into(),
            ));
        }
        if let Some(existing) = self.automatic_experiment_outcome(outcome.observation_id)? {
            let mut expected = outcome.clone();
            expected.replayed = true;
            if existing == expected {
                return Ok(existing);
            }
            return Err(StorageError::Conflict(
                "automatic experiment outcome changed under the same observation".into(),
            ));
        }
        let observation = self
            .supervision_observation(outcome.observation_id)?
            .ok_or_else(|| {
                StorageError::Conflict("supervision observation was not found".into())
            })?;
        match outcome.disposition {
            AutomaticExperimentDisposition::Ready => {
                let candidate_id = outcome.candidate_revision_id.ok_or_else(|| {
                    StorageError::InvalidData("ready outcome requires a candidate".into())
                })?;
                let experiment_id = outcome.experiment_id.ok_or_else(|| {
                    StorageError::InvalidData("ready outcome requires an experiment".into())
                })?;
                let candidate = self.organization_revision(candidate_id)?.ok_or_else(|| {
                    StorageError::Conflict("automatic candidate was not found".into())
                })?;
                let experiment = self.experiment(experiment_id)?.ok_or_else(|| {
                    StorageError::Conflict("automatic experiment was not found".into())
                })?;
                if experiment.state != ExperimentState::Ready
                    || experiment.candidate_revision_id != candidate.revision_id
                    || !experiment.observation_ids.contains(&observation.id)
                {
                    return Err(StorageError::Conflict(
                        "ready outcome does not match its candidate experiment".into(),
                    ));
                }
            }
            AutomaticExperimentDisposition::Stale => {
                if outcome.candidate_revision_id.is_some() || outcome.experiment_id.is_some() {
                    return Err(StorageError::InvalidData(
                        "stale outcome cannot reference created experiment state".into(),
                    ));
                }
            }
        }
        self.connection.execute(
            "INSERT INTO automatic_experiment_outcomes(
                observation_id, disposition, candidate_revision_id, experiment_id,
                detail, processed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                outcome.observation_id.to_string(),
                match outcome.disposition {
                    AutomaticExperimentDisposition::Ready => "ready",
                    AutomaticExperimentDisposition::Stale => "stale",
                },
                outcome.candidate_revision_id.map(|id| id.to_string()),
                outcome.experiment_id.map(|id| id.to_string()),
                &outcome.detail,
                outcome.processed_at.to_rfc3339(),
            ],
        )?;
        let mut recorded = outcome.clone();
        recorded.replayed = false;
        Ok(recorded)
    }

    /// Loads the immutable automatic-supervision result for one evaluation.
    ///
    /// # Errors
    /// Returns an error when marker JSON is invalid or referenced evidence is missing.
    pub fn automatic_supervision(
        &self,
        evaluation_id: EvaluationId,
    ) -> Result<Option<AutomaticSupervisionMutation>, StorageError> {
        let ids_json = self
            .connection
            .query_row(
                "SELECT observation_ids_json FROM supervised_evaluations
                 WHERE evaluation_id = ?1",
                [evaluation_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(ids_json) = ids_json else {
            return Ok(None);
        };
        let ids: Vec<SupervisionObservationId> = serde_json::from_str(&ids_json)?;
        let mut observations = Vec::with_capacity(ids.len());
        for id in ids {
            observations.push(self.supervision_observation(id)?.ok_or_else(|| {
                StorageError::InvalidData(
                    "automatic supervision marker references missing evidence".into(),
                )
            })?);
        }
        Ok(Some(AutomaticSupervisionMutation {
            observations,
            replayed: true,
            events: Vec::new(),
        }))
    }

    /// Lists durable evaluations that have not yet received automatic supervision.
    ///
    /// # Errors
    /// Returns an error for invalid bounds or database failures.
    pub fn unsupervised_evaluation_ids(
        &self,
        limit: usize,
    ) -> Result<Vec<EvaluationId>, StorageError> {
        if limit == 0 || limit > 1_024 {
            return Err(StorageError::InvalidData(
                "automatic supervision batch must contain 1 to 1024 evaluations".into(),
            ));
        }
        let limit = i64::try_from(limit)
            .map_err(|_| StorageError::InvalidData("supervision batch is too large".into()))?;
        let mut statement = self.connection.prepare(
            "SELECT e.evaluation_id FROM evaluations e
             LEFT JOIN supervised_evaluations s ON s.evaluation_id = e.evaluation_id
             WHERE s.evaluation_id IS NULL
             ORDER BY e.evaluated_at, e.evaluation_id LIMIT ?1",
        )?;
        statement
            .query_map([limit], |row| row.get::<_, String>(0))?
            .map(|row| {
                row?.parse().map_err(|_| {
                    rusqlite::Error::InvalidColumnType(
                        0,
                        "evaluation_id".into(),
                        rusqlite::types::Type::Text,
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    /// Records a candidate revision without changing the active organization.
    ///
    /// # Errors
    ///
    /// Returns an error unless the candidate is the next immutable revision and
    /// branches from the currently active known-good revision.
    pub fn record_candidate_organization_revision(
        &mut self,
        candidate: &mcg_agents::OrganizationGenome,
    ) -> Result<CandidateRevisionMutation, StorageError> {
        candidate
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        if let Some(existing) = self.organization_revision(candidate.revision_id)? {
            if existing == *candidate {
                return Ok(CandidateRevisionMutation {
                    candidate: existing,
                    replayed: true,
                    events: Vec::new(),
                });
            }
            return Err(StorageError::Conflict(
                "candidate revision identity was reused with different content".into(),
            ));
        }
        let current = self
            .organization(candidate.organization_id)?
            .ok_or_else(|| StorageError::Conflict("candidate organization was not found".into()))?;
        if candidate.parent_revision_id != Some(current.revision_id) {
            return Err(StorageError::Conflict(
                "candidate must branch from the active organization revision".into(),
            ));
        }
        let latest_revision: i64 = self.connection.query_row(
            "SELECT MAX(revision) FROM organization_revisions WHERE organization_id = ?1",
            [candidate.organization_id.to_string()],
            |row| row.get(0),
        )?;
        let expected_revision = u64::try_from(latest_revision)
            .map_err(|_| StorageError::InvalidData("negative organization revision".into()))?
            .saturating_add(1);
        if candidate.revision != expected_revision {
            return Err(StorageError::Conflict(
                "candidate must use the next organization revision number".into(),
            ));
        }
        let event = EventEnvelope::new(
            EventKind::OrganizationRevisionRecorded,
            None,
            serde_json::json!({"candidate": candidate}),
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO organization_revisions(
                revision_id, organization_id, revision, parent_revision_id,
                organization_json, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                candidate.revision_id.to_string(),
                candidate.organization_id.to_string(),
                i64::try_from(candidate.revision).map_err(|_| StorageError::InvalidData(
                    "organization revision exceeds SQLite range".into()
                ))?,
                candidate.parent_revision_id.map(|id| id.to_string()),
                serde_json::to_string(candidate)?,
                candidate.created_at.to_rfc3339(),
            ],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(CandidateRevisionMutation {
            candidate: candidate.clone(),
            replayed: false,
            events: vec![event],
        })
    }

    /// Records one immutable, evidence-bearing supervision observation.
    ///
    /// Exact identity retries are observation-only; conflicting identity reuse fails.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed evidence or conflicting persisted identity.
    pub fn record_supervision_observation(
        &mut self,
        observation: &SupervisionObservation,
    ) -> Result<bool, StorageError> {
        observation
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        if let Some(existing) = self.supervision_observation(observation.id)? {
            if existing == *observation {
                return Ok(true);
            }
            return Err(StorageError::Conflict(
                "supervision observation identity was reused with different content".into(),
            ));
        }
        let event = EventEnvelope::new(
            EventKind::SupervisionObserved,
            observation.task_id,
            serde_json::to_value(observation)?,
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO supervision_observations(
                observation_id, loop_kind, signal, observation_json, observed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                observation.id.to_string(),
                supervision_loop_name(observation.loop_kind),
                supervision_signal_name(observation.signal),
                serde_json::to_string(observation)?,
                observation.observed_at.to_rfc3339(),
            ],
        )?;
        Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(false)
    }

    /// Atomically records all deterministic findings for one durable evaluation.
    /// Exact analysis replay is observation-only, including evaluations with no findings.
    ///
    /// # Errors
    /// Returns an error for unknown evaluations, mismatched scope, invalid findings,
    /// malformed analysis identity, or conflicting replay.
    #[allow(clippy::too_many_lines)] // Findings, marker, and events are one exactly-once transaction.
    pub fn record_automatic_supervision(
        &mut self,
        evaluation_id: EvaluationId,
        analysis_sha256: &str,
        observations: &[SupervisionObservation],
        processed_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<AutomaticSupervisionMutation, StorageError> {
        if analysis_sha256.len() != 64
            || !analysis_sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(StorageError::InvalidData(
                "automatic supervision analysis hash is invalid".into(),
            ));
        }
        let evaluation_json = self
            .connection
            .query_row(
                "SELECT evaluation_json FROM evaluations WHERE evaluation_id = ?1",
                [evaluation_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| StorageError::Conflict("evaluation was not found".into()))?;
        let evaluation: mcg_agents::EvaluationReport = serde_json::from_str(&evaluation_json)?;
        for observation in observations {
            observation
                .validate()
                .map_err(|error| StorageError::InvalidData(error.to_string()))?;
            if observation.task_id != Some(evaluation.task_id)
                || observation.run_id != Some(evaluation.run_id)
            {
                return Err(StorageError::Conflict(
                    "automatic observation scope does not match its evaluation".into(),
                ));
            }
        }
        let existing = self
            .connection
            .query_row(
                "SELECT analysis_sha256, observation_ids_json
                 FROM supervised_evaluations WHERE evaluation_id = ?1",
                [evaluation_id.to_string()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        if let Some((existing_hash, ids_json)) = existing {
            if existing_hash != analysis_sha256 {
                return Err(StorageError::Conflict(
                    "evaluation supervision was replayed with different analysis".into(),
                ));
            }
            let ids: Vec<SupervisionObservationId> = serde_json::from_str(&ids_json)?;
            let mut existing_observations = Vec::with_capacity(ids.len());
            for id in ids {
                existing_observations.push(self.supervision_observation(id)?.ok_or_else(|| {
                    StorageError::InvalidData(
                        "automatic supervision marker references missing evidence".into(),
                    )
                })?);
            }
            if existing_observations != observations {
                return Err(StorageError::Conflict(
                    "evaluation supervision observations changed under the same analysis".into(),
                ));
            }
            return Ok(AutomaticSupervisionMutation {
                observations: existing_observations,
                replayed: true,
                events: Vec::new(),
            });
        }
        let transaction = self.connection.transaction()?;
        let mut events = Vec::with_capacity(observations.len());
        for observation in observations {
            transaction.execute(
                "INSERT INTO supervision_observations(
                    observation_id, loop_kind, signal, observation_json, observed_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    observation.id.to_string(),
                    supervision_loop_name(observation.loop_kind),
                    supervision_signal_name(observation.signal),
                    serde_json::to_string(observation)?,
                    observation.observed_at.to_rfc3339(),
                ],
            )?;
            events.push(Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::SupervisionObserved,
                    observation.task_id,
                    serde_json::to_value(observation)?,
                ),
            )?);
        }
        let ids = observations
            .iter()
            .map(|observation| observation.id)
            .collect::<Vec<_>>();
        transaction.execute(
            "INSERT INTO supervised_evaluations(
                evaluation_id, analysis_sha256, observation_ids_json, processed_at
             ) VALUES (?1, ?2, ?3, ?4)",
            params![
                evaluation_id.to_string(),
                analysis_sha256,
                serde_json::to_string(&ids)?,
                processed_at.to_rfc3339(),
            ],
        )?;
        transaction.commit()?;
        Ok(AutomaticSupervisionMutation {
            observations: observations.to_vec(),
            replayed: false,
            events,
        })
    }

    /// Loads one immutable supervision observation.
    ///
    /// # Errors
    ///
    /// Returns an error when persisted JSON is invalid.
    pub fn supervision_observation(
        &self,
        id: SupervisionObservationId,
    ) -> Result<Option<SupervisionObservation>, StorageError> {
        self.read_optional_json(
            "SELECT observation_json FROM supervision_observations WHERE observation_id = ?1",
            &id.to_string(),
        )
    }

    /// Lists immutable supervision observations in observation order.
    ///
    /// # Errors
    ///
    /// Returns an error when persisted JSON is invalid.
    pub fn supervision_observations(&self) -> Result<Vec<SupervisionObservation>, StorageError> {
        self.read_all_json(
            "SELECT observation_json FROM supervision_observations
             ORDER BY observed_at, observation_id",
        )
    }

    /// Creates an experiment over an inactive candidate and active baseline.
    ///
    /// # Errors
    ///
    /// Returns an error for stale baselines, unknown evidence, revision mismatch,
    /// or conflicting retry identity.
    pub fn create_experiment(
        &mut self,
        experiment: &ExperimentDefinition,
    ) -> Result<ExperimentMutation, StorageError> {
        experiment
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        if experiment.state != ExperimentState::Draft {
            return Err(StorageError::InvalidData(
                "new experiments must start as draft".into(),
            ));
        }
        if let Some(existing) = self.experiment(experiment.id)? {
            if existing == *experiment {
                return Ok(ExperimentMutation {
                    experiment: existing,
                    replayed: true,
                    events: Vec::new(),
                });
            }
            return Err(StorageError::Conflict(
                "experiment identity was reused with different content".into(),
            ));
        }
        let baseline = self.experiment_revision(
            experiment.organization_id,
            experiment.baseline_revision_id,
            "baseline",
        )?;
        let candidate = self.experiment_revision(
            experiment.organization_id,
            experiment.candidate_revision_id,
            "candidate",
        )?;
        let current = self
            .organization(experiment.organization_id)?
            .ok_or_else(|| StorageError::Conflict("organization was not found".into()))?;
        if current.revision_id != baseline.revision_id
            || candidate.parent_revision_id != Some(baseline.revision_id)
        {
            return Err(StorageError::Conflict(
                "experiment baseline is stale or candidate has a different parent".into(),
            ));
        }
        for observation_id in &experiment.observation_ids {
            if self.supervision_observation(*observation_id)?.is_none() {
                return Err(StorageError::Conflict(format!(
                    "supervision observation {observation_id} was not found"
                )));
            }
        }
        let event = EventEnvelope::new(
            EventKind::ExperimentCreated,
            None,
            serde_json::to_value(experiment)?,
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO experiments(
                experiment_id, organization_id, baseline_revision_id,
                candidate_revision_id, mode, state, experiment_json, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                experiment.id.to_string(),
                experiment.organization_id.to_string(),
                experiment.baseline_revision_id.to_string(),
                experiment.candidate_revision_id.to_string(),
                experiment_mode_name(experiment.mode),
                experiment_state_name(experiment.state),
                serde_json::to_string(experiment)?,
                experiment.created_at.to_rfc3339(),
                experiment.updated_at.to_rfc3339(),
            ],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(ExperimentMutation {
            experiment: experiment.clone(),
            replayed: false,
            events: vec![event],
        })
    }

    /// Loads one experiment projection.
    ///
    /// # Errors
    ///
    /// Returns an error when persisted JSON is invalid.
    pub fn experiment(
        &self,
        id: ExperimentId,
    ) -> Result<Option<ExperimentDefinition>, StorageError> {
        self.read_optional_json(
            "SELECT experiment_json FROM experiments WHERE experiment_id = ?1",
            &id.to_string(),
        )
    }

    /// Lists experiments for one organization in creation order.
    ///
    /// # Errors
    ///
    /// Returns an error when persisted JSON is invalid.
    pub fn experiments(
        &self,
        organization_id: OrganizationId,
    ) -> Result<Vec<ExperimentDefinition>, StorageError> {
        self.read_json_rows(
            "SELECT experiment_json FROM experiments
             WHERE organization_id = ?1 ORDER BY created_at, experiment_id",
            &organization_id.to_string(),
        )
    }

    /// Applies one legal experiment state transition and appends evidence.
    ///
    /// # Errors
    ///
    /// Returns an error for stale callers or illegal transitions.
    pub fn transition_experiment(
        &mut self,
        id: ExperimentId,
        expected: ExperimentState,
        next: ExperimentState,
        at: chrono::DateTime<chrono::Utc>,
    ) -> Result<ExperimentMutation, StorageError> {
        let mut experiment = self
            .experiment(id)?
            .ok_or_else(|| StorageError::Conflict("experiment was not found".into()))?;
        if experiment.state != expected || !expected.can_transition_to(next) {
            return Err(StorageError::Conflict(
                "experiment transition is stale or illegal".into(),
            ));
        }
        experiment.state = next;
        experiment.updated_at = at;
        let event = EventEnvelope::new(
            EventKind::ExperimentStateChanged,
            None,
            serde_json::json!({"from": expected, "to": next, "experiment": &experiment}),
        );
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE experiments SET state = ?1, experiment_json = ?2, updated_at = ?3
             WHERE experiment_id = ?4 AND state = ?5",
            params![
                experiment_state_name(next),
                serde_json::to_string(&experiment)?,
                at.to_rfc3339(),
                id.to_string(),
                experiment_state_name(expected),
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::Conflict(
                "experiment changed concurrently".into(),
            ));
        }
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(ExperimentMutation {
            experiment,
            replayed: false,
            events: vec![event],
        })
    }

    /// Records an immutable proof that candidate output had evaluation-only authority.
    ///
    /// # Errors
    ///
    /// Returns an error unless a shadow experiment is running and the source task exists.
    pub fn record_shadow_execution(
        &mut self,
        receipt: &ShadowExecutionReceipt,
    ) -> Result<bool, StorageError> {
        receipt
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let experiment = self
            .experiment(receipt.experiment_id)?
            .ok_or_else(|| StorageError::Conflict("experiment was not found".into()))?;
        if experiment.mode != ExperimentMode::Shadow || experiment.state != ExperimentState::Running
        {
            return Err(StorageError::Conflict(
                "shadow execution requires a running shadow experiment".into(),
            ));
        }
        let existing = self
            .connection
            .query_row(
                "SELECT receipt_json FROM shadow_execution_receipts WHERE candidate_run_id = ?1",
                [receipt.candidate_run_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if let Some(json) = existing {
            let existing: ShadowExecutionReceipt = serde_json::from_str(&json)?;
            if existing == *receipt {
                return Ok(true);
            }
            return Err(StorageError::Conflict(
                "shadow run identity was reused with different content".into(),
            ));
        }
        let event = EventEnvelope::new(
            EventKind::ShadowExecutionRecorded,
            Some(receipt.source_task_id),
            serde_json::to_value(receipt)?,
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO shadow_execution_receipts(
                candidate_run_id, experiment_id, source_task_id, receipt_json, completed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                receipt.candidate_run_id.to_string(),
                receipt.experiment_id.to_string(),
                receipt.source_task_id.to_string(),
                serde_json::to_string(receipt)?,
                receipt.completed_at.to_rfc3339(),
            ],
        )?;
        Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(false)
    }

    /// Lists immutable shadow execution receipts for one experiment.
    ///
    /// # Errors
    /// Returns an error when persisted evidence cannot be decoded.
    pub fn shadow_executions(
        &self,
        id: ExperimentId,
    ) -> Result<Vec<ShadowExecutionReceipt>, StorageError> {
        self.read_json_rows(
            "SELECT receipt_json FROM shadow_execution_receipts
             WHERE experiment_id = ?1 ORDER BY completed_at, candidate_run_id",
            &id.to_string(),
        )
    }

    /// Records one immutable paired benchmark.
    ///
    /// # Errors
    ///
    /// Returns an error unless the experiment is running and both arms use its revisions.
    pub fn record_experiment_benchmark(
        &mut self,
        benchmark: &ExperimentBenchmark,
    ) -> Result<bool, StorageError> {
        benchmark
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let experiment = self
            .experiment(benchmark.experiment_id)?
            .ok_or_else(|| StorageError::Conflict("experiment was not found".into()))?;
        if experiment.state != ExperimentState::Running
            || benchmark.baseline.organization_revision_id != experiment.baseline_revision_id
            || benchmark.candidate.organization_revision_id != experiment.candidate_revision_id
        {
            return Err(StorageError::Conflict(
                "benchmark arms do not match the running experiment".into(),
            ));
        }
        if let Some(existing) = self.experiment_benchmark(benchmark.id)? {
            if same_json(&existing, benchmark)? {
                return Ok(true);
            }
            return Err(StorageError::Conflict(
                "benchmark identity was reused with different content".into(),
            ));
        }
        let event = EventEnvelope::new(
            EventKind::ExperimentBenchmarkRecorded,
            None,
            serde_json::to_value(benchmark)?,
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO experiment_benchmarks(
                benchmark_id, experiment_id, scenario_key, benchmark_json, measured_at
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                benchmark.id.to_string(),
                benchmark.experiment_id.to_string(),
                &benchmark.scenario_key,
                serde_json::to_string(benchmark)?,
                benchmark.measured_at.to_rfc3339(),
            ],
        )?;
        Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(false)
    }

    /// Lists immutable benchmarks for one experiment.
    ///
    /// # Errors
    ///
    /// Returns an error when persisted JSON is invalid.
    pub fn experiment_benchmarks(
        &self,
        id: ExperimentId,
    ) -> Result<Vec<ExperimentBenchmark>, StorageError> {
        self.read_json_rows(
            "SELECT benchmark_json FROM experiment_benchmarks
             WHERE experiment_id = ?1 ORDER BY measured_at, benchmark_id",
            &id.to_string(),
        )
    }

    /// Atomically records a decision and promotes, rejects, or reverts its revision.
    ///
    /// # Errors
    ///
    /// Returns an error for stale heads, insufficient evidence, regression-policy
    /// failure, wrong revision direction, or conflicting replay identity.
    #[allow(clippy::too_many_lines)] // Decision evidence, projection, head, and event must commit atomically.
    pub fn decide_experiment(
        &mut self,
        decision: &ExperimentDecision,
    ) -> Result<ExperimentMutation, StorageError> {
        decision
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        if let Some(existing) = self.experiment_decision(decision.id)? {
            if existing == *decision {
                let experiment = self
                    .experiment(decision.experiment_id)?
                    .ok_or_else(|| StorageError::Conflict("experiment was not found".into()))?;
                return Ok(ExperimentMutation {
                    experiment,
                    replayed: true,
                    events: Vec::new(),
                });
            }
            return Err(StorageError::Conflict(
                "decision identity was reused with different content".into(),
            ));
        }
        let mut experiment = self
            .experiment(decision.experiment_id)?
            .ok_or_else(|| StorageError::Conflict("experiment was not found".into()))?;
        let benchmarks = self.decision_benchmarks(&experiment, decision)?;
        let current = self
            .organization(experiment.organization_id)?
            .ok_or_else(|| StorageError::Conflict("organization was not found".into()))?;

        let (next_state, update_head) = match decision.kind {
            ExperimentDecisionKind::Promote => {
                if experiment.state != ExperimentState::AwaitingDecision
                    || decision.from_revision_id != experiment.baseline_revision_id
                    || decision.to_revision_id != experiment.candidate_revision_id
                    || current.revision_id != experiment.baseline_revision_id
                {
                    return Err(StorageError::Conflict(
                        "promotion direction, state, or active baseline is stale".into(),
                    ));
                }
                Self::validate_promotion_evidence(&experiment, &benchmarks)?;
                (ExperimentState::Promoted, true)
            }
            ExperimentDecisionKind::Revert => {
                if experiment.state != ExperimentState::Promoted
                    || decision.from_revision_id != experiment.candidate_revision_id
                    || decision.to_revision_id != experiment.baseline_revision_id
                    || current.revision_id != experiment.candidate_revision_id
                {
                    return Err(StorageError::Conflict(
                        "reversion direction, state, or active candidate is stale".into(),
                    ));
                }
                (ExperimentState::Reverted, true)
            }
            ExperimentDecisionKind::Reject => {
                if !experiment
                    .state
                    .can_transition_to(ExperimentState::Rejected)
                    || decision.from_revision_id != experiment.candidate_revision_id
                    || decision.to_revision_id != experiment.baseline_revision_id
                    || current.revision_id != experiment.baseline_revision_id
                {
                    return Err(StorageError::Conflict(
                        "rejection direction, state, or active baseline is stale".into(),
                    ));
                }
                (ExperimentState::Rejected, false)
            }
        };
        let previous_state = experiment.state;
        experiment.state = next_state;
        experiment.updated_at = decision.decided_at;
        let event = EventEnvelope::new(
            EventKind::ExperimentDecisionRecorded,
            None,
            serde_json::json!({"decision": decision, "experiment": &experiment}),
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO experiment_decisions(
                decision_id, experiment_id, kind, decision_json, decided_at
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                decision.id.to_string(),
                decision.experiment_id.to_string(),
                decision_kind_name(decision.kind),
                serde_json::to_string(decision)?,
                decision.decided_at.to_rfc3339(),
            ],
        )?;
        if update_head {
            let target = Self::experiment_revision_from_transaction(
                &transaction,
                experiment.organization_id,
                decision.to_revision_id,
            )?;
            let changed = transaction.execute(
                "UPDATE organizations SET current_revision_id = ?1,
                    organization_json = ?2, updated_at = ?3
                 WHERE organization_id = ?4 AND current_revision_id = ?5",
                params![
                    target.revision_id.to_string(),
                    serde_json::to_string(&target)?,
                    decision.decided_at.to_rfc3339(),
                    experiment.organization_id.to_string(),
                    decision.from_revision_id.to_string(),
                ],
            )?;
            if changed != 1 {
                return Err(StorageError::Conflict(
                    "organization head changed concurrently".into(),
                ));
            }
        }
        let changed = transaction.execute(
            "UPDATE experiments SET state = ?1, experiment_json = ?2, updated_at = ?3
             WHERE experiment_id = ?4 AND state = ?5",
            params![
                experiment_state_name(next_state),
                serde_json::to_string(&experiment)?,
                decision.decided_at.to_rfc3339(),
                experiment.id.to_string(),
                experiment_state_name(previous_state),
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::Conflict(
                "experiment changed concurrently".into(),
            ));
        }
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(ExperimentMutation {
            experiment,
            replayed: false,
            events: vec![event],
        })
    }

    /// Lists immutable decisions for one experiment.
    ///
    /// # Errors
    ///
    /// Returns an error when persisted JSON is invalid.
    pub fn experiment_decisions(
        &self,
        id: ExperimentId,
    ) -> Result<Vec<ExperimentDecision>, StorageError> {
        self.read_json_rows(
            "SELECT decision_json FROM experiment_decisions
             WHERE experiment_id = ?1 ORDER BY decided_at, decision_id",
            &id.to_string(),
        )
    }

    fn experiment_revision(
        &self,
        organization_id: OrganizationId,
        revision_id: OrganizationRevisionId,
        label: &str,
    ) -> Result<mcg_agents::OrganizationGenome, StorageError> {
        let revision = self
            .organization_revision(revision_id)?
            .ok_or_else(|| StorageError::Conflict(format!("{label} revision was not found")))?;
        if revision.organization_id != organization_id {
            return Err(StorageError::Conflict(format!(
                "{label} revision belongs to a different organization"
            )));
        }
        Ok(revision)
    }

    fn experiment_revision_from_transaction(
        transaction: &rusqlite::Transaction<'_>,
        organization_id: OrganizationId,
        revision_id: OrganizationRevisionId,
    ) -> Result<mcg_agents::OrganizationGenome, StorageError> {
        let json = transaction
            .query_row(
                "SELECT organization_json FROM organization_revisions WHERE revision_id = ?1",
                [revision_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| StorageError::Conflict("target revision was not found".into()))?;
        let revision: mcg_agents::OrganizationGenome = serde_json::from_str(&json)?;
        if revision.organization_id != organization_id {
            return Err(StorageError::Conflict(
                "target revision belongs to a different organization".into(),
            ));
        }
        Ok(revision)
    }

    fn experiment_benchmark(
        &self,
        id: ExperimentBenchmarkId,
    ) -> Result<Option<ExperimentBenchmark>, StorageError> {
        self.read_optional_json(
            "SELECT benchmark_json FROM experiment_benchmarks WHERE benchmark_id = ?1",
            &id.to_string(),
        )
    }

    fn experiment_decision(
        &self,
        id: mcg_domain::ExperimentDecisionId,
    ) -> Result<Option<ExperimentDecision>, StorageError> {
        self.read_optional_json(
            "SELECT decision_json FROM experiment_decisions WHERE decision_id = ?1",
            &id.to_string(),
        )
    }

    fn decision_benchmarks(
        &self,
        experiment: &ExperimentDefinition,
        decision: &ExperimentDecision,
    ) -> Result<Vec<ExperimentBenchmark>, StorageError> {
        let mut benchmarks = Vec::with_capacity(decision.benchmark_ids.len());
        for id in &decision.benchmark_ids {
            let benchmark = self
                .experiment_benchmark(*id)?
                .ok_or_else(|| StorageError::Conflict(format!("benchmark {id} was not found")))?;
            if benchmark.experiment_id != experiment.id {
                return Err(StorageError::Conflict(
                    "decision references a benchmark from another experiment".into(),
                ));
            }
            benchmarks.push(benchmark);
        }
        Ok(benchmarks)
    }

    fn validate_promotion_evidence(
        experiment: &ExperimentDefinition,
        benchmarks: &[ExperimentBenchmark],
    ) -> Result<(), StorageError> {
        if benchmarks.len() < usize::from(experiment.required_benchmarks) {
            return Err(StorageError::Conflict(
                "promotion has insufficient benchmark evidence".into(),
            ));
        }
        let regression_count = benchmarks
            .iter()
            .map(|benchmark| benchmark.regressions.len())
            .sum::<usize>();
        if regression_count > usize::from(experiment.maximum_regressions)
            || benchmarks.iter().any(|benchmark| {
                benchmark.candidate.outcome != mcg_agents::EvaluationOutcome::Passed
                    || benchmark.candidate.confidence.basis_points
                        < experiment.minimum_confidence.basis_points
            })
        {
            return Err(StorageError::Conflict(
                "candidate does not satisfy promotion policy".into(),
            ));
        }
        Ok(())
    }
}

fn same_json<T: serde::Serialize>(left: &T, right: &T) -> Result<bool, StorageError> {
    Ok(serde_json::to_value(left)? == serde_json::to_value(right)?)
}

const fn experiment_state_name(state: ExperimentState) -> &'static str {
    match state {
        ExperimentState::Draft => "draft",
        ExperimentState::Ready => "ready",
        ExperimentState::Running => "running",
        ExperimentState::AwaitingDecision => "awaiting_decision",
        ExperimentState::Promoted => "promoted",
        ExperimentState::Rejected => "rejected",
        ExperimentState::Reverted => "reverted",
    }
}

const fn experiment_mode_name(mode: ExperimentMode) -> &'static str {
    match mode {
        ExperimentMode::Shadow => "shadow",
        ExperimentMode::Scenario => "scenario",
    }
}

const fn decision_kind_name(kind: ExperimentDecisionKind) -> &'static str {
    match kind {
        ExperimentDecisionKind::Promote => "promote",
        ExperimentDecisionKind::Reject => "reject",
        ExperimentDecisionKind::Revert => "revert",
    }
}

const fn supervision_loop_name(value: SupervisionLoop) -> &'static str {
    match value {
        SupervisionLoop::Fast => "fast",
        SupervisionLoop::Medium => "medium",
        SupervisionLoop::Slow => "slow",
    }
}

const fn supervision_signal_name(value: SupervisionSignal) -> &'static str {
    match value {
        SupervisionSignal::RepeatedFailure => "repeated_failure",
        SupervisionSignal::AgentDisagreement => "agent_disagreement",
        SupervisionSignal::RunawayLoop => "runaway_loop",
        SupervisionSignal::Overconfidence => "overconfidence",
        SupervisionSignal::Underconfidence => "underconfidence",
        SupervisionSignal::ToolFailure => "tool_failure",
        SupervisionSignal::BudgetPressure => "budget_pressure",
        SupervisionSignal::ExcessiveOverhead => "excessive_overhead",
        SupervisionSignal::UserCorrection => "user_correction",
        SupervisionSignal::Regression => "regression",
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};
    use mcg_agents::{
        BenchmarkArmResult, ConfidenceEstimate, EXPERIMENT_SCHEMA_VERSION, EvaluationOutcome,
        ExperimentBenchmark, ExperimentDecision, ExperimentDecisionKind, ExperimentDefinition,
        ExperimentMode, ExperimentState, OrganizationalMetrics, ProspectiveAnalysis,
        SUPERVISION_SCHEMA_VERSION, ShadowAuthority, ShadowExecutionReceipt, SupervisionEvidence,
        SupervisionLoop, SupervisionObservation, SupervisionSignal,
    };
    use mcg_domain::{
        EvaluationId, ExperimentBenchmarkId, ExperimentDecisionId, ExperimentId,
        OrganizationRevisionId, RunId, SupervisionObservationId, TaskEnvelope,
    };

    use super::*;

    fn candidate(
        baseline: &mcg_agents::OrganizationGenome,
        revision: u64,
    ) -> mcg_agents::OrganizationGenome {
        let mut value = baseline.clone();
        value.revision_id = OrganizationRevisionId::new();
        value.revision = revision;
        value.parent_revision_id = Some(baseline.revision_id);
        value.name = format!("{} candidate {revision}", baseline.name);
        value.created_at += Duration::seconds(i64::try_from(revision).expect("revision"));
        value
    }

    fn arm(revision_id: OrganizationRevisionId, passed: bool) -> BenchmarkArmResult {
        BenchmarkArmResult {
            organization_revision_id: revision_id,
            run_id: RunId::new(),
            evaluation_id: EvaluationId::new(),
            outcome: if passed {
                EvaluationOutcome::Passed
            } else {
                EvaluationOutcome::Failed
            },
            confidence: ConfidenceEstimate::new(9_000, 10).expect("confidence"),
            metrics: OrganizationalMetrics::default(),
            evidence_sha256: vec!["a".repeat(64)],
        }
    }

    fn experiment(
        baseline: &mcg_agents::OrganizationGenome,
        candidate: &mcg_agents::OrganizationGenome,
        observation_id: SupervisionObservationId,
        now: chrono::DateTime<Utc>,
    ) -> ExperimentDefinition {
        ExperimentDefinition {
            schema_version: EXPERIMENT_SCHEMA_VERSION,
            id: ExperimentId::new(),
            organization_id: baseline.organization_id,
            baseline_revision_id: baseline.revision_id,
            candidate_revision_id: candidate.revision_id,
            mode: ExperimentMode::Shadow,
            state: ExperimentState::Draft,
            hypothesis: "candidate reduces retries without reducing correctness".into(),
            task_classes: vec!["coding/build".into()],
            required_benchmarks: 1,
            maximum_regressions: 0,
            minimum_confidence: ConfidenceEstimate::new(8_000, 1).expect("confidence"),
            observation_ids: vec![observation_id],
            created_at: now,
            updated_at: now,
        }
    }

    fn observation(
        baseline: &mcg_agents::OrganizationGenome,
        now: chrono::DateTime<Utc>,
    ) -> SupervisionObservation {
        SupervisionObservation {
            schema_version: SUPERVISION_SCHEMA_VERSION,
            id: SupervisionObservationId::new(),
            loop_kind: SupervisionLoop::Slow,
            signal: SupervisionSignal::RepeatedFailure,
            severity_basis_points: 7_500,
            recommended_intervention: mcg_agents::InterventionLevel::Nudge,
            task_id: None,
            run_id: None,
            organization_revision_id: Some(baseline.revision_id),
            summary: "three comparable build tasks repeated the same retry pattern".into(),
            evidence: vec![SupervisionEvidence {
                method: "evaluation_history".into(),
                artifact_sha256: Some("b".repeat(64)),
                event_sequence: Some(1),
                detail: "three failed first attempts".into(),
            }],
            observed_at: now,
        }
    }

    fn decision(
        experiment: &ExperimentDefinition,
        kind: ExperimentDecisionKind,
        benchmark_id: ExperimentBenchmarkId,
        now: chrono::DateTime<Utc>,
    ) -> ExperimentDecision {
        let (from, to) = match kind {
            ExperimentDecisionKind::Promote => (
                experiment.baseline_revision_id,
                experiment.candidate_revision_id,
            ),
            ExperimentDecisionKind::Reject | ExperimentDecisionKind::Revert => (
                experiment.candidate_revision_id,
                experiment.baseline_revision_id,
            ),
        };
        ExperimentDecision {
            schema_version: EXPERIMENT_SCHEMA_VERSION,
            id: ExperimentDecisionId::new(),
            experiment_id: experiment.id,
            kind,
            from_revision_id: from,
            to_revision_id: to,
            benchmark_ids: vec![benchmark_id],
            prospective_analysis: ProspectiveAnalysis {
                expected_benefits: vec!["fewer retries".into()],
                expected_risks: vec!["higher cost".into()],
                historical_evidence: vec!["coding/build".into()],
            },
            reason: "paired evidence satisfies the configured decision policy".into(),
            decided_at: now,
        }
    }

    #[test]
    fn observations_are_idempotent_immutable_and_restart_durable() {
        let directory = tempfile::tempdir().expect("directory");
        let database = directory.path().join("mcg.sqlite3");
        let now = Utc::now();
        let baseline = mcg_agents::OrganizationGenome::consumer_default(now);
        let value = observation(&baseline, now);
        let mut store = SqliteEventStore::open(&database).expect("store");
        store
            .record_organization_revision(&baseline)
            .expect("baseline");
        assert!(
            !store
                .record_supervision_observation(&value)
                .expect("record")
        );
        assert!(
            store
                .record_supervision_observation(&value)
                .expect("replay")
        );
        assert!(
            store
                .connection
                .execute("DELETE FROM supervision_observations", [])
                .is_err()
        );
        drop(store);
        let reopened = SqliteEventStore::open(&database).expect("reopen");
        assert_eq!(
            reopened
                .supervision_observation(value.id)
                .expect("observation"),
            Some(value)
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One contiguous promotion/reversion crash-safety scenario.
    fn promotion_and_reversion_atomically_move_the_active_revision() {
        let directory = tempfile::tempdir().expect("directory");
        let database = directory.path().join("mcg.sqlite3");
        let now = Utc::now();
        let baseline = mcg_agents::OrganizationGenome::consumer_default(now);
        let candidate = candidate(&baseline, 2);
        let observed = observation(&baseline, now);
        let definition = experiment(&baseline, &candidate, observed.id, now);
        let benchmark = ExperimentBenchmark {
            schema_version: EXPERIMENT_SCHEMA_VERSION,
            id: ExperimentBenchmarkId::new(),
            experiment_id: definition.id,
            scenario_key: "coding/build".into(),
            baseline: arm(baseline.revision_id, true),
            candidate: arm(candidate.revision_id, true),
            regressions: Vec::new(),
            measured_at: now + Duration::seconds(3),
        };
        let mut store = SqliteEventStore::open(&database).expect("store");
        store
            .record_organization_revision(&baseline)
            .expect("baseline");
        store
            .record_candidate_organization_revision(&candidate)
            .expect("candidate");
        assert_eq!(
            store
                .organization(baseline.organization_id)
                .expect("organization")
                .expect("present")
                .revision_id,
            baseline.revision_id
        );
        store
            .record_supervision_observation(&observed)
            .expect("observation");
        store.create_experiment(&definition).expect("experiment");
        store
            .transition_experiment(
                definition.id,
                ExperimentState::Draft,
                ExperimentState::Ready,
                now + Duration::seconds(1),
            )
            .expect("ready");
        let automatic_outcome = AutomaticExperimentOutcome {
            observation_id: observed.id,
            disposition: AutomaticExperimentDisposition::Ready,
            candidate_revision_id: Some(candidate.revision_id),
            experiment_id: Some(definition.id),
            detail: "candidate is ready for isolated evaluation".into(),
            processed_at: now + Duration::seconds(1),
            replayed: false,
        };
        assert!(
            !store
                .record_automatic_experiment_outcome(&automatic_outcome)
                .expect("automatic outcome")
                .replayed
        );
        let mut expected_replay = automatic_outcome.clone();
        expected_replay.replayed = true;
        assert_eq!(
            store
                .record_automatic_experiment_outcome(&automatic_outcome)
                .expect("automatic outcome replay"),
            expected_replay
        );
        assert!(
            store
                .connection
                .execute("DELETE FROM automatic_experiment_outcomes", [])
                .is_err()
        );
        store
            .transition_experiment(
                definition.id,
                ExperimentState::Ready,
                ExperimentState::Running,
                now + Duration::seconds(2),
            )
            .expect("running");
        store
            .record_experiment_benchmark(&benchmark)
            .expect("benchmark");
        store
            .transition_experiment(
                definition.id,
                ExperimentState::Running,
                ExperimentState::AwaitingDecision,
                now + Duration::seconds(4),
            )
            .expect("decision ready");
        let promote = decision(
            &definition,
            ExperimentDecisionKind::Promote,
            benchmark.id,
            now + Duration::seconds(5),
        );
        let promoted = store.decide_experiment(&promote).expect("promote");
        assert_eq!(promoted.experiment.state, ExperimentState::Promoted);
        assert_eq!(
            store
                .organization(baseline.organization_id)
                .expect("organization")
                .expect("present")
                .revision_id,
            candidate.revision_id
        );
        let revert = decision(
            &definition,
            ExperimentDecisionKind::Revert,
            benchmark.id,
            now + Duration::seconds(6),
        );
        store.decide_experiment(&revert).expect("revert");
        assert_eq!(
            store
                .organization(baseline.organization_id)
                .expect("organization")
                .expect("present")
                .revision_id,
            baseline.revision_id
        );
        drop(store);
        let reopened = SqliteEventStore::open(&database).expect("reopen");
        assert_eq!(
            reopened
                .experiment(definition.id)
                .expect("experiment")
                .expect("present")
                .state,
            ExperimentState::Reverted
        );
        assert_eq!(
            reopened
                .experiment_decisions(definition.id)
                .expect("decisions"),
            vec![promote, revert]
        );
        assert_eq!(
            reopened
                .automatic_experiment_outcome(observed.id)
                .expect("automatic outcome"),
            Some(expected_replay)
        );
        assert!(
            reopened
                .pending_candidate_observation_ids(8)
                .expect("pending candidates")
                .is_empty()
        );
    }

    #[test]
    fn seeded_regression_cannot_promote_and_shadow_cannot_settle_real_task() {
        let now = Utc::now();
        let baseline = mcg_agents::OrganizationGenome::consumer_default(now);
        let candidate = candidate(&baseline, 2);
        let observed = observation(&baseline, now);
        let definition = experiment(&baseline, &candidate, observed.id, now);
        let task = TaskEnvelope::new("real production outcome must remain untouched");
        let benchmark = ExperimentBenchmark {
            schema_version: EXPERIMENT_SCHEMA_VERSION,
            id: ExperimentBenchmarkId::new(),
            experiment_id: definition.id,
            scenario_key: "coding/build".into(),
            baseline: arm(baseline.revision_id, true),
            candidate: arm(candidate.revision_id, false),
            regressions: vec!["seeded build regression".into()],
            measured_at: now + Duration::seconds(3),
        };
        let mut store = SqliteEventStore::open_in_memory().expect("store");
        store.create_task(&task).expect("task");
        store
            .record_organization_revision(&baseline)
            .expect("baseline");
        store
            .record_candidate_organization_revision(&candidate)
            .expect("candidate");
        store
            .record_supervision_observation(&observed)
            .expect("observation");
        store.create_experiment(&definition).expect("experiment");
        store
            .transition_experiment(
                definition.id,
                ExperimentState::Draft,
                ExperimentState::Ready,
                now + Duration::seconds(1),
            )
            .expect("ready");
        store
            .transition_experiment(
                definition.id,
                ExperimentState::Ready,
                ExperimentState::Running,
                now + Duration::seconds(2),
            )
            .expect("running");
        let shadow = ShadowExecutionReceipt {
            schema_version: EXPERIMENT_SCHEMA_VERSION,
            experiment_id: definition.id,
            source_task_id: task.id,
            candidate_run_id: RunId::new(),
            authority: ShadowAuthority::EvaluationOnly,
            output_artifact_sha256: "c".repeat(64),
            completed_at: now + Duration::seconds(3),
        };
        store.record_shadow_execution(&shadow).expect("shadow");
        assert_eq!(
            store.load_task(task.id).expect("task").expect("present"),
            task
        );
        store
            .record_experiment_benchmark(&benchmark)
            .expect("benchmark");
        store
            .transition_experiment(
                definition.id,
                ExperimentState::Running,
                ExperimentState::AwaitingDecision,
                now + Duration::seconds(4),
            )
            .expect("decision ready");
        let promote = decision(
            &definition,
            ExperimentDecisionKind::Promote,
            benchmark.id,
            now + Duration::seconds(5),
        );
        assert!(store.decide_experiment(&promote).is_err());
        assert_eq!(
            store
                .organization(baseline.organization_id)
                .expect("organization")
                .expect("present")
                .revision_id,
            baseline.revision_id
        );
        assert!(
            store
                .experiment_decisions(definition.id)
                .expect("decisions")
                .is_empty()
        );
    }
}
