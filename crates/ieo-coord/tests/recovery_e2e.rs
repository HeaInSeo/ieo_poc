//! 02-05 step 7b end to end: the official recovery after a crash (user decision 2026-11-16 (나),
//! docs/poc/User_Decisions_2026-11-16_recovery.md §2). The coordinator binary, the role supervisor
//! service (`ieo-supervisor`, fake `claude`, no model call) and a real PostgreSQL
//! (`IEO_TEST_DB_URL`): a person approves a new session over an unfinished checkpoint, the
//! approval is checked again when a delivery uses it, and it starts at most one run. A crash is
//! simulated by the takeover checkpoint the supervisor reports before a run (crafted with a fixture
//! delivery), as the coordinator and the role volume hold it after a crash.
//! Skipped without `IEO_TEST_DB_URL` outside CI.

mod common;
use common::free_port;
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
const SHA2: &str = "2222222222222222222222222222222222222222";
const BRANCH: &str = "refs/heads/feat/a1";
const ADAPTER: &str = "e2e-adapter-token";

fn digest(t: &str) -> String {
    hex::encode(Sha256::digest(t.as_bytes()))
}

fn http(
    port: u16,
    who: Option<&str>,
    key: Option<&str>,
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
    if let Some(k) = key {
        req += &format!("Idempotency-Key: {k}\r\n");
    }
    req += &format!(
        "Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{payload}",
        payload.len()
    );
    s.write_all(req.as_bytes()).ok()?;
    let mut out = String::new();
    s.read_to_string(&mut out).ok()?;
    let status = out.split_whitespace().nth(1)?.parse().ok()?;
    let (head, b) = out.split_once("\r\n\r\n").unwrap_or((&out, ""));
    // a large answer comes chunked (the assignment chain with its recoveries)
    let body = if head
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        let mut rest = b;
        let mut body = String::new();
        while let Some((size, tail)) = rest.split_once("\r\n") {
            let n = usize::from_str_radix(size.trim(), 16).unwrap_or(0);
            if n == 0 || tail.len() < n {
                break;
            }
            body.push_str(&tail[..n]);
            rest = tail[n..].trim_start_matches("\r\n");
        }
        body
    } else {
        b.to_string()
    };
    Some((status, serde_json::from_str(&body).unwrap_or(Value::Null)))
}

struct Coord {
    child: Option<Child>,
    port: u16,
    args: Vec<String>,
    db_url: String,
    /// The coordinator's stderr, appended across restarts, inside the test's directory (removed
    /// with it; PR #84 review b291d7c I-1).
    log: std::path::PathBuf,
}

impl Coord {
    fn start(&mut self) {
        // the coordinator's stderr is kept (appended across restarts) and shown when it does not
        // start (main CI ad96116: "the coordinator did not start" without a reason)
        let log = self.log.clone();
        let err = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)
            .expect("coordinator log");
        let mut child = Command::new(env!("CARGO_BIN_EXE_ieo-coord"))
            .args(&self.args)
            .env("IEO_DB_URL", &self.db_url)
            .stderr(err)
            .spawn()
            .expect("spawn coordinator");
        let t0 = Instant::now();
        while http(self.port, None, None, "GET", "/v0/health", None).map(|r| r.0) != Some(200) {
            let exited = child.try_wait().ok().flatten();
            assert!(
                t0.elapsed() < Duration::from_secs(20) && exited.is_none(),
                "the coordinator did not start (exit {exited:?}): {}",
                std::fs::read_to_string(&log).unwrap_or_default()
            );
            std::thread::sleep(Duration::from_millis(100));
        }
        self.child = Some(child);
    }
    /// SIGTERM, then SIGKILL after 30 s; whether it exited by itself with status 0.
    fn stop(&mut self) -> bool {
        let Some(mut c) = self.child.take() else {
            return true;
        };
        let _ = Command::new("kill")
            .args(["-TERM", &c.id().to_string()])
            .status();
        let t0 = Instant::now();
        let mut killed = false;
        loop {
            if let Some(st) = c.try_wait().ok().flatten() {
                return !killed && st.success();
            }
            if t0.elapsed() > Duration::from_secs(30) && !killed {
                let _ = c.kill();
                killed = true;
            }
            std::thread::sleep(Duration::from_millis(50));
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
    _svc: Arc<Mutex<Arc<Service>>>,
    port: u16,
    /// The next start or reconnect is handled but its answer is lost (the connection closes).
    drop_next: Arc<std::sync::atomic::AtomicBool>,
    /// Start and reconnect requests that reached the service.
    launches: Arc<std::sync::atomic::AtomicUsize>,
    /// While set, the status answers 503 (the coordinator cannot observe the role).
    fail_status: Arc<std::sync::atomic::AtomicBool>,
    /// The body of the last start or reconnect that reached the service.
    last_launch: Arc<Mutex<Value>>,
}

impl Role {
    fn new(cfg: Config) -> Role {
        let svc = Arc::new(Mutex::new(Arc::new(Service::new(cfg))));
        let server = tiny_http::Server::http("127.0.0.1:0").expect("bind");
        let port = server.server_addr().to_ip().expect("ip").port();
        let h = Arc::clone(&svc);
        let drop_next = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let launches = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let fail_status = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let last_launch = Arc::new(Mutex::new(Value::Null));
        let (d, n, f, last) = (
            Arc::clone(&drop_next),
            Arc::clone(&launches),
            Arc::clone(&fail_status),
            Arc::clone(&last_launch),
        );
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
                let launch = req.method().as_str() == "POST"
                    && (req.url().ends_with("/start") || req.url().ends_with("/reconnect"));
                if launch {
                    n.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    *last.lock().expect("last") =
                        serde_json::from_slice(&body).unwrap_or(Value::Null);
                }
                if req.url().ends_with("/status") && f.load(std::sync::atomic::Ordering::SeqCst) {
                    let _ =
                        req.respond(tiny_http::Response::from_string("{}").with_status_code(503));
                    continue;
                }
                let r = s.handle(req.method().as_str(), req.url(), bearer.as_deref(), &body);
                if launch && d.swap(false, std::sync::atomic::Ordering::SeqCst) {
                    // handled, answer lost: no readable response reaches the caller
                    let mut w = req.into_writer();
                    let _ = w.write_all(b"LOST\r\n\r\n");
                    let _ = w.flush();
                    drop(w);
                    continue;
                }
                let _ = req.respond(
                    tiny_http::Response::from_data(serde_json::to_vec(&r.body).unwrap_or_default())
                        .with_status_code(r.status),
                );
            }
        });
        Role {
            _svc: svc,
            port,
            drop_next,
            launches,
            fail_status,
            last_launch,
        }
    }
    fn call(&self, method: &str, path: &str, body: &Value) -> (u16, Value) {
        http(self.port, Some(ADAPTER), None, method, path, Some(body)).expect("role answered")
    }
    fn status(&self) -> Value {
        http(
            self.port,
            Some(ADAPTER),
            None,
            "GET",
            "/v0/role/status",
            None,
        )
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

/// A fake `claude` (no model call) whose session totals grow per session, so a resumed run's
/// share is measurable (the 02-04 caller's fake, reduced). Prompt "SLEEP" runs for 3 s.
fn fake_claude(dir: &Path) -> PathBuf {
    let p = dir.join("fake-claude");
    let cum = dir.join("cum");
    std::fs::write(
        &p,
        format!(
            r#"#!/bin/sh
prompt=$(head -c 4096)
sid=$(printf '%s\n' "$@" | awk 'prev=="--session-id"||prev=="--resume"{{print; exit}} {{prev=$0}}')
f="{cum}-$sid"
set -- $(cat "$f" 2>/dev/null || echo 0 0 0 0)
i=$(($1+3)); o=$(($2+7)); r=$(($3+20)); c=$(($4+10))
echo "$i $o $r $c" > "$f"
printf '{{"type":"system","subtype":"init","session_id":"%s"}}\n' "$sid"
printf '{{"type":"assistant","message":{{"id":"m-%s-%s","usage":{{"input_tokens":3,"output_tokens":5,"cache_creation_input_tokens":10,"cache_read_input_tokens":20}}}}}}\n' "$sid" "$i"
printf '{{"type":"rate_limit_event","rate_limit_info":{{"status":"allowed","overageStatus":"rejected","isUsingOverage":false}}}}\n'
case "$prompt" in *SLEEP*) sleep 3;; esac
printf '{{"type":"result","subtype":"success","is_error":false,"session_id":"%s","usage":{{"input_tokens":3,"output_tokens":7,"cache_creation_input_tokens":10,"cache_read_input_tokens":20}},"modelUsage":{{"m":{{"inputTokens":%s,"outputTokens":%s,"cacheReadInputTokens":%s,"cacheCreationInputTokens":%s}}}},"total_cost_usd":0.01}}\n' "$sid" "$i" "$o" "$r" "$c"
"#,
            cum = cum.display()
        ),
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

/// A delivery's session id as a lowercase UUID (a start's `claude --session-id` takes only a
/// UUID; PR #83 review 67b5cae M-1): the test's short name, fixed per name.
fn sid(name: &str) -> String {
    let h = name.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    });
    format!(
        "{:08x}-{:04x}-4{:03x}-8{:03x}-{:012x}",
        h >> 32,
        (h >> 16) & 0xffff,
        h & 0xfff,
        (h >> 4) & 0xfff,
        h & 0xffff_ffff_ffff
    )
}

