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

test("a trivial module is never published, downloaded, or installed", () => {
  const decision = decideRetention(load("fixtures/modules/slugify.json"), {});
  assert.equal(decision.action, "local");
  assert.deepEqual(decision.reasons, ["no-publish", "no-download", "no-install"]);
});

test("a direct dependency stays off the machine until each subdependency is audited", () => {
  const sharp = load("fixtures/modules/sharp.json");
  const held = decideRetention(sharp, {});
  assert.equal(held.action, "hold");
  assert.deepEqual(held.reasons, ["subdep-unaudited:color@4.2.3"]);

  const color = load("fixtures/modules/color.json");
  assert.equal(decideRetention(color, {}).action, "hold");
  assert.deepEqual(decideRetention(color, {}).reasons, ["needs-audit"]);

  const audits = load("fixtures/audits/color.json");
  assert.equal(decideRetention(color, audits).action, "store");
  const stored = decideRetention(sharp, audits);
  assert.equal(stored.action, "store");
  assert.deepEqual(stored.reasons, ["hard", "matured", "direct", "subdeps-audited"]);
});

test("a library ships audited extracted source and does not ask for node_modules", () => {
  const bare = decideRetention(load("fixtures/modules/slug-kit-install.json"), {});
  assert.equal(bare.action, "hold");
  assert.deepEqual(bare.reasons, ["needs-no-node-modules", "needs-vendor:slugify@1.0.0"]);

  const shipped = decideRetention(load("fixtures/modules/slug-kit.json"), {});
  assert.equal(shipped.action, "ship");
  assert.deepEqual(shipped.reasons, ["vendored", "audited", "extracted", "no-node-modules"]);
});

test("a direct dependency that is not matured is maintained locally", () => {
  const decision = decideRetention(load("fixtures/modules/fresh-util.json"), {});
  assert.equal(decision.action, "local");
  assert.ok(decision.reasons.includes("not-hard-or-matured"));
});
