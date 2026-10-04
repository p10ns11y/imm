use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use crate::hash::sha256_text;
use crate::policy::{has_lifecycle_scripts, judge_update, Proposal};

#[derive(Debug, Clone, Copy)]
pub struct Budgets {
    pub max_packages: usize,
    pub max_files: usize,
    pub max_bytes: usize,
    pub max_depth: usize,
}

impl Default for Budgets {
    fn default() -> Self {
        Self { max_packages: 8, max_files: 40, max_bytes: 200_000, max_depth: 6 }
    }
}

#[derive(Debug, Clone)]
pub struct SliceFile {
    pub path: String,
    pub depth: usize,
    pub bytes: usize,
    pub sha256: String,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct Slice {
    pub name: String,
    pub version: String,
    pub exports: Vec<String>,
    pub blocked: bool,
    pub reasons: Vec<String>,
    pub files: Vec<SliceFile>,
    pub omitted: Vec<String>,
}

#[derive(Clone)]
struct Parsed {
    local: HashSet<String>,
    reexports: HashMap<String, (String, String)>,
    imports: Vec<String>,
    reasons: Vec<String>,
}

pub fn format_slice(slice: &Slice) -> String {
    let mut lines = vec![
        format!("{}@{}", slice.name, slice.version),
        if slice.blocked { "blocked".to_string() } else { "accept".to_string() },
    ];
    if !slice.reasons.is_empty() {
        lines.push(format!("reasons {}", slice.reasons.join(" ")));
    }
    lines.push("keep".to_string());
    if slice.files.is_empty() {
        lines.push("  (none)".to_string());
    }
    for file in &slice.files {
        lines.push(format!("  {}", file.path));
    }
    lines.push("omit".to_string());
    if slice.omitted.is_empty() {
        lines.push("  (none)".to_string());
    }
    for rel in &slice.omitted {
        lines.push(format!("  {rel}"));
    }
    lines.join("\n")
}

pub fn slice_package(pkg_dir: &Path, name: &str, version: &str, exports: &[&str], budgets: Budgets, now_ms: i64) -> Slice {
    let manifest_path = pkg_dir.join("package.json");
    if !manifest_path.is_file() {
        return blank(name, version, exports, vec!["missing-package".to_string()]);
    }
    let manifest: Value = match fs::read_to_string(&manifest_path).ok().and_then(|text| serde_json::from_str(&text).ok()) {
        Some(value) => value,
        None => return blank(name, version, exports, vec!["missing-package".to_string()]),
    };

    let mut reasons = Vec::new();
    if exports.is_empty() {
        reasons.push("missing-export".to_string());
    }
    for export_name in exports {
        if !export_name_ok(export_name) {
            reasons.push(format!("bad-export-name:{export_name}"));
        }
    }
    let manifest_version = manifest.get("version").and_then(Value::as_str).unwrap_or("");
    if version != manifest_version {
        reasons.push("version-mismatch".to_string());
    }

    let proposal = Proposal {
        name: manifest.get("name").and_then(Value::as_str).unwrap_or(name).to_string(),
        version: version.to_string(),
        range: Some(version.to_string()),
        published_at: manifest.get("publishedAt").and_then(Value::as_str).map(str::to_string),
        trust: manifest.get("trust").and_then(Value::as_str).map(str::to_string),
        previous_trust: manifest.get("previousTrust").and_then(Value::as_str).map(str::to_string),
        lifecycle_scripts: has_lifecycle_scripts(&manifest),
        integrity: manifest.get("integrity").and_then(Value::as_str).filter(|text| !text.is_empty()).map(str::to_string),
    };
    reasons.extend(judge_update(&proposal, now_ms).reasons);

    let mut files = Vec::new();
    let mut seen = HashSet::new();
    let mut cache: HashMap<PathBuf, Parsed> = HashMap::new();
    let entry = normalize(&pkg_dir.join(entry_rel(&manifest)));
    let names_ok = reasons.iter().all(|reason| !reason.starts_with("bad-export-name"));
    if !entry.is_file() {
        reasons.push("missing-file".to_string());
    } else if names_ok {
        for export_name in exports {
            reasons.extend(trace_export(pkg_dir, &mut cache, &entry, export_name, budgets.max_depth, &mut seen, &mut files));
        }
    }

    let kept: HashSet<&str> = files.iter().map(|file| file.path.as_str()).collect();
    let omitted = list_js(pkg_dir).into_iter().filter(|rel| !kept.contains(rel.as_str())).collect();
    let reasons = unique(reasons);
    Slice {
        name: name.to_string(),
        version: version.to_string(),
        exports: exports.iter().map(|item| (*item).to_string()).collect(),
        blocked: !reasons.is_empty(),
        reasons,
        files,
        omitted,
    }
}

fn blank(name: &str, version: &str, exports: &[&str], reasons: Vec<String>) -> Slice {
    Slice {
        name: name.to_string(),
        version: version.to_string(),
        exports: exports.iter().map(|item| (*item).to_string()).collect(),
        blocked: true,
        reasons,
        files: vec![],
        omitted: vec![],
    }
}

fn entry_rel(manifest: &Value) -> String {
    match manifest.get("exports").and_then(|exports| exports.get(".")) {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Object(map)) => map
            .get("import")
            .and_then(Value::as_str)
            .or_else(|| map.get("default").and_then(Value::as_str))
            .unwrap_or("./src/index.js")
            .to_string(),
        _ => "./src/index.js".to_string(),
    }
}

