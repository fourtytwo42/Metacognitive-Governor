# Production-Readiness Evidence Matrix

This document is the current completion audit for the full MCG roadmap. It is
deliberately stricter than a feature inventory: `implemented` means source and
focused tests exist, while `qualified` additionally requires the milestone's
cross-interface, restart, security, packaging, and owner-machine exit evidence.
Passing a narrower test does not close a broader milestone.

Status values:

- **Qualified checkpoint** — the implemented milestone contract has matching
  automated evidence on its supported owner-controlled platforms.
- **Implemented, gates open** — substantial end-to-end implementation exists,
  but one or more named exit gates remain.
- **Partial** — material roadmap deliverables are absent or deliberately
  inconclusive.
- **Not started** — no first-class product implementation was found.

No GitHub-hosted build, test, or paid automation is permitted. GitHub is source
hosting only.

## Milestone matrix

| Milestone | Current status | Authoritative implementation/evidence | Gates that remain open |
| --- | --- | --- | --- |
| M0 — Foundations | Qualified checkpoint | Rust workspace, Tauri/React shell, local `scripts/verify.ps1` and `scripts/verify.sh`, formatting, strict Clippy, Rust, Vitest, Playwright, documentation, and packaging checks. | Repeat the complete verifier on each final signed candidate. |
| M1 — Runtime spine | Qualified checkpoint | Durable event/projection storage, authenticated loopback API, SSE replay, crash recovery, migrations, daemon/CLI/desktop integration, and restart tests. | Final-candidate migration and installer rerun. |
| M2 — Provider fabric | Implemented, gates open | Explicit OpenAI subscription OAuth and API-key modes, OpenRouter, GPU45/local/arbitrary compatible endpoints, discovery/probes, capabilities, pricing provenance, routing, streaming, durable dispatch accounting, and exact post-v5 Ubuntu/Win11 provider/ensemble/shadow qualification. | Normal browser OAuth lifecycle on final Ubuntu and macOS packages; never silently fall back from subscription OAuth to API billing. |
| M3 — Tools and policy | Qualified checkpoint | Typed filesystem/process/Git/HTTP/system/memory tools, project confinement, approvals, leases, redaction, evidence, audit, and adversarial tests on Windows/Linux. | Re-run against the final package candidates. |
| M4 — Projects and memory | Qualified checkpoint | Project discovery, instruction precedence/conflicts, build/test discovery, snapshots, watches, bounded context, provenance-bearing memory, lifecycle, and restart tests. | Broader real-project UAT and final-package regression. |
| M5 — Organization and useful work | Implemented, gates open | Versioned organizations, Executive/Coder/Conversation/Evaluator/Tool Worker roles, plans, leases, budgets, model/tool evidence, objective settlement, durable restart, and provider-backed project fixtures. Manifest-bound Windows/Linux/macOS packages now pass successful human/JSON/NDJSON worker-plus-Evaluator work with a governed discovered project command, encrypted restore, restart, and provider-free replay; the deterministic macOS reports match Windows and Ubuntu byte for byte. | Complete several representative website/application projects through installed final-candidate packages; retain independent browser/evaluator evidence and bounded-cost results, then repeat the lane on signed final candidates. |
| M6 — Interface family | Implemented, gates open | Native desktop simple/advanced modes, CLI human/JSON/NDJSON/streaming, daemon REST/SSE, OpenAI Responses/Chat/models adapter, MCP client/server, Tauri commands, and shared durable contracts. Manifest-bound Windows/Linux/macOS packages now have provider-backed CLI completion plus direct REST discovery, buffered/streaming Responses, Chat Completions, a complete MCP task/evaluator lifecycle, content-free proxy history, encrypted restore, restart, provider-free replay, and real native launch evidence. | Broader native simple/advanced success, accessibility/privacy/OAuth UAT, usability refinement, and signed-final-candidate repetition remain. |
| M7 — Scheduler and autonomy | Qualified checkpoint | Interval/cron/file/event/heartbeat triggers, durable occurrences, leases, recovery, notifications, policy and budget boundaries, CLI/API/MCP/Tauri/desktop surfaces. | Final-candidate unattended soak and notification UAT. |
| M8 — Expanded execution | Implemented, gates open | Persistent confined browser sessions, bounded Windows/X11 computer use, Sites, monitoring/deployment, and typed SSH with durable authority, approval, audit, and recovery. | Clipboard/window/process computer capabilities are still explicitly unavailable; Wayland input is unqualified; repeat real installed-package browser/computer/Sites/SSH UAT on final candidates. |
| M9 — Measured improvement | Implemented, gates open | Durable supervision, calibration/failure observations, isolated candidate workspaces, shadow models, paired benchmarks, promotion/reversion, and restart-safe experiment evidence exist. Every typed success-condition variant has a deterministic objective-specific verifier: command/build/test/static/configuration, file, HTTP/service health, bounded TCP listener, page render, CSS element, accessibility, typed browser workflow, screenshot, and research-source checks. Arbitrary prose remains deliberately non-executable rather than being treated as evidence. Provider ensemble/shadow is exact-qualified on Ubuntu and Win11. Same-sandbox candidate-service verification and the production Bubblewrap/AppArmor `.deb` lane pass exact Ubuntu qualification. Exact schema 8 qualifies HTTP/TCP/hydrated-page/CSS-element/accessibility/typed-workflow/screenshot pass/failure evidence on both Ubuntu Bubblewrap and Win11 Windows Sandbox with launch-digest-bound PNG baselines, a canonical viewport, established noise/changed-area thresholds, strict host-port isolation, and zero residual processes/sessions. Exact `af64d8e` Ubuntu `.deb`/AppImage UAT exercises one representative promotion across REST, installed CLI, and MCP, then reopens the same database and proves the immutable receipt, benchmark, evidence-linked decision, and active candidate agree after restart; agent-operated simple/advanced native UI UAT shows the same promoted experiment and protected-baseline contract. Real Win11 UAT used the earlier package to find an unreadable advanced evidence-card layout, corrected it, and rebuilt exact clean `4589a4e`. That final NSIS package passes complete install/repair/restart/CLI/MCP/backup/plugin/native-launch/uninstall automation with 49 MCP tools; agent-operated simple and advanced Improvement views both show readable promoted evidence, benchmark counts, protected-baseline state, and advanced immutable IDs. The matched `27fa6e5` macOS package now passes the same promoted-experiment REST/CLI/MCP/restart contract and real native launch; agent-operated simple/advanced views visibly retain the promoted benchmark, decision counts, baseline protection, and immutable identities after desktop restart. | Repeat the matched cross-platform checkpoint on signed final candidates. |
| M10 — Extensibility and advanced operation | Partial | Trusted-native and Wasmtime/WASI plugins, registry signatures, brokered grants, portable/encrypted backups, content-addressed evidence, schema-2 pinned organization inheritance, bounded content-addressed organization export/import/history/comparison, deterministic run replay, and schema-31 dependency-ordered multi-host SSH workflows with deterministic capability routing now span REST/CLI/MCP/Tauri/desktop with fail-closed recovery. Schema 34 adds signed organization sharing: owner-selected Ed25519 publisher keys are scoped to exact HTTPS registries, revocation is terminal, verification creates no authority, and only a separate reasoned import rechecks current trust and atomically creates fresh local authority; REST/CLI/MCP/Tauri/simple-and-advanced desktop plus tamper/restart/replay tests share the contract. Exact `26353f2` source and installed Win11 sharing/revocation/restart UAT pass; corrective exact `8f50b99` source, manifest-bound Windows/Linux packages, complete installed harnesses, and focused native visible-browser UAT also pass after the same session exposed an empty-origin defect. Project intelligence now has bounded stable provenance topology for cross-language dependencies/runtimes, .NET solutions/projects/packages/project references, named Compose services/databases/edges/endpoints, Kubernetes workloads/images/selector-matched Services/Ingress routes, Helm charts/dependencies/database edges, typed CloudFormation/SAM resources and explicit dependencies, Terraform JSON configurations/providers/typed resources/modules/explicit dependencies with value and cache exclusion, OpenAPI origins, and deployment targets; readable provenance-bearing topology feeds the shared worker/evaluator context under a strict token bound. A snapshot-bound read-only planner exposes deterministic explicit-dependency stages across REST/CLI/MCP/Tauri/desktop, never promotes routing or inventory into authority, and emits zero partial stages on cycles. Schema 33 adds a separate immutable owner-reviewed execution plan that pins the exact snapshot, fingerprint, topology digest, stage, and structured manifest command; fresh on-disk discovery, normal policy/approval, durable pre-spawn claims, content-addressed logs, terminal observation-only replay, and restart uncertainty span REST/CLI/MCP/Tauri/desktop. Native desktop also exposes the graph plus the full memory lifecycle, and scheduler-driven freshness consolidation is restart-safe. Exact-current source qualification passes the complete local verifier on both Ubuntu and Windows, including the topology contract through API/CLI/MCP/Tauri/desktop, real Chromium, live GPU45, native builds, and 9/9 Playwright journeys on each platform. Exact v12 `.deb`/AppImage qualification additionally includes agent-operated native create/duplicate/export/history/compare UAT at 1280x800. | Include signed sharing and its corrected browser-session boundary in the final matched macOS package. Workflows and desktop plan/memory/world-model/consolidation/context plus the new reviewed execution flow still need representative installed-package real-project UAT. Additional manifest ecosystems and operational UX remain open. |
| M11 — Consumer release | Partial | Owner-built Windows NSIS, Linux `.deb`/AppImage, and macOS app/DMG packaging plus clean-machine install/repair/uninstall/backup UAT, diagnostics, and recovery harnesses exist. Windows/Linux builders now fail closed unless every clean-source manifest entry passes a hash-rechecked scan by enabled Microsoft Defender or protected package-managed ClamAV with every definition database no older than 72 hours; compact evidence is atomic, no-clobber, and contains no absolute paths or scanner output. Real owner-machine execution reports the exact current `cad52f2` NSIS, `.deb`, and AppImage clean with retained evidence. Exact v27 passes complete canonical verification on Ubuntu and Win11, full Win11 NSIS install/repair/restart/CLI/doctor/backup/MCP/GUI/uninstall/data-retention UAT, and matched two-hour/10,000-task Windows/Ubuntu optimized-daemon soaks with verifier-bound exact identities, forced-crash recovery, database/schema checks, and zero residual candidate processes. Exact v27 Linux signed-AppImage UAT additionally passes shutdown-timeout and real ENOSPC rollback with baseline preservation, verified backups, authenticated recovery, zero temporary/zombie residue, and exact disposable-CA teardown. The corrected exact-v34 Windows updater binds the reviewed signed-feed version to the embedded installer `ProductVersion`; its real installed UI lanes pass changed-version, no-update, corrupt-response, unavailable-feed, bad-signature, backup-write failure, native installer failure/recovery, valid signed update/relaunch/migration, and offline rollback to the sealed `0.1.0` baseline while preserving durable state, verified secret-free backups, registration/process/listener invariants, and zero staged residue. Exact-subject/thumbprint/DER CA teardown leaves zero trust matches; the still-live feed passes explicit pinned-CA health while ordinary system trust fails `SEC_E_UNTRUSTED_ROOT`. | Trusted signing identities, Windows/Linux signing policy, macOS Developer ID notarization/stapling, owner-controlled updater publication, repeat native scans and public malware reputation checks on final signed artifacts, final matched upgrade candidates, and final OAuth/accessibility/privacy UAT remain. |
| M12 — Long-term roadmap | Partial | The first cross-project learning control is implemented as an explicit owner-reviewed, source-digest-bound, one-claim transfer. Schema 32 commits the copied target claim, immutable transfer evidence, and event atomically; caller-owned IDs replay safely after restart; occupied targets and changed/stale evidence fail closed. Read-only review, mutation, and history span REST, CLI, MCP, Tauri, and simple/advanced desktop, with core/API/component tests. Exact Windows, Ubuntu, and matched macOS package qualification now passes, including encrypted restore, daemon restart, cross-interface replay, and immutable inbound/outbound history. Signed organization sharing now has a first-class schema-34 trust/review/import vertical; existing experiments, ensembles, Sites, and typed SSH remain foundations for later slices. | Native macOS memory/sharing interaction and signed-final-candidate repetition remain open. Ambient or automatic cross-project learning intentionally remains absent. Distributed execution beyond typed SSH, fleet management, predictive intervention, long-term operational workflows, and an Android capability-profile application still require first-class vertical implementation and threat-model/benchmark evidence. |

