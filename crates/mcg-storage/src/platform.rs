use std::collections::BTreeSet;

use chrono::{DateTime, Duration, Utc};
use mcg_domain::{
    PLATFORM_SCHEMA_VERSION, PlatformCapabilitySnapshot, PlatformSession, PlatformSessionEvent,
    PlatformSessionEventId, PlatformSessionId, PlatformSessionOwner, PlatformSessionState,
};
use rusqlite::{OptionalExtension, Transaction, params};
use sha2::{Digest, Sha256};
use url::Url;

use crate::{SqliteEventStore, StorageError};

const MAX_SESSION_LIFETIME: Duration = Duration::hours(24);

impl SqliteEventStore {
    /// Records a privacy-safe immutable snapshot, deduplicating identical observations.
    ///
    /// # Errors
    /// Returns an error when the snapshot is invalid or cannot be serialized or stored.
    pub fn record_platform_capability_snapshot(
        &mut self,
        snapshot: &PlatformCapabilitySnapshot,
    ) -> Result<PlatformCapabilitySnapshot, StorageError> {
        validate_snapshot(snapshot)?;
        let fingerprint_input = serde_json::to_vec(&(
            snapshot.schema_version,
            &snapshot.os,
            &snapshot.architecture,
            &snapshot.capabilities,
        ))?;
        let fingerprint = format!("{:x}", Sha256::digest(fingerprint_input));
        if let Some(existing) = self.read_optional_json(
            "SELECT snapshot_json FROM platform_capability_snapshots WHERE fingerprint = ?1",
            &fingerprint,
        )? {
            return Ok(existing);
        }
        self.connection.execute(
            "INSERT INTO platform_capability_snapshots(
                snapshot_id, fingerprint, snapshot_json, observed_at
             ) VALUES (?1, ?2, ?3, ?4)",
            params![
                snapshot.id.to_string(),
                fingerprint,
                serde_json::to_string(snapshot)?,
                snapshot.observed_at.to_rfc3339(),
            ],
        )?;
        Ok(snapshot.clone())
    }

    /// Loads the newest immutable platform capability observation.
    ///
    /// # Errors
    /// Returns an error when `SQLite` cannot be read or stored JSON is invalid.
    pub fn latest_platform_capability_snapshot(
        &self,
    ) -> Result<Option<PlatformCapabilitySnapshot>, StorageError> {
        let json = self
            .connection
            .query_row(
                "SELECT snapshot_json FROM platform_capability_snapshots
                 ORDER BY observed_at DESC, snapshot_id DESC LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        json.map(|value| serde_json::from_str(&value).map_err(StorageError::from))
            .transpose()
    }

    /// Creates a requested session atomically. Exact caller-ID replay is idempotent.
    /// The returned boolean is true only when this call committed the new session.
    ///
    /// # Errors
    /// Returns an error for invalid input, missing references, identity conflict, or storage failure.
    pub fn create_platform_session(
        &mut self,
        session: &PlatformSession,
        reason: &str,
    ) -> Result<(PlatformSession, bool), StorageError> {
        validate_new_session(session)?;
        validate_reason(reason)?;
        if let Some(existing) = self.platform_session(session.id)? {
            if existing.definition_hash == session.definition_hash {
                return Ok((existing, false));
            }
            return Err(StorageError::Conflict(format!(
                "platform session {} already exists with a different definition",
                session.id
            )));
        }
        validate_references(&self.connection, session)?;
        let event = PlatformSessionEvent {
            schema_version: PLATFORM_SCHEMA_VERSION,
            id: PlatformSessionEventId::new(),
            session_id: session.id,
            from_state: None,
            to_state: PlatformSessionState::Requested,
            owner: PlatformSessionOwner::Mcg,
            reason: reason.trim().to_owned(),
            occurred_at: session.created_at,
        };
        let transaction = self.connection.transaction()?;
        insert_session(&transaction, session)?;
        insert_session_event(&transaction, &event)?;
        transaction.commit()?;
        Ok((session.clone(), true))
    }

    /// Loads one current platform session projection.
    ///
    /// # Errors
    /// Returns an error when `SQLite` cannot be read or stored JSON is invalid.
    pub fn platform_session(
        &self,
        session_id: PlatformSessionId,
    ) -> Result<Option<PlatformSession>, StorageError> {
        self.read_optional_json(
            "SELECT session_json FROM platform_sessions WHERE session_id = ?1",
            &session_id.to_string(),
        )
    }

    /// Lists current platform session projections in stable recency order.
    ///
    /// # Errors
    /// Returns an error when `SQLite` cannot be read or stored JSON is invalid.
    pub fn platform_sessions(&self) -> Result<Vec<PlatformSession>, StorageError> {
        self.read_all_json(
            "SELECT session_json FROM platform_sessions ORDER BY created_at DESC, session_id",
        )
    }

    /// Loads immutable lifecycle history for one platform session.
    ///
    /// # Errors
    /// Returns not-found for an unknown session or an error for invalid storage data.
    pub fn platform_session_history(
        &self,
        session_id: PlatformSessionId,
    ) -> Result<Vec<PlatformSessionEvent>, StorageError> {
        if self.platform_session(session_id)?.is_none() {
            return Err(StorageError::PlatformSessionNotFound(session_id));
        }
        let mut statement = self.connection.prepare(
            "SELECT event_json FROM platform_session_events
             WHERE session_id = ?1 ORDER BY sequence",
        )?;
        let rows = statement.query_map([session_id.to_string()], |row| row.get::<_, String>(0))?;
        rows.map(|row| serde_json::from_str(&row?).map_err(StorageError::from))
            .collect()
    }

    /// Applies one legal lifecycle transition and appends immutable history atomically.
    ///
    /// # Errors
    /// Returns an error for an unknown session, illegal transition, invalid reason, or storage failure.
    pub fn transition_platform_session(
        &mut self,
        session_id: PlatformSessionId,
        next: PlatformSessionState,
        reason: &str,
        now: DateTime<Utc>,
    ) -> Result<(PlatformSession, Option<PlatformSessionEvent>), StorageError> {
        validate_reason(reason)?;
        let mut session = self
            .platform_session(session_id)?
            .ok_or(StorageError::PlatformSessionNotFound(session_id))?;
        if session.state == next {
            return Ok((session, None));
        }
        if !session.state.can_transition_to(next) {
            return Err(StorageError::InvalidData(format!(
                "platform session cannot transition from {:?} to {:?}",
                session.state, next
            )));
        }
        if now < session.updated_at {
            return Err(StorageError::InvalidData(
                "platform session transition time precedes its current state".into(),
            ));
        }
        let previous = session.state;
        session.state = next;
        session.owner = match next {
            PlatformSessionState::Active => PlatformSessionOwner::Mcg,
            PlatformSessionState::HandedOff => PlatformSessionOwner::LocalOwner,
            _ => session.owner,
        };
        session.updated_at = now;
        if matches!(
            next,
            PlatformSessionState::Closed | PlatformSessionState::Failed
        ) {
            session.closed_at = Some(now);
        }
        let event = PlatformSessionEvent {
            schema_version: PLATFORM_SCHEMA_VERSION,
            id: PlatformSessionEventId::new(),
            session_id,
            from_state: Some(previous),
            to_state: next,
            owner: session.owner,
            reason: reason.trim().to_owned(),
            occurred_at: now,
        };
        let transaction = self.connection.transaction()?;
        update_session(&transaction, &session)?;
        insert_session_event(&transaction, &event)?;
        transaction.commit()?;
        Ok((session, Some(event)))
    }

    /// Interrupts every live authority record after process restart.
    ///
    /// # Errors
    /// Returns an error when live sessions cannot be decoded or atomically updated.
    pub fn recover_platform_sessions(&mut self, now: DateTime<Utc>) -> Result<u64, StorageError> {
        let sessions: Vec<PlatformSession> = self
            .platform_sessions()?
            .into_iter()
            .filter(|session| session.state.is_live())
            .collect();
        let transaction = self.connection.transaction()?;
        for mut session in sessions.iter().cloned() {
            let previous = session.state;
            session.state = PlatformSessionState::Interrupted;
            session.updated_at = now.max(session.updated_at);
            session.recovery_count = session.recovery_count.saturating_add(1);
            update_session(&transaction, &session)?;
            insert_session_event(
                &transaction,
                &PlatformSessionEvent {
                    schema_version: PLATFORM_SCHEMA_VERSION,
                    id: PlatformSessionEventId::new(),
                    session_id: session.id,
                    from_state: Some(previous),
                    to_state: PlatformSessionState::Interrupted,
                    owner: session.owner,
                    reason: "daemon restart interrupted live session authority".into(),
                    occurred_at: session.updated_at,
                },
            )?;
        }
        transaction.commit()?;
        u64::try_from(sessions.len())
            .map_err(|_| StorageError::InvalidData("platform recovery count overflow".into()))
    }

    /// Closes every live reservation whose bounded lifetime has elapsed.
    ///
    /// # Errors
    /// Returns an error when expired sessions cannot be decoded or atomically updated.
    pub fn expire_platform_sessions(&mut self, now: DateTime<Utc>) -> Result<u64, StorageError> {
        let expired = self.expire_platform_session_ids(now)?;
        u64::try_from(expired.len())
            .map_err(|_| StorageError::InvalidData("platform expiration count overflow".into()))
    }

    /// Closes expired live reservations and returns their identities for executor cleanup.
    ///
    /// # Errors
    /// Returns an error when expired sessions cannot be decoded or atomically updated.
    pub fn expire_platform_session_ids(
        &mut self,
        now: DateTime<Utc>,
    ) -> Result<Vec<PlatformSessionId>, StorageError> {
        let sessions: Vec<PlatformSession> = self
            .platform_sessions()?
            .into_iter()
            .filter(|session| session.state.is_live() && session.expires_at <= now)
            .collect();
        let transaction = self.connection.transaction()?;
        for mut session in sessions.iter().cloned() {
            let previous = session.state;
            session.state = PlatformSessionState::Closed;
            session.updated_at = now.max(session.updated_at);
            session.closed_at = Some(session.updated_at);
            update_session(&transaction, &session)?;
            insert_session_event(
                &transaction,
                &PlatformSessionEvent {
                    schema_version: PLATFORM_SCHEMA_VERSION,
                    id: PlatformSessionEventId::new(),
                    session_id: session.id,
                    from_state: Some(previous),
                    to_state: PlatformSessionState::Closed,
                    owner: session.owner,
                    reason: "bounded platform session lifetime expired".into(),
                    occurred_at: session.updated_at,
                },
            )?;
        }
        transaction.commit()?;
        Ok(sessions.into_iter().map(|session| session.id).collect())
    }
}

