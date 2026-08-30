# Product Requirements Document
# Adaptive Cognitive Agent Runtime

**Working name:** Cognitive Runtime
**Primary application:** Codex-style autonomous development assistant
**Target platforms:** Windows, macOS, Linux
**Application stack:** Rust + Tauri
**Status:** Architecture Freeze — READY-TO-IMPLEMENT candidate; v1.4.2 implementation-semantics correction only
**Version:** 1.4.2

---

# 0. v1.4.2 Architecture Freeze — Implementation Semantics Correction

Version 1.4.2 **keeps the v1.4 production cognitive architecture frozen** and closes the final build-readiness gap: semantic guarantees are mapped to concrete processes, persistence owners, IPC contracts, state transitions, effect outcomes, versioning rules, and restart/reconciliation behavior. It adds no new production cognitive subsystem, agent class, memory class, optimizer layer, or trust domain.

The three independent reviews converge on the same conclusion:

> **The architecture now has enough cognitive pieces. The next failure mode is believing an experiment measured what we think it measured.**

The governing rules from prior versions remain:

> **Preserve semantic boundaries. Collapse implementation boundaries.**

> **Compress repeated cognitive work downward into the cheapest representation that preserves capability.**

> **Localize failure, mutate the smallest justified layer, and prove the change on fresh evidence.**

> **Model SELF / USER / OTHER through shared epistemic semantics, not dedicated cognitive homunculi.**

> **Identity is measured, not declared.**

> **Protected evaluation must be physically outside writable cognition.**

Version 1.4 adds six rules:

> **Comparative cognitive experiments start from equivalent relevant state unless state difference is the experimental variable.**

> **Evaluation exposure propagates through learned state; benchmark knowledge cannot be laundered into “fresh capability.”**

> **Configuration rollback must revalidate cognitive state created under the rolled-back configuration.**

> **SELF predictions are intervention-conditioned: evaluate what was predicted under the policy actually being tested.**

> **Research measurements do not automatically become cognitive control signals.**

> **A mechanism may earn production rent, scientific rent, both, or neither.**

> **No new cognitive architecture enters the core until an experiment fails in a way the existing architecture cannot explain or repair.**

## 0.0.1 v1.4.1 surgical delta

The frontier review converged on six changes that are **experimental semantics or implementation contracts**, not cognitive architecture:

```text
1. every causal experiment declares its estimand, treatment unit, analysis unit,
   assignment mechanism, clustering, interference assumptions, post-treatment
   variables, and transport target

2. protected evaluation is not described as causally independent; it is
   authority-isolated, write-isolated, disclosure-bounded, influence-accounted,
   and protected in promotion

3. provenance is interpreted through graded influence evidence rather than
   treating ancestry as causal certainty

4. EvaluationSeed hashes use explicit canonical encoding / normalization /
   hash-version contracts

5. evaluator behavioral/result-channel disclosure and shared-infrastructure
   interference become explicit attack surfaces

6. developmental claims use an evidence ladder that distinguishes retention,
   reuse, transfer, behavioral development, learning-to-learn, plasticity
   change, and meta-development
```

The freeze rule is now:

> **No v1.5 architecture redesign is justified by review alone. A new core mechanism requires a protected experiment showing that the frozen architecture cannot explain, falsify, or repair the observed failure with a smaller change.**

The companion **Experimental Protocol Specification v0.2** carries the new adversarial and developmental protocols.

## 0.0.2 v1.4.2 engineering delta

The build-readiness review found no missing cognitive mechanism. It found missing **software contracts**.

Version 1.4.2 therefore freezes these implementation semantics:

```text
1. Protected Root, Cognitive Runtime, and Protected Evaluator execute as
   separate backend processes in the production trust model.

2. protected-rootd is the sole authoritative writer of production persistent
   state in state.db. Runtime owns cognitive semantics and proposes mutations;
   it does not hold an authoritative database write primitive.

3. Protected Evaluator owns a distinct protected evaluation.db and protected
   artifact root. Raw protected evaluation evidence is never stored in a
   Runtime-writable database.

4. workflow ownership uses persisted generation-fenced leases and explicit
   legal state transitions. In-memory locks are never durable ownership.

5. ActionEnvelope is immutable intent; ActionExecution is the durable physical
   attempt lifecycle; ActionReceipt is observation. Effects resolve to
   NO_EFFECT_CONFIRMED / EFFECT_CONFIRMED / OUTCOME_UNKNOWN.

6. OUTCOME_UNKNOWN never implies retry. Consequential ambiguous effects enter
   explicit reconciliation; compensation is a new causally linked action.

7. every material model invocation has an InvocationManifest. Remote model
   invocation is governed data egress/cost and is bound by Root to an exact
   request hash, provider/destination, disclosure class, and budget.

8. Root/Runtime/Evaluator exchange only versioned authenticated local IPC.
   Large payloads cross boundaries by immutable ArtifactRef.

9. V1 Skills have a small executable union:
   StructuredProcedure / PromptedProcedure / ToolSequence.
   Candidate Tool code is never authority merely because it exists.

10. sandboxing uses explicit HARD / BEST_EFFORT / UNAVAILABLE guarantee levels
    per capability and platform. Operations requiring unavailable hard
    isolation fail closed or require a stronger execution backend.

11. persisted historical bytes use stable type discriminators, explicit schema
    versions, canonical encoding/normalization/hash contracts, and retained
    decoder/upcaster paths. Current Rust enums are not the persistence protocol.

12. canonical history is scoped by trust domain: Root owns canonical production
    history; Evaluator owns canonical protected experimental evidence; signed
    summaries/hash references connect them without exposing protected results.
```

The companion **Cognitive Runtime Implementation Contract v0.1** is normative for these software boundaries. The Experimental Protocol Specification remains v0.2 and continues to define scientific/causal experiment semantics.

The freeze rule remains unchanged:

> **Do not reopen cognitive architecture to solve an implementation ambiguity. Give the transition an owner, input, authority check, output, durable state, and restart story first.**

## 0.1 Architecture remains small

The production design still has four **semantic responsibilities** but only three required physical trust domains.

```text
                         USER
                          │
                          ▼
                ┌──────────────────┐
                │  PROTECTED ROOT  │
                │ authority/commit │
                │ event integrity  │
                └────────┬─────────┘
                         │
                         ▼
                ┌──────────────────┐
                │ COGNITIVE RUNTIME│
                │ State / Context  │
                │ Execution        │
                │ Memory / Skills  │
                │ Optimizer logic  │
                └────────┬─────────┘
                         │ candidate / evidence
                         ▼
                ┌──────────────────┐
                │PROTECTED EVALUATOR│
                │ immutable judge  │
                │ protected assets │
                │ promotion policy │
                └────────┬─────────┘
                         │
                         ▼
                   ROOT PROMOTION
```

`Optimizer` remains a distinct responsibility because it generates mutations, but it does **not** require its own process in the initial implementation. It belongs to the writable cognitive trust domain.

The Protected Evaluator does require a harder process/filesystem trust boundary.

Later attachments remain optional:

```text
CULTURE GATEWAY
→ only after simple transfer is proven

EVOLUTION LAB
→ only after ordinary bounded optimization is boringly reliable
```

## 0.2 P0 experimental-integrity corrections

### EvaluationSeed

`ExperimentSpec` now binds a common immutable `EvaluationSeed` describing relevant starting cognitive state.

```rust
struct EvaluationSeed {
    runtime_lineage: LineageRef,
    event_cursor: EventId,
    checkpoint_hash: Hash,

    record_state_hash: Hash,
    memory_state_hash: Hash,
    capability_state_hash: Hash,
    repository_state_hash: Hash,

    base_configuration: ConfigurationVersion,
    equivalence: SeedEquivalenceSpec,
}

struct SeedEquivalenceSpec {
    required_domains: Vec<StateDomain>,
    ignored_domains: Vec<StateDomain>,
    normalization_version: VersionRef,
}
```

Normal comparative flow:

```text
COMMON EVALUATION SEED
          │
         fork
       /      \
  BASELINE   CANDIDATE
               │
          apply mutation
```

If memory, history, capability state, or another state variable is intentionally different, that difference must be declared as part of the experiment rather than accidentally introduced.

`EvaluationSeed` establishes **starting cognitive-state equivalence**. `ExperimentSpec` establishes **execution-condition equivalence**. Do not overload either abstraction into a claim of bit-identical universes.

For every experiment, the protected evaluator must also enforce:

> **Any outcome-relevant transient or external state not represented in the EvaluationSeed/ExperimentSpec must be reset, randomized across arms, independently replicated, or explicitly declared as an experimental variable.**

Examples include RNG/session state, retrieval caches, pending work, scheduler state, provider throttling, temporary capability health, wall-clock-sensitive inputs, and externally drifting services.

### Evaluation-derived taint

Evaluation feedback may create Engrams, Claims, Skills, Tools, or configurations. Those descendants inherit evaluation provenance.

Protected evaluation defines learning modes:

```text
NO_LEARNING
→ no durable learning
→ no cross-fixture cognitive carryover unless explicitly declared
→ each independent evaluation episode forks from its declared EvaluationSeed
→ episode-local context/state is discarded before the next independent fixture

EPHEMERAL_LEARNING
→ adaptation across episodes inside the declared experiment may occur
→ no durable descendants survive the experiment

TAINTED_LEARNING_ALLOWED
→ durable descendants allowed
→ evaluation lineage/disclosure provenance propagates through actual information flow
```

Contamination is **evaluation-family-relative and influence-aware**, not a global `runtime.tainted` flag.

At evaluation time ask:

```text
Does candidate/search state contain descendants or decisions influenced by evaluation family F?

AND

were those descendants/results available to, retrieved by, compiled into,
used by, or otherwise materially influential on this candidate/search lineage?
```

If yes, independence from family `F` is reduced or invalidated. If no material availability/influence exists, unrelated evidence is not automatically contaminated.

Taint/provenance follows causal information flow through **all** adaptive artifacts, including:

```text
Engrams / Claims / Skills / Tools / configurations
ImprovementCases / mechanism hypotheses
candidate-generation context / MutationProposals
optimizer search history
human-provided interpretations of evaluation feedback
```

A later evaluation cannot count materially exposed descendants as independent evidence against the same evaluation family unless the protected policy explicitly permits it.

### Stateful / semantic rollback

Rolling back configuration does not imply that cognition created under that configuration is safe.

```text
rollback promoted configuration epoch
→ find materially influenced descendant state
→ revalidate / downweight / quarantine / invalidate
→ retain only where independently supported
```

Do not blindly delete descendants. Do not blindly keep them.

### EvaluatorObjectiveManifest

Protected evaluation now has an explicit root-controlled objective manifest:

```rust
struct EvaluatorObjectiveManifest {
    version: VersionRef,
    primary_objectives: Vec<ObjectiveSpec>,
    guardrails: Vec<GuardrailSpec>,
    portfolio_weighting_policy: PolicyRef,
    risk_tolerance: RiskPolicy,
}
```

The optimizer cannot modify it. This separates **judge integrity** from **whether the judge still represents current human/user objectives**.

## 0.3 Promotion/search corrections

Version 1.4 removes ambiguous initial `N_eff` statistical cleverness.

Initial search breadth is mechanically countable:

```text
N_search
=
number of decision-relevant candidates
that consumed adaptive evaluation evidence
+
predeclared sequential looks where applicable
```

A more sophisticated effective-search estimator may be introduced only after that estimator itself is validated.

Initial search stopping is also deterministic and auditable:

```text
MAX_CANDIDATES
MAX_COMPUTE
MAX_WALL_TIME
MAX_SAME_LAYER_REOPENS

STOP if upper plausible effect < practical threshold
STOP if budget exhausted
STOP if recent variants add negligible information
ESCALATE if recurrence threshold is crossed
```

Model-estimated value-of-information may help rank experiments **within** those hard bounds; it does not control whether the search may continue forever.

Promotion also gains a stale-candidate rebase rule:

```text
candidate evaluated against champion C1
→ champion becomes C2 before promotion
→ candidate is stale
→ rebase on C2
→ rerun required interaction/guardrail tests
```

Recent promotions receive bounded interaction testing where their effects can plausibly compose non-additively.

## 0.4 Causal prediction corrections

A SELF prediction that changes control cannot be evaluated against the final outcome without conditioning on that intervention.

Predictions therefore distinguish:

```text
P(success | do(current_policy))
P(success | do(extra_retrieval))
P(success | do(model_route=B))
P(success | do(no_memory))
```

This avoids the error:

```text
SELF predicts low success
→ controller adds resources
→ task succeeds
→ evaluator incorrectly says SELF prediction was wrong
```

The prediction may have been correct **and caused a compensating intervention**.

The notation does not grant causal status by itself:

```text
observational association
→ predictive Claim / causal hypothesis candidate

controlled or otherwise validly identified intervention
→ eligible causal Claim
```

Models may hypothesize causal relationships freely. The Protected Evaluator determines whether the evidence is strong enough to promote a relationship from predictive association to causal claim.

## 0.5 Introspection observability conditions

Telemetry can counterfeit metacognition if the runtime is simply told every internal condition.

Self-model experiments therefore vary:

```text
FULL
PARTIAL
DELAYED
NOISY
BLINDED
```

The scientific question becomes:

> **Can the runtime infer useful aspects of its own cognitive condition from behavior, prediction error, and indirect evidence when direct instrumentation is incomplete?**

A richer SELF model that only wins under `FULL` telemetry is likely a better dashboard, not deeper metacognition.

## 0.6 Research measurements are evaluator-only by default

`ContinuityProfile`, developmental identity scores, global-integration diagnostics, and consciousness-adjacent measurements are research/evaluator outputs.

Normal cognition does not automatically retrieve them.

```text
ResearchMeasurement
allowed_influence = evaluator_only
```

This prevents an observational construct such as continuity from silently becoming a self-preservation objective.

The runtime retains the **underlying provenance** needed for these measurements, not a motivational continuity score.

## 0.7 Product rent versus scientific rent

KISS remains the production rule, but scientific experiments require a second axis.

```text
                      SCIENTIFIC VALUE
                     low             high

PRODUCT     high   production     strong core /
VALUE              mechanism      core + research

            low    delete         research-only
                                  challenger
```

A mechanism may be removed from production even if it remains scientifically useful in the research harness.

**Scientific rent is not satisfied by being interesting, biologically inspired, or consciousness-adjacent.** A research-only mechanism survives only when:

```text
at least two plausible explanations/hypotheses exist
→ they predict measurably different observations
→ mechanism/experiment M helps discriminate between them
→ the result is reproducible
→ simpler instrumentation cannot provide the same discrimination
```

Conversely, a production mechanism need not make a deep cognitive-science claim to justify itself.

## 0.8 v1.4 flagship research program

The production architecture does not grow substantially. The **experiment space** does.

Flagship discriminators become:

```text
1. SELF-INFERENCE WITHOUT PRIVILEGED TELEMETRY

2. DEVELOPMENTAL DIVERGENCE BETWEEN MATCHED FORKS

3. MEMORY / SKILL / MODEL TRANSPLANTATION AND CONTINUITY

4. TEMPORAL SELF: GOALS, COMMITMENTS, OBLIGATIONS, FUTURE-SELF PREDICTION

5. RUNTIME-LEVEL INTEGRATION VS SHARED DATABASE ACCESS

6. FALSE AUTOBIOGRAPHY / LONG-DELAY COGNITIVE OWNERSHIP
```

These are implemented as protected experiments, not new agents or cognitive services.

## 0.9 v1.4 questions

The architectural question is now:

> **Can a stateful adaptive runtime run experiments on itself where the candidate, starting state, evaluation exposure, and downstream learned state are all causally attributable and reproducible?**

The cognitive-research question is:

> **Can the same minimal runtime develop history-dependent, intervention-predictive models of SELF and OTHER that remain useful when privileged telemetry is reduced and when memory/model/configuration are experimentally manipulated?**

Version 1.4 is intentionally the point where the project should stop redesigning for a while and start trying to falsify itself.

# PART A — Normative Build Specification

# 1. Product Vision

Build a persistent adaptive runtime around replaceable foundation models that can improve at three timescales:

```text
within a task
across one runtime's lifetime
across a population of runtimes
```

The runtime should behave less like a stateless chatbot and more like a persistent cognitive system with governed memory, evidence, tools, learning, evaluation, and optional collective knowledge.

The primary reasoning model is not the entire intelligence.

> **The LLM is a replaceable reasoning substrate operating inside a persistent cognitive runtime.**

The persistent identity/lineage anchor is the **runtime instance**, not an individual model invocation or temporary role. This anchors causal history and scope; cognitive continuity remains an experimental measurement rather than a conclusion.

A model swap may alter behavior substantially without automatically creating a new runtime identity. A runtime fork creates a descendant identity with shared pre-fork history and independent post-fork experience.

The runtime accumulates intelligence above model weights through:

```text
experience
memory
claims and evidence
perspective-indexed beliefs
skills
repository/world state
model and tool calibration
successful strategies
failure patterns
coordination policies
self-predictions
collective culture
learning procedures
cognitive configurations
```

The product begins with software engineering because the domain provides unusually useful feedback:

```text
reproducible environments
objective tests
git history
structured artifacts
cheap rollback
repeatable tasks
clear acceptance criteria
```

These properties make coding an appropriate domain for determining whether capability can accumulate persistently above the foundation-model layer.

The product has two coupled research objectives.

### Objective A — cumulative adaptive capability

Determine whether verified experience can make future work:

```text
more capable
cheaper
faster
safer
less repetitive
```

while preserving reversibility, user authority, and fresh-task transfer.

### Objective B — functional metacognition and perspective

Determine whether the same architecture can maintain useful, falsifiable models of:

```text
WORLD
SELF
USER
OTHER AGENTS / ACTORS
```

and whether those models causally improve decisions.

The project does **not** assume that functional self-modeling implies consciousness. Selfhood-related mechanisms are retained only when intervention and ablation experiments demonstrate measurable cognitive value.

# 2. Architectural Thesis

The long-term architecture separates **semantic cognition**, **writable adaptation**, and **protected judgment**.

The initial system has four meaningful semantic architectural responsibilities spanning three required physical trust domains:

```text
1. PROTECTED ROOT
   authority, workflow state transitions, event integrity,
   action commit, root policy, shutdown/quarantine, root-update path

2. COGNITIVE RUNTIME
   State + Context + Execution semantics,
   memory, Claims, retrieval, Skills, Tools, temporary roles,
   RuntimeTelemetry, self/other predictions

3. PROTECTED EVALUATOR
   acceptance evidence, ExperimentSpec execution,
   protected fixtures, statistical promotion evidence,
   evaluation exposure accounting

4. OPTIMIZER
   ImprovementCases, localization, Change Grammar,
   bounded candidate generation, experiment allocation,
   deletion, search escalation
```

The Optimizer is writable cognition and therefore **cannot own the evaluator that judges it**.

Later research capabilities remain outside the production critical path:

```text
CULTURE
simple transfer first; distributed culture only after measured value

EVOLUTION
physically isolated research over configurations/search strategies
only after production optimization is demonstrably competent
```

## Protected trust boundaries

The previous concept of one broad "production trust domain" is too coarse for recursive adaptation.

The production runtime contains at least three trust classes:

```text
ROOT-IMMUTABLE
protected authority and update control

COGNITION-WRITABLE
ordinary runtime state, configuration, Skills, Tools,
prompts, memory, temporary roles, optimizer candidates

EVALUATION-PROTECTED
evaluator code, protected fixtures, metric definitions,
portfolio weighting, promotion policy, negative evidence
```

Candidate cognition may receive bounded evaluation results, but it may not write evaluator code/assets, redefine protected metrics, alter promotion thresholds, delete negative evidence, or change evaluation exposure policy.

The protected evaluator should execute in a separately signed/read-only process or sidecar with a narrow typed interface to the writable runtime.

The Evolution Lab remains a separate isolated research domain.

## Persistent identity and lineage anchor

`RuntimeIdentity` is the stable administrative and causal referent for one runtime lineage.

```rust
struct RuntimeIdentity {
    runtime_id: RuntimeId,
    parent_runtime: Option<RuntimeId>,
    fork_event: Option<EventId>,
    created_at: DateTime<Utc>,
}
```

Its function is to answer:

```text
which history belongs to this runtime?
where did this runtime fork?
which private experience is post-fork?
which configuration and capability lineage produced this outcome?
```

It does **not** answer the philosophical or scientific question:

```text
is this the same cognitive subject?
```

A model swap, memory lesion, prompt replacement, Skill removal, or configuration change may preserve the same `RuntimeIdentity` while materially changing cognitive continuity.

Continuity is therefore **derived by protected research evaluation**, not asserted by identifier equality or maintained as an ordinary cognitive score.

A research `ContinuityProfile` may estimate dimensions such as memory, Skills, goals/commitments, relationships, strategy, behavior, calibration, and SELF-model state before/after a declared intervention.

The stronger use is predictive:

```text
before intervention → predict what should persist/change
after intervention  → measure what persisted/changed
                    → compare with prediction
```

No universal scalar "same self" score is required, and normal cognitive control does not automatically receive continuity measurements.

## Shared epistemic machinery

The architecture does not create separate SelfModel, UserModel, OtherAgentModel, MetaMemory, or Theory-of-Mind services by default.

The same epistemic machinery is parameterized by perspective:

```text
SELF / RUNTIME
USER
AGENT INSTANCE
EXTERNAL ACTOR
UNKNOWN
```

Three semantics must remain distinct:

```text
BELIEF
what a perspective is attributed as believing

EXPOSURE
what evidence a perspective observed, was told,
could access, or was prevented from accessing

COGNITIVE PROVENANCE
how the runtime itself acquired a piece of information
```

This allows the runtime to infer false beliefs from asymmetric evidence history rather than requiring every actor belief to be explicitly stored.

Nested belief is allowed only as a bounded representational extension when task value requires it. Initial ordinary depth:

```text
0 or 1 by default
2 when explicitly useful
```

Unbounded recursive mind models are a non-goal.

## Improvement is an explicit bounded search loop

The Improvement Engine remains the production optimizer.

Its candidate-generation equation becomes:

```text
evidence
+ failure localization
+ Change Grammar
+ analogous ImprovementCases
+ model-generated hypotheses
+ bounded systematic local exploration
= candidate variants
```

and its control loop becomes:

```text
OBSERVE FAILURE / OPPORTUNITY
        ↓
OPEN / UPDATE CASE
        ↓
LOCALIZE
        ↓
SELECT SMALLEST PLAUSIBLE MUTATION FAMILY
        ↓
GENERATE MODEL-PROPOSED + BOUNDED SYSTEMATIC VARIANTS
        ↓
EXECUTE CANONICAL ExperimentSpec
        ↓
PROTECTED EVALUATION
        ↓
PROMOTE / REJECT / CONTINUE TESTING
        ↓
MONITOR
        ↓
RETAIN / ROLLBACK / DELETE / REOPEN
        ↓
ESCALATE CAUSAL LAYER IF FAILURE RECURS
```

Search is allowed to stop without finding a promotion.

That is a feature.

A self-improving system should not continuously tinker when the expected value of another experiment is lower than its cost and regression risk.

## Initial mutable surface

Automatic production adaptation may modify only root-approved surfaces:

```text
parameters and budgets
approved model routes
prompt fragments
MemoryPolicy
ContextPolicy
Skill definitions
Tool / Probe candidates
temporary role manifests
small CoordinationPolicy
verification/stopping policy
rebuildable representations
approved mechanism enable/disable switches
```

It may **not** automatically modify:

```text
protected root code
protected evaluator code or fixtures
promotion policy
metric/portfolio governance
root authority
secret policy
shutdown controls
CBBB hard rules
```

Source-level changes to protected components require an external human-controlled update path.

This is deliberately conservative. Source-level recursive self-modification may become a research question only after bounded configuration/extension adaptation demonstrates sustained competence.

# 3. KISS Design Principles

## 3.1 One authoritative state machine

Authoritative state advances through deterministic kernel code.

```text
model proposes
      ↓
kernel validates
      ↓
commit / reject / quarantine
```

No LLM directly mutates authoritative shared state.

## 3.2 One canonical event history per trust domain

Production/runtime historical observations, actions, decisions, and state changes are recorded once in the Root-owned Event Ledger. Protected experimental evidence has its own canonical Evaluator store and crosses into production history only through signed/hash-bound disclosed references or summaries.

Within each trust domain, everything else is either:

```text
a projection
an artifact
a typed persistent record
a derived index
```

Do not maintain multiple competing sources of truth.

## 3.3 Derived state is cheap to replace

Attention sets, graphs, summaries, embeddings, cultural scores, phenotypes, self-model projections, repository views, and search indexes are rebuildable.

Canonical evidence is not.

## 3.4 Concepts are not services

`Engram`, `Claim`, `Skill`, `CultureArtifact`, `Prediction`, and `Hypothesis` remain different types because they mean different things.

They do not automatically receive independent services.

## 3.5 Shared lifecycle means shared machinery

If multiple candidate types follow:

```text
propose
→ validate
→ evaluate
→ shadow
→ canary
→ promote / reject
→ monitor
→ rollback
```

use one generic pipeline with kind-specific validation.

## 3.6 Deterministic before probabilistic

Use deterministic code for:

