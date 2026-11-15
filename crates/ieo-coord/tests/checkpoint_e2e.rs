//! 02-04 decision item 9 end to end: the role supervisor service (`ieo-supervisor`, fake
//! `claude`, no model call) reports its checkpoints to the coordinator binary on a real
//! PostgreSQL (`IEO_TEST_DB_URL`); the coordinator stores them, compares them with the local
//! checkpoint and the current assignment, and judges resumption — across restarts of the
//! coordinator and of the role service. Skipped without `IEO_TEST_DB_URL` outside CI.

use ieo_supervisor::serve::{Config, Service};
use postgres::NoTls;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const MANIFEST: &str = include_str!("../../../docs/poc/Console_Coordinator_Sprints_v0.json");
const SHA1: &str = "1111111111111111111111111111111111111111";
const ADAPTER: &str = "e2e-adapter-token";

fn digest(t: &str) -> String {
    hex::encode(Sha256::digest(t.as_bytes()))
}

fn http(
    port: u16,
    who: Option<&str>,
    method: &str,
    path: &str,
    body: Option<&Value>,
) -> Option<(u16, Value)> {
    let payload = body.map(|b| b.to_string()).unwrap_or_default();
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).ok()?;
    s.set_read_timeout(Some(Duration::from_secs(30))).ok()?;
    let mut req = format!("{method} {path} HTTP/1.1\r\nHost: t\r\nConnection: close\r\n");
    if let Some(t) = who {
        req += &format!("Authorization: Bearer {t}\r\n");
    }
    if path.starts_with("/v0/sprints?") {
        req += "Idempotency-Key: e2e-reg-0001\r\n";
    }
    req += &format!(
        "Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{payload}",
        payload.len()
    );
    s.write_all(req.as_bytes()).ok()?;
    let mut out = String::new();
    s.read_to_string(&mut out).ok()?;
    let status = out.split_whitespace().nth(1)?.parse().ok()?;
    let b = out.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or("");
    Some((status, serde_json::from_str(b).unwrap_or(Value::Null)))
}

struct Coord {
    child: Option<Child>,
    port: u16,
    args: Vec<String>,
    db_url: String,
}

