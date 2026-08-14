import { expect, test } from "@playwright/test";

test("consumer shell starts simple and can reveal advanced status", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Your AI workspace" })).toBeVisible();
  await expect(page.getByText("System details are hidden in simple mode.")).toBeVisible();

  await page.getByRole("button", { name: "Advanced mode" }).click();

  await expect(page.getByRole("heading", { name: "Runtime status" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "MCP servers" })).toBeVisible();
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
