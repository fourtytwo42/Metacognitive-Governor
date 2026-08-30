import { expect, test } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const initial = {
      schema_version: 2,
      revision: 1,
      interface_complexity: "simple",
      autonomy: "autonomous",
      shadow_experiments: true,
      updated_at: "2026-08-15T00:00:00Z"
    };
    let settings = initial;
    const plugin = {
      installation: {
        schema_version: 1,
        plugin_id: "example.weather",
        active_revision_id: "00000000-0000-4000-8000-000000000081",
        enabled: false,
        capability_grants: [] as unknown[],
        revision: 1,
        updated_at: "2026-08-15T00:00:00Z"
      },
      active_revision: {
        schema_version: 1,
        id: "00000000-0000-4000-8000-000000000081",
        manifest: { schema_version: 1, id: "example.weather", display_name: "Weather helper", version: "1.0.0", class: "sandboxed_wasi", requested_capabilities: ["network"] },
        module_sha256: "a".repeat(64),
        module_size_bytes: 128,
        source: { kind: "local_import" },
        signature: { state: "unsigned" },
        installed_at: "2026-08-15T00:00:00Z"
      }
    };
    const now = "2026-08-18T00:00:00Z";
    let codexFeatures = [
      { name: "memories", stage: "beta", displayName: "Memories", description: "Use durable memories.", enabled: false, defaultEnabled: false },
      { name: "shell_tool", stage: "stable", displayName: null, description: null, enabled: true, defaultEnabled: true }
    ];
    const sshHosts = [
      { id: "00000000-0000-4000-8000-000000000221", revision: 4, definition_hash: "d".repeat(64), display_name: "Linux builder", hostname: "builder.test", port: 22, username: "mcg", known_hosts_entry: "builder.test ssh-ed25519 AAAA", host_key_sha256: "SHA256:builder", capabilities: ["os:linux", "tool:cargo"], state: "active", created_at: now, updated_at: now },
      { id: "00000000-0000-4000-8000-000000000222", revision: 2, definition_hash: "e".repeat(64), display_name: "Linux tester", hostname: "tester.test", port: 22, username: "mcg", known_hosts_entry: "tester.test ssh-ed25519 BBBB", host_key_sha256: "SHA256:tester", capabilities: ["os:linux", "tool:playwright"], state: "active", created_at: now, updated_at: now }
    ];
    const project = {
      schema_version: 1,
      id: "00000000-0000-4000-8000-000000000241",
      project_id: "00000000-0000-4000-8000-000000000242",
      root: "C:\\Projects\\consumer-app",
      fingerprint: "a".repeat(64),
      observed_at: now,
      repository: { is_git: true, branch: "main", head: "abc", dirty: false, recent_commits: [] },
      languages: ["TypeScript"], build_systems: ["npm"], test_systems: ["vitest"], package_managers: ["pnpm"],
      services: ["web"], ci_systems: [], important_directories: ["src"],
      commands: [{ name: "test", command: "pnpm test", executable: "pnpm", arguments: ["test"], source: "package.json", confidence_basis: "manifest" }],
      instructions: { conflicts: [] },
      world: {
        nodes: [{ id: "service:web", kind: "service", label: "web", source: "package.json" }],
        relationships: [{ from: "repository", relation: "provides", to: "service:web", source: "package.json" }]
      },
      watched_inputs: {}
    };
    const baseOrganization = {
      schema_version: 2,
      organization_id: "00000000-0000-4000-8000-000000000251",
      revision_id: "00000000-0000-4000-8000-000000000252",
      revision: 1,
      name: "Consumer team",
      roles: [{ id: "00000000-0000-4000-8000-000000000253", kind: "coder", name: "Coder", purpose: "Build", tool_capabilities: ["filesystem.project"] }],
      relationships: [],
      policy: { prefer_minimal_organization: true, max_parallel_agents: 4, max_total_agents: 8, max_organizational_overhead_ratio: 0.3, default_lease_seconds: 900 },
      evaluation: { require_independent_evaluation: false, require_objective_evidence: true, minimum_confidence: 0.8, calibration_window: 100 },
      created_at: now
    };
    const portableOrganization = {
      schema_version: 1,
      source: { organization_id: baseOrganization.organization_id, revision_id: baseOrganization.revision_id, revision: 1, exported_at: now },
      definition: {
        name: baseOrganization.name,
        roles: [{ key: "role-000", kind: "coder", name: "Coder", purpose: "Build", prompt_policy: "Stay bounded", model_policy: { capability_class: "coding", preferred_models: [], allow_fallback: true, max_escalation_tier: 2 }, tool_capabilities: ["filesystem.project"], memory_namespace: "role/coder" }],
        relationships: [], policy: baseOrganization.policy, evaluation: baseOrganization.evaluation
      },
      content_sha256: "c".repeat(64)
    };
    let organizations = [baseOrganization];
    const organizationPublisher = {
      schema_version: 1,
      id: "example-publisher",
      display_name: "Example publisher",
      public_key_base64: "e".repeat(44),
      registries: ["https://organizations.example.com"],
      state: "trusted",
      revision: 1,
      reason: "Owner reviewed",
      created_at: now,
      updated_at: now
    };
    let organizationShareReviews: any[] = [];
    let memoryClaims: any[] = [];
    let sshWorkflows: any[] = [];
    let reviewedPlans: any[] = [];
    let reviewedRunCount = 0;
    let cloudApplyCandidate: any = null;
    let windowsSandboxReady = false;
    const canonicalWorkflowId = "00000000-0000-4000-8000-000000000601";
    const canonicalAttemptId = "00000000-0000-4000-8000-000000000602";
    let canonicalWorkflow = {
      workflow_id: canonicalWorkflowId,
      attempt_id: canonicalAttemptId,
      task_contract: { id: "00000000-0000-4000-8000-000000000603", version: 1, content_hash: { algorithm: "blake3-256", domain: "TaskContract", digest_hex: "c".repeat(64) } },
      parent_attempt_id: null,
      attempt_number: 1,
      state: "running",
      revision: 7,
      lease_owner: "runtime-worker-1",
      lease_generation: 3,
      lease_expires_at: "2026-08-18T00:05:00Z",
      heartbeat_at: "2026-08-18T00:00:30Z",
      checkpoint: null,
      environment_snapshot: { algorithm: "sha256", domain: "EnvironmentSnapshot", digest_hex: "d".repeat(64) },
      configuration: { id: "00000000-0000-4000-8000-000000000604", version: 1, content_hash: { algorithm: "blake3-256", domain: "RuntimeConfiguration", digest_hex: "e".repeat(64) } },
      idempotency_key: "workflow-create-1",
      retry_policy: { max_attempts: 3, initial_backoff_ms: 1000, maximum_backoff_ms: 30000 },
      compensation_policy: null,
      created_at: now,
      updated_at: now
    };
    const canonicalTaskContract = {
      id: canonicalWorkflow.task_contract.id,
      version: 1,
      previous_version: null,
      objective: "Recover the exact durable workflow",
      non_goals: [],
      requirements: [{ clause_id: "REQ-1", statement: "Preserve canonical workflow truth across reconnect." }],
      constraints: [],
      assumptions: [],
      acceptance_criteria: [{ clause_id: "AC-1", statement: "The same attempt and event sequence remain visible after reconnect." }],
      status: "active",
      created_at: now
    };
    const canonicalEvents = [{
      id: "00000000-0000-4000-8000-000000000605",
      type_id: "mcg.workflow.attempt.claimed",
      schema_version: 1,
      sequence: 2,
      workflow_id: canonicalWorkflowId,
      attempt_id: canonicalAttemptId,
      attempt_revision: 7,
      lease_generation: 3,
      correlation_id: "00000000-0000-4000-8000-000000000606",
      causation_id: "00000000-0000-4000-8000-000000000607",
      idempotency_key: "workflow-claim-1",
      occurred_at: now,
      payload: { owner: "runtime-worker-1" }
    }];
    const canonicalActionId = "00000000-0000-4000-8000-000000000608";
    const canonicalExecutionId = "00000000-0000-4000-8000-000000000609";
    const actionPayload = { type_id: "mcg.governed_filesystem_operation.v1", schema_version: 1, canonical_encoding: "canonical-cbor-v1", normalization: "canonical-json-v1", bytes: [1, 2, 3] };
    const canonicalAction = {
      action_id: canonicalActionId,
      execution_id: canonicalExecutionId,
      workflow_id: canonicalWorkflowId,
      attempt_id: canonicalAttemptId,
      actor: "root-action-runner",
      effect_class: "reversible",
      state: "outcome_unknown",
      outcome: { outcome: "outcome_unknown", hint: actionPayload },
      revision: 3,
      lease_generation: 2,
      updated_at: now
    };
    const canonicalActionInspection = {
      envelope: { id: canonicalActionId, workflow_id: canonicalWorkflowId, attempt_id: canonicalAttemptId, actor: canonicalAction.actor, origin: { conversation: { source_type: "mcg.conversation.v1", locator: "00000000-0000-4000-8000-00000000060d", content_hash: { algorithm: "blake3-256", domain: "MetaCog.Conversation", digest_hex: "1".repeat(64) } }, invocation: { source_type: "mcg.invocation_manifest.v1", locator: "00000000-0000-4000-8000-00000000060e", content_hash: { algorithm: "blake3-256", domain: "MetaCog.Invocation", digest_hex: "2".repeat(64) } }, command: { source_type: "mcg.command_execution.v1", locator: "command-60f", content_hash: { algorithm: "blake3-256", domain: "MetaCog.Command", digest_hex: "3".repeat(64) } }, approval: { source_type: "mcg.approval_request.v1", locator: "00000000-0000-4000-8000-000000000610", content_hash: { algorithm: "blake3-256", domain: "MetaCog.Approval", digest_hex: "4".repeat(64) } }, agent: { source_type: "mcg.runtime_actor.v1", locator: canonicalAction.actor, content_hash: { algorithm: "blake3-256", domain: "MetaCog.Actor", digest_hex: "5".repeat(64) } }, evidence: [{ source_type: "mcg.action_evidence.v1", locator: "artifact-611", content_hash: { algorithm: "blake3-256", domain: "MetaCog.Evidence", digest_hex: "6".repeat(64) } }] }, effect_class: canonicalAction.effect_class, operation: actionPayload, operation_hash: { algorithm: "blake3-256", domain: "MetaCog.ActionOperation", digest_hex: "f".repeat(64) }, created_at: now },
      executions: [{ id: canonicalExecutionId, action_id: canonicalActionId, attempt_number: 1, state: canonicalAction.state, revision: 3, lease_generation: 2, executor: canonicalAction.actor, outcome: canonicalAction.outcome, created_at: now, updated_at: now }],
      receipts: [],
      reconciliation_jobs: [{ id: "00000000-0000-4000-8000-00000000060a", action_id: canonicalActionId, execution_id: canonicalExecutionId, adapter: actionPayload, state: "pending", revision: 1, lease_generation: 0, lease_owner: null, lease_expires_at: null, next_attempt_at: now, attempts: 0, deadline: "2026-08-18T01:00:00Z", last_hint: null, created_at: now, updated_at: now }],
      events: [{ id: "00000000-0000-4000-8000-00000000060b", type_id: "mcg.action_classified.v1", schema_version: 1, sequence: 4, action_id: canonicalActionId, execution_id: canonicalExecutionId, correlation_id: "00000000-0000-4000-8000-00000000060c", causation_id: null, idempotency_key: "action-classify-1", occurred_at: now, payload: actionPayload }]
    };
    const history = [{ settings: initial, reason: "initial settings", changed_at: initial.updated_at }];
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      value: {
        invoke: async (command: string, args: Record<string, any> = {}) => {
          if (command === "get_user_settings") return settings;
          if (command === "protected_root_status") return {
            schema_version: 1,
            service: "protected-root",
            process_id: 4242,
            protocol_version: 1,
            session_id: "00000000-0000-4000-8000-000000000401",
            runtime_identity: { instance_id: "00000000-0000-4000-8000-000000000402", build_hash: { algorithm: "sha256", domain: "RootExecutable", digest_hex: "a".repeat(64) }, protocol_version: 1, started_at: "2026-08-29T00:00:00Z" },
            storage_schema_version: 51,
            writer_queue_capacity: 64,
            migration_receipt: { schema_version: 1, from_storage_schema: 50, to_storage_schema: 51, backup_path: "backups/pre-root.mcgb", backup_sha256: "b".repeat(64), migrated_at: "2026-08-29T00:00:00Z" }
          };
          if (command === "protected_root_workflows") return [canonicalWorkflow];
          if (command === "protected_root_workflow") {
            if (args.workflowId !== canonicalWorkflowId) return null;
            return { attempt: canonicalWorkflow, task_contract: canonicalTaskContract, events: canonicalEvents };
          }
          if (command === "protected_root_actions") return [canonicalAction];
          if (command === "protected_root_action") return args.actionId === canonicalActionId ? canonicalActionInspection : null;
          if (command === "cancel_protected_root_workflow") {
            if (args.request.attempt_id !== canonicalAttemptId || args.request.expected_revision !== 7 || args.request.expected_generation !== 3) {
              throw new Error("stale workflow cancellation fence");
            }
            canonicalWorkflow = { ...canonicalWorkflow, state: "cancel_requested", revision: 8, updated_at: "2026-08-18T00:01:00Z" };
            return canonicalWorkflow;
          }
          if (command === "update_configuration") return { schema_version: 1, configured: false, current_version: "0.1.0", detail: "This build has no trusted update endpoint." };
          if (command === "user_settings_history") return { revisions: [...history] };
          if (command === "list_backups") return { backups: [] };
          if (command === "list_plugins") return { plugins: [plugin] };
          if (command === "list_ssh_hosts") return { hosts: sshHosts };
          if (command === "list_ssh_workflows") return { workflows: sshWorkflows };
          if (command === "list_projects") return { projects: [project] };
          if (command === "prepare_cloud_diff_apply") {
            cloudApplyCandidate = {
              schema_version: 1,
              id: "00000000-0000-4000-8000-000000000271",
              project_id: args.projectId,
              task_id: args.taskId,
              state: "ready",
              root: "C:\\MetaCog\\cloud-applies\\fixture",
              branch: "mcg/cloud/fixture",
              base_head: "abc",
              baseline_head: "abc",
              baseline_sha256: "b".repeat(64),
              baseline_entries: [],
              baseline_exclusions: [],
              changed_paths: Array.from({ length: 18 }, (_, index) => `src/cloud/change-${String(index + 1).padStart(2, "0")}.ts`),
              touched_paths: [],
              patch_sha256: "c".repeat(64),
              output: "Applied patch to 18 files in the isolated MetaCog worktree.",
              error: null,
              revision: 2,
              created_at: now,
              updated_at: now
            };
            return cloudApplyCandidate;
          }
          if (command === "list_cloud_diff_applies") {
            return { applies: cloudApplyCandidate?.state === "ready" ? [cloudApplyCandidate] : [] };
          }
          if (command === "apply_cloud_diff") {
            if (!cloudApplyCandidate || args.applyId !== cloudApplyCandidate.id || args.expectedRevision !== cloudApplyCandidate.revision) throw new Error("stale cloud apply candidate");
            cloudApplyCandidate = { ...cloudApplyCandidate, state: "applied", revision: cloudApplyCandidate.revision + 1, updated_at: "2026-08-18T00:01:00Z" };
            return cloudApplyCandidate;
          }
          if (command === "discard_cloud_diff_apply") {
            if (!cloudApplyCandidate || args.applyId !== cloudApplyCandidate.id || args.expectedRevision !== cloudApplyCandidate.revision) throw new Error("stale cloud apply candidate");
            cloudApplyCandidate = { ...cloudApplyCandidate, state: "removed", changed_paths: [], output: null, revision: cloudApplyCandidate.revision + 1, updated_at: "2026-08-18T00:01:00Z" };
            return cloudApplyCandidate;
          }
          if (command === "codex_permission_profiles") return [
            { id: ":workspace", description: "Project workspace", allowed: true },
            { id: "workspace-safe", description: "Write only inside the project.", allowed: true },
            { id: "organization-blocked", description: "Blocked by managed policy.", allowed: false }
          ];
          if (command === "pick_writable_folder") return "C:\\Shared\\assets";
          if (command === "pick_sandbox_read_folder") return "C:\\Reference\\docs";
          if (command === "search_project_mentions") return {
            candidates: [
              { kind: "file", label: "main.ts", resource: "src/main.ts", detail: "src/main.ts" },
              { kind: "folder", label: "src", resource: "src", detail: "src" }
            ].filter((candidate) => !args.query || candidate.label.toLowerCase().includes(String(args.query).toLowerCase()))
          };
          if (command === "list_conversations") return { conversations: [] };
          if (command === "codex_features") return codexFeatures;
          if (command === "codex_configuration_status") return {
            requirements: {},
            provider_capabilities: {},
            windows_sandbox: { status: windowsSandboxReady ? "ready" : "notConfigured" },
            local_providers: {
              configured_provider: "ollama",
              providers: [
                { id: "ollama", display_name: "Ollama", default_model: "gpt-oss:20b", endpoint: "http://localhost:11434/v1", running: true, responses_supported: true, version: "0.13.4", models: ["gpt-oss:20b", "gpt-oss:120b"], detail: "Ready for Codex Responses API work" },
                { id: "lmstudio", display_name: "LM Studio", default_model: "openai/gpt-oss-20b", endpoint: "http://localhost:1234/v1", running: false, responses_supported: false, models: [], detail: "LM Studio is not responding" }
              ]
            }
          };
          if (command === "codex_harness_status") return {
            bundled_version: "0.150.1", protocol_version: "0.150.1", schema_sha256: "8".repeat(64),
            process_state: "ready", initialized: true, account_state: "signed_out", active_threads: 0,
            pending_approvals: 0, pending_requests: 0, unsupported_requests: 0, restart_count: 0,
            configuration_state: "ready", windows_sandbox_state: "ready", mcp_bridge_state: "ready", recovery_state: "healthy"
          };
          if (command === "setup_windows_sandbox") {
            windowsSandboxReady = true;
            return { mode: args.mode, readiness: "ready", config_write_status: "ok" };
          }
          if (command === "grant_windows_sandbox_read_directory") return { path: args.path, permission_profile: args.permissionProfile };
          if (command === "codex_stable_discovery") return {
            skills: [],
            hooks: [],
            apps: [{ id: "calendar", name: "Calendar", description: "Work with calendar events", isAccessible: true, isEnabled: true }],
            appsAvailable: true,
            refreshedAt: now
          };
          if (command === "set_codex_feature") {
            codexFeatures = codexFeatures.map((feature) => feature.name === args.name ? { ...feature, enabled: args.enabled } : feature);
            return codexFeatures;
          }
          if (command === "list_open_in_applications") return { applications: [] };
          if (command === "list_tasks") return { tasks: [] };
          if (command === "list_organizations") return { organizations };
          if (command === "list_organization_publishers") return [organizationPublisher];
          if (command === "list_organization_share_reviews") return organizationShareReviews;
          if (command === "review_shared_organization") {
            const review = {
              schema_version: 1,
              id: args.request.review_id,
              package: args.request.package,
              evidence: {
                schema_version: 1,
                registry: args.request.package.registry,
                package: args.request.package.package,
                publisher: args.request.package.publisher,
                publisher_key_sha256: "b".repeat(64),
                portable_sha256: args.request.package.portable.content_sha256,
                package_sha256: "c".repeat(64),
                signature_sha256: "d".repeat(64)
              },
              state: "pending",
              created_at: now,
              updated_at: now
            };
            organizationShareReviews = [review];
            return { review, replayed: false };
          }
          if (command === "import_shared_organization") {
            const review = { ...organizationShareReviews[0], state: "imported", import_id: args.request.import_id, organization_id: baseOrganization.organization_id, decision_reason: args.request.reason };
            organizationShareReviews = [review];
            return { review, organization: baseOrganization, replayed: false, events: [] };
          }
          if (command === "organization_history") return { revisions: organizations.filter((item) => item.organization_id === args.organizationId) };
          if (command === "export_organization") return portableOrganization;
          if (command === "import_organization") {
            const copy = { ...baseOrganization, organization_id: "00000000-0000-4000-8000-000000000254", revision_id: "00000000-0000-4000-8000-000000000255", name: args.request.name ?? args.request.portable.definition.name };
            organizations = [baseOrganization, copy];
            return { organization: copy, portable_sha256: args.request.portable.content_sha256, replayed: false };
          }
          if (command === "compare_organizations") return { equivalent: true, left_sha256: portableOrganization.content_sha256, right_sha256: portableOrganization.content_sha256, sections_changed: [], roles_added: [], roles_removed: [], roles_changed: [], relationships_added: [], relationships_removed: [] };
          if (command === "list_project_memory") return { claims: memoryClaims };
          if (command === "list_project_memory_transfers") return { transfers: [] };
          if (command === "list_reviewed_topology_plans") return { plans: reviewedPlans };
          if (command === "project_topology_plan") return {
            schema_version: 2,
            sha256: "b".repeat(64),
            project_id: project.project_id,
            project_snapshot_id: project.id,
            status: "ready",
            stages: [
              { index: 0, nodes: [{ id: "database:main", kind: "database", label: "main database", source: "compose.yaml" }] },
              { index: 1, nodes: [{ id: "service:web", kind: "service", label: "web", source: "compose.yaml" }] }
            ],
            unresolved_nodes: []
          };
          if (command === "create_reviewed_topology_plan") {
            const step = args.request.steps[0];
            const plan = {
              schema_version: 1,
              id: args.request.id,
              definition_hash: "c".repeat(64),
              project_id: project.project_id,
              project_snapshot_id: project.id,
              project_fingerprint: project.fingerprint,
              topology_sha256: args.request.topology_sha256,
              display_name: args.request.display_name,
              reason: args.request.reason,
              state: "reviewed",
              steps: [{ ...step, executable: "pnpm", arguments: ["test"], source: "package.json", state: "pending", detail: "", log_artifact_sha256: [] }],
              created_at: now,
              updated_at: now
            };
            reviewedPlans = [plan];
            return { plan, replayed: false, events: [], outcomes: [] };
          }
          if (command === "run_reviewed_topology_plan") {
            reviewedRunCount += 1;
            const waiting = reviewedRunCount === 1;
            const plan = {
              ...reviewedPlans[0],
              state: waiting ? "waiting" : "succeeded",
              steps: reviewedPlans[0].steps.map((step: any) => waiting ? step : { ...step, state: "succeeded", detail: "process completed" })
            };
            reviewedPlans = [plan];
            return { plan, replayed: false, events: [], outcomes: [], approval_id: waiting ? "00000000-0000-4000-8000-000000000534" : undefined };
          }
          if (command === "cancel_reviewed_topology_plan") return { plan: { ...reviewedPlans[0], state: "cancelled" }, replayed: false, events: [], outcomes: [] };
          if (command === "list_approvals") return { approvals: [] };
          if (command === "resolve_approval") return {};
          if (command === "managed_project_memory") return { markdown: `# Managed Project Memory\n\n${memoryClaims.map((claim) => `- ${claim.key}: ${claim.value}`).join("\n")}` };
          if (command === "create_project_memory") {
            const claim = { schema_version: 1, id: "00000000-0000-4000-8000-000000000243", project_id: project.project_id, state: "new", observed_at: now, correction_count: 0, usefulness_count: 0, ...args.request };
            memoryClaims = [claim];
            return { claim, event: { sequence: 1, kind: "memory_claim_recorded", occurred_at: now } };
          }
          if (command === "transition_project_memory") {
            const previous = memoryClaims[0];
            const claim = { ...previous, id: "00000000-0000-4000-8000-000000000244", state: args.request.action === "reinforce" ? "working" : args.request.action === "decay" ? "decaying" : "retracted", usefulness_count: args.request.action === "reinforce" ? 1 : previous.usefulness_count };
            memoryClaims = [claim];
            return { claim, previous, event: { sequence: 2, kind: "memory_claim_state_changed", occurred_at: now } };
          }
          if (command === "project_memory_history") return { claims: memoryClaims };
          if (command === "create_ssh_workflow") {
            const workflow = { schema_version: 1, definition_hash: "f".repeat(64), state: "planned", created_at: now, updated_at: now, ...args.request, steps: args.request.steps.map((step: any, index: number) => ({ ...step, operation_id: `00000000-0000-4000-8000-${String(230 + index).padStart(12, "0")}`, state: "pending", detail: "" })) };
            sshWorkflows = [workflow];
            return { workflow, replayed: false, events: [] };
          }
          if (command === "run_ssh_workflow") {
            const workflow = { ...sshWorkflows[0], state: "succeeded", updated_at: "2026-08-18T00:01:00Z", steps: sshWorkflows[0].steps.map((step: any) => ({ ...step, state: "succeeded", detail: "remote inspection completed" })) };
            sshWorkflows = [workflow];
            return { workflow, replayed: false, events: [] };
          }
          if (command === "set_plugin_grants") {
            plugin.installation.capability_grants = args.request.grants;
            plugin.installation.revision += 1;
            return plugin.installation;
          }
          if (command === "set_plugin_enabled") {
            plugin.installation.enabled = args.request.enabled;
            plugin.installation.revision += 1;
            return plugin.installation;
          }
          if (command === "invoke_plugin") return {
            invocation: { schema_version: 1, id: args.invocationId, plugin_id: plugin.installation.plugin_id, plugin_revision_id: plugin.active_revision.id, input_sha256: "b".repeat(64), state: "succeeded", started_at: "2026-08-15T00:02:00Z", completed_at: "2026-08-15T00:02:01Z" },
            result: { schema_version: 1, plugin_id: plugin.installation.plugin_id, output: { forecast: "sunny" } },
            replayed: false
          };
          if (command === "update_user_settings" && args.request) {
            settings = {
              schema_version: 2,
              revision: settings.revision + 1,
              interface_complexity: args.request.interface_complexity,
              autonomy: args.request.autonomy,
              shadow_experiments: args.request.shadow_experiments,
              updated_at: "2026-08-15T00:01:00Z"
            };
            history.push({ settings, reason: args.request.reason ?? "updated", changed_at: settings.updated_at });
            return settings;
          }
          throw new Error(`No browser fixture for ${command}`);
        }
      },
      configurable: true
    });
  });
});

