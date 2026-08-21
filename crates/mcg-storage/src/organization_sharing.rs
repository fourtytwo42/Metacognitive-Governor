use mcg_agents::{
    OrganizationGenome, OrganizationPublisherState, OrganizationShareReview,
    OrganizationShareReviewState, TrustedOrganizationPublisher,
};
use mcg_domain::OrganizationShareReviewId;
use mcg_events::{EventEnvelope, EventKind};
use rusqlite::{OptionalExtension, params};

use crate::{SqliteEventStore, StorageError};

impl SqliteEventStore {
    /// Loads one durable publisher trust head.
    ///
    /// # Errors
    /// Returns an error when persisted JSON is invalid.
    pub fn organization_publisher(
        &self,
        publisher_id: &str,
    ) -> Result<Option<TrustedOrganizationPublisher>, StorageError> {
        self.connection
            .query_row(
                "SELECT publisher_json FROM organization_publishers WHERE publisher_id = ?1",
                [publisher_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
            .transpose()
    }

    /// Lists durable publisher trust heads in stable identity order.
    ///
    /// # Errors
    /// Returns an error when a row cannot be decoded.
    pub fn organization_publishers(
        &self,
    ) -> Result<Vec<TrustedOrganizationPublisher>, StorageError> {
        let mut statement = self
            .connection
            .prepare("SELECT publisher_json FROM organization_publishers ORDER BY publisher_id")?;
        statement
            .query_map([], |row| row.get::<_, String>(0))?
            .map(|row| {
                row.map_err(StorageError::from)
                    .and_then(|json| serde_json::from_str(&json).map_err(StorageError::from))
            })
            .collect()
    }

    /// Atomically records the first publisher trust revision or one monotonic
    /// successor and appends audit evidence.
    ///
    /// # Errors
    /// Rejects malformed, stale, key-changing, or post-revocation updates.
    pub fn record_organization_publisher(
        &mut self,
        publisher: &TrustedOrganizationPublisher,
        expected_revision: Option<u64>,
    ) -> Result<EventEnvelope, StorageError> {
        publisher
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let current = self.organization_publisher(&publisher.id)?;
        match current {
            None if expected_revision.is_some() || publisher.revision != 1 => {
                return Err(StorageError::Conflict(
                    "first organization publisher revision must be revision 1".into(),
                ));
            }
            Some(ref current)
                if expected_revision != Some(current.revision)
                    || publisher.revision != current.revision.saturating_add(1)
                    || publisher.public_key_base64 != current.public_key_base64
                    || publisher.created_at != current.created_at
                    || current.state == OrganizationPublisherState::Revoked =>
            {
                return Err(StorageError::Conflict(
                    "organization publisher trust revision is stale or changes immutable identity"
                        .into(),
                ));
            }
            _ => {}
        }
        let event = EventEnvelope::new(
            EventKind::OrganizationPublisherRecorded,
            None,
            serde_json::to_value(publisher)?,
        );
        let transaction = self.connection.transaction()?;
        match current {
            None => {
                transaction.execute(
                    "INSERT INTO organization_publishers(
                        publisher_id, revision, state, publisher_json, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        publisher.id,
                        i64::try_from(publisher.revision).map_err(|_| {
                            StorageError::InvalidData(
                                "publisher revision exceeds SQLite range".into(),
                            )
                        })?,
                        publisher_state_name(publisher.state),
                        serde_json::to_string(publisher)?,
                        publisher.created_at.to_rfc3339(),
                        publisher.updated_at.to_rfc3339(),
                    ],
                )?;
            }
            Some(current) => {
                let changed = transaction.execute(
                    "UPDATE organization_publishers SET
                        revision = ?1, state = ?2, publisher_json = ?3, updated_at = ?4
                     WHERE publisher_id = ?5 AND revision = ?6",
                    params![
                        i64::try_from(publisher.revision).map_err(|_| {
                            StorageError::InvalidData(
                                "publisher revision exceeds SQLite range".into(),
                            )
                        })?,
                        publisher_state_name(publisher.state),
                        serde_json::to_string(publisher)?,
                        publisher.updated_at.to_rfc3339(),
                        publisher.id,
                        i64::try_from(current.revision).map_err(|_| {
                            StorageError::InvalidData(
                                "publisher revision exceeds SQLite range".into(),
                            )
                        })?,
                    ],
                )?;
                if changed != 1 {
                    return Err(StorageError::Conflict(
                        "organization publisher changed concurrently".into(),
                    ));
                }
            }
        }
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    /// Loads one durable organization-share review.
    ///
    /// # Errors
    /// Returns an error when persisted JSON is invalid.
    pub fn organization_share_review(
        &self,
        review_id: OrganizationShareReviewId,
    ) -> Result<Option<OrganizationShareReview>, StorageError> {
        self.connection
            .query_row(
                "SELECT review_json FROM organization_share_reviews WHERE review_id = ?1",
                [review_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
            .transpose()
    }

    /// Lists durable organization-share reviews newest first.
    ///
    /// # Errors
    /// Returns an error when a row cannot be decoded.
    pub fn organization_share_reviews(&self) -> Result<Vec<OrganizationShareReview>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT review_json FROM organization_share_reviews
             ORDER BY created_at DESC, review_id",
        )?;
        statement
            .query_map([], |row| row.get::<_, String>(0))?
            .map(|row| {
                row.map_err(StorageError::from)
                    .and_then(|json| serde_json::from_str(&json).map_err(StorageError::from))
            })
            .collect()
    }

    /// Atomically persists a verified package as a pending, non-authoritative
    /// owner review and appends audit evidence.
    ///
    /// # Errors
    /// Rejects malformed/non-pending reviews, unknown publishers, or duplicate IDs.
    pub fn create_organization_share_review(
        &mut self,
        review: &OrganizationShareReview,
    ) -> Result<EventEnvelope, StorageError> {
        review
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        if review.state != OrganizationShareReviewState::Pending
            || self
                .organization_publisher(&review.evidence.publisher)?
                .is_none()
        {
            return Err(StorageError::Conflict(
                "organization share review is not pending or its publisher is unknown".into(),
            ));
        }
        let event = EventEnvelope::new(
            EventKind::OrganizationShareReviewed,
            None,
            serde_json::to_value(review)?,
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO organization_share_reviews(
                review_id, package_sha256, publisher_id, state, import_id,
                organization_id, review_json, created_at, updated_at
             ) VALUES (?1, ?2, ?3, 'pending', NULL, NULL, ?4, ?5, ?6)",
            params![
                review.id.to_string(),
                review.evidence.package_sha256,
                review.evidence.publisher,
                serde_json::to_string(review)?,
                review.created_at.to_rfc3339(),
                review.updated_at.to_rfc3339(),
            ],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    /// Atomically rejects one pending review and appends immutable evidence.
    ///
    /// # Errors
    /// Rejects stale or malformed review transitions.
    pub fn reject_organization_share_review(
        &mut self,
        review: &OrganizationShareReview,
    ) -> Result<EventEnvelope, StorageError> {
        review
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        if review.state != OrganizationShareReviewState::Rejected {
            return Err(StorageError::InvalidData(
                "organization share review is not rejected".into(),
            ));
        }
        let event = EventEnvelope::new(
            EventKind::OrganizationShareRejected,
            None,
            serde_json::to_value(review)?,
        );
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE organization_share_reviews SET
                state = 'rejected', review_json = ?1, updated_at = ?2
             WHERE review_id = ?3 AND state = 'pending'",
            params![
                serde_json::to_string(review)?,
                review.updated_at.to_rfc3339(),
                review.id.to_string(),
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::Conflict(
                "organization share review is no longer pending".into(),
            ));
        }
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    /// Atomically materializes a newly reviewed root organization, resolves the
    /// pending review, and appends both audit events.
    ///
    /// # Errors
    /// Rejects stale reviews, invalid genomes, occupied identities, or partial writes.
    pub fn import_organization_share_review(
        &mut self,
        review: &OrganizationShareReview,
        genome: &OrganizationGenome,
    ) -> Result<Vec<EventEnvelope>, StorageError> {
        review
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        genome
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        if review.state != OrganizationShareReviewState::Imported
            || review.organization_id != Some(genome.organization_id)
            || genome.revision != 1
            || genome.parent_revision_id.is_some()
            || self.organization(genome.organization_id)?.is_some()
        {
            return Err(StorageError::Conflict(
                "organization share import identity or state is stale".into(),
            ));
        }
        let organization_event = EventEnvelope::new(
            EventKind::OrganizationRevisionRecorded,
            None,
            serde_json::to_value(genome)?,
        );
        let review_event = EventEnvelope::new(
            EventKind::OrganizationShareImported,
            None,
            serde_json::to_value(review)?,
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO organization_revisions(
                revision_id, organization_id, revision, parent_revision_id,
                organization_json, created_at
             ) VALUES (?1, ?2, 1, NULL, ?3, ?4)",
            params![
                genome.revision_id.to_string(),
                genome.organization_id.to_string(),
                serde_json::to_string(genome)?,
                genome.created_at.to_rfc3339(),
            ],
        )?;
        transaction.execute(
            "INSERT INTO organizations(
                organization_id, current_revision_id, organization_json, updated_at
             ) VALUES (?1, ?2, ?3, ?4)",
            params![
                genome.organization_id.to_string(),
                genome.revision_id.to_string(),
                serde_json::to_string(genome)?,
                genome.created_at.to_rfc3339(),
            ],
        )?;
        let changed = transaction.execute(
            "UPDATE organization_share_reviews SET
                state = 'imported', import_id = ?1, organization_id = ?2,
                review_json = ?3, updated_at = ?4
             WHERE review_id = ?5 AND state = 'pending'",
            params![
                review.import_id.map(|id| id.to_string()),
                genome.organization_id.to_string(),
                serde_json::to_string(review)?,
                review.updated_at.to_rfc3339(),
                review.id.to_string(),
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::Conflict(
                "organization share review is no longer pending".into(),
            ));
        }
        let events = vec![
            Self::insert_event(&transaction, &organization_event)?,
            Self::insert_event(&transaction, &review_event)?,
        ];
        transaction.commit()?;
        Ok(events)
    }
}

const fn publisher_state_name(state: OrganizationPublisherState) -> &'static str {
    match state {
        OrganizationPublisherState::Trusted => "trusted",
        OrganizationPublisherState::Revoked => "revoked",
    }
}

#[cfg(test)]
mod tests {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use chrono::Utc;
    use ed25519_dalek::{Signer as _, SigningKey};
    use mcg_agents::{
        ORGANIZATION_PUBLISHER_SCHEMA_VERSION, ORGANIZATION_SHARE_SCHEMA_VERSION,
        ORGANIZATION_SHARE_SIGNATURE_SCHEME, OrganizationGenome, OrganizationSharePackage,
        OrganizationShareSignature, OrganizationShareTrustStore, PortableOrganizationGenome,
        organization_share_signing_payload,
    };
    use mcg_domain::{OrganizationImportId, OrganizationShareReviewId};

    use super::*;

    fn publisher(key: &SigningKey) -> TrustedOrganizationPublisher {
        let now = Utc::now();
        TrustedOrganizationPublisher {
            schema_version: ORGANIZATION_PUBLISHER_SCHEMA_VERSION,
            id: "publisher.example".into(),
            display_name: "Example Publisher".into(),
            public_key_base64: STANDARD.encode(key.verifying_key().to_bytes()),
            registries: vec!["https://registry.example/".into()],
            state: OrganizationPublisherState::Trusted,
            revision: 1,
            reason: "Owner reviewed the test publisher.".into(),
            created_at: now,
            updated_at: now,
        }
    }

    fn package(key: &SigningKey) -> OrganizationSharePackage {
        let portable = PortableOrganizationGenome::export(
            &OrganizationGenome::consumer_default(Utc::now()),
            Utc::now(),
        )
        .expect("portable organization");
        let mut package = OrganizationSharePackage {
            schema_version: ORGANIZATION_SHARE_SCHEMA_VERSION,
            registry: "https://registry.example/".into(),
            package: "consumer-default".into(),
            publisher: "publisher.example".into(),
            portable,
            signature: OrganizationShareSignature {
                scheme: ORGANIZATION_SHARE_SIGNATURE_SCHEME.into(),
                signature_base64: STANDARD.encode([0_u8; 64]),
            },
        };
        let payload = organization_share_signing_payload(&package).expect("signing payload");
        package.signature.signature_base64 = STANDARD.encode(key.sign(&payload).to_bytes());
        package
    }

    #[test]
    fn publisher_review_and_import_survive_reopen_atomically() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("sharing.sqlite3");
        let key = SigningKey::from_bytes(&[47; 32]);
        let publisher = publisher(&key);
        let package = package(&key);
        let review_id = OrganizationShareReviewId::new();
        let import_id = OrganizationImportId::new();
        let organization_id;
        {
            let mut store = SqliteEventStore::open(&path).expect("open store");
            store
                .record_organization_publisher(&publisher, None)
                .expect("record publisher");
            let verified = OrganizationShareTrustStore::new([publisher.clone()])
                .expect("trust store")
                .verify(&package)
                .expect("verify package");
            let pending = OrganizationShareReview::pending(review_id, &verified, Utc::now());
            store
                .create_organization_share_review(&pending)
                .expect("create review");
            let genome = package
                .portable
                .materialize_for_import(import_id, None, Utc::now())
                .expect("materialize import");
            organization_id = genome.organization_id;
            let imported = pending
                .imported(
                    import_id,
                    organization_id,
                    "Owner reviewed the signed package.",
                    Utc::now(),
                )
                .expect("imported review");
            let events = store
                .import_organization_share_review(&imported, &genome)
                .expect("atomic import");
            assert_eq!(events.len(), 2);
        }
        let mut reopened = SqliteEventStore::open(&path).expect("reopen store");
        let review = reopened
            .organization_share_review(review_id)
            .expect("read review")
            .expect("stored review");
        assert_eq!(review.state, OrganizationShareReviewState::Imported);
        assert_eq!(review.organization_id, Some(organization_id));
        assert!(
            reopened
                .organization(organization_id)
                .expect("read organization")
                .is_some()
        );
        assert!(
            review
                .rejected("Cannot reject terminal review.", Utc::now())
                .is_err()
        );
        assert!(reopened.reject_organization_share_review(&review).is_err());
    }

    #[test]
    fn failed_review_resolution_rolls_back_organization_insert() {
        let mut store = SqliteEventStore::open_in_memory().expect("store");
        let key = SigningKey::from_bytes(&[53; 32]);
        let publisher = publisher(&key);
        store
            .record_organization_publisher(&publisher, None)
            .expect("publisher");
        let package = package(&key);
        let verified = OrganizationShareTrustStore::new([publisher])
            .expect("trust store")
            .verify(&package)
            .expect("verified package");
        let pending = OrganizationShareReview::pending(
            OrganizationShareReviewId::new(),
            &verified,
            Utc::now(),
        );
        store
            .create_organization_share_review(&pending)
            .expect("pending review");
        let import_id = OrganizationImportId::new();
        let genome = package
            .portable
            .materialize_for_import(import_id, None, Utc::now())
            .expect("genome");
        let wrong_review = OrganizationShareReview {
            id: OrganizationShareReviewId::new(),
            ..pending
                .imported(
                    import_id,
                    genome.organization_id,
                    "Owner reviewed the package.",
                    Utc::now(),
                )
                .expect("resolved review")
        };
        assert!(
            store
                .import_organization_share_review(&wrong_review, &genome)
                .is_err()
        );
        assert!(
            store
                .organization(genome.organization_id)
                .expect("organization lookup")
                .is_none()
        );
        assert_eq!(
            store
                .organization_share_review(pending.id)
                .expect("review lookup")
                .expect("pending review")
                .state,
            OrganizationShareReviewState::Pending
        );
    }

    #[test]
    fn publisher_revocation_is_terminal() {
        let mut store = SqliteEventStore::open_in_memory().expect("store");
        let key = SigningKey::from_bytes(&[59; 32]);
        let first = publisher(&key);
        store
            .record_organization_publisher(&first, None)
            .expect("publisher");
        let mut revoked = first.clone();
        revoked.revision = 2;
        revoked.state = OrganizationPublisherState::Revoked;
        revoked.reason = "Owner revoked the publisher key.".into();
        revoked.updated_at = Utc::now();
        store
            .record_organization_publisher(&revoked, Some(1))
            .expect("revoke publisher");
        let mut restored = revoked;
        restored.revision = 3;
        restored.state = OrganizationPublisherState::Trusted;
        restored.reason = "Attempt to restore revoked authority.".into();
        restored.updated_at = Utc::now();
        assert!(
            store
                .record_organization_publisher(&restored, Some(2))
                .is_err()
        );
    }
}
