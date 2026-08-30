# Stage 1 — Stable Contracts, Canonical Encoding, and Compatibility

**Status:** Implementation complete; local automated and browser E2E evidence green; native packaged UAT remains part of the cumulative release gate

## Delivered foundation

- `mcg-contracts` is a dependency-minimal crate shared by future Root, Runtime, and Evaluator processes.
- Stable UUID newtypes and explicit versioned type discriminators never depend on Rust enum ordering or type names.
- `VersionedPayload` records schema, canonical encoding, normalization, and immutable canonical bytes.
- ACAR normalization v1 uses NFC text, deterministic RFC 8949 CBOR map ordering, shortest integer encodings, rejects floating point values, and fails closed when distinct source keys collide after normalization.
- Canonical decoding rejects malformed, oversized, unknown-version, and valid-but-noncanonical payloads.
- BLAKE3-256 hashes include an explicit semantic domain and schema version. Existing SHA-256 identities remain explicitly representable.
- Checked-in v1 payload, canonical bytes, and hash fixtures guard library upgrades.
- Explicit decoder/upcaster dispatch fails closed for unknown type/schema combinations.
- Initial contracts cover runtime identity, task/workflow lineage, event payloads, action intent/execution/receipt, context and invocation manifests, completion certificates, evaluation seeds, experiment specifications, RPC envelopes, error taxonomy, disposition, and redaction.
- The contracts crate has an automated forbidden-dependency check and cannot import storage, runtime, policy, provider, tool, HTTP, Tauri, or SQLite implementations.

## Interface compatibility

- Durable user settings schema v2 stores one `interface_complexity` discriminator: `simple`, `advanced`, or `developer`.
- Legacy schema-v1 `advanced_mode=false` upcasts to Simple and `advanced_mode=true` upcasts to Advanced. No migration selects Developer.
- Unknown future settings schemas fail closed. New persisted revisions never write the legacy Boolean.
- The API continues accepting legacy `advanced_mode` requests and emitting a derived response-only Boolean during the compatibility window; an explicit v2 value wins. Storage writes only the enum.
- The existing `advanced` UI gates remain the practical-control layer. Developer includes that same layer and adds read-only contract/manifest inspection.
- Switching levels preserves the current workspace, composer, autonomy, shadow-experiment setting, workflow state, and all backend authority.
- Developer copy explicitly states that protected policy, evaluator fixtures, Root authority, signing keys, secrets, direct database writes, and trust bypass remain inaccessible or read-only.
- CLI `settings set --interface-complexity simple|advanced|developer` is additive; legacy `--advanced-mode` remains accepted.

## Evidence

- Strict focused Clippy: `cargo clippy -p mcg-contracts -p mcg-config --all-targets -- -D warnings`
- Contract/config/storage suites: 85 tests passed, including property tests and the full 73-test storage suite.
- API owner-settings integration and CLI settings parser regression passed.
- Desktop TypeScript build/typecheck passed.
- Focused Vitest interface tests passed, including active-draft preservation and Simple → Advanced → Developer → Simple behavior.
- Playwright browser E2E for all three levels passed and retained the same active composer/workspace.
- The pre-existing signed-organization-sharing test that timed out under a slow verbose full run passed independently in 11.2 seconds.
- The definitive serialized all-feature Rust workspace suite passed, including all three real Windows updater-supervisor tests. Its asynchronous self-delete assertion now uses a bounded poll instead of a timing-sensitive fixed sleep.

## Cumulative gates still required

- Complete deterministic Vitest passed 121/121 and serialized Playwright passed 28/28 after all Stage 1 changes; final focused three-level reruns passed after the compatibility projection was finalized.
- Native packaged Windows GUI UAT for migration, all three levels, keyboard/accessibility, and Developer disclosures.
- Cross-process/cross-version fixture exchange repeats on Windows, macOS, and Linux when Stage 2 IPC exists.
- Old-profile upgrade, backup/restore, and supported downgrade-window qualification repeats against final packages.

Stage 2 may now build Protected Root on these stable contracts. No Stage 1 UI or compatibility projection is an authority boundary.
