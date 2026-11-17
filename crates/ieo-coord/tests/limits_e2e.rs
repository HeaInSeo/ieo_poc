//! 02-05 step 5 end to end: assignment-level time and token limits (C7, 02-04 item 1a; user
//! decision 2026-11-16: `total_tokens` = input + output + cache_creation, `elapsed_seconds` = the
//! sum of run durations, both over all generations) with the coordinator binary, a real
//! PostgreSQL (`IEO_TEST_DB_URL`), the role supervisor service (fake `claude`, no model call) and
//! the shared budget library. Skipped without `IEO_TEST_DB_URL` outside CI.

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
    /// The delivery bounds (in flight, start deadline) in seconds, set short so the lost-result
    /// and deadline paths are reached without waiting minutes (`delivery::bounds`).
    bounds: (u64, u64),
}

impl Coord {
    fn start(&mut self) {
        let child = Command::new(env!("CARGO_BIN_EXE_ieo-coord"))
            .args(&self.args)
            .env("IEO_DB_URL", &self.db_url)
            .env(
                "IEO_COORD_DELIVERY_IN_FLIGHT_SECS",
                self.bounds.0.to_string(),
            )
            .env(
                "IEO_COORD_DELIVERY_START_DEADLINE_SECS",
                self.bounds.1.to_string(),
            )
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

/// The role service behind an HTTP port.
struct Role {
    svc: Arc<Mutex<Arc<Service>>>,
    port: u16,
    /// While set, a start or reconnect is handled at once, `handled` is set, and the answer
    /// waits until `release` is set (at most 30 s). Conditions, not times (review 1b362ec I-4).
    slow_start: Arc<std::sync::atomic::AtomicBool>,
    handled: Arc<std::sync::atomic::AtomicBool>,
    release: Arc<std::sync::atomic::AtomicBool>,
}

impl Role {
    fn new(cfg: Config) -> Role {
        let svc = Arc::new(Mutex::new(Arc::new(Service::new(cfg))));
        let server = tiny_http::Server::http("127.0.0.1:0").expect("bind");
        let port = server.server_addr().to_ip().expect("ip").port();
        let h = Arc::clone(&svc);
        let slow_start = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let slow = Arc::clone(&slow_start);
        let handled = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let release = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (handled2, release2) = (Arc::clone(&handled), Arc::clone(&release));
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
                let launch = req.method().as_str() == "POST"
                    && (req.url().ends_with("/start") || req.url().ends_with("/reconnect"));
                if launch && slow.load(std::sync::atomic::Ordering::SeqCst) {
                    handled2.store(true, std::sync::atomic::Ordering::SeqCst);
                    let t0 = Instant::now();
                    while !release2.load(std::sync::atomic::Ordering::SeqCst)
                        && t0.elapsed() < Duration::from_secs(30)
                    {
                        std::thread::sleep(Duration::from_millis(20));
                    }
                }
                let _ = req.respond(
                    tiny_http::Response::from_data(serde_json::to_vec(&r.body).unwrap_or_default())
                        .with_status_code(r.status),
                );
            }
        });
        Role {
            svc,
            port,
            slow_start,
            handled,
            release,
        }
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

/// A fake `claude` (no model call; the step 3 fake with actions). Session totals grow per session
/// (a resumed run's share is measurable). Lines: `init`, an `assistant` event without content,
/// then by prompt: default a `tool_use` event (the first action, offset 2); `TEXT` a text event;
/// `NOACT` none; `LATEACT` the `tool_use` event after 2 s, then 2 s more; `PRE` a text event
/// before `init` (not an action) and none after.
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
u='"usage":{{"input_tokens":3,"output_tokens":5,"cache_creation_input_tokens":10,"cache_read_input_tokens":20}}'
case "$prompt" in *PRE*) printf '{{"type":"assistant","message":{{"id":"m-%s-%s","content":[{{"type":"text","text":"before init"}}],%s}}}}\n' "$sid" "$i" "$u";; esac
printf '{{"type":"system","subtype":"init","session_id":"%s"}}\n' "$sid"
printf '{{"type":"assistant","message":{{"id":"m-%s-%s",%s}}}}\n' "$sid" "$i" "$u"
act() {{ printf '{{"type":"assistant","message":{{"id":"m-%s-%s","content":[{{"type":"tool_use","id":"t1","name":"Bash","input":{{}}}}],%s}}}}\n' "$sid" "$i" "$u"; }}
case "$prompt" in
  *NOACT*|*PRE*) ;;
  *TEXT*) printf '{{"type":"assistant","message":{{"id":"m-%s-%s","content":[{{"type":"text","text":"hello"}}],%s}}}}\n' "$sid" "$i" "$u";;
  *LATEACT*) sleep 2; act; sleep 2;;
  *) act;;
