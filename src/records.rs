use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::ImmError;
use crate::hash::{sha256_hex, sha256_text};
use crate::policy::{has_lifecycle_scripts, judge_update, Proposal};
use crate::slice::{slice_package, Budgets};
use crate::store::{decide_retention, Audit, Decision, Module};

/// One on-disk shape for a direct install and for a use.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Record {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub version: String,
    #[serde(rename = "fn", skip_serializing_if = "Option::is_none")]
    pub func: Option<String>,
    pub action: String,
    pub files: Vec<RecordFile>,
    pub hash: String,
    pub audit: Option<Audit>,
    #[serde(rename = "agentModified")]
    pub agent_modified: bool,
    pub diff: String,
    pub reasons: Vec<String>,
    pub executed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecordFile {
    pub path: String,
    pub sha256: String,
    pub bytes: usize,
}

impl Record {
    pub fn as_module(&self) -> Module {
        Module {
            name: self.name.clone(),
            kind: self.kind.clone(),
            version: Some(self.version.clone()),
            func: self.func.clone(),
            hash: if self.hash.is_empty() { None } else { Some(self.hash.clone()) },
            audit: self.audit.clone(),
            diff: if self.diff.is_empty() { None } else { Some(self.diff.clone()) },
            agent_modified: Some(self.agent_modified),
            node_modules: None,
            subdeps: vec![],
            vendored: vec![],
        }
    }

    pub fn decision(&self) -> Decision {
        decide_retention(&self.as_module())
    }
}

fn encode_id(id: &str) -> String {
    let mut out = String::new();
    for byte in id.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'@') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn record_path(state_dir: &Path, id: &str) -> PathBuf {
    state_dir.join("records").join(format!("{}.json", encode_id(id)))
}

pub fn write_record(state_dir: &Path, record: &Record) -> Result<(), ImmError> {
    let path = record_path(state_dir, &record.id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| ImmError::new("IO", &err.to_string()))?;
    }
    let mut text = serde_json::to_string_pretty(record).map_err(|err| ImmError::new("IO", &err.to_string()))?;
    text.push('\n');
    fs::write(path, text).map_err(|err| ImmError::new("IO", &err.to_string()))
}

pub fn read_record(state_dir: &Path, id: &str) -> Result<Record, ImmError> {
    let text = fs::read_to_string(record_path(state_dir, id)).map_err(|_| ImmError::new("MISSING", "missing record"))?;
    serde_json::from_str(&text).map_err(|err| ImmError::new("IO", &err.to_string()))
}

fn copy_tree(from: &Path, to: &Path) -> Result<Vec<RecordFile>, ImmError> {
    let mut files = Vec::new();
    copy_walk(from, from, to, &mut files)?;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn copy_walk(root: &Path, dir: &Path, dest_root: &Path, files: &mut Vec<RecordFile>) -> Result<(), ImmError> {
    let entries = fs::read_dir(dir).map_err(|err| ImmError::new("IO", &err.to_string()))?;
    for entry in entries {
        let entry = entry.map_err(|err| ImmError::new("IO", &err.to_string()))?;
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap_or(&path);
        let dest = dest_root.join(rel);
        if path.is_dir() {
            fs::create_dir_all(&dest).map_err(|err| ImmError::new("IO", &err.to_string()))?;
            copy_walk(root, &path, dest_root, files)?;
        } else {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|err| ImmError::new("IO", &err.to_string()))?;
            }
            let bytes = fs::read(&path).map_err(|err| ImmError::new("IO", &err.to_string()))?;
            fs::write(&dest, &bytes).map_err(|err| ImmError::new("IO", &err.to_string()))?;
            let rel_posix = rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
            files.push(RecordFile { path: rel_posix, sha256: sha256_hex(&bytes), bytes: bytes.len() });
        }
    }
    Ok(())
}

pub fn install_source(pkg_dir: &Path, version: &str, state_dir: &Path, now_ms: i64) -> Result<Record, ImmError> {
    let manifest_text = fs::read_to_string(pkg_dir.join("package.json")).map_err(|_| ImmError::new("MISSING", "missing package"))?;
    let manifest: Value = serde_json::from_str(&manifest_text).map_err(|err| ImmError::new("IO", &err.to_string()))?;
    let name = manifest.get("name").and_then(Value::as_str).unwrap_or("package").to_string();
    let proposal = Proposal {
        name: name.clone(),
        version: version.to_string(),
        range: Some(version.to_string()),
        published_at: manifest.get("publishedAt").and_then(Value::as_str).map(str::to_string),
        trust: manifest.get("trust").and_then(Value::as_str).map(str::to_string),
        previous_trust: manifest.get("previousTrust").and_then(Value::as_str).map(str::to_string),
        lifecycle_scripts: has_lifecycle_scripts(&manifest),
        integrity: manifest.get("integrity").and_then(Value::as_str).map(str::to_string),
    };
    let verdict = judge_update(&proposal, now_ms);
    if !verdict.accept {
        return Ok(Record {
            id: format!("{name}@{version}"),
            kind: "direct".to_string(),
            name,
            version: version.to_string(),
            func: None,
            action: "hold".to_string(),
            files: vec![],
            hash: String::new(),
            audit: None,
            agent_modified: false,
            diff: String::new(),
            reasons: verdict.reasons,
            executed: false,
        });
    }
    let dest = state_dir.join("installed").join(&name).join(version);
    if dest.exists() {
        fs::remove_dir_all(&dest).map_err(|err| ImmError::new("IO", &err.to_string()))?;
    }
    let files = copy_tree(pkg_dir, &dest)?;
    let hash = sha256_text(&files.iter().map(|file| fs::read_to_string(dest.join(&file.path)).unwrap_or_default()).collect::<Vec<_>>().join("\n"));
    let record = Record {
        id: format!("{name}@{version}"),
        kind: "direct".to_string(),
        name,
        version: version.to_string(),
        func: None,
        action: "install-source".to_string(),
        files,
        hash,
        audit: None,
        agent_modified: false,
        diff: String::new(),
        reasons: vec!["direct".to_string(), "whole-source".to_string()],
        executed: false,
    };
    write_record(state_dir, &record)?;
    Ok(record)
}