impl Coord {
    fn start(&mut self) {
        let child = Command::new(env!("CARGO_BIN_EXE_ieo-coord"))
            .args(&self.args)
            .env("IEO_DB_URL", &self.db_url)
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn coordinator");
        self.child = Some(child);
        let t0 = Instant::now();
        while http(self.port, None, "GET", "/v0/health", None).map(|r| r.0) != Some(200) {
            assert!(
                t0.elapsed() < Duration::from_secs(20),
                "the coordinator did not start"
            );
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    fn stop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = Command::new("kill")
                .args(["-TERM", &c.id().to_string()])
                .status();
            let t0 = Instant::now();
            while c.try_wait().ok().flatten().is_none() {
                if t0.elapsed() > Duration::from_secs(10) {
                    let _ = c.kill();
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

impl Drop for Coord {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The role service behind a fixed HTTP port; the service inside can be replaced (a restart of
/// the role process on the same volume).
struct Role {
    svc: Arc<Mutex<Arc<Service>>>,
    port: u16,
}

impl Role {
    fn new(cfg: Config) -> Role {
        let svc = Arc::new(Mutex::new(Arc::new(Service::new(cfg))));
        let server = tiny_http::Server::http("127.0.0.1:0").expect("bind");
        let port = server.server_addr().to_ip().expect("ip").port();
        let h = Arc::clone(&svc);
        std::thread::spawn(move || {
            for mut req in server.incoming_requests() {
                let bearer = req
                    .headers()
                    .iter()
                    .find(|x| x.field.equiv("Authorization"))
                    .and_then(|x| x.value.as_str().strip_prefix("Bearer ").map(str::to_string));
                let mut body = Vec::new();
                let _ = req.as_reader().read_to_end(&mut body);
                let s = Arc::clone(&h.lock().expect("svc"));
                let r = s.handle(req.method().as_str(), req.url(), bearer.as_deref(), &body);
                let _ = req.respond(
                    tiny_http::Response::from_data(serde_json::to_vec(&r.body).unwrap_or_default())
                        .with_status_code(r.status),
                );
            }
        });
        Role { svc, port }
    }
    fn restart(&self, cfg: Config) {
        *self.svc.lock().expect("svc") = Arc::new(Service::new(cfg));
    }
    fn call(&self, method: &str, path: &str, body: &Value) -> (u16, Value) {
        http(self.port, Some(ADAPTER), method, path, Some(body)).expect("role answered")
    }
    fn status(&self) -> Value {
        http(self.port, Some(ADAPTER), "GET", "/v0/role/status", None)
            .expect("status")
            .1
    }
    fn wait(&self, attempt: &str) -> Value {
        let t0 = Instant::now();
        loop {
            let s = self.status();
            if s["running"].is_null() && s["last"]["attempt_id"] == attempt {
                return s;
            }
            assert!(t0.elapsed() < Duration::from_secs(30), "{s}");
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

fn fake_claude(dir: &Path) -> PathBuf {
    let p = dir.join("fake-claude");
    std::fs::write(
        &p,
        r#"#!/bin/sh
prompt=$(head -c 4096)
sid=$(printf '%s\n' "$@" | awk 'prev=="--session-id"||prev=="--resume"{print; exit} {prev=$0}')
printf '{"type":"system","subtype":"init","session_id":"%s"}\n' "$sid"
printf '{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","overageStatus":"rejected","isUsingOverage":false}}\n'
case "$prompt" in *SLEEP*) sleep 3;; esac
printf '{"type":"assistant","message":{"id":"m1","usage":{"input_tokens":3,"output_tokens":5,"cache_creation_input_tokens":10,"cache_read_input_tokens":20}}}\n'
printf '{"type":"result","subtype":"success","is_error":false,"session_id":"%s","usage":{"input_tokens":3,"output_tokens":5,"cache_creation_input_tokens":10,"cache_read_input_tokens":20},"modelUsage":{"m":{"inputTokens":3,"outputTokens":5,"cacheReadInputTokens":20,"cacheCreationInputTokens":10}},"total_cost_usd":0.01}\n' "$sid"
"#,
    )
    .expect("fake");
    let _ = Command::new("chmod").arg("+x").arg(&p).status();
    p
}

fn spec() -> Value {
    let m: Value = serde_json::from_str(MANIFEST).expect("manifest");
    let mut s = m["sprints"][0].clone();
    s["registration_state"] = json!("ready");
    s["target"] = json!({"repo_id": "repo-a", "repository_url": "https://example.invalid/repo-a"});
    s["limits"] = json!({"elapsed_seconds": 3600, "total_tokens": 100000, "correction_attempts": 3,
                         "unresolved_reason": null});
    s["unresolved_bindings"] = json!([]);
    s["revision"] = json!(5);
    s["tasks"][0]["outputs"] = json!([
        {"artifact_id": "01-01-output", "description": "required output", "required": true},
        {"artifact_id": "01-01-notes", "description": "optional notes", "required": false}]);
    s["digest"] = json!(ieo_spec::spec_digest(&s));
    s
}

#[test]
fn checkpoints_are_reported_compared_and_judged_across_restarts() {
    let Ok(base) = std::env::var("IEO_TEST_DB_URL") else {
        assert!(
            std::env::var("CI").is_err(),
            "IEO_TEST_DB_URL must be set in CI"
        );
        eprintln!("SKIP: IEO_TEST_DB_URL not set");
        return;
    };
    let cfg: postgres::Config = base.parse().expect("IEO_TEST_DB_URL");
    let dbname = format!("ieo_t_cpe2e_{}", std::process::id());
    let mut admin = cfg.connect(NoTls).expect("connect");
    let _ = admin.batch_execute(&format!("DROP DATABASE IF EXISTS {dbname} WITH (FORCE)"));
    admin
        .batch_execute(&format!("CREATE DATABASE {dbname}"))
        .expect("create db");
    let dir = std::env::temp_dir().join(format!("ieo-cp-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    let tok = |n: &str| format!("e2e-{n}-{}", std::process::id());
    let principals = json!({"principals": [
        {"name": "alice", "token_sha256": digest(&tok("alice")), "kind": "user", "scopes": ["sprints:register", "sprints:read"], "repo_ids": ["repo-a"]},
        {"name": "bob", "token_sha256": digest(&tok("bob")), "kind": "user", "scopes": ["review:semantic", "sprints:read"], "repo_ids": ["repo-a"]},
        {"name": "assigner", "token_sha256": digest(&tok("assigner")), "kind": "service", "scopes": ["coordinator:assign"], "repo_ids": ["repo-a"]},
        {"name": "operator", "token_sha256": digest(&tok("operator")), "kind": "service", "scopes": ["roles:control"], "repo_ids": ["repo-a"]},
        {"name": "sup-dev", "token_sha256": digest(&tok("sup")), "kind": "agent", "scopes": ["checkpoint:report"], "repo_ids": ["repo-a"], "role": "development"}]});
    std::fs::write(dir.join("tokens.json"), principals.to_string()).expect("tokens");
    // the role volume and its read-only parts
    let root = dir.join("dev");
    std::fs::create_dir_all(root.join("etc/template")).expect("etc");
    std::fs::write(
        root.join("etc/template/settings.json"),
        r#"{"autoMemoryEnabled": false}"#,
    )
    .expect("t");
    std::fs::create_dir_all(root.join("cred")).expect("cred");
    std::fs::write(root.join("cred/oauth_token"), "fake-oauth-token\n").expect("o");
    std::fs::write(root.join("cred/adapter.sha256"), digest(ADAPTER)).expect("a");
    std::fs::write(root.join("cred/coordinator_token"), tok("sup")).expect("c");
    std::fs::write(dir.join("adapter_token"), ADAPTER).expect("at");
    let claude = fake_claude(&dir);
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .expect("port")
        .port();
    let role_cfg = || {
        Config::from_json(&json!({"role": "development", "repo": "repo-a",
            "state": root.join("state"), "etc": root.join("etc"), "cred": root.join("cred"),
            "claude": claude, "budget": "02-04", "quota_group_id": "anthropic-shared-0104",
            "checkpoint_central": {"url": format!("http://127.0.0.1:{port}"),
                                   "token_file": root.join("cred/coordinator_token")},
            "share": {"max_runs": 8, "total_seconds": 9_600, "billable_tokens": 1_000_000,
                      "cache_read_tokens": 10_000_000}}))
        .expect("role config")
    };
    let role = Role::new(role_cfg());
    std::fs::write(dir.join("roles.json"), json!({"repo-a": {
        "development": {"url": format!("http://127.0.0.1:{}", role.port), "adapter_token_file": dir.join("adapter_token"),
                        "quota_group_id": "anthropic-shared-0104"},
        "guardrail": {"url": "http://127.0.0.1:9", "adapter_token_file": "/nonexistent"}}}).to_string()).expect("roles");
    let rootdir = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let mut coord = Coord {
        child: None,
        port,
        db_url: format!("{base} dbname={dbname}"),
        args: vec![
            "serve".into(),
            "--listen".into(),
            format!("127.0.0.1:{port}"),
            "--tokens".into(),
            dir.join("tokens.json").display().to_string(),
            "--roles".into(),
            dir.join("roles.json").display().to_string(),
            "--manifest".into(),
            format!("{rootdir}/docs/poc/Console_Coordinator_Sprints_v0.json"),
            "--policy".into(),
            format!("{rootdir}/contracts/judgment-policy-v0.json"),
        ],
    };
    coord.start();
    let c = |who: &str, method: &str, path: &str, body: Option<&Value>| {
        http(port, Some(&tok(who)), method, path, body).expect("coordinator answered")
    };
    // an executable, reviewed sprint and the current assignment of (repo-a, development)
    let s = spec();
    let sd = s["digest"].as_str().expect("digest").to_string();
    assert_eq!(
        c("alice", "POST", "/v0/sprints?mode=executable", Some(&s)).0,
        201
    );
    let (st, b) = c(
        "bob",
        "POST",
        "/v0/sprints/POC-01/revisions/5/semantic-review",
        Some(&json!({"decision": "passed", "reason": "scope ok", "digest": sd})),
    );
    assert_eq!(st, 201, "{b}");
    let assign = |generation: u64, expected: Value| {
        c(
            "assigner",
            "POST",
            "/v0/coordinator/assignments",
            Some(&json!({"repo_id": "repo-a",
            "role": "development", "assignment_id": "A1", "generation": generation, "sprint_id": "POC-01",
            "sprint_revision": 5, "task_id": "01-01", "target_sha": SHA1, "expected_current": expected})),
        )
    };
    assert_eq!(assign(0, Value::Null).0, 201);
    let compare = || {
        c(
            "operator",
            "GET",
            "/v0/repos/repo-a/roles/development/checkpoints/compare",
            None,
        )
        .1
    };
    let latest = || {
        c(
            "operator",
            "GET",
            "/v0/repos/repo-a/roles/development/checkpoints/latest",
            None,
        )
        .1
    };
    let req = |attempt: &str, generation: u64, prompt: &str, extra: Value| {
        let mut v = json!({"attempt_id": attempt, "prompt": prompt, "target_sha": SHA1,
            "coordinator": {"generation": generation, "assignment_id": "A1", "sprint_id": "POC-01",
                "sprint_revision": 5, "spec_digest": sd, "image_digest": "sha256:img",
                "profile_id": "p", "profile_revision": 1},
            "limits": {"timeout_ms": 20_000, "billable_room": 100_000, "cache_read_room": 100_000, "grace_ms": 1_000}});
        for (k, x) in extra.as_object().expect("obj") {
            v[k] = x.clone();
        }
        v
    };
    let state = root.join("state");
    let cp_path = state.join("checkpoint.json");
    let ran = |a: &str| state.join("runs").join(a).exists();

    // 1. nothing yet: a new session may start
    assert_eq!(compare()["decision"], "no_checkpoint");
    // 2. a normal run: takeover (1) and after_run (2) reported, consistent
    let (st, b) = role.call(
        "POST",
        "/v0/role/start",
        &req("a1", 0, "hi", json!({"session_id": "s1"})),
    );
    assert_eq!(st, 202, "{b}");
    let s = role.wait("a1");
    assert_eq!(s["safe_shutdown"]["checkpoint_reported"], true, "{s}");
    let l = latest();
    assert_eq!(
        (
            l["seq"].as_u64(),
            l["phase"].as_str(),
            l["session_id"].as_str()
        ),
        (Some(2), Some("after_run"), Some("s1")),
        "{l}"
    );
    assert_eq!(
        l["checkpoint_sha256"].as_str(),
        Some(digest_bytes(&std::fs::read(&cp_path).expect("cp")).as_str())
    );
    assert_eq!(compare()["decision"], "resume_allowed");
    // 3. the coordinator restarts: the record persists
    coord.stop();
    coord.start();
    assert_eq!(compare()["decision"], "resume_allowed");
    // 4. the role process restarts on its volume: compared again before the resume
    role.restart(role_cfg());
    let (st, b) = role.call("POST", "/v0/role/reconnect", &req("a2", 0, "hi", json!({})));
    assert_eq!(st, 202, "{b}");
    assert_eq!(b["accepted"]["mode"], "resume", "{b}");
    role.wait("a2");
    assert_eq!(latest()["seq"], 4);
    // 5. mismatch, missing, rolled back local checkpoint: nothing runs, the coordinator agrees
    let good = std::fs::read(&cp_path).expect("cp");
    let mut v: Value = serde_json::from_slice(&good).expect("j");
    v["last_event_offset"] = json!(999);
    std::fs::write(&cp_path, serde_json::to_vec_pretty(&v).expect("j")).expect("w");
    assert_eq!(compare()["decision"], "external_reconciliation_required");
    let (st, b) = role.call("POST", "/v0/role/reconnect", &req("a3", 0, "hi", json!({})));
    assert_eq!(
        (st, b["code"].as_str()),
        (409, Some("EXTERNAL_RECONCILIATION_REQUIRED")),
        "{b}"
    );
    std::fs::remove_file(&cp_path).expect("rm");
    let (st, b) = role.call("POST", "/v0/role/reconnect", &req("a3", 0, "hi", json!({})));
    assert_eq!(
        (st, b["code"].as_str()),
        (409, Some("EXTERNAL_RECONCILIATION_REQUIRED")),
        "{b}"
    );
    let older = std::fs::read(state.join("checkpoints").join(format!("{:020}.json", 3)))
        .expect("archived 3");
    std::fs::write(&cp_path, &older).expect("w");
    let (st, b) = role.call("POST", "/v0/role/reconnect", &req("a3", 0, "hi", json!({})));
    assert_eq!(
        (st, b["code"].as_str()),
        (409, Some("EXTERNAL_RECONCILIATION_REQUIRED")),
        "{b}"
    );
    assert!(
        b["reason"].as_str().is_some_and(|r| r.contains("older")),
        "{b}"
    );
    assert!(!ran("a3"), "a refused reconnect started nothing");
    std::fs::write(&cp_path, &good).expect("restore");
    assert_eq!(compare()["decision"], "resume_allowed");
    // 6. the coordinator goes away during a run: the after_run report fails and says so; on
    //    the next reconnect the missing report is sent from the archive first
    let (st, b) = role.call(
        "POST",
        "/v0/role/reconnect",
        &req("a3", 0, "SLEEP", json!({})),
    );
    assert_eq!(st, 202, "{b}");
    std::thread::sleep(Duration::from_millis(500));
    coord.stop();
    let s = role.wait("a3");
    assert_eq!(s["safe_shutdown"]["checkpoint_reported"], false, "{s}");
    assert_eq!(s["central_checkpoint"]["last_report"]["ok"], false, "{s}");
    // a start while the coordinator is unreachable: not observed, nothing runs
    let (st, b) = role.call("POST", "/v0/role/reconnect", &req("a4", 0, "hi", json!({})));
    assert_eq!((st, b["code"].as_str()), (503, Some("NOT_OBSERVED")), "{b}");
    assert!(!ran("a4"));
    coord.start();
    let j = compare();
    assert_eq!(j["decision"], "external_reconciliation_required", "{j}");
    assert!(
        j["reasons"][0]
            .as_str()
            .is_some_and(|r| r.contains("not yet recorded")),
        "{j}"
    );
    let (st, b) = role.call("POST", "/v0/role/reconnect", &req("a4", 0, "hi", json!({})));
    assert_eq!(st, 202, "{b}");
    assert_eq!(b["accepted"]["central"]["sent"], 1, "{b}");
    role.wait("a4");
    assert_eq!(latest()["seq"], 8);
    // 7. a directive change in the config dir: tamper detected, a new session, reported with
    //    its checkpoints and recorded once
    std::fs::write(state.join("claude/CLAUDE.md"), "injected").expect("tamper");
    let (st, b) = role.call(
        "POST",
        "/v0/role/reconnect",
        &req("a5", 0, "hi", json!({"new_session_id": "s2"})),
    );
    assert_eq!(st, 202, "{b}");
    assert_eq!(
        b["accepted"]["tamper"]["config_tamper_detected"], true,
        "{b}"
    );
    role.wait("a5");
    let l = latest();
    assert_eq!(
        (
            l["seq"].as_u64(),
            l["session_id"].as_str(),
            l["tamper_events"].as_i64()
        ),
        (Some(10), Some("s2"), Some(1)),
        "{l}"
    );
    // 8. a newer generation of the assignment: the checkpoint is stale
    assert_eq!(
        assign(1, json!({"assignment_id": "A1", "generation": 0})).0,
        201
    );
    assert_eq!(compare()["decision"], "stale_generation");
    let (st, b) = role.call("POST", "/v0/role/reconnect", &req("a6", 1, "hi", json!({})));
    assert_eq!(
        (st, b["code"].as_str()),
        (409, Some("STALE_GENERATION")),
        "{b}"
    );
    assert!(!ran("a6"));
    // the database holds the sequence without gaps or changes
    let mut db = {
        let mut c2 = cfg.clone();
        c2.dbname(&dbname);
        c2.connect(NoTls).expect("db")
    };
    let n: i64 = db
        .query_one(
            "SELECT count(*) FROM role_checkpoints WHERE repo_id = 'repo-a'",
            &[],
        )
        .expect("q")
        .get(0);
    assert_eq!(n, 10);
    drop(db);
    coord.stop();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = admin.batch_execute(&format!("DROP DATABASE IF EXISTS {dbname} WITH (FORCE)"));
}

fn digest_bytes(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}
