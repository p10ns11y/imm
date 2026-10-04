use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::agent::{advise, AdviseInput};
use crate::package_flow::run_package_flow;
use crate::error::ImmError;
use crate::plan::{approve_plan, plan_install, verify_lock, write_plan};
use crate::policy::{format_verdict, judge_update, Proposal};
use crate::records::{audit_use, extract_use, install_source, lock_install};
use crate::slice::{format_slice, slice_package, Budgets};
use crate::store::{decide_retention, format_retention, Module};
use crate::timeutil::parse_unix_ms;

pub fn main() -> i32 {
    match run(env::args().skip(1).collect()) {
        Ok(Run::Text(text)) => {
            println!("{text}");
            0
        }
        Ok(Run::Refuse(text)) => {
            println!("{text}");
            2
        }
        Err(error) => {
            let _ = writeln!(io::stderr(), "{error}");
            if error.code == "IO" || error.code == "MISSING" { 1 } else { 2 }
        }
    }
}

enum Run {
    Text(String),
    Refuse(String),
}

fn run(args: Vec<String>) -> Result<Run, ImmError> {
    let (cmd, opts) = parse(&args);
    if cmd.is_empty() || cmd == "help" || opts.contains_key("help") {
        return Ok(Run::Text(usage()));
    }
    let now = now_ms(&opts)?;
    match cmd.as_str() {
        "package-flow" => Ok(Run::Text(run_package_flow(&project_root())?)),
        "judge" => {
            let proposal = read_proposal(req(&opts, "proposal")?)?;
            let verdict = judge_update(&proposal, now);
            let text = format_verdict(&verdict);
            if verdict.accept { Ok(Run::Text(text)) } else { Ok(Run::Refuse(text)) }
        }
        "slice" => {
            let package = PathBuf::from(req(&opts, "package")?);
            let exports = split_csv(opts.get("exports").map(String::as_str).unwrap_or(""));
            let export_refs: Vec<&str> = exports.iter().map(String::as_str).collect();
            let name = package.file_name().and_then(|n| n.to_str()).unwrap_or("package");
            let slice = slice_package(
                &package,
                name,
                req(&opts, "version")?,
                &export_refs,
                Budgets { max_depth: opts.get("maxDepth").and_then(|v| v.parse().ok()).unwrap_or(6), ..Budgets::default() },
                now,
            );
            if slice.blocked { Ok(Run::Refuse(format_slice(&slice))) } else { Ok(Run::Text(format_slice(&slice))) }
        }
        "store" => {
            let module = read_module(req(&opts, "module")?)?;
            let decision = decide_retention(&module);
            let text = format_retention(&decision);
            if decision.action == "hold" { Ok(Run::Refuse(text)) } else { Ok(Run::Text(text)) }
        }
        "agent" => {
            let proposal = opt_proposal(&opts)?;
            let module = opt_module(&opts)?;
            let slice = opt_slice(&opts, now)?;
            let advice = advise(AdviseInput {
                proposal: proposal.as_ref(),
                now_ms: now,
                module: module.as_ref(),
                slice: slice.as_ref(),
            });
            let text = serde_json::to_string_pretty(&advice).map_err(|err| ImmError::new("IO", &err.to_string()))?;
            if advice.ok { Ok(Run::Text(text)) } else { Ok(Run::Refuse(text)) }
        }
        "plan" => {
            let manifest = read_json(req(&opts, "manifest")?)?;
            let planned = plan_install(&manifest, &PathBuf::from(req(&opts, "registry")?), now);
            let out = PathBuf::from(req(&opts, "out")?);
            write_plan(&out, &planned)?;
            let text = if planned.blocked {
                let mut reasons = planned.reasons.clone();
                for pkg in &planned.packages {
                    reasons.extend(pkg.reasons.clone());
                }
                format!("blocked {}", reasons.join(" "))
            } else {
                format!("staged {}", out.display())
            };
            if planned.blocked { Ok(Run::Refuse(text)) } else { Ok(Run::Text(text)) }
        }
        "approve" => {
            approve_plan(&PathBuf::from(req(&opts, "state")?), req(&opts, "approver")?)?;
            Ok(Run::Text(format!("approved {}", req(&opts, "state")?)))
        }
        "verify" => {
            let problems = verify_lock(&PathBuf::from(req(&opts, "state")?))?;
            if problems.is_empty() { Ok(Run::Text("ok".to_string())) } else { Ok(Run::Refuse(problems.join("\n"))) }
        }
        "install" => {
            let package = PathBuf::from(req(&opts, "package")?);
            let record = install_source(&package, req(&opts, "version")?, &PathBuf::from(req(&opts, "state")?), now)?;
            let text = serde_json::to_string_pretty(&record).map_err(|err| ImmError::new("IO", &err.to_string()))?;
            if record.action == "hold" || record.executed { Ok(Run::Refuse(text)) } else { Ok(Run::Text(text)) }
        }
        "extract" => {
            let package = PathBuf::from(req(&opts, "package")?);
            let name = package.file_name().and_then(|n| n.to_str()).unwrap_or("package");
            let func = split_csv(req(&opts, "exports")?).into_iter().next().unwrap_or_default();
            let record = extract_use(&package, name, req(&opts, "version")?, &func, &PathBuf::from(req(&opts, "state")?), now)?;
            let text = serde_json::to_string_pretty(&record).map_err(|err| ImmError::new("IO", &err.to_string()))?;
            if record.executed || record.action == "hold" && record.hash.is_empty() {
                Ok(Run::Refuse(text))
            } else if record.action == "hold" {
                Ok(Run::Refuse(text))
            } else {
                Ok(Run::Text(text))
            }
        }
        "audit" => {
            let record = audit_use(
                &PathBuf::from(req(&opts, "state")?),
                req(&opts, "id")?,
                req(&opts, "by")?,
                req(&opts, "at")?,
            )?;
            let text = serde_json::to_string_pretty(&record).map_err(|err| ImmError::new("IO", &err.to_string()))?;
            if record.action == "hold" { Ok(Run::Refuse(text)) } else { Ok(Run::Text(text)) }
        }
        "sandbox" => {
            let package = PathBuf::from(req(&opts, "package")?);
            let name = package.file_name().and_then(|n| n.to_str()).unwrap_or("package");
            let locked = lock_install(&package, name, req(&opts, "version")?, &PathBuf::from(req(&opts, "state")?))?;
            if locked.get("executed").and_then(Value::as_bool) == Some(true) {
                return Err(ImmError::new("EXECUTED", "sandbox ran a package"));
            }
            let text = serde_json::to_string_pretty(&locked).map_err(|err| ImmError::new("IO", &err.to_string()))?;
            if locked.get("ok").and_then(Value::as_bool) == Some(true) { Ok(Run::Text(text)) } else { Ok(Run::Refuse(text)) }
        }
        _ => Ok(Run::Refuse(usage())),
    }
}

