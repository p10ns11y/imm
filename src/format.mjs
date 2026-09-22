export function formatVerdict(verdict) {
  if (verdict.accept) return "accept";
  return `reject ${verdict.reasons.join(" ")}`;
}

export function formatSlice(pkg) {
  const lines = [`${pkg.name}@${pkg.version}`, pkg.blocked ? "blocked" : "accept"];
  if (pkg.reasons.length) lines.push(`reasons ${pkg.reasons.join(" ")}`);
  lines.push("keep");
  if (pkg.files.length === 0) lines.push("  (none)");
  for (const file of pkg.files) lines.push(`  ${file.path}`);
  lines.push("omit");
  if (pkg.omitted.length === 0) lines.push("  (none)");
  for (const rel of pkg.omitted) lines.push(`  ${rel}`);
  return lines.join("\n");
}
