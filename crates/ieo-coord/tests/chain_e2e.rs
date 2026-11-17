//! 02-05 step 4 end to end: the chain from the sprint revision to the first action, with the
//! coordinator binary, a real PostgreSQL (`IEO_TEST_DB_URL`) and the role supervisor service
//! (`ieo-supervisor`, fake `claude`, no model call): a delivery's takeover is linked to it, the
//! first action is observed in the stream and reported (or filled from the after_run checkpoint
//! when the report is lost), a request alone is never a takeover, a takeover without a delivery
//! is refused, and getAssignmentChain answers the same after a coordinator restart. Skipped
//! without `IEO_TEST_DB_URL` outside CI.

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

/// The role service behind an HTTP port.
struct Role {
    _svc: Arc<Mutex<Arc<Service>>>,
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
        Role { _svc: svc, port }
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
fn the_chain_links_delivery_takeover_and_first_action() {
    e2e("http");
}

/// The supervisor reaches the coordinator over gRPC (takeover, first action, own assignment);
/// deliveries and the chain are HTTP only.
#[test]
fn the_chain_links_delivery_takeover_and_first_action_over_grpc() {
    e2e("grpc");
}

const TABLES: [&str; 7] = [
    "role_checkpoints",
    "role_takeovers",
    "role_first_actions",
    "role_tamper_events",
    "role_deliveries",
    "role_delivery_results",
    "history",
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
    let dbname = format!("ieo_t_chain_{transport}_{}", std::process::id());
    let mut admin = cfg.connect(NoTls).expect("connect");
    let _ = admin.batch_execute(&format!("DROP DATABASE IF EXISTS {dbname} WITH (FORCE)"));
    admin
        .batch_execute(&format!("CREATE DATABASE {dbname}"))
        .expect("create db");
    let dir =
        std::env::temp_dir().join(format!("ieo-chain-e2e-{transport}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    let tok = |n: &str| format!("chn-{n}-{}", std::process::id());
    let principals = json!({"principals": [
        {"name": "alice", "token_sha256": digest(&tok("alice")), "kind": "user", "scopes": ["sprints:register", "sprints:read"], "repo_ids": ["repo-a"]},
        {"name": "bob", "token_sha256": digest(&tok("bob")), "kind": "user", "scopes": ["review:semantic", "sprints:read"], "repo_ids": ["repo-a"]},
        {"name": "assigner", "token_sha256": digest(&tok("assigner")), "kind": "service", "scopes": ["coordinator:assign"], "repo_ids": ["repo-a"]},
        {"name": "operator", "token_sha256": digest(&tok("operator")), "kind": "service", "scopes": ["roles:control"], "repo_ids": ["repo-a"]},
        {"name": "operator-b", "token_sha256": digest(&tok("operator-b")), "kind": "service", "scopes": ["roles:control"], "repo_ids": ["repo-b"]},
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
            "budget_plan": {"budget": "02-04", "ledger_dir": ledger,
                "first_observation": {"stream": first, "observed_unix": unix_now()},
                "roles": {"development": {"max_window_seconds": 120,
                    "share": {"max_runs": 8, "total_seconds": 9_600,
                              "billable_tokens": 1_000_000, "cache_read_tokens": 10_000_000}},
                "guardrail": {"max_window_seconds": 120,
                    "share": {"max_runs": 2, "total_seconds": 4_800,
                              "billable_tokens": 500_000, "cache_read_tokens": 5_000_000}}}}}})
        .to_string(),
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
    let state = root.join("state");
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
    let dbody = |attempt: &str, generation: u64, session: &str, extra: Value| {
        let mut v = json!({"assignment_id": "A1", "generation": generation,
                           "attempt_id": attempt, "session_id": sid(session), "prompt": "hi"});
        for (k, x) in extra.as_object().expect("obj") {
            v[k] = x.clone();
        }
        v
    };
    let chain_as = |who: &str, generation: u64| {
        c(
            who,
            "GET",
            &format!("/v0/coordinator/assignments/A1/generations/{generation}/chain"),
            None,
        )
    };
    let chain = |generation: u64| {
        let (st, b) = chain_as("operator", generation);
        assert_eq!(st, 200, "{b}");
        b
    };
    let cp_report = |raw: &Value| {
        let seq = raw["seq"].as_u64().expect("seq");
        c(
            "sup",
            "POST",
            "/v0/repos/repo-a/roles/development/checkpoints",
            Some(
                &json!({"seq": seq, "checkpoint_json": serde_json::to_string_pretty(raw).expect("j")}),
            ),
        )
    };
    let local_cp = || -> Value {
        serde_json::from_slice(&std::fs::read(state.join("checkpoint.json")).expect("cp"))
            .expect("cp json")
    };
    // the first-action body of the local after_run checkpoint, as the supervisor sent it
    let fa_body = |cp: &Value| {
        let f = &cp["first_action"];
        json!({"assignment_id": cp["assignment_id"], "generation": cp["generation"],
               "attempt_id": f["attempt_id"], "session_id": cp["session_id"],
               "event_offset": f["event_offset"], "observed_at": f["observed_at"], "kind": f["kind"]})
    };
    let report_done = |attempt: &str| -> Value {
        let t0 = Instant::now();
        loop {
            let s = role.status();
            let r = &s["first_action_report"];
            if r["attempt_id"] == attempt && r["pending"] == false {
                return s;
            }
            assert!(t0.elapsed() < Duration::from_secs(30), "{s}");
            std::thread::sleep(Duration::from_millis(50));
        }
    };
    let link = |ch: &Value, d: &str| -> Value {
        ch["links"]
            .as_array()
            .expect("links")
            .iter()
            .find(|l| l["delivery_id"] == d)
            .cloned()
            .unwrap_or_else(|| panic!("no link {d}: {ch}"))
    };

    // the sprint revision and assignment generation 0 of (repo-a, development)
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
            Some(
                &json!({"repo_id": "repo-a", "role": "development", "assignment_id": "A1",
                "generation": generation, "sprint_id": "POC-01", "sprint_revision": 5,
                "task_id": "01-01", "target_sha": SHA1, "expected_current": expected,
                "branch_ref": "refs/heads/feat/a1"}),
            ),
        )
    };
    assert_eq!(assign(0, Value::Null).0, 201);
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
    // the chain of a generation with nothing delivered yet
    let ch = chain(0);
    assert_eq!(
        (
            ch["deliveries"].clone(),
            ch["takeovers"].clone(),
            ch["links"].clone()
        ),
        (json!([]), json!([]), json!([]))
    );
    for (who, st) in [
        ("sup", 403),
        ("alice", 403),
        ("operator-b", 404),
        ("assigner", 200),
    ] {
        assert_eq!(chain_as(who, 0).0, st, "{who}");
    }

    // 1. the whole chain: delivery (verified) → takeover → first action → after_run (claims)
    let (st, d1) = deliver("chn-key-0001", &dbody("a1", 0, "s1", json!({})));
    assert_eq!(st, 201, "{d1}");
    assert_eq!(d1["result"]["outcome"], "accepted", "{d1}");
    let d1_id = d1["delivery_id"].as_str().expect("id").to_string();
    role.wait("a1");
    settled("a1");
    let st1 = report_done("a1");
    assert_eq!(st1["first_action_report"]["result"]["ok"], true, "{st1}");
    let cp1 = local_cp();
    assert_eq!(
        (
            cp1["phase"].as_str(),
            cp1["delivery_id"].as_str(),
            cp1["first_action"]["event_offset"].as_u64(),
            cp1["first_action"]["kind"].as_str(),
            cp1["first_action"]["attempt_id"].as_str()
        ),
        (
            Some("after_run"),
            Some(d1_id.as_str()),
            Some(2),
            Some("tool_use"),
            Some("a1")
        ),
        "{cp1}"
    );
    // the session status's last actual action is that observation (C14), not the run's end
    assert_eq!(
        st1["last_actual_action_at"], cp1["first_action"]["observed_at"],
        "{st1}"
    );
    assert_ne!(st1["last_actual_action_at"], st1["last"]["ended_at"]);
    let ch = chain(0);
    assert_eq!(ch["sprint"]["evidence_level"], "verified");
    assert_eq!(
        (
            ch["sprint"]["revision"].as_u64(),
            ch["sprint"]["digest"].as_str()
        ),
        (Some(5), Some(sd.as_str()))
    );
    assert_eq!(
        ch["sprint"]["sources"].as_array().map(Vec::len),
        s["sources"].as_array().map(Vec::len)
    );
    assert_eq!(ch["assignment"]["evidence_level"], "verified");
    assert_eq!(
        (
            ch["assignment"]["target_sha"].as_str(),
            ch["assignment"]["current"].as_bool(),
            ch["assignment"]["limits"]["total_tokens"].as_u64()
        ),
        (Some(SHA1), Some(true), Some(100_000))
    );
    let d = &ch["deliveries"][0];
    assert_eq!(
        (
            d["delivery_id"].as_str(),
            d["evidence_level"].as_str(),
            d["outcome"].as_str(),
            d["cause"].as_str()
        ),
        (
            Some(d1_id.as_str()),
            Some("verified"),
            Some("accepted"),
            Some("operator")
        ),
        "{d}"
    );
    assert!(
        d["requested_at"].is_string() && d["responded_at"].is_string(),
        "{d}"
    );
    let t = &ch["takeovers"][0];
    assert_eq!(
        (
            t["delivery_id"].as_str(),
            t["seq"].as_u64(),
            t["session_id"].as_str(),
            t["evidence_level"].as_str()
        ),
        (
            Some(d1_id.as_str()),
            Some(1),
            Some(sid("s1").as_str()),
            Some("claim")
        ),
        "{t}"
    );
    assert_eq!(
        t["applied"],
        json!({"sprint_id": "POC-01", "sprint_revision": 5, "spec_digest": sd})
    );
    let f = &ch["first_actions"][0];
    assert_eq!(
        (
            f["attempt_id"].as_str(),
            f["event_offset"].as_u64(),
            f["kind"].as_str(),
            f["source"].as_str(),
            f["delivery_id"].as_str(),
            f["evidence_level"].as_str()
        ),
        (
            Some("a1"),
            Some(2),
            Some("tool_use"),
            Some("report"),
            Some(d1_id.as_str()),
            Some("claim")
        ),
        "{f}"
    );
    assert_eq!(
        ch["checkpoints"]
            .as_array()
            .expect("cps")
            .iter()
            .map(|c| (c["phase"].clone(), c["delivery_id"].clone()))
            .collect::<Vec<_>>(),
        vec![
            (json!("takeover"), json!(d1_id)),
            (json!("after_run"), json!(d1_id))
        ]
    );
    let l = link(&ch, &d1_id);
    assert_eq!(
        (
            l["taken_over"].as_bool(),
            l["takeover_seq"].as_u64(),
            l["first_action"]["source"].as_str(),
            l["first_action_conflict"].as_bool()
        ),
        (Some(true), Some(1), Some("report"), Some(false)),
        "{l}"
    );
    assert_eq!(ch["agent_reports"], json!([]));
    // the same first action again: 200; other content: 409; nothing new either way
    let fa_path = "/v0/repos/repo-a/roles/development/first-actions";
    let before = counts();
    let (st, b) = c("sup", "POST", fa_path, Some(&fa_body(&cp1)));
    assert_eq!(st, 200, "{b}");
    let mut other = fa_body(&cp1);
    other["event_offset"] = json!(3);
    let (st, b) = c("sup", "POST", fa_path, Some(&other));
    assert_eq!(
        (st, b["rejections"][0]["code"].as_str()),
        (409, Some("EVENT_CONTENT_CONFLICT")),
        "{b}"
    );
    assert_eq!(counts(), before);

    // 2. a request alone is never a takeover: the supervisor refuses the delivery (no model
    //    credential); the chain shows the delivery without a takeover
    std::fs::rename(root.join("cred/oauth_token"), root.join("cred/oauth.away")).expect("mv");
    let (st, d2) = deliver("chn-key-0002", &dbody("a2", 0, "s9", json!({})));
    std::fs::rename(root.join("cred/oauth.away"), root.join("cred/oauth_token")).expect("mv");
    assert_eq!(
        (st, d2["result"]["outcome"].as_str()),
        (201, Some("refused")),
        "{d2}"
    );
    let d2_id = d2["delivery_id"].as_str().expect("id").to_string();
    let ch = chain(0);
    assert_eq!(ch["takeovers"].as_array().map(Vec::len), Some(1));
    let l = link(&ch, &d2_id);
    assert_eq!(
        (l["taken_over"].as_bool(), l["first_action"].clone()),
        (Some(false), Value::Null),
        "{l}"
    );
    // a takeover naming it, none, an unknown one or one already taken over: 409, no row
    let crafted = |delivery: Value, generation: u64| {
        let mut cp = local_cp();
        cp["seq"] = json!(cp["seq"].as_u64().expect("seq") + 1);
        cp["phase"] = json!("takeover");
        cp["session_id"] = json!("s-crafted");
        cp["generation"] = json!(generation);
        cp["first_action"] = Value::Null;
        cp["delivery_id"] = delivery;
        cp
    };
    let before = counts();
    for (delivery, code) in [
        (json!(d2_id), "DELIVERY_MISMATCH"),
        (Value::Null, "DELIVERY_REQUIRED"),
        (json!("D-unknown"), "DELIVERY_MISMATCH"),
        (json!(d1_id), "DELIVERY_MISMATCH"),
    ] {
        let (st, b) = cp_report(&crafted(delivery.clone(), 0));
        assert_eq!(
            (st, b["rejections"][0]["code"].as_str()),
            (409, Some(code)),
            "{delivery} {b}"
        );
    }
    assert_eq!(counts(), before, "refused takeovers leave no row");

    // 3. a run without an action (an assistant event without content, a text event before
    //    `init`): the after_run says `none`, nothing is recorded, never offset 0
    let (st, d3) = deliver(
        "chn-key-0003",
        &dbody("a3", 0, "s9", json!({"prompt": "PRE"})),
    );
    assert_eq!(
        (
            st,
            d3["result"]["op"].as_str(),
            d3["result"]["outcome"].as_str()
        ),
        (201, Some("reconnect"), Some("accepted")),
        "{d3}"
    );
    let d3_id = d3["delivery_id"].as_str().expect("id").to_string();
    role.wait("a3");
    settled("a3");
    assert_eq!(local_cp()["first_action"], "none");
    let ch = chain(0);
    assert_eq!(ch["first_actions"].as_array().map(Vec::len), Some(1));
    assert_eq!(link(&ch, &d3_id)["first_action"], "none");
    // C1: the delivery named session s9; the reconnect resumed the checkpoint's session s1, and
    // the takeover records that session (never the delivery's or a logical name)
    let t3 = ch["takeovers"]
        .as_array()
        .expect("t")
        .iter()
        .find(|t| t["delivery_id"] == d3_id.as_str())
        .cloned()
        .expect("takeover of d3");
    assert_eq!(t3["session_id"], sid("s1"), "{t3}");
    assert_eq!(d3["session_id"], sid("s9"));
    assert_eq!(link(&ch, &d3_id)["taken_over"], true);
    // the last actual action is still the one of a1
    assert_eq!(
        role.status()["last_actual_action_at"],
        cp1["first_action"]["observed_at"]
    );

    // 4. a newer generation: a delivery of the older one is never the newer one's takeover; a
    //    delivery of the newer one is
    assert_eq!(
        assign(1, json!({"assignment_id": "A1", "generation": 0})).0,
        201
    );
    let before = counts();
    let (st, b) = deliver("chn-key-0010", &dbody("a10", 0, "s9", json!({})));
    assert_eq!(
        (st, b["rejections"][0]["code"].as_str()),
        (409, Some("STALE_ASSIGNMENT")),
        "{b}"
    );
    let (st, b) = cp_report(&crafted(json!(d3_id), 1));
    assert_eq!(
        (st, b["rejections"][0]["code"].as_str()),
        (409, Some("DELIVERY_MISMATCH")),
        "{b}"
    );
    assert_eq!(counts(), before);
    let (st, d4) = deliver(
        "chn-key-0004",
        &dbody(
            "a4",
            1,
            "s2",
            json!({"new_session": true, "prompt": "TEXT"}),
        ),
    );
    assert_eq!(
        (
            st,
            d4["result"]["decision"].as_str(),
            d4["result"]["op"].as_str()
        ),
        (201, Some("stale_generation"), Some("start")),
        "{d4}"
    );
    let d4_id = d4["delivery_id"].as_str().expect("id").to_string();
    role.wait("a4");
    settled("a4");
    report_done("a4");
    let ch1 = chain(1);
    assert_eq!(ch1["assignment"]["current"], true);
    assert_eq!(chain(0)["assignment"]["current"], false);
    assert_eq!(
        ch1["deliveries"]
            .as_array()
            .expect("d")
            .iter()
            .map(|d| d["delivery_id"].clone())
            .collect::<Vec<_>>(),
        vec![json!(d4_id)]
    );
    assert_eq!(ch1["takeovers"][0]["delivery_id"], d4_id.as_str());
    assert_eq!(ch1["takeovers"][0]["session_id"], sid("s2"));
    assert_eq!(
        (
            ch1["first_actions"][0]["kind"].as_str(),
            ch1["first_actions"][0]["event_offset"].as_u64()
        ),
        (Some("text"), Some(2))
    );
    let ch0 = chain(0);
    assert_eq!(ch0["deliveries"].as_array().map(Vec::len), Some(3));
    assert_eq!(ch0["takeovers"].as_array().map(Vec::len), Some(2));

    // 5. the first-action report is lost (the supervisor cannot read its coordinator
    //    credential at that moment): the run goes on and its after_run fills the record
    let (st, d5) = deliver(
        "chn-key-0005",
        &dbody("a5", 1, "s9", json!({"prompt": "LATEACT"})),
    );
    assert_eq!(
        (st, d5["result"]["outcome"].as_str()),
        (201, Some("accepted")),
        "{d5}"
    );
    let d5_id = d5["delivery_id"].as_str().expect("id").to_string();
    let t0 = Instant::now();
    loop {
        let n: i64 = db()
            .query_one(
                "SELECT count(*) FROM role_takeovers WHERE delivery_id = $1",
                &[&d5_id],
            )
            .expect("q")
            .get(0);
        if n == 1 {
            break;
        }
        assert!(t0.elapsed() < Duration::from_secs(10), "no takeover of d5");
        std::thread::sleep(Duration::from_millis(20));
    }
    let token = root.join("cred/coordinator_token");
    std::fs::rename(&token, root.join("cred/coordinator_token.away")).expect("mv");
    let s5 = report_done("a5");
    std::fs::rename(root.join("cred/coordinator_token.away"), &token).expect("mv");
    assert_eq!(s5["first_action_report"]["result"]["ok"], false, "{s5}");
    assert!(s5["running"].is_object(), "the run goes on: {s5}");
    role.wait("a5");
    settled("a5");
    let cp5 = local_cp();
    let ch1 = chain(1);
    let l = link(&ch1, &d5_id);
    assert_eq!(
        (
            l["first_action"]["source"].as_str(),
            l["first_action"]["kind"].as_str(),
            l["first_action"]["event_offset"].as_u64()
        ),
        (Some("checkpoint"), Some("tool_use"), Some(2)),
        "{l}"
    );
    // the late report of the same action: 200, nothing new
    let before = counts();
    let (st, b) = c("sup", "POST", fa_path, Some(&fa_body(&cp5)));
    assert_eq!(st, 200, "{b}");
    assert_eq!(counts(), before);

    // 6. the coordinator restarts: the chain is the same
    let both = (chain(0), chain(1));
    coord.stop();
    coord.start();
    assert_eq!((chain(0), chain(1)), both);

    // 7. review fe924b7 I-2: the re-query of a delivery without a recorded request digest (the
    //    coordinator stopped before transaction 2) never counts the supervisor's stored answer
    //    as accepted. The delivery as transaction 1 left it (test fixture, older than 120 s);
    //    the supervisor holds an accepted attempt under its attempt id.
    let b6 = dbody("a6", 1, "s9", json!({}));
    db().execute(
        "INSERT INTO role_deliveries (delivery_id, repo_id, role, request_id, body, assignment_id,
             generation, attempt_id, session_id, logical_name, coordinator_record, target_sha,
             requested_by, requested_at)
         VALUES ('D-nodigest', 'repo-a', 'development', 'chn-key-0006', $1, 'A1', 1, 'a6', 's9',
                 'repo-a-development', $2, $3, 'operator', now() - interval '10 minutes')",
        &[
            &b6,
            &json!({"generation": 1, "assignment_id": "A1", "sprint_id": "POC-01",
                    "sprint_revision": 5, "spec_digest": sd, "image_digest": "sha256:img",
                    "profile_id": "dev-profile", "profile_revision": 1, "task_id": "01-01",
                    "limits": {"elapsed_seconds": 3600, "total_tokens": 100000}}),
            &SHA1,
        ],
    )
    .expect("fixture");
    let direct = |attempt: &str, extra: Value| {
        let mut v = json!({"attempt_id": attempt, "prompt": "hi", "target_sha": SHA1,
            "coordinator": {"generation": 1, "assignment_id": "A1", "sprint_id": "POC-01",
                "sprint_revision": 5, "spec_digest": sd, "image_digest": "sha256:img",
                "profile_id": "dev-profile", "profile_revision": 1},
            "limits": {"timeout_ms": 20_000, "billable_room": 100_000, "cache_read_room": 100_000,
                       "grace_ms": 1_000}});
        for (k, x) in extra.as_object().expect("obj") {
            v[k] = x.clone();
        }
        v
    };
    let (st, b) = role.call(
        "POST",
        "/v0/role/reconnect",
        &direct("a6", json!({"delivery_id": "D-nodigest"})),
    );
    assert_eq!(st, 202, "{b}");
    role.wait("a6");
    let (st, d6) = deliver("chn-key-0006", &b6);
    assert_eq!(st, 200, "{d6}");
    let r = &d6["result"];
    assert_eq!(
        (
            r["n"].as_u64(),
            r["outcome"].as_str(),
            r["requeried"].as_bool()
        ),
        (Some(1), Some("unknown"), Some(true)),
        "{d6}"
    );
    assert!(
        r["reasons"][0]
            .as_str()
            .is_some_and(|x| x.contains("no recorded request digest")),
        "{d6}"
    );

