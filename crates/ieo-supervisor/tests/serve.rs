//! Role supervisor service (02-04) with a fake `claude`, no model calls.

use ieo_supervisor::serve::{Config, Service};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

const TOKEN: &str = "adapter-test-token";

/// The session lock is a flock on an O_CLOEXEC file. A process spawned on another test thread
/// shares the descriptor until it execs, so a just-released lock can look held (PR #64 review
/// c6c0656 M-1; the same guard as tests/caller.rs). Every test in this file runs alone.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

struct Tmp(PathBuf);
impl Tmp {
    fn new(name: &str) -> Tmp {
        let d = std::env::temp_dir().join(format!(
            "ieo-serve-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&d).expect("tmp");
        Tmp(d)
    }
}
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A fake `claude`: reads the stream-json prompt from stdin, reports argv, the prompt and the
/// config dir's top-level entries, and answers like the CLI (init, assistant, rate limit,
/// result with modelUsage). Prompt "SLEEP" keeps it running.
fn fake_claude(dir: &Path) -> PathBuf {
    let p = dir.join("fake-claude");
    std::fs::write(
        &p,
        r#"#!/bin/sh
prompt=$(head -c 4096)
args="$*"
sid=$(printf '%s\n' "$@" | awk 'prev=="--session-id"||prev=="--resume"{print; exit} {prev=$0}')
cfg=$(ls -A "$CLAUDE_CONFIG_DIR" 2>/dev/null | tr '\n' ',')
printf '{"type":"system","subtype":"init","session_id":"%s","args":"%s","config":"%s","prompt_seen":%s}\n' "$sid" "$args" "$cfg" "$(echo "$prompt" | grep -q '"type":"user"' && echo true || echo false)"
case "$prompt" in
  *LATE*) trap 'printf "{\"type\":\"result\",\"subtype\":\"error_during_execution\",\"is_error\":true,\"session_id\":\"%s\",\"usage\":{\"input_tokens\":0,\"output_tokens\":0,\"cache_creation_input_tokens\":0,\"cache_read_input_tokens\":0},\"modelUsage\":{},\"total_cost_usd\":0.99}\n" "$sid"; exit 0' TERM; while :; do sleep 0.1; done;;
  *SLEEP*) sleep 60;;
esac
printf '{"type":"assistant","message":{"id":"m1","usage":{"input_tokens":3,"output_tokens":5,"cache_creation_input_tokens":10,"cache_read_input_tokens":20}}}\n'
printf '{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","overageStatus":"rejected","isUsingOverage":false}}\n'
printf '{"type":"result","subtype":"success","is_error":false,"session_id":"%s","usage":{"input_tokens":3,"output_tokens":7,"cache_creation_input_tokens":10,"cache_read_input_tokens":20},"modelUsage":{"m":{"inputTokens":3,"outputTokens":7,"cacheReadInputTokens":20,"cacheCreationInputTokens":10}},"total_cost_usd":0.01}\n' "$sid"
"#,
    )
    .expect("fake");
    let _ = std::process::Command::new("chmod")
        .arg("+x")
        .arg(&p)
        .status();
    p
}

fn setup(name: &str) -> (Tmp, Arc<Service>) {
    let t = Tmp::new(name);
    let root = &t.0;
    let etc = root.join("etc");
    std::fs::create_dir_all(etc.join("template")).expect("etc");
    std::fs::write(
        etc.join("template/settings.json"),
        r#"{"autoMemoryEnabled": false}"#,
    )
    .expect("template");
    let cred = root.join("cred");
    std::fs::create_dir_all(&cred).expect("cred");
    std::fs::write(cred.join("oauth_token"), "fake-oauth-token\n").expect("token");
    std::fs::write(
        cred.join("adapter.sha256"),
        hex::encode(Sha256::digest(TOKEN.as_bytes())),
    )
    .expect("adapter");
    fake_claude(root);
    let cfg = svc_config(&t);
    (t, Arc::new(Service::new(cfg)))
}

/// The configuration `setup` uses, for a second service on the same volume.
fn svc_config(t: &Tmp) -> Config {
    let root = &t.0;
    Config::from_json(&json!({"role": "development", "repo": "sori",
        "state": root.join("state"), "etc": root.join("etc"), "cred": root.join("cred"),
        "claude": root.join("fake-claude"), "quota_group_id": "test-shared", "checkpoint_central": "local_only", "budget": "02-04",
        "share": {"max_runs": 8, "total_seconds": 9_600, "billable_tokens": 1_000_000,
                  "cache_read_tokens": 10_000_000}}))
    .expect("config")
}

fn coord(generation: u64) -> Value {
    json!({"generation": generation, "assignment_id": "A1", "sprint_id": "POCX", "sprint_revision": 3,
           "spec_digest": "dg", "image_digest": "sha256:img", "profile_id": "dev", "profile_revision": 1})
}

fn req(attempt: &str, prompt: &str, extra: Value) -> Vec<u8> {
    let mut v = json!({"attempt_id": attempt, "prompt": prompt, "coordinator": coord(0),
        "limits": {"timeout_ms": 20_000, "billable_room": 100_000, "cache_read_room": 100_000,
                   "grace_ms": 1_000}});
    for (k, x) in extra.as_object().expect("obj") {
        v[k] = x.clone();
    }
    serde_json::to_vec(&v).expect("json")
}

fn call(svc: &Arc<Service>, method: &str, path: &str, body: &[u8]) -> (u16, Value) {
    let r = svc.handle(method, path, Some(TOKEN), body);
    (r.status, r.body)
}

/// Wait for `attempt` to finish; returns its outcome.
fn wait(svc: &Arc<Service>, attempt: &str) -> Value {
    let t0 = Instant::now();
    loop {
        let s = svc.status();
        if s["running"].is_null() && s["last"]["attempt_id"] == attempt {
            return s["last"].clone();
        }
        assert!(t0.elapsed() < Duration::from_secs(30), "{s}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn init_line(svc: &Arc<Service>, attempt: &str) -> Value {
    let (st, b) = call(svc, "GET", &format!("/v0/role/runs/{attempt}/stdout"), b"");
    assert_eq!(st, 200, "{b}");
    let text = b["content"].as_str().expect("content").to_string();
    serde_json::from_str(text.lines().next().expect("init")).expect("json")
}

#[test]
fn the_adapter_credential_is_required() {
    let _serial = serial();
    let (t, svc) = setup("auth");
    assert_eq!(svc.handle("GET", "/v0/role/status", None, b"").status, 401);
    assert_eq!(
        svc.handle("GET", "/v0/role/status", None, b"").body["code"],
        "UNAUTHORIZED"
    );
    assert_eq!(
        svc.handle("GET", "/v0/role/status", Some("wrong"), b"")
            .status,
        401
    );
    std::fs::remove_file(t.0.join("cred/adapter.sha256")).expect("rm");
    assert_eq!(
        svc.handle("GET", "/v0/role/status", Some(TOKEN), b"")
            .status,
        401,
        "no verification key must refuse"
    );
}

#[test]
fn start_runs_a_new_session_with_the_contract_arguments_and_environment() {
    let _serial = serial();
    let (_t, svc) = setup("start");
    let (st, b) = call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("a1", "hello", json!({"session_id": "s-new"})),
    );
    assert_eq!(st, 202, "{b}");
    let out = wait(&svc, "a1");
    assert_eq!(
        (out["stop"].as_str(), out["exit_code"].as_i64()),
        (Some("Exited"), Some(0)),
        "{out}"
    );
    let argv: Vec<String> = serde_json::from_value(out["argv"].clone()).expect("argv");
    assert!(
        argv.windows(2)
            .any(|w| w[0] == "--session-id" && w[1] == "s-new")
    );
    assert!(!argv.iter().any(|a| a == "--resume" || a.contains("Write(")));
    assert_eq!(
        out["env_names"],
        json!([
            "HOME",
            "PATH",
            "LANG",
            "CLAUDE_CONFIG_DIR",
            "CLAUDE_CODE_OAUTH_TOKEN"
        ])
    );
    // the prompt reached the process on stdin as a stream-json user message
    assert_eq!(init_line(&svc, "a1")["prompt_seen"], true);
    // the credential value is never in the outcome or the status
    assert!(!out.to_string().contains("fake-oauth-token"));
    assert!(!svc.status().to_string().contains("fake-oauth-token"));
    // a checkpoint names the session and the cleaned digest
    let cp = &svc.status()["checkpoint"];
    assert_eq!(cp["session_id"], "s-new");
    assert_eq!(cp["settings_digest"], out["config_cleanup"]["digest"]);
    assert_eq!(cp["session_cumulative"]["total_cost_usd"], 0.01);
    // an attempt id is used once
    let (st, _) = call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("a1", "again", json!({"session_id": "s-other"})),
    );
    assert_eq!(st, 409);
}

#[test]
fn a_cleanup_removes_directive_markers_before_the_first_start() {
    let _serial = serial();
    let (t, svc) = setup("clean");
    let cfg = t.0.join("state/claude");
    for f in [
        "CLAUDE.md",
        "skills/x/SKILL.md",
        "settings.json",
        ".credentials.json",
    ] {
        let p = cfg.join(f);
        std::fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
        std::fs::write(p, "MARKER").expect("marker");
    }
    let (st, _) = call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("a1", "hello", json!({"session_id": "s1"})),
    );
    assert_eq!(st, 202);
    let out = wait(&svc, "a1");
    // what the process saw in its config dir: only the template's settings
    assert_eq!(init_line(&svc, "a1")["config"], "settings.json,");
    assert_eq!(
        out["config_cleanup"]["credentials_removed"],
        json!([".credentials.json"])
    );
    assert_eq!(
        std::fs::read_to_string(cfg.join("settings.json")).expect("settings"),
        r#"{"autoMemoryEnabled": false}"#
    );
}

