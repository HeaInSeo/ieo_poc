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
//! The in-run stop on observed usage is [`crate::run`]; it observes a lower bound and stops
//! after the request that crossed the room (see its limits).
//!
//! **Extra billing.** Only subscription usage is allowed (additional charge USD 0). The CLI's
//! `total_cost_usd` is a conversion estimate, not a bill, and is never capped at 0. One rule
//! ([`rate_limit_verdict`], user decision 2026-11-17 on `allowed_warning`,
//! `docs/poc/User_Decisions_2026-11-17_warning.md`) judges every `rate_limit_event` on every
//! path: before a call ([`observation_check`], every event of the latest observed stream, not
//! older than [`OBSERVATION_MAX_AGE_SECONDS`]), during a run ([`stream_stop`]: the adapter stops
//! the process on the first event that is a stop) and after it (the caller's record). An event
//! passes only when the subscription status is `allowed` or `allowed_warning` **and** paid extra
//! usage is shown off (`overageStatus: "rejected"` and `isUsingOverage: false`, both present);
//! `allowed_warning` passes as a recorded warning ([`rate_limit_warning`]). Anything else is a
//! stop, and a later event never clears an earlier stop. The CLI (2.1.322) types `status`,
//! `overageStatus` and each window's status as `allowed | allowed_warning | rejected`; for
//! `overageStatus` the first two mean paid extra usage covers the overflow.

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    pub name: &'static str,
    pub max_runs: u64,
    pub per_run_seconds: u64,
    pub total_seconds: u64,
    pub billable_tokens: u64,
    pub cache_read_tokens: u64,
    /// The role shares the approval itself fixes (02-05). Empty: the shares are configured per
    /// deployment within the budget (02-04, X-02).
    pub role_shares: &'static [RoleShare],
    /// The approval's spare run, if any (02-05: one, development only).
    pub spare: Option<SpareRule>,
}

/// A spare run (user decision 2026-11-17 (나), docs/poc/User_Decisions_2026-11-17_spare.md):
/// the role runs at most `planned_runs` runs without a marker; its 1 spare carries an explicit
/// marker (a person's reason, the earlier failed run of this role and budget it replaces, the
/// cause report or evidence reference, who approved it and when) and may follow either planned
/// run; no other role has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpareRule {
    pub role: &'static str,
    pub planned_runs: u64,
}

/// One role's share of a budget as the user approved it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoleShare {
    pub role: &'static str,
    pub max_runs: u64,
    pub total_seconds: u64,
    pub billable_tokens: u64,
    pub cache_read_tokens: u64,
}

impl Budget {
    /// The approved share of `role`, when the approval fixes the shares.
    pub fn approved_share(&self, role: &str) -> Option<RoleShare> {
        self.role_shares.iter().find(|s| s.role == role).copied()
    }
}

/// The approved budgets by name. Each is its own: no budget's usage counts for another, and
/// none can be used under another's name.
pub fn named(name: &str) -> Option<Budget> {
    [BUDGET_0204, BUDGET_X02, BUDGET_0205]
        .into_iter()
        .find(|b| b.name == name)
}

/// 02-04 real-session acceptance tests.
pub const BUDGET_0204: Budget = Budget {
    name: "02-04",
    max_runs: 12,
    per_run_seconds: 1_800,
    total_seconds: 14_400,
    billable_tokens: 1_500_000,
    cache_read_tokens: 15_000_000,
    role_shares: &[],
    spare: None,
};

/// X-02 PoC development and review on the Notion test task.
pub const BUDGET_X02: Budget = Budget {
    name: "X-02",
    max_runs: 6,
    per_run_seconds: 3_600,
    total_seconds: 14_400,
    billable_tokens: 3_000_000,
    cache_read_tokens: 30_000_000,
    role_shares: &[],
    spare: None,
};