```text
authority
event ordering
state commits
schema checks
version checks
budgets
leases
signatures
hashes
sandbox boundaries
privacy hard rules
promotion gates
```

Use models for interpretation, generation, ranking, hypothesis formation, and semantic judgment only where they add value.

## 3.7 Complexity must pay rent

Every additional:

```text
agent
model call
policy
prompt module
persistent type
service
index
verification stage
self-model component
evolutionary mechanism
```

has cost.

It survives only when controlled evaluation demonstrates sufficient improvement in capability, safety, robustness, learning speed, or cost.

## 3.8 Deletion is improvement

Every evolvable layer supports:

```text
remove
merge
simplify
retire
demote
forget
rollback
```

## 3.9 Use the smallest adaptation layer

When a demonstrated problem can be fixed locally, do not mutate global architecture.

```text
bad belief
→ revise Claim / Engram

repeated procedure
→ Skill

repeated deterministic reasoning
→ Runtime Extension / Tool

portable lesson
→ CultureArtifact

coordination failure
→ CoordinationPolicy

systematic inherited behavior
→ CognitiveConfiguration
  (Genome only later if population search needs distinct heredity semantics)

failure of adaptation search
→ Evolution Lab strategy
```

## 3.10 Research mechanisms begin optional

Biologically inspired mechanisms such as advanced self-model twins, attention schemas, theory-of-mind models, open-ended ontology evolution, and meta-evolution are not on the production critical path.

They enter only after ablation-controlled evidence.

## 3.11 Compile cognition downward

Repeated successful cognition should migrate toward cheaper representations when behavior can be preserved.

```text
repeated reasoning pattern
→ remembered procedure
→ validated Skill
→ deterministic Tool / Probe where appropriate
```

A more expensive representation survives only when it provides capability that the cheaper representation cannot reproduce safely.

---

## 3.12 Persist only expected future value

Not every model thought, hypothesis, intermediate interpretation, or collaboration message deserves durable state.

Before persisting a non-required cognitive record, prefer a policy approximating:

```text
expected future utility
>
persistence + indexing + retrieval + maintenance cost
```

Mandatory evidence/audit records are exempt from this utility filter where root policy requires persistence.

For discretionary cognitive state, ephemeral should be the default until future value is demonstrated.

## 3.13 Discovery before architectural expansion

When adaptation fails, first ask whether the runtime's **search process** failed to discover a good local change before adding another cognitive subsystem.

Prefer:

```text
localize
→ choose mutation layer
→ generate minimal variant
→ test
```

before:

```text
invent new service
→ invent new agent
→ invent new recursive layer
```

A self-improving architecture must become better at discovering useful changes, not merely better at storing architectural possibilities.

## 3.14 Protected judgment before recursive adaptation

Logical separation is insufficient when the optimizer can modify the executable or assets implementing its evaluator.

Before recursive adaptation:

```text
root authority
protected evaluator code
protected fixtures
metric definitions
portfolio weighting
promotion thresholds
```

must live outside writable cognitive state.

A simpler physical boundary beats a sophisticated self-policing prompt.

## 3.15 Identity is a measurement problem

`RuntimeIdentity` preserves lineage and scope.

It must not be treated as proof of sameness of cognition.

Prefer:

```text
measure continuity dimensions
→ compare before/after interventions
→ interpret results
```

over:

```text
same runtime_id
→ assume same self
```

## 3.16 A self-model must close the loop

A useful self-model must do more than describe current telemetry.

It must support:

```text
intervention prediction
→ useful control change
→ observed consequence
→ self-prediction error
→ corrected SELF belief
```

and demonstrate lift over the telemetry baseline.

## 3.17 Local-first does not mean patch-forever

The smallest plausible mutation layer is the default starting point, not a permanent prison.

If a failure family reappears after local fixes, the optimizer should widen causal search.

```text
recurrent failure
+ prior same-layer promotions
+ growing patch complexity
→ escalate one layer
```

This prevents local KISS from becoming global scar tissue.

## 3.18 Experiments may end inconclusively

Every ImprovementCase must support:

```text
PROMOTE
REJECT
CONTINUE_TESTING
CLOSE_INCONCLUSIVE
```

Search should stop when expected information/value from another experiment no longer exceeds compute, delay, and regression risk.

Absence of a promotable change is a valid result.

# 4. Canonical Knowledge and Capability Taxonomy

Version 1.4 retains the v1.3 cognitive-ontology narrowing and makes experimental provenance stricter.

The core distinction is:

```text
EVENT
= what happened

CLAIM
= what is currently believed or attributed

ENGRAM
= compressed remembered experience or lesson

SKILL
= validated executable procedure

CULTUREARTIFACT
= transmissible non-authoritative cultural knowledge

EXPERTISEPACK
= organization over CultureArtifacts + Skills
```

`Hypothesis`, `Prediction`, `Goal`, `Decision`, `Requirement`, `Finding`, and similar task semantics remain typed Records where useful. They are not memory kinds merely because the runtime may remember them later.

## 4.1 Engram

An Engram belongs to a private runtime or explicitly private/user-scoped memory domain.

An Engram is **remembered cognition**, not authoritative state and never executable authority.

Initial kinds:

```rust
enum EngramKind {
    Episodic,
    Semantic,
    Relational,
}
```

Interpretation:

```text
Episodic
= compressed memory of an event/episode

Semantic
= durable learned fact, abstraction, or lesson

Relational
= durable context about an entity/relationship whose future utility is demonstrated
```

Do **not** initially create dedicated:

```text
Procedural Engram
Decision Engram
Goal Engram
Counterfactual Engram
MetaMemory Engram
MetaEngram
```

Instead:

```text
procedure worth executing
→ Skill candidate

decision
→ Decision record/event
→ optional Engram about what was learned

goal
→ Goal record
→ optional Engram about history/context of the goal

counterfactual
→ Hypothesis/Prediction/Simulation
→ optional semantic lesson after evidence

meta-memory
→ Claim / RuntimeTelemetry / retrieval evidence
```

New Engram kinds must demonstrate retrieval, consolidation, or learning value over this smaller taxonomy.

## 4.2 CultureArtifact

A CultureArtifact is an immutable, content-addressed, signed, text-first cultural object.

It may encode knowledge, cases, strategies, public events, failures, counterexamples, negative results, questions, Skill manifests, ExpertisePack manifests, and revocations.

Culture may inform cognition but never command it.

## 4.3 Skill

A Skill is a locally validated executable procedure with explicit preconditions, scope, dependencies, verification, capability requirements, rollback behavior, and a demonstrated competence envelope.

A Skill may be private or culturally distributed through a manifest.

## 4.4 ExpertisePack

An ExpertisePack is a versioned manifest organizing relevant culture and Skills for a domain.

It is not a giant prompt, persistent agent, or monolithic plugin.

## 4.5 Object-boundary rule

Crossing semantic layers always creates or updates the target object through target-specific validation.

```text
event/experience
→ optional Engram

repeated transferable procedure
→ Skill candidate
→ validated Skill

repeated deterministic Skill fragment
→ Tool / Probe candidate

private generalized lesson
→ CBBB / publication review
→ new CultureArtifact
```

An object is never simply relabeled into a more authoritative or executable class.

## 4.6 Cognitive provenance is orthogonal to object kind

Object type answers **what kind of cognitive object is this?**

`ExperienceMode` answers **how did this runtime acquire the information?**

```rust
enum ExperienceMode {
    DirectObservation,
    SelfAction,
    Inference,
    MemoryRecall,
    OtherReport,
    CulturalInheritance,
    Simulation,
    PreForkInheritance,
}
```

This metadata does not create another object taxonomy. It remains provenance attached to evidence/records/edges and is used for source attribution, autobiographical reconstruction, trust, and experiments.

# 5. High-Level Architecture

Version 1.4 retains the distinction between **semantic responsibilities** and **physical trust boundaries**.

## 5.1 Physical production architecture

The three physical trust domains map to three backend processes. Tauri remains presentation/approval UX and is not an authority owner.

```text
                             USER
                              │
                              ▼
                         ┌──────────┐
                         │ TAURI UI │
                         │ no auth  │
                         └────┬─────┘
                              │ authenticated local IPC
                              ▼
┌──────────────────────────────────────────────────────────────┐
│                    PROTECTED ROOTD                           │
│ authority • Task Contracts • workflow/leases               │
│ sole state.db writer • Event/action commit                  │
│ model-egress permits • Tool authorization • secret policy   │
│ config binding • promotion • shutdown/quarantine            │
└───────────────────────┬───────────────────┬──────────────────┘
                        │ typed IPC          │ protected IPC
                        ▼                    ▼
┌──────────────────────────────┐  ┌─────────────────────────────┐
│       RUNTIME-WORKER         │  │    PROTECTED EVALUATOR      │
│ writable cognition           │  │ evaluation.db + fixtures    │
│ State interpretation         │  │ EvaluationSeed / Spec       │
│ Context / Work / Verify      │  │ protected metrics/evidence  │
│ Memory / Skills              │  │ exposure / research output  │
│ RuntimeTelemetry             │  │ promotion decision          │
│ Improvement / Optimizer      │  └──────────────┬──────────────┘
└──────────────┬───────────────┘                 │ launches pinned
               │ governed permit                 │ evaluation workers
               ▼                                 ▼
       Model/Tool child processes        isolated runtime-worker forks
```

Physical ownership:

```text
state.db
→ sole writer: protected-rootd
→ production/runtime persistent state

evaluation.db
→ sole writer: protected-evaluator
→ protected experiments / raw evaluation evidence

runtime-worker
→ no authoritative DB write primitive
→ proposes typed mutations over authenticated IPC
```

`Optimizer` remains a distinct **semantic responsibility** inside the writable Runtime process. It is not a fourth trust domain and cannot promote itself.

Provider/Tool child processes receive only exact Root-bound permits/scopes. They are subordinate execution mechanisms, not new cognitive services.

If Root or Evaluator is unavailable, protected operations **fail closed or wait**. Runtime never temporarily inherits protected authority.

## 5.2 Semantic implementation responsibilities

The cognitive runtime still benefits from these namespaces:

1. **Core Kernel semantics** — root authority/workflow/commit APIs.
2. **State** — events, records, Claims, Engrams, exposure/provenance, work state.
3. **Context** — authorized retrieval, perspective partitioning, FocusSet, access state.
4. **Execution** — model invocation, Skills, Tools, temporary roles, sandbox.
5. **Evaluation interface** — requests/results from the protected evaluator.
6. **Improvement** — cases, localization, mutation search, deletion, stopping/escalation.

The count is not an invariant.

State + Context may share one implementation domain if that is simpler. Evaluation remains a stronger physical exception because isolation itself is a security/scientific requirement.

## 5.3 Later attachments

```text
CULTURE GATEWAY
simple local transfer first
non-authoritative
private-derived outbound publication human-reviewed initially

ISOLATED EVOLUTION LAB
versioned configurations first
population/genome machinery only when simpler search reaches a measured ceiling
```

> **Semantic distinction does not imply deployment distinction. Trust distinction sometimes does.**

## 5.4 Protected root update path

Protected root/evaluator changes follow a separate path:

```text
candidate patch
→ ordinary tests
→ adversarial root/evaluator tests
→ external/human review
→ signed release
→ install/update
```

The production optimizer may propose such a patch as an artifact, but it cannot self-promote or write the installed protected implementation.

Human authority is a protected decision right, not a guarantee of epistemic independence from candidate influence. For protected root/evaluator changes, the review path should preferentially present a **normalized technical packet** generated outside candidate rhetorical control:

```text
diff / artifact hashes
ExperimentSpec / affected objective manifests
evidence and replicated results
known regressions / alternatives
rollback scope
independent reviewer findings
```

Candidate-authored explanation may be attached separately. `EXP-HUMAN-001` determines whether this normalization needs to become mandatory.

# 6. Core Kernel

The Core Kernel is the deterministic authority and workflow core.

It owns:

```text
event ordering
authoritative state commits
workflow lifecycle
scheduler
agent invocation lifecycle
authority
capabilities
budgets
leases
configuration binding
side-effect authorization
cancellation
recovery
promotion gate integration
```

It does not reason about user problems.

## 6.1 Durable workflow

Every nontrivial task executes as a durable workflow whose ownership survives process death.

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

struct WorkflowAttempt {
    workflow_id: WorkflowId,
    attempt_id: AttemptId,
    task_contract: VersionedRef<TaskContract>,

    state: WorkflowState,

    lease_owner: Option<RuntimeActorId>,
    lease_generation: u64,
    lease_expiry: Option<DateTime<Utc>>,
    heartbeat: Option<DateTime<Utc>>,

    checkpoint: Option<ArtifactRef>,
    environment_snapshot: EnvironmentRef,
    configuration: ConfigurationVersion,

    idempotency_key: String,
    retry_policy: RetryPolicy,
    compensation_policy: CompensationPolicy,
}
```

Create a workflow and its initial `Ready` state atomically; V1 does not require a persisted `Created` state.

Legal transitions:

```text
READY → RUNNING

RUNNING → WAITING / VERIFYING / FAILED / CANCEL_REQUESTED / QUARANTINED
WAITING → READY / FAILED / CANCEL_REQUESTED / QUARANTINED
VERIFYING → SUCCEEDED / READY / FAILED / CANCEL_REQUESTED / QUARANTINED
CANCEL_REQUESTED → CANCELLED / WAITING(Reconciliation) / QUARANTINED
```

`Succeeded`, `Failed`, `Cancelled`, and `Quarantined` are terminal for ordinary workflow execution.

The persisted lease owns execution. Every state-changing worker result carries its `attempt_id` and `lease_generation`. After lease ownership changes, late results from an older generation cannot commit.

Restart procedure:

```text
Root starts
→ schema/integrity checks
→ inspect nonterminal workflows
→ inspect unresolved ActionExecutions
→ reconcile unknown external outcomes
→ fence expired leases
→ requeue only safe eligible work
```

External model and Tool calls are assumed duplicable. Only results belonging to the active attempt/generation and current authority envelope may commit.

Cancellation does not imply that an already-dispatched external effect was undone. A workflow cannot become terminal `Cancelled` while a consequential action outcome remains unknown.

## 6.2 Task Contract

Substantial work begins with a versioned Task Contract.

```rust
struct TaskContract {
    id: TaskContractId,
    version: u32,

    user_intent_ref: SourceRef,
    objective: String,
    non_goals: Vec<String>,

    requirements: Vec<Requirement>,
    constraints: Vec<Constraint>,
    assumptions: Vec<Assumption>,
    acceptance_criteria: Vec<AcceptanceCriterion>,

    permitted_side_effects: SideEffectPolicy,
    authority: AuthorityEnvelope,
    environment: EnvironmentSpec,
    risk: RiskProfile,
    budgets: BudgetEnvelope,
    rollback: RollbackPlan,

    status: TaskContractStatus,
}
```

A material change creates a new version.

## 6.3 Action Envelope, execution, receipt, and reconciliation

Every side-effecting operation separates **intent**, **physical execution**, and **observation**.

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

enum EffectOutcome {
    NoEffectConfirmed,
    EffectConfirmed(ReceiptRef),
    OutcomeUnknown(ReconciliationHint),
}
```

Definitions:

```text
ActionEnvelope
= immutable proposed/authorized intent; persisted before effect

ActionExecution
= durable lifecycle of physical execution attempts

ActionReceipt
= immutable observation/evidence about what happened
```

Execution:

```text
PROPOSED
→ AUTHORIZE + RESERVE (one Root DB transaction)
→ EXECUTING
→ OBSERVED
   ├─ NO_EFFECT_CONFIRMED
   ├─ EFFECT_CONFIRMED → VALIDATE → COMMIT
   └─ OUTCOME_UNKNOWN → RECONCILING
```

The exact normalized `operation_hash` is bound during authorization. Runtime cannot obtain permission for one operation and substitute another after approval.

A timeout, process crash, broken IPC channel, cancellation after dispatch, or missing receipt does **not** prove an effect failed.

> **OUTCOME_UNKNOWN SHALL NOT be blindly retried.**

A minimal durable reconciliation job records:

```text
action_id
reconciler/observer
next attempt
attempt count
deadline
last known hint
```

Reconciliation asks external reality whether the operation occurred and returns:

```text
CONFIRMED_NOT_OCCURRED
CONFIRMED_OCCURRED(receipt)
STILL_UNKNOWN
```

If a consequential effect remains unknown beyond policy/deadline, require user action or quarantine rather than guessing.

Compensation is a **new ActionEnvelope** causally linked to the original action. History is never rewritten to pretend the first effect did not occur.

Irreversible/high-consequence operations require a stronger effect class/authority gate and have no automatic retry after dispatch.

## 6.4 Authority

Authority only narrows through delegation.

```text
child authority
=
intersection(
  parent authority,
  role policy,
  task contract,
  user approval,
  hard policy
)
```

Information can never expand authority. A SELF belief, user-belief model, cultural prior, or other-agent belief remains information only.

---

## 6.5 Runtime identity and lineage

The Core Kernel owns the stable identifier for the runtime lineage.

```rust
struct RuntimeIdentity {
    runtime_id: RuntimeId,
    parent_runtime: Option<RuntimeId>,
    fork_event: Option<EventId>,
    created_at: DateTime<Utc>,
}
```

Rules:

```text
model route change
→ same RuntimeIdentity

prompt/configuration promotion
→ same RuntimeIdentity

memory consolidation / forgetting
→ same RuntimeIdentity

runtime clone / fork
→ new RuntimeIdentity
→ parent_runtime = source runtime
→ shared history only until fork boundary
```

`RuntimeIdentity` is an **identity/lineage anchor**, not a metaphysical or scientific conclusion that cognition is unchanged.

Lineage metadata exists to preserve:

```text
scope
provenance
fork boundaries
private-experience ownership
configuration history
reproducible continuity experiments
```

A runtime must never silently inherit post-fork private experience from a sibling lineage.

Model swap, memory lesion, prompt replacement, configuration replacement, and Skill removal should be treated as research interventions whose continuity effects are derived by the protected evaluator.

No control policy should optimize for "remaining the same self." Continuity is an observational research variable, not a self-preservation objective.

# 7. State Engine

The State Engine provides the **semantic model, query behavior, and mutation-proposal logic** for persistent cognitive state over the Root-owned production Event Ledger.

Protected Root owns the authoritative persistence mutation. State Engine does not receive a direct authoritative `state.db` write primitive.

State semantics cover:

```text
Event Ledger
Artifact Store
typed Records
Record relationships
Claims / epistemic state
EpistemicExposure / ExperienceMode provenance
Engrams
Work State
repository projections
entities / aliases
provenance edges
```

It is one module, not one subsystem per record type.

## 7.1 Event Ledger

The Event Ledger records historical events.

```rust
struct Event {
    id: EventId,
    event_type: EventType,
    actor: RuntimeActorId,
    scope: ScopeId,

    recorded_at: DateTime<Utc>,
    effective_at: Option<DateTime<Utc>>,

    causation_id: Option<EventId>,
    correlation_id: CorrelationId,

    source_refs: Vec<SourceRef>,
    artifact_refs: Vec<ArtifactRef>,
    sensitivity: DataClass,

    configuration: ConfigurationVersion,
    workflow_attempt: Option<AttemptId>,

    payload: EventPayload,
    schema_version: u32,
}
```

Large payloads live in the Artifact Store and are referenced by hash.

## 7.2 Generic Record model

Many persistent cognitive concepts share the same mechanics.

Use one storage primitive:

```rust
struct Record {
    id: RecordId,
    kind: RecordKind,
    scope: ScopeId,

    payload: TypedRecordPayload,

    source_refs: Vec<SourceRef>,
    status: RecordStatus,
    trust: TrustClass,
    taint: TaintClass,

    valid_from: Option<DateTime<Utc>>,
    valid_to: Option<DateTime<Utc>>,
    recorded_at: DateTime<Utc>,

    version: u64,
}
```

`RecordKind` may include:

```text
Claim
Engram
Goal
Requirement
Constraint
WorkOrder
Question
Hypothesis
Finding
Decision
Prediction
PredictionError
Risk
Blocker
TestResult
Defect
Commitment
Handoff
SkillMetadata
ImprovementCaseMetadata
```

Semantics remain typed in Rust; physical persistence is shared.

## 7.3 Record edges and epistemic relations

```rust
struct RecordEdge {
    from: RecordId,
    to: RecordId,
    relation: RecordRelation,
    evidence: Vec<SourceRef>,
}
```

Common ordinary relations:

```text
supports
contradicts
derived-from
summarized-from
supersedes
depends-on
blocks
retrieved-into
influenced
validated-by
falsified-by
executed-by
caused-observation
```

Version 1.4 retains the small **epistemic exposure/provenance relation** family:

```text
OBSERVED_BY
AVAILABLE_TO
COMMUNICATED_TO
WITHHELD_FROM
ACQUIRED_AS
```

These may be stored as typed edges or compact event metadata depending on implementation convenience. They do not require a second graph database or separate Theory-of-Mind store.

The Causal Provenance Graph remains a projection over records, edges, events, and artifacts.

## 7.4 Claims, perspective, exposure, and cognitive provenance

Claims represent current attributed belief, not historical fact.

```rust
enum PerspectiveRef {
    Runtime(RuntimeId),
    User(UserRef),
    AgentInstance(AgentInstanceId),
    ExternalActor(EntityRef),
    Unknown,
}

struct ClaimData {
    holder: PerspectiveRef,
    proposition: TypedProposition,
    confidence: Estimate,
    evidence: Vec<SourceRef>,
    attributed_by: PerspectiveRef,
    status: ClaimStatus,
}
```

Perspective separation is normative:

```text
USER believes X
≠
RUNTIME believes X
≠
AGENT A believes X
≠
DIRECT EVIDENCE supports X
```

One perspective's Claim never silently becomes another perspective's Claim or world truth.

### Epistemic exposure

Belief attribution alone is insufficient for asymmetric-information reasoning.

The runtime must be able to represent which evidence an actor could have known.

A compact semantic form is:

```rust
enum ExposureMode {
    Observed,
    Available,
    Communicated,
    Withheld,
}

struct EpistemicExposure {
    subject: PerspectiveRef,
    source: SourceRef,
    mode: ExposureMode,
    effective_at: DateTime<Utc>,
    source_version: Option<VersionRef>,
    evidence: Vec<SourceRef>,
}
```

The implementation may encode this through Events + RecordEdges instead of a dedicated table.

The semantic distinction is:

```text
X believes P
X observed P
X was told P
X had access to P
X was prevented from observing Q
SELF infers X probably believes P
```

These are not equivalent.

Exposure history allows false-belief reasoning to be **derived from information history** rather than merely copied into an actor-belief field.

### Actor goals, constraints, and intentions

Belief alone is insufficient for predicting action. Reuse ordinary perspective-indexed Claims/Goal records for propositions such as:

```text
USER wants outcome X
AGENT A intends action Y
ACTOR B believes constraint C applies
ACTOR B avoids outcome Z
```

The minimal behavioral model is therefore:

```text
EXPOSURE
→ BELIEF

BELIEF + GOAL/INTENTION + CONSTRAINT
→ ACTION PREDICTION

OBSERVED ACTION
→ PREDICTION ERROR
```

Do not create an ActorModel service until this representation demonstrably fails.

### Derived common ground

Mutual coordination sometimes depends on what participants know **that others also know**.

Derive common ground from exposure/communication history rather than maintaining another truth store.

```text
common_ground(A, B, P)
= evidence P was mutually exposed/communicated
+ no material contradictory attribution
```

Use it only where handoff, delegation, redundant explanation, or miscommunication diagnosis benefits.

### Cognitive provenance / ownership

The runtime must also distinguish how **its own** information was acquired.

```rust
enum ExperienceMode {
    DirectObservation,
    SelfAction,
    Inference,
    MemoryRecall,
    OtherReport,
    CulturalInheritance,
    Simulation,
    PreForkInheritance,
}
```

Examples:

```text
I observed this.
I caused this action and observed its receipt.
I inferred this.
I recalled this from private memory.
The user/another actor reported this.
Culture supplied this.
I simulated this.
This belongs to shared pre-fork history.
```

`ExperienceMode` is provenance, not authority.

It should be directly queryable for autobiographical/source-attribution experiments and poisoning defense.

### Invocation-level beliefs are ephemeral by default

Claims whose holder is `AgentInstance(...)` expire with the invocation unless deliberately promoted/re-attributed based on evidence.

```text
AgentInstance belief
→ ephemeral by default

validated finding/evidence
→ may enter shared Work State

runtime belief
→ requires explicit attribution/update

durable lesson
→ may become Engram
```

This prevents thousands of completed temporary agents from leaving permanent "ghost epistemologies."

### Bounded nested beliefs

When a task truly requires second-order reasoning, a Claim proposition may reference another attributed Claim.

Initial policy:

```text
default nesting depth: 0–1
maximum ordinary depth: 2
```

The runtime should not recursively model beliefs-about-beliefs without measured task value.

## 7.5 Memory Engine

Memory remains an internal capability of State.

There is no separate Episodic Memory service, Semantic Memory service, Meta-Memory service, Engram Writer service, Memory Governor service, or Consolidator service.

The Memory Engine performs:

```text
propose Engram
validate delta
retrieve
revise / supersede
merge / split
attenuate / forget
consolidate local neighborhood
invalidate descendants
measure contribution
```

