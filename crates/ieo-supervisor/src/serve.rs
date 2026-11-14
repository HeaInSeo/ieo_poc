//! Role supervisor service (02-04): the PID 1 process of one role container. It exposes the
//! adapter operations `start` / `reconnect` / `stop` / `status` (runtime contract §7) over a small
//! authenticated HTTP endpoint, and runs the role's `claude` process through [`crate::run`].
//!
//! It controls only its own role: one session at a time ([`crate::lock`]), the role's own
//! `CLAUDE_CONFIG_DIR` ([`crate::configdir`]), its own checkpoint ([`crate::checkpoint`]). It does
//! not decide budgets: the caller passes the limits for each run (computed with
//! [`crate::run::Limits::for_next`] from the evidence records), and charges the run afterwards with
//! [`crate::run::charge`]. It never prints the credential.
//!
//! Paths follow the contract's container layout (`/state/...`, `/etc/ieo/role`, `/run/ieo/cred`);
//! [`Config`] maps them to any root so the same code runs in tests without a container.

use crate::args::{self, Role};
use crate::checkpoint::{self, Checkpoint, CoordinatorRecord, Reconnect};
use crate::lock::{LockError, SessionLock};
use crate::run::{self, Limits, Options, Outcome, Stop};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Service configuration (a JSON file in the role's read-only `/etc/ieo/role`).
#[derive(Debug, Clone)]
pub struct Config {
    pub role: Role,
    pub repo: String,
    /// Role volume root (contract `/state`).
    pub state: PathBuf,
    /// Read-only role settings (contract `/etc/ieo/role`), incl. `template/` for the config dir.
    pub etc: PathBuf,
    /// Read-only credential directory (contract `/run/ieo/cred`): `oauth_token`, `adapter.sha256`.
    pub cred: PathBuf,
    /// The `claude` program.
    pub claude: PathBuf,
    /// Program used to signal process groups.
    pub kill: String,
    /// The approved budget this role's runs belong to (user decision 2026-11-13 ①): the service
    /// refuses runs beyond its caps on its own (a backstop; exact accounting is the caller's).
    pub budget: crate::budget::Budget,
    /// This role's share of the budget (config `share`): each field at most the budget's cap.
    /// The shares of the roles of one budget must sum to at most the approval; the role
    /// configuration (`/etc/ieo/role`, read-only) is where that is fixed, so the backstop alone
    /// never grants more than the approval (review 6975d5f N2).
    pub share: Share,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Share {
    pub max_runs: u64,
    pub total_seconds: u64,
    pub billable_tokens: u64,
    pub cache_read_tokens: u64,
}

impl Config {
    pub fn from_json(v: &Value) -> Result<Config, String> {
        let s = |k: &str| {
            v[k].as_str()
                .map(str::to_string)
                .ok_or(format!("config.{k} missing"))
        };
        let role = match s("role")?.as_str() {
            "development" => Role::Development {
                append_system_prompt_file: v["append_system_prompt_file"]
                    .as_str()
                    .map(str::to_string),
            },
            "guardrail" => Role::Guardrail,
            r => return Err(format!("unknown role {r}")),
        };
        Config {
            role,
            repo: s("repo")?,
            state: s("state")?.into(),
            etc: s("etc")?.into(),
            cred: s("cred")?.into(),
            claude: s("claude")?.into(),
            kill: v["kill"].as_str().unwrap_or("kill").to_string(),
            budget: match v["budget"].as_str() {
                Some("02-04") => crate::budget::BUDGET_0204,
                Some("X-02") => crate::budget::BUDGET_X02,
                _ => {
                    return Err("config.budget must name an approved budget (02-04 or X-02)".into());
                }
            },
            share: Share {
                max_runs: 0,
                total_seconds: 0,
                billable_tokens: 0,
                cache_read_tokens: 0,
            },
        }
        .with_share(&v["share"])
    }

    fn with_share(mut self, v: &Value) -> Result<Config, String> {
        let b = self.budget;
        let f = |k: &str, cap: u64| match v[k].as_u64() {
            Some(x) if x > 0 && x <= cap => Ok(x),
            _ => Err(format!(
                "config.share.{k} must be a number in 1..={cap} (budget {})",
                b.name
            )),
        };
        self.share = Share {
            max_runs: f("max_runs", b.max_runs)?,
            total_seconds: f("total_seconds", b.total_seconds)?,
            billable_tokens: f("billable_tokens", b.billable_tokens)?,
            cache_read_tokens: f("cache_read_tokens", b.cache_read_tokens)?,
        };
        Ok(self)
    }
    fn config_dir(&self) -> PathBuf {
        self.state.join("claude")
    }
    fn home(&self) -> PathBuf {
        self.state.join("home")
    }
    fn runs(&self) -> PathBuf {
        self.state.join("runs")
    }
    fn locks(&self) -> PathBuf {
        self.state.join("locks")
    }
    fn checkpoint_path(&self) -> PathBuf {
        self.state.join("checkpoint.json")
    }
    /// Attempts started under this role's budget (append-only, on the persistent volume).
    fn attempts_path(&self) -> PathBuf {
        self.state.join("budget").join("attempts.jsonl")
    }
    /// The role's working directory, mapped from the contract's `/state/...` path.
    fn cwd(&self) -> PathBuf {
        let c = args::cwd(&self.role, &self.repo);
        self.state.join(c.trim_start_matches("/state/"))
    }
    fn role_name(&self) -> &'static str {
        match self.role {
            Role::Development { .. } => "development",
            Role::Guardrail => "guardrail",
        }
    }
}

#[derive(Debug, Default)]
struct State {
    running: Option<Value>,
    stop: Option<Arc<AtomicBool>>,
    last: Option<Value>,
}

pub struct Service {
    cfg: Config,
    state: Mutex<State>,
}

pub struct Response {
    pub status: u16,
    pub body: Value,
}

fn resp(status: u16, body: Value) -> Response {
    Response { status, body }
}

fn err(status: u16, code: &str, message: &str) -> Response {
    resp(status, json!({"code": code, "message": message}))
}

/// The stream-json user message carrying `prompt`.
pub fn prompt_message(prompt: &str) -> Vec<u8> {
    let mut v = serde_json::to_vec(&json!({"type": "user", "message": {"role": "user",
        "content": [{"type": "text", "text": prompt}]}}))
    .unwrap_or_default();
    v.push(b'\n');
    v
}

fn limits_from(v: &Value) -> Result<Limits, String> {
    let n = |k: &str| v[k].as_u64().ok_or(format!("limits.{k} missing"));
    Ok(Limits {
        timeout: Duration::from_millis(n("timeout_ms")?),
        timeout_is_total: v["timeout_is_total"].as_bool().unwrap_or(false),
        billable_room: n("billable_room")?,
        cache_read_room: n("cache_read_room")?,
        grace: Duration::from_millis(n("grace_ms")?),
    })
}

/// Ids used as file names and CLI arguments: 1–64 of `[A-Za-z0-9_-]`, starting with a letter or
/// digit (never `-`, so never read as an option; never `.` or `/`).
fn safe_id(s: &str) -> bool {
    s.len() <= 64
        && s.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// The last `result` event's session totals, for the next resumed run's base (§11 item 7).
fn session_cumulative(stdout: &Path) -> Value {
    let text = std::fs::read_to_string(stdout).unwrap_or_default();
    text.lines()
        .rev()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find(|v| v["type"] == "result")
        .map(|r| {
            json!({"modelUsage": r["modelUsage"], "total_cost_usd": r["total_cost_usd"],
                         "usage": r["usage"]})
        })
        .unwrap_or(Value::Null)
}

impl Service {
    pub fn new(cfg: Config) -> Service {
        Service {
            cfg,
            state: Mutex::new(State::default()),
        }
    }

    fn authorized(&self, bearer: Option<&str>) -> bool {
        let Ok(want) = std::fs::read_to_string(self.cfg.cred.join("adapter.sha256")) else {
            return false; // no verification key: refuse everything (fail closed)
        };
        let Some(tok) = bearer else { return false };
        hex::encode(Sha256::digest(tok.as_bytes())) == want.trim()
    }

    pub fn status(&self) -> Value {
        let st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let cp = std::fs::read(self.cfg.checkpoint_path())
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
        json!({"role": self.cfg.role_name(), "repo": self.cfg.repo,
               "running": st.running, "last": st.last, "checkpoint": cp})
    }

    /// Handle one request. `bearer` is the token from `Authorization: Bearer …`.
    pub fn handle(
        self: &Arc<Self>,
        method: &str,
        path: &str,
        bearer: Option<&str>,
        body: &[u8],
    ) -> Response {
        if !self.authorized(bearer) {
            return err(401, "UNAUTHORIZED", "missing or wrong adapter credential");
        }
        let segs: Vec<&str> = path.trim_matches('/').split('/').collect();
        match (method, segs.as_slice()) {
            ("GET", ["v0", "role", "status"]) => resp(200, self.status()),
            ("POST", ["v0", "role", "stop"]) => {
                let st = self.state.lock().unwrap_or_else(|e| e.into_inner());
                match &st.stop {
                    Some(f) if st.running.is_some() => {
                        f.store(true, std::sync::atomic::Ordering::SeqCst);
                        resp(202, json!({"stopping": st.running}))
                    }
                    _ => resp(200, json!({"stopping": null, "message": "nothing running"})),
                }
            }
            ("GET", ["v0", "role", "runs", id, file]) if safe_id(id) => {
                let name = match *file {
                    "stdout" => "stdout.jsonl",
                    "stderr" => "stderr.txt",
                    "outcome" => "outcome.json",
                    _ => return err(404, "NOT_FOUND", "no such file"),
                };
                match std::fs::read(self.cfg.runs().join(id).join(name)) {
                    Ok(b) => resp(200, json!({"content": String::from_utf8_lossy(&b)})),
                    Err(_) => err(404, "NOT_FOUND", "no such run"),
                }
            }
            ("POST", ["v0", "role", op @ ("start" | "reconnect")]) => {
                let Ok(req) = serde_json::from_slice::<Value>(body) else {
                    return err(400, "SCHEMA_VIOLATION", "body is not JSON");
                };
                self.launch(op, &req)
            }
            _ => err(404, "NOT_FOUND", "no such operation"),
        }
    }

    fn launch(self: &Arc<Self>, op: &str, req: &Value) -> Response {
        let s = |k: &str| req[k].as_str().map(str::to_string);
        let Some(attempt) = s("attempt_id").filter(|a| safe_id(a)) else {
            return err(
                400,
                "SCHEMA_VIOLATION",
                "attempt_id (1-64 of [A-Za-z0-9_-]) required",
            );
        };
        let Some(prompt) = s("prompt") else {
            return err(400, "SCHEMA_VIOLATION", "prompt required");
        };
        let limits = match limits_from(&req["limits"]) {
            Ok(l) => l,
            Err(e) => return err(400, "SCHEMA_VIOLATION", &e),
        };
        // the stop must fit the container runtime's stop timeout (podman default 10 s)
        if limits.grace > MAX_GRACE {
            return err(
                400,
                "SCHEMA_VIOLATION",
                &format!("limits.grace_ms above {} ms", MAX_GRACE.as_millis()),
            );
        }
        let c = &req["coordinator"];
        let coord = (
            c["generation"].as_u64(),
            s_of(c, "assignment_id"),
            s_of(c, "sprint_id"),
            c["sprint_revision"].as_u64(),
            s_of(c, "spec_digest"),
            s_of(c, "image_digest"),
            s_of(c, "profile_id"),
            c["profile_revision"].as_u64(),
        );
        let (
            Some(generation),
            Some(assignment_id),
            Some(sprint_id),
            Some(sprint_revision),
            Some(spec_digest),
            Some(image_digest),
            Some(profile_id),
            Some(profile_revision),
        ) = coord
        else {
            return err(400, "SCHEMA_VIOLATION", "coordinator record incomplete");
        };
        if self.cfg.runs().join(&attempt).exists() {
            return err(409, "ID_CONTENT_CONFLICT", "attempt_id already used");
        }
        // a run in this role (also one still starting, before its checkpoint exists) answers
        // with the status first; checked again under the state lock below
        if self.is_running() {
            return resp(
                409,
                json!({"code": "ALREADY_RUNNING", "status": self.status()}),
            );
        }
        // decide what runs
        let cp_raw = std::fs::read(self.cfg.checkpoint_path());
        let cp = cp_raw
            .as_ref()
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(b).ok());
        let (session_id, resume, tamper) = if op == "start" {
            let Some(sid) = s("session_id").filter(|x| safe_id(x)) else {
                return err(400, "SCHEMA_VIOLATION", "session_id required");
            };
            (sid, false, None)
        } else {
            // a reconnect without a usable checkpoint is never silently replaced by a new session
            // (§5-4): absent → NOT_OBSERVED, unreadable/corrupt/unsafe → external reconciliation
            match (&cp_raw, &cp) {
                (Err(e), _) if e.kind() == std::io::ErrorKind::NotFound => {
                    return err(
                        409,
                        "NOT_OBSERVED",
                        "no checkpoint to reconnect to; start a new session explicitly",
                    );
                }
                (Err(_), _) | (Ok(_), None) => {
                    return resp(
                        409,
                        json!({"code": "EXTERNAL_RECONCILIATION_REQUIRED",
                               "reason": "the checkpoint cannot be read"}),
                    );
                }
                _ => {}
            }
            if let Some(phase @ ("takeover" | "error")) =
                cp.as_ref().and_then(|c| c["phase"].as_str())
            {
                // a run that never reached its end: children, external requests and the session
                // record were not observed, so nothing resumes on it silently (§6)
                return resp(
                    409,
                    json!({"code": "EXTERNAL_RECONCILIATION_REQUIRED",
                           "reason": format!("the previous run did not finish (checkpoint phase {phase})")}),
                );
            }
            if cp
                .as_ref()
                .and_then(|c| c["session_id"].as_str())
                .is_some_and(|sid| !safe_id(sid))
            {
                return resp(
                    409,
                    json!({"code": "EXTERNAL_RECONCILIATION_REQUIRED",
                           "reason": "the checkpoint session_id is not a safe id"}),
                );
            }
            // a running session answers with its status, never a second process (§5-5)
            let running = cp
                .as_ref()
                .and_then(|c| c["session_id"].as_str())
                .and_then(|sid| {
                    match SessionLock::acquire(&self.cfg.locks(), sid, std::process::id()) {
                        Err(LockError::Running { pid }) => Some(pid),
                        _ => None, // dropped at once: only probed
                    }
                });
            let record = CoordinatorRecord {
                generation,
                assignment_id: &assignment_id,
                sprint_id: &sprint_id,
                sprint_revision,
                spec_digest: &spec_digest,
                image_digest: &image_digest,
                profile_id: &profile_id,
                profile_revision,
            };
            let observed = crate::configdir::observed(&self.cfg.config_dir());
            match checkpoint::decide(cp.as_ref(), &record, &observed, running) {
                Reconnect::AlreadyRunning { pid } => {
                    return resp(
                        409,
                        json!({"code": "ALREADY_RUNNING", "pid": pid, "status": self.status()}),
                    );
                }
                Reconnect::StaleGeneration => {
                    return err(
                        409,
                        "STALE_GENERATION",
                        "the checkpoint generation is older",
                    );
                }
                Reconnect::ExternalReconciliation { reason } => {
                    return resp(
                        409,
                        json!({"code": "EXTERNAL_RECONCILIATION_REQUIRED", "reason": reason}),
                    );
                }
                Reconnect::Resume { session_id } => (session_id, true, None),
                Reconnect::NewSession | Reconnect::TamperDetected { .. } => {
                    let Some(new_sid) = s("new_session_id").filter(|x| safe_id(x)) else {
                        return err(400, "SCHEMA_VIOLATION", "new_session_id required");
                    };
                    let tamper = match checkpoint::decide(cp.as_ref(), &record, &observed, None) {
                        Reconnect::TamperDetected {
                            old_session_id,
                            checkpoint_digest,
                            observed_digest,
                        } => Some(json!({"config_tamper_detected": true,
                            "old_session_id": old_session_id, "new_session_id": new_sid,
                            "checkpoint_digest": checkpoint_digest, "observed_digest": observed_digest})),
                        _ => None,
                    };
                    (new_sid, false, tamper)
                }
            }
        };
        let credential = match std::fs::read_to_string(self.cfg.cred.join("oauth_token")) {
            Ok(c) if !c.trim().is_empty() => c.trim().to_string(),
            _ => {
                return err(
                    503,
                    "CREDENTIAL_UNAVAILABLE",
                    "the role credential file is missing",
                );
            }
        };
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if st.running.is_some() {
            drop(st);
            return resp(
                409,
                json!({"code": "ALREADY_RUNNING", "status": self.status()}),
            );
        }
        // budget backstop (F4): the caller computes exact limits; the service still refuses
        // anything beyond the approved caps, and counts every attempt it starts
        if let Err(why) = self.budget_backstop(&limits) {
            return resp(409, json!({"code": "BUDGET_EXCEEDED", "reason": why}));
        }
        if let Err(e) = self.record_attempt(&attempt, &limits) {
            return err(500, "INTERNAL", &format!("attempt ledger: {e}"));
        }
        let stop = Arc::new(AtomicBool::new(false));
        let info = json!({"attempt_id": attempt, "session_id": session_id, "mode":
            if resume { "resume" } else { "new" }, "tamper": tamper});
        st.running = Some(info.clone());
        st.stop = Some(stop.clone());
        drop(st);
        let me = Arc::clone(self);
        let job = Job {
            attempt,
            session_id,
            resume,
            prompt,
            limits,
            credential,
            coord: json!({"generation": generation, "assignment_id": assignment_id,
                "sprint_id": sprint_id, "sprint_revision": sprint_revision,
                "spec_digest": spec_digest, "image_digest": image_digest,
                "profile_id": profile_id, "profile_revision": profile_revision,
                "target_sha": req["target_sha"].as_str().unwrap_or("unknown")}),
            tamper,
            stop,
        };
        std::thread::spawn(move || me.execute(job));
        resp(202, json!({"accepted": info}))
    }

    fn execute(&self, job: Job) {
        let result = self.execute_inner(&job);
        // the attempt's end, for the backstop's token count: observed lower bounds when the
        // guard ran to an outcome; none after an error (usage unknown → reconcile)
        if let Ok(v) = &result {
            let o = &v["observed"];
            let end = match (
                o["billable_lower_bound"].as_u64(),
                o["cache_read_lower_bound"].as_u64(),
            ) {
                (Some(b), Some(c)) => json!({"observed_billable": b, "observed_cache_read": c}),
                _ if v["spawned"] == false => {
                    json!({"observed_billable": 0, "observed_cache_read": 0})
                }
                _ => json!({"unknown": true}),
            };
            let _ = self.append_ledger(&json!({"attempt_id": job.attempt, "end": end}));
        }
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        st.last = Some(match result {
            Ok(v) => v,
            Err(e) => json!({"attempt_id": job.attempt, "session_id": job.session_id,
                             "error": e}),
        });
        st.running = None;
        st.stop = None;
    }

    fn execute_inner(&self, job: &Job) -> Result<Value, String> {
        let cfg = &self.cfg;
        let dir = cfg.runs().join(&job.attempt);
        for d in [
            &dir,
            &cfg.locks(),
            &cfg.home(),
            &cfg.config_dir(),
            &cfg.cwd(),
        ] {
            std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
        let _lock = SessionLock::acquire(&cfg.locks(), &job.session_id, std::process::id())
            .map_err(|e| format!("session lock: {e:?}"))?;
        let report = crate::configdir::clean(&cfg.config_dir(), &cfg.etc.join("template"))
            .map_err(|e| format!("config cleanup: {e}"))?;
        if job.stop.load(std::sync::atomic::Ordering::SeqCst) {
            // stopped before anything was spawned: no process, no model call
            let v = json!({"attempt_id": job.attempt, "session_id": job.session_id,
                "mode": if job.resume { "resume" } else { "new" },
                "stop": "Requested", "stopped_by_guard": false, "spawned": false,
                "tamper": job.tamper});
            let _ = std::fs::write(
                dir.join("outcome.json"),
                serde_json::to_vec_pretty(&v).unwrap_or_default(),
            );
            return Ok(v);
        }
        // §5-1: a checkpoint at takeover, after the stop check, before anything runs (a crash mid-run then names this
        // session); again after the run, and on errors
        self.write_checkpoint(job, &report.digest, None, "takeover")?;
        let argv = args::build(&cfg.role, &cfg.repo, &job.session_id, job.resume);
        let mut cmd = Command::new(&cfg.claude);
        cmd.args(&argv).current_dir(cfg.cwd());
        let env = crate::env::build(
            &cfg.home().to_string_lossy(),
            &cfg.config_dir().to_string_lossy(),
            &job.credential,
        );
        crate::env::apply(&mut cmd, &env)?;
        let opts = Options {
            kill: cfg.kill.clone(),
            stdin: Some(prompt_message(&job.prompt)),
            stop: Some(job.stop.clone()),
        };
        let stdout = dir.join("stdout.jsonl");
        let outcome: Outcome =
            match run::run_with(cmd, job.limits, &stdout, &dir.join("stderr.txt"), &opts) {
                Ok(o) => o,
                Err(e) => {
                    let _ = self.write_checkpoint(job, &report.digest, Some(&stdout), "error");
                    return Err(format!("run: {e}"));
                }
            };
        self.write_checkpoint(job, &report.digest, Some(&stdout), "after_run")?;
        let v = json!({"attempt_id": job.attempt, "session_id": job.session_id,
            "mode": if job.resume { "resume" } else { "new" },
            "stop": format!("{:?}", outcome.stop),
            "stopped_by_guard": outcome.stop != Stop::Exited,
            "exit_code": outcome.exit_code, "elapsed_ms": outcome.elapsed.as_millis() as u64,
            "observed": {"billable_lower_bound": outcome.observed.billable,
                         "cache_read_lower_bound": outcome.observed.cache_read,
                         "messages": outcome.observed.messages,
                         "incomplete_lines": outcome.observed.incomplete_lines},
            "cleanup_failed": outcome.cleanup_failed,
            "config_cleanup": {"digest": report.digest, "removed": report.removed.len(),
                               "credentials_removed": report.credentials_removed},
            "tamper": job.tamper,
            "argv": argv, "env_names": crate::env::names(&env)});
        let _ = std::fs::write(
            dir.join("outcome.json"),
            serde_json::to_vec_pretty(&v).unwrap_or_default(),
        );
        Ok(v)
    }

    /// Write the §5-1 checkpoint durably (temp file, fsync, rename, directory fsync). `phase`
    /// records when it was written (`takeover`, `after_run`, `error`).
    fn write_checkpoint(
        &self,
        job: &Job,
        settings_digest: &str,
        stdout: Option<&Path>,
        phase: &str,
    ) -> Result<(), String> {
        let cfg = &self.cfg;
        let c = &job.coord;
        let n = |k: &str| c[k].as_u64().unwrap_or_default();
        let t = |k: &str| c[k].as_str().unwrap_or_default().to_string();
        let cp = checkpoint::record(&Checkpoint {
            session_id: &job.session_id,
            generation: n("generation"),
            assignment_id: &t("assignment_id"),
            target_sha: &t("target_sha"),
            sprint_id: &t("sprint_id"),
            sprint_revision: n("sprint_revision"),
            spec_digest: &t("spec_digest"),
            image_digest: &t("image_digest"),
            profile_id: &t("profile_id"),
            profile_revision: n("profile_revision"),
            settings_digest,
            worktree_head: "none",
            diff_sha256: "none",
            last_event_offset: stdout
                .and_then(|p| std::fs::read_to_string(p).ok())
                .map(|s| s.lines().count() as u64)
                .unwrap_or_default(),
            active_children: vec![],
            pending_external_requests: vec![],
            session_cumulative: Value::Null,
        });
        let mut cp = cp;
        cp["phase"] = json!(phase);
        // the resume base (§11 item 7): this run's own result if it wrote one; otherwise, on a
        // resume, the previous base of the same session carried forward with the attempt it
        // came from (the caller checks it is the run actually resumed); else unknown
        let own = stdout.map(session_cumulative).unwrap_or(Value::Null);
        let (cum, from) = if !own.is_null() {
            (own, json!(job.attempt))
        } else if job.resume {
            std::fs::read(cfg.checkpoint_path())
                .ok()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                .filter(|p| p["session_id"] == job.session_id.as_str())
                .map(|p| {
                    (
                        p["session_cumulative"].clone(),
                        p["session_cumulative_attempt"].clone(),
                    )
                })
                .unwrap_or((Value::Null, Value::Null))
        } else {
            (Value::Null, Value::Null)
        };
        cp["session_cumulative"] = cum;
        cp["session_cumulative_attempt"] = from;
        let path = cfg.checkpoint_path();
        let tmp = path.with_extension("json.tmp");
        let write = || -> std::io::Result<()> {
            use std::io::Write;
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(&serde_json::to_vec_pretty(&cp).unwrap_or_default())?;
            f.sync_all()?;
            std::fs::rename(&tmp, &path)?;
            std::fs::File::open(&cfg.state)?.sync_all()
        };
        write().map_err(|e| format!("checkpoint: {e}"))
    }
}

/// Largest stop grace a caller may ask for: the stop must finish within the container
/// runtime's stop timeout (podman default 10 s).
pub const MAX_GRACE: Duration = run::GRACE;

impl Service {
    /// The attempt ledger, read fail-closed: a line that is not a JSON object with a string
    /// `attempt_id` makes the whole ledger unusable (the backstop then refuses). A missing ledger
    /// while attempt directories exist is unusable too.
    fn ledger(&self) -> Result<Vec<Value>, String> {
        let lines = match std::fs::read_to_string(self.cfg.attempts_path()) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let runs = std::fs::read_dir(self.cfg.runs())
                    .map(|d| d.count())
                    .unwrap_or(0);
                if runs > 0 {
                    return Err(format!(
                        "attempt ledger missing while {runs} attempt directories exist"
                    ));
                }
                String::new()
            }
            Err(e) => return Err(format!("attempt ledger unreadable: {e}")),
        };
        lines
            .lines()
            .filter(|l| !l.trim().is_empty())
            .enumerate()
            .map(|(i, l)| match serde_json::from_str::<Value>(l) {
                Ok(v) if v["attempt_id"].is_string() => Ok(v),
                _ => Err(format!("attempt ledger line {} unreadable", i + 1)),
            })
            .collect()
    }

    /// The service's own ceiling within this role's share, independent of the caller:
    /// * runs: attempts started (ledger `start` lines, or attempt directories if more);
    /// * time: the windows granted (timeout + grace), not the time used — conservative; a
    ///   start line without a numeric `window_ms` counts as a full per-run window;
    /// * tokens: the guard's observed lower bounds of finished attempts plus the requested room
    ///   must fit the share. An attempt without a recorded end (supervisor died mid-run, or the
    ///   run failed) has unknown usage: nothing further starts until a person reconciles.
    ///
    /// Exact accounting (collector figures, unmeasured runs) is the caller's (`budget::next`).
    fn budget_backstop(&self, l: &Limits) -> Result<(), String> {
        let b = &self.cfg.budget;
        let sh = &self.cfg.share;
        let ledger = self.ledger()?;
        let starts: Vec<&Value> = ledger.iter().filter(|v| v["end"].is_null()).collect();
        let dirs = std::fs::read_dir(self.cfg.runs())
            .map(|d| d.count())
            .unwrap_or(0);
        let runs = (starts.len()).max(dirs) as u64;
        if runs >= sh.max_runs {
            return Err(format!(
                "{runs} of {} runs of this role's share already started",
                sh.max_runs
            ));
        }
        let window = l.timeout + l.grace;
        if window > Duration::from_secs(b.per_run_seconds) {
            return Err(format!(
                "timeout + grace {} s above the per-run limit {} s",
                window.as_secs(),
                b.per_run_seconds
            ));
        }
        let full = b.per_run_seconds * 1000;
        let granted: u64 = starts
            .iter()
            .map(|a| a["window_ms"].as_u64().unwrap_or(full))
            .sum();
        if granted + window.as_millis() as u64 > sh.total_seconds * 1000 {
            return Err(format!(
                "granted time {} ms + {} ms above this role's share {} s",
                granted,
                window.as_millis(),
                sh.total_seconds
            ));
        }
        let (mut billable, mut cache_read) = (0u64, 0u64);
        for a in &starts {
            let id = a["attempt_id"].as_str().unwrap_or_default();
            let end = ledger
                .iter()
                .find(|v| v["end"].is_object() && v["attempt_id"] == id)
                .map(|v| &v["end"]);
            match end.map(|e| {
                (
                    e["observed_billable"].as_u64(),
                    e["observed_cache_read"].as_u64(),
                )
            }) {
                Some((Some(x), Some(y))) => {
                    billable = billable.saturating_add(x);
                    cache_read = cache_read.saturating_add(y);
                }
                _ => {
                    return Err(format!(
                        "attempt {id} has no recorded end with observed usage; reconcile first"
                    ));
                }
            }
        }
        if billable.saturating_add(l.billable_room) > sh.billable_tokens
            || cache_read.saturating_add(l.cache_read_room) > sh.cache_read_tokens
        {
            return Err(format!(
                "observed {billable} billable / {cache_read} cache_read plus the requested room \
                 above this role's share {} / {}",
                sh.billable_tokens, sh.cache_read_tokens
            ));
        }
        Ok(())
    }

    fn append_ledger(&self, line: &Value) -> std::io::Result<()> {
        use std::io::Write;
        let p = self.cfg.attempts_path();
        std::fs::create_dir_all(p.parent().unwrap_or(&self.cfg.state))?;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&p)?;
        writeln!(f, "{line}")?;
        f.sync_all()
    }

    fn record_attempt(&self, attempt: &str, l: &Limits) -> std::io::Result<()> {
        self.append_ledger(
            &json!({"attempt_id": attempt, "budget": self.cfg.budget.name,
            "window_ms": (l.timeout + l.grace).as_millis() as u64}),
        )
    }
}