#[test]
fn reconnect_resumes_an_untouched_session_and_starts_anew_after_tamper() {
    let _serial = serial();
    let (t, svc) = setup("reconnect");
    call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("a1", "hello", json!({"session_id": "s1"})),
    );
    wait(&svc, "a1");
    // (c) normal resume: same session id, --resume
    let (st, b) = call(
        &svc,
        "POST",
        "/v0/role/reconnect",
        &req("a2", "more", json!({"new_session_id": "s-unused"})),
    );
    assert_eq!(st, 202, "{b}");
    let out = wait(&svc, "a2");
    let argv: Vec<String> = serde_json::from_value(out["argv"].clone()).expect("argv");
    assert!(
        argv.windows(2).any(|w| w[0] == "--resume" && w[1] == "s1"),
        "{argv:?}"
    );
    assert_eq!(out["mode"], "resume");
    assert!(out["tamper"].is_null());
    // (b) tamper: a skill planted after the checkpoint → new session, reported, not resumed
    let skill = t.0.join("state/claude/skills/evil/SKILL.md");
    std::fs::create_dir_all(skill.parent().expect("p")).expect("mk");
    std::fs::write(&skill, "MARKER").expect("plant");
    let (st, b) = call(
        &svc,
        "POST",
        "/v0/role/reconnect",
        &req("a3", "more", json!({"new_session_id": "s2"})),
    );
    assert_eq!(st, 202, "{b}");
    let out = wait(&svc, "a3");
    let argv: Vec<String> = serde_json::from_value(out["argv"].clone()).expect("argv");
    assert!(
        argv.windows(2)
            .any(|w| w[0] == "--session-id" && w[1] == "s2")
    );
    assert!(!argv.iter().any(|a| a == "--resume"));
    assert_eq!(out["tamper"]["config_tamper_detected"], true, "{out}");
    assert_eq!(out["tamper"]["old_session_id"], "s1");
    assert!(
        !skill.exists(),
        "the planted skill was cleaned before the new session"
    );
}

#[test]
fn one_running_process_per_session_and_stop() {
    let _serial = serial();
    let (_t, svc) = setup("single");
    call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("a1", "SLEEP", json!({"session_id": "s1"})),
    );
    // wait until it runs
    let t0 = Instant::now();
    while svc.status()["running"].is_null() {
        assert!(t0.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(20));
    }
    // wait until the fake has started (its init line), so the stop below reaches a process
    let t0 = Instant::now();
    while !std::fs::read_to_string(_t.0.join("state/runs/a1/stdout.jsonl"))
        .unwrap_or_default()
        .contains("\"init\"")
    {
        assert!(t0.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(20));
    }
    // a second reconnect while running does not start anything and answers with status (§5-5)
    let (st, b) = call(
        &svc,
        "POST",
        "/v0/role/reconnect",
        &req("a2", "more", json!({"new_session_id": "s9"})),
    );
    assert_eq!(st, 409, "{b}");
    assert_eq!(b["code"], "ALREADY_RUNNING");
    assert!(!_t.0.join("state/runs/a2").exists());
    // the takeover checkpoint is on disk while the run is going (§5-1)
    let cp: Value = serde_json::from_slice(
        &std::fs::read(_t.0.join("state/checkpoint.json")).expect("checkpoint"),
    )
    .expect("json");
    assert_eq!(
        (cp["phase"].as_str(), cp["session_id"].as_str()),
        (Some("takeover"), Some("s1"))
    );
    // a start while running answers with the status too
    let (st, b) = call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("a3", "more", json!({"session_id": "s3"})),
    );
    assert_eq!(
        (st, b["code"].as_str()),
        (409, Some("ALREADY_RUNNING")),
        "{b}"
    );
    assert_eq!(b["status"]["running"]["attempt_id"], "a1", "{b}");
    // stop ends the run through the guard
    let (st, _) = call(&svc, "POST", "/v0/role/stop", b"");
    assert_eq!(st, 202);
    let out = wait(&svc, "a1");
    assert_eq!(out["stop"], "Requested", "{out}");
    assert_eq!(out["stopped_by_guard"], true);
    let cp: Value = serde_json::from_slice(
        &std::fs::read(_t.0.join("state/checkpoint.json")).expect("checkpoint"),
    )
    .expect("json");
    assert_eq!(cp["phase"], "after_run");
}

#[test]
fn reconnect_refuses_stale_or_mismatched_records_and_missing_credentials() {
    let _serial = serial();
    let (t, svc) = setup("refuse");
    call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("a1", "hello", json!({"session_id": "s1"})),
    );
    wait(&svc, "a1");
    let mut stale =
        serde_json::from_slice::<Value>(&req("a2", "x", json!({"new_session_id": "s2"})))
            .expect("json");
    stale["coordinator"]["generation"] = json!(1);
    let (st, b) = call(
        &svc,
        "POST",
        "/v0/role/reconnect",
        &serde_json::to_vec(&stale).expect("j"),
    );
    assert_eq!(
        (st, b["code"].as_str()),
        (409, Some("STALE_GENERATION")),
        "{b}"
    );
    let mut other = stale.clone();
    other["coordinator"]["generation"] = json!(0);
    other["coordinator"]["spec_digest"] = json!("other");
    let (st, b) = call(
        &svc,
        "POST",
        "/v0/role/reconnect",
        &serde_json::to_vec(&other).expect("j"),
    );
    assert_eq!(
        (st, b["code"].as_str()),
        (409, Some("EXTERNAL_RECONCILIATION_REQUIRED")),
        "{b}"
    );
    std::fs::remove_file(t.0.join("cred/oauth_token")).expect("rm");
    let (st, b) = call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("a3", "x", json!({"session_id": "s3"})),
    );
    assert_eq!(
        (st, b["code"].as_str()),
        (503, Some("CREDENTIAL_UNAVAILABLE"))
    );
    assert!(!t.0.join("state/runs/a3").exists());
}

// --- review 88a4535 ---

fn code(r: (u16, Value)) -> (u16, Option<String>) {
    (r.0, r.1["code"].as_str().map(str::to_string))
}

#[test]
fn a_reconnect_needs_a_readable_checkpoint() {
    let _serial = serial();
    let (t, svc) = setup("nocp");
    let body = req("a1", "x", json!({"new_session_id": "s1"}));
    // F1: nothing to reconnect to → never a silent new session
    assert_eq!(
        code(call(&svc, "POST", "/v0/role/reconnect", &body)),
        (409, Some("NOT_OBSERVED".into()))
    );
    std::fs::create_dir_all(t.0.join("state")).expect("state");
    std::fs::write(t.0.join("state/checkpoint.json"), "{not json").expect("corrupt");
    assert_eq!(
        code(call(&svc, "POST", "/v0/role/reconnect", &body)),
        (409, Some("EXTERNAL_RECONCILIATION_REQUIRED".into()))
    );
    // F2: a checkpoint session id that is not a safe id is never used as a path or argument
    call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("a2", "hi", json!({"session_id": "s2"})),
    );
    wait(&svc, "a2");
    let p = t.0.join("state/checkpoint.json");
    let mut cp: Value = serde_json::from_slice(&std::fs::read(&p).expect("cp")).expect("json");
    for bad in ["--dangerously-skip-permissions", "../x", "", "a/b"] {
        cp["session_id"] = json!(bad);
        std::fs::write(&p, serde_json::to_vec(&cp).expect("j")).expect("w");
        assert_eq!(
            code(call(
                &svc,
                "POST",
                "/v0/role/reconnect",
                &req("a3", "x", json!({"new_session_id": "s3"}))
            )),
            (409, Some("EXTERNAL_RECONCILIATION_REQUIRED".into())),
            "{bad}"
        );
    }
    assert!(!t.0.join("state/runs/a1").exists() && !t.0.join("state/runs/a3").exists());
}

#[test]
fn ids_must_be_safe() {
    let _serial = serial();
    let (t, svc) = setup("ids");
    for (attempt, sid) in [
        ("-a", "s1"),
        ("a..", "s1"),
        ("a1", "-s"),
        ("a1", "s/1"),
        ("a1", "_s"),
    ] {
        let (st, b) = call(
            &svc,
            "POST",
            "/v0/role/start",
            &req(attempt, "x", json!({"session_id": sid})),
        );
        assert_eq!(
            (st, b["code"].as_str()),
            (400, Some("SCHEMA_VIOLATION")),
            "{attempt} {sid}"
        );
    }
    let long = "a".repeat(65);
    let (st, _) = call(
        &svc,
        "POST",
        "/v0/role/start",
        &req(&long, "x", json!({"session_id": "s1"})),
    );
    assert_eq!(st, 400);
    assert!(
        !t.0.join("state/runs").exists()
            || std::fs::read_dir(t.0.join("state/runs"))
                .expect("d")
                .next()
                .is_none()
    );
}

#[test]
fn a_session_held_by_another_process_is_reported_not_restarted() {
    let _serial = serial();
    let (t, svc) = setup("lockprobe");
    call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("a1", "hi", json!({"session_id": "s1"})),
    );
    wait(&svc, "a1");
    // another holder of s1 (e.g. a process left from before a supervisor restart)
    let _held = ieo_supervisor::lock::SessionLock::acquire(&t.0.join("state/locks"), "s1", 4242)
        .expect("lock");
    let (st, b) = call(
        &svc,
        "POST",
        "/v0/role/reconnect",
        &req("a2", "x", json!({"new_session_id": "s2"})),
    );
    assert_eq!(
        (st, b["code"].as_str()),
        (409, Some("ALREADY_RUNNING")),
        "{b}"
    );
    assert_eq!(b["pid"], 4242);
    assert!(b["status"].is_object(), "{b}");
    assert!(!t.0.join("state/runs/a2").exists());
}

