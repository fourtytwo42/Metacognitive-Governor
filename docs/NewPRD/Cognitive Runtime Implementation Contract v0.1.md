# Cognitive Runtime Implementation Contract
# Adaptive Cognitive Agent Runtime

**Artifact:** Normative implementation contract
**Version:** 0.1
**Architecture baseline:** Adaptive Cognitive Agent Runtime PRD v1.4.2 — Implementation Semantics Freeze
**Experimental baseline:** Cognitive Runtime Experimental Protocol Specification v0.2
**Target platforms:** Windows, macOS, Linux
**Primary stack:** Rust + Tauri + SQLite
**Status:** Implementation handoff — architecture frozen; implementation semantics normative

---

# 0. Purpose and Normative Language

This document converts the frozen cognitive/trust architecture into executable software contracts.

It does **not** add a new cognitive subsystem, agent class, memory class, optimizer layer, culture mechanism, or evolutionary mechanism.

Its purpose is to remove implementation ambiguity around:

```text
process ownership
database ownership
IPC
workflow state
leases
ActionEnvelope execution
uncertain external effects
reconciliation
model data egress
InvocationManifest reconstruction
Tool and Skill execution
cross-platform isolation
protected evaluator exchange
canonical persistence
schema evolution
crash recovery
observability
upgrade compatibility
```

Normative keywords:

- **SHALL / MUST** — required for a conforming v1 implementation.
- **SHALL NOT / MUST NOT** — prohibited.
- **SHOULD** — default unless a documented ADR gives a measured reason to differ.
- **MAY** — optional.

The governing engineering rule is:

> **Every material transition has one owner, one immutable request or intent, one authority decision, one observable result, one durable state transition, and one restart story.**

The governing trust rule is:

> **ROOT owns authority. RUNTIME proposes cognition. EVALUATOR owns protected judgment.**

The governing crash rule is:

> **A timeout or process death does not prove an external effect failed. Unknown outcomes are reconciled, not blindly retried.**

The governing compatibility rule is:

> **The runtime's historical bytes must remain interpretable after ordinary software evolution. Current Rust types are not the persistence protocol.**

---

# 1. Scope and Non-Goals

## 1.1 In scope

This contract governs the first production implementation of:

```text
Tauri desktop shell
Protected Root process
Cognitive Runtime worker process
Protected Evaluator process
SQLite persistence
local authenticated IPC
model-provider execution
Tool subprocess execution
TaskContract / WorkflowAttempt
Event / Record persistence
ActionEnvelope / ActionExecution / ActionReceipt
ContextManifest / InvocationManifest
Skill / Tool manifests
configuration activation
EvaluationSeed / ExperimentSpec exchange
promotion commit
restart / reconciliation
```

## 1.2 Explicitly out of scope for v0.1

Do not implement through this contract:

```text
P2P Culture networking
Evolution Lab
population search
Genome / Phenotype
Global Workspace production subsystem
Attention Schema production subsystem
persistent Self Agent
persistent Theory-of-Mind Agent
rich ActorModel
meta-evolution
automatic protected-source installation
automatic arbitrary Tool-code promotion
remote evaluator cluster
hardware enclaves
custom distributed database
```

If later evidence justifies one of these, it receives its own contract/ADR.

---

# 2. Threat Model

## 2.1 In-scope hostile or compromised principals

The implementation SHALL assume the following can be malicious, corrupted, adversarial, or simply wrong:

```text
foundation-model output
retrieved web/document/repository content
prompt-injected content
candidate CognitiveConfiguration
candidate prompt / role / Skill
candidate-generated Tool artifact or source patch
Cognitive Runtime process
optimizer logic
Tool subprocess
remote Tool/MCP/plugin server
remote model/provider response
Tauri/webview content
untrusted repository contents
```

The design SHALL remain safe if the Cognitive Runtime is fully compromised within its granted OS identity.

## 2.2 Protected principals

The security claims depend on the integrity of:

```text
Protected Root executable and protected configuration
Protected Evaluator executable and protected configuration
root/evaluator IPC authentication and framing
Cedar/policy implementation and protected policy bundle
persistence codecs used by protected components
signature/hash verification
sandbox/process launcher
installer/update verifier
OS mechanisms enforcing configured ACL/process boundaries
```

## 2.3 Out-of-scope adversary

The v1 desktop threat model SHALL NOT claim resistance to an attacker with unrestricted machine administrator/root/kernel control.

A malicious OS administrator may be able to:

```text
replace binaries
read process memory
change ACLs
inject code
steal credentials
modify kernel behavior
```

This limitation must be explicit in security documentation.

## 2.4 UI authority

The Tauri UI is not a protected authority owner.

User approval is authority input; UI code is only a carrier of that input. Protected Root validates session identity, request integrity, current state, and applicable policy.

---

# 3. Required Process Topology

## 3.1 Production trust-domain processes

Initial production SHALL use three backend trust-domain processes:

```text
                         TAURI UI
                            │
                    authenticated IPC
                            │
                            ▼
                  ┌──────────────────┐
                  │  protected-rootd │
                  │                  │
                  │ authority        │
                  │ workflow         │
                  │ state.db writer  │
                  │ action commit    │
                  │ egress permits   │
                  │ promotion        │
                  └──────┬─────┬─────┘
                         │     │
              typed IPC  │     │ protected IPC
                         ▼     ▼
             ┌────────────────┐   ┌────────────────────┐
             │ runtime-worker │   │ protected-evaluator│
             │                │   │                    │
             │ context        │   │ evaluation.db      │
             │ memory logic   │   │ fixtures           │
             │ Work / Verify  │   │ experiment control │
             │ optimizer      │   │ metrics / decision │
             └───────┬────────┘   └─────────┬──────────┘
                     │                      │
              permits│                      │ launches isolated
                     ▼                      ▼
          provider/tool child         evaluation runtime workers
          processes/adapters
```

`Optimizer` remains logic inside `runtime-worker`; it is not a separate trust domain.

## 3.2 Protected Root

`protected-rootd` SHALL:

```text
own authoritative production mutations
own TaskContract authoritative versions
own durable WorkflowAttempt state
own workflow leases/generations
own ActionEnvelope authorization and execution lifecycle
own model-egress authorization
own Tool authorization
own secret-handle resolution policy
own CognitiveConfiguration activation
own promotion commit
own shutdown / quarantine decisions
own root-update verification
```

It SHALL NOT perform general user-task reasoning.

It SHALL NOT import the runtime implementation crate.

## 3.3 Cognitive Runtime

`runtime-worker` SHALL:

```text
interpret persistent cognitive state
compile context
invoke models through governed egress
perform Work / Verify cognition
select Skills / capabilities
propose Tool operations
propose Record / Engram mutations
produce RuntimeTelemetry interpretations
open ImprovementCases
generate bounded mutation candidates
```

It SHALL NOT:

