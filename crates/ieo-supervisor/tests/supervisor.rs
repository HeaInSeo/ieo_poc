//! 02-04 supervisor core tests (no model calls).

use ieo_supervisor::args::{self, Role};
use ieo_supervisor::budget::{self, BUDGET_0204, BUDGET_X02, Budget, Decision, Observation};
use ieo_supervisor::checkpoint::{self, Checkpoint, CoordinatorRecord, Reconnect};
use ieo_supervisor::configdir;
use ieo_supervisor::env;
use ieo_supervisor::lock::{LockError, SessionLock};
use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const SID: &str = "6f1d2c3b-0000-4000-8000-000000000001";

/// A temporary directory removed on drop (restoring permissions first).
struct Tmp(PathBuf);

impl Tmp {
    fn new(name: &str) -> Tmp {
        let d = std::env::temp_dir().join(format!(
            "ieo-sup-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&d).expect("tmp");
        Tmp(d)
    }
    fn p(&self) -> &Path {
        &self.0
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        fn open(p: &Path) {
            if let Ok(m) = std::fs::symlink_metadata(p)
                && m.is_dir()
            {
                let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o700));
                if let Ok(rd) = std::fs::read_dir(p) {
                    for e in rd.flatten() {
                        open(&e.path());
                    }
                }
            }
        }
        open(&self.0);
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn put(root: &Path, rel: &str, body: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
    std::fs::write(p, body).expect("write");
}

fn files(root: &Path) -> Vec<String> {
    fn walk(r: &Path, p: &Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(p).expect("rd").flatten() {
            let q = e.path();
            let m = std::fs::symlink_metadata(&q).expect("meta");
            if m.is_dir() {
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
    let mut v = Vec::new();
    walk(root, root, &mut v);
    v.sort();
    v
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
            let other = if resume { "--session-id" } else { "--resume" };
            assert!(!a.iter().any(|x| x == other));
        }
    }
    // Guardrail rules: one argv entry each, in the documented order
    let g = args::build(&Role::Guardrail, "sori", SID, false);
    let at = |k: &str| g.iter().position(|x| x == k).expect(k);
    let allow = at("--allowedTools");
    assert_eq!(
        g[allow + 1..allow + 1 + args::GUARDRAIL_ALLOWED.len()].to_vec(),
        args::GUARDRAIL_ALLOWED.to_vec()
    );
    let deny = at("--disallowedTools");
    assert_eq!(
        g[deny + 1..deny + 1 + args::GUARDRAIL_DISALLOWED.len()].to_vec(),
        args::GUARDRAIL_DISALLOWED.to_vec()
    );
    assert!(args::GUARDRAIL_ALLOWED.contains(&"Edit(//state/guardrail-scratch/**)"));
    assert!(args::GUARDRAIL_DISALLOWED.contains(&"Bash"));
    assert!(!g.iter().any(|x| x == "--add-dir"));
    let d = args::build(&dev, "sori", SID, false);
    assert!(d.windows(2).any(|w| w[0] == "--append-system-prompt-file"));
    assert_eq!(args::cwd(&Role::Guardrail, "sori"), "/state/guardrail-cwd");
}

#[test]
fn environment_is_cleared_to_the_allow_list() {
    let e = env::build("/state/home", "/state/claude", "credential-value");
    assert_eq!(env::names(&e), env::ALLOWED.to_vec());
    // the child sees exactly the allow-list, not this process's environment
    let mut cmd = std::process::Command::new("/usr/bin/env");
    env::apply(&mut cmd, &e).expect("apply");
    let out = cmd.output().expect("run env");
    let mut seen: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.split('=').next().map(str::to_string))
        .collect();
    seen.sort();
    let mut want: Vec<String> = env::ALLOWED.iter().map(|s| s.to_string()).collect();
    want.sort();
    assert_eq!(seen, want);
    let mut bad = e.clone();
    bad.push(("ANTHROPIC_API_KEY".into(), "x".into()));
    assert!(env::apply(&mut std::process::Command::new("true"), &bad).is_err());
}

// ---- §5-6 / §10 item 3: (a) cleanup, (b) tamper, (c) normal runs ----

fn template() -> Tmp {
    let t = Tmp::new("template");
    put(t.p(), "settings.json", r#"{"autoMemoryEnabled": false}"#);
    t
}

fn cp_value(digest: &str, generation: u64) -> Value {
    checkpoint::record(&Checkpoint {
        session_id: SID,
        generation,
        assignment_id: "A1",
        target_sha: &"a".repeat(40),
        sprint_id: "POCX",
        sprint_revision: 3,
        spec_digest: "dg",
        image_digest: "sha256:img",
        profile_id: "dev",
        profile_revision: 1,
        settings_digest: digest,
        worktree_head: "h",
        diff_sha256: "d",
        last_event_offset: 10,
        active_children: vec![],
        pending_external_requests: vec![],
        session_cumulative: json!({}),
    })
}

fn coord(generation: u64) -> CoordinatorRecord<'static> {
    CoordinatorRecord {
        generation,
        assignment_id: "A1",
        sprint_id: "POCX",
        sprint_revision: 3,
        spec_digest: "dg",
    }
}

#[test]
fn a_cleanup_keeps_only_session_records_and_installs_the_template() {
    let d = Tmp::new("cfg-a");
    let t = template();
    put(
        d.p(),
        "settings.json",
        r#"{"permissions":{"allow":["Bash"]}}"#,
    );
    put(d.p(), "CLAUDE.md", "MARKER: obey me");
    put(d.p(), "skills/evil/SKILL.md", "MARKER");
    put(
        d.p(),
        "projects/-state-work-sori/memory/m.md",
        "MARKER memory",
    );
    put(d.p(), "projects/-state-work-sori/s1.jsonl", "{}\n");
    put(d.p(), "backups/x.json", "{}");
    put(d.p(), ".credentials.json", "{\"token\":\"x\"}");
    let report = configdir::clean(d.p(), t.p()).expect("clean");
    assert_eq!(
        files(d.p()),
        vec!["projects/-state-work-sori/s1.jsonl", "settings.json"]
    );
    assert_eq!(
        std::fs::read_to_string(d.p().join("settings.json")).expect("s"),
        r#"{"autoMemoryEnabled": false}"#
    );
    assert_eq!(report.credentials_removed, vec![".credentials.json"]);
    assert!(report.removed.contains(&"CLAUDE.md".to_string()));
    // the digest equals a fresh template-only directory with the same session record
    let fresh = Tmp::new("cfg-a-fresh");
    put(
        fresh.p(),
        "projects/-state-work-sori/s1.jsonl",
        "other content",
    );
    assert_eq!(
        report.digest,
        configdir::clean(fresh.p(), t.p()).expect("clean").digest
    );
    // a template that leaves auto memory on is refused
    let bad = Tmp::new("template-bad");
    put(bad.p(), "settings.json", "{}");
    assert!(configdir::clean(d.p(), bad.p()).is_err());
}

#[test]
fn a_cleanup_never_follows_links_and_reopens_hidden_directories() {
    let d = Tmp::new("cfg-link");
    let outside = Tmp::new("outside");
    put(outside.p(), "keep.txt", "outside the config dir");
    put(outside.p(), "s.jsonl", "outside");
    std::os::unix::fs::symlink(outside.p(), d.p().join("skills")).expect("link");
    std::fs::create_dir_all(d.p().join("projects/p")).expect("mk");
    std::os::unix::fs::symlink(
        outside.p().join("s.jsonl"),
        d.p().join("projects/p/l.jsonl"),
    )
    .expect("link");
    put(d.p(), "agents/hidden/a.md", "MARKER");
    std::fs::set_permissions(d.p().join("agents"), std::fs::Permissions::from_mode(0o000))
        .expect("chmod");
    let report = configdir::clean(d.p(), template().p()).expect("clean");
    assert_eq!(files(outside.p()), vec!["keep.txt", "s.jsonl"]);
    assert_eq!(files(d.p()), vec!["settings.json"]);
    assert!(report.removed.contains(&"skills".to_string()));
    assert!(report.removed.contains(&"projects/p/l.jsonl".to_string()));
    assert!(report.removed.contains(&"agents/hidden/a.md".to_string()));
}

#[test]
fn b_tamper_after_a_checkpoint_is_detected_and_not_resumed() {
    let t = template();
    for (rel, body) in [
        ("settings.local.json", "{}"),
        ("CLAUDE.md", "MARKER"),
        (".mcp.json", "{}"),
        ("agents/a.md", "MARKER"),
        ("skills/x/SKILL.md", "MARKER"),
        ("commands/c.md", "MARKER"),
        ("output-styles/o.md", "MARKER"),
        ("hooks/h.sh", "MARKER"),
        ("plugins/p/plugin.json", "MARKER"),
        ("projects/p/memory/m.md", "MARKER"),
    ] {
        let dd = Tmp::new("cfg-b");
        let digest = configdir::clean(dd.p(), t.p()).expect("clean").digest;
        let cp = cp_value(&digest, 0);
        put(dd.p(), rel, body);
        let observed = configdir::observed(dd.p());
        assert!(
            matches!(
                checkpoint::decide(Some(&cp), &coord(0), &observed, None),
                Reconnect::TamperDetected { .. }
            ),
            "{rel}"
        );
    }
    // .claude.json directive key
    let d = Tmp::new("cfg-b-json");
    let digest = configdir::clean(d.p(), t.p()).expect("clean").digest;
    put(
        d.p(),
        ".claude.json",
        r#"{"mcpServers":{"x":{"command":"evil"}},"numStartups":3}"#,
    );
    assert_ne!(configdir::observed(d.p()), digest);
    // a planted link and an unreadable directive directory are tamper too (fail closed)
    let l = Tmp::new("cfg-b-link");
    let digest = configdir::clean(l.p(), t.p()).expect("clean").digest;
    std::os::unix::fs::symlink("/elsewhere", l.p().join("skills")).expect("link");
    assert_ne!(configdir::observed(l.p()), digest);
    let u = Tmp::new("cfg-b-perm");
    let digest = configdir::clean(u.p(), t.p()).expect("clean").digest;
    put(u.p(), "skills/x/SKILL.md", "MARKER");
    std::fs::set_permissions(u.p().join("skills"), std::fs::Permissions::from_mode(0o000))
        .expect("chmod");
    let observed = configdir::observed(u.p());
    assert!(observed.starts_with("unreadable"), "{observed}");
    assert!(matches!(
        checkpoint::decide(Some(&cp_value(&digest, 0)), &coord(0), &observed, None),
        Reconnect::TamperDetected { .. }
    ));
}

#[test]
fn c_normal_cli_writes_are_not_tamper() {
    let d = Tmp::new("cfg-c");
    let digest = configdir::clean(d.p(), template().p())
        .expect("clean")
        .digest;
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
        put(d.p(), rel, body);
    }
    let observed = configdir::observed(d.p());
    assert_eq!(observed, digest);
    assert_eq!(
        checkpoint::decide(Some(&cp_value(&digest, 2)), &coord(2), &observed, None),
        Reconnect::Resume {
            session_id: SID.into()
        }
    );
}

// ---- §5-1/2/5, §6 / §10 item 4 ----

#[test]
fn checkpoint_record_has_the_5_1_fields() {
    let cp = cp_value("d0", 2);
    for k in [
        "session_id",
        "generation",
        "assignment_id",
        "target_sha",
        "sprint_id",
        "sprint_revision",
        "spec_digest",
        "image_digest",
        "profile_id",
        "profile_revision",
        "settings_digest",
        "worktree_head",
        "diff_sha256",
        "last_event_offset",
        "active_children",
        "pending_external_requests",
        "session_cumulative",
    ] {
        assert!(!cp[k].is_null(), "{k}");
    }
}

#[test]
fn reconnect_decisions() {
    let cp = cp_value("d0", 2);
    let c = coord(2);
    let resume = Reconnect::Resume {
        session_id: SID.into(),
    };
    assert_eq!(checkpoint::decide(Some(&cp), &c, "d0", None), resume);
    assert_eq!(
        checkpoint::decide(None, &c, "d0", None),
        Reconnect::NewSession
    );
    assert_eq!(
        checkpoint::decide(Some(&cp), &c, "d0", Some(Some(42))),
        Reconnect::AlreadyRunning { pid: Some(42) }
    );
    assert_eq!(
        checkpoint::decide(Some(&cp), &coord(3), "d0", None),
        Reconnect::StaleGeneration
    );
    let reconcile = |v: &Value, c: &CoordinatorRecord<'_>| {
        matches!(
            checkpoint::decide(Some(v), c, "d0", None),
            Reconnect::ExternalReconciliation { .. }
        )
    };
    assert!(reconcile(&cp, &coord(1)), "newer checkpoint generation");
    for (field, value) in [
        ("spec_digest", json!("other")),
        ("sprint_id", json!("OTHER")),
        ("sprint_revision", json!(4)),
        ("assignment_id", json!("A2")),
        ("session_id", json!("")),
        ("session_id", Value::Null),
        ("generation", Value::Null),
        ("settings_digest", Value::Null),
        ("active_children", json!([{"id": "child-1"}])),
        ("pending_external_requests", json!(["req-1"])),
        ("active_children", Value::Null),
    ] {
        let mut v = cp.clone();
        v[field] = value.clone();
        assert!(reconcile(&v, &c), "{field}={value}");
    }
}

