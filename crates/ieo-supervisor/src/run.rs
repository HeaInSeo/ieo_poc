//! In-run budget enforcement around one role process (user decision 2026-11-14 §3).
//!
//! [`crate::budget::next`] decides *before* a run whether it may start ([`Limits::for_next`]
//! wraps it). This module enforces limits *during* the run and cleans up afterwards:
//!
//! * **time**: the run gets `min(per-run limit, total time left) − grace`, so that the stop
//!   (SIGTERM to the whole process group, SIGKILL after the grace) completes within the limit, up
//!   to tens of milliseconds of polling and `kill` calls (recorded durations round up to seconds).
//! * **copy after a stop**: lines that arrive after a stop decision — during the grace and for a
//!   bounded 2 s afterwards — are still copied (per line: bytes decoded lossily, a trailing `\r`
//!   removed), so a `result` written on the way out is kept for the report; it is not trusted for
//!   charging ([`charge`]).
//! * **observed tokens**: stream-json `assistant` lines carry a `usage` object; the guard sums it
//!   (per message id, the largest value seen; parent and child messages alike) and stops the group
//!   once observed billable tokens (input + output + cache_creation) or cache_read reach the room
//!   left in the budget.
//! * **extra usage / limits**: any `rate_limit_event` that breaks the no-extra-billing condition
//!   ([`crate::budget::stream_stop`]) stops the group at once.
//! * **blindness**: when the stream ends or cannot be read while the process is still running, the
//!   guard can no longer observe usage or limit signals, so it stops the group
//!   ([`Stop::StreamLost`]). Bytes that are not UTF-8 are read lossily, never dropped.
//! * **cleanup on every path**: after the leader exits, after a stop, and on any internal error,
//!   the group gets SIGKILL; if signalling the group fails, the leader is killed directly and the
//!   failure is reported ([`Outcome::cleanup_failed`]).
//!
//! **Limits of this guard — it observes and then stops; it is not a provider-side limit:**
//! * The `usage` on an `assistant` line is **not final**. The CLI writes the usage known when the
//!   message starts: input and cache figures are complete, but `output_tokens` is only the start
//!   value (02-01 fixture r1-new: 5 in the stream, 612 in `result`). Model calls the CLI makes on
//!   its own — compaction, side calls — appear in no `assistant` line at all (l3-compact: 0 in the
//!   stream, 8,035 billable and 26,400 cache_read in `modelUsage`). The observed sum is therefore a **lower bound**; the stop
//!   comes only after the request that crossed the room, whose tokens are already spent.
//! * Final usage is known only at the end, from the `result` event; the 02-01 collector
//!   (`ieo-evidence`) computes the run share from it. A run the guard **stopped** is charged as
//!   unmeasured even when it wrote a `result` on the way out ([`charge`]): an interrupted process
//!   may report less than it used. The next call is then blocked ([`crate::budget::next`]).
//! * The first `rate_limit_event` of a session appears only after the first request (it may come
//!   before any `assistant` line, l3-compact); holding a
//!   call before that request is the pre-run observation's job (with the subscription and
//!   environment check), not this guard's.
//! * A descendant that leaves the process group (`setsid`) is not reached by group signals; inside
//!   a role container, stopping the container is the last cleanup (adapter step).

use serde_json::Value;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How long the guard keeps reading after the leader exited (bounded; descendants are killed
/// afterwards).
const DRAIN: Duration = Duration::from_secs(2);

/// SIGTERM → SIGKILL grace used by [`Limits::for_next`].
pub const GRACE: Duration = Duration::from_secs(10);

/// What the guard allows for one run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Time until the stop starts; the stop itself takes at most `grace` more.
    pub timeout: Duration,
    /// The timeout comes from the total time left (smaller than the per-run limit).
    pub timeout_is_total: bool,
    pub billable_room: u64,
    pub cache_read_room: u64,
    /// SIGTERM → SIGKILL grace.
    pub grace: Duration,
}