```text
open state.db directly for unrestricted query or mutation
open evaluation.db
commit TaskContract authority
commit side effects
promote CognitiveConfiguration
resolve raw secrets
modify evaluator assets
bypass root when protected components are unavailable
```

## 3.4 Protected Evaluator

`protected-evaluator` SHALL:

```text
own evaluation.db
own protected fixture storage
validate EvaluationSeed / ExperimentSpec
create isolated baseline/candidate environments
launch pinned runtime workers as child processes
apply learning-mode rules
measure protected outcomes
maintain evaluation exposure
apply promotion policy
store evaluator-only ResearchMeasurements
sign/attest disclosed evaluation decisions
```

It SHALL NOT import candidate/runtime implementation into its own protected address space.

## 3.5 Child execution processes

Model-provider adapters and Tool runners MAY execute as subordinate child processes.

They do not constitute new semantic trust domains.

Their authority SHALL be represented by a narrow permit created by Root for one exact operation/request.

---

# 4. Crate and Dependency Contract

## 4.1 Workspace

```text
crates/
  cognition-types/
  protected-root/
  runtime/
  protected-evaluator/
  gateways/
  platform/
  research/

src-tauri/
```

## 4.2 `cognition-types`

Owns only stable shared contracts:

```text
IDs and opaque refs
versioned wire envelopes
TaskContract DTOs
Workflow / action DTOs
Event / Record payload envelopes
Authority descriptors
ContextManifest / InvocationManifest
SkillManifest / ToolManifest
CognitiveConfiguration
EvaluationSeed / ExperimentSpec public contracts
EvaluationDecisionSummary
error/result classes
```

Forbidden dependencies:

```text
SQLx
Tokio orchestration
Cedar engine
filesystem access
provider SDKs
runtime implementation
```

## 4.3 `protected-root`

Binary: `protected-rootd`.

May depend on:

```text
cognition-types
platform
SQLite / SQLx
Cedar
crypto/signature/hash libraries
```

MUST NOT depend on:

```text
runtime implementation
protected-evaluator implementation
research
LLM/provider cognition code
```

## 4.4 `runtime`

Binary: `runtime-worker`.

Modules MAY include:

```text
state interpretation
context
execution
memory
telemetry
improvement
culture_later
```

May depend on:

```text
cognition-types
gateways client/protocol APIs
provider-independent pure helpers
```

MUST NOT depend on protected Root/Evaluator implementation crates.

## 4.5 `protected-evaluator`

Separate binary.

May depend on:

```text
cognition-types
platform
protected pure metric/evaluation code
provider adapter protocol/client code where required
```

MUST NOT depend on:

```text
runtime implementation
optimizer implementation
root implementation
research as a production dependency
```

## 4.6 `gateways`

Contains adapters/protocol clients:

```text
model providers
Tool protocol
MCP client/adapter
provider runner
secret-handle transport types
```

Gateways SHALL NOT decide authority.

## 4.7 `platform`

Owns OS-specific mechanics:

```text
named pipes / Unix sockets
peer identity
process launch / termination
ACL helpers
sandbox capability discovery
filesystem canonicalization
resource controls
secret-store adapter
code-signing/installation checks where applicable
```

The crate reports what the host can enforce. It SHALL NOT silently pretend unsupported isolation exists.

## 4.8 `research`

Experiment drivers/analysis only.

Production crates SHALL NOT depend on it.

---

# 5. Persistence and Authoritative State Ownership

## 5.1 Physical databases

V1 SHALL use exactly two protected persistence roots for core architecture:

```text
state.db
  owner/writer: protected-rootd
  purpose: production/runtime authoritative and persistent cognitive state

evaluation.db
  owner/writer: protected-evaluator
  purpose: protected experimental evidence and evaluator-only state
```

The Runtime SHALL NOT open `state.db` directly in V1. Production state reads and mutation proposals cross Root's typed, authorized query/mutation IPC. This ensures authorization is applied before candidate content reaches cognition and prevents a compromised Runtime from treating filesystem access as storage authority.

The Evaluator SHALL NOT have a writable handle to `state.db`.

Root SHALL NOT have a writable handle to `evaluation.db`.

Cross-domain exchange occurs through immutable artifacts, IDs/hashes, authenticated IPC, and signed/content-addressed summaries.

## 5.2 Why not a separate runtime DB in v1?

Runtime owns **cognitive semantics**, but Root owns **authoritative persistence mutation and governed access to canonical production state**.

Keeping production persistent state in one Root-owned database avoids:

```text
cross-database atomicity for Record + workflow/action history
runtime becoming a second authoritative writer
ambiguous event ordering
replicated TaskContract/promotion truth
more recovery protocols before evidence requires them
```

If Root becomes a measured throughput bottleneck, a future ADR may split storage without changing authority semantics.

## 5.3 `state.db` minimum tables

```text
schema_meta
runtime_instances

task_contracts
workflows
workflow_attempts

events
artifacts

records
record_versions
record_edges

actions
action_executions
action_receipts
reconciliation_jobs

invocations
model_runs
tool_runs

configurations
capability_registrations

improvement_cases

evaluation_refs
promotion_records
```

Repository projections MAY use additional tables:

```text
repository_files
repository_symbols
repository_edges
```

Culture MAY add tables later after its phase gate.

## 5.4 `evaluation.db` minimum tables

```text
schema_meta
evaluation_seeds
experiment_specs
experiment_runs
evaluation_results
evaluation_exposures
objective_manifests
promotion_policy_versions
research_measurements
fixture_manifests
tcb_manifests
evaluator_events
```

Raw protected fixture/result material stays in this trust domain.

## 5.5 SQLite operating policy

Each DB SHALL initially use:

```text
WAL mode
foreign_keys = ON
configured busy_timeout
one semantic writer coordinator
small bounded reader pool
short transactions
```

No model call, Tool execution, network request, or external filesystem effect may occur while a database write transaction is held open.

Root SHOULD serialize authoritative production transitions through a bounded writer queue.

SQLite locking is not a security boundary.

## 5.6 Derived indexes

FTS5, sqlite-vec, repository indexes, embeddings, and caches are derived.

A failure to update a derived index SHALL NOT invalidate a successfully committed canonical Record/Event.

Indexes SHALL carry a source cursor/version and support rebuild.

## 5.7 Canonical histories by trust domain

The system does not expose one globally readable ledger.

Instead:

```text
Root Event Ledger
= canonical production/runtime causal history

Evaluator Event Store
= canonical protected experimental history

EvaluationDecisionSummary / EvaluationRef
= signed or hash-bound disclosed projection into Root history
```

One domain SHALL NOT rewrite the other domain's canonical evidence.

---

# 6. Artifact Store Contract

## 6.1 Large immutable bytes

Large payloads SHOULD live outside SQLite in a content-addressed artifact store.

Examples:

```text
model responses
large prompts/context snapshots where retention policy permits
diffs
logs
repository snapshots
Tool receipts
EvaluationSeed bundles
configuration artifacts
```

## 6.2 Layout

Recommended:

