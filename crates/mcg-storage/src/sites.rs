use chrono::{DateTime, Utc};
use mcg_domain::{
    SITE_SCHEMA_VERSION, ScheduleDefinition, ScheduleId, ScheduleState, Site, SiteDeployment,
    SiteDeploymentId, SiteDeploymentState, SiteHealthCheck, SiteHealthCheckId, SiteHealthState,
    SiteId, SiteMonitor, SiteRevision, SiteState,
};
use rusqlite::{OptionalExtension, Transaction, params};
use url::Url;

use crate::{SqliteEventStore, StorageError};

impl SqliteEventStore {
    /// Creates a durable site and its immutable first revision atomically.
    /// Exact caller-ID replay is observation-only.
    ///
    /// # Errors
    /// Returns an error for invalid definitions, missing projects, conflicts, or storage failure.
    pub fn create_site(
        &mut self,
        site: &Site,
        revision: &SiteRevision,
    ) -> Result<(Site, bool), StorageError> {
        validate_site(site)?;
        validate_revision(site, revision)?;
        if let Some(existing) = self.site(site.id)? {
            if existing.definition_hash == site.definition_hash {
                return Ok((existing, false));
            }
            return Err(StorageError::Conflict(format!(
                "site {} already exists with a different definition",
                site.id
            )));
        }
        let project_exists = self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE project_id = ?1)",
            [site.project_id.to_string()],
            |row| row.get::<_, bool>(0),
        )?;
        if !project_exists {
            return Err(StorageError::ProjectNotFound(site.project_id));
        }
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO sites(
                site_id, project_id, state, revision, definition_hash, site_json,
                created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                site.id.to_string(),
                site.project_id.to_string(),
                enum_json(&site.state)?,
                site.revision,
                site.definition_hash,
                serde_json::to_string(site)?,
                site.created_at.to_rfc3339(),
                site.updated_at.to_rfc3339(),
            ],
        )?;
        insert_revision(&transaction, revision)?;
        transaction.commit()?;
        Ok((site.clone(), true))
    }

    /// Replaces the current projection while retaining an immutable revision.
    ///
    /// # Errors
    /// Returns an error for stale revisions, changed identity, invalid data, or storage failure.
    pub fn update_site(
        &mut self,
        expected_revision: u32,
        site: &Site,
        revision: &SiteRevision,
    ) -> Result<Site, StorageError> {
        validate_site(site)?;
        validate_revision(site, revision)?;
        let current = self
            .site(site.id)?
            .ok_or(StorageError::SiteNotFound(site.id))?;
        if current.revision != expected_revision {
            return Err(StorageError::Conflict(format!(
                "site {} changed from expected revision {expected_revision}",
                site.id
            )));
        }
        if site.project_id != current.project_id
            || site.created_at != current.created_at
            || site.revision != expected_revision.saturating_add(1)
            || site.updated_at <= current.updated_at
        {
            return Err(StorageError::InvalidData(
                "site update changed immutable identity or revision ordering".into(),
            ));
        }
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE sites SET state = ?1, revision = ?2, definition_hash = ?3,
                site_json = ?4, updated_at = ?5
             WHERE site_id = ?6 AND revision = ?7",
            params![
                enum_json(&site.state)?,
                site.revision,
                site.definition_hash,
                serde_json::to_string(site)?,
                site.updated_at.to_rfc3339(),
                site.id.to_string(),
                expected_revision,
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::Conflict(
                "site revision changed concurrently".into(),
            ));
        }
        insert_revision(&transaction, revision)?;
        sync_monitor_schedule_state(&transaction, site, site.updated_at)?;
        transaction.commit()?;
        Ok(site.clone())
    }

    /// Loads one current site projection.
    ///
    /// # Errors
    /// Returns an error when `SQLite` or stored JSON is invalid.
    pub fn site(&self, site_id: SiteId) -> Result<Option<Site>, StorageError> {
        self.read_optional_json(
            "SELECT site_json FROM sites WHERE site_id = ?1",
            &site_id.to_string(),
        )
    }

    /// Lists current sites in stable recency order.
    ///
    /// # Errors
    /// Returns an error when `SQLite` or stored JSON is invalid.
    pub fn sites(&self) -> Result<Vec<Site>, StorageError> {
        self.read_all_json("SELECT site_json FROM sites ORDER BY updated_at DESC, site_id")
    }

    /// Lists immutable revisions for one site.
    ///
    /// # Errors
    /// Returns not-found for an unknown site or an error for invalid stored data.
    pub fn site_revisions(&self, site_id: SiteId) -> Result<Vec<SiteRevision>, StorageError> {
        if self.site(site_id)?.is_none() {
            return Err(StorageError::SiteNotFound(site_id));
        }
        let mut statement = self.connection.prepare(
            "SELECT revision_json FROM site_revisions WHERE site_id = ?1 ORDER BY revision",
        )?;
        let rows = statement.query_map([site_id.to_string()], |row| row.get::<_, String>(0))?;
        rows.map(|row| serde_json::from_str(&row?).map_err(StorageError::from))
            .collect()
    }

    /// Claims one deployment before any publication process starts.
    ///
    /// # Errors
    /// Returns an error for conflict, stale site revision, missing command, or invalid data.
    pub fn claim_site_deployment(
        &mut self,
        deployment: &SiteDeployment,
    ) -> Result<(SiteDeployment, bool), StorageError> {
        validate_deployment_claim(deployment)?;
        if let Some(existing) = self.site_deployment(deployment.id)? {
            if existing.request_hash == deployment.request_hash {
                return Ok((existing, false));
            }
            return Err(StorageError::Conflict(format!(
                "site deployment {} already exists with a different request",
                deployment.id
            )));
        }
        let site = self
            .site(deployment.site_id)?
            .ok_or(StorageError::SiteNotFound(deployment.site_id))?;
        if site.state != SiteState::Active
            || site.revision != deployment.site_revision
            || site.deployment_command.is_none()
        {
            return Err(StorageError::InvalidData(
                "deployment requires the current active site revision and an exact command".into(),
            ));
        }
        self.connection.execute(
            "INSERT INTO site_deployments(
                deployment_id, site_id, state, request_hash, deployment_json,
                created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                deployment.id.to_string(),
                deployment.site_id.to_string(),
                enum_json(&deployment.state)?,
                deployment.request_hash,
                serde_json::to_string(deployment)?,
                deployment.created_at.to_rfc3339(),
                deployment.updated_at.to_rfc3339(),
            ],
        )?;
        Ok((deployment.clone(), true))
    }

    /// Loads one deployment disposition.
    ///
    /// # Errors
    /// Returns an error when `SQLite` or stored JSON is invalid.
    pub fn site_deployment(
        &self,
        deployment_id: SiteDeploymentId,
    ) -> Result<Option<SiteDeployment>, StorageError> {
        self.read_optional_json(
            "SELECT deployment_json FROM site_deployments WHERE deployment_id = ?1",
            &deployment_id.to_string(),
        )
    }

    /// Lists deployment history for one site.
    ///
    /// # Errors
    /// Returns not-found for an unknown site or an error for invalid stored data.
    pub fn site_deployments(&self, site_id: SiteId) -> Result<Vec<SiteDeployment>, StorageError> {
        if self.site(site_id)?.is_none() {
            return Err(StorageError::SiteNotFound(site_id));
        }
        let mut statement = self.connection.prepare(
            "SELECT deployment_json FROM site_deployments
             WHERE site_id = ?1 ORDER BY created_at, deployment_id",
        )?;
        let rows = statement.query_map([site_id.to_string()], |row| row.get::<_, String>(0))?;
        rows.map(|row| serde_json::from_str(&row?).map_err(StorageError::from))
            .collect()
    }

    /// Commits the terminal disposition of one claimed deployment.
    ///
    /// # Errors
    /// Returns an error for invalid transitions, malformed evidence, or storage failure.
    pub fn finish_site_deployment(
        &mut self,
        deployment_id: SiteDeploymentId,
        state: SiteDeploymentState,
        detail: &str,
        log_artifact_sha256: Vec<String>,
        now: DateTime<Utc>,
    ) -> Result<SiteDeployment, StorageError> {
        if state == SiteDeploymentState::Executing
            || detail.trim().is_empty()
            || detail.len() > 512
            || log_artifact_sha256.len() > 4
            || log_artifact_sha256
                .iter()
                .any(|digest| !valid_digest(digest))
        {
            return Err(StorageError::InvalidData(
                "invalid terminal site deployment disposition".into(),
            ));
        }
        let mut deployment = self
            .site_deployment(deployment_id)?
            .ok_or_else(|| StorageError::InvalidData("site deployment not found".into()))?;
        if deployment.state != SiteDeploymentState::Executing {
            if deployment.state == state {
                return Ok(deployment);
            }
            return Err(StorageError::Conflict(
                "site deployment already has a terminal disposition".into(),
            ));
        }
        if now < deployment.updated_at {
            return Err(StorageError::InvalidData(
                "site deployment completion precedes its claim".into(),
            ));
        }
        deployment.state = state;
        deployment.detail = detail.trim().into();
        deployment.log_artifact_sha256 = log_artifact_sha256;
        deployment.updated_at = now;
        self.connection.execute(
            "UPDATE site_deployments SET state = ?1, deployment_json = ?2, updated_at = ?3
             WHERE deployment_id = ?4 AND state = 'executing'",
            params![
                enum_json(&deployment.state)?,
                serde_json::to_string(&deployment)?,
                deployment.updated_at.to_rfc3339(),
                deployment.id.to_string(),
            ],
        )?;
        Ok(deployment)
    }

    /// Converts crash-interrupted deployment claims to non-replayable uncertainty.
    ///
    /// # Errors
    /// Returns an error when deployments cannot be decoded or atomically updated.
    pub fn recover_site_deployments(&mut self, now: DateTime<Utc>) -> Result<u64, StorageError> {
        let mut statement = self
            .connection
            .prepare("SELECT deployment_json FROM site_deployments WHERE state = 'executing'")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        let mut deployments = rows
            .map(|row| serde_json::from_str::<SiteDeployment>(&row?).map_err(StorageError::from))
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let transaction = self.connection.transaction()?;
        for deployment in &mut deployments {
            deployment.state = SiteDeploymentState::Uncertain;
            deployment.detail = "daemon restart interrupted a claimed site deployment".into();
            deployment.updated_at = now.max(deployment.updated_at);
            transaction.execute(
                "UPDATE site_deployments SET state = 'uncertain', deployment_json = ?1,
                    updated_at = ?2 WHERE deployment_id = ?3 AND state = 'executing'",
                params![
                    serde_json::to_string(deployment)?,
                    deployment.updated_at.to_rfc3339(),
                    deployment.id.to_string(),
                ],
            )?;
        }
        transaction.commit()?;
        u64::try_from(deployments.len())
            .map_err(|_| StorageError::InvalidData("site recovery count overflow".into()))
    }

    /// Records one immutable, idempotent health observation.
    ///
    /// # Errors
    /// Returns an error for malformed observations, identity conflicts, or storage failure.
    pub fn record_site_health_check(
        &mut self,
        check: &SiteHealthCheck,
    ) -> Result<(SiteHealthCheck, bool), StorageError> {
        validate_health_check(check)?;
        if let Some(existing) = self.site_health_check(check.id)? {
            if existing.request_hash == check.request_hash {
                return Ok((existing, false));
            }
            return Err(StorageError::Conflict(format!(
                "site health check {} already exists with a different request",
                check.id
            )));
        }
        if self.site(check.site_id)?.is_none() {
            return Err(StorageError::SiteNotFound(check.site_id));
        }
        self.connection.execute(
            "INSERT INTO site_health_checks(check_id, site_id, request_hash, check_json, checked_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                check.id.to_string(),
                check.site_id.to_string(),
                check.request_hash,
                serde_json::to_string(check)?,
                check.checked_at.to_rfc3339(),
            ],
        )?;
        Ok((check.clone(), true))
    }

    /// Loads one health observation.
    ///
    /// # Errors
    /// Returns an error when `SQLite` or stored JSON is invalid.
    pub fn site_health_check(
        &self,
        check_id: SiteHealthCheckId,
    ) -> Result<Option<SiteHealthCheck>, StorageError> {
        self.read_optional_json(
            "SELECT check_json FROM site_health_checks WHERE check_id = ?1",
            &check_id.to_string(),
        )
    }

    /// Lists immutable health history for one site.
    ///
    /// # Errors
    /// Returns not-found for an unknown site or an error for invalid stored data.
    pub fn site_health_checks(
        &self,
        site_id: SiteId,
    ) -> Result<Vec<SiteHealthCheck>, StorageError> {
        if self.site(site_id)?.is_none() {
            return Err(StorageError::SiteNotFound(site_id));
        }
        let mut statement = self.connection.prepare(
            "SELECT check_json FROM site_health_checks
             WHERE site_id = ?1 ORDER BY checked_at, check_id",
        )?;
        let rows = statement.query_map([site_id.to_string()], |row| row.get::<_, String>(0))?;
        rows.map(|row| serde_json::from_str(&row?).map_err(StorageError::from))
            .collect()
    }

    /// Atomically binds one Site to one durable interval schedule.
    /// Exact retries replay the existing monitor definition.
    ///
    /// # Errors
    /// Returns an error for archived Sites, changed identity, or invalid schedules.
    pub fn create_site_monitor(
        &mut self,
        monitor: &SiteMonitor,
        schedule: &ScheduleDefinition,
    ) -> Result<(SiteMonitor, bool), StorageError> {
        validate_monitor(monitor, schedule)?;
        let site = self
            .site(monitor.site_id)?
            .ok_or(StorageError::SiteNotFound(monitor.site_id))?;
        if site.state == SiteState::Archived {
            return Err(StorageError::InvalidData(
                "archived Sites cannot be monitored".into(),
            ));
        }
        if let Some(existing) = self.site_monitor(monitor.site_id)? {
            if existing.schedule_id == monitor.schedule_id
                && existing.interval_seconds == monitor.interval_seconds
                && existing.expected_status == monitor.expected_status
            {
                return Ok((existing, false));
            }
            return Err(StorageError::Conflict(
                "Site monitor already exists with a different definition".into(),
            ));
        }
        let transaction = self.connection.transaction()?;
        Self::insert_schedule_tx(&transaction, schedule)?;
        transaction.execute(
            "INSERT INTO site_monitors(site_id, schedule_id, monitor_json, updated_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                monitor.site_id.to_string(),
                monitor.schedule_id.to_string(),
                serde_json::to_string(monitor)?,
                monitor.updated_at.to_rfc3339(),
            ],
        )?;
        transaction.commit()?;
        Ok((monitor.clone(), true))
    }

    /// Loads the monitor for one Site.
    ///
    /// # Errors
    /// Returns an error when the durable monitor cannot be decoded or read.
    pub fn site_monitor(&self, site_id: SiteId) -> Result<Option<SiteMonitor>, StorageError> {
        self.read_optional_json(
            "SELECT monitor_json FROM site_monitors WHERE site_id = ?1",
            &site_id.to_string(),
        )
    }

    /// Loads the Site monitor owning one scheduler definition.
    ///
    /// # Errors
    /// Returns an error when the durable monitor cannot be decoded or read.
    pub fn site_monitor_by_schedule(
        &self,
        schedule_id: ScheduleId,
    ) -> Result<Option<SiteMonitor>, StorageError> {
        self.read_optional_json(
            "SELECT monitor_json FROM site_monitors WHERE schedule_id = ?1",
            &schedule_id.to_string(),
        )
    }
}

