import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

const client = vi.hoisted(() => ({
  start: vi.fn(),
  complete: vi.fn(),
  cancel: vi.fn(),
  open: vi.fn()
}));

vi.mock("./desktop-client", () => ({
  startOpenAiOAuth: client.start,
  completeOpenAiOAuth: client.complete,
  cancelOpenAiOAuth: client.cancel,
  openExternalUrl: client.open
}));

import { App } from "./App";

beforeEach(() => {
  client.start.mockReset();
  client.complete.mockReset();
  client.cancel.mockReset();
  client.open.mockReset();
  client.open.mockResolvedValue(undefined);
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