fn validate_snapshot(snapshot: &PlatformCapabilitySnapshot) -> Result<(), StorageError> {
    if snapshot.schema_version != PLATFORM_SCHEMA_VERSION
        || snapshot.os.trim().is_empty()
        || snapshot.os.len() > 64
        || snapshot.architecture.trim().is_empty()
        || snapshot.architecture.len() > 64
        || snapshot.capabilities.is_empty()
        || snapshot.capabilities.len() > 16
    {
        return Err(StorageError::InvalidData(
            "invalid platform capability snapshot".into(),
        ));
    }
    let mut kinds = BTreeSet::new();
    for capability in &snapshot.capabilities {
        if !kinds.insert(capability.kind)
            || capability.detail.trim().is_empty()
            || capability.detail.len() > 512
        {
            return Err(StorageError::InvalidData(
                "platform capabilities must be unique and have bounded detail".into(),
            ));
        }
    }
    Ok(())
}

fn validate_new_session(session: &PlatformSession) -> Result<(), StorageError> {
    let valid_hash = session.definition_hash.len() == 64
        && session
            .definition_hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if session.schema_version != PLATFORM_SCHEMA_VERSION
        || session.state != PlatformSessionState::Requested
        || session.owner != PlatformSessionOwner::Mcg
        || session.display_name.trim().is_empty()
        || session.display_name.len() > 256
        || !valid_hash
        || session.created_at != session.updated_at
        || session.closed_at.is_some()
        || session.recovery_count != 0
        || session.expires_at <= session.created_at
        || session.expires_at - session.created_at > MAX_SESSION_LIFETIME
        || session.allowed_origins.len() > 32
    {
        return Err(StorageError::InvalidData(
            "invalid requested platform session".into(),
        ));
    }
    let mut origins = BTreeSet::new();
    for origin in &session.allowed_origins {
        let canonical = validate_origin(origin)?;
        if !origins.insert(canonical) {
            return Err(StorageError::InvalidData(
                "platform session origins must be unique".into(),
            ));
        }
    }
    Ok(())
}