Three shared installed real-project gates are now implemented. The website
gate in `scripts/test-installed-real-project.py` passes the complete existing
manifest-qualified Windows NSIS, Ubuntu `.deb`/AppImage, and macOS app/DMG lanes.
It proves a seeded website regression fails before work, three governed writes
build the site, the discovered
test and independent evaluation pass, real Chromium checks the rendered document
and local references at 1280x720, exact replay is side-effect-free, and files/audit
history/browser output survive daemon restart without the provider. This is one
representative project and therefore strengthens, but does not close, the broader
M5/M10 requirement for several installed final-candidate application projects.
The second gate in `scripts/test-installed-real-api-project.py` builds a bounded
loopback JSON ledger service, tests health/mutation/summary and invalid/oversized/
unknown inputs, tears down its listener, and binds runtime behavior across replay
and daemon restart. It now also passes the complete existing manifest-qualified
Windows NSIS, Ubuntu `.deb`/AppImage, and macOS app/DMG lanes with byte-identical
exercise, restart, and runtime reports on all three platforms. Compact secret-free
evidence is retained under `qualification-evidence/20260820-installed-real-project/`
and `qualification-evidence/20260820-installed-real-api-project/`. The third gate
in `scripts/test-installed-real-fullstack-project.py` builds HomeBase Inventory:
five governed files provide an accessible browser UI, bounded loopback service,
atomic disk persistence, generated regression test, and independent evaluation.
Real Chromium mutates the service through the UI; exact API state, files, audit,
runtime behavior, and provider-free replay survive both application and MCG daemon
restart. It passes the complete existing Windows NSIS, Ubuntu `.deb`/AppImage, and
macOS app/DMG lanes, with compact evidence under
`qualification-evidence/20260821-installed-real-fullstack-project/`. These three
representative projects close the initial several-project breadth checkpoint, but
do not replace broader native workflow/accessibility/privacy UAT or the required
repeat on signed final candidates.