async function openOwnerMenu(page: import("@playwright/test").Page) {
  const menu = page.locator(".owner-menu");
  if (!await menu.isVisible()) {
    await page.getByRole("button", { name: "Open owner menu" }).click();
  }
  await expect(menu).toBeVisible();
  return menu;
}

async function openOwnerSurface(page: import("@playwright/test").Page, name: string) {
  if (name === "Settings" || name === "Usage") {
    const menu = await openOwnerMenu(page);
    await menu.getByRole("button", { name: new RegExp(`^${name}`) }).click();
    return;
  }
  const surface = ({ Projects: "projects", Providers: "providers", MCP: "mcp", SSH: "ssh", Improvement: "improvement" } as Record<string, string>)[name];
  if (!surface) throw new Error(`Unknown test surface ${name}`);
  await page.evaluate((target) => {
    window.history.pushState(null, "", `#${target}`);
    window.dispatchEvent(new PopStateEvent("popstate"));
  }, surface);
}

async function enableAdvancedMode(page: import("@playwright/test").Page) {
  if (!await page.getByRole("region", { name: "Settings" }).isVisible()) {
    await openOwnerSurface(page, "Settings");
  }
  await page.getByRole("button", { name: "General", exact: true }).click();
  await page.getByRole("radio", { name: /Advanced/ }).click();
  await expect(page.getByRole("radio", { name: /Advanced/ })).toBeChecked();
  await page.getByRole("button", { name: "Back to app" }).click();
  await expect(page.getByRole("heading", { name: "New chat", level: 1 })).toBeVisible();
}