fn export_name_ok(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(ch) if ch.is_ascii_alphabetic() || ch == '_' || ch == '$' => {}
        _ => return false,
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
}

fn unique(reasons: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for reason in reasons {
        if !out.contains(&reason) {
            out.push(reason);
        }
    }
    out
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn rel_posix(root: &Path, abs: &Path) -> String {
    abs.strip_prefix(root)
        .unwrap_or(abs)
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn inside(pkg_dir: &Path, abs: &Path) -> bool {
    let pkg = normalize(pkg_dir);
    let abs = normalize(abs);
    abs == pkg || abs.starts_with(&pkg)
}

fn resolve_inside(pkg_dir: &Path, from_abs: &Path, rel: &str) -> Result<PathBuf, &'static str> {
    let abs = normalize(&from_abs.parent().unwrap_or(from_abs).join(rel));
    if !inside(pkg_dir, &abs) {
        return Err("path-escape");
    }
    if !abs.is_file() {
        return Err("missing-file");
    }
    Ok(abs)
}

fn parse_module(text: &str) -> Parsed {
    let mut local = HashSet::new();
    let mut reexports = HashMap::new();
    let mut imports = Vec::new();
    let mut reasons = Vec::new();
    for raw in text.split(['\n', '\r']) {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if line.starts_with("export * from") || line.starts_with("export *  from") {
            reasons.push("star-reexport".to_string());
        }
        if line.starts_with("export *") && line.contains(" from") {
            reasons.push("star-reexport".to_string());
        }
        if line.starts_with("import ") && !line.contains(" from \"") && !line.contains(" from '") && !import_side(line).is_some() {
            reasons.push("parse-limit".to_string());
        }
        if let Some(rel) = import_side(line) {
            imports.push(rel);
        }
        if let Some(rel) = import_from(line) {
            imports.push(rel);
        }
        if let Some((names, rel)) = export_from(line) {
            for part in names.split(',') {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }
                let mut bits = part.split(" as ");
                let source = bits.next().unwrap_or("").trim();
                let exported = bits.next().unwrap_or(source).trim();
                if source.is_empty() || exported.is_empty() {
                    continue;
                }
                reexports.insert(exported.to_string(), (rel.clone(), source.to_string()));
            }
        }
        if let Some(name) = after_keyword(line, "export function ") {
            local.insert(name);
        }
        if let Some(name) = after_keyword(line, "export const ") {
            local.insert(name);
        }
    }
    Parsed { local, reexports, imports, reasons }
}

fn after_keyword(line: &str, prefix: &str) -> Option<String> {
    let rest = line.strip_prefix(prefix)?;
    let name: String = rest.chars().take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '$').collect();
    if export_name_ok(&name) { Some(name) } else { None }
}

fn quoted_rel(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let quote = *bytes.first()?;
    if quote != b'"' && quote != b'\'' {
        return None;
    }
    let end = bytes.iter().skip(1).position(|byte| *byte == quote)?;
    let rel = text.get(1..1 + end)?;
    if rel.starts_with('.') { Some(rel.to_string()) } else { None }
}

fn import_side(line: &str) -> Option<String> {
    let rest = line.strip_prefix("import ")?.trim_start();
    quoted_rel(rest)
}

