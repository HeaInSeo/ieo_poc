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
}

impl Role {
    fn new(cfg: Config) -> Role {
        let svc = Arc::new(Mutex::new(Arc::new(Service::new(cfg))));
        let server = tiny_http::Server::http("127.0.0.1:0").expect("bind");
        let port = server.server_addr().to_ip().expect("ip").port();
        let h = Arc::clone(&svc);
        let drop_next = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let launches = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (d, n) = (Arc::clone(&drop_next), Arc::clone(&launches));
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

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .expect("port")
        .port()
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
        {"name": "sup-dev", "token_sha256": digest(&tok("sup")), "kind": "agent", "scopes": ["checkpoint:report"], "repo_ids": ["repo-a"], "role": "development"}]});
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
    // the no-extra-billing observation for the first run of the budget (a stored stream)
    let first = dir.join("first-observation.jsonl");
    std::fs::write(&first, "{\"type\":\"rate_limit_event\",\"rate_limit_info\":{\"status\":\"allowed\",\"overageStatus\":\"rejected\",\"isUsingOverage\":false}}\n").expect("obs");
    let ledger = dir.join("ledger");
    let roles_json = |max_runs: u64| {
        json!({"repo-a": {
            "development": {"url": format!("http://127.0.0.1:{}", role.port),
                            "adapter_token_file": dir.join("adapter_token"),
                            "quota_group_id": "anthropic-shared-0104",
                            "image_digest": "sha256:img", "profile_id": "dev-profile",
                            "profile_revision": 1},
            // nobody listens here: its judgment is not_observed
            "guardrail": {"url": "http://127.0.0.1:9", "adapter_token_file": dir.join("adapter_token"),
                          "image_digest": "sha256:img", "profile_id": "guard-profile",
                          "profile_revision": 1},
            "budget_plan": {"budget": "02-04", "ledger_dir": ledger,
                "first_observation": {"stream": first, "observed_unix": unix_now()},
                "roles": {"development": {"max_window_seconds": 120,
                    "share": {"max_runs": max_runs, "total_seconds": 9_600,
                              "billable_tokens": 1_000_000, "cache_read_tokens": 10_000_000}}}}}})
        .to_string()
    };
    std::fs::write(dir.join("roles.json"), roles_json(4)).expect("roles");
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
                           "attempt_id": attempt, "session_id": session, "prompt": "hi"});
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
        dbody("a1", 0, "s/1", json!({})),
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
            "sprint_revision": 5, "task_id": "01-01", "target_sha": SHA1, "expected_current": expected})),
        )
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
        (Some("s1"), Some("new"))
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

    // 4. a normal after_run → reconnect with the same session; the logical name is recorded but
    //    is not the resume key (C1)
    let (st, d2) = deliver(
        "operator",
        "dlv-key-0002",
        "development",
        &dbody("a2", 0, "s9", json!({"logical_name": "renamed"})),
    );
    assert_eq!(st, 201, "{d2}");
    assert_eq!(
        outcome(&d2),
        o("resume_allowed", Some("reconnect"), "accepted"),
        "{d2}"
    );
    let accepted = &d2["result"]["supervisor"]["body"]["accepted"];
    assert_eq!(
        (accepted["session_id"].as_str(), accepted["mode"].as_str()),
        (Some("s1"), Some("resume")),
        "{d2}"
    );
    assert_eq!(d2["logical_name"], "renamed");
    role.wait("a2");
    settled("a2");

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
        &dbody("a3", 0, "s9", json!({"prompt": "SLEEP"})),
    );
    assert_eq!(
        outcome(&d3),
        o("resume_allowed", Some("reconnect"), "accepted"),
        "{st} {d3}"
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
    let (st, d) = deliver(
        "operator",
        "dlv-key-0020",
        "guardrail",
        &json!({"assignment_id": "G1", "generation": 0, "attempt_id": "g1", "session_id": "gs1",
                "prompt": "hi"}),
    );
    assert_eq!(st, 201, "{d}");
    assert_eq!(outcome(&d), o("not_observed", None, "not_called"), "{d}");
    assert_eq!(launches(), n, "no refusal called the supervisor");

    // 7. a newer generation: stale without an explicit new session; with one, a start
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
        "s2"
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
    assert_eq!(launches(), n, "a budget refusal calls nothing");

    // 9. the coordinator restarts with a larger share: the stored deliveries and the judgment
    //    are the same
    coord.stop();
    std::fs::write(dir.join("roles.json"), roles_json(6)).expect("roles");
    coord.start();
    let (st, again) = deliver("operator", "dlv-key-0001", "development", &b1);
    assert_eq!((st, &again), (200, &d1));

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
    std::fs::write(ledger.join("records.jsonl"), &kept).expect("reconciled");
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
    // the lost answer asks for a person: the budget stops every further delivery
    let (st, d) = deliver(
        "operator",
        "dlv-key-0012",
        "development",
        &dbody("a12", 1, "s2", json!({})),
    );
    assert_eq!(
        outcome(&d),
        o("resume_allowed", Some("reconnect"), "budget_refused"),
        "{st} {d}"
    );
    assert!(
        d["result"]["reasons"][0]
            .as_str()
            .is_some_and(|r| r.contains("asks for a person")),
        "{d}"
    );

    // every run went through a delivery; the database holds every request and result
    assert_eq!(runs(), 5);
    assert_eq!(count("role_deliveries"), 15);
    // each run that started reported its takeover and after_run; refusals reported nothing
    assert_eq!(count("role_checkpoints"), 10);
    assert_eq!(count("role_delivery_results"), 17);
    let n: i64 = db
        .query_one(
            "SELECT count(*) FROM role_delivery_results WHERE outcome = 'accepted'",
            &[],
        )
        .expect("q")
        .get(0);
    assert_eq!(n, 5);
    drop(db);
    coord.stop();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = admin.batch_execute(&format!("DROP DATABASE IF EXISTS {dbname} WITH (FORCE)"));
}
