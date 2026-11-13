//! Role process environment: empty, then only the allow-list (runtime contract §4.3).

/// Variable that carries the role credential (subscription OAuth token). Its value is read
/// from the credential file by the supervisor and never logged.
pub const CREDENTIAL_VAR: &str = "CLAUDE_CODE_OAUTH_TOKEN";

/// Names that may reach the role process. Anything else (e.g. `ANTHROPIC_API_KEY`, which
/// would switch billing to the paid API, or the parent session's `CLAUDECODE`/`CLAUDE_CODE_*`)
/// is not passed.
pub const ALLOWED: [&str; 5] = ["HOME", "PATH", "LANG", "CLAUDE_CONFIG_DIR", CREDENTIAL_VAR];

/// Build the environment. `credential` is the token value read from the role's credential
/// file; the returned list is passed to the process, never printed.
pub fn build(home: &str, config_dir: &str, credential: &str) -> Vec<(String, String)> {
    vec![
        ("HOME".into(), home.into()),
        ("PATH".into(), "/usr/local/bin:/usr/bin:/bin".into()),
        ("LANG".into(), "C.UTF-8".into()),
        ("CLAUDE_CONFIG_DIR".into(), config_dir.into()),
        (CREDENTIAL_VAR.into(), credential.into()),
    ]
}

/// Names only, for evidence records.
pub fn names(env: &[(String, String)]) -> Vec<String> {
    env.iter().map(|(k, _)| k.clone()).collect()
}