#[test]
fn an_empty_credential_is_unavailable() {
    let _serial = serial();
    let (t, svc) = setup("emptycred");
    std::fs::write(t.0.join("cred/oauth_token"), " \n").expect("empty");
    assert_eq!(
        code(call(
            &svc,
            "POST",
            "/v0/role/start",
            &req("a1", "x", json!({"session_id": "s1"}))
        )),
        (503, Some("CREDENTIAL_UNAVAILABLE".into()))
    );
    assert!(!t.0.join("state/runs/a1").exists());
}

fn with(a: &str, limits: Value) -> Vec<u8> {
    let mut v: Value =
        serde_json::from_slice(&req(a, "x", json!({"session_id": format!("s{a}")}))).expect("j");
    for (k, x) in limits.as_object().expect("o") {
        v["limits"][k] = x.clone();
    }
    serde_json::to_vec(&v).expect("j")
}

fn ledger_lines(t: &Tmp) -> Vec<Value> {
    std::fs::read_to_string(t.0.join("state/budget/attempts.jsonl"))
        .expect("ledger")
        .lines()
        .map(|l| serde_json::from_str(l).expect("line"))
        .collect()
}

fn write_ledger(t: &Tmp, lines: &[Value]) {
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    std::fs::write(t.0.join("state/budget/attempts.jsonl"), text).expect("w");
}

fn refused(svc: &Arc<Service>, a: &str, limits: Value) -> String {
    let (st, b) = call(svc, "POST", "/v0/role/start", &with(a, limits));
    assert_eq!(
        (st, b["code"].as_str()),
        (409, Some("BUDGET_EXCEEDED")),
        "{b}"
    );
    b["reason"].as_str().unwrap_or_default().to_string()
}

#[test]
fn the_service_enforces_its_own_budget_ceiling() {
    let _serial = serial();
    let (t, svc) = setup("budget");
    // grace beyond the shared constant: schema
    assert_eq!(
        code(call(
            &svc,
            "POST",
            "/v0/role/start",
            &with("a1", json!({"grace_ms": 6_001}))
        )),
        (400, Some("SCHEMA_VIOLATION".into()))
    );
    // per-run window above 1,800 s; token rooms above this role's share
    refused(
        &svc,
        "a1",
        json!({"timeout_ms": 1_794_001, "grace_ms": 6_000}),
    );
    refused(&svc, "a1", json!({"billable_room": 1_000_001}));
    refused(&svc, "a1", json!({"cache_read_room": 10_000_001}));
    assert!(!t.0.join("state/runs/a1").exists());
    // an accepted start is counted with its granted window, and its end with observed usage
    call(&svc, "POST", "/v0/role/start", &with("a1", json!({})));
    wait(&svc, "a1");
    let lines = ledger_lines(&t);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(
        (
            lines[0]["attempt_id"].as_str(),
            lines[0]["window_ms"].as_u64()
        ),
        (Some("a1"), Some(21_000))
    );
    // fake: 3 + 5 + 10 billable, 20 cache_read observed
    assert_eq!(
        lines[1]["end"],
        json!({"observed_billable": 18, "observed_cache_read": 20})
    );
    // tokens are cumulative within the share: observed so far + the requested room
    refused(&svc, "a2", json!({"billable_room": 1_000_000 - 17}));
    let (st, _) = call(
        &svc,
        "POST",
        "/v0/role/start",
        &with("a2", json!({"billable_room": 1_000_000 - 18})),
    );
    assert_eq!(st, 202);
    wait(&svc, "a2");
    // total granted time within the share (9,600 s)
    let start = lines[0].clone();
    let end = lines[1].clone();
    let fill = json!({"attempt_id": "x", "window_ms": 9_600_000 - 21_000 - 20_000});
    write_ledger(&t, &[start.clone(), end.clone(), fill]);
    assert!(refused(&svc, "a3", json!({})).contains("granted time"));
    // a start line without a numeric window counts as a full per-run window (1,800 s)
    let odd = json!({"attempt_id": "y", "window_ms": "1"});
    let six: Vec<Value> = std::iter::repeat_n(odd, 6).collect();
    let mut l = vec![start.clone(), end.clone()];
    l.extend(six);
    write_ledger(&t, &l);
    assert!(refused(&svc, "a3", json!({})).contains("granted time"));
    // run count: 8 started attempts end this role's share
    let eight: Vec<Value> = (0..8)
        .map(|i| json!({"attempt_id": format!("r{i}"), "window_ms": 1}))
        .collect();
    write_ledger(&t, &eight);
    assert!(refused(&svc, "a3", json!({})).contains("8 of 8"));
    // fail closed: an unreadable line, a deleted ledger while attempts exist, an attempt
    // without a recorded end
    std::fs::write(t.0.join("state/budget/attempts.jsonl"), "{not json\n").expect("w");
    assert!(refused(&svc, "a3", json!({})).contains("unreadable"));
    std::fs::remove_file(t.0.join("state/budget/attempts.jsonl")).expect("rm");
    assert!(refused(&svc, "a3", json!({})).contains("missing"));
    write_ledger(&t, &[start]);
    assert!(refused(&svc, "a3", json!({})).contains("reconcile"));
    assert!(!t.0.join("state/runs/a3").exists());
}

#[test]
fn limits_from_the_budget_path_are_accepted() {
    let _serial = serial();
    use ieo_supervisor::budget::{BUDGET_0204, Observation};
    use ieo_supervisor::run::Limits;
    let (_t, svc) = setup("fornext");
    let stream = json!({"type": "rate_limit_event", "rate_limit_info": {"status": "allowed",
        "overageStatus": "rejected", "isUsingOverage": false}})
    .to_string();
    let l = Limits::for_next(
        &BUDGET_0204,
        &[],
        Some(Observation {
            stream: &stream,
            age_seconds: 0,
        }),
    )
    .expect("start");
    // the whole per-run window, with the shared grace
    assert_eq!(l.timeout + l.grace, Duration::from_secs(1_800));
    // the caller clamps the rooms to the role's share (here: what this test run may use)
    let (st, b) = call(
        &svc,
        "POST",
        "/v0/role/start",
        &with(
            "a1",
            json!({"timeout_ms": l.timeout.as_millis() as u64, "grace_ms": l.grace.as_millis() as u64,
                   "billable_room": 100_000, "cache_read_room": 100_000}),
        ),
    );
    assert_eq!(st, 202, "{b}");
    wait(&svc, "a1");
}

#[test]
fn an_unfinished_run_is_not_resumed_and_the_resume_base_is_kept() {
    let _serial = serial();
    let (t, svc) = setup("unfinished");
    call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("a1", "hi", json!({"session_id": "s1"})),
    );
    wait(&svc, "a1");
    let p = t.0.join("state/checkpoint.json");
    let read =
        || -> Value { serde_json::from_slice(&std::fs::read(&p).expect("cp")).expect("json") };
    let base = read()["session_cumulative"].clone();
    assert!(!base.is_null());
    assert_eq!(read()["session_cumulative_attempt"], "a1");
    // N3: a resumed run stopped before its result keeps the base and names its run
    call(
        &svc,
        "POST",
        "/v0/role/reconnect",
        &req("a2", "SLEEP", json!({"new_session_id": "s9"})),
    );
    let t0 = Instant::now();
    while !std::fs::read_to_string(t.0.join("state/runs/a2/stdout.jsonl"))
        .unwrap_or_default()
        .contains("\"init\"")
    {
        assert!(t0.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        (
            read()["phase"].as_str(),
            read()["session_cumulative"].clone()
        ),
        (Some("takeover"), base.clone())
    );
    call(&svc, "POST", "/v0/role/stop", b"");
    let out = wait(&svc, "a2");
    assert_eq!(out["mode"], "resume");
    let cp = read();
    assert_eq!(
        (cp["phase"].as_str(), &cp["session_cumulative"]),
        (Some("after_run"), &base)
    );
    assert_eq!(cp["session_cumulative_attempt"], "a1");
    // N4: a checkpoint left at takeover (PID 1 died mid-run) is not resumed silently
    for phase in ["takeover", "error"] {
        let mut c = read();
        c["phase"] = json!(phase);
        std::fs::write(&p, serde_json::to_vec(&c).expect("j")).expect("w");
        let (st, b) = call(
            &svc,
            "POST",
            "/v0/role/reconnect",
            &req("a3", "x", json!({"new_session_id": "s3"})),
        );
        assert_eq!(
            (st, b["code"].as_str()),
            (409, Some("EXTERNAL_RECONCILIATION_REQUIRED")),
            "{b}"
        );
        assert!(
            b["reason"]
                .as_str()
                .is_some_and(|r| r.contains("did not finish")),
            "{b}"
        );
    }
    assert!(!t.0.join("state/runs/a3").exists());
}

#[test]
fn a_run_error_leaves_an_error_checkpoint_and_stops_further_starts() {
    let _serial = serial();
    let (t, svc) = setup("runerror");
    // the configured program is missing: the spawn fails
    std::fs::remove_file(t.0.join("fake-claude")).expect("rm");
    call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("a1", "hi", json!({"session_id": "s1"})),
    );
    let out = wait(&svc, "a1");
    assert!(
        out["error"].as_str().is_some_and(|e| e.starts_with("run:")),
        "{out}"
    );
    let cp: Value =
        serde_json::from_slice(&std::fs::read(t.0.join("state/checkpoint.json")).expect("cp"))
            .expect("j");
    assert_eq!(cp["phase"], "error");
    // usage of a failed attempt is unknown: the backstop refuses until a person reconciles
    let (st, b) = call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("a2", "hi", json!({"session_id": "s2"})),
    );
    assert_eq!(
        (st, b["code"].as_str()),
        (409, Some("BUDGET_EXCEEDED")),
        "{b}"
    );
    assert!(
        b["reason"].as_str().is_some_and(|r| r.contains("a1")),
        "{b}"
    );
}

