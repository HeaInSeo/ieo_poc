//! 02-03 integration tests against a real PostgreSQL and the real HTTP server.
//!
//! `IEO_TEST_DB_URL` (key=value or URL form) names a server where the test user may create
//! databases. Each test creates its own database and server on 127.0.0.1:0. Without the
//! variable the tests fail under CI (`CI` set) and are skipped with a message elsewhere.

use ieo_coord::api::{Ctx, migrate};
use ieo_coord::auth::{Credentials, token_digest};
use ieo_coord::http::{App, serve};
use ieo_coord::policy;
use postgres::{Client, Config, NoTls};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const MANIFEST: &str = include_str!("../../../docs/poc/Console_Coordinator_Sprints_v0.json");
const POLICY: &[u8] = include_bytes!("../../../contracts/judgment-policy-v0.json");

const SHA1: &str = "1111111111111111111111111111111111111111";
const SHA2: &str = "2222222222222222222222222222222222222222";
const SHA3: &str = "3333333333333333333333333333333333333333";
const MERGE: &str = "4444444444444444444444444444444444444444";

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct Env {
    addr: String,
    base: Config,
    dbname: String,
    cfg: Config,
    tokens: std::collections::BTreeMap<&'static str, String>,
}

fn base_config() -> Option<Config> {
    match std::env::var("IEO_TEST_DB_URL") {
        Ok(u) => Some(u.parse().unwrap_or_else(|e| panic!("IEO_TEST_DB_URL: {e}"))),
        Err(_) => {
            assert!(
                std::env::var("CI").is_err(),
                "IEO_TEST_DB_URL must be set in CI"
            );
            eprintln!("SKIP: IEO_TEST_DB_URL not set; 02-03 DB tests not run");
            None
        }
    }
}

/// (name, kind, scopes, repos, role)
type PrincipalRow = (
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
    Option<&'static str>,
);

const PRINCIPALS: [PrincipalRow; 11] = [
    (
        "alice",
        "user",
        &["sprints:register", "sprints:read"],
        &["repo-a"],
        None,
    ),
    (
        "bob",
        "user",
        &["review:semantic", "sprints:read"],
        &["repo-a"],
        None,
    ),
    (
        "carol",
        "user",
        &["sprints:register", "sprints:read", "review:semantic"],
        &["repo-b"],
        None,
    ),
    (
        "assigner",
        "service",
        &["coordinator:assign"],
        &["repo-a", "repo-b"],
        None,
    ),
    (
        "ingest",
        "service",
        &["evidence:ingest"],
        &["repo-a", "repo-b"],
        None,
    ),
    (
        "judge",
        "service",
        &["judge:verify"],
        &["repo-a", "repo-b"],
        None,
    ),
    ("courier", "service", &["outbox:deliver"], &["repo-a"], None),
    (
        "dev-a",
        "agent",
        &["agent:report"],
        &["repo-a"],
        Some("development"),
    ),
    (
        "gr-a",
        "agent",
        &["agent:report", "agent:repro"],
        &["repo-a"],
        Some("guardrail"),
    ),
    (
        "dev-b",
        "agent",
        &["agent:report"],
        &["repo-b"],
        Some("development"),
    ),
    (
        "gr-b",
        "agent",
        &["agent:report", "agent:repro"],
        &["repo-b"],
        Some("guardrail"),
    ),
];

fn start() -> Option<Env> {
    let base = base_config()?;
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let dbname = format!("ieo_t_{}_{n}_{}", std::process::id(), nanos % 1_000_000_000);
    let mut admin = base.connect(NoTls).expect("connect admin");
    admin
        .batch_execute(&format!("CREATE DATABASE {dbname}"))
        .expect("create db");
    let mut cfg = base.clone();
    cfg.dbname(&dbname);
    let mut db = cfg.connect(NoTls).expect("connect test db");
    migrate(&mut db).expect("migrate");
    let mut tokens = std::collections::BTreeMap::new();
    let mut list = Vec::new();
    for (name, kind, scopes, repos, role) in PRINCIPALS {
        let t = format!("tok-{name}-{n}-{nanos}");
        let mut p = json!({"name": name, "token_sha256": token_digest(&t), "kind": kind,
                           "scopes": scopes, "repo_ids": repos});
        if let Some(r) = role {
            p["role"] = json!(r);
        }
        list.push(p);
        tokens.insert(name, t);
    }
    let creds = Credentials::from_json(&json!({ "principals": list })).expect("creds");
    let mut manifest: Value = serde_json::from_str(MANIFEST).expect("manifest");
    // an approved test profile for the human-input binding of repo-a
    if let Some(p) = manifest["inspection_profiles"].as_array_mut() {
        p.push(json!({"profile_id": "hum-profile", "revision": 1, "items": {"11-01.accept": {}}}));
    }
    let bindings = json!({"repo-a": {
        "sprint_id": "HUM-01", "sprint_number": 11, "plan_version": "v1.3.3",
        "repository_url": null, "timezone": "Asia/Seoul", "track": "핵심",
        "acceptance_profile_id": "hum-profile", "acceptance_profile_revision": 1,
        "delegation": {"allowed": ["승인된 범위의 구현"], "forbidden": ["범위 밖 repo 쓰기"],
                       "approval_scope_ref": "User_Decisions_2026-11-11.md"},
        "human_validation": {"required": false, "timing": "해당 없음", "criteria_ref": "Human_Sprint_Authoring_v0.md"},
        "sources": [{"kind": "human_input", "ref": "fixture://human-input", "revision": "1",
                     "digest": "0000000000000000000000000000000000000000000000000000000000000000",
                     "observed_at": null}]}});
    let app = App {
        ctx: Ctx {
            policy: policy::load(POLICY).expect("policy"),
            manifest,
            bindings,
        },
        creds,
    };
    let server = tiny_http::Server::http("127.0.0.1:0").expect("bind");
    let addr = server
        .server_addr()
        .to_ip()
        .map(|a| a.to_string())
        .expect("addr");
    let server = Arc::new(server);
    let app = Arc::new(app);
    let c2 = cfg.clone();
    std::thread::spawn(move || {
        let _ = serve(server, app, &c2, 6);
    });
    Some(Env {
        addr,
        base,
        dbname,
        cfg,
        tokens,
    })
}

impl Drop for Env {
    /// Each test drops its own database (the server's connections are closed by FORCE).
    fn drop(&mut self) {
        if let Ok(mut admin) = self.base.connect(NoTls) {
            let _ = admin.batch_execute(&format!(
                "DROP DATABASE IF EXISTS {} WITH (FORCE)",
                self.dbname
            ));
        }
    }
}