fn s_of(v: &Value, k: &str) -> Option<String> {
    v[k].as_str().map(str::to_string)
}

struct Job {
    attempt: String,
    session_id: String,
    resume: bool,
    prompt: String,
    limits: Limits,
    credential: String,
    coord: Value,
    tamper: Option<Value>,
    stop: Arc<AtomicBool>,
}

impl Service {
    /// PID 1 duty: reap orphans (processes reparented to PID 1, e.g. a run's descendants after
    /// the group kill). Only while no run is active and under the state lock that also gates new
    /// runs, so it never takes the exit status of a run's own child. Returns how many were reaped.
    /// PID 1 only: never call from a process that waits on its own children elsewhere.
    fn reap_orphans(&self) -> usize {
        use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
        let st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if st.running.is_some() {
            return 0;
        }
        let mut n = 0;
        while let Ok(ws) = waitpid(nix::unistd::Pid::from_raw(-1), Some(WaitPidFlag::WNOHANG)) {
            if matches!(ws, WaitStatus::StillAlive) {
                break;
            }
            n += 1;
        }
        drop(st);
        n
    }

    /// Ask the running attempt (if any) to stop; returns whether one was running.
    pub fn stop_running(&self) -> bool {
        let st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        match &st.stop {
            Some(f) if st.running.is_some() => {
                f.store(true, std::sync::atomic::Ordering::SeqCst);
                true
            }
            _ => false,
        }
    }