fn validate_monitor(
    monitor: &SiteMonitor,
    schedule: &ScheduleDefinition,
) -> Result<(), StorageError> {
    if monitor.schema_version != SITE_SCHEMA_VERSION
        || monitor.schedule_id != schedule.id
        || monitor.interval_seconds < 5
        || monitor.interval_seconds > 2_592_000
        || !(100..=599).contains(&monitor.expected_status)
        || monitor.created_at > monitor.updated_at
        || !matches!(schedule.trigger, mcg_domain::ScheduleTrigger::Interval { seconds } if seconds == monitor.interval_seconds)
    {
        return Err(StorageError::InvalidData("invalid Site monitor".into()));
    }
    Ok(())
}

fn sync_monitor_schedule_state(
    transaction: &Transaction<'_>,
    site: &Site,
    now: DateTime<Utc>,
) -> Result<(), StorageError> {
    let schedule_json = transaction
        .query_row(
            "SELECT s.schedule_json FROM schedules s
             JOIN site_monitors m ON m.schedule_id = s.schedule_id WHERE m.site_id = ?1",
            [site.id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let Some(schedule_json) = schedule_json else {
        return Ok(());
    };
    let mut schedule: ScheduleDefinition = serde_json::from_str(&schedule_json)?;
    schedule.state = match site.state {
        SiteState::Active => ScheduleState::Enabled,
        SiteState::MonitoringPaused => ScheduleState::Paused,
        SiteState::Archived => ScheduleState::Completed,
    };
    if site.state == SiteState::Archived {
        schedule.next_run_at = None;
    }
    schedule.updated_at = now;
    transaction.execute(
        "UPDATE schedules SET state = ?1, next_run_at = ?2, schedule_json = ?3, updated_at = ?4
         WHERE schedule_id = ?5",
        params![
            enum_json(&schedule.state)?,
            schedule.next_run_at.map(|value| value.to_rfc3339()),
            serde_json::to_string(&schedule)?,
            now.to_rfc3339(),
            schedule.id.to_string(),
        ],
    )?;
    Ok(())
}

fn validate_site(site: &Site) -> Result<(), StorageError> {
    if site.schema_version != SITE_SCHEMA_VERSION
        || site.revision == 0
        || !valid_digest(&site.definition_hash)
        || site.display_name.trim().is_empty()
        || site.display_name.len() > 160
        || site.hosting_provider.trim().is_empty()
        || site.hosting_provider.len() > 128
        || site.created_at > site.updated_at
        || site
            .credential_reference
            .as_ref()
            .is_some_and(|reference| !valid_reference(reference))
    {
        return Err(StorageError::InvalidData("invalid site definition".into()));
    }
    let public = validated_url(&site.public_url)?;
    let health = validated_url(&site.health_url)?;
    if public.origin() != health.origin() {
        return Err(StorageError::InvalidData(
            "site health URL must share the public URL origin".into(),
        ));
    }
    if let Some(command) = &site.deployment_command
        && (command.executable.trim().is_empty()
            || command.executable.len() > 256
            || command.executable.contains(['/', '\\'])
            || command.arguments.len() > 64
            || command
                .arguments
                .iter()
                .any(|argument| argument.len() > 4_096 || argument.contains('\0'))
            || !valid_relative_directory(&command.working_directory))
    {
        return Err(StorageError::InvalidData(
            "invalid structured site deployment command".into(),
        ));
    }
    Ok(())
}

fn validate_revision(site: &Site, revision: &SiteRevision) -> Result<(), StorageError> {
    if revision.schema_version != SITE_SCHEMA_VERSION
        || revision.site_id != site.id
        || revision.revision != site.revision
        || revision.definition_hash != site.definition_hash
        || revision.site != *site
        || revision.reason.trim().is_empty()
        || revision.reason.len() > 512
        || revision.created_at != site.updated_at
    {
        return Err(StorageError::InvalidData("invalid site revision".into()));
    }
    Ok(())
}

fn validate_deployment_claim(deployment: &SiteDeployment) -> Result<(), StorageError> {
    if deployment.schema_version != SITE_SCHEMA_VERSION
        || deployment.site_revision == 0
        || !valid_digest(&deployment.request_hash)
        || deployment.state != SiteDeploymentState::Executing
        || !deployment.detail.is_empty()
        || !deployment.log_artifact_sha256.is_empty()
        || deployment.created_at != deployment.updated_at
    {
        return Err(StorageError::InvalidData(
            "invalid site deployment claim".into(),
        ));
    }
    Ok(())
}

fn validate_health_check(check: &SiteHealthCheck) -> Result<(), StorageError> {
    let status_matches = match check.state {
        SiteHealthState::Healthy => check
            .status_code
            .is_some_and(|status| (200..400).contains(&status)),
        SiteHealthState::Degraded => check.status_code.is_some(),
        SiteHealthState::Unavailable => check.status_code.is_none(),
    };
    if check.schema_version != SITE_SCHEMA_VERSION
        || !valid_digest(&check.request_hash)
        || !status_matches
        || check.detail.trim().is_empty()
        || check.detail.len() > 512
        || check
            .response_artifact_sha256
            .as_ref()
            .is_some_and(|digest| !valid_digest(digest))
    {
        return Err(StorageError::InvalidData(
            "invalid site health observation".into(),
        ));
    }
    Ok(())
}

fn validated_url(value: &str) -> Result<Url, StorageError> {
    let url = Url::parse(value)
        .map_err(|error| StorageError::InvalidData(format!("invalid site URL: {error}")))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(StorageError::InvalidData(
            "site URLs must be credential-free HTTP(S) URLs without query or fragment".into(),
        ));
    }
    Ok(url)
}

fn valid_relative_directory(value: &str) -> bool {
    value.is_empty()
        || (value.len() <= 1_024
            && !value.starts_with('/')
            && !value.contains('\\')
            && value
                .split('/')
                .all(|component| !component.is_empty() && !matches!(component, "." | "..")))
}

fn valid_reference(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._:-".contains(character))
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn insert_revision(
    transaction: &rusqlite::Transaction<'_>,
    revision: &SiteRevision,
) -> Result<(), StorageError> {
    transaction.execute(
        "INSERT INTO site_revisions(
            revision_id, site_id, revision, definition_hash, revision_json, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            revision.id.to_string(),
            revision.site_id.to_string(),
            revision.revision,
            revision.definition_hash,
            serde_json::to_string(revision)?,
            revision.created_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

fn enum_json<T: serde::Serialize>(value: &T) -> Result<String, StorageError> {
    Ok(serde_json::to_string(value)?.trim_matches('"').to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcg_domain::{
        ProjectId, RiskLevel, SCHEDULE_SCHEMA_VERSION, ScheduleTrigger, SiteDeploymentCommand,
        SiteEnvironment, SiteRevisionId, TaskBudget,
    };

    fn insert_project(store: &SqliteEventStore, project_id: ProjectId, now: DateTime<Utc>) {
        store
            .connection
            .execute(
                "INSERT INTO projects(project_id, root, current_snapshot_id, project_json, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    project_id.to_string(),
                    format!("/fixture/{project_id}"),
                    mcg_domain::ProjectSnapshotId::new().to_string(),
                    "{}",
                    now.to_rfc3339(),
                ],
            )
            .expect("insert project");
    }

    fn fixture_site(project_id: ProjectId, now: DateTime<Utc>) -> (Site, SiteRevision) {
        let site = Site {
            schema_version: SITE_SCHEMA_VERSION,
            id: SiteId::new(),
            definition_hash: "a".repeat(64),
            revision: 1,
            project_id,
            display_name: "Fixture site".into(),
            environment: SiteEnvironment::Production,
            public_url: "https://example.com/".into(),
            health_url: "https://example.com/health".into(),
            hosting_provider: "fixture".into(),
            deployment_command: Some(SiteDeploymentCommand {
                executable: "npm".into(),
                arguments: vec!["run".into(), "deploy".into()],
                working_directory: String::new(),
            }),
            credential_reference: Some("sites.fixture.production".into()),
            state: SiteState::Active,
            created_at: now,
            updated_at: now,
        };
        let revision = SiteRevision {
            schema_version: SITE_SCHEMA_VERSION,
            id: SiteRevisionId::new(),
            site_id: site.id,
            revision: 1,
            definition_hash: site.definition_hash.clone(),
            site: site.clone(),
            reason: "initial registration".into(),
            created_at: now,
        };
        (site, revision)
    }

    fn fixture_monitor(site: &Site, now: DateTime<Utc>) -> (SiteMonitor, ScheduleDefinition) {
        let schedule_id = ScheduleId::new();
        let monitor = SiteMonitor {
            schema_version: SITE_SCHEMA_VERSION,
            site_id: site.id,
            schedule_id,
            interval_seconds: 300,
            expected_status: 200,
            last_state: None,
            created_at: now,
            updated_at: now,
        };
        let schedule = ScheduleDefinition {
            schema_version: SCHEDULE_SCHEMA_VERSION,
            id: schedule_id,
            definition_hash: "f".repeat(64),
            name: format!("Monitor {}", site.display_name),
            project_id: site.project_id,
            goal: format!("Monitor Site {}", site.id),
            constraints: Vec::new(),
            success_conditions: Vec::new(),
            risk: RiskLevel::Low,
            budget: TaskBudget {
                max_tokens: None,
                max_cost_usd: None,
                max_duration_seconds: Some(60),
            },
            trigger: ScheduleTrigger::Interval { seconds: 300 },
            state: ScheduleState::Enabled,
            next_run_at: Some(now),
            target_path: None,
            context_token_budget: 1,
            requested_tools: vec!["http.request".into()],
            estimated_output_tokens: 0,
            max_model_calls: 1,
            watch_fingerprint: None,
            last_watch_at: None,
            last_event_sequence: 0,
            created_at: now,
            updated_at: now,
        };
        (monitor, schedule)
    }

    #[test]
    fn site_revisions_are_atomic_idempotent_and_immutable() {
        let now = Utc::now();
        let project_id = ProjectId::new();
        let mut store = SqliteEventStore::open_in_memory().expect("store");
        insert_project(&store, project_id, now);
        let (site, revision) = fixture_site(project_id, now);
        assert!(store.create_site(&site, &revision).expect("create").1);
        assert!(!store.create_site(&site, &revision).expect("replay").1);
        let (monitor, schedule) = fixture_monitor(&site, now);
        assert!(
            store
                .create_site_monitor(&monitor, &schedule)
                .expect("monitor")
                .1
        );

        let mut updated = site.clone();
        updated.revision = 2;
        updated.definition_hash = "b".repeat(64);
        updated.state = SiteState::MonitoringPaused;
        updated.updated_at = now + chrono::Duration::seconds(1);
        let updated_revision = SiteRevision {
            schema_version: SITE_SCHEMA_VERSION,
            id: SiteRevisionId::new(),
            site_id: updated.id,
            revision: 2,
            definition_hash: updated.definition_hash.clone(),
            site: updated.clone(),
            reason: "pause monitoring".into(),
            created_at: updated.updated_at,
        };
        assert_eq!(
            store
                .update_site(1, &updated, &updated_revision)
                .expect("update"),
            updated
        );
        assert_eq!(store.site_revisions(site.id).expect("history").len(), 2);
        assert_eq!(
            store
                .schedule(schedule.id)
                .expect("schedule")
                .expect("present")
                .state,
            ScheduleState::Paused
        );
        let mut archived = updated.clone();
        archived.revision = 3;
        archived.definition_hash = "c".repeat(64);
        archived.state = SiteState::Archived;
        archived.updated_at = now + chrono::Duration::seconds(2);
        let archived_revision = SiteRevision {
            schema_version: SITE_SCHEMA_VERSION,
            id: SiteRevisionId::new(),
            site_id: archived.id,
            revision: 3,
            definition_hash: archived.definition_hash.clone(),
            site: archived.clone(),
            reason: "archive".into(),
            created_at: archived.updated_at,
        };
        store
            .update_site(2, &archived, &archived_revision)
            .expect("archive");
        let archived_schedule = store
            .schedule(schedule.id)
            .expect("schedule")
            .expect("present");
        assert_eq!(archived_schedule.state, ScheduleState::Completed);
        assert!(archived_schedule.next_run_at.is_none());
        assert!(
            store
                .connection
                .execute("DELETE FROM site_revisions", [])
                .is_err()
        );
    }

    #[test]
    fn deployment_recovery_and_health_history_are_durable() {
        let now = Utc::now();
        let directory = tempfile::tempdir().expect("directory");
        let database = directory.path().join("sites.sqlite3");
        let project_id = ProjectId::new();
        let (site, revision) = fixture_site(project_id, now);
        let deployment = SiteDeployment {
            schema_version: SITE_SCHEMA_VERSION,
            id: SiteDeploymentId::new(),
            site_id: site.id,
            site_revision: 1,
            task_id: None,
            request_hash: "c".repeat(64),
            state: SiteDeploymentState::Executing,
            detail: String::new(),
            log_artifact_sha256: Vec::new(),
            created_at: now,
            updated_at: now,
        };
        let check = SiteHealthCheck {
            schema_version: SITE_SCHEMA_VERSION,
            id: SiteHealthCheckId::new(),
            site_id: site.id,
            request_hash: "d".repeat(64),
            state: SiteHealthState::Healthy,
            status_code: Some(204),
            latency_milliseconds: 12,
            detail: "health endpoint returned 204".into(),
            response_artifact_sha256: Some("e".repeat(64)),
            checked_at: now,
        };
        let (monitor, schedule) = fixture_monitor(&site, now);
        {
            let mut store = SqliteEventStore::open(&database).expect("store");
            insert_project(&store, project_id, now);
            store.create_site(&site, &revision).expect("site");
            store
                .create_site_monitor(&monitor, &schedule)
                .expect("monitor");
            assert!(store.claim_site_deployment(&deployment).expect("claim").1);
            assert!(store.record_site_health_check(&check).expect("health").1);
            assert!(
                !store
                    .record_site_health_check(&check)
                    .expect("health replay")
                    .1
            );
        }
        let mut reopened = SqliteEventStore::open(&database).expect("reopen");
        assert_eq!(
            reopened.site_monitor(site.id).expect("monitor"),
            Some(monitor)
        );
        assert!(reopened.schedule(schedule.id).expect("schedule").is_some());
        assert_eq!(
            reopened
                .recover_site_deployments(now + chrono::Duration::seconds(1))
                .expect("recover"),
            1
        );
        assert_eq!(
            reopened
                .site_deployment(deployment.id)
                .expect("deployment")
                .expect("present")
                .state,
            SiteDeploymentState::Uncertain
        );
        assert_eq!(
            reopened.site_health_checks(site.id).expect("health").len(),
            1
        );
        assert!(
            reopened
                .connection
                .execute("DELETE FROM site_health_checks", [])
                .is_err()
        );
    }

    #[test]
    fn unsafe_urls_commands_and_identity_reuse_fail_closed() {
        let now = Utc::now();
        let project_id = ProjectId::new();
        let mut store = SqliteEventStore::open_in_memory().expect("store");
        insert_project(&store, project_id, now);
        let (site, revision) = fixture_site(project_id, now);
        store.create_site(&site, &revision).expect("site");
        let mut conflict = site.clone();
        conflict.definition_hash = "f".repeat(64);
        assert!(matches!(
            store.create_site(&conflict, &revision),
            Err(StorageError::InvalidData(_) | StorageError::Conflict(_))
        ));
        let (mut unsafe_site, _) = fixture_site(project_id, now);
        unsafe_site.public_url = "https://user:secret@example.com/".into();
        unsafe_site.health_url = "https://other.example/health".into();
        unsafe_site.deployment_command = Some(SiteDeploymentCommand {
            executable: "../deploy".into(),
            arguments: Vec::new(),
            working_directory: "../outside".into(),
        });
        assert!(validate_site(&unsafe_site).is_err());
    }
}
