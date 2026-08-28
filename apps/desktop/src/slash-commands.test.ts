import { describe, expect, it } from "vitest";
import { parseSlashCommand, slashCommandMatches, SLASH_COMMANDS } from "./slash-commands";

describe("Codex-aligned slash command registry", () => {
  it("contains the complete official command inventory without duplicates", () => {
    expect(SLASH_COMMANDS).toHaveLength(25);
    expect(new Set(SLASH_COMMANDS.map((command) => command.name)).size).toBe(25);
    expect(SLASH_COMMANDS.map((command) => command.name)).toEqual([
      "approve", "apps", "cloud", "cloud-environment", "compact", "fast", "feedback", "fork",
      "goal", "ide-context", "init", "local", "mcp", "memories", "model", "pet",
      "personality", "plan", "project", "reasoning", "review", "side", "status", "task",
      "worktree"
    ]);
  });

  it("filters only while the first composer token is being entered", () => {
    expect(slashCommandMatches("/")).toHaveLength(25);
    expect(slashCommandMatches("/cl").map((command) => command.name)).toEqual(["cloud", "cloud-environment"]);
    expect(slashCommandMatches("please /cl")).toEqual([]);
    expect(slashCommandMatches("/cloud now")).toEqual([]);
  });

  it("parses exact commands and preserves their argument", () => {
    expect(parseSlashCommand("/goal ship the release")).toEqual({ name: "goal", argument: "ship the release" });
    expect(parseSlashCommand(" /STATUS ")).toEqual({ name: "status", argument: "" });
    expect(parseSlashCommand("/unknown")).toBeNull();
  });
});
