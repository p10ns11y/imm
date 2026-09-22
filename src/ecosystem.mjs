import fs from "node:fs";
import path from "node:path";
import { ImmError } from "./errors.mjs";
import { sha256 } from "./hash.mjs";
import { DEFAULT_POLICY } from "./policy.mjs";
import { slicePackage } from "./slice.mjs";

const DEFAULT_BUDGETS = {
  maxPackages: 8,
  maxFiles: 40,
  maxBytes: 200000,
  maxDepth: 6,
};

function stubSource(specifier, exportName) {
  return `export function ${exportName}() {\n  throw new Error("imm: ${specifier}#${exportName} is not filled");\n}\n`;
}

export function planInstall(manifest, registryDir, policy = DEFAULT_POLICY, now = new Date()) {
  const budgets = { ...DEFAULT_BUDGETS, ...manifest.budgets };
  const needs = manifest.needs ?? [];
  const reasons = [];
  if (needs.length > budgets.maxPackages) reasons.push("budget-packages");

  const packages = [];
  if (!reasons.includes("budget-packages")) {
    for (const need of needs) {
      packages.push(slicePackage(path.join(registryDir, need.specifier), need, budgets, policy, now));
    }
  }

  const fileCount = packages.reduce((total, pkg) => total + pkg.files.length, 0);
  const bytes = packages.reduce(
    (total, pkg) => total + pkg.files.reduce((sum, file) => sum + file.bytes, 0),
    0,
  );
  if (fileCount > budgets.maxFiles) reasons.push("budget-files");
  if (bytes > budgets.maxBytes) reasons.push("budget-bytes");

  const blocked = reasons.length > 0 || packages.some((pkg) => pkg.blocked);
  return { blocked, reasons, budgets, packages };
}

export function publicPlan(planResult) {
  return {
    blocked: planResult.blocked,
    reasons: planResult.reasons,
    budgets: planResult.budgets,
    packages: planResult.packages.map((pkg) => ({
      name: pkg.name,
      version: pkg.version,
      blocked: pkg.blocked,
      reasons: pkg.reasons,
      omitted: pkg.omitted,
      exports: pkg.exports,
      files: pkg.files.map(({ path: filePath, sha256: sum, bytes, depth }) => ({
        path: filePath,
        sha256: sum,
        bytes,
        depth,
      })),
    })),
  };
}

export function writePlan(outDir, planResult) {
  fs.rmSync(outDir, { recursive: true, force: true });
  fs.mkdirSync(outDir, { recursive: true });

  for (const pkg of planResult.packages) {
    for (const exportName of pkg.exports) {
      if (!/^[A-Za-z_$][\w$]*$/.test(exportName)) continue;
      const dest = path.join(outDir, "stubs", pkg.name, `${exportName}.js`);
      fs.mkdirSync(path.dirname(dest), { recursive: true });
      fs.writeFileSync(dest, stubSource(pkg.name, exportName));
    }
  }

  if (!planResult.blocked) {
    for (const pkg of planResult.packages) {
      for (const file of pkg.files) {
        const dest = path.join(outDir, "stage", pkg.name, file.path);
        fs.mkdirSync(path.dirname(dest), { recursive: true });
        fs.writeFileSync(dest, file.text);
      }
    }
  }

  const body = publicPlan(planResult);
  fs.writeFileSync(path.join(outDir, "proposal.json"), `${JSON.stringify(body, null, 2)}\n`);
  return body;
}

export function approvePlan(outDir, approver) {
  if (approver !== "human") {
    throw new ImmError("AGENT_APPROVER", "An agent cannot approve a fill.");
  }
  const proposal = JSON.parse(fs.readFileSync(path.join(outDir, "proposal.json"), "utf8"));
  if (proposal.blocked || proposal.packages.some((pkg) => pkg.blocked)) {
    throw new ImmError("BLOCKED", "The proposal is blocked.");
  }

  const lock = { packages: {} };
  for (const pkg of proposal.packages) {
    lock.packages[pkg.name] = { version: pkg.version, files: {} };
    for (const file of pkg.files) {
      const stagePath = path.join(outDir, "stage", pkg.name, file.path);
      const text = fs.readFileSync(stagePath);
      const sum = sha256(text);
      if (sum !== file.sha256) {
        throw new ImmError("HASH_MISMATCH", `${pkg.name}/${file.path} changed after the proposal.`);
      }
      const dest = path.join(outDir, "vendor", pkg.name, file.path);
      fs.mkdirSync(path.dirname(dest), { recursive: true });
      fs.writeFileSync(dest, text);
      lock.packages[pkg.name].files[file.path] = sum;
    }
  }
  fs.writeFileSync(path.join(outDir, "lock.json"), `${JSON.stringify(lock, null, 2)}\n`);
  return lock;
}

export function verifyLock(outDir) {
  const lock = JSON.parse(fs.readFileSync(path.join(outDir, "lock.json"), "utf8"));
  const problems = [];
  for (const [name, pkg] of Object.entries(lock.packages)) {
    for (const [rel, sum] of Object.entries(pkg.files)) {
      const abs = path.join(outDir, "vendor", name, rel);
      if (!fs.existsSync(abs)) problems.push(`missing ${name}/${rel}`);
      else if (sha256(fs.readFileSync(abs)) !== sum) problems.push(`hash ${name}/${rel}`);
    }
  }
  return { ok: problems.length === 0, problems };
}