test("Simple Advanced and Developer share one workspace while exposing progressive controls", async ({ page }) => {
  await page.goto("/");
  const composer = page.getByRole("textbox", { name: "Message" });
  await composer.fill("Keep this active work unchanged");
  await openOwnerSurface(page, "Settings");
  await page.getByRole("button", { name: "General", exact: true }).click();

  await page.getByRole("radio", { name: /Advanced/ }).click();
  await expect(page.getByRole("radio", { name: /Advanced/ })).toBeChecked();
  await page.getByRole("radio", { name: /Developer/ }).click();
  await expect(page.getByText(/Developer mode remains governed/)).toBeVisible();
  await expect(page.getByText("Runtime contract inspector")).toBeVisible();
  await expect(page.getByLabel("Current runtime settings contract")).toContainText("protected-root");
  await expect(page.getByLabel("Current runtime settings contract")).toContainText("pre-root.mcgb");

  await page.getByRole("button", { name: "Back to app" }).click();
  await expect(page.getByRole("button", { name: "Cycle interface complexity" })).toHaveText("Developer");
  await expect(composer).toHaveValue("Keep this active work unchanged");

  await openOwnerSurface(page, "Settings");
  await page.getByRole("radio", { name: /Simple/ }).click();
  await expect(page.getByRole("radio", { name: /Simple/ })).toBeChecked();
  await page.getByRole("button", { name: "Back to app" }).click();
  await expect(composer).toHaveValue("Keep this active work unchanged");
});

test("durable workflows keep one Root truth across Simple Advanced and Developer", async ({ page }) => {
  test.setTimeout(90_000);
  await page.goto("/");
  await page.getByRole("link", { name: "Agents", exact: true }).click();

  let workflows = page.getByRole("article", { name: "Durable workflows" });
  await expect(workflows.getByText("Working", { exact: true })).toBeVisible();
  await expect(workflows.getByText(/attempt 1/)).toHaveCount(0);
  await expect(workflows.getByRole("button", { name: "Inspect" })).toHaveCount(0);
  await expect(workflows.getByRole("button", { name: "Canonical workflow evidence" })).toHaveCount(0);

  await enableAdvancedMode(page);
  await page.getByRole("link", { name: "Agents", exact: true }).click();
  workflows = page.getByRole("article", { name: "Durable workflows" });
  await expect(workflows.getByText(/Working · attempt 1 · revision 7 · generation 3/)).toBeVisible();
  await expect(workflows.getByText(/Lease owner/)).toContainText("runtime-worker-1");
  await workflows.getByRole("button", { name: "Inspect" }).click();
  await expect(workflows.getByText("Recover the exact durable workflow")).toBeVisible();
  await expect(workflows.getByText("The same attempt and event sequence remain visible after reconnect.")).toBeVisible();
  await expect(workflows.getByRole("button", { name: "Canonical workflow evidence" })).toHaveCount(0);

  await openOwnerSurface(page, "Settings");
  await page.getByRole("button", { name: "General", exact: true }).click();
  await page.getByRole("radio", { name: /Developer/ }).click();
  await expect(page.getByRole("radio", { name: /Developer/ })).toBeChecked();
  await page.getByRole("button", { name: "Back to app" }).click();
  await page.getByRole("link", { name: "Agents", exact: true }).click();
  workflows = page.getByRole("article", { name: "Durable workflows" });
  const evidence = workflows.getByRole("button", { name: "Canonical workflow evidence" });
  await evidence.click();
  await expect(workflows.getByRole("textbox", { name: /Canonical workflow .* evidence/ })).toContainText("mcg.workflow.attempt.claimed");
  await expect(workflows.getByRole("textbox", { name: /Canonical workflow .* evidence/ })).toContainText("correlation_id");

  page.once("dialog", (dialog) => dialog.accept());
  await workflows.getByRole("button", { name: "Request cancellation" }).click();
  await expect(workflows.getByText(/Stopping safely · attempt 1 · revision 8 · generation 3/)).toBeVisible();

  await openOwnerSurface(page, "Settings");
  await page.getByRole("button", { name: "General", exact: true }).click();
  await page.getByRole("radio", { name: /Simple/ }).click();
  await page.getByRole("button", { name: "Back to app" }).click();
  await page.getByRole("link", { name: "Agents", exact: true }).click();
  workflows = page.getByRole("article", { name: "Durable workflows" });
  await expect(workflows.getByText("Stopping safely", { exact: true })).toBeVisible();
  await expect(workflows.getByRole("button", { name: "Canonical workflow evidence" })).toHaveCount(0);
});

test("governed actions keep one Root truth across Simple Advanced and Developer", async ({ page }) => {
  test.setTimeout(90_000);
  await page.goto("/");
  await page.getByRole("link", { name: "Agents", exact: true }).click();

  let actions = page.getByRole("article", { name: "Governed actions" });
  await expect(actions.getByText("Checking what happened", { exact: true })).toBeVisible();
  await expect(actions.getByText(/revision 3/)).toHaveCount(0);
  await expect(actions.getByRole("button", { name: "Inspect" })).toHaveCount(0);
  await expect(actions.getByRole("button", { name: "Canonical action evidence" })).toHaveCount(0);

  await enableAdvancedMode(page);
  await page.getByRole("link", { name: "Agents", exact: true }).click();
  actions = page.getByRole("article", { name: "Governed actions" });
  await expect(actions.getByText(/Checking what happened · reversible · revision 3 · generation 2/)).toBeVisible();
  await actions.getByRole("button", { name: "Inspect" }).click();
  await expect(actions.getByText("mcg.governed_filesystem_operation.v1")).toBeVisible();
  await expect(actions.getByText("Reconciliation: pending")).toBeVisible();
  await expect(actions.getByRole("region", { name: "Bound action origin" })).toContainText("command-60f");
  await expect(actions.getByRole("region", { name: "Bound action origin" })).toContainText("artifact-611");
  await expect(actions.getByRole("button", { name: /Open originating conversation/ })).toBeDisabled();
  await expect(actions.getByRole("button", { name: "Canonical action evidence" })).toHaveCount(0);

  await openOwnerSurface(page, "Settings");
  await page.getByRole("button", { name: "General", exact: true }).click();
  await page.getByRole("radio", { name: /Developer/ }).click();
  await page.getByRole("button", { name: "Back to app" }).click();
  await page.getByRole("link", { name: "Agents", exact: true }).click();
  actions = page.getByRole("article", { name: "Governed actions" });
  await actions.getByRole("button", { name: "Canonical action evidence" }).click();
  const evidence = actions.getByRole("textbox", { name: /Canonical action .* evidence/ });
  await expect(evidence).toContainText("mcg.action_classified.v1");
  await expect(evidence).toContainText("correlation_id");
  const accessibility = await new AxeBuilder({ page }).include(".canonical-action-section").analyze();
  expect(accessibility.violations, "governed-action accessibility violations").toEqual([]);

  await openOwnerSurface(page, "Settings");
  await page.getByRole("button", { name: "General", exact: true }).click();
  await page.getByRole("radio", { name: /Simple/ }).click();
  await page.getByRole("button", { name: "Back to app" }).click();
  await page.getByRole("link", { name: "Agents", exact: true }).click();
  actions = page.getByRole("article", { name: "Governed actions" });
  await expect(actions.getByText("Checking what happened", { exact: true })).toBeVisible();
  await expect(actions.getByRole("button", { name: "Inspect" })).toHaveCount(0);
  await expect(actions.getByRole("button", { name: "Canonical action evidence" })).toHaveCount(0);
});