## Immediate dependency order

1. Extend the now-qualified successful Windows/Linux/macOS installed package journeys
   through several representative consumer website/application projects and every
   remaining native simple/advanced, accessibility, privacy, and OAuth interaction.
2. Qualify pinned organization inheritance on exact final Windows/Linux packages, then
   continue M10 in independently releasable vertical slices: deterministic
   evidence replay, exact qualification for multi-host workflows, and advanced memory/world-model
   behavior. Every slice must reach desktop, CLI, REST, MCP where applicable,
   durable recovery, and tests.
3. Exercise multiple representative projects through installed candidates and
   refine simple/advanced UX plus accessibility from observed UAT evidence.
4. Provision the production updater publication path, acquire owner-controlled
   signing and notarization authority, then run matched package, upgrade, rollback, soak,
   security, privacy, OAuth, and UAT gates.
5. Treat M12 as product implementation, not aspirational documentation, if the
   complete PRD remains the release definition.

## Evidence discipline

- Raw qualification evidence belongs outside disposable `target/debug` output.
- A platform-specific capability difference must be visible to the consumer and
  cannot be counted as a pass for that platform.
- Ignored VM tests count only after execution on the named owner-controlled VM.
- Unsigned/ad-hoc packages are qualification artifacts, not production releases.
- A feature is not complete merely because its storage type, API shape, or unit
  test exists; the milestone exit evidence in `milestones.md` controls.
