import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { pathToFileURL, fileURLToPath } from "node:url";
import { approvePlan, planInstall, verifyLock, writePlan } from "../src/ecosystem.mjs";
import { ImmError } from "../src/errors.mjs";
import { DEFAULT_POLICY } from "../src/policy.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const registry = path.join(root, "fixtures", "registry");
const now = new Date("2026-09-22T12:00:00.000Z");

function manifest(name) {
  return JSON.parse(fs.readFileSync(path.join(root, "fixtures", name), "utf8"));
}

test("stubs throw, an agent cannot approve, a human fill runs offline", async () => {
  const planned = planInstall(manifest("agent-manifest.json"), registry, DEFAULT_POLICY, now);
  assert.equal(planned.blocked, false);
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), "imm-"));
  writePlan(outDir, planned);

  const stub = await import(pathToFileURL(path.join(outDir, "stubs", "pure-slug", "slugify.js")).href);
  assert.throws(() => stub.slugify("Hello"), /not filled/);

  assert.throws(() => approvePlan(outDir, "agent"), (error) => {
    assert.ok(error instanceof ImmError);
    assert.equal(error.code, "AGENT_APPROVER");
    return true;
  });
  assert.equal(fs.existsSync(path.join(outDir, "vendor")), false);

  approvePlan(outDir, "human");
  const filled = await import(
    pathToFileURL(path.join(outDir, "vendor", "pure-slug", "src", "index.js")).href,
  );
  assert.equal(filled.slugify("Hello World"), "hello-world");
  assert.equal(verifyLock(outDir).ok, true);

  const vendorFile = path.join(outDir, "vendor", "pure-slug", "src", "slugify.js");
  fs.writeFileSync(vendorFile, `${fs.readFileSync(vendorFile, "utf8")}\n`);
  assert.equal(verifyLock(outDir).ok, false);
});

test("a blocked proposal has stubs and no stage", () => {
  const planned = planInstall(
    {
      needs: [{ specifier: "shell-out", version: "1.0.0", exports: ["slugify"] }],
      budgets: { maxPackages: 4, maxFiles: 20, maxBytes: 100000, maxDepth: 6 },
    },
    registry,
    DEFAULT_POLICY,
    now,
  );
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), "imm-"));
  writePlan(outDir, planned);
  assert.equal(fs.existsSync(path.join(outDir, "stubs", "shell-out", "slugify.js")), true);
  assert.equal(fs.existsSync(path.join(outDir, "stage")), false);
  assert.throws(() => approvePlan(outDir, "human"), (error) => error.code === "BLOCKED");
});

test("package count over budget never opens the registry packages", () => {
  const planned = planInstall(manifest("over-budget.json"), registry, DEFAULT_POLICY, now);
  assert.equal(planned.blocked, true);
  assert.deepEqual(planned.reasons, ["budget-packages"]);
  assert.deepEqual(planned.packages, []);
});

test("edited stage bytes fail approval", () => {
  const planned = planInstall(manifest("agent-manifest.json"), registry, DEFAULT_POLICY, now);
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), "imm-"));
  writePlan(outDir, planned);
  const staged = path.join(outDir, "stage", "pure-slug", "src", "slugify.js");
  fs.writeFileSync(staged, "export function slugify(){ return 'tampered'; }\n");
  assert.throws(() => approvePlan(outDir, "human"), (error) => error.code === "HASH_MISMATCH");
});
