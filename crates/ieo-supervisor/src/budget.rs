//! Model-call budget guard (user decision 2026-11-13 ①).
//!
//! Counted from `ieo-evidence` records (02-01): every model run of the budget — parent and
//! children, development and review, failed, incomplete and correction runs alike — with its
//! `usage.run` share (resumed sessions: cumulative minus the resumed base, rev 7 §11 item 7).
//! Billable tokens = input + output + cache_creation; cache_read has its own cap.
//! An unmeasured run is never zero: it stops further calls. A record whose duration, run
//! state or token fields are missing or not numbers counts as unmeasured, and a record that
//! does not say whether it was a model run stops the budget too. No automatic retry.
//!
//! **When caps are checked.** Run count and seconds are hard: a run starts only while one is
//! left, and its timeout is the per-run limit capped by the seconds left. Tokens cannot be
//! stopped mid-run by this guard, so a run starts only when the spent amount plus a *reserve*
//! (the largest run so far) does not exceed the cap. This is a projection, not a guarantee:
//! the first run has no history and is bounded only by its timeout, and any run can exceed
//! the reserve; [`Spent`] then shows the overrun, which is reported, and the next call stops.
//! The adapter's in-stream stop on a cap crossing is a later 02-04 step.
//!
//! **Extra billing.** Only subscription usage is allowed (additional charge USD 0). The CLI's
//! `total_cost_usd` is a conversion estimate, not a bill, and is never capped at 0. The CLI
//! (2.1.322) types `overageStatus` as `allowed | allowed_warning | rejected`, where the first
//! two mean paid extra usage covers the overflow. A run may start only when **every**
//! `rate_limit_event` of the latest observed stream shows `overageStatus: "rejected"`,
//! `isUsingOverage: false` and `status: "allowed"`, and that observation is not older than
//! [`OBSERVATION_MAX_AGE_SECONDS`]. Otherwise that call is held with the reason. During a
//! run, [`stream_stop`] tells the adapter to stop the process on the first event that breaks
//! the condition.

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    pub name: &'static str,
    pub max_runs: u64,
    pub per_run_seconds: u64,
    pub total_seconds: u64,
    pub billable_tokens: u64,
    pub cache_read_tokens: u64,
}

/// 02-04 real-session acceptance tests.
pub const BUDGET_0204: Budget = Budget {
    name: "02-04",
    max_runs: 12,
    per_run_seconds: 1_800,
    total_seconds: 14_400,
    billable_tokens: 1_500_000,
    cache_read_tokens: 15_000_000,
};

/// X-02 PoC development and review on the Notion test task.
pub const BUDGET_X02: Budget = Budget {
    name: "X-02",
    max_runs: 6,
    per_run_seconds: 3_600,
    total_seconds: 14_400,
    billable_tokens: 3_000_000,
    cache_read_tokens: 30_000_000,
};

/// Oldest no-overage observation a run may start on (7 days). The account setting it reflects
/// (extra usage off) changes only by a user action; the in-stream stop covers a change
/// during the run.
pub const OBSERVATION_MAX_AGE_SECONDS: u64 = 7 * 24 * 3600;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Spent {
    pub runs: u64,
    pub seconds: u64,
    pub billable_tokens: u64,
    pub cache_read_tokens: u64,
    pub largest_run_billable: u64,
    pub largest_run_cache_read: u64,
    /// Runs whose usage or duration is not measured (never counted as zero).
    pub unmeasured_runs: Vec<String>,
    /// Records that do not say whether they were model runs.
    pub unclassified_records: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// The next run may start with this timeout (seconds).
    Start { timeout_seconds: u64 },
    /// Stop and report; no further call of this budget.
    Stop { reason: String },
    /// Hold this call only (the no-extra-billing condition is not confirmed).
    Hold { reason: String },
}

/// A no-overage observation: the stream text of the newest run (or, for the first run of a
/// budget, the newest stored stream of the same account) and its age.
#[derive(Debug, Clone, Copy)]
pub struct Observation<'a> {
    pub stream: &'a str,
    pub age_seconds: u64,
}

fn id(r: &Value) -> String {
    format!(
        "{}/{}",
        r["attempt_id"].as_str().unwrap_or("?"),
        r["command_id"].as_str().unwrap_or("?")
    )
}

/// Sum the model runs of `records` (ieo-evidence records as JSON).
pub fn spent(records: &[Value]) -> Spent {
    let mut s = Spent::default();
    for r in records {
        match r["model_request"].as_bool() {
            Some(false) => continue,
            None => {
                s.unclassified_records.push(id(r));
                continue;
            }
            Some(true) => {}
        }
        s.runs += 1;
        let duration = r["duration_ms"].as_u64();
        let run = &r["usage"]["run"];
        let t = &run["tokens"];
        let n = |k: &str| t[k].as_u64();
        let tokens = (run["state"] == "measured")
            .then(|| {
                Some((
                    n("input")?,
                    n("output")?,
                    n("cache_creation")?,
                    n("cache_read")?,
                ))
            })
            .flatten();
        if let Some(ms) = duration {
            s.seconds = s.seconds.saturating_add(ms.div_ceil(1000));
        }
        // an overflowing value is not a measurement
        let sums = tokens.and_then(|(i, o, cc, cr)| {
            let billable = i.checked_add(o)?.checked_add(cc)?;
            Some((
                billable,
                cr,
                s.billable_tokens.checked_add(billable)?,
                s.cache_read_tokens.checked_add(cr)?,
            ))
        });
        match (duration, sums) {
            (Some(_), Some((billable, cr, total_b, total_cr))) => {
                s.billable_tokens = total_b;
                s.cache_read_tokens = total_cr;
                s.largest_run_billable = s.largest_run_billable.max(billable);
                s.largest_run_cache_read = s.largest_run_cache_read.max(cr);
            }
            _ => s.unmeasured_runs.push(id(r)),
        }
    }
    s
}