    pub fn is_running(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .running
            .is_some()
    }
}

/// Serve HTTP on `listen` until SIGTERM/SIGINT. As PID 1 it reaps orphans between requests; on
/// SIGTERM it stops a running attempt through the run guard, waits for it (bounded), and exits.
pub fn serve(cfg: Config, listen: &str) -> Result<(), String> {
    let server = tiny_http::Server::http(listen).map_err(|e| e.to_string())?;
    let term = Arc::new(AtomicBool::new(false));
    for sig in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT] {
        signal_hook::flag::register(sig, Arc::clone(&term)).map_err(|e| e.to_string())?;
    }
    let svc = Arc::new(Service::new(cfg));
    loop {
        if term.load(std::sync::atomic::Ordering::SeqCst) {
            if svc.stop_running() {
                let t0 = std::time::Instant::now();
                // the run guard stops within its grace (≤ MAX_GRACE) plus its drain; the
                // container stop timeout must be at least 10 s (podman default)
                while svc.is_running()
                    && t0.elapsed() < MAX_GRACE + run::DRAIN + Duration::from_secs(1)
                {
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
            svc.reap_orphans();
            return Ok(());
        }
        let req = match server.recv_timeout(Duration::from_millis(200)) {
            Ok(Some(r)) => r,
            Ok(None) => {
                svc.reap_orphans();
                continue;
            }
            Err(e) => return Err(e.to_string()),
        };
        let mut req = req;
        let bearer = req
            .headers()
            .iter()
            .find(|h| {
                h.field
                    .as_str()
                    .as_str()
                    .eq_ignore_ascii_case("authorization")
            })
            .and_then(|h| h.value.as_str().strip_prefix("Bearer "))
            .map(str::to_string);
        let mut body = Vec::new();
        let _ = std::io::Read::read_to_end(
            &mut std::io::Read::take(req.as_reader(), 1 << 20),
            &mut body,
        );
        let method = req.method().as_str().to_string();
        let url = req.url().to_string();
        let r = svc.handle(&method, &url, bearer.as_deref(), &body);
        let bytes = serde_json::to_vec(&r.body).unwrap_or_default();
        let _ = req.respond(tiny_http::Response::from_data(bytes).with_status_code(r.status));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stop requested before the worker spawns: nothing is started, no takeover checkpoint is
    /// written over the previous one, and the outcome says so (review 6975d5f N6).
    #[test]
    fn a_stop_before_the_spawn_starts_nothing() {
        let root = std::env::temp_dir().join(format!("ieo-serve-unit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("etc/template")).expect("etc");
        std::fs::write(
            root.join("etc/template/settings.json"),
            r#"{"autoMemoryEnabled": false}"#,
        )
        .expect("t");
        // a program that would leave a trace if it ever ran
        let trace = root.join("ran");
        let prog = root.join("prog");
        std::fs::write(&prog, format!("#!/bin/sh\ntouch {}\n", trace.display())).expect("p");
        let _ = Command::new("chmod").arg("+x").arg(&prog).status();
        let cfg = Config::from_json(&json!({"role": "guardrail", "repo": "r",
            "state": root.join("state"), "etc": root.join("etc"), "cred": root.join("cred"),
            "claude": prog, "budget": "02-04",
            "share": {"max_runs": 1, "total_seconds": 100, "billable_tokens": 1, "cache_read_tokens": 1}}))
        .expect("cfg");
        let svc = Service::new(cfg);
        let job = Job {
            attempt: "a1".into(),
            session_id: "s1".into(),
            resume: false,
            prompt: "x".into(),
            limits: Limits {
                timeout: Duration::from_secs(5),
                timeout_is_total: false,
                billable_room: 1,
                cache_read_room: 1,
                grace: Duration::from_secs(1),
            },
            credential: "c".into(),
            coord: json!({}),
            tamper: None,
            stop: Arc::new(AtomicBool::new(true)),
        };
        let v = svc.execute_inner(&job).expect("outcome");
        assert_eq!(
            (v["spawned"].as_bool(), v["stop"].as_str()),
            (Some(false), Some("Requested"))
        );
        assert!(!trace.exists(), "the program ran");
        assert!(
            !svc.cfg.checkpoint_path().exists(),
            "a takeover checkpoint was written"
        );
        assert!(root.join("state/runs/a1/outcome.json").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
}
