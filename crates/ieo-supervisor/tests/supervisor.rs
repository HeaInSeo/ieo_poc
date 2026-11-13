//! 02-04 supervisor core tests (no model calls).

use ieo_supervisor::args::{self, Role};
use ieo_supervisor::budget::{self, BUDGET_0204, BUDGET_X02, Decision};
use ieo_supervisor::checkpoint::{self, Reconnect};
use ieo_supervisor::configdir;
use ieo_supervisor::env;
use ieo_supervisor::lock::{LockError, SessionLock};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const SID: &str = "6f1d2c3b-0000-4000-8000-000000000001";

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "ieo-sup-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    ));
    std::fs::create_dir_all(&d).expect("tmp");
    d
}

fn put(root: &Path, rel: &str, body: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
    std::fs::write(p, body).expect("write");
}

// ---- §4.1 / §10 items 1, 2, 5 ----

#[test]
fn role_arguments_use_edit_paths_and_session_id_only() {
    let dev = Role::Development {
        append_system_prompt_file: Some("/run/ieo/claude-md/CLAUDE.md".into()),
    };
    for role in [dev.clone(), Role::Guardrail] {
        for resume in [false, true] {
            let a = args::build(&role, "sori", SID, resume);
            assert!(a.iter().all(|x| !x.contains("Write(")), "{a:?}");
            assert!(
                a.windows(2)
                    .any(|w| w[0] == "--setting-sources" && w[1] == "user")
            );
            assert!(
                !a.iter()
                    .any(|x| x == "--dangerously-skip-permissions" || x == "bypassPermissions")
            );
            // the resume key is the session id, never the -n name (§10 item 5)
            let key = if resume { "--resume" } else { "--session-id" };
            assert!(a.windows(2).any(|w| w[0] == key && w[1] == SID), "{a:?}");
            let name = a
                .windows(2)
                .find(|w| w[0] == "-n")
                .map(|w| w[1].clone())
                .unwrap_or_default();
            assert!(!a.windows(2).any(|w| w[0] == "--resume" && w[1] == name));
        }
    }
    let g = args::build(&Role::Guardrail, "sori", SID, false);
    assert!(
        g.iter()
            .any(|x| x.contains("Edit(//state/guardrail-scratch/**)"))
    );
    assert!(
        g.windows(2)
            .any(|w| w[0] == "--disallowedTools" && w[1].contains("Bash"))
    );
    assert!(!g.iter().any(|x| x == "--add-dir"));
    let d = args::build(&dev, "sori", SID, false);
    assert!(d.windows(2).any(|w| w[0] == "--append-system-prompt-file"));
    assert_eq!(args::cwd(&Role::Guardrail, "sori"), "/state/guardrail-cwd");
}

#[test]
fn environment_is_the_allow_list_only() {
    let e = env::build("/state/home", "/state/claude", "credential-value");
    assert_eq!(env::names(&e), env::ALLOWED.to_vec());
    assert!(
        !env::names(&e)
            .iter()
            .any(|n| n == "ANTHROPIC_API_KEY" || n.starts_with("CLAUDECODE"))
    );
}

// ---- §5-6 / §10 item 3: (a) cleanup, (b) tamper, (c) normal runs ----