impl Env {
    fn call(
        &self,
        who: Option<&str>,
        method: &str,
        path: &str,
        key: Option<&str>,
        body: Option<&Value>,
    ) -> (u16, Value) {
        let payload = body.map(|b| b.to_string()).unwrap_or_default();
        let mut req = format!("{method} {path} HTTP/1.1\r\nHost: t\r\nConnection: close\r\n");
        if let Some(w) = who {
            let t = self.tokens.get(w).cloned().unwrap_or_else(|| w.to_string());
            req += &format!("Authorization: Bearer {t}\r\n");
        }
        if let Some(k) = key {
            req += &format!("Idempotency-Key: {k}\r\n");
        }
        req += &format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{payload}",
            payload.len()
        );
        let mut s = TcpStream::connect(&self.addr).expect("connect");
        s.write_all(req.as_bytes()).expect("write");
        let mut out = String::new();
        s.read_to_string(&mut out).expect("read");
        let status: u16 = out
            .split_whitespace()
            .nth(1)
            .and_then(|x| x.parse().ok())
            .expect("status");
        let body = out.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or("");
        (status, serde_json::from_str(body).unwrap_or(Value::Null))
    }
    fn post(&self, who: &str, path: &str, body: &Value) -> (u16, Value) {
        self.call(Some(who), "POST", path, None, Some(body))
    }
    fn post_k(&self, who: &str, path: &str, key: &str, body: &Value) -> (u16, Value) {
        self.call(Some(who), "POST", path, Some(key), Some(body))
    }
    fn get(&self, who: &str, path: &str) -> (u16, Value) {
        self.call(Some(who), "GET", path, None, None)
    }
    fn db(&self) -> Client {
        self.cfg.connect(NoTls).expect("db")
    }
    /// Row counts of every table: a refused request must leave all of them unchanged.
    fn counts(&self) -> Vec<(String, i64)> {
        let mut db = self.db();
        [
            "sprint_revisions",
            "semantic_reviews",
            "idempotency",
            "assignments",
            "assignment_heads",
            "task_attempts",
            "events",
            "repro_requests",
            "evidence",
            "judgments",
            "outbox",
            "history",
        ]
        .iter()
        .map(|t| {
            let n: i64 = db
                .query_one(&format!("SELECT count(*) FROM {t}"), &[])
                .expect("count")
                .get(0);
            (t.to_string(), n)
        })
        .collect()
    }
}

fn code(v: &Value) -> String {
    v["code"]
        .as_str()
        .or_else(|| v["rejections"][0]["code"].as_str())
        .unwrap_or_default()
        .to_string()
}

fn spec(repo: &str, revision: u64, ceiling: u64) -> Value {
    let m: Value = serde_json::from_str(MANIFEST).expect("manifest");
    let mut s = m["sprints"][0].clone();
    s["registration_state"] = json!("ready");
    s["target"] =
        json!({"repo_id": repo, "repository_url": format!("https://example.invalid/{repo}")});
    s["limits"] = json!({"elapsed_seconds": 3600, "total_tokens": 100000, "correction_attempts": ceiling,
                         "unresolved_reason": null});
    s["unresolved_bindings"] = json!([]);
    s["revision"] = json!(revision);
    // 01-01 also carries an optional output (excluded from verdicts, policy G7)
    s["tasks"][0]["outputs"] = json!([
        {"artifact_id": "01-01-output", "description": "required output", "required": true},
        {"artifact_id": "01-01-notes", "description": "optional notes", "required": false}]);
    s["digest"] = json!(ieo_spec::spec_digest(&s));
    s
}

/// Register POC-01 executable for repo-a and pass its semantic review (bob ≠ alice).
fn ready_sprint(e: &Env, ceiling: u64) -> Value {
    let s = spec("repo-a", 5, ceiling);
    let (st, r) = e.post_k("alice", "/v0/sprints?mode=executable", "reg-key-0001", &s);
    assert_eq!(st, 201, "{r}");
    let (st, r) = e.post(
        "bob",
        "/v0/sprints/POC-01/revisions/5/semantic-review",
        &json!({"decision": "passed", "reason": "scope matches the plan", "digest": s["digest"]}),
    );
    assert_eq!(st, 201, "{r}");
    s
}

fn assign(
    e: &Env,
    role: &str,
    aid: &str,
    generation: u64,
    task: &str,
    sha: &str,
    expected: Value,
) -> (u16, Value) {
    e.post(
        "assigner",
        "/v0/coordinator/assignments",
        &json!({"repo_id": "repo-a", "role": role, "assignment_id": aid, "generation": generation,
                   "sprint_id": "POC-01", "sprint_revision": 5, "task_id": task, "target_sha": sha,
                   "expected_current": expected}),
    )
}

fn report(role: &str, event_id: &str, aid: &str, generation: u64, task: &str, sha: &str) -> Value {
    let name = format!(
        "contract_examples/{}.json",
        if role == "development" {
            "development_progress"
        } else {
            "guardrail_review"
        }
    );
    let raw = match name.as_str() {
        "contract_examples/development_progress.json" => {
            include_str!("../../../docs/poc/reports/contract_examples/development_progress.json")
        }
        _ => include_str!("../../../docs/poc/reports/contract_examples/guardrail_review.json"),
    };
    let mut r: Value = serde_json::from_str(raw).expect("example");
    r["event_id"] = json!(event_id);
    r["event_revision"] = json!(1);
    r["sprint_id"] = json!("POC-01");
    r["sprint_revision"] = json!(5);
    r["task_id"] = json!(task);
    r["repo_id"] = json!("repo-a");
    r["assignment_id"] = json!(aid);
    r["generation"] = json!(generation);
    r["actor"]["role"] = json!(role);
    r["body"]["target_sha"] = json!(sha);
    if role == "guardrail" {
        r["body"]["verdict"] = json!("approve");
        r["body"]["findings"] = json!([]);
    }
    r
}

fn evidence(e: &Env, id: &str, kind: &str, extra: Value) -> (u16, Value) {
    let mut b = json!({"evidence_id": id, "repo_id": "repo-a", "kind": kind});
    if let (Some(m), Some(x)) = (b.as_object_mut(), extra.as_object()) {
        m.extend(x.clone());
    }
    e.post("ingest", "/v0/coordinator/evidence", &b)
}