// --- review 475b409 (carried to the caller PR) ---

fn wait_init(t: &Tmp, attempt: &str) {
    let t0 = Instant::now();
    while !std::fs::read_to_string(t.0.join(format!("state/runs/{attempt}/stdout.jsonl")))
        .unwrap_or_default()
        .contains("\"init\"")
    {
        assert!(t0.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_live_session_is_reported_running_before_its_checkpoint_phase_is_judged() {
    let _serial = serial();
    // R1: a holder of the session lock answers ALREADY_RUNNING even with a takeover checkpoint
    let (t, svc) = setup("r1");
    call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("a1", "hi", json!({"session_id": "s1"})),
    );
    wait(&svc, "a1");
    let p = t.0.join("state/checkpoint.json");
    let mut c: Value = serde_json::from_slice(&std::fs::read(&p).expect("cp")).expect("j");
    c["phase"] = json!("takeover");
    std::fs::write(&p, serde_json::to_vec(&c).expect("j")).expect("w");
    let held = ieo_supervisor::lock::SessionLock::acquire(&t.0.join("state/locks"), "s1", 4243)
        .expect("lock");
    let (st, b) = call(
        &svc,
        "POST",
        "/v0/role/reconnect",
        &req("a2", "x", json!({"new_session_id": "s2"})),
    );
    assert_eq!(
        (st, b["code"].as_str(), b["pid"].as_u64()),
        (409, Some("ALREADY_RUNNING"), Some(4243)),
        "{b}"
    );
    drop(held);
    let (st, b) = call(
        &svc,
        "POST",
        "/v0/role/reconnect",
        &req("a2", "x", json!({"new_session_id": "s2"})),
    );
    assert_eq!(
        (st, b["code"].as_str()),
        (409, Some("EXTERNAL_RECONCILIATION_REQUIRED")),
        "{b}"
    );
}

#[test]
fn a_result_written_after_a_guard_stop_is_not_a_resume_base() {
    let _serial = serial();
    // R2: the stopped run's late result (zero usage) does not replace the carried base
    let (t, svc) = setup("r2");
    call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("a1", "hi", json!({"session_id": "s1"})),
    );
    wait(&svc, "a1");
    call(
        &svc,
        "POST",
        "/v0/role/reconnect",
        &req("a2", "LATE", json!({"new_session_id": "s9"})),
    );
    wait_init(&t, "a2");
    call(&svc, "POST", "/v0/role/stop", b"");
    let out = wait(&svc, "a2");
    assert_eq!(out["stop"], "Requested", "{out}");
    let stdout = std::fs::read_to_string(t.0.join("state/runs/a2/stdout.jsonl")).expect("s");
    assert!(
        stdout.contains("0.99"),
        "the fake wrote no late result: {stdout}"
    );
    let cp: Value =
        serde_json::from_slice(&std::fs::read(t.0.join("state/checkpoint.json")).expect("cp"))
            .expect("j");
    assert_eq!(cp["session_cumulative"]["total_cost_usd"], 0.01, "{cp}");
    assert_eq!(cp["session_cumulative_attempt"], "a1");
}

#[test]
fn runs_are_counted_by_attempt_directories_too_and_shares_are_bounded() {
    let _serial = serial();
    // R6: more attempt directories than ledger lines count
    let (t, svc) = setup("r6");
    call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("a1", "hi", json!({"session_id": "s1"})),
    );
    wait(&svc, "a1");
    for i in 0..7 {
        std::fs::create_dir_all(t.0.join(format!("state/runs/x{i}"))).expect("d");
    }
    let (st, b) = call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("a2", "hi", json!({"session_id": "s2"})),
    );
    assert_eq!(
        (st, b["code"].as_str()),
        (409, Some("BUDGET_EXCEEDED")),
        "{b}"
    );
    assert!(
        b["reason"].as_str().is_some_and(|r| r.contains("8 of 8")),
        "{b}"
    );
    // R6: the share must be present and within the budget's caps
    let cfg = |share: Value| {
        let mut v = json!({"role": "guardrail", "repo": "r", "state": "/s", "etc": "/e",
            "cred": "/c", "claude": "/x", "quota_group_id": "test-shared", "checkpoint_central": "local_only", "budget": "02-04"});
        if !share.is_null() {
            v["share"] = share;
        }
        Config::from_json(&v)
    };
    let ok = json!({"max_runs": 12, "total_seconds": 14_400, "billable_tokens": 1_500_000,
                    "cache_read_tokens": 15_000_000});
    assert!(cfg(ok.clone()).is_ok());
    for (k, bad) in [
        ("max_runs", 13),
        ("total_seconds", 14_401),
        ("billable_tokens", 1_500_001),
        ("cache_read_tokens", 15_000_001),
        ("max_runs", 0),
    ] {
        let mut s = ok.clone();
        s[k] = json!(bad);
        assert!(cfg(s).is_err(), "{k} {bad}");
    }
    assert!(cfg(Value::Null).is_err());
}

#[test]
fn the_largest_end_line_of_an_attempt_counts() {
    let _serial = serial();
    // R4: a forged smaller end line (before or after the real one) does not lower the count
    let (t, svc) = setup("r4");
    call(&svc, "POST", "/v0/role/start", &with("a1", json!({})));
    wait(&svc, "a1");
    let lines = ledger_lines(&t);
    let (start, end) = (lines[0].clone(), lines[1].clone());
    let forged =
        json!({"attempt_id": "a1", "end": {"observed_billable": 0, "observed_cache_read": 0}});
    write_ledger(&t, &[start.clone(), forged.clone(), end.clone(), forged]);
    // observed 18: a room of 1,000,000 − 17 does not fit the share, 1,000,000 − 18 does
    refused(&svc, "a2", json!({"billable_room": 1_000_000 - 17}));
    // an end line without numbers makes the attempt unknown
    let broken = json!({"attempt_id": "a1", "end": {"unknown": true}});
    write_ledger(&t, &[start, end, broken]);
    assert!(refused(&svc, "a2", json!({})).contains("reconcile"));
}

// --- review adf760e (PR #54): §7 status fields and the safe-shutdown report ---

#[test]
fn status_reports_the_section_7_fields_and_an_honest_safe_shutdown() {
    let _serial = serial();
    let (t, svc) = setup("status7");
    let s = svc.status();
    assert_eq!(s["quota_group_id"], "test-shared");
    assert_eq!(s["provider_status_or_unknown"], "unknown");
    assert!(s["last_actual_action_at"].is_null());
    assert_eq!(s["restored_checkpoint_ref"]["found"], false);
    assert_eq!(s["safe_shutdown"]["active_children"], "not_tracked");
    assert_eq!(
        s["safe_shutdown"]["pending_external_requests"],
        "not_tracked"
    );
    assert!(
        s["capability_gaps"]
            .as_array()
            .is_some_and(|g| g.len() == 4),
        "{s}"
    );
    call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("a1", "hi", json!({"session_id": "s1"})),
    );
    wait(&svc, "a1");
    let s = svc.status();
    assert_eq!(s["safe_shutdown"]["checkpoint_saved"], true, "{s}");
    // the contract value, with the raw signal beside it (decision item 4)
    assert_eq!(s["provider_status_or_unknown"], "available", "{s}");
    assert_eq!(
        s["provider_status_detail"]["observation"]["source"]["overageStatus"], "rejected",
        "{s}"
    );
    assert!(s["last_actual_action_at"].is_string(), "{s}");
    // the checkpoint says what it does not track
    let cp: Value =
        serde_json::from_slice(&std::fs::read(t.0.join("state/checkpoint.json")).expect("cp"))
            .expect("j");
    assert_eq!(cp["tracked"]["active_children"], false, "{cp}");
    // a service started on this volume names the checkpoint it found
    let again = Service::new(svc_config(&t));
    // after a restart the earlier "available" is history, never the current status
    let s2 = again.status();
    assert_eq!(s2["provider_status_or_unknown"], "unknown", "{s2}");
    assert_eq!(
        s2["provider_status_before_start"]["observation"]["value"], "available",
        "{s2}"
    );
    assert_eq!(s2["provider_status_before_start"]["current"], false, "{s2}");
    let r = &again.status()["restored_checkpoint_ref"];
    assert_eq!(
        (
            r["found"].as_bool(),
            r["session_id"].as_str(),
            r["phase"].as_str()
        ),
        (Some(true), Some("s1"), Some("after_run")),
        "{r}"
    );
    // an unreadable checkpoint is not a saved one
    std::fs::write(t.0.join("state/checkpoint.json"), "{not json").expect("w");
    assert_eq!(svc.status()["safe_shutdown"]["checkpoint_saved"], false);
    assert_eq!(
        Service::new(svc_config(&t)).status()["restored_checkpoint_ref"]["readable"],
        false
    );
}

// --- 02-04 decision item 4: provider status values, freshness, live run, restart ---

/// A fake `claude` that prints `lines` (stream-json) and then sleeps `sleep` seconds.
fn scripted_claude(t: &Tmp, name: &str, lines: &[&str], sleep: u32) -> PathBuf {
    let p = t.0.join(name);
    let mut body = String::from("#!/bin/sh\nhead -c 4096 >/dev/null\n");
    for l in lines {
        body += &format!("printf '%s\\n' '{l}'\n");
    }
    body += &format!("sleep {sleep}\n");
    std::fs::write(&p, body).expect("fake");
    let _ = std::process::Command::new("chmod")
        .arg("+x")
        .arg(&p)
        .status();
    p
}

fn svc_with(t: &Tmp, claude: &Path, max_age: u64) -> Arc<Service> {
    let root = &t.0;
    Arc::new(Service::new(
        Config::from_json(&json!({"role": "development", "repo": "sori",
            "state": root.join("state"), "etc": root.join("etc"), "cred": root.join("cred"),
            "claude": claude, "budget": "02-04", "quota_group_id": "test-shared",
            "checkpoint_central": "local_only", "provider_status_max_age_seconds": max_age,
            "share": {"max_runs": 8, "total_seconds": 9_600, "billable_tokens": 1_000_000,
                      "cache_read_tokens": 10_000_000}}))
        .expect("config"),
    ))
}

