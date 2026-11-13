//! `CLAUDE_CONFIG_DIR` directive digest, tamper detection and cleanup (runtime contract §5-6,
//! rev 6 §10 item 3, §4.1).
//!
//! Only *directive* items are digested: `settings*.json`, `CLAUDE.md`, `agents/`, `skills/`,
//! `commands/`, `output-styles/`, `hooks/`, `plugins/` (except the CLI's own
//! `plugins/store/*_builtin-*.json`), `.mcp.json`, `projects/*/memory/`, and from `.claude.json`
//! only `mcpServers`, `projects.*.mcpServers`, `projects.*.allowedTools`,
//! `projects.*.enabledMcpjsonServers`. Items the CLI writes on every normal run are outside it.
//!
//! Symbolic links are never followed. A link inside the directive scope is digested by its
//! target text (so planting one is tamper), and cleanup removes the link itself. A directory
//! or file that cannot be read makes [`directive_digest`] fail; [`observed`] turns that into a
//! value that never equals a digest, so the reconnect decision reports tamper (fail closed).
//!
//! Cleanup keeps only regular files `projects/<cwd>/*.jsonl` (session records needed by
//! `--resume`), removes everything else (restoring owner permissions on directories the role
//! made unreadable), records what it removed — credential files separately (§4.1) — and then
//! installs the read-only template, which must disable auto memory.

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const DIRECTIVE_DIRS: [&str; 7] = [
    "agents",
    "skills",
    "commands",
    "output-styles",
    "hooks",
    "plugins",
    "projects",
];

#[derive(Debug, Clone, PartialEq, Eq)]
enum Entry {
    File(PathBuf),
    Link(PathBuf),
}

/// Read-only walk: never follows links, propagates every error.
fn walk(dir: &Path, out: &mut Vec<Entry>) -> io::Result<()> {
    let mut entries = std::fs::read_dir(dir)?
        .map(|e| e.map(|e| e.path()))
        .collect::<io::Result<Vec<_>>>()?;
    entries.sort();
    for p in entries {
        let ft = std::fs::symlink_metadata(&p)?.file_type();
        if ft.is_symlink() {
            out.push(Entry::Link(p));
        } else if ft.is_dir() {
            walk(&p, out)?;
        } else {
            out.push(Entry::File(p));
        }
    }
    Ok(())
}

fn is_directive(rel: &str) -> bool {
    let parts: Vec<&str> = rel.split('/').collect();
    match parts.as_slice() {
        [f] => {
            (f.starts_with("settings") && f.ends_with(".json"))
                || *f == "CLAUDE.md"
                || *f == ".mcp.json"
                || (*f != "projects" && DIRECTIVE_DIRS.contains(f))
        }
        ["plugins", "store", f] if f.contains("_builtin-") && f.ends_with(".json") => false,
        ["projects", _, "memory", ..] => true,
        ["projects", ..] => false,
        [d, ..] => DIRECTIVE_DIRS.contains(d),
        [] => false,
    }
}

fn claude_json_directives(dir: &Path) -> io::Result<Value> {
    // a missing file and a file without directive keys digest the same (normal CLI writes);
    // an unreadable or non-JSON file is an error
    let v: Value = match std::fs::read(dir.join(".claude.json")) {
        Ok(raw) => serde_json::from_slice(&raw).map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidData, format!(".claude.json: {e}"))
        })?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => Value::Null,
        Err(e) => return Err(e),
    };
    let mut projects = serde_json::Map::new();
    for (k, p) in v["projects"].as_object().into_iter().flatten() {
        let keep = json!({"mcpServers": p["mcpServers"], "allowedTools": p["allowedTools"],
                          "enabledMcpjsonServers": p["enabledMcpjsonServers"]});
        if keep
            .as_object()
            .is_some_and(|m| m.values().any(|x| !x.is_null()))
        {
            projects.insert(k.clone(), keep);
        }
    }
    Ok(json!({"mcpServers": v["mcpServers"], "projects": projects}))
}

