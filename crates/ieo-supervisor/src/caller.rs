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
//!    `rate_limit_event` must pass [`budget::rate_limit_verdict`]: subscription `allowed` or
//!    `allowed_warning`, `overageStatus: "rejected"`, `isUsingOverage: false`; a warning is
//!    recorded as `rate_limit_warnings`), the role environment (the run's
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
use std::path::{Path, PathBuf};
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
        let budget = match v["budget"].as_str().and_then(budget::named) {
            Some(b) => b,
            None => return Err("plan.budget must be 02-04, X-02 or 02-05".into()),
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
            // a budget whose approval fixes the role shares (02-05): exactly those, never
            // configured; a share given in the plan must equal them
            let share = if budget.role_shares.is_empty() {
                Share {
                    max_runs: f("max_runs")?,
                    total_seconds: f("total_seconds")?,
                    billable_tokens: f("billable_tokens")?,
                    cache_read_tokens: f("cache_read_tokens")?,
                }
            } else {
                let a = budget.approved_share(name).ok_or(format!(
                    "roles.{name}: the {} budget approves no share for this role",
                    budget.name
                ))?;
                let approved = Share {
                    max_runs: a.max_runs,
                    total_seconds: a.total_seconds,
                    billable_tokens: a.billable_tokens,
                    cache_read_tokens: a.cache_read_tokens,
                };
                if !sh.is_null() {
                    let given = Share {
                        max_runs: f("max_runs")?,
                        total_seconds: f("total_seconds")?,
                        billable_tokens: f("billable_tokens")?,
                        cache_read_tokens: f("cache_read_tokens")?,
                    };
                    if given != approved {
                        return Err(format!(
                            "roles.{name}.share differs from the {} budget's approved share \
                             (omit it or give exactly the approved values)",
                            budget.name
                        ));
                    }
                }
                approved
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

pub const CORRECTION_FORMAT: &str = "ieo-role-run-correction/v0";

/// The records of the budget as they count: [`raw_records`] with each correction applied to the
/// run it corrects (the original line stays in the file; the corrected run carries
/// `corrected_by` and the original values under `before_correction`). Corrections are not runs.
///
/// A correction is not trusted for what it says: it is measured again from the kept stream and
/// child transcripts it names ([`remeasure`], the same rules as [`reevaluate`]), and its usage
/// must equal that result (review 608b367 F1). Fail-closed: a correction naming an unknown or
/// later run, a second correction of one run, a missing or changed file, or any difference
/// makes the ledger unusable.
pub fn records(plan: &Plan) -> Result<Vec<Value>, String> {
    let recs = effective(raw_records(plan)?, &plan.ledger_dir)?;
    // one ledger belongs to one budget (02-05 budget PR): a record of another budget is never
    // counted for, or charged to, this one; the ledger is refused instead (fail closed)
    if let Some(other) = recs
        .iter()
        .filter_map(|r| r["budget"].as_str())
        .find(|b| *b != plan.budget.name)
    {
        return Err(format!(
            "the ledger {} holds records of the budget {other}, not {}: one ledger per budget \
             (usage is never transferred)",
            plan.ledger_dir.display(),
            plan.budget.name
        ));
    }
    Ok(recs)
}

fn effective(raw: Vec<Value>, ledger_dir: &Path) -> Result<Vec<Value>, String> {
    let mut runs: Vec<Value> = Vec::new();
    let mut done = std::collections::BTreeSet::new();
    let mut settled = std::collections::BTreeSet::new();
    for line in raw {
        if line["format"] == crate::settlement::SETTLEMENT_FORMAT {
            // a settlement (user decision 2026-11-17 r1-settlement): checked again from its
            // files on every read, applied to the earlier run it names, one per run, never to a
            // corrected run (no double counting: the run counts once, the line is no run)
            let id = line["settles"]
                .as_str()
                .ok_or("settlement without `settles`")?
                .to_owned();
            if !settled.insert(id.clone()) {
                return Err(format!("run {id} has more than one settlement"));
            }
            let pos = runs
                .iter()
                .position(|r| r["attempt_id"] == id.as_str())
                .ok_or(format!("settlement of an unknown or later run {id}"))?;
            if !runs[pos]["corrected_by"].is_null() {
                return Err(format!("settlement of {id}: the run is corrected already"));
            }
            let sp = runs[pos]["stream"]["path"].as_str().unwrap_or_default();
            let stream = std::fs::read_to_string(sp).map_err(|e| {
                format!(
                    "settlement of {id}: stream {sp}: {}{}",
                    e.kind(),
                    moved_hint(&e)
                )
            })?;
            runs[pos] = crate::settlement::apply(&runs[pos], &line, &stream, ledger_dir)?;
            continue;
        }
        if line["format"] != CORRECTION_FORMAT {
            // what only a settlement sets is never taken from a written run line
            if !line["settlement"].is_null() || !line["before_settlement"].is_null() {
                return Err(format!(
                    "record {} carries settlement fields itself",
                    line["attempt_id"]
                ));
            }
            runs.push(line);
            continue;
        }
        let c = line;
        let id = c["corrects"]
            .as_str()
            .ok_or("correction without `corrects`")?
            .to_owned();
        if !done.insert(id.clone()) {
            return Err(format!("run {id} has more than one correction"));
        }
        if settled.contains(&id) {
            return Err(format!("correction of {id}: the run is settled already"));
        }
        let pos = runs
            .iter()
            .position(|r| r["attempt_id"] == id.as_str())
            .ok_or(format!("correction of an unknown or later run {id}"))?;
        if runs[pos]["stream"]["sha256"] != c["original_stream_sha256"]
            || c["after"]["stop_required"] != false
            || c["decision"].as_str().is_none_or(str::is_empty)
        {
            return Err(format!("correction of {id} does not match the run"));
        }
        let mut transcripts = std::collections::BTreeMap::new();
        for t in c["child_transcripts"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            let (Some(a), Some(p)) = (t["agent_id"].as_str(), t["path"].as_str()) else {
                return Err(format!(
                    "correction of {id}: child transcript entry incomplete"
                ));
            };
            // read again on every ledger read: only an absolute path (review 12c9a21 N1)
            if !std::path::Path::new(p).is_absolute() {
                return Err(format!(
                    "correction of {id}: transcript path {p} is not absolute"
                ));
            }
            transcripts.insert(a.to_owned(), PathBuf::from(p));
        }
        let (usage, kept) = remeasure(&runs[pos], &runs[..pos], &transcripts)
            .map_err(|e| format!("correction of {id} does not hold: {e}"))?;
        if usage["run"] != c["after"]["usage"]["run"] || json!(kept) != c["child_transcripts"] {
            return Err(format!(
                "correction of {id} differs from its measurement again"
            ));
        }
        let r = &mut runs[pos];
        r["before_correction"] = json!({"usage": r["usage"].take(),
            "stop_required": r["stop_required"], "stop_reason": r["stop_reason"]});
        r["usage"] = usage;
        r["stop_required"] = json!(false);
        r["stop_reason"] = Value::Null;
        r["corrected_by"] = json!({"decision": c["decision"], "corrected_at": c["corrected_at"]});
    }
    Ok(runs)
}

/// All lines of the ledger as written, fail-closed: an unreadable line is an error.
pub fn raw_records(plan: &Plan) -> Result<Vec<Value>, String> {
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
    // one write of the whole line under an exclusive lock (review c4620d2 L2)
    let line = format!("{rec}\n");
    f.lock()
        .and_then(|()| f.write_all(line.as_bytes()))
        .and_then(|()| f.sync_all())
        .map_err(|e| format!("record append: {e}"))
}

fn inflight_path(plan: &Plan) -> PathBuf {
    plan.ledger_dir.join("inflight.json")
}

/// Marks a call as started before the start request is sent: until the call's record is
/// appended, every further call is refused (a caller that dies mid-run leaves it behind, and a
/// person reconciles it — review c4620d2 H1).
fn mark_inflight(plan: &Plan, c: &Call, delivery_id: Option<&str>) -> Result<(), String> {
    use std::io::Write;
    std::fs::create_dir_all(&plan.ledger_dir).map_err(|e| e.to_string())?;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(inflight_path(plan))
        .map_err(|e| format!("in-flight marker: {e}"))?;
    let mut v = json!({"attempt_id": c.attempt_id, "role": c.role, "op": c.op,
                      "started_at": ieo_evidence::time::now()});
    if let Some(id) = delivery_id {
        v["delivery_id"] = json!(id);
    }
    f.write_all(format!("{v}\n").as_bytes())
        .and_then(|()| f.sync_all())
        .and_then(|()| std::fs::File::open(&plan.ledger_dir)?.sync_all())
        .map_err(|e| format!("in-flight marker: {e}"))
}

fn clear_inflight(plan: &Plan) -> Result<(), String> {
    std::fs::remove_file(inflight_path(plan)).map_err(|e| format!("in-flight marker: {e}"))
}

/// How long a call waits for `caller.lock` before it is refused. Production never needs the
/// wait: `ieo-role-call` is one process that starts none, and `call.sh` holds the lock only
/// around a restart. It serves the in-process test harness, where a process forked by another
/// test thread holds a copy of the previous call's lock until it execs (reviews f90f5b5 N1,
/// dc11612 L-c3-2). A real second caller stays refused.
pub const LOCK_WAIT: Duration = Duration::from_secs(3);

/// The ledger names its files (streams, child transcripts) by absolute path, so a moved or
/// renamed ledger directory makes every read refuse (review 71e408a P4). Said with the error.
fn moved_hint(e: &std::io::Error) -> &'static str {
    if e.kind() == std::io::ErrorKind::NotFound {
        " (the ledger names its files by absolute path: a ledger directory moved or renamed \
         while its budget is open is refused; move it back)"
    } else {
        ""
    }
}

/// A record that asks for a person stops every call until it is reconciled.
fn asks_for_a_person(recs: &[Value]) -> Option<String> {
    asks_for_a_person_but(recs, None)
}

/// [`asks_for_a_person`], where the failed run a spare replaces counts as answered: the spare
/// marker (a reason and that attempt) is the person's recorded decision about it (02-05 budget,
/// user decision 2026-11-17 (나)). A spare recorded earlier answers its run for every later
/// call; `this_spare` is the one the call carries. Every other record that asks for a person
/// still stops the call; an unmeasured run still stops the budget ([`budget::next`]).
fn asks_for_a_person_but(recs: &[Value], this_spare: Option<&str>) -> Option<String> {
    let answered: Vec<&str> = recs
        .iter()
        .filter_map(|r| r["spare"]["replaces"].as_str())
        .chain(this_spare)
        .collect();
    recs.iter()
        .filter(|r| {
            r["attempt_id"]
                .as_str()
                .is_none_or(|a| !answered.contains(&a))
        })
        .find(|r| r["stop_required"] == true)
        .map(|r| {
            format!(
                "record {} asks for a person: {}",
                r["attempt_id"], r["stop_reason"]
            )
        })
}

/// The services' state before a call or a restart, for **every** role of the plan (`first`
/// asked first): each answers its status, runs nothing, and the last attempt it ran and the
/// attempt its persisted checkpoint names are known to the ledger (reviews f90f5b5 N2, dc11612
/// L-c3-3: a run of one role that the ledger lost blocks the other role too). `Ok(Ok(statuses
/// by role))`, `Ok(Err(why))` for a refusal, `Err` when a service cannot be asked.
fn services_clear(
    plan: &Plan,
    recs: &[Value],
    adapter: &dyn Adapter,
    first: &str,
) -> Result<Result<BTreeMap<String, Value>, String>, String> {
    let known = |a: &str| recs.iter().any(|r| r["attempt_id"] == a);
    let mut order: Vec<&String> = plan.roles.keys().filter(|r| *r == first).collect();
    order.extend(plan.roles.keys().filter(|r| *r != first));
    let mut out = BTreeMap::new();
    for role in order {
        let target = &plan.roles[role];
        let (st, s) = adapter.send(target, "GET", "/v0/role/status", None)?;
        if st != 200 {
            return Ok(Err(format!("role {role}: status {st}: {}", s["code"])));
        }
        if !s["running"].is_null() {
            return Ok(Err(format!("role {role} is running another attempt")));
        }
        if let Some(a) = s["last"]["attempt_id"].as_str().filter(|a| !known(a)) {
            return Ok(Err(format!(
                "role {role}: the service's last attempt {a} has no caller record: a person \
                 reconciles it"
            )));
        }
        // the persisted checkpoint survives restarts (the in-memory `last` does not): the run
        // it names must be known too (review f90f5b5 N2)
        if let Some(a) = s["checkpoint"]["session_cumulative_attempt"]
            .as_str()
            .filter(|a| !known(a))
        {
            return Ok(Err(format!(
                "role {role}: the service's checkpoint names attempt {a}, which has no caller \
                 record: a person reconciles it"
            )));
        }
        out.insert(role.clone(), s);
    }
    Ok(Ok(out))
}

/// The guards of a call without the call (read only, no lock taken, nothing written): no call
/// in flight, the ledger reads (every correction measured again), no record asks for a person,
/// and every service is idle with its runs known. `call.sh` runs it under `caller.lock` before
/// a container restart (reviews 12c9a21 N6, dc11612 L-c3-3/L-c3-4). `Ok(None)`: clear;
/// `Ok(Some(why))`: refused; `Err`: the ledger or a service could not be read.
pub fn check(plan: &Plan, adapter: &dyn Adapter) -> Result<Option<String>, String> {
    if let Ok(t) = std::fs::read_to_string(inflight_path(plan)) {
        return Ok(Some(format!(
            "a call is in flight without a record ({}): a person reconciles it",
            t.trim()
        )));
    }
    let recs = records(plan)?;
    if let Some(why) = asks_for_a_person(&recs) {
        return Ok(Some(why));
    }
    let first = plan.roles.keys().next().cloned().unwrap_or_default();
    Ok(services_clear(plan, &recs, adapter, &first)?.err())
}

/// Ledger files are private (review 71e408a P3): a directory created here is 0700, a file
/// created here 0600.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    if let Some(d) = path.parent() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(d)?;
    }
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(bytes).and_then(|()| f.sync_all())
}