fn validate_origin(origin: &str) -> Result<String, StorageError> {
    if origin.len() > 2_048 {
        return Err(StorageError::InvalidData(
            "platform session origin is too long".into(),
        ));
    }
    let parsed = Url::parse(origin)
        .map_err(|_| StorageError::InvalidData("platform session origin is invalid".into()))?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || !matches!(parsed.path(), "" | "/")
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(StorageError::InvalidData(
            "platform session origins must be credential-free HTTP(S) origins".into(),
        ));
    }
    Ok(parsed.origin().ascii_serialization())
}

fn validate_reason(reason: &str) -> Result<(), StorageError> {
    if reason.trim().is_empty() || reason.len() > 2_048 {
        return Err(StorageError::InvalidData(
            "platform session reason must contain 1 to 2048 bytes".into(),
        ));
    }
    Ok(())
}

fn validate_references(
    connection: &rusqlite::Connection,
    session: &PlatformSession,
) -> Result<(), StorageError> {
    if let Some(project_id) = session.project_id {
        let exists = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE project_id = ?1)",
            [project_id.to_string()],
            |row| row.get::<_, bool>(0),
        )?;
        if !exists {
            return Err(StorageError::ProjectNotFound(project_id));
        }
    }
    if let Some(task_id) = session.task_id {
        let task_json = connection
            .query_row(
                "SELECT task_json FROM tasks WHERE task_id = ?1",
                [task_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or(StorageError::TaskNotFound(task_id))?;
        let task: mcg_domain::TaskEnvelope = serde_json::from_str(&task_json)?;
        if let Some(project_id) = session.project_id
            && task.project_id != Some(project_id)
        {
            return Err(StorageError::InvalidData(
                "platform session task does not belong to the requested project".into(),
            ));
        }
    }
    Ok(())
}

fn insert_session(
    transaction: &Transaction<'_>,
    session: &PlatformSession,
) -> Result<(), StorageError> {
    transaction.execute(
        "INSERT INTO platform_sessions(
            session_id, kind, state, owner, definition_hash, session_json,
            created_at, updated_at, expires_at, closed_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            session.id.to_string(),
            enum_json(&session.kind)?,
            enum_json(&session.state)?,
            enum_json(&session.owner)?,
            session.definition_hash,
            serde_json::to_string(session)?,
            session.created_at.to_rfc3339(),
            session.updated_at.to_rfc3339(),
            session.expires_at.to_rfc3339(),
            session.closed_at.map(|value| value.to_rfc3339()),
        ],
    )?;
    Ok(())
}

