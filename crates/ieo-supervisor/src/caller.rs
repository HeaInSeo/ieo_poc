//! Host-side caller of the role services (02-04): the budget authority for real runs.
//!
//! The role service's own ceiling (`serve`) is a per-role backstop inside a container the
//! development agent can write to. The authority is here, outside the containers, over the
//! records of **every** run of the budget (both roles):
//!
//! 1. Records are read fail-closed. A record that asks for a person (`stop_required`) stops
//!    everything until it is reconciled; an unmeasured run stops the budget ([`budget::next`]).
//! 2. No-extra-billing check before the call, from three sources together (user decision
//!    2026-11-14 pr30 §2): the newest observed stream of the account (age-limited, every
//!    `rate_limit_event` must show `overageStatus: "rejected"`), the role environment (the run's
//!    reported variable names: the OAuth variable and no API key), and this run's own signal
//!    (the run guard stops on the first breaking event; the caller re-checks the stream after the
//!    run and asks for a person if any event breaks the condition).
//! 3. Limits come from [`Limits::for_next`] and are reduced to the role's share.
//! 4. After the run the stream is measured with the 02-01 collector (`ieo_evidence::stream`),
//!    with the resume base taken from the caller's own record of the run actually resumed, and
//!    charged with [`run::charge_reported`]. The record is appended and fsynced.
//! 5. Cleanup failure, a run error, a lost run or a broken condition: the record says
//!    `stop_required` and nothing further starts. No automatic retry.
//!
//! Raw streams are kept in the ledger directory, outside the public repository.

use crate::budget::{self, Budget, Observation};
use crate::run::{self, Limits, Observed};
use crate::serve::Share;
use ieo_evidence::record::{Measure, ResumeBase, SessionMode};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const RECORD_FORMAT: &str = "ieo-role-run/v0";

#[derive(Debug, Clone)]
pub struct RoleTarget {
    /// Base URL of the role service, e.g. `http://127.0.0.1:58201`.
    pub url: String,
    /// File holding the adapter bearer token (never printed or recorded).
    pub adapter_token_file: PathBuf,
    pub share: Share,
    /// Largest window (timeout + grace) one call of this role is granted. The role service
    /// counts granted windows against its share, so short runs ask for short windows.
    pub max_window_seconds: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub budget: Budget,
    pub ledger_dir: PathBuf,
    pub roles: BTreeMap<String, RoleTarget>,
    /// For the first run of the budget: the newest stored stream of the same account and when
    /// it was observed (unix seconds).
    pub first_observation: Option<(PathBuf, u64)>,
}

impl Plan {
    pub fn from_json(v: &Value) -> Result<Plan, String> {
        let budget = match v["budget"].as_str() {
            Some("02-04") => budget::BUDGET_0204,
            Some("X-02") => budget::BUDGET_X02,
            _ => return Err("plan.budget must be 02-04 or X-02".into()),
        };
        let ledger_dir = v["ledger_dir"]
            .as_str()
            .ok_or("plan.ledger_dir missing")?
            .into();
        let mut roles = BTreeMap::new();
        let obj = v["roles"].as_object().ok_or("plan.roles missing")?;
        let mut sum = [0u64; 4];
        for (name, r) in obj {
            if name != "development" && name != "guardrail" {
                return Err(format!("unknown role {name}"));
            }
            let sh = &r["share"];
            let f = |k: &str| {
                sh[k]
                    .as_u64()
                    .filter(|x| *x > 0)
                    .ok_or(format!("roles.{name}.share.{k}"))
            };
            let share = Share {
                max_runs: f("max_runs")?,
                total_seconds: f("total_seconds")?,
                billable_tokens: f("billable_tokens")?,
                cache_read_tokens: f("cache_read_tokens")?,
            };
            for (s, x) in sum.iter_mut().zip([
                share.max_runs,
                share.total_seconds,
                share.billable_tokens,
                share.cache_read_tokens,
            ]) {
                *s = s.saturating_add(x);
            }
            roles.insert(
                name.clone(),
                RoleTarget {
                    url: r["url"].as_str().ok_or(format!("roles.{name}.url"))?.into(),
                    adapter_token_file: r["adapter_token_file"]
                        .as_str()
                        .ok_or(format!("roles.{name}.adapter_token_file"))?
                        .into(),
                    share,
                    max_window_seconds: r["max_window_seconds"].as_u64(),
                },
            );
        }
        // the shares of all roles may not exceed the approval
        if sum[0] > budget.max_runs
            || sum[1] > budget.total_seconds
            || sum[2] > budget.billable_tokens
            || sum[3] > budget.cache_read_tokens
        {
            return Err(format!(
                "role shares {sum:?} exceed the {} budget",
                budget.name
            ));
        }
        let first_observation = match &v["first_observation"] {
            Value::Null => None,
            o => Some((
                o["stream"]
                    .as_str()
                    .ok_or("first_observation.stream")?
                    .into(),
                o["observed_unix"]
                    .as_u64()
                    .ok_or("first_observation.observed_unix")?,
            )),
        };
        Ok(Plan {
            budget,
            ledger_dir,
            roles,
            first_observation,
        })
    }

