import fs from "node:fs";
import path from "node:path";
import { sha256 } from "./hash.mjs";
import { hasLifecycleScripts, DEFAULT_POLICY } from "./policy.mjs";
import { slicePackage } from "./slice.mjs";

const BUDGETS = { maxPackages: 8, maxFiles: 40, maxBytes: 200000, maxDepth: 6 };

export function resolveInsideSandbox(sandboxRoot, requested) {
  const root = path.resolve(sandboxRoot);
  const abs = path.resolve(root, requested);
  if (abs !== root && !abs.startsWith(root + path.sep)) {
    return { ok: false, reasons: ["sandbox-escape"] };
  }
  return { ok: true, abs };
}

export function executeOnHost() {
  return { ok: false, action: "refuse", executed: false, hostAccess: false, reasons: ["host-execution"] };
}

function lockFile(stateDir, name, version) {
  return path.join(stateDir, "sandbox", name, version);
}

export function lockInstall({ name, version, registryDir, stateDir }) {
  const pkgDir = path.join(registryDir, name);
  const manifestPath = path.join(pkgDir, "package.json");
  if (!fs.existsSync(manifestPath)) {
    return { ok: false, action: "hold", reasons: ["missing-package"], executed: false, hostAccess: false };
  }
  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
  if (manifest.version !== version) {
    return { ok: false, action: "hold", reasons: ["version-mismatch"], executed: false, hostAccess: false };
  }
  const dest = lockFile(stateDir, name, version);
  fs.rmSync(dest, { recursive: true, force: true });
  fs.cpSync(pkgDir, dest, { recursive: true });
  const record = {
    ok: true,
    action: "locked-install",
    name,
    version,
    sandbox: dest,
    mode: "locked",
    network: false,
    hostAccess: false,
    executed: false,
    scriptsExecuted: false,
    scriptsIgnored: hasLifecycleScripts(manifest),
  };
  fs.mkdirSync(dest, { recursive: true });
  fs.writeFileSync(path.join(dest, ".imm-lock.json"), `${JSON.stringify(record, null, 2)}\n`);
  return record;
}

export function bringSources({ name, version, fn, stateDir, now = new Date() }) {
  const sandbox = lockFile(stateDir, name, version);
  const inside = resolveInsideSandbox(path.join(stateDir, "sandbox"), path.join(name, version));
  if (!inside.ok) return { ok: false, ...inside, executed: false, hostAccess: false };
  if (!fs.existsSync(sandbox)) {
    return { ok: false, action: "hold", reasons: ["missing-sandbox"], executed: false, hostAccess: false };
  }
  const sliced = slicePackage(
    sandbox,
    { specifier: name, version, exports: [fn] },
    BUDGETS,
    DEFAULT_POLICY,
    now,
  );
  if (sliced.blocked) {
    return {
      ok: false,
      action: "hold",
      reasons: sliced.reasons,
      brought: [],
      omitted: sliced.omitted,
      executed: false,
      hostAccess: false,
    };
  }
  const id = `${name}@${version}#${fn}`;
  const out = path.join(stateDir, "brought", encodeURIComponent(id));
  fs.rmSync(out, { recursive: true, force: true });
  const brought = [];
  for (const file of sliced.files) {
    const from = resolveInsideSandbox(sandbox, file.path);
    if (!from.ok) {
      return { ok: false, ...from, executed: false, hostAccess: false };
    }
    const dest = path.join(out, file.path);
    fs.mkdirSync(path.dirname(dest), { recursive: true });
    const text = fs.readFileSync(from.abs);
    fs.writeFileSync(dest, text);
    brought.push({ path: file.path, sha256: sha256(text), bytes: text.length });
  }
  return {
    ok: true,
    action: "brought",
    id,
    brought,
    omitted: sliced.omitted,
    executed: false,
    hostAccess: false,
    bundler: "reachability",
  };
}
