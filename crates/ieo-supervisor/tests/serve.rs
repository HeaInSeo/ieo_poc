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
case "$prompt" in *SLEEP*) sleep 60;; esac
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
        "claude": fake_claude(root)}))
    .expect("config");
    (t, Arc::new(Service::new(cfg)))
}

fn coord(generation: u64) -> Value {
    json!({"generation": generation, "assignment_id": "A1", "sprint_id": "POCX", "sprint_revision": 3,
           "spec_digest": "dg", "image_digest": "sha256:img", "profile_id": "dev", "profile_revision": 1})
}

fn req(attempt: &str, prompt: &str, extra: Value) -> Vec<u8> {
    let mut v = json!({"attempt_id": attempt, "prompt": prompt, "coordinator": coord(0),
        "limits": {"timeout_ms": 20_000, "billable_room": 1_000_000, "cache_read_room": 1_000_000,
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
    std::thread::sleep(Duration::from_millis(300));
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
    // stop ends the run through the guard
    let (st, _) = call(&svc, "POST", "/v0/role/stop", b"");
    assert_eq!(st, 202);
    let out = wait(&svc, "a1");
    assert_eq!(out["stop"], "Requested", "{out}");
    assert_eq!(out["stopped_by_guard"], true);
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