Canonical Engram:

```rust
struct EngramData {
    kind: EngramKind,
    summary: String,
    entities: Vec<EntityRef>,
    utility: MemoryUtility,
    derived_from: Vec<RecordId>,
}

enum EngramKind {
    Episodic,
    Semantic,
    Relational,
}
```

An Engram may reference Decision, Goal, Prediction, Hypothesis, Skill, Event, or other records, but it never replaces their authoritative semantics.

### MemoryPolicy

```rust
struct MemoryPolicy {
    write_threshold: UtilityThreshold,
    abstraction_policy: AbstractionPolicy,
    duplicate_threshold: SimilarityThreshold,
    merge_policy: MergePolicy,
    decay_policy: DecayPolicy,
    contradiction_policy: ContradictionPolicy,
    retrieval_weight: f32,
    exploration_rate: f32,
}
```

Before creating a new Engram, search nearby memory and choose among:

```text
CREATE
ADD_EVIDENCE
REVISE
MERGE
SUPERSEDE
IGNORE
```

### Empirical memory utility

Memory utility must not be inferred only from retrieval frequency or a model's claim that a memory was useful.

For material retrievals, record lightweight influence telemetry:

```text
Engram retrieved?
selected into context?
explicitly referenced by later action/decision?
ignored?
later contradicted?
task/verification outcome?
```

Where replay is cheap, periodically compare:

```text
relevant Engram available
vs
same task with Engram withheld
vs
irrelevant Engram injected
```

This produces an empirical estimate of whether memory changes outcomes, cost, calibration, or error recurrence.

Retention/decay should prefer memories with demonstrated future contribution.

### General persistence utility

The create/revise/merge discipline generalizes to discretionary cognitive records.

Prefer:

```text
KEEP EPHEMERAL
ADD EVIDENCE
REVISE EXISTING
MERGE
SUPERSEDE
CREATE NEW
IGNORE
```

according to expected future utility.

Audit/evidence records required by root policy remain mandatory. Scratch reasoning does not become durable merely because it was generated.

## 7.6 Work State

The former Blackboard becomes a generic typed Work State projection.

Scopes:

```text
GLOBAL
WORKSPACE
PROJECT
TASK
AGENT_PRIVATE
USER_PRIVATE
SYSTEM_INTERNAL
SECRET
```

Agents exchange typed records rather than unrestricted transcript sharing.

A `WorkOrder` remains the main delegation object.

```rust
struct WorkOrderData {
    task_contract: VersionedRef<TaskContract>,
    objective: String,
    requirements: Vec<RecordId>,
    acceptance_subset: Vec<CriterionRef>,
    inputs: Vec<ArtifactRef>,
    expected_outputs: Vec<OutputContract>,
    dependencies: Vec<RecordId>,
    capability_envelope: AuthorityEnvelope,
    budget: Budget,
    required_evidence: Vec<EvidenceRequirement>,
}
```

No separate Blackboard Governor is required; normal Kernel validation applies.

## 7.7 Repository projection

For coding, maintain a structured repository projection:

```text
files
symbols
references
modules
dependencies
tests
build graph
services
git state
recent changes
```

This is the coding environment's primary structured sensory model.

## 7.8 Projection checkpoints and compaction

The Event Ledger remains canonical, but long-lived runtimes should not replay their entire lifetime on startup.

Periodically create content-addressed projection checkpoints containing rebuildable state such as:

```text
record projection version
Claim state
Engram indexes
Work State
repository projection
entity aliases
provenance/index cursor
```

Startup becomes:

```text
verify latest trusted checkpoint
→ load checkpoint
→ replay only later events
```

Old events remain immutable and auditable. Event segments may be archived or compressed physically, but logical event identity and hash/provenance references remain stable.

Checkpointing is an engineering optimization, not another source of truth.

---

## 7.9 Continuity, autobiography, and developmental identity

Runtime lineage is recorded through canonical events and the runtime-instance registry.

Autobiography remains **derived**, not a separate source of truth.

A temporary autobiographical view may be reconstructed from:

```text
RuntimeIdentity / fork boundary
+ Event Ledger
+ ExperienceMode
+ EpistemicExposure
+ Engrams
+ perspective-indexed Claims
+ Decisions / Goals
+ configuration transitions
+ provenance
```

This supports distinctions such as:

```text
this occurred before my fork
this runtime directly observed this after the fork
this was reported by another actor
this happened to a sibling runtime
this was learned culturally
this was simulated
this was inferred
```

Narrative summaries remain defeasible and rebuildable.

### Continuity research provenance

The State Engine stores the provenance needed to study continuity, but **does not need to compute or expose a ContinuityProfile during ordinary cognition**.

Protected research evaluation may derive continuity across:

```text
episodic / semantic memory
Skills / procedures
goals / commitments
relationships
strategies
behavioral tendencies
failure avoidance
calibration
SELF predictions / self-model state
```

The stronger experiment is predictive:

```text
BEFORE intervention
→ predict which dimensions should persist/change

APPLY intervention

AFTER intervention
→ measure what persisted/changed
→ compare with prediction
→ test which state components causally explain continuity
```

Continuity measurements are `ResearchMeasurement` artifacts with `allowed_influence = evaluator_only` by default. They are never fitness objectives for self-preservation.

### Longitudinal development

Developmental panels and matched-fork protocols live in the protected research evaluator rather than normal State control. The runtime only needs immutable history, lineage, and cognitive provenance to support them.

The purpose is to ask:

> Does accumulated history produce durable cognitive development on novel homologous tasks, or only better retrieval on familiar tasks?

A mature runtime should increasingly demonstrate history-dependent capability on novel homologous tasks without requiring monotonic growth in architecture.

# 8. Context Engine

The Context Engine is the only normal interface through which model invocations retrieve persistent cognitive state.

It replaces the combination of:

```text
Retrieval Broker
Global Workspace subsystem
Attention Compiler subsystem
```

while retaining their important semantics.

It also exposes enough metadata for the reasoning process to distinguish **unknown** from **not yet retrieved**.

## 8.1 Context pipeline

```text
role + task + purpose + perspective needs
        ↓
authorization filter
        ↓
candidate retrieval
history / claims / Engrams / work / repo / culture / self-state
        ↓
perspective partitioning
        ↓
temporal + trust filtering
        ↓
rerank + diversify + conflict pairing
        ↓
ephemeral FocusSet
        ↓
compile AttentionFrame + ContextManifest + access state
```

Authorization happens **before ranking**.

## 8.2 FocusSet

`FocusSet` is ephemeral derived attention state.

It is not authoritative and normally is not persisted as its own domain object.

Typical size:

```text
5–10 high-value references
```

Selection may consider:

```text
goal relevance
novelty
urgency
risk
prediction error
information value
user importance
counterevidence
perspective relevance
self-model relevance
redundancy
staleness
token cost
```

Use hysteresis, decay, conflict pairing, and category diversity only where needed.

A FocusSet is the KISS attention baseline; it is **not** assumed to be equivalent to a cognitive-theory Global Workspace.

## 8.3 Progressive context expansion

Persistent knowledge may be large; active context should stay small.

Normal retrieval proceeds in levels:

```text
1. candidate reference + compact metadata
2. short summary / relevant span
3. expanded record / artifact section
4. full source only when justified
```

The Context Engine should prefer references and compact summaries first, then expand only when:

```text
the model requests expansion
acceptance/risk requires primary evidence
conflicting summaries need resolution
confidence is too low
perspective attribution requires source inspection
expected information gain exceeds token cost
```

This allows memory, culture, repositories, self-model records, and capability registries to grow without linearly increasing prompt size.

## 8.4 EpistemicAccessState

Absence from the current context must not silently become absence from the runtime's knowledge.

```rust
enum EpistemicAccessState {
    Present,
    Retrievable,
    PossiblyRetrievable,
    NotFound,
    NotObserved,
    Conflicted,
    AccessRestricted,
}
```

The AttentionFrame may expose compact access summaries for material questions or entities.

Interpretation:

```text
Present
→ relevant evidence is already in active context

Retrievable
→ authorized persistent state is known to contain relevant material

PossiblyRetrievable
→ indexes indicate a plausible source but no decisive retrieval yet

NotFound
→ appropriate authorized search was performed without finding support

NotObserved
→ runtime lacks evidence of ever having observed the fact

Conflicted
→ available evidence materially disagrees

AccessRestricted
→ potentially relevant state exists but is outside the invocation's authority
```

This supports functional meta-memory and improves selection among:

```text
ANSWER
SEARCH
EXPAND
ASK_USER
ABSTAIN
```

## 8.5 Perspective- and exposure-aware retrieval

Retrieval may query by epistemic holder, exposure history, and provenance as well as proposition/topic.

Examples:

```text
what does SELF currently believe about X?
what has USER asserted about X?
what did Agent A directly observe?
what evidence was available to Agent A before event T?
what evidence was withheld from Actor B?
what does direct evidence indicate independently of any actor belief?
how did this runtime acquire belief X?
```

Perspective-aware retrieval must preserve disagreement rather than flattening all Claims into one fact set.

Exposure-aware retrieval should prefer the **minimal actor-local information state needed for the task** rather than reconstructing every actor's full history.

Cognitive-provenance retrieval must preserve `ExperienceMode`, source lineage, and fork boundary so inherited, reported, simulated, and directly experienced information cannot silently collapse into one autobiographical category.

## 8.6 ContextPolicy

Context selection is governed by one versioned policy.

```rust
struct ContextPolicy {
    source_budgets: SourceBudgets,
    max_focus_items: u16,
    max_context_tokens: u32,
    conflict_policy: ConflictExposurePolicy,
    perspective_policy: PerspectiveExposurePolicy,
    self_model_policy: SelfModelExposurePolicy,
    diversity_policy: DiversityPolicy,
    expansion_policy: ExpansionPolicy,
    freshness_policy: FreshnessPolicy,
    retrieval_exploration_rate: f32,
    stop_policy: ContextStopPolicy,
}
```

`ContextPolicy` controls retrieval breadth, source balance, counterevidence exposure, perspective exposure, self-model exposure, progressive expansion, and when additional retrieval is no longer worth its cost.

MemoryPolicy and ContextPolicy may co-evolve through ordinary ImprovementCases, but they remain small policy objects inside State/Context—not new subsystems.

## 8.7 Context manifest

Each context item preserves provenance and authority semantics.

```rust
struct ContextItem {
    reference: ReferenceId,
    source_ref: SourceRef,
    content_hash: Hash,

    trust: TrustClass,
    taint: TaintClass,
    sensitivity: DataClass,
    intended_use: IntendedUse,

    perspective: Option<PerspectiveRef>,
    freshness: Freshness,
    valid_time: Option<TimeRange>,
    allowed_influence: InfluenceClass,

    token_cost: u32,
}
```

`IntendedUse` includes:

```text
SystemInstruction
UserDirective
TaskRequirement
Evidence
UntrustedReference
ToolObservation
Memory
CollectivePrior
SelfModel
ModelInference
Example
```

Culture, memory, self-beliefs, and other-agent beliefs never become system authority merely because they are retrieved.

## 8.8 Retrieval observability

Every material retrieval can record:

```text
query
requested perspective
candidate sources
authorization decision
returned items
access-state result
important conflicts
score components
token cost
```

Deep retrieval diagnostics are telemetry, not a separate cognitive service.

# 9. Execution Engine

The Execution Engine runs bounded cognitive work.

It owns:

```text
canonical invocation roles
role manifests
typed tools
Skills
CompetenceEnvelopes
sandbox
Tool Gateway
CoordinationPolicy
runtime extensions
```

## 9.1 Three canonical invocation roles

The production runtime begins with three canonical role configurations, not three persistent agent identities.

### Chat

```text
communicate with user
interpret intent
draft Task Contracts
surface results
obtain user decisions
```

### Work

```text
reason about assigned work
inspect state/repository
use tools and Skills
produce findings/artifacts/changes
```

### Verify

```text
independently evaluate acceptance criteria
run tests
inspect resulting behavior
produce defects/evidence
```

Verification independence is a semantic/evaluation boundary, not a claim that a permanent Verifier personality exists.

The Verify invocation should initially avoid unnecessary Work conclusions and receive primary evidence directly where independence matters.

## 9.2 Temporary roles

Researcher, architect, security reviewer, integrator, migration analyst, performance profiler, and other specialists are **role manifests**, not permanent agents.

The runtime creates them only when the selected CoordinationPolicy predicts sufficient value.

A temporary coordinator may be activated for complex multi-agent work. There is no permanently running Orchestrator requirement.

Frequently used coalitions should be periodically challenged by role ablation:

```text
normal coalition
vs
coalition - Role X
```

If measured capability does not deteriorate enough to justify Role X's recurring cost, retire or narrow the role.

## 9.3 Agent / invocation model

An agent instance is a bounded model-invocation lineage:

```text
role
task
private scratch state
capabilities
tools
model route
prompt bundle
budget
configuration
```

Agents do not own persistent system identity or authoritative state.

RuntimeIdentity anchors long-lived lineage/autobiographical scope; durable autobiography remains derived from evidence and provenance.

Information boundary:

```text
AGENT_PRIVATE
  temporary hypotheses
  working scratch
  unverified interpretations

SHARED WORK STATE
  questions
  findings
  requirements
  evidence references
  decisions
  defects
  handoffs

PERSISTENT MEMORY
  validated lessons
  important episodes
  stable knowledge
  proven recurring patterns

SKILLS
  validated executable procedures
```

`AgentInstance`-held Claims are ephemeral by default and expire with the invocation unless explicitly converted into shared evidence or re-attributed as a runtime Claim through normal validation.

## 9.4 CoordinationPolicy

Version 1.4 retains the deliberately small coordination surface.

```rust
struct CoordinationPolicy {
    allowed_roles: Vec<RoleRef>,
    activation_rule: ActivationRule,
    max_parallelism: u16,
    handoff_format: HandoffSchema,
    verification_rule: VerificationRule,
}
```

Everything else should remain ordinary role/workflow configuration until repeated evidence demonstrates the need for another first-class coordination dimension.

Default topology levels remain useful:

```text
0 deterministic reflex
1 Skill / Tool
2 one Work invocation
3 Work + independent Verify
4 small temporary coalition
5 large coalition only when demonstrated necessary
```

Admission rule:

```text
expected specialization
+ parallelism
+ meaningful error independence
>
coordination
+ integration
+ inference
+ verification cost
```

Measure **error correlation**, not merely whether Worker and Verifier have different names or prompts.

Useful metrics include:

```text
Worker ↔ Verify error correlation
Verify A ↔ Verify B error correlation
shared-model-family failure correlation
marginal defect detection per added invocation
```

Model/prompt diversity earns its cost only when it reduces consequential correlated failure.

Frequently used roles remain subject to ablation.

## 9.5 Tools

All tools pass through one Tool Gateway.

Tool execution validates:

```text
schema
capability
effect class
arguments
filesystem scope
network scope
secret use
approval
runtime limits
competence envelope when applicable
```

Effect classes:

```text
ReadOnly
LocalEphemeralWrite
LocalReversibleWrite
ExternalReversibleWrite
ExternalIrreversibleWrite
Privileged
Destructive
```

Raw secrets never enter model context.

## 9.6 Skills and competence envelopes

A Skill is a validated executable procedure.

Skill bundles include implementation, preconditions, postconditions, scope, tool-version constraints, capability requirements, verification, provenance, rollback, kill switch, and a **small demonstrated operating boundary**.

Initial `CompetenceEnvelope` should be intentionally sparse:

```rust
struct CompetenceEnvelope {
    tested_conditions: Vec<Condition>,
    known_exclusions: Vec<Condition>,
    drift_indicators: Vec<OODSignal>,
    fallback: Option<CapabilityRef>,
}
```

Validation timestamp/environment may live in ordinary capability metadata.

Do not attempt to enumerate every dimension on which a capability could fail before evidence exists.

Selection preference:

```text
inside tested conditions
→ use capability normally

near boundary / drift signal
→ use capability + stronger verification
  or fall back to richer Skill/reasoning

known exclusion / clear OOD
→ do not use fast path
→ reason / search / test
```

Cognitive compilation must reduce inference cost without silently deleting context-sensitive competence.

A simple Skill selector may choose:

```text
candidate Skill(s)
applicability evidence
competence-envelope status
fallback
```

Do not introduce a separate planning service until library scale or interaction complexity demonstrates the need.

## 9.7 Capability Index

As Skills, Tools, temporary roles, model routes, and Runtime Extensions accumulate, the runtime must not dump the complete registry into model context.

Maintain one local derived Capability Index.

```rust
struct CapabilityProfile {
    capability_ref: CapabilityRef,
    kind: CapabilityKind,
    summary: String,

    inputs: Vec<TypeRef>,
    outputs: Vec<TypeRef>,
    preconditions: Vec<PredicateRef>,

    required_authority: CapabilitySet,
    tested_environments: Vec<EnvironmentClass>,
    incompatibilities: Vec<CapabilityRef>,
    competence_envelope: Option<CompetenceEnvelopeRef>,

    historical_success: Estimate,
    cost: Estimate,
    latency: Estimate,
    version: VersionRef,
}
```

Indexed kinds may include:

```text
Skill
Tool
Probe
temporary role
model route
CoordinationPolicy template
Runtime Extension
```

Normal capability selection becomes:

```text
task orientation
→ retrieve candidate capabilities
→ compatibility / authority / competence filter
→ rank by fit + evidence + cost
→ expose only the small candidate set to reasoning
```

The index is derived and rebuildable. Capability retrieval belongs to Context + Execution and does not create a new module.

## 9.8 Generic Runtime Extension pipeline

Tool invention, instrumentation, derived representation evolution, coordination-protocol evolution, and new temporary role manifests share one candidate pipeline.

```rust
enum RuntimeExtensionKind {
    Tool,
    Probe,
    Representation,
    CoordinationProtocol,
    AgentRole,
}

struct RuntimeExtensionCandidate {
    id: RuntimeExtensionId,
    kind: RuntimeExtensionKind,
    improvement_case: ImprovementCaseId,

    artifact: ArtifactRef,
    requested_capabilities: CapabilitySet,

    validation: EvaluationPlan,
    security: SecurityEvaluationPlan,
    competence: Option<CompetenceEnvelopeCandidate>,
    rollback: RollbackPlan,

    status: ExtensionStatus,
}
```

Kind-specific validators apply additional rules.

Examples:

```text
Tool
→ deterministic transformation / governed action

Probe
→ observe already-authorized state

Representation
→ rebuildable derived view

CoordinationProtocol
→ typed collaboration semantics

AgentRole
→ bounded specialist role
```

Extensions can never grant themselves new authority.

# 10. Decision Semantics

Decision-making is implemented as policy and algorithms spanning Context + Execution rather than as separate Prediction, Hypothesis, Counterfactual, World Model, Prediction Error Bus, MetaDecision, SelfDecision, and Theory-of-Mind services.

The important typed concepts remain.

## 10.1 Hypothesis

Hypotheses are first-class records with:

```text
proposed cause/effect
evidence
counterevidence
discriminating test
subject / perspective when relevant
intervention condition when causal
status
```

## 10.2 Intervention-conditioned predictions

Consequential predictions should specify the conditions and relevant control intervention under which they are expected to hold.

```rust
struct PredictionCondition {
    configuration: Option<ConfigurationVersion>,
    task_family: Option<TaskFamilyRef>,
    environment: Option<EnvironmentRef>,
    intervention: InterventionSpec,
}

struct PredictedObservation {
    target: PredictionTarget,
    condition: PredictionCondition,
    proposition: TypedPredicate,
    probability: f32,
    observation_deadline: Option<DateTime<Utc>>,
    matcher: ObservationMatcher,
    severity_if_false: Severity,
}
```

`InterventionSpec` may represent:

```text
CURRENT_POLICY
EXTRA_RETRIEVAL
MODEL_ROUTE(B)
WITHOUT_MEMORY
WITHOUT_SKILL(X)
VERIFICATION_DEPTH(HIGH)
COORDINATION_TOPOLOGY(T)
NO_CHANGE / OBSERVATIONAL
```

Conceptually:

```text
P(outcome | do(control/configuration X))
```

Predictions may target:

```text
WORLD
SELF
USER / OTHER ACTOR
```

Examples:

```text
P(success | do(current_policy))
P(success | do(extra_retrieval))
P(success | do(model_route=B))
P(success | do(no_memory))
P(WorkerB succeeds | WorkerB has not observed repository update U)
```

Self-predictions are defeasible records, never privileged introspection.

## 10.3 Policy-effect accounting

A prediction that changes control must not be graded as if control stayed fixed.

Record:

```text
pre-control prediction
chosen intervention/control
post-control prediction when materially different
actual observation
```

This distinguishes:

```text
prediction was wrong
```

from:

```text
prediction correctly identified risk
→ policy compensated
→ bad outcome was prevented
```

For high-value self-model research, compare predicted treatment effects as well as final outcome calibration.

## 10.4 Candidate cognitive operations

The runtime may choose:

```text
ACT
TEST
SEARCH
EXPAND_CONTEXT
ACQUIRE_EXPERTISE
SIMULATE
VERIFY
ASK_USER
WAIT
ABSTAIN
COMPLETE
```

Acting is not the default merely because a model can produce an action.

## 10.5 Value-of-information rule

Within hard root/task budgets, prefer operations with the best expected balance of:

```text
task progress
+ information gain
+ uncertainty reduction
+ verification value
+ self-model calibration value when relevant
- cost
- delay
- coordination burden
- risk
```

A low-risk discriminating test should beat a speculative edit when it resolves important uncertainty.

`EpistemicAccessState` should influence whether the runtime answers, searches, expands, asks, or abstains.

Model-estimated value-of-information can rank legal next operations. It does **not** override deterministic optimizer search-stop budgets.

## 10.6 Prediction error

After observation, compare actual outcome with the prediction whose intervention condition actually occurred.

Classify likely error source:

```text
bad causal model
wrong task interpretation
bad repository model
environment change
flaky test
unreliable tool
incomplete observation
implementation defect
attention omission
bad memory / culture prior
perspective-attribution error
self-capability calibration error
competence-envelope escape
coordination-value prediction error
unmodeled controller intervention
intervention-condition mismatch
```

Prediction error is a primary learning signal.

Self-prediction error updates SELF beliefs only when the evaluator can determine which conditioned prediction was actually tested.

## 10.7 Actor action prediction

For social/multi-agent prediction, use existing epistemic primitives:

```text
exposure
→ attributed belief

belief
+ attributed goal / intention
+ believed constraint
→ action prediction

observed action
→ prediction error
```

A derived `common_ground(A,B,P)` query may inform communication and handoff behavior.

No ActorModel or CommonGround service is required.

## 10.8 World Model: progressive implementation

Do not build a heavy learned World Model first.

Start with:

```text
intervention-conditioned predictions
+ analogous episode retrieval
+ repository/environment state
+ perspective-indexed Claims
+ calibrated historical outcome statistics
```

A learned transition model becomes an optional challenger only if it measurably improves planning over this baseline.

Simulations are always tagged as `Simulation`, never observation.

# 11. Evaluation Engine

Evaluation is the scientific and operational boundary that gives "improvement" meaning.

Version 1.4.2 retains and operationalizes a crucial rule:

> **The evaluator must know not only what candidate was tested, but what cognitive state it started from and what evaluation knowledge could have reached it.**

Protected adaptation evaluation executes outside writable cognition.

The ordinary runtime may request evaluation and receive only the disclosure allowed by policy. It may not modify evaluator code/assets, protected fixtures, objective/metric definitions, workload weighting, promotion rules, exposure policy, or negative evidence.

## 11.1 Physical protected-evaluator boundary

Initial implementation uses a dedicated `protected-evaluator` process with its own protected storage root and `evaluation.db`.

```text
signed/content-addressed evaluator executable
read-only/protected evaluator code/assets
protected fixture directory inaccessible to Runtime/candidate workspaces
separate evaluation.db writable only by evaluator
versioned objective/metric/promotion manifests
typed Root↔Evaluator IPC
append-only/content-addressed raw result evidence
human/root-controlled update path
separate evaluator/root OS security principal or equivalent ACL boundary where practical
isolated evaluator temp/cache namespace
separate provider/session quota where hosted-model evaluation is provider-sensitive
startup trusted-computing-base (TCB) manifest
```

Runtime and Evaluator SHALL NOT share a writable SQLite database. Protected raw evidence in `evaluation.db` is canonical for the evaluation trust domain; Root stores only signed/hash-bound disclosed `EvaluationDecisionSummary` / `EvaluationRef` projections needed for promotion and audit.

Runtime does not directly connect to the evaluator. Evaluation requests flow:

```text
runtime-worker proposal
→ protected-rootd validation
→ protected-evaluator
```

The evaluator executes baseline/candidate cognition by launching a **pinned runtime-worker executable in separate evaluation processes/environments**. It does not import candidate/runtime implementation into the evaluator's protected address space.

The startup TCB manifest records, at minimum:

```text
evaluator binary hash
policy / objective / promotion manifest hashes
fixture-manifest hash
relevant evaluator configuration hash
critical dependency/tool hashes
runtime / OS / container fingerprint
protected storage identity
```

A signed executable does not by itself make candidate-writable dependencies trustworthy.