#[test]
fn one_running_process_per_session() {
    let d = Tmp::new("lock");
    let me = std::process::id();
    let held = SessionLock::acquire(d.p(), SID, me).expect("first");
    assert_eq!(
        SessionLock::acquire(d.p(), SID, me).err(),
        Some(LockError::Running { pid: Some(me) })
    );
    drop(held);
    let again = SessionLock::acquire(d.p(), SID, me).expect("after release");
    drop(again);
    // a leftover or empty file without a holder is not a lock
    let path = d.p().join(format!("{SID}.lock"));
    std::fs::write(&path, "").expect("w");
    let held = SessionLock::acquire(d.p(), SID, me).expect("leftover file");
    assert_eq!(std::fs::read_to_string(&path).expect("r"), me.to_string());
    // a holder that has not written its pid yet still holds the lock
    std::fs::write(&path, "").expect("truncate");
    assert_eq!(
        SessionLock::acquire(d.p(), SID, me).err(),
        Some(LockError::Running { pid: None })
    );
    drop(held);
    // a holder in another process blocks; its death releases the lock
    let mut child = std::process::Command::new("flock")
        .arg(&path)
        .args(["sleep", "30"])
        .spawn()
        .expect("flock(1)");
    let t0 = std::time::Instant::now();
    while SessionLock::acquire(d.p(), SID, me).is_ok() {
        assert!(t0.elapsed().as_secs() < 10, "child never took the lock");
        std::thread::yield_now();
    }
    child.kill().expect("kill");
    child.wait().expect("wait");
    assert!(SessionLock::acquire(d.p(), SID, me).is_ok());
}