fn import_from(line: &str) -> Option<String> {
    if !line.starts_with("import ") {
        return None;
    }
    let marker = line.find(" from ").or_else(|| line.find(" from\t"))?;
    quoted_rel(line[marker + " from ".len()..].trim_start())
}

fn export_from(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("export ")?.trim_start();
    let rest = rest.strip_prefix('{')?;
    let end = rest.find('}')?;
    let names = rest[..end].to_string();
    let after = rest[end + 1..].trim_start();
    let after = after.strip_prefix("from")?.trim_start();
    Some((names, quoted_rel(after)?))
}

fn forbidden(text: &str, rel: &str) -> Vec<String> {
    let mut hits = Vec::new();
    if text.contains("child_process") {
        hits.push(format!("child_process:{rel}"));
    }
    if contains_call(text, "require") {
        hits.push(format!("dynamic-require:{rel}"));
    }
    if contains_word_call(text, "eval") {
        hits.push(format!("eval:{rel}"));
    }
    if contains_word_call(text, "Function") {
        hits.push(format!("function-constructor:{rel}"));
    }
    if text.contains("http://") || text.contains("https://") {
        hits.push(format!("network-url:{rel}"));
    }
    if text.contains("process.binding") {
        hits.push(format!("process-binding:{rel}"));
    }
    hits
}

fn contains_call(text: &str, name: &str) -> bool {
    let bytes = text.as_bytes();
    let needle = name.as_bytes();
    bytes.windows(needle.len()).enumerate().any(|(index, window)| {
        if window != needle {
            return false;
        }
        let mut cursor = index + needle.len();
        while cursor < bytes.len() && bytes[cursor] == b' ' {
            cursor += 1;
        }
        cursor < bytes.len() && bytes[cursor] == b'('
    })
}

fn contains_word_call(text: &str, name: &str) -> bool {
    let bytes = text.as_bytes();
    let needle = name.as_bytes();
    bytes.windows(needle.len()).enumerate().any(|(index, window)| {
        if window != needle {
            return false;
        }
        let before_ok = index == 0 || !is_ident(bytes[index - 1]);
        if !before_ok {
            return false;
        }
        let mut cursor = index + needle.len();
        while cursor < bytes.len() && bytes[cursor] == b' ' {
            cursor += 1;
        }
        cursor < bytes.len() && bytes[cursor] == b'('
    })
}