/// All GitHub/run evidence that makes 01-01 at `sha` complete.
fn full_evidence(e: &Env, sha: &str, tag: &str) {
    for (id, kind, extra) in [
        (
            "ci",
            "ci_run",
            json!({"sha": sha, "conclusion": "success", "source_ref": "actions/run/1"}),
        ),
        (
            "merge",
            "merge",
            json!({"head_sha": sha, "merged_sha": MERGE, "source_ref": "pull/1"}),
        ),
        (
            "main",
            "main_ci",
            json!({"sha": MERGE, "conclusion": "success", "source_ref": "actions/run/2"}),
        ),
        (
            "out",
            "output",
            json!({"sha": sha, "task_id": "01-01", "artifact_id": "01-01-output", "ref": "docs/x.md"}),
        ),
        (
            "fu",
            "followups",
            json!({"sha": MERGE, "task_id": "01-01", "accepted": true, "ref": "ledger"}),
        ),
    ] {
        let (st, r) = evidence(e, &format!("{tag}-{id}"), kind, extra);
        assert_eq!(st, 201, "{r}");
    }
}

fn judge(e: &Env, jid: &str, task: &str) -> (u16, Value) {
    e.post(
        "judge",
        "/v0/coordinator/judgments",
        &json!({"judgment_id": jid, "repo_id": "repo-a", "sprint_id": "POC-01", "task_id": task}),
    )
}

// ---------------------------------------------------------------------------

#[test]
fn health_is_liveness_only_and_needs_no_credential() {
    let Some(e) = start() else { return };
    let (st, b) = e.call(None, "GET", "/v0/health", None, None);
    assert_eq!(st, 200);
    assert_eq!(b["status"], "up");
    assert!(
        b["scope"]
            .as_str()
            .unwrap_or_default()
            .contains("not a P04 result")
    );
    assert_eq!(b["policy_digest"], policy::PINNED_POLICY_DIGEST);
}

#[test]
fn authentication_and_scope_refusals_change_nothing() {
    let Some(e) = start() else { return };
    let s = spec("repo-a", 5, 1);
    let before = e.counts();
    assert_eq!(
        e.call(None, "POST", "/v0/sprints", Some("k-00000001"), Some(&s))
            .0,
        401
    );
    assert_eq!(
        e.call(
            Some("not-a-token"),
            "GET",
            "/v0/sprints/POC-01/state",
            None,
            None
        )
        .0,
        401
    );
    let (st, b) = e.post_k("dev-a", "/v0/sprints", "k-00000002", &s);
    assert_eq!((st, code(&b)), (403, "AGENT_READ_DENIED".into()));
    let (st, b) = e.get("gr-a", "/v0/sprints/POC-01/state");
    assert_eq!((st, code(&b)), (403, "AGENT_READ_DENIED".into()));
    let (st, b) = e.post_k("bob", "/v0/sprints", "k-00000003", &s);
    assert_eq!((st, code(&b)), (403, "SCOPE_DENIED".into()));
    let (st, b) = e.post_k("carol", "/v0/sprints", "k-00000004", &s);
    assert_eq!((st, code(&b)), (403, "REPO_SCOPE_DENIED".into()));
    let (st, b) = e.post("dev-a", "/v0/coordinator/judgments", &json!({}));
    assert_eq!((st, code(&b)), (403, "SCOPE_DENIED".into()));
    assert_eq!(e.counts(), before, "refusals leave every table unchanged");
}

#[test]
fn registration_is_format_intake_and_semantic_review_is_separate() {
    let Some(e) = start() else { return };
    let s = spec("repo-a", 5, 1);
    let (st, r) = e.post_k("alice", "/v0/sprints?mode=executable", "reg-key-0001", &s);
    assert_eq!(st, 201, "{r}");
    assert_eq!(r["state"], "ACCEPTED_FOR_SEMANTIC_REVIEW");
    // idempotent replay and key conflict
    assert_eq!(
        e.post_k("alice", "/v0/sprints?mode=executable", "reg-key-0001", &s)
            .0,
        200
    );
    let (st, b) = e.post_k("alice", "/v0/sprints?mode=draft", "reg-key-0001", &s);
    assert_eq!((st, code(&b)), (409, "IDEMPOTENCY_KEY_CONFLICT".into()));
    let (st, rev) = e.get("alice", "/v0/sprints/POC-01/revisions/5");
    assert_eq!(st, 200);
    assert_eq!(rev["review_state"], "pending_semantic_review");
    assert_eq!(rev["digest"], s["digest"]);
    // a sprint of another repo is not visible
    assert_eq!(e.get("carol", "/v0/sprints/POC-01/revisions/5").0, 404);
    // format-valid but not semantically reviewed → no assignment
    let (st, b) = assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null);
    assert_eq!((st, code(&b)), (422, "SEMANTIC_REVIEW_NOT_PASSED".into()));
    // the registrant cannot review its own registration; a review names the exact digest
    let (st, b) = e.post(
        "carol",
        "/v0/sprints/POC-01/revisions/5/semantic-review",
        &json!({"decision": "passed", "reason": "x", "digest": s["digest"]}),
    );
    assert_eq!(st, 404, "carol's repo scope does not include repo-a: {b}");
    let wrong = json!({"decision": "passed", "reason": "ok", "digest": "0".repeat(64)});
    let (st, b) = e.post(
        "bob",
        "/v0/sprints/POC-01/revisions/5/semantic-review",
        &wrong,
    );
    assert_eq!((st, code(&b)), (409, "DIGEST_MISMATCH".into()));
    let good = json!({"decision": "passed", "reason": "ok", "digest": s["digest"]});
    assert_eq!(
        e.post(
            "bob",
            "/v0/sprints/POC-01/revisions/5/semantic-review",
            &good
        )
        .0,
        201
    );
    assert_eq!(
        e.post(
            "bob",
            "/v0/sprints/POC-01/revisions/5/semantic-review",
            &good
        )
        .0,
        200
    );
    let other = json!({"decision": "rejected", "reason": "changed mind", "digest": s["digest"]});
    assert_eq!(
        e.post(
            "bob",
            "/v0/sprints/POC-01/revisions/5/semantic-review",
            &other
        )
        .0,
        409
    );
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
}

#[test]
fn self_review_is_refused() {
    let Some(e) = start() else { return };
    let s = spec("repo-b", 5, 1);
    assert_eq!(
        e.post_k("carol", "/v0/sprints?mode=executable", "reg-key-0002", &s)
            .0,
        201
    );
    let (st, b) = e.post(
        "carol",
        "/v0/sprints/POC-01/revisions/5/semantic-review",
        &json!({"decision": "passed", "reason": "mine", "digest": s["digest"]}),
    );
    assert_eq!((st, code(&b)), (403, "SELF_REVIEW_DENIED".into()));
}

