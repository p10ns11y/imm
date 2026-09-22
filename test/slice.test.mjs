import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { DEFAULT_POLICY } from "../src/policy.mjs";
import { slicePackage } from "../src/slice.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const registry = path.join(root, "fixtures", "registry");
const now = new Date("2026-09-22T12:00:00.000Z");
const budgets = { maxPackages: 4, maxFiles: 20, maxBytes: 100000, maxDepth: 6 };

function slice(specifier, version, exports, budget = budgets) {
  return slicePackage(
    path.join(registry, specifier),
    { specifier, version, exports },
    budget,
    DEFAULT_POLICY,
    now,
  );
}

test("keeps the slugify import graph and omits the unused cli", () => {
  const pkg = slice("pure-slug", "1.0.0", ["slugify"]);
  assert.equal(pkg.blocked, false);
  assert.deepEqual(
    pkg.files.map((file) => file.path),
    ["src/index.js", "src/slugify.js", "src/unicode.js"],
  );
  assert.deepEqual(pkg.omitted, ["src/cli.js"]);
});

test("blocks a reached child_process import and does not throw while reading", () => {
  const dangerous = slice("shell-out", "1.0.0", ["slugify"]);
  assert.equal(dangerous.blocked, true);
  assert.ok(dangerous.reasons.some((reason) => reason.startsWith("child_process:")));

  const sideEffect = slice("side-effect", "1.0.0", ["ok"]);
  assert.equal(sideEffect.blocked, false);
  assert.ok(sideEffect.files.some((file) => file.path === "src/boom.js"));
});

test("blocks a fresh publish, a trust drop, a lifecycle script, and a shallow budget", () => {
  assert.ok(slice("fresh-slug", "9.9.9", ["slugify"]).reasons.includes("too-fresh"));
  assert.ok(slice("trust-drop", "2.0.0", ["slugify"]).reasons.includes("trust-downgrade"));
  assert.ok(slice("install-script", "1.0.0", ["slugify"]).reasons.includes("lifecycle-script"));

  const shallow = slice("pure-slug", "1.0.0", ["slugify"], { ...budgets, maxDepth: 1 });
  assert.ok(shallow.reasons.includes("budget-depth"));
  assert.equal(shallow.files.some((file) => file.path === "src/unicode.js"), false);
});
