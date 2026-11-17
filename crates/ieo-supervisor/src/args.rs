//! Fixed role arguments (runtime contract §4.1, rev 6 §10 items 1 and 2).

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    /// cwd = /state/work/<repo>. `append_system_prompt_file`: the CLAUDE.md of the merged base
    /// commit, supplied explicitly because `--setting-sources user` does not load repo CLAUDE.md
    /// (§10 item 2); `None` records "not supplied".
    Development {
        append_system_prompt_file: Option<String>,
        /// Read-only tool restriction (02-05 step 8, PR #86 review 3e4e3d6 M-1): when set, the
        /// CLI offers only [`DEVELOPMENT_READ_TOOLS`], reads are scoped to the work tree and the
        /// supervisor's own paths are denied. Unset (the default, every accepted run so far): no
        /// tool options, as before.
        read_only: bool,
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

/// The tools a read-only development run is offered (`--tools`): no child agents, no shell,
/// no writes, no network.
pub const DEVELOPMENT_READ_TOOLS: &str = "Read,Grep,Glob";

/// Deny rules of a read-only development run: the supervisor's credentials, configuration and
/// state, and the process table (the Guardrail's list without its own paths).
pub const DEVELOPMENT_READ_ONLY_DISALLOWED: [&str; 6] = [
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
            read_only,
        } => {
            // The general development lists of §4.1 are still open (§9, set with 03-02). A
            // read-only run (02-05 step 8) gets a fixed restriction instead.
            if let Some(f) = append_system_prompt_file {
                a.extend(["--append-system-prompt-file".into(), f.clone()]);
            }
            if *read_only {
                a.push("--disable-slash-commands".into());
                a.extend(["--tools".into(), DEVELOPMENT_READ_TOOLS.into()]);
                // the rule forms 02-04 gave this CLI: a scoped `Read(//…)` (the CLI applies it to
                // Grep and Glob too) and plain `Grep`, `Glob` (PR #86 review a5f9402 L-8)
                a.push("--allowedTools".into());
                a.extend([
                    format!("Read(//state/work/{repo}/**)"),
                    "Grep".into(),
                    "Glob".into(),
                ]);
                a.push("--disallowedTools".into());
                a.extend(
                    DEVELOPMENT_READ_ONLY_DISALLOWED
                        .iter()
                        .map(|s| s.to_string()),
                );
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
