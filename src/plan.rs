use std::fs;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use crate::error::ImmError;
use crate::hash::sha256_hex;
use crate::slice::{slice_package, Budgets, Slice};

#[derive(Debug)]
pub struct Plan {
    pub blocked: bool,
    pub reasons: Vec<String>,
    pub budgets: Budgets,
    pub packages: Vec<Slice>,
}

#[derive(Serialize)]
struct PublicFile<'a> {
    path: &'a str,
    sha256: &'a str,
    bytes: usize,
    depth: usize,
}

#[derive(Serialize)]
struct PublicPackage<'a> {
    name: &'a str,
    version: &'a str,
    blocked: bool,
    reasons: &'a [String],
    omitted: &'a [String],
    exports: &'a [String],
    files: Vec<PublicFile<'a>>,
}

#[derive(Serialize)]
struct PublicPlan<'a> {
    blocked: bool,
    reasons: &'a [String],
    budgets: BudgetJson,
    packages: Vec<PublicPackage<'a>>,
}

#[derive(Serialize)]
struct BudgetJson {
    #[serde(rename = "maxPackages")]
    max_packages: usize,
    #[serde(rename = "maxFiles")]
    max_files: usize,
    #[serde(rename = "maxBytes")]
    max_bytes: usize,
    #[serde(rename = "maxDepth")]
    max_depth: usize,
}

pub fn plan_install(manifest: &Value, registry_dir: &Path, now_ms: i64) -> Plan {
    let defaults = Budgets::default();
    let budgets_value = manifest.get("budgets");
    let budgets = Budgets {
        max_packages: number(budgets_value, "maxPackages", defaults.max_packages),
        max_files: number(budgets_value, "maxFiles", defaults.max_files),
        max_bytes: number(budgets_value, "maxBytes", defaults.max_bytes),
        max_depth: number(budgets_value, "maxDepth", defaults.max_depth),
    };
    let needs = manifest.get("needs").and_then(Value::as_array);
    let mut reasons = Vec::new();
    let need_count = needs.map(Vec::len).unwrap_or(0);
    if need_count > budgets.max_packages {
        reasons.push("budget-packages".to_string());
    }
    let mut packages = Vec::new();
    if !reasons.iter().any(|reason| reason == "budget-packages") {
        if let Some(needs) = needs {
            for need in needs {
                let specifier = need.get("specifier").and_then(Value::as_str).unwrap_or("");
                let version = need.get("version").and_then(Value::as_str).unwrap_or("");
                let exports = need
                    .get("exports")
                    .and_then(Value::as_array)
                    .map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>())
                    .unwrap_or_default();
                packages.push(slice_package(
                    &registry_dir.join(specifier),
                    specifier,
                    version,
                    &exports,
                    Budgets { ..budgets },
                    now_ms,
                ));
            }
        }
    }
    let file_count: usize = packages.iter().map(|pkg| pkg.files.len()).sum();
    let bytes: usize = packages.iter().map(|pkg| pkg.files.iter().map(|file| file.bytes).sum::<usize>()).sum();
    if file_count > budgets.max_files {
        reasons.push("budget-files".to_string());
    }
    if bytes > budgets.max_bytes {
        reasons.push("budget-bytes".to_string());
    }
    let blocked = !reasons.is_empty() || packages.iter().any(|pkg| pkg.blocked);
    Plan { blocked, reasons, budgets, packages }
}

fn number(budgets: Option<&Value>, key: &str, default: usize) -> usize {
    budgets
        .and_then(|value| value.get(key))
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(default)
}

fn stub_source(specifier: &str, export_name: &str) -> String {
    format!("export function {export_name}() {{\n  throw new Error(\"imm: {specifier}#{export_name} is not filled\");\n}}\n")
}

pub fn write_plan(out_dir: &Path, plan: &Plan) -> Result<(), ImmError> {
    if out_dir.exists() {
        fs::remove_dir_all(out_dir).map_err(|err| ImmError::new("IO", &err.to_string()))?;
    }
    fs::create_dir_all(out_dir).map_err(|err| ImmError::new("IO", &err.to_string()))?;
    for pkg in &plan.packages {
        for export_name in &pkg.exports {
            if !export_name.chars().next().is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_' || ch == '$') {
                continue;
            }
            if !export_name.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$') {
                continue;
            }
            let dest = out_dir.join("stubs").join(&pkg.name).join(format!("{export_name}.js"));
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|err| ImmError::new("IO", &err.to_string()))?;
            }
            fs::write(&dest, stub_source(&pkg.name, export_name)).map_err(|err| ImmError::new("IO", &err.to_string()))?;
        }
    }
    if !plan.blocked {
        for pkg in &plan.packages {
            for file in &pkg.files {
                let dest = out_dir.join("stage").join(&pkg.name).join(&file.path);
                if let Some(parent) = dest.parent() {
                    fs::create_dir_all(parent).map_err(|err| ImmError::new("IO", &err.to_string()))?;
                }
                fs::write(&dest, &file.text).map_err(|err| ImmError::new("IO", &err.to_string()))?;
            }
        }
    }
    let body = public_plan(plan);
    let mut text = serde_json::to_string_pretty(&body).map_err(|err| ImmError::new("IO", &err.to_string()))?;
    text.push('\n');
    fs::write(out_dir.join("proposal.json"), text).map_err(|err| ImmError::new("IO", &err.to_string()))?;
    Ok(())
}