esac
printf '{{"type":"rate_limit_event","rate_limit_info":{{"status":"allowed","overageStatus":"rejected","isUsingOverage":false}}}}\n'
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
    s["limits"] = json!({"elapsed_seconds": 3600, "total_tokens": 100000, "correction_attempts": 10,
                         "unresolved_reason": null});
    s["unresolved_bindings"] = json!([]);
    s["revision"] = json!(5);
    s["tasks"][0]["outputs"] = json!([
        {"artifact_id": "01-01-output", "description": "required output", "required": true},
        {"artifact_id": "01-01-notes", "description": "optional notes", "required": false}]);
    s["digest"] = json!(ieo_spec::spec_digest(&s));
    s
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs()
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
fn assignment_limits_bound_and_stop_deliveries() {
    e2e("http");
}

/// The supervisor reaches the coordinator over gRPC; deliveries and the chain are HTTP only.
#[test]
fn assignment_limits_bound_and_stop_deliveries_over_grpc() {
    e2e("grpc");
}

const TABLES: [&str; 6] = [
    "assignments",
    "role_deliveries",
    "role_delivery_results",
    "assignment_run_usage",
    "assignment_limit_events",
    "role_checkpoints",
];

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
    let dbname = format!("ieo_t_lim_{transport}_{}", std::process::id());
    let mut admin = cfg.connect(NoTls).expect("connect");
    let _ = admin.batch_execute(&format!("DROP DATABASE IF EXISTS {dbname} WITH (FORCE)"));
    admin
        .batch_execute(&format!("CREATE DATABASE {dbname}"))
        .expect("create db");
    let dir = std::env::temp_dir().join(format!("ieo-lim-e2e-{transport}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    let tok = |n: &str| format!("lim-{n}-{}", std::process::id());
    let principals = json!({"principals": [
        {"name": "alice", "token_sha256": digest(&tok("alice")), "kind": "user", "scopes": ["sprints:register", "sprints:read"], "repo_ids": ["repo-a"]},
        {"name": "bob", "token_sha256": digest(&tok("bob")), "kind": "user", "scopes": ["review:semantic", "sprints:read"], "repo_ids": ["repo-a"]},
        {"name": "assigner", "token_sha256": digest(&tok("assigner")), "kind": "service", "scopes": ["coordinator:assign"], "repo_ids": ["repo-a"]},
        {"name": "operator", "token_sha256": digest(&tok("operator")), "kind": "service", "scopes": ["roles:control"], "repo_ids": ["repo-a"]},
        {"name": "sup-dev", "token_sha256": digest(&tok("sup")), "kind": "agent", "scopes": ["checkpoint:report"], "repo_ids": ["repo-a"], "role": "development"},
        {"name": "ingest", "token_sha256": digest(&tok("ingest")), "kind": "service", "scopes": ["evidence:ingest"], "repo_ids": ["repo-a"]}]});
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
    let first = dir.join("first-observation.jsonl");
    std::fs::write(&first, "{\"type\":\"rate_limit_event\",\"rate_limit_info\":{\"status\":\"allowed\",\"overageStatus\":\"rejected\",\"isUsingOverage\":false}}\n").expect("obs");
    let ledger = dir.join("ledger");
    // the development role's share: 10 runs, 1,000 billable tokens (a test fixture value, PLAN §3.2
    // step 5: no approved 02-05 budget is wired here); the plan's window is 120 s
    let roles_json = |ledger: &Path| {
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
                "roles": {"development": {"max_window_seconds": 120,
                    "share": {"max_runs": 10, "total_seconds": 9_600,
                              "billable_tokens": 1_000, "cache_read_tokens": 10_000_000}},
                "guardrail": {"max_window_seconds": 120,
                    "share": {"max_runs": 2, "total_seconds": 4_800,
                              "billable_tokens": 500_000, "cache_read_tokens": 5_000_000}}}}}})
        .to_string()
    };
    std::fs::write(dir.join("roles.json"), roles_json(&ledger)).expect("roles");
    let rootdir = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let mut coord = Coord {
        child: None,
        port,
        db_url: format!("{base} dbname={dbname}"),
        bounds: (47, 2),
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
    let db = || {
        let mut c2 = cfg.clone();
        c2.dbname(&dbname);
        c2.connect(NoTls).expect("db")
    };
    let counts = || -> Vec<i64> {
        let mut d = db();
        TABLES
            .iter()
            .map(|t| {
                d.query_one(&format!("SELECT count(*) FROM {t}"), &[])
                    .expect("count")
                    .get(0)
            })
            .collect()
    };
    let records = || -> Vec<Value> {
        std::fs::read_to_string(ledger.join("records.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).expect("record"))
            .collect()
    };
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
    let deliver = |key: &str, body: &Value| {
        http(
            port,
            Some(&tok("operator")),
            Some(key),
            "POST",
            "/v0/repos/repo-a/roles/development/deliveries",
            Some(body),
        )
        .expect("coordinator answered")
    };
    let dbody = |aid: &str, generation: u64, attempt: &str, session: &str, extra: Value| {
        let mut v = json!({"assignment_id": aid, "generation": generation,
                           "attempt_id": attempt, "session_id": sid(session), "prompt": "hi"});
        for (k, x) in extra.as_object().expect("obj") {
            v[k] = x.clone();
        }
        v
    };
    let chain = |aid: &str, generation: u64| {
        let (st, b) = c(
            "operator",
            "GET",
            &format!("/v0/coordinator/assignments/{aid}/generations/{generation}/chain"),
            None,
        );
        assert_eq!(st, 200, "{b}");
        b
    };
    let code = |b: &Value| b["rejections"][0]["code"].as_str().map(str::to_string);
    // a refused delivery leaves the assignments, deliveries and results as they were; at most
    // one limit event is added
    let refused = |key: &str, body: &Value, want: &str| -> Value {
        let before = counts();
        let (st, b) = deliver(key, body);
        assert_eq!((st, code(&b).as_deref()), (409, Some(want)), "{b}");
        assert_eq!(b["operational_state"], "awaiting_human_decision", "{b}");
        let after = counts();
        // assignments, deliveries, results unchanged; usage rows change only when a finished
        // run is settled first (a measured fact, recorded whatever the answer)
        assert_eq!(after[..3], before[..3], "{b}");
        assert_eq!(after[5], before[5]);
        assert!(after[4] - before[4] <= 1);
        b
    };

    // the sprint revision (limits 3600 s / 100,000 tokens) and assignment A1/0
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
    let assign = |aid: &str, generation: u64, expected: Value, limits: Value| {
        c(
            "assigner",
            "POST",
            "/v0/coordinator/assignments",
            Some(
                &json!({"repo_id": "repo-a", "role": "development", "assignment_id": aid,
                "generation": generation, "sprint_id": "POC-01", "sprint_revision": 5,
                "task_id": "01-01", "target_sha": SHA1, "expected_current": expected,
                "limits": limits, "branch_ref": "refs/heads/feat/a1"}),
            ),
        )
    };
    // 1. the request's limits never exceed the revision's; the format is checked
    let before = counts();
    for (limits, st, want) in [
        (json!({"total_tokens": 100_001}), 422, "LIMITS_ABOVE_SPEC"),
        (json!({"elapsed_seconds": 3_601}), 422, "LIMITS_ABOVE_SPEC"),
        (json!({"total_tokens": 0}), 400, "SCHEMA_VIOLATION"),
        (json!({"cache_read": 5}), 400, "SCHEMA_VIOLATION"),
        (json!(7), 400, "SCHEMA_VIOLATION"),
    ] {
        let (got, b) = assign("A1", 0, Value::Null, limits.clone());
        assert_eq!((got, code(&b).as_deref()), (st, Some(want)), "{limits} {b}");
    }
    assert_eq!(counts(), before);
    let (st, b) = assign("A1", 0, Value::Null, json!({"total_tokens": 40}));
    assert_eq!(st, 201, "{b}");
    // 02-05 step 7: the evidence service's record of the assigned branch at the target SHA
    // (trusted input, fixed here): the GitHub basis of every development resume below
    let (st, b) = c(
        "ingest",
        "POST",
        "/v0/coordinator/evidence",
        Some(
            &json!({"evidence_id": "gh-head-1", "repo_id": "repo-a", "kind": "branch_head",
                     "sha": SHA1, "ref": "refs/heads/feat/a1", "source_ref": "fixture://github/head"}),
        ),
    );
    assert_eq!(st, 201, "{b}");
    let ch = chain("A1", 0);
    assert_eq!(
        ch["assignment"]["limits"]["total_tokens"], 40,
        "the assignment's own limit: {ch}"
    );
    assert_eq!(ch["assignment"]["limits"]["elapsed_seconds"], 3600);
    assert_eq!(ch["usage"]["operational_state"], "open");

    // 2. the run's limits are the smallest of the assignment's, the budget's and the share's:
    //    tokens from the assignment (40 < share 1,000), time from the plan (120 s < 3,600 s)
    let (st, d1) = deliver("lim-key-0001", &dbody("A1", 0, "a1", "s1", json!({})));
    assert_eq!(
        (st, d1["result"]["outcome"].as_str()),
        (201, Some("accepted")),
        "{d1}"
    );
    assert_eq!(d1["result"]["limits"]["billable_room"], 40, "{d1}");
    assert_eq!(d1["result"]["limits"]["timeout_ms"], 114_000, "{d1}");
    role.wait("a1");
    settled("a1");
    // 3. a resumed run: its share is the cumulative difference (02-01 meter)
    let (st, d2) = deliver("lim-key-0002", &dbody("A1", 0, "a2", "s9", json!({})));
    assert_eq!(
        (st, d2["result"]["op"].as_str()),
        (201, Some("reconnect")),
        "{d2}"
    );
    assert_eq!(d2["result"]["limits"]["billable_room"], 20, "40 - 20: {d2}");
    role.wait("a2");
    let r2 = settled("a2");
    assert_eq!(r2["usage"]["run"]["state"], "measured", "{r2}");

    // 4. a new generation: consumption counts over all generations; 40 of 40 used
    assert_eq!(
        assign(
            "A1",
            1,
            json!({"assignment_id": "A1", "generation": 0}),
            json!({"total_tokens": 40})
        )
        .0,
        201
    );
    let b3 = dbody("A1", 1, "a3", "s2", json!({"new_session": true}));
    let r = refused("lim-key-0003", &b3, "ASSIGNMENT_LIMIT_REACHED");
    assert!(
        r["rejections"][0]["message"]
            .as_str()
            .is_some_and(|m| m.contains("total_tokens: 40 of 40")),
        "{r}"
    );
    let ch = chain("A1", 1);
    let runs = ch["usage_runs"].as_array().expect("runs").clone();
    assert_eq!(runs.len(), 2, "{ch}");
    for u in &runs {
        // each run's own share (the resumed one is the difference of the session totals)
        assert_eq!(
            (
                u["state"].as_str(),
                u["tokens"]["input"].as_i64(),
                u["tokens"]["output"].as_i64(),
                u["tokens"]["cache_creation"].as_i64(),
                u["tokens"]["cache_read"].as_i64()
            ),
            (Some("measured"), Some(3), Some(7), Some(10), Some(20)),
            "{u}"
        );
        assert_eq!(u["evidence_level"], "verified");
    }
    assert_eq!(
        (
            ch["usage"]["total_tokens"].as_u64(),
            ch["usage"]["tokens"]["cache_read"].as_u64(),
            ch["usage"]["remaining"]["total_tokens"].as_u64(),
            ch["usage"]["operational_state"].as_str()
        ),
        (Some(40), Some(40), Some(0), Some("awaiting_human_decision")),
        "{ch}"
    );
    // the same refusal again: no second event
    refused("lim-key-0003", &b3, "ASSIGNMENT_LIMIT_REACHED");
    refused(
        "lim-key-0004",
        &dbody("A1", 1, "a4", "s2", json!({"new_session": true})),
        "ASSIGNMENT_LIMIT_REACHED",
    );
    assert_eq!(
        chain("A1", 1)["limit_events"].as_array().map(Vec::len),
        Some(1)
    );

    // 4b. review 97ff0bc M-1: a missing ledger record never lowers consumption
    let rec_path = ledger.join("records.jsonl");
    let full = std::fs::read_to_string(&rec_path).expect("ledger");
    let probe = |key: &str, want: &str| {
        let b = refused(
            key,
            &dbody("A1", 1, "a3", "s2", json!({"new_session": true})),
            want,
        );
        let ch = chain("A1", 1);
        (b, ch)
    };
    // (a) the file is gone (moved, truncated, not mounted): it says nothing; usage stays
    std::fs::rename(&rec_path, ledger.join("records.away")).expect("mv");
    let (_, ch) = probe("lim-key-0003", "ASSIGNMENT_LIMIT_REACHED");
    assert_eq!(
        (
            ch["usage"]["total_tokens"].as_u64(),
            ch["usage"]["runs"].as_u64()
        ),
        (Some(40), Some(2)),
        "{ch}"
    );
    std::fs::rename(ledger.join("records.away"), &rec_path).expect("mv back");
    // (b) one record of a sent call is gone: the run is missing, stops the assignment and
    //     keeps its last known counts
    let only_a1: String = full
        .lines()
        .filter(|l| !l.contains("\"attempt_id\":\"a2\""))
        .map(|l| format!("{l}\n"))
        .collect();
    assert!(only_a1.len() < full.len());
    std::fs::write(&rec_path, &only_a1).expect("partial");
    let (b, ch) = probe("lim-key-0003", "ASSIGNMENT_USAGE_UNMEASURED");
    assert!(
        b["rejections"][0]["message"]
            .as_str()
            .is_some_and(|m| m.contains("record is missing")),
        "{b}"
    );
    assert_eq!(ch["usage"]["total_tokens"], 40, "never lowered: {ch}");
    assert!(
        ch["usage_runs"]
            .as_array()
            .expect("runs")
            .iter()
            .any(|u| u["attempt_id"] == "a2" && u["state"] == "missing"),
        "{ch}"
    );
    // (c) the record comes back: a newer row counts it again (the probe's case: never 0)
    std::fs::write(&rec_path, &full).expect("restore");
    let (_, ch) = probe("lim-key-0003", "ASSIGNMENT_LIMIT_REACHED");
    assert_eq!(
        (
            ch["usage"]["total_tokens"].as_u64(),
            ch["usage"]["unmeasured_runs"].as_u64()
        ),
        (Some(40), Some(0)),
        "{ch}"
    );
    // (d) another ledger (e.g. a new budget's directory): the rows are never continued from it;
    //     the delivery is refused until the configured ledger is the one they came from
    std::fs::write(
        dir.join("roles.json"),
        roles_json(&dir.join("ledger-other")),
    )
    .expect("roles");
    coord.stop();
    coord.start();
    let (b, ch2) = probe("lim-key-0003", "ASSIGNMENT_USAGE_UNMEASURED");
    assert!(
        b["rejections"][0]["message"]
            .as_str()
            .is_some_and(|m| m.contains("settled from the ledger")),
        "{b}"
    );
    assert_eq!(ch2["usage"]["total_tokens"], 40);
    // review 1548061 I-2: the chain reads against the configured ledger and says so
    assert_eq!(
        (
            ch2["usage"]["ledger_mismatch"].as_bool(),
            ch2["usage"]["operational_state"].as_str()
        ),
        (Some(true), Some("awaiting_human_decision")),
        "{ch2}"
    );
    std::fs::write(dir.join("roles.json"), roles_json(&ledger)).expect("roles");
    coord.stop();
    coord.start();
    probe("lim-key-0003", "ASSIGNMENT_LIMIT_REACHED");
    // review 97ff0bc I-2: an attempt id the budget ledger already holds is refused
    std::fs::write(
        &rec_path,
        format!(
            "{full}{}\n",
            json!({"format": "ieo-role-run/v0", "attempt_id": "x-host", "role": "development",
                   "model_request": false, "reason": "another caller of the budget"})
        ),
    )
    .expect("entry");
    let (st, b) = deliver("lim-key-0099", &dbody("A1", 1, "x-host", "s2", json!({})));
    assert_eq!(
        (st, b["rejections"][0]["code"].as_str()),
        (409, Some("ATTEMPT_ID_CONFLICT")),
        "{b}"
    );

    // 5. a later generation with a larger limit (still within the revision's): 20 left
    assert_eq!(
        assign(
            "A1",
            2,
            json!({"assignment_id": "A1", "generation": 1}),
            json!({"total_tokens": 60})
        )
        .0,
        201
    );
    let (st, d5) = deliver(
        "lim-key-0005",
        &dbody("A1", 2, "a5", "s3", json!({"new_session": true})),
    );
    assert_eq!(
        (st, d5["result"]["outcome"].as_str()),
        (201, Some("accepted")),
        "{d5}"
    );
    assert_eq!(d5["result"]["limits"]["billable_room"], 20, "60 - 40: {d5}");
    // review 97ff0bc I-1: the verified delivery record carries the assignment's own limits
    assert_eq!(
        d5["coordinator_record"]["limits"]["total_tokens"], 60,
        "{d5}"
    );
    role.wait("a5");
    settled("a5");
    // review 97ff0bc L-1: the ledger is busy when the delivery arrives (the pre-settle is
    // skipped, the request's check sees 40 of 60); the budget check waits for the lock and the
    // settle under it finds 60 of 60: refused before any call
    let lock_path = ledger.join("caller.lock");
    let holder = std::thread::spawn(move || {
        let f = std::fs::OpenOptions::new()
            .write(true)
            .open(&lock_path)
            .expect("lock file");
        f.lock().expect("lock");
        std::thread::sleep(Duration::from_millis(1_000));
        drop(f);
    });
    std::thread::sleep(Duration::from_millis(200));
    let (st, busy) = deliver("lim-key-0015", &dbody("A1", 2, "a15", "s3", json!({})));
    holder.join().expect("holder");
    assert_eq!(
        (st, busy["result"]["outcome"].as_str()),
        (201, Some("budget_refused")),
        "{busy}"
    );
    assert!(
        busy["result"]["reasons"][0]
            .as_str()
            .is_some_and(|r| r.contains("assignment limits")),
        "{busy}"
    );
    refused(
        "lim-key-0006",
        &dbody("A1", 2, "a6", "s3", json!({})),
        "ASSIGNMENT_LIMIT_REACHED",
    );
    // 6. the coordinator restarts: the same totals and the same refusal
    let before_restart = chain("A1", 2);
    assert_eq!(before_restart["usage"]["total_tokens"], 60);
    assert_eq!(before_restart["usage"]["runs"], 3);
    coord.stop();
    coord.start();
    assert_eq!(chain("A1", 2), before_restart);
    refused(
        "lim-key-0006",
        &dbody("A1", 2, "a6", "s3", json!({})),
        "ASSIGNMENT_LIMIT_REACHED",
    );

    // 7. the budget smaller than the assignment: a generation without its own limits (the
    //    revision's 100,000 tokens, 99,940 left) gets the role share's 940 tokens left
    assert_eq!(
        assign(
            "A1",
            3,
            json!({"assignment_id": "A1", "generation": 2}),
            Value::Null
        )
        .0,
        201
    );
    let (st, d7) = deliver(
        "lim-key-0007",
        &dbody("A1", 3, "a7", "s7", json!({"new_session": true})),
    );
    assert_eq!(
        (st, d7["result"]["outcome"].as_str()),
        (201, Some("accepted")),
        "{d7}"
    );
    assert_eq!(d7["result"]["limits"]["billable_room"], 940, "{d7}");
    role.wait("a7");
    settled("a7");

    // 8. the time limit: the time used so far plus 7 s gives a 7 s window (1 s before the stop
    //    grace); after a measured run of 1 s, 6 s are left: not more than the stop grace
    // the time used so far, as the ledger charges it (each run's duration rounded up)
    let used: u64 = records()
        .iter()
        .filter_map(|r| r["duration_ms"].as_u64())
        .map(|ms| ms.div_ceil(1000))
        .sum();
    assert_eq!(
        assign(
            "A1",
            4,
            json!({"assignment_id": "A1", "generation": 3}),
            json!({"elapsed_seconds": used + 7})
        )
        .0,
        201
    );
    let (st, d8) = deliver(
        "lim-key-0008",
        &dbody(
            "A1",
            4,
            "a8",
            "s8",
            json!({"new_session": true, "prompt": "NOACT"}),
        ),
    );
    assert_eq!(
        (st, d8["result"]["outcome"].as_str()),
        (201, Some("accepted")),
        "{d8}"
    );
    assert_eq!(d8["result"]["limits"]["timeout_ms"], 1_000, "{d8}");
    role.wait("a8");
    assert_eq!(settled("a8")["usage"]["run"]["state"], "measured");
    let r = refused(
        "lim-key-0009",
        &dbody("A1", 4, "a9", "s8", json!({})),
        "ASSIGNMENT_LIMIT_REACHED",
    );
    assert!(
        r["rejections"][0]["message"]
            .as_str()
            .is_some_and(|m| m.contains(&format!("elapsed_seconds: {} of {}", used + 1, used + 7))),
        "{r}"
    );

    // 8b. review 1548061: the deadline before the call (I-3), and a delivery whose result
    //     transaction is lost after its call (N-M1). This coordinator runs with short bounds:
    //     in flight 47 s, start deadline 2 s.
    let total_before = chain("A1", 4)["usage"]["total_tokens"]
        .as_u64()
        .expect("total");
    assert_eq!(
        assign(
            "A1",
            5,
            json!({"assignment_id": "A1", "generation": 4}),
            json!({"total_tokens": total_before + 40})
        )
        .0,
        201
    );
    // (i) the budget's ledger is busy for 2.5 s: the call would start past the deadline, so
    //     nothing is called
    let lock_path = ledger.join("caller.lock");
    // the delivery is sent once the lock is held (a signal, not a 200 ms guess; review 6e0ad85
    // I-a)
    let (locked_tx, locked) = std::sync::mpsc::channel();
    let busy = std::thread::spawn(move || {
        let f = std::fs::OpenOptions::new()
            .write(true)
            .open(&lock_path)
            .expect("lock file");
        f.lock().expect("lock");
        locked_tx.send(()).expect("signal");
        std::thread::sleep(Duration::from_millis(2_500));
        drop(f);
    });
    locked.recv().expect("the ledger lock is held");
    let (st, dl) = deliver(
        "lim-key-0020",
        &dbody("A1", 5, "ad", "s20", json!({"new_session": true})),
    );
    busy.join().expect("busy");
    assert_eq!(
        (st, dl["result"]["outcome"].as_str()),
        (201, Some("not_called")),
        "{dl}"
    );
    assert!(
        dl["result"]["reasons"][0]
            .as_str()
            .is_some_and(|r| r.contains("start deadline")),
        "{dl}"
    );
    assert!(records().iter().all(|r| r["attempt_id"] != "ad"));
    // the call is made and handled; once the supervisor is done with it (for an accepted run:
    // the run, whose takeover report itself takes the delivery lock, has ended), the delivery
    // lock is taken and held 15 s, and only then is the call answered: the result's transaction
    // waits past the 10 s statement bound and is rolled back. Review 1b362ec I-4: the lock was
    // taken 1.5 s after the delivery began; under load the takeover report came later, waited
    // for the lock, was refused, and aL ran nothing (unmeasured). Conditions, not times, now.
    let hold_delivery_lock = |attempt: &'static str, ran: bool| {
        use std::sync::atomic::Ordering;
        let mut c2 = cfg.clone();
        c2.dbname(&dbname);
        let svc = Arc::clone(&role.svc);
        let (handled, release) = (Arc::clone(&role.handled), Arc::clone(&role.release));
        handled.store(false, Ordering::SeqCst);
        release.store(false, Ordering::SeqCst);
        std::thread::spawn(move || {
            let t0 = Instant::now();
            loop {
                let done = handled.load(Ordering::SeqCst) && {
                    let s = svc.lock().expect("svc").status();
                    !ran || (s["running"].is_null() && s["last"]["attempt_id"] == attempt)
                };
                if done {
                    break;
                }
                assert!(
                    t0.elapsed() < Duration::from_secs(30),
                    "{attempt} not handled"
                );
                std::thread::sleep(Duration::from_millis(20));
            }
            let mut holder = c2.connect(NoTls).expect("db");
            let mut tx = holder.transaction().expect("tx");
            tx.execute(
                "SELECT pg_advisory_xact_lock(hashtextextended('delivery:repo-a:development', 0))",
                &[],
            )
            .expect("hold");
            release.store(true, Ordering::SeqCst);
            std::thread::sleep(Duration::from_secs(15));
            tx.rollback().expect("release");
        })
    };
    // (ii) the supervisor accepts aL and runs it; the result is lost
    let bl = dbody("A1", 5, "aL", "s21", json!({"new_session": true}));
    role.slow_start
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let h = hold_delivery_lock("aL", true);
    let (st, b) = deliver("lim-key-0021", &bl);
    role.slow_start
        .store(false, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(st, 500, "{b}");
    assert!(b.to_string().contains("rolled back"), "{b}");
    h.join().expect("released");
    role.wait("aL");
    assert_eq!(settled("aL")["usage"]["run"]["state"], "measured");
    // (iii) the supervisor refuses aR (no model credential); its result is lost too
    std::fs::rename(root.join("cred/oauth_token"), root.join("cred/oauth.away")).expect("mv");
    let br = dbody("A1", 5, "aR", "s21", json!({}));
    role.slow_start
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let h = hold_delivery_lock("aR", false);
    let (st, b) = deliver("lim-key-0022", &br);
    role.slow_start
        .store(false, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(st, 500, "{b}");
    h.join().expect("released");
    std::fs::rename(root.join("cred/oauth.away"), root.join("cred/oauth_token")).expect("mv");
    assert!(records().iter().all(|r| r["attempt_id"] != "aR"));
    // 02-05 step 6: the repo stop reports the coordinator's two deliveries whose outcome is not
    // known as pending external requests; the roles' own lists are empty, and the untracked
    // children keep the stop unconfirmed (user decision 2026-11-15 item 3)
    let (st, stop) = c(
        "operator",
        "POST",
        "/v0/repos/repo-a/stop",
        Some(&json!({})),
    );
    assert_eq!(st, 200, "{stop}");
    assert_eq!(stop["safe_shutdown_confirmed"], false, "{stop}");
    let pend: Vec<String> = stop["coordinator_pending_external_requests"]
        .as_array()
        .expect("pending")
        .iter()
        .map(|p| p["attempt_id"].as_str().unwrap_or_default().to_string())
        .collect();
    // exactly those two: every other delivery has a known outcome
    assert_eq!(pend, ["aL", "aR"], "{stop}");
    let reasons = stop["unconfirmed"].to_string();
    assert!(reasons.contains("coordinator: delivery"), "{stop}");
    assert!(reasons.contains("development: active_children"), "{stop}");
    assert_eq!(
        stop["roles"]["development"]["safe_shutdown"]["pending_external_requests"],
        json!([]),
        "{stop}"
    );
    // while the two are younger than the in-flight bound, nothing is settled for them; past
    // it, they may have run
    std::thread::sleep(Duration::from_secs(48));
    // the replay of aL finds the supervisor's stored answer but no digest: `unknown`
    let (st, r) = deliver("lim-key-0021", &bl);
    assert_eq!(
        (
            st,
            r["result"]["outcome"].as_str(),
            r["result"]["sent_sha256"].clone()
        ),
        (200, Some("unknown"), Value::Null),
        "{r}"
    );
    // review 1b362ec L-3 (R3): a delivery whose newest result is `unknown` is still a pending
    // external request of the coordinator, reported as "result unknown"
    let (st, stop) = c(
        "operator",
        "POST",
        "/v0/repos/repo-a/stop",
        Some(&json!({})),
    );
    assert_eq!(st, 200, "{stop}");
    let pend: Vec<(String, String)> = stop["coordinator_pending_external_requests"]
        .as_array()
        .expect("pending")
        .iter()
        .map(|p| {
            (
                p["attempt_id"].as_str().unwrap_or_default().to_string(),
                p["state"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    assert_eq!(
        pend,
        [
            ("aL".to_string(), "result unknown".to_string()),
            ("aR".to_string(), "no result recorded".to_string())
        ],
        "{stop}"
    );
    assert!(
        stop["unconfirmed"]
            .to_string()
            .contains("has no known outcome (result unknown)"),
        "{stop}"
    );
    // without aL's ledger record, the `unknown` run is missing: the assignment stops
    let with_al = std::fs::read_to_string(&rec_path).expect("ledger");
    let without_al: String = with_al
        .lines()
        .filter(|l| !l.contains("\"attempt_id\":\"aL\""))
        .map(|l| format!("{l}\n"))
        .collect();
    std::fs::write(&rec_path, &without_al).expect("partial");
    refused(
        "lim-key-0023",
        &dbody("A1", 5, "x23", "s21", json!({})),
        "ASSIGNMENT_USAGE_UNMEASURED",
    );
    // a record with aL's attempt id under another role is not aL's
    std::fs::write(
        &rec_path,
        format!(
            "{without_al}{}\n",
            json!({"format": "ieo-role-run/v0", "attempt_id": "aL", "role": "guardrail",
                   "model_request": false, "reason": "another role's entry"})
        ),
    )
    .expect("other role");
    refused(
        "lim-key-0024",
        &dbody("A1", 5, "x24", "s21", json!({})),
        "ASSIGNMENT_USAGE_UNMEASURED",
    );
    std::fs::write(&rec_path, &with_al).expect("restore");
    let ch = chain("A1", 5);
    let st_of = |a: &str| {
        ch["usage_runs"]
            .as_array()
            .expect("runs")
            .iter()
            .find(|u| u["attempt_id"] == a)
            .map(|u| u["state"].as_str().unwrap_or_default().to_string())
    };
    assert_eq!(st_of("aR").as_deref(), Some("missing"), "{ch}");
    // the replay of aR: the supervisor holds no accepted attempt → not_accepted; the missing
    // row of aR is then closed ("sent nothing") at the next settlement
    let (st, r) = deliver("lim-key-0022", &br);
    assert_eq!(
        (st, r["result"]["outcome"].as_str()),
        (200, Some("not_accepted")),
        "{r}"
    );
    // aL is charged: 20 of the 40 tokens are left (not 40)
    let (st, dm) = deliver("lim-key-0025", &dbody("A1", 5, "aM", "s21", json!({})));
    assert_eq!(
        (st, dm["result"]["outcome"].as_str()),
        (201, Some("accepted")),
        "{dm}"
    );
    assert_eq!(dm["result"]["limits"]["billable_room"], 20, "{dm}");
    role.wait("aM");
    settled("aM");
    let ch = chain("A1", 5);
    let row_of = |a: &str| {
        ch["usage_runs"]
            .as_array()
            .expect("runs")
            .iter()
            .find(|u| u["attempt_id"] == a)
            .cloned()
            .unwrap_or_default()
    };
    assert_eq!(row_of("aL")["state"], "measured", "{ch}");
    assert_eq!(row_of("aR")["state"], "withdrawn", "{ch}");
    assert!(
        row_of("aR")["reason"]
            .as_str()
            .is_some_and(|r| r.contains("sent nothing")),
        "{ch}"
    );
    assert_eq!(
        ch["usage"]["total_tokens"],
        total_before + 20,
        "aM not settled yet: {ch}"
    );

    // 9. a run that uses up its window is stopped by the guard and is unmeasured: the
    //    assignment's next delivery stops (never counted as 0)
    let used9: u64 = records()
        .iter()
        .filter_map(|r| r["duration_ms"].as_u64())
        .map(|ms| ms.div_ceil(1000))
        .sum();
    assert_eq!(
        assign(
            "A1",
            6,
            json!({"assignment_id": "A1", "generation": 5}),
            json!({"elapsed_seconds": used9 + 8})
        )
        .0,
        201
    );
    let (st, d10) = deliver(
        "lim-key-0010",
        &dbody(
            "A1",
            6,
            "a10",
            "s10",
            json!({"new_session": true, "prompt": "LATEACT"}),
        ),
    );
    assert_eq!(
        (st, d10["result"]["outcome"].as_str()),
        (201, Some("accepted")),
        "{d10}"
    );
    assert_eq!(d10["result"]["limits"]["timeout_ms"], 2_000, "{d10}");
    let s10 = role.wait("a10");
    assert_eq!(s10["last"]["stop"], "TotalTimeout", "{s10}");
    assert_ne!(settled("a10")["usage"]["run"]["state"], "measured");
    let r = refused(
        "lim-key-0011",
        &dbody("A1", 6, "a11", "s10", json!({})),
        "ASSIGNMENT_USAGE_UNMEASURED",
    );
    assert_eq!(r["usage"]["unmeasured_runs"], 1, "{r}");
    let ch = chain("A1", 6);
    assert!(
        ch["usage_runs"]
            .as_array()
            .expect("runs")
            .iter()
            .any(|u| u["attempt_id"] == "a10" && u["state"] == "unmeasured"),
        "{ch}"
    );
    assert_eq!(ch["usage"]["operational_state"], "awaiting_human_decision");

    // totals: eleven deliveries (eight runs, the busy-ledger budget_refused, the deadline
    // not_called, the refused aR); usage rows: a2 three (measured, missing, measured), aL two
    // (missing, measured), aR two (missing, withdrawn), the other runs one each — 13
    let n = counts();
    assert_eq!((n[1], n[3]), (11, 13), "{n:?}");
    // A1/1 (tokens, then missing/other ledger), A1/2 (tokens), A1/4 (time), A1/5 (missing),
    // A1/6 (unmeasured)
    assert_eq!(n[4], 6, "{n:?}");

    // 10. the 02-05 budget (its own ledger, the approved shares, none configured): an
    //     assignment whose runs were settled from the 02-04 ledger is refused, never continued
    //     or transferred; its usage stays where it was (02-05 budget PR)
    std::fs::write(
        dir.join("roles.json"),
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
            "budget_plan": {"budget": "02-05", "ledger_dir": dir.join("ledger-0205"),
                "first_observation": {"stream": first, "observed_unix": unix_now()},
                "roles": {"development": {}, "guardrail": {}}}}})
        .to_string(),
    )
    .expect("roles");
    coord.stop();
    coord.start();
    let before = chain("A1", 6)["usage"].clone();
    let (st, b) = deliver("lim-key-0012", &dbody("A1", 6, "a12", "s10", json!({})));
    assert_eq!(
        (st, b["rejections"][0]["code"].as_str()),
        (409, Some("ASSIGNMENT_USAGE_UNMEASURED")),
        "{b}"
    );
    let after = chain("A1", 6)["usage"].clone();
    assert_eq!(
        (
            after["ledger_mismatch"].as_bool(),
            after["total_tokens"].clone(),
            after["runs"].clone()
        ),
        (
            Some(true),
            before["total_tokens"].clone(),
            before["runs"].clone()
        ),
        "{after}"
    );
    assert!(
        after["ledgers"].as_array().is_some_and(|l| l
            .iter()
            .all(|x| x.as_str().is_some_and(|x| x.starts_with("02-04@")))),
        "{after}"
    );
    assert!(
        !dir.join("ledger-0205/records.jsonl").exists(),
        "nothing was charged to the 02-05 ledger"
    );
    coord.stop();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = admin.batch_execute(&format!("DROP DATABASE IF EXISTS {dbname} WITH (FORCE)"));
}