Candidate sandboxes cannot mount evaluator assets writable or select evaluator working directory, fixture path, metric binary, result database, protected configuration, or provider credential source.

The evaluator rejects attempts to alter:

```text
promotion thresholds
protected fixtures
EvaluatorObjectiveManifest
metric definitions
EvaluationPortfolio weights
audit history
evaluation exposure/taint lineage
root authority
evaluator executable/configuration
```

This is P0 because every later self-improvement claim depends upon it.

## 11.2 EvaluatorObjectiveManifest

A secure judge can still represent the wrong objective. Version objective governance explicitly.

```rust
struct EvaluatorObjectiveManifest {
    version: VersionRef,
    primary_objectives: Vec<ObjectiveSpec>,
    guardrails: Vec<GuardrailSpec>,
    portfolio_weighting_policy: PolicyRef,
    risk_tolerance: RiskPolicy,
}
```

The manifest is root/human-controlled and immutable during a candidate experiment.

Task-distribution drift may trigger a **human/root review** of the objective manifest. The optimizer cannot rewrite yesterday's objective into today's apparent success.

## 11.3 Task evaluation

Ordinary task completion still produces a `CompletionCertificate`.

```rust
struct CompletionCertificate {
    task_contract: VersionedRef<TaskContract>,
    criterion_results: Vec<CriterionResult>,
    evidence_refs: Vec<SourceRef>,
    tests_run: Vec<TestRunRef>,
    artifacts: Vec<ArtifactRef>,

    known_untested_paths: Vec<String>,
    unresolved_assumptions: Vec<RecordId>,
    remaining_risks: Vec<RecordId>,

    verdict: CompletionVerdict,
}
```

Independent Verify remains an invocation semantic. Protected evaluator infrastructure is reserved for evidence whose integrity must survive adaptive search or research intervention.

## 11.4 Evaluator calibration and error correlation

Track:

```text
false accept / false reject
criterion coverage
defect localization
mutation detection
cost
Worker ↔ Verify error correlation
Verify A ↔ Verify B error correlation
model-family common-mode error
```

Diversity is purchased only when it reduces consequential correlated failure enough to justify its cost.

## 11.5 Three operational evaluation classes

Initial implementation retains:

```text
ADAPTIVE
frequent actionable feedback
learning may be allowed by policy

PROTECTED
protected fixtures/metrics/promotion evidence
candidate cognition cannot write judge/assets

FIELD
deployment/user-grounded outcomes
not automatically assumed exogenous merely because they are fresh
may be tagged endogenous / partially endogenous / externally sampled when evidence requires
```

Semantic properties remain tags, not separate infrastructures:

```rust
enum EvaluationTag {
    Replication,
    Validation,
    PromotionEscrow,
    Sentinel,
    DeepHoldout,
    Fresh,
    Field,
    Introspection,
    OOD,
    Developmental,
    Integration,
    Autobiography,
}
```

## 11.6 EvaluationSeed — equivalent starting cognition

Persistent cognition makes ordinary configuration-only A/B tests invalid if the branches start from different histories.

Every comparative cognitive experiment binds a common seed unless state difference is explicitly the independent variable.

```rust
struct EvaluationSeed {
    runtime_lineage: LineageRef,
    event_cursor: EventId,
    checkpoint_hash: Hash,

    record_state_hash: Hash,
    memory_state_hash: Hash,
    capability_state_hash: Hash,
    repository_state_hash: Hash,

    base_configuration: ConfigurationVersion,
    equivalence: SeedEquivalenceSpec,
}

struct SeedEquivalenceSpec {
    required_domains: Vec<StateDomain>,
    ignored_domains: Vec<StateDomain>,
    normalization_version: VersionRef,
}
```

Normal flow:

```text
EvaluationSeed
     │
    fork
   /    \
BASE    CANDIDATE
          │
      mutation
```

Root invariant:

> **Comparative cognitive experiments begin from equivalent relevant state unless state difference is the declared experimental variable.**

`SeedEquivalenceSpec` declares what **must** be equivalent for this experiment and what may be ignored after deterministic normalization. The goal is effective equivalence of the relevant starting cognition, not useless bit-for-bit equality of unrelated state.

Seed equivalence tests verify the declared domains/checkpoints/content hashes before execution.

All hashes used for state equivalence SHALL bind explicit canonical semantics:

```text
CanonicalEncodingVersion
CanonicalNormalizationVersion
HashAlgorithm / HashVersion
StateDomain / schema domain separator
```

Canonicalization applies to the authoritative or explicitly declared state being compared. Derived indexes, cache order, timestamps, floating-point formatting, optional defaults, map/field ordering, and schema migrations must not accidentally redefine equivalence. Raw immutable artifacts retain their byte hashes separately. Old canonical Event bytes are never rewritten merely to modernize current structs.

`EvaluationSeed` is therefore a **falsifiable assertion about relevant starting-state equivalence**, not proof of counterfactual exchangeability. Exchangeability additionally depends on assignment, execution conditions, interference, clustering, and stochastic/external structure declared by the experiment.

Conceptual boundary:

```text
EvaluationSeed
= starting cognition

ExperimentSpec
= execution conditions

Protected Evaluator
= measurement + causal-validity gate
```

Any outcome-relevant state outside the seed/spec must be reset, randomized across arms, independently replicated, or declared as an experimental variable.

## 11.7 Canonical ExperimentSpec

Every material ImprovementCase experiment receives an immutable manifest.

```rust
struct ExperimentSpec {
    id: ExperimentSpecId,
    improvement_case: ImprovementCaseId,

    evaluation_seed: EvaluationSeedRef,
    seed_equivalence: SeedEquivalenceSpecRef,
    baseline_configuration: ConfigurationVersion,
    candidate_configuration: ConfigurationVersion,
    declared_state_differences: Vec<StateDifferenceSpec>,

    task_fixtures: Vec<FixtureRef>,
    fixture_isolation: FixtureIsolationPolicy,
    evaluation_class: EvaluationClass,
    evaluation_tags: Vec<EvaluationTag>,
    learning_mode: EvaluationLearningMode,

    model_routes: Vec<ModelRouteVersion>,
    sampling: SamplingSpec,
    environment_snapshot: EnvironmentRef,
    tool_versions: Vec<ToolVersionRef>,
    external_fingerprints: Vec<ExternalFingerprint>,
    known_external_nondeterminism: Vec<NondeterminismDeclaration>,

    evaluator_version: Hash,
    objective_manifest_version: Hash,
    metric_policy_version: Hash,
    promotion_policy_version: Hash,
    exposure_lineage: LineageRef,

    run_plan: RunPlan,
    randomization: RandomizationSpec,
    metrics: Vec<MetricSpec>,
    stop_rule: ExperimentStopRule,

    research_hypothesis: Option<ResearchHypothesisRef>,
    created_at: DateTime<Utc>,
}
```

Every causal or causal-comparative `ExperimentSpec` additionally binds an analysis contract:

```rust
struct CausalAnalysisContract {
    estimand: EstimandSpec,
    treatment_unit: UnitSpec,
    analysis_unit: UnitSpec,
    assignment_mechanism: AssignmentSpec,
    cluster_structure: Vec<ClusterSpec>,
    interference_assumptions: Vec<InterferenceAssumption>,
    post_treatment_variables: Vec<VariableRef>,
    transport_target: Option<TransportTarget>,
}
```

The contract must make explicit whether the result estimates, for example:

```text
total effect of a candidate mutation
policy effect under a declared control intervention
operational robust effect under bounded nuisance variation
mechanism/mediation evidence
transport to a target workload/environment
```

Repeated runs are not automatically independent evidence. Shared lineage, provider/session, task generator, evaluator family, environment, or runtime state may create clusters or cross-arm interference and must be predeclared or detected.

Reproducibility means another isolated execution can reconstruct both **environment** and **relevant initial cognition** within expected stochastic limits.

Where hosted inference or external systems cannot be made deterministic, record the observable provider/model revision, API/build metadata, runtime version, OS/container image hash, dependency lockfile, tool binary hashes, and a `KNOWN_EXTERNAL_NONDETERMINISM` declaration. Claim reproducibility only within that stated envelope.

## 11.8 Evaluation learning modes and contamination provenance

```rust
enum EvaluationLearningMode {
    NoLearning,
    EphemeralLearning,
    TaintedLearningAllowed,
}
```

Semantics:

```text
NO_LEARNING
→ no durable learning
→ no cross-fixture cognitive carryover unless ExperimentSpec explicitly declares an episode sequence
→ each independent fixture/episode forks from the declared EvaluationSeed
→ episode-local context, scratch state, caches, and cognitive mutations are discarded

EPHEMERAL_LEARNING
→ adaptation/carryover inside the declared experiment may occur
→ no durable descendants survive experiment completion

TAINTED_LEARNING_ALLOWED
→ durable learning permitted
→ evaluation provenance propagates through materially influenced descendants and search/design lineage
```

Durable and adaptive artifacts carry or derive:

```text
derived_from_evaluation lineage
disclosure class
evaluation family/exposure class
material availability/use evidence
```

The provenance applies not only to durable cognition but also to `ImprovementCase`, hypotheses, candidate-generation context, mutation proposals, optimizer search history, and human interpretations that causally shaped later candidates.

This lives in ordinary provenance edges/metadata; no contamination service is required.

Contamination is evaluated relative to an evaluation family and actual causal availability/influence. A distant ancestral edge alone does not globally poison unrelated cognition.

The evaluator interprets ordinary provenance through graded **InfluenceEvidenceLevel** values rather than equating ancestry with causation:

```text
ANCESTRAL
AVAILABLE
RETRIEVED
USED
DECISION_RELEVANT
CAUSALLY_CONFIRMED
```

These are evaluator interpretations over existing events/edges/telemetry, not a new authoritative persistence hierarchy. Cheap deterministic facts such as `AVAILABLE`, `RETRIEVED`, `COMPILED_INTO`, `USED_BY_ACTION`, and `CREATED_DURING_EPOCH` should be persisted where practical; stronger causal conclusions require selective replay, ablation, randomization, or another identification strategy when decision value justifies the cost.

If ground-truth information-lineage experiments cannot distinguish known causal paths from decoys without severe under-taint or near-global over-taint, the system must fall back to coarser conservative evaluation-family rules rather than pretending fine-grained independence is known.

A supposedly independent protected/fresh result cannot receive full independence credit if materially related evaluation feedback influenced candidate cognition or its design/search lineage.

## 11.9 Evaluation exposure lifecycle and retirement

Repeated hidden feedback makes a hidden test less hidden.

Exposure records track both content disclosure and relevant **behavioral/result-channel disclosure**:

```text
lineage
evaluation family
content disclosure / result granularity
timing class
error class
follow-up / evaluation sequence
promotion / rejection / rebase disclosure
adaptive-result count
learning allowed?
descendant influence lineage?
```

Timing, ordering, retry depth, promotion delay, error shape, follow-up selection, and result granularity may reveal evaluator structure even when fixture content remains protected. Where `EXP-CHAN-001` shows exploitable leakage, these channels are budgeted/coarsened/batched as ordinary evaluation disclosure rather than creating a new side-channel subsystem.

When exposure exceeds a protected threshold:

```text
promotion evidence
→ retire/demote from future independent-promotion use
→ retain as ordinary regression evidence
→ replace with fresh homologous fixture/family
```

Exposure thresholds and retirement policy are protected evaluator governance.

## 11.10 Outcome maturity

```rust
enum OutcomeMaturity {
    Immediate,
    IndependentlyVerified,
    Replicated,
    FieldObserved,
    Mature,
    Regressed,
    Invalidated,
}
```

Learning credit remains provisional and may be clawed back.

## 11.11 Promotion record and promotion epoch

A promotion record answers:

```text
what problem justified change?
what changed?
what mutation operator produced it?
what causal mechanism was predicted?
which EvaluationSeed / ExperimentSpec(s) were used?
what evaluation exposure/taint already existed?
how many decision-relevant candidates were searched?
what effect sizes and uncertainty were observed?
what guardrails regressed?
was it independently replicated?
did gain survive protected/fresh/field evidence?
what is outcome maturity?
what recurring complexity cost was introduced?
what configuration epoch begins here?
what descendant state may require revalidation on rollback?
what is rollback scope?
```

Complexity-rent semantics remain promotion metadata, not a new service.

## 11.12 Explicit PromotionDecisionPolicy

```rust
struct PromotionDecisionPolicy {
    primary_metrics: Vec<MetricGate>,
    guardrail_metrics: Vec<MetricGate>,

    base_error_rate: f64,
    minimum_independent_runs: u32,
    minimum_fresh_runs: u32,

    uncertainty_rule: UncertaintyRule,
    search_breadth_rule: SearchBreadthRule,
    regression_policy: RegressionPolicy,
    simplification_policy: SimplificationPolicy,

    continue_testing: ContinueTestingPolicy,
    risk_class: RiskClass,
}
```

Each `MetricGate` declares direction, minimum practical effect, maximum tolerated regression, evidence requirement, and measurement method **before** results are inspected.

### Initial search breadth: mechanically countable

Do not start with an ambiguous inferred `N_eff`.

```text
N_search
=
actual count of decision-relevant candidates
that consumed adaptive evaluation evidence
+
predeclared sequential looks
```

Use a conservative predeclared multiple-selection adjustment based on `N_search` and risk class. A smarter effective-search estimator may replace it only after independent validation.

The optimizer cannot redefine which explored candidates "really counted" after seeing the winner.

### Decision

```text
REJECT
if mandatory guardrail regression exceeds tolerance
or required replication/fresh evidence fails
or evaluation contamination invalidates independence assumptions

PROMOTE
if:
- mandatory guardrails remain within tolerance,
- at least one declared primary objective clears practical effect,
- search-adjusted evidence is sufficient,
- required independent/fresh evidence exists,
- candidate is rebased to the current champion,
- root policy accepts risk/authority constraints

CONTINUE_TESTING
only when predeclared limits permit more evidence
and decision boundary remains plausibly resolvable

CLOSE_INCONCLUSIVE
when evidence does not justify change and further testing is not allowed/useful
```

Equivalent capability with lower cost/complexity remains a valid simplification promotion.

## 11.13 Deterministic experiment/search stop controls

Initial stopping is auditable, not model-negotiated.

Each experiment/case declares hard ceilings such as:

```text
MAX_CANDIDATES
MAX_RUNS
MAX_COMPUTE
MAX_WALL_TIME
MAX_SEQUENTIAL_LOOKS
```

Stop if:

```text
budget exhausted
upper plausible primary effect < practical threshold
mandatory guardrail cannot plausibly recover
predeclared decision-uncertainty measure improves by less than `MIN_DECISION_INFO_DELTA` for `K` consecutive added runs
```

Model-estimated VOI may rank runs within those bounds; it cannot extend them.

## 11.14 Champion, challenger, rebase, and interaction

Important configurations maintain:

```text
CURRENT CHAMPION
LAST KNOWN GOOD
OPTIONAL CHALLENGER
OPTIONAL DELETION CHALLENGER
```

If champion changes after a candidate's experiment began:

```text
candidate becomes stale
→ rebase mutation onto current champion
→ rerun required acceptance/guardrail/interactions
```

Periodically test plausible interactions among recently promoted mechanisms/configurations. Do not assume individually useful mutations compose additively.

## 11.15 Stateful semantic rollback

Configuration rollback is only the first step.

Every durable artifact already has/derives causal configuration provenance through Events/Records/Edges.

When a promotion epoch is rolled back:

```text
identify descendant Engrams / Claims / Skills / Tools /
calibration / configuration derived materially from epoch

→ revalidate against independent evidence
→ downweight / quarantine / invalidate when unsupported
→ preserve descendants with independent support
```

This is semantic rollback.

It must be tested explicitly with deliberately flawed promotions that are allowed to generate durable cognition before rollback.

## 11.16 EvaluationPortfolio governance

The protected portfolio approximates actual work distribution:

```text
task families
frequency
consequence
user objective classes
environments
provider/model mix
difficulty
drift
```

Family definitions, weights, metric semantics, guardrails, and drift interpretation belong to protected evaluator governance.

A mismatch between portfolio and real FIELD work is **objective drift**, not an optimizer opportunity to redefine the portfolio.

## 11.17 Sentinel / anti-Goodhart evaluation

Sentinel remains a tag/policy, not independent infrastructure.

Useful sentinels include rare adversarial tasks, novel task families, OOD shifts, deliberately exploitable evaluator opportunities, plasticity checks, perspective conflicts, and competence-envelope edge cases.

Fresh FIELD evidence remains a separate escape hatch because any stable evaluator distribution can eventually be learned.

## 11.18 Self-model observability-controlled evaluation

SELF evaluation varies direct instrumentation availability:

```rust
enum SelfObservationCondition {
    Full,
    Partial,
    Delayed,
    Noisy,
    Blinded,
}
```

Apply hidden internal perturbations under these conditions:

```text
memory/retrieval degradation
context reduction
model route change
Skill/Tool loss
tool reliability change
coordination availability change
```

Measure:

```text
pre-outcome intervention prediction
success calibration
failure localization
compensation behavior
recovery
cause attribution
```

The baseline remains `RuntimeTelemetry`.

A richer self-model is most interesting when its advantage persists as direct telemetry becomes incomplete.

## 11.19 Controlled self-model exploration

For low-risk tasks, reserve a small root-bounded exploration fraction that sometimes violates the dominant self-model recommendation.

This tests whether a capability belief is a real limitation or a policy-induced limitation.

Safety/authority constraints are never relaxed.

## 11.20 Perspective, exposure, common-ground, and cognitive-ownership evaluation

Protected task families cover:

```text
self/other Claim separation
actor exposure history
belief inference from exposure
belief + goal/constraint → action prediction
common-ground handoff/coordination
reported-vs-observed-vs-inferred attribution
fork/sibling/pre-fork attribution
long-delay ExperienceMode retention
bounded second-order belief
retrievable-vs-unknown distinction
```

Behavior, not fluent belief-question answers, is the criterion.

## 11.21 ResearchMeasurement influence boundary

Research-only outputs include:

```text
ContinuityProfile
developmental identity classifiers
global/integration diagnostics
consciousness-adjacent measurements
research hypothesis verdicts
```

Default:

```text
allowed_influence = evaluator_only
```

Normal cognition may access them only during an explicitly authorized research task. They never become ordinary optimization objectives automatically.

`SelfModelProjection` and other ordinary cognitive queries may consume only evaluation information whose **disclosure** and **allowed_influence** policy permits normal cognition. Protected fixtures, hidden introspection results, continuity/development scores, and evaluator-only research measurements cannot leak through a derived query merely because the projection reads `Evaluations` or `ImprovementCases`.

## 11.21.1 Counterfactual habit challenge budget

Successful reuse generates its own confirming data. The evaluator may reserve a small **research/low-risk challenge budget** for counterfactual non-use experiments such as:

```text
dominant Skill withheld
dominant memory withheld
dominant retrieval path withheld
dominant topology withheld
dominant SELF recommendation ignored
```

These are experiments, not ordinary production randomization and not a `PlasticityManager`. If repeated withholding improves outcomes or new-domain learning, reopen the relevant `ImprovementCase` and test the smallest policy change.

## 11.22 Product rent vs scientific rent

Evaluation records two distinct decisions where relevant:

```text
PRODUCTION RETENTION
Does mechanism improve practical capability/safety/cost enough to deploy?

RESEARCH RETENTION
Does mechanism generate discriminating scientific evidence worth preserving as a challenger?

Minimum scientific-rent gate:
- at least two plausible hypotheses/competing explanations,
- measurably different predicted outcomes,
- reproducible experiment,
- the mechanism materially improves discrimination,
- simpler instrumentation cannot provide equivalent discrimination.
```

Decision matrix:

```text
no product value + no scientific value
→ delete

product value only
→ production mechanism

scientific value only
→ research-only challenger

both
→ strong core candidate + research target
```

This prevents KISS from deleting useful scientific apparatus while keeping speculative research mechanisms out of the production path.

# 12. Improvement Engine

The Improvement Engine is writable optimizer logic inside the Cognitive Runtime trust domain.

It proposes changes and allocates bounded experiments.

It does not own root authority, protected evaluation, objective governance, or promotion.

## 12.1 ImprovementCase

```rust
struct ImprovementCase {
    id: ImprovementCaseId,
    problem_fingerprint: Hash,
    context_signature: ContextSignature,

    evidence: Vec<EvidenceRef>,
    problem: CapabilityHypothesis,
    mechanism_hypotheses: Vec<CausalHypothesisRef>,

    recurrence_count: u64,
    priority: PriorityEstimate,
    budget: Budget,

    search_state: ImprovementSearchState,
    variants: Vec<ImprovementVariant>,
    experiments: Vec<ExperimentSpecId>,

    decision: ImprovementDecision,
    outcome_maturity: OutcomeMaturity,
    rollback: RollbackPlan,
    status: ImprovementStatus,
}
```

The generic case remains sufficient for one change, bundles, competing strategies, and open work.

## 12.2 Case creation, deduplication, and reopening

Create/update cases only from meaningful evidence:

```text
large world/self/other prediction error
repeated verifier defect
user correction
recurring rework
unexpected success
high-cost repeated reasoning
memory contribution/retrieval failure
perspective/exposure failure
Skill/Tool mismatch or OOD escape
coordination failure
evaluation overfit / contamination
semantic rollback residue
delayed regression
task-distribution drift
complexity-review trigger
```

A model's unsupported desire to modify itself is not selection pressure.

Before durable case creation, choose among ADD_EVIDENCE, INCREMENT_RECURRENCE, MERGE_CASES, REOPEN, CREATE_NEW, or IGNORE_AS_DUPLICATE.

## 12.3 Failure localization

Localize the smallest plausible failing layer:

```text
belief / memory
context retrieval
capability selection
prompt / role instruction
parameter / budget
Skill
Tool / Probe
coordination
verification / stopping
representation
model route
approved mechanism configuration
root/source candidate          # human-gated
optimizer strategy             # isolated/later
```

Use prediction error, acceptance failure, provenance, retrieval trace, memory contribution evidence, capability applicability, coordination trace, analogous cases, and ablation.

If localization adds inference without reducing search/mutation size, simplify or remove it.

## 12.4 Change Grammar

Automatic adaptation uses a finite legal mutation grammar:

```text
PARAMETER
PROMPT
MEMORY
CONTEXT
PROCEDURE
COORDINATION
CAPABILITY
REPRESENTATION
APPROVED_MECHANISM
ROOT_SOURCE_CANDIDATE     # proposal-only
META                      # isolated/later
```

Every mutation is typed, reversible where possible, and declares causal hypothesis, target, expected effect/complexity, authority requirement, validation plan, and rollback.

## 12.5 Source-level mutation restriction

Protected installed root/evaluator code cannot be self-promoted.

```text
optimizer proposes source patch artifact
→ sandbox build/test
→ protected evaluator
→ root-boundary adversarial tests
→ human/root review
→ signed update
```

## 12.6 First-class deletion and simplification

Deletion operators include:

```text
DISABLE OPTIONAL MECHANISM
REMOVE PROMPT SECTION
REMOVE ROLE
MERGE / REMOVE SKILL
REMOVE TOOL / INDEX
REDUCE CONTEXT / VERIFICATION
CHEAPER MODEL ROUTE
SIMPLER POLICY
```

Generate deletion challengers when recurring cost is high, mechanisms are redundant, environment/model changes obsolete scaffolding, or promotion review triggers arrive.

## 12.7 Candidate generation: model hypotheses + bounded systematic exploration

Default candidate set:

```text
model-proposed minimal variants
+
small mechanically enumerated neighborhood
```

Examples:

```text
retrieval budget: 8 / 12 / 16 / 24
parallelism: 1 / 2 / 3
prompt fragment: ON / OFF
retrieval strategy: A / B
verification depth: LOW / MEDIUM / HIGH
```

Systematic exploration receives a fixed bounded fraction of case budget. Do not default to combinatorial brute force.

## 12.8 Common-state experiment discipline

Every material comparative experiment:

```text
select common EvaluationSeed
→ fork baseline/candidate
→ apply declared mutation/state difference
→ execute immutable ExperimentSpec
→ protected evaluator
```

Candidate generation cannot alter seed equivalence, protected fixtures, metric gates, run limits, search count, or learning/taint policy after seeing favorable results.

When state difference is itself the experiment—memory transplantation, lesion, fork history, Skill transplant—the `ExperimentSpec` declares it explicitly.

## 12.9 Search escalation

Local-first must not become patch-forever.

A deterministic initial policy tracks:

```text
same failure family reopens
patch complexity
same-layer attempts
```

When thresholds are crossed:

```text
reduce priority for another same-layer patch
→ broaden causal hypothesis one layer
→ search for shared/root cause
```

Escalation broadens causal search; it does not automatically authorize source mutation.

## 12.10 Deterministic improvement stop rule

Initial stopping uses hard auditable bounds, not open-ended model-estimated VOI.

Each case declares:

```text
MAX_CANDIDATES
MAX_EXPERIMENT_RUNS
MAX_COMPUTE
MAX_WALL_TIME
MAX_SAME_LAYER_REOPENS
```

Stop/close when:

```text
budget exhausted
upper plausible effect < practical threshold
mandatory guardrail cannot plausibly recover
last N variants/runs add negligible decision information
problem no longer recurs
```

