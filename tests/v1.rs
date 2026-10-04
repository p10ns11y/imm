use std::fs;
use std::path::PathBuf;
use std::process::Command;

use imm::records::{audit_use, extract_use, execute_on_host, install_source};
use imm::timeutil::parse_unix_ms;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn now() -> i64 {
    parse_unix_ms("2026-09-22T12:00:00.000Z").unwrap()
}

fn state(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("imm-v1-{name}-{}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn install_runs_the_release_check_and_does_not_execute() {
    let fresh = state("fresh");
    let held = install_source(&root().join("fixtures/registry/fresh-slug"), "9.9.9", &fresh, now()).unwrap();
    assert_eq!(held.action, "hold");
    assert!(held.reasons.iter().any(|reason| reason == "too-fresh"));
    assert!(!held.executed);
    assert!(!fresh.join("installed").exists());

    let dir = state("pure");
    let record = install_source(&root().join("fixtures/registry/pure-slug"), "1.0.0", &dir, now()).unwrap();
    assert_eq!(record.action, "install-source");
    assert!(!record.executed);
    assert!(dir.join("installed/pure-slug/1.0.0/src/cli.js").is_file());
    assert!(dir.join("records").join("pure-slug@1.0.0.json").is_file());

    let side = state("side");
    let copied = install_source(&root().join("fixtures/registry/side-effect"), "1.0.0", &side, now()).unwrap();
    assert!(!copied.executed);
    let boom = fs::read_to_string(side.join("installed/side-effect/1.0.0/src/boom.js")).unwrap();
    assert!(boom.contains("executed during scan"));
}

#[test]
fn extract_then_audit_is_one_record() {
    let dir = state("extract");
    let extracted = extract_use(
        &root().join("fixtures/registry/pure-slug"),
        "pure-slug",
        "1.0.0",
        "slugify",
        &dir,
        now(),
    )
    .unwrap();
    assert_eq!(extracted.action, "hold");
    assert_eq!(extracted.reasons, ["needs-audit"]);
    assert!(!extracted.hash.is_empty());
    assert!(extracted.audit.is_none());
    assert!(!extracted.executed);
    assert!(extracted.files.iter().any(|file| file.path == "src/slugify.js"));
    assert!(!extracted.files.iter().any(|file| file.path == "src/cli.js"));

    let audited = audit_use(&dir, &extracted.id, "human", "2026-09-22T12:00:00.000Z").unwrap();
    assert_eq!(audited.action, "extract");
    assert_eq!(audited.reasons, ["per-usage", "audited", "hash"]);
    assert_eq!(audited.decision().action, "extract");
    assert!(!audited.executed);
}

#[test]
fn the_tool_refuses_to_execute_on_the_host() {
    let answer = execute_on_host();
    assert_eq!(answer["executed"], false);
    assert_eq!(answer["ok"], false);
}

#[test]
fn cli_judge_and_agent_approver() {
    let bin = env!("CARGO_BIN_EXE_imm");
    let judged = Command::new(bin)
        .current_dir(root())
        .args(["judge", "--proposal", "fixtures/proposals/trust-downgrade.json", "--now", "2026-09-22T12:00:00.000Z"])
        .output()
        .unwrap();
    assert_eq!(judged.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&judged.stdout).contains("trust-downgrade"));

    let out = state("plan");
    let plan = Command::new(bin)
        .current_dir(root())
        .args([
            "plan",
            "--manifest",
            "fixtures/agent-manifest.json",
            "--registry",
            "fixtures/registry",
            "--out",
            out.to_str().unwrap(),
            "--now",
            "2026-09-22T12:00:00.000Z",
        ])
        .output()
        .unwrap();
    assert_eq!(plan.status.code(), Some(0), "{}", String::from_utf8_lossy(&plan.stderr));

    let approve = Command::new(bin)
        .args(["approve", "--state", out.to_str().unwrap(), "--approver", "agent"])
        .output()
        .unwrap();
    assert_eq!(approve.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&approve.stderr).contains("AGENT_APPROVER"));

    let human = Command::new(bin)
        .args(["approve", "--state", out.to_str().unwrap(), "--approver", "human"])
        .output()
        .unwrap();
    assert_eq!(human.status.code(), Some(0), "{}", String::from_utf8_lossy(&human.stderr));
    assert!(out.join("vendor/pure-slug/src/index.js").is_file());
}

#[test]
fn cli_agent_and_store() {
    let bin = env!("CARGO_BIN_EXE_imm");
    let advice = Command::new(bin)
        .current_dir(root())
        .args(["agent", "--module", "fixtures/modules/sharp.json"])
        .output()
        .unwrap();
    assert_eq!(advice.status.code(), Some(0), "{}", String::from_utf8_lossy(&advice.stderr));
    let body: serde_json::Value = serde_json::from_slice(&advice.stdout).unwrap();
    assert_eq!(body["verdict"], "install-source");
    assert_eq!(body["next"]["do"], "install-whole-source");
    assert_eq!(body["next"]["target"], "sharp@0.33.5");

    let held = Command::new(bin)
        .current_dir(root())
        .args(["store", "--module", "fixtures/modules/slug-kit-install.json"])
        .output()
        .unwrap();
    assert_eq!(held.status.code(), Some(2));
    let text = String::from_utf8_lossy(&held.stdout);
    assert!(text.contains("needs-no-node-modules"));
    assert!(text.contains("needs-vendor:slugify@1.0.0"));
}
