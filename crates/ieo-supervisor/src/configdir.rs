//! `CLAUDE_CONFIG_DIR` directive digest, tamper detection and cleanup (runtime contract §5-6,
//! rev 6 §10 item 3).
//!
//! Only *directive* items are digested: `settings*.json`, `CLAUDE.md`, `agents/`, `skills/`,
//! `commands/`, `output-styles/`, `hooks/`, `plugins/` (except the CLI's own
//! `plugins/store/*_builtin-*.json`), `.mcp.json`, `projects/*/memory/`, and from `.claude.json`
//! only `mcpServers`, `projects.*.mcpServers`, `projects.*.allowedTools`,
//! `projects.*.enabledMcpjsonServers`. Items the CLI writes on every normal run are outside it.
//! Cleanup keeps only `projects/<cwd>/*.jsonl` (session records needed by `--resume`) and then
//! installs the read-only template.

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
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

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                walk(&p, out);
            } else {
                out.push(p);
            }
        }
    }
}

fn is_directive(rel: &str) -> bool {
    let parts: Vec<&str> = rel.split('/').collect();
    match parts.as_slice() {
        [f] => {
            (f.starts_with("settings") && f.ends_with(".json"))
                || *f == "CLAUDE.md"
                || *f == ".mcp.json"
        }
        ["plugins", "store", f] if f.contains("_builtin-") && f.ends_with(".json") => false,
        ["projects", _, "memory", ..] => true,
        ["projects", ..] => false,
        [d, ..] => DIRECTIVE_DIRS.contains(d),
        [] => false,
    }
}

fn claude_json_directives(dir: &Path) -> Value {
    // a missing file and a file without directive keys digest the same (normal CLI writes)
    let v: Value = std::fs::read(dir.join(".claude.json"))
        .ok()
        .and_then(|raw| serde_json::from_slice(&raw).ok())
        .unwrap_or(Value::Null);
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
    json!({"mcpServers": v["mcpServers"], "projects": projects})
}

/// SHA-256 over the sorted (path, content digest) list of directive items plus the directive
/// keys of `.claude.json`.
pub fn directive_digest(dir: &Path) -> String {
    let mut files = Vec::new();
    walk(dir, &mut files);
    let mut h = Sha256::new();
    for f in files {
        let Ok(rel) = f.strip_prefix(dir) else {
            continue;
        };
        let rel = rel.to_string_lossy().to_string();
        if rel == ".claude.json" || !is_directive(&rel) {
            continue;
        }
        let content = std::fs::read(&f).unwrap_or_default();
        h.update(rel.as_bytes());
        h.update([0]);
        h.update(Sha256::digest(&content));
    }
    h.update(b".claude.json\0");
    h.update(claude_json_directives(dir).to_string().as_bytes());
    hex::encode(h.finalize())
}

/// Remove everything except `projects/<cwd>/*.jsonl`, then copy `template` (read-only role
/// settings) into `dir`. Returns the directive digest after cleanup (the checkpoint value).
pub fn clean(dir: &Path, template: &Path) -> std::io::Result<String> {
    let mut files = Vec::new();
    walk(dir, &mut files);
    for f in files {
        let rel = f
            .strip_prefix(dir)
            .map(|r| r.to_string_lossy().to_string())
            .unwrap_or_default();
        let parts: Vec<&str> = rel.split('/').collect();
        let keep = matches!(parts.as_slice(), ["projects", _, name] if name.ends_with(".jsonl"));
        if !keep {
            std::fs::remove_file(&f)?;
        }
    }
    // drop now-empty directories (kept session directories stay)
    fn prune(d: &Path, root: &Path) {
        if let Ok(rd) = std::fs::read_dir(d) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    prune(&p, root);
                    if p != root
                        && std::fs::read_dir(&p)
                            .map(|mut x| x.next().is_none())
                            .unwrap_or(false)
                    {
                        let _ = std::fs::remove_dir(&p);
                    }
                }
            }
        }
    }
    prune(dir, dir);
    let mut tfiles = Vec::new();
    walk(template, &mut tfiles);
    for f in tfiles {
        let Ok(rel) = f.strip_prefix(template) else {
            continue;
        };
        let to = dir.join(rel);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&f, &to)?;
    }
    Ok(directive_digest(dir))
}