/// An agent id usable in a file name and a URL path: letters, digits, `-` and `_` only (review
/// 71e408a P1: never a path separator or `..`).
fn safe_agent_id(a: &str) -> bool {
    !a.is_empty()
        && a.len() <= 128
        && a.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// The children of a stream: the `agentId` of every `tool_use_result`.
fn stream_agents(stream: &[u8]) -> std::collections::BTreeSet<String> {
    stream
        .split(|b| *b == b'\n')
        .filter_map(|l| serde_json::from_slice::<Value>(l).ok())
        .filter_map(|v| v["tool_use_result"]["agentId"].as_str().map(str::to_owned))
        .collect()
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
    let text = std::fs::read_to_string(&path).map_err(|e| {
        format!(
            "observation stream {}: {}{}",
            path.display(),
            e.kind(),
            moved_hint(&e)
        )
    })?;
    let now = unix_now();
    if now < at {
        // a clock before the observation would make a stale observation look fresh (L1)
        return Err(format!("clock ({now}) is before the observation ({at})"));
    }
    Ok(Some((text, now - at)))
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
    call_with(plan, c, adapter, poll, Duration::from_secs(60))
}

/// [`call`] with the wait margin after the run's window (and again after a stop).
///
/// Any exit other than "recorded, nothing to look at" or "not started" means stop: an `Err`
/// before the start request starts nothing; after it, the run is recorded as unmeasured with
/// `stop_required`, and if even that record cannot be written the in-flight marker stays and
/// blocks every further call.
///
/// It is the three steps of the shared budget check in a row: [`admit`], [`Admission::start`],
/// [`Running::finish`].
pub fn call_with(
    plan: &Plan,
    c: &Call,
    adapter: &dyn Adapter,
    poll: Duration,
    margin: Duration,
) -> Result<Called, String> {
    call_with_extra(plan, c, adapter, poll, margin, &json!({}))
}

/// [`call_with`] with extra top-level keys in the start body (02-05 step 7b: the approved
/// `recovery_id` of a start over an unfinished run).
pub fn call_with_extra(
    plan: &Plan,
    c: &Call,
    adapter: &dyn Adapter,
    poll: Duration,
    margin: Duration,
    extra: &Value,
) -> Result<Called, String> {
    plan.roles
        .get(&c.role)
        .ok_or(format!("role {} not in the plan", c.role))?;
    if c.op != "start" && c.op != "reconnect" {
        return Err(format!("op {} is not start or reconnect", c.op));
    }
    let adm = match admit_with(plan, &c.role, &c.attempt_id, adapter, extra)? {
        Ok(a) => a,
        Err(why) => return Ok(Called::NotStarted(why)),
    };
    match adm.start_with(plan, c, adapter, extra)? {
        Sent::Refused { why, .. } => Ok(Called::NotStarted(why)),
        Sent::Lost(r) => Ok(Called::Recorded(r)),
        Sent::Accepted { running, .. } => {
            running.finish(adapter, poll, margin).map(Called::Recorded)
        }
    }
}

// ---------------------------------------------------------------------------
// The shared budget check (02-05 step 3, PLAN §3 "예산 검사의 위치")
// ---------------------------------------------------------------------------
//
// The host caller above and the coordinator's delivery path (`deliverAssignment`) call a role
// service only through these three steps, so every run of a budget passes the same checks with
// the same code, outside the role containers (runtime contract §2.1): the ledger read fail-closed,
// a record that asks for a person, an unmeasured run, the no-extra-billing observation, the
// limits of the budget and the role's share, every service idle with its runs known, the
// in-flight marker, and after the run the measurement and the charge with `stop_required`.

/// A call the budget check admitted. It holds the ledger lock (`caller.lock`) until the call's
/// record is appended or the call is refused, so one call of a budget runs at a time.
#[derive(Debug)]
pub struct Admission {
    lock: std::fs::File,
    recs: Vec<Value>,
    limits: Limits,
    before: Value,
    /// The spare marker this call carries, checked (02-05 budget; written to its record).
    spare: Option<Value>,
}

/// The spare gate of a budget that has one (user decision 2026-11-17 (나)): `Ok(None)` when the
/// call is no spare, `Ok(Some(marker))` for an allowed spare, `Err(why)` otherwise (nothing
/// starts). Without a marker the role runs at most its `planned_runs` (runs that are no spare);
/// the spare needs `extra.spare = {reason, replaces, evidence_ref, by, approved_at}` (user
/// decision 2026-11-17, addition): a person (`by.kind` "user") names the earlier failed run of
/// this role and budget it replaces, a reason, and a reference to the cause report or evidence
/// (the reason alone proves no environment defect). The replaced run's effective record is a
/// failure (`stop_required`, or a result that is not a clean success), and its stop is one the
/// marker may release: the run's result only ([`spare_releasable`]); unmeasured usage, a
/// limit or overage signal, a failed cleanup or an environment that is not subscription-only
/// keep blocking. The spare may redo either planned run (R1 or R2), so it need not be the
/// third call. One spare only; a role without a spare never carries a marker. An unreadable or
/// ambiguous record refuses.
pub fn spare_gate(
    plan: &Plan,
    role: &str,
    recs: &[Value],
    raw: &[Value],
    extra: &Value,
) -> Result<Option<Value>, String> {
    let marker = &extra["spare"];
    let Some(rule) = plan.budget.spare.filter(|r| r.role == role) else {
        if marker.is_null() {
            return Ok(None);
        }
        return Err(format!(
            "role {role} has no spare run in the {} budget",
            plan.budget.name
        ));
    };
    // the role's runs that are no spare (its planned runs)
    let planned_used = recs
        .iter()
        .filter(|r| r["role"] == role && r["model_request"] == true && r["spare"].is_null())
        .count() as u64;
    if marker.is_null() {
        if planned_used < rule.planned_runs {
            return Ok(None);
        }
        return Err(format!(
            "role {role}: its {} planned runs are used; the spare run needs a marker with a \
             person's reason, evidence reference and the failed run it replaces (deliverAssignment \
             `spare`, a user credential)",
            rule.planned_runs
        ));
    }
    if recs
        .iter()
        .any(|r| r["role"] == role && !r["spare"].is_null())
    {
        return Err(format!(
            "role {role}: the spare run of the {} budget was used already",
            plan.budget.name
        ));
    }
    let reason = marker["reason"].as_str().filter(|r| !r.trim().is_empty());
    let replaces = marker["replaces"].as_str().filter(|r| !r.is_empty());
    let (Some(reason), Some(replaces)) = (reason, replaces) else {
        return Err("the spare marker needs a reason and the attempt it replaces".into());
    };
    // the cause report or evidence the person states (user decision 2026-11-17, addition)
    let Some(evidence_ref) = marker["evidence_ref"]
        .as_str()
        .filter(|r| !r.trim().is_empty())
    else {
        return Err("the spare marker needs an evidence_ref (the cause report or evidence)".into());
    };
    // who attached it and when (review b38e8df M-1): a person; the record keeps it
    let by = &marker["by"];
    if by["kind"] != "user" || by["name"].as_str().is_none_or(|n| n.trim().is_empty()) {
        return Err("the spare marker is attached by a person (`by`: kind user, a name)".into());
    }
    let Some(approved_at) = marker["approved_at"].as_str().filter(|a| !a.is_empty()) else {
        return Err("the spare marker has no approval time (`approved_at`)".into());
    };
    let found: Vec<&Value> = raw
        .iter()
        .filter(|r| r["format"] == RECORD_FORMAT && r["attempt_id"] == replaces)
        .collect();
    let [r] = found.as_slice() else {
        return Err(format!(
            "spare: the replaced attempt {replaces} is not one record of the ledger ({} found)",
            found.len()
        ));
    };
    // the raw line decides "one record"; the effective record (a correction applied) decides
    // what the run was (review b38e8df L-1)
    let _one = r;
    let Some(r) = recs.iter().find(|e| e["attempt_id"] == replaces) else {
        return Err(format!(
            "spare: the replaced attempt {replaces} has no effective record"
        ));
    };
    if r["role"] != role || r["model_request"] != true || r["budget"] != plan.budget.name {
        return Err(format!(
            "spare: the replaced attempt {replaces} is not a {role} run of the {} budget",
            plan.budget.name
        ));
    }
    let result = &r["outcome"]["result"];
    let failed = r["stop_required"] == true
        || !(result["subtype"] == "success" && result["is_error"] == false);
    if !failed {
        return Err(format!(
            "spare: the replaced attempt {replaces} did not fail (a spare replaces a failed run)"
        ));
    }
    if let Err(why) = spare_releasable(r) {
        return Err(format!(
            "spare: the stop of {replaces} is not one a spare releases: {why}"
        ));
    }
    Ok(Some(
        serde_json::json!({"reason": reason, "replaces": replaces,
        "evidence_ref": evidence_ref, "by": by, "approved_at": approved_at}),
    ))
}

/// The stop reasons of a run's own result (`result <subtype> (is_error …)`, `no result event`):
/// the only reasons a spare marker releases ([`spare_releasable`]).
const RESULT_STOP_REASON: &str = "result ";
const NO_RESULT_STOP_REASON: &str = "no result event";

fn is_result_stop_reason(p: &str) -> bool {
    p.starts_with(RESULT_STOP_REASON) || p == NO_RESULT_STOP_REASON
}

/// Every stop reason that is **not** the run's own result goes through here (PR #81 review
/// 291a9b0 N-2): it never contains the separator `"; "` and never reads as a result reason, so a
/// new reason added later cannot become releasable by its wording. A record's `stop_reason` is
/// its reasons joined with `"; "`.
pub fn other_stop_reason(reason: impl Into<String>) -> String {
    let s = reason.into().replace("; ", ", ");
    if is_result_stop_reason(&s) {
        format!("other: {s}")
    } else {
        s
    }
}

/// Whether a spare marker may release the stop of the failed run it names (user decision
/// 2026-11-17, addition, §3): only a stop for the run's own result ("result …", "no result
/// event"), and only with its usage measured. Any other reason (unmeasured usage, a limit or
/// overage signal, a failed process-group cleanup, an environment that is not
/// subscription-only, an answer that was lost) keeps blocking. Fail-closed: a reason not on
/// the list keeps blocking.
pub fn spare_releasable(r: &Value) -> Result<(), String> {
    if r["usage"]["run"]["state"] != "measured" {
        return Err(format!("usage {}", r["usage"]["run"]["state"]));
    }
    // a run the guard stopped has no result of its own: its "no result event" is the guard's,
    // and only a settled stop on the old rule's warning may be released (user decision
    // 2026-11-17 r1-settlement §2.3, scoped to that case; PR #89 review 91e87ce L-4). Real
    // rejects, extra usage, time or token caps and other unmeasured runs are never settled, so
    // they keep blocking.
    if r["outcome"]["stopped_by_guard"] == true
        && (r["settlement"]["policy"] != crate::settlement::POLICY
            || crate::settlement::pinned(r).is_err())
    {
        return Err(format!(
            "the guard stopped the run ({}) and it has no settlement",
            r["outcome"]["stop"]
        ));
    }
    if r["stop_required"] != true {
        return Ok(());
    }
    let reason = r["stop_reason"].as_str().unwrap_or_default();
    if reason.is_empty() {
        return Err("a stop without a reason".into());
    }
    match reason.split("; ").find(|p| !is_result_stop_reason(p)) {
        None => Ok(()),
        Some(p) => Err(p.to_string()),
    }
}

/// The ledger's effective records when no call of the budget is running (02-05 step 5): the
/// ledger lock is free and no in-flight marker exists, so every run started so far has its
/// record. `Ok(None)` when a call holds the lock or a marker is left (nothing can be said yet);
/// the lock is taken only for the read, without waiting.
pub fn records_when_idle(plan: &Plan) -> Result<Option<Vec<Value>>, String> {
    let path = plan.ledger_dir.join("caller.lock");
    if !plan.ledger_dir.exists() {
        // a budget with no call yet: no records
        return Ok(Some(Vec::new()));
    }
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .map_err(|e| format!("caller lock: {e}"))?;
    match lock.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => return Ok(None),
        Err(std::fs::TryLockError::Error(e)) => return Err(format!("caller lock: {e}")),
    }
    if inflight_path(plan).exists() {
        return Ok(None);
    }
    records(plan).map(Some)
}

