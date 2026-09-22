import fs from "node:fs";
import path from "node:path";
import { sha256 } from "./hash.mjs";
import { hasLifecycleScripts, judgeUpdate } from "./policy.mjs";

// Lexical tripwires only. A comment that mentions one of these strings blocks the file.
const FORBIDDEN = [
  ["child_process", /child_process/],
  ["dynamic-require", /require\s*\(/],
  ["eval", /\beval\s*\(/],
  ["function-constructor", /\bFunction\s*\(/],
  ["network-url", /https?:\/\//],
  ["process-binding", /process\.binding/],
];

const EXPORT_NAME = /^[A-Za-z_$][\w$]*$/;

function relPosix(root, abs) {
  return path.relative(root, abs).split(path.sep).join("/");
}

function resolveInside(pkgDir, fromAbs, rel) {
  const abs = path.normalize(path.join(path.dirname(fromAbs), rel));
  const root = path.normalize(pkgDir + path.sep);
  if (abs !== path.normalize(pkgDir) && !abs.startsWith(root)) return { error: "path-escape" };
  if (!fs.existsSync(abs) || !fs.statSync(abs).isFile()) return { error: "missing-file" };
  return { abs };
}

function parseModule(text) {
  const local = new Set();
  const reexports = new Map();
  const imports = [];
  const reasons = [];

  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim();
    if (!line || line.startsWith("//")) continue;
    if (/^export\s+\*\s+from\b/.test(line)) reasons.push("star-reexport");
    if (line.startsWith("import ") && !/from\s+["']/.test(line) && !/^import\s+["']/.test(line)) {
      reasons.push("parse-limit");
    }

    const side = line.match(/^import\s+["'](\.[^"']+)["']/);
    if (side) imports.push(side[1]);
    const imported = line.match(/^import\s+.+\s+from\s+["'](\.[^"']+)["']/);
    if (imported) imports.push(imported[1]);

    const rex = line.match(/^export\s+\{\s*([^}]+)\s*\}\s+from\s+["'](\.[^"']+)["']/);
    if (rex) {
      for (const part of rex[1].split(",")) {
        const bits = part.trim().split(/\s+as\s+/);
        if (!bits[0]) continue;
        const exported = (bits[1] ?? bits[0]).trim();
        reexports.set(exported, { rel: rex[2], sourceName: bits[0].trim() });
      }
    }

    const fn = line.match(/^export\s+function\s+([A-Za-z_$][\w$]*)/);
    if (fn) local.add(fn[1]);
    const cn = line.match(/^export\s+const\s+([A-Za-z_$][\w$]*)/);
    if (cn) local.add(cn[1]);
  }

  return { local, reexports, imports, reasons };
}

function forbiddenReasons(text, rel) {
  const hits = [];
  for (const [id, pattern] of FORBIDDEN) {
    if (pattern.test(text)) hits.push(`${id}:${rel}`);
  }
  return hits;
}

function listJs(pkgDir) {
  const out = [];
  const walk = (dir) => {
    for (const ent of fs.readdirSync(dir, { withFileTypes: true })) {
      const abs = path.join(dir, ent.name);
      if (ent.isDirectory()) walk(abs);
      else if (ent.name.endsWith(".js")) out.push(relPosix(pkgDir, abs));
    }
  };
  walk(pkgDir);
  return out.sort();
}

function entryRel(manifest) {
  const field = manifest.exports?.["."];
  if (typeof field === "string") return field;
  if (field && typeof field === "object") return field.import ?? field.default ?? "./src/index.js";
  return "./src/index.js";
}

function readCached(cache, abs) {
  if (!cache.has(abs)) {
    const text = fs.readFileSync(abs, "utf8");
    cache.set(abs, { text, ...parseModule(text) });
  }
  return cache.get(abs);
}

function pushFile(pkgDir, abs, depth, files, seen) {
  if (seen.has(abs)) return [];
  seen.add(abs);
  const text = fs.readFileSync(abs, "utf8");
  const rel = relPosix(pkgDir, abs);
  files.push({
    path: rel,
    depth,
    bytes: Buffer.byteLength(text),
    sha256: sha256(text),
    text,
  });
  return forbiddenReasons(text, rel);
}

function walkImports(pkgDir, cache, startAbs, startDepth, maxDepth, seen, files) {
  const reasons = [];
  const queued = new Set(seen);
  const queue = [{ abs: startAbs, depth: startDepth }];
  while (queue.length > 0) {
    const current = queue.shift();
    const mod = readCached(cache, current.abs);
    reasons.push(...mod.reasons);
    for (const rel of mod.imports) {
      const next = resolveInside(pkgDir, current.abs, rel);
      if (next.error) {
        reasons.push(next.error);
        continue;
      }
      const childDepth = current.depth + 1;
      if (childDepth > maxDepth) {
        reasons.push("budget-depth");
        continue;
      }
      if (queued.has(next.abs)) continue;
      queued.add(next.abs);
      reasons.push(...pushFile(pkgDir, next.abs, childDepth, files, seen));
      queue.push({ abs: next.abs, depth: childDepth });
    }
  }
  return reasons;
}

function traceExport(pkgDir, cache, entryAbs, exportName, maxDepth, seen, files) {
  const reasons = [];
  let abs = entryAbs;
  let name = exportName;
  let depth = 0;
  const guard = new Set();

  while (abs) {
    const mark = `${abs}#${name}`;
    if (guard.has(mark)) {
      reasons.push("cycle");
      break;
    }
    guard.add(mark);
    if (depth > maxDepth) {
      reasons.push("budget-depth");
      break;
    }

    const mod = readCached(cache, abs);
    reasons.push(...mod.reasons);
    reasons.push(...pushFile(pkgDir, abs, depth, files, seen));

    if (mod.local.has(name)) {
      reasons.push(...walkImports(pkgDir, cache, abs, depth, maxDepth, seen, files));
      break;
    }

    const hop = mod.reexports.get(name);
    if (!hop) {
      reasons.push(`missing-export:${exportName}`);
      break;
    }
    const next = resolveInside(pkgDir, abs, hop.rel);
    if (next.error) {
      reasons.push(next.error);
      break;
    }
    abs = next.abs;
    name = hop.sourceName;
    depth += 1;
  }

  return reasons;
}

function blank(need, reasons) {
  return {
    name: need.specifier,
    version: need.version ?? "",
    exports: need.exports ?? [],
    blocked: true,
    reasons,
    files: [],
    omitted: [],
  };
}

export function slicePackage(pkgDir, need, budgets, policy, now) {
  if (!fs.existsSync(path.join(pkgDir, "package.json"))) return blank(need, ["missing-package"]);

  const manifest = JSON.parse(fs.readFileSync(path.join(pkgDir, "package.json"), "utf8"));
  const reasons = [];
  const exportNames = need.exports ?? [];
  if (exportNames.length === 0) reasons.push("missing-export");
  for (const name of exportNames) {
    if (!EXPORT_NAME.test(name)) reasons.push(`bad-export-name:${name}`);
  }
  if (need.version !== manifest.version) reasons.push("version-mismatch");

  const verdict = judgeUpdate(
    {
      name: manifest.name,
      version: need.version,
      range: need.version,
      publishedAt: manifest.publishedAt,
      trust: manifest.trust,
      previousTrust: manifest.previousTrust,
      lifecycleScripts: hasLifecycleScripts(manifest),
      integrity: manifest.integrity,
    },
    policy,
    now,
  );
  reasons.push(...verdict.reasons);

  const files = [];
  const seen = new Set();
  const cache = new Map();
  const entryAbs = path.normalize(path.join(pkgDir, entryRel(manifest)));
  if (!fs.existsSync(entryAbs)) reasons.push("missing-file");
  else if (reasons.every((reason) => !reason.startsWith("bad-export-name"))) {
    for (const name of exportNames) {
      reasons.push(...traceExport(pkgDir, cache, entryAbs, name, budgets.maxDepth, seen, files));
    }
  }

  const kept = new Set(files.map((file) => file.path));
  const omitted = listJs(pkgDir).filter((rel) => !kept.has(rel));
  const unique = [...new Set(reasons)];
  return {
    name: need.specifier,
    version: need.version,
    exports: exportNames,
    blocked: unique.length > 0,
    reasons: unique,
    files,
    omitted,
  };
}
