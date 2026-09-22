import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { decideRetention } from "../src/store.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function load(rel) {
  return JSON.parse(fs.readFileSync(path.join(root, rel), "utf8"));
}

test("a trivial use is an extract, not an install, and it waits for a hash and an audit", () => {
  const decision = decideRetention(load("fixtures/modules/slugify.json"), {});
  assert.equal(decision.action, "hold");
  assert.deepEqual(decision.reasons, ["needs-hash", "needs-audit"]);
});

test("a direct dependency installs its whole source and does not install subdependencies", () => {
  const sharp = decideRetention(load("fixtures/modules/sharp.json"), {});
  assert.equal(sharp.action, "install-source");
  assert.deepEqual(sharp.reasons, ["direct", "whole-source"]);
  assert.equal(sharp.name, "sharp@0.33.5");
});

test("a per-usage function keeps a hash, an audit, and the agent diff", () => {
  const bare = decideRetention(load("fixtures/modules/color.json"), {});
  assert.equal(bare.action, "hold");
  assert.deepEqual(bare.reasons, ["needs-hash", "needs-audit"]);

  const edited = decideRetention(load("fixtures/modules/color-edited.json"), {});
  assert.equal(edited.action, "hold");
  assert.deepEqual(edited.reasons, ["needs-agent-diff"]);

  const used = decideRetention(load("fixtures/modules/color-use.json"), {});
  assert.equal(used.action, "extract");
  assert.equal(used.name, "color@4.2.3#convert");
  assert.deepEqual(used.reasons, ["per-usage", "audited", "hash", "agent-diff"]);
});

test("a library ships audited extracted source and does not ask for node_modules", () => {
  const bare = decideRetention(load("fixtures/modules/slug-kit-install.json"), {});
  assert.equal(bare.action, "hold");
  assert.deepEqual(bare.reasons, ["needs-no-node-modules", "needs-vendor:slugify@1.0.0"]);

  const shipped = decideRetention(load("fixtures/modules/slug-kit.json"), {});
  assert.equal(shipped.action, "ship");
  assert.deepEqual(shipped.reasons, ["vendored", "audited", "extracted", "no-node-modules"]);
});

test("every direct dependency installs as whole source", () => {
  const decision = decideRetention(load("fixtures/modules/fresh-util.json"), {});
  assert.equal(decision.action, "install-source");
  assert.deepEqual(decision.reasons, ["direct", "whole-source"]);
});
