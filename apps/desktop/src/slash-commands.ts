export const SLASH_COMMANDS = [
  { name: "approve", description: "Approve one recent automatic-review denial", availability: "conditional" },
  { name: "apps", description: "Choose an enabled app and insert its $ reference", availability: "always" },
  { name: "cloud", description: "Run the chat in the cloud", availability: "conditional" },
  { name: "cloud-environment", description: "Choose the cloud environment for the chat", availability: "conditional" },
  { name: "compact", description: "Compact the current chat context", availability: "chat" },
  { name: "fast", description: "Turn the Fast service tier on or off", availability: "always" },
  { name: "feedback", description: "Open feedback and optional diagnostic details", availability: "always" },
  { name: "fork", description: "Fork the current local chat context", availability: "chat" },
  { name: "goal", description: "Set a persistent goal to keep pursuing", availability: "always" },
  { name: "ide-context", description: "Turn shared IDE context on or off", availability: "conditional" },
  { name: "init", description: "Generate an AGENTS.md scaffold for the project", availability: "project" },
  { name: "local", description: "Run the chat in the selected local project", availability: "always" },
  { name: "mcp", description: "Open MCP status and connected servers", availability: "always" },
  { name: "memories", description: "Configure chat memories", availability: "conditional" },
  { name: "model", description: "Choose the model for the current chat", availability: "always" },
  { name: "pet", description: "Wake or tuck away the desktop pet", availability: "conditional" },
  { name: "personality", description: "Choose how the chat agent responds", availability: "always" },
  { name: "plan", description: "Toggle plan mode for multi-step planning", availability: "always" },
  { name: "project", description: "Choose a project for new chats", availability: "always" },
  { name: "reasoning", description: "Choose the reasoning effort for the chat", availability: "always" },
  { name: "review", description: "Review uncommitted changes or compare a branch", availability: "project" },
  { name: "side", description: "Start a side chat without interrupting this one", availability: "always" },
  { name: "status", description: "Show chat identity, context, and rate limits", availability: "always" },
  { name: "task", description: "Start a chat without a project", availability: "always" },
  { name: "worktree", description: "Run new coding work in a managed Git worktree", availability: "project" }
] as const;

export type SlashCommandName = (typeof SLASH_COMMANDS)[number]["name"];
export type SlashCommand = (typeof SLASH_COMMANDS)[number];

export interface ParsedSlashCommand {
  name: SlashCommandName;
  argument: string;
}

export function slashCommandMatches(value: string): SlashCommand[] {
  const match = value.match(/^\/([^\s]*)$/);
  if (!match) return [];
  const query = match[1].toLocaleLowerCase();
  return SLASH_COMMANDS.filter((command) => command.name.startsWith(query));
}

export function parseSlashCommand(value: string): ParsedSlashCommand | null {
  const match = value.trim().match(/^\/([^\s]+)(?:\s+([\s\S]*))?$/);
  if (!match) return null;
  const command = SLASH_COMMANDS.find((candidate) => candidate.name === match[1].toLocaleLowerCase());
  return command ? { name: command.name, argument: match[2]?.trim() ?? "" } : null;
}