// ---- user decision 2026-11-13 ①: budget guard ----

fn rec(run: Value, ms: u64) -> Value {
    json!({"model_request": true, "attempt_id": "a", "command_id": "c", "duration_ms": ms, "usage": {"run": run}})
}

fn measured(input: u64, output: u64, cc: u64, cr: u64) -> Value {
    json!({"state": "measured", "source": "x", "tokens": {"input": input, "output": output, "cache_creation": cc, "cache_read": cr}})
}

fn ok_stream() -> String {
    json!({"type": "rate_limit_event", "rate_limit_info": {"status": "allowed", "overageStatus": "rejected",
           "overageDisabledReason": "out_of_credits", "isUsingOverage": false}})
    .to_string()
}

fn fresh(s: &str) -> Option<Observation<'_>> {
    Some(Observation {
        stream: s,
        age_seconds: 60,
    })
}

fn stops(b: &Budget, r: &[Value]) -> bool {
    let s = ok_stream();
    matches!(budget::next(b, r, fresh(&s)), Decision::Stop { .. })
}

fn runs(n: u64, run: Value, ms: u64) -> Vec<Value> {
    (0..n).map(|_| rec(run.clone(), ms)).collect()
}

#[test]
fn budget_constants_match_decision_1() {
    assert_eq!(
        BUDGET_0204,
        Budget {
            name: "02-04",
            max_runs: 12,
            per_run_seconds: 1_800,
            total_seconds: 14_400,
            billable_tokens: 1_500_000,
            cache_read_tokens: 15_000_000
        }
    );
    assert_eq!(
        BUDGET_X02,
        Budget {
            name: "X-02",
            max_runs: 6,
            per_run_seconds: 3_600,
            total_seconds: 14_400,
            billable_tokens: 3_000_000,
            cache_read_tokens: 30_000_000
        }
    );
}

