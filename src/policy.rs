use serde_json::Value;

use crate::timeutil::parse_unix_ms;

pub const MINIMUM_RELEASE_AGE_MINUTES: i64 = 2880;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub accept: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Proposal {
    pub name: String,
    pub version: String,
    pub range: Option<String>,
    pub published_at: Option<String>,
    pub trust: Option<String>,
    pub previous_trust: Option<String>,
    pub lifecycle_scripts: bool,
    pub integrity: Option<String>,
}

impl Proposal {
    pub fn from_value(value: &Value) -> Self {
        Self {
            name: value.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
            version: json_string(value, "version"),
            range: value.get("range").and_then(Value::as_str).map(str::to_string),
            published_at: value.get("publishedAt").and_then(Value::as_str).map(str::to_string),
            trust: value.get("trust").and_then(Value::as_str).map(str::to_string),
            previous_trust: optional_string(value, "previousTrust"),
            lifecycle_scripts: value.get("lifecycleScripts").and_then(Value::as_bool).unwrap_or(false),
            integrity: nonempty(value, "integrity"),
        }
    }
}

fn json_string(value: &Value, key: &str) -> String {
    match value.get(key) {
        Some(Value::String(text)) => text.clone(),
        Some(other) if !other.is_null() => other.to_string(),
        _ => String::new(),
    }
}

fn nonempty(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

fn optional_string(value: &Value, key: &str) -> Option<String> {
    match value.get(key) {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) => Some(text.clone()),
        Some(other) => Some(other.to_string()),
    }
}

pub fn trust_rank(trust: &str) -> Option<i32> {
    match trust {
        "none" => Some(0),
        "provenance" => Some(1),
        "trusted-publisher" => Some(2),
        _ => None,
    }
}

pub fn has_lifecycle_scripts(manifest: &Value) -> bool {
    let Some(scripts) = manifest.get("scripts").and_then(Value::as_object) else {
        return false;
    };
    scripts.iter().any(|(name, value)| {
        matches!(name.as_str(), "preinstall" | "install" | "postinstall" | "prepare") && truthy(value)
    })
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::String(text) => !text.is_empty(),
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64().unwrap_or(0.0) != 0.0,
        _ => false,
    }
}

pub fn judge_update(proposal: &Proposal, now_ms: i64) -> Verdict {
    let mut reasons = Vec::new();
    let range = proposal.range.clone().unwrap_or_else(|| proposal.version.clone());
    if !exact_version(&proposal.version) || range != proposal.version {
        reasons.push("floating-range".to_string());
    }
    if proposal.integrity.as_deref().unwrap_or("").is_empty() {
        reasons.push("missing-integrity".to_string());
    }
    if proposal.lifecycle_scripts {
        reasons.push("lifecycle-script".to_string());
    }
    match proposal.published_at.as_deref().and_then(parse_unix_ms) {
        None => reasons.push("missing-publish-time".to_string()),
        Some(published) if (now_ms - published) / 60_000 < MINIMUM_RELEASE_AGE_MINUTES => {
            reasons.push("too-fresh".to_string());
        }
        Some(_) => {}
    }
    match proposal.trust.as_deref().and_then(trust_rank) {
        None => reasons.push("unknown-trust".to_string()),
        Some(_) => {}
    }
    if let Some(previous) = proposal.previous_trust.as_deref() {
        let previous_rank = trust_rank(previous);
        let next_rank = proposal.trust.as_deref().and_then(trust_rank).unwrap_or(-1);
        if previous_rank.is_none() || next_rank < previous_rank.unwrap_or(0) {
            reasons.push("trust-downgrade".to_string());
        }
    }
    Verdict {
        accept: reasons.is_empty(),
        reasons,
    }
}

fn exact_version(version: &str) -> bool {
    let mut parts = version.split('.');
    let Some(major) = parts.next() else { return false };
    let Some(minor) = parts.next() else { return false };
    let Some(patch) = parts.next() else { return false };
    parts.next().is_none()
        && !major.is_empty()
        && !minor.is_empty()
        && !patch.is_empty()
        && major.bytes().all(|b| b.is_ascii_digit())
        && minor.bytes().all(|b| b.is_ascii_digit())
        && patch.bytes().all(|b| b.is_ascii_digit())
}

pub fn format_verdict(verdict: &Verdict) -> String {
    if verdict.accept {
        "accept".to_string()
    } else {
        format!("reject {}", verdict.reasons.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    fn now() -> i64 {
        parse_unix_ms("2026-09-22T12:00:00.000Z").unwrap()
    }

    fn load(name: &str) -> Proposal {
        let path = format!("{}/fixtures/proposals/{name}", env!("CARGO_MANIFEST_DIR"));
        let text = fs::read_to_string(path).unwrap();
        Proposal::from_value(&serde_json::from_str(&text).unwrap())
    }

    #[test]
    fn accepts_an_exact_aged_trusted_update() {
        let verdict = judge_update(&load("ok.json"), now());
        assert!(verdict.accept);
        assert!(verdict.reasons.is_empty());
    }

    #[test]
    fn rejects_a_release_inside_the_age_window() {
        let verdict = judge_update(&load("fresh.json"), now());
        assert!(!verdict.accept);
        assert!(verdict.reasons.iter().any(|r| r == "too-fresh"));
    }

    #[test]
    fn rejects_a_trust_downgrade() {
        let verdict = judge_update(&load("trust-downgrade.json"), now());
        assert!(verdict.reasons.iter().any(|r| r == "trust-downgrade"));
    }

    #[test]
    fn rejects_lifecycle_scripts_and_floating_ranges() {
        assert!(judge_update(&load("lifecycle.json"), now()).reasons.iter().any(|r| r == "lifecycle-script"));
        assert!(judge_update(&load("floating.json"), now()).reasons.iter().any(|r| r == "floating-range"));
    }

    #[test]
    fn package_manifest_scripts_count_as_lifecycle() {
        let manifest = json!({"scripts": {"postinstall": "node scripts/setup.js"}});
        assert!(has_lifecycle_scripts(&manifest));
    }
}
