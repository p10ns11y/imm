use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::agent::{advise, AdviseInput};
use crate::error::ImmError;
use crate::plan::{approve_plan, plan_install, verify_lock, write_plan};
use crate::policy::{format_verdict, judge_update, Proposal};
use crate::slice::{format_slice, slice_package, Budgets};
use crate::store::{decide_retention, format_retention, Module};
use crate::timeutil::parse_unix_ms;

pub fn run_package_flow(root: &Path) -> Result<String, ImmError> {
    let now = parse_unix_ms("2026-09-22T12:00:00.000Z").ok_or_else(|| ImmError::new("CLOCK", "bad fixture clock"))?;
    let mut lines = Vec::new();
    let registry = root.join("fixtures/registry");
    let proposals = root.join("fixtures/proposals");
    let budgets = Budgets { max_packages: 4, max_files: 20, max_bytes: 100_000, max_depth: 6 };

    lines.push("1. Release check. An agent may propose. The check still rejects.".to_string());
    for name in ["ok.json", "fresh.json", "trust-downgrade.json", "lifecycle.json", "floating.json"] {
        let text = fs::read_to_string(proposals.join(name)).map_err(|err| ImmError::new("IO", &err.to_string()))?;
        let value: Value = serde_json::from_str(&text).map_err(|err| ImmError::new("IO", &err.to_string()))?;
        let verdict = judge_update(&Proposal::from_value(&value), now);
        lines.push(format!("  {name} {}", format_verdict(&verdict)));
    }

    lines.push(String::new());
    lines.push("2. Reachable fill. Named exports keep the files they need.".to_string());
    let pure = slice_package(&registry.join("pure-slug"), "pure-slug", "1.0.0", &["slugify"], budgets, now);
    for line in format_slice(&pure).lines() {
        lines.push(format!("  {line}"));
    }

    lines.push(String::new());
    lines.push("3. A reached forbidden call blocks the package. The bytes stay out.".to_string());
    let shell = slice_package(&registry.join("shell-out"), "shell-out", "1.0.0", &["slugify"], budgets, now);
    for line in format_slice(&shell).lines() {
        lines.push(format!("  {line}"));
    }

    lines.push(String::new());
    lines.push("4. Install writes throw-stubs. Approval is a second principal.".to_string());
    let manifest_text = fs::read_to_string(root.join("fixtures/agent-manifest.json")).map_err(|err| ImmError::new("IO", &err.to_string()))?;
    let manifest: Value = serde_json::from_str(&manifest_text).map_err(|err| ImmError::new("IO", &err.to_string()))?;
    let planned = plan_install(&manifest, &registry, now);
    let out_dir = std::env::temp_dir().join(format!("imm-package-flow-{}", std::process::id()));
    write_plan(&out_dir, &planned)?;
    let stub = fs::read_to_string(out_dir.join("stubs/pure-slug/slugify.js")).unwrap_or_default();
    if let Some(message) = stub.split("imm: ").nth(1).and_then(|rest| rest.split('"').next()) {
        lines.push(format!("  stub imm: {message}"));
    }
    match approve_plan(&out_dir, "agent") {
        Err(error) => lines.push(format!("  agent approve {}", error.code)),
        Ok(()) => lines.push("  agent approve accepted".to_string()),
    }
    approve_plan(&out_dir, "human")?;
    let vendor = out_dir.join("vendor/pure-slug/src/index.js");
    if vendor.is_file() {
        lines.push("  human approve wrote vendor/pure-slug; the package was not executed".to_string());
    }
    let problems = verify_lock(&out_dir)?;
    lines.push(format!("  verify reads the vendor lock {}", if problems.is_empty() { "ok".to_string() } else { problems.join(" ") }));

    lines.push(String::new());
    lines.push("5. Install only the direct dependency, as whole source. Other uses stay extracts.".to_string());
    lines.push(format!("  {}", format_retention(&decide_retention(&module_value(r#"{"name":"slugify","kind":"trivial"}"#)?))));
    lines.push(format!(
        "  {}",
        format_retention(&decide_retention(&module_value(
            r#"{"name":"sharp","kind":"direct","version":"0.33.5","subdeps":["color@4.2.3"]}"#
        )?))
    ));
    lines.push(format!(
        "  {}",
        format_retention(&decide_retention(&module_value(
            r#"{"name":"color","kind":"use","version":"4.2.3","fn":"convert","hash":"sha256-abc","audit":{"by":"agent","at":"2026-08-01"}}"#
        )?))
    ));
    let sharp = module_value(r#"{"name":"sharp","kind":"direct","version":"0.33.5"}"#)?;
    let step = advise(AdviseInput { proposal: None, now_ms: now, module: Some(&sharp), slice: None });
    lines.push(format!("  agent next {} {}", step.next.do_, step.next.target));
    lines.push("  Library authors ship that extract inside the package.".to_string());
    lines.push(format!(
        "  {}",
        format_retention(&decide_retention(&module_value(
            r#"{"name":"slug-kit","kind":"library","nodeModules":true,"subdeps":["slugify@1.0.0"],"vendored":[]}"#
        )?))
    ));
    lines.push(format!(
        "  {}",
        format_retention(&decide_retention(&module_value(
            r#"{"name":"slug-kit","kind":"library","nodeModules":false,"subdeps":["slugify@1.0.0"],"vendored":[{"id":"slugify@1.0.0","files":["vendor/slugify.js"],"audit":{"by":"author","at":"2026-08-01"}}]}"#
        )?))
    ));
    lines.push(String::new());
    lines.push(format!("state {}", out_dir.display()));
    Ok(lines.join("\n"))
}

fn module_value(text: &str) -> Result<Module, ImmError> {
    let value = serde_json::from_str(text).map_err(|err| ImmError::new("IO", &err.to_string()))?;
    Module::from_value(value).map_err(|err| ImmError::new("IO", &err))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_flow_prints_the_check_the_omit_and_does_not_run_the_package() {
        let text = run_package_flow(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        assert!(text.contains("fresh.json reject too-fresh"));
        assert!(text.contains("omit\n  src/cli.js") || text.contains("omit\n    src/cli.js"));
        assert!(text.contains("child_process:src/index.js"));
        assert!(text.contains("AGENT_APPROVER"));
        assert!(text.contains("the package was not executed"));
        assert!(!text.contains("hello-world"));
    }
}