#[test]
fn budget_counts_every_run_and_never_reads_missing_as_zero() {
    let s = ok_stream();
    assert_eq!(
        budget::next(&BUDGET_X02, &[], fresh(&s)),
        Decision::Start {
            timeout_seconds: 3_600
        }
    );
    // billable = input + output + cache_creation; cache_read separate
    let sp = budget::spent(&[rec(measured(100, 20, 3, 1000), 1500)]);
    assert_eq!(
        (
            sp.runs,
            sp.billable_tokens,
            sp.cache_read_tokens,
            sp.seconds
        ),
        (1, 123, 1000, 2)
    );
    // an unmeasured run, a missing duration, a missing token key, a record that does not say
    // whether it was a model run: all stop, none counts as zero
    let mut no_key = measured(1, 1, 1, 1);
    no_key["tokens"]
        .as_object_mut()
        .expect("obj")
        .remove("cache_creation");
    let mut no_duration = rec(measured(1, 1, 1, 1), 1);
    no_duration
        .as_object_mut()
        .expect("obj")
        .remove("duration_ms");
    for bad in [
        rec(json!({"state": "unmeasured", "reason": "no result"}), 1000),
        rec(no_key, 1000),
        no_duration,
        json!({"attempt_id": "a", "duration_ms": 5}),
    ] {
        assert!(
            stops(&BUDGET_X02, &[rec(measured(1, 1, 1, 1), 1000), bad.clone()]),
            "{bad}"
        );
    }
    // non-model commands do not count
    assert_eq!(
        budget::spent(&[json!({"model_request": false, "duration_ms": 9_999_000})]).runs,
        0
    );
}