test("consumer shell starts simple and can reveal advanced status", async ({ page }) => {
  test.setTimeout(180_000);
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "New chat" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "What should we build?" })).toBeVisible();
  await expect(page.locator("#projects")).toBeHidden();
  await page.getByRole("link", { name: "Pull requests", exact: true }).click();
  const pullRequests = page.getByRole("region", { name: "Pull requests" });
  await expect(pullRequests.getByText("No local reviews yet")).toBeVisible();
  await expect(pullRequests.getByText("Select a review to view")).toBeVisible();
  await expect(pullRequests.getByRole("button", { name: "Start local review" })).toBeVisible();
  await page.getByRole("link", { name: "Sites", exact: true }).click();
  const simpleSites = page.getByRole("region", { name: "Sites" });
  await expect(simpleSites.getByRole("heading", { name: "Sites" })).toBeVisible();
  await expect(simpleSites.getByRole("button", { name: "New site" })).toBeVisible();
  await expect(simpleSites.getByText("No sites yet")).toBeVisible();
  await expect(simpleSites.getByLabel("Public URL")).toHaveCount(0);
  await simpleSites.getByRole("button", { name: "New site" }).click();
  await expect(page.getByRole("textbox", { name: "Message" })).toHaveValue(/Build a website/);
  await openOwnerSurface(page, "Settings");
  await page.getByRole("button", { name: "Import", exact: true }).click();
  const globalFolders = page.getByLabel("Global folders");
  await expect(globalFolders.getByRole("button", { name: "Add global folder" })).toBeVisible();
  await expect(globalFolders).toContainText("does not create a project");
  await page.getByRole("button", { name: "Configuration", exact: true }).click();
  const updates = page.getByLabel("Application updates");
  await expect(updates.getByText(/Updates are unavailable in this build/)).toBeVisible();
  await expect(updates.getByText(/never accept an unsigned fallback/)).toBeVisible();

  await enableAdvancedMode(page);

  await openOwnerSurface(page, "Projects");
  await expect(page.getByRole("heading", { name: "Runtime status" })).toBeVisible();
  await openOwnerSurface(page, "MCP");
  await expect(page.getByRole("heading", { name: "MCP servers" })).toBeVisible();
  await page.getByText("Server transport", { exact: true }).click();
  await expect(page.getByLabel("Connection id")).toBeVisible();
  await expect(page.getByLabel("Protocol")).toHaveValue("auto");
  await openOwnerSurface(page, "Settings");
  const settings = page.getByLabel("Owner settings");
  await expect(settings.getByLabel("Autonomy")).toHaveValue("autonomous");
  await expect(settings.getByLabel("Run opt-in shadow model experiments")).toBeChecked();
  await expect(settings.getByLabel("Settings revision history")).toContainText("Enabled advanced interface");
  await page.getByRole("button", { name: "Configuration", exact: true }).click();
  await page.getByText("Windows sandbox", { exact: true }).click();
  const windowsSandbox = page.getByLabel("Windows sandbox configuration");
  await expect(windowsSandbox.getByLabel("Windows sandbox project")).toHaveValue("00000000-0000-4000-8000-000000000242");
  await windowsSandbox.getByRole("button", { name: "Set up" }).click();
  await expect(windowsSandbox.getByText(/Elevated Windows sandbox is configured/)).toBeVisible();
  await windowsSandbox.getByRole("button", { name: "Browse" }).click();
  await expect(windowsSandbox.getByLabel("Windows sandbox read directory")).toHaveValue("C:\\Reference\\docs");
  await windowsSandbox.getByRole("button", { name: "Grant read access" }).click();
  await expect(windowsSandbox.getByText(/Codex sandbox read access is ready/)).toBeVisible();
  await page.getByText("Codex feature flags", { exact: true }).click();
  const featureFlags = page.locator('.codex-feature-list[aria-label="Codex feature flags"]');
  await expect(featureFlags).toBeVisible();
  await page.getByRole("textbox", { name: "Search Codex feature flags" }).fill("memor");
  await expect(featureFlags.getByText("Memories", { exact: true })).toBeVisible();
  await expect(featureFlags.getByText("shell_tool", { exact: true })).toHaveCount(0);
  await featureFlags.getByRole("checkbox", { name: "Enable memories" }).check();
  await expect(featureFlags.getByRole("checkbox", { name: "Disable memories" })).toBeChecked();
  expect(await featureFlags.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
  await page.getByRole("button", { name: "Back to app" }).click();
  await page.getByRole("link", { name: "Sites", exact: true }).click();
  const sites = page.getByRole("region", { name: "Sites" });
  await expect(sites.getByRole("heading", { name: "Sites" })).toBeVisible();
  await expect(sites.getByRole("button", { name: "New site" })).toBeVisible();
  await page.getByRole("link", { name: "Scheduled", exact: true }).click();
  const scheduler = page.getByRole("region", { name: "Scheduler" });
  await scheduler.locator("summary").filter({ hasText: "Exact scheduler controls" }).click();
  await expect(scheduler.getByLabel("Trigger")).toBeVisible();
  await scheduler.getByLabel("Trigger").selectOption("cron");
  await expect(scheduler.getByLabel("Five-field cron")).toHaveValue("0 2 * * *");

  await openOwnerSurface(page, "Providers");
  await page.getByRole("button", { name: "Add API or local provider" }).click();
  await expect(page.getByLabel("Provider type")).toHaveValue("open_ai_compatible");
  await expect(page.getByLabel("Connection id")).toHaveCount(2);

  await openOwnerSurface(page, "Settings");
  await page.getByRole("radio", { name: /Simple/ }).click();
  await expect(page.getByRole("radio", { name: /Simple/ })).toBeChecked();
  await page.getByRole("button", { name: "Back to app" }).click();
  await page.getByRole("link", { name: "Scheduled", exact: true }).click();
  await expect(scheduler.getByRole("heading", { name: "Scheduled tasks" })).toBeVisible();
  await expect(scheduler.getByRole("button", { name: "Set up in chat" })).toBeVisible();
  await expect(scheduler.getByLabel("Trigger")).toHaveCount(0);
  await scheduler.getByLabel("Project").selectOption({ index: 1 });
  await scheduler.getByLabel("Scheduled work").fill("Summarize important project changes every morning");
  await scheduler.getByRole("button", { name: "Set up in chat" }).click();
  const activeNewChat = page.locator(".new-chat-surface:not([hidden])");
  await expect(activeNewChat.getByRole("textbox", { name: "Message" })).toHaveValue(/Summarize important project changes every morning/);
});

test("Codex cloud apply stays reviewable and contained before explicit integration", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("link", { name: "Pull requests", exact: true }).click();
  const pullRequests = page.getByRole("region", { name: "Pull requests" });
  const codeChangeActions = pullRequests.getByLabel("Code change actions");
  const applyCloudTask = codeChangeActions.getByRole("button", { name: "Apply cloud task" });
  await applyCloudTask.click();
  await expect(pullRequests.getByRole("heading", { name: "Apply a Codex cloud task" })).toBeVisible();
  await expect(pullRequests.getByText("Your project is unchanged while MetaCog prepares and validates the diff.")).toBeVisible();
  await pullRequests.getByLabel("Repository").selectOption("00000000-0000-4000-8000-000000000242");
  await pullRequests.getByLabel("Codex cloud task ID").fill("cloud-task-42");
  await pullRequests.getByRole("button", { name: "Check changes" }).click();

  const files = pullRequests.getByLabel("Cloud task changed files");
  await expect(files).toBeVisible();
  await expect(files.getByText("src/cloud/change-01.ts")).toBeVisible();
  await expect(pullRequests.getByText("18 changed files")).toBeVisible();
  await expect(pullRequests.getByText(/If the repository changed since preparation/)).toBeVisible();
  const geometry = await files.evaluate((element) => ({
    clientHeight: element.clientHeight,
    scrollHeight: element.scrollHeight,
    right: element.getBoundingClientRect().right,
    viewport: document.documentElement.clientWidth
  }));
  expect(geometry.scrollHeight).toBeGreaterThan(geometry.clientHeight);
  expect(geometry.right).toBeLessThanOrEqual(geometry.viewport);
  await files.hover();
  await page.mouse.wheel(0, 500);
  await expect.poll(() => files.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);

  const darkAccessibility = await new AxeBuilder({ page }).include(".pull-request-detail-pane").analyze();
  expect(darkAccessibility.violations, "cloud apply dark-theme accessibility violations").toEqual([]);
  await page.evaluate(() => document.documentElement.setAttribute("data-theme", "light"));
  const lightActionContrast = await applyCloudTask.evaluate((button) => {
    const parseRgb = (value: string) => value.match(/[\d.]+/g)?.slice(0, 3).map(Number) ?? [];
    const luminance = (value: string) => {
      const [red, green, blue] = parseRgb(value).map((channel) => {
        const normalized = channel / 255;
        return normalized <= 0.04045
          ? normalized / 12.92
          : ((normalized + 0.055) / 1.055) ** 2.4;
      });
      return 0.2126 * red + 0.7152 * green + 0.0722 * blue;
    };
    const style = getComputedStyle(button);
    const foreground = luminance(style.color);
    const background = luminance(style.backgroundColor);
    return (Math.max(foreground, background) + 0.05) / (Math.min(foreground, background) + 0.05);
  });
  expect(lightActionContrast, "light-theme Pull Requests action contrast").toBeGreaterThanOrEqual(4.5);
  const lightAccessibility = await new AxeBuilder({ page }).include(".pull-request-detail-pane").analyze();
  expect(lightAccessibility.violations, "cloud apply light-theme accessibility violations").toEqual([]);
  await page.evaluate(() => document.documentElement.setAttribute("data-theme", "dark"));

  await pullRequests.getByRole("button", { name: "Apply changes" }).click();
  await expect(pullRequests.getByRole("status")).toHaveText("Changes applied to the working tree.");
  await expect(page.locator("html")).toHaveJSProperty("scrollWidth", await page.locator("html").evaluate((element) => element.clientWidth));
});