    fn records_path(&self) -> PathBuf {
        self.ledger_dir.join("records.jsonl")
    }
}

/// One call: which role, start or reconnect, and what to send.
#[derive(Debug, Clone)]
pub struct Call {
    pub task_id: String,
    pub role: String,
    /// `start` or `reconnect`.
    pub op: String,
    pub attempt_id: String,
    /// `start`: the new session id; `reconnect`: the id used if a new session is needed.
    pub session_id: String,
    pub prompt: String,
    pub coordinator: Value,
}

/// Transport to a role service. `Http` in use; tests may call the service in process.
pub trait Adapter {
    fn send(
        &self,
        role: &RoleTarget,
        method: &str,
        path: &str,
        body: Option<&Value>,
    ) -> Result<(u16, Value), String>;
}

pub struct Http;

impl Adapter for Http {
    fn send(
        &self,
        role: &RoleTarget,
        method: &str,
        path: &str,
        body: Option<&Value>,
    ) -> Result<(u16, Value), String> {
        let token = std::fs::read_to_string(&role.adapter_token_file)
            .map_err(|e| format!("adapter token file: {}", e.kind()))?;
        let req = ureq::request(method, &format!("{}{path}", role.url))
            .timeout(Duration::from_secs(30))
            .set("Authorization", &format!("Bearer {}", token.trim()));
        let r = match body {
            Some(b) => req.send_json(b.clone()),
            None => req.call(),
        };
        let resp = match r {
            Ok(r) => r,
            Err(ureq::Error::Status(_, r)) => r,
            Err(e) => return Err(format!("transport: {}", e.kind())),
        };
        let status = resp.status();
        let v: Value = resp
            .into_json()
            .map_err(|e| format!("response body: {}", e.kind()))?;
        Ok((status, v))
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

/// All records of the budget, fail-closed: an unreadable line is an error.
pub fn records(plan: &Plan) -> Result<Vec<Value>, String> {
    let text = match std::fs::read_to_string(plan.records_path()) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("records unreadable: {e}")),
    };
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .enumerate()
        .map(|(i, l)| match serde_json::from_str::<Value>(l) {
            Ok(v) if v.is_object() => Ok(v),
            _ => Err(format!("record line {} unreadable", i + 1)),
        })
        .collect()
}

fn append(plan: &Plan, rec: &Value) -> Result<(), String> {
    use std::io::Write;
    std::fs::create_dir_all(&plan.ledger_dir).map_err(|e| e.to_string())?;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(plan.records_path())
        .map_err(|e| e.to_string())?;
    writeln!(f, "{rec}")
        .and_then(|()| f.sync_all())
        .map_err(|e| format!("record append: {e}"))
}

/// The no-overage observation for the next run: the newest record's stream, or for the first
/// run the configured stored stream. `None` when neither exists (the call is then held).
fn observation(plan: &Plan, recs: &[Value]) -> Result<Option<(String, u64)>, String> {
    let newest = recs
        .iter()
        .rev()
        .find(|r| r["stream"]["path"].is_string() && r["ended_unix"].is_u64());
    let (path, at): (PathBuf, u64) = match (newest, &plan.first_observation) {
        (Some(r), _) => (
            r["stream"]["path"].as_str().unwrap_or_default().into(),
            r["ended_unix"].as_u64().unwrap_or_default(),
        ),
        (None, Some((p, at))) => (p.clone(), *at),
        (None, None) => return Ok(None),
    };
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("observation stream {}: {}", path.display(), e.kind()))?;
    Ok(Some((text, unix_now().saturating_sub(at))))
}

