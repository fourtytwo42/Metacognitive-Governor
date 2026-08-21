import { expect, test } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const initial = {
      schema_version: 1,
      revision: 1,
      advanced_mode: false,
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
    const history = [{ settings: initial, reason: "initial settings", changed_at: initial.updated_at }];
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      value: {
        invoke: async (command: string, args: Record<string, any> = {}) => {
          if (command === "get_user_settings") return settings;
          if (command === "update_configuration") return { schema_version: 1, configured: false, current_version: "0.1.0", detail: "This build has no trusted update endpoint." };
          if (command === "user_settings_history") return { revisions: [...history] };
          if (command === "list_backups") return { backups: [] };
          if (command === "list_plugins") return { plugins: [plugin] };
          if (command === "list_ssh_hosts") return { hosts: sshHosts };
          if (command === "list_ssh_workflows") return { workflows: sshWorkflows };
          if (command === "list_projects") return { projects: [project] };
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
              schema_version: 1,
              revision: settings.revision + 1,
              advanced_mode: args.request.advanced_mode,
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
    await page.getByRole("button", { name: /Local owner/ }).click();
  }
  await expect(menu).toBeVisible();
  return menu;
}

async function openOwnerSurface(page: import("@playwright/test").Page, name: string) {
  const menu = await openOwnerMenu(page);
  await menu.getByRole("button", { name, exact: true }).click();
}

async function enableAdvancedMode(page: import("@playwright/test").Page) {
  const menu = await openOwnerMenu(page);
  await menu.getByRole("button", { name: "Advanced mode" }).click();
}

test("consumer shell starts simple and can reveal advanced status", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "New chat" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "What would you like to build?" })).toBeVisible();
  await expect(page.locator("#projects")).toBeHidden();
  await page.getByRole("link", { name: "Sites", exact: true }).click();
  const simpleSites = page.getByRole("region", { name: "Sites" });
  await expect(simpleSites.getByRole("heading", { name: "Sites" })).toBeVisible();
  await expect(simpleSites.getByRole("button", { name: "New site" })).toBeVisible();
  await expect(simpleSites.getByText("No sites yet")).toBeVisible();
  await expect(simpleSites.getByLabel("Public URL")).toHaveCount(0);
  await simpleSites.getByRole("button", { name: "New site" }).click();
  await expect(page.getByRole("textbox", { name: "Goal", exact: true })).toHaveValue(/Build a website/);
  await openOwnerSurface(page, "Settings");
  const globalFolders = page.getByLabel("Global folders");
  await expect(globalFolders.getByRole("button", { name: "Add global folder" })).toBeVisible();
  await expect(globalFolders).toContainText("does not create a project");
  const updates = page.getByLabel("Application updates");
  await expect(updates.getByText(/Updates are unavailable in this build/)).toBeVisible();
  await expect(updates.getByText(/never accept an unsigned fallback/)).toBeVisible();

  await enableAdvancedMode(page);

  await expect(page.getByRole("heading", { name: "Runtime status" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "MCP servers" })).toBeVisible();
  const settings = page.getByLabel("Owner settings");
  await expect(settings.getByLabel("Autonomy")).toHaveValue("autonomous");
  await expect(settings.getByLabel("Run opt-in shadow model experiments")).toBeChecked();
  await expect(settings.getByLabel("Settings revision history")).toContainText("Enabled advanced interface");
  const sites = page.getByRole("region", { name: "Sites" });
  await expect(sites.getByRole("heading", { name: "Web properties" })).toBeVisible();
  await expect(sites.getByLabel("Deployment executable")).toBeVisible();
  await expect(sites.getByText(/Retry identity:/)).toBeVisible();
  await expect(page.getByLabel("Connection id")).toBeVisible();
  await expect(page.getByLabel("Protocol")).toHaveValue("auto");
  const scheduler = page.getByRole("region", { name: "Scheduler" });
  await expect(scheduler.getByLabel("Trigger")).toBeVisible();
  await scheduler.getByLabel("Trigger").selectOption("cron");
  await expect(scheduler.getByLabel("Five-field cron")).toHaveValue("0 2 * * *");

  await page.getByRole("button", { name: "Add API or local provider" }).click();
  await expect(page.getByLabel("Provider type")).toHaveValue("open_ai_compatible");
  await expect(page.getByLabel("Connection id")).toHaveCount(2);

  const ownerMenu = await openOwnerMenu(page);
  await ownerMenu.getByRole("button", { name: "Simple mode" }).click();
  await page.getByRole("link", { name: "Scheduled", exact: true }).click();
  await expect(scheduler.getByLabel("Repeat")).toHaveValue("3600");
});