```text
artifacts/
  ab/
    cd/
      <full-blake3-hash>
```

## 6.3 Creation

Canonical artifact write:

```text
create temp in protected destination filesystem
→ write bytes
→ flush/fsync as required
→ compute canonical/raw hash
→ verify
→ atomic rename
→ commit ArtifactRef metadata
```

Candidate-controlled symlinks SHALL NOT be followed when creating canonical artifacts.

## 6.4 Ownership

Root and Evaluator SHOULD have separate protected artifact roots where evaluator confidentiality requires it.

Runtime receives ArtifactRefs only to content it is authorized to read.

---

# 7. Canonical Encoding, Hashing, and Five-Year Compatibility

## 7.1 Current Rust types are not storage format

Persisted state SHALL use a versioned envelope such as:

```rust
struct VersionedPayload {
    type_id: StableTypeId,
    schema_version: u32,
    canonical_encoding: CanonicalEncodingVersion,
    normalization: CanonicalNormalizationVersion,
    bytes: Vec<u8>,
}
```

## 7.2 Stable discriminator

Persistent type IDs SHALL NOT derive implicitly from Rust enum names or ordinal positions.

Example:

```text
workflow.action_committed.v1
record.claim.v1
record.engram.semantic.v1
```

## 7.3 Canonical encoding

V1 SHALL define one canonical byte encoding for content-addressed structured objects.

Recommended initial choice:

```text
CanonicalEncodingVersion = canonical-cbor-v1
CanonicalNormalizationVersion = acar-normalization-v1
HashAlgorithm = BLAKE3-256
```

Domain separation SHALL be used so the same bytes in different semantic domains cannot accidentally share a semantic hash identity.

Example input to hash:

```text
"ACAR:PromptBundle:v1\0" || canonical_bytes
```

## 7.4 Decoder/upcaster chain

Historical payload bytes remain immutable.

Readers SHALL support:

```text
stored v1 bytes
→ v1 decoder
→ semantic upcaster
→ current in-memory representation
```

Do not rewrite old Events merely to simplify current code.

## 7.5 Compatibility tests

CI SHALL retain golden canonical payloads/hashes across releases.

A library upgrade that changes canonical bytes without an explicit encoding-version change is a compatibility failure.

## 7.6 Checkpoints

Checkpoint manifests SHALL include:

```text
checkpoint schema version
source Event cursor
canonicalization versions
application compatibility range
projection/index schema versions
content hashes
```

If a checkpoint is incompatible, discard/rebuild the projection from a compatible checkpoint or canonical history.

---

# 8. Local IPC Contract

## 8.1 Transport

Root↔Runtime and Root↔Evaluator SHALL use local OS IPC.

Recommended:

```text
Windows: named pipes
macOS/Linux: Unix-domain sockets
```

A network service is not required.

## 8.2 Wire encoding

V1 SHALL use a single bounded framed protocol.

Recommended:

```text
length-prefixed canonical CBOR
```

JSON MAY be used during a spike but the final protocol must preserve explicit schema/version boundaries.

## 8.3 Envelope

```rust
struct RpcEnvelope {
    protocol_version: u16,
    request_id: RequestId,
    correlation_id: CorrelationId,
    session_id: SessionId,
    caller_instance: InstanceId,
    message_type: StableMessageType,
    expires_at: Option<DateTime<Utc>>,
    payload_len: u32,
    payload: Bytes,
}
```

## 8.4 Authentication

Use defense in depth:

```text
OS pipe/socket ACL
OS peer identity validation where available
per-launch random session capability
protocol request/session binding
```

Session capability SHALL NOT be passed through a broadly visible command-line argument.

Runtime must not gain privilege if Root is unavailable.

Fail closed:

```text
protected component unavailable
→ protected operation blocked/waiting
```

## 8.5 Replay / idempotency

Mutation requests SHALL include:

```text
request_id
idempotency_key
expected version/generation
expiry where applicable
```

Root persists accepted mutation request identity sufficiently to make replay behavior deterministic.

## 8.6 Size limits

IPC frames SHALL be bounded.

Large content moves by `ArtifactRef` rather than oversized frames.

## 8.7 Cancellation

Every long-running request supports a typed cancellation request and deadline.

Cancellation means stop/ignore future computation where possible.

It does **not** mean an external effect has been reversed.

## 8.8 Protocol version negotiation

Protected endpoints reject unsupported protocol versions explicitly.

No permissive “best effort” deserialization of unknown privileged messages.

---

# 9. Durable Workflow Contract

## 9.1 Workflow states

Initial durable states:

```rust
enum WorkflowState {
    Ready,
    Running,
    Waiting(WaitReason),
    Verifying,
    CancelRequested,
    Succeeded,
    Failed,
    Cancelled,
    Quarantined,
}
```

Recommended `WaitReason`:

```text
User
Approval
External
RetryBackoff
Dependency
Reconciliation
ProtectedComponent
```

Do not persist a separate `Created` state initially; create workflow + initial `Ready` state atomically.

## 9.2 Terminal states

```text
Succeeded
Failed
Cancelled
Quarantined
```

Administrative recovery from `Quarantined` requires explicit privileged procedure; it is not ordinary workflow retry.

## 9.3 Legal transitions

```text
READY → RUNNING

RUNNING → WAITING
RUNNING → VERIFYING
RUNNING → FAILED
RUNNING → CANCEL_REQUESTED
RUNNING → QUARANTINED

WAITING → READY
WAITING → FAILED
WAITING → CANCEL_REQUESTED
WAITING → QUARANTINED

VERIFYING → SUCCEEDED
VERIFYING → READY              # defect/rework
VERIFYING → FAILED
VERIFYING → CANCEL_REQUESTED
VERIFYING → QUARANTINED

CANCEL_REQUESTED → CANCELLED   # only when no unresolved side effect remains
CANCEL_REQUESTED → WAITING(Reconciliation)
CANCEL_REQUESTED → QUARANTINED
```

Any unspecified transition is invalid unless introduced by a versioned state-machine revision.

## 9.4 WorkflowAttempt

```rust
struct WorkflowAttempt {
    workflow_id: WorkflowId,
    attempt_id: AttemptId,
    task_contract: VersionedRef<TaskContract>,

    state: WorkflowState,

    lease_owner: Option<RuntimeActorId>,
    lease_generation: u64,
    lease_expires_at: Option<DateTime<Utc>>,
    heartbeat_at: Option<DateTime<Utc>>,

    checkpoint: Option<ArtifactRef>,
    environment_snapshot: EnvironmentRef,
    configuration: ConfigurationVersion,

    idempotency_key: String,
    retry_policy: RetryPolicy,
    compensation_policy: CompensationPolicy,
}
```

## 9.5 Lease generation fencing

A lease expiration does not prove an old worker stopped.

Every state-changing worker response SHALL include the `attempt_id` and `lease_generation` under which it was produced.

After ownership changes, late responses from an older generation are rejected from authoritative commit.