const RL_ALLOWED: &str = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","rateLimitType":"five_hour","overageStatus":"rejected","isUsingOverage":false}}"#;
const RL_REJECTED: &str = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","rateLimitType":"five_hour","overageStatus":"rejected","isUsingOverage":false}}"#;
const AUTH_ERR: &str =
    r#"{"type":"assistant","error":"authentication_failed","message":{"id":"m0"}}"#;

#[test]
fn provider_status_maps_raw_signals_and_never_reuses_an_old_value() {
    let _serial = serial();
    let (t, _) = setup("provider");
    // 1. an allowed window → available
    let ok = scripted_claude(&t, "ok", &[RL_ALLOWED], 0);
    let svc = svc_with(&t, &ok, 600);
    call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("p1", "hi", json!({"session_id": "s1"})),
    );
    wait(&svc, "p1");
    assert_eq!(svc.status()["provider_status_or_unknown"], "available");
    // 2. while the next attempt runs, its own stream counts, never the previous attempt's
    //    "available". An auth error does not stop the run, so the live window is the fake's
    //    3 s sleep (review accd6c8 L-1: a rejected window stops the run within milliseconds)
    let live = scripted_claude(&t, "live", &[AUTH_ERR], 3);
    let svc = svc_with(&t, &live, 600);
    call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("p2", "hi", json!({"session_id": "s2"})),
    );
    let t0 = Instant::now();
    let s = loop {
        let s = svc.status();
        assert!(
            !s["running"].is_null() || s["provider_status_or_unknown"] == "auth_or_billing",
            "the run ended before its signal was seen live: {s}"
        );
        if s["provider_status_or_unknown"] == "auth_or_billing" {
            break s;
        }
        // before the event is written the running attempt has no signal: unknown, not available
        assert_ne!(s["provider_status_or_unknown"], "available", "{s}");
        assert!(t0.elapsed() < Duration::from_secs(10), "{s}");
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(!s["running"].is_null(), "observed during the run: {s}");
    assert_eq!(
        s["provider_status_detail"]["observation"]["source"]["error"],
        "authentication_failed"
    );
    // the service on this volume restarted: history only, current unknown
    let again = svc_with(&t, &live, 600);
    assert_eq!(again.status()["provider_status_or_unknown"], "unknown");
    wait(&svc, "p2");
    // a rejected window: the run is stopped by the budget guard and its status is usage_limit
    let limited = scripted_claude(&t, "limited", &[RL_REJECTED], 3);
    let svc = svc_with(&t, &limited, 600);
    call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("p2b", "hi", json!({"session_id": "s2b"})),
    );
    wait(&svc, "p2b");
    let s = svc.status();
    assert_eq!(s["provider_status_or_unknown"], "usage_limit", "{s}");
    assert_eq!(
        s["provider_status_detail"]["observation"]["source"]["status"],
        "rejected"
    );
    // 3. an auth error outranks a later allowed window of the same run
    let auth = scripted_claude(&t, "auth", &[RL_ALLOWED, AUTH_ERR, RL_ALLOWED], 0);
    let svc = svc_with(&t, &auth, 600);
    call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("p3", "hi", json!({"session_id": "s3"})),
    );
    wait(&svc, "p3");
    assert_eq!(
        svc.status()["provider_status_or_unknown"],
        "auth_or_billing"
    );
    // 4. no signal at all → unknown (never available, never zero)
    let silent = scripted_claude(&t, "silent", &[], 0);
    let svc = svc_with(&t, &silent, 600);
    call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("p4", "hi", json!({"session_id": "s4"})),
    );
    wait(&svc, "p4");
    assert_eq!(svc.status()["provider_status_or_unknown"], "unknown");
    // 5. an observation older than the maximum age is not current (age counted from the start)
    let slow = scripted_claude(&t, "slow", &[RL_ALLOWED], 2);
    let svc = svc_with(&t, &slow, 1);
    call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("p5", "hi", json!({"session_id": "s5"})),
    );
    wait(&svc, "p5");
    let s = svc.status();
    assert_eq!(s["provider_status_or_unknown"], "unknown", "{s}");
    assert_eq!(
        s["provider_status_detail"]["observation"]["value"], "available",
        "{s}"
    );
    // the quota group is the configured one; a missing or "unknown" group is refused
    assert_eq!(s["quota_group_id"], "test-shared");
    for q in [Value::Null, json!("unknown"), json!(""), json!("a b")] {
        let mut v = json!({"role": "guardrail", "repo": "r", "state": "/s", "etc": "/e", "cred": "/c",
            "claude": "/x", "budget": "02-04", "checkpoint_central": "local_only",
            "share": {"max_runs": 1, "total_seconds": 1, "billable_tokens": 1, "cache_read_tokens": 1}});
        if !q.is_null() {
            v["quota_group_id"] = q.clone();
        }
        assert!(Config::from_json(&v).is_err(), "{q}");
    }
}

/// `checkpoint.json` is written before its archive copy: a write stopped between the two leaves
/// the archive one behind, and the copy is written from `checkpoint.json` before the next start
/// (review accd6c8 L-3). A `checkpoint.json` older than its archive is never replaced: a
/// rollback stays visible (here: the older `takeover` checkpoint refuses the resume).
#[test]
fn a_missing_archive_copy_is_written_and_a_rollback_is_not_hidden() {
    let _serial = serial();
    let (t, svc) = setup("cp-repair");
    call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("r1", "hi", json!({"session_id": "s1"})),
    );
    wait(&svc, "r1");
    let archive = t.0.join("state/checkpoints");
    let second = archive.join(format!("{:020}.json", 2));
    std::fs::remove_file(&second).expect("rm archived 2");
    let (st, b) = call(
        &svc,
        "POST",
        "/v0/role/reconnect",
        &req("r2", "hi", json!({})),
    );
    assert_eq!(st, 202, "{b}");
    wait(&svc, "r2");
    assert!(
        second.exists(),
        "the archive copy was written from checkpoint.json"
    );
    // one step back (archive 4, checkpoint.json = archived 3, a takeover): refused, not restored
    let third = std::fs::read(archive.join(format!("{:020}.json", 3))).expect("archived 3");
    std::fs::write(t.0.join("state/checkpoint.json"), &third).expect("w");
    let (st, b) = call(
        &svc,
        "POST",
        "/v0/role/reconnect",
        &req("r3", "hi", json!({})),
    );
    assert_eq!(
        (st, b["code"].as_str()),
        (409, Some("EXTERNAL_RECONCILIATION_REQUIRED")),
        "{b}"
    );
    assert_eq!(
        std::fs::read(t.0.join("state/checkpoint.json")).expect("r"),
        third
    );
}

// --- review 802e4a1 (PR #57): staged takeover adoption, tamper reported once ---

/// A fake central record: stores reported checkpoints in order and answers `latest`. With
/// `lose` set, the next report is stored but answered with a server error (an answer the
/// supervisor cannot trust); with `dup` set, the next report is stored but answered as a
/// duplicate. Like the coordinator, it records a tamper event once per new session id, says so in
/// `tamper_recorded`, and says in `latest` (and a duplicate answer) whether that checkpoint's own
/// report recorded one. X-04: the same record is served over HTTP or gRPC
/// (`ieo.internal.v0`, the status and `ieo-error-bin` trailer of the coordinator's adapter); every
/// test that uses it runs once per transport.
struct FakeCentral {
    url: String,
    transport: &'static str,
    store: Store,
    events: Events,
    lose: Arc<std::sync::atomic::AtomicBool>,
    reject: Arc<std::sync::atomic::AtomicBool>,
    dup: Arc<std::sync::atomic::AtomicBool>,
}

/// The tamper events the fake recorded: new session id → seq of the report that recorded it.
type Events = Arc<std::sync::Mutex<std::collections::BTreeMap<String, u64>>>;

type Store = Arc<std::sync::Mutex<Vec<(u64, String, Value)>>>;

#[derive(Clone)]
struct Fake {
    store: Store,
    events: Events,
    lose: Arc<std::sync::atomic::AtomicBool>,
    reject: Arc<std::sync::atomic::AtomicBool>,
    dup: Arc<std::sync::atomic::AtomicBool>,
}

impl Fake {
    fn recorded_by(&self, seq: u64) -> bool {
        self.events
            .lock()
            .expect("events")
            .values()
            .any(|n| *n == seq)
    }

