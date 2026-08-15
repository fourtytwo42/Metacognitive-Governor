import assert from "node:assert/strict";
import process from "node:process";

import {
  Client,
  StreamableHTTPClientTransport,
} from "@modelcontextprotocol/client";

const [endpoint, projectId] = process.argv.slice(2);
const token = process.env.MCG_SDK_TOKEN;

assert(endpoint, "MCP endpoint argument is required");
assert(projectId, "project id argument is required");
assert(token, "MCG_SDK_TOKEN is required");

const client = new Client(
  { name: "mcg-typescript-sdk-contract", version: "1.0.0" },
  { versionNegotiation: { mode: "auto" } },
);
const transport = new StreamableHTTPClientTransport(new URL(endpoint), {
  authProvider: { token: async () => token },
});

try {
  await client.connect(transport);
  assert.equal(client.getProtocolEra(), "modern");
  assert.equal(client.getNegotiatedProtocolVersion(), "2026-07-28");
  assert.equal(client.getServerVersion()?.name, "metacognitive-governor");
  assert(client.getServerCapabilities()?.tools);
  assert(client.getDiscoverResult()?.supportedVersions.includes("2026-07-28"));

  const listed = await client.listTools();
  const toolNames = listed.tools.map((tool) => tool.name);
  assert(toolNames.includes("mcg_system_status"));
  assert(toolNames.includes("mcg_run_task"));
  assert.equal(toolNames[0], "mcg_memory_search");
  assert.equal(toolNames.at(-1), "mcg_system_status");

  const status = await client.callTool({
    name: "mcg_system_status",
    arguments: {},
  });
  assert.equal(status.isError, false);
  assert.equal(status.structuredContent?.state, "ready");

  const progress = [];
  const taskArguments = {
    request_key: "typescript-sdk-progress-1",
    project_id: projectId,
    goal: "Verify the SDK fixture readme",
    success_conditions: ["FILE EXISTS README.md"],
    context_token_budget: 512,
    estimated_output_tokens: 128,
    max_model_calls: 2,
  };
  const completed = await client.callTool(
    { name: "mcg_run_task", arguments: taskArguments },
    {
      onprogress: (update) => progress.push(update),
      timeout: 10_000,
      resetTimeoutOnProgress: true,
      maxTotalTimeout: 20_000,
    },
  );
  assert.equal(completed.isError, false);
  assert.equal(completed.structuredContent?.task?.state, "completed");
  assert(progress.length >= 2);
  assert(progress.every((update) => Number.isFinite(update.progress)));
  assert(
    progress.every(
      (update, index) =>
        index === 0 || progress[index - 1].progress < update.progress,
    ),
  );

  const replay = await client.callTool({
    name: "mcg_run_task",
    arguments: taskArguments,
  });
  assert.equal(replay.structuredContent?.replayed, true);
  assert.equal(
    replay.structuredContent?.task?.id,
    completed.structuredContent?.task?.id,
  );

  const cancellation = new AbortController();
  const cancelledArguments = {
    ...taskArguments,
    request_key: "typescript-sdk-cancel-1",
    goal: "Cancel this SDK task by closing its request stream",
  };
  let cancellationRejected = false;
  try {
    await client.callTool(
      { name: "mcg_run_task", arguments: cancelledArguments },
      {
        signal: cancellation.signal,
        onprogress: (update) => {
          if (update.progress > 0) cancellation.abort("SDK cancellation fixture");
        },
        timeout: 10_000,
      },
    );
  } catch {
    cancellationRejected = true;
  }
  assert.equal(cancellation.signal.aborted, true);
  assert.equal(cancellationRejected, true);
  await new Promise((resolve) => setTimeout(resolve, 100));
  const cancelled = await client.callTool({
    name: "mcg_run_task",
    arguments: cancelledArguments,
  });
  assert.equal(cancelled.structuredContent?.replayed, true);
  assert.equal(cancelled.structuredContent?.task?.state, "cancelled");
  assert.equal(cancelled.structuredContent?.run?.run?.state, "cancelled");

  const deniedClient = new Client(
    { name: "mcg-typescript-sdk-denied", version: "1.0.0" },
    { versionNegotiation: { mode: "auto" } },
  );
  const deniedTransport = new StreamableHTTPClientTransport(new URL(endpoint), {
    authProvider: { token: async () => "intentionally-invalid-fixture-token" },
  });
  let authenticationRejected = false;
  try {
    await deniedClient.connect(deniedTransport);
  } catch {
    authenticationRejected = true;
  } finally {
    await deniedClient.close();
  }
  assert.equal(authenticationRejected, true);

  process.stdout.write(
    `${JSON.stringify({
      era: client.getProtocolEra(),
      protocolVersion: client.getNegotiatedProtocolVersion(),
      serverName: client.getServerVersion()?.name,
      toolCount: listed.tools.length,
      progressCount: progress.length,
      taskId: completed.structuredContent?.task?.id,
      cancelledTaskId: cancelled.structuredContent?.task?.id,
    })}\n`,
  );
} finally {
  await client.close();
}