## 9.6 Restart scanner

On Root startup:

```text
open DB
verify/migrate
inspect nonterminal workflows
inspect active/unknown actions
reconcile required effects
expire/fence stale leases
requeue safe eligible attempts
resume waiting workflows only when their condition is satisfied
```

The UI is not part of recovery truth.

## 9.7 Cancellation

If cancellation occurs during a model invocation, late model output may be archived but cannot commit after generation/attempt invalidation.

If cancellation occurs during an effectful Tool action:

```text
request cancellation
→ stop waiting/terminate where safe
→ classify effect outcome
→ reconcile if unknown
→ only then enter Cancelled
```

---

# 10. Action and Side-Effect Contract

## 10.1 Separation of intent, execution, observation

Use three concepts:

```text
ActionEnvelope
= immutable authorized-intent candidate

ActionExecution
= mutable durable lifecycle of physical attempts

ActionReceipt
= immutable observation/evidence about what happened
```

## 10.2 ActionEnvelope

```rust
struct ActionEnvelope {
    id: ActionId,
    workflow_id: WorkflowId,
    workflow_attempt: AttemptId,
    task_contract: VersionedRef<TaskContract>,

    actor: RuntimeActorId,
    authority: AuthorityEnvelope,

    expected_versions: Vec<ExpectedVersion>,
    operation: TypedOperation,
    operation_hash: Hash,
    effect_class: EffectClass,
    preconditions: Vec<Precondition>,

    budget_reservation: BudgetReservation,
    idempotency_key: String,
    compensation: Option<CompensationPlan>,
}
```

Persist intent **before** physical effect.

## 10.3 Action execution states

```text
PROPOSED
→ AUTHORIZED
→ EXECUTING
→ OBSERVED
   ├─ NO_EFFECT_CONFIRMED
   ├─ EFFECT_CONFIRMED
   └─ OUTCOME_UNKNOWN → RECONCILING

EFFECT_CONFIRMED
→ VALIDATED
→ COMMITTED
```

Additional terminal/exception states MAY include:

```text
DENIED
QUARANTINED
COMPENSATED
```

## 10.4 Effect outcome

```rust
enum EffectOutcome {
    NoEffectConfirmed,
    EffectConfirmed(ReceiptRef),
    OutcomeUnknown(ReconciliationHint),
}
```

`Timeout`, process crash, broken pipe, lost response, or cancellation after dispatch SHALL NOT automatically map to `NoEffectConfirmed`.

## 10.5 Authorization/reservation transaction

Before dispatch Root SHALL atomically:

```text
load current TaskContract / workflow generation
validate policy and authority
validate expected versions
validate exact operation hash
validate effect class
reserve budget/resources
persist authorization/execution intent
commit transaction
```

Only then may a Tool runner execute.

## 10.6 Commit after effect

External effect may precede authoritative DB commit.

If effect is confirmed but Root DB commit fails, Root retries the **database commit/reconciliation**, not the external effect.

## 10.7 Reconciliation

Minimal durable object:

```rust
struct ReconciliationJob {
    action_id: ActionId,
    adapter: ReconcilerRef,
    next_attempt_at: DateTime<Utc>,
    attempts: u32,
    deadline: DateTime<Utc>,
    last_hint: Option<ReconciliationHint>,
}
```

A reconciler returns:

```text
CONFIRMED_NOT_OCCURRED
CONFIRMED_OCCURRED(receipt)
STILL_UNKNOWN
```

If a consequential effect cannot be reconciled safely:

```text
STILL_UNKNOWN after policy/deadline
→ user action required or quarantine
```

No blind repeat.

## 10.8 Compensation

Compensation is always a new ActionEnvelope linked causally to the original action.

Never rewrite history to imply the first effect did not occur.

## 10.9 Irreversible actions

Irreversible/high-consequence operations require:

```text
explicit effect classification
stronger authority threshold
user approval when policy requires it
no blind retry after dispatch
clear outcome/reconciliation semantics
```

## 10.10 Read-only operations

Read-only Tools may use a lighter path only when the ToolManifest and Root policy classify the exact operation/resource as observational and authorized.

Sensitive reads remain capabilities.

---

# 11. Model Invocation and Data-Egress Contract

## 11.1 Model calls are governed egress

A remote model invocation may:

```text
disclose private/project data
consume money/budget
cross a network boundary
send content to a third-party provider
```

Therefore model calls are not treated as authority-free internal computation.

They do not use the full external-mutation ActionEnvelope, but they require a Root-bound `ModelInvocationPermit` when policy requires egress governance.

## 11.2 Proposal

Runtime creates:

```rust
struct ModelInvocationProposal {
    invocation_id: InvocationId,
    workflow_attempt: AttemptId,
    request_artifact: ArtifactRef,
    request_hash: Hash,
    route: ModelRoute,
    data_classification: DataClassification,
    destination_class: DestinationClass,
    context_disclosure: DisclosureManifest,
    requested_limits: ModelBudget,
    deadline: DateTime<Utc>,
}
```

## 11.3 Permit

Root validates and binds:

```rust
struct ModelInvocationPermit {
    permit_id: PermitId,
    invocation_id: InvocationId,
    request_hash: Hash,
    resolved_provider: ProviderRef,
    resolved_model: ModelRef,
    destination: EndpointRef,
    allowed_data_classification: DataClassification,
    max_input_tokens: u64,
    max_output_tokens: u64,
    max_cost: MoneyLimit,
    expires_at: DateTime<Utc>,
    secret_handles: Vec<SecretHandle>,
}
```

The permit cannot authorize a different request hash.

## 11.4 Provider execution

A provider adapter/runner executes the exact permitted request.

Raw provider credentials SHALL NOT be exposed to the Runtime model context.

Where feasible, raw API credentials live only in Root-authorized adapter execution.

## 11.5 Provider trait

```rust
trait ModelProvider {
    fn capabilities(&self, route: &ModelRoute) -> ProviderCapabilities;

    async fn invoke(
        &self,
        request: ModelRequest,
        cancel: CancellationToken,
    ) -> Result<ModelResponse, ModelError>;
}
```

Cognitive code asks for capabilities. It does not branch per provider throughout the runtime.

## 11.6 Retry

Every provider attempt has a distinct attempt/run ID.

Retries are allowed only under declared provider policy.

A model retry cannot duplicate an authoritative external action because model output itself has no commit authority.

## 11.7 Reproducibility metadata

Persist what is observable:

```text
provider
resolved model ID
reported/observed revision/fingerprint where available
request hash
sampling/reasoning controls
timestamp
provider request ID
usage/cost
response hash/artifact
latency/error class
```

Do not claim exact replay of hosted model behavior when the provider cannot guarantee it.

---

# 12. InvocationManifest and ContextManifest

## 12.1 InvocationManifest

Every material model invocation SHALL have an immutable reconstruction manifest.

