//! Model-call budget guard (user decision 2026-11-13 ①).
//!
//! Counted from `ieo-evidence` records (02-01): every model run of the budget — parent and
//! children, development and review, failed, incomplete and correction runs alike — with its
//! `usage.run` share (resumed sessions: cumulative minus the resumed base, rev 7 §11 item 7).
//! Billable tokens = input + output + cache_creation; cache_read has its own cap.
//! An unmeasured run is never zero: it stops further calls. No automatic retry.
//!
//! Extra billing: only subscription usage is allowed (additional charge USD 0). The CLI's
//! `total_cost_usd` is a conversion estimate, not a bill, and is never capped at 0. A run may
//! start only when the latest observed `rate_limit_event` shows that extra usage is not in use
//! and cannot start (`isUsingOverage:false`, `overageStatus` not `allowed`); otherwise that call
//! is held with the reason.

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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Spent {
    pub runs: u64,
    pub seconds: u64,
    pub billable_tokens: u64,
    pub cache_read_tokens: u64,
    /// Largest billable share of a single run so far (used to refuse a run that would cross the cap).
    pub largest_run_billable: u64,
    pub unmeasured_runs: Vec<String>,
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

/// Sum the model runs of `records` (ieo-evidence records as JSON).
pub fn spent(records: &[Value]) -> Spent {
    let mut s = Spent::default();
    for r in records.iter().filter(|r| r["model_request"] == true) {
        s.runs += 1;
        s.seconds += r["duration_ms"].as_u64().unwrap_or(0).div_ceil(1000);
        let run = &r["usage"]["run"];
        if run["state"] == "measured" {
            let t = &run["tokens"];
            let n = |k: &str| t[k].as_u64().unwrap_or(0);
            let billable = n("input") + n("output") + n("cache_creation");
            s.billable_tokens += billable;
            s.cache_read_tokens += n("cache_read");
            s.largest_run_billable = s.largest_run_billable.max(billable);
        } else {
            s.unmeasured_runs.push(format!(
                "{}/{}",
                r["attempt_id"].as_str().unwrap_or("?"),
                r["command_id"].as_str().unwrap_or("?")
            ));
        }
    }
    s
}

/// The latest `rate_limit_event` in a stream-json text, if any.
pub fn last_rate_limit(stream: &str) -> Option<Value> {
    stream
        .lines()
        .rev()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find(|v| v["type"] == "rate_limit_event")
}

/// No-extra-billing condition from an observed `rate_limit_info`.
pub fn overage_check(event: Option<&Value>) -> Result<(), String> {
    let Some(e) = event else {
        return Err(
            "no rate_limit_event observed: the no-extra-billing condition is unconfirmed".into(),
        );
    };
    let info = &e["rate_limit_info"];
    if info["isUsingOverage"] != false {
        return Err(format!(
            "isUsingOverage is {} (extra usage in use or unknown)",
            info["isUsingOverage"]
        ));
    }
    match info["overageStatus"].as_str() {
        Some("allowed") | None => Err(format!(
            "overageStatus {} — extra usage could be billed",
            info["overageStatus"]
        )),
        Some(_) => {
            if info["status"] == "allowed" {
                Ok(())
            } else {
                Err(format!(
                    "rate limit status {} (not allowed)",
                    info["status"]
                ))
            }
        }
    }
}

/// May the next run of `budget` start? `latest_rate_limit`: the newest observed event (the last
/// run's stream, or for the first run the newest stored observation of the same account).
pub fn next(budget: &Budget, records: &[Value], latest_rate_limit: Option<&Value>) -> Decision {
    let s = spent(records);
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
    if s.billable_tokens + s.largest_run_billable >= budget.billable_tokens {
        return Decision::Stop {
            reason: format!(
                "billable tokens {} (+ largest run {}) reach the cap {}",
                s.billable_tokens, s.largest_run_billable, budget.billable_tokens
            ),
        };
    }
    if s.cache_read_tokens >= budget.cache_read_tokens {
        return Decision::Stop {
            reason: format!(
                "cache_read {} reaches the cap {}",
                s.cache_read_tokens, budget.cache_read_tokens
            ),
        };
    }
    if let Err(why) = overage_check(latest_rate_limit) {
        return Decision::Hold { reason: why };
    }
    Decision::Start {
        timeout_seconds: budget.per_run_seconds.min(budget.total_seconds - s.seconds),
    }
}