fn template() -> PathBuf {
    let t = tmp("template");
    put(&t, "settings.json", r#"{"autoMemoryEnabled": false}"#);
    t
}

#[test]
fn a_cleanup_keeps_only_session_records_and_installs_the_template() {
    let d = tmp("cfg-a");
    let t = template();
    put(&d, "settings.json", r#"{"permissions":{"allow":["Bash"]}}"#);
    put(&d, "CLAUDE.md", "MARKER: obey me");
    put(&d, "skills/evil/SKILL.md", "MARKER");
    put(&d, "projects/-state-work-sori/memory/m.md", "MARKER memory");
    put(&d, "projects/-state-work-sori/s1.jsonl", "{}\n");
    put(&d, "backups/x.json", "{}");
    put(&d, ".credentials.json", "{\"token\":\"x\"}");
    let digest = configdir::clean(&d, &t).expect("clean");
    let mut left = Vec::new();
    fn walk(r: &Path, p: &Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(p).expect("rd").flatten() {
            let q = e.path();
            if q.is_dir() {
                walk(r, &q, out)
            } else {
                out.push(
                    q.strip_prefix(r)
                        .expect("rel")
                        .to_string_lossy()
                        .to_string(),
                )
            }
        }
    }
    walk(&d, &d, &mut left);
    left.sort();
    assert_eq!(
        left,
        vec![
            "projects/-state-work-sori/s1.jsonl".to_string(),
            "settings.json".to_string()
        ]
    );
    assert_eq!(
        std::fs::read_to_string(d.join("settings.json")).expect("s"),
        r#"{"autoMemoryEnabled": false}"#
    );
    // the digest equals a fresh template-only directory with the same session record
    let fresh = tmp("cfg-a-fresh");
    put(
        &fresh,
        "projects/-state-work-sori/s1.jsonl",
        "other content",
    );
    assert_eq!(digest, configdir::clean(&fresh, &t).expect("clean"));
}

#[test]
fn b_tamper_after_a_checkpoint_is_detected_and_not_resumed() {
    let d = tmp("cfg-b");
    let digest = configdir::clean(&d, &template()).expect("clean");
    let cp = checkpoint::record(
        SID,
        "A1",
        0,
        &"a".repeat(40),
        ("POCX", 3, "dg"),
        &digest,
        ("h", "d"),
        10,
        json!({}),
    );
    for (rel, body) in [
        ("skills/x/SKILL.md", "MARKER"),
        ("projects/p/memory/m.md", "MARKER"),
        ("agents/a.md", "MARKER"),
        ("hooks/h.sh", "MARKER"),
    ] {
        let dd = tmp("cfg-b2");
        configdir::clean(&dd, &template()).expect("clean");
        put(&dd, rel, body);
        let observed = configdir::directive_digest(&dd);
        assert!(
            matches!(
                checkpoint::decide(Some(&cp), 0, "dg", &observed, None),
                Reconnect::TamperDetected { .. }
            ),
            "{rel}"
        );
    }
    // .claude.json directive key
    put(
        &d,
        ".claude.json",
        r#"{"mcpServers":{"x":{"command":"evil"}},"numStartups":3}"#,
    );
    assert_ne!(configdir::directive_digest(&d), digest);
}

#[test]
fn c_normal_cli_writes_are_not_tamper() {
    let d = tmp("cfg-c");
    let digest = configdir::clean(&d, &template()).expect("clean");
    for (rel, body) in [
        ("backups/b.json", "{}"),
        ("sessions/s.json", "{}"),
        ("shell-snapshots/x.sh", "x"),
        ("session-env/e", "x"),
        ("policy-limits.json", "{}"),
        ("remote-settings.json", "{}"),
        (".last-cleanup", "1"),
        ("plugins/store/cc-plugin-agents-md_builtin-1.json", "{}"),
        ("projects/-state-work-sori/s2.jsonl", "{}\n"),
        (
            ".claude.json",
            r#"{"numStartups":7,"projects":{"/state/work/sori":{"lastCost":1}}}"#,
        ),
    ] {
        put(&d, rel, body);
    }
    let observed = configdir::directive_digest(&d);
    assert_eq!(observed, digest);
    let cp = checkpoint::record(
        SID,
        "A1",
        2,
        &"a".repeat(40),
        ("POCX", 3, "dg"),
        &digest,
        ("h", "d"),
        10,
        json!({}),
    );
    assert_eq!(
        checkpoint::decide(Some(&cp), 2, "dg", &observed, None),
        Reconnect::Resume {
            session_id: SID.into()
        }
    );
}

// ---- §5-2/5 / §10 item 4 ----

#[test]
fn reconnect_decisions() {
    let cp = checkpoint::record(
        SID,
        "A1",
        2,
        &"a".repeat(40),
        ("POCX", 3, "dg"),
        "d0",
        ("h", "d"),
        10,
        json!({}),
    );
    assert_eq!(
        checkpoint::decide(None, 2, "dg", "d0", None),
        Reconnect::NewSession
    );
    assert_eq!(
        checkpoint::decide(Some(&cp), 2, "dg", "d0", Some(42)),
        Reconnect::AlreadyRunning { pid: 42 }
    );
    assert_eq!(
        checkpoint::decide(Some(&cp), 3, "dg", "d0", None),
        Reconnect::StaleGeneration
    );
    assert_eq!(
        checkpoint::decide(Some(&cp), 2, "other", "d0", None),
        Reconnect::ExternalReconciliation
    );
}

#[test]
fn one_running_process_per_session() {
    let d = tmp("lock");
    let me = std::process::id();
    let held = SessionLock::acquire(&d, SID, me).expect("first");
    assert_eq!(
        SessionLock::acquire(&d, SID, me).err(),
        Some(LockError::Running { pid: me })
    );
    drop(held);
    let again = SessionLock::acquire(&d, SID, me).expect("after release");
    drop(again);
    // a lock left by a process that no longer exists is stale and replaced
    std::fs::write(d.join(format!("{SID}.lock")), "4194303").expect("w");
    assert!(SessionLock::acquire(&d, SID, me).is_ok());
}

// ---- user decision 2026-11-13 ①: budget guard ----

fn rec(run: Value, ms: u64) -> Value {
    json!({"model_request": true, "attempt_id": "a", "command_id": "c", "duration_ms": ms, "usage": {"run": run}})
}

fn measured(input: u64, output: u64, cc: u64, cr: u64) -> Value {
    json!({"state": "measured", "source": "x", "tokens": {"input": input, "output": output, "cache_creation": cc, "cache_read": cr}})
}

fn ok_event() -> Value {
    json!({"type": "rate_limit_event", "rate_limit_info": {"status": "allowed", "overageStatus": "rejected",
           "overageDisabledReason": "out_of_credits", "isUsingOverage": false}})
}

#[test]
fn budget_counts_every_run_and_stops_at_caps() {
    assert_eq!(
        (
            BUDGET_0204.max_runs,
            BUDGET_0204.billable_tokens,
            BUDGET_0204.cache_read_tokens
        ),
        (12, 1_500_000, 15_000_000)
    );
    assert_eq!(
        (
            BUDGET_X02.max_runs,
            BUDGET_X02.per_run_seconds,
            BUDGET_X02.billable_tokens
        ),
        (6, 3_600, 3_000_000)
    );
    let e = ok_event();
    assert_eq!(
        budget::next(&BUDGET_X02, &[], Some(&e)),
        Decision::Start {
            timeout_seconds: 3_600
        }
    );
    // billable = input + output + cache_creation; cache_read separate
    let s = budget::spent(&[rec(measured(100, 20, 3, 1000), 1500)]);
    assert_eq!(
        (s.runs, s.billable_tokens, s.cache_read_tokens, s.seconds),
        (1, 123, 1000, 2)
    );
    // failed/incomplete runs count; an unmeasured run stops everything
    let r = [
        rec(measured(1, 1, 1, 1), 1000),
        rec(json!({"state": "unmeasured", "reason": "no result"}), 1000),
    ];
    assert!(matches!(
        budget::next(&BUDGET_X02, &r, Some(&e)),
        Decision::Stop { .. }
    ));
    // run count
    let six: Vec<Value> = (0..6).map(|_| rec(measured(1, 1, 1, 1), 1000)).collect();
    assert!(matches!(
        budget::next(&BUDGET_X02, &six, Some(&e)),
        Decision::Stop { .. }
    ));
    // a run that would cross the token cap is refused
    let big = [rec(measured(1_600_000, 0, 0, 0), 1000)];
    assert!(matches!(
        budget::next(&BUDGET_X02, &big, Some(&e)),
        Decision::Stop { .. }
    ));
    assert!(matches!(
        budget::next(
            &BUDGET_X02,
            &[rec(measured(1, 0, 0, 30_000_000), 1000)],
            Some(&e)
        ),
        Decision::Stop { .. }
    ));
    // remaining time bounds the next timeout
    let long = [rec(measured(1, 0, 0, 0), 13_000_000)];
    assert_eq!(
        budget::next(&BUDGET_X02, &long, Some(&e)),
        Decision::Start {
            timeout_seconds: 1_400
        }
    );
    // non-model commands do not count
    assert_eq!(
        budget::spent(&[json!({"model_request": false, "duration_ms": 9_999_000})]).runs,
        0
    );
}

#[test]
fn extra_usage_must_be_off_before_a_call() {
    let ok = ok_event();
    assert!(budget::overage_check(Some(&ok)).is_ok());
    let mut using = ok.clone();
    using["rate_limit_info"]["isUsingOverage"] = json!(true);
    assert!(matches!(
        budget::next(&BUDGET_0204, &[], Some(&using)),
        Decision::Hold { .. }
    ));
    let mut allowed = ok.clone();
    allowed["rate_limit_info"]["overageStatus"] = json!("allowed");
    assert!(matches!(
        budget::next(&BUDGET_0204, &[], Some(&allowed)),
        Decision::Hold { .. }
    ));
    assert!(matches!(
        budget::next(&BUDGET_0204, &[], None),
        Decision::Hold { .. }
    ));
    // the real 01-04 stream format of this account parses and passes
    let stream = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"),
        "/../../docs/poc/reports/tasks/01-04/first-probes-20261112/evidence/attempts/01-P1-development/stdout.jsonl"))
        .expect("stored 01-04 stream");
    let last = budget::last_rate_limit(&stream).expect("rate_limit_event present");
    assert!(budget::overage_check(Some(&last)).is_ok(), "{last}");
}
