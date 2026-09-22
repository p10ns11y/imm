import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { bringSources, executeOnHost, lockInstall, resolveInsideSandbox } from "../src/sandbox.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const registryDir = path.join(root, "fixtures", "registry");
const now = new Date("2026-09-22T12:00:00.000Z");

function state() {
  return fs.mkdtempSync(path.join(os.tmpdir(), "imm-sandbox-"));
}

test("a locked install does not execute, and the bundler leaves unused files inside", () => {
  const stateDir = state();
  const locked = lockInstall({ name: "pure-slug", version: "1.0.0", registryDir, stateDir });
  assert.equal(locked.executed, false);
  assert.equal(locked.hostAccess, false);
  assert.equal(locked.scriptsExecuted, false);
  assert.equal(fs.existsSync(path.join(locked.sandbox, "src", "cli.js")), true);

  const brought = bringSources({ name: "pure-slug", version: "1.0.0", fn: "slugify", stateDir, now });
  assert.equal(brought.ok, true);
  assert.equal(brought.executed, false);
  assert.deepEqual(
    brought.brought.map((file) => file.path),
    ["src/index.js", "src/slugify.js", "src/unicode.js"],
  );
  assert.deepEqual(brought.omitted, ["src/cli.js"]);
  assert.equal(fs.existsSync(path.join(stateDir, "brought")), true);
});

test("install scripts stay in the sandbox and are not run", () => {
  const stateDir = state();
  const locked = lockInstall({ name: "install-script", version: "1.0.0", registryDir, stateDir });
  assert.equal(locked.scriptsIgnored, true);
  assert.equal(locked.scriptsExecuted, false);
  assert.equal(locked.executed, false);
});

test("the sandbox cannot escape or execute on the host", () => {
  const stateDir = state();
  const sandbox = path.join(stateDir, "sandbox");
  fs.mkdirSync(sandbox);
  const escaped = resolveInsideSandbox(sandbox, "../host-secret");
  assert.equal(escaped.ok, false);
  assert.deepEqual(escaped.reasons, ["sandbox-escape"]);
  const ran = executeOnHost();
  assert.equal(ran.executed, false);
  assert.deepEqual(ran.reasons, ["host-execution"]);
});

test("reading a throwing module as text does not execute it", () => {
  const stateDir = state();
  lockInstall({ name: "side-effect", version: "1.0.0", registryDir, stateDir });
  const brought = bringSources({ name: "side-effect", version: "1.0.0", fn: "ok", stateDir, now });
  assert.equal(brought.ok, true);
  assert.equal(brought.executed, false);
  assert.ok(brought.brought.some((file) => file.path === "src/boom.js"));
});

test("a reached dangerous call is not brought out of the sandbox", () => {
  const stateDir = state();
  lockInstall({ name: "shell-out", version: "1.0.0", registryDir, stateDir });
  const brought = bringSources({ name: "shell-out", version: "1.0.0", fn: "slugify", stateDir, now });
  assert.equal(brought.ok, false);
  assert.equal(brought.executed, false);
  assert.deepEqual(brought.brought, []);
  assert.ok(brought.reasons.some((reason) => reason.startsWith("child_process:")));
});
