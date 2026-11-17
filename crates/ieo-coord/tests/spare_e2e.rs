//! 02-05 pre-step-8 (PR #81 review 291a9b0 N-1): the only spare path end to end — a person's
//! credential → deliverAssignment → the shared 02-05 budget check → the role supervisor — with
//! the coordinator binary, a real PostgreSQL (`IEO_TEST_DB_URL`), the role supervisor service
//! (fake `claude`, no model call) and the 02-05 budget (user decision 2026-11-17 (나) and its
//! addition). Skipped without `IEO_TEST_DB_URL` outside CI.

use ieo_supervisor::serve::{Config, Service};
use postgres::NoTls;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::Arc;
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
    port: u16,
}

impl Role {
    fn new(cfg: Config) -> Role {
        let svc = Arc::new(Service::new(cfg));
        let server = tiny_http::Server::http("127.0.0.1:0").expect("bind");
        let port = server.server_addr().to_ip().expect("ip").port();
        std::thread::spawn(move || {
            for mut req in server.incoming_requests() {
                let bearer = req
                    .headers()
                    .iter()
                    .find(|x| x.field.equiv("Authorization"))
                    .and_then(|x| x.value.as_str().strip_prefix("Bearer ").map(str::to_string));
                let mut body = Vec::new();
                let _ = req.as_reader().read_to_end(&mut body);
                let r = svc.handle(req.method().as_str(), req.url(), bearer.as_deref(), &body);
                let _ = req.respond(
                    tiny_http::Response::from_data(serde_json::to_vec(&r.body).unwrap_or_default())
                        .with_status_code(r.status),
                );
            }
        });
        Role { port }
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

/// A fake `claude` (no model call; the step 3 fake with actions; `FAIL` in the prompt makes the
/// result an error, a failed run that asks for a person). Session totals grow per session
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
err=false; case "$prompt" in *FAIL*) err=true;; esac
printf '{{"type":"result","subtype":"success","is_error":%s,"session_id":"%s","usage":{{"input_tokens":3,"output_tokens":7,"cache_creation_input_tokens":10,"cache_read_input_tokens":20}},"modelUsage":{{"m":{{"inputTokens":%s,"outputTokens":%s,"cacheReadInputTokens":%s,"cacheCreationInputTokens":%s}}}},"total_cost_usd":0.01}}\n' "$err" "$sid" "$i" "$o" "$r" "$c"
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

/// R1 fails → a service's spare is refused (403) → a person's spare re-runs R1 as the second
/// call and its ledger record carries who approved it, when and the evidence reference → the
/// same request again starts nothing → a second spare is refused → R2 runs as the third call
/// without a marker → the development role stops at its share.
#[test]
fn a_person_attaches_the_0205_spare_through_deliver_assignment() {
    let Ok(base) = std::env::var("IEO_TEST_DB_URL") else {
        assert!(
            std::env::var("CI").is_err(),
            "IEO_TEST_DB_URL must be set in CI"
        );
        eprintln!("SKIP: IEO_TEST_DB_URL not set");
        return;
    };
    let cfg: postgres::Config = base.parse().expect("IEO_TEST_DB_URL");
    let dbname = format!("ieo_t_spare_{}", std::process::id());
    let mut admin = cfg.connect(NoTls).expect("connect");
    let _ = admin.batch_execute(&format!("DROP DATABASE IF EXISTS {dbname} WITH (FORCE)"));
    admin
        .batch_execute(&format!("CREATE DATABASE {dbname}"))
        .expect("create db");
    let dir = std::env::temp_dir().join(format!("ieo-spare-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    let tok = |n: &str| format!("spare-{n}-{}", std::process::id());
    let principals = json!({"principals": [
        {"name": "alice", "token_sha256": digest(&tok("alice")), "kind": "user", "scopes": ["sprints:register", "sprints:read"], "repo_ids": ["repo-a"]},
        {"name": "bob", "token_sha256": digest(&tok("bob")), "kind": "user", "scopes": ["review:semantic", "sprints:read"], "repo_ids": ["repo-a"]},
        {"name": "assigner", "token_sha256": digest(&tok("assigner")), "kind": "service", "scopes": ["coordinator:assign"], "repo_ids": ["repo-a"]},
        {"name": "operator", "token_sha256": digest(&tok("operator")), "kind": "service", "scopes": ["roles:control"], "repo_ids": ["repo-a"]},
        {"name": "human-op", "token_sha256": digest(&tok("human-op")), "kind": "user", "scopes": ["roles:control"], "repo_ids": ["repo-a"]},
        {"name": "sup-dev", "token_sha256": digest(&tok("sup")), "kind": "agent", "scopes": ["checkpoint:report"], "repo_ids": ["repo-a"], "role": "development"},
        {"name": "ingest", "token_sha256": digest(&tok("ingest")), "kind": "service", "scopes": ["evidence:ingest"], "repo_ids": ["repo-a"]}]});
    std::fs::write(dir.join("tokens.json"), principals.to_string()).expect("tokens");
    let mk_role = |name: &str, claude: Value, central: Value| {
        let root = dir.join(name);
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
        let role = if name == "dev" {
            "development"
        } else {
            "guardrail"
        };
        // the approved 02-05 budget: no share configured (the approval fixes it)
        Role::new(
            Config::from_json(&json!({"role": role, "repo": "repo-a",
                "state": root.join("state"), "etc": root.join("etc"), "cred": root.join("cred"),
                "claude": claude, "budget": "02-05", "quota_group_id": "anthropic-shared-0104",
                "checkpoint_central": central}))
            .expect("role config"),
        )
    };
    std::fs::write(dir.join("adapter_token"), ADAPTER).expect("at");
    let claude = fake_claude(&dir);
    let port = free_port();
    let role = mk_role(
        "dev",
        json!(claude),
        json!({"url": format!("http://127.0.0.1:{port}"),
               "token_file": dir.join("dev/cred/coordinator_token"), "transport": "http"}),
    );
    let guard = mk_role("guard", json!("/nonexistent"), json!("local_only"));
    let first = dir.join("first-observation.jsonl");
    std::fs::write(&first, "{\"type\":\"rate_limit_event\",\"rate_limit_info\":{\"status\":\"allowed\",\"overageStatus\":\"rejected\",\"isUsingOverage\":false}}\n").expect("obs");
    let ledger = dir.join("ledger-0205");
    let roles = json!({"repo-a": {
        "development": {"url": format!("http://127.0.0.1:{}", role.port),
                        "adapter_token_file": dir.join("adapter_token"),
                        "quota_group_id": "anthropic-shared-0104",
                        "image_digest": "sha256:img", "profile_id": "dev-profile",
                        "profile_revision": 1},
        "guardrail": {"url": format!("http://127.0.0.1:{}", guard.port),
                      "adapter_token_file": dir.join("adapter_token"),
                      "image_digest": "sha256:img", "profile_id": "guard-profile",
                      "profile_revision": 1},
        "budget_plan": {"budget": "02-05", "ledger_dir": ledger,
            "first_observation": {"stream": first, "observed_unix": unix_now()},
            "roles": {"development": {"max_window_seconds": 120},
                      "guardrail": {"max_window_seconds": 120}}}}});
    std::fs::write(dir.join("roles.json"), roles.to_string()).expect("roles");
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
    coord.start();
    let c = |who: &str, method: &str, path: &str, body: Option<&Value>| {
        http(port, Some(&tok(who)), None, method, path, body).expect("coordinator answered")
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
    let deliver = |who: &str, key: &str, body: &Value| {
        http(
            port,
            Some(&tok(who)),
            Some(key),
            "POST",
            "/v0/repos/repo-a/roles/development/deliveries",
            Some(body),
        )
        .expect("coordinator answered")
    };
    let dbody = |attempt: &str, session: &str, prompt: &str, extra: Value| {
        let mut v = json!({"assignment_id": "A1", "generation": 0,
                           "attempt_id": attempt, "session_id": sid(session), "prompt": prompt});
        for (k, x) in extra.as_object().expect("obj") {
            v[k] = x.clone();
        }
        v
    };
    let deliveries = || -> i64 {
        let mut c2 = cfg.clone();
        c2.dbname(&dbname);
        c2.connect(NoTls)
            .expect("db")
            .query_one("SELECT count(*) FROM role_deliveries", &[])
            .expect("count")
            .get(0)
    };
    let outcome = |d: &Value| d["result"]["outcome"].as_str().map(str::to_string);
    let reason = |d: &Value| d["result"]["reasons"][0].as_str().unwrap_or("").to_string();

    // the sprint revision, the assignment A1/0 and the GitHub head of its branch
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
    let (st, b) = c(
        "assigner",
        "POST",
        "/v0/coordinator/assignments",
        Some(
            &json!({"repo_id": "repo-a", "role": "development", "assignment_id": "A1",
                "generation": 0, "sprint_id": "POC-01", "sprint_revision": 5,
                "task_id": "01-01", "target_sha": SHA1, "expected_current": null,
                "branch_ref": "refs/heads/feat/a1"}),
        ),
    );
    assert_eq!(st, 201, "{b}");
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

    // 0. PR #83 review 67b5cae M-1: a start whose session id is not a lowercase UUID calls
    //    nothing (`claude --session-id` would fail at once and spend a run)
    let mut bad = dbody("r0", "s0", "hi", json!({}));
    bad["session_id"] = json!("s0");
    let (st, d) = deliver("operator", "sp-key-0000", &bad);
    assert_eq!(
        (st, outcome(&d).as_deref()),
        (201, Some("not_called")),
        "{d}"
    );
    assert!(reason(&d).contains("UUID"), "{d}");
    assert!(records().is_empty(), "no run, no ledger record");
    // 1. R1 fails (an error result): its record asks for a person
    let (st, d) = deliver(
        "operator",
        "sp-key-0001",
        &dbody("r1", "s1", "FAIL", json!({})),
    );
    assert_eq!((st, outcome(&d).as_deref()), (201, Some("accepted")), "{d}");
    role.wait("r1");
    let r1 = settled("r1");
    assert_eq!(r1["stop_required"], true, "{r1}");
    // 2. without a marker nothing runs
    let (st, d) = deliver(
        "operator",
        "sp-key-0002",
        &dbody("x2", "s2", "hi", json!({})),
    );
    assert_eq!(
        (st, outcome(&d).as_deref()),
        (201, Some("budget_refused")),
        "{d}"
    );
    assert!(reason(&d).contains("asks for a person"), "{d}");
    // 3. a service's spare: 403, nothing recorded
    let marker = json!({"spare": {"reason": "execution environment defect, cause reported",
                                  "replaces": "r1", "evidence_ref": "cause-report#r1"}});
    let n = deliveries();
    let (st, d) = deliver(
        "operator",
        "sp-key-0003",
        &dbody("sp", "s3", "hi", marker.clone()),
    );
    assert_eq!(
        (st, d["code"].as_str()),
        (403, Some("HUMAN_APPROVAL_REQUIRED")),
        "{d}"
    );
    assert_eq!(deliveries(), n, "nothing recorded");
    // 4. a person's spare: the second call re-runs R1; the record keeps who, when and the
    //    evidence reference; R1's record stays as it was
    let spare_body = dbody("sp", "s3", "hi", marker.clone());
    let (st, sp) = deliver("human-op", "sp-key-0004", &spare_body);
    assert_eq!(
        (st, outcome(&sp).as_deref()),
        (201, Some("accepted")),
        "{sp}"
    );
    role.wait("sp");
    let rs = settled("sp");
    let m = &rs["spare"];
    assert_eq!(
        (
            &m["reason"],
            &m["replaces"],
            &m["evidence_ref"],
            &m["by"]["name"],
            &m["by"]["kind"],
            &m["by"]["via"]
        ),
        (
            &json!("execution environment defect, cause reported"),
            &json!("r1"),
            &json!("cause-report#r1"),
            &json!("human-op"),
            &json!("user"),
            &json!("deliverAssignment")
        ),
        "{rs}"
    );
    assert!(
        m["approved_at"].as_str().is_some_and(|a| a.ends_with('Z')),
        "{rs}"
    );
    assert_eq!(rs["stop_required"], false, "{rs}");
    let r1s: Vec<Value> = records()
        .into_iter()
        .filter(|r| r["attempt_id"] == "r1")
        .collect();
    assert_eq!(r1s, vec![r1.clone()], "R1's record and usage are kept");
    // 5. the same request again: the stored answer, no new delivery, no new run
    let (n, lines) = (deliveries(), records().len());
    let (st, again) = deliver("human-op", "sp-key-0004", &spare_body);
    assert_eq!((st, &again), (200, &sp), "the stored answer");
    assert_eq!((deliveries(), records().len()), (n, lines), "nothing new");
    // 6. the spare is used: another one is refused
    let (st, d) = deliver(
        "human-op",
        "sp-key-0006",
        &dbody("sp2", "s4", "hi", marker.clone()),
    );
    assert_eq!(
        (st, outcome(&d).as_deref()),
        (201, Some("budget_refused")),
        "{d}"
    );
    assert!(reason(&d).contains("used already"), "{d}");
    // 7. R2 without a marker: the third development call. The coordinator is stopped (SIGTERM)
    //    while R2 runs (about 4 s): it waits for the run's record before it exits (review
    //    63fefc1 M-1 (a)), so the ledger has R2's record and no in-flight marker at once
    let (st, d) = deliver(
        "operator",
        "sp-key-0007",
        &dbody("r2", "s5", "LATEACT", json!({})),
    );
    assert_eq!((st, outcome(&d).as_deref()), (201, Some("accepted")), "{d}");
    assert!(
        !role.status()["running"].is_null(),
        "R2 is still running when the coordinator is stopped"
    );
    let pid = coord.child.as_ref().expect("running").id();
    let _ = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status();
    // while it stops, a new delivery starts nothing and records nothing (the signal is seen
    // within 100 ms)
    std::thread::sleep(Duration::from_millis(1000));
    let n = deliveries();
    let (st, d) = deliver(
        "operator",
        "sp-key-0071",
        &dbody("x7", "s7", "hi", json!({})),
    );
    assert_eq!(
        (st, d["code"].as_str()),
        (503, Some("COORDINATOR_STOPPING")),
        "{d}"
    );
    assert_eq!(deliveries(), n);
    coord.stop();
    let r2 = records().into_iter().find(|r| r["attempt_id"] == "r2");
    assert!(
        r2.as_ref().is_some_and(|r| r["stop_required"] == false),
        "R2's record is written before the coordinator exits: {r2:?}"
    );
    assert!(
        !ledger.join("inflight.json").exists(),
        "no in-flight marker"
    );
    coord.start();
    // R2's after_run checkpoint reached the coordinator before it stopped, so the next delivery
    // is judged (not held: "local checkpoint … is not yet recorded")
    // 8. the development role has used its 2 planned runs and its spare
    let (st, d) = deliver(
        "operator",
        "sp-key-0008",
        &dbody("r3", "s6", "hi", json!({})),
    );
    assert_eq!(
        (st, outcome(&d).as_deref()),
        (201, Some("budget_refused")),
        "{d}"
    );
    assert!(reason(&d).contains("planned runs are used"), "{d}");
    let dev: Vec<String> = records()
        .iter()
        .filter(|r| r["model_request"] == true)
        .filter_map(|r| r["attempt_id"].as_str().map(str::to_string))
        .collect();
    assert_eq!(dev, ["r1", "sp", "r2"], "three development runs, one spare");
    coord.stop();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = admin.batch_execute(&format!("DROP DATABASE IF EXISTS {dbname} WITH (FORCE)"));
}
