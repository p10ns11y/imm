import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { encodeMessage, handleMessage, takeMessage } from "../src/mcp.mjs";
import { auditUse, extractUse, installSource, overwriteUse } from "../src/ops.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const registryDir = path.join(root, "fixtures", "registry");
const now = new Date("2026-09-22T12:00:00.000Z");

function state() {
  return fs.mkdtempSync(path.join(os.tmpdir(), "imm-mcp-"));
}

test("install keeps whole source and extract drops the unused cli", () => {
  const stateDir = state();
  const installed = installSource({ name: "pure-slug", version: "1.0.0", registryDir, stateDir });
  assert.equal(installed.ok, true);
  assert.equal(installed.action, "install-source");
  assert.ok(installed.files.some((file) => file.path === "src/cli.js"));

  const extracted = extractUse({
    name: "pure-slug",
    version: "1.0.0",
    fn: "slugify",
    registryDir,
    stateDir,
    now,
  });
  assert.equal(extracted.ok, false);
  assert.deepEqual(extracted.reasons, ["needs-audit"]);
  assert.deepEqual(extracted.omitted, ["src/cli.js"]);
  assert.match(extracted.hash, /^[a-f0-9]{64}$/);

  const audited = auditUse({
    id: "pure-slug@1.0.0#slugify",
    by: "agent",
    at: "2026-09-22",
    stateDir,
  });
  assert.equal(audited.ok, true);
  assert.equal(audited.action, "extract");

  const overwritten = overwriteUse({
    id: "pure-slug@1.0.0#slugify",
    text: "export function slugify(input){ return 'x'; }\n",
    stateDir,
  });
  assert.equal(overwritten.ok, true);
  assert.equal(overwritten.hash, extracted.hash);
  assert.match(overwritten.diff, /agent|^\+\+\+|^---/m);
  assert.notEqual(overwritten.currentHash, overwritten.hash);
});

test("MCP lists the four tools and install returns whole source", async () => {
  const stateDir = state();
  const ctx = { registryDir, stateDir, now };
  const listed = handleMessage({ jsonrpc: "2.0", id: 1, method: "tools/list" }, ctx);
  assert.deepEqual(
    listed.result.tools.map((tool) => tool.name),
    ["imm_install", "imm_extract", "imm_audit", "imm_sandbox", "imm_overwrite"],
  );
  const called = handleMessage(
    {
      jsonrpc: "2.0",
      id: 2,
      method: "tools/call",
      params: { name: "imm_install", arguments: { name: "pure-slug", version: "1.0.0" } },
    },
    ctx,
  );
  const body = JSON.parse(called.result.content[0].text);
  assert.equal(body.action, "install-source");
  assert.equal(called.result.isError, false);
});

test("stdio server answers initialize and tools/list", async () => {
  const bin = path.join(root, "bin", "imm.mjs");
  const child = spawn(process.execPath, [bin, "mcp", "--registry", registryDir, "--state", state()], {
    stdio: ["pipe", "pipe", "pipe"],
  });
  let buf = Buffer.alloc(0);
  const messages = [];
  child.stdout.on("data", (chunk) => {
    buf = Buffer.concat([buf, chunk]);
    while (true) {
      const taken = takeMessage(buf);
      if (!taken || taken.error || !taken.message) break;
      messages.push(taken.message);
      buf = Buffer.from(taken.rest);
    }
  });
  const ready = new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error("mcp timed out")), 2000);
    const check = () => {
      if (messages.length >= 2) {
        clearTimeout(timer);
        resolve();
      }
    };
    child.stdout.on("data", check);
    check();
  });
  const send = (message) => child.stdin.write(encodeMessage(message));
  send({
    jsonrpc: "2.0",
    id: 1,
    method: "initialize",
    params: { protocolVersion: "2024-11-05", capabilities: {}, clientInfo: { name: "test", version: "0" } },
  });
  send({ jsonrpc: "2.0", method: "notifications/initialized" });
  send({ jsonrpc: "2.0", id: 2, method: "tools/list" });
  await ready;
  child.kill();
  assert.equal(messages[0].result.serverInfo.name, "imm");
  assert.equal(messages[1].result.tools.length, 5);
});