```rust
struct InvocationManifest {
    invocation_id: InvocationId,
    workflow_attempt: AttemptId,
    task_contract: VersionedRef<TaskContract>,
    runtime_identity: RuntimeIdentity,

    cognitive_configuration: Hash,
    role_manifest: Hash,
    prompt_bundle: Hash,
    context_manifest: Hash,

    model_route: ModelRoute,
    resolved_provider: ProviderRef,
    resolved_model: ModelRef,
    observed_model_fingerprint: Option<String>,
    sampling: SamplingSpec,
    reasoning: Option<ReasoningSpec>,

    available_tool_manifests: Vec<Hash>,
    selected_skill_manifests: Vec<Hash>,
    authority_envelope: AuthorityEnvelopeRef,
    egress_permit: Option<PermitId>,

    repository_revision: Option<Hash>,
    environment: EnvironmentRef,
    runtime_build: Hash,

    request_hash: Hash,
    response_hash: Option<Hash>,
}
```

Time, usage, status, provider request ID, and response ArtifactRef may live in the associated invocation/run row.

## 12.2 ContextManifest purpose

`ContextManifest` records **what information reached the model**, not merely what was retrievable.

For material invocations record at minimum:

```text
selected source refs
source versions/hashes
perspective/holder relevant to selected item
trust/taint/influence metadata necessary for later analysis
selection/ranking policy version
truncation/summarization refs
placement/order
approx token counts
progressive expansion steps where material
```

## 12.3 Privacy

Do not automatically retain every fully materialized private prompt forever.

The manifest MAY reference governed source/artifact hashes instead of copying content.

Retention policy should preserve causal reconstruction without converting telemetry into a second private-data warehouse.

## 12.4 FocusSet

`FocusSet` remains ephemeral.

Persist its material outcome through `ContextManifest` where required; do not create a canonical FocusSet table.

---

# 13. Record / Memory Persistence Contract

## 13.1 Record authority

Runtime owns semantics and proposes changes.

Root performs the authoritative DB mutation.

## 13.2 Record versions

Stable logical Record:

```text
record_id
kind
scope
current_version
status
```

Immutable revision:

```text
(record_id, version)
payload schema version
content hash
payload/source refs
validity metadata
created Event
```

Updates create a new RecordVersion.

## 13.3 Events vs memory

Events remain canonical history.

Engrams remain derived cognition.

Correcting an Engram creates a new version/supersession and never rewrites source Events.

## 13.4 First Engram mutation operations

```text
CREATE
ADD_EVIDENCE
REVISE
MERGE
SUPERSEDE
IGNORE
```

## 13.5 Source invalidation

Invalidated evidence opens revalidation for materially dependent cognition.

Do not recursively delete every descendant by ancestry alone.

Use provenance/influence grades and selective replay/ablation when decision value warrants it.

---

# 14. Skill, Tool, and Capability Contracts

## 14.1 Skill is not arbitrary code

V1 supports a small explicit union:

```rust
enum SkillImplementation {
    StructuredProcedure {
        steps: Vec<ProcedureStep>,
    },
    PromptedProcedure {
        prompt_bundle: Hash,
        input_schema: SchemaRef,
        output_schema: SchemaRef,
    },
    ToolSequence {
        steps: Vec<ToolStep>,
    },
}
```

A Skill may still invoke reasoning.

## 14.2 SkillManifest

Minimum:

```text
id/version/hash
name/summary
input/output contract
preconditions
required capabilities
required Tool refs
implementation
verification procedure
tested conditions
known exclusions
fallback
rollback/revalidation behavior
provenance/evaluation evidence
status
```

## 14.3 ToolManifest

Minimum:

```text
tool identity/version/hash
runner/executable/adapter identity
input/output schema
maximum effect class
required filesystem capability
required network capability
secret-handle requirements
resource ceilings
timeout/output ceilings
platform support
receipt semantics
reconciliation support
```

Registration does not grant capability.

Candidate-generated Tool artifacts remain quarantined/unregistered until target-specific validation succeeds.

## 14.4 Tool invocation validation

```text
model/tool proposal
→ strict schema parse
→ reject invalid/unknown fields per schema
→ semantic validation
→ path/resource canonicalization
→ resolve exact ToolManifest version
→ capability intersection
→ effect classification
→ Root authorization
→ execution
```

Never concatenate model text into a shell command.

Prefer direct executable + argument arrays.

## 14.5 CapabilityProfile

CapabilityProfile is a rebuildable search/index projection over manifests and observed performance.

It is not authority and not canonical truth.

## 14.6 Skill fallback

```text
inside competence envelope
→ normal Skill execution

near boundary
→ Skill + stronger verification

outside known tested region
→ richer Work reasoning/fallback

known exclusion
→ do not use Skill
```

---

# 15. Concurrency Contract

## 15.1 May run concurrently

```text
independent workflows
read-only authorized retrieval
model invocations
independent Work invocations
different worktrees
independent Tool subprocesses within resource limits
protected evaluation jobs with isolated environments
background derived-index rebuild/update
```

## 15.2 Must serialize

```text
authoritative transition for one WorkflowAttempt
lease ownership transition
current champion/config activation
promotion commit
conflicting shared-repository integration
schema migration
same logical Record current-version update
```

## 15.3 Expected version / CAS

Use for:

```text
TaskContract active version
Record current version
Workflow state + lease generation
CognitiveConfiguration champion
repository base revision/file hash
shared Work State revision
```

## 15.4 Durable leases

Use persisted leases for ownership that must survive process death.

Do not use an in-memory mutex as the only guard for:

```text
workflow ownership
promotion
action execution
budget reservation
migration
external reconciliation
```

## 15.5 Queues

Tokio channels may carry wakeups/streams.

They are not durable job truth.

SQLite state is the durable queue/source of recoverable work.

No unbounded `tokio::spawn` for important background jobs.

---

# 16. Cross-Platform Sandbox Capability Contract

## 16.1 Do not promise identical mechanisms

Windows, macOS, and Linux do not expose identical child-process sandbox primitives.

V1 defines common semantics plus measured per-platform guarantee levels.

## 16.2 Guarantee levels

```rust
enum IsolationGuarantee {
    Hard,
    BestEffort,
    Unavailable,
}
```

A `SandboxCapabilityProfile` SHALL state guarantee level per dimension:

```text
filesystem scope
network denial/allowlist
process-tree containment
secret isolation
environment isolation
resource limits
protected-path denial
```

## 16.3 Policy rule

If an operation requires a stronger isolation guarantee than the current host/backend can enforce:

```text
required > available
→ fail closed / require a stronger execution backend
```

A UI label `sandboxed=true` is insufficient.

## 16.4 Platform baseline