/// 02-05 real sessions (step 8 only; user decision 2026-11-15 (나),
/// docs/poc/User_Decisions_2026-11-15_0205-x08.md §2.1): 3 planned runs (development 2,
/// Guardrail 1) and 1 spare for development only; 600 s per run (the window: timeout + grace);
/// 2,400 s in all; billable tokens (input + output + cache_creation) 200,000; cache_read
/// 2,000,000; additional charge USD 0. The role shares are part of the approval: development 3
/// runs (its 2 planned and the spare), 1,800 s, 150,000, 1,500,000; Guardrail 1 run (never
/// retried), 600 s, 50,000, 500,000.
pub const BUDGET_0205: Budget = Budget {
    name: "02-05",
    max_runs: 4,
    per_run_seconds: 600,
    total_seconds: 2_400,
    billable_tokens: 200_000,
    cache_read_tokens: 2_000_000,
    role_shares: &[
        RoleShare {
            role: "development",
            max_runs: 3,
            total_seconds: 1_800,
            billable_tokens: 150_000,
            cache_read_tokens: 1_500_000,
        },
        RoleShare {
            role: "guardrail",
            max_runs: 1,
            total_seconds: 600,
            billable_tokens: 50_000,
            cache_read_tokens: 500_000,
        },
    ],
    // at most 2 development runs without a marker; the 1 spare carries the marker and replaces
    // a failed development run (decision 2026-11-17 (나)); Guardrail has none
    spare: Some(SpareRule {
        role: "development",
        planned_runs: 2,
    }),
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

/// How one `rate_limit_event` is judged (the same on every path).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateLimitVerdict {
    /// Subscription usage allowed, paid extra usage off.
    Allowed,
    /// Allowed, paid extra usage off, but a warning threshold is passed: go on, record it.
    Warning(String),
    /// Stop (before a call: hold it; during a run: stop the process; after: stop the budget).
    Stop(String),
}

const LIMIT_STATUS: [&str; 3] = ["allowed", "allowed_warning", "rejected"];
const RATE_LIMIT_TYPES: [&str; 6] = [
    "five_hour",
    "seven_day",
    "seven_day_opus",
    "seven_day_sonnet",
    "seven_day_overage_included",
    "overage",
];