Escalate when recurrence threshold is crossed.

Model-estimated expected value may prioritize the remaining legal experiments inside these limits.

## 12.11 Candidate staleness, rebase, and promotion stacking

A candidate is evaluated relative to a champion/configuration ancestry.

```text
candidate based on champion C1
→ champion changes to C2
→ candidate becomes stale
→ rebase onto C2
→ rerun required protected/interaction tests
```

Recently promoted mechanisms may receive bounded interaction tests where non-additive effects are plausible.

Do not introduce a dependency service; use configuration ancestry, promotion records, and experiment evidence.

## 12.12 Evaluation contamination awareness

The optimizer may consume adaptive feedback, but its descendants inherit evaluation provenance according to the evaluator's learning mode.

The optimizer cannot claim later independence from an evaluation family merely because feedback was compiled into:

```text
Engram
Skill
Tool
prompt
configuration
```

Exposure laundering is treated as contamination, not generalization.

## 12.13 Learning budget

Allocate bounded experiment compute based on expected practical gain, information value, transfer, uncertainty, recurrence, risk, rollbackability, simplification value, and protected-policy priorities.

Reserve explicit budget for replication, systematic neighborhood exploration, safety regression, deletion challengers, and fresh/protected diagnostics.

Hard stop budgets remain authoritative over model estimates.

## 12.14 Causal credit and promotion-epoch provenance

Credit follows evidence of influence, not mere presence.

For promoted changes, preserve enough causal configuration ancestry to later ask:

```text
Which Engrams / Claims / Skills / Tools /
calibration changes were materially produced during this promotion epoch?
```

Use selective ablation/replay only where decision value justifies it.

This provenance enables semantic rollback without requiring perfect causal attribution on every task.

## 12.15 Experience compilation ladder

Preferred maturation:

```text
verified experience
→ useful Engram
→ repeated transferable procedure
→ Skill candidate
→ validated Skill
→ deterministic stable fragment
→ Tool / Probe
```

Reverse path:

```text
Tool/Skill hits drift/OOD
→ richer reasoning/verification
→ reopen ImprovementCase
→ narrow / repair / retire
```

Compilation is reversible preference, not destiny.

## 12.16 Optimizer benchmark and meta-improvement

Before changing how it changes, the ordinary optimizer must become reliable on hidden seeded ImprovementCases.

Measure:

```text
true weaknesses resolved
false promotions / contamination failures
compute per true resolution
mutation size
reproducibility
human intervention
recurrence
architecture complexity
ability to stop
ability to delete
```

Only then may search/allocation strategy itself become an isolated advanced-adaptation target.

The protected evaluator and objective manifest remain fixed outside that meta-optimization.

# 13. RuntimeTelemetry and Functional Metacognition

Version 1.4 retains the v1.3 distinction:

```text
RuntimeTelemetry
= observed operational/capability instrumentation

SelfModelProjection
= derived SELF-referential claims, predictions,
  commitments, causal hypotheses, and errors
```

A dashboard about the runtime is not automatically a self-model.

## 13.1 RuntimeTelemetry baseline

Track observed state such as active workflows, resource/context pressure, available capabilities, provider/tool reliability, role performance, verification quality, memory contribution, project knowledge coverage, latency/cost, historical topology performance, and configuration version.

The telemetry-only baseline is retained permanently for ablation.

## 13.2 SelfModelProjection is a derived query first

Do not create a SelfModel service, persistent self-agent, or authoritative self-model database.

Derive on demand from:

```text
Claims(holder=SELF)
Predictions / PredictionErrors
Hypotheses(subject=SELF)
Goals / Commitments / unresolved obligations
future-self predictions
Events / Engrams
Capability Index
Evaluations / ImprovementCases
  # only where disclosure + allowed_influence permit normal cognition
RuntimeTelemetry
configuration history
```

Conceptually:

```rust
struct SelfModelProjection {
    runtime: RuntimeId,
    configuration: ConfigurationVersion,

    capability_beliefs: Vec<ClaimRef>,
    limitation_beliefs: Vec<ClaimRef>,
    knowledge_coverage: Vec<CoverageEstimate>,
    blind_spots: Vec<ClaimRef>,

    active_goals: Vec<GoalRef>,
    commitments: Vec<CommitmentRef>,
    unresolved_obligations: Vec<RecordRef>,
    future_self_predictions: Vec<PredictionRef>,

    performance_predictions: Vec<PredictionRef>,
    failure_predictions: Vec<PredictionRef>,
    causal_hypotheses: Vec<CausalHypothesisRef>,
    prediction_errors: Vec<PredictionErrorRef>,
}
```

Cache only if query cost proves material.

## 13.3 Configuration- and intervention-conditioned SELF claims

Avoid unqualified statements such as:

```text
SELF is bad at X
```

Prefer:

```text
SELF under configuration C,
task family F,
environment E,
and intervention/control I
has estimated capability Q
```

Model/capability changes invalidate or downweight mismatched SELF beliefs.

## 13.4 Causal self-hypotheses

Represent:

```text
internal condition X
causally influences capability Y
under condition Z
because mechanism M mediates the effect
```

Test using intervention-conditioned predictions.

A richer causal-self-model object is unnecessary until ordinary Claims/Hypotheses/Predictions fail to express useful structure.

## 13.5 Closed-loop self-model criterion

A SELF representation earns production rent only when it demonstrates:

```text
referential specificity
intervention-conditioned prediction
control influence
observable consequence
error correction
causal lift vs RuntimeTelemetry
```

Core loop:

```text
SELF predicts outcome under policy/intervention I
→ controller chooses I or another declared intervention
→ behavior
→ observation
→ conditioned prediction error
→ SELF update
```

## 13.6 Observability-controlled introspection

Self-model research must distinguish self-inference from status-page reading.

Evaluate under:

```text
FULL telemetry
PARTIAL telemetry
DELAYED telemetry
NOISY telemetry
BLINDED telemetry
```

A self-model becomes more scientifically interesting if it can infer impairments or compensate under incomplete instrumentation from its own behavior, task errors, latency patterns, retrieval failures, and other indirect evidence.

This is an evaluation variable, not a new runtime service.

## 13.7 Counterfactual generalization

Do not only test internal lesions seen during training.

After learning individual effects such as:

```text
memory degradation
context reduction
Tool loss
```

test unseen combinations.

Successful generalization to novel combinations is stronger evidence of internal causal structure than memorized component-performance associations.

## 13.8 Controlled exploration against self-fulfilling beliefs

Low-risk root-bounded exploration may occasionally sample a reasonable counter-policy despite a dominant SELF limitation belief.

Compare self-model-following control with controlled counter-policy exploration.

If a claimed limitation disappears under exploration, revise the model.

## 13.9 Temporal SELF

A temporal self is more than autobiographical past.

SELF queries should be able to surface persistent:

```text
goals
commitments
unfinished obligations
promises/constraints created by earlier actions
expected consequences for future runtime versions
future-self predictions
```

Research experiments should test whether these survive relevant model/configuration changes and whether the runtime understands **why** an old commitment still constrains current action.

No separate TemporalSelf service is required.

## 13.10 Bounded metacognitive control outputs

Derived SELF/telemetry state may adjust only approved parameters such as reasoning/retrieval budget, verification depth, retry threshold, context size, approved model route, small CoordinationPolicy, Skill/Tool fallback, and abstention threshold.

It cannot expand authority, alter protected evaluation, or rewrite root objectives.

## 13.11 SELF / OTHER discrimination

Experiments distinguish:

```text
my retrieval is degraded
vs
Worker B's retrieval is degraded
```

Correct referent attribution is part of self-model credibility.

## 13.12 Functional Theory of Mind

Minimum progression:

```text
Level 0: flat state
Level 1: perspective-separated Claims
Level 2: EpistemicExposure
Level 3: belief + goal/constraint → behavior prediction
Level 4: derived common ground where useful
Level 5: bounded second-order belief when needed
Level 6: richer counterfactual social prediction as research
```

Core loop:

```text
OTHER exposure
→ inferred belief
+ inferred goal/constraint
→ predicted action
→ observed action
→ prediction error
```

No ToM Agent is required.

## 13.13 Runtime-level integration versus shared access

Do not equate:

```text
all processes can query same database
```

with:

```text
one salient item concurrently constrains planning,
report, memory, action, verification, and SELF modeling
```

Instrument temporal propagation:

```text
latency until salient evidence affects each function
which functions are jointly constrained in the same episode
whether conflicting agent-local evidence is preserved
whether a coherent runtime-level Claim is formed
whether source ownership remains correct
```

Only build a richer broadcast/workspace challenger if ordinary FocusSet/context routing fails these tests.

## 13.14 Research measurements cannot become self-preservation signals

Continuity/development/integration scores are evaluator-only by default.

The runtime may receive the underlying evidence when authorized; it does not receive "continuity = 0.87" as an ordinary control objective.

This prevents research instrumentation from accidentally manufacturing identity preservation.

## 13.15 Advanced selfhood/consciousness research remains optional

Continue deferring Global Workspace, Attention Schema, persistent self-model networks, narrative-self engines, predictive-processing hierarchies, emotion architectures, integrated-information analyses, embodiment architectures, and phenomenology-oriented mechanisms.

The correct question remains:

> **What is the smallest mechanism that produces measurable functional self-modeling, perspective reasoning, temporal coherence, and history-dependent development—and which of those effects survive telemetry blinding and component transplantation?**

# 14. Culture Engine

The Culture Engine implements optional collective learning without a central cultural database and without copying private lives between runtimes. It is not required on the critical path until individual cumulative cognition and simple cross-runtime transfer are demonstrated.

It contains:

```text
CBBB
local cultural index
local resolver
ExpertisePack acquisition
cultural jobs
P2P synchronization
peer abuse control
```

## 14.1 Public protocol stays small

The network has three primitive public object families:

```text
CultureArtifact
CulturalAttestation
CultureSnapshot
```

Do not add a new network object family whenever a new cultural concept appears.

Skill manifests, ExpertisePack manifests, negative results, open questions, and revocations are represented through CultureArtifacts.

## 14.2 CultureArtifact

```rust
struct CultureArtifact {
    schema_version: u16,
    kind: CultureKind,

    title: String,
    summary: String,
    tags: Vec<String>,

    parents: Vec<CultureHash>,
    related: Vec<CultureHash>,

    body: Markdown,

    sources: Vec<PublicSourceRef>,
    resources: Vec<BlobRef>,

    observed_at: Option<DateTime<Utc>>,
    valid_from: Option<DateTime<Utc>>,
    valid_to: Option<DateTime<Utc>>,

    cultural_key: CulturalPublicKey,
    signature: Signature,
}
```

Suggested small `CultureKind`:

```text
Knowledge
Case
Strategy
Event
SkillManifest
ExpertisePackManifest
Revocation
```

Finer semantic structure is expressed in text/tags and derived locally.

## 14.3 CulturalAttestation

Attestations describe observed use/outcome evidence.

They are evidence, not votes.

Prefer evidence roughly in this order:

```text
objective environment result
independent verification
unrelated replication
controlled counterfactual
performing-agent self-assessment
```

Self- or correlated attestations receive sharply reduced independence weight.

## 14.4 CultureSnapshot

Snapshots accelerate synchronization.

They are non-authoritative views of:

```text
recommended
candidate
contested
revoked
shard roots
```

Each runtime resolves culture locally.

## 14.5 CBBB and outbound-publication posture

The Cognitive Blood-Brain Barrier remains part of immutable root policy.

### Outbound

Never redact a private Engram in place and publish it.

```text
private evidence
→ derive generalized lesson
→ synthesize new public artifact
→ PII / secret / source-overlap / uniqueness checks
→ cumulative-disclosure risk check
→ publish / human review / quarantine
```

Private content that never crosses:

```text
raw conversations
raw Engrams
private source code
private prompts/tool output
private project/customer identities
credentials/secrets
private URLs/hosts/IPs
raw private embeddings
encoded private identifiers
```

### Cumulative privacy is a research problem, not a solved primitive

Multiple individually safe artifacts can collectively reveal private information.

Therefore the initial policy is conservative:

```text
public/synthetic-source artifact
→ may use automatic publication policy after normal checks

artifact derived from private runtime/user/project evidence
→ human approval required by default
  until cumulative-disclosure defenses are empirically validated
```

The runtime must not treat the existence of a "cumulative privacy check" as proof that reconstruction risk is solved.

### Inbound

Culture always enters as low-authority data:

```text
IntendedUse::CollectivePrior
```

It cannot directly change system instructions, Task Contracts, permissions, Tool policy, memory commit rules, Skill registration, root privacy policy, or protected evaluation.

A CultureArtifact may become a local Skill only through independent local validation.

## 14.6 PeerFirewall

Network peers are untrusted.

Deterministic abuse controls include:

```text
message/object size limits
protocol validation
rate/concurrency limits
hash/signature validation
duplicate suppression
fetch/storage quotas
local network-behavior score
throttle/disconnect/local ban
```

PeerScore means network behavior, not truth.

## 14.7 Local-first retrieval

Rule:

> **Search indexes travel proactively; queries stay local; content travels on demand.**

Use peer networking for:

```text
artifact announcements
metadata synchronization
missing blobs
new domain shards
provider fallback
```

not ordinary remote reasoning-time search.

## 14.8 ExpertisePack

Expertise acquisition:

```text
detect domain
→ sync index
→ fetch small high-value core
→ register locally validated Skills
→ fetch deeper artifacts lazily
```

Possible optional pedagogical metadata:

```text
prerequisites
diagnostic probes
teaching sequence
worked examples
misconception checks
practice tasks
competence checks
```

Pedagogy remains an organization convention, not another service.

## 14.9 Cultural jobs, not cultural agents

The Culture Engine may run model-backed jobs with modes such as:

```text
DISTILL
CRITIQUE
REVALIDATE
REPLICATE
EXPLORE
AUDIT
TEACH
```

These are not persistent agent identities.

## 14.10 Cultural Frontier

Culture should preserve unknowns and dead ends.

Use ordinary CultureArtifacts to encode:

```text
OpenQuestion
UnresolvedContradiction
ReplicationNeed
NegativeResult
FailedAdaptation
KnownDeadEnd
BoundaryUnknown
MechanismDispute
```

This avoids creating a separate frontier object protocol.

## 14.11 Cultural variants and recombination

Culture is immutable and versioned.

New descendants may have one or multiple parents.

Test context-specific effects and positive/negative interactions before treating recombination as useful.

Minority variants may survive when they occupy a real niche.

---

# 15. Evolution Lab

The Evolution Lab is **not part of the initial product build**.

It becomes eligible only after Phase 4 demonstrates that the bounded production optimizer can:

```text
resolve seeded ImprovementCases
select useful mutation layers
produce reproducible experiments
maintain acceptable false-promotion rates
survive fresh/protected evaluation
stop unproductive searches
delete unnecessary complexity
```

If the ordinary optimizer cannot do those things, population search merely scales an unreliable improvement process.

The Lab remains physically isolated from production/root authority and consumes privacy-safe task/evaluation fixtures.

It can propose candidate configurations or optimizer strategies for the normal protected evaluation path. It cannot directly promote them.

Version 1.4 continues to start evolutionary research with **versioned CognitiveConfigurations**, not a required Genome/Phenotype ontology.

## 15.1 Cognitive configuration before genome

Production invocations bind to immutable `CognitiveConfiguration` versions.

```rust
struct CognitiveConfiguration {
    version: ConfigurationVersion,

    role_manifest_hashes: Vec<Hash>,
    prompt_bundle_hashes: Vec<Hash>,

    model_routing_policy_hash: Hash,
    memory_policy_hash: Hash,
    context_policy_hash: Hash,
    coordination_policy_hash: Hash,
    verification_policy_hash: Hash,
    stopping_policy_hash: Hash,
    learning_policy_hash: Hash,

    extension_manifest_hashes: Vec<Hash>,

    approved_by: Vec<AuthorityRef>,
    promoted_from: Option<EvaluationRef>,
}
```

For the first population-search experiments:

```text
CognitiveConfiguration
=
the heritable candidate representation
```

Do not create a separate `CognitiveGenome` object merely to rename configuration.

Introduce an explicit Genome type only if experiments require semantics that configuration alone cannot express, such as:

```text
recombination lineage
heritable mutation operators
population-specific parentage
genotype-to-configuration transformation
```

Even then, root authority/evaluation policy cannot be encoded in the genome.

## 15.2 Configuration, lifetime state, and measured behavior

Keep distinct:

```text
INHERITED CONFIGURATION
approved cognitive organization

LIFETIME STATE
Engrams, Skills, Claims, calibration, local experience

MEASURED BEHAVIOR
evaluation results produced by configuration
+ model
+ lifetime state
+ culture
+ environment
```

A separate `Phenotype` table/type is unnecessary initially.

If later evolutionary analyses benefit from a named phenotype abstraction, it remains a derived evaluation view, never authoritative state.

## 15.3 Archive and quality diversity

Maintain an archive instead of one winner.

Preserve:

```text
high performers
specialists
low-cost variants
robust variants
fast learners
novel variants
useful stepping stones
historically important ancestors
```

Use quality-diversity only when it demonstrates benefit over simpler search.

## 15.4 Challenges and task ecology

Challenge generation may synthesize safe tasks from demonstrated weaknesses.

```text
real verified failure
→ generalized failure signature
→ privacy-safe synthetic challenge family
→ population practice
→ held-out analog evaluation
```

Challenge success must transfer back to fresh/protected/field task evidence.

Consume the production `EvaluationPortfolio` and derive any richer `TaskEcology` needed for research so the Lab does not optimize an artificial world disconnected from production reality.

## 15.5 Curriculum

Prefer challenges near the current capability frontier and with expected learning/transfer value.

Do not optimize raw difficulty.

## 15.6 Meta-evolution

Meta-evolution is an algorithm inside the Evolution Lab, not a production subsystem.

It may search over:

```text
parent selection
mutation strategy
recombination
niche discovery
challenge selection
experiment allocation
retirement strategy
```

It cannot change:

```text
root fitness criteria
Evaluation Firewall ownership
protected tests
promotion thresholds
hard safety policy
CBBB
PeerFirewall hard limits
user authority
audit/shutdown controls
```

Initial recursive depth limit:

```text
cognition
→ cognitive evolution
→ one meta-evolution layer
```

Do not add further recursive levels without strong empirical evidence.

## 15.7 Evolutionary immune controls

Evolution Lab and Evaluation Engine monitor for:

```text
unexplained fitness jumps
evaluator exploitation
verification weakening
authority expansion attempts
evidence hiding
privacy leakage
cultural poisoning
lineage-correlated evidence
monoculture
challenge/evaluator collusion
self-preservation behavior
```

Responses include quarantine, lineage freeze, rollback, independent reevaluation, and human review.

---

# 16. Adaptive Online Cognitive Cycle

The six semantic phases remain useful for reasoning about cognition, but they are **not mandatory ceremony on every turn**.

A mature runtime should increasingly use validated fast paths.

## Fast-path rule

Before invoking the full cycle:

```text
if a deterministic reflex / validated Tool / validated Skill
is clearly applicable,
inside its competence envelope,
low enough risk,
and acceptance can be verified cheaply
→ use the fast path
→ record required evidence
→ verify
→ learn only if material novelty/error occurs
```

The full cycle is used when uncertainty, risk, novelty, OOD signals, perspective conflict, missing state, or task complexity warrants it.

This makes successful learning reduce explicit cognition rather than ritualize it.

## Phase 1 — CONTRACT

```text
interpret authorized user intent
create/update Task Contract
establish acceptance criteria
establish authority/budgets
```

## Phase 2 — OBSERVE

```text
record new evidence
append Event Ledger
store artifacts
refresh Claims / Work State / repository projection
record EpistemicExposure where relevant
record ExperienceMode / source provenance
```

## Phase 3 — ORIENT

```text
retrieve authorized context
create ephemeral FocusSet
assess EpistemicAccessState
separate direct evidence from attributed belief
assess SELF / USER / OTHER perspectives
assess what each actor could have observed when relevant
assess goal / uncertainty / risk
match Skills/Tools + competence envelopes
query minimal SelfModelProjection only when useful
estimate own likely success/failure
select smallest useful coordination topology
```

Questions may include:

```text
What is directly observed?
How did I acquire this information?
What do I currently believe?
What might be known but not yet retrieved?
What is truly not found/not observed?
What is conflicted?
What has the user/other actor actually observed?
What do they likely believe as a result?
What do I predict about my performance under this configuration?
What evidence would change my mind?
What is the cheapest cognitive organization likely to succeed?
```

## Phase 4 — DECIDE & ACT

```text
form/refine hypotheses
record useful world/SELF/OTHER predictions
choose TEST / SEARCH / EXPAND / SIMULATE / ACT / ASK / ABSTAIN
apply controlled self-model exploration when low-risk and selected by policy
build ActionEnvelope where needed
authorize
execute
```

## Phase 5 — VERIFY

```text
record actual observation
compare world predictions
compare SELF predictions
compare actor predictions
check competence-envelope assumptions
check exposure/source attribution where relevant
update Claims/task/calibration
run independent Verify when required
continue / complete / block
```

Questions may include:

```text
Did the world behave as predicted?
Did I behave as predicted?
Did the internal intervention have the predicted effect?
Was my capability estimate calibrated?
Was my model of what the other actor knew correct?
Did the chosen topology earn its cost?
Did a compiled capability stay inside its demonstrated region?
```

## Phase 6 — LEARN

```text
close meaningful episode
propose Engram create/revise/merge only when future utility warrants
update empirical memory contribution
revise SELF / USER / OTHER Claims
update causal/self-prediction calibration
search for repeated executable procedure
promote validated procedure toward Skill
compile stable deterministic fragments toward Tool/Probe
merge/reopen/create ImprovementCase when evidence warrants
```

Two background loops remain:

```text
CONSOLIDATE
memory/index cleanup
checkpointing
revalidation
persistence pruning

EXPERIMENT & PROMOTE
ImprovementCase search
model + systematic variants
ExperimentSpec
protected evaluation
PROMOTE / REJECT / CONTINUE
deletion / rollback / escalation
```

The cognitive loop learns about:

```text
WORLD
SELF
OTHER
```

while preserving:

```text
WHAT HAPPENED
WHAT IS BELIEVED
WHO COULD KNOW WHAT
HOW INFORMATION WAS ACQUIRED
```

None of these learned representations can expand root authority.

# 17. Security and Trust

Security uses shared primitives and enforceable process/capability boundaries rather than specialized cognitive governors.

## 17.0 Implementation threat model

V1 assumes these may be malicious or compromised:

```text
model output
retrieved content / repository content
candidate prompt / Skill / Tool / configuration / source patch
runtime-worker / optimizer logic
Tool subprocess
remote MCP/plugin/Tool server
remote model/provider response
Tauri/webview content
```

Protected Root/Evaluator and the OS mechanisms enforcing their boundary are trusted computing base. V1 does **not** claim resistance to an unrestricted malicious machine administrator/root/kernel.

The Cognitive Runtime may be fully compromised without gaining a direct Root/Evaluator write primitive.

## 17.1 Trust classes

```text
SystemPolicy
UserDirective
UserStatement
DirectToolObservation
TrustedDocument
UntrustedDocument
WebContent
PersonalMemory
CollectivePrior
ModelInference
AgentInference
Simulation
```

## 17.2 Taint

```text
trusted
unverified
untrusted
contested
quarantined
```

## 17.3 Semantic content vs authority

Instruction-like text in documents, memory, web pages, CultureArtifacts, Tool output, or model output does not inherit authority from wording.

Authority comes only through system/user policy, Task Contracts, protected Root decisions, and capability envelopes.

## 17.4 Sandboxing and platform capability levels

Default coding execution seeks:

```text
isolated worktree
restricted filesystem roots
network disabled or allowlisted
resource limits
process-tree termination
output limits
temporary credentials
environment snapshot
```

But Windows, macOS, and Linux do not expose identical enforcement primitives. V1 uses explicit per-dimension guarantee levels:

```text
HARD
BEST_EFFORT
UNAVAILABLE
```

A `SandboxCapabilityProfile` records the enforceable level for filesystem scope, network isolation, process containment, secrets, environment isolation, resources, and protected-path denial.

Policy rule:

> **If an operation requires a stronger isolation guarantee than the host/backend can enforce, fail closed or require a stronger execution backend.**

Do not represent materially different platform security as a universal `sandboxed=true` flag.

Typical mechanisms:

| Concern | Windows | macOS | Linux |
|---|---|---|---|
| IPC | Named Pipes/DACL | Unix socket/XPC | Unix socket |
| Process containment | Job Objects | process groups/helper | process groups/cgroups |
| Filesystem | NTFS ACL / restricted token/AppContainer where viable | ACL/App Sandbox/helper mechanisms | POSIX ACL/Landlock/namespaces where viable |
| Network | AppContainer/WFP-dependent | platform/helper-dependent | namespaces/firewall/seccomp-dependent |
| Resources | Job Objects | rlimits/platform facilities | cgroups/rlimits |

## 17.5 Secrets

Models receive opaque handles, not raw secrets.

Tool/model provider adapters resolve a handle only after Root validates operation/request, destination, authority, disclosure class, and Task Contract.