test("new chat run menus stay compact, aligned, and use the shared icon set", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "New chat" })).toBeVisible();
  for (const label of ["Choose project", "Workspace: Local", "Add files and more", "Change permissions"]) {
    await expect(page.getByLabel(label).locator("svg").first()).toBeVisible();
  }

  const selector = page.locator('summary[aria-label="New chat model, effort, speed, and team"]');
  await selector.click();
  const dialog = page.getByRole("dialog", { name: "New chat model, effort, speed, and team" });
  await expect(dialog.locator(".chat-run-menu-icon svg")).toHaveCount(4);
  await dialog.getByRole("button", { name: /^Effort/ }).click();

  const effortMenu = page.getByRole("menu", { name: "Effort" });
  const choices = effortMenu.getByRole("menuitemradio");
  await expect(choices).toHaveCount(5);
  const layout = await choices.evaluateAll((rows) => rows.map((row) => {
    const copy = row.querySelector<HTMLElement>(".chat-run-choice-copy");
    const check = row.querySelector<HTMLElement>(".chat-run-choice-check");
    if (!copy || !check) throw new Error("Run-setting choice columns are missing");
    return {
      copyLeft: Math.round(copy.getBoundingClientRect().left * 10) / 10,
      checkLeft: Math.round(check.getBoundingClientRect().left * 10) / 10,
      copyColor: getComputedStyle(copy).color,
      hasCheck: Boolean(check.querySelector("svg"))
    };
  }));
  expect(new Set(layout.map((row) => row.copyLeft)).size).toBe(1);
  expect(new Set(layout.map((row) => row.checkLeft)).size).toBe(1);
  expect(new Set(layout.map((row) => row.copyColor)).size).toBe(1);
  expect(layout.filter((row) => row.hasCheck)).toHaveLength(1);

  const selectorBox = await selector.boundingBox();
  const dialogBox = await dialog.boundingBox();
  expect(selectorBox).not.toBeNull();
  expect(dialogBox).not.toBeNull();
  expect(dialogBox?.width ?? 999).toBeLessThanOrEqual(350);
  expect(Math.abs((dialogBox?.x ?? 0) + (dialogBox?.width ?? 0) - ((selectorBox?.x ?? 0) + (selectorBox?.width ?? 0)))).toBeLessThanOrEqual(2);
  expect(dialogBox?.y ?? -1).toBeGreaterThanOrEqual(0);
});

test("new chat exposes a compact read-only authority preset", async ({ page }) => {
  await page.goto("/");
  const trigger = page.getByLabel("Change permissions");
  await trigger.click();

  const menu = page.getByRole("menu", { name: "How should MetaCog actions be approved?" });
  await expect(menu.getByRole("menuitemradio")).toHaveCount(4);
  const readOnly = menu.getByRole("menuitemradio", { name: /Read only/ });
  await expect(readOnly).toContainText("Inspect the project without allowing file changes");
  await readOnly.click();

  await expect(trigger).toContainText("Read only");
  await expect(trigger.locator("svg").first()).toBeVisible();
  await expect(menu).toBeHidden();
});

test("advanced project chat exposes only allowed Codex permission profiles without overflow", async ({ page }) => {
  await page.goto("/");
  await enableAdvancedMode(page);
  await page.getByRole("button", { name: "consumer-app", exact: true }).hover();
  await page.getByRole("button", { name: "New chat in consumer-app" }).click();

  const trigger = page.getByLabel("Change permissions");
  await trigger.click();
  const menu = page.getByRole("menu", { name: "How should MetaCog actions be approved?" });
  await expect(menu.getByText("Codex permission profiles", { exact: true })).toBeVisible();
  const profile = menu.getByRole("menuitemradio", { name: /workspace-safe/i });
  await expect(profile).toContainText("Write only inside the project.");
  await expect(menu.getByRole("menuitemradio", { name: /organization-blocked/i })).toHaveCount(0);

  const layout = await menu.evaluate((element) => ({
    clientWidth: element.clientWidth,
    scrollWidth: element.scrollWidth,
    clientHeight: element.clientHeight,
    scrollHeight: element.scrollHeight,
    overflowY: getComputedStyle(element).overflowY
  }));
  expect(layout.scrollWidth).toBeLessThanOrEqual(layout.clientWidth + 1);
  expect(["auto", "scroll"]).toContain(layout.overflowY);
  expect(layout.clientHeight).toBeLessThanOrEqual(520);

  await profile.click();
  await expect(trigger).toContainText("workspace-safe");
  await expect(menu).toBeHidden();
});

test("advanced run settings manage additional writable folders without overflow", async ({ page }) => {
  await page.goto("/");
  await openOwnerSurface(page, "Projects");
  await page.getByRole("button", { name: "Start chat" }).click();

  const selector = page.locator('summary[aria-label="New chat model, effort, speed, and team"]');
  await selector.click();
  const dialog = page.getByRole("dialog", { name: "New chat model, effort, speed, and team" });
  await dialog.getByText("Advanced", { exact: true }).click();
  const addFolder = dialog.getByRole("button", { name: "Add folder" });
  await expect(addFolder).toBeEnabled();
  await addFolder.click();
  await expect(dialog.getByText("C:\\Shared\\assets")).toBeVisible();

  const dialogBox = await dialog.boundingBox();
  const viewport = page.viewportSize();
  expect(dialogBox).not.toBeNull();
  expect((dialogBox?.x ?? 0) + (dialogBox?.width ?? 0)).toBeLessThanOrEqual(viewport?.width ?? 0);
  expect((dialogBox?.y ?? 0) + (dialogBox?.height ?? 0)).toBeLessThanOrEqual(viewport?.height ?? 0);
  await dialog.getByRole("button", { name: "Remove writable folder C:\\Shared\\assets" }).click();
  await expect(dialog.getByText("C:\\Shared\\assets")).toHaveCount(0);
});

test("advanced run settings expose local Codex providers without leaking them into Simple", async ({ page }) => {
  await page.goto("/");
  await openOwnerSurface(page, "Projects");
  await page.getByRole("button", { name: "Start chat" }).click();

  await page.locator('summary[aria-label="New chat model, effort, speed, and team"]').click();
  let dialog = page.getByRole("dialog", { name: "New chat model, effort, speed, and team" });
  await expect(dialog.getByRole("button", { name: /^Provider/ })).toHaveCount(0);
  await page.locator('summary[aria-label="New chat model, effort, speed, and team"]').click();

  await page.getByRole("button", { name: "Cycle interface complexity" }).click();
  await page.locator('summary[aria-label="New chat model, effort, speed, and team"]').click();
  dialog = page.getByRole("dialog", { name: "New chat model, effort, speed, and team" });
  await dialog.getByRole("button", { name: /^Provider/ }).click();
  await expect(dialog.getByRole("menuitemradio", { name: /Ollama.*Running.*0\.13\.4/ })).toBeVisible();
  await expect(dialog.getByRole("menuitemradio", { name: /LM Studio.*Not running/ })).toBeVisible();
  await dialog.getByRole("menuitemradio", { name: /Ollama.*Running.*0\.13\.4/ }).click();
  await expect(page.locator('summary[aria-label="New chat model, effort, speed, and team"]')).toContainText("gpt-oss:20b");

  await page.locator('summary[aria-label="New chat model, effort, speed, and team"]').click();
  dialog = page.getByRole("dialog", { name: "New chat model, effort, speed, and team" });
  await dialog.getByRole("button", { name: /^Model/ }).click();
  await expect(dialog.getByRole("menuitemradio", { name: /gpt-oss:120b.*Ollama/ })).toBeVisible();

  const box = await dialog.boundingBox();
  const viewport = page.viewportSize();
  expect(box).not.toBeNull();
  expect((box?.x ?? 0) + (box?.width ?? 0)).toBeLessThanOrEqual(viewport?.width ?? 0);
  expect((box?.y ?? 0) + (box?.height ?? 0)).toBeLessThanOrEqual(viewport?.height ?? 0);
  expect(await dialog.evaluate((element) => element.scrollWidth)).toBeLessThanOrEqual(await dialog.evaluate((element) => element.clientWidth + 1));
});

test("advanced run settings expose bounded per-session Codex feature overrides", async ({ page }) => {
  await page.goto("/");
  await openOwnerSurface(page, "Projects");
  await page.getByRole("button", { name: "Start chat" }).click();
  await page.getByRole("button", { name: "Cycle interface complexity" }).click();

  await page.locator('summary[aria-label="New chat model, effort, speed, and team"]').click();
  const dialog = page.getByRole("dialog", { name: "New chat model, effort, speed, and team" });
  await dialog.getByText("Session features", { exact: true }).click();
  await expect(dialog.getByText("MetaCog-governed authority features are managed elsewhere.")).toBeVisible();
  await dialog.getByRole("searchbox", { name: "Search session features" }).fill("memor");
  await expect(dialog.getByText("Memories", { exact: true })).toBeVisible();
  await expect(dialog.getByText("shell_tool", { exact: true })).toHaveCount(0);
  await dialog.getByRole("combobox", { name: "Memories session override" }).selectOption("enabled");
  await expect(dialog.getByRole("combobox", { name: "Memories session override" })).toHaveValue("enabled");

  const box = await dialog.boundingBox();
  const viewport = page.viewportSize();
  expect(box).not.toBeNull();
  expect((box?.x ?? 0) + (box?.width ?? 0)).toBeLessThanOrEqual(viewport?.width ?? 0);
  expect((box?.y ?? 0) + (box?.height ?? 0)).toBeLessThanOrEqual(viewport?.height ?? 0);
});

test("advanced run settings expose typed bounded Codex config overrides", async ({ page }) => {
  await page.goto("/");
  await openOwnerSurface(page, "Projects");
  await page.getByRole("button", { name: "Start chat" }).click();
  await page.getByRole("button", { name: "Cycle interface complexity" }).click();

  await page.locator('summary[aria-label="New chat model, effort, speed, and team"]').click();
  const dialog = page.getByRole("dialog", { name: "New chat model, effort, speed, and team" });
  await dialog.getByText("Codex config", { exact: true }).click();
  const codexProfile = dialog.getByRole("textbox", { name: "Codex profile" });
  await codexProfile.fill("team.fast");
  await expect(codexProfile).toHaveValue("team.fast");
  const strictConfig = dialog.getByRole("checkbox", { name: "Require strict Codex configuration" });
  await expect(strictConfig).not.toBeChecked();
  await strictConfig.check();
  await expect(strictConfig).toBeChecked();
  await dialog.getByRole("textbox", { name: "Codex configuration key" }).fill("sandbox_mode");
  await dialog.getByRole("textbox", { name: "Codex configuration value" }).fill('"danger-full-access"');
  await dialog.getByRole("button", { name: "Add override" }).click();
  await expect(dialog.getByRole("alert")).toContainText("governed by MetaCog");
  await dialog.getByRole("textbox", { name: "Codex configuration key" }).fill("model_verbosity");
  await dialog.getByRole("textbox", { name: "Codex configuration value" }).fill('"low"');
  await dialog.getByRole("button", { name: "Add override" }).click();
  await expect(dialog.getByText("model_verbosity", { exact: true })).toBeVisible();
  await expect(dialog.getByText('"low"', { exact: true })).toBeVisible();

  await dialog.getByRole("button", { name: "Codex config", exact: true }).click();
  await expect(dialog.getByText(/^Profile team\.fast · Strict/)).toBeVisible();
  await page.locator('summary[aria-label="New chat model, effort, speed, and team"]').click();
  await page.locator('summary[aria-label="New chat model, effort, speed, and team"]').click();
  await dialog.getByText("Codex config", { exact: true }).click();
  await expect(dialog.getByRole("textbox", { name: "Codex profile" })).toHaveValue("team.fast");
  await expect(dialog.getByRole("checkbox", { name: "Require strict Codex configuration" })).toBeChecked();

  const box = await dialog.boundingBox();
  const viewport = page.viewportSize();
  expect(box).not.toBeNull();
  expect((box?.x ?? 0) + (box?.width ?? 0)).toBeLessThanOrEqual(viewport?.width ?? 0);
  expect((box?.y ?? 0) + (box?.height ?? 0)).toBeLessThanOrEqual(viewport?.height ?? 0);
});

