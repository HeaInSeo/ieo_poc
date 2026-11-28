//! Shared helpers of the coordinator integration tests (moved from coordinator.rs for X-04, so
//! the HTTP·gRPC regression in grpc.rs runs the same environment). Not every test file uses all
//! of them.
#![allow(dead_code)]

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

pub const MANIFEST: &str = include_str!("../../../../docs/poc/Console_Coordinator_Sprints_v0.json");
pub const POLICY: &[u8] = include_bytes!("../../../../contracts/judgment-policy-v0.json");

pub const SHA1: &str = "1111111111111111111111111111111111111111";
pub const SHA2: &str = "2222222222222222222222222222222222222222";
pub const SHA3: &str = "3333333333333333333333333333333333333333";
pub const MERGE: &str = "4444444444444444444444444444444444444444";

pub static COUNTER: AtomicU64 = AtomicU64::new(0);

/// At most this many test environments (each: a server with DB workers + a database) run at
/// once, so the suite stays under PostgreSQL's default max_connections (100) at any size.
pub const MAX_ENVS: usize = 8;
pub static ENVS: (std::sync::Mutex<usize>, std::sync::Condvar) =
    (std::sync::Mutex::new(0), std::sync::Condvar::new());

pub struct Permit;

impl Permit {
    pub fn acquire() -> Self {
        let (m, cv) = &ENVS;
        let mut n = m.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        while *n >= MAX_ENVS {
            n = cv
                .wait(n)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        *n += 1;
        Permit
    }
}

impl Drop for Permit {
    fn drop(&mut self) {
        let (m, cv) = &ENVS;
        let mut n = m.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        *n -= 1;
        cv.notify_one();
    }
}

pub struct Env {
    pub _permit: Permit,
    pub addr: String,
    pub base: Option<Config>,
    pub dbname: String,
    pub cfg: Config,
    pub tokens: std::collections::BTreeMap<&'static str, String>,
    /// The server's credentials and context (the gRPC adapter of grpc.rs serves the same app).
    pub app: Arc<App>,
}

pub fn base_config() -> Option<Config> {
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
pub type PrincipalRow = (
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
    Option<&'static str>,
);

pub const PRINCIPALS: [PrincipalRow; 21] = [
    // role supervisors' checkpoint report credentials (02-04 decision item 9)
    (
        "sup-dev-a",
        "agent",
        &["checkpoint:report"],
        &["repo-a"],
        Some("development"),
    ),
    (
        "sup-gr-a",
        "agent",
        &["checkpoint:report"],
        &["repo-a"],
        Some("guardrail"),
    ),
    ("operator-norepo", "service", &["roles:control"], &[], None),
    ("operator", "service", &["roles:control"], &["repo-a"], None),
    (
        "importer",
        "service",
        &["notion:import", "sprints:read"],
        &["repo-a"],
        None,
    ),
    (
        "importer-b",
        "service",
        &["notion:import", "sprints:read"],
        &["repo-b"],
        None,
    ),
    (
        "uma",
        "user",
        &["exceptions:approve", "sprints:read"],
        &["repo-a"],
        None,
    ),
    (
        "erin",
        "user",
        &["sprints:register", "review:semantic", "sprints:read"],
        &["repo-a"],
        None,
    ),
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
        &["judge:verify", "github:merge"],
        &["repo-a", "repo-b"],
        None,
    ),
    (
        "dev-a-other",
        "agent",
        &["github:push"],
        &["repo-a"],
        Some("development"),
    ),
    (
        "watcher",
        "service",
        &["followups:reconcile"],
        &["repo-a"],
        None,
    ),
    ("courier", "service", &["outbox:deliver"], &["repo-a"], None),
    (
        "dev-a",
        "agent",
        &["agent:report", "github:push"],
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

pub type FixtureTokens = std::collections::BTreeMap<&'static str, String>;

pub fn fixture(cfg: &Config, suffix: &str) -> (App, FixtureTokens, Value) {
    let mut tokens = std::collections::BTreeMap::new();
    let mut list = Vec::new();
    for (name, kind, scopes, repos, role) in PRINCIPALS {
        let t = format!("tok-{name}-{suffix}");
        let mut p = json!({"name": name, "token_sha256": token_digest(&t), "kind": kind,
                           "scopes": scopes, "repo_ids": repos});
        if let Some(r) = role {
            p["role"] = json!(r);
        }
        list.push(p);
        tokens.insert(name, t);
    }
    let credentials = json!({ "principals": list });
    let creds = Credentials::from_json(&credentials).expect("creds");
    let mut manifest: Value = serde_json::from_str(MANIFEST).expect("manifest");
    // an approved test profile for the human-input binding of repo-a
    if let Some(p) = manifest["inspection_profiles"].as_array_mut() {
        p.push(json!({"profile_id": "hum-profile", "revision": 1, "items": {"11-01.accept": {}, "11-02.accept": {}, "11-03.accept": {}, "12-01.accept": {}}}));
    }
    let bindings = json!({"repo-a": {
        "sprint_id": "HUM-01", "sprint_number": 11, "import_sprint_numbers": [11, 12], "plan_version": "v1.3.3",
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
            db_config: cfg.clone(),
            policy: policy::load(POLICY).expect("policy"),
            manifest,
            bindings,
            github_writes: None,
            // repo-a's supervisors on a closed port: the report says unreachable, never "stopped"
            roles: json!({"repo-a": {
                "development": {"url": "http://127.0.0.1:9", "adapter_token_file": "/nonexistent"},
                "guardrail": {"url": "http://127.0.0.1:9", "adapter_token_file": "/nonexistent"}}}),
        },
        creds,
    };
    (app, tokens, credentials)
}

pub fn start() -> Option<Env> {
    start_with_github(None)
}

pub fn start_with_github(github: Option<ieo_coord::github_write::Config>) -> Option<Env> {
    let base = base_config()?;
    let permit = Permit::acquire();
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
    let (mut app, tokens, _) = fixture(&cfg, &format!("{n}-{nanos}"));
    app.ctx.github_writes = github;
    let server = tiny_http::Server::http("127.0.0.1:0").expect("bind");
    let addr = server
        .server_addr()
        .to_ip()
        .map(|a| a.to_string())
        .expect("addr");
    let server = Arc::new(server);
    let app = Arc::new(app);
    let c2 = cfg.clone();
    let served = Arc::clone(&app);
    std::thread::spawn(move || {
        let _ = serve(server, served, &c2, 3); // 3 workers: ~30 parallel tests stay under max_connections 100
    });
    Some(Env {
        _permit: permit,
        addr,
        base: Some(base),
        dbname,
        cfg,
        tokens,
        app,
    })
}

impl Drop for Env {
    /// Each test drops its own database (the server's connections are closed by FORCE).
    fn drop(&mut self) {
        if let Some(base) = &self.base
            && let Ok(mut admin) = base.connect(NoTls)
        {
            let _ = admin.batch_execute(&format!(
                "DROP DATABASE IF EXISTS {} WITH (FORCE)",
                self.dbname
            ));
        }
    }
}

impl Env {
    pub fn call(
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
    /// A request with exact body bytes (X-04: the same bytes the gRPC adapter passes on). A
    /// refused oversized body may be answered before it is read: write errors are ignored and
    /// the answer is read as far as it arrived.
    pub fn call_raw(
        &self,
        who: Option<&str>,
        method: &str,
        path: &str,
        body: &[u8],
    ) -> (u16, Value) {
        self.call_raw_k(who, method, path, None, body)
    }
    /// [`Env::call_raw`] with an `Idempotency-Key` header.
    pub fn call_raw_k(
        &self,
        who: Option<&str>,
        method: &str,
        path: &str,
        key: Option<&str>,
        body: &[u8],
    ) -> (u16, Value) {
        let mut req = format!("{method} {path} HTTP/1.1\r\nHost: t\r\nConnection: close\r\n");
        if let Some(w) = who {
            let t = self.tokens.get(w).cloned().unwrap_or_else(|| w.to_string());
            req += &format!("Authorization: Bearer {t}\r\n");
        }
        if let Some(k) = key {
            req += &format!("Idempotency-Key: {k}\r\n");
        }
        req += &format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            body.len()
        );
        let mut s = TcpStream::connect(&self.addr).expect("connect");
        let mut bytes = req.into_bytes();
        bytes.extend_from_slice(body);
        let _ = s.write_all(&bytes);
        let mut out = Vec::new();
        let _ = s.read_to_end(&mut out);
        let out = String::from_utf8_lossy(&out);
        let status: u16 = out
            .split_whitespace()
            .nth(1)
            .and_then(|x| x.parse().ok())
            .expect("status");
        let body = out.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or("");
        (status, serde_json::from_str(body).unwrap_or(Value::Null))
    }
    pub fn post(&self, who: &str, path: &str, body: &Value) -> (u16, Value) {
        self.call(Some(who), "POST", path, None, Some(body))
    }
    pub fn post_k(&self, who: &str, path: &str, key: &str, body: &Value) -> (u16, Value) {
        self.call(Some(who), "POST", path, Some(key), Some(body))
    }
    pub fn get(&self, who: &str, path: &str) -> (u16, Value) {
        self.call(Some(who), "GET", path, None, None)
    }
    pub fn reopen_github_server(&mut self, config: ieo_coord::github_write::Config) {
        self.reopen_server(Some(config));
    }

    pub fn reopen_server(&mut self, config: Option<ieo_coord::github_write::Config>) {
        let (mut app, tokens, _) = fixture(&self.cfg, "reopened");
        app.ctx.github_writes = config;
        let server = std::sync::Arc::new(tiny_http::Server::http("127.0.0.1:0").unwrap());
        self.addr = server.server_addr().to_ip().unwrap().to_string();
        self.app = std::sync::Arc::new(app);
        self.tokens = tokens;
        let app = std::sync::Arc::clone(&self.app);
        let cfg = self.cfg.clone();
        std::thread::spawn(move || {
            let _ = serve(server, app, &cfg, 3);
        });
    }

    pub fn db(&self) -> Client {
        self.cfg.connect(NoTls).expect("db")
    }
    /// Row counts of every table: a refused request must leave all of them unchanged.
    pub fn counts(&self) -> Vec<(String, i64)> {
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
            "role_checkpoints",
            "role_tamper_events",
            "role_first_actions",
            "role_session_status",
            "role_takeovers",
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

pub fn code(v: &Value) -> String {
    v["code"]
        .as_str()
        .or_else(|| v["rejections"][0]["code"].as_str())
        .unwrap_or_default()
        .to_string()
}

pub fn spec(repo: &str, revision: u64, ceiling: u64) -> Value {
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
pub fn ready_sprint(e: &Env, ceiling: u64) -> Value {
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

pub fn assign(
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

pub fn report(
    role: &str,
    event_id: &str,
    aid: &str,
    generation: u64,
    task: &str,
    sha: &str,
) -> Value {
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
            include_str!("../../../../docs/poc/reports/contract_examples/development_progress.json")
        }
        _ => include_str!("../../../../docs/poc/reports/contract_examples/guardrail_review.json"),
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

pub fn evidence(e: &Env, id: &str, kind: &str, extra: Value) -> (u16, Value) {
    let mut b = json!({"evidence_id": id, "repo_id": "repo-a", "kind": kind});
    if let (Some(m), Some(x)) = (b.as_object_mut(), extra.as_object()) {
        m.extend(x.clone());
    }
    e.post("ingest", "/v0/coordinator/evidence", &b)
}

/// All GitHub/run evidence that makes 01-01 at `sha` complete.
pub fn full_evidence(e: &Env, sha: &str, tag: &str) {
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

pub fn judge(e: &Env, jid: &str, task: &str) -> (u16, Value) {
    e.post(
        "judge",
        "/v0/coordinator/judgments",
        &json!({"judgment_id": jid, "repo_id": "repo-a", "sprint_id": "POC-01", "task_id": task}),
    )
}

/// A development checkpoint file of repo-a (02-04 decision item 9), pretty-printed as the
/// supervisor writes it.
pub fn checkpoint(seq: u64, generation: u64, digest: &str, extra: Value) -> String {
    let mut c = json!({"format": "ieo-checkpoint/v0", "seq": seq, "repo_id": "repo-a",
        "role": "development", "phase": "after_run", "session_id": "s1", "assignment_id": "A1",
        "generation": generation, "sprint_id": "POC-01", "sprint_revision": 5, "spec_digest": digest,
        "target_sha": SHA1, "image_digest": "sha256:img", "profile_id": "p", "profile_revision": 1,
        "settings_digest": "e".repeat(64), "config_tamper_detected": null});
    for (k, v) in extra.as_object().expect("obj") {
        c[k] = v.clone();
    }
    // 02-05 step 4: a takeover names its delivery; the test fixture's id (recorded by
    // [`ensure_delivery`] before the report) unless the test sets `delivery_id` itself
    if c["phase"] == "takeover"
        && !extra
            .as_object()
            .is_some_and(|m| m.contains_key("delivery_id"))
    {
        c["delivery_id"] = json!(format!(
            "D-t-{}-{}-{}-{}-{}",
            c["repo_id"].as_str().unwrap_or_default(),
            c["role"].as_str().unwrap_or_default(),
            c["assignment_id"].as_str().unwrap_or_default(),
            c["generation"],
            c["seq"]
        ));
    }
    // the attempt the delivery called (review 5dbb3d8 M-1): fixture deliveries use their id as
    // their attempt id, so a checkpoint naming a delivery names that attempt unless the test
    // sets `attempt_id` itself
    if c["delivery_id"].is_string()
        && !extra
            .as_object()
            .is_some_and(|m| m.contains_key("attempt_id"))
    {
        c["attempt_id"] = c["delivery_id"].clone();
    }
    serde_json::to_string_pretty(&c).expect("json")
}

/// Test fixture (02-05 step 4): the delivery a test takeover names (`D-t-…`, from
/// [`checkpoint`]) recorded directly, as deliverAssignment would have recorded it before its
/// start, when the named assignment generation was issued to the checkpoint's repo and role.
/// No result row: a delivery whose answer is not recorded yet. Accepted 02-04 checkpoint tests
/// keep their meaning; the delivery rules themselves are tested with real deliveries.
pub fn ensure_delivery(e: &Env, raw: &[u8]) {
    let Ok(c) = serde_json::from_slice::<Value>(raw) else {
        return;
    };
    let Some(d) = c["delivery_id"].as_str().filter(|d| d.starts_with("D-t-")) else {
        return;
    };
    if c["phase"] != "takeover" {
        return;
    }
    let s = |k: &str| c[k].as_str().unwrap_or_default().to_string();
    let generation = c["generation"].as_i64().unwrap_or(-1);
    e.db()
        .execute(
            "INSERT INTO role_deliveries (delivery_id, repo_id, role, request_id, body,
                 assignment_id, generation, attempt_id, session_id, logical_name,
                 coordinator_record, target_sha, requested_by)
             SELECT $1, a.repo_id, a.role, $1, '{}'::jsonb, a.assignment_id, a.generation, $1,
                    $2, 'test-fixture', $7, a.target_sha, 'test-fixture'
             FROM assignments a WHERE a.assignment_id = $3 AND a.generation = $4
               AND a.repo_id = $5 AND a.role = $6
             ON CONFLICT DO NOTHING",
            &[
                &d,
                &s("session_id"),
                &s("assignment_id"),
                &generation,
                &s("repo_id"),
                &s("role"),
                &json!({"image_digest": "sha256:img", "profile_id": "p", "profile_revision": 1}),
            ],
        )
        .expect("test delivery");
}

/// A free port **below the ephemeral range**, for a coordinator child that binds it itself.
/// A port from binding port 0 is released before the child binds it, and another `bind(0)` in
/// the same test process (a role service's `tiny_http`, the second port, the parallel variant)
/// can take it in between (PR #84 review b291d7c L-1: an outgoing connection is unlikely, since
/// Linux ≥ 4.6 gives `connect()` and `bind(0)` ports of opposite parity). With the default
/// `ip_local_port_range` (32768-60999) no `bind(0)` and no outgoing connection gets a port from
/// 20000-32767. The pick is spread by process and call and checked by binding.
pub fn free_port() -> u16 {
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
