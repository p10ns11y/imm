use serde::Serialize;

use crate::policy::{judge_update, Proposal, Verdict};
use crate::slice::Slice;
use crate::store::{decide_retention, Decision, Module};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Next {
    #[serde(rename = "do")]
    pub do_: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Advice {
    pub ok: bool,
    pub verdict: String,
    pub next: Next,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drop: Option<Vec<String>>,
    pub skip: Vec<String>,
    pub later: Vec<String>,
    pub why: Vec<String>,
}

fn ids(reasons: &[String], prefix: &str) -> Vec<String> {
    reasons
        .iter()
        .filter_map(|reason| reason.strip_prefix(prefix).map(str::to_string))
        .collect()
}

fn advice(ok: bool, verdict: &str, next_do: &str, target: &str, skip: &[&str], later: Vec<String>, why: Vec<String>) -> Advice {
    Advice {
        ok,
        verdict: verdict.to_string(),
        next: Next { do_: next_do.to_string(), target: target.to_string() },
        drop: None,
        skip: skip.iter().map(|item| (*item).to_string()).collect(),
        later,
        why,
    }
}

pub fn retention_advice(decision: &Decision) -> Advice {
    let why = decision.reasons.clone();
    if decision.action == "install-source" {
        return advice(true, "install-source", "install-whole-source", &decision.name, &["subdependencies", "node_modules"], vec![], why);
    }
    if decision.action == "extract" {
        return advice(true, "extract", "keep-extract", &decision.name, &["install"], vec![], why);
    }
    if why.iter().any(|reason| reason == "needs-hash") {
        let mut later = Vec::new();
        if why.iter().any(|reason| reason == "needs-audit") {
            later.push("audit".to_string());
        }
        if why.iter().any(|reason| reason == "needs-agent-diff") {
            later.push("record the agent diff".to_string());
        }
        return advice(false, "extract", "record-hash", &decision.name, &["install"], later, why);
    }
    if decision.action == "ship" {
        return advice(true, "ship", "none", &decision.name, &["node_modules", "install"], vec![], why);
    }
    if why.iter().any(|reason| reason == "needs-audit") {
        let later = if why.iter().any(|reason| reason == "needs-agent-diff") {
            vec!["record the agent diff".to_string()]
        } else {
            vec![]
        };
        return advice(false, "prepare-audit", "audit", &decision.name, &["install"], later, why);
    }
    if why.iter().any(|reason| reason == "needs-agent-diff") {
        return advice(false, "extract", "record-agent-diff", &decision.name, &["install"], vec![], why);
    }
    let missing_vendor = ids(&why, "needs-vendor:");
    if !missing_vendor.is_empty() {
        let mut later: Vec<String> = missing_vendor.iter().skip(1).map(|id| format!("vendor {id}")).collect();
        if why.iter().any(|reason| reason == "needs-no-node-modules") {
            later.push("set nodeModules false".to_string());
        }
        return advice(false, "vendor", "vendor-extract", &missing_vendor[0], &["node_modules", "install"], later, why);
    }
    let vendored_unaudited = ids(&why, "vendored-unaudited:");
    if !vendored_unaudited.is_empty() {
        let later = vendored_unaudited.iter().skip(1).map(|id| format!("audit {id}")).collect();
        return advice(false, "prepare-audit", "audit", &vendored_unaudited[0], &["ship", "install"], later, why);
    }
    let not_extracted = ids(&why, "not-extracted:");
    if !not_extracted.is_empty() {
        let later = not_extracted.iter().skip(1).map(|id| format!("extract {id}")).collect();
        return advice(false, "vendor", "extract-files", &not_extracted[0], &["ship", "install"], later, why);
    }
    if why.iter().any(|reason| reason == "needs-no-node-modules") {
        return advice(false, "vendor", "set-node-modules-false", &decision.name, &["node_modules", "install"], vec![], why);
    }
    advice(false, "hold", "stop", &decision.name, &["store", "install"], vec![], why)
}

pub struct AdviseInput<'a> {
    pub proposal: Option<&'a Proposal>,
    pub now_ms: i64,
    pub module: Option<&'a Module>,
    pub slice: Option<&'a Slice>,
}