#[test]
fn draft_and_superseded_revisions_are_not_assignable() {
    let Some(e) = start() else { return };
    let s5 = spec("repo-a", 5, 1);
    assert_eq!(e.post_k("alice", "/v0/sprints", "reg-key-d001", &s5).0, 201);
    assert_eq!(
        e.post(
            "bob",
            "/v0/sprints/POC-01/revisions/5/semantic-review",
            &json!({"decision": "passed", "reason": "ok", "digest": s5["digest"]})
        )
        .0,
        201
    );
    let (st, b) = assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null);
    assert_eq!((st, code(&b)), (422, "UNBOUND_DRAFT".into()));
    let s6 = spec("repo-a", 6, 1);
    assert_eq!(
        e.post_k("alice", "/v0/sprints?mode=executable", "reg-key-d002", &s6)
            .0,
        201
    );
    // old revision: promotion refused, assignment refused, replay is historical
    let (st, b) = e.post_k("alice", "/v0/sprints?mode=executable", "reg-key-d003", &s5);
    assert_eq!((st, code(&b)), (409, "REVISION_SUPERSEDED".into()));
    let (st, b) = assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null);
    assert_eq!((st, code(&b)), (409, "REVISION_SUPERSEDED".into()));
    let (st, b) = e.post_k("alice", "/v0/sprints", "reg-key-d001", &s5);
    assert_eq!(
        (st, b["state"].clone(), b["superseded_by"].clone()),
        (200, json!("SUPERSEDED"), json!(6))
    );
}

#[test]
fn ownership_head_is_compare_and_swap() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 3);
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    // replay: historical answer, no change
    let (st, b) = assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null);
    assert_eq!(
        (st, b["outcome"].clone(), b["is_current"].clone()),
        (200, json!("already_issued"), json!(true))
    );
    // a second issuer that did not see A1 loses
    let (st, b) = assign(&e, "development", "B1", 0, "01-01", SHA2, Value::Null);
    assert_eq!((st, code(&b)), (409, "OWNERSHIP_CONFLICT".into()));
    // generations must follow the head directly
    let (st, b) = assign(
        &e,
        "development",
        "A1",
        2,
        "01-01",
        SHA2,
        json!({"assignment_id": "A1", "generation": 0}),
    );
    assert_eq!((st, code(&b)), (409, "OWNERSHIP_CONFLICT".into()));
    let (st, b) = assign(
        &e,
        "development",
        "A1",
        1,
        "01-02",
        SHA2,
        json!({"assignment_id": "A1", "generation": 0}),
    );
    assert_eq!(
        (st, code(&b)),
        (422, "REPORT_SEMANTIC_VIOLATION".into()),
        "a generation keeps its task"
    );
    assert_eq!(
        assign(
            &e,
            "development",
            "A1",
            1,
            "01-01",
            SHA2,
            json!({"assignment_id": "A1", "generation": 0})
        )
        .0,
        201
    );
    // an old (assignment, generation) replay never reactivates it
    let (st, b) = assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null);
    assert_eq!((st, b["is_current"].clone()), (200, json!(false)));
    let head: (String, i64) = {
        let mut db = e.db();
        let r = db.query_one("SELECT assignment_id, generation FROM assignment_heads WHERE repo_id='repo-a' AND role='development'", &[]).expect("head");
        (r.get(0), r.get(1))
    };
    assert_eq!(head, ("A1".to_string(), 1));
    // a used assignment_id cannot restart at generation 0
    let (st, b) = assign(
        &e,
        "development",
        "A1",
        0,
        "01-01",
        SHA3,
        json!({"assignment_id": "A1", "generation": 1}),
    );
    assert_eq!((st, code(&b)), (409, "EVENT_CONTENT_CONFLICT".into()));
}

#[test]
fn prerequisites_need_a_verified_accepted_verdict() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 2);
    let (st, b) = assign(&e, "development", "A2", 0, "01-02", SHA1, Value::Null);
    assert_eq!((st, code(&b)), (409, "PREREQUISITE_UNMET".into()), "{b}");
    // a claim of completion does not satisfy the prerequisite
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    let mut done = report("development", "dev-done-1", "A1", 0, "01-01", SHA1);
    done["operational_state"] = json!("완료");
    assert_eq!(e.post("dev-a", "/v0/agent/reports", &done).0, 202);
    let (st, b) = assign(
        &e,
        "development",
        "A2",
        0,
        "01-02",
        SHA2,
        json!({"assignment_id": "A1", "generation": 0}),
    );
    assert_eq!((st, code(&b)), (409, "PREREQUISITE_UNMET".into()));
    // a not_accepted verdict does not either
    let (st, v) = judge(&e, "j-early", "01-01");
    assert_eq!(
        (st, v["outcome"].clone()),
        (201, json!("not_accepted")),
        "{v}"
    );
    assert_eq!(
        assign(
            &e,
            "development",
            "A2",
            0,
            "01-02",
            SHA2,
            json!({"assignment_id": "A1", "generation": 0})
        )
        .0,
        409
    );
    // evidence + current guardrail approval → accepted → prerequisite met
    assert_eq!(
        assign(&e, "guardrail", "G1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    assert_eq!(
        e.post(
            "gr-a",
            "/v0/agent/reports",
            &report("guardrail", "gr-ok-1", "G1", 0, "01-01", SHA1)
        )
        .0,
        202
    );
    full_evidence(&e, SHA1, "p");
    let (st, v) = judge(&e, "j-final", "01-01");
    assert_eq!((st, v["outcome"].clone()), (201, json!("accepted")), "{v}");
    assert_eq!(
        assign(
            &e,
            "development",
            "A2",
            0,
            "01-02",
            SHA2,
            json!({"assignment_id": "A1", "generation": 0})
        )
        .0,
        201
    );
}