    /// The fake's answer, as HTTP status and body, to `GET latest` or a report body.
    fn answer(&self, method: &str, body: &[u8]) -> (u16, Value) {
        use std::sync::atomic::Ordering;
        let mut s = self.store.lock().expect("store");
        if method == "GET" {
            return match s.last() {
                Some((n, sha, _)) => (
                    200,
                    json!({"seq": n, "checkpoint_sha256": sha,
                           "tamper_recorded": self.recorded_by(*n)}),
                ),
                None => (404, json!({"code": "NO_CHECKPOINT"})),
            };
        }
        let v: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
        let seq = v["seq"].as_u64().unwrap_or(0);
        let raw = v["checkpoint_json"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let sha = hex::encode(Sha256::digest(raw.as_bytes()));
        let last = s.last().map(|x| x.0).unwrap_or(0);
        if self.reject.swap(false, Ordering::SeqCst) {
            (
                409,
                json!({"rejections": [{"code": "ASSIGNMENT_SHA_MISMATCH"}]}),
            )
        } else if seq == last + 1 {
            let cp: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
            // ON CONFLICT (repo, role, new_session_id) DO NOTHING
            let t = &cp["config_tamper_detected"];
            let tamper_recorded = !t.is_null() && {
                let mut ev = self.events.lock().expect("events");
                let id = t["new_session_id"].as_str().unwrap_or_default().to_string();
                !ev.contains_key(&id) && ev.insert(id, seq).is_none()
            };
            s.push((seq, sha.clone(), cp));
            if self.lose.swap(false, Ordering::SeqCst) {
                (502, json!({"code": "BAD_GATEWAY"}))
            } else if self.dup.swap(false, Ordering::SeqCst) {
                (
                    200,
                    json!({"duplicate": true, "seq": seq, "checkpoint_sha256": sha,
                           "tamper_recorded": tamper_recorded}),
                )
            } else {
                (
                    201,
                    json!({"stored": true, "seq": seq, "checkpoint_sha256": sha,
                           "tamper_recorded": tamper_recorded, "current_assignment": true}),
                )
            }
        } else {
            (
                409,
                json!({"rejections": [{"code": "CHECKPOINT_SEQUENCE_GAP"}]}),
            )
        }
    }
}

#[tonic::async_trait]
impl ieo_proto::v0::supervisor_service_server::SupervisorService for Fake {
    async fn submit_agent_report(
        &self,
        _: tonic::Request<ieo_proto::v0::SubmitAgentReportRequest>,
    ) -> Result<tonic::Response<ieo_proto::v0::SubmitAgentReportResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("fake"))
    }
    async fn submit_repro_request(
        &self,
        _: tonic::Request<ieo_proto::v0::SubmitReproRequestRequest>,
    ) -> Result<tonic::Response<ieo_proto::v0::SubmitReproRequestResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("fake"))
    }
    async fn get_repro_request(
        &self,
        _: tonic::Request<ieo_proto::v0::GetReproRequestRequest>,
    ) -> Result<tonic::Response<ieo_proto::v0::GetReproRequestResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("fake"))
    }
    async fn report_checkpoint(
        &self,
        r: tonic::Request<ieo_proto::v0::ReportCheckpointRequest>,
    ) -> Result<tonic::Response<ieo_proto::v0::ReportCheckpointResponse>, tonic::Status> {
        let m = r.into_inner();
        let body = ieo_proto::checkpoint_request_body(m.seq, &m.checkpoint_json);
        match self.answer("POST", &body) {
            (st @ 200..=299, v) => Ok(tonic::Response::new(
                ieo_proto::checkpoint_answer_from_json(st, &v),
            )),
            (st, v) => Err(ieo_proto::status_with_error(
                ieo_proto::grpc_code(st, &v),
                st,
                &v,
            )),
        }
    }
    // 02-05 step 2 RPCs: the supervisor does not call them yet (step 3)
    async fn get_own_assignment(
        &self,
        _: tonic::Request<ieo_proto::v0::GetOwnAssignmentRequest>,
    ) -> Result<tonic::Response<ieo_proto::v0::GetOwnAssignmentResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("fake"))
    }
    async fn record_first_action(
        &self,
        _: tonic::Request<ieo_proto::v0::RecordFirstActionRequest>,
    ) -> Result<tonic::Response<ieo_proto::v0::RecordFirstActionResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("fake"))
    }
    async fn report_session_status(
        &self,
        _: tonic::Request<ieo_proto::v0::ReportSessionStatusRequest>,
    ) -> Result<tonic::Response<ieo_proto::v0::ReportSessionStatusResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("fake"))
    }
    async fn get_latest_checkpoint(
        &self,
        _: tonic::Request<ieo_proto::v0::GetLatestCheckpointRequest>,
    ) -> Result<tonic::Response<ieo_proto::v0::GetLatestCheckpointResponse>, tonic::Status> {
        match self.answer("GET", &[]) {
            (200, v) => Ok(tonic::Response::new(ieo_proto::latest_from_json(&v))),
            (st, v) => Err(ieo_proto::status_with_error(
                ieo_proto::grpc_code(st, &v),
                st,
                &v,
            )),
        }
    }
}

fn fake_central(transport: &'static str) -> FakeCentral {
    use std::sync::atomic::AtomicBool;
    let fake = Fake {
        store: Arc::new(std::sync::Mutex::new(Vec::new())),
        events: Arc::new(std::sync::Mutex::new(Default::default())),
        lose: Arc::new(AtomicBool::new(false)),
        reject: Arc::new(AtomicBool::new(false)),
        dup: Arc::new(AtomicBool::new(false)),
    };
    let url = if transport == "grpc" {
        let (tx, rx) = std::sync::mpsc::channel();
        let f = fake.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .expect("runtime");
            rt.block_on(async move {
                let incoming = tonic::transport::server::TcpIncoming::bind(
                    "127.0.0.1:0".parse().expect("addr"),
                )
                .expect("bind");
                let _ = tx.send(incoming.local_addr().expect("addr"));
                let _ = tonic::transport::Server::builder()
                    .add_service(
                        ieo_proto::v0::supervisor_service_server::SupervisorServiceServer::new(f),
                    )
                    .serve_with_incoming(incoming)
                    .await;
            });
        });
        format!("http://{}", rx.recv().expect("grpc fake"))
    } else {
        let server = tiny_http::Server::http("127.0.0.1:0").expect("bind");
        let url = format!("http://{}", server.server_addr().to_ip().expect("ip"));
        let f = fake.clone();
        std::thread::spawn(move || {
            for mut req in server.incoming_requests() {
                let mut body = Vec::new();
                let _ = std::io::Read::read_to_end(req.as_reader(), &mut body);
                let (code, out) = f.answer(req.method().as_str(), &body);
                let _ = req.respond(
                    tiny_http::Response::from_string(out.to_string()).with_status_code(code),
                );
            }
        });
        url
    };
    FakeCentral {
        url,
        transport,
        events: fake.events,
        store: fake.store,
        lose: fake.lose,
        reject: fake.reject,
        dup: fake.dup,
    }
}

/// Each test of the central record, once per coordinator transport (X-04).
macro_rules! over_both_transports {
    ($($name:ident),* $(,)?) => {$(
        mod $name {
            #[test]
            fn http() {
                let _serial = super::serial();
                super::$name("http")
            }
            #[test]
            fn grpc() {
                let _serial = super::serial();
                super::$name("grpc")
            }
        }
    )*};
}

over_both_transports!(
    a_lost_takeover_answer_is_adopted_and_its_tamper_is_reported_once,
    a_takeover_accepted_but_not_written_locally_is_adopted_next_time,
    a_detection_survives_refusals_and_a_refused_takeover_keeps_it_pending,
    a_tamper_the_coordinator_did_not_record_stays_pending,
    an_unrecorded_tamper_stays_pending_after_a_duplicate_or_an_adopted_takeover,
    an_unreadable_pending_file_stops_until_a_person_reconciles_it,
);

fn svc_central(t: &Tmp, c: &FakeCentral) -> Arc<Service> {
    let root = &t.0;
    std::fs::write(root.join("cred/coordinator_token"), "sup-token").expect("token");
    Arc::new(Service::new(
        Config::from_json(&json!({"role": "development", "repo": "sori",
            "state": root.join("state"), "etc": root.join("etc"), "cred": root.join("cred"),
            "claude": root.join("fake-claude"), "budget": "02-04", "quota_group_id": "test-shared",
            "checkpoint_central": {"url": c.url, "token_file": root.join("cred/coordinator_token"),
                                   "transport": c.transport},
            "share": {"max_runs": 8, "total_seconds": 9_600, "billable_tokens": 1_000_000,
                      "cache_read_tokens": 10_000_000}}))
        .expect("config"),
    ))
}

fn a_lost_takeover_answer_is_adopted_and_its_tamper_is_reported_once(transport: &'static str) {
    let (t, _) = setup("lost-answer");
    let c = fake_central(transport);
    let svc = svc_central(&t, &c);
    let state = t.0.join("state");
    call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("l1", "hi", json!({"session_id": "s1"})),
    );
    wait(&svc, "l1");
    assert_eq!(c.store.lock().expect("s").len(), 2);
    // a tamper, and the takeover carrying it is stored but its answer is a server error
    std::fs::write(state.join("claude/CLAUDE.md"), "injected").expect("tamper");
    c.lose.store(true, std::sync::atomic::Ordering::SeqCst);
    let (st, b) = call(
        &svc,
        "POST",
        "/v0/role/reconnect",
        &req("l2", "hi", json!({"new_session_id": "s2"})),
    );
    assert_eq!(st, 202, "{b}");
    let last = wait(&svc, "l2");
    assert_eq!(
        (last["spawned"].as_bool(), last["stop"].as_str()),
        (Some(false), Some("CentralRefused")),
        "{last}"
    );
    assert!(
        state.join("checkpoint-staged.json").exists(),
        "a 5xx keeps the staged copy"
    );
    assert!(state.join("pending-tamper.json").exists());
    // the next reconnect adopts the stored takeover and its tamper counts as reported
    // (review 802e4a1 N-1); the takeover is an unfinished run, so nothing resumes on it
    let (st, b) = call(
        &svc,
        "POST",
        "/v0/role/reconnect",
        &req("l3", "hi", json!({"new_session_id": "s3"})),
    );
    assert_eq!(
        (st, b["code"].as_str()),
        (409, Some("EXTERNAL_RECONCILIATION_REQUIRED")),
        "{b}"
    );
    assert!(!state.join("checkpoint-staged.json").exists());
    assert!(
        !state.join("pending-tamper.json").exists(),
        "the tamper was reported with the adopted takeover"
    );
    let cp: Value =
        serde_json::from_slice(&std::fs::read(state.join("checkpoint.json")).expect("cp"))
            .expect("j");
    assert_eq!(
        (cp["seq"].as_u64(), cp["session_id"].as_str()),
        (Some(3), Some("s2"))
    );
    // a new session starts without carrying the tamper again
    let (st, b) = call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("l4", "hi", json!({"session_id": "s4"})),
    );
    assert_eq!(st, 202, "{b}");
    assert!(b["accepted"]["tamper"].is_null(), "{b}");
    wait(&svc, "l4");
    let tampers = c
        .store
        .lock()
        .expect("s")
        .iter()
        .filter(|x| !x.2["config_tamper_detected"].is_null())
        .count();
    assert_eq!(tampers, 1, "one detection, one report");
}