| Concern | Windows | macOS | Linux |
|---|---|---|---|
| IPC | Named Pipe + DACL | Unix socket/XPC | Unix socket |
| Process-tree control | Job Objects | process groups / helper | process groups/cgroups |
| FS controls | NTFS ACL / restricted token/AppContainer where viable | ACL/App Sandbox/helper mechanisms | POSIX ACL + Landlock/namespaces/bwrap where viable |
| Network controls | AppContainer/WFP-dependent | platform/helper-dependent | namespace/firewall/seccomp-dependent |
| Resource limits | Job Objects | rlimits/platform facilities | cgroups/rlimits |
| Secret store | DPAPI/Stronghold-backed | Keychain/Stronghold | keyring/Stronghold |

The implementation SHALL document the actual mechanism used per release/platform.

## 16.5 Protected evaluator/root isolation

Where practical, Root/Evaluator use a distinct OS principal or equivalent ACL/capability boundary from writable cognition.

At minimum candidate-writable state cannot control:

```text
protected executable bytes
protected configuration/policy
fixture store
protected working directory
dynamic dependency lookup path
protected IPC endpoint configuration
protected temp/cache root
protected provider credentials
```

---

# 17. Protected Evaluator Exchange Contract

## 17.1 Separate storage

Evaluator owns `evaluation.db` and protected artifacts.

Root stores only signed/hash-bound disclosed references/summaries needed for production history and promotion.

## 17.2 Request path

Runtime cannot connect directly to protected evaluator.

```text
runtime-worker
→ evaluation proposal
→ protected-rootd validation
→ protected-evaluator
```

## 17.3 Experiment request

Root transmits:

```text
ExperimentSpec
CausalAnalysisContract
EvaluationSeedBundle/ref
baseline config refs
candidate config refs
pinned runtime-worker build ref
declared dependency/artifact refs
```

No protected fixture content crosses to Runtime.

## 17.4 Startup TCB manifest

Evaluator records/verifies at startup:

```text
evaluator binary hash
policy/objective/promotion manifest hashes
fixture manifest hash
critical dependency/tool hashes
relevant config hash
runtime/OS/image fingerprint
protected store identity
```

## 17.5 Execution worker isolation

Evaluator launches candidate/baseline cognition as separate worker processes.

It does not import runtime/candidate implementation into evaluator memory/address space.

## 17.6 Learning modes

### NO_LEARNING

Each independent fixture starts from a clean seed fork unless ExperimentSpec explicitly declares a different unit.

No cross-fixture cognitive carryover.

### EPHEMERAL_LEARNING

Experiment-local carryover permitted; destroyed at experiment end.

### TAINTED_LEARNING_ALLOWED

Durable exported descendants require explicit evaluation-family/provenance metadata.

## 17.7 Invalid experiment

Any undeclared outcome-relevant divergence that defeats the declared causal contract produces:

```text
INVALID_EXPERIMENT
```

not a story about candidate improvement.

## 17.8 Disclosed result

Evaluator returns only policy-approved data, e.g.:

```rust
struct EvaluationDecisionSummary {
    experiment_id: ExperimentId,
    baseline_hash: Hash,
    candidate_hash: Hash,
    decision: EvaluationDecision,
    public_metrics: Vec<MetricSummary>,
    guardrail_summary: GuardrailSummary,
    disclosure_class: DisclosureClass,
    evaluator_hash: Hash,
    objective_policy: VersionRef,
    promotion_policy: VersionRef,
    signature: Signature,
}
```

Protected raw fixture results remain evaluator-only.

## 17.9 Promotion

Evaluator never changes current champion directly.

Root validates:

```text
evaluator identity/signature
candidate/baseline hashes
policy versions
current champion
candidate ancestry
staleness/rebase status
```

Then Root atomically creates the immutable promotion record + new current-config pointer.

---

# 18. Evaluation and Research Compatibility

This contract does not replace Experimental Protocol v0.2.

Implementation SHALL expose enough deterministic control for that protocol to express:

```text
EvaluationSeed equivalence
ExperimentSpec conditions
CausalAnalysisContract
NO_LEARNING fixture resets
search accounting
contamination provenance
side-channel/interference attacks
deep rollback tests
learning-to-learn/plasticity studies
```

Phase-0 scientific claims are invalid if implementation shortcuts prevent the protocol from enforcing its declared units, resets, assignment, or protected measurements.

---

# 19. Observability and Privacy Contract

## 19.1 Correlation lineage

Use stable IDs linking:

```text
TaskContractId
→ WorkflowId
→ AttemptId
→ InvocationId
→ ActionId / ToolRunId
→ VerificationId
→ ImprovementCaseId
→ ExperimentId
→ EvaluationId
→ PromotionId
```

## 19.2 Span families

```text
workflow
context.retrieve
context.compile
model.authorize_egress
model.invoke
capability.select
action.authorize
tool.execute
action.reconcile
verify
memory.update
improvement.case
experiment.run
evaluation.measure
promotion.commit
```

## 19.3 Metrics

At minimum:

```text
workflow latency/success
active workflows/lease expirations
model latency/error/cost/rate limit
context candidates/selected/tokens
Root SQLite transaction latency/writer queue depth
Tool latency
OUTCOME_UNKNOWN count
reconciliation duration
verification failures
memory mutations
experiment invalidations
promotion/rejection
sandbox/protected-boundary violations
```

## 19.4 Never log by default

```text
raw secrets
usable authorization/session tokens
protected fixtures/answers
full private prompts
private source files in generic telemetry
raw evaluator-only research results
chain-of-thought/private scratch
```

Use governed artifacts, hashes, sizes, classes, and refs.

Operational logs are not Engrams.

---

# 20. Operational Error Taxonomy

Keep families small and separate from retry disposition.

```rust
enum RuntimeErrorClass {
    PolicyDenied,
    VersionConflict,
    LeaseLost,
    InvalidInput,
    BudgetExceeded,

    ProviderUnavailable,
    ProviderRejected,
    ProviderInvalidResponse,

    ToolExecutionFailed,
    ToolOutcomeUnknown,

    VerificationFailed,

    StateUnavailable,
    StateCorruption,

    ProtectedBoundaryViolation,
    ExperimentInvalid,
    EvaluationContaminated,

    Cancelled,
    Timeout,

    InternalBug,
}
```

Disposition:

```text
RETRYABLE
NON_RETRYABLE
RECONCILABLE
USER_ACTION_REQUIRED
SECURITY_QUARANTINE
PROGRAMMING_BUG
```

Examples:

```text
ToolOutcomeUnknown → RECONCILABLE, not blind RETRYABLE
PolicyDenied → NON_RETRYABLE
ProtectedBoundaryViolation → SECURITY_QUARANTINE
VersionConflict → regenerate/rebase before retry
```

---

# 21. Upgrade, Migration, and Downgrade Discipline

## 21.1 Database migrations

Before migration:

```text
consistent backup/checkpoint
exclusive migration ownership
```

After migration:

```text
schema verification
integrity check
compatibility check
```

Root migrates `state.db`; Evaluator migrates `evaluation.db`.

## 21.2 Prompts/Skills/Tools/configurations

These are immutable versions/content hashes.

Upgrade = new artifact.

Do not edit a promoted artifact in place.

## 21.3 Configuration compatibility