#[test]
fn claims_late_duplicate_stale_and_mismatched() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 3);
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    let c1 = report("development", "dev-1", "A1", 0, "01-01", SHA1);
    let (st, r) = e.post("dev-a", "/v0/agent/reports", &c1);
    assert_eq!(
        (
            st,
            r["disposition"].clone(),
            r["counts_for_current_judgment"].clone()
        ),
        (202, json!("current"), json!(true)),
        "{r}"
    );
    // duplicate: same content → 200, other content → 409
    assert_eq!(e.post("dev-a", "/v0/agent/reports", &c1).0, 200);
    let mut c1x = c1.clone();
    c1x["next_action"] = json!("different");
    assert_eq!(
        code(&e.post("dev-a", "/v0/agent/reports", &c1x).1),
        "EVENT_CONTENT_CONFLICT"
    );
    // generation bump
    assert_eq!(
        assign(
            &e,
            "development",
            "A1",
            1,
            "01-01",
            SHA2,
            json!({"assignment_id": "A1", "generation": 0})
        )
        .0,
        201
    );
    // replay of the earlier current claim: 200, re-evaluated → no longer counts (PR #10 N3)
    let (st, r) = e.post("dev-a", "/v0/agent/reports", &c1);
    assert_eq!(
        (
            st,
            r["disposition"].clone(),
            r["counts_for_current_judgment"].clone()
        ),
        (200, json!("current"), json!(false))
    );
    // a late claim for gen 0 is preserved with gen 0's SHA and excluded
    let late = report("development", "dev-late", "A1", 0, "01-01", SHA1);
    let (st, r) = e.post("dev-a", "/v0/agent/reports", &late);
    assert_eq!(
        (
            st,
            r["disposition"].clone(),
            r["attributed_target_sha"].clone(),
            r["counts_for_current_judgment"].clone()
        ),
        (202, json!("preserved_late"), json!(SHA1), json!(false))
    );
    let before = e.counts();
    // never issued → 409; SHA of another generation → 409; wrong task → 422; other repo → 403
    assert_eq!(
        code(
            &e.post(
                "dev-a",
                "/v0/agent/reports",
                &report("development", "dev-x", "ZZ", 0, "01-01", SHA1)
            )
            .1
        ),
        "STALE_ASSIGNMENT"
    );
    assert_eq!(
        code(
            &e.post(
                "dev-a",
                "/v0/agent/reports",
                &report("development", "dev-y", "A1", 1, "01-01", SHA1)
            )
            .1
        ),
        "ASSIGNMENT_SHA_MISMATCH"
    );
    let (st, b) = e.post(
        "dev-a",
        "/v0/agent/reports",
        &report("development", "dev-z", "A1", 1, "01-02", SHA2),
    );
    assert_eq!((st, code(&b)), (422, "REPORT_SEMANTIC_VIOLATION".into()));
    let (st, b) = e.post(
        "dev-b",
        "/v0/agent/reports",
        &report("development", "dev-w", "A1", 1, "01-01", SHA2),
    );
    assert_eq!((st, code(&b)), (403, "REPO_SCOPE_DENIED".into()));
    let mut verified = report("development", "dev-v", "A1", 1, "01-01", SHA2);
    verified["evidence_level"] = json!("verified");
    assert_eq!(
        code(&e.post("dev-a", "/v0/agent/reports", &verified).1),
        "VERIFIED_FROM_AGENT_DENIED"
    );
    let gr_as_dev = report("guardrail", "gr-as-dev", "A1", 1, "01-01", SHA2);
    assert_eq!(
        code(&e.post("dev-a", "/v0/agent/reports", &gr_as_dev).1),
        "ROLE_MISMATCH"
    );
    let mut bad = report("development", "dev-bad", "A1", 1, "01-01", SHA2);
    bad["phase"] = json!("없는 단계");
    assert_eq!(e.post("dev-a", "/v0/agent/reports", &bad).0, 400);
    assert_eq!(
        e.counts(),
        before,
        "refused claims leave every table unchanged"
    );
    // read model: the late claim is history, not current state
    let (_, st) = e.get("alice", "/v0/sprints/POC-01/state");
    let ids: Vec<&str> = st["latest_claims"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["event_id"].as_str())
        .collect();
    assert_eq!(ids, vec!["dev-1"], "{st}");
    assert_eq!(st["verified"], json!([]));
}

#[test]
fn repro_requests_new_must_be_current_replays_are_not_requeued() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 3);
    assert_eq!(
        assign(&e, "guardrail", "G1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    let req = |id: &str, generation: u64, sha: &str| {
        json!({"request_id": id, "repo_id": "repo-a", "assignment_id": "G1",
        "generation": generation, "target_sha": sha, "bundle_digest": "a".repeat(64), "command": ["cargo", "test"],
        "expected": "test fails on the bug"})
    };
    let (st, r) = e.post(
        "gr-a",
        "/v0/agent/repro-requests",
        &req("repro-0001", 0, SHA1),
    );
    assert_eq!((st, r["state"].clone()), (202, json!("queued")));
    assert_eq!(
        assign(
            &e,
            "guardrail",
            "G1",
            1,
            "01-01",
            SHA2,
            json!({"assignment_id": "G1", "generation": 0})
        )
        .0,
        201
    );
    let before = e.counts();
    assert_eq!(
        e.post(
            "gr-a",
            "/v0/agent/repro-requests",
            &req("repro-0001", 0, SHA1)
        )
        .0,
        200
    );
    assert_eq!(
        code(
            &e.post(
                "gr-a",
                "/v0/agent/repro-requests",
                &req("repro-0002", 0, SHA1)
            )
            .1
        ),
        "STALE_ASSIGNMENT"
    );
    assert_eq!(
        code(
            &e.post(
                "gr-a",
                "/v0/agent/repro-requests",
                &req("repro-0003", 1, SHA1)
            )
            .1
        ),
        "STALE_ASSIGNMENT"
    );
    assert_eq!(
        e.post(
            "dev-a",
            "/v0/agent/repro-requests",
            &req("repro-0004", 1, SHA2)
        )
        .0,
        403
    );
    assert_eq!(e.counts(), before);
    assert_eq!(
        e.post(
            "gr-a",
            "/v0/agent/repro-requests",
            &req("repro-0005", 1, SHA2)
        )
        .0,
        202
    );
    assert_eq!(e.get("gr-a", "/v0/agent/repro-requests/repro-0001").0, 200);
    assert_eq!(e.get("gr-b", "/v0/agent/repro-requests/repro-0001").0, 404);
    assert_eq!(e.get("dev-a", "/v0/agent/repro-requests/repro-0001").0, 403);
    assert_eq!(e.get("alice", "/v0/agent/repro-requests/repro-0001").0, 403);
}