pub fn extract_use(pkg_dir: &Path, name: &str, version: &str, func: &str, state_dir: &Path, now_ms: i64) -> Result<Record, ImmError> {
    let slice = slice_package(pkg_dir, name, version, &[func], Budgets::default(), now_ms);
    let id = format!("{name}@{version}#{func}");
    if slice.blocked {
        return Ok(Record {
            id,
            kind: "use".to_string(),
            name: name.to_string(),
            version: version.to_string(),
            func: Some(func.to_string()),
            action: "hold".to_string(),
            files: vec![],
            hash: String::new(),
            audit: None,
            agent_modified: false,
            diff: String::new(),
            reasons: slice.reasons,
            executed: false,
        });
    }
    let body = slice.files.iter().map(|file| file.text.as_str()).collect::<Vec<_>>().join("\n");
    let root = state_dir.join("extracts").join(encode_id(&id));
    if root.exists() {
        fs::remove_dir_all(&root).map_err(|err| ImmError::new("IO", &err.to_string()))?;
    }
    let mut files = Vec::new();
    for file in &slice.files {
        let dest = root.join(&file.path);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|err| ImmError::new("IO", &err.to_string()))?;
        }
        fs::write(&dest, &file.text).map_err(|err| ImmError::new("IO", &err.to_string()))?;
        files.push(RecordFile { path: file.path.clone(), sha256: file.sha256.clone(), bytes: file.bytes });
    }
    let mut record = Record {
        id,
        kind: "use".to_string(),
        name: name.to_string(),
        version: version.to_string(),
        func: Some(func.to_string()),
        action: "hold".to_string(),
        files,
        hash: sha256_text(&body),
        audit: None,
        agent_modified: false,
        diff: String::new(),
        reasons: vec!["needs-audit".to_string()],
        executed: false,
    };
    let decision = record.decision();
    record.action = decision.action;
    record.reasons = decision.reasons;
    write_record(state_dir, &record)?;
    Ok(record)
}

pub fn audit_use(state_dir: &Path, id: &str, by: &str, at: &str) -> Result<Record, ImmError> {
    let mut record = read_record(state_dir, id)?;
    if record.hash.is_empty() {
        record.action = "hold".to_string();
        record.reasons = vec!["needs-hash".to_string()];
        write_record(state_dir, &record)?;
        return Ok(record);
    }
    record.audit = Some(Audit { by: by.to_string(), at: at.to_string() });
    let decision = record.decision();
    record.action = decision.action;
    record.reasons = decision.reasons;
    write_record(state_dir, &record)?;
    Ok(record)
}

pub fn lock_install(pkg_dir: &Path, name: &str, version: &str, state_dir: &Path) -> Result<Value, ImmError> {
    let manifest_text = fs::read_to_string(pkg_dir.join("package.json")).map_err(|_| ImmError::new("MISSING", "missing package"))?;
    let manifest: Value = serde_json::from_str(&manifest_text).map_err(|err| ImmError::new("IO", &err.to_string()))?;
    if manifest.get("version").and_then(Value::as_str) != Some(version) {
        return Ok(serde_json::json!({
            "ok": false,
            "action": "hold",
            "reasons": ["version-mismatch"],
            "executed": false,
            "hostAccess": false
        }));
    }
    let dest = state_dir.join("sandbox").join(name).join(version);
    if dest.exists() {
        fs::remove_dir_all(&dest).map_err(|err| ImmError::new("IO", &err.to_string()))?;
    }
    copy_tree(pkg_dir, &dest)?;
    let record = serde_json::json!({
        "ok": true,
        "action": "locked-install",
        "name": name,
        "version": version,
        "executed": false,
        "hostAccess": false,
        "scriptsExecuted": false,
        "scriptsIgnored": has_lifecycle_scripts(&manifest),
        "network": false
    });
    let mut text = serde_json::to_string_pretty(&record).map_err(|err| ImmError::new("IO", &err.to_string()))?;
    text.push('\n');
    fs::write(dest.join(".imm-lock.json"), text).map_err(|err| ImmError::new("IO", &err.to_string()))?;
    Ok(record)
}

/// Package code is never started. This is the only execution answer the tool gives.
pub fn execute_on_host() -> Value {
    serde_json::json!({
        "ok": false,
        "action": "refuse",
        "executed": false,
        "hostAccess": false,
        "reasons": ["host-execution"]
    })
}
