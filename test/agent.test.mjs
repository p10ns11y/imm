import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { advise } from "../src/agent.mjs";
import { DEFAULT_POLICY } from "../src/policy.mjs";
import { slicePackage } from "../src/slice.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const now = new Date("2026-09-22T12:00:00.000Z");

function load(rel) {
  return JSON.parse(fs.readFileSync(path.join(root, rel), "utf8"));
}

test("a failed release check keeps the pin and does not bump", () => {
  const advice = advise({
    proposal: load("fixtures/proposals/fresh.json"),
    policy: DEFAULT_POLICY,
    now,
  });
  assert.equal(advice.ok, false);
  assert.equal(advice.verdict, "reject-update");
  assert.deepEqual(advice.next, { do: "keep-pinned", target: "fresh-slug@9.9.9" });
  assert.ok(advice.skip.includes("bump"));
  assert.ok(advice.why.includes("too-fresh"));
});

test("a trivial use records its hash before it is audited", () => {
  const advice = advise({ mod: load("fixtures/modules/slugify.json") });
  assert.equal(advice.ok, false);
  assert.equal(advice.verdict, "extract");
  assert.deepEqual(advice.next, { do: "record-hash", target: "slugify" });
  assert.deepEqual(advice.later, ["audit"]);
  assert.ok(advice.skip.includes("install"));
});

test("a direct dependency installs whole source and skips its subdependencies", () => {
  const advice = advise({ mod: load("fixtures/modules/sharp.json"), audits: {} });
  assert.equal(advice.ok, true);
  assert.equal(advice.verdict, "install-source");
  assert.deepEqual(advice.next, { do: "install-whole-source", target: "sharp@0.33.5" });
  assert.ok(advice.skip.includes("subdependencies"));
});

test("a library that still installs is told to vendor one extract", () => {
  const advice = advise({ mod: load("fixtures/modules/slug-kit-install.json") });
  assert.equal(advice.verdict, "vendor");
  assert.deepEqual(advice.next, { do: "vendor-extract", target: "slugify@1.0.0" });
  assert.deepEqual(advice.later, ["set nodeModules false"]);
  assert.ok(advice.skip.includes("node_modules"));
});

test("a shipped library has nothing left to maintain", () => {
  const advice = advise({ mod: load("fixtures/modules/slug-kit.json") });
  assert.equal(advice.ok, true);
  assert.equal(advice.verdict, "ship");
  assert.equal(advice.next.do, "none");
});

test("a reached dangerous call is not vendored", () => {
  const slice = slicePackage(
    path.join(root, "fixtures", "registry", "shell-out"),
    { specifier: "shell-out", version: "1.0.0", exports: ["slugify"] },
    { maxDepth: 6 },
    DEFAULT_POLICY,
    now,
  );
  const advice = advise({ slice });
  assert.equal(advice.verdict, "refuse-extract");
  assert.deepEqual(advice.next, { do: "do-not-vendor", target: "shell-out" });
  assert.ok(advice.why.some((reason) => reason.startsWith("child_process:")));
});

test("unused files are dropped and the reached files stay", () => {
  const slice = slicePackage(
    path.join(root, "fixtures", "registry", "pure-slug"),
    { specifier: "pure-slug", version: "1.0.0", exports: ["slugify"] },
    { maxDepth: 6 },
    DEFAULT_POLICY,
    now,
  );
  const advice = advise({ slice });
  assert.equal(advice.ok, true);
  assert.equal(advice.verdict, "extract");
  assert.deepEqual(advice.next, { do: "keep-reached-files", target: "pure-slug" });
  assert.deepEqual(advice.drop, ["src/cli.js"]);
});
