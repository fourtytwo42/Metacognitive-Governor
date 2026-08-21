#!/usr/bin/env node
import {
  createHash,
  generateKeyPairSync,
  randomUUID,
  sign as signEd25519
} from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  writeFileSync
} from "node:fs";
import { dirname, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const MCP_VERSION = "2026-07-28";
const SIGNATURE_DOMAIN = Buffer.from("4d43472d4f5247414e495a4154494f4e2d53484152450001", "hex");

function parseArguments(values) {
  const parsed = {};
  for (let index = 0; index < values.length; index += 2) {
    const name = values[index];
    const value = values[index + 1];
    if (!name?.startsWith("--") || value == null) {
      throw new Error(`invalid argument boundary near ${name ?? "end of input"}`);
    }
    parsed[name.slice(2).replaceAll("-", "_")] = value;
  }
  for (const required of ["mode", "host", "token_file", "cli", "state", "output"]) {
    if (!parsed[required]) throw new Error(`missing --${required.replaceAll("_", "-")}`);
  }
  if (!new Set(["exercise", "verify"]).has(parsed.mode)) {
    throw new Error("--mode must be exercise or verify");
  }
  return parsed;
}

const args = parseArguments(process.argv.slice(2));
const host = args.host.replace(/\/$/, "");
const tokenFile = resolve(args.token_file);
const cliPath = resolve(args.cli);
const statePath = resolve(args.state);
const outputPath = resolve(args.output);
const token = readFileSync(tokenFile, "utf8").trim();
if (!token) throw new Error("loopback token is empty");

async function request(path, { method = "GET", body, allowFailure = false } = {}) {
  const response = await fetch(`${host}${path}`, {
    method,
    headers: {
      Authorization: `Bearer ${token}`,
      Accept: "application/json",
      ...(body === undefined ? {} : { "Content-Type": "application/json" })
    },
    body: body === undefined ? undefined : JSON.stringify(body),
    signal: AbortSignal.timeout(120_000)
  });
  const text = await response.text();
  const value = text ? JSON.parse(text) : null;
  if (!allowFailure && !response.ok) {
    throw new Error(`${method} ${path} failed with HTTP ${response.status}`);
  }
  return { status: response.status, ok: response.ok, value };
}

async function mcpTool(requestId, name, toolArguments) {
  const response = await fetch(`${host}/mcp`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${token}`,
      "Content-Type": "application/json",
      Accept: "application/json, text/event-stream",
      "MCP-Protocol-Version": MCP_VERSION,
      "Mcp-Method": "tools/call",
      "Mcp-Name": name
    },
    body: JSON.stringify({
      jsonrpc: "2.0",
      id: requestId,
      method: "tools/call",
      params: {
        name,
        arguments: toolArguments,
        _meta: {
          "io.modelcontextprotocol/protocolVersion": MCP_VERSION,
          "io.modelcontextprotocol/clientInfo": {
            name: "mcg-installed-organization-sharing-uat",
            version: "1.0.0"
          },
          "io.modelcontextprotocol/clientCapabilities": {}
        }
      }
    }),
    signal: AbortSignal.timeout(120_000)
  });
  if (!response.ok) throw new Error(`MCP ${name} failed with HTTP ${response.status}`);
  const value = await response.json();
  if (value.error || value.result?.isError === true) {
    throw new Error(`MCP ${name} returned an error`);
  }
  return value.result?.structuredContent;
}

function cliOrganization(commandArguments) {
  const completed = spawnSync(
    cliPath,
    [
      "--host",
      host,
      "--token-file",
      tokenFile,
      "organization",
      ...commandArguments
    ],
    {
      encoding: "utf8",
      timeout: 120_000,
      maxBuffer: 4 * 1024 * 1024,
      windowsHide: true
    }
  );
  if (completed.error) throw completed.error;
  if (completed.status !== 0 || completed.stderr !== "") {
    throw new Error(`installed CLI organization command failed with status ${completed.status}`);
  }
  return JSON.parse(completed.stdout);
}

function pushU16(value) {
  const bytes = Buffer.alloc(2);
  bytes.writeUInt16BE(value);
  return bytes;
}

function pushText(value) {
  const bytes = Buffer.from(value, "utf8");
  const length = Buffer.alloc(4);
  length.writeUInt32BE(bytes.length);
  return Buffer.concat([length, bytes]);
}

function signedPackage(portable, registry, packageId, publisherId, privateKey) {
  const portableBytes = Buffer.from(JSON.stringify(portable), "utf8");
  const payload = Buffer.concat([
    SIGNATURE_DOMAIN,
    pushU16(1),
    pushText(registry),
    pushText(packageId),
    pushText(publisherId),
    pushText(portable.content_sha256),
    createHash("sha256").update(portableBytes).digest()
  ]);
  const signature = signEd25519(null, payload, privateKey);
  if (signature.length !== 64) throw new Error("Ed25519 signature length mismatch");
  return {
    schema_version: 1,
    registry,
    package: packageId,
    publisher: publisherId,
    portable,
    signature: {
      scheme: "ed25519-v1",
      signature_base64: signature.toString("base64")
    }
  };
}

function writeJsonAtomic(path, value) {
  if (existsSync(path)) throw new Error(`refusing to replace existing output: ${path}`);
  mkdirSync(dirname(path), { recursive: true });
  const temporary = `${path}.tmp-${randomUUID()}`;
  writeFileSync(temporary, `${JSON.stringify(value, null, 2)}\n`, { flag: "wx", mode: 0o600 });
  renameSync(temporary, path);
}

function findById(values, id) {
  return values.find((value) => value.id === id);
}

async function exercise() {
  if (existsSync(statePath)) throw new Error(`refusing to replace existing state: ${statePath}`);
  const identity = randomUUID();
  const compactIdentity = identity.replaceAll("-", "");
  const publisherId = `uat.publisher.${compactIdentity}`;
  const packageId = `uat-package-${compactIdentity}`;
  const registry = "https://registry.example/";
  const importedName = `Installed shared organization ${identity.slice(0, 8)}`;
  const importReason = "Owner reviewed the signed installed-package organization definition.";

  const created = await request("/api/v1/organizations", {
    method: "POST",
    body: { name: `Sharing source ${identity.slice(0, 8)}` }
  });
  if (created.status !== 201) throw new Error("source organization was not created");
  const source = created.value.organization;
  const exported = await request(`/api/v1/organization-revisions/${source.revision_id}/export`);
  const portable = exported.value;

  const { publicKey, privateKey } = generateKeyPairSync("ed25519");
  const publicJwk = publicKey.export({ format: "jwk" });
  const publicKeyBytes = Buffer.from(publicJwk.x, "base64url");
  if (publicKeyBytes.length !== 32) throw new Error("Ed25519 public-key length mismatch");
  const publicKeyBase64 = publicKeyBytes.toString("base64");
  const publicKeySha256 = createHash("sha256").update(publicKeyBytes).digest("hex");
  const packageValue = signedPackage(portable, registry, packageId, publisherId, privateKey);

  const trusted = cliOrganization([
    "trust-publisher",
    publisherId,
    "--display-name",
    "Installed Package UAT Publisher",
    "--public-key-base64",
    publicKeyBase64,
    "--registry",
    registry,
    "--reason",
    "Owner reviewed the disposable installed-package publisher fingerprint.",
    "--json"
  ]);
  if (trusted.replayed || trusted.publisher.state !== "trusted" || trusted.publisher.revision !== 1) {
    throw new Error("CLI publisher trust result mismatch");
  }

  const importedReviewId = randomUUID();
  const reviewed = await request("/api/v1/organization-share-reviews", {
    method: "POST",
    body: { review_id: importedReviewId, package: packageValue }
  });
  if (reviewed.status !== 201 || reviewed.value.review.state !== "pending") {
    throw new Error("REST signed review result mismatch");
  }
  const importId = randomUUID();
  const importPath = `/api/v1/organization-share-reviews/${importedReviewId}/import`;
  const imported = await request(importPath, {
    method: "POST",
    body: { import_id: importId, name: importedName, reason: importReason }
  });
  if (
    imported.status !== 201 ||
    imported.value.replayed ||
    imported.value.events?.length !== 2 ||
    imported.value.organization.name !== importedName
  ) {
    throw new Error("REST signed import result mismatch");
  }
  const replay = await request(importPath, {
    method: "POST",
    body: { import_id: importId, name: importedName, reason: importReason }
  });
  if (replay.status !== 200 || !replay.value.replayed || replay.value.events?.length !== 0) {
    throw new Error("REST signed import replay was not observation-only");
  }

  const rejectedReviewId = randomUUID();
  const mcpReview = await mcpTool(701, "mcg_organization_share_mutate", {
    action: "review",
    review_id: rejectedReviewId,
    package: packageValue
  });
  if (mcpReview.review?.state !== "pending") throw new Error("MCP review result mismatch");
  const rejectionReason = "Owner rejected the second installed-package review fixture.";
  const mcpReject = await mcpTool(702, "mcg_organization_share_mutate", {
    action: "reject",
    review_id: rejectedReviewId,
    reason: rejectionReason
  });
  if (mcpReject.review?.state !== "rejected") throw new Error("MCP reject result mismatch");

  const tampered = structuredClone(packageValue);
  tampered.portable.definition.name = "Tampered installed package";
  const tamperedResult = await request("/api/v1/organization-share-reviews", {
    method: "POST",
    body: { review_id: randomUUID(), package: tampered },
    allowFailure: true
  });
  if (tamperedResult.ok) throw new Error("tampered signed package was accepted");

  const cliReviews = cliOrganization(["share-reviews", "--json"]);
  if (
    findById(cliReviews.reviews, importedReviewId)?.state !== "imported" ||
    findById(cliReviews.reviews, rejectedReviewId)?.state !== "rejected"
  ) {
    throw new Error("CLI review history disagrees with REST/MCP mutations");
  }

  const revoked = cliOrganization([
    "revoke-publisher",
    publisherId,
    "1",
    "--reason",
    "Owner completed the disposable publisher revocation gate.",
    "--json"
  ]);
  if (revoked.replayed || revoked.publisher.state !== "revoked" || revoked.publisher.revision !== 2) {
    throw new Error("CLI publisher revocation result mismatch");
  }
  const reviewCountBeforeRevokedAttempt = cliReviews.reviews.length;
  const revokedReview = await request("/api/v1/organization-share-reviews", {
    method: "POST",
    body: { review_id: randomUUID(), package: packageValue },
    allowFailure: true
  });
  if (revokedReview.ok) throw new Error("revoked publisher created a new review");
  const reviewsAfterRevocation = (await request("/api/v1/organization-share-reviews")).value.reviews;
  if (reviewsAfterRevocation.length !== reviewCountBeforeRevokedAttempt) {
    throw new Error("revoked review attempt mutated durable history");
  }

  const state = {
    schema_version: 1,
    publisher_id: publisherId,
    publisher_public_key_sha256: publicKeySha256,
    publisher_revision: 2,
    registry,
    imported_review_id: importedReviewId,
    rejected_review_id: rejectedReviewId,
    import_id: importId,
    imported_organization_id: imported.value.organization.organization_id,
    imported_revision_id: imported.value.organization.revision_id,
    imported_name: importedName,
    import_reason: importReason,
    package_sha256: reviewed.value.review.evidence.package_sha256
  };
  writeJsonAtomic(statePath, state);
  writeJsonAtomic(outputPath, {
    schema_version: 1,
    mode: "exercise",
    publisher_trusted_via_cli: true,
    signed_reviewed_via_rest: true,
    imported_via_rest: true,
    exact_import_replay_observation_only: true,
    rejected_via_mcp: true,
    tampered_package_rejected: true,
    publisher_revoked_via_cli: true,
    revoked_review_mutation_free: true,
    private_signing_key_persisted: false,
    package_sha256: state.package_sha256
  });
}

async function verify() {
  const state = JSON.parse(readFileSync(statePath, "utf8"));
  if (state.schema_version !== 1) throw new Error("organization-sharing state schema mismatch");

  const publishers = cliOrganization(["publishers", "--json"]);
  const publisher = findById(publishers.publishers, state.publisher_id);
  if (publisher?.state !== "revoked" || publisher.revision !== state.publisher_revision) {
    throw new Error("revoked publisher did not survive restart");
  }
  if (publisher.public_key_base64 !== undefined) {
    const publicBytes = Buffer.from(publisher.public_key_base64, "base64");
    const observedHash = createHash("sha256").update(publicBytes).digest("hex");
    if (observedHash !== state.publisher_public_key_sha256) {
      throw new Error("publisher fingerprint changed after restart");
    }
  }

  const reviews = cliOrganization(["share-reviews", "--json"]);
  const importedReview = findById(reviews.reviews, state.imported_review_id);
  const rejectedReview = findById(reviews.reviews, state.rejected_review_id);
  if (
    importedReview?.state !== "imported" ||
    importedReview.organization_id !== state.imported_organization_id ||
    importedReview.evidence?.package_sha256 !== state.package_sha256 ||
    rejectedReview?.state !== "rejected"
  ) {
    throw new Error("share review decisions did not survive restart");
  }

  const organizations = (await request("/api/v1/organizations")).value.organizations;
  const importedOrganization = organizations.find(
    (organization) => organization.organization_id === state.imported_organization_id
  );
  if (
    importedOrganization?.revision_id !== state.imported_revision_id ||
    importedOrganization.name !== state.imported_name
  ) {
    throw new Error("imported organization did not survive restart");
  }

  const replay = await request(
    `/api/v1/organization-share-reviews/${state.imported_review_id}/import`,
    {
      method: "POST",
      body: {
        import_id: state.import_id,
        name: state.imported_name,
        reason: state.import_reason
      }
    }
  );
  if (replay.status !== 200 || !replay.value.replayed || replay.value.events?.length !== 0) {
    throw new Error("post-restart import replay was not observation-only");
  }

  const mcpPublishers = await mcpTool(703, "mcg_organization_publishers", {});
  const mcpReviews = await mcpTool(704, "mcg_organization_share_reviews", {});
  if (
    findById(mcpPublishers.publishers ?? [], state.publisher_id)?.state !== "revoked" ||
    findById(mcpReviews.reviews ?? [], state.imported_review_id)?.state !== "imported" ||
    findById(mcpReviews.reviews ?? [], state.rejected_review_id)?.state !== "rejected"
  ) {
    throw new Error("MCP durable sharing view disagrees after restart");
  }

  writeJsonAtomic(outputPath, {
    schema_version: 1,
    mode: "verify",
    publisher_revocation_survived_restart: true,
    imported_review_survived_restart: true,
    rejected_review_survived_restart: true,
    imported_organization_survived_restart: true,
    post_restart_import_replay_observation_only: true,
    cli_rest_mcp_agree: true,
    private_signing_key_persisted: false,
    package_sha256: state.package_sha256
  });
}

if (args.mode === "exercise") await exercise();
else await verify();