#[test]
fn budget_boundaries() {
    let one = measured(1, 0, 0, 0);
    // runs: 5 of 6 may continue, 6 stop; 11 of 12, 12 stop
    assert!(!stops(&BUDGET_X02, &runs(5, one.clone(), 1000)));
    assert!(stops(&BUDGET_X02, &runs(6, one.clone(), 1000)));
    assert!(!stops(&BUDGET_0204, &runs(11, one.clone(), 1000)));
    assert!(stops(&BUDGET_0204, &runs(12, one.clone(), 1000)));
    // seconds: 14,399 left 1 s; 14,400 stop; the timeout is capped by what is left
    let s = ok_stream();
    assert_eq!(
        budget::next(&BUDGET_X02, &[rec(one.clone(), 14_399_000)], fresh(&s)),
        Decision::Start { timeout_seconds: 1 }
    );
    assert!(stops(&BUDGET_X02, &[rec(one.clone(), 14_400_000)]));
    assert_eq!(
        budget::next(&BUDGET_0204, &[rec(one.clone(), 13_000_000)], fresh(&s)),
        Decision::Start {
            timeout_seconds: 1_400
        }
    );
    // billable tokens: a run starts while spent + largest run <= cap (X-02 cap 3,000,000)
    let at = |n: u64| rec(measured(n, 0, 0, 0), 1000);
    assert!(!stops(&BUDGET_X02, &[at(1_000_000), at(1_000_000)])); // 2M + 1M == cap
    assert!(stops(&BUDGET_X02, &[at(1_000_000), at(1_000_000), at(1)])); // one token over
    assert!(!stops(&BUDGET_X02, &[at(1_500_000)]));
    assert!(stops(&BUDGET_X02, &[at(1_500_001)]));
    assert!(stops(&BUDGET_X02, &[at(3_000_000)]));
    // input, output and cache_creation all count toward it
    for t in [
        measured(0, 1_500_001, 0, 0),
        measured(0, 0, 1_500_001, 0),
        measured(500_000, 500_000, 500_001, 0),
    ] {
        assert!(stops(&BUDGET_X02, &[rec(t.clone(), 1000)]), "{t}");
    }
    // cache_read has its own cap (X-02 30,000,000) and does not count as billable
    let cr = |n: u64| rec(measured(0, 0, 0, n), 1000);
    assert!(!stops(&BUDGET_X02, &[cr(15_000_000)]));
    assert!(stops(&BUDGET_X02, &[cr(15_000_001)]));
    assert!(!stops(&BUDGET_0204, &[cr(7_500_000)]));
    assert!(stops(&BUDGET_0204, &[cr(7_500_001)]));
    assert!(!stops(&BUDGET_0204, &[at(750_000)]));
    assert!(stops(&BUDGET_0204, &[at(750_001)]));
    assert!(stops(&BUDGET_X02, &[cr(30_000_000)]));
}