fn public_plan(plan: &Plan) -> PublicPlan<'_> {
    PublicPlan {
        blocked: plan.blocked,
        reasons: &plan.reasons,
        budgets: BudgetJson {
            max_packages: plan.budgets.max_packages,
            max_files: plan.budgets.max_files,
            max_bytes: plan.budgets.max_bytes,
            max_depth: plan.budgets.max_depth,
        },
        packages: plan
            .packages
            .iter()
            .map(|pkg| PublicPackage {
                name: &pkg.name,
                version: &pkg.version,
                blocked: pkg.blocked,
                reasons: &pkg.reasons,
                omitted: &pkg.omitted,
                exports: &pkg.exports,
                files: pkg
                    .files
                    .iter()
                    .map(|file| PublicFile {
                        path: &file.path,
                        sha256: &file.sha256,
                        bytes: file.bytes,
                        depth: file.depth,
                    })
                    .collect(),
            })
            .collect(),
    }
}

pub fn approve_plan(out_dir: &Path, approver: &str) -> Result<(), ImmError> {
    if approver != "human" {
        return Err(ImmError::new("AGENT_APPROVER", "An agent cannot approve a fill."));
    }
    let proposal_text = fs::read_to_string(out_dir.join("proposal.json")).map_err(|_| ImmError::new("IO", "missing proposal"))?;
    let proposal: Value = serde_json::from_str(&proposal_text).map_err(|err| ImmError::new("IO", &err.to_string()))?;
    if proposal.get("blocked").and_then(Value::as_bool).unwrap_or(false)
        || proposal
            .get("packages")
            .and_then(Value::as_array)
            .is_some_and(|packages| packages.iter().any(|pkg| pkg.get("blocked").and_then(Value::as_bool).unwrap_or(false)))
    {
        return Err(ImmError::new("BLOCKED", "The proposal is blocked."));
    }
    let mut lock = serde_json::Map::new();
    let packages = proposal.get("packages").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut lock_packages = serde_json::Map::new();
    for pkg in packages {
        let name = pkg.get("name").and_then(Value::as_str).unwrap_or("").to_string();
        let version = pkg.get("version").and_then(Value::as_str).unwrap_or("").to_string();
        let mut files_lock = serde_json::Map::new();
        let files = pkg.get("files").and_then(Value::as_array).cloned().unwrap_or_default();
        for file in files {
            let rel = file.get("path").and_then(Value::as_str).unwrap_or("");
            let expected = file.get("sha256").and_then(Value::as_str).unwrap_or("");
            let stage = out_dir.join("stage").join(&name).join(rel);
            let bytes = fs::read(&stage).map_err(|_| ImmError::new("IO", &format!("missing stage {name}/{rel}")))?;
            let sum = sha256_hex(&bytes);
            if sum != expected {
                return Err(ImmError::new("HASH_MISMATCH", &format!("{name}/{rel} changed after the proposal.")));
            }
            let dest = out_dir.join("vendor").join(&name).join(rel);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|err| ImmError::new("IO", &err.to_string()))?;
            }
            fs::write(&dest, &bytes).map_err(|err| ImmError::new("IO", &err.to_string()))?;
            files_lock.insert(rel.to_string(), Value::String(sum));
        }
        lock_packages.insert(
            name,
            serde_json::json!({ "version": version, "files": Value::Object(files_lock) }),
        );
    }
    lock.insert("packages".to_string(), Value::Object(lock_packages));
    let mut text = serde_json::to_string_pretty(&Value::Object(lock)).map_err(|err| ImmError::new("IO", &err.to_string()))?;
    text.push('\n');
    fs::write(out_dir.join("lock.json"), text).map_err(|err| ImmError::new("IO", &err.to_string()))?;
    Ok(())
}

pub fn verify_lock(out_dir: &Path) -> Result<Vec<String>, ImmError> {
    let text = fs::read_to_string(out_dir.join("lock.json")).map_err(|_| ImmError::new("IO", "missing lock"))?;
    let lock: Value = serde_json::from_str(&text).map_err(|err| ImmError::new("IO", &err.to_string()))?;
    let mut problems = Vec::new();
    let Some(packages) = lock.get("packages").and_then(Value::as_object) else {
        return Ok(vec!["missing packages".to_string()]);
    };
    for (name, pkg) in packages {
        let Some(files) = pkg.get("files").and_then(Value::as_object) else { continue };
        for (rel, sum) in files {
            let expected = sum.as_str().unwrap_or("");
            let abs = out_dir.join("vendor").join(name).join(rel);
            match fs::read(&abs) {
                Err(_) => problems.push(format!("missing {name}/{rel}")),
                Ok(bytes) if sha256_hex(&bytes) != expected => problems.push(format!("hash {name}/{rel}")),
                Ok(_) => {}
            }
        }
    }
    Ok(problems)
}