impl Limits {
    /// Limits for the next run: `budget::next` must say `Start`, and every room must be positive.
    /// Otherwise the decision that refuses the run is returned. The room is the cap minus what is
    /// spent; the reserve `budget::next` uses (the largest run so far) only decides whether to start.
    pub fn for_next(
        budget: &crate::budget::Budget,
        records: &[Value],
        observation: Option<crate::budget::Observation<'_>>,
    ) -> Result<Limits, crate::budget::Decision> {
        use crate::budget::Decision;
        let decision = crate::budget::next(budget, records, observation);
        let Decision::Start { timeout_seconds } = decision else {
            return Err(decision);
        };
        let s = crate::budget::spent(records);
        let stop = |reason: String| Err(Decision::Stop { reason });
        let billable_room = budget.billable_tokens.saturating_sub(s.billable_tokens);
        let cache_read_room = budget.cache_read_tokens.saturating_sub(s.cache_read_tokens);
        if billable_room == 0 || cache_read_room == 0 {
            return stop("no token room left".into());
        }
        let window = Duration::from_secs(timeout_seconds);
        if window <= GRACE {
            return stop(format!(
                "{timeout_seconds} s left is not more than the {} s stop grace",
                GRACE.as_secs()
            ));
        }
        Ok(Limits {
            timeout: window - GRACE,
            timeout_is_total: timeout_seconds < budget.per_run_seconds,
            billable_room,
            cache_read_room,
            grace: GRACE,
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
    /// The stream ended or could not be read while the process kept running.
    StreamLost(String),
    /// Stopped on request (adapter `stop`).
    Requested,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Observed {
    /// Lower bound (see the module limits).
    pub billable: u64,
    pub cache_read: u64,
    pub messages: usize,
    /// `assistant` lines whose usage was missing or not a number in some field.
    pub incomplete_lines: usize,
}

#[derive(Debug)]
pub struct Outcome {
    pub stop: Stop,
    /// Exit code of the leader, `None` when it ended by a signal.
    pub exit_code: Option<i32>,
    pub observed: Observed,
    pub elapsed: Duration,
    /// Signalling the process group failed at least once (the leader was killed directly).
    pub cleanup_failed: bool,
}

enum Event {
    Line(String),
    /// The pipe closed.
    End,
    /// Reading failed.
    Broken,
}

#[derive(Default)]
struct Usage {
    per_message: BTreeMap<String, [u64; 4]>,
    unnamed: usize,
    observed: Observed,
}

impl Usage {
    fn add(&mut self, v: &Value) {
        if v["type"] != "assistant" {
            return;
        }
        let m = &v["message"];
        let u = &m["usage"];
        let fields = [
            "input_tokens",
            "output_tokens",
            "cache_creation_input_tokens",
            "cache_read_input_tokens",
        ]
        .map(|k| u[k].as_u64());
        if fields.iter().any(Option::is_none) {
            self.observed.incomplete_lines += 1;
        }
        let vals = fields.map(|x| x.unwrap_or(0));
        let id = match m["id"].as_str() {
            Some(id) if !id.is_empty() => id.to_string(),
            _ => {
                // no id: never merged with another line
                self.unnamed += 1;
                format!("\u{0}unnamed-{}", self.unnamed)
            }
        };
        let e = self.per_message.entry(id).or_insert([0; 4]);
        for (slot, v) in e.iter_mut().zip(vals) {
            *slot = (*slot).max(v);
        }
        self.observed.messages = self.per_message.len();
        self.observed.billable = self
            .per_message
            .values()
            .map(|u| u[0].saturating_add(u[1]).saturating_add(u[2]))
            .fold(0, u64::saturating_add);
        self.observed.cache_read = self
            .per_message
            .values()
            .map(|u| u[3])
            .fold(0, u64::saturating_add);
    }
}

/// Whether any live (non-zombie) process belongs to process group `pgid` (`/proc/*/stat`).
fn group_alive(pgid: u32) -> bool {
    let Ok(rd) = std::fs::read_dir("/proc") else {
        return true; // cannot tell: assume alive (fail closed)
    };
    for e in rd.flatten() {
        if !e
            .file_name()
            .to_string_lossy()
            .bytes()
            .all(|b| b.is_ascii_digit())
        {
            continue;
        }
        let Ok(stat) = std::fs::read_to_string(e.path().join("stat")) else {
            continue;
        };
        // "pid (comm) state ppid pgrp …"; comm may contain spaces or parentheses
        let Some(rest) = stat.rfind(')').map(|i| &stat[i + 1..]) else {
            continue;
        };
        let f: Vec<&str> = rest.split_whitespace().collect();
        if f.len() > 2 && f[0] != "Z" && f[2] == pgid.to_string() {
            return true;
        }
    }
    false
}

/// Signal the whole process group (`<kill> -SIG -- -PGID`: the form both util-linux and procps-ng 4
/// accept — procps-ng 4.0.2 rejects `-s SIG -- -PGID`; the workspace forbids `unsafe`, so
/// no direct kill(2)). Returns whether the group is reached: the signal was delivered, or no live
/// process of the group is left (nothing to signal). A `kill` that fails while members remain is
/// a failure.
fn signal_group(kill: &str, pgid: u32, sig: &str) -> bool {
    match Command::new(kill)
        .args([format!("-{sig}"), "--".to_string(), format!("-{pgid}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(s) if s.success() => true,
        _ => !group_alive(pgid),
    }
}

/// Copies stdout lines that arrived after a stop decision (no more checks): everything the reader
/// queued during the stop grace, plus a bounded time afterwards, so a `result` written on the way
/// out is kept.
struct Copier<'a> {
    rx: &'a mpsc::Receiver<Event>,
    out: &'a mut std::fs::File,
    ended: bool,
    error: Option<std::io::Error>,
}

impl Copier<'_> {
    /// Copy what arrives within `for_`.
    fn pump(&mut self, for_: Duration) {
        let until = Instant::now() + for_;
        while !self.ended {
            let Some(left) = until.checked_duration_since(Instant::now()) else {
                break;
            };
            match self.rx.recv_timeout(left) {
                Ok(Event::Line(line)) => {
                    if let Err(e) = writeln!(self.out, "{line}")
                        && self.error.is_none()
                    {
                        self.error = Some(e);
                    }
                }
                Ok(Event::End) | Ok(Event::Broken) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                    self.ended = true
                }
                Err(mpsc::RecvTimeoutError::Timeout) => break,
            }
        }
    }
}

/// Stop the group: SIGTERM, wait up to `grace`, then SIGKILL. Falls back to
/// killing the leader directly when group signalling fails. Returns the leader's status when it
/// could be waited for.
fn stop_group(
    kill: &str,
    child: &mut Child,
    pgid: u32,
    grace: Duration,
    failed: &mut bool,
) -> Option<ExitStatus> {
    if !signal_group(kill, pgid, "TERM") {
        *failed = true;
    }
    let until = Instant::now() + grace;
    loop {
        if let Ok(Some(s)) = child.try_wait() {
            return Some(s);
        }
        if Instant::now() >= until {
            break;
        }
        // output keeps arriving in the channel; it is copied after the stop
        std::thread::sleep(Duration::from_millis(50));
    }
    if !signal_group(kill, pgid, "KILL") {
        *failed = true;
    }
    let _ = child.kill();
    match child.wait() {
        Ok(s) => Some(s),
        Err(_) => {
            *failed = true;
            None
        }
    }
}

/// Run `cmd` (its own process group) under `limits`, copying stdout to `stdout_path` and stderr
/// to `stderr_path`. On every return path — including errors — no process of the group is left
/// running (best effort for descendants that left the group, see the module limits); when that
/// cannot be confirmed, [`Outcome::cleanup_failed`] is set.
pub fn run(
    cmd: Command,
    limits: Limits,
    stdout_path: &Path,
    stderr_path: &Path,
) -> std::io::Result<Outcome> {
    run_with_kill(cmd, limits, stdout_path, stderr_path, "kill")
}

/// [`run`] with the program used to signal the group (tests inject a failing one).
pub fn run_with_kill(
    cmd: Command,
    limits: Limits,
    stdout_path: &Path,
    stderr_path: &Path,
    kill: &str,
) -> std::io::Result<Outcome> {
    run_with(
        cmd,
        limits,
        stdout_path,
        stderr_path,
        &Options {
            kill: kill.to_string(),
            ..Options::default()
        },
    )
}

/// Extra inputs of a run.
#[derive(Debug, Clone)]
pub struct Options {
    /// Program used to signal the group.
    pub kill: String,
    /// Bytes written to the process's stdin, which is then closed (the prompt for
    /// `--input-format stream-json`); `None` = no stdin.
    pub stdin: Option<Vec<u8>>,
    /// Set from outside to stop the run (adapter `stop`); checked on every loop iteration.
    pub stop: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            kill: "kill".into(),
            stdin: None,
            stop: None,
        }
    }
}

/// [`run`] with [`Options`].
pub fn run_with(
    mut cmd: Command,
    limits: Limits,
    stdout_path: &Path,
    stderr_path: &Path,
    opts: &Options,
) -> std::io::Result<Outcome> {
    let kill = opts.kill.as_str();
    // everything that can fail before the process exists happens before spawn
    let mut out = std::fs::File::create(stdout_path)?;
    let err = std::fs::File::create(stderr_path)?;
    let started = Instant::now();
    cmd.process_group(0)
        .stdin(if opts.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(err);
    let mut child = cmd.spawn()?;
    let pgid = child.id();
    if let (Some(bytes), Some(mut stdin)) = (opts.stdin.clone(), child.stdin.take()) {
        // written from a thread so a process that does not read cannot block the guard
        std::thread::spawn(move || {
            let _ = stdin.write_all(&bytes);
        });
    }
    let (tx, rx) = mpsc::channel::<Event>();
    let Some(stdout) = child.stdout.take() else {
        let mut failed = false;
        stop_group(kill, &mut child, pgid, limits.grace, &mut failed);
        return Err(std::io::Error::other("no stdout pipe"));
    };
    let reader = std::thread::spawn(move || {
        let mut r = BufReader::new(stdout);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match r.read_until(b'\n', &mut buf) {
                Ok(0) => {
                    let _ = tx.send(Event::End);
                    break;
                }
                Ok(_) => {
                    let line = String::from_utf8_lossy(&buf)
                        .trim_end_matches(['\n', '\r'])
                        .to_string();
                    if tx.send(Event::Line(line)).is_err() {
                        break;
                    }
                }
                Err(_) => {
                    let _ = tx.send(Event::Broken);
                    break;
                }
            }
        }
    });
    let mut cleanup_failed = false;
    let mut usage = Usage::default();
    let watched = watch(
        &mut child,
        &rx,
        &mut out,
        &mut usage,
        &limits,
        started,
        opts.stop.as_deref(),
    );
    let mut copier = Copier {
        rx: &rx,
        out: &mut out,
        ended: false,
        error: None,
    };
    // cleanup on every path, including an error inside `watch`
    let status = match &watched {
        Ok(Watch::Exited(s)) => Some(*s),
        Ok(Watch::Stopped(_)) | Err(_) => {
            stop_group(kill, &mut child, pgid, limits.grace, &mut cleanup_failed)
        }
    };
    if !signal_group(kill, pgid, "KILL") {
        cleanup_failed = true;
    }
    if matches!(watched, Ok(Watch::Stopped(_))) {
        // lines written on the way out (e.g. `result` from a TERM handler) are kept
        copier.pump(DRAIN);
    }
    let copy_error = copier.error.take();
    // the reader ends when the pipe closes; a descendant that left the group could keep it open,
    // so it is detached rather than joined
    drop(rx);
    drop(reader);
    let with_cleanup = |e: std::io::Error| {
        if cleanup_failed {
            std::io::Error::new(
                e.kind(),
                format!("{e}; cleanup failed: processes of the run may remain"),
            )
        } else {
            e
        }
    };
    let watched = watched.map_err(with_cleanup)?;
    if let Some(e) = copy_error {
        return Err(with_cleanup(e));
    }
    out.flush().map_err(with_cleanup)?;
    Ok(Outcome {
        stop: match watched {
            Watch::Stopped(s) => s,
            Watch::Exited(_) => Stop::Exited,
        },
        exit_code: status.and_then(|s| s.code()),
        observed: usage.observed,
        elapsed: started.elapsed(),
        cleanup_failed,
    })
}

enum Watch {
    Exited(ExitStatus),
    Stopped(Stop),
}

/// Handle one line; `Some(stop)` when it requires stopping.
fn on_line(
    line: &str,
    out: &mut std::fs::File,
    usage: &mut Usage,
    limits: &Limits,
) -> std::io::Result<Option<Stop>> {
    writeln!(out, "{line}")?;
    if let Some(why) = crate::budget::stream_stop(line) {
        return Ok(Some(Stop::Overage(why)));
    }
    if let Ok(v) = serde_json::from_str::<Value>(line) {
        usage.add(&v);
        if usage.observed.billable >= limits.billable_room {
            return Ok(Some(Stop::TokenCap));
        }
        if usage.observed.cache_read >= limits.cache_read_room {
            return Ok(Some(Stop::CacheReadCap));
        }
    }
    Ok(None)
}

fn watch(
    child: &mut Child,
    rx: &mpsc::Receiver<Event>,
    out: &mut std::fs::File,
    usage: &mut Usage,
    limits: &Limits,
    started: Instant,
    stop: Option<&std::sync::atomic::AtomicBool>,
) -> std::io::Result<Watch> {
    let deadline = started + limits.timeout;
    let timed_out = || {
        if limits.timeout_is_total {
            Stop::TotalTimeout
        } else {
            Stop::RunTimeout
        }
    };
    loop {
        let now = Instant::now();
        if now >= deadline {
            return Ok(Watch::Stopped(timed_out()));
        }
        if stop.is_some_and(|f| f.load(std::sync::atomic::Ordering::SeqCst)) {
            return Ok(Watch::Stopped(Stop::Requested));
        }
        // checked on every iteration: a descendant writing continuously must not hide the exit
        if let Some(s) = child.try_wait()? {
            return drained(s, rx, out, usage, limits, deadline);
        }
        match rx.recv_timeout((deadline - now).min(Duration::from_millis(200))) {
            Ok(Event::Line(line)) => {
                if let Some(stop) = on_line(&line, out, usage, limits)? {
                    return Ok(Watch::Stopped(stop));
                }
            }
            Ok(Event::End) | Ok(Event::Broken) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                // no more stream: the leader should be finishing; otherwise the guard is blind
                let until = (Instant::now() + DRAIN).min(deadline);
                loop {
                    if let Some(s) = child.try_wait()? {
                        return Ok(Watch::Exited(s));
                    }
                    if Instant::now() >= until {
                        return Ok(Watch::Stopped(Stop::StreamLost(
                            "stdout closed or unreadable while the process kept running".into(),
                        )));
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

/// After the leader exited: a bounded drain of what is still buffered or written by descendants
/// (they are killed right after), every line still checked.
fn drained(
    status: ExitStatus,
    rx: &mpsc::Receiver<Event>,
    out: &mut std::fs::File,
    usage: &mut Usage,
    limits: &Limits,
    deadline: Instant,
) -> std::io::Result<Watch> {
    let until = (Instant::now() + DRAIN).min(deadline);
    while let Some(left) = until.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(Event::Line(line)) => {
                if let Some(stop) = on_line(&line, out, usage, limits)? {
                    return Ok(Watch::Stopped(stop));
                }
            }
            _ => break,
        }
    }
    Ok(Watch::Exited(status))
}

/// The usage a run is charged to the budget with (the `usage.run` value of its record), from the
/// guard's [`Outcome`] and the 02-01 collector's `usage.run` for the copied stream.
///
/// * A run the guard **stopped** is always charged as **unmeasured**, whatever `result` it wrote on
///   the way out: a process interrupted mid-request may report less than it used (the CLI writes
///   `error_during_execution` results with zero usage), so such a figure is never trusted. The
///   collector's figure and the guard's observed lower bound are kept for the report. An
///   unmeasured run stops the budget until a person decides ([`crate::budget::next`]).
/// * A run that **exited** on its own is charged with the collector's figure, unless that figure
///   is below what the guard observed (then unmeasured: the two disagree).
pub fn charge(outcome: &Outcome, collected_run: &Value) -> Value {
    let observed = serde_json::json!({
        "billable_lower_bound": outcome.observed.billable,
        "cache_read_lower_bound": outcome.observed.cache_read,
        "messages": outcome.observed.messages,
        "incomplete_lines": outcome.observed.incomplete_lines,
    });
    let unmeasured = |reason: String| {
        serde_json::json!({"state": "unmeasured", "reason": reason,
                           "guard_observed": observed, "collector_reported": collected_run})
    };
    if outcome.stop != Stop::Exited {
        return unmeasured(format!(
            "stopped by the run guard ({:?}); a result written after the stop is not trusted",
            outcome.stop
        ));
    }
    if collected_run["state"] == "measured" {
        let t = &collected_run["tokens"];
        let n = |k: &str| t[k].as_u64();
        let billable = n("input")
            .zip(n("output"))
            .zip(n("cache_creation"))
            .map(|((i, o), c)| i.saturating_add(o).saturating_add(c));
        match (billable, n("cache_read")) {
            (Some(b), Some(cr))
                if b >= outcome.observed.billable && cr >= outcome.observed.cache_read => {}
            _ => {
                return unmeasured(
                    "collector figure below the guard's observed lower bound (or incomplete)"
                        .into(),
                );
            }
        }
    }
    collected_run.clone()
}
