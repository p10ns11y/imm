import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { ImmError } from "./errors.mjs";
import { approvePlan, planInstall, verifyLock, writePlan } from "./ecosystem.mjs";
import { formatSlice, formatVerdict } from "./format.mjs";
import { DEFAULT_POLICY, judgeUpdate } from "./policy.mjs";
import { slicePackage } from "./slice.mjs";
import { advise } from "./agent.mjs";
import { decideRetention, formatRetention } from "./store.mjs";

const NOW = new Date("2026-09-22T12:00:00.000Z");
const BUDGETS = { maxPackages: 4, maxFiles: 20, maxBytes: 100000, maxDepth: 6 };

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

export async function runDemo(root) {
  const lines = [];
  const say = (line = "") => lines.push(line);
  const registry = path.join(root, "fixtures", "registry");
  const proposals = path.join(root, "fixtures", "proposals");

  say("1. Release check. An agent may propose. The check still rejects.");
  for (const name of ["ok.json", "fresh.json", "trust-downgrade.json", "lifecycle.json", "floating.json"]) {
    const verdict = judgeUpdate(readJson(path.join(proposals, name)), DEFAULT_POLICY, NOW);
    say(`  ${name} ${formatVerdict(verdict)}`);
  }

  say("");
  say("2. Reachable fill. Named exports keep the files they need.");
  const pure = slicePackage(
    path.join(registry, "pure-slug"),
    { specifier: "pure-slug", version: "1.0.0", exports: ["slugify"] },
    BUDGETS,
    DEFAULT_POLICY,
    NOW,
  );
  say(formatSlice(pure).split("\n").map((line) => `  ${line}`).join("\n"));

  say("");
  say("3. A reached forbidden call blocks the package. The bytes stay out.");
  const shell = slicePackage(
    path.join(registry, "shell-out"),
    { specifier: "shell-out", version: "1.0.0", exports: ["slugify"] },
    BUDGETS,
    DEFAULT_POLICY,
    NOW,
  );
  say(formatSlice(shell).split("\n").map((line) => `  ${line}`).join("\n"));

  say("");
  say("4. Install writes throw-stubs. Approval is a second principal.");
  const manifest = readJson(path.join(root, "fixtures", "agent-manifest.json"));
  const planned = planInstall(manifest, registry, DEFAULT_POLICY, NOW);
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), "imm-"));
  writePlan(outDir, planned);
  const stubUrl = pathToFileURL(path.join(outDir, "stubs", "pure-slug", "slugify.js")).href;
  const stub = await import(stubUrl);
  try {
    stub.slugify("Hello");
    say("  stub returned a value");
  } catch (error) {
    say(`  stub ${error.message}`);
  }

  try {
    approvePlan(outDir, "agent");
    say("  agent approve accepted");
  } catch (error) {
    if (error instanceof ImmError) say(`  agent approve ${error.code}`);
    else throw error;
  }

  approvePlan(outDir, "human");
  const filledUrl = pathToFileURL(path.join(outDir, "vendor", "pure-slug", "src", "index.js")).href;
  const filled = await import(filledUrl);
  say(`  human approve slugify("Hello World") => ${filled.slugify("Hello World")}`);

  const check = verifyLock(outDir);
  say(`  verify reads the vendor lock ${check.ok ? "ok" : check.problems.join(" ")}`);

  say("");
  say("5. Install only the direct dependency, as whole source. Other uses stay extracts.");
  say(`  ${formatRetention(decideRetention({ name: "slugify", kind: "trivial" }, {}))}`);
  say(
    `  ${formatRetention(decideRetention({ name: "sharp", kind: "direct", version: "0.33.5", subdeps: ["color@4.2.3"] }, {}))}`,
  );
  say(
    `  ${formatRetention(decideRetention({ name: "color", kind: "use", version: "4.2.3", fn: "convert", hash: "sha256-abc", audit: { by: "agent", at: "2026-08-01" } }, {}))}`,
  );
  const agentStep = advise({ mod: { name: "sharp", kind: "direct", version: "0.33.5" } });
  say(`  agent next ${agentStep.next.do} ${agentStep.next.target}`);
  say("  Library authors ship that extract inside the package.");
  say(
    `  ${formatRetention(decideRetention({ name: "slug-kit", kind: "library", nodeModules: true, subdeps: ["slugify@1.0.0"], vendored: [] }, {}))}`,
  );
  say(
    `  ${formatRetention(decideRetention({ name: "slug-kit", kind: "library", nodeModules: false, subdeps: ["slugify@1.0.0"], vendored: [{ id: "slugify@1.0.0", files: ["vendor/slugify.js"], audit: { by: "author", at: "2026-08-01" } }] }, {}))}`,
  );

  say("");
  say(`state ${outDir}`);
  return lines.join("\n");
}
