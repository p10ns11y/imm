#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { runDemo } from "../src/demo.mjs";
import { approvePlan, planInstall, verifyLock, writePlan } from "../src/ecosystem.mjs";
import { ImmError } from "../src/errors.mjs";
import { formatSlice, formatVerdict } from "../src/format.mjs";
import { DEFAULT_POLICY, judgeUpdate } from "../src/policy.mjs";
import { slicePackage } from "../src/slice.mjs";
import { decideRetention, formatRetention } from "../src/store.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function parseArgs(argv) {
  const [cmd, ...rest] = argv;
  const opts = {};
  for (let i = 0; i < rest.length; i += 1) {
    const arg = rest[i];
    if (!arg.startsWith("--")) continue;
    const key = arg.slice(2);
    const next = rest[i + 1];
    if (next === undefined || next.startsWith("--")) opts[key] = "true";
    else {
      opts[key] = next;
      i += 1;
    }
  }
  return { cmd, opts };
}

function nowFrom(opts) {
  return opts.now ? new Date(opts.now) : new Date();
}

function printUsage() {
  return [
    "imm judge --proposal <file.json> [--now <iso>]",
    "imm slice --package <dir> --version <x.y.z> --exports <a,b> [--now <iso>]",
    "imm plan --manifest <file.json> --registry <dir> --out <dir> [--now <iso>]",
    "imm approve --state <dir> --approver human|agent",
    "imm verify --state <dir>",
    "imm store --module <file.json> [--audits <file.json>]",
    "imm demo",
  ].join("\n");
}

export async function main(argv) {
  const { cmd, opts } = parseArgs(argv);
  if (!cmd || cmd === "help" || opts.help) return { code: 0, text: printUsage() };

  if (cmd === "demo") {
    const text = await runDemo(root);
    return { code: 0, text };
  }

  if (cmd === "judge") {
    const proposal = JSON.parse(fs.readFileSync(opts.proposal, "utf8"));
    const verdict = judgeUpdate(proposal, DEFAULT_POLICY, nowFrom(opts));
    return { code: verdict.accept ? 0 : 2, text: formatVerdict(verdict) };
  }

  if (cmd === "slice") {
    const pkg = slicePackage(
      opts.package,
      {
        specifier: path.basename(opts.package),
        version: opts.version,
        exports: String(opts.exports ?? "").split(",").filter(Boolean),
      },
      { maxDepth: Number(opts.maxDepth ?? 6), maxFiles: 40, maxBytes: 200000, maxPackages: 8 },
      DEFAULT_POLICY,
      nowFrom(opts),
    );
    return { code: pkg.blocked ? 2 : 0, text: formatSlice(pkg) };
  }

  if (cmd === "plan") {
    const manifest = JSON.parse(fs.readFileSync(opts.manifest, "utf8"));
    const planned = planInstall(manifest, opts.registry, DEFAULT_POLICY, nowFrom(opts));
    writePlan(opts.out, planned);
    const text = planned.blocked
      ? `blocked ${[...planned.reasons, ...planned.packages.flatMap((pkg) => pkg.reasons)].join(" ")}`
      : `staged ${opts.out}`;
    return { code: planned.blocked ? 2 : 0, text };
  }

  if (cmd === "approve") {
    approvePlan(opts.state, opts.approver);
    return { code: 0, text: `approved ${opts.state}` };
  }

  if (cmd === "verify") {
    const check = verifyLock(opts.state);
    return { code: check.ok ? 0 : 2, text: check.ok ? "ok" : check.problems.join("\n") };
  }

  if (cmd === "store") {
    const mod = JSON.parse(fs.readFileSync(opts.module, "utf8"));
    const audits = opts.audits ? JSON.parse(fs.readFileSync(opts.audits, "utf8")) : {};
    const decision = decideRetention(mod, audits);
    return { code: decision.action === "hold" ? 2 : 0, text: formatRetention(decision) };
  }

  return { code: 2, text: printUsage() };
}

const isDirect = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isDirect) {
  main(process.argv.slice(2))
    .then(({ code, text }) => {
      process.stdout.write(`${text}\n`);
      process.exitCode = code;
    })
    .catch((error) => {
      if (error instanceof ImmError) {
        process.stderr.write(`${error.code} ${error.message}\n`);
        process.exitCode = 2;
        return;
      }
      process.stderr.write(`${error.stack ?? error.message}\n`);
      process.exitCode = 1;
    });
}