fn usage() -> String {
    [
        "imm judge --proposal <file.json> [--now <iso>]",
        "imm slice --package <dir> --version <x.y.z> --exports <a,b> [--now <iso>]",
        "imm plan --manifest <file.json> --registry <dir> --out <dir> [--now <iso>]",
        "imm approve --state <dir> --approver human|agent",
        "imm verify --state <dir>",
        "imm store --module <file.json>",
        "imm agent [--module <file.json>] [--proposal <file.json>] [--package <dir> --version <x.y.z> --exports <a,b>] [--now <iso>]",
        "imm install --package <dir> --version <x.y.z> --state <dir> [--now <iso>]",
        "imm extract --package <dir> --version <x.y.z> --exports <name> --state <dir> [--now <iso>]",
        "imm audit --state <dir> --id <id> --by <name> --at <iso>",
        "imm sandbox --package <dir> --version <x.y.z> --state <dir>",
        "imm package-flow",
    ]
    .join("\n")
}

fn parse(args: &[String]) -> (String, HashMap<String, String>) {
    let mut iter = args.iter();
    let cmd = iter.next().cloned().unwrap_or_default();
    let mut opts = HashMap::new();
    let rest: Vec<&String> = iter.collect();
    let mut i = 0;
    while i < rest.len() {
        let arg = rest[i];
        if let Some(key) = arg.strip_prefix("--") {
            let next = rest.get(i + 1);
            if next.is_none_or(|value| value.starts_with("--")) {
                opts.insert(key.to_string(), "true".to_string());
            } else {
                opts.insert(key.to_string(), (*next.unwrap()).clone());
                i += 1;
            }
        }
        i += 1;
    }
    (cmd, opts)
}

fn req<'a>(opts: &'a HashMap<String, String>, key: &str) -> Result<&'a str, ImmError> {
    opts.get(key).map(String::as_str).ok_or_else(|| ImmError::new("USAGE", &format!("missing --{key}")))
}

fn now_ms(opts: &HashMap<String, String>) -> Result<i64, ImmError> {
    if let Some(text) = opts.get("now") {
        return parse_unix_ms(text).ok_or_else(|| ImmError::new("CLOCK", "bad --now"));
    }
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|err| ImmError::new("CLOCK", &err.to_string()))?;
    Ok(ms.as_millis() as i64)
}

fn project_root() -> PathBuf {
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if cwd.join("fixtures").is_dir() {
        cwd
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }
}

fn read_json(path: &str) -> Result<Value, ImmError> {
    let text = fs::read_to_string(path).map_err(|err| ImmError::new("IO", &err.to_string()))?;
    serde_json::from_str(&text).map_err(|err| ImmError::new("IO", &err.to_string()))
}

fn read_proposal(path: &str) -> Result<Proposal, ImmError> {
    Ok(Proposal::from_value(&read_json(path)?))
}

fn read_module(path: &str) -> Result<Module, ImmError> {
    Module::from_value(read_json(path)?).map_err(|err| ImmError::new("IO", &err))
}

fn opt_proposal(opts: &HashMap<String, String>) -> Result<Option<Proposal>, ImmError> {
    match opts.get("proposal") {
        Some(path) => Ok(Some(read_proposal(path)?)),
        None => Ok(None),
    }
}

fn opt_module(opts: &HashMap<String, String>) -> Result<Option<Module>, ImmError> {
    match opts.get("module") {
        Some(path) => Ok(Some(read_module(path)?)),
        None => Ok(None),
    }
}

fn opt_slice(opts: &HashMap<String, String>, now: i64) -> Result<Option<crate::slice::Slice>, ImmError> {
    let Some(package) = opts.get("package") else { return Ok(None) };
    let exports = split_csv(opts.get("exports").map(String::as_str).unwrap_or(""));
    let refs: Vec<&str> = exports.iter().map(String::as_str).collect();
    let path = PathBuf::from(package);
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("package");
    Ok(Some(slice_package(&path, name, req(opts, "version")?, &refs, Budgets::default(), now)))
}

fn split_csv(text: &str) -> Vec<String> {
    text.split(',').filter(|part| !part.is_empty()).map(str::to_string).collect()
}
