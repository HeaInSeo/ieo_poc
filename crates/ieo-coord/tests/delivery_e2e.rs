//! 02-05 step 3 end to end: the coordinator binary delivers the current assignment to the role
//! supervisor service (`ieo-supervisor`, fake `claude`, no model call) on a real PostgreSQL
//! (`IEO_TEST_DB_URL`): the request recorded, the resume judgment, the shared budget check (the
//! 02-04 caller's code, ledger outside the role), the supervisor's start or reconnect and its
//! own check against the current assignment, the answer or `unknown` recorded, and the re-query.
//! Skipped without `IEO_TEST_DB_URL` outside CI.

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
const SHA_P: &str = "3333333333333333333333333333333333333333";
const SHA_F: &str = "4444444444444444444444444444444444444444";
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
        while http(self.port, None, None, "GET", "/v0/health", None).map(|r| r.0) != Some(200) {
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
fn deliveries_go_through_compare_budget_and_the_supervisor() {
    e2e("http");
}

/// The supervisor reaches the coordinator over gRPC (its own-assignment check included); the
/// delivery itself is HTTP only.
#[test]
fn deliveries_go_through_compare_budget_and_the_supervisor_over_grpc() {
    e2e("grpc");
}

/// A free port **below the ephemeral range** (02-05 pre-step-8c): a port taken by binding port 0
/// is an ephemeral port, and between this check and the coordinator's own bind the system may
/// hand it out as the local port of an outgoing connection (to PostgreSQL, to a role service),
/// so the coordinator cannot bind it and never answers (main CI 44003424073: "the coordinator
/// did not start" at its first start). Ports from 20000-32767 are never given to outgoing
/// connections; the pick is spread by process and call and checked by binding.
fn free_port() -> u16 {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    for _ in 0..1000 {
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let p = 20_000 + (std::process::id().wrapping_mul(7919).wrapping_add(n * 131)) % 12_768;
        let p = u16::try_from(p).expect("port");
        if std::net::TcpListener::bind(("127.0.0.1", p)).is_ok() {
            return p;
        }
    }
    panic!("no free port below the ephemeral range");
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
    let dbname = format!("ieo_t_dlv_{transport}_{}", std::process::id());
    let mut admin = cfg.connect(NoTls).expect("connect");
    let _ = admin.batch_execute(&format!("DROP DATABASE IF EXISTS {dbname} WITH (FORCE)"));
    admin
        .batch_execute(&format!("CREATE DATABASE {dbname}"))
        .expect("create db");
    let dir = std::env::temp_dir().join(format!("ieo-dlv-e2e-{transport}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    let tok = |n: &str| format!("dlv-{n}-{}", std::process::id());
    let principals = json!({"principals": [
        {"name": "alice", "token_sha256": digest(&tok("alice")), "kind": "user", "scopes": ["sprints:register", "sprints:read"], "repo_ids": ["repo-a"]},
        {"name": "bob", "token_sha256": digest(&tok("bob")), "kind": "user", "scopes": ["review:semantic", "sprints:read"], "repo_ids": ["repo-a"]},
        {"name": "assigner", "token_sha256": digest(&tok("assigner")), "kind": "service", "scopes": ["coordinator:assign"], "repo_ids": ["repo-a"]},
        {"name": "operator", "token_sha256": digest(&tok("operator")), "kind": "service", "scopes": ["roles:control"], "repo_ids": ["repo-a"]},
        {"name": "operator-b", "token_sha256": digest(&tok("operator-b")), "kind": "service", "scopes": ["roles:control"], "repo_ids": ["repo-b"]},
        {"name": "sup-dev", "token_sha256": digest(&tok("sup")), "kind": "agent", "scopes": ["checkpoint:report"], "repo_ids": ["repo-a"], "role": "development"},
        {"name": "human-op", "token_sha256": digest(&tok("human-op")), "kind": "user", "scopes": ["roles:control"], "repo_ids": ["repo-a"]},
        {"name": "ingest", "token_sha256": digest(&tok("ingest")), "kind": "service", "scopes": ["evidence:ingest"], "repo_ids": ["repo-a"]},
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
    let role = Role::new(
        Config::from_json(&json!({"role": "development", "repo": "repo-a",
            "state": root.join("state"), "etc": root.join("etc"), "cred": root.join("cred"),
            "claude": claude, "budget": "02-04", "quota_group_id": "anthropic-shared-0104",
            "checkpoint_central": {"url": format!("http://127.0.0.1:{central_port}"),
                                   "token_file": root.join("cred/coordinator_token"),
                                   "transport": transport},
            "share": {"max_runs": 12, "total_seconds": 9_600, "billable_tokens": 1_000_000,
                      "cache_read_tokens": 10_000_000}}))
        .expect("role config"),
    );
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
        roles_json(4, &["development", "guardrail"]),
    )
    .expect("roles");
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
    // another generation than the head: refused, nothing recorded
    let (st, b) = deliver(
        "operator",
        "dlv-key-0001",
        "development",
        &dbody("a1", 1, "s1", json!({})),
    );
    assert_eq!(
        (st, b["rejections"][0]["code"].as_str()),
        (409, Some("STALE_ASSIGNMENT")),
        "{b}"
    );
    assert_eq!((count("role_deliveries"), launches()), (0, 0));

    // 2. no checkpoint → start; the coordinator record and limits come from the database
    let (st, d1) = deliver("operator", "dlv-key-0001", "development", &b1);
    assert_eq!(st, 201, "{d1}");
    assert_eq!(
        outcome(&d1),
        o("no_checkpoint", Some("start"), "accepted"),
        "{d1}"
    );
    assert_eq!(d1["evidence_level"], "verified");
    assert_eq!(d1["logical_name"], "repo-a-development");
    assert_eq!(d1["target_sha"], SHA1);
    assert_eq!(
        d1["coordinator_record"],
        json!({"generation": 0, "assignment_id": "A1", "sprint_id": "POC-01",
               "sprint_revision": 5, "spec_digest": sd, "image_digest": "sha256:img",
               "profile_id": "dev-profile", "profile_revision": 1, "task_id": "01-01",
               "branch_ref": BRANCH,
               "limits": {"elapsed_seconds": 3600, "total_tokens": 100000}}),
        "{d1}"
    );
    // the revision's total_tokens bounds the run's billable room; the plan's window bounds time
    assert_eq!(d1["result"]["limits"]["billable_room"], 100_000, "{d1}");
    assert_eq!(d1["result"]["limits"]["timeout_ms"], 114_000, "{d1}");
    assert_eq!(d1["result"]["supervisor"]["http_status"], 202);
    let accepted = &d1["result"]["supervisor"]["body"]["accepted"];
    assert_eq!(
        (accepted["session_id"].as_str(), accepted["mode"].as_str()),
        (Some(sid("s1").as_str()), Some("new"))
    );
    role.wait("a1");
    let r1 = settled("a1");
    assert_eq!(r1["stop_required"], false, "{r1}");
    assert_eq!(r1["usage"]["run"]["state"], "measured", "{r1}");
    let d1_id = d1["delivery_id"].as_str().expect("id").to_string();
    // the supervisor's own assignment names the delivery and the image and profile it expects
    let (st, own) = c(
        "sup",
        "GET",
        "/v0/repos/repo-a/roles/development/assignment",
        None,
    );
    assert_eq!(st, 200, "{own}");
    assert_eq!(own["last_delivery_id"], d1_id.as_str());
    assert_eq!(
        own["expected_runtime"],
        json!({"image_digest": "sha256:img", "profile_id": "dev-profile", "profile_revision": 1})
    );

    // 3. the same request again: the stored delivery, nothing sent; other content → 409; the
    //    same attempt under another key → 409
    let (st, again) = deliver("operator", "dlv-key-0001", "development", &b1);
    assert_eq!(st, 200, "{again}");
    assert_eq!(again, d1);
    let (st, b) = deliver(
        "operator",
        "dlv-key-0001",
        "development",
        &dbody("a1", 0, "s1", json!({"prompt": "other"})),
    );
    assert_eq!(
        (st, b["rejections"][0]["code"].as_str()),
        (409, Some("IDEMPOTENCY_KEY_CONFLICT"))
    );
    let (st, b) = deliver("operator", "dlv-key-0002", "development", &b1);
    assert_eq!(
        (st, b["rejections"][0]["code"].as_str()),
        (409, Some("ATTEMPT_ID_CONFLICT"))
    );
    assert_eq!((launches(), runs(), count("role_deliveries")), (1, 1, 1));

    // 02-05 step 7: without a GitHub record of the assigned branch, nothing resumes
    let cmp = compare("development");
    assert_eq!(cmp["decision"], "not_observed", "{cmp}");
    assert!(
        cmp["reasons"][0]
            .as_str()
            .is_some_and(|r| r.contains("no GitHub record of refs/heads/feat/a1")),
        "{cmp}"
    );
    assert_eq!(cmp["github"]["expected_from"], "target_sha", "{cmp}");
    // the reusable check's own result and the nature of its input (02-05 step 7b reuses it)
    assert_eq!(cmp["github"]["result"], "not_observed", "{cmp}");
    assert!(
        cmp["github"]["source"]
            .as_str()
            .is_some_and(|x| x.contains("not a live GitHub read")),
        "{cmp}"
    );
    // the run's checkpoint names the branch and observed no push: the target SHA is expected
    assert_eq!(
        (
            cmp["github"]["checkpoint_branch_ref"].as_str(),
            cmp["github"]["push_provenance"].as_str()
        ),
        (
            Some(BRANCH),
            Some("no push observed on the branch (target SHA expected)")
        ),
        "{cmp}"
    );
    // review f0e0ca9 M-1 (developer decision, fail closed): an explicit new session chosen from
    // `resume_allowed` (same generation: the same branch and work tree) is GitHub-gated too. No
    // record → not_called with the GitHub decision; a different head → not_called
    let n = (launches(), runs());
    let (st, d) = deliver(
        "operator",
        "dlv-key-0090",
        "development",
        &dbody("a90", 0, "s90", json!({"new_session": true})),
    );
    assert_eq!(st, 201, "{d}");
    assert_eq!(outcome(&d), o("not_observed", None, "not_called"), "{d}");
    assert_eq!(d["result"]["github"]["result"], "not_observed", "{d}");
    head("gh-head-0", SHA2);
    let (st, d) = deliver(
        "operator",
        "dlv-key-0091",
        "development",
        &dbody("a91", 0, "s91", json!({"new_session": true})),
    );
    assert_eq!(st, 201, "{d}");
    assert_eq!(
        outcome(&d),
        o("external_reconciliation_required", None, "not_called"),
        "{d}"
    );
    assert_eq!(d["result"]["github"]["result"], "mismatch", "{d}");
    assert_eq!((launches(), runs()), n, "nothing was called");
    // the evidence service records the branch at the target SHA (nothing pushed yet)
    head("gh-head-1", SHA1);
    let cmp = compare("development");
    assert_eq!(cmp["decision"], "resume_allowed", "{cmp}");
    assert_eq!(
        (
            cmp["github"]["head"]["sha"].as_str(),
            cmp["github"]["matches"].as_bool(),
            cmp["github"]["ci"]["result"].as_str()
        ),
        (Some(SHA1), Some(true), Some("none")),
        "{cmp}"
    );
    assert_eq!(cmp["github"]["result"], "match", "{cmp}");

    // 4. a normal after_run → reconnect with the same session; the logical name is recorded but
    //    is not the resume key (C1). The body's session id is not a UUID: a reconnect resumes the
    //    checkpoint's session, so only a start needs a UUID (PR #83 review 8336044 I-B)
    let mut b2 = dbody("a2", 0, "s9", json!({"logical_name": "renamed"}));
    b2["session_id"] = json!("s9");
    let (st, d2) = deliver("operator", "dlv-key-0002", "development", &b2);
    assert_eq!(st, 201, "{d2}");
    assert_eq!(
        outcome(&d2),
        o("resume_allowed", Some("reconnect"), "accepted"),
        "{d2}"
    );
    let accepted = &d2["result"]["supervisor"]["body"]["accepted"];
    assert_eq!(
        (accepted["session_id"].as_str(), accepted["mode"].as_str()),
        (Some(sid("s1").as_str()), Some("resume")),
        "{d2}"
    );
    assert_eq!(d2["logical_name"], "renamed");
    // 02-05 step 7: the judgment's GitHub comparison is recorded with the result and sent with
    // the reconnect (the head and the CI result at it), with the assignment's branch
    assert_eq!(d2["result"]["github"]["head"]["sha"], SHA1, "{d2}");
    assert_eq!(
        role.last_launch.lock().expect("last")["github"],
        json!({"branch_ref": BRANCH, "head_sha": SHA1, "ci": "none"})
    );
    assert_eq!(role.last_launch.lock().expect("last")["branch_ref"], BRANCH);
    role.wait("a2");
    settled("a2");
    // CI is read at the head SHA only: another SHA's failure is never used
    ingest(
        "ci-other",
        "ci_run",
        json!({"sha": SHA2, "conclusion": "failure", "source_ref": "fixture://ci/other"}),
    );
    assert_eq!(
        compare("development")["github"]["ci"],
        json!({"sha": SHA1, "result": "none", "evidence": []})
    );
    ingest(
        "ci-head",
        "ci_run",
        json!({"sha": SHA1, "conclusion": "success", "source_ref": "fixture://ci/head"}),
    );

    // 5. the supervisor checks a start against the coordinator's current assignment and the
    //    newest delivery's image and profile: nothing runs on a mismatch
    let direct = |attempt: &str, generation: u64, change: (&str, Value)| {
        let mut coordr = json!({"generation": generation, "assignment_id": "A1", "sprint_id": "POC-01",
            "sprint_revision": 5, "spec_digest": sd, "image_digest": "sha256:img",
            "profile_id": "dev-profile", "profile_revision": 1});
        coordr[change.0] = change.1;
        role.call(
            "POST",
            "/v0/role/start",
            &json!({"attempt_id": attempt, "prompt": "hi",
            "session_id": "sx", "target_sha": SHA1, "coordinator": coordr,
            "limits": {"timeout_ms": 20_000, "billable_room": 100_000, "cache_read_room": 100_000,
                       "grace_ms": 1_000}}),
        )
    };
    for change in [
        ("spec_digest", json!("other")),
        ("sprint_revision", json!(4)),
        ("image_digest", json!("sha256:other")),
        ("profile_revision", json!(2)),
        ("generation", json!(1)),
    ] {
        let (st, b) = direct("x1", 0, change.clone());
        assert_eq!(
            (st, b["code"].as_str()),
            (409, Some("EXTERNAL_RECONCILIATION_REQUIRED")),
            "{change:?} {b}"
        );
    }
    assert_eq!(runs(), 2);

    // 6. compare refusals call nothing: running, external reconciliation, a checkpoint whose
    //    session id was changed under the same name (C1), not observed
    let (st, d3) = deliver(
        "operator",
        "dlv-key-0003",
        "development",
        &dbody(
            "a3",
            0,
            "s9",
            json!({"prompt": "SLEEP", "new_session": true}),
        ),
    );
    // review f0e0ca9 M-1: with a matching head, the explicit new session starts, and its start
    // carries the GitHub comparison too
    assert_eq!(
        outcome(&d3),
        o("resume_allowed", Some("start"), "accepted"),
        "{st} {d3}"
    );
    assert_eq!(
        d3["result"]["supervisor"]["body"]["accepted"]["session_id"],
        sid("s9"),
        "{d3}"
    );
    assert_eq!(
        d3["result"]["github"]["ci"],
        json!({"sha": SHA1, "result": "success", "evidence": ["ci-head"]}),
        "{d3}"
    );
    assert_eq!(
        role.last_launch.lock().expect("last")["github"]["ci"],
        "success"
    );
    std::thread::sleep(Duration::from_millis(300));
    let n = launches();
    let (st, d) = deliver(
        "operator",
        "dlv-key-0004",
        "development",
        &dbody("a4", 0, "s9", json!({})),
    );
    assert_eq!(st, 201, "{d}");
    assert_eq!(outcome(&d), o("already_running", None, "not_called"), "{d}");
    assert!(d["result"]["supervisor"].is_null());
    role.wait("a3");
    settled("a3");
    let cp_path = state.join("checkpoint.json");
    let good = std::fs::read(&cp_path).expect("cp");
    for (i, (k, v)) in [
        ("last_event_offset", json!(999)),
        ("session_id", json!("s-other")),
    ]
    .into_iter()
    .enumerate()
    {
        let mut cp: Value = serde_json::from_slice(&good).expect("cp");
        cp[k] = v;
        std::fs::write(&cp_path, serde_json::to_vec_pretty(&cp).expect("j")).expect("w");
        let (st, d) = deliver(
            "operator",
            &format!("dlv-key-003{i}"),
            "development",
            &dbody(&format!("a3{i}"), 0, "s9", json!({"new_session": true})),
        );
        assert_eq!(st, 201, "{d}");
        assert_eq!(
            outcome(&d),
            o("external_reconciliation_required", None, "not_called"),
            "{k} {d}"
        );
    }
    std::fs::write(&cp_path, &good).expect("restore");
    assert_eq!(assign("guardrail", 0, Value::Null).0, 201);
    guard
        .fail_status
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let (st, d) = deliver(
        "operator",
        "dlv-key-0020",
        "guardrail",
        &json!({"assignment_id": "G1", "generation": 0, "attempt_id": "g1", "session_id": "gs1",
                "prompt": "hi"}),
    );
    assert_eq!(st, 201, "{d}");
    assert_eq!(outcome(&d), o("not_observed", None, "not_called"), "{d}");
    guard
        .fail_status
        .store(false, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(launches(), n, "no refusal called the supervisor");

    // 6b. review c11e734 L-3: the judgment is made again once the budget's lock is held, and a
    //     same-key retry of a delivery still in flight records nothing (409)
    {
        let held = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(ledger.join("caller.lock"))
            .expect("lock file");
        held.lock().expect("hold the ledger lock");
        let b50 = dbody("a50", 0, "s9", json!({}));
        let (st, d50) = std::thread::scope(|sc| {
            let h = sc.spawn(|| deliver("operator", "dlv-key-0050", "development", &b50));
            std::thread::sleep(Duration::from_millis(600));
            let (st, b) = deliver("operator", "dlv-key-0050", "development", &b50);
            assert_eq!(
                (st, b["rejections"][0]["code"].as_str()),
                (409, Some("DELIVERY_IN_PROGRESS")),
                "{b}"
            );
            // the state changes while the delivery waits for the budget's lock
            let mut cp: Value = serde_json::from_slice(&good).expect("cp");
            cp["last_event_offset"] = json!(999);
            std::fs::write(&cp_path, serde_json::to_vec_pretty(&cp).expect("j")).expect("w");
            held.unlock().expect("release");
            h.join().expect("delivery thread")
        });
        assert_eq!(st, 201, "{d50}");
        assert_eq!(
            outcome(&d50),
            o("external_reconciliation_required", None, "not_called"),
            "{d50}"
        );
        assert!(
            d50["result"]["reasons"][0]
                .as_str()
                .is_some_and(|r| r.contains("judgment changed")),
            "{d50}"
        );
        assert_eq!(d50["result"]["n"], 1);
        std::fs::write(&cp_path, &good).expect("restore");
        let (st, again) = deliver("operator", "dlv-key-0050", "development", &b50);
        assert_eq!((st, &again), (200, &d50));
        assert_eq!(launches(), n);
    }

    // 7. a newer generation: an older generation's unfinished run (takeover) is reconciliation
    //    even with an explicit new session (review c11e734 M-1); once it finished (after_run),
    //    stale without an explicit new session, with one a start
    let report = |seq: u64, phase: &str| {
        let mut cp: Value = serde_json::from_slice(&good).expect("cp");
        cp["seq"] = json!(seq);
        cp["phase"] = json!(phase);
        if phase == "takeover" {
            // 02-05 step 4: a takeover names a delivery not taken over yet (a test fixture
            // recorded as deliverAssignment records one, without a result)
            let id = format!("D-e2e-step7-{seq}");
            let mut c2 = cfg.clone();
            c2.dbname(&dbname);
            c2.connect(NoTls)
                .expect("db")
                .execute(
                    "INSERT INTO role_deliveries (delivery_id, repo_id, role, request_id, body,
                         assignment_id, generation, attempt_id, session_id, logical_name,
                         coordinator_record, target_sha, requested_by)
                     VALUES ($1, 'repo-a', 'development', $1, '{}'::jsonb, 'A1', 0, $1, 'f',
                             'f', $3, $2, 'test-fixture')",
                    &[
                        &id,
                        &SHA1,
                        &json!({"image_digest": "sha256:img", "profile_id": "dev-profile",
                                "profile_revision": 1}),
                    ],
                )
                .expect("fixture delivery");
            cp["delivery_id"] = json!(id);
            // the fixture delivery's attempt is its id (review 5dbb3d8 M-1)
            cp["attempt_id"] = json!(id);
        }
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
    };
    let good_seq = serde_json::from_slice::<Value>(&good).expect("cp")["seq"]
        .as_u64()
        .expect("seq");
    report(good_seq + 1, "takeover");
    assert_eq!(
        assign(
            "development",
            1,
            json!({"assignment_id": "A1", "generation": 0})
        )
        .0,
        201
    );
    let (st, d) = deliver(
        "operator",
        "dlv-key-0040",
        "development",
        &dbody("a40", 1, "s2", json!({"new_session": true})),
    );
    assert_eq!(
        outcome(&d),
        o("external_reconciliation_required", None, "not_called"),
        "{st} {d}"
    );
    assert!(
        d["result"]["reasons"][0]
            .as_str()
            .is_some_and(|r| r.contains("did not finish")),
        "{d}"
    );
    report(good_seq + 2, "after_run");
    let (st, d) = deliver(
        "operator",
        "dlv-key-0005",
        "development",
        &dbody("a5", 1, "s2", json!({})),
    );
    assert_eq!(
        outcome(&d),
        o("stale_generation", None, "not_called"),
        "{st} {d}"
    );
    assert_eq!(launches(), n);
    // the supervisor itself refuses the old generation (its check against the current one)
    let (st, b) = direct("x2", 0, ("generation", json!(0)));
    assert_eq!(
        (st, b["code"].as_str()),
        (409, Some("STALE_GENERATION")),
        "{b}"
    );
    let (st, d) = deliver(
        "operator",
        "dlv-key-0006",
        "development",
        &dbody("a6", 1, "s2", json!({"new_session": true})),
    );
    assert_eq!(
        outcome(&d),
        o("stale_generation", Some("start"), "accepted"),
        "{st} {d}"
    );
    assert_eq!(
        d["result"]["supervisor"]["body"]["accepted"]["session_id"],
        sid("s2")
    );
    role.wait("a6");
    settled("a6");

    // 8. budget refusals stop the delivery before the supervisor: an unmeasured run in the
    //    ledger, then the role's share used up (4 runs)
    let kept = std::fs::read(ledger.join("records.jsonl")).expect("ledger");
    let mut unmeasured = records().last().expect("record").clone();
    unmeasured["attempt_id"] = json!("u1");
    unmeasured["usage"]["run"] = json!({"state": "unmeasured", "reason": "test"});
    unmeasured["stop_required"] = json!(false);
    std::fs::write(
        ledger.join("records.jsonl"),
        [kept.clone(), format!("{unmeasured}\n").into_bytes()].concat(),
    )
    .expect("w");
    let n = launches();
    let (st, d) = deliver(
        "operator",
        "dlv-key-0007",
        "development",
        &dbody("a7", 1, "s2", json!({})),
    );
    assert_eq!(
        outcome(&d),
        o("resume_allowed", Some("reconnect"), "budget_refused"),
        "{st} {d}"
    );
    assert!(
        d["result"]["reasons"][0]
            .as_str()
            .is_some_and(|r| r.contains("nmeasured")),
        "{d}"
    );
    std::fs::write(ledger.join("records.jsonl"), &kept).expect("restore");
    let (st, d) = deliver(
        "operator",
        "dlv-key-0008",
        "development",
        &dbody("a8", 1, "s2", json!({})),
    );
    assert_eq!(
        outcome(&d),
        o("resume_allowed", Some("reconnect"), "budget_refused"),
        "{st} {d}"
    );
    assert!(
        d["result"]["reasons"][0]
            .as_str()
            .is_some_and(|r| r.contains("4 of 4 runs")),
        "{d}"
    );
    // the 02-05 spare marker travels in the delivery body to the shared budget check (user
    // decision 2026-11-17 (나)): a malformed one is refused; this 02-04 budget has no spare
    for bad in [
        json!({"reason": "x"}),
        json!({"reason": "", "replaces": "a7", "evidence_ref": "r"}),
        json!({"reason": "x", "replaces": "a/7", "evidence_ref": "r"}),
        json!({"reason": "x", "replaces": "a7", "evidence_ref": "r", "more": 1}),
        // the cause report reference is required (user decision 2026-11-17, addition)
        json!({"reason": "x", "replaces": "a7"}),
        json!({"reason": "x", "replaces": "a7", "evidence_ref": " "}),
        json!({"reason": "x", "replaces": "a7", "evidence_ref": "r".repeat(2001)}),
        // who approved it and when are the coordinator's to write, not the body's
        json!({"reason": "x", "replaces": "a7", "evidence_ref": "r",
               "by": {"kind": "user", "name": "x"}}),
    ] {
        let (st, b) = deliver(
            "operator",
            "dlv-key-0080",
            "development",
            &dbody("a80", 1, "s2", json!({"spare": bad})),
        );
        assert_eq!(st, 400, "{b}");
    }
    // a person attaches a spare (review b38e8df M-1): a service credential is refused, nothing
    // recorded
    let spare_body = dbody(
        "a81",
        1,
        "s2",
        json!({"spare": {"reason": "env defect", "replaces": "a7",
                         "evidence_ref": "cause-report#1"}}),
    );
    let (st, b) = deliver("operator", "dlv-key-0082", "development", &spare_body);
    assert_eq!(
        (st, b["code"].as_str()),
        (403, Some("HUMAN_APPROVAL_REQUIRED")),
        "{b}"
    );
    let (st, d) = deliver("human-op", "dlv-key-0081", "development", &spare_body);
    assert_eq!(st, 201, "{d}");
    assert_eq!(d["result"]["outcome"], "budget_refused", "{d}");
    assert!(
        d["result"]["reasons"][0]
            .as_str()
            .is_some_and(|r| r.contains("has no spare run in the 02-04 budget")),
        "{d}"
    );
    assert_eq!(launches(), n, "a budget refusal calls nothing");

    // 9. the coordinator restarts with a larger share: the stored deliveries and the judgment
    //    are the same
    coord.stop();
    // review c11e734 L-2: a budget plan that leaves out a configured role is refused
    std::fs::write(dir.join("roles.json"), roles_json(6, &["development"])).expect("roles");
    coord.start();
    let (st, again) = deliver("operator", "dlv-key-0001", "development", &b1);
    assert_eq!((st, &again), (200, &d1));
    let n = launches();
    let (st, d) = deliver(
        "operator",
        "dlv-key-0042",
        "development",
        &dbody("a42", 1, "s2", json!({})),
    );
    assert_eq!(
        outcome(&d),
        o("resume_allowed", Some("reconnect"), "budget_refused"),
        "{st} {d}"
    );
    assert!(
        d["result"]["reasons"][0]
            .as_str()
            .is_some_and(|r| r.contains("not the configured roles")),
        "{d}"
    );
    assert_eq!(launches(), n);
    coord.stop();
    std::fs::write(
        dir.join("roles.json"),
        roles_json(6, &["development", "guardrail"]),
    )
    .expect("roles");
    coord.start();

    // 10. the supervisor refuses (no credential): recorded as refused, nothing ran, no ledger
    //     record; the same refusal with its answer lost: unknown, and the re-query finds no
    //     accepted attempt (a person reconciles the ledger's lost-answer record, as for the
    //     02-04 caller)
    let cred = root.join("cred/oauth_token");
    std::fs::remove_file(&cred).expect("rm cred");
    let (st, d) = deliver(
        "operator",
        "dlv-key-0009",
        "development",
        &dbody("a9", 1, "s2", json!({})),
    );
    assert_eq!(
        outcome(&d),
        o("resume_allowed", Some("reconnect"), "refused"),
        "{st} {d}"
    );
    assert_eq!(
        (
            d["result"]["supervisor"]["http_status"].as_i64(),
            d["result"]["supervisor"]["body"]["code"].as_str()
        ),
        (Some(503), Some("CREDENTIAL_UNAVAILABLE")),
        "{d}"
    );
    assert!(records().iter().all(|r| r["attempt_id"] != "a9"));
    let kept = std::fs::read(ledger.join("records.jsonl")).expect("ledger");
    role.drop_next
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let b10 = dbody("a10", 1, "s2", json!({}));
    let (st, d10) = deliver("operator", "dlv-key-0010", "development", &b10);
    assert_eq!(
        outcome(&d10),
        o("resume_allowed", Some("reconnect"), "unknown"),
        "{st} {d10}"
    );
    let (st, r10) = deliver("operator", "dlv-key-0010", "development", &b10);
    assert_eq!(st, 200, "{r10}");
    assert_eq!(
        outcome(&r10),
        o("resume_allowed", Some("reconnect"), "not_accepted"),
        "{r10}"
    );
    assert_eq!(r10["result"]["n"], 2);
    let (st, again) = deliver("operator", "dlv-key-0010", "development", &b10);
    assert_eq!((st, &again), (200, &r10));
    // review af68c93 C-2: a10's call was sent (its result carries the sent digest) and the
    // budget ledger still holds its lost-answer record after the `not_accepted` replay: the
    // record counts — unmeasured, so the assignment's next delivery stops (02-05 step 5)
    let a10_state = || -> String {
        let mut c2 = cfg.clone();
        c2.dbname(&dbname);
        c2.connect(NoTls)
            .expect("db")
            .query_one(
                "SELECT state FROM assignment_run_usage WHERE attempt_id = 'a10'
                 ORDER BY seq DESC LIMIT 1",
                &[],
            )
            .expect("a10 row")
            .get(0)
    };
    let (st, b) = deliver(
        "operator",
        "dlv-key-0060",
        "development",
        &dbody("a60", 1, "s2", json!({})),
    );
    assert_eq!(
        (st, b["rejections"][0]["code"].as_str()),
        (409, Some("ASSIGNMENT_USAGE_UNMEASURED")),
        "{b}"
    );
    assert_eq!(a10_state(), "unmeasured");
    std::fs::write(ledger.join("records.jsonl"), &kept).expect("reconciled");
    // the person's reconciliation removed the record: a record settled before and gone now is
    // `missing` (never lowered) …
    let (st, b) = deliver(
        "operator",
        "dlv-key-0061",
        "development",
        &dbody("a61", 1, "s2", json!({})),
    );
    assert_eq!(
        (st, b["rejections"][0]["code"].as_str()),
        (409, Some("ASSIGNMENT_USAGE_UNMEASURED")),
        "{b}"
    );
    assert_eq!(a10_state(), "missing");
    // … and its result says nothing ran (`not_accepted`): the next settlement closes it
    // review c11e734 L-1: the supervisor holds another request under the delivery's attempt id
    // (accepted before, other content): the delivery is refused and its answer lost, and the
    // re-query does not take the other request's answer for this delivery's. (Its own
    // settlement first closes a10: `withdrawn`, the delivery sent nothing.)
    std::fs::create_dir_all(state.join("attempts")).expect("dir");
    std::fs::write(
        state.join("attempts/a41.json"),
        json!({"attempt_id": "a41", "op": "reconnect", "request_sha256": "0".repeat(64),
               "answer": {"accepted": {"attempt_id": "a41", "session_id": "s2"}}})
        .to_string(),
    )
    .expect("other request");
    role.drop_next
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let b41 = dbody("a41", 1, "s2", json!({}));
    let (st, d41) = deliver("operator", "dlv-key-0041", "development", &b41);
    assert_eq!(
        outcome(&d41),
        o("resume_allowed", Some("reconnect"), "unknown"),
        "{st} {d41}"
    );
    assert_eq!(a10_state(), "withdrawn");
    let (st, r41) = deliver("operator", "dlv-key-0041", "development", &b41);
    assert_eq!(st, 200, "{r41}");
    assert_eq!(
        outcome(&r41),
        o("resume_allowed", Some("reconnect"), "unknown"),
        "{r41}"
    );
    assert_eq!(
        (
            r41["result"]["n"].as_i64(),
            r41["result"]["requeried"].as_bool()
        ),
        (Some(2), Some(true))
    );
    assert!(
        r41["result"]["reasons"][0]
            .as_str()
            .is_some_and(|r| r.contains("another request")),
        "{r41}"
    );
    std::fs::write(ledger.join("records.jsonl"), &kept).expect("reconciled");
    // the person's reconciliation also names the attempt the supervisor never ran: an entry
    // with `model_request: false` (02-05 step 5: a ledger record of a sent call that is gone
    // would otherwise stop the assignment; review 97ff0bc M-1)
    {
        use std::io::Write as _;
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(ledger.join("records.jsonl"))
            .expect("ledger");
        writeln!(
            f,
            "{}",
            json!({"format": "ieo-role-run/v0", "attempt_id": "a41", "role": "development",
                   "model_request": false,
                   "reason": "reconciled: the supervisor held another request under a41"})
        )
        .expect("entry");
    }
    std::fs::write(&cred, "fake-oauth-token\n").expect("cred");
    assert_eq!(runs(), 4);

    // 11. the answer of an accepted reconnect is lost: recorded as unknown; the same request
    //     again asks the supervisor for the attempt's stored answer and runs nothing new
    role.drop_next
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let b11 = dbody("a11", 1, "s2", json!({}));
    let (st, d11) = deliver("operator", "dlv-key-0011", "development", &b11);
    assert_eq!(st, 201, "{d11}");
    assert_eq!(
        outcome(&d11),
        o("resume_allowed", Some("reconnect"), "unknown"),
        "{d11}"
    );
    role.wait("a11");
    let lost = records()
        .into_iter()
        .find(|r| r["attempt_id"] == "a11")
        .expect("lost record");
    assert_eq!(lost["stop_required"], true, "{lost}");
    let n = (launches(), runs());
    let (st, r11) = deliver("operator", "dlv-key-0011", "development", &b11);
    assert_eq!(st, 200, "{r11}");
    assert_eq!(
        outcome(&r11),
        o("resume_allowed", Some("reconnect"), "accepted"),
        "{r11}"
    );
    assert_eq!(
        (
            r11["result"]["n"].as_i64(),
            r11["result"]["requeried"].as_bool()
        ),
        (Some(2), Some(true))
    );
    assert_eq!(
        r11["result"]["supervisor"]["body"]["accepted"]["attempt_id"],
        "a11"
    );
    assert_eq!(r11["result"]["sent_sha256"], d11["result"]["sent_sha256"]);
    let (st, r11b) = deliver("operator", "dlv-key-0011", "development", &b11);
    assert_eq!((st, &r11b), (200, &r11));
    assert_eq!((launches(), runs()), n, "a re-query starts nothing");
    // the lost answer asks for a person: the run is unmeasured, so the assignment's next
    // delivery is refused before anything is recorded (02-05 step 5; the budget would refuse it
    // too); one limit event records the refusal
    let deliveries = count("role_deliveries");
    let (st, d) = deliver(
        "operator",
        "dlv-key-0012",
        "development",
        &dbody("a12", 1, "s2", json!({})),
    );
    assert_eq!(
        (st, d["rejections"][0]["code"].as_str()),
        (409, Some("ASSIGNMENT_USAGE_UNMEASURED")),
        "{d}"
    );
    assert_eq!(d["operational_state"], "awaiting_human_decision");
    assert_eq!(count("role_deliveries"), deliveries);
    // one event for a10's two refusals and this one: the same generation, code and number of
    // settled runs (a10 withdrawn, a11 unmeasured)
    assert_eq!(count("assignment_limit_events"), 1);

    // every run went through a delivery; the database holds every request and result
    assert_eq!(runs(), 5);
    // 20 deliveries (the 21st is refused unrecorded, 02-05 step 5; two are step 7's gated new
    // sessions) and the fixture of step 7's crafted takeover (02-05 step 4)
    assert_eq!(count("role_deliveries"), 22);
    // each run that started reported its takeover and after_run; refusals reported nothing
    assert_eq!(count("role_checkpoints"), 12);
    // every takeover is linked to its delivery: the five runs and step 7's (02-05 step 4)
    assert_eq!(count("role_takeovers"), 6);
    assert_eq!(count("role_delivery_results"), 24);
    let n: i64 = db
        .query_one(
            "SELECT count(*) FROM role_delivery_results WHERE outcome = 'accepted'",
            &[],
        )
        .expect("q")
        .get(0);
    assert_eq!(n, 5);

    // 12. 02-05 step 7: the GitHub record is the final basis of a development resume (compare
    //     is read-only; head and CI are evidence-service input, not a live GitHub read)
    let cmp = compare("development");
    assert_eq!(cmp["decision"], "resume_allowed", "{cmp}");
    assert_eq!(cmp["github"]["ci"]["result"], "success", "{cmp}");
    // an agent's CI claim that differs from the GitHub record: the record is used, the
    // difference recorded
    let mut claim: Value = serde_json::from_str(include_str!(
        "../../../docs/poc/reports/contract_examples/development_progress.json"
    ))
    .expect("example");
    for (k, v) in [
        ("event_id", json!("dev-ci-1")),
        ("event_revision", json!(1)),
        ("sprint_id", json!("POC-01")),
        ("sprint_revision", json!(5)),
        ("task_id", json!("01-01")),
        ("repo_id", json!("repo-a")),
        ("assignment_id", json!("A1")),
        ("generation", json!(1)),
    ] {
        claim[k] = v;
    }
    claim["body"]["target_sha"] = json!(SHA1);
    claim["body"]["internal_checks"] = json!([{"name": "ci", "result": "FAIL", "evidence": []}]);
    let (st, b) = c("dev-agent", "POST", "/v0/agent/reports", Some(&claim));
    assert_eq!(st, 202, "{b}");
    let cmp = compare("development");
    assert_eq!(
        (
            cmp["decision"].as_str(),
            cmp["github"]["ci"]["result"].as_str()
        ),
        (Some("resume_allowed"), Some("success")),
        "{cmp}"
    );
    assert_eq!(
        cmp["github"]["ci_claim"],
        json!({"event_id": "dev-ci-1", "claimed": "failure", "matches_github": false}),
        "{cmp}"
    );
    // a push: the after_run checkpoint names the pushed SHA (crafted here as the supervisor
    // reports it; its own recording from the work tree is tested in ieo-supervisor)
    let push_report_on = |branch: Value, seq: u64, pushed: Value, report_it: bool| {
        let mut cp: Value =
            serde_json::from_slice(&std::fs::read(&cp_path).expect("cp")).expect("cp");
        cp["seq"] = json!(seq);
        cp["branch_ref"] = branch;
        cp["pushed_sha"] = pushed;
        let raw = serde_json::to_string_pretty(&cp).expect("j");
        if report_it {
            let (st, b) = c(
                "sup",
                "POST",
                "/v0/repos/repo-a/roles/development/checkpoints",
                Some(&json!({"seq": seq, "checkpoint_json": raw})),
            );
            assert_eq!(st, 201, "{b}");
            std::fs::write(
                state.join("checkpoints").join(format!("{seq:020}.json")),
                &raw,
            )
            .expect("archive");
        }
        std::fs::write(&cp_path, &raw).expect("local");
    };
    let push_report = |seq: u64, pushed: Value, report_it: bool| {
        push_report_on(json!(BRANCH), seq, pushed, report_it)
    };
    let seq =
        serde_json::from_slice::<Value>(&std::fs::read(&cp_path).expect("cp")).expect("cp")["seq"]
            .as_u64()
            .expect("seq");
    push_report(seq + 1, json!(SHA_P), true);
    let cmp = compare("development");
    assert_eq!(cmp["decision"], "external_reconciliation_required", "{cmp}");
    assert!(
        cmp["reasons"][0]
            .as_str()
            .is_some_and(|r| r.contains(&format!(
                "is {SHA1}, not the checkpoint's pushed SHA {SHA_P}"
            ))),
        "{cmp}"
    );
    assert_eq!(cmp["github"]["expected_from"], "pushed_sha", "{cmp}");
    assert_eq!(
        (
            cmp["github"]["result"].as_str(),
            cmp["github"]["assignment_id"].as_str(),
            cmp["github"]["generation"].as_i64()
        ),
        (Some("mismatch"), Some("A1"), Some(1)),
        "{cmp}"
    );
    // the evidence service sees the push: resume; the CI of the old head is not this head's
    head("gh-head-2", SHA_P);
    let cmp = compare("development");
    assert_eq!(cmp["decision"], "resume_allowed", "{cmp}");
    assert_eq!(
        cmp["github"]["ci"],
        json!({"sha": SHA_P, "result": "none", "evidence": []}),
        "{cmp}"
    );
    assert!(cmp["github"]["ci_claim"].is_null(), "{cmp}");
    // every CI run at the head counts: one failure among them is a failure
    for (id, conclusion) in [("ci-p-a", "success"), ("ci-p-b", "failure")] {
        ingest(
            id,
            "ci_run",
            json!({"sha": SHA_P, "conclusion": conclusion, "source_ref": format!("fixture://ci/{id}")}),
        );
    }
    let cmp = compare("development");
    assert_eq!(
        (
            cmp["decision"].as_str(),
            cmp["github"]["ci"]["result"].as_str()
        ),
        (Some("resume_allowed"), Some("failure")),
        "{cmp}"
    );
    // the local checkpoint changed after it was reported (another pushed SHA): the digest
    // differs from the central record
    let kept_cp = std::fs::read(&cp_path).expect("cp");
    push_report(seq + 1, json!(SHA_F), false);
    let cmp = compare("development");
    assert_eq!(cmp["decision"], "external_reconciliation_required", "{cmp}");
    std::fs::write(&cp_path, &kept_cp).expect("restore");
    // development code moved the remote-tracking ref without pushing: the supervisor reports a
    // pushed SHA that GitHub does not have
    // another branch's head is not this branch's
    ingest(
        "gh-other",
        "branch_head",
        json!({"sha": SHA_F, "ref": "refs/heads/other", "source_ref": "fixture://github/other"}),
    );
    push_report(seq + 2, json!(SHA_F), true);
    let cmp = compare("development");
    assert_eq!(cmp["decision"], "external_reconciliation_required", "{cmp}");
    assert!(
        cmp["reasons"][0]
            .as_str()
            .is_some_and(|r| r.contains(&format!(
                "is {SHA_P}, not the checkpoint's pushed SHA {SHA_F}"
            ))),
        "{cmp}"
    );
    // review f0e0ca9 L-2: a checkpoint written for another branch: its pushed SHA is not this
    // branch's, even when it equals this branch's GitHub head
    push_report_on(json!("refs/heads/other"), seq + 3, json!(SHA_P), true);
    let cmp = compare("development");
    assert_eq!(
        (
            cmp["decision"].as_str(),
            cmp["github"]["result"].as_str(),
            cmp["github"]["push_provenance"].as_str()
        ),
        (
            Some("external_reconciliation_required"),
            Some("mismatch"),
            Some("checkpoint names another branch")
        ),
        "{cmp}"
    );
    assert!(
        cmp["reasons"][0]
            .as_str()
            .is_some_and(|r| r.contains("refs/heads/other is not the assignment's")),
        "{cmp}"
    );
    // review 47e4e3f L-4: a checkpoint that names no branch observed no push: its pushed_sha is
    // not read, the target SHA is expected (and differs from the head here)
    push_report_on(Value::Null, seq + 4, json!(SHA_P), true);
    let cmp = compare("development");
    assert_eq!(
        (
            cmp["github"]["result"].as_str(),
            cmp["github"]["expected_from"].as_str(),
            cmp["github"]["push_provenance"].as_str()
        ),
        (
            Some("mismatch"),
            Some("target_sha"),
            Some("the checkpoint names no branch: no push observed (target SHA expected)")
        ),
        "{cmp}"
    );
    // a guardrail whose review target is not the development assignment's target SHA runs
    // nothing: a new review assignment is needed
    assert_eq!(compare("guardrail")["decision"], "no_checkpoint");
    let assign_g = |generation: u64, sha: &str, expected: Value| {
        c(
            "assigner",
            "POST",
            "/v0/coordinator/assignments",
            Some(
                &json!({"repo_id": "repo-a", "role": "guardrail", "assignment_id": "G1",
                "generation": generation, "sprint_id": "POC-01", "sprint_revision": 5,
                "task_id": "01-01", "target_sha": sha, "expected_current": expected}),
            ),
        )
        .0
    };
    assert_eq!(
        assign_g(1, SHA2, json!({"assignment_id": "G1", "generation": 0})),
        201
    );
    let cmp = compare("guardrail");
    assert_eq!(cmp["decision"], "stale_target", "{cmp}");
    assert!(
        cmp["reasons"][0]
            .as_str()
            // one line, no run of spaces from a broken continuation (review f0e0ca9 L-1)
            .is_some_and(|r| r.contains("target SHA 1111") && !r.contains("  ")),
        "{cmp}"
    );
    let n = launches();
    let (st, d) = deliver(
        "operator",
        "dlv-key-0070",
        "guardrail",
        &json!({"assignment_id": "G1", "generation": 1, "attempt_id": "g70", "session_id": "gs70",
                "prompt": "hi"}),
    );
    assert_eq!(st, 201, "{d}");
    assert_eq!(outcome(&d), o("stale_target", None, "not_called"), "{d}");
    assert_eq!(launches(), n);
    assert_eq!(
        assign_g(2, SHA1, json!({"assignment_id": "G1", "generation": 1})),
        201
    );
    assert_eq!(compare("guardrail")["decision"], "no_checkpoint");
    drop(db);
    coord.stop();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = admin.batch_execute(&format!("DROP DATABASE IF EXISTS {dbname} WITH (FORCE)"));
}