/// Step 1: the checks before a call (read only apart from the ledger lock). `Ok(Err(why))`: not
/// admitted, nothing to record; `Err`: the ledger or a service could not be read.
pub fn admit(
    plan: &Plan,
    role: &str,
    attempt_id: &str,
    adapter: &dyn Adapter,
) -> Result<Result<Admission, String>, String> {
    admit_with(plan, role, attempt_id, adapter, &json!({}))
}

/// [`admit`] for a call whose start carries `extra` (its spare marker is checked here).
pub fn admit_with(
    plan: &Plan,
    role: &str,
    attempt_id: &str,
    adapter: &dyn Adapter,
    extra: &Value,
) -> Result<Result<Admission, String>, String> {
    plan.roles
        .get(role)
        .ok_or(format!("role {role} not in the plan"))?;
    // one caller at a time over the ledger
    std::fs::create_dir_all(&plan.ledger_dir).map_err(|e| e.to_string())?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(plan.ledger_dir.join("caller.lock"))
        .map_err(|e| format!("caller lock: {e}"))?;
    let t0 = std::time::Instant::now();
    loop {
        match lock.try_lock() {
            Ok(()) => break,
            Err(std::fs::TryLockError::WouldBlock) if t0.elapsed() < LOCK_WAIT => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                return Ok(Err("another caller holds the ledger".into()));
            }
            Err(std::fs::TryLockError::Error(e)) => return Err(format!("caller lock: {e}")),
        }
    }
    if let Ok(t) = std::fs::read_to_string(inflight_path(plan)) {
        return Ok(Err(format!(
            "a call is in flight without a record ({}): a person reconciles it",
            t.trim()
        )));
    }
    let recs = records(plan)?;
    // the spare gate first: a valid spare answers the failed run it replaces
    let spare = match spare_gate(plan, role, &recs, &raw_records(plan)?, extra) {
        Ok(s) => s,
        Err(why) => return Ok(Err(why)),
    };
    if let Some(why) =
        asks_for_a_person_but(&recs, spare.as_ref().and_then(|s| s["replaces"].as_str()))
    {
        return Ok(Err(why));
    }
    if recs.iter().any(|r| r["attempt_id"] == attempt_id) {
        return Ok(Err(format!("attempt {attempt_id} already recorded")));
    }
    let limits = match limits_for(plan, role, &recs) {
        Ok(l) => l,
        Err(why) => return Ok(Err(why)),
    };
    // every service's state before the run (the target's checkpoint is the resume base
    // candidate)
    let mut statuses = match services_clear(plan, &recs, adapter, role)? {
        Ok(s) => s,
        Err(why) => return Ok(Err(why)),
    };
    let before = statuses.remove(role).unwrap_or_default();
    Ok(Ok(Admission {
        lock,
        recs,
        limits,
        before,
        spare,
    }))
}