pub fn advise(input: AdviseInput<'_>) -> Advice {
    if let Some(proposal) = input.proposal {
        let verdict: Verdict = judge_update(proposal, input.now_ms);
        if !verdict.accept {
            return advice(
                false,
                "reject-update",
                "keep-pinned",
                &format!("{}@{}", proposal.name, proposal.version),
                &["bump", "publish", "download", "install"],
                vec![],
                verdict.reasons,
            );
        }
    }
    if let Some(slice) = input.slice {
        if slice.blocked {
            return advice(
                false,
                "refuse-extract",
                "do-not-vendor",
                &slice.name,
                &["vendor", "install"],
                vec![],
                slice.reasons.clone(),
            );
        }
    }
    if let Some(module) = input.module {
        return retention_advice(&decide_retention(module));
    }
    if let Some(slice) = input.slice {
        if !slice.omitted.is_empty() {
            let mut body = advice(
                true,
                "extract",
                "keep-reached-files",
                &slice.name,
                &["install"],
                vec![],
                vec!["unused-files-dropped".to_string()],
            );
            body.drop = Some(slice.omitted.clone());
            return body;
        }
    }
    if let Some(proposal) = input.proposal {
        return advice(
            true,
            "accept-update",
            "read-the-diff",
            &format!("{}@{}", proposal.name, proposal.version),
            &["install"],
            vec![],
            vec!["release-check-passed".to_string()],
        );
    }
    advice(false, "needs-input", "provide-module-or-proposal", "", &[], vec![], vec!["missing-input".to_string()])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::Proposal;
    use crate::slice::{slice_package, Budgets};
    use crate::store::Module;
    use crate::timeutil::parse_unix_ms;
    use std::fs;
    use std::path::PathBuf;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn now() -> i64 {
        parse_unix_ms("2026-09-22T12:00:00.000Z").unwrap()
    }

    fn module(rel: &str) -> Module {
        let text = fs::read_to_string(root().join(rel)).unwrap();
        Module::from_value(serde_json::from_str(&text).unwrap()).unwrap()
    }

    fn proposal(name: &str) -> Proposal {
        let text = fs::read_to_string(root().join("fixtures/proposals").join(name)).unwrap();
        Proposal::from_value(&serde_json::from_str(&text).unwrap())
    }

    #[test]
    fn a_failed_release_check_keeps_the_pin() {
        let proposal = proposal("fresh.json");
        let advice = advise(AdviseInput { proposal: Some(&proposal), now_ms: now(), module: None, slice: None });
        assert!(!advice.ok);
        assert_eq!(advice.verdict, "reject-update");
        assert_eq!(advice.next.do_, "keep-pinned");
        assert_eq!(advice.next.target, "fresh-slug@9.9.9");
        assert!(advice.skip.iter().any(|item| item == "bump"));
        assert!(advice.why.iter().any(|item| item == "too-fresh"));
    }

    #[test]
    fn a_trivial_use_records_its_hash_before_audit() {
        let module = module("fixtures/modules/slugify.json");
        let advice = advise(AdviseInput { proposal: None, now_ms: now(), module: Some(&module), slice: None });
        assert!(!advice.ok);
        assert_eq!(advice.next.do_, "record-hash");
        assert_eq!(advice.next.target, "slugify");
        assert_eq!(advice.later, ["audit"]);
        assert!(advice.skip.iter().any(|item| item == "install"));
    }

    #[test]
    fn a_direct_dependency_skips_subdependencies() {
        let module = module("fixtures/modules/sharp.json");
        let advice = advise(AdviseInput { proposal: None, now_ms: now(), module: Some(&module), slice: None });
        assert!(advice.ok);
        assert_eq!(advice.verdict, "install-source");
        assert_eq!(advice.next, Next { do_: "install-whole-source".into(), target: "sharp@0.33.5".into() });
        assert!(advice.skip.iter().any(|item| item == "subdependencies"));
    }

    #[test]
    fn a_library_that_still_installs_is_told_to_vendor() {
        let module = module("fixtures/modules/slug-kit-install.json");
        let advice = advise(AdviseInput { proposal: None, now_ms: now(), module: Some(&module), slice: None });
        assert_eq!(advice.verdict, "vendor");
        assert_eq!(advice.next.do_, "vendor-extract");
        assert_eq!(advice.next.target, "slugify@1.0.0");
        assert_eq!(advice.later, ["set nodeModules false"]);
    }

    #[test]
    fn a_shipped_library_has_nothing_left_to_maintain() {
        let module = module("fixtures/modules/slug-kit.json");
        let advice = advise(AdviseInput { proposal: None, now_ms: now(), module: Some(&module), slice: None });
        assert!(advice.ok);
        assert_eq!(advice.verdict, "ship");
        assert_eq!(advice.next.do_, "none");
    }

    #[test]
    fn a_reached_dangerous_call_is_not_vendored() {
        let slice = slice_package(
            &root().join("fixtures/registry/shell-out"),
            "shell-out",
            "1.0.0",
            &["slugify"],
            Budgets { max_depth: 6, max_files: 20, max_bytes: 100_000, max_packages: 4 },
            now(),
        );
        let advice = advise(AdviseInput { proposal: None, now_ms: now(), module: None, slice: Some(&slice) });
        assert_eq!(advice.verdict, "refuse-extract");
        assert_eq!(advice.next.do_, "do-not-vendor");
        assert!(advice.why.iter().any(|reason| reason.starts_with("child_process:")));
    }

    #[test]
    fn unused_files_are_dropped() {
        let slice = slice_package(
            &root().join("fixtures/registry/pure-slug"),
            "pure-slug",
            "1.0.0",
            &["slugify"],
            Budgets { max_depth: 6, max_files: 20, max_bytes: 100_000, max_packages: 4 },
            now(),
        );
        let advice = advise(AdviseInput { proposal: None, now_ms: now(), module: None, slice: Some(&slice) });
        assert!(advice.ok);
        assert_eq!(advice.next.do_, "keep-reached-files");
        assert_eq!(advice.drop.unwrap(), vec!["src/cli.js".to_string()]);
    }
}