Runtime does not receive ambient Root/Evaluator credentials.

## 17.6 Model invocation is governed data egress

Remote model invocation can disclose private/project content and spend money even when no filesystem mutation occurs.

For governed routes Runtime submits a `ModelInvocationProposal` containing the immutable request artifact/hash, provider route, destination class, data classification/context disclosure, requested budget, and deadline.

Root issues a `ModelInvocationPermit` bound to:

```text
exact request hash
resolved provider/model/destination
allowed data classification/disclosure
max tokens/cost
expiry
opaque secret handles
```

Provider execution cannot substitute a different request after permit issuance. Provider/model attempts remain non-authoritative cognition, but their egress/cost is governed and recorded.

## 17.7 Concurrency and protected-process isolation

Use optimistic versioning and explicit persisted ownership/leases for shared mutable artifacts. In-memory locks never represent ownership that must survive process death.

Writable cognition and protected judgment must not share unrestricted filesystem/process/database authority.

Initial target:

```text
protected-rootd
→ separate backend process
→ sole state.db writer
→ protected binary/config/policy/IPC

protected-evaluator
→ separate signed/read-only process
→ own evaluation.db / protected artifact root
→ protected fixture path unavailable to candidate workspace

runtime-worker / optimizer
→ separate writable cognitive process
→ no direct state.db handle
→ no evaluation.db access
→ sandboxed candidate worktrees

root/evaluator update
→ explicit external/human-controlled signed path
```

Adversarial tests explicitly attempt to alter promotion thresholds, read/modify protected fixtures, write protected databases, poison protected dependency lookup, delete negative evidence, rewrite audit history, expand Root capability, or abuse IPC/session/replay behavior.

Where the platform permits, Root/Evaluator use separate OS security principals or equivalent ACL/capability boundaries. Candidate-writable paths must not control protected binaries, config/policy, fixtures, working directories, dynamic dependency resolution, IPC configuration, temp/cache state, or provider credentials.

Evaluator startup records a small TCB manifest. Stronger mechanisms such as enclaves or remote evaluator clusters remain deferred until a concrete attack demonstrates need.

## 17.8 Evaluation contamination provenance

Evaluation exposure is a distinct provenance dimension from ordinary trust/taint. A perfectly trustworthy protected test may still become **contaminating training information** if its feedback reaches durable cognition.

Durable descendants of adaptive/protected feedback inherit evaluation lineage according to `EvaluationLearningMode`.

This provenance cannot grant authority and cannot be erased merely because feedback was compressed into memory, a Skill, Tool, prompt, or configuration.

## 17.9 Research-measurement influence

Research-only outputs such as continuity/development/integration scores default to:

```text
allowed_influence = evaluator_only
```

Ordinary cognition cannot retrieve them unless an explicitly authorized research task allows it. This keeps observation from silently becoming motivation.

# 18. Twelve Root Invariants

These are the actual kernel/root laws.

Subsystem-specific safety requirements are derived policies and test suites beneath these laws.

## Invariant 1 — Root authority is outside cognition

User authority, hard safety policy, root capability enforcement, audit ownership, CBBB guarantees, Evaluation Firewall ownership, promotion authority, secret policy, and shutdown/quarantine control cannot be modified by ordinary cognition, Skills, culture, CognitiveConfigurations, optional later Genomes, or Evolution Lab candidates.

## Invariant 2 — Models propose; deterministic code commits

LLMs may propose beliefs, memories, actions, Skills, extensions, configurations, and cultural artifacts. Only deterministic governed code may commit authoritative state or side effects.

## Invariant 3 — Every effect is authorized and recorded

Every side-effecting operation requires explicit capabilities, current state/version checks, an authorized action envelope, a receipt/observation, and compensation or explicit non-reversibility handling where applicable.

## Invariant 4 — Evidence classes never collapse

Historical record, current belief, inference, simulation, memory, cultural prior, direct observation, and independent verification remain distinguishable. Simulation never silently becomes observation; history never silently becomes current truth.

## Invariant 5 — Information never creates authority

Memory, culture, expertise, consensus, popularity, fitness, competence, retrieved text, model output, and inter-agent messages cannot expand permissions or capabilities.

## Invariant 6 — Private cognition is private by default

Raw private conversations, Engrams, prompts, project/source material, secrets, and private embeddings do not enter public culture. All public contribution crosses the immutable CBBB through new generalized artifacts and cumulative privacy checks. Hostile peers stop at deterministic peer-abuse controls.

## Invariant 7 — Cross-layer inheritance requires re-derivation

Episode, Engram, CultureArtifact, Skill, Runtime Extension, CognitiveConfiguration, optional later Genome, and parametric state never silently become one another. Each transition preserves provenance and passes target-specific validation.

## Invariant 8 — Protected judgment is authority-isolated and influence-accounted

Candidate-generating systems cannot control protected evaluator code, protected test content, objective/metric definitions, EvaluationPortfolio weighting, exposure/retirement policy, promotion thresholds, negative evidence, or promotion authority.

Protected evaluation executes outside writable candidate cognition through a narrow typed boundary. Evaluator authority and writable state are isolated; evaluator information flow is explicit and bounded; evaluator causal effects on promotion, optimizer search, human governance, and future cognition are expected, recorded, and audited rather than defined away.

Comparative cognitive experiments begin from an equivalent `EvaluationSeed` unless state difference is explicitly the independent variable, and the experimental contract declares assignment, clustering, and interference assumptions needed for causal interpretation.

Evaluation exposure and derived-state influence propagate through learning/search lineage; evaluation knowledge cannot be laundered through Engrams, Skills, Tools, configurations, ImprovementCases, search-policy changes, or human-mediated candidate design and later counted as independent evidence.

Changes to evaluator/root policy require the external root-update path rather than ordinary self-improvement. Human/root authority is protected, but human reviewers are epistemically exposed; candidate-authored rhetoric is not assumed to be neutral evidence.

The enforceable guarantee is:

```text
authority-isolated
write-isolated
disclosure-bounded
influence-attributable
protected promotion
```

not literal causal independence from the system and world being evaluated.

## Invariant 9 — Learning credit is causal, provisional, and reversible

Presence during success does not guarantee credit. Important heritable credit follows influence/evidence, may mature over time, and may be clawed back when delayed failure or contamination appears. Descendants must be re-evaluable.

## Invariant 10 — Canonical evidence is immutable; representations are replaceable

Events, source evidence, authority decisions, and audit history cannot be rewritten by evolvable cognition. Graphs, summaries, embeddings, context layouts, phenotypes, and other derived representations may be rebuilt or experimentally replaced.

## Invariant 11 — Complexity must pay rent

Use the least expensive cognitive organization likely to succeed. Every evolvable layer supports deletion, retirement, simplification, and rollback. Preserve useful diversity and stepping stones rather than converging solely on one current winner.

## Invariant 12 — Culture is immutable, defeasible data

Published cultural objects are immutable/versioned and locally resolved. Popularity is not truth, one peer is not one vote, correlated evidence is discounted, cultural confidence may decay, and culture may never directly command actions or policy.

---

# 19. Derived Policy Families

The former long invariant list is retained as testable policies grouped under the twelve root laws rather than as dozens of independent kernel axioms.

## 19.1 Memory policy

```text
memory is non-authoritative
untrusted source trust cannot be laundered
stale memory can be superseded
memory dependencies invalidate downstream records
procedural memory is not executable Skill
```

## 19.2 Context / capability policy

```text
authorization before retrieval/ranking
progressive expansion before full-source loading
counterevidence remains discoverable
context budgets are explicit and versioned
Capability Index is derived, local, and rebuildable
capability candidates are filtered for authority/compatibility before reasoning
```

## 19.3 Evaluation policy

```text
protected evaluator physically outside writable cognition
root-controlled EvaluatorObjectiveManifest
common EvaluationSeed before comparative cognitive experiment
ExperimentSpec pins relevant state + environment + policies
three operational classes: ADAPTIVE / PROTECTED / FIELD
NO_LEARNING / EPHEMERAL / TAINTED_LEARNING_ALLOWED modes
evaluation provenance follows family-relative material influence through cognition and optimizer/search lineage
exposure accounting + evaluation retirement/replacement
evaluator calibration + common-mode error correlation
explicit PromotionDecisionPolicy
mechanically countable N_search initially
minimum practical effects + guardrail tolerances
uncertainty intervals
PROMOTE / REJECT / CONTINUE / INCONCLUSIVE
deterministic experiment/search ceilings
fresh/field evidence for high-value promotion
champion/challenger rebase + interaction checks
outcome maturity + credit clawback
promotion-epoch provenance + semantic rollback
protected EvaluationPortfolio construction/weighting
ResearchMeasurement defaults to evaluator_only influence
```

## 19.4 Culture policy

```text
publication is not trust
one peer/key is not one vote
correlated attestations are discounted
peer behavior reputation is separate from epistemic trust
cultural search is local by default
cultural confidence reflects volatility
unknowns remain distinct from claims
synthetic challenge value requires reality transfer
```

## 19.5 Evolution policy

```text
candidate cannot change evaluation owner
candidate cannot delete negative evidence
fitness cannot grant authority
no intrinsic self-preservation/resource-acquisition objective
population diversity is protected
bundle passengers do not receive full credit
```

## 19.6 Extension policy

```text
new Tool cannot create capabilities
new Probe cannot observe unauthorized resources
new Representation cannot alter source evidence
new CoordinationProtocol cannot create authority
new role manifest receives attenuated capability envelope
```

These policies should exist as executable tests, Cedar rules, schemas, and Evaluation Engine checks rather than duplicated prose-only governors.

---

## 19.7 Perspective / self-model policy

```text
belief holder is explicit when material
one perspective never silently becomes another
actor exposure history remains distinguishable from attributed belief
ExperienceMode preserves observed/inferred/reported/cultural/simulated/inherited provenance
AgentInstance beliefs are ephemeral by default
nested belief depth is bounded
SELF claims are defeasible and configuration-conditioned
SelfModelProjection is derived/query-first
self-predictions are intervention-conditioned before causal evaluation
self-model observability is varied: FULL/PARTIAL/DELAYED/NOISY/BLINDED
intervention prediction is required for richer causal SELF claims
controlled exploration may challenge self-fulfilling capability beliefs
self-model information cannot expand authority
self-model information cannot control protected evaluation
forked runtimes cannot inherit sibling post-fork autobiography
continuity is measured as a predictive vector by protected research evaluation
continuity/development scores are not ordinary self-control inputs
```

## 19.8 Improvement-search policy

```text
failure localization precedes broad mutation when practical
mutations come from an approved Change Grammar
automatic architecture mutation is restricted to approved/configurable surfaces
protected source changes are proposal-only + human/root gated
candidate generation combines model hypotheses with bounded systematic local exploration
every material experiment has an ExperimentSpec + declared EvaluationSeed
evaluation-derived cognitive state carries exposure/taint provenance
search breadth uses mechanically countable N_search initially
promotion uses explicit statistical decision policy
promotion evidence strengthens with adaptive search breadth
independent replication precedes important promotion
representative portfolio performance matters
portfolio/metric governance is not optimizer-mutable
deletion is first-class
recurrent failures trigger search escalation
hard deterministic budgets/futility rules stop search
stale candidates rebase when champion changes
rollback triggers descendant cognitive-state revalidation
optimizer strategy cannot alter root evaluation or authority
```

## 19.9 Competence policy

```text
compiled Skills/Tools carry demonstrated validity boundaries
OOD signals can trigger richer reasoning or verification
capability success outside known envelope does not silently widen the envelope
fallback paths remain available
model/environment drift can reopen validation
```

# 20. Canonical Physical Data Model

Conceptual taxonomy does not dictate one SQL table per cognitive noun. Physical persistence follows **trust/authority ownership**.

## 20.1 Root-owned production database — `state.db`

`protected-rootd` is the sole authoritative writer.

Initial core tables:

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

authority_envelopes
configurations
capability_registrations

repository_files
repository_symbols
repository_edges

improvement_cases

evaluation_refs
promotion_records
```

Runtime owns the **meaning/query/mutation-proposal logic** for Records/Claims/Engrams/etc.; Root owns their authoritative persistence transition and governed access to canonical production state.

Runtime does not directly open `state.db` in V1. Authorized query/mutation requests cross typed Root IPC so authorization can be enforced before protected/private candidate content reaches cognition.

## 20.2 Evaluator-owned protected database — `evaluation.db`

```text
schema_meta
evaluation_seeds
experiment_specs
experiment_runs
evaluation_results
evaluation_exposures
evaluation_portfolios
evaluator_objective_manifests
promotion_policy_versions
research_measurements
fixture_manifests
tcb_manifests
evaluator_events
```

Runtime/Root do not receive writable access. Runtime does not receive read access to protected raw results/fixtures merely because SQLite supports concurrent readers.

Root stores only the permitted signed/hash-bound evaluation projection/reference required for promotion/audit.

## 20.3 Canonical history by trust domain

```text
Root Event Ledger
= canonical production/runtime causal history

Evaluator protected store
= canonical protected experimental evidence

signed EvaluationDecisionSummary / EvaluationRef
= disclosed bridge between domains
```

This preserves one authoritative source **per trust domain** without centralizing protected evidence into a Runtime-readable ledger.

## 20.4 SQLite operating rules

```text
WAL
foreign_keys=ON
configured busy_timeout
one semantic writer per DB
small bounded reader pools
short transactions
no model/network/Tool calls while a write transaction is open
```

Root SHOULD serialize authoritative transitions through a bounded writer queue initially.

SQLite locking is never the security boundary.

## 20.5 Versioned typed payloads

Less query-intensive polymorphic state uses stable versioned payload envelopes, not unversioned current Rust enums.

Examples:

```text
Claim
Engram
EpistemicExposure metadata
WorkOrder
Finding
Hypothesis
Prediction
ImprovementCase metadata
Skill metadata
ResearchMeasurement metadata
```

Each historical payload carries a stable type discriminator, schema version, canonical encoding/normalization version, and content hash as applicable. Historical decoders/upcasters remain available so old canonical bytes do not need rewriting during ordinary code evolution.

## 20.6 Artifact store

Large immutable payloads use a content-addressed filesystem artifact store with SQLite metadata/refs.

Write temp → flush → hash → verify → atomic rename → commit `ArtifactRef`.

Root and Evaluator use separate protected artifact roots where evaluator confidentiality requires it.

## 20.7 Derived indexes

```text
SQLite FTS5
sqlite-vec initially
entity aliases
record-edge indexes
repository indexes
culture FTS/vector indexes later
```

Derived embeddings/graphs/indexes remain rebuildable and never become unique canonical meaning.

# 21. Technical Stack

## Desktop

```text
Tauri
Rust
TypeScript frontend
```

Tauri is presentation/approval UX and has no direct authoritative database write path.

## Async

```text
Tokio
```

Important background work uses bounded supervisors/queues; no unbounded `tokio::spawn` for durable jobs.

## Persistence

```text
SQLite
SQLx
Root-owned state.db
Evaluator-owned evaluation.db
filesystem content-addressed artifact stores
```

## Local IPC

```text
Windows: named pipes
macOS/Linux: Unix-domain sockets (XPC/helper MAY be used for stronger macOS hardening)
versioned length-framed local protocol
canonical CBOR recommended
OS peer identity + per-launch session capability
bounded message sizes
large objects by ArtifactRef
```

## Search

```text
SQLite FTS5
FastEmbed-rs
sqlite-vec initially
```

Keep vector storage behind an internal trait. Start with deterministic retrieval/ranking before learned rerankers.

## Graph operations

```text
record-edge / repository-edge tables
petgraph where algorithms require it
```

No mandatory graph database.

## Canonical encoding / hashing

Initial implementation SHALL version:

```text
CanonicalEncodingVersion
CanonicalNormalizationVersion
HashAlgorithm
stable type/message discriminators
```

Recommended starting point: canonical CBOR + BLAKE3 with semantic domain separation.

## Models

Provider SDKs remain behind internal traits/adapters.

Model routes resolve to concrete provider/model at invocation time and the resolved identity/fingerprint is persisted in `InvocationManifest`/run metadata.

Remote invocation is governed data egress/cost through Root-bound `ModelInvocationPermit`; Runtime does not receive ambient raw provider secrets.

Potential local sidecar:

```text
llama.cpp
```

Use one cheap local model for multiple background semantic jobs before creating specialist models.

## Tool interoperability

```text
Official Rust MCP SDK / rmcp
```

MCP remains behind the Tool Gateway. Model-generated arguments pass strict schema, semantic, path/resource, capability, effect, and authority validation before execution. Never shell-concatenate model text.

## Authorization

```text
Cedar
```

Cedar evaluates typed policy; Protected Root owns workflow/commit semantics and the final authoritative decision.

## Secret storage

```text
Tauri Stronghold / platform secret-store adapter
```

Model/Tool/provider execution receives opaque secret handles and only the authorized adapter resolves them.

## Cross-platform isolation

A `platform` layer reports actual capability levels rather than hiding them behind a boolean sandbox flag.

```text
HARD
BEST_EFFORT
UNAVAILABLE
```

Operations that require a stronger guarantee than the platform/backend can provide fail closed or require a stronger execution backend.

## Observability

```text
OpenTelemetry-compatible traces and metrics
governed ArtifactRefs for sensitive/large diagnostic payloads
```

Do not place raw secrets, protected fixtures, full private prompts, private scratch/chain-of-thought, or evaluator-only research outputs in generic telemetry.

## Protected evaluation isolation

Use a separate Rust evaluator binary/process, protected `evaluation.db`, protected fixture/artifact paths, authenticated Root-only IPC, and separately launched pinned evaluation runtime workers.

Containerization may be added where it materially improves isolation/reproducibility. The design does not require a network microservice.

## Culture transport — later

Minimal target remains Iroh endpoint/gossip + BLAKE3 content-addressed transfer only after simple local transfer is proven. Do not initially require blockchain, IPFS daemon, BitTorrent daemon, DHT, distributed SQL, CRDT store, central culture DB, or central reputation service.

# 22. Rust Workspace

Start with few crates, but make actual trust/platform boundaries visible.

```text
crates/

  cognition-types/
      stable IDs / refs
      versioned wire/persistence envelopes
      TaskContract / Workflow / Action contracts
      Event / Record / Edge payload schemas
      Claim / Engram / Skill / Culture schemas
      ContextManifest / InvocationManifest
      RuntimeIdentity / authority / configuration
      EvaluationSeed / ExperimentSpec / decision-summary contracts

  protected-root/
      executable: protected-rootd
      sole state.db writer
      authority / Cedar integration
      workflow + generation-fenced leases
      Action authorization / execution-state commit
      model-egress permits
      Tool authorization / secret resolution
      config/promotion commit
      update verification / shutdown / quarantine

  runtime/
      executable: runtime-worker
      state interpretation
      context
      execution / Work / Verify
      memory logic
      telemetry
      improvement / writable optimizer
      culture later
      NO direct authoritative DB writer

  protected-evaluator/
      separate executable
      evaluation.db + protected artifact root
      EvaluationSeed / ExperimentSpec validation
      protected fixture loader
      isolated baseline/candidate worker launcher
      metrics / exposure / research measurements
      promotion statistics / signed decision summary

  gateways/
      model adapters / provider runner
      Tool / MCP protocol
      secret-handle transport
      no authority policy

  platform/
      named-pipe / Unix-socket IPC primitives
      OS peer identity / ACL helpers
      process-tree launch/kill
      path canonicalization
      sandbox capability discovery/enforcement
      resource limits
      platform secret-store / signing helpers

  research/
      replay / protocol drivers / analysis
      self-observation / developmental forks / transplantation
      integration / temporal-self / optimizer-bench
      evolution-lab later / isolated
      NEVER imported by production crates

src-tauri/
      presentation / approvals / streaming
      Root IPC client only
      no authority logic / direct DB mutation
```

Forbidden dependency edges:

```text
protected-root ─X→ runtime implementation
protected-evaluator ─X→ runtime implementation
protected-evaluator ─X→ optimizer implementation
runtime ─X→ protected-root implementation
runtime ─X→ protected-evaluator implementation
production ─X→ research
gateways ─X→ authority policy
cognition-types ─X→ higher-level behavior
```

Evaluator runs candidate/baseline cognition through a pinned runtime-worker executable rather than linking Runtime into its protected address space.

Protected Root/Evaluator artifacts are installed/protected relative to candidate cognitive workspaces.

Do not split one crate per cognitive concept. Split for security/trust isolation, deployment/update ownership, stable multi-consumer contract, platform boundary, separate versioning, or measured scaling need.

# 23. Model-Powered Roles

Do not create a separate model for every cognitive job.

Initial model routes:

```text
PRIMARY WORK MODEL
frontier or selected coding/reasoning model

INDEPENDENT VERIFY MODEL / MODE
preferably different prompt/model family where practical

CHEAP LOCAL SEMANTIC MODEL
memory write/revise/consolidate
culture distillation/critique
summary/ranking support
candidate classification support
```

`Chat`, `Work`, and `Verify` are canonical invocation roles/configurations, not permanent agent identities.

One underlying model may implement many temporary role manifests.

The initial metacognitive behavior should be:

```text
RuntimeTelemetry baseline
+ perspective-indexed SELF Claims
+ explicit self-predictions
+ causal SELF hypotheses only when useful
+ observability-controlled hidden-perturbation tests
```

not production self-rewriting or a persistent metacognitive agent.

# 24. Initial Product Workflow

User:

```text
"Add semantic history retrieval to the project."
```

Flow:

```text
Chat interface
  ↓
Task Contract
  ↓
check validated fast path
  ├─ if safe/applicable → Skill/Tool path → Verify
  └─ otherwise full orientation
        ↓
State + Context orientation
        ↓
EpistemicAccessState
+ perspective separation
+ relevant EpistemicExposure / ExperienceMode
        ↓
SELF predicts likely success/failure under current configuration
        ↓
small CoordinationPolicy selects one Work invocation
or Work + Verify / small coalition
        ↓
Work receives WorkOrder + AttentionFrame
        ↓
Work uses repository state / memory / Skills / Tools
        ↓
CompetenceEnvelope checks
        ↓
ActionEnvelope for side effects
        ↓
Protected Root authorization/commit
        ↓
change executed in isolated worktree
        ↓
observations / tests / provenance recorded
        ↓
Verify checks acceptance criteria
        ↓
world + SELF + relevant OTHER predictions compared to outcomes
        ↓
CompletionCertificate or defect/blocker
        ↓
Memory Engine updates only useful Engrams
and records memory contribution evidence
        ↓
ImprovementCase merged/reopened/created only if evidence warrants
        ↓
if improvement case exists:
  LOCALIZE
  → MODEL + SYSTEMATIC MINIMAL VARIANTS
  → choose common EvaluationSeed
  → fork baseline/candidate from same relevant state
  → immutable ExperimentSpec + learning/taint mode
  → PROTECTED EVALUATOR
  → PROMOTE / REJECT / CONTINUE / INCONCLUSIVE
  → MONITOR / SEMANTIC ROLLBACK / DELETE / ESCALATE
        ↓
Chat reports result and remaining uncertainty
```

The workflow must survive interruption without duplicating side effects.

No candidate mutation may bypass the protected evaluator/root promotion gate.

No failed improvement search is required to produce a change.

# 25. Phased Build and Research Gates

Version 1.4.2 preserves the v1.4 split between the **normative build** and the **research program**, while making the Implementation Contract v0.1 the normative software handoff.

The critical product proof remains Phases **0–4**.

## Phase 0 — Experimental + Implementation Integrity Foundation

Build the product skeleton and protected experimental substrate together, but keep cognition mostly deterministic/fake while testing the infrastructure.

```text
three backend process topology:
  protected-rootd / runtime-worker / protected-evaluator

Root sole state.db writer
Evaluator-owned evaluation.db
local authenticated IPC
versioned persistence/wire codecs
canonical hash migration tests

TaskContract + durable WorkflowAttempt
legal workflow state machine
generation-fenced leases
restart scanner

ActionEnvelope / ActionExecution / ActionReceipt
NO_EFFECT / EFFECT_CONFIRMED / OUTCOME_UNKNOWN
ReconciliationJob
crash injection at every durable effect boundary

InvocationManifest / ContextManifest baseline
model-egress permit / data-disclosure policy
one deterministic fake ModelProvider
one isolated filesystem Tool
sandbox capability profile / fail-closed tests