test("simple and advanced interfaces meet the WCAG A and AA accessibility baseline", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "New chat" })).toBeVisible();

  const simple = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"])
    .analyze();
  expect(simple.violations, "simple-mode accessibility violations").toEqual([]);

  await enableAdvancedMode(page);
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
    "Search chats",
    "New chat",
    "Pull requests",
    "Sites",
    "Scheduled",
    "Plugins"
  ];
  for (const name of navigation) {
    await page.keyboard.press("Tab");
    const control = name === "Search chats"
      ? page.getByRole("textbox", { name })
      : page.getByRole(name === "New chat" ? "button" : name === "Pull requests" || name === "Sites" || name === "Scheduled" || name === "Plugins" ? "link" : "button", { name, exact: true });
    await expect(control).toBeFocused();
    await expectVisibleKeyboardFocus(control);
  }

  await page.keyboard.press("Tab");
  await expect(page.getByRole("button", { name: "consumer-app", exact: true })).toBeFocused();
  await page.keyboard.press("Tab");
  const projectChat = page.getByRole("button", { name: "New chat in consumer-app" });
  await expect(projectChat).toBeFocused();
  await expectVisibleKeyboardFocus(projectChat);
  await page.keyboard.press("Tab");
  const owner = page.getByRole("button", { name: /Local owner/ });
  await expect(owner).toBeFocused();
  await expectVisibleKeyboardFocus(owner);
  await page.keyboard.press("Enter");
  await expect(page.locator(".owner-menu")).toBeVisible();
});

test("narrow layout keeps the mode switch reachable without page overflow", async ({ page }) => {
  await page.setViewportSize({ width: 720, height: 900 });
  await page.goto("/");

  const owner = page.getByRole("button", { name: /Local owner/ });
  await expect(owner).toBeVisible();
  await owner.click();
  const mode = page.getByRole("button", { name: "Advanced mode" });
  await expect(mode).toBeVisible();
  const box = await mode.boundingBox();
  expect(box).not.toBeNull();
  expect((box?.x ?? 0) + (box?.width ?? 0)).toBeLessThanOrEqual(720);
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(720);
});

test("workspace navigation always opens a surface at its top", async ({ page }) => {
  await page.goto("/");
  await openOwnerSurface(page, "Projects");
  await expect(page.getByRole("heading", { name: "Choose a project", level: 2 })).toBeVisible();
  await expect(page.getByLabel("Project intelligence and memory")).toHaveCount(0);
  await page.getByRole("button", { name: "Start chat" }).click();
  await expect(page.getByRole("heading", { name: "New chat", level: 1 })).toBeVisible();
  await expect(page.getByLabel("Project", { exact: true })).toHaveValue("00000000-0000-4000-8000-000000000242");
  await expect(page.locator(".selected-project")).toHaveCount(0);
  await expect(page.getByRole("region", { name: "Tasks" })).toBeHidden();
  const folderButton = page.getByRole("button", { name: "Open folder" });
  await expect(folderButton).toHaveAttribute("title", "Create a project from a folder");
  await expect(folderButton).not.toContainText("Open folder");

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
  await expect(plugins.getByRole("heading", { name: "Weather helper" })).toBeVisible();
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
