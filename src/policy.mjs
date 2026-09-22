const TRUST_RANK = {
  none: 0,
  provenance: 1,
  "trusted-publisher": 2,
};

const LIFECYCLE = new Set(["preinstall", "install", "postinstall", "prepare"]);
const EXACT_VERSION = /^\d+\.\d+\.\d+$/;

export const DEFAULT_POLICY = {
  minimumReleaseAgeMinutes: 2880,
};

export function hasLifecycleScripts(manifest) {
  const scripts = manifest?.scripts ?? {};
  return Object.keys(scripts).some((name) => LIFECYCLE.has(name) && scripts[name]);
}

export function judgeUpdate(proposal, policy = DEFAULT_POLICY, now = new Date()) {
  const reasons = [];
  const version = String(proposal.version ?? "");
  const range = proposal.range === undefined ? version : String(proposal.range);
  if (!EXACT_VERSION.test(version) || range !== version) reasons.push("floating-range");
  if (!proposal.integrity) reasons.push("missing-integrity");
  if (proposal.lifecycleScripts) reasons.push("lifecycle-script");

  const published = Date.parse(proposal.publishedAt);
  if (Number.isNaN(published)) reasons.push("missing-publish-time");
  else if ((now.getTime() - published) / 60000 < policy.minimumReleaseAgeMinutes) {
    reasons.push("too-fresh");
  }

  if (TRUST_RANK[proposal.trust] === undefined) reasons.push("unknown-trust");
  if (proposal.previousTrust !== undefined) {
    const previous = TRUST_RANK[proposal.previousTrust];
    const next = TRUST_RANK[proposal.trust] ?? -1;
    if (previous === undefined || next < previous) reasons.push("trust-downgrade");
  }

  return { accept: reasons.length === 0, reasons };
}
