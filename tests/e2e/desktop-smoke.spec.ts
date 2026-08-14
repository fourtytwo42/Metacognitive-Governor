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
});
