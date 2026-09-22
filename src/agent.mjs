import { judgeUpdate } from "./policy.mjs";
import { decideRetention } from "./store.mjs";

function ids(reasons, prefix) {
  return reasons.filter((reason) => reason.startsWith(prefix)).map((reason) => reason.slice(prefix.length));
}

function retentionAdvice(decision) {
  const why = decision.reasons;
  if (decision.action === "install-source") {
    return {
      ok: true,
      verdict: "install-source",
      next: { do: "install-whole-source", target: decision.name },
      skip: ["subdependencies", "node_modules"],
      later: [],
      why,
    };
  }
  if (decision.action === "extract") {
    return {
      ok: true,
      verdict: "extract",
      next: { do: "keep-extract", target: decision.name },
      skip: ["install"],
      later: [],
      why,
    };
  }
  if (why.includes("needs-hash")) {
    const later = [];
    if (why.includes("needs-audit")) later.push("audit");
    if (why.includes("needs-agent-diff")) later.push("record the agent diff");
    return {
      ok: false,
      verdict: "extract",
      next: { do: "record-hash", target: decision.name },
      skip: ["install"],
      later,
      why,
    };
  }
  if (decision.action === "local") {
    return {
      ok: true,
      verdict: "maintain-local",
      next: { do: "keep-source", target: decision.name },
      skip: ["publish", "download", "install"],
      later: [],
      why,
    };
  }
  if (decision.action === "ship") {
    return {
      ok: true,
      verdict: "ship",
      next: { do: "none", target: decision.name },
      skip: ["node_modules", "install"],
      later: [],
      why,
    };
  }
  if (decision.action === "store") {
    return {
      ok: true,
      verdict: "store",
      next: { do: "store", target: decision.name },
      skip: ["install"],
      later: [],
      why,
    };
  }

  const unaudited = ids(why, "subdep-unaudited:");
  if (unaudited.length > 0) {
    return {
      ok: false,
      verdict: "prepare-audit",
      next: { do: "audit", target: unaudited[0] },
      skip: ["store", "publish", "download", "install"],
      later: unaudited.slice(1).map((id) => `audit ${id}`),
      why,
    };
  }
  if (why.includes("needs-audit")) {
    const later = why.includes("needs-agent-diff") ? ["record the agent diff"] : [];
    return {
      ok: false,
      verdict: "prepare-audit",
      next: { do: "audit", target: decision.name },
      skip: ["install"],
      later,
      why,
    };
  }
  if (why.includes("needs-agent-diff")) {
    return {
      ok: false,
      verdict: "extract",
      next: { do: "record-agent-diff", target: decision.name },
      skip: ["install"],
      later: [],
      why,
    };
  }

  const missingVendor = ids(why, "needs-vendor:");
  if (missingVendor.length > 0) {
    const later = missingVendor.slice(1).map((id) => `vendor ${id}`);
    if (why.includes("needs-no-node-modules")) later.push("set nodeModules false");
    return {
      ok: false,
      verdict: "vendor",
      next: { do: "vendor-extract", target: missingVendor[0] },
      skip: ["node_modules", "install"],
      later,
      why,
    };
  }

  const vendoredUnaudited = ids(why, "vendored-unaudited:");
  if (vendoredUnaudited.length > 0) {
    return {
      ok: false,
      verdict: "prepare-audit",
      next: { do: "audit", target: vendoredUnaudited[0] },
      skip: ["ship", "install"],
      later: vendoredUnaudited.slice(1).map((id) => `audit ${id}`),
      why,
    };
  }

  const notExtracted = ids(why, "not-extracted:");
  if (notExtracted.length > 0) {
    return {
      ok: false,
      verdict: "vendor",
      next: { do: "extract-files", target: notExtracted[0] },
      skip: ["ship", "install"],
      later: notExtracted.slice(1).map((id) => `extract ${id}`),
      why,
    };
  }

  if (why.includes("needs-no-node-modules")) {
    return {
      ok: false,
      verdict: "vendor",
      next: { do: "set-node-modules-false", target: decision.name },
      skip: ["node_modules", "install"],
      later: [],
      why,
    };
  }

  return {
    ok: false,
    verdict: "hold",
    next: { do: "stop", target: decision.name },
    skip: ["store", "install"],
    later: [],
    why,
  };
}

export function advise({ proposal, policy, now, mod, audits = {}, slice } = {}) {
  if (proposal) {
    const verdict = judgeUpdate(proposal, policy, now);
    if (!verdict.accept) {
      return {
        ok: false,
        verdict: "reject-update",
        next: { do: "keep-pinned", target: `${proposal.name}@${proposal.version}` },
        skip: ["bump", "publish", "download", "install"],
        later: [],
        why: verdict.reasons,
      };
    }
  }

  if (slice?.blocked) {
    return {
      ok: false,
      verdict: "refuse-extract",
      next: { do: "do-not-vendor", target: slice.name },
      skip: ["vendor", "install"],
      later: [],
      why: slice.reasons,
    };
  }

  if (mod) return retentionAdvice(decideRetention(mod, audits));

  if (slice && slice.omitted?.length > 0) {
    return {
      ok: true,
      verdict: "extract",
      next: { do: "keep-reached-files", target: slice.name },
      drop: slice.omitted,
      skip: ["install"],
      later: [],
      why: ["unused-files-dropped"],
    };
  }

  if (proposal) {
    return {
      ok: true,
      verdict: "accept-update",
      next: { do: "read-the-diff", target: `${proposal.name}@${proposal.version}` },
      skip: ["install"],
      later: [],
      why: ["release-check-passed"],
    };
  }

  return {
    ok: false,
    verdict: "needs-input",
    next: { do: "provide-module-or-proposal", target: "" },
    skip: [],
    later: [],
    why: ["missing-input"],
  };
}
