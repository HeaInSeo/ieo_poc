//! In-run budget enforcement around one role process (user decision 2026-11-14 §3).
//!
//! [`crate::budget::next`] decides *before* a run whether it may start. This module enforces the
//! limits *during* the run and cleans up afterwards:
//!
//! * **time**: the run gets `min(per-run limit, total time left)`; at the deadline the whole
//!   process group gets SIGTERM, then SIGKILL after a grace period.
//! * **observed tokens**: every stream-json `assistant` message carries a `usage` object. The guard
//!   sums it (per message id, the largest value seen, parent and child messages alike) and stops
//!   the group once the observed billable tokens (input + output + cache_creation) or cache_read
//!   reach the room left in the budget.
//! * **extra usage / limits**: any `rate_limit_event` that breaks the no-extra-billing condition
//!   ([`crate::budget::stream_stop`]) stops the group at once.
//! * **cleanup**: after the leader exits (for any reason) the group gets SIGKILL, so no child
//!   process outlives the run.
//!
//! **Limits of this guard** (it is not a provider-side limit):
//! * Usage is known only when a message's `usage` appears in the stream, i.e. after the model
//!   request that produced it has completed. A stop therefore happens *after* the request that
//!   crossed the room; that request's tokens are spent and are reported as an overrun.
//! * The first `rate_limit_event` of a session appears only after the first request; the
//!   pre-run check ([`crate::budget::observation_check`]) is what holds a call before it starts.
//! * The authoritative per-run share is computed afterwards from the run's `result` event by the
//!   02-01 collector (`ieo-evidence`); a run without it is unmeasured and stops the next call
//!   ([`crate::budget::next`]). The observed sum here is only the in-run stop signal.

use serde_json::Value;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// What the guard allows for one run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub timeout: Duration,
    /// The timeout is the total time left (smaller than the per-run limit).
    pub timeout_is_total: bool,
    pub billable_room: u64,
    pub cache_read_room: u64,
    /// SIGTERM → SIGKILL grace.
    pub grace: Duration,
}

impl Limits {
    /// Limits for the next run of `budget`, or `None` when the budget allows no run.
    pub fn for_next(
        budget: &crate::budget::Budget,
        spent: &crate::budget::Spent,
    ) -> Option<Limits> {
        if spent.runs >= budget.max_runs || spent.seconds >= budget.total_seconds {
            return None;
        }
        let left = budget.total_seconds - spent.seconds;
        Some(Limits {
            timeout: Duration::from_secs(budget.per_run_seconds.min(left)),
            timeout_is_total: left < budget.per_run_seconds,
            billable_room: budget.billable_tokens.checked_sub(spent.billable_tokens)?,
            cache_read_room: budget
                .cache_read_tokens
                .checked_sub(spent.cache_read_tokens)?,
            grace: Duration::from_secs(10),
        })
    }
}