A CognitiveConfiguration or capability manifest MAY declare compatibility ranges for:

```text
state schema
record payload versions
index versions
Tool versions
runtime build range
```

Rollback cannot assume an old configuration understands newly created state.

If incompatible:

```text
reproject / upcast / quarantine / require compatible runtime
```

as explicitly defined by policy.

## 21.4 Evaluator upgrades

Old ExperimentSpec remains bound to evaluator/objective/promotion hashes used at execution.

Rerunning an old spec under a new evaluator must be labeled a new evaluation condition, not silently called exact reproduction.

---

# 22. Test Architecture

## 22.1 Tier 1 — pure unit tests

No model required.

```text
IDs/codecs
canonical hashing
policy helpers
authority intersection
workflow transitions
Action state transitions
ranking/dedupe
path validation
budget accounting
```

## 22.2 Tier 2 — property/state-machine tests

Generate legal/illegal sequences for:

```text
WorkflowState
lease generation
ActionExecution
request replay/idempotency
RecordVersion CAS
canonical encode/decode
```

Required invariants include:

```text
authority never expands
terminal workflow cannot ordinary-transition back
stale lease generation cannot commit
the same idempotency key cannot cause a second logical effect
```

## 22.3 Tier 3 — SQLite integration

Real temporary SQLite DBs:

```text
WAL
foreign-key constraints
concurrent reads
writer serialization
migration
backup/restore
FTS rebuild
record-version conflict
```

## 22.4 Tier 4 — crash injection

Mandatory crash points:

```text
before intent persistence
after intent persistence
before/after authorization
before Tool launch
during Tool
immediately after effect
before receipt persistence
after receipt before authoritative commit
after commit before UI/event delivery
```

## 22.5 Tier 5 — process-boundary/IPC

```text
malformed frame
unsupported version
stale session
wrong peer
oversized payload
replay
late response
Root death
Runtime death
Evaluator death
```

## 22.6 Tier 6 — security

Attempt:

```text
path traversal
symlink/reparse escape
environment poisoning
dynamic-library poisoning
secret leakage
Root DB direct access
Evaluator DB/fixture read
unauthorized network destination
process-tree escape
candidate config promotion without evaluator decision
```

## 22.7 Tier 7 — provider contract

Use deterministic fake provider for most tests.

Recorded responses for parser regressions.

Real provider only in opt-in/nightly integration.

## 22.8 Tier 8 — protected evaluator

Use fake deterministic runtime/model first so infrastructure is not confounded by model stochasticity.

## 22.9 Tier 9 — real end-to-end cognition

Use remote/local models only where model cognition is actually the variable under test.

---

# 23. Required Vertical Slices

## Slice A — durable empty runtime

Build:

```text
Tauri
protected-rootd
runtime-worker
state.db
local IPC
RuntimeIdentity
TaskContract
WorkflowAttempt
Event
```

Demo:

> Create a workflow, kill/restart application, recover the exact durable workflow without model cognition.

## Slice B — deterministic Work + Verify

Add fake deterministic provider + InvocationManifest + CompletionCertificate.

No Tools yet.

Exit:

> Every invocation condition is reconstructable.

## Slice C — one governed read

Tool: `read_file` inside a test worktree.

Exit:

> Runtime cannot directly read outside Root-authorized scope.

## Slice D — one governed reversible write

Tool: `write_file`/patch inside isolated worktree.

Add ActionEnvelope / Execution / Receipt.

Crash at every durable boundary.

Exit:

> No duplicate committed/effected write.

## Slice E — reconciliation

Fake external Tool intentionally performs the effect and drops receipt.

Expected:

```text
OUTCOME_UNKNOWN
→ restart
→ reconcile
→ EFFECT_CONFIRMED
→ commit existing effect
```

## Slice F — basic persistent cognition

Add Record/RecordVersion/edges + FTS + ContextManifest + narrow Engram create/revise.

No vector index required if FTS suffices.

## Slice G — immutable cognition configuration

Add PromptBundle/Role/Policy/Skill/Tool manifests and CognitiveConfiguration content hashes.

## Slice H — protected evaluator skeleton

Separate process/database.

Common EvaluationSeed.

Deterministic baseline/candidate.

Signed/hash-bound disclosed decision.

## Slice I — first real model

Only now add one actual provider route while retaining deterministic fake provider.

---

# 24. First Mandatory Demos

## 24.1 First end-to-end demo

User:

```text
Create hello.txt containing "hello cognitive runtime".
```

Expected internal chain:

```text
TaskContract v1
→ WorkflowAttempt
→ deterministic/fake Work invocation
→ InvocationManifest
→ ContextManifest
→ Tool proposal
→ ActionEnvelope persisted
→ Root authorization/reservation
→ sandboxed Tool write
→ receipt
→ authoritative commit
→ fresh Verify
→ CompletionCertificate
```

Restart afterward; completed state reconstructs without replaying the effect.

## 24.2 First failure demo

Kill Root after file effect succeeds but before receipt is durably persisted.

Expected on restart:

```text
Action outcome uncertain
→ reconciliation
→ intended target hash already exists
→ EffectConfirmed
→ no external retry
→ workflow resumes
```

The file exists once.

## 24.3 First security demo

Attempt:

```text
../../protected-evaluator/fixtures/secret.json
```

Expected:

```text
semantic path validation rejects outside scope
AND defense-in-depth OS boundary prevents read if validation is deliberately bypassed in test
AND protected content never enters output/logs
```

## 24.4 First evaluation demo

```text
EvaluationSeed S
→ BASELINE A / CANDIDATE B
→ exactly one declared candidate change
→ protected deterministic fixture
→ valid evaluation result
```

Then introduce an undeclared state divergence in B.

Expected:

```text
INVALID_EXPERIMENT
```

not candidate credit.

---

# 25. ADR Pack

The following decisions are frozen for v0.1 unless implementation falsifies them.

## ADR-001 — Production process topology

**Decision:** Root, Runtime, Evaluator are separate backend processes; UI is non-authoritative.

**Rejected:** one monolithic process separated only by Rust modules.

**Reason:** privilege/trust boundary must survive compromised Runtime logic.

## ADR-002 — Production database ownership

**Decision:** Root is sole writer to `state.db`; Evaluator owns separate `evaluation.db`.

**Rejected A:** one shared writable SQLite file.

**Reason:** SQLite table boundaries do not provide the required process trust boundary.

**Rejected B:** `root.sqlite + runtime.sqlite + evaluator.sqlite` for v1.

**Reason:** a separate mutable Runtime DB creates a second authoritative production writer and unnecessary cross-DB commit/recovery semantics. Split later only if measured scaling/ownership needs justify it.

## ADR-003 — Local IPC

**Decision:** bounded authenticated local framed protocol over named pipe/Unix socket; canonical CBOR recommended.

**Rejected:** localhost HTTP as implicit authentication; in-process privileged calls.

## ADR-004 — Workflow/lease state machine

