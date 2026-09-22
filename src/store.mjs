function idOf(mod) {
  return mod.version ? `${mod.name}@${mod.version}` : mod.name;
}

export function decideRetention(mod, audits = {}) {
  if (mod.kind === "trivial") {
    return {
      name: mod.name,
      action: "local",
      reasons: ["no-publish", "no-download", "no-install"],
    };
  }

  if (mod.kind === "sub") {
    const name = idOf(mod);
    if (!audits[name]) return { name, action: "hold", reasons: ["needs-audit"] };
    return { name, action: "store", reasons: ["audited"] };
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
    if (!mod.hard || !mod.matured) {
      return {
        name: mod.name,
        action: "local",
        reasons: ["not-hard-or-matured", "no-publish", "no-download", "no-install"],
      };
    }
    const missing = (mod.subdeps ?? []).filter((dep) => !audits[dep]);
    if (missing.length > 0) {
      return {
        name: mod.name,
        action: "hold",
        reasons: missing.map((dep) => `subdep-unaudited:${dep}`),
      };
    }
    return {
      name: mod.name,
      action: "store",
      reasons: ["hard", "matured", "direct", "subdeps-audited"],
    };
  }

  return { name: mod.name ?? "unknown", action: "hold", reasons: ["unknown-kind"] };
}

export function formatRetention(decision) {
  if (decision.reasons.length === 0) return `${decision.name} ${decision.action}`;
  return `${decision.name} ${decision.action} ${decision.reasons.join(" ")}`;
}
