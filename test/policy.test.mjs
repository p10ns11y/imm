import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { DEFAULT_POLICY, judgeUpdate } from "../src/policy.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const now = new Date("2026-09-22T12:00:00.000Z");

function load(name) {
  return JSON.parse(fs.readFileSync(path.join(root, "fixtures", "proposals", name), "utf8"));
}

test("accepts an exact, aged, trusted update with no scripts", () => {
  const verdict = judgeUpdate(load("ok.json"), DEFAULT_POLICY, now);
  assert.deepEqual(verdict, { accept: true, reasons: [] });
});

test("rejects a release inside the age window", () => {
  const verdict = judgeUpdate(load("fresh.json"), DEFAULT_POLICY, now);
  assert.equal(verdict.accept, false);
  assert.ok(verdict.reasons.includes("too-fresh"));
});

test("rejects a trust downgrade", () => {
  const verdict = judgeUpdate(load("trust-downgrade.json"), DEFAULT_POLICY, now);
  assert.equal(verdict.accept, false);
  assert.ok(verdict.reasons.includes("trust-downgrade"));
});

test("rejects lifecycle scripts and floating ranges", () => {
  assert.ok(judgeUpdate(load("lifecycle.json"), DEFAULT_POLICY, now).reasons.includes("lifecycle-script"));
  assert.ok(judgeUpdate(load("floating.json"), DEFAULT_POLICY, now).reasons.includes("floating-range"));
});