#[test]
fn extra_usage_must_be_off_before_and_during_a_call() {
    let ok = ok_stream();
    assert!(budget::observation_check(fresh(&ok)).is_ok());
    let with = |k: &str, v: Value| {
        let mut e: Value = serde_json::from_str(&ok).expect("json");
        e["rate_limit_info"][k] = v;
        e.to_string()
    };
    let breaking = [
        with("isUsingOverage", json!(true)),
        with("isUsingOverage", Value::Null),
        with("overageStatus", json!("allowed")),
        with("overageStatus", json!("allowed_warning")),
        with("overageStatus", Value::Null),
        with("status", json!("rejected")),
        with("status", json!("allowed_warning")),
    ];
    for bad in &breaking {
        assert!(
            matches!(
                budget::next(&BUDGET_0204, &[], fresh(bad)),
                Decision::Hold { .. }
            ),
            "{bad}"
        );
        // an earlier breaking event in the same stream is not hidden by a later good one
        let mixed = format!("{bad}\n{ok}\n");
        assert!(budget::observation_check(fresh(&mixed)).is_err(), "{mixed}");
        // during a run the adapter stops on it
        assert!(budget::stream_stop(bad).is_some(), "{bad}");
    }
    assert!(budget::stream_stop(&ok).is_none());
    assert!(budget::stream_stop(r#"{"type":"assistant"}"#).is_none());
    // no observation, an observation without events, an old observation: hold
    for obs in [
        None,
        fresh("{\"type\":\"result\"}"),
        Some(Observation {
            stream: &ok,
            age_seconds: budget::OBSERVATION_MAX_AGE_SECONDS + 1,
        }),
    ] {
        assert!(matches!(
            budget::next(&BUDGET_0204, &[], obs),
            Decision::Hold { .. }
        ));
    }
    assert!(
        budget::observation_check(Some(Observation {
            stream: &ok,
            age_seconds: budget::OBSERVATION_MAX_AGE_SECONDS
        }))
        .is_ok()
    );
    // the real 01-04 stream of this account parses and passes (every event)
    let stream = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"),
        "/../../docs/poc/reports/tasks/01-04/first-probes-20261112/evidence/attempts/01-P1-development/stdout.jsonl"))
        .expect("stored 01-04 stream");
    assert!(!budget::rate_limit_events(&stream).is_empty());
    assert!(budget::observation_check(fresh(&stream)).is_ok());
}