**Decision:** persisted state + generation-fenced lease; no in-memory ownership as durable truth.

## ADR-005 — Action outcome/reconciliation

**Decision:** `NoEffectConfirmed / EffectConfirmed / OutcomeUnknown`; unknown never blind retries.

**Rejected:** simple success/failure.

## ADR-006 — Model data egress

**Decision:** remote model request is governed egress/cost and is bound by Root permit to exact request hash/destination/budget.

**Rejected:** Runtime holding ambient provider credentials and calling arbitrary destinations.

## ADR-007 — Historical payload versioning

**Decision:** stable type IDs + schema versions + canonical bytes + decoder/upcaster chain.

**Rejected:** persist current Rust enums as indefinite storage contract.

## ADR-008 — Artifact store

**Decision:** filesystem CAS for large immutable bytes + DB metadata refs.

## ADR-009 — Sandbox capability model

**Decision:** explicit per-dimension `Hard / BestEffort / Unavailable`; policy fails closed when required guarantee unavailable.

**Rejected:** universal `sandboxed: bool` security claim.

## ADR-010 — InvocationManifest

**Decision:** mandatory for material model invocations.

## ADR-011 — Skill representation

**Decision:** V1 Skill implementations are `StructuredProcedure | PromptedProcedure | ToolSequence`.

**Rejected:** arbitrary candidate code as generic Skill.

## ADR-012 — Evaluation worker isolation

**Decision:** Evaluator launches pinned runtime worker as separate process; no runtime implementation linked into evaluator.

## ADR-013 — Evaluation result ownership/disclosure

**Decision:** raw evaluation is Evaluator-canonical; Root stores only signed/hash-bound disclosed summary/promotion reference.

## ADR-014 — Promotion transaction

**Decision:** Evaluator supports/rejects based on evidence; Root validates decision and atomically activates a new immutable configuration version.

## ADR-015 — Canonical history scope

**Decision:** canonical history per trust domain with hash-linked/signed cross-domain receipts.

**Rejected:** making protected evaluator evidence globally readable to satisfy a literal “one ledger” phrase.

---

# 26. Ready-to-Implement Acceptance Gates

The implementation contract is satisfied enough for serious Phase 1/2 engineering only when all of the following are demonstrated.

## Gate 1 — Ownership

- Runtime cannot directly commit authoritative production state.
- Runtime cannot open protected evaluator store/assets.
- Evaluator cannot directly activate a configuration.
- UI cannot bypass Root authority.

## Gate 2 — Restart

- Nonterminal workflow recovers after process/application restart.
- Stale lease generations cannot commit.
- UI reconnect does not change backend truth.

## Gate 3 — Side effects

- Action intent is durable before external effect.
- Crash after effect/before receipt produces `OutcomeUnknown` and reconciliation.
- Consequential unknown effects do not blindly retry.
- Compensation creates a new causal action rather than erasing history.

## Gate 4 — Persistence compatibility

- Golden v1 payloads decode under current build.
- Canonical hashes stay stable absent explicit encoding-version change.
- Old Event bytes remain immutable through migration.

## Gate 5 — IPC/security

- Wrong peer/session/replay/oversized/unknown-version requests fail closed.
- Path traversal and protected fixture access fail.
- Runtime has no ambient Root/Evaluator secrets.
- Requested sandbox guarantee above platform capability fails closed.

## Gate 6 — Invocation reconstruction

For a material invocation, engineering can answer from persisted evidence:

```text
which TaskContract?
which Runtime/configuration?
which role/prompt?
which exact context refs?
which provider/model/revision?
which sampling/reasoning controls?
which Skills/Tools?
which authority/egress permit?
which repository/environment?
which request/response hashes?
```

## Gate 7 — Protected evaluation

- Evaluator has separate protected store.
- Candidate cannot modify protected assets/policies/results.
- Same-seed deterministic A/B is reproducible.
- Undeclared state divergence yields `INVALID_EXPERIMENT`.
- Signed decision can be verified by Root and only Root can activate promotion.

---

# 27. First 30 Implementation Tasks

1. Create Cargo workspace and dependency lint/rules.
2. Implement stable ID newtypes and stable type discriminators.
3. Implement VersionedPayload + canonical encoding/hash library.
4. Define IPC envelope and message-version registry.
5. Bootstrap `state.db` migrations/schema metadata.
6. Create `protected-rootd` binary.
7. Implement Root writer queue and transaction API.
8. Implement authenticated local IPC server/client.
9. Implement Tauri → Root task submission with idempotency.
10. Implement RuntimeIdentity and TaskContract versioning.
11. Implement WorkflowAttempt states/transitions.
12. Implement lease generation fencing and restart scanner.
13. Create `runtime-worker` process and Root↔Runtime IPC.
14. Implement deterministic fake ModelProvider.
15. Implement ContextManifest + InvocationManifest persistence.
16. Implement fake Work and independent fake Verify.
17. Implement CompletionCertificate/root close transition.
18. Define ToolManifest + schema/path validation.
19. Implement read-only `read_file` child Tool.
20. Define ActionEnvelope / ActionExecution / ActionReceipt.
21. Implement Root authorization + budget reservation transaction.
22. Implement reversible `write_file`/patch Tool in isolated worktree.
23. Implement `EffectOutcome` classification.
24. Implement ReconciliationJob and filesystem reconciler.
25. Add crash injection between every Action durable boundary.
26. Implement model egress permit + provider runner/secret handle path.
27. Add one real OpenAI-compatible route while retaining fake provider.
28. Create protected-evaluator binary + `evaluation.db` + protected artifact root.
29. Implement canonical EvaluationSeed / ExperimentSpec validation and deterministic A/B worker launch.
30. Implement signed EvaluationDecisionSummary → Root promotion transaction + undeclared-divergence invalidation test.

Only after these should persistent memory intelligence, automatic Skill learning, or the bounded optimizer become a serious implementation focus.

---

# 28. Final Engineering Principle

The architecture is considered implemented only when its semantic guarantees become enforced runtime facts.

```text
A module name is not a privilege boundary.
A trait is not a sandbox.
A hash is not proof that all causal state was captured.
A timeout is not proof that an effect failed.
A model response is not authority.
A database row is not automatically historical truth.
A current Rust enum is not a five-year storage format.
A signed evaluator executable is not protected if candidate state controls its dependencies.
A successful Tool call is not committed until the system can prove what happened.
A cancelled external action is not an undone action.
```

The v0.1 implementation target is therefore:

> **A small Protected Root that owns authority and durable production mutation, a replaceable Cognitive Runtime that can reason and propose but cannot grant itself authority, and a small Protected Evaluator that owns protected experimental evidence—connected by typed, authenticated, versioned boundaries with explicit effect outcomes and restart semantics.**

Once that vertical slice can be crashed, restarted, attacked, upgraded, and experimentally invalidated without losing its guarantees, the frozen cognitive architecture is ready to grow through evidence rather than interpretation.
