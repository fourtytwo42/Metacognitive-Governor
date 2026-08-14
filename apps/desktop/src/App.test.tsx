import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

const client = vi.hoisted(() => ({
  start: vi.fn(),
  complete: vi.fn(),
  cancel: vi.fn(),
  open: vi.fn(),
  listMcp: vi.fn(),
  configureMcp: vi.fn(),
  discoverMcp: vi.fn()
}));

vi.mock("./desktop-client", () => ({
  startOpenAiOAuth: client.start,
  completeOpenAiOAuth: client.complete,
  cancelOpenAiOAuth: client.cancel,
  openExternalUrl: client.open,
  listMcpConnections: client.listMcp,
  configureMcpConnection: client.configureMcp,
  discoverMcpConnection: client.discoverMcp
}));

import { App } from "./App";

beforeEach(() => {
  client.start.mockReset();
  client.complete.mockReset();
  client.cancel.mockReset();
  client.open.mockReset();
  client.listMcp.mockReset();
  client.configureMcp.mockReset();
  client.discoverMcp.mockReset();
  client.open.mockResolvedValue(undefined);
  client.listMcp.mockResolvedValue({ connections: [] });
});

test("advanced mode reveals runtime details", () => {
  render(<App />);

  expect(screen.queryByRole("heading", { name: "Runtime status" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Advanced mode" }));
  expect(screen.getByRole("heading", { name: "Runtime status" })).toBeInTheDocument();
});

test("OpenAI subscription login exposes verification then confirms connection", async () => {
  client.start.mockResolvedValue({
    session_id: "fixture-session",
    connection_id: "openai-subscription",
    auth_mode: "chat_gpt_device_code",
    verification_url: "https://auth.openai.com/codex/device",
    user_code: "ABCD-1234"
  });
  client.complete.mockResolvedValue({
    connection: {
      id: "openai-subscription",
      display_name: "OpenAI Subscription",
      auth_mode: "chat_gpt_device_code"
    },
    catalog: { models: [] }
  });
  render(<App />);

  fireEvent.click(screen.getByRole("button", { name: "Connect with OpenAI" }));
  expect(await screen.findByText("Code:", { exact: false })).toHaveTextContent("ABCD-1234");
  expect(screen.getByRole("button", { name: "Open verification page" })).toBeInTheDocument();
  expect(client.start).toHaveBeenCalledWith("browser");
  expect(client.open).toHaveBeenCalledWith("https://auth.openai.com/codex/device");

  fireEvent.click(screen.getByRole("button", { name: "I’ve authorized MCG" }));
  expect(await screen.findByText("OpenAI Subscription connected")).toBeInTheDocument();
  expect(client.complete).toHaveBeenCalledWith("fixture-session");
});

test("simple MCP setup discovers a server without exposing advanced credentials", async () => {
  client.configureMcp.mockResolvedValue({
    connection: {
      contract_version: 1,
      id: "my_tools",
      display_name: "My Tools",
      transport: { kind: "streamable_http", url: "https://tools.example.com/mcp" },
      auth_mode: "none",
      protocol_mode: "auto",
      enabled: true
    },
    catalog: {
      connection_id: "my_tools",
      protocol_version: "2026-07-28",
      server_info: { name: "fixture" },
      tools: [{ name: "lookup" }],
      discovered_at: "2026-08-14T00:00:00Z",
      expires_at: "2026-08-14T00:05:00Z"
    }
  });
  render(<App />);

  expect(screen.queryByLabelText("Bearer token")).not.toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Server name"), { target: { value: "My Tools" } });
  fireEvent.change(screen.getByLabelText("Server URL"), {
    target: { value: "https://tools.example.com/mcp" }
  });
  fireEvent.click(screen.getByRole("button", { name: "Add server" }));

  expect(await screen.findByRole("heading", { name: "My Tools" })).toBeInTheDocument();
  expect(screen.getByText("1 tools · 2026-07-28")).toBeInTheDocument();
  expect(client.configureMcp).toHaveBeenCalledWith(
    expect.objectContaining({
      id: "my_tools",
      auth_mode: "none",
      protocol_mode: "auto"
    }),
    undefined
  );
});

test("advanced MCP setup passes stdio executable arguments without a credential", async () => {
  client.configureMcp.mockResolvedValue({
    connection: {
      contract_version: 1,
      id: "local_tools",
      display_name: "Local Tools",
      transport: { kind: "stdio", command: "npx", args: ["-y", "fixture-server"] },
      auth_mode: "none",
      protocol_mode: "auto",
      enabled: true
    },
    catalog: {
      connection_id: "local_tools",
      protocol_version: "2026-07-28",
      server_info: { name: "fixture" },
      tools: [],
      discovered_at: "2026-08-14T00:00:00Z",
      expires_at: "2026-08-14T00:05:00Z"
    }
  });
  render(<App />);

  fireEvent.click(screen.getByRole("button", { name: "Advanced mode" }));
  fireEvent.change(screen.getByLabelText("Transport"), { target: { value: "stdio" } });
  expect(screen.queryByLabelText("Server URL")).not.toBeInTheDocument();
  expect(screen.queryByLabelText("Authentication")).not.toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Server name"), { target: { value: "Local Tools" } });
  fireEvent.change(screen.getByLabelText("Executable"), { target: { value: "npx" } });
  fireEvent.change(screen.getByLabelText("Arguments (one per line)"), {
    target: { value: "-y\nfixture-server" }
  });
  fireEvent.click(screen.getByRole("button", { name: "Add server" }));

  expect(await screen.findByRole("heading", { name: "Local Tools" })).toBeInTheDocument();
  expect(client.configureMcp).toHaveBeenCalledWith(
    expect.objectContaining({
      transport: { kind: "stdio", command: "npx", args: ["-y", "fixture-server"] },
      auth_mode: "none"
    }),
    undefined
  );
});
