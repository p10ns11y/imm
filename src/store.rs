use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub name: String,
    pub action: String,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Audit {
    pub by: String,
    pub at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Vendored {
    pub id: String,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub audit: Option<Audit>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Module {
    pub name: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default, rename = "fn")]
    pub func: Option<String>,
    #[serde(default)]
    pub hash: Option<String>,
    #[serde(default)]
    pub audit: Option<Audit>,
    #[serde(default)]
    pub diff: Option<String>,
    #[serde(default, rename = "agentModified")]
    pub agent_modified: Option<bool>,
    #[serde(default, rename = "nodeModules")]
    pub node_modules: Option<bool>,
    #[serde(default)]
    pub subdeps: Vec<String>,
    #[serde(default)]
    pub vendored: Vec<Vendored>,
}

impl Module {
    pub fn from_value(value: Value) -> Result<Self, String> {
        serde_json::from_value(value).map_err(|err| err.to_string())
    }
}

fn id_of(module: &Module) -> String {
    match module.version.as_deref() {
        Some(version) if !version.is_empty() => format!("{}@{version}", module.name),
        _ => module.name.clone(),
    }
}

fn present(text: &Option<String>) -> bool {
    text.as_deref().is_some_and(|value| !value.is_empty())
}

pub fn decide_retention(module: &Module) -> Decision {
    if matches!(module.kind.as_str(), "trivial" | "sub" | "use") {
        let name = match module.func.as_deref() {
            Some(func) if !func.is_empty() => format!("{}#{func}", id_of(module)),
            _ => id_of(module),
        };
        let mut reasons = Vec::new();
        if !present(&module.hash) {
            reasons.push("needs-hash".to_string());
        }
        if module.audit.is_none() {
            reasons.push("needs-audit".to_string());
        }
        let modified = module.agent_modified.unwrap_or(false);
        if modified && !present(&module.diff) {
            reasons.push("needs-agent-diff".to_string());
        }
        if !reasons.is_empty() {
            return Decision { name, action: "hold".to_string(), reasons };
        }
        let mut why = vec!["per-usage".to_string(), "audited".to_string(), "hash".to_string()];
        if present(&module.diff) {
            why.push("agent-diff".to_string());
        }
        return Decision { name, action: "extract".to_string(), reasons: why };
    }

    if module.kind == "library" {
        let mut reasons = Vec::new();
        if module.node_modules != Some(false) {
            reasons.push("needs-no-node-modules".to_string());
        }
        for item in &module.vendored {
            if item.audit.is_none() {
                reasons.push(format!("vendored-unaudited:{}", item.id));
            }
            if item.files.is_empty() {
                reasons.push(format!("not-extracted:{}", item.id));
            }
        }
        let ready: Vec<&str> = module
            .vendored
            .iter()
            .filter(|item| item.audit.is_some() && !item.files.is_empty())
            .map(|item| item.id.as_str())
            .collect();
        for dep in &module.subdeps {
            if !ready.contains(&dep.as_str()) {
                reasons.push(format!("needs-vendor:{dep}"));
            }
        }
        if !reasons.is_empty() {
            return Decision { name: module.name.clone(), action: "hold".to_string(), reasons };
        }
        return Decision {
            name: module.name.clone(),
            action: "ship".to_string(),
            reasons: vec![
                "vendored".to_string(),
                "audited".to_string(),
                "extracted".to_string(),
                "no-node-modules".to_string(),
            ],
        };
    }

    if module.kind == "direct" {
        return Decision {
            name: id_of(module),
            action: "install-source".to_string(),
            reasons: vec!["direct".to_string(), "whole-source".to_string()],
        };
    }

    Decision {
        name: if module.name.is_empty() { "unknown".to_string() } else { module.name.clone() },
        action: "hold".to_string(),
        reasons: vec!["unknown-kind".to_string()],
    }
}

pub fn format_retention(decision: &Decision) -> String {
    if decision.reasons.is_empty() {
        format!("{} {}", decision.name, decision.action)
    } else {
        format!("{} {} {}", decision.name, decision.action, decision.reasons.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn load(rel: &str) -> Module {
        let path = format!("{}/{rel}", env!("CARGO_MANIFEST_DIR"));
        let text = fs::read_to_string(path).unwrap();
        Module::from_value(serde_json::from_str(&text).unwrap()).unwrap()
    }

    #[test]
    fn a_trivial_use_waits_for_a_hash_and_an_audit() {
        let decision = decide_retention(&load("fixtures/modules/slugify.json"));
        assert_eq!(decision.action, "hold");
        assert_eq!(decision.reasons, ["needs-hash", "needs-audit"]);
    }

    #[test]
    fn a_direct_dependency_installs_whole_source() {
        let sharp = decide_retention(&load("fixtures/modules/sharp.json"));
        assert_eq!(sharp.action, "install-source");
        assert_eq!(sharp.reasons, ["direct", "whole-source"]);
        assert_eq!(sharp.name, "sharp@0.33.5");
    }

    #[test]
    fn a_per_usage_function_keeps_hash_audit_and_diff() {
        let bare = decide_retention(&load("fixtures/modules/color.json"));
        assert_eq!(bare.reasons, ["needs-hash", "needs-audit"]);
        let edited = decide_retention(&load("fixtures/modules/color-edited.json"));
        assert_eq!(edited.reasons, ["needs-agent-diff"]);
        let used = decide_retention(&load("fixtures/modules/color-use.json"));
        assert_eq!(used.action, "extract");
        assert_eq!(used.name, "color@4.2.3#convert");
        assert_eq!(used.reasons, ["per-usage", "audited", "hash", "agent-diff"]);
    }

    #[test]
    fn a_library_ships_only_when_the_extract_is_inside_the_package() {
        let bare = decide_retention(&load("fixtures/modules/slug-kit-install.json"));
        assert_eq!(bare.action, "hold");
        assert_eq!(bare.reasons, ["needs-no-node-modules", "needs-vendor:slugify@1.0.0"]);
        let shipped = decide_retention(&load("fixtures/modules/slug-kit.json"));
        assert_eq!(shipped.action, "ship");
        assert_eq!(shipped.reasons, ["vendored", "audited", "extracted", "no-node-modules"]);
    }
}
