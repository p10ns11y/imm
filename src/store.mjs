function idOf(mod) {
  return mod.version ? `${mod.name}@${mod.version}` : mod.name;
}

function asAudit(record) {
  if (!record) return null;
  if (record.audit) return record.audit;
  if (record.by || record.at) return record;
  return null;
}

export function decideRetention(mod, audits = {}) {
  if (mod.kind === "trivial" || mod.kind === "sub" || mod.kind === "use") {
    const name = mod.fn ? `${idOf(mod)}#${mod.fn}` : idOf(mod);
    const record = audits[name] ?? audits[idOf(mod)] ?? {};
    const hash = mod.hash ?? record.hash;
    const audit = mod.audit ?? asAudit(record);
    const diff = mod.diff ?? record.diff;
    const modified = Boolean(mod.agentModified ?? record.agentModified);
    const reasons = [];
    if (!hash) reasons.push("needs-hash");
    if (!audit) reasons.push("needs-audit");
    if (modified && !diff) reasons.push("needs-agent-diff");
    if (reasons.length > 0) return { name, action: "hold", reasons };
    const why = ["per-usage", "audited", "hash"];
    if (diff) why.push("agent-diff");
    return { name, action: "extract", reasons: why };
  }

  if (mod.kind === "library") {
    const reasons = [];
    if (mod.nodeModules !== false) reasons.push("needs-no-node-modules");
    const vendored = mod.vendored ?? [];
    for (const item of vendored) {
      if (!item.audit) reasons.push(`vendored-unaudited:${item.id}`);
      if (!item.files || item.files.length === 0) reasons.push(`not-extracted:${item.id}`);
    }
    const ready = new Set(
      vendored
        .filter((item) => item.audit && item.files && item.files.length > 0)
        .map((item) => item.id),
    );
    for (const dep of mod.subdeps ?? []) {
      if (!ready.has(dep)) reasons.push(`needs-vendor:${dep}`);
    }
    if (reasons.length > 0) return { name: mod.name, action: "hold", reasons };
    return {
      name: mod.name,
      action: "ship",
      reasons: ["vendored", "audited", "extracted", "no-node-modules"],
    };
  }

  if (mod.kind === "direct") {
    return {
      name: idOf(mod),
      action: "install-source",
      reasons: ["direct", "whole-source"],
    };
  }

  return { name: mod.name ?? "unknown", action: "hold", reasons: ["unknown-kind"] };
}

export function formatRetention(decision) {
  if (decision.reasons.length === 0) return `${decision.name} ${decision.action}`;
  return `${decision.name} ${decision.action} ${decision.reasons.join(" ")}`;
}