fn update_session(
    transaction: &Transaction<'_>,
    session: &PlatformSession,
) -> Result<(), StorageError> {
    transaction.execute(
        "UPDATE platform_sessions SET state = ?1, owner = ?2, session_json = ?3,
            updated_at = ?4, closed_at = ?5 WHERE session_id = ?6",
        params![
            enum_json(&session.state)?,
            enum_json(&session.owner)?,
            serde_json::to_string(session)?,
            session.updated_at.to_rfc3339(),
            session.closed_at.map(|value| value.to_rfc3339()),
            session.id.to_string(),
        ],
    )?;
    Ok(())
}

fn insert_session_event(
    transaction: &Transaction<'_>,
    event: &PlatformSessionEvent,
) -> Result<(), StorageError> {
    transaction.execute(
        "INSERT INTO platform_session_events(
            event_id, session_id, from_state, to_state, event_json, occurred_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            event.id.to_string(),
            event.session_id.to_string(),
            event
                .from_state
                .map(|state| enum_json(&state))
                .transpose()?,
            enum_json(&event.to_state)?,
            serde_json::to_string(event)?,
            event.occurred_at.to_rfc3339(),
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
        PlatformCapability, PlatformCapabilityAvailability, PlatformCapabilityKind,
        PlatformCapabilitySnapshotId, PlatformSessionKind,
    };

    fn session(now: DateTime<Utc>) -> PlatformSession {
        PlatformSession {
            schema_version: PLATFORM_SCHEMA_VERSION,
            id: PlatformSessionId::new(),
            definition_hash: "a".repeat(64),
            kind: PlatformSessionKind::Browser,
            state: PlatformSessionState::Requested,
            owner: PlatformSessionOwner::Mcg,
            project_id: None,
            task_id: None,
            display_name: "Research session".into(),
            allowed_origins: vec!["https://example.com".into()],
            created_at: now,
            updated_at: now,
            expires_at: now + Duration::hours(1),
            closed_at: None,
            recovery_count: 0,
        }
    }

    #[test]
    fn snapshots_deduplicate_and_sessions_recover_with_immutable_history() {
        let temporary = tempfile::tempdir().expect("tempdir");
        let database = temporary.path().join("platform.sqlite3");
        let now = Utc::now();
        let mut store = SqliteEventStore::open(&database).expect("store");
        let snapshot = PlatformCapabilitySnapshot {
            schema_version: PLATFORM_SCHEMA_VERSION,
            id: PlatformCapabilitySnapshotId::new(),
            os: "windows".into(),
            architecture: "x86_64".into(),
            capabilities: vec![PlatformCapability {
                kind: PlatformCapabilityKind::BrowserAutomation,
                availability: PlatformCapabilityAvailability::Available,
                detail: "verified configured executable".into(),
            }],
            observed_at: now,
        };
        assert_eq!(
            store
                .record_platform_capability_snapshot(&snapshot)
                .expect("record"),
            snapshot
        );
        let mut replay = snapshot.clone();
        replay.id = PlatformCapabilitySnapshotId::new();
        replay.observed_at = now + Duration::seconds(1);
        assert_eq!(
            store
                .record_platform_capability_snapshot(&replay)
                .expect("dedupe")
                .id,
            snapshot.id
        );

        let requested = session(now);
        assert!(
            store
                .create_platform_session(&requested, "owner requested session")
                .expect("create")
                .1
        );
        assert!(
            !store
                .create_platform_session(&requested, "retry")
                .expect("replay")
                .1
        );
        store
            .transition_platform_session(
                requested.id,
                PlatformSessionState::Active,
                "executor attached",
                now + Duration::seconds(1),
            )
            .expect("activate");
        drop(store);

        let mut reopened = SqliteEventStore::open(&database).expect("reopen");
        assert_eq!(
            reopened
                .recover_platform_sessions(now + Duration::seconds(2))
                .expect("recover"),
            1
        );
        let recovered = reopened
            .platform_session(requested.id)
            .expect("load")
            .expect("present");
        assert_eq!(recovered.state, PlatformSessionState::Interrupted);
        assert_eq!(recovered.recovery_count, 1);
        let history = reopened
            .platform_session_history(requested.id)
            .expect("history");
        assert_eq!(
            history
                .iter()
                .map(|event| event.to_state)
                .collect::<Vec<_>>(),
            vec![
                PlatformSessionState::Requested,
                PlatformSessionState::Active,
                PlatformSessionState::Interrupted,
            ]
        );
        assert!(
            reopened
                .connection
                .execute(
                    "DELETE FROM platform_session_events WHERE session_id = ?1",
                    [requested.id.to_string()],
                )
                .is_err()
        );
    }

    #[test]
    fn session_identity_conflicts_and_unsafe_origins_are_rejected() {
        let now = Utc::now();
        let mut store = SqliteEventStore::open_in_memory().expect("store");
        let requested = session(now);
        store
            .create_platform_session(&requested, "request")
            .expect("create");
        let mut conflict = requested.clone();
        conflict.definition_hash = "b".repeat(64);
        assert!(matches!(
            store.create_platform_session(&conflict, "request"),
            Err(StorageError::Conflict(_))
        ));
        let mut unsafe_session = session(now);
        unsafe_session.allowed_origins = vec!["https://user:secret@example.com/path".into()];
        assert!(matches!(
            store.create_platform_session(&unsafe_session, "request"),
            Err(StorageError::InvalidData(_))
        ));
        let mut duplicate_origin = session(now);
        duplicate_origin.allowed_origins = vec![
            "https://example.com".into(),
            "https://example.com:443".into(),
        ];
        assert!(matches!(
            store.create_platform_session(&duplicate_origin, "request"),
            Err(StorageError::InvalidData(_))
        ));
    }

    #[test]
    fn expired_live_session_closes_once_with_retained_reason() {
        let now = Utc::now();
        let mut store = SqliteEventStore::open_in_memory().expect("store");
        let requested = session(now);
        store
            .create_platform_session(&requested, "request")
            .expect("create");
        assert_eq!(
            store
                .expire_platform_sessions(now + Duration::hours(2))
                .expect("expire"),
            1
        );
        assert_eq!(
            store
                .expire_platform_sessions(now + Duration::hours(3))
                .expect("idempotent"),
            0
        );
        let stored = store
            .platform_session(requested.id)
            .expect("load")
            .expect("present");
        assert_eq!(stored.state, PlatformSessionState::Closed);
        let history = store
            .platform_session_history(requested.id)
            .expect("history");
        assert_eq!(history.len(), 2);
        assert_eq!(
            history[1].reason,
            "bounded platform session lifetime expired"
        );
    }
}