/// What sending an admitted call did.
#[derive(Debug)]
pub enum Sent {
    /// The service answered with a refusal: nothing started, nothing recorded.
    Refused {
        status: u16,
        body: Value,
        why: String,
    },
    /// Sent, but its answer was lost: recorded as an unmeasured run that asks for a person.
    Lost(Value),
    /// Accepted (202): the run goes on; [`Running::finish`] records it.
    Accepted {
        answer: Value,
        running: Box<Running>,
    },
}

impl Admission {
    /// The ledger's effective records as this admission read them (under the ledger lock, no
    /// call in flight): the authority for what every earlier run of the budget used (02-05
    /// step 5, assignment-level limits).
    pub fn records(&self) -> &[Value] {
        &self.recs
    }

    /// The limits the budget and the role's share allow for this call.
    pub fn limits(&self) -> Limits {
        self.limits
    }

    /// Lower the limits (never raise them): the window (timeout + grace) to at most
    /// `window_seconds`, the billable room to at most `billable`. A window not above the stop
    /// grace is a refusal (`Err(why)`), as in [`limits_for`].
    pub fn lower(
        &mut self,
        window_seconds: Option<u64>,
        billable: Option<u64>,
    ) -> Result<(), String> {
        // checked first, changed after: a refusal leaves the limits as they were
        let mut l = self.limits;
        if let Some(w) = window_seconds {
            let full = l.timeout + l.grace;
            let window = full.min(Duration::from_secs(w));
            if window <= l.grace {
                return Err(format!("a window of {w} s is not more than the stop grace"));
            }
            if window < full {
                l.timeout = window - l.grace;
                l.timeout_is_total = true;
            }
        }
        if let Some(b) = billable {
            if b == 0 {
                return Err("no billable token room".into());
            }
            l.billable_room = l.billable_room.min(b);
        }
        self.limits = l;
        Ok(())
    }

    /// The start or reconnect request body of `c` under these limits.
    pub fn body(&self, c: &Call) -> Value {
        let limits = &self.limits;
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
        body
    }

    /// Step 2: mark the call in flight and send it ([`Admission::body`], plus `extra` keys).
    pub fn start(self, plan: &Plan, c: &Call, adapter: &dyn Adapter) -> Result<Sent, String> {
        self.start_with(plan, c, adapter, &json!({}))
    }

