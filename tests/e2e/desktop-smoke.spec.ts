import { expect, test } from "@playwright/test";

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
    const history = [{ settings: initial, reason: "initial settings", changed_at: initial.updated_at }];
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      value: {
        invoke: async (command: string, args: Record<string, any> = {}) => {
          if (command === "get_user_settings") return settings;
          if (command === "user_settings_history") return { revisions: [...history] };
          if (command === "list_backups") return { backups: [] };
          if (command === "list_plugins") return { plugins: [plugin] };
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

test("consumer shell starts simple and can reveal advanced status", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Your AI workspace" })).toBeVisible();
  await expect(page.getByText("System details are hidden in simple mode.")).toBeVisible();

  await page.getByRole("button", { name: "Advanced mode" }).click();

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

  await page.getByRole("button", { name: "Simple mode" }).click();
  await expect(scheduler.getByLabel("Repeat")).toHaveValue("3600");
});

test("narrow layout keeps the mode switch reachable without page overflow", async ({ page }) => {
  await page.setViewportSize({ width: 720, height: 900 });
  await page.goto("/");

  const mode = page.getByRole("button", { name: "Advanced mode" });
  await expect(mode).toBeVisible();
  const box = await mode.boundingBox();
  expect(box).not.toBeNull();
  expect((box?.x ?? 0) + (box?.width ?? 0)).toBeLessThanOrEqual(720);
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(720);
});

test("scaled 1280x800 Windows layout keeps browser OAuth reachable", async ({ page }) => {
  await page.setViewportSize({ width: 853, height: 533 });
  await page.goto("/");

  const providerLink = page.getByRole("link", { name: "Providers" });
  await expect(providerLink).toBeVisible();
  await providerLink.click();

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