fn rel(dir: &Path, p: &Path) -> String {
    p.strip_prefix(dir)
        .map(|r| r.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// SHA-256 over the sorted (path, kind, content or link-target digest) list of directive items
/// plus the directive keys of `.claude.json`. Any read error is returned.
pub fn directive_digest(dir: &Path) -> io::Result<String> {
    let mut entries = Vec::new();
    walk(dir, &mut entries)?;
    let mut h = Sha256::new();
    for e in entries {
        let (p, kind, content) = match &e {
            Entry::File(p) => (p, b'f', std::fs::read(p)?),
            Entry::Link(p) => (
                p,
                b'l',
                std::fs::read_link(p)?.to_string_lossy().as_bytes().to_vec(),
            ),
        };
        let r = rel(dir, p);
        if (r == ".claude.json" && kind == b'f') || !is_directive(&r) {
            continue;
        }
        h.update(r.as_bytes());
        h.update([0, kind]);
        h.update(Sha256::digest(&content));
    }
    h.update(b".claude.json\0");
    h.update(claude_json_directives(dir)?.to_string().as_bytes());
    Ok(hex::encode(h.finalize()))
}

/// The value the reconnect decision compares: the digest, or `unreadable: <error>` (never equal
/// to a checkpoint digest, so it is reported as tamper).
pub fn observed(dir: &Path) -> String {
    directive_digest(dir).unwrap_or_else(|e| format!("unreadable: {e}"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanReport {
    /// Directive digest after cleanup (the checkpoint's `settings_digest`).
    pub digest: String,
    /// Removed paths (relative), links included.
    pub removed: Vec<String>,
    /// Removed credential files (§4.1: deleted before start and recorded).
    pub credentials_removed: Vec<String>,
}

fn is_credential(rel: &str) -> bool {
    let name = rel.rsplit('/').next().unwrap_or(rel).to_ascii_lowercase();
    name.contains("credential") || name.contains("oauth") || name.contains("token")
}

/// Remove everything under `d` except regular files `projects/<x>/*.jsonl`; restores owner
/// rwx on directories first so a role cannot hide items by removing permissions.
fn clean_dir(root: &Path, d: &Path, report: &mut CleanReport) -> io::Result<()> {
    std::fs::set_permissions(d, std::fs::Permissions::from_mode(0o700))?;
    let entries = std::fs::read_dir(d)?
        .map(|e| e.map(|e| e.path()))
        .collect::<io::Result<Vec<_>>>()?;
    for p in entries {
        let r = rel(root, &p);
        let ft = std::fs::symlink_metadata(&p)?.file_type();
        if ft.is_dir() {
            clean_dir(root, &p, report)?;
            if std::fs::read_dir(&p)?.next().is_none() {
                std::fs::remove_dir(&p)?;
            }
            continue;
        }
        let parts: Vec<&str> = r.split('/').collect();
        let keep = ft.is_file()
            && matches!(parts.as_slice(), ["projects", _, name] if name.ends_with(".jsonl"));
        if !keep {
            std::fs::remove_file(&p)?; // a link: removes the link, never its target
            if is_credential(&r) {
                report.credentials_removed.push(r.clone());
            }
            report.removed.push(r);
        }
    }
    Ok(())
}

/// Clean `dir` and install `template` (read-only role settings). The template's
/// `settings.json` must set `autoMemoryEnabled: false`.
pub fn clean(dir: &Path, template: &Path) -> io::Result<CleanReport> {
    let settings: Value = serde_json::from_slice(&std::fs::read(template.join("settings.json"))?)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    if settings["autoMemoryEnabled"] != false {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "template settings.json does not set autoMemoryEnabled: false",
        ));
    }
    let mut report = CleanReport {
        digest: String::new(),
        removed: vec![],
        credentials_removed: vec![],
    };
    clean_dir(dir, dir, &mut report)?;
    let mut tfiles = Vec::new();
    walk(template, &mut tfiles)?;
    for e in tfiles {
        let Entry::File(f) = e else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "template contains a symbolic link",
            ));
        };
        let to = dir.join(rel(template, &f));
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&f, &to)?;
    }
    report.digest = directive_digest(dir)?;
    Ok(report)
}