#[test]
fn a_person_approves_one_new_session_over_an_unfinished_checkpoint() {
    e2e("http");
}

/// The supervisor reaches the coordinator over gRPC; the delivery and the approval are HTTP.
#[test]
fn a_person_approves_one_new_session_over_an_unfinished_checkpoint_over_grpc() {
    e2e("grpc");
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs()
}

fn e2e(transport: &str) {
    let Ok(base) = std::env::var("IEO_TEST_DB_URL") else {
        assert!(
            std::env::var("CI").is_err(),
            "IEO_TEST_DB_URL must be set in CI"
        );
        eprintln!("SKIP: IEO_TEST_DB_URL not set");
        return;
    };
    let cfg: postgres::Config = base.parse().expect("IEO_TEST_DB_URL");
    let dbname = format!("ieo_t_rcv_{transport}_{}", std::process::id());
    let mut admin = cfg.connect(NoTls).expect("connect");
    let _ = admin.batch_execute(&format!("DROP DATABASE IF EXISTS {dbname} WITH (FORCE)"));
    admin
        .batch_execute(&format!("CREATE DATABASE {dbname}"))
        .expect("create db");
    let dir = std::env::temp_dir().join(format!("ieo-rcv-e2e-{transport}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    let tok = |n: &str| format!("rcv-{n}-{}", std::process::id());
    let principals = json!({"principals": [
        {"name": "alice", "token_sha256": digest(&tok("alice")), "kind": "user", "scopes": ["sprints:register", "sprints:read"], "repo_ids": ["repo-a"]},
        {"name": "bob", "token_sha256": digest(&tok("bob")), "kind": "user", "scopes": ["review:semantic", "sprints:read"], "repo_ids": ["repo-a"]},
        {"name": "assigner", "token_sha256": digest(&tok("assigner")), "kind": "service", "scopes": ["coordinator:assign"], "repo_ids": ["repo-a"]},
        {"name": "operator", "token_sha256": digest(&tok("operator")), "kind": "service", "scopes": ["roles:control"], "repo_ids": ["repo-a"]},
        {"name": "operator-b", "token_sha256": digest(&tok("operator-b")), "kind": "service", "scopes": ["roles:control"], "repo_ids": ["repo-b"]},
        {"name": "sup-dev", "token_sha256": digest(&tok("sup")), "kind": "agent", "scopes": ["checkpoint:report"], "repo_ids": ["repo-a"], "role": "development"},
        {"name": "sup-gr", "token_sha256": digest(&tok("sup-gr")), "kind": "agent", "scopes": ["checkpoint:report"], "repo_ids": ["repo-a"], "role": "guardrail"},
        {"name": "ingest", "token_sha256": digest(&tok("ingest")), "kind": "service", "scopes": ["evidence:ingest"], "repo_ids": ["repo-a"]},
        {"name": "alice-ops", "token_sha256": digest(&tok("alice-ops")), "kind": "user", "scopes": ["roles:control"], "repo_ids": ["repo-a"]},
        {"name": "bob-ops", "token_sha256": digest(&tok("bob-ops")), "kind": "user", "scopes": ["roles:control"], "repo_ids": ["repo-b"]},
        {"name": "carol", "token_sha256": digest(&tok("carol")), "kind": "user", "scopes": ["sprints:read"], "repo_ids": ["repo-a"]},
        {"name": "dev-agent", "token_sha256": digest(&tok("dev-agent")), "kind": "agent", "scopes": ["agent:report"], "repo_ids": ["repo-a"], "role": "development"}]});
    std::fs::write(dir.join("tokens.json"), principals.to_string()).expect("tokens");
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
    let port = free_port();
    let grpc_port = free_port();
    let central_port = if transport == "grpc" { grpc_port } else { port };
    let role_cfg = || {
        Config::from_json(&json!({"role": "development", "repo": "repo-a",
            "state": root.join("state"), "etc": root.join("etc"), "cred": root.join("cred"),
            "claude": claude, "budget": "02-04", "quota_group_id": "anthropic-shared-0104",
            "checkpoint_central": {"url": format!("http://127.0.0.1:{central_port}"),
                                   "token_file": root.join("cred/coordinator_token"),
                                   "transport": transport},
            "share": {"max_runs": 12, "total_seconds": 9_600, "billable_tokens": 1_000_000,
                      "cache_read_tokens": 10_000_000}}))
        .expect("role config")
    };
    let role = Role::new(role_cfg());
    // the guardrail role's service (idle throughout; the budget check asks it too)
    let groot = dir.join("guard");
    std::fs::create_dir_all(groot.join("etc/template")).expect("etc");
    std::fs::write(
        groot.join("etc/template/settings.json"),
        r#"{"autoMemoryEnabled": false}"#,
    )
    .expect("t");
    std::fs::create_dir_all(groot.join("cred")).expect("cred");
    std::fs::write(groot.join("cred/adapter.sha256"), digest(ADAPTER)).expect("a");
    let guard = Role::new(
        Config::from_json(&json!({"role": "guardrail", "repo": "repo-a",
            "state": groot.join("state"), "etc": groot.join("etc"), "cred": groot.join("cred"),
            "claude": "/nonexistent", "budget": "02-04", "quota_group_id": "anthropic-shared-0104",
            "checkpoint_central": "local_only",
            "share": {"max_runs": 2, "total_seconds": 4_800, "billable_tokens": 500_000,
                      "cache_read_tokens": 5_000_000}}))
        .expect("guardrail config"),
    );
    // the no-extra-billing observation for the first run of the budget (a stored stream)
    let first = dir.join("first-observation.jsonl");
    std::fs::write(&first, "{\"type\":\"rate_limit_event\",\"rate_limit_info\":{\"status\":\"allowed\",\"overageStatus\":\"rejected\",\"isUsingOverage\":false}}\n").expect("obs");
    let ledger = dir.join("ledger");
    let roles_json = |max_runs: u64, plan_roles: &[&str]| {
        let mut plan = json!({"development": {"max_window_seconds": 120,
                    "share": {"max_runs": max_runs, "total_seconds": 9_600,
                              "billable_tokens": 1_000_000, "cache_read_tokens": 10_000_000}},
                "guardrail": {"max_window_seconds": 120,
                    "share": {"max_runs": 2, "total_seconds": 4_800,
                              "billable_tokens": 500_000, "cache_read_tokens": 5_000_000}}});
        for r in ["development", "guardrail"] {
            if !plan_roles.contains(&r) {
                plan.as_object_mut().expect("o").remove(r);
            }
        }
        json!({"repo-a": {
            "development": {"url": format!("http://127.0.0.1:{}", role.port),
                            "adapter_token_file": dir.join("adapter_token"),
                            "quota_group_id": "anthropic-shared-0104",
                            "image_digest": "sha256:img", "profile_id": "dev-profile",
                            "profile_revision": 1},
            "guardrail": {"url": format!("http://127.0.0.1:{}", guard.port),
                          "adapter_token_file": dir.join("adapter_token"),
                          "image_digest": "sha256:img", "profile_id": "guard-profile",
                          "profile_revision": 1},
            "budget_plan": {"budget": "02-04", "ledger_dir": ledger,
                "first_observation": {"stream": first, "observed_unix": unix_now()},
                "roles": plan}}})
        .to_string()
    };
    std::fs::write(
        dir.join("roles.json"),
        roles_json(8, &["development", "guardrail"]),
    )
    .expect("roles");
    let rootdir = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let mut coord = Coord {
        child: None,
        log: dir.join("coordinator-stderr.log"),
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
    if transport == "grpc" {
        coord.args.push("--grpc-listen".into());
        coord.args.push(format!("127.0.0.1:{grpc_port}"));
    }
    coord.start();
    let c = |who: &str, method: &str, path: &str, body: Option<&Value>| {
        http(port, Some(&tok(who)), None, method, path, body).expect("coordinator answered")
    };
    let mut db = {
        let mut c2 = cfg.clone();
        c2.dbname(&dbname);
        c2.connect(NoTls).expect("db")
    };
    let mut count = |table: &str| -> i64 {
        db.query_one(&format!("SELECT count(*) FROM {table}"), &[])
            .expect("count")
            .get(0)
    };
    let state = root.join("state");
    let runs = || {
        std::fs::read_dir(state.join("runs"))
            .map(|d| d.count())
            .unwrap_or(0)
    };
    let launches = || role.launches.load(std::sync::atomic::Ordering::SeqCst);
    let records = || -> Vec<Value> {
        std::fs::read_to_string(ledger.join("records.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).expect("record"))
            .collect()
    };
    // the delivery path finishes and records an accepted run in the background
    let settled = |attempt: &str| -> Value {
        let t0 = Instant::now();
        loop {
            if let Some(r) = records().into_iter().find(|r| r["attempt_id"] == attempt)
                && !ledger.join("inflight.json").exists()
            {
                return r;
            }
            assert!(
                t0.elapsed() < Duration::from_secs(60),
                "no record of {attempt}"
            );
            std::thread::sleep(Duration::from_millis(100));
        }
    };
    let deliver = |who: &str, key: &str, role_name: &str, body: &Value| {
        http(
            port,
            Some(&tok(who)),
            Some(key),
            "POST",
            &format!("/v0/repos/repo-a/roles/{role_name}/deliveries"),
            Some(body),
        )
        .expect("coordinator answered")
    };
    let dbody = |attempt: &str, generation: u64, session: &str, extra: Value| {
        let mut v = json!({"assignment_id": "A1", "generation": generation,
                           "attempt_id": attempt, "session_id": sid(session), "prompt": "hi"});
        for (k, x) in extra.as_object().expect("obj") {
            v[k] = x.clone();
        }
        v
    };
    let outcome = |b: &Value| {
        (
            b["result"]["decision"].as_str().map(str::to_string),
            b["result"]["op"].as_str().map(str::to_string),
            b["result"]["outcome"].as_str().map(str::to_string),
        )
    };
    let o = |d: &str, op: Option<&str>, out: &str| {
        (
            Some(d.to_string()),
            op.map(str::to_string),
            Some(out.to_string()),
        )
    };

    // 1. authorization, format and configuration refusals leave nothing behind
    let b1 = dbody("a1", 0, "s1", json!({}));
    let p = "/v0/repos/repo-a/roles/development/deliveries";
    assert_eq!(
        http(port, None, Some("dlv-key-0001"), "POST", p, Some(&b1))
            .expect("answer")
            .0,
        401
    );
    for (who, code) in [
        ("assigner", "SCOPE_DENIED"),
        ("sup", "SCOPE_DENIED"),
        ("operator-b", "REPO_SCOPE_DENIED"),
    ] {
        let (st, b) = deliver(who, "dlv-key-0001", "development", &b1);
        assert_eq!((st, b["code"].as_str()), (403, Some(code)), "{who} {b}");
    }
    let (st, _) = deliver("operator", "dlv-key-0001", "reviewer", &b1);
    assert_eq!(st, 404);
    let (st, b) = http(port, Some(&tok("operator")), None, "POST", p, Some(&b1)).expect("a");
    assert_eq!(
        (st, b["rejections"][0]["field"].as_str()),
        (400, Some("Idempotency-Key"))
    );
    for bad in [
        dbody("-a1", 0, "s1", json!({})),
        {
            // a session id that is not a safe id (the helper maps names to UUIDs)
            let mut b = dbody("a1", 0, "s1", json!({}));
            b["session_id"] = json!("s/1");
            b
        },
        dbody("a1", 0, "s1", json!({"coordinator": {}})),
        dbody("a1", 0, "s1", json!({"new_session": "yes"})),
    ] {
        let (st, b) = deliver("operator", "dlv-key-0001", "development", &bad);
        assert_eq!(st, 400, "{bad} {b}");
    }
    // no current assignment yet
    let (st, b) = deliver("operator", "dlv-key-0001", "development", &b1);
    assert_eq!(
        (st, b["code"].as_str()),
        (404, Some("NO_CURRENT_ASSIGNMENT")),
        "{b}"
    );
    assert_eq!(count("role_deliveries"), 0);

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
    let assign = |role_name: &str, generation: u64, expected: Value| {
        c(
            "assigner",
            "POST",
            "/v0/coordinator/assignments",
            Some(&json!({"repo_id": "repo-a",
            "role": role_name, "assignment_id": if role_name == "guardrail" { "G1" } else { "A1" },
            "generation": generation, "sprint_id": "POC-01",
            "sprint_revision": 5, "task_id": "01-01", "target_sha": SHA1, "expected_current": expected,
            "branch_ref": if role_name == "guardrail" { Value::Null } else { json!(BRANCH) }})),
        )
    };
    // GitHub records come from the evidence service (02-05 step 7): trusted input, fixed here
    let ingest = |id: &str, kind: &str, extra: Value| {
        let mut b = json!({"evidence_id": id, "repo_id": "repo-a", "kind": kind});
        for (k, x) in extra.as_object().expect("obj") {
            b[k] = x.clone();
        }
        let (st, r) = c("ingest", "POST", "/v0/coordinator/evidence", Some(&b));
        assert_eq!(st, 201, "{r}");
    };
    let head = |id: &str, sha: &str| {
        ingest(
            id,
            "branch_head",
            json!({"sha": sha, "ref": BRANCH, "source_ref": format!("fixture://github/{id}")}),
        )
    };
    let compare = |role_name: &str| {
        let (st, b) = c(
            "operator",
            "GET",
            &format!("/v0/repos/repo-a/roles/{role_name}/checkpoints/compare"),
            None,
        );
        assert_eq!(st, 200, "{b}");
        b
    };
    assert_eq!(assign("development", 0, Value::Null).0, 201);
    head("gh-head-1", SHA1);
    let approve = |who: &str, key: &str, body: &Value| {
        http(
            port,
            Some(&tok(who)),
            Some(key),
            "POST",
            "/v0/repos/repo-a/roles/development/recoveries",
            Some(body),
        )
        .expect("coordinator answered")
    };
    let recovery = |id: &str| {
        let (st, b) = c(
            "operator",
            "GET",
            &format!("/v0/repos/repo-a/roles/development/recoveries/{id}"),
            None,
        );
        assert_eq!(st, 200, "{b}");
        b
    };
    let cp_path = state.join("checkpoint.json");
    let local_cp =
        || -> Value { serde_json::from_slice(&std::fs::read(&cp_path).expect("cp")).expect("cp") };
    let target = |cp: &Value, reason: &str| {
        json!({"assignment_id": "A1", "generation": 0, "checkpoint_seq": cp["seq"],
               "checkpoint_sha256": digest_bytes(&std::fs::read(&cp_path).expect("cp")),
               "reason": reason})
    };
    // a crash: the takeover checkpoint of a run that never reported its end, as the
    // coordinator and the role volume hold it (its delivery, a fixture, was answered)
    // a delivery recorded as the coordinator records one (a fixture), answered `accepted`
    let fixture = |role_name: &str, id: &str, aid: &str, profile: &str| {
        let mut c2 = cfg.clone();
        c2.dbname(&dbname);
        let mut d = c2.connect(NoTls).expect("db");
        d.execute(
            "INSERT INTO role_deliveries (delivery_id, repo_id, role, request_id, body,
                 assignment_id, generation, attempt_id, session_id, logical_name,
                 coordinator_record, target_sha, requested_by)
             VALUES ($1, 'repo-a', $4, $1, '{}'::jsonb, $5, 0, $1, 'f', 'f', $3, $2,
                     'test-fixture')",
            &[
                &id,
                &SHA1,
                &json!({"image_digest": "sha256:img", "profile_id": profile,
                        "profile_revision": 1}),
                &role_name,
                &aid,
            ],
        )
        .expect("fixture delivery");
        d.execute(
            "INSERT INTO role_delivery_results (delivery_id, n, decision, reasons, op, outcome)
             VALUES ($1, 1, 'resume_allowed', '[]'::jsonb, 'reconnect', 'accepted')",
            &[&id],
        )
        .expect("fixture result");
        // the person's reconciliation of the crashed run's budget record (as in 02-04): without
        // it the assignment stays stopped as unmeasured, recovery or not
        {
            use std::io::Write as _;
            let mut f = std::fs::OpenOptions::new()
                .append(true)
                .open(ledger.join("records.jsonl"))
                .expect("ledger");
            writeln!(
                f,
                "{}",
                json!({"format": "ieo-role-run/v0", "attempt_id": id, "role": role_name,
                       "model_request": false, "reason": "reconciled after the crash fixture"})
            )
            .expect("entry");
        }
    };
    // a takeover reported over the local checkpoint (a crash: the run never reported its end)
    let takeover = |id: &str, recovery_id: Value| {
        let mut cp = local_cp();
        let seq = cp["seq"].as_u64().expect("seq") + 1;
        cp["seq"] = json!(seq);
        cp["phase"] = json!("takeover");
        cp["delivery_id"] = json!(id);
        cp["attempt_id"] = json!(id);
        cp["first_action"] = Value::Null;
        cp["recovery_id"] = recovery_id;
        let raw = serde_json::to_string_pretty(&cp).expect("j");
        let (st, b) = c(
            "sup",
            "POST",
            "/v0/repos/repo-a/roles/development/checkpoints",
            Some(&json!({"seq": seq, "checkpoint_json": raw})),
        );
        if st != 201 {
            return (st, b);
        }
        std::fs::write(&cp_path, &raw).expect("local");
        std::fs::write(
            state.join("checkpoints").join(format!("{seq:020}.json")),
            &raw,
        )
        .expect("archive");
        (st, b)
    };
    let crash = |name: &str| {
        let id = format!("D-e2e-crash-{name}");
        fixture("development", &id, "A1", "dev-profile");
        let (st, b) = takeover(&id, Value::Null);
        assert_eq!(st, 201, "{b}");
    };

    // 1. a run in progress: its takeover is the latest checkpoint, and a recovery is refused
    let (st, d1) = deliver(
        "operator",
        "rcv-key-0001",
        "development",
        &dbody("a1", 0, "s1", json!({"prompt": "SLEEP"})),
    );
    assert_eq!(
        outcome(&d1),
        o("no_checkpoint", Some("start"), "accepted"),
        "{st} {d1}"
    );
    let t0 = Instant::now();
    while std::fs::read(&cp_path)
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .is_none_or(|c| c["phase"] != "takeover")
    {
        assert!(t0.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(50));
    }
    let running = local_cp();
    let (st, b) = approve(
        "alice-ops",
        "rec-key-0000",
        &target(&running, "the run looks stuck"),
    );
    assert_eq!(
        (st, b["rejections"][0]["code"].as_str()),
        (409, Some("RECOVERY_RUNNING")),
        "{b}"
    );
    role.wait("a1");
    settled("a1");
    assert_eq!(count("role_recoveries"), 0);

    // 2. a crash: nothing resumes or starts over the unfinished checkpoint
    crash("1");
    let cmp = compare("development");
    assert_eq!(cmp["decision"], "external_reconciliation_required", "{cmp}");
    let n = launches();
    let (st, d) = deliver(
        "operator",
        "rcv-key-0002",
        "development",
        &dbody("a2", 0, "s2", json!({"new_session": true})),
    );
    assert_eq!(
        outcome(&d),
        o("external_reconciliation_required", None, "not_called"),
        "{st} {d}"
    );
    assert_eq!(launches(), n);
    // the supervisor itself refuses a start over it without an approved recovery
    let (st, b) = role.call(
        "POST",
        "/v0/role/start",
        &json!({"attempt_id": "x1", "prompt": "hi", "session_id": "sx", "target_sha": SHA1,
                "coordinator": d1["coordinator_record"],
                "limits": {"timeout_ms": 20_000, "billable_room": 100_000,
                           "cache_read_room": 100_000, "grace_ms": 1_000}}),
    );
    assert_eq!(
        (st, b["code"].as_str()),
        (409, Some("EXTERNAL_RECONCILIATION_REQUIRED")),
        "{b}"
    );
    // review 185d12b L-1: a takeover over the unfinished checkpoint is a recovery: without the
    // approval its delivery used, or with another id, the coordinator refuses it
    fixture("development", "D-e2e-l1", "A1", "dev-profile");
    for rid in [Value::Null, json!("R-forged")] {
        let (st, b) = takeover("D-e2e-l1", rid.clone());
        assert_eq!(
            (st, b["rejections"][0]["code"].as_str()),
            (409, Some("RECOVERY_REQUIRED")),
            "{rid} {b}"
        );
    }
    let crashed = local_cp();
    let good = target(
        &crashed,
        "supervisor crash during run a1; branch checked on GitHub",
    );
    let usage_before = c(
        "operator",
        "GET",
        "/v0/coordinator/assignments/A1/generations/0/chain",
        None,
    )
    .1["usage"]
        .clone();

    // 3. who may approve: a person with roles:control for the repo; never a service, an agent
    //    (the role's own credentials) or another repo's operator
    for (who, code) in [
        ("operator", "HUMAN_APPROVAL_REQUIRED"),
        ("sup", "SCOPE_DENIED"),
        ("dev-agent", "SCOPE_DENIED"),
        ("carol", "SCOPE_DENIED"),
        ("bob-ops", "REPO_SCOPE_DENIED"),
    ] {
        let (st, b) = approve(who, "rec-key-0001", &good);
        assert_eq!((st, b["code"].as_str()), (403, Some(code)), "{who} {b}");
    }
    let (st, b) = http(
        port,
        Some(&tok("alice-ops")),
        None,
        "POST",
        "/v0/repos/repo-a/roles/development/recoveries",
        Some(&good),
    )
    .expect("answer");
    assert_eq!(
        (st, b["rejections"][0]["field"].as_str()),
        (400, Some("Idempotency-Key")),
        "{b}"
    );
    // 4. the target must be exactly the latest unfinished checkpoint of the current assignment
    for (k, v) in [
        (
            "checkpoint_seq",
            json!(crashed["seq"].as_u64().expect("seq") - 1),
        ),
        ("checkpoint_sha256", json!("0".repeat(64))),
        ("generation", json!(1)),
        ("assignment_id", json!("A9")),
    ] {
        let mut bad = good.clone();
        bad[k] = v;
        let (st, b) = approve("alice-ops", "rec-key-0001", &bad);
        assert_eq!(
            (st, b["rejections"][0]["code"].as_str()),
            (409, Some("RECOVERY_TARGET_MISMATCH")),
            "{k} {b}"
        );
    }
    assert_eq!(count("role_recoveries"), 0, "refusals record nothing");

    // 5. what cannot be confirmed is recorded as a hold with its evidence: the supervisor not
    //    observed; a GitHub head that does not match (a crashed run that pushed included)
    role.fail_status
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let (st, h1) = approve("alice-ops", "rec-key-0002", &good);
    role.fail_status
        .store(false, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        (st, h1["state"].as_str()),
        (201, Some("needs_confirmation")),
        "{h1}"
    );
    assert!(
        h1["reasons"][0]
            .as_str()
            .is_some_and(|r| r.contains("not observed")),
        "{h1}"
    );
    // the role's local checkpoint is not the central one (review 185d12b L-3, M5)
    let kept_cp = std::fs::read(&cp_path).expect("cp");
    std::fs::write(&cp_path, [kept_cp.clone(), b"\n".to_vec()].concat()).expect("w");
    let (st, h5) = approve("alice-ops", "rec-key-0013", &good);
    std::fs::write(&cp_path, &kept_cp).expect("restore");
    assert_eq!(
        (st, h5["state"].as_str()),
        (201, Some("needs_confirmation")),
        "{h5}"
    );
    assert!(
        h5["reasons"][0]
            .as_str()
            .is_some_and(|r| r.contains("not the central one")),
        "{h5}"
    );
    head("gh-head-2", SHA2);
    let (st, h2) = approve("alice-ops", "rec-key-0003", &good);
    assert_eq!(
        (st, h2["state"].as_str(), h2["github"]["result"].as_str()),
        (201, Some("needs_confirmation"), Some("mismatch")),
        "{h2}"
    );
    assert!(
        h2["github"]["source"]
            .as_str()
            .is_some_and(|x| x.contains("not a live GitHub read")),
        "{h2}"
    );
    head("gh-head-3", SHA1);
    // an external request of the role whose outcome is not known (the supervisor's list)
    std::fs::write(
        state.join("pending-requests.json"),
        json!([{"request_id": "first-action:x", "kind": "first_action_report",
                "sent_at": "2026-11-17T00:00:00Z", "target": "coordinator recordFirstAction",
                "detail": {"body": {}}}])
        .to_string(),
    )
    .expect("pending list");
    let (st, h3) = approve("alice-ops", "rec-key-0010", &good);
    std::fs::remove_file(state.join("pending-requests.json")).expect("rm");
    assert_eq!(
        (st, h3["state"].as_str()),
        (201, Some("needs_confirmation")),
        "{h3}"
    );
    assert!(
        h3["reasons"][0]
            .as_str()
            .is_some_and(|r| r.contains("external requests")),
        "{h3}"
    );
    // a call of the coordinator to this role whose outcome is not known
    {
        let mut c2 = cfg.clone();
        c2.dbname(&dbname);
        let mut d = c2.connect(NoTls).expect("db");
        d.execute(
            "INSERT INTO role_deliveries (delivery_id, repo_id, role, request_id, body,
                 assignment_id, generation, attempt_id, session_id, logical_name,
                 coordinator_record, target_sha, requested_by)
             VALUES ('D-e2e-unknown', 'repo-a', 'development', 'D-e2e-unknown', '{}'::jsonb,
                     'A1', 0, 'D-e2e-unknown', 'f', 'f', '{}'::jsonb, $1, 'test-fixture')",
            &[&SHA1],
        )
        .expect("fixture delivery");
        let (st, h4) = approve("alice-ops", "rec-key-0011", &good);
        assert_eq!(
            (st, h4["state"].as_str()),
            (201, Some("needs_confirmation")),
            "{h4}"
        );
        assert!(
            h4["reasons"][0]
                .as_str()
                .is_some_and(|r| r.contains("no known outcome")),
            "{h4}"
        );
        // its outcome becomes known: the supervisor refused it (nothing ran)
        d.execute(
            "INSERT INTO role_delivery_results (delivery_id, n, decision, reasons, op, outcome)
             VALUES ('D-e2e-unknown', 1, 'resume_allowed', '[]'::jsonb, 'reconnect', 'refused')",
            &[],
        )
        .expect("fixture result");
    }
    // a hold is never used
    let n = launches();
    let (st, d) = deliver(
        "operator",
        "rcv-key-0003",
        "development",
        &dbody("a3", 0, "s3", json!({"recovery_id": h2["recovery_id"]})),
    );
    assert_eq!(st, 201, "{d}");
    assert_eq!(d["result"]["outcome"], "not_called", "{d}");
    assert!(
        d["result"]["reasons"][0]
            .as_str()
            .is_some_and(|r| r.contains("not approved (needs_confirmation)")),
        "{d}"
    );
    assert_eq!(launches(), n);

    // 6. the approval: who, why, when, the target and the GitHub comparison it relied on
    let (st, r) = approve("alice-ops", "rec-key-0004", &good);
    assert_eq!((st, r["state"].as_str()), (201, Some("approved")), "{r}");
    let rid = r["recovery_id"].as_str().expect("id").to_string();
    assert_eq!(
        (
            r["approved_by"].as_str(),
            r["approver_kind"].as_str(),
            r["reason"].as_str(),
            r["assignment_id"].as_str(),
            r["generation"].as_i64(),
            r["checkpoint"]["seq"].as_u64(),
            r["checkpoint"]["phase"].as_str(),
            r["github"]["result"].as_str()
        ),
        (
            Some("alice-ops"),
            Some("user"),
            good["reason"].as_str(),
            Some("A1"),
            Some(0),
            crashed["seq"].as_u64(),
            Some("takeover"),
            Some("match")
        ),
        "{r}"
    );
    assert!(r["approved_at"].is_string() && r["used"].is_null(), "{r}");
    assert_eq!(
        r["checkpoint"]["checkpoint_sha256"], good["checkpoint_sha256"],
        "{r}"
    );
    // the same request again is the same approval; other content under the key is refused
    let (st, again) = approve("alice-ops", "rec-key-0004", &good);
    assert_eq!((st, &again), (200, &r));
    let mut other = good.clone();
    other["reason"] = json!("another reason");
    let (st, b) = approve("alice-ops", "rec-key-0004", &other);
    assert_eq!(
        (st, b["rejections"][0]["code"].as_str()),
        (409, Some("IDEMPOTENCY_KEY_CONFLICT")),
        "{b}"
    );
    assert_eq!(recovery(&rid), r);
    // two identical requests at once: one approval, both answered with it (review 185d12b I-2)
    let mut both: Vec<(u16, Value)> = std::thread::scope(|sc| {
        let a = sc.spawn(|| approve("alice-ops", "rec-key-0030", &good));
        let b = sc.spawn(|| approve("alice-ops", "rec-key-0030", &good));
        vec![a.join().expect("a"), b.join().expect("b")]
    });
    both.sort_by_key(|x| x.0);
    assert_eq!(
        (both[0].0, both[1].0, &both[0].1["recovery_id"]),
        (200, 201, &both[1].1["recovery_id"]),
        "{both:?}"
    );
    // an approval changes no usage: the assignment's totals are as before
    let usage_after = c(
        "operator",
        "GET",
        "/v0/coordinator/assignments/A1/generations/0/chain",
        None,
    )
    .1["usage"]
        .clone();
    assert_eq!(usage_after, usage_before);

    // 7. the head changed after the approval: checked again at use, nothing called, unused
    head("gh-head-4", SHA2);
    let (st, d) = deliver(
        "operator",
        "rcv-key-0004",
        "development",
        &dbody("a4", 0, "s4", json!({"recovery_id": rid})),
    );
    assert_eq!(st, 201, "{d}");
    assert_eq!(d["result"]["outcome"], "not_called", "{d}");
    assert!(
        d["result"]["reasons"][0]
            .as_str()
            .is_some_and(|r| r.contains("re-checked at use") && r.contains("mismatch")),
        "{d}"
    );
    assert!(recovery(&rid)["used"].is_null());
    head("gh-head-5", SHA1);
    // 8. the budget check still applies: an unmeasured run in the ledger refuses the call, and
    //    the approval stays unused (an approval is neither a budget reset nor a call approval)
    let kept = std::fs::read(ledger.join("records.jsonl")).expect("ledger");
    let mut unmeasured = records()
        .into_iter()
        .rev()
        .find(|r| r["model_request"] != false)
        .expect("a measured run record");
    unmeasured["attempt_id"] = json!("u1");
    unmeasured["usage"]["run"] = json!({"state": "unmeasured", "reason": "test"});
    unmeasured["stop_required"] = json!(false);
    std::fs::write(
        ledger.join("records.jsonl"),
        [kept.clone(), format!("{unmeasured}\n").into_bytes()].concat(),
    )
    .expect("w");
    let (st, d) = deliver(
        "operator",
        "rcv-key-0005",
        "development",
        &dbody("a5", 0, "s5", json!({"recovery_id": rid})),
    );
    assert_eq!(
        outcome(&d),
        o("recovery_allowed", Some("start"), "budget_refused"),
        "{st} {d}"
    );
    assert!(recovery(&rid)["used"].is_null());
    std::fs::write(ledger.join("records.jsonl"), &kept).expect("restore");

    // 8b. review 185d12b M-1: a use whose delivery ran nothing (the supervisor refused the
    //     start: its attempt id was taken) does not consume the checkpoint; a new approval is
    //     `approved` and the next use runs once
    let (st, rx) = approve("alice-ops", "rec-key-0014", &good);
    assert_eq!((st, rx["state"].as_str()), (201, Some("approved")), "{rx}");
    let ridx = rx["recovery_id"].as_str().expect("id").to_string();
    std::fs::create_dir_all(state.join("runs/a6x")).expect("taken attempt");
    let n = runs();
    let (st, d) = deliver(
        "operator",
        "rcv-key-0016",
        "development",
        &dbody("a6x", 0, "s6x", json!({"recovery_id": ridx})),
    );
    assert_eq!(
        outcome(&d),
        o("recovery_allowed", Some("start"), "refused"),
        "{st} {d}"
    );
    assert_eq!(recovery(&ridx)["used"]["delivery_id"], d["delivery_id"]);
    assert_eq!(runs(), n, "nothing ran");
    let (st, ry) = approve("alice-ops", "rec-key-0015", &good);
    assert_eq!((st, ry["state"].as_str()), (201, Some("approved")), "{ry}");
    let earlier = &ry["facts"]["earlier_uses"][0];
    assert_eq!(
        (
            earlier["recovery_id"].as_str(),
            earlier["outcome"].as_str(),
            earlier["consumes"].as_bool()
        ),
        (Some(ridx.as_str()), Some("refused"), Some(false)),
        "{ry}"
    );
    // 8c. review 185d12b L-3 (M3): the approval is checked again once the budget's lock is
    //     held; the head changes while the delivery waits for it → nothing called, unused
    {
        let held = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(ledger.join("caller.lock"))
            .expect("lock file");
        held.lock().expect("hold the ledger lock");
        let bm = dbody("a6m", 0, "s6m", json!({"recovery_id": rid}));
        let (st, dm) = std::thread::scope(|sc| {
            let h = sc.spawn(|| deliver("operator", "rcv-key-0017", "development", &bm));
            std::thread::sleep(Duration::from_millis(600));
            head("gh-head-6", SHA2);
            held.unlock().expect("release");
            h.join().expect("delivery thread")
        });
        assert_eq!(st, 201, "{dm}");
        assert_eq!(dm["result"]["outcome"], "not_called", "{dm}");
        assert!(
            dm["result"]["reasons"][0]
                .as_str()
                .is_some_and(|r| r.contains("judgment changed")),
            "{dm}"
        );
        assert!(recovery(&rid)["used"].is_null());
        head("gh-head-7", SHA1);
    }

    // A final approval-fence refusal does not consume the recovery. The next
    // delivery below uses this exact approval after the fixture restores review.
    {
        let mut fixture = cfg.clone();
        fixture.dbname(&dbname);
        let mut fixture = fixture.connect(NoTls).unwrap();
        let changes = |fixture: &mut postgres::Client, state: &str| {
            assert_eq!(fixture.execute(
                "UPDATE sprint_revisions SET review_state=$1 WHERE sprint_id='POC-01' AND revision=5",
                &[&state],
            ).unwrap(), 1);
        };
        changes(&mut fixture, "semantic_review_rejected");
        let before = launches();
        let (st, refused) = deliver(
            "operator",
            "rcv-final-fence",
            "development",
            &dbody("afence", 0, "sfence", json!({"recovery_id": rid})),
        );
        assert_eq!(
            (st, refused["result"]["outcome"].as_str()),
            (201, Some("not_called")),
            "{refused}"
        );
        assert_eq!(launches(), before);
        assert!(recovery(&rid)["used"].is_null());
        changes(&mut fixture, "semantic_review_passed");
    }

    // 9. the use: a new session over the unfinished checkpoint, never a resume
    let b6 = dbody("a6", 0, "s6", json!({"recovery_id": rid}));
    let (st, d6) = deliver("operator", "rcv-key-0006", "development", &b6);
    assert_eq!(
        outcome(&d6),
        o("recovery_allowed", Some("start"), "accepted"),
        "{st} {d6}"
    );
    assert_eq!(
        (
            d6["result"]["supervisor"]["body"]["accepted"]["session_id"].as_str(),
            d6["result"]["supervisor"]["body"]["accepted"]["mode"].as_str()
        ),
        (Some(sid("s6").as_str()), Some("new")),
        "{d6}"
    );
    assert_eq!(d6["recovery"]["recovery_id"], rid.as_str(), "{d6}");
    assert_eq!(d6["recovery"]["approved_by"], "alice-ops", "{d6}");
    // the head was re-compared at use; the approval's head is beside it
    assert_eq!(d6["result"]["github"]["result"], "match", "{d6}");
    assert_eq!(
        d6["result"]["github"]["head_at_approval"]["evidence_id"], "gh-head-3",
        "{d6}"
    );
    assert_eq!(d6["result"]["github"]["head"]["evidence_id"], "gh-head-7");
    assert_eq!(
        role.last_launch.lock().expect("last")["recovery_id"],
        rid.as_str()
    );
    role.wait("a6");
    settled("a6");
    let after = local_cp();
    assert_eq!(
        (
            after["phase"].as_str(),
            after["session_id"].as_str(),
            after["recovery_id"].as_str()
        ),
        (
            Some("after_run"),
            Some(sid("s6").as_str()),
            Some(rid.as_str())
        ),
        "{after}"
    );
    // the crashed checkpoint was consumed by an accepted use: it is no longer the latest, a new
    // approval of it is refused
    let (st, b) = approve("alice-ops", "rec-key-0018", &good);
    assert_eq!(
        (st, b["rejections"][0]["code"].as_str()),
        (409, Some("RECOVERY_TARGET_MISMATCH")),
        "{b}"
    );
    // a finished run is nothing to recover
    let (st, b) = approve(
        "alice-ops",
        "rec-key-0012",
        &target(&after, "nothing to recover"),
    );
    assert_eq!(
        (st, b["rejections"][0]["code"].as_str()),
        (409, Some("RECOVERY_TARGET_MISMATCH")),
        "{b}"
    );
    assert!(
        b["rejections"][0]["message"]
            .as_str()
            .is_some_and(|m| m.contains("its run finished")),
        "{b}"
    );
    let used = recovery(&rid)["used"].clone();
    assert_eq!(used["delivery_id"], d6["delivery_id"], "{used}");
    // the chain shows the approval, its use and the delivery that used it; the crashed
    // checkpoint is kept (append-only)
    let ch = c(
        "operator",
        "GET",
        "/v0/coordinator/assignments/A1/generations/0/chain",
        None,
    )
    .1;
    let recs: Vec<&str> = ch["recoveries"]
        .as_array()
        .unwrap_or_else(|| panic!("recoveries: {ch}"))
        .iter()
        .filter_map(|x| x["recovery_id"].as_str())
        .collect();
    assert!(recs.contains(&rid.as_str()), "{ch}");
    assert!(
        ch["deliveries"]
            .as_array()
            .expect("deliveries")
            .iter()
            .any(|x| x["delivery_id"] == d6["delivery_id"]
                && x["recovery"]["recovery_id"] == rid.as_str()),
        "{ch}"
    );
    assert!(
        ch["checkpoints"]
            .as_array()
            .expect("checkpoints")
            .iter()
            .any(|x| x["seq"] == crashed["seq"] && x["phase"] == "takeover"),
        "{ch}"
    );

    // 10. one approval, one run: the same request again is the stored delivery; another
    //     delivery with the approval calls nothing
    let n = (launches(), runs());
    let (st, again) = deliver("operator", "rcv-key-0006", "development", &b6);
    assert_eq!((st, &again), (200, &d6));
    let (st, d) = deliver(
        "operator",
        "rcv-key-0007",
        "development",
        &dbody("a7", 0, "s7", json!({"recovery_id": rid})),
    );
    assert_eq!(st, 201, "{d}");
    assert_eq!(d["result"]["outcome"], "not_called", "{d}");
    assert!(
        d["result"]["reasons"][0]
            .as_str()
            .is_some_and(|r| r.contains("already used")),
        "{d}"
    );
    assert_eq!((launches(), runs()), n);

    // 11. a lost answer and restarts: the use is recorded before the call, the re-query never
    //     starts anything, and after restarts of the coordinator and the supervisor the use row
    //     stays (a second use cannot be tried here: the lost answer's unmeasured run refuses the
    //     assignment's next delivery first; review 185d12b I-3)
    crash("2");
    let crashed2 = local_cp();
    let (st, r2) = approve(
        "alice-ops",
        "rec-key-0005",
        &target(&crashed2, "second crash"),
    );
    assert_eq!((st, r2["state"].as_str()), (201, Some("approved")), "{r2}");
    let rid2 = r2["recovery_id"].as_str().expect("id").to_string();
    role.drop_next
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let b8 = dbody("a8", 0, "s8", json!({"recovery_id": rid2}));
    let (st, d8) = deliver("operator", "rcv-key-0008", "development", &b8);
    assert_eq!(
        outcome(&d8),
        o("recovery_allowed", Some("start"), "unknown"),
        "{st} {d8}"
    );
    role.wait("a8");
    assert_eq!(recovery(&rid2)["used"]["delivery_id"], d8["delivery_id"]);
    let n = (launches(), runs());
    let (st, r8) = deliver("operator", "rcv-key-0008", "development", &b8);
    assert_eq!(st, 200, "{r8}");
    assert_eq!(r8["result"]["outcome"], "accepted", "{r8}");
    assert_eq!(r8["result"]["requeried"], true, "{r8}");
    // SIGTERM ends the coordinator by itself (exit 0) even with the supervisor's gRPC connection
    // open: the gRPC side waits only for calls in progress (stop_grace), not for idle
    // connections (02-05 pre-step-8c; before, the stop waited until SIGKILL)
    let t0 = Instant::now();
    assert!(coord.stop(), "the coordinator exits by itself on SIGTERM");
    assert!(
        t0.elapsed()
            < ieo_coord::grpc::stop_grace(Duration::from_secs(15)) + Duration::from_secs(5),
        "within the gRPC stop grace"
    );
    coord.start();
    {
        // the role process restarts on the same volume
        let fresh = Arc::new(Service::new(role_cfg()));
        *role._svc.lock().expect("svc") = fresh;
    }
    let (st, again) = deliver("operator", "rcv-key-0008", "development", &b8);
    assert_eq!((st, &again), (200, &r8));
    let (st, d) = deliver(
        "operator",
        "rcv-key-0009",
        "development",
        &dbody("a9", 0, "s9", json!({"recovery_id": rid2})),
    );
    // the lost answer left an unmeasured run that asks for a person: the assignment's next
    // delivery is refused before anything is recorded, approval or not (usage is never reset)
    assert_eq!(
        (st, d["rejections"][0]["code"].as_str()),
        (409, Some("ASSIGNMENT_USAGE_UNMEASURED")),
        "{d}"
    );
    assert_eq!(recovery(&rid2)["used"]["delivery_id"], d8["delivery_id"]);
    assert_eq!((launches(), runs()), n, "nothing ran twice");
    // every request and result is kept; three approvals were used (one ran nothing)
    assert_eq!(count("role_recovery_uses"), 3);

    // 12. review 185d12b M-1: a use whose delivery may have run (here `accepted`, its run never
    //     reported a takeover) consumes the checkpoint: a later approval of it is a hold that
    //     names that delivery
    crash("3");
    let crashed3 = local_cp();
    let (st, rz) = approve(
        "alice-ops",
        "rec-key-0019",
        &target(&crashed3, "third crash"),
    );
    assert_eq!((st, rz["state"].as_str()), (201, Some("approved")), "{rz}");
    fixture("development", "D-e2e-acc", "A1", "dev-profile");
    {
        let mut c2 = cfg.clone();
        c2.dbname(&dbname);
        c2.connect(NoTls)
            .expect("db")
            .execute(
                "INSERT INTO role_recovery_uses (recovery_id, delivery_id, repo_id, role,
                     checkpoint_seq)
                 VALUES ($1, 'D-e2e-acc', 'repo-a', 'development', $2)",
                &[
                    &rz["recovery_id"].as_str().expect("id"),
                    &crashed3["seq"].as_i64().expect("seq"),
                ],
            )
            .expect("fixture use");
    }
    let (st, h6) = approve("alice-ops", "rec-key-0020", &target(&crashed3, "again"));
    assert_eq!(
        (st, h6["state"].as_str()),
        (201, Some("needs_confirmation")),
        "{h6}"
    );
    assert!(
        h6["reasons"].to_string().contains("D-e2e-acc")
            && h6["reasons"].to_string().contains("outcome accepted"),
        "{h6}"
    );

    // 12b. review d336849 L-b: the run ends in an `error` checkpoint; a takeover over it is a
    //      recovery as well
    {
        let mut cp = local_cp();
        let seq = cp["seq"].as_u64().expect("seq") + 1;
        cp["seq"] = json!(seq);
        cp["phase"] = json!("error");
        let raw = serde_json::to_string_pretty(&cp).expect("j");
        let (st, b) = c(
            "sup",
            "POST",
            "/v0/repos/repo-a/roles/development/checkpoints",
            Some(&json!({"seq": seq, "checkpoint_json": raw})),
        );
        assert_eq!(st, 201, "{b}");
        std::fs::write(&cp_path, &raw).expect("local");
        std::fs::write(
            state.join("checkpoints").join(format!("{seq:020}.json")),
            &raw,
        )
        .expect("archive");
    }
    fixture("development", "D-e2e-err", "A1", "dev-profile");
    let (st, b) = takeover("D-e2e-err", Value::Null);
    assert_eq!(
        (st, b["rejections"][0]["code"].as_str()),
        (409, Some("RECOVERY_REQUIRED")),
        "{b}"
    );
    // 12c. review d336849 L-a: a delivery whose use was recorded for another checkpoint (the
    //      first crash's, approval `ry`) does not cover this one: refused
    let ry_id = ry["recovery_id"].as_str().expect("id").to_string();
    fixture("development", "D-e2e-bind", "A1", "dev-profile");
    {
        let mut c2 = cfg.clone();
        c2.dbname(&dbname);
        c2.connect(NoTls)
            .expect("db")
            .execute(
                "INSERT INTO role_recovery_uses (recovery_id, delivery_id, repo_id, role,
                     checkpoint_seq)
                 VALUES ($1, 'D-e2e-bind', 'repo-a', 'development', $2)",
                &[&ry_id, &crashed["seq"].as_i64().expect("seq")],
            )
            .expect("fixture use");
    }
    let (st, b) = takeover("D-e2e-bind", json!(ry_id));
    assert_eq!(
        (st, b["rejections"][0]["code"].as_str()),
        (409, Some("RECOVERY_REQUIRED")),
        "{b}"
    );

    // 12d. review d336849 L-a: the approval must be of the reported assignment and generation:
    //      an approval of A1/0 does not cover a takeover under the new generation A1/1
    let errcp = local_cp();
    let (st, re) = approve(
        "alice-ops",
        "rec-key-0023",
        &target(&errcp, "the run ended in an error"),
    );
    assert_eq!((st, re["state"].as_str()), (201, Some("approved")), "{re}");
    assert_eq!(
        assign(
            "development",
            1,
            json!({"assignment_id": "A1", "generation": 0})
        )
        .0,
        201
    );
    {
        let mut c2 = cfg.clone();
        c2.dbname(&dbname);
        let mut d = c2.connect(NoTls).expect("db");
        d.execute(
            "INSERT INTO role_deliveries (delivery_id, repo_id, role, request_id, body,
                 assignment_id, generation, attempt_id, session_id, logical_name,
                 coordinator_record, target_sha, requested_by)
             VALUES ('D-e2e-gen', 'repo-a', 'development', 'D-e2e-gen', '{}'::jsonb, 'A1', 1,
                     'D-e2e-gen', 'f', 'f', $2, $1, 'test-fixture')",
            &[
                &SHA1,
                &json!({"image_digest": "sha256:img", "profile_id": "dev-profile",
                        "profile_revision": 1}),
            ],
        )
        .expect("fixture delivery");
        d.execute(
            "INSERT INTO role_delivery_results (delivery_id, n, decision, reasons, op, outcome)
             VALUES ('D-e2e-gen', 1, 'recovery_allowed', '[]'::jsonb, 'start', 'accepted')",
            &[],
        )
        .expect("fixture result");
        d.execute(
            "INSERT INTO role_recovery_uses (recovery_id, delivery_id, repo_id, role,
                 checkpoint_seq)
             VALUES ($1, 'D-e2e-gen', 'repo-a', 'development', $2)",
            &[
                &re["recovery_id"].as_str().expect("id"),
                &errcp["seq"].as_i64().expect("seq"),
            ],
        )
        .expect("fixture use");
    }
    {
        let mut cp = errcp.clone();
        let seq = cp["seq"].as_u64().expect("seq") + 1;
        cp["seq"] = json!(seq);
        cp["phase"] = json!("takeover");
        cp["generation"] = json!(1);
        cp["session_id"] = json!("s-gen1");
        cp["delivery_id"] = json!("D-e2e-gen");
        cp["attempt_id"] = json!("D-e2e-gen");
        cp["first_action"] = Value::Null;
        cp["recovery_id"] = re["recovery_id"].clone();
        let raw = serde_json::to_string_pretty(&cp).expect("j");
        let (st, b) = c(
            "sup",
            "POST",
            "/v0/repos/repo-a/roles/development/checkpoints",
            Some(&json!({"seq": seq, "checkpoint_json": raw})),
        );
        assert_eq!(
            (st, b["rejections"][0]["code"].as_str()),
            (409, Some("RECOVERY_REQUIRED")),
            "{b}"
        );
    }

    // 13. guardrail (review 185d12b L-3, M2): the same approval for the review role, judged on
    //     its review target instead of a GitHub head; a stale target is refused
    assert_eq!(assign("guardrail", 0, Value::Null).0, 201);
    fixture("guardrail", "D-e2e-gr", "G1", "guard-profile");
    let mut gcp = local_cp();
    for (k, v) in [
        ("role", json!("guardrail")),
        ("assignment_id", json!("G1")),
        ("generation", json!(0)),
        ("seq", json!(1)),
        ("phase", json!("takeover")),
        ("session_id", json!("gs1")),
        ("profile_id", json!("guard-profile")),
        ("delivery_id", json!("D-e2e-gr")),
        ("attempt_id", json!("D-e2e-gr")),
        ("recovery_id", Value::Null),
        ("branch_ref", Value::Null),
        ("pushed_sha", Value::Null),
        ("first_action", Value::Null),
    ] {
        gcp[k] = v;
    }
    let graw = serde_json::to_string_pretty(&gcp).expect("j");
    let (st, b) = c(
        "sup-gr",
        "POST",
        "/v0/repos/repo-a/roles/guardrail/checkpoints",
        Some(&json!({"seq": 1, "checkpoint_json": graw})),
    );
    assert_eq!(st, 201, "{b}");
    std::fs::create_dir_all(groot.join("state")).expect("state");
    std::fs::write(groot.join("state/checkpoint.json"), &graw).expect("local");
    let gapprove = |key: &str, generation: u64| {
        http(
            port,
            Some(&tok("alice-ops")),
            Some(key),
            "POST",
            "/v0/repos/repo-a/roles/guardrail/recoveries",
            Some(&json!({"assignment_id": "G1", "generation": generation,
                         "checkpoint_seq": 1, "checkpoint_sha256": digest_bytes(graw.as_bytes()),
                         "reason": "guardrail crash"})),
        )
        .expect("answer")
    };
    let (st, g1) = gapprove("rec-key-0021", 0);
    assert_eq!(
        (st, g1["state"].as_str(), g1["github"].is_null()),
        (201, Some("approved"), true),
        "{g1}"
    );
    let (st, b) = c(
        "assigner",
        "POST",
        "/v0/coordinator/assignments",
        Some(
            &json!({"repo_id": "repo-a", "role": "guardrail", "assignment_id": "G1",
            "generation": 1, "sprint_id": "POC-01", "sprint_revision": 5, "task_id": "01-01",
            "target_sha": SHA2, "expected_current": {"assignment_id": "G1", "generation": 0}}),
        ),
    );
    assert_eq!(st, 201, "{b}");
    let (st, b) = gapprove("rec-key-0022", 1);
    assert_eq!(
        (st, b["rejections"][0]["code"].as_str()),
        (409, Some("RECOVERY_TARGET_MISMATCH")),
        "{b}"
    );
    assert!(
        b["rejections"][0]["message"]
            .as_str()
            .is_some_and(|m| m.contains("new review assignment")),
        "{b}"
    );
    drop(db);
    coord.stop();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = admin.batch_execute(&format!("DROP DATABASE IF EXISTS {dbname} WITH (FORCE)"));
}

fn digest_bytes(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}
