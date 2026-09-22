import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const bin = path.join(root, "bin", "imm.mjs");

function run(args) {
  return spawnSync(process.execPath, [bin, ...args], { encoding: "utf8" });
}

test("demo prints the three steps and the filled call", () => {
  const result = run(["demo"]);
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /fresh\.json reject too-fresh/);
  assert.match(result.stdout, /omit\n\s+src\/cli\.js/);
  assert.match(result.stdout, /child_process:src\/index\.js/);
  assert.match(result.stdout, /AGENT_APPROVER/);
  assert.match(result.stdout, /hello-world/);
});

test("judge and approve exit 2 on a bad update and an agent approver", () => {
  const judged = run([
    "judge",
    "--proposal",
    path.join(root, "fixtures", "proposals", "trust-downgrade.json"),
    "--now",
    "2026-09-22T12:00:00.000Z",
  ]);
  assert.equal(judged.status, 2);
  assert.match(judged.stdout, /trust-downgrade/);

  const out = path.join(os.tmpdir(), "imm-cli-test");
  const plan = run([
    "plan",
    "--manifest",
    path.join(root, "fixtures", "agent-manifest.json"),
    "--registry",
    path.join(root, "fixtures", "registry"),
    "--out",
    out,
    "--now",
    "2026-09-22T12:00:00.000Z",
  ]);
  assert.equal(plan.status, 0, plan.stderr);
  const approve = run(["approve", "--state", out, "--approver", "agent"]);
  assert.equal(approve.status, 2);
  assert.match(approve.stderr, /AGENT_APPROVER/);
});

test("store holds a library that still wants node_modules and ships one that vendored the extract", () => {
  const held = run(["store", "--module", path.join(root, "fixtures", "modules", "slug-kit-install.json")]);
  assert.equal(held.status, 2);
  assert.match(held.stdout, /needs-no-node-modules/);
  assert.match(held.stdout, /needs-vendor:slugify@1\.0\.0/);

  const shipped = run(["store", "--module", path.join(root, "fixtures", "modules", "slug-kit.json")]);
  assert.equal(shipped.status, 0, shipped.stderr);
  assert.match(shipped.stdout, /ship vendored audited extracted no-node-modules/);
});