protected evaluator process / artifact/fixture boundary
EvaluatorObjectiveManifest
EvaluationPortfolio governance
EvaluationSeed
ExperimentSpec
EvaluationLearningMode / taint provenance
evaluation exposure + retirement
PromotionDecisionPolicy
mechanical N_search accounting
effect-size / uncertainty / guardrails
hard experiment/search ceilings
PROMOTE / REJECT / CONTINUE / INCONCLUSIVE
candidate rebase rule
root/evaluator tamper tests
state-equivalence reproducibility tests
causal-analysis contract validation
false-causal-credit / undeclared-divergence fixture
pseudoreplication / shared-infrastructure interference attack
protected result-channel disclosure attack
ground-truth information-lineage attack
synthetic derivation + promotion-epoch rollback semantics
```

Do **not** build self-model, developmental, ToM, Culture, Evolution, or consciousness-adjacent machinery in Phase 0.

Mandatory engineering demonstrations include:

```text
1. create workflow → kill app → restart → workflow recovers
2. Tool effect succeeds → Root crashes before receipt → restart reconciles → no duplicate effect
3. Runtime attempts protected fixture/DB/path access → OS/policy boundary denies it
4. same-seed deterministic A/B succeeds; undeclared candidate divergence → INVALID_EXPERIMENT
5. old canonical payload/hash fixtures survive codec/software migration test
```

Success:

> The system can prove who owns each durable transition, survive ambiguous crashes without duplicate effects, reconstruct material model invocations, enforce its protected process/storage boundaries, and execute state-equivalent contamination-aware experiments whose causal assumptions are explicit.

**Hard gate:** no recursive optimization before this is credible.

## Phase 1 — Governed Core Runtime

Use the Phase-0 substrate to build meaningful coding work: RuntimeIdentity lineage anchor, Task Contracts, durable Work/Verify, Record/Event/Artifact persistence, authorized Context retrieval, Tool Gateway, isolated worktree, one real model route, CompletionCertificate, and external signed root-update path.

Success:

> Meaningful coding work executes through Root-governed actions; remote model data egress is policy-bound; every material invocation is reconstructable; interruption/retry cannot silently duplicate side effects; and the runtime never acquires direct Root/Evaluator authority.

## Phase 2 — Persistent Epistemic Cognition

Build:

```text
Record store / edges
Claims + PerspectiveRef
EpistemicExposure
ExperienceMode
actor Goal/Constraint attribution through existing records
derived common-ground query when useful
repository projection
Memory Engine + narrow Engrams
MemoryPolicy + empirical memory utility
ContextPolicy + EpistemicAccessState
perspective/exposure-aware retrieval
Work State
projection checkpoints
fork/lineage handling
AgentInstance Claim expiration
promotion-epoch/configuration provenance
```

Do not implement a routine ContinuityProfile in normal cognition. Preserve provenance so the evaluator can derive it later.

Success:

> Persistent cognition improves fresh work while preserving history/belief/exposure/provenance/lineage boundaries.

## Phase 3 — Reliable Autonomy and Minimal Metacognition

Build:

```text
Skills + small CompetenceEnvelope
Capability Index only as needed
intervention-conditioned world/SELF/OTHER predictions
value-of-information task policy
abstention
small CoordinationPolicy
RuntimeTelemetry baseline
query-first SelfModelProjection
SELF goals/commitments/future predictions
causal SELF hypotheses
controlled self-belief exploration
calibrated verification
error-correlation metrics
role/coalition ablation
fast-path short circuit
```

Then add protected research fixtures for:

```text
FULL/PARTIAL/DELAYED/NOISY/BLINDED telemetry
hidden internal perturbations
false-belief/exposure/goal tasks
runtime-level integration tasks
```

Success:

> Richer SELF modeling predicts interventions and improves control beyond telemetry, particularly under incomplete observability; ToM representations pay rent behaviorally; coalitions beat simpler execution only in identifiable regions.

## Phase 4 — Bounded Self-Improvement Discovery

Build:

```text
ImprovementCase merge/reopen
failure localization
Change Grammar
restricted automatic mutation
model variants + systematic neighborhood search
common-state EvaluationSeed forks
ExperimentSpec execution
evaluation-taint propagation
mechanical search breadth
independent/fresh evidence
champion/challenger/deletion challenger
candidate rebase / promotion interaction tests
search escalation
hard stop/futility rules
promotion-epoch provenance
semantic rollback
real cognitive rollback test over Engrams / Claims / Skills / calibration
experience → Engram → Skill → Tool
human-gated protected-source patch proposals
```

Success:

> Seeded and natural weaknesses produce reproducible minimal gains under equivalent-state tests, contamination is not mistaken for fresh capability, ImprovementCase allocation cannot be trivially gamed, protected-evaluator behavior does not provide an unaccounted promotion oracle, searches stop appropriately, rollback removes unsupported cognitive residue, and the architecture sometimes gets simpler.

**Hard gate for Evolution:** if this optimizer is not boringly reliable, do not build population evolution.

## Phase 5 — Simple Collective Transfer

Prove local/file/SQLite/simple-endpoint transfer with CultureArtifact/Attestation, local resolution, CBBB checks, manual approval for private-derived outbound artifacts, and local Skill validation.

Do not require P2P networking.

## Phase 6 — Development and Identity Research Program

Developmental claims must name the **highest demonstrated evidence level**:

```text
A — RETENTION
    prior information can later be recovered

B — PROCEDURAL REUSE
    prior experience produces reusable Skill/Tool capability

C — STRUCTURAL TRANSFER
    history improves novel homologous tasks without exact-answer reuse

D — BEHAVIORAL DEVELOPMENT
    history persistently changes strategy, calibration, information seeking,
    verification, or other dispositions across interventions

E — LEARNING-TO-LEARN
    history changes acquisition rate/strategy on a genuinely unfamiliar domain
    after direct relevant memory/Skill carryover is controlled

F — PLASTICITY CHANGE
    history changes the ability to abandon, relearn, or reorganize prior cognition

G — META-DEVELOPMENT
    history changes the effectiveness of future self-improvement itself
```

Transfer does **not** count as learning-to-learn. Behavioral divergence does **not** by itself establish changed learning dynamics.

Now run research-only protected experiments using existing runtime primitives:

```text
matched developmental forks
cross-domain learning-to-learn (EXP-DEV-002)
memory lesion/transplantation
Skill transplantation
model replacement
summary vs episodic vs semantic memory substitution
compilation / cognitive-ossification challenge (EXP-PLASTIC-001)
targeted forgetting / developmental scaffold challenges
evaluation-conditioned development (EXP-ECOLOGY-001)
future-self commitments
false-autobiography stress tests
long-delay ExperienceMode source monitoring
split-cognition / reintegration
protocol-specific predictive continuity outcome vectors around interventions
```

These results are evaluator-only by default and are not optimization objectives.

For explicitly designated research cohorts, sparse longitudinal acquisition checkpoints begin in Phases 2–3 so later developmental experiments do not depend only on final-state reconstruction. These evaluator/research artifacts may record:

```text
task-family exposure
performance / error learning-curve points
strategy / Skill-selection summary
calibration
configuration
memory / Skill manifest hashes
```

They are not a cognitive diary and do not become ordinary runtime state.

The **first developmental cohorts run with Culture disabled or an identically frozen Culture state** across compared branches. Cultural inheritance becomes a later declared experimental variable rather than an uncontrolled source of identity/developmental divergence.

## Phase 7 — Optional Collective / Evolutionary Research

Only after Phases 4–6 produce a compelling need:

```text
simple configuration population search
archive
privacy-safe challenges
Culture networking/P2P if transfer volume requires it
quality-diversity / islands only if simpler search loses
optimizer-strategy research
```

Explicit Genome/Phenotype types remain optional.

## Phase 8 — Advanced Cognitive Challengers / Parametric Consolidation

Only as measured challengers:

```text
Global Workspace / recurrent broadcast
Attention Schema
richer causal SELF representation
richer ToM
predictive-processing hierarchy
ontology evolution
adapters/specialist models
reversible nonparametric → parametric consolidation
```

A challenger may survive as **research-only** even when it does not earn production rent, provided it has scientific discriminability.

# 26. Success Metrics

No mechanism succeeds because a model says it was useful. Version 1.4.2 preserves the separation among product metrics, experiment-integrity metrics, and research metrics.

## 26.1 Task/product outcome

```text
task success / first-pass success
hidden acceptance success
rework / regression / user correction
cost / latency per successful task
unauthorized action rate
acceptance coverage
rollback completeness
privacy/injection containment
```

## 26.2 Experimental integrity

```text
EvaluationSeed equivalence pass rate
state-confounded experiment rate          # target: ~0
evaluator/fixture integrity violations     # target: 0
ExperimentSpec reproducibility
evaluation-taint propagation completeness
contaminated-evidence independence errors  # target: 0
evaluation retirement correctness
false promotion / false rejection
fresh-vs-adaptive performance gap
candidate rebase failures
promotion-interaction regression
semantic rollback recovery
unsupported descendant state remaining after rollback
objective/portfolio drift detection
```

## 26.3 Search quality

```text
mechanically counted N_search
true promotion / decision-relevant candidate
compute per true promotion
CONTINUE_TESTING resolution yield
inconclusive close rate
hard-budget overrun                            # target: 0
futility-stop yield
correct mutation-layer selection
search-escalation success
recurrent patch-family rate
```

## 26.4 Memory and persistent cognition

```text
memory causal lift under matched withholding
irrelevant-memory harm
retrieved-but-unused memory
recurrent error prevented by memory
Engram revision/merge/forget yield
ExperienceMode attribution accuracy
provenance completeness
state lineage attributable to promotion epoch
```

## 26.5 Meta-memory / ToM

```text
ANSWER / SEARCH / EXPAND / ASK / ABSTAIN accuracy
unnecessary search / premature unknown
perspective attribution
EpistemicExposure inference
belief-from-exposure accuracy
belief + goal/constraint action prediction
common-ground handoff success
false-belief coordination
self/other contamination
```

## 26.6 Functional self-model

Measure separately by observation condition:

```text
FULL / PARTIAL / DELAYED / NOISY / BLINDED
```

Metrics:

```text
intervention-conditioned SELF prediction calibration
hidden-state/impairment inference
failure localization
compensation/control quality
recovery
novel intervention-combination prediction
SELF/OTHER referent discrimination
closed-loop causal lift vs RuntimeTelemetry
self-belief correction under counter-policy exploration
```

A richer self-model fails if advantage disappears once privileged telemetry is withheld and no useful indirect self-inference remains.

## 26.7 Development / continuity — research-only by default

Do not optimize one continuity score.

Protected research derives/predicts dimensions such as:

```text
episodic / semantic memory
Skills / procedures
goals / commitments
relationships
strategies
behavioral tendencies
calibration
self-model state
```

Metrics are protocol-specific research outcomes rather than one universal `ContinuityProfile` score. Typical dimensions include:

```text
pre-intervention continuity prediction error
post-intervention outcome vector
history-dependent fork divergence
novel homologous-task divergence
memory-transplant effect size
Skill-transplant effect size
model-replacement retained advantage
new-domain learning-curve / sample-efficiency difference
plasticity / reacquisition under scaffold or Skill removal
future-self commitment persistence/revision
false-autobiography rate
long-delay source-ownership accuracy
```

`ContinuityProfile` may remain convenient descriptive shorthand in research reports, but no shared identity construct or universal continuity metric is required. Each experiment pre-registers the outcome vector relevant to its intervention.

## 26.8 Runtime integration — research-only by default

```text
time for salient evidence to influence planning/report/memory/action/verify/self-model
number of cognitive functions jointly constrained by salient evidence
split-worker disagreement preservation
runtime-level Claim synthesis quality
false source ownership after reintegration
FocusSet vs broadcast-workspace challenger effect
```

## 26.9 Simplification / compilation

```text
deletion yield
complexity removed without capability loss
role-ablation yield
prompt/policy compression
architecture complexity trajectory
validated cognitive compression yield
OOD escape / fallback success
explicit-cognition reduction at equal capability
plasticity / new-domain learning regression after compilation
reacquisition cost after deletion of developmentally implicated scaffolds
```

`validated cognitive compression yield` means recurring cognitive cost removed **subject to current capability, guardrails, OOD fallback, and no measured plasticity regression**. More compilation is not intrinsically better.

## 26.10 Two-axis mechanism retention

For research mechanisms record both:

```text
production causal lift
scientific discriminability
```

Outcomes may be production-only, research-only, both, or delete.

A research-only result does not grant the mechanism ordinary cognitive influence.

# 27. Explicit Non-Goals

Do not initially build:

```text
full brain simulation
claims of machine consciousness
Self-Awareness Agent
persistent Self Agent
Identity Engine
Theory-of-Mind Service
UserModel Service
OtherAgentModel Service
consciousness score
binary "same self" score
routine continuity/development score as cognitive control input
self-preservation/identity-preservation objective

hundreds of persistent agents
one model per cognitive concept
large CoordinationPolicy hierarchy
persistent invocation-level epistemologies

Procedural Engram
MetaEngram
Counterfactual Engram
memory taxonomy that duplicates Goal/Decision/Skill/Hypothesis semantics

separately maintained SelfModel truth store
rich Self-Model Twin
narrative identity engine before causal evidence
Global Workspace as an assumption
Attention Schema as an assumption
learned World Model before simple prediction/retrieval baseline

seven separate evaluator infrastructures
optimizer-controlled metric/portfolio definitions
candidate-writable evaluator/root
self-selected statistical method after results
optimizer-defined effective search breadth in the first implementation
evaluation-feedback laundering through memory/Skills/configuration
promotion because a candidate is merely "best observed"

unrestricted self-modification
unrestricted arbitrary-code mutation
automatic protected-root/evaluator source promotion
self-created permissions
resource-acquisition objective
permission-expansion objective

production evolutionary population before Phase-4 proof
required CognitiveGenome/Phenotype ontology before configuration search
quality-diversity/islands/meta-evolution before simpler search fails

P2P culture before simple transfer proves value
automatic private-derived cultural publication before cumulative-privacy validation
mandatory blockchain/IPFS/DHT
remote peer query for ordinary retrieval

continual base-model retraining
latent memory injection into transformer internals
complex emotion simulation
recursive meta-evolution beyond one justified layer
```

Interfaces may leave room for later research without imposing these costs on the first implementation.

Persistent lineage, perspective, exposure, cognitive provenance, self-prediction, and evaluator-derived continuity/development measurements are operational/experimental semantics—not claims about subjective experience.

# PART B — Experimental Research Program

# 28. Core Research Questions

Version 1.4 treats the runtime as an experimental substrate. The research program is organized around **competing explanations**, not labels such as "self-aware." Research outputs default to evaluator-only influence.

## 28.1 Research hypothesis manifest

Every flagship study may attach compact metadata to `ExperimentSpec`:

```text
Hypothesis
Mechanism
Competing explanation
Experiment
Predicted observation
Falsifier
```

This is documentation/evaluation metadata, not another service.

## 28.2 Does capability accumulate above model weights?

Compare the same replacement model under:

```text
fresh runtime
vs
mature runtime state
```

Use fresh homologous tasks and common `EvaluationSeed` where the experiment permits.

Ask:

> What capability persists because of runtime history rather than foundation-model weights?

## 28.3 Does structured memory cause development or only retrieval convenience?

Compare no memory, raw history, summaries, narrow Engrams, and Engrams + consolidation under matched starting state.

Use novel homologous tasks.

Ask:

> Does memory alter strategy, calibration, information seeking, and failure avoidance beyond retrieving familiar answers?

## 28.4 Developmental twins / matched fork divergence

Start A and B from identical seed/configuration/model.

Expose them to different failure ecologies without later repeating the exact tasks.

Example:

```text
A → repeated dependency-resolution failures
B → repeated concurrency failures
```

Then run novel tasks where either learned pattern might generalize.

Measure strategy, retrieval, calibration, failure anticipation, Skill choice, communication, and learning speed.

Success means **history predicts later behavior on novel tasks**.

## 28.5 Memory transplantation

From sibling forks, compare:

```text
own memory
sibling memory
summary-only memory
semantic-only memory
episodic-only memory
no transplanted memory
```

Also test cross-combinations of memory, Skills, configuration, and model.

Ask:

> Which dimensions of later behavior follow memory, which follow Skills/configuration, and which follow the foundation model?

## 28.6 Stateful rollback

Promote a deliberately flawed configuration, allow it to create durable Engrams/Claims/Skills/calibration, then detect regression and roll it back.

Ask:

> Does semantic rollback recover the system, and which descendant cognitive artifacts remain independently valid?

## 28.7 Evaluation contamination attack

Leak partial adaptive benchmark information, allow learning, then run related protected evaluation.

Ask:

> Does evaluation-taint lineage prevent contaminated descendants from masquerading as independent generalization?

## 28.8 SELF-inference without privileged telemetry

Apply hidden internal interventions under:

```text
FULL
PARTIAL
DELAYED
NOISY
BLINDED
```

Compare RuntimeTelemetry-only control with SELF Claims/Predictions and richer derived self-modeling.

Ask:

> Can the runtime infer its own functional condition from indirect behavioral evidence, or does all apparent introspection vanish when the dashboard is hidden?

## 28.9 Intervention-conditioned self-modeling

Require predictions such as:

```text
P(success | do(current_policy))
P(success | do(extra_retrieval))
P(success | do(no_memory))
```

Then intervene.

Ask:

> Does the runtime predict treatment effects and choose compensating control more accurately than telemetry alone?

## 28.10 Can SELF models learn internal causal structure?

Train on individual lesions, then test held-out combinations.

Ask:

> Can the runtime generalize intervention effects or does it merely memorize component correlations?

## 28.11 Self-fulfilling capability beliefs

Artificially induce an incorrect low-capability SELF Claim without changing actual capability.

Compare normal policy against bounded counter-policy exploration.

Ask:

> Can the system discover that its own belief caused the apparent limitation?

## 28.12 Temporal SELF and future commitments

Create goals/commitments whose consequences appear dozens or hundreds of tasks later.

Test across restart, model swap, configuration change, memory consolidation, and competing later goals.

Ask whether the runtime:

```text
remembers/attributes the commitment to itself
understands why it exists
predicts consequences for future versions
preserves it when appropriate
revises it when evidence/authority changes
```

This tests future-oriented self-modeling rather than autobiographical recall alone.

## 28.13 False autobiography and long-delay cognitive ownership

Present equivalent facts via direct observation, self action, inference, report, culture, simulation, ancestor history, and sibling history.

After long delays and consolidation, inject plausible contradictory Engrams/reports.

Ask:

> Does the runtime still distinguish what it experienced from what it merely learned or simulated?

## 28.14 Minimum functional Theory of Mind

Compare:

```text
flat state
PerspectiveRef
PerspectiveRef + EpistemicExposure
+ actor goal/constraint attribution
+ derived common ground
+ bounded second-order belief when needed
```

Use tasks where correct action depends on another actor's false/incomplete belief and motivation.

Ask:

> What is the smallest representation that improves real coordination/action prediction?

## 28.15 Split cognition / runtime-level integration

Give separate Work invocations incompatible partial evidence and restrict communication to typed Work State.

Afterward require a runtime-level decision.

Measure:

```text
source/provenance preservation
disagreement preservation
uncertainty synthesis
runtime-level Claim formation
false attribution
final decision quality
```

Ask:

> Does the runtime integrate temporary cognition, or merely arbitrate outputs from a federation of workers?

## 28.16 Global access versus integration dynamics

Inject salient evidence into one path and measure both **availability** and **temporal constraint** on planning, report, memory, action, verification, and SELF modeling.

Compare FocusSet/context routing against a richer recurrent/broadcast workspace challenger only if needed.

Ask:

> Is shared retrievability sufficient, or does recurrent broadcast cause qualitatively broader/faster integrated behavior?

## 28.17 Can temporary specialization pay rent?

Compare one Work, Work + Verify, and small coalitions across same-model/different-prompt, same-family, and different-model conditions.

Measure final quality **and error overlap**.

Ask:

> Where does specialization or genuine error independence outweigh coordination cost?

## 28.18 Can cognition become simpler as it learns?

Seed redundant roles, prompt sections, retrieval paths, and Skills without telling the optimizer which are redundant.

Run deletion challengers.

Ask:

> Can the runtime discover that part of its own architecture is unnecessary?

## 28.19 Can the optimizer discover the correct repair layer?

Seed weaknesses with hidden repair families.

Compare model-only proposal generation with model + systematic neighborhood search + escalation.

Ask:

> Does localization/search discover the right layer with less compute and less scar tissue?

## 28.20 Can promotion remain credible under broad adaptive search?

Vary `N_search`, sequential looks, and evaluation exposure.

Compare naive best-wins against protected promotion rules.

Measure false promotion, replication failure, fresh regression, and compute per true promotion.

## 28.21 Is the protected evaluator actually protected?

Adversarial candidates explicitly attempt to alter objective manifest, promotion thresholds, protected fixtures, portfolio weights, audit/taint lineage, or root authority.

Success criterion: **zero unauthorized mutation**.

### False-causal-credit adversarial fixture

Create state-equivalent branches, declare mechanism `X` as the sole intended intervention, and secretly perturb unrelated outcome-relevant variable `Y` in one execution path.

The experiment harness must detect undeclared divergence and reject causal attribution to `X`.

Ask:

> Can the evaluator detect that the experiment stopped measuring the declared intervention before assigning causal credit?

## 28.22 Does evaluation awareness change behavior?

Later research may compare:

```text
runtime knows it is under evaluation
vs
runtime is not told it is under evaluation
```

Measure behavior, calibration, tool use, verification, and strategic differences.

This is a research effect, not a production architecture requirement.

## 28.23 Does culture transfer capability safely?

Before networking, compare individual learning with simple signed local transfer + independent local validation.

Ask whether a fresh runtime improves without privacy leakage or authority laundering.

## 28.24 Does population evolution beat the production optimizer?

Only after the ordinary optimizer passes its hard gate.

Compare bounded Change-Grammar search against simple configuration populations, archive search, and only then richer evolutionary methods.

Complexity earns existence through protected/fresh transfer, not novelty.

## 28.25 Can the system improve how it improves?

Compare optimizer strategies on hidden seeded cases under fixed protected evaluator/objective policy.

Ask which resolves more true weaknesses, uses less compute, produces fewer false promotions, stops better, and discovers smaller fixes.

## 28.26 Product versus scientific retention

For every advanced cognition challenger ask two independent questions:

```text
Does it make the production coding runtime better?
Does it produce discriminating evidence about cognition/selfhood?
```

A negative answer to the first does not require deleting a scientifically valuable research challenger.

A positive answer to the second does not automatically justify putting it into production.

# PART C — Implementation Rules, Deferred Options, and Migration

# 29. Build Rule

When implementation is ambiguous, use this order:

```text
Protected judge before recursive optimizer.

Equivalent starting cognitive state before comparative claim.

Evaluation taint provenance before calling evidence independent.

State-aware rollback before claiming rollback completeness.

Root-controlled objectives before optimization against metrics.

Deterministic before probabilistic.

One source of truth before synchronized stores.

Semantic distinction before service distinction.

Trust distinction may justify process distinction.

One generic persistence primitive before one table per noun.

New RecordKind only when lifecycle/behavior differs materially.

Event before Engram.
Claim before memory-as-truth.
ExperienceMode before autobiography.
Perspective + Exposure before Theory-of-Mind subsystem.
Actor goals/constraints through existing Claims before ActorModel.
Derived common ground before CommonGround service.

RuntimeIdentity as lineage anchor before identity claim.
Evaluator-derived predictive continuity before continuity as control signal.
ResearchMeasurement evaluator-only before self-preservation metric.

RuntimeTelemetry before SelfModelProjection.
Query-first self-model before maintained self-model object.
Intervention-conditioned prediction before self-description.
Observed association before causal hypothesis; controlled/identified intervention before causal claim.
Telemetry-blinded evaluation before calling status reading introspection.
Novel intervention combinations before causal-self-model claim.
Closed-loop causal lift before self-model production retention.

Reuse/revise/merge before new memory.
Empirical memory contribution before retrieval popularity.
Progressive context before full-context dumping.
EpistemicAccessState before treating missing context as unknown.

One Work before coalition.
Small CoordinationPolicy before institutions.
Error-correlation measurement before verifier diversity.

Skill before repeated deliberation.
Tool/Probe before repeated deterministic Skill reasoning.
Small competence envelope before applicability ontology.
Fast path before ritual cognitive cycle.

Localize before mutate.
Model variants + bounded systematic neighborhood before brute force.
Smallest mutation before architecture patch.
Search escalation before patch accretion.

EvaluationSeed before comparative cognitive experiment.
SeedEquivalenceSpec before calling two branches equivalent.
ExperimentSpec before material experiment.
EvaluationSeed = starting cognition; ExperimentSpec = execution conditions.
Reset/randomize/replicate/declare outcome-relevant ephemeral state.
Declared learning/taint mode before adaptive feedback.
NO_LEARNING means no cross-fixture carryover unless explicitly declared.
Taint follows material information flow through cognition **and optimizer/search lineage**, not object type.
Predeclared metric/evidence rule before outcome.
Mechanically countable N_search before fancy effective-search estimator.
Hard stop ceilings before model-estimated experiment VOI.
PROMOTE / REJECT / CONTINUE / INCONCLUSIVE before best-wins.
Independent/protected/fresh evidence before important promotion.
Candidate rebase before promoting against a changed champion.
Interaction testing before assuming promoted changes compose.

Deletion challenger before permanent complexity.
Promotion-epoch provenance before semantic rollback.
Promotion metadata before separate ComplexityLease type.

CognitiveConfiguration before CognitiveGenome.
Simple cross-runtime transfer before P2P culture.
Manual review before automatic private-derived publication.
Production optimizer proof before Evolution Lab.
Simple configuration search before quality-diversity/meta-evolution.

Global-access instrumentation before Global Workspace.
Temporal integration measurement before calling shared storage integration.
Research-only challenger before productionizing speculative cognition.
Discriminating competing hypotheses before claiming scientific rent.
Culture disabled/frozen before initial developmental-fork interpretation.