fn a_takeover_accepted_but_not_written_locally_is_adopted_next_time(transport: &'static str) {
    let (t, _) = setup("local-write");
    let c = fake_central(transport);
    let svc = svc_central(&t, &c);
    let state = t.0.join("state");
    // checkpoint.json cannot be written (a directory in its place)
    std::fs::create_dir_all(state.join("checkpoint.json")).expect("dir");
    call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("w1", "hi", json!({"session_id": "s1"})),
    );
    let last = wait(&svc, "w1");
    assert_eq!(
        (last["spawned"].as_bool(), last["stop"].as_str()),
        (Some(false), Some("LocalWriteFailed")),
        "{last}"
    );
    assert_eq!(
        c.store.lock().expect("s").len(),
        1,
        "the coordinator holds the takeover"
    );
    assert!(state.join("checkpoint-staged.json").exists());
    // the attempt closed as not spawned (0/0): the next start is not blocked
    let ledger = std::fs::read_to_string(state.join("budget/attempts.jsonl")).expect("ledger");
    assert!(
        ledger.contains(r#""end":{"observed_billable":0,"observed_cache_read":0}"#),
        "{ledger}"
    );
    std::fs::remove_dir(state.join("checkpoint.json")).expect("rmdir");
    let (st, b) = call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("w2", "hi", json!({"session_id": "s2"})),
    );
    assert_eq!(st, 202, "{b}");
    wait(&svc, "w2");
    assert!(!state.join("checkpoint-staged.json").exists());
    let cp: Value =
        serde_json::from_slice(&std::fs::read(state.join("checkpoint.json")).expect("cp"))
            .expect("j");
    assert_eq!(
        (cp["seq"].as_u64(), cp["phase"].as_str()),
        (Some(3), Some("after_run")),
        "{cp}"
    );
    assert_eq!(c.store.lock().expect("s").len(), 3);
}

/// A pending tamper is reported with a new session only: the tampered session id is refused
/// (review 802e4a1 N-2).
#[test]
fn a_pending_tamper_needs_a_session_other_than_the_tampered_one() {
    let _serial = serial();
    let (t, svc) = setup("tamper-new");
    call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("n1", "hi", json!({"session_id": "s1"})),
    );
    wait(&svc, "n1");
    std::fs::write(
        t.0.join("state/pending-tamper.json"),
        json!({"config_tamper_detected": true, "old_session_id": "s1", "new_session_id": "x",
               "checkpoint_digest": "a".repeat(64), "observed_digest": "b".repeat(64)})
        .to_string(),
    )
    .expect("pending");
    for (op, extra) in [
        ("start", json!({"session_id": "s1"})),
        ("reconnect", json!({"new_session_id": "s1"})),
    ] {
        let (st, b) = call(
            &svc,
            "POST",
            &format!("/v0/role/{op}"),
            &req("n2", "hi", extra),
        );
        assert_eq!(
            (st, b["code"].as_str()),
            (400, Some("SCHEMA_VIOLATION")),
            "{op}: {b}"
        );
    }
    let (st, b) = call(
        &svc,
        "POST",
        "/v0/role/reconnect",
        &req("n2", "hi", json!({})),
    );
    assert_eq!(
        (st, b["code"].as_str()),
        (400, Some("SCHEMA_VIOLATION")),
        "{b}"
    );
    let (st, b) = call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("n2", "hi", json!({"session_id": "s2"})),
    );
    assert_eq!(st, 202, "{b}");
    assert_eq!(b["accepted"]["tamper"]["old_session_id"], "s1", "{b}");
    assert_eq!(b["accepted"]["tamper"]["new_session_id"], "s2", "{b}");
    wait(&svc, "n2");
    assert!(
        !t.0.join("state/pending-tamper.json").exists(),
        "reported with the takeover"
    );
}

/// A fresh tamper detection survives a refused request (review 704330f P-1); a takeover the
/// coordinator refuses (4xx) drops the staged copy but keeps the tamper pending, and a staged
/// copy not newer than the local checkpoint is dropped (P-3, review 802e4a1 I-1/I-2).
fn a_detection_survives_refusals_and_a_refused_takeover_keeps_it_pending(transport: &'static str) {
    let (t, _) = setup("tamper-keep");
    let c = fake_central(transport);
    let svc = svc_central(&t, &c);
    let state = t.0.join("state");
    call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("k1", "hi", json!({"session_id": "s1"})),
    );
    wait(&svc, "k1");
    std::fs::write(state.join("claude/CLAUDE.md"), "injected").expect("tamper");
    // refused before anything runs: no new session id, then the tampered one
    let (st, _) = call(
        &svc,
        "POST",
        "/v0/role/reconnect",
        &req("k2", "hi", json!({})),
    );
    assert_eq!(st, 400);
    let (st, _) = call(
        &svc,
        "POST",
        "/v0/role/reconnect",
        &req("k2", "hi", json!({"new_session_id": "s1"})),
    );
    assert_eq!(st, 400);
    assert!(
        state.join("pending-tamper.json").exists(),
        "the detection was kept"
    );
    // the config is clean again, but the detection is not lost
    std::fs::remove_file(state.join("claude/CLAUDE.md")).expect("rm");
    // a takeover the coordinator refuses: staged copy dropped, detection still pending
    c.reject.store(true, std::sync::atomic::Ordering::SeqCst);
    let (st, b) = call(
        &svc,
        "POST",
        "/v0/role/reconnect",
        &req("k2", "hi", json!({"new_session_id": "s2"})),
    );
    assert_eq!(st, 202, "{b}");
    assert_eq!(b["accepted"]["mode"], "new", "{b}");
    assert_eq!(wait(&svc, "k2")["stop"], "CentralRefused");
    assert!(
        !state.join("checkpoint-staged.json").exists(),
        "a 4xx drops the staged copy"
    );
    assert!(state.join("pending-tamper.json").exists());
    // a staged copy not newer than the local checkpoint is dropped, never adopted (review
    // 802e4a1 I-2). Its bytes differ from the coordinator's latest record at the same seq, so
    // only that rule removes it (review ae65f0c Q-1): without it the copy would be kept
    let local = std::fs::read(state.join("checkpoint.json")).expect("cp");
    let mut other: Value = serde_json::from_slice(&local).expect("j");
    other["phase"] = json!("takeover");
    other["session_id"] = json!("s-other");
    let other = serde_json::to_vec_pretty(&other).expect("j");
    assert_ne!(other, local);
    std::fs::write(state.join("checkpoint-staged.json"), &other).expect("staged");
    // a request refused after the comparison: the copy is gone, checkpoint.json unchanged
    let (st, _) = call(
        &svc,
        "POST",
        "/v0/role/reconnect",
        &req("k3", "hi", json!({})),
    );
    assert_eq!(st, 400);
    assert!(
        !state.join("checkpoint-staged.json").exists(),
        "a staged copy not newer than local is dropped"
    );
    assert_eq!(
        std::fs::read(state.join("checkpoint.json")).expect("cp"),
        local,
        "and never adopted"
    );
    let (st, b) = call(
        &svc,
        "POST",
        "/v0/role/reconnect",
        &req("k3", "hi", json!({"new_session_id": "s3"})),
    );
    assert_eq!(st, 202, "{b}");
    assert!(!state.join("checkpoint-staged.json").exists());
    assert_eq!(b["accepted"]["tamper"]["old_session_id"], "s1", "{b}");
    wait(&svc, "k3");
    assert!(!state.join("pending-tamper.json").exists());
    // the coordinator records a detection once per new session; every checkpoint of that
    // session carries it
    let sessions: std::collections::BTreeSet<String> = c
        .store
        .lock()
        .expect("s")
        .iter()
        .filter(|x| !x.2["config_tamper_detected"].is_null())
        .map(|x| x.2["config_tamper_detected"]["new_session_id"].to_string())
        .collect();
    assert_eq!(sessions.len(), 1, "one detection, one report: {sessions:?}");
}

/// Review 704330f P-2: a takeover whose tamper event the coordinator dropped
/// (`tamper_recorded: false`, a new session id used by an earlier event) keeps the detection
/// pending; the next new session reports it.
fn a_tamper_the_coordinator_did_not_record_stays_pending(transport: &'static str) {
    let (t, _) = setup("tamper-unrecorded");
    let c = fake_central(transport);
    let svc = svc_central(&t, &c);
    let state = t.0.join("state");
    let run = |attempt: &str, op: &str, extra: Value| {
        let (st, b) = call(
            &svc,
            "POST",
            &format!("/v0/role/{op}"),
            &req(attempt, "hi", extra),
        );
        assert_eq!(st, 202, "{attempt}: {b}");
        wait(&svc, attempt);
        b
    };
    run("u1", "start", json!({"session_id": "s1"}));
    // a tamper reported with s2: recorded, the pending file goes
    std::fs::write(state.join("claude/CLAUDE.md"), "injected").expect("tamper");
    let b = run("u2", "reconnect", json!({"new_session_id": "s2"}));
    assert_eq!(b["accepted"]["tamper"]["new_session_id"], "s2", "{b}");
    assert!(!state.join("pending-tamper.json").exists());
    run("u3", "start", json!({"session_id": "s3"}));
    // a second tamper, reported with s2 again: the coordinator drops the event
    std::fs::write(state.join("claude/CLAUDE.md"), "injected again").expect("tamper");
    let b = run("u4", "reconnect", json!({"new_session_id": "s2"}));
    assert_eq!(b["accepted"]["tamper"]["old_session_id"], "s3", "{b}");
    assert_eq!(
        c.events.lock().expect("e").len(),
        1,
        "the event was dropped"
    );
    let pending: Value = serde_json::from_slice(
        &std::fs::read(state.join("pending-tamper.json")).expect("still pending"),
    )
    .expect("j");
    assert_eq!(pending["old_session_id"], "s3", "{pending}");
    // never resumed over: the next reconnect is a new session that reports it
    let b = run("u5", "reconnect", json!({"new_session_id": "s5"}));
    assert_eq!(b["accepted"]["mode"], "new", "{b}");
    assert_eq!(b["accepted"]["tamper"]["old_session_id"], "s3", "{b}");
    assert!(!state.join("pending-tamper.json").exists());
    assert!(c.events.lock().expect("e").contains_key("s5"));
}