    /// [`Admission::start`] with extra top-level keys in the body (e.g. `target_sha`).
    pub fn start_with(
        self,
        plan: &Plan,
        c: &Call,
        adapter: &dyn Adapter,
        extra: &Value,
    ) -> Result<Sent, String> {
        let target = plan
            .roles
            .get(&c.role)
            .ok_or(format!("role {} not in the plan", c.role))?
            .clone();
        let mut body = self.body(c);
        // the spare marker is the budget check's, not the role service's (review b38e8df I-4)
        for (k, v) in extra
            .as_object()
            .into_iter()
            .flatten()
            .filter(|(k, _)| *k != "spare")
        {
            body[k] = v.clone();
        }
        let base_rec = json!({"format": RECORD_FORMAT, "task_id": c.task_id, "budget": plan.budget.name,
            "attempt_id": c.attempt_id, "command_id": format!("{}-{}", c.role, c.op), "role": c.role,
            "model_request": true, "started_at": ieo_evidence::time::now(),
            "started_unix": unix_now(), "limits": body["limits"]});
        // the spare marker, as checked at admission (02-05 budget)
        let mut base_rec = base_rec;
        if let Some(s) = &self.spare {
            base_rec["spare"] = s.clone();
        }
        let delivery_id = body["delivery_id"].as_str();
        if let Some(id) = delivery_id {
            base_rec["delivery_id"] = json!(id);
        }
        mark_inflight(plan, c, delivery_id)?;
        match adapter.send(&target, "POST", &format!("/v0/role/{}", c.op), Some(&body)) {
            Ok((202, answer)) => Ok(Sent::Accepted {
                answer,
                running: Box::new(Running {
                    _lock: self.lock,
                    plan: plan.clone(),
                    c: c.clone(),
                    target,
                    limits: self.limits,
                    before: self.before,
                    recs: self.recs,
                    base_rec,
                }),
            }),
            Ok((st, b)) => {
                // refused: nothing started by this call. A 200 `replayed` is the stored answer
                // of the same attempt accepted earlier — by a caller on another ledger, since
                // `admit` refuses an attempt this ledger knows (review c11e734 I-4); this call
                // started nothing and records nothing
                clear_inflight(plan)?;
                let why = if st == 200 && b["replayed"] == true {
                    format!(
                        "{} 200 replayed: attempt {} was accepted earlier, not by this call",
                        c.op, c.attempt_id
                    )
                } else {
                    format!("{} {st} {}: {}", c.op, b["code"], b["reason"])
                };
                Ok(Sent::Refused {
                    why,
                    status: st,
                    body: b,
                })
            }
            Err(e) => Ok(Sent::Lost(record_lost(
                plan,
                &base_rec,
                format!("{} sent but its answer was lost ({e})", c.op),
            )?)),
        }
    }
}

/// A run that may have started but whose end or usage is unknown: unmeasured, no duration, a
/// person decides.
fn record_lost(plan: &Plan, base_rec: &Value, reason: String) -> Result<Value, String> {
    let mut r = base_rec.clone();
    r["usage"] = json!({"run": {"state": "unmeasured", "reason": reason}});
    r["stop_required"] = json!(true);
    r["stop_reason"] = json!(other_stop_reason(reason));
    append(plan, &r)?;
    clear_inflight(plan)?;
    Ok(r)
}

/// An accepted call whose record is not written yet. It keeps the ledger lock: no other call of
/// the budget starts until [`Running::finish`] returns (or the process ends, which leaves the
/// in-flight marker for a person).
#[derive(Debug)]
pub struct Running {
    _lock: std::fs::File,
    plan: Plan,
    c: Call,
    target: RoleTarget,
    limits: Limits,
    before: Value,
    recs: Vec<Value>,
    base_rec: Value,
}

impl Running {
    /// The attempt this run belongs to.
    pub fn attempt_id(&self) -> &str {
        &self.c.attempt_id
    }

    /// Step 3: wait for the run's end, measure and charge it, append its record.
    pub fn finish(
        self,
        adapter: &dyn Adapter,
        poll: Duration,
        margin: Duration,
    ) -> Result<Value, String> {
        let ctx = After {
            plan: &self.plan,
            c: &self.c,
            target: &self.target,
            adapter,
            limits: &self.limits,
            before: &self.before,
            recs: &self.recs,
            poll,
            margin,
        };
        match ctx.finish(self.base_rec.clone()) {
            Ok(r) => {
                append(&self.plan, &r)?;
                clear_inflight(&self.plan)?;
                Ok(r)
            }
            Err(reason) => record_lost(&self.plan, &self.base_rec, reason),
        }
    }
}

struct After<'a> {
    plan: &'a Plan,
    c: &'a Call,
    target: &'a RoleTarget,
    adapter: &'a dyn Adapter,
    limits: &'a Limits,
    before: &'a Value,
    recs: &'a [Value],
    poll: Duration,
    margin: Duration,
}