test("advanced structured output editor validates JSON without leaking into Simple", async ({ page }) => {
  await page.setViewportSize({ width: 960, height: 720 });
  await page.goto("/");
  await openOwnerSurface(page, "Projects");
  await page.getByRole("button", { name: "Start chat" }).click();

  const selector = page.locator('summary[aria-label="New chat model, effort, speed, and team"]');
  await selector.click();
  let dialog = page.getByRole("dialog", { name: "New chat model, effort, speed, and team" });
  await expect(dialog.getByText("Output format", { exact: true })).toHaveCount(0);
  await selector.click();

  await page.getByRole("button", { name: "Cycle interface complexity" }).click();
  await selector.click();
  dialog = page.getByRole("dialog", { name: "New chat model, effort, speed, and team" });
  await dialog.getByText("Output format", { exact: true }).click();
  const editor = dialog.getByRole("textbox", { name: "Output JSON Schema" });
  await editor.fill("[]");
  await dialog.getByRole("button", { name: "Apply schema" }).click();
  await expect(dialog.getByRole("alert")).toContainText("must be a JSON object");
  await editor.fill('{"type":"object","properties":{"summary":{"type":"string"}},"required":["summary"]}');
  await dialog.getByRole("button", { name: "Apply schema" }).click();
  await expect(dialog.getByRole("alert")).toHaveCount(0);
  await dialog.getByRole("button", { name: "Output format", exact: true }).click();
  await expect(dialog.getByText("Structured JSON", { exact: true })).toBeVisible();

  const box = await dialog.boundingBox();
  const viewport = page.viewportSize();
  expect(box).not.toBeNull();
  expect((box?.x ?? 0) + (box?.width ?? 0)).toBeLessThanOrEqual(viewport?.width ?? 0);
  expect((box?.y ?? 0) + (box?.height ?? 0)).toBeLessThanOrEqual(viewport?.height ?? 0);
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(960);
});

test("advanced run settings expose live web search without changing Simple", async ({ page }) => {
  await page.goto("/");
  await openOwnerSurface(page, "Projects");
  await page.getByRole("button", { name: "Start chat" }).click();

  await page.locator('summary[aria-label="New chat model, effort, speed, and team"]').click();
  let dialog = page.getByRole("dialog", { name: "New chat model, effort, speed, and team" });
  await expect(dialog.getByText("Web search", { exact: true })).toHaveCount(0);
  await page.locator('summary[aria-label="New chat model, effort, speed, and team"]').click();

  await page.getByRole("button", { name: "Cycle interface complexity" }).click();
  await page.locator('summary[aria-label="New chat model, effort, speed, and team"]').click();
  dialog = page.getByRole("dialog", { name: "New chat model, effort, speed, and team" });
  await dialog.getByText("Web search", { exact: true }).click();
  await expect(dialog.getByRole("menuitemradio", { name: /Default/ })).toHaveAttribute("aria-checked", "true");
  await dialog.getByRole("menuitemradio", { name: /Live/ }).click();
  await page.locator('summary[aria-label="New chat model, effort, speed, and team"]').click();
  await expect(dialog.getByText("Live", { exact: true })).toBeVisible();
});

test("new chat resolves project mentions with keyboard selection and exclusive palettes", async ({ page }) => {
  await page.goto("/");
  await openOwnerSurface(page, "Projects");
  await page.getByRole("button", { name: "Start chat" }).click();

  const composer = page.getByRole("textbox", { name: "Message" });
  await composer.fill("/");
  await expect(page.getByRole("listbox", { name: "Slash commands" })).toBeVisible();
  await composer.fill("Review @ma");
  const mentions = page.getByRole("listbox", { name: "Mention files, folders, or plugins" });
  await expect(mentions).toBeVisible();
  await expect(page.getByRole("listbox", { name: "Slash commands" })).toBeHidden();
  await expect(mentions.getByRole("option", { name: /main\.ts/ })).toBeVisible();

  await composer.press("Enter");
  await expect(composer).toHaveValue("Review @main.ts ");
  await expect(mentions).toBeHidden();

  await composer.fill("email person@example.com");
  await expect(mentions).toBeHidden();

  await composer.fill("Use $cal");
  const apps = page.getByRole("listbox", { name: "Enabled apps" });
  await expect(apps.getByRole("option", { name: /Calendar/ })).toBeVisible();
  await composer.press("Tab");
  await expect(composer).toHaveValue("Use $calendar ");
  await expect(apps).toBeHidden();

  await composer.fill("/apps ");
  await expect(apps).toBeVisible();
  await page.locator(".desktop-titlebar-brand").click();
  await expect(apps).toBeHidden();
});

test("transient menus dismiss outside, on Escape, and when the app loses focus", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "New chat" })).toBeVisible();
  const outsideTarget = page.locator(".desktop-titlebar-brand");

  const permissionTrigger = page.getByLabel("Change permissions");
  const permissionDetails = permissionTrigger.locator("xpath=..");
  await permissionTrigger.click();
  await expect(permissionDetails).toHaveAttribute("open", "");
  await outsideTarget.click();
  await expect(permissionDetails).not.toHaveAttribute("open", "");

  const workspaceTrigger = page.getByLabel("Workspace: Local");
  const workspaceDetails = workspaceTrigger.locator("xpath=..");
  await workspaceTrigger.click();
  await expect(workspaceDetails).toHaveAttribute("open", "");
  await page.keyboard.press("Escape");
  await expect(workspaceDetails).not.toHaveAttribute("open", "");
  await expect(workspaceTrigger).toBeFocused();

  const ownerTrigger = page.getByRole("button", { name: "Open owner menu" });
  await ownerTrigger.click();
  await expect(page.locator(".owner-menu")).toBeVisible();
  await outsideTarget.click();
  await expect(page.locator(".owner-menu")).toBeHidden();

  await page.getByRole("button", { name: "Share" }).click();
  await expect(page.getByRole("menu", { name: "Share workspace" })).toBeVisible();
  await page.evaluate(() => window.dispatchEvent(new Event("blur")));
  await expect(page.getByRole("menu", { name: "Share workspace" })).toHaveCount(0);

  await page.getByRole("button", { name: "File", exact: true }).click();
  await expect(page.getByLabel("File menu")).toBeVisible();
  await outsideTarget.click();
  await expect(page.getByLabel("File menu")).toBeHidden();

  await page.getByRole("link", { name: "Pull requests", exact: true }).click();
  const reviewPanel = page.getByRole("region", { name: "Pull requests" });
  await reviewPanel.getByRole("button", { name: "Filter pull requests" }).click();
  await expect(reviewPanel.getByRole("menu", { name: "Pull request filters" })).toBeVisible();
  await reviewPanel.getByRole("searchbox", { name: "Search pull requests" }).click();
  await expect(reviewPanel.getByRole("menu", { name: "Pull request filters" })).toHaveCount(0);
});

test("simple and advanced interfaces meet the WCAG A and AA accessibility baseline", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "New chat" })).toBeVisible();

  const simple = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"])
    .analyze();
  expect(simple.violations, "simple-mode accessibility violations").toEqual([]);

  await enableAdvancedMode(page);
  await openOwnerSurface(page, "Projects");
  await expect(page.getByRole("heading", { name: "Runtime status" })).toBeVisible();

  const advanced = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"])
    .analyze();
  expect(advanced.violations, "advanced-mode accessibility violations").toEqual([]);
});

test("keyboard-only navigation reaches every workspace and exposes visible focus", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "New chat" })).toBeVisible();

  const navigation = [
    "Hide sidebar",
    "Back",
    "Forward",
    "File",
    "Edit",
    "View",
    "Help",
    "Minimize",
    "Maximize or restore",
    "Close window",
    "Search chats",
    "Open activity",
    "New chat",
    "Pull requests",
    "Projects",
    "Sites",
    "Scheduled",
    "Agents",
    "Plugins"
  ];
  for (const name of navigation) {
    await page.keyboard.press("Tab");
    const control = name === "Search chats"
      ? page.getByRole("button", { name })
      : page.getByRole(name === "New chat" ? "button" : name === "Pull requests" || name === "Projects" || name === "Sites" || name === "Scheduled" || name === "Agents" || name === "Plugins" ? "link" : "button", { name, exact: true });
    await expect(control).toBeFocused();
    await expectVisibleKeyboardFocus(control);
  }

  await page.keyboard.press("Tab");
  await expect(page.getByRole("button", { name: "consumer-app", exact: true })).toBeFocused();
  await page.keyboard.press("Tab");
  const projectChat = page.getByRole("button", { name: "New chat in consumer-app" });
  await expect(projectChat).toBeFocused();
  await expectVisibleKeyboardFocus(projectChat);
  const owner = page.getByRole("button", { name: "Open owner menu" });
  for (let index = 0; index < 8 && !(await owner.evaluate((element) => document.activeElement === element)); index += 1) {
    await page.keyboard.press("Tab");
  }
  await expect(owner).toBeFocused();
  await expectVisibleKeyboardFocus(owner);
  await page.keyboard.press("Enter");
  await expect(page.locator(".owner-menu")).toBeVisible();
});