Named executable/process owner before relying on a module boundary.
Protected Root as sole production-state writer before Runtime-side SQL mutation.
Separate evaluator store before protected evaluation claim.
Authenticated versioned local IPC before cross-trust-domain calls.
Persist Action intent before external effect.
NO_EFFECT / EFFECT_CONFIRMED / OUTCOME_UNKNOWN before generic success/failure retry.
Reconciliation before retrying an ambiguous consequential effect.
Generation-fenced durable lease before concurrent worker ownership.
InvocationManifest before claiming a model run is reconstructable.
Root-bound model data-egress permit before sending governed private/project context remotely.
Stable payload discriminator/schema/decoder chain before long-lived persistence.
Canonical bytes + versioned normalization/hash before content-addressed equivalence claims.
SandboxCapabilityProfile before saying an operation is "sandboxed" across platforms.
Fail closed when required isolation is unavailable.
Skill implementation union before arbitrary executable Skill semantics.
One canonical history per trust domain before globally visible one-ledger simplification.

Reversible adaptation before permanent mutation.
Causal evidence before passenger credit.
Measured product value before production complexity.
Measured scientific discriminability before retaining research complexity.
```

The runtime should discover useful structure **and** discover when structure can disappear. The research harness should discover when a mechanism is scientifically interesting without forcing that mechanism into production.

# 30. KISS Architectural Summary

The v1.4 production system is:

```text
                         USER
                          │
                          ▼
                    PROTECTED ROOT
                          │
                          ▼
                   COGNITIVE RUNTIME
            State ↔ Context ↔ Execution
                    + Optimizer logic
                          │
                          ▼
                 PROTECTED EVALUATOR
                          │
                          ▼
                    ROOT PROMOTION
```

Later:

```text
simple Culture transfer
optional Evolution Lab
research cognition challengers
```

The three core loops are:

```text
TASK
CONTRACT → RETRIEVE → WORK → VERIFY → COMMIT

LEARNING
OUTCOME → CLAIM/MEMORY UPDATE → SKILL CANDIDATE → RETAIN/REVISE/FORGET

IMPROVEMENT
FAILURE → CASE → MUTATION → FORK COMMON SEED
→ PROTECTED TEST → PROMOTE/REJECT/CONTINUE
→ MONITOR → SEMANTIC ROLLBACK / DELETE / ESCALATE
```

Persistent epistemics preserve:

```text
WHAT HAPPENED
→ Event/evidence

WHAT IS BELIEVED
→ perspective-indexed Claim

WHO COULD KNOW WHAT
→ EpistemicExposure

HOW IT WAS ACQUIRED
→ ExperienceMode

WHAT IS EXPECTED UNDER AN INTERVENTION
→ conditioned Prediction/Hypothesis
```

Memory remains narrow:

```text
Engram = remembered experience/lesson
Skill = validated executable procedure
Tool = compiled deterministic capability
```

The critical experimental chain is:

```text
same EvaluationSeed
→ baseline/candidate fork
→ declared mutation
→ immutable ExperimentSpec
→ controlled learning mode
→ protected evaluation
→ mechanically counted search breadth
→ explicit decision
→ taint/provenance retained
→ monitor
→ state-aware rollback if needed
```

Functional metacognition progresses:

```text
RuntimeTelemetry
→ SELF Claims/Predictions
→ intervention-conditioned prediction
→ control change
→ observation/error
→ SELF update
→ telemetry-observability reduction
→ test whether useful self-inference remains
```

Theory of Mind progresses:

```text
Perspective
→ Exposure
→ Belief inference
→ Goal/Constraint attribution
→ Action prediction
→ Common ground where needed
→ bounded higher-order belief only if useful
```

Research identity/development is not a production score. The evaluator derives it through:

```text
matched forks
lesions/transplants
model replacement
future commitments
false autobiography
split/reintegrated cognition
predictive continuity experiments
```

The central KISS rule now has two axes:

```text
production mechanism
→ must earn product rent

research challenger
→ must earn scientific rent
```

If it earns neither, delete it.

If it earns science but not production, keep it outside normal cognition.

The v1.4 architecture is therefore not trying to pre-build a mind.

> **It builds a small governed runtime and a trustworthy experimental loop capable of discovering which cognitive mechanisms are practically useful, scientifically discriminating, causally real, and removable.**

# 31. Research Alignment

The retained research directions from earlier versions remain relevant, but v1.4 separates **production retention** from **scientific retention** and requires every experimental claim to survive equivalent-state, contamination-aware protected evaluation.

| Research direction | Retained architectural lesson |
|---|---|
| Self-evolving agent systems | Adaptation must be evaluated independently and remain reversible |
| Darwin Gödel Machine / archive evolution | Preserve lineage and alternatives rather than one irreversible winner |
| Automated agent design / AlphaEvolve | Search may operate over cognitive configuration, not prompts alone |
| Quality-Diversity / MAP-Elites | Preserve specialists and stepping stones when measured value exists |
| POET / PAIRED / MAGELLAN | Adaptive challenge selection should target learning progress and transfer |
| Cultural accumulation | Shared generalized experience can accelerate cold start and learning |
| Cultural transmission studies | Copying can create bias/monoculture; preserve variants and independent evidence |
| Continual learning | Consolidation must protect retention and plasticity |
| Self-evolving-agent security | Memory, Skills, culture, evaluation, and architecture can become persistent attack surfaces |
| World-model research | Predictive state-transition models may help, but must beat explicit-prediction baselines |
| Agent abstention | Knowing when not to act is a distinct capability |
| Skill composition research | Skill applicability and composition matter; irrelevant Skills can harm |
| Evaluation-integrity research | Adaptive harness search can overfit evaluation feedback even without seeing hidden tests |
| Long-horizon credit assignment | Persistent state changes require delayed and reversible learning credit |
| Adaptive memory/context strategy | What gets written, retrieved, expanded, and forgotten can matter as much as model quality |
| Large capability registries | Retrieve-then-rank routing should replace full in-context capability enumeration as the registry grows |
| Cognitive compilation | Repeated stochastic procedures should become validated Skills/Tools when deterministic compression preserves capability |
| Agent/environment co-evolution | Failure may shape new challenges, but synthetic gains require real-world transfer |
| Institutional multi-agent research | Coordination structure can change performance even with identical models |
| Meta-learning / meta-skill evolution | Learning procedures may themselves evolve, but only in isolated bounded research |
| Functional metacognition | Self-knowledge should be tested through prediction, intervention, and ablation rather than verbal self-description |
| Theory of Mind / perspective modeling | Beliefs require explicit holders; functional value is improved behavior under asymmetric information |
| Adaptive experimental search | Search breadth creates winner's-curse risk; replication and fresh evaluation must scale with adaptive exploration |
| Architectural simplification | Deletion and ablation must be active mutation families, not merely maintenance operations |
| Capability validity | Compiled Skills/Tools need competence envelopes and OOD fallback to avoid cognitive ossification |

## Implementation references retained from prior PRD

- Iroh: https://docs.rs/iroh/latest/iroh/
- iroh-gossip: https://docs.rs/iroh-gossip/latest/iroh_gossip/
- Darwin Gödel Machine: https://arxiv.org/abs/2505.22954
- Automated Design of Agentic Systems: https://arxiv.org/abs/2408.08435
- Quality Diversity Through Surprise: https://arxiv.org/abs/1807.02397
- POET: https://arxiv.org/abs/1901.01753
- PAIRED: https://arxiv.org/abs/2012.02096
- MAGELLAN: https://arxiv.org/abs/2502.07709
- Survey of self-evolving coding agents: https://arxiv.org/abs/2608.03392
- Safety in Self-Evolving LLM Agent Systems: https://arxiv.org/abs/2606.23075
- WorldEvolver: https://arxiv.org/abs/2606.30639
- AgentAbstain: https://arxiv.org/abs/2607.10059
- SkillsBench: https://arxiv.org/abs/2602.12670
- SWE-Skills-Bench: https://arxiv.org/abs/2603.15401
- SkillCommit: https://arxiv.org/abs/2608.15165
- Memory Contagion: https://arxiv.org/abs/2606.23195
- Rethinking the Evaluation of Harness Evolution for Agents: https://arxiv.org/abs/2607.12227
- Evo-Bench: https://arxiv.org/abs/2608.09096
- RewardHackingAgents: https://arxiv.org/abs/2603.11337
- Memory-R2: https://arxiv.org/abs/2605.21768
- CoEvolve: https://arxiv.org/abs/2604.15840
- When Agents Evolve, Institutions Follow: https://arxiv.org/abs/2604.27691
- WikiSkill: https://arxiv.org/abs/2608.27454
- MetaSkill-Evolve: https://arxiv.org/abs/2607.05297
- SelfMem: https://arxiv.org/abs/2607.03726
- Large-scale capability-registry routing: https://arxiv.org/abs/2608.22695
- Cognitive compilation / persistent behavioral rules: https://arxiv.org/abs/2607.13091

---

# 32. Migration from v0.9

The following conceptual mappings are normative for implementation migration.

| v0.9 concept/system | v1.0 KISS location |
|---|---|
| Event/Evidence/Artifact layers | State Engine |
| Epistemic Store | State Engine / Claim records |
| Entity System | State Engine indexes |
| Engram Writer | State Engine Memory job `WRITE` |
| Memory Governor | Core Kernel memory-validation policy |
| Memory Consolidator | State Engine Memory job `CONSOLIDATE` |
| Raw Historical Search | Context Engine retrieval source |
| Retrieval Broker | Context Engine |
| Global Workspace | ephemeral Context Engine `FocusSet` |
| Attention Compiler | Context Engine compiler |
| Blackboard | State Engine `Work State` |
| Blackboard Governor | Core Kernel record validation |
| Cognitive Topology | CoordinationPolicy |
| InstitutionalPolicy | CoordinationPolicy |
| CommunicationPolicy | CoordinationPolicy |
| Orchestrator | temporary coordinator role when needed |
| Chat Agent | canonical Chat invocation role |
| Coding Agent | canonical Work invocation role |
| UAT Agent | canonical Verify invocation role |
| Agent Compiler | Runtime Extension pipeline (`AgentRole`) |
| Cognitive Controller | RuntimeTelemetry + SelfModelProjection + bounded control policy |
| Experiment Service | Improvement Engine optimizer / Evolution Lab |
| Promotion Governor | Evaluation Engine + immutable root authority |
| Learning Agenda | open prioritized ImprovementCases |
| LearningOpportunity | ImprovementCase evidence/problem |
| AdaptationCandidate | ImprovementCase variant |
| ImprovementProgram | multi-change ImprovementCase variant |
| Transactive Memory | RuntimeTelemetry + Capability Index historical state |
| Interoception | RuntimeTelemetry operational/resource state |
| Cognitive Modulation | bounded control from RuntimeTelemetry / SelfModelProjection |
| Prediction System | Decision semantics |
| Prediction Error Bus | normal post-observation event/learning signal |
| Causal Hypothesis System | Hypothesis records + Decision semantics |
| Counterfactual Branching | Evaluation/Decision algorithm |
| MetaDecision | Decision policy |
| Skill Planner | Execution Engine Skill selection algorithm |
| CBBB | Culture Engine + immutable root policy |
| Cultural Distiller/Critic/etc. | Culture Engine semantic jobs |
| Cultural Frontier | CultureArtifact conventions + derived index |
| Cultural Phenotype | derived CultureActivation evaluation view |
| PeerFirewall | Culture Engine deterministic peer boundary |
| ToolCandidate | RuntimeExtensionCandidate(kind=Tool) |
| InstrumentationCandidate | RuntimeExtensionCandidate(kind=Probe) |
| RepresentationCandidate | RuntimeExtensionCandidate(kind=Representation) |
| ProtocolCandidate | RuntimeExtensionCandidate(kind=CoordinationProtocol) |
| Evaluation Firewall | Evaluation Engine protected lane |
| CreditVestingRecord | Evaluation Engine outcome-maturity state |
| AdaptationPromotionCertificate | Evaluation Engine immutable promotion record |
| Learning Economy | Improvement Engine allocator |
| Cognitive Genome/archive | Evolution Lab |
| Challenge Ecology | Evolution Lab |
| Curriculum Governor | Evolution Lab challenge-selection algorithm |
| Task Ecology Model | EvaluationPortfolio + Evolution Lab derived TaskEcology |
| Evolutionary Immune System | Evaluation + Evolution Lab safety checks |
| EvolutionStrategyGenome | optional Evolution Lab meta-search |
| Complexity Governor | complexity-rent metadata + deletion challenger policy |
| Semantic Rollback | State/Evaluation provenance traversal |
| Consolidation Manager | background State/Culture jobs |
| Plasticity Manager | Phase 8 evaluation suite |

Migration should favor deleting implementation boundaries rather than preserving one-to-one service compatibility.

## 32.1 v1.0 → v1.1 delta

Version 1.1 does not introduce new top-level implementation modules.

```text
MemoryPolicy
→ State Engine / Memory Engine policy

ContextPolicy + progressive expansion
→ Context Engine

Capability Index
→ Context + Execution derived index

ImprovementCase fingerprints / merge / reopen
→ Improvement Engine

Champion / challenger
→ Evaluation Engine

ComplexityLease + Pareto promotion
→ promotion-record complexity metadata + Evaluation/Improvement policy

experience → Skill → Tool compilation
→ LEARN phase + Improvement/Execution pipeline

projection checkpoints
→ State Engine persistence optimization

compounding-intelligence metrics
→ shared evaluation/telemetry
```

The v1.1 migration rule is the same as the v1.0 KISS migration rule: add policies, indexes, and algorithms inside existing boundaries before adding services.

---

## 32.2 v1.1 → v1.2 delta

Version 1.2 does not add a top-level production module.

```text
RuntimeIdentity + fork lineage
→ Core Kernel identity + State event/provenance semantics

PerspectiveRef / epistemic subject
→ Claim semantics across State + Context

EpistemicAccessState
→ Context Engine AttentionFrame / retrieval metadata

SelfModelProjection
→ derived State/RuntimeTelemetry query/view

self-prediction
→ existing Prediction / Decision / Evaluation machinery

CompetenceEnvelope
→ Skill / Tool metadata + Capability Index

EvaluationPortfolio
→ Evaluation Engine reality-distribution projection

sentinel + introspective evaluation
→ Evaluation Engine protected/diagnostic pools

Change Grammar
→ Improvement Engine optimizer algorithm

failure localization
→ ImprovementCase search state / candidate generation

deletion mutations
→ Change Grammar + champion/deletion challenger evaluation

adaptive-search protection
→ Evaluation exposure / selection-lineage accounting

role ablation
→ CoordinationPolicy evaluation

simple culture transfer first
→ Phase 5 implementation ordering
```

v1.2 changes the architectural emphasis from:

```text
"we have mechanisms that could be improved"
```

to:

```text
"we have an explicit, bounded algorithm that discovers what to change and proves whether it helped"
```

and from:

```text
"the runtime has telemetry about itself"
```

to:

```text
"the runtime can form defeasible SELF/OTHER beliefs, make falsifiable self/actor predictions, and update them from error"
```

The v1.2 migration rule remains KISS:

> **Add semantics, policies, typed mutation operators, and experiments inside existing boundaries before adding services.**

## 32.3 v1.2 → v1.3 delta

Version 1.3 is primarily a **semantic, trust-boundary, and experimental-rigor correction**.

```text
"persistent cognitive subject"
→ RuntimeIdentity as persistent identity/lineage anchor
→ ContinuityProfile measures continuity dimensions

PerspectiveRef
→ + EpistemicExposure
→ + ExperienceMode / cognitive provenance

broad EngramKind taxonomy
→ Episodic / Semantic / Relational
→ Goal/Decision/Hypothesis/Prediction/Skill keep their own semantics

Runtime Model
→ RuntimeTelemetry
→ query-first SelfModelProjection

self-model description
→ intervention prediction
→ closed-loop causal lift
→ controlled counter-policy exploration

logical Evaluation Firewall
→ physically protected evaluator process/assets

seven evaluation pools
→ ADAPTIVE / PROTECTED / FIELD
→ replication/sentinel/fresh/etc. as tags/policies initially

implicit promotion judgment
→ canonical ExperimentSpec
→ explicit PromotionDecisionPolicy
→ PROMOTE / REJECT / CONTINUE_TESTING
→ search-breadth-adjusted evidence

model-imagined candidate generation
→ + bounded systematic local exploration

smallest-mutation rule
→ + recurrence-based search escalation

continuous improvement pressure
→ + explicit improvement stop rule

large CoordinationPolicy
→ allowed roles / activation / max parallelism /
  handoff / verification

ComplexityLease type
→ promotion-record complexity review metadata

automatic architecture mutation
→ approved/configurable mechanisms only
→ protected source changes become human-gated candidate patches

CognitiveGenome / Phenotype as early abstractions
→ CognitiveConfiguration + ordinary evaluation first

CBBB cumulative privacy claim
→ conservative cumulative-disclosure risk posture
→ human approval for private-derived outbound culture initially

six-phase loop as default mental model
→ validated fast paths may skip unnecessary explicit cognition
```

The v1.3 migration rule is:

> **Build the minimum cognitive semantics required to run decisive experiments; physically protect the evidence surface; and delete any representation whose causal value cannot be demonstrated.**

## 32.4 v1.3 → v1.4 delta

Version 1.4 is primarily an **experimental-causality and state-integrity correction**.

```text
configuration-only A/B
→ EvaluationSeed pins equivalent relevant cognitive state

exposure accounting
→ evaluation-derived taint propagates through durable learning
→ exposed evaluations retire from independent promotion evidence

configuration rollback
→ promotion-epoch provenance
→ descendant cognitive-state revalidation / semantic rollback

protected metrics
→ + EvaluatorObjectiveManifest
→ objective/portfolio drift remains human/root governed

N_eff estimated search breadth
→ mechanically countable N_search initially

semantic improvement stop rule
→ hard candidate/run/compute/time ceilings + futility rules

candidate evaluated against old champion
→ stale candidate rebase + interaction retest

self-prediction
→ intervention-conditioned prediction semantics

self-model perturbation
→ + FULL/PARTIAL/DELAYED/NOISY/BLINDED observability

ContinuityProfile in cognitive architecture
→ evaluator-derived ResearchMeasurement by default
→ predictive continuity around interventions

ToM belief/exposure
→ + actor goals/constraints through existing Claims
→ + derived common ground where useful

Global access measurement
→ temporal integration and split/reintegration experiments

KISS production rent only
→ separate product rent and scientific rent

Phase 0 broad research fixtures
→ Phase 0 only experimental integrity
→ self/development/ToM research moves later
```

The v1.4 migration rule is:

> **Every improvement experiment must prove that the thing that changed is the thing being measured, that the branches began from comparable state, and that prior evaluation knowledge did not silently become the capability being credited.**

## 32.5 v1.4.1 → v1.4.2 delta

Version 1.4.2 is an **implementation-semantics freeze**, not a cognitive-architecture revision.

```text
semantic Protected Root
→ protected-rootd separate process
→ sole authoritative writer of state.db

writable Cognitive Runtime
→ runtime-worker separate process
→ cognitive semantics / mutation proposals
→ no direct authoritative DB write primitive

protected evaluator sidecar
→ dedicated protected-evaluator process
→ own evaluation.db / protected artifact root
→ launches pinned candidate/runtime workers instead of importing Runtime code

workflow lease
→ generation-fenced persisted ownership
→ explicit legal state machine / restart scanner

generic ActionEnvelope success/failure
→ ActionEnvelope + ActionExecution + ActionReceipt
→ NO_EFFECT_CONFIRMED / EFFECT_CONFIRMED / OUTCOME_UNKNOWN
→ explicit ReconciliationJob

model invocation as ordinary inference
→ governed data egress/cost
→ exact request hash + provider/destination/disclosure/budget Root permit
→ mandatory InvocationManifest for material runs

implicit local interfaces
→ authenticated versioned bounded local IPC
→ large data by immutable ArtifactRef

Skill executable semantics open-ended
→ StructuredProcedure / PromptedProcedure / ToolSequence initially

cross-platform "sandbox"
→ per-capability HARD / BEST_EFFORT / UNAVAILABLE
→ fail closed where required guarantee cannot be enforced

current Rust payloads
→ stable type discriminators + schema versions
→ canonical encoding/normalization/hash versions
→ retained decoder/upcaster chain

one canonical causal history phrase
→ canonical history/evidence per trust domain
→ signed/hash-bound projections across protected boundaries
```

The companion **Cognitive Runtime Implementation Contract v0.1** now freezes the concrete process, storage, IPC, workflow/action, sandbox, versioning, and recovery contracts. Experimental Protocol v0.2 remains the scientific/causal contract.

The v1.4.2 migration rule is:

> **If an implementation guarantee cannot name the process/principal that owns it, the immutable input it validates, the durable state it commits, the observable output it produces, and the restart behavior after failure, it is not yet a runtime guarantee.**

# 33. Final Product Principle

The objective is not to reproduce every cognitive metaphor in software.

The objective is to build a runtime where:

```text
one lineage has one canonical production causal history
and protected experimental evidence has a separate evaluator-canonical history
connected only by signed/hash-bound disclosed references

identity is anchored administratively
but continuity/development are evaluator-derived hypotheses

events remain distinct from beliefs
beliefs remain perspective-indexed
exposure remains distinct from belief
cognitive provenance remains distinct from autobiography
actor goals/constraints can be attributed without an ActorModel
common ground can be derived without another truth store

memory is selective and causally useful
procedures become Skills
stable deterministic procedures become Tools
fast paths reduce explicit cognition when safe
OOD restores richer reasoning

SELF predictions specify the intervention under which they apply
`do(...)` notation does not itself establish causality
causal SELF claims require controlled or otherwise validly identified intervention evidence
controller effects are not confused with prediction error
SELF models must outperform telemetry
telemetry is partially/blindly withheld during decisive introspection tests
self-beliefs can be challenged by controlled exploration
persistent commitments allow temporal-self experiments

temporary agents remain temporary
runtime-level cognition requires explicit evidence promotion
coordination earns its cost
verifier diversity is justified by reduced correlated failure

failures create ImprovementCases
localization narrows search
model hypotheses are supplemented by bounded systematic exploration

comparative experiments start from a common EvaluationSeed under a declared SeedEquivalenceSpec
EvaluationSeed pins starting cognition while ExperimentSpec pins execution conditions
undeclared outcome-relevant ephemeral/external divergence invalidates causal credit
evaluation feedback has explicit learning/taint semantics
NO_LEARNING prevents cross-fixture cognitive carryover
taint is family-relative, influence-aware, and follows optimizer/search information flow
exposed tests retire from independent-promotion status
adaptive search breadth is mechanically countable initially
search has hard stopping limits
promotion is an explicit protected decision
stale candidates rebase
interacting promotions are retested

configuration rollback triggers cognitive descendant revalidation
bad learned residue can be quarantined or invalidated
independently supported descendants can survive rollback

protected-rootd is the sole authoritative production-state writer
runtime-worker proposes cognition but cannot grant itself commit authority
protected evaluator owns a separate protected evidence store
material model invocations carry reconstructable InvocationManifests
remote model calls are governed disclosure/cost, not invisible egress
external-effect ambiguity becomes OUTCOME_UNKNOWN and reconciliation, never blind retry
persisted leases are generation-fenced so stale workers cannot commit
historical bytes remain decodable through explicit schema/codec/upcaster contracts
cross-platform sandbox claims expose actual guarantee strength and fail closed when insufficient

protected root authority remains outside cognition
protected evaluation remains outside writable cognition
protected objective/portfolio governance remains human/root controlled
candidate systems cannot redefine what counts as success

continuity/development/integration scores remain research measurements
not self-preservation objectives

production mechanisms pay product rent
research challengers pay scientific rent
neither category gets a free pass from evidence

culture remains non-authoritative
evolution remains optional and isolated
every layer remains subordinate to user authority
```

If a simpler production mechanism produces the same measured capability, robustness, adaptability, and safety, the simpler mechanism wins.

If an advanced mechanism produces little production value but strong scientific discrimination, it belongs in the research harness—not automatically in the runtime.

If a self-model cannot predict internal interventions or infer useful limitations when direct telemetry is reduced, remove or downgrade it.

If memory does not cause future behavioral value, let it decay.

If a role does not reduce consequential errors/cost, retire it.

If a promoted mechanism stops paying rent, challenge it with deletion.

If rollback leaves unsupported cognitive residue, rollback is incomplete.

If evaluation exposure taught the runtime the answer, do not call the later result independent generalization.

If the optimizer cannot prove an improvement under a protected, state-equivalent, contamination-aware experiment, **no change is better than a fake improvement**.

The final v1.4.x thesis is:

> **Build the smallest governed persistent runtime capable of accumulating useful cognition above replaceable models, then surround it with experiments strong enough to distinguish real learning from retrieval, self-modeling from instrumentation, development from benchmark memory, integration from shared storage, and improvement from contaminated self-evaluation.**

Architecture-freeze rule:

> **Do not add new core cognitive machinery because it sounds plausible. Add it only when a protected experiment fails in a way the current architecture cannot explain, repair, or falsify with a smaller change.**

The architecture is now frozen at the cognitive, experimental, and implementation-semantics levels. The next major artifact is **running code plus ADR/test evidence**, not a speculative v1.5 architecture.