fn is_ident(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

fn list_js(pkg_dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    walk_js(pkg_dir, pkg_dir, &mut out);
    out.sort();
    out
}

fn walk_js(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_js(root, &path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("js") {
            out.push(rel_posix(root, &path));
        }
    }
}

fn read_cached(cache: &mut HashMap<PathBuf, Parsed>, abs: &Path) -> Parsed {
    if let Some(parsed) = cache.get(abs) {
        return parsed.clone();
    }
    let text = fs::read_to_string(abs).unwrap_or_default();
    let parsed = parse_module(&text);
    cache.insert(abs.to_path_buf(), parsed.clone());
    parsed
}

fn push_file(pkg_dir: &Path, abs: &Path, depth: usize, files: &mut Vec<SliceFile>, seen: &mut HashSet<PathBuf>) -> Vec<String> {
    if !seen.insert(abs.to_path_buf()) {
        return vec![];
    }
    let text = fs::read_to_string(abs).unwrap_or_default();
    let rel = rel_posix(pkg_dir, abs);
    let hits = forbidden(&text, &rel);
    files.push(SliceFile {
        path: rel,
        depth,
        bytes: text.len(),
        sha256: sha256_text(&text),
        text,
    });
    hits
}

fn walk_imports(
    pkg_dir: &Path,
    cache: &mut HashMap<PathBuf, Parsed>,
    start: &Path,
    start_depth: usize,
    max_depth: usize,
    seen: &mut HashSet<PathBuf>,
    files: &mut Vec<SliceFile>,
) -> Vec<String> {
    let mut reasons = Vec::new();
    let mut queued = seen.clone();
    let mut queue = vec![(start.to_path_buf(), start_depth)];
    while let Some((current, depth)) = queue.first().cloned() {
        queue.remove(0);
        let module = read_cached(cache, &current);
        reasons.extend(module.reasons);
        for rel in module.imports {
            match resolve_inside(pkg_dir, &current, &rel) {
                Err(error) => reasons.push(error.to_string()),
                Ok(next) => {
                    let child_depth = depth + 1;
                    if child_depth > max_depth {
                        reasons.push("budget-depth".to_string());
                        continue;
                    }
                    if !queued.insert(next.clone()) {
                        continue;
                    }
                    reasons.extend(push_file(pkg_dir, &next, child_depth, files, seen));
                    queue.push((next, child_depth));
                }
            }
        }
    }
    reasons
}

fn trace_export(
    pkg_dir: &Path,
    cache: &mut HashMap<PathBuf, Parsed>,
    entry: &Path,
    export_name: &str,
    max_depth: usize,
    seen: &mut HashSet<PathBuf>,
    files: &mut Vec<SliceFile>,
) -> Vec<String> {
    let mut reasons = Vec::new();
    let mut abs = entry.to_path_buf();
    let mut name = export_name.to_string();
    let mut depth = 0usize;
    let mut guard = HashSet::new();
    loop {
        let mark = format!("{}#{name}", abs.display());
        if !guard.insert(mark) {
            reasons.push("cycle".to_string());
            break;
        }
        if depth > max_depth {
            reasons.push("budget-depth".to_string());
            break;
        }
        let module = read_cached(cache, &abs);
        reasons.extend(module.reasons.clone());
        reasons.extend(push_file(pkg_dir, &abs, depth, files, seen));
        if module.local.contains(&name) {
            reasons.extend(walk_imports(pkg_dir, cache, &abs, depth, max_depth, seen, files));
            break;
        }
        let Some((rel, source_name)) = module.reexports.get(&name).cloned() else {
            reasons.push(format!("missing-export:{export_name}"));
            break;
        };
        match resolve_inside(pkg_dir, &abs, &rel) {
            Err(error) => {
                reasons.push(error.to_string());
                break;
            }
            Ok(next) => {
                abs = next;
                name = source_name;
                depth += 1;
            }
        }
    }
    reasons
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeutil::parse_unix_ms;

    fn now() -> i64 {
        parse_unix_ms("2026-09-22T12:00:00.000Z").unwrap()
    }

    fn budgets() -> Budgets {
        Budgets { max_packages: 4, max_files: 20, max_bytes: 100_000, max_depth: 6 }
    }

    fn slice(name: &str, version: &str, exports: &[&str], budgets: Budgets) -> Slice {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/registry").join(name);
        slice_package(&dir, name, version, exports, budgets, now())
    }

    #[test]
    fn keeps_the_slugify_graph_and_omits_the_unused_cli() {
        let pkg = slice("pure-slug", "1.0.0", &["slugify"], budgets());
        assert!(!pkg.blocked);
        let paths: Vec<_> = pkg.files.iter().map(|file| file.path.as_str()).collect();
        assert_eq!(paths, ["src/index.js", "src/slugify.js", "src/unicode.js"]);
        assert_eq!(pkg.omitted, ["src/cli.js"]);
    }

    #[test]
    fn blocks_a_reached_child_process_and_does_not_execute_a_throw() {
        let dangerous = slice("shell-out", "1.0.0", &["slugify"], budgets());
        assert!(dangerous.blocked);
        assert!(dangerous.reasons.iter().any(|reason| reason.starts_with("child_process:")));
        let side = slice("side-effect", "1.0.0", &["ok"], budgets());
        assert!(!side.blocked);
        assert!(side.files.iter().any(|file| file.path == "src/boom.js"));
    }

    #[test]
    fn blocks_fresh_trust_lifecycle_and_a_shallow_budget() {
        assert!(slice("fresh-slug", "9.9.9", &["slugify"], budgets()).reasons.iter().any(|r| r == "too-fresh"));
        assert!(slice("trust-drop", "2.0.0", &["slugify"], budgets()).reasons.iter().any(|r| r == "trust-downgrade"));
        assert!(slice("install-script", "1.0.0", &["slugify"], budgets()).reasons.iter().any(|r| r == "lifecycle-script"));
        let mut shallow = budgets();
        shallow.max_depth = 1;
        let pkg = slice("pure-slug", "1.0.0", &["slugify"], shallow);
        assert!(pkg.reasons.iter().any(|r| r == "budget-depth"));
        assert!(!pkg.files.iter().any(|file| file.path == "src/unicode.js"));
    }
}
