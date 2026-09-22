import fs from "node:fs";
import path from "node:path";
import { sha256 } from "./hash.mjs";
import { DEFAULT_POLICY } from "./policy.mjs";
import { slicePackage } from "./slice.mjs";

const BUDGETS = { maxPackages: 8, maxFiles: 40, maxBytes: 200000, maxDepth: 6 };

function recordPath(stateDir, id) {
  return path.join(stateDir, "records", `${encodeURIComponent(id)}.json`);
}

function readRecord(stateDir, id) {
  const file = recordPath(stateDir, id);
  if (!fs.existsSync(file)) return null;
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

function writeRecord(stateDir, record) {
  const file = recordPath(stateDir, record.id);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, `${JSON.stringify(record, null, 2)}\n`);
  return record;
}

function useId(name, version, fn) {
  return fn ? `${name}@${version}#${fn}` : `${name}@${version}`;
}

export function wholeFileDiff(before, after, filePath) {
  if (before === after) return "";
  const removed = before.split("\n").map((line) => `-${line}`);
  const added = after.split("\n").map((line) => `+${line}`);
  return [`--- a/${filePath}`, `+++ b/${filePath}`, "@@", ...removed, ...added].join("\n");
}

function copyTree(fromDir, toDir) {
  fs.mkdirSync(toDir, { recursive: true });
  const files = [];
  const walk = (dir) => {
    for (const ent of fs.readdirSync(dir, { withFileTypes: true })) {
      const abs = path.join(dir, ent.name);
      if (ent.isDirectory()) walk(abs);
      else if (ent.isFile()) {
        const rel = path.relative(fromDir, abs).split(path.sep).join("/");
        const dest = path.join(toDir, rel);
        fs.mkdirSync(path.dirname(dest), { recursive: true });
        fs.copyFileSync(abs, dest);
        const text = fs.readFileSync(dest);
        files.push({ path: rel, sha256: sha256(text), bytes: text.length });
      }
    }
  };
  walk(fromDir);
  files.sort((a, b) => a.path.localeCompare(b.path));
  return files;
}

export function installSource({ name, version, registryDir, stateDir }) {
  const pkgDir = path.join(registryDir, name);
  const manifestPath = path.join(pkgDir, "package.json");
  if (!fs.existsSync(manifestPath)) {
    return { ok: false, action: "hold", reasons: ["missing-package"], id: useId(name, version) };
  }
  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
  if (manifest.version !== version) {
    return { ok: false, action: "hold", reasons: ["version-mismatch"], id: useId(name, version) };
  }
  const dest = path.join(stateDir, "installed", name, version);
  fs.rmSync(dest, { recursive: true, force: true });
  const files = copyTree(pkgDir, dest);
  const record = writeRecord(stateDir, {
    id: useId(name, version),
    kind: "direct",
    action: "install-source",
    name,
    version,
    files,
    reasons: ["direct", "whole-source"],
  });
  return { ok: true, ...record };
}

export function extractUse({ name, version, fn, registryDir, stateDir, now = new Date() }) {
  const id = useId(name, version, fn);
  const pkgDir = path.join(registryDir, name);
  const sliced = slicePackage(
    pkgDir,
    { specifier: name, version, exports: [fn] },
    BUDGETS,
    DEFAULT_POLICY,
    now,
  );
  if (sliced.blocked) {
    return { ok: false, id, action: "hold", reasons: sliced.reasons };
  }
  const root = path.join(stateDir, "extracts", encodeURIComponent(id));
  fs.rmSync(root, { recursive: true, force: true });
  for (const file of sliced.files) {
    const dest = path.join(root, file.path);
    fs.mkdirSync(path.dirname(dest), { recursive: true });
    fs.writeFileSync(dest, file.text);
  }
  const body = sliced.files.map((file) => file.text).join("\n");
  const record = writeRecord(stateDir, {
    id,
    kind: "use",
    action: "hold",
    name,
    version,
    fn,
    files: sliced.files.map(({ path: filePath, sha256: sum, bytes }) => ({ path: filePath, sha256: sum, bytes })),
    omitted: sliced.omitted,
    originalText: body,
    text: body,
    hash: sha256(body),
    audit: null,
    agentModified: false,
    diff: "",
    reasons: ["needs-audit"],
  });
  return { ok: false, ...record };
}

export function auditUse({ id, by, at, stateDir }) {
  const current = readRecord(stateDir, id);
  if (!current) return { ok: false, id, action: "hold", reasons: ["missing-extract"] };
  if (!current.hash) return { ok: false, id, action: "hold", reasons: ["needs-hash"] };
  const audit = { by, at };
  const reasons = ["per-usage", "audited", "hash"];
  if (current.diff) reasons.push("agent-diff");
  if (current.agentModified && !current.diff) reasons.splice(0, reasons.length, "needs-agent-diff");
  const ready = reasons[0] !== "needs-agent-diff";
  const record = writeRecord(stateDir, {
    ...current,
    audit,
    action: ready ? "extract" : "hold",
    reasons,
  });
  return { ok: ready, ...record };
}

export function overwriteUse({ id, text, stateDir }) {
  const current = readRecord(stateDir, id);
  if (!current) return { ok: false, id, action: "hold", reasons: ["missing-extract"] };
  const filePath = current.files?.[0]?.path ?? `${current.fn ?? "use"}.js`;
  const diff = wholeFileDiff(current.originalText ?? "", text, filePath);
  const root = path.join(stateDir, "extracts", encodeURIComponent(id));
  if (current.files?.[0]) {
    const dest = path.join(root, current.files[0].path);
    fs.mkdirSync(path.dirname(dest), { recursive: true });
    fs.writeFileSync(dest, text);
  }
  const reasons = ["per-usage", "hash", "agent-diff"];
  if (current.audit) reasons.splice(1, 0, "audited");
  else reasons.splice(0, reasons.length, "needs-audit");
  const record = writeRecord(stateDir, {
    ...current,
    text,
    hash: current.hash,
    currentHash: sha256(text),
    agentModified: diff.length > 0,
    diff,
    action: current.audit ? "extract" : "hold",
    reasons,
  });
  return { ok: Boolean(current.audit), ...record };
}