test("desktop menus mirror Codex commands without ChatGPT product branding", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("button", { name: "File", exact: true }).click();
  let menu = page.getByLabel("File menu");
  await expect(menu.getByRole("button", { name: /New Chat.*Ctrl\+N/ })).toBeVisible();
  await expect(menu.getByRole("button", { name: /Open Folder.*Ctrl\+O/ })).toBeVisible();
  await expect(menu.getByRole("button", { name: /New Temporary Chat/ })).toBeDisabled();
  await expect(menu.getByRole("button", { name: /Quit MetaCog.*Ctrl\+Q/ })).toBeVisible();
  await expect(menu.getByText("Quit ChatGPT")).toHaveCount(0);

  await page.getByRole("button", { name: "Edit", exact: true }).click();
  menu = page.getByLabel("Edit menu");
  await expect(menu.getByRole("button", { name: /Undo.*Ctrl\+Z/ })).toBeVisible();
  await expect(menu.getByRole("button", { name: /Settings.*Ctrl\+,/ })).toBeVisible();

  await page.getByRole("button", { name: "View", exact: true }).click();
  menu = page.getByLabel("View menu");
  await expect(menu.getByRole("button", { name: /Toggle Sidebar.*Ctrl\+B/ })).toBeVisible();
  await expect(menu.getByRole("button", { name: "Open Terminal Ctrl+`" })).toBeEnabled();
  await expect(menu.getByRole("button", { name: /Toggle Full Screen.*F11/ })).toBeVisible();

  await page.getByRole("button", { name: "Help", exact: true }).click();
  menu = page.getByLabel("Help menu");
  await expect(menu.getByRole("button", { name: /Keyboard Shortcuts.*Ctrl\+\// })).toBeVisible();
  await expect(menu.getByRole("button", { name: "About MetaCog" })).toBeVisible();
  await expect(menu.getByText("About ChatGPT")).toHaveCount(0);

  await page.keyboard.press("Escape");
  await page.keyboard.press("Control+F");
  await expect(page.getByRole("textbox", { name: "Search chats" })).toBeFocused();
});

test("account menu and Settings follow the compact Codex information architecture", async ({ page }) => {
  await page.goto("/");
  const menu = await openOwnerMenu(page);
  await expect(menu.getByRole("button", { name: /^Usage/ })).toBeVisible();
  await expect(menu.getByRole("button", { name: /^Settings/ })).toBeVisible();
  await expect(menu.getByRole("button", { name: "Sign in with ChatGPT" })).toBeVisible();
  await expect(menu.getByText("Show pet")).toHaveCount(0);
  await expect(menu.getByText("Invite a friend")).toHaveCount(0);

  await page.keyboard.press("Escape");
  await page.keyboard.press("Control+,");
  await expect(page.getByRole("textbox", { name: "Search settings" })).toBeVisible();
  const settings = page.getByRole("navigation", { name: "Settings sections" });
  await expect(settings.getByRole("button", { name: "General" })).toBeVisible();
  await expect(settings.getByRole("button", { name: "Account" })).toBeVisible();
  await expect(settings.getByRole("button", { name: "Plugins" })).toBeVisible();
  await expect(settings.getByRole("button", { name: "Archived chats" })).toBeVisible();
  await expect(settings.getByText("Pets")).toHaveCount(0);

  await page.getByRole("textbox", { name: "Search settings" }).fill("account");
  await expect(settings.getByRole("button", { name: "Account" })).toBeVisible();
  await expect(settings.getByRole("button", { name: "General" })).toHaveCount(0);

  await page.getByRole("textbox", { name: "Search settings" }).fill("");
  await settings.getByRole("button", { name: "Appearance" }).click();
  await expect(page.getByRole("group", { name: "Theme" })).toBeVisible();
  await page.getByRole("button", { name: "Light" }).click();
  await expect.poll(() => page.evaluate(() => document.documentElement.dataset.theme)).toBe("light");
  const lightThemeContrast = await page.evaluate(() => {
    const root = getComputedStyle(document.documentElement);
    const parseRgb = (value: string) => {
      const trimmed = value.trim();
      const channels = trimmed.startsWith("#")
        ? (trimmed.length === 4
          ? [...trimmed.slice(1)].map((channel) => Number.parseInt(channel + channel, 16))
          : trimmed.slice(1).match(/../g)?.slice(0, 3).map((channel) => Number.parseInt(channel, 16)) ?? [])
        : trimmed.match(/[\d.]+/g)?.slice(0, 3).map(Number) ?? [];
      return channels.map((channel) => {
        const normalized = channel / 255;
        return normalized <= 0.04045
          ? normalized / 12.92
          : ((normalized + 0.055) / 1.055) ** 2.4;
      });
    };
    const ratio = (foreground: string, background: string) => {
      const [red, green, blue] = parseRgb(foreground);
      const [backgroundRed, backgroundGreen, backgroundBlue] = parseRgb(background);
      const foregroundLuminance = 0.2126 * red + 0.7152 * green + 0.0722 * blue;
      const backgroundLuminance = 0.2126 * backgroundRed + 0.7152 * backgroundGreen + 0.0722 * backgroundBlue;
      return (Math.max(foregroundLuminance, backgroundLuminance) + 0.05)
        / (Math.min(foregroundLuminance, backgroundLuminance) + 0.05);
    };
    const background = root.getPropertyValue("--mcg-surface").trim();
    return {
      primary: ratio(root.getPropertyValue("--mcg-text").trim(), background),
      muted: ratio(root.getPropertyValue("--mcg-muted").trim(), background),
      placeholder: ratio(root.getPropertyValue("--mcg-placeholder").trim(), background),
      settingsCopy: ratio(getComputedStyle(document.querySelector(".settings-card p")!).color, background),
    };
  });
  expect(lightThemeContrast.primary).toBeGreaterThanOrEqual(4.5);
  expect(lightThemeContrast.muted).toBeGreaterThanOrEqual(4.5);
  expect(lightThemeContrast.placeholder).toBeGreaterThanOrEqual(4.5);
  expect(lightThemeContrast.settingsCopy).toBeGreaterThanOrEqual(4.5);
  await page.getByRole("button", { name: "Dark" }).click();

  await settings.getByRole("button", { name: "Keyboard shortcuts" }).click();
  await page.getByRole("textbox", { name: "Search shortcuts" }).fill("global folder");
  const shortcutList = page.locator(".shortcut-list");
  await expect(shortcutList.getByText("Open global folder")).toBeVisible();
  await expect(shortcutList.getByText("Toggle full screen")).toHaveCount(0);

  await settings.getByRole("button", { name: "Usage & billing" }).click();
  await expect(page.getByText("MetaCog does not sell credits, collect subscription payments, or silently move subscription work to API billing.")).toBeVisible();

  await settings.getByRole("button", { name: "Browser" }).click();
  await expect(page.getByRole("heading", { name: "Browser", level: 2 })).toBeVisible();
  await expect(page.getByText("Site permissions", { exact: true })).toBeVisible();

  await settings.getByRole("button", { name: "Git" }).click();
  await expect(page.getByText("Branch prefix")).toBeVisible();
  await expect(page.getByText("codex/")).toBeVisible();
});

test("narrow layout keeps the mode switch reachable without page overflow", async ({ page }) => {
  await page.setViewportSize({ width: 720, height: 900 });
  await page.goto("/");

  const owner = page.getByRole("button", { name: "Open owner menu" });
  await expect(owner).toBeVisible();
  await owner.click();
  await openOwnerSurface(page, "Settings");
  const mode = page.getByRole("radio", { name: /Advanced/ });
  await expect(mode).toBeVisible();
  const box = await mode.boundingBox();
  expect(box).not.toBeNull();
  expect((box?.x ?? 0) + (box?.width ?? 0)).toBeLessThanOrEqual(720);
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(720);
});

test("the desktop shell prevents phantom document scrolling", async ({ page }) => {
  await page.goto("/");
  await openOwnerSurface(page, "Projects");

  const layout = await page.evaluate(() => {
    const workspace = document.querySelector<HTMLElement>(".workspace")!;
    const frame = document.querySelector<HTMLElement>(".desktop-frame")!;
    return {
      htmlOverflowY: getComputedStyle(document.documentElement).overflowY,
      bodyOverflowY: getComputedStyle(document.body).overflowY,
      workspaceOverflowY: getComputedStyle(workspace).overflowY,
      frameHeight: frame.getBoundingClientRect().height,
      viewportHeight: window.innerHeight
    };
  });
  expect(layout.htmlOverflowY).toBe("hidden");
  expect(layout.bodyOverflowY).toBe("hidden");
  expect(layout.workspaceOverflowY).toBe("auto");
  expect(Math.round(layout.frameHeight)).toBe(layout.viewportHeight);

  const agentRoom = await page.evaluate(() => {
    const shell = document.querySelector<HTMLElement>(".app-shell")!;
    const host = document.querySelector<HTMLElement>(".workspace")!;
    const previousSurface = shell.dataset.activeSurface;
    shell.dataset.activeSurface = "threads";

    const conversation = document.createElement("div");
    conversation.className = "chat-conversation";
    const outer = document.createElement("div");
    outer.className = "chat-message-list";
    outer.setAttribute("data-scroll-chain", "");
    Object.assign(outer.style, { height: "160px", overflowY: "auto" });
    const outerContent = document.createElement("div");
    outerContent.style.height = "420px";
    outer.appendChild(outerContent);
    conversation.appendChild(outer);

    const room = document.createElement("section");
    room.className = "agent-chat-room";
    room.setAttribute("data-scroll-chain", "");
    Object.assign(room.style, { height: "100px", maxHeight: "100px" });
    const transcript = document.createElement("div");
    transcript.className = "agent-chat-transcript chat-message-list";
    const transcriptContent = document.createElement("div");
    transcriptContent.style.height = "360px";
    transcript.appendChild(transcriptContent);
    room.appendChild(transcript);
    outerContent.appendChild(room);
    host.appendChild(conversation);

    const canceled = !transcriptContent.dispatchEvent(new WheelEvent("wheel", { bubbles: true, cancelable: true, deltaY: 40 }));
    const activity = document.createElement("div");
    activity.className = "agent-live-activity-scroll";
    activity.setAttribute("data-scroll-chain", "");
    activity.style.height = "60px";
    const activityContent = document.createElement("div");
    activityContent.style.height = "180px";
    activity.appendChild(activityContent);
    room.appendChild(activity);
    activityContent.dispatchEvent(new WheelEvent("wheel", { bubbles: true, cancelable: true, deltaY: 30 }));
    const result = {
      canceled,
      transcriptOverflowY: getComputedStyle(transcript).overflowY,
      room: room.scrollTop,
      activity: activity.scrollTop,
      outer: outer.scrollTop
    };
    conversation.remove();
    if (previousSurface) shell.dataset.activeSurface = previousSurface;
    else delete shell.dataset.activeSurface;
    return result;
  });
  expect(agentRoom.canceled).toBe(true);
  expect(agentRoom.transcriptOverflowY).toBe("visible");
  expect(agentRoom.room).toBe(40);
  expect(agentRoom.activity).toBe(30);
  expect(agentRoom.outer).toBe(0);

  const chained = await page.evaluate(() => {
    const host = document.querySelector<HTMLElement>(".workspace")!;
    const makeViewport = () => {
      const viewport = document.createElement("div");
      viewport.setAttribute("data-scroll-chain", "");
      Object.assign(viewport.style, { height: "100px", overflowY: "auto" });
      const content = document.createElement("div");
      content.style.height = "300px";
      viewport.appendChild(content);
      return { viewport, content };
    };
    const outer = makeViewport();
    const middle = makeViewport();
    const inner = makeViewport();
    outer.content.appendChild(middle.viewport);
    middle.content.appendChild(inner.viewport);
    host.appendChild(outer.viewport);
    const firstCanceled = !inner.viewport.dispatchEvent(new WheelEvent("wheel", { bubbles: true, cancelable: true, deltaY: 40 }));
    const first = [inner.viewport.scrollTop, middle.viewport.scrollTop, outer.viewport.scrollTop];
    inner.viewport.scrollTop = inner.viewport.scrollHeight;
    inner.viewport.dispatchEvent(new WheelEvent("wheel", { bubbles: true, cancelable: true, deltaY: 40 }));
    const second = [inner.viewport.scrollTop, middle.viewport.scrollTop, outer.viewport.scrollTop];
    outer.viewport.remove();
    return { firstCanceled, first, second };
  });
  expect(chained.firstCanceled).toBe(true);
  expect(chained.first).toEqual([40, 0, 0]);
  expect(chained.second).toEqual([200, 40, 0]);

  await page.locator(".workspace").evaluate((workspace) => { workspace.scrollTop = workspace.scrollHeight; });
  await page.locator(".workspace").hover();
  await page.mouse.wheel(0, 1_000);
  expect(await page.evaluate(() => window.scrollY)).toBe(0);
});

test("workspace navigation always opens a surface at its top", async ({ page }) => {
  await page.goto("/");
  await openOwnerSurface(page, "Projects");
  await expect(page.getByRole("heading", { name: "Choose a project", level: 2 })).toBeVisible();
  await expect(page.getByLabel("Project intelligence and memory")).toHaveCount(0);
  await page.getByRole("button", { name: "Start chat" }).click();
  await expect(page.getByRole("heading", { name: "New chat", level: 1 })).toBeVisible();
  await expect(page.getByRole("heading", { name: "What should we build in consumer-app?" })).toBeVisible();
  await expect(page.getByLabel("Choose project")).toContainText("consumer-app");
  await expect(page.locator(".selected-project")).toHaveCount(0);
  await expect(page.getByRole("region", { name: "Tasks" })).toBeHidden();

  await page.getByRole("button", { name: "New chat", exact: true }).click();
  await expect(page.getByRole("region", { name: "Tasks" })).toBeHidden();
  await expect.poll(() => page.evaluate(() => Math.round(window.scrollY))).toBe(0);

  await openOwnerSurface(page, "Projects");
  await expect(page.locator("#projects")).toBeVisible();
  await expect(page.getByRole("region", { name: "Tasks" })).toBeHidden();
  await expect.poll(() => page.evaluate(() => Math.round(window.scrollY))).toBe(0);
  await expect(page.getByRole("heading", { name: "Choose a project", level: 2 })).toBeVisible();
});

test("scaled 1280x800 Windows layout keeps browser OAuth reachable", async ({ page }) => {
  await page.setViewportSize({ width: 853, height: 533 });
  await page.goto("/");

  await openOwnerSurface(page, "Providers");

  const connect = page.getByRole("button", { name: "Continue with OpenAI" });
  await connect.scrollIntoViewIfNeeded();
  await expect(connect).toBeVisible();
  const box = await connect.boundingBox();
  expect(box).not.toBeNull();
  expect(box?.x ?? -1).toBeGreaterThanOrEqual(0);
  expect((box?.x ?? 0) + (box?.width ?? 0)).toBeLessThanOrEqual(853);
  expect((box?.y ?? 0) + (box?.height ?? 0)).toBeLessThanOrEqual(533);
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(853);
});

test("plugin permission review gates enablement and invocation", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("link", { name: "Plugins", exact: true }).click();

  const plugins = page.getByRole("region", { name: "Plugins" });
  await expect(plugins.getByRole("heading", { name: "Plugin directory" })).toBeVisible();
  await expect(plugins.getByRole("heading", { name: "Weather helper" })).toBeVisible();
  await expect(plugins.getByLabel("Plugin input (JSON)")).toHaveCount(0);
  await enableAdvancedMode(page);
  await page.getByRole("link", { name: "Plugins", exact: true }).click();
  await plugins.locator("summary").filter({ hasText: "Plugin administration" }).click();
  await expect(plugins.getByRole("heading", { name: "Plugin administration" })).toBeVisible();
  await expect(plugins.getByRole("button", { name: "Enable" })).toBeDisabled();
  await plugins.getByLabel("Weather helper network origin").fill("https://weather.example");
  await plugins.getByRole("button", { name: "Grant requested access" }).click();
  await expect(plugins.getByRole("button", { name: "Enable" })).toBeEnabled();
  await plugins.getByRole("button", { name: "Enable" }).click();
  await expect(plugins.getByText("Enabled", { exact: true })).toBeVisible();
  await plugins.getByLabel("Plugin input (JSON)").fill('{"city":"Chicago"}');
  await plugins.getByRole("button", { name: "Run plugin" }).click();
  await expect(plugins.getByText('"forecast": "sunny"')).toBeVisible();
});

test("consumer composes and runs a revision-pinned multi-host workflow", async ({ page }) => {
  await page.goto("/");
  await openOwnerSurface(page, "SSH");
  const workflows = page.getByRole("region", { name: "SSH workflows" });
  await workflows.getByLabel("Workflow name").fill("Build and test remotely");
  await workflows.getByRole("checkbox", { name: /Linux builder/ }).check();
  await workflows.getByRole("checkbox", { name: /Linux tester/ }).check();
  await workflows.getByRole("button", { name: "Create durable workflow" }).click();
  await expect(workflows.getByRole("heading", { name: "Build and test remotely" })).toBeVisible();
  await expect(workflows.getByText("2 dependency-ordered steps")).toBeVisible();
  await workflows.getByRole("button", { name: "Run workflow" }).click();
  await expect(workflows.getByText("succeeded", { exact: true }).first()).toBeVisible();
  await expect(workflows.getByText(/remote inspection completed/).first()).toBeVisible();
});

test("consumer manages provenance-bearing project memory and inspects the world model", async ({ page }) => {
  test.slow();
  await page.goto("/");
  await enableAdvancedMode(page);
  await openOwnerSurface(page, "Projects");
  await page.getByRole("button", { name: "Use project" }).click();
  const intelligence = page.getByLabel("Project intelligence and memory");
  await expect(intelligence.getByRole("list", { name: "Project topology plan" })).toContainText("Stage 1");
  await expect(intelligence.getByRole("list", { name: "Project topology plan" })).toContainText("main database");
  await expect(intelligence.getByRole("list", { name: "Project topology plan" })).toContainText("compose.yaml");
  await intelligence.getByLabel("Memory fact name").fill("package_manager");
  await intelligence.getByLabel("Memory value").fill("pnpm");
  await intelligence.getByRole("button", { name: "Remember this" }).click();
  await expect(intelligence.getByText("package_manager", { exact: true })).toBeVisible();
  await expect(intelligence.getByText("pnpm", { exact: true })).toBeVisible();

  await expect(intelligence.getByRole("list", { name: "Project topology plan" })).toContainText("database · compose.yaml");
  await expect(intelligence.getByLabel("Memory namespace")).toHaveValue("project");
  await expect(intelligence.getByLabel("Memory fresh until")).toBeVisible();
  await intelligence.getByText("World model and managed MEMORY.md").click();
  await expect(intelligence.getByRole("list", { name: "Project world model" })).toContainText("service web");
  await expect(intelligence.getByRole("list", { name: "Project world model" })).toContainText("repository → provides → web");
  await intelligence.getByRole("button", { name: "Reinforce" }).click();
  await expect(intelligence.getByText("working", { exact: true })).toBeVisible();
  await intelligence.getByRole("button", { name: "History" }).click();
  await expect(intelligence.locator(".memory-history")).toContainText("working");
  await expect(intelligence.locator(".managed-memory")).toContainText("package_manager: pnpm");
});

test("consumer reviews, approves, and runs an exact topology command", async ({ page }) => {
  test.slow();
  await page.goto("/");
  await enableAdvancedMode(page);
  await openOwnerSurface(page, "Projects");
  await page.getByRole("button", { name: "Use project" }).click();
  const intelligence = page.getByLabel("Project intelligence and memory");
  await intelligence.getByLabel("Reviewed topology reason").fill("Run the reviewed project tests");
  await intelligence.getByRole("button", { name: "Review this plan" }).click();
  await expect(intelligence.getByLabel("Reviewed topology plans")).toContainText("Run the reviewed project tests");
  await intelligence.getByRole("button", { name: "Run reviewed plan" }).click();
  await expect(intelligence.getByRole("button", { name: "Approve and run" })).toBeVisible();
  await intelligence.getByRole("button", { name: "Approve and run" }).click();
  await expect(intelligence.getByText("succeeded", { exact: true })).toBeVisible();
  await expect(intelligence.getByText(/process completed/)).toBeVisible();
});

test("consumer duplicates and inspects a portable organization without exposing local authority", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await openOwnerSurface(page, "Improvement");
  const improvement = page.getByRole("region", { name: "Measured improvement" });
  const organizations = improvement.getByLabel("Agent organizations");
  await expect(organizations.locator("strong", { hasText: "Consumer team" })).toBeVisible();
  await organizations.getByRole("button", { name: "Duplicate" }).click();
  await expect(organizations.locator("strong", { hasText: "Consumer team copy" })).toBeVisible();
  await expectOrganizationCardsToFit(organizations);

  await enableAdvancedMode(page);
  await openOwnerSurface(page, "Improvement");
  await expectOrganizationCardsToFit(organizations);
  await organizations.getByRole("button", { name: "Show portable JSON" }).first().click();
  await expect(organizations.getByLabel("Portable organization JSON")).toContainText("cccccccc");
  await organizations.getByRole("button", { name: "Revision history" }).first().click();
  await expect(organizations.getByRole("list", { name: "Consumer team revision history" })).toContainText("Revision 1");
  await organizations.getByRole("button", { name: "Compare portable definitions" }).click();
  await expect(organizations.getByText("Definitions are equivalent.")).toBeVisible();
  await expect(organizations.getByText(/Retry identity:/)).toBeVisible();
});