/// All `rate_limit_event`s of a stream-json text, in order. A line that mentions
/// `rate_limit_event` but is not a complete JSON object (e.g. truncated) is an error: it may
/// have carried a breaking value.
pub fn rate_limit_events(stream: &str) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    for (n, l) in stream.lines().enumerate() {
        match serde_json::from_str::<Value>(l) {
            Ok(v) if v["type"] == "rate_limit_event" => out.push(v),
            Ok(_) => {}
            Err(_) if l.contains("rate_limit_event") => {
                return Err(format!("line {}: unreadable rate_limit_event", n + 1));
            }
            Err(_) => {}
        }
    }
    Ok(out)
}

/// No-extra-billing condition for one event.
pub fn overage_check(event: &Value) -> Result<(), String> {
    let info = &event["rate_limit_info"];
    if info["overageStatus"] != "rejected" {
        return Err(format!(
            "overageStatus {} (only \"rejected\" excludes paid extra usage)",
            info["overageStatus"]
        ));
    }
    if info["isUsingOverage"] != false {
        return Err(format!(
            "isUsingOverage {} (extra usage in use or unknown)",
            info["isUsingOverage"]
        ));
    }
    if info["status"] != "allowed" {
        return Err(format!(
            "rate limit status {} (not allowed)",
            info["status"]
        ));
    }
    Ok(())
}

/// The no-extra-billing condition from an observation: at least one event, every event passes,
/// and the observation is fresh enough.
pub fn observation_check(obs: Option<Observation<'_>>) -> Result<(), String> {
    let Some(obs) = obs else {
        return Err("no observation: the no-extra-billing condition is unconfirmed".into());
    };
    if obs.age_seconds > OBSERVATION_MAX_AGE_SECONDS {
        return Err(format!(
            "observation is {} s old (max {OBSERVATION_MAX_AGE_SECONDS})",
            obs.age_seconds
        ));
    }
    let events = rate_limit_events(obs.stream)?;
    if events.is_empty() {
        return Err(
            "no rate_limit_event in the observation: the no-extra-billing condition is unconfirmed"
                .into(),
        );
    }
    for (i, e) in events.iter().enumerate() {
        overage_check(e).map_err(|why| format!("event {}: {why}", i + 1))?;
    }
    Ok(())
}

/// During a run: `Some(reason)` when this stream line is a `rate_limit_event` that breaks the
/// no-extra-billing condition; the adapter then stops the process and reports.
pub fn stream_stop(line: &str) -> Option<String> {
    match serde_json::from_str::<Value>(line) {
        Ok(v) => (v["type"] == "rate_limit_event")
            .then(|| overage_check(&v).err())
            .flatten(),
        Err(_) if line.contains("rate_limit_event") => {
            Some("unreadable rate_limit_event line".into())
        }
        Err(_) => None,
    }
}

/// May the next run of `budget` start?
pub fn next(budget: &Budget, records: &[Value], observation: Option<Observation<'_>>) -> Decision {
    let s = spent(records);
    if !s.unclassified_records.is_empty() {
        return Decision::Stop {
            reason: format!(
                "record(s) {:?} do not say whether they were model runs",
                s.unclassified_records
            ),
        };
    }
    if !s.unmeasured_runs.is_empty() {
        return Decision::Stop {
            reason: format!(
                "unmeasured run(s) {:?}: usage is not zero, no further call",
                s.unmeasured_runs
            ),
        };
    }
    if s.runs >= budget.max_runs {
        return Decision::Stop {
            reason: format!("{} runs used of {}", s.runs, budget.max_runs),
        };
    }
    if s.seconds >= budget.total_seconds {
        return Decision::Stop {
            reason: format!("{} s used of {}", s.seconds, budget.total_seconds),
        };
    }
    let reserve = s.largest_run_billable;
    // an overflowing projection is over the cap (review bc55edf R1)
    if s.billable_tokens
        .checked_add(reserve)
        .is_none_or(|t| t > budget.billable_tokens)
    {
        return Decision::Stop {
            reason: format!(
                "billable tokens {} + reserve {} exceed the cap {}",
                s.billable_tokens, reserve, budget.billable_tokens
            ),
        };
    }
    let reserve_cr = s.largest_run_cache_read;
    if s.cache_read_tokens
        .checked_add(reserve_cr)
        .is_none_or(|t| t > budget.cache_read_tokens)
    {
        return Decision::Stop {
            reason: format!(
                "cache_read {} + reserve {} exceed the cap {}",
                s.cache_read_tokens, reserve_cr, budget.cache_read_tokens
            ),
        };
    }
    if let Err(why) = observation_check(observation) {
        return Decision::Hold { reason: why };
    }
    Decision::Start {
        timeout_seconds: budget.per_run_seconds.min(budget.total_seconds - s.seconds),
    }
}