#[test]
fn verdict_uses_github_and_run_evidence_for_the_recorded_sha_only() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 3);
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    assert_eq!(
        assign(&e, "guardrail", "G1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    // evidence for another SHA never fills the head's scopes (G2)
    full_evidence(&e, SHA2, "other");
    assert_eq!(
        e.post(
            "gr-a",
            "/v0/agent/reports",
            &report("guardrail", "gr-1", "G1", 0, "01-01", SHA1)
        )
        .0,
        202
    );
    let (_, v) = judge(&e, "j1", "01-01");
    assert_eq!(v["outcome"], "not_accepted");
    assert_eq!(v["facts"]["ci_passed_on_head"], false);
    assert_eq!(v["facts"]["merged_sha"], Value::Null);
    // a failing collector run at the head blocks CI even with a green check (02-01 judgment)
    full_evidence(&e, SHA1, "head");
    let rec = json!({"format": "ieo-evidence-record-v0", "judgment": {"expected": "success", "status": "failed",
                     "reasons": ["exit 1"], "counts_as_success": false}});
    assert_eq!(
        evidence(
            &e,
            "run-1",
            "run_record",
            json!({"sha": SHA1, "source_ref": "collector/run-1", "record": rec})
        )
        .0,
        201
    );
    let (_, v) = judge(&e, "j2", "01-01");
    assert_eq!(
        (
            v["outcome"].clone(),
            v["facts"]["ci_passed_on_head"].clone()
        ),
        (json!("not_accepted"), json!(false))
    );
    let ok_rec = json!({"format": "ieo-evidence-record-v0", "judgment": {"expected": "success", "status": "succeeded",
                        "reasons": [], "counts_as_success": true}});
    assert_eq!(
        evidence(
            &e,
            "run-2",
            "run_record",
            json!({"sha": SHA1, "source_ref": "collector/run-1", "record": ok_rec})
        )
        .0,
        201
    );
    let (st, v) = judge(&e, "j3", "01-01");
    assert_eq!((st, v["outcome"].clone()), (201, json!("accepted")), "{v}");
    // G7: the optional output is listed as not judged
    assert!(
        v["facts"]["excluded_not_judged"]
            .to_string()
            .contains("01-01-notes"),
        "{v}"
    );
    // the verdict, its outbox entry and the judgment row exist together
    let mut db = e.db();
    let n: i64 = db.query_one("SELECT count(*) FROM judgments j JOIN outbox o ON o.sprint_id = j.event_sprint AND o.sprint_seq = j.event_seq WHERE j.judgment_id = 'j3'", &[]).expect("q").get(0);
    assert_eq!(n, 1);
    // judgment replay and conflict
    assert_eq!(judge(&e, "j3", "01-01").0, 200);
    assert_eq!(code(&judge(&e, "j3", "01-02").1), "EVENT_CONTENT_CONFLICT");
    // read model: verified separate from claims; delivery follows the outbox
    let (_, st) = e.get("alice", "/v0/sprints/POC-01/state");
    assert_eq!(st["verified"].as_array().map(Vec::len), Some(1));
    assert_eq!(st["verified"][0]["evidence_level"], "verified");
    assert_eq!(st["delivery"], "pending");
    let (_, ob) = e.get("courier", "/v0/coordinator/outbox?state=pending");
    // the entry of the latest verdict (j3) is the last one
    let id = ob["items"]
        .as_array()
        .and_then(|a| a.last())
        .and_then(|i| i["outbox_id"].as_i64())
        .unwrap_or_default();
    assert_eq!(
        e.post(
            "courier",
            &format!("/v0/coordinator/outbox/{id}/delivery"),
            &json!({"state": "sent"})
        )
        .0,
        200
    );
    assert_eq!(
        code(
            &e.post(
                "courier",
                &format!("/v0/coordinator/outbox/{id}/delivery"),
                &json!({"state": "unknown"})
            )
            .1
        ),
        "OUTBOX_STATE_CONFLICT"
    );
    assert_eq!(
        e.get("alice", "/v0/sprints/POC-01/state").1["delivery"],
        "sent"
    );
}

#[test]
fn review_inputs_are_current_independent_and_for_the_head() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 3);
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    assert_eq!(
        assign(&e, "guardrail", "G1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    full_evidence(&e, SHA1, "h");
    // approval by gen 0 of the guardrail, then the guardrail moves to gen 1 (same SHA):
    // the gen-0 approval is no longer a current judgment input (PR #10 N4)
    assert_eq!(
        e.post(
            "gr-a",
            "/v0/agent/reports",
            &report("guardrail", "gr-g0", "G1", 0, "01-01", SHA1)
        )
        .0,
        202
    );
    assert_eq!(judge(&e, "jA", "01-01").1["outcome"], "accepted");
    assert_eq!(
        assign(
            &e,
            "guardrail",
            "G1",
            1,
            "01-01",
            SHA1,
            json!({"assignment_id": "G1", "generation": 0})
        )
        .0,
        201
    );
    let (_, v) = judge(&e, "jB", "01-01");
    assert_eq!(v["outcome"], "not_accepted", "{v}");
    assert!(
        v["summary"]
            .as_str()
            .unwrap_or_default()
            .contains("미완료: 현재 SHA 1111111 독립 검토 승인"),
        "{v}"
    );
    // 01-08 N2: the confirmed merge is still shown although the approval is missing
    assert!(
        v["summary"]
            .as_str()
            .unwrap_or_default()
            .contains("병합 4444444"),
        "{v}"
    );
    // a GitHub self-review does not count; an independent one does
    assert_eq!(evidence(&e, "rv-self", "review", json!({"sha": SHA1, "reviewer": "dev", "author": "dev", "state": "approved", "source_ref": "pull/1#r1"})).0, 201);
    assert_eq!(judge(&e, "jC", "01-01").1["outcome"], "not_accepted");
    assert_eq!(evidence(&e, "rv-ind", "review", json!({"sha": SHA1, "reviewer": "rev", "author": "dev", "state": "approved", "source_ref": "pull/1#r2"})).0, 201);
    assert_eq!(judge(&e, "jD", "01-01").1["outcome"], "accepted");
    // a current reject at the head blocks the approval
    let mut rej = report("guardrail", "gr-g1-reject", "G1", 1, "01-01", SHA1);
    rej["body"]["verdict"] = json!("reject");
    rej["body"]["findings"] = json!([{"finding_id": "F1", "violated_condition": "acceptance",
        "reproduction": {"command": "cargo test", "input": "case 1"}, "expected": "pass", "actual": "fail",
        "severity": "P2", "confidence": "reproduced", "original_log_ref": "logs/1.txt"}]);
    assert_eq!(e.post("gr-a", "/v0/agent/reports", &rej).0, 202);
    assert_eq!(judge(&e, "jE", "01-01").1["outcome"], "not_accepted");
    // approval of an older SHA is not an approval of the head
    assert_eq!(
        assign(
            &e,
            "development",
            "A1",
            1,
            "01-01",
            SHA2,
            json!({"assignment_id": "A1", "generation": 0})
        )
        .0,
        201
    );
    let (_, v) = judge(&e, "jF", "01-01");
    assert_eq!(
        (v["outcome"].clone(), v["head_sha"].clone()),
        (json!("not_accepted"), json!(SHA2))
    );
}

#[test]
fn attempt_ceiling_is_cumulative_and_records_one_decision_request() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 1);
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    assert_eq!(
        assign(
            &e,
            "development",
            "A1",
            1,
            "01-01",
            SHA2,
            json!({"assignment_id": "A1", "generation": 0})
        )
        .0,
        201
    );
    // a fresh assignment_id does not reset the count (G4)
    let (st, b) = assign(
        &e,
        "development",
        "B1",
        0,
        "01-01",
        SHA3,
        json!({"assignment_id": "A1", "generation": 1}),
    );
    assert_eq!(
        (st, b["outcome"].clone()),
        (202, json!("limit_reached")),
        "{b}"
    );
    let before = e.counts();
    let (st, b) = assign(
        &e,
        "development",
        "A1",
        2,
        "01-01",
        SHA3,
        json!({"assignment_id": "A1", "generation": 1}),
    );
    assert_eq!((st, code(&b)), (409, "LIMIT_REACHED".into()));
    assert_eq!(e.counts(), before, "the decision request is recorded once");
    // the owner keeps the assignment; the decision request is verified and in the outbox
    let mut db = e.db();
    let head: String = db
        .query_one(
            "SELECT assignment_id FROM assignment_heads WHERE role = 'development'",
            &[],
        )
        .expect("h")
        .get(0);
    assert_eq!(head, "A1");
    let (_, st) = e.get("alice", "/v0/sprints/POC-01/state");
    assert_eq!(st["verified"][0]["report_type"], "decision_request", "{st}");
    assert_eq!(st["verified"][0]["decision"]["default_action"], "정지 유지");
    assert_eq!(st["delivery"], "pending");
    // other roles count separately
    assert_eq!(
        assign(&e, "guardrail", "G1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
}

#[test]
fn event_pages_are_append_only_and_reads_change_nothing() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 3);
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    for i in 0..5 {
        assert_eq!(
            e.post(
                "dev-a",
                "/v0/agent/reports",
                &report("development", &format!("ev-{i}"), "A1", 0, "01-01", SHA1)
            )
            .0,
            202
        );
    }
    let before = e.counts();
    let (st, p1) = e.get("alice", "/v0/sprints/POC-01/events?limit=2");
    assert_eq!(
        (st, p1["has_more"].clone(), p1["next_cursor"].clone()),
        (200, json!(true), json!("s2"))
    );
    let (_, p2) = e.get("alice", "/v0/sprints/POC-01/events?limit=2&cursor=s2");
    assert_eq!(p2["events"][0]["event_id"], "ev-2");
    let (_, p3) = e.get("alice", "/v0/sprints/POC-01/events?limit=2&cursor=s4");
    assert_eq!(
        (
            p3["events"].as_array().map(Vec::len),
            p3["has_more"].clone()
        ),
        (Some(1), json!(false))
    );
    let (_, p4) = e.get("alice", "/v0/sprints/POC-01/events?cursor=s5");
    assert_eq!(
        (p4["events"].clone(), p4["next_cursor"].clone()),
        (json!([]), json!("s5"))
    );
    for bad in ["s9", "x1", "s-1", "s01"] {
        let (st, b) = e.get("alice", &format!("/v0/sprints/POC-01/events?cursor={bad}"));
        assert_eq!((st, code(&b)), (400, "INVALID_CURSOR".into()), "{bad}");
    }
    assert_eq!(e.get("alice", "/v0/sprints/POC-01/events?limit=0").0, 400);
    assert_eq!(e.counts(), before, "reads create no rows");
    // history tables refuse UPDATE/DELETE at the database level
    let mut db = e.db();
    assert!(
        db.execute("UPDATE events SET report = '{}'::jsonb", &[])
            .is_err()
    );
    assert!(db.execute("DELETE FROM history", &[]).is_err());
    assert!(db.execute("DELETE FROM assignments", &[]).is_err());
}

