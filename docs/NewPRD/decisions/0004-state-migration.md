# ADR-004 — Authoritative State Migration and Compatibility

- **Status:** Accepted for implementation
- **Date:** 2026-08-29
- **Source:** Cognitive Runtime Implementation Contract v0.1, Sections 5, 7, 21, and 23

## Context

MetaCog already has a large production SQLite schema, immutable event history, projections, backups, and user-visible identifiers. The new architecture changes the owner of production state and adds a separate evaluator store. A new empty database or dual-authority cutover would discard value or create reconciliation ambiguity.

## Decision

### Production state

The existing canonical production database evolves in place into Root-owned `state.db`. Preserve the existing configured path unless a later platform-specific migration demonstrates that an atomic path move is required.

Before the first ownership-changing migration:

1. stop accepting new work;
2. drain or durably pause active authority;
3. create and verify a backup;
4. acquire exclusive data-directory ownership;
5. migrate in one Root-controlled sequence;
6. verify schema, event history, projections, artifact references, and backup metadata;
7. start Runtime only after Root owns the migrated store.

There is never more than one authoritative production writer.

### Runtime identity

Migration creates one default `RuntimeIdentity` for the existing installation and binds current production records to it where required. Existing identifiers and historical payload meanings remain unchanged. Records that cannot be truthfully reconstructed receive an explicit legacy/unknown provenance marker rather than fabricated manifests.

### Evaluator state

Create a new protected `evaluation.db` and artifact root with separate ownership/ACL. Existing evaluations remain legacy production evidence. They may be referenced or imported only with their actual provenance and are never retroactively called protected, independent, same-seed, or contamination-aware.

### Compatibility window

- Root hosts compatibility adapters for existing local HTTP clients.
- Old setting and request shapes are upcast into new contracts.
- New-only state is not silently downgraded.
- If an older package cannot interpret the new ownership/schema, downgrade fails with a restore-from-backup path.
- Compatibility adapters are time-bounded and removed only after supported package upgrade paths are qualified.

### Rollback

Rollback before new work commits may restore the verified pre-migration backup. Once new-version canonical events exist, binary downgrade must not write the database. Recovery uses forward repair or an explicit backup restore into a separate validated location.

## Consequences

- Root must be introduced before Runtime is denied database access.
- Backup manifests must cover both production and evaluator stores once Evaluator exists while preserving their confidentiality boundary.
- Migration tests require representative historical databases from supported versions.
- Packaging must coordinate clean shutdown across Root, Runtime, Evaluator, and gateway descendants.

## Rejected alternatives

- **Start a new empty cognitive-runtime profile:** loses continuity and violates product migration requirements.
- **Dual-write old and new databases:** creates two sources of truth and ambiguous crash recovery.
- **Bulk rewrite historical events into new types:** invents provenance and breaks immutable evidence.
- **Import current experiments as protected evidence:** falsely upgrades their trust class.

## Verification

- Fresh, representative old, interrupted, corrupt, and already-migrated fixtures are covered.
- Backup verification occurs before mutation.
- Event bytes and existing IDs remain stable.
- Projections rebuild to equivalent user-visible state.
- Runtime cannot open the migrated database.
- Old binaries refuse unsafe writes after new canonical state exists.
- Packaged upgrade, restart, backup restore, and data-retention UAT pass on Windows, macOS, and Linux.