/// Why the run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stop {
    /// The process exited on its own.
    Exited,
    RunTimeout,
    TotalTimeout,
    /// Observed billable tokens reached the room left.
    TokenCap,
    /// Observed cache_read reached the room left.
    CacheReadCap,
    /// A rate-limit event broke the no-extra-billing condition.
    Overage(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Observed {
    pub billable: u64,
    pub cache_read: u64,
    pub messages: usize,
}

#[derive(Debug)]
pub struct Outcome {
    pub stop: Stop,
    /// Exit code of the leader, `None` when it ended by a signal.
    pub exit_code: Option<i32>,
    pub observed: Observed,
    pub elapsed: Duration,
}

fn usage_of(v: &Value) -> Option<(String, [u64; 4])> {
    if v["type"] != "assistant" {
        return None;
    }
    let m = &v["message"];
    let u = &m["usage"];
    let n = |k: &str| u[k].as_u64().unwrap_or(0);
    Some((
        m["id"].as_str().unwrap_or_default().to_string(),
        [
            n("input_tokens"),
            n("output_tokens"),
            n("cache_creation_input_tokens"),
            n("cache_read_input_tokens"),
        ],
    ))
}

/// Signal the whole process group (`kill -s SIG -- -PGID`; the workspace forbids `unsafe`, so
/// no direct kill(2)). The group exists until its last member exits.
fn signal_group(pgid: u32, sig: &str) {
    let _ = Command::new("kill")
        .args(["-s", sig, "--", &format!("-{pgid}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Run `cmd` (its own process group) under `limits`, copying stdout to `stdout_path` and
/// stderr to `stderr_path`.
pub fn run(
    mut cmd: Command,
    limits: Limits,
    stdout_path: &Path,
    stderr_path: &Path,
) -> std::io::Result<Outcome> {
    let started = Instant::now();
    cmd.process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(std::fs::File::create(stderr_path)?);
    let mut child = cmd.spawn()?;
    let pgid = child.id();
    let stdout = child.stdout.take().expect("piped stdout");
    let (tx, rx) = mpsc::channel::<String>();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let mut out = std::fs::File::create(stdout_path)?;
    let mut per_message: BTreeMap<String, [u64; 4]> = BTreeMap::new();
    let mut observed = Observed::default();
    let deadline = started + limits.timeout;
    let mut stop: Option<Stop> = None;
    loop {
        let now = Instant::now();
        if now >= deadline {
            stop = Some(if limits.timeout_is_total {
                Stop::TotalTimeout
            } else {
                Stop::RunTimeout
            });
            break;
        }
        match rx.recv_timeout((deadline - now).min(Duration::from_millis(200))) {
            Ok(line) => {
                writeln!(out, "{line}")?;
                if let Some(why) = crate::budget::stream_stop(&line) {
                    stop = Some(Stop::Overage(why));
                    break;
                }
                if let Ok(v) = serde_json::from_str::<Value>(&line)
                    && let Some((id, u)) = usage_of(&v)
                {
                    let e = per_message.entry(id).or_insert([0; 4]);
                    for i in 0..4 {
                        e[i] = e[i].max(u[i]);
                    }
                    observed.messages = per_message.len();
                    observed.billable = per_message
                        .values()
                        .map(|u| u[0].saturating_add(u[1]).saturating_add(u[2]))
                        .fold(0u64, u64::saturating_add);
                    observed.cache_read = per_message
                        .values()
                        .map(|u| u[3])
                        .fold(0u64, u64::saturating_add);
                    if observed.billable >= limits.billable_room {
                        stop = Some(Stop::TokenCap);
                        break;
                    }
                    if observed.cache_read >= limits.cache_read_room {
                        stop = Some(Stop::CacheReadCap);
                        break;
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if child.try_wait()?.is_some() {
                    // drain what the reader still holds, then finish
                    while let Ok(line) = rx.recv_timeout(Duration::from_millis(200)) {
                        writeln!(out, "{line}")?;
                    }
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // stdout closed: wait for the leader within the deadline
                let left = deadline.saturating_duration_since(Instant::now());
                let until = Instant::now() + left;
                while Instant::now() < until {
                    if child.try_wait()?.is_some() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                if child.try_wait()?.is_none() {
                    stop = Some(if limits.timeout_is_total {
                        Stop::TotalTimeout
                    } else {
                        Stop::RunTimeout
                    });
                }
                break;
            }
        }
    }
    let status = if stop.is_some() {
        signal_group(pgid, "TERM");
        let until = Instant::now() + limits.grace;
        loop {
            if let Some(s) = child.try_wait()? {
                break s;
            }
            if Instant::now() >= until {
                signal_group(pgid, "KILL");
                break child.wait()?;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    } else {
        child.wait()?
    };
    // no process of the run outlives it
    signal_group(pgid, "KILL");
    // the reader ends when the pipe closes; a descendant that left the group could keep it open,
    // so the reader is detached rather than joined
    drop(rx);
    drop(reader);
    out.flush()?;
    Ok(Outcome {
        stop: stop.unwrap_or(Stop::Exited),
        exit_code: status.code(),
        observed,
        elapsed: started.elapsed(),
    })
}
