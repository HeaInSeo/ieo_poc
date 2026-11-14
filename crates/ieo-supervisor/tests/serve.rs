//! Role supervisor service (02-04) with a fake `claude`, no model calls.

use ieo_supervisor::serve::{Config, Service};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

const TOKEN: &str = "adapter-test-token";

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
    let cfg = Config::from_json(&json!({"role": "development", "repo": "sori",
        "state": root.join("state"), "etc": etc, "cred": cred,
        "claude": fake_claude(root), "budget": "02-04",
        "share": {"max_runs": 8, "total_seconds": 9_600, "billable_tokens": 1_000_000,
                  "cache_read_tokens": 10_000_000}}))
    .expect("config");
    (t, Arc::new(Service::new(cfg)))
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
            "cred": "/c", "claude": "/x", "budget": "02-04"});
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