/// Limits for this call: the budget decision, then the role's share.
pub fn limits_for(plan: &Plan, role: &str, recs: &[Value]) -> Result<Limits, String> {
    let target = plan
        .roles
        .get(role)
        .ok_or(format!("role {role} not in the plan"))?;
    let obs = observation(plan, recs)?;
    let l = Limits::for_next(
        &plan.budget,
        recs,
        obs.as_ref().map(|(s, age)| Observation {
            stream: s,
            age_seconds: *age,
        }),
    )
    .map_err(|d| format!("{d:?}"))?;
    let mine: Vec<Value> = recs.iter().filter(|r| r["role"] == role).cloned().collect();
    let s = budget::spent(&mine);
    let sh = target.share;
    if s.runs >= sh.max_runs {
        return Err(format!(
            "role {role}: {} of {} runs of its share used",
            s.runs, sh.max_runs
        ));
    }
    let billable = l
        .billable_room
        .min(sh.billable_tokens.saturating_sub(s.billable_tokens));
    let cache_read = l
        .cache_read_room
        .min(sh.cache_read_tokens.saturating_sub(s.cache_read_tokens));
    if billable == 0 || cache_read == 0 {
        return Err(format!("role {role}: no token room left in its share"));
    }
    let left = Duration::from_secs(sh.total_seconds.saturating_sub(s.seconds));
    let full = l.timeout + l.grace;
    let mut window = full.min(left);
    // the timeout counts as "total" when the time left (budget or share) cut it, not a cap
    let total = l.timeout_is_total || window < full;
    if let Some(m) = target.max_window_seconds {
        window = window.min(Duration::from_secs(m));
    }
    if window <= l.grace {
        return Err(format!(
            "role {role}: {} s left in its share is not more than the stop grace",
            window.as_secs()
        ));
    }
    Ok(Limits {
        timeout: window - l.grace,
        timeout_is_total: total,
        billable_room: billable,
        cache_read_room: cache_read,
        grace: l.grace,
    })
}

/// What [`call`] did.
#[derive(Debug)]
pub enum Called {
    /// Nothing was started (refused or held before the call); no record.
    NotStarted(String),
    /// A run was started; its record was appended.
    Recorded(Value),
}