#[test]
fn concurrent_duplicates_and_competing_issuers() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 5);
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    let e = Arc::new(e);
    // 8 identical claims at once: one store, seven replays, one row
    let c = report("development", "dup-1", "A1", 0, "01-01", SHA1);
    let hs: Vec<_> = (0..8)
        .map(|_| {
            let (e, c) = (Arc::clone(&e), c.clone());
            std::thread::spawn(move || e.post("dev-a", "/v0/agent/reports", &c).0)
        })
        .collect();
    let mut st: Vec<u16> = hs.into_iter().map(|h| h.join().unwrap_or(0)).collect();
    st.sort_unstable();
    assert_eq!(st, vec![200, 200, 200, 200, 200, 200, 200, 202]);
    // 8 issuers racing for generation 1 of the same head: exactly one wins
    let hs: Vec<_> = (0..8)
        .map(|i| {
            let e = Arc::clone(&e);
            std::thread::spawn(move || {
                let sha = format!("{:040x}", 0xabc0 + i);
                assign(
                    &e,
                    "development",
                    "A1",
                    1,
                    "01-01",
                    &sha,
                    json!({"assignment_id": "A1", "generation": 0}),
                )
                .0
            })
        })
        .collect();
    let mut st: Vec<u16> = hs.into_iter().map(|h| h.join().unwrap_or(0)).collect();
    st.sort_unstable();
    assert_eq!(st, vec![201, 409, 409, 409, 409, 409, 409, 409]);
    // concurrent registrations of the same new revision with different keys: one 201
    let s7 = spec("repo-a", 7, 5);
    let hs: Vec<_> = (0..6)
        .map(|i| {
            let (e, s) = (Arc::clone(&e), s7.clone());
            std::thread::spawn(move || {
                e.post_k(
                    "alice",
                    "/v0/sprints?mode=executable",
                    &format!("race-key-{i:04}"),
                    &s,
                )
                .0
            })
        })
        .collect();
    let mut st: Vec<u16> = hs.into_iter().map(|h| h.join().unwrap_or(0)).collect();
    st.sort_unstable();
    assert_eq!(st, vec![200, 200, 200, 200, 200, 201]);
    let mut db = e.db();
    let n: i64 = db
        .query_one("SELECT count(*) FROM events WHERE event_id = 'dup-1'", &[])
        .expect("q")
        .get(0);
    assert_eq!(n, 1);
    let seqs: Vec<i64> = db
        .query(
            "SELECT sprint_seq FROM events WHERE sprint_id = 'POC-01' ORDER BY sprint_seq",
            &[],
        )
        .expect("q")
        .iter()
        .map(|r| r.get(0))
        .collect();
    assert_eq!(
        seqs,
        (1..=i64::try_from(seqs.len()).unwrap_or_default()).collect::<Vec<_>>(),
        "gap-free per-sprint order"
    );
}