/// Review ae65f0c Q-2 and acceptance §4: an earlier pending detection is kept over a fresh one
/// (the `(Some(p), false, _)` arm and the "not yet pending" condition), and a pending file that
/// does not read is replaced by the fresh detection instead of blocking it.
#[test]
fn an_older_pending_detection_is_kept_and_an_unreadable_one_is_replaced() {
    let _serial = serial();
    let (t, svc) = setup("tamper-older");
    let state = t.0.join("state");
    call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("o1", "hi", json!({"session_id": "s1"})),
    );
    wait(&svc, "o1");
    std::fs::write(state.join("claude/CLAUDE.md"), "first").expect("tamper");
    let (st, _) = call(
        &svc,
        "POST",
        "/v0/role/reconnect",
        &req("o2", "hi", json!({})),
    );
    assert_eq!(st, 400);
    let first = std::fs::read(state.join("pending-tamper.json")).expect("saved");
    let first_v: Value = serde_json::from_slice(&first).expect("j");
    // a different tamper, refused again: the earlier detection stays as it was
    std::fs::write(state.join("claude/CLAUDE.md"), "second, different").expect("tamper");
    let (st, _) = call(
        &svc,
        "POST",
        "/v0/role/reconnect",
        &req("o2", "hi", json!({})),
    );
    assert_eq!(st, 400);
    assert_eq!(
        std::fs::read(state.join("pending-tamper.json")).expect("kept"),
        first
    );
    let (st, b) = call(
        &svc,
        "POST",
        "/v0/role/reconnect",
        &req("o2", "hi", json!({"new_session_id": "s3"})),
    );
    assert_eq!(st, 202, "{b}");
    let tamper = &b["accepted"]["tamper"];
    assert_eq!(
        (
            tamper["old_session_id"].as_str(),
            tamper["new_session_id"].as_str(),
            &tamper["observed_digest"]
        ),
        (Some("s1"), Some("s3"), &first_v["observed_digest"]),
        "the earlier detection is the one reported: {b}"
    );
    wait(&svc, "o2");
    assert!(!state.join("pending-tamper.json").exists());

    // an unreadable pending file does not block a fresh detection
    let (t, svc) = setup("tamper-unreadable");
    let state = t.0.join("state");
    call(
        &svc,
        "POST",
        "/v0/role/start",
        &req("r1", "hi", json!({"session_id": "s1"})),
    );
    wait(&svc, "r1");
    std::fs::write(state.join("pending-tamper.json"), "{not json").expect("corrupt");
    std::fs::write(state.join("claude/CLAUDE.md"), "injected").expect("tamper");
    let (st, _) = call(
        &svc,
        "POST",
        "/v0/role/reconnect",
        &req("r2", "hi", json!({})),
    );
    assert_eq!(st, 400);
    let saved: Value =
        serde_json::from_slice(&std::fs::read(state.join("pending-tamper.json")).expect("p"))
            .expect("the fresh detection replaced the unreadable file");
    assert_eq!(
        (
            saved["config_tamper_detected"].as_bool(),
            saved["old_session_id"].as_str()
        ),
        (Some(true), Some("s1")),
        "{saved}"
    );
    // the config clean again: still reported, not resumed over
    std::fs::remove_file(state.join("claude/CLAUDE.md")).expect("rm");
    let (st, b) = call(
        &svc,
        "POST",
        "/v0/role/reconnect",
        &req("r2", "hi", json!({"new_session_id": "s3"})),
    );
    assert_eq!(st, 202, "{b}");
    assert_eq!(
        (
            b["accepted"]["mode"].as_str(),
            b["accepted"]["tamper"]["old_session_id"].as_str()
        ),
        (Some("new"), Some("s1")),
        "{b}"
    );
    wait(&svc, "r2");
}

/// Review 681c1d3 L-1: the other two paths that cleared a pending tamper — a duplicate answer to
/// the takeover, and the adoption of a staged takeover whose answer was lost — clear it only when
/// the coordinator says that takeover's report recorded the event. Here the event is dropped
/// (the new session id was used by an earlier event, the P-2 case), so it stays pending.
fn an_unrecorded_tamper_stays_pending_after_a_duplicate_or_an_adopted_takeover(
    transport: &'static str,
) {
    use std::sync::atomic::Ordering;
    for path in ["duplicate", "adopted"] {
        let (t, _) = setup(&format!("tamper-l1-{path}"));
        let c = fake_central(transport);
        let svc = svc_central(&t, &c);
        let state = t.0.join("state");
        let send = |attempt: &str, op: &str, extra: Value| {
            call(
                &svc,
                "POST",
                &format!("/v0/role/{op}"),
                &req(attempt, "hi", extra),
            )
        };
        let run = |attempt: &str, op: &str, extra: Value| {
            let (st, b) = send(attempt, op, extra);
            assert_eq!(st, 202, "{path} {attempt}: {b}");
            wait(&svc, attempt)
        };
        run("v1", "start", json!({"session_id": "s1"}));
        std::fs::write(state.join("claude/CLAUDE.md"), "injected").expect("tamper");
        run("v2", "reconnect", json!({"new_session_id": "s2"}));
        assert!(!state.join("pending-tamper.json").exists());
        run("v3", "start", json!({"session_id": "s3"}));
        std::fs::write(state.join("claude/CLAUDE.md"), "injected again").expect("tamper");
        // the takeover carrying the second detection reuses s2: stored, event dropped
        if path == "duplicate" {
            c.dup.store(true, Ordering::SeqCst);
            run("v4", "reconnect", json!({"new_session_id": "s2"}));
        } else {
            c.lose.store(true, Ordering::SeqCst);
            let last = run("v4", "reconnect", json!({"new_session_id": "s2"}));
            assert_eq!(last["stop"], "CentralRefused", "{last}");
            assert!(state.join("checkpoint-staged.json").exists());
            // the next request adopts the staged takeover (the coordinator holds exactly it)
            let (st, b) = send("v5", "reconnect", json!({"new_session_id": "s5"}));
            assert_eq!(st, 409, "{b}");
            assert!(!state.join("checkpoint-staged.json").exists(), "adopted");
        }
        assert_eq!(
            c.events.lock().expect("e").len(),
            1,
            "{path}: the event was dropped"
        );
        let pending: Value = serde_json::from_slice(
            &std::fs::read(state.join("pending-tamper.json"))
                .unwrap_or_else(|_| panic!("{path}: the detection must stay pending")),
        )
        .expect("j");
        assert_eq!(pending["old_session_id"], "s3", "{path}: {pending}");
    }
}

/// User decision 2026-11-15 (X-08; reviews ae65f0c Q-2, 681c1d3 I-4): a pending tamper file
/// that does not read as a detection (corrupt bytes, or JSON that is not an object) stops every
/// start and reconnect with `EXTERNAL_RECONCILIATION_REQUIRED` and is kept, unless a fresh
/// detection replaces it. Before the decision it read as "no pending" and the session resumed.
fn an_unreadable_pending_file_stops_until_a_person_reconciles_it(transport: &'static str) {
    for content in ["{not json", "[1, 2]", "\"text\""] {
        let (t, _) = setup("tamper-unreadable-stop");
        let c = fake_central(transport);
        let svc = svc_central(&t, &c);
        let state = t.0.join("state");
        let (st, b) = call(
            &svc,
            "POST",
            "/v0/role/start",
            &req("a1", "hi", json!({"session_id": "s1"})),
        );
        assert_eq!(st, 202, "{b}");
        wait(&svc, "a1");
        std::fs::write(state.join("pending-tamper.json"), content).expect("w");
        for (op, extra) in [
            ("reconnect", json!({"new_session_id": "s2"})),
            ("start", json!({"session_id": "s2"})),
        ] {
            let (st, b) = call(
                &svc,
                "POST",
                &format!("/v0/role/{op}"),
                &req("a2", "hi", extra),
            );
            assert_eq!(
                (st, b["code"].as_str()),
                (409, Some("EXTERNAL_RECONCILIATION_REQUIRED")),
                "{content} {op}: {b}"
            );
            assert_eq!(
                std::fs::read_to_string(state.join("pending-tamper.json")).expect("kept"),
                content,
                "the file is kept for a person"
            );
        }
        assert!(!state.join("runs/a2").exists(), "nothing started");
        // a fresh detection replaces the file and is reported with a new session
        std::fs::write(state.join("claude/CLAUDE.md"), "injected").expect("tamper");
        let (st, b) = call(
            &svc,
            "POST",
            "/v0/role/reconnect",
            &req("a2", "hi", json!({"new_session_id": "s2"})),
        );
        assert_eq!(st, 202, "{content}: {b}");
        assert_eq!(b["accepted"]["mode"], "new", "{b}");
        assert_eq!(b["accepted"]["tamper"]["old_session_id"], "s1", "{b}");
        wait(&svc, "a2");
        assert!(!state.join("pending-tamper.json").exists());
    }
}
