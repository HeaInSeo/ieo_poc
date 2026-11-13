//! Fixed role arguments (runtime contract §4.1, rev 6 §10 items 1 and 2).

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    /// cwd = /state/work/<repo>. `append_system_prompt_file`: the CLAUDE.md of the merged base
    /// commit, supplied explicitly because `--setting-sources user` does not load repo CLAUDE.md
    /// (§10 item 2); `None` records "not supplied".
    Development {
        append_system_prompt_file: Option<String>,
    },
    Guardrail,
}

pub const ROLE_DIR: &str = "/etc/ieo/role";

/// Guardrail allow rules (§4.1): file writes as `Edit(//…)`, never `Write(` (§10 item 1).
pub const GUARDRAIL_ALLOWED: [&str; 4] = [
    "Read(//state/review/**)",
    "Read(//state/inputs/**)",
    "Read(//state/guardrail-scratch/**)",
    "Edit(//state/guardrail-scratch/**)",
];

/// Guardrail deny rules (§4.1).
pub const GUARDRAIL_DISALLOWED: [&str; 6] = [
    "Read(//run/ieo/**)",
    "Read(//etc/ieo/**)",
    "Read(//state/claude/**)",
    "Read(//state/home/**)",
    "Read(//proc/**)",
    "Bash",
];

/// `claude -p …` argv (without the program name). `resume` = the checkpoint's session id;
/// otherwise a new session with the coordinator-issued `session_id`.
pub fn build(role: &Role, repo: &str, session_id: &str, resume: bool) -> Vec<String> {
    let mut a: Vec<String> = [
        "-p",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--verbose",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    if resume {
        a.extend(["--resume".into(), session_id.into()]);
    } else {
        a.extend(["--session-id".into(), session_id.into()]);
    }
    let name = match role {
        Role::Development { .. } => format!("{repo}-development"),
        Role::Guardrail => format!("{repo}-guardrail"),
    };
    a.extend(["-n".into(), name]);
    for (k, v) in [
        ("--permission-mode", "dontAsk".to_string()),
        ("--permission-prompts", "none".to_string()),
        ("--setting-sources", "user".to_string()),
        ("--settings", format!("{ROLE_DIR}/settings.json")),
        ("--mcp-config", format!("{ROLE_DIR}/mcp.json")),
        ("--agents", format!("{ROLE_DIR}/agents.json")),
    ] {
        a.extend([k.to_string(), v]);
    }
    a.push("--strict-mcp-config".into());
    match role {
        Role::Development {
            append_system_prompt_file,
        } => {
            // Not here yet: the development role's --allowedTools/--disallowedTools lists. §4.1
            // names them, but their content is open (§9) and is set with 03-02; until then the
            // development role relies on the read-only settings and permission mode.
            if let Some(f) = append_system_prompt_file {
                a.extend(["--append-system-prompt-file".into(), f.clone()]);
            }
        }
        Role::Guardrail => {
            a.push("--disable-slash-commands".into());
            a.extend(["--tools".into(), "Read,Grep,Glob,Write,Edit".into()]);
            // one argv entry per rule (the options are variadic); 01-04 never passed these
            // two options, so their acceptance is checked in the 02-04 real-session tests
            a.push("--allowedTools".into());
            a.extend(GUARDRAIL_ALLOWED.iter().map(|s| s.to_string()));
            a.push("--disallowedTools".into());
            a.extend(GUARDRAIL_DISALLOWED.iter().map(|s| s.to_string()));
        }
    }
    a
}

/// Working directory of the role (§4.1).
pub fn cwd(role: &Role, repo: &str) -> String {
    match role {
        Role::Development { .. } => format!("/state/work/{repo}"),
        Role::Guardrail => "/state/guardrail-cwd".into(),
    }
}