/// Run one call end to end. `poll` is the status polling interval.
pub fn call(
    plan: &Plan,
    c: &Call,
    adapter: &dyn Adapter,
    poll: Duration,
) -> Result<Called, String> {
    let target = plan
        .roles
        .get(&c.role)
        .ok_or(format!("role {} not in the plan", c.role))?;
    if c.op != "start" && c.op != "reconnect" {
        return Err(format!("op {} is not start or reconnect", c.op));
    }
    let recs = records(plan)?;
    if let Some(r) = recs.iter().find(|r| r["stop_required"] == true) {
        return Ok(Called::NotStarted(format!(
            "record {} asks for a person: {}",
            r["attempt_id"], r["stop_reason"]
        )));
    }
    if recs
        .iter()
        .any(|r| r["attempt_id"] == c.attempt_id.as_str())
    {
        return Ok(Called::NotStarted(format!(
            "attempt {} already recorded",
            c.attempt_id
        )));
    }
    let limits = match limits_for(plan, &c.role, &recs) {
        Ok(l) => l,
        Err(why) => return Ok(Called::NotStarted(why)),
    };
    // the checkpoint before the run: the resume base candidate
    let (st, before) = adapter.send(target, "GET", "/v0/role/status", None)?;
    if st != 200 {
        return Ok(Called::NotStarted(format!(
            "status {st}: {}",
            before["code"]
        )));
    }
    if !before["running"].is_null() {
        return Ok(Called::NotStarted(
            "the role is running another attempt".into(),
        ));
    }
    let mut body = json!({"attempt_id": c.attempt_id, "prompt": c.prompt,
        "coordinator": c.coordinator,
        "limits": {"timeout_ms": limits.timeout.as_millis() as u64,
                   "grace_ms": limits.grace.as_millis() as u64,
                   "billable_room": limits.billable_room,
                   "cache_read_room": limits.cache_read_room,
                   "timeout_is_total": limits.timeout_is_total}});
    body[if c.op == "start" {
        "session_id"
    } else {
        "new_session_id"
    }] = json!(c.session_id);
    let started_at = ieo_evidence::time::now();
    let started_unix = unix_now();
    let base_rec = json!({"format": RECORD_FORMAT, "task_id": c.task_id, "budget": plan.budget.name,
        "attempt_id": c.attempt_id, "command_id": format!("{}-{}", c.role, c.op), "role": c.role,
        "model_request": true, "started_at": started_at, "started_unix": started_unix,
        "limits": body["limits"]});
    let fail = |reason: String| -> Result<Called, String> {
        // a run that may have started but whose end is unknown: unmeasured, a person decides
        let mut r = base_rec.clone();
        r["usage"] = json!({"run": {"state": "unmeasured", "reason": reason}});
        r["stop_required"] = json!(true);
        r["stop_reason"] = json!(reason);
        append(plan, &r)?;
        Ok(Called::Recorded(r))
    };
    match adapter.send(target, "POST", &format!("/v0/role/{}", c.op), Some(&body)) {
        Ok((202, _)) => {}
        Ok((st, b)) => {
            return Ok(Called::NotStarted(format!(
                "{} {st} {}: {}",
                c.op, b["code"], b["reason"]
            )));
        }
        Err(e) => return fail(format!("{} sent but its answer was lost ({e})", c.op)),
    }
    // wait for the end: the window plus a margin, then a stop, then give up
    let deadline =
        std::time::Instant::now() + limits.timeout + limits.grace + Duration::from_secs(60);
    let mut stop_sent = false;
    let last = loop {
        match adapter.send(target, "GET", "/v0/role/status", None) {
            Ok((200, s))
                if s["running"].is_null() && s["last"]["attempt_id"] == c.attempt_id.as_str() =>
            {
                break s["last"].clone();
            }
            _ => {}
        }
        if std::time::Instant::now() > deadline {
            if stop_sent {
                return fail("the run did not end after a stop; its usage is unknown".into());
            }
            let _ = adapter.send(target, "POST", "/v0/role/stop", None);
            stop_sent = true;
        }
        std::thread::sleep(poll);
        if stop_sent && std::time::Instant::now() > deadline + Duration::from_secs(30) {
            return fail("the run did not end after a stop; its usage is unknown".into());
        }
    };
    if let Some(e) = last["error"].as_str() {
        return fail(format!("run error: {e}"));
    }
    let (st, out) = adapter.send(
        target,
        "GET",
        &format!("/v0/role/runs/{}/stdout", c.attempt_id),
        None,
    )?;
    let stream = out["content"].as_str().unwrap_or_default().to_string();
    if st != 200 {
        return fail(format!("stdout not available ({st})"));
    }
    let streams = plan.ledger_dir.join("streams");
    std::fs::create_dir_all(&streams).map_err(|e| e.to_string())?;
    let stream_path = streams.join(format!("{}.jsonl", c.attempt_id));
    std::fs::write(&stream_path, &stream).map_err(|e| format!("stream save: {e}"))?;
    let mode = if last["mode"] == "resume" {
        SessionMode::Resume
    } else {
        SessionMode::New
    };
    let session_id = last["session_id"].as_str().unwrap_or_default().to_string();
    // the base: the caller's own newest record of this session, and only when the service
    // resumed from exactly that run (§11 item 7: the run actually resumed)
    let base = (mode == SessionMode::Resume)
        .then(|| {
            let newest = recs
                .iter()
                .rev()
                .find(|r| r["session"]["session_id"] == session_id.as_str())?;
            let cp_from = before["checkpoint"]["session_cumulative_attempt"].as_str()?;
            (before["checkpoint"]["session_id"] == session_id.as_str()
                && newest["attempt_id"] == cp_from)
                .then_some(())?;
            let session: Measure =
                serde_json::from_value(newest["usage"]["session"].clone()).ok()?;
            Some(ResumeBase {
                task_id: newest["task_id"].as_str().unwrap_or_default().into(),
                attempt_id: cp_from.into(),
                command_id: newest["command_id"].as_str().unwrap_or_default().into(),
                session_id: Some(session_id.clone()),
                session,
                cost_usd_reported: newest["usage"]["cost_usd_reported"].as_f64(),
            })
        })
        .flatten();
    let base_attempt = base.as_ref().map(|b| b.attempt_id.clone());
    let parsed = ieo_evidence::stream::parse(stream.as_bytes(), mode, base);
    let collected_run = serde_json::to_value(&parsed.usage.run).map_err(|e| e.to_string())?;
    let o = &last["observed"];
    let observed = Observed {
        billable: o["billable_lower_bound"].as_u64().unwrap_or(u64::MAX),
        cache_read: o["cache_read_lower_bound"].as_u64().unwrap_or(u64::MAX),
        messages: o["messages"].as_u64().unwrap_or_default() as usize,
        incomplete_lines: o["incomplete_lines"].as_u64().unwrap_or_default() as usize,
    };
    let stop = last["stop"].as_str().unwrap_or("unknown");
    let charged = run::charge_reported(stop == "Exited", stop, &observed, &collected_run);
    let mut usage = serde_json::to_value(&parsed.usage).map_err(|e| e.to_string())?;
    usage["collector_run"] = usage["run"].take();
    usage["run"] = charged;
    // reasons a person must look before the next call
    let mut reasons = Vec::new();
    if last["cleanup_failed"] == true {
        reasons.push("process-group cleanup failed".to_string());
    }
    let env: Vec<&str> = last["env_names"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    if !env.contains(&"CLAUDE_CODE_OAUTH_TOKEN") || env.iter().any(|n| n.starts_with("ANTHROPIC_"))
    {
        reasons.push(format!("environment not subscription-only: {env:?}"));
    }
    match budget::rate_limit_events(&stream) {
        Ok(evs) => {
            for e in &evs {
                if let Err(why) = budget::overage_check(e) {
                    reasons.push(format!("this run's signal: {why}"));
                }
            }
        }
        Err(e) => reasons.push(format!("this run's signal: {e}")),
    }
    let ended_unix = unix_now();
    let mut r = base_rec;
    r["ended_at"] = json!(ieo_evidence::time::now());
    r["ended_unix"] = json!(ended_unix);
    if let Some(ms) = last["elapsed_ms"].as_u64() {
        r["duration_ms"] = json!(ms);
    }
    r["session"] = json!({"mode": last["mode"], "session_id": session_id,
                          "resume_base_attempt": base_attempt});
    r["outcome"] = json!({"stop": stop, "stopped_by_guard": last["stopped_by_guard"],
        "exit_code": last["exit_code"], "cleanup_failed": last["cleanup_failed"],
        "observed": last["observed"], "tamper": last["tamper"],
        "config_cleanup": last["config_cleanup"], "env_names": last["env_names"]});
    r["result_event"] = serde_json::to_value(&parsed.result).unwrap_or(Value::Null);
    r["usage"] = usage;
    r["stream"] = json!({"path": stream_path, "bytes": stream.len(),
                         "sha256": hex::encode(Sha256::digest(stream.as_bytes()))});
    r["stop_required"] = json!(!reasons.is_empty());
    if !reasons.is_empty() {
        r["stop_reason"] = json!(reasons.join("; "));
    }
    append(plan, &r)?;
    Ok(Called::Recorded(r))
}

/// Ledger file of a plan, for reports.
pub fn records_path(plan: &Plan) -> PathBuf {
    plan.records_path()
}

/// Shares as JSON (for a role config `share`).
pub fn share_json(s: &Share) -> Value {
    json!({"max_runs": s.max_runs, "total_seconds": s.total_seconds,
           "billable_tokens": s.billable_tokens, "cache_read_tokens": s.cache_read_tokens})
}