/// The no-extra-billing rule for one `rate_limit_event` (user decision 2026-11-17, warning).
///
/// Stop when any of these holds, checked in this order (the first one is the reason):
/// * the event has no `rate_limit_info` object, or `status` is missing or not one of
///   `allowed | allowed_warning | rejected` (unknown);
/// * `status` is `rejected` (the subscription refuses requests);
/// * `overageStatus` is not the string `rejected` (missing: unconfirmed; `allowed` or
///   `allowed_warning`: paid extra usage would cover the overflow);
/// * `isUsingOverage` is not the boolean `false` (missing, not a boolean, or extra usage in use);
/// * `overageInUse` is present and not `false`, or `overageEnabled` is present and not `false`
///   (usage credits turned on: paid extra usage is possible), or `errorCode` is present;
/// * `rateLimitType` is present and unknown, or is `overage` / `seven_day_overage_included`
///   (a warning about the paid bucket contradicts `overageStatus: rejected`);
/// * `unifiedWindows` is present and not an object, or a window in it is not an object, has no
///   readable `status` (the CLI leaves it out when it cannot read it) or has `rejected` (an
///   exhausted window is never overridden by the top-level status; conservative, the CLI says
///   the top-level status decides whether requests are refused).
///
/// Otherwise: `allowed_warning` at the top level or in a window is a [`RateLimitVerdict::Warning`]
/// (recorded and shown, the run goes on); else [`RateLimitVerdict::Allowed`].
pub fn rate_limit_verdict(event: &Value) -> RateLimitVerdict {
    use RateLimitVerdict::{Allowed, Stop, Warning};
    let info = &event["rate_limit_info"];
    if !info.is_object() {
        return Stop("rate_limit_info missing".into());
    }
    let status = match info["status"].as_str() {
        Some(s) if LIMIT_STATUS.contains(&s) => s,
        _ => {
            return Stop(format!(
                "rate limit status {} (missing or unknown)",
                info["status"]
            ));
        }
    };
    if status == "rejected" {
        return Stop("rate limit status \"rejected\" (subscription usage refused)".into());
    }
    if info["overageStatus"] != "rejected" {
        return Stop(format!(
            "overageStatus {} (only \"rejected\" excludes paid extra usage)",
            info["overageStatus"]
        ));
    }
    if info["isUsingOverage"] != false {
        return Stop(format!(
            "isUsingOverage {} (extra usage in use or unknown)",
            info["isUsingOverage"]
        ));
    }
    for k in ["overageInUse", "overageEnabled"] {
        if !info[k].is_null() && info[k] != false {
            return Stop(format!(
                "{k} {} (paid extra usage possible or unknown)",
                info[k]
            ));
        }
    }
    if !info["errorCode"].is_null() {
        return Stop(format!("errorCode {}", info["errorCode"]));
    }
    let kind = &info["rateLimitType"];
    if !kind.is_null() {
        match kind.as_str() {
            Some("overage" | "seven_day_overage_included") => {
                return Stop(format!(
                    "rateLimitType {kind} with overageStatus \"rejected\" (contradictory)"
                ));
            }
            Some(k) if RATE_LIMIT_TYPES.contains(&k) => {}
            _ => return Stop(format!("rateLimitType {kind} (unknown)")),
        }
    }
    let mut warned: Vec<String> = Vec::new();
    if status == "allowed_warning" {
        warned.push(format!(
            "{} {} of the limit (threshold {}, resets {})",
            kind.as_str().unwrap_or("limit"),
            info["utilization"],
            info["surpassedThreshold"],
            info["resetsAt"]
        ));
    }
    let windows = &info["unifiedWindows"];
    if !windows.is_null() {
        let Some(ws) = windows.as_object() else {
            return Stop("unifiedWindows is not an object".into());
        };
        for (name, w) in ws {
            match w["status"].as_str() {
                _ if !w.is_object() => {
                    return Stop(format!("unifiedWindows.{name} is not an object"));
                }
                Some("allowed") => {}
                Some("allowed_warning") => {
                    let line = format!(
                        "window {name} {} (threshold {}, resets {})",
                        w["utilization"], w["surpassedThreshold"], w["resetsAt"]
                    );
                    warned.push(line);
                }
                Some("rejected") => {
                    return Stop(format!("unifiedWindows.{name} status \"rejected\""));
                }
                _ => {
                    return Stop(format!(
                        "unifiedWindows.{name} status {} (missing or unknown)",
                        w["status"]
                    ));
                }
            }
        }
    }
    if warned.is_empty() {
        Allowed
    } else {
        Warning(format!(
            "rate limit warning (allowed_warning; paid extra usage off): {}",
            warned.join("; ")
        ))
    }
}

/// No-extra-billing condition for one event: `Err` exactly when [`rate_limit_verdict`] is a
/// stop (a warning passes).
pub fn overage_check(event: &Value) -> Result<(), String> {
    match rate_limit_verdict(event) {
        RateLimitVerdict::Stop(why) => Err(why),
        _ => Ok(()),
    }
}

/// The warning of one event, when it passes with one.
pub fn rate_limit_warning(event: &Value) -> Option<String> {
    match rate_limit_verdict(event) {
        RateLimitVerdict::Warning(w) => Some(w),
        _ => None,
    }
}

/// The warnings of a stream's events that pass (in order); stops are not listed here.
pub fn rate_limit_warnings(stream: &str) -> Vec<String> {
    rate_limit_events(stream)
        .unwrap_or_default()
        .iter()
        .filter_map(rate_limit_warning)
        .collect()
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
