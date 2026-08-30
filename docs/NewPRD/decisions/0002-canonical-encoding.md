# ADR-002 — Canonical Encoding, Hashing, and Historical Compatibility

- **Status:** Accepted for implementation
- **Date:** 2026-08-29
- **Source:** Cognitive Runtime Implementation Contract v0.1, Sections 7 and 18

## Context

Current Rust types, JSON DTOs, and SQLite rows are not a sufficient five-year persistence or cross-process protocol. Historical facts, action identities, invocation manifests, experiment seeds, and evaluator decisions require stable bytes and hashes across ordinary software evolution.

## Decision

### Stable envelope

Every long-lived polymorphic payload and cross-trust-domain message uses an envelope containing:

```text
stable discriminator
payload schema version
canonical encoding version
canonical normalization version
hash algorithm identifier where hashed
payload bytes
```

Rust enum variant order, type names, memory layout, and default Serde representation are not protocol identifiers.

### Encoding

Canonical CBOR is the initial binary encoding. MetaCog will define and test its own accepted canonical subset rather than relying on undocumented serializer behavior. The normalizer must define map-key ordering, integer representation, floating-point restrictions, text normalization, optional/default handling, timestamps, identifiers, paths, and set ordering.

JSON remains available for public APIs and human inspection, but JSON serialization is not automatically the canonical hash input.

### Hashing

BLAKE3 is the initial canonical content hash. Every hash uses semantic domain separation including object family and relevant encoding/normalization version. A hash identifier is stored with the digest so future algorithms can coexist.

Existing SHA-256 artifact and compatibility hashes remain valid under their original declared contracts. Historical hashes are not silently relabeled or recomputed as BLAKE3.

### Evolution

- Decoders and upcasters read every supported historical schema.
- Ordinary migrations do not rewrite immutable historical payload bytes.
- A new encoding or normalization version requires new golden fixtures and explicit compatibility behavior.
- Unknown stable discriminators or unsupported versions fail closed at authority boundaries and remain preservable for backup/export where safe.
- Checkpoints may compact projections, but canonical event/evidence bytes remain immutable.

## Consequences

- `mcg-domain` gains a narrow canonical-codec contract or depends on a lower-level compatibility module that has no higher-level behavior dependencies.
- Database columns distinguish canonical bytes from display JSON.
- IPC and persistence may share canonical payload definitions while retaining separate transport envelopes.
- Golden fixtures become release artifacts and must be reviewed like migrations.

## Rejected alternatives

- **Hash current JSON:** whitespace, key ordering, number handling, and serializer evolution are too fragile.
- **Hash current Rust bincode/layout:** Rust types are not a stable protocol.
- **Rewrite all history on migration:** destroys immutable evidence and complicates causal audit.
- **Replace every existing SHA-256 use immediately:** unnecessary compatibility break; algorithms remain explicit per contract.

## Verification

- Golden v1 bytes and hashes are checked in.
- Encode/decode/property tests cover canonical equivalence and malformed inputs.
- Current builds decode old fixtures and preserve bytes.
- Cross-process tests exchange canonical payloads on all three platforms.
- An intentional normalization-version change creates a new digest rather than mutating an old one.
