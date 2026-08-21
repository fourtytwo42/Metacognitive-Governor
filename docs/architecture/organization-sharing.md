# Signed Organization Sharing

Organization sharing is an owner-reviewed import boundary for portable organization genomes. It is not ambient discovery, federation, package execution, or delegated administration. A verified package can describe roles, relationships, model/tool/memory policy, organization policy, and evaluation policy; it cannot carry credentials, leases, runtime state, provider bindings, or local authority.

## Threat Model

The boundary assumes registries, packages, networks, and publisher claims may be malicious or stale. It defends against package tampering, publisher impersonation, duplicate-key identities, registry-scope confusion, replay with changed content, revocation races, and a UI treating cryptographic verification as owner consent. It does not claim that a valid signature makes an organization useful, safe for a particular project, or entitled to tools.

Trust is local and explicit. An owner records one bounded publisher identity, one immutable 32-byte Ed25519 public key, and one or more exact canonical HTTPS registry origins. Registry URLs cannot contain credentials, paths, queries, or fragments. Publisher keys must be unique. Trust revisions are monotonic; revocation is terminal and cannot be deleted or silently reversed.

## Signed Package Contract

Schema 1 uses the `ed25519-v1` scheme and a domain-separated canonical payload. The signature binds all of:

- the schema version;
- canonical registry origin;
- bounded package name;
- publisher identity;
- the complete portable organization genome, including source provenance and its content digest.

Verification revalidates the portable genome, package bounds, current publisher state, exact registry scope, canonical base64 encodings, and Ed25519 signature. Durable evidence records SHA-256 digests for the publisher key, portable definition, complete package, and signature. Package bytes and public keys are not credentials; private signing keys never enter MCG.

## Review Before Import

Verification creates only a durable `pending` review. It grants no organization, model, provider, tool, project, or execution authority. A second explicit owner action must supply a printable reason and caller-owned import UUID before import. Import rechecks the current trust head and signature, then atomically creates a fresh local root organization, resolves the review, and appends both import events. Local organization and role identities are new; publisher authority is never inherited.

A pending review may instead become `rejected` with an owner reason. Imported and rejected states are terminal. Exact retries are observation-only; reuse of an identity with changed input conflicts. Revocation between review and import fails closed. Daemon restart preserves the trust head, pending review, evidence, and retry identities in SQLite schema 34.

## Interfaces

Authenticated REST exposes:

| Endpoint | Behavior |
| --- | --- |
| `GET/POST /api/v1/organization-publishers` | List trust heads or trust/revise one publisher. |
| `POST /api/v1/organization-publishers/{id}/revoke` | Permanently revoke the exact expected revision. |
| `GET/POST /api/v1/organization-share-reviews` | List reviews or verify one signed package into pending review. |
| `POST /api/v1/organization-share-reviews/{id}/import` | Reverify current trust and explicitly import. |
| `POST /api/v1/organization-share-reviews/{id}/reject` | Explicitly reject a pending review. |

The CLI exposes `organization publishers`, `trust-publisher`, `revoke-publisher`, `share-reviews`, `review-share`, `import-share`, and `reject-share`. MCP exposes read and mutation tools for publishers and reviews. Tauri and desktop use the same loopback API. Simple mode shows publisher, registry/package, and verified key/package digests, then requires a separate reasoned import click. Advanced mode adds exact identities, all evidence digests, registry scopes, trust/revision controls, and terminal revocation.

## Verification

Core tests cover tampering, wrong registry scope, duplicate keys, and revoked or unknown publishers. Storage tests cover restart/reopen, atomic rollback, immutable terminal state, and event/projection agreement. Runtime/API tests cover review/import replay, revocation between review and import, authentication, REST, MCP, and daemon restart. CLI parsing, Tauri compilation, component journeys, and Playwright verify every first-party interface and the visible separation between verification and import.