#[test]
fn independent_repos_progress_separately() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 3);
    // repo-b's agent cannot touch repo-a's assignment; repo-a's head is unaffected
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    let (st, _) = e.post(
        "dev-b",
        "/v0/agent/reports",
        &report("development", "x-1", "A1", 0, "01-01", SHA1),
    );
    assert_eq!(st, 403);
    let (st, b) = e.post(
        "assigner",
        "/v0/coordinator/assignments",
        &json!({"repo_id": "repo-b", "role": "development", "assignment_id": "BB", "generation": 0,
                "sprint_id": "POC-01", "sprint_revision": 5, "task_id": "01-01", "target_sha": SHA1,
                "expected_current": null}),
    );
    assert_eq!((st, code(&b)), (422, "REPO_MISMATCH".into()));
}

#[test]
fn human_input_conversion_needs_an_approved_binding() {
    let Some(e) = start() else { return };
    let input = json!({"title": "설명용 예시", "repo": "repo-a", "goal": "설명용 목표",
        "out_of_scope": ["새 기능"], "decisions": ["납기 변경 없음"],
        "deadline": {"state": "none", "at": null, "reason": "기한 없음"},
        "tasks": [{"name": "검사 준비", "output": "시험 입력", "done_when": "입력 확정", "how_to_check": "목록 대조"}]});
    let (st, b) = e.post_k("alice", "/v0/sprint-inputs", "conv-key-01", &input);
    assert_eq!(st, 201, "{b}");
    // the server applied the approved binding; the result is a draft awaiting review
    assert_eq!(b["registration"]["mode"], "draft");
    assert_eq!(b["registration"]["state"], "ACCEPTED_FOR_SEMANTIC_REVIEW");
    assert_eq!(b["conversion"]["spec"]["sprint_id"], "HUM-01");
    assert_eq!(b["conversion"]["spec"]["tasks"][0]["task_id"], "11-01");
    // out_of_scope is preserved, never turned into a judged item (G7)
    assert!(
        b["conversion"]["preserved_unrepresented"]
            .to_string()
            .contains("새 기능"),
        "{b}"
    );
    assert_eq!(
        e.post_k("alice", "/v0/sprint-inputs", "conv-key-01", &input)
            .0,
        200
    );
    // a draft from conversion is not assignable
    let (st, c) = e.post(
        "assigner",
        "/v0/coordinator/assignments",
        &json!({"repo_id": "repo-a", "role": "development", "assignment_id": "H1", "generation": 0,
                "sprint_id": "HUM-01", "sprint_revision": 1, "task_id": "11-01", "target_sha": SHA1,
                "expected_current": null}),
    );
    assert_eq!((st, code(&c)), (422, "UNBOUND_DRAFT".into()));
    let mut other = input.clone();
    other["repo"] = json!("repo-b");
    let (st, b) = e.post_k("carol", "/v0/sprint-inputs", "conv-key-02", &other);
    assert_eq!((st, code(&b)), (422, "UNBOUND_DRAFT".into()));
}

#[test]
fn policy_is_pinned_and_fails_closed() {
    assert_eq!(policy::digest_bytes(POLICY), policy::PINNED_POLICY_DIGEST);
    assert!(policy::load(POLICY).is_ok());
    let mut changed = POLICY.to_vec();
    changed.push(b'\n');
    assert!(policy::load(&changed).is_err());
    let v: Value = serde_json::from_slice(POLICY).unwrap_or(Value::Null);
    let ids: Vec<&str> = v["rules"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| r["rule_id"].as_str())
        .collect();
    assert_eq!(ids.len(), policy::IMPLEMENTED_RULES.len());
    assert!(
        v["numeric_values"]
            .as_str()
            .unwrap_or_default()
            .contains("sets no numeric limit")
    );
}

#[test]
fn credentials_refuse_inconsistent_principals() {
    let d = token_digest("t");
    let bad = [
        json!({"name": "a", "token_sha256": d, "kind": "agent", "scopes": ["agent:report"], "repo_ids": ["r1", "r2"], "role": "development"}),
        json!({"name": "a", "token_sha256": d, "kind": "agent", "scopes": ["sprints:read"], "repo_ids": ["r1"], "role": "development"}),
        json!({"name": "a", "token_sha256": d, "kind": "agent", "scopes": ["agent:repro"], "repo_ids": ["r1"], "role": "development"}),
        json!({"name": "a", "token_sha256": d, "kind": "user", "scopes": ["judge:verify"], "repo_ids": ["r1"]}),
        json!({"name": "a", "token_sha256": d, "kind": "user", "scopes": ["agent:report"], "repo_ids": ["r1"]}),
        json!({"name": "a", "token_sha256": "ABC", "kind": "user", "scopes": [], "repo_ids": []}),
    ];
    for p in bad {
        assert!(
            Credentials::from_json(&json!({"principals": [p]})).is_err(),
            "{p}"
        );
    }
    let c = Credentials::from_json(&json!({"principals": [
        {"name": "s", "token_sha256": d, "kind": "service", "scopes": ["judge:verify"], "repo_ids": ["r1"]}]}))
        .expect("valid");
    assert!(c.authenticate(Some("Bearer t")).is_some());
    assert!(c.authenticate(Some("Bearer u")).is_none());
    assert!(c.authenticate(Some("t")).is_none());
    assert!(c.authenticate(None).is_none());
}

#[test]
fn every_emitted_code_is_declared_in_the_openapi_contract() {
    let src = include_str!("../src/api.rs");
    let api = include_str!("../../../contracts/ieo-api-v0.openapi.yaml");
    let declared: std::collections::BTreeSet<&str> = api
        .split(|c: char| !(c.is_ascii_uppercase() || c == '_'))
        .filter(|w| w.len() >= 6)
        .collect();
    let emitted: std::collections::BTreeSet<&str> = src
        .split('"')
        .skip(1)
        .step_by(2)
        .filter(|w| w.len() >= 6 && w.chars().all(|c| c.is_ascii_uppercase() || c == '_'))
        .collect();
    assert!(emitted.len() >= 20, "{emitted:?}");
    let missing: Vec<&&str> = emitted.iter().filter(|c| !declared.contains(**c)).collect();
    assert!(missing.is_empty(), "codes not in OpenAPI: {missing:?}");
}