    // 8. a start sent to the supervisor without a delivery (the 02-04 host caller's path): the
    //    coordinator refuses its takeover, nothing runs and no row is written
    let before = counts();
    let (st, b) = role.call(
        "POST",
        "/v0/role/start",
        &direct("a7", json!({"session_id": "s7"})),
    );
    assert_eq!(st, 202, "{b}");
    let s7 = role.wait("a7");
    assert_eq!(
        (
            s7["last"]["spawned"].as_bool(),
            s7["last"]["stop"].as_str(),
            s7["last"]["central_refusal"]["coordinator_answer"]["rejections"][0]["code"].as_str()
        ),
        (
            Some(false),
            Some("CentralRefused"),
            Some("DELIVERY_REQUIRED")
        ),
        "{s7}"
    );
    assert_eq!(counts(), before);
    assert!(!state.join("runs/a7/stdout.jsonl").exists());

    // every takeover has its delivery; deliveries without a takeover are visible as such
    let mut d = db();
    let n: i64 = d
        .query_one(
            "SELECT count(*) FROM role_checkpoints c WHERE c.phase = 'takeover'
               AND NOT EXISTS (SELECT 1 FROM role_takeovers t WHERE t.repo_id = c.repo_id
                                 AND t.role = c.role AND t.seq = c.seq)",
            &[],
        )
        .expect("q")
        .get(0);
    assert_eq!(n, 0, "a stored takeover without its delivery link");
    let n: i64 = d
        .query_one("SELECT count(*) FROM role_takeovers", &[])
        .expect("q")
        .get(0);
    // a1, a3, a4, a5, a6
    assert_eq!(n, 5);
    drop(d);
    coord.stop();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = admin.batch_execute(&format!("DROP DATABASE IF EXISTS {dbname} WITH (FORCE)"));
}