impl After<'_> {
    /// Everything after an accepted start, up to the finished record (not yet appended). An
    /// error here is recorded by the caller as an unmeasured run that asks for a person.
    fn finish(&self, base_rec: Value) -> Result<Value, String> {
        let (plan, c, target, adapter) = (self.plan, self.c, self.target, self.adapter);
        // wait for the end: the window plus a margin, then a stop, then the margin again
        let deadline =
            std::time::Instant::now() + self.limits.timeout + self.limits.grace + self.margin;
        let mut stop_sent = false;
        let last = loop {
            match adapter.send(target, "GET", "/v0/role/status", None) {
                Ok((200, s))
                    if s["running"].is_null()
                        && s["last"]["attempt_id"] == c.attempt_id.as_str() =>
                {
                    break s["last"].clone();
                }
                _ => {}
            }
            let now = std::time::Instant::now();
            if now > deadline && !stop_sent {
                let _ = adapter.send(target, "POST", "/v0/role/stop", None);
                stop_sent = true;
            }
            if stop_sent && now > deadline + self.margin {
                return Err("the run did not end after a stop; its usage is unknown".into());
            }
            std::thread::sleep(self.poll);
        };
        if let Some(e) = last["error"].as_str() {
            return Err(format!("run error: {e}"));
        }
        let (st, out) = adapter
            .send(
                target,
                "GET",
                &format!("/v0/role/runs/{}/stdout", c.attempt_id),
                None,
            )
            .map_err(|e| format!("stdout fetch: {e}"))?;
        if st != 200 {
            return Err(format!("stdout not available ({st})"));
        }
        // the service reads the file lossily as UTF-8: the saved text and its digest are of
        // that text (review c4620d2 L8)
        let stream = out["content"].as_str().unwrap_or_default().to_string();
        let stream_path = plan
            .ledger_dir
            .join("streams")
            .join(format!("{}.jsonl", c.attempt_id));
        write_private(&stream_path, stream.as_bytes()).map_err(|e| format!("stream save: {e}"))?;
        if last["runtime"] == "codex-trial" {
            let mut record = base_rec;
            let delivery = record["delivery_id"]
                .as_str()
                .ok_or("missing admitted delivery ID")?;
            if stop_sent
                || last["stop"] != "Exited"
                || last["exit_code"] != 0
                || last["cleanup_failed"] != false
                || last["mode"] != "new"
            {
                return Err(
                    "the Codex trial did not exit cleanly; receipt requires reconciliation".into(),
                );
            }
            let (receipt, tokens) = crate::codex_trial::receipt(&stream, delivery, &c.attempt_id)?;
            let measured = match tokens {
                Some(tokens) => Measure::Measured {
                    tokens,
                    source: "codex-trial-receipt".into(),
                },
                None => Measure::unmeasured("no authoritative final Codex usage"),
            };
            record["ended_at"] = json!(ieo_evidence::time::now());
            record["ended_unix"] = json!(unix_now());
            record["duration_ms"] =
                json!(last["elapsed_ms"].as_u64().ok_or("missing elapsed time")?);
            record["session"] = json!({"mode":"new", "session_id":last["session_id"]});
            record["outcome"] =
                json!({"stop":"Exited", "runtime":"codex-trial", "result":receipt["result"]});
            record["codex"] = json!({"thread_id":receipt["thread_id"], "turn_id":receipt["turn_id"],
                "provider_usage":receipt["usage"], "observed_usage":receipt["observed_usage"]});
            record["usage"] = json!({"run":measured, "session":measured, "cost_usd_reported":null});
            record["stream"] = json!({"path":stream_path,"bytes":stream.len(),
                "sha256":hex::encode(Sha256::digest(stream.as_bytes()))});
            record["stop_required"] = json!(true);
            record["stop_reason"] = json!(
                "isolated Codex trial ended; reconcile usage, repository effects and billing"
            );
            return Ok(record);
        }
        // the children's transcripts (a child's total comes only from its transcript)
        let mut transcripts = std::collections::BTreeMap::new();
        let mut children = Vec::new();
        for a in stream_agents(stream.as_bytes()) {
            // the id names a file and a URL path: anything else stays unmeasured (review
            // 71e408a P1)
            if !safe_agent_id(&a) {
                children.push(json!({"agent_id": a, "path": null, "refused": "unsafe agent id"}));
                continue;
            }
            let got = adapter
                .send(
                    target,
                    "GET",
                    &format!("/v0/role/runs/{}/children/{a}", c.attempt_id),
                    None,
                )
                .ok()
                .filter(|(st, _)| *st == 200)
                .and_then(|(_, v)| v["content"].as_str().map(str::to_owned));
            match got {
                Some(t) => {
                    let p = plan
                        .ledger_dir
                        .join("children")
                        .join(&c.attempt_id)
                        .join(format!("agent-{a}.jsonl"));
                    write_private(&p, t.as_bytes())
                        .map_err(|e| format!("child transcript save: {e}"))?;
                    children.push(json!({"agent_id": a, "path": p, "bytes": t.len(),
                        "sha256": hex::encode(Sha256::digest(t.as_bytes()))}));
                    transcripts.insert(a, t.into_bytes());
                }
                None => children.push(json!({"agent_id": a, "path": null})),
            }
        }
        let mode = if last["mode"] == "resume" {
            SessionMode::Resume
        } else {
            SessionMode::New
        };
        let session_id = last["session_id"].as_str().unwrap_or_default().to_string();
        let before = self.before;
        // the base: the caller's own newest record of this session, and only when the service
        // resumed from exactly that run (§11 item 7: the run actually resumed)
        let base = (mode == SessionMode::Resume)
            .then(|| {
                let newest = self
                    .recs
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
        let parsed =
            ieo_evidence::stream::parse_with_children(stream.as_bytes(), mode, base, &transcripts);
        let collected_run = serde_json::to_value(&parsed.usage.run).map_err(|e| e.to_string())?;
        let o = &last["observed"];
        // a missing guard observation is never 0: it makes any collector figure "below" it
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
            reasons.push(other_stop_reason("process-group cleanup failed"));
        }
        // the role environment as the run reported it (fixed by env::build's allow-list)
        let env: Vec<&str> = last["env_names"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        if !env.contains(&"CLAUDE_CODE_OAUTH_TOKEN")
            || env.iter().any(|n| n.starts_with("ANTHROPIC_"))
        {
            reasons.push(other_stop_reason(format!(
                "environment not subscription-only: {env:?}"
            )));
        }
        // the run's result: an authentication or other failure must not read as success
        let result = stream
            .lines()
            .rev()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .find(|v| v["type"] == "result");
        match &result {
            Some(r) if r["subtype"] == "success" && r["is_error"] == false => {}
            // the run's own result: the only reasons a spare releases (no "; " inside)
            Some(r) => reasons.push(
                format!(
                    "{RESULT_STOP_REASON}{} (is_error {})",
                    r["subtype"], r["is_error"]
                )
                .replace("; ", ", "),
            ),
            None => reasons.push(NO_RESULT_STOP_REASON.into()),
        }
        // an unmeasured run stops the budget anyway; the record says so at once (review f90f5b5)
        if usage["run"]["state"] != "measured" {
            reasons.push(other_stop_reason(format!(
                "usage unmeasured: {}",
                usage["run"]["reason"]
            )));
        }
        let mut notes = Vec::new();
        let mut warnings: Vec<String> = Vec::new();
        if stop_sent {
            notes.push("the caller sent a stop at its deadline".to_string());
        }
        match budget::rate_limit_events(&stream) {
            Ok(evs) if evs.is_empty() => notes.push(
                "no rate_limit_event in this run: the next call is held until one is observed"
                    .to_string(),
            ),
            Ok(evs) => {
                // every event is judged: a warning never clears a stop (user decision
                // 2026-11-17, warning)
                for e in &evs {
                    match budget::rate_limit_verdict(e) {
                        budget::RateLimitVerdict::Stop(why) => {
                            reasons.push(other_stop_reason(format!("this run's signal: {why}")))
                        }
                        budget::RateLimitVerdict::Warning(w) => warnings.push(w),
                        budget::RateLimitVerdict::Allowed => {}
                    }
                }
            }
            Err(e) => reasons.push(other_stop_reason(format!("this run's signal: {e}"))),
        }
        let mut r = base_rec;
        r["ended_at"] = json!(ieo_evidence::time::now());
        r["ended_unix"] = json!(unix_now());
        if let Some(ms) = last["elapsed_ms"].as_u64() {
            r["duration_ms"] = json!(ms);
        }
        r["session"] = json!({"mode": last["mode"], "session_id": session_id,
                              "resume_base_attempt": base_attempt});
        r["outcome"] = json!({"stop": stop, "stopped_by_guard": last["stopped_by_guard"],
            "exit_code": last["exit_code"], "cleanup_failed": last["cleanup_failed"],
            "observed": last["observed"], "tamper": last["tamper"],
            "config_cleanup": last["config_cleanup"], "env_names": last["env_names"],
            "result": result.map(|r| json!({"subtype": r["subtype"], "is_error": r["is_error"]}))});
        r["result_event"] = serde_json::to_value(&parsed.result).unwrap_or(Value::Null);
        r["usage"] = usage;
        r["children"] = json!(children);
        r["stream"] = json!({"path": stream_path, "bytes": stream.len(),
                             "sha256": hex::encode(Sha256::digest(stream.as_bytes()))});
        r["stop_required"] = json!(!reasons.is_empty());
        if !reasons.is_empty() {
            r["stop_reason"] = json!(reasons.join("; "));
        }
        if !notes.is_empty() {
            r["notes"] = json!(notes);
        }
        // allowed_warning events that passed: recorded and shown, never a stop by themselves
        if !warnings.is_empty() {
            r["rate_limit_warnings"] = json!(warnings);
        }
        Ok(r)
    }
}

/// Measure a recorded run again from its kept stream and the given child transcripts (no model
/// call). Only for a run stopped for unmeasured usage alone; the stream must be the recorded
/// one, a transcript the run recorded must have the recorded digest, the base must be the
/// recorded base of the same session, and the result must be measured and consistent. Returns
/// the usage (as a record carries it) and the transcripts used (path, size, digest).
fn remeasure(
    r: &Value,
    earlier: &[Value],
    transcripts: &std::collections::BTreeMap<String, PathBuf>,
) -> Result<(Value, Vec<Value>), String> {
    let attempt = r["attempt_id"].as_str().unwrap_or("?");
    // only a run stopped for its usage alone (nothing else to look at)
    let reason = r["stop_reason"].as_str().unwrap_or_default();
    if r["format"] != RECORD_FORMAT
        || r["usage"]["run"]["state"] != "unmeasured"
        || !reason.starts_with("usage unmeasured")
        || reason.contains("; ")
    {
        return Err(format!(
            "{attempt} is not a run stopped only for unmeasured usage"
        ));
    }
    let path = r["stream"]["path"].as_str().ok_or("no stream path")?;
    let stream =
        std::fs::read(path).map_err(|e| format!("stream {path}: {e}{}", moved_hint(&e)))?;
    if hex::encode(Sha256::digest(&stream)) != r["stream"]["sha256"].as_str().unwrap_or_default() {
        return Err("the kept stream differs from the recorded digest".into());
    }
    // a transcript only for a child of this run, under a plain id (review 71e408a P1)
    let agents = stream_agents(&stream);
    if let Some(a) = transcripts
        .keys()
        .find(|a| !safe_agent_id(a) || !agents.contains(*a))
    {
        return Err(format!(
            "{a:?} is not an agent id of {attempt}'s stream ({agents:?})"
        ));
    }
    let mut bytes = std::collections::BTreeMap::new();
    let mut kept = Vec::new();
    for (a, p) in transcripts {
        let b = std::fs::read(p)
            .map_err(|e| format!("transcript {}: {e}{}", p.display(), moved_hint(&e)))?;
        let sha = hex::encode(Sha256::digest(&b));
        // a transcript the run itself kept must be that one (review 608b367 F4)
        if let Some(rec) = r["children"]
            .as_array()
            .and_then(|cs| cs.iter().find(|x| x["agent_id"] == a.as_str()))
            .filter(|x| x["sha256"].is_string())
            && rec["sha256"] != sha.as_str()
        {
            return Err(format!(
                "transcript of {a} differs from the one the run kept"
            ));
        }
        kept.push(json!({"agent_id": a, "path": p, "bytes": b.len(), "sha256": sha}));
        bytes.insert(a.clone(), b);
    }
    let mode = if r["session"]["mode"] == "resume" {
        SessionMode::Resume
    } else {
        SessionMode::New
    };
    let base = match r["session"]["resume_base_attempt"].as_str() {
        None => None,
        Some(b) => {
            let br = earlier
                .iter()
                .find(|x| x["attempt_id"] == b)
                .ok_or(format!("base {b} not recorded"))?;
            // the base's own session must be the run's (review 608b367 F3)
            if br["session"]["session_id"] != r["session"]["session_id"] {
                return Err(format!("base {b} is another session"));
            }
            Some(ResumeBase {
                task_id: br["task_id"].as_str().unwrap_or_default().into(),
                attempt_id: b.into(),
                command_id: br["command_id"].as_str().unwrap_or_default().into(),
                session_id: br["session"]["session_id"].as_str().map(str::to_owned),
                session: serde_json::from_value(br["usage"]["session"].clone())
                    .map_err(|e| format!("base session: {e}"))?,
                cost_usd_reported: br["usage"]["cost_usd_reported"].as_f64(),
            })
        }
    };
    let parsed = ieo_evidence::stream::parse_with_children(&stream, mode, base, &bytes);
    if parsed.usage.attribution_check != "consistent" || parsed.usage.run.tokens().is_none() {
        return Err(format!(
            "{attempt} is still not measured: {} {:?}",
            parsed.usage.attribution_check, parsed.usage.attribution_detail
        ));
    }
    let o = &r["outcome"]["observed"];
    let observed = Observed {
        billable: o["billable_lower_bound"].as_u64().unwrap_or(u64::MAX),
        cache_read: o["cache_read_lower_bound"].as_u64().unwrap_or(u64::MAX),
        messages: o["messages"].as_u64().unwrap_or_default() as usize,
        incomplete_lines: o["incomplete_lines"].as_u64().unwrap_or_default() as usize,
    };
    let stop = r["outcome"]["stop"].as_str().unwrap_or("unknown");
    let collected = serde_json::to_value(&parsed.usage.run).map_err(|e| e.to_string())?;
    let charged = run::charge_reported(stop == "Exited", stop, &observed, &collected);
    if charged["state"] != "measured" {
        return Err(format!("{attempt} charge is not measured: {charged}"));
    }
    let mut usage = serde_json::to_value(&parsed.usage).map_err(|e| e.to_string())?;
    usage["collector_run"] = usage["run"].take();
    usage["run"] = charged;
    Ok((usage, kept))
}

/// Re-evaluate a recorded run from its kept stream and child transcripts (no model call), for a
/// user decision (02-04 D3). Appends a correction only when [`remeasure`] holds; the run is not
/// counted again and the original line stays. Returns the correction.
pub fn reevaluate(
    plan: &Plan,
    attempt: &str,
    transcripts: &std::collections::BTreeMap<String, PathBuf>,
    decision: &str,
) -> Result<Value, String> {
    if decision.trim().is_empty() {
        return Err("a correction needs the decision it follows".into());
    }
    let raw = raw_records(plan)?;
    if raw
        .iter()
        .any(|r| r["format"] == CORRECTION_FORMAT && r["corrects"] == attempt)
    {
        return Err(format!("{attempt} is already corrected"));
    }
    let pos = raw
        .iter()
        .position(|r| r["attempt_id"] == attempt && r["format"] == RECORD_FORMAT)
        .ok_or(format!("no record of {attempt}"))?;
    let earlier = effective(raw[..pos].to_vec(), &plan.ledger_dir)?;
    let r = &raw[pos];
    // measured first from the given files: nothing is copied or written for a refusal
    remeasure(r, &earlier, transcripts)?;
    // the transcripts go with the ledger (children/<attempt>/), by absolute path: a correction is
    // measured again on every read (review 12c9a21 N1, N2)
    let dir = std::path::absolute(plan.ledger_dir.join("children").join(attempt))
        .map_err(|e| format!("ledger path: {e}"))?;
    // what this call creates is removed again when it ends in a refusal (review 71e408a P1)
    let dir_existed = dir.exists();
    let mut created: Vec<PathBuf> = Vec::new();
    let copied = (|| -> Result<(Value, Vec<Value>), String> {
        let mut in_ledger = std::collections::BTreeMap::new();
        for (a, p) in transcripts {
            let b = std::fs::read(p).map_err(|e| format!("transcript {}: {e}", p.display()))?;
            let to = dir.join(format!("agent-{a}.jsonl"));
            match std::fs::read(&to) {
                Ok(existing) if existing != b => {
                    return Err(format!(
                        "{} already holds a different transcript of {a}",
                        to.display()
                    ));
                }
                Ok(_) => {}
                Err(_) => {
                    created.push(to.clone());
                    write_private(&to, &b).map_err(|e| format!("transcript copy: {e}"))?;
                }
            }
            in_ledger.insert(a.clone(), to);
        }
        remeasure(r, &earlier, &in_ledger)
    })();
    let (usage, kept) = match copied {
        Ok(x) => x,
        Err(e) => {
            for f in &created {
                let _ = std::fs::remove_file(f);
            }
            if !dir_existed {
                let _ = std::fs::remove_dir(&dir);
            }
            return Err(e);
        }
    };
    let c = json!({"format": CORRECTION_FORMAT, "corrects": attempt, "model_request": false,
        "original_stream_sha256": r["stream"]["sha256"], "child_transcripts": kept,
        "decision": decision, "corrected_at": ieo_evidence::time::now(),
        "before": {"usage_run": r["usage"]["run"], "stop_required": r["stop_required"],
                   "stop_reason": r["stop_reason"]},
        "after": {"usage": usage, "stop_required": false}});
    append(plan, &c)?;
    // the ledger must read back with the correction applied
    records(plan)?;
    Ok(c)
}

/// Settle a guard-stopped run from its session transcript (user decision 2026-11-17
/// r1-settlement (가); [`crate::settlement`]). Under the ledger lock, with no call in flight:
/// the run must be eligible, the transcript (`transcript`, the verified evidence copy) is copied
/// into `children/<attempt>/settlement-transcript.jsonl` (0600; an existing copy must be the
/// same), its SHA-256 must be `original_sha256`, and the settlement line is appended only when
/// the ledger then reads back with it applied. With `dry_run` nothing is written: the line is
/// returned as it would be appended (its copy path is where it would be).
#[allow(clippy::too_many_arguments)]
pub fn settle(
    plan: &Plan,
    attempt: &str,
    transcript: &Path,
    original_path: &str,
    original_sha256: &str,
    decision: &str,
    independent_check: &Value,
    dry_run: bool,
) -> Result<Value, String> {
    if decision.trim().is_empty() {
        return Err("a settlement needs the decision it follows".into());
    }
    crate::settlement::independent_check_ok(independent_check)?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(plan.ledger_dir.join("caller.lock"))
        .map_err(|e| format!("caller lock: {e}"))?;
    match lock.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => {
            return Err("another caller holds the ledger".into());
        }
        Err(std::fs::TryLockError::Error(e)) => return Err(format!("caller lock: {e}")),
    }
    if inflight_path(plan).exists() {
        return Err("a call is in flight without a record: a person reconciles it first".into());
    }
    let raw = raw_records(plan)?;
    if raw
        .iter()
        .any(|r| r["format"] == crate::settlement::SETTLEMENT_FORMAT && r["settles"] == attempt)
    {
        return Err(format!("{attempt} is already settled"));
    }
    let found: Vec<&Value> = raw
        .iter()
        .filter(|r| r["format"] == RECORD_FORMAT && r["attempt_id"] == attempt)
        .collect();
    let [r] = found.as_slice() else {
        return Err(format!("{attempt}: {} records (one expected)", found.len()));
    };
    if raw
        .iter()
        .any(|c| c["format"] == CORRECTION_FORMAT && c["corrects"] == attempt)
    {
        return Err(format!("{attempt} is corrected already"));
    }
    // the ledger reads as it is (fail closed) before anything is added
    records(plan)?;
    let sp = r["stream"]["path"].as_str().unwrap_or_default();
    let stream =
        std::fs::read_to_string(sp).map_err(|e| format!("stream {sp}: {e}{}", moved_hint(&e)))?;
    crate::settlement::eligible(r, &stream)?;
    let b = std::fs::read(transcript).map_err(|e| format!("{}: {e}", transcript.display()))?;
    if hex::encode(Sha256::digest(&b)) != original_sha256 {
        return Err(format!(
            "{}: SHA-256 differs from the original's {original_sha256}",
            transcript.display()
        ));
    }
    // the independent check's report, read once here; the ledger keeps a copy (review 7b904de N-1)
    let rp = independent_check["evidence"].as_str().unwrap_or_default();
    let report = std::fs::read(rp).map_err(|e| format!("independent check report {rp}: {e}"))?;
    if dry_run {
        // measured from the given files' bytes; nothing is written anywhere (review d971f59 L-3)
        return crate::settlement::build(
            r,
            &stream,
            &b,
            &report,
            &plan.ledger_dir,
            false,
            original_path,
            original_sha256,
            decision,
            independent_check,
        );
    }
    // the copies: created new in the ledger, never through a symlink; an existing regular file
    // must be the same bytes (reviews d971f59 L-1, 7b904de N-1)
    // the directories first, each checked before anything is created (review 758ac03 O-1)
    let made_dirs = crate::settlement::prepare_dirs(&plan.ledger_dir, attempt)?;
    let mut created: Vec<PathBuf> = Vec::new();
    let mut keep = |name: &str, bytes: &[u8]| -> Result<(), String> {
        let (p, new) = crate::settlement::write_in_ledger(&plan.ledger_dir, attempt, name, bytes)?;
        if new {
            created.push(p);
        }
        Ok(())
    };
    let kept = keep(crate::settlement::COPY_NAME, &b)
        .and_then(|()| keep(crate::settlement::REPORT_NAME, &report));
    let s = kept.and_then(|()| {
        crate::settlement::build(
            r,
            &stream,
            &b,
            &report,
            &plan.ledger_dir,
            true,
            original_path,
            original_sha256,
            decision,
            independent_check,
        )
    });
    let s = match s {
        Ok(s) => s,
        Err(e) => {
            for p in &created {
                let _ = std::fs::remove_file(p);
            }
            crate::settlement::remove_made(&made_dirs);
            return Err(e);
        }
    };
    // An append error can occur after bytes reach the ledger. Keep the evidence copies
    // and tell the operator to inspect the ledger before doing anything further.
    append(plan, &s).map_err(|e| format!("settlement append uncertain: {e}"))?;
    // the ledger must read back with the settlement applied; a failure here is after the append
    // (PR #91 review 4b427c5 F-6): the line stays and the ledger refuses until a person looks
    let after_append = |why: String| {
        format!(
            "the settlement line was appended, but the ledger does not read back with it: {why}; \
             the ledger now refuses every call until a person inspects records.jsonl"
        )
    };
    let recs = records(plan).map_err(after_append)?;
    if recs
        .iter()
        .find(|x| x["attempt_id"] == attempt)
        .is_none_or(|x| x["settlement"]["policy"] != crate::settlement::POLICY)
    {
        return Err(after_append(format!(
            "{attempt}: the settlement did not apply"
        )));
    }
    drop(lock);
    Ok(s)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn admission(timeout: u64, billable: u64) -> Admission {
        Admission {
            lock: std::fs::File::open("/dev/null").expect("file"),
            recs: vec![],
            limits: Limits {
                timeout: Duration::from_secs(timeout),
                timeout_is_total: false,
                billable_room: billable,
                cache_read_room: 500,
                grace: Duration::from_secs(6),
            },
            before: Value::Null,
            spare: None,
        }
    }

    #[test]
    fn codex_trial_completed_result_and_unmeasured_usage_share_admitted_id() {
        struct MockRole;
        impl Adapter for MockRole {
            fn send(
                &self,
                _: &RoleTarget,
                method: &str,
                path: &str,
                _: Option<&Value>,
            ) -> Result<(u16, Value), String> {
                assert_eq!(method, "GET");
                match path {
                    "/v0/role/status" => Ok((
                        200,
                        json!({"running":null, "last":{
                            "attempt_id":"a", "session_id":"s", "runtime":"codex-trial",
                            "stop":"Exited", "exit_code":0, "cleanup_failed":false, "mode":"new", "elapsed_ms":7
                        }}),
                    )),
                    "/v0/role/runs/a/stdout" => Ok((
                        200,
                        json!({"content": json!({
                        "type":"ieo_codex_trial_receipt", "auth":"chatgpt", "delivery_id":"D-1",
                        "attempt_id":"a", "thread_id":"th", "turn_id":"tu",
                        "result":{"id":"tu","status":"completed"}, "observed_usage":{"totalTokens":100}
                    }).to_string()}),
                    )),
                    other => panic!("unexpected {other}"),
                }
            }
        }
        let dir = std::env::temp_dir().join(format!("ieo-codex-receipt-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let role = RoleTarget {
            url: String::new(),
            adapter_token_file: PathBuf::new(),
            max_window_seconds: Some(120),
            share: Share {
                max_runs: 1,
                total_seconds: 120,
                billable_tokens: 500,
                cache_read_tokens: 500,
            },
        };
        let running = Running {
            _lock: std::fs::File::open("/dev/null").unwrap(),
            plan: Plan {
                budget: budget::named("02-05").unwrap(),
                ledger_dir: dir.clone(),
                roles: BTreeMap::new(),
                first_observation: None,
            },
            c: Call {
                task_id: "trial".into(),
                role: "development".into(),
                op: "start".into(),
                attempt_id: "a".into(),
                session_id: "s".into(),
                prompt: String::new(),
                coordinator: Value::Null,
            },
            target: role,
            limits: admission(114, 500).limits(),
            before: Value::Null,
            recs: Vec::new(),
            base_rec: json!({"delivery_id":"D-1", "attempt_id":"a"}),
        };
        let path = running.plan.records_path();
        mark_inflight(&running.plan, &running.c, Some("D-1")).unwrap();
        let row = running
            .finish(&MockRole, Duration::ZERO, Duration::ZERO)
            .unwrap();
        let persisted: Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        std::fs::remove_dir_all(dir).unwrap();
        assert_eq!(persisted, row);
        assert_eq!(row["delivery_id"], "D-1");
        assert_eq!(row["codex"]["thread_id"], "th");
        assert_eq!(row["codex"]["turn_id"], "tu");
        assert_eq!(row["outcome"]["result"]["status"], "completed");
        assert_eq!(row["codex"]["observed_usage"]["totalTokens"], 100);
        assert_eq!(row["usage"]["run"]["state"], "unmeasured");
        assert_eq!(row["stop_required"], true);
    }

    /// `lower` only lowers (review c11e734 L-1): a bound above the room or window changes
    /// nothing, one below it lowers, a window not above the grace and a zero room are refused.
    #[test]
    fn lower_never_raises_a_limit() {
        let mut a = admission(114, 1_000);
        a.lower(Some(3_600), Some(100_000)).expect("lower");
        assert_eq!(a.limits(), admission(114, 1_000).limits(), "nothing raised");
        a.lower(Some(60), Some(400)).expect("lower");
        let l = a.limits();
        assert_eq!(
            (
                l.timeout,
                l.timeout_is_total,
                l.billable_room,
                l.cache_read_room,
                l.grace
            ),
            (
                Duration::from_secs(54),
                true,
                400,
                500,
                Duration::from_secs(6)
            )
        );
        a.lower(None, None).expect("no bound");
        assert_eq!(a.limits(), l);
        assert!(
            a.lower(Some(6), None).is_err(),
            "window not above the grace"
        );
        assert!(a.lower(None, Some(0)).is_err(), "no billable room");
        assert!(a.lower(Some(30), Some(0)).is_err(), "refused as a whole");
        assert_eq!(a.limits(), l, "a refusal changes nothing");
    }
}