test("consumer verifies a signed organization before a separate reasoned import", async ({ page }) => {
  await page.goto("/");
  await openOwnerSurface(page, "Improvement");
  const sharing = page.getByLabel("Signed organization sharing");
  await sharing.getByLabel("Signed organization package").fill(JSON.stringify({
    schema_version: 1,
    registry: "https://organizations.example.com",
    package: "consumer-team",
    publisher: "example-publisher",
    portable: {
      schema_version: 1,
      source: { organization_id: "00000000-0000-4000-8000-000000000251", revision_id: "00000000-0000-4000-8000-000000000252", revision: 1, exported_at: "2026-08-18T00:00:00Z" },
      definition: { name: "Consumer team", roles: [], relationships: [], policy: { prefer_minimal_organization: true, max_parallel_agents: 4, max_total_agents: 8, max_organizational_overhead_ratio: 0.3, default_lease_seconds: 900 }, evaluation: { require_independent_evaluation: false, require_objective_evidence: true, minimum_confidence: 0.8, calibration_window: 100 } },
      content_sha256: "c".repeat(64)
    },
    signature: { scheme: "ed25519-v1", signature_base64: "s".repeat(88) }
  }));
  await sharing.getByRole("button", { name: "Verify for review" }).click();
  await expect(sharing.getByText("Publisher key digest")).toBeVisible();
  await expect(sharing.getByText("Verified package digest")).toBeVisible();
  await expect(sharing.getByRole("button", { name: "Import verified organization" })).toBeDisabled();
  await sharing.getByLabel("Owner decision reason").fill("Reviewed for this local project");
  await sharing.getByRole("button", { name: "Import verified organization" }).click();
  await expect(sharing.getByText(/Reviewed for this local project/)).toBeVisible();
});

async function expectOrganizationCardsToFit(organizations: import("@playwright/test").Locator) {
  const layouts = await organizations.locator(".organization-card").evaluateAll((cards) => cards.map((card) => {
    const summary = card.querySelector<HTMLElement>(".organization-summary");
    return {
      clientWidth: (card as HTMLElement).clientWidth,
      scrollWidth: (card as HTMLElement).scrollWidth,
      summaryWidth: summary?.getBoundingClientRect().width ?? 0
    };
  }));
  expect(layouts.length).toBeGreaterThan(0);
  for (const layout of layouts) {
    expect(layout.scrollWidth).toBeLessThanOrEqual(layout.clientWidth + 1);
    expect(layout.summaryWidth).toBeGreaterThan(180);
  }
}

async function expectVisibleKeyboardFocus(locator: import("@playwright/test").Locator) {
  const outline = await locator.evaluate((element) => {
    const style = getComputedStyle(element);
    return {
      style: style.outlineStyle,
      width: Number.parseFloat(style.outlineWidth),
      color: style.outlineColor
    };
  });
  expect(outline.style).not.toBe("none");
  expect(outline.width).toBeGreaterThanOrEqual(3);
  expect(outline.color).not.toBe("transparent");
}
