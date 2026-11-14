//! In-run budget enforcement (user decision 2026-11-14 §3), without model calls: a fake `claude`
//! (a shell script) emits stream-json lines, spawns children, sleeps or ignores SIGTERM.

use ieo_supervisor::budget::{self, BUDGET_0204, BUDGET_X02, Decision};
use ieo_supervisor::run::{self, Limits, Stop};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

struct Tmp(PathBuf);
impl Tmp {
    fn new(name: &str) -> Tmp {
        let d = std::env::temp_dir().join(format!(
            "ieo-run-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&d).expect("tmp");
        Tmp(d)
    }
    fn p(&self) -> &Path {
        &self.0
    }
}
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn assistant(id: &str, input: u64, output: u64, cc: u64, cr: u64) -> String {
    json!({"type": "assistant", "message": {"id": id, "usage": {"input_tokens": input, "output_tokens": output,
           "cache_creation_input_tokens": cc, "cache_read_input_tokens": cr}}})
    .to_string()
}

fn rate(overage: &str) -> String {
    json!({"type": "rate_limit_event", "rate_limit_info": {"status": "allowed", "overageStatus": overage,
           "isUsingOverage": false}})
    .to_string()
}

/// A fake role process: prints `lines` (one per 50 ms), then runs `tail` (shell).
fn fake(dir: &Path, lines: &[String], tail: &str) -> Command {
    let script = dir.join("fake-claude.sh");
    let mut body = String::from("#!/bin/sh\n");
    for l in lines {
        body.push_str(&format!(
            "printf '%s\\n' '{}'\nsleep 0.05\n",
            l.replace('\'', "'\\''")
        ));
    }
    body.push_str(tail);
    body.push('\n');
    std::fs::write(&script, body).expect("script");
    let mut c = Command::new("sh");
    c.arg(&script);
    c
}

fn limits(timeout_ms: u64) -> Limits {
    Limits {
        timeout: Duration::from_millis(timeout_ms),
        timeout_is_total: false,
        billable_room: 1_000_000,
        cache_read_room: 1_000_000,
        grace: Duration::from_secs(1),
    }
}

/// Runs the guard with a watchdog: a guard that never returns fails the test instead of hanging.
fn go(dir: &Path, cmd: Command, l: Limits) -> run::Outcome {
    let (out, err) = (dir.join("stdout.jsonl"), dir.join("stderr.txt"));
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(run::run(cmd, l, &out, &err));
    });
    rx.recv_timeout(l.timeout + l.grace + Duration::from_secs(15))
        .expect("the guard did not return (deadline or SIGKILL not enforced)")
        .expect("run")
}

/// The pid written by the fake into `file` is gone (not alive, not a zombie of ours).
fn gone(file: &Path) -> bool {
    let pid: u32 = std::fs::read_to_string(file)
        .expect("pid file")
        .trim()
        .parse()
        .expect("pid");
    let t0 = std::time::Instant::now();
    while t0.elapsed() < Duration::from_secs(5) {
        match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
            Err(_) => return true,
            Ok(s) if s.split(' ').nth(2) == Some("Z") => return true,
            Ok(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    }
    false
}

#[test]
fn a_normal_run_is_allowed_and_measured() {
    let d = Tmp::new("ok");
    let lines = [
        json!({"type": "system", "subtype": "init"}).to_string(),
        assistant("m1", 2, 6, 1900, 649),
        rate("rejected"),
        assistant("m2", 3, 10, 0, 2549),
        json!({"type": "result", "subtype": "success"}).to_string(),
    ];
    let o = go(d.p(), fake(d.p(), &lines, "exit 0"), limits(10_000));
    assert_eq!((o.stop.clone(), o.exit_code), (Stop::Exited, Some(0)));
    assert_eq!(
        (
            o.observed.billable,
            o.observed.cache_read,
            o.observed.messages
        ),
        (1921, 3198, 2)
    );
    let copied = std::fs::read_to_string(d.p().join("stdout.jsonl")).expect("copy");
    assert_eq!(copied.lines().count(), lines.len());
}

#[test]
fn a_repeated_message_counts_once_with_its_largest_usage() {
    let d = Tmp::new("dup");
    let lines = [
        assistant("m1", 1, 10, 0, 0),
        assistant("m1", 1, 40, 0, 0),
        assistant("m1", 1, 25, 0, 0),
    ];
    let o = go(d.p(), fake(d.p(), &lines, "exit 0"), limits(10_000));
    assert_eq!((o.observed.billable, o.observed.messages), (41, 1));
}

#[test]
fn the_per_run_time_limit_ends_the_run_and_its_children() {
    let d = Tmp::new("timeout");
    let pid = d.p().join("child.pid");
    let tail = format!("sleep 300 & echo $! > {}\nsleep 300", pid.display());
    let o = go(
        d.p(),
        fake(d.p(), &[assistant("m1", 1, 1, 0, 0)], &tail),
        limits(1_000),
    );
    assert_eq!(o.stop, Stop::RunTimeout);
    assert!(o.elapsed < Duration::from_secs(4), "{:?}", o.elapsed);
    assert!(gone(&pid), "a child outlived the run");
}

#[test]
fn the_total_time_left_ends_the_run() {
    let d = Tmp::new("total");
    let mut l = limits(800);
    l.timeout_is_total = true;
    let o = go(d.p(), fake(d.p(), &[], "sleep 300"), l);
    assert_eq!(o.stop, Stop::TotalTimeout);
}

#[test]
fn a_process_ignoring_sigterm_is_killed_after_the_grace() {
    let d = Tmp::new("term");
    let pid = d.p().join("child.pid");
    let tail = format!(
        "trap '' TERM\n(trap '' TERM; sleep 300) & echo $! > {}\nwhile :; do sleep 1; done",
        pid.display()
    );
    let o = go(d.p(), fake(d.p(), &[], &tail), limits(500));
    assert_eq!((o.stop.clone(), o.exit_code), (Stop::RunTimeout, None));
    assert!(o.elapsed < Duration::from_secs(4), "{:?}", o.elapsed);
    assert!(gone(&pid));
}

#[test]
fn observed_tokens_reaching_the_room_stop_the_run() {
    let d = Tmp::new("tokens");
    let pid = d.p().join("child.pid");
    let lines = [
        assistant("m1", 100, 500, 0, 0),
        assistant("m2", 100, 300, 0, 0),
        assistant("m3", 100, 900, 0, 0),
    ];
    let tail = format!("sleep 300 & echo $! > {}\nsleep 300", pid.display());
    let mut l = limits(10_000);
    l.billable_room = 1_000;
    let o = go(d.p(), fake(d.p(), &lines, &tail), l);
    // stops on m2 (600 + 400 = 1,000 = room): m3 is never read
    assert_eq!(o.stop, Stop::TokenCap);
    assert_eq!((o.observed.billable, o.observed.messages), (1_000, 2));
    // the fake reached its tail before the stop only if it outran the guard; either way the
    // group is gone afterwards
    if pid.exists() {
        assert!(gone(&pid));
    }
    // one token below the room is allowed
    let d2 = Tmp::new("tokens-ok");
    let mut l = limits(10_000);
    l.billable_room = 1_001;
    let o = go(d2.p(), fake(d2.p(), &lines[..2], "exit 0"), l);
    assert_eq!(o.stop, Stop::Exited);
}

#[test]
fn observed_cache_read_reaching_the_room_stops_the_run() {
    let d = Tmp::new("cache");
    let mut l = limits(10_000);
    l.cache_read_room = 5_000;
    let o = go(
        d.p(),
        fake(d.p(), &[assistant("m1", 1, 1, 0, 5_000)], "sleep 300"),
        l,
    );
    assert_eq!(o.stop, Stop::CacheReadCap);
}

#[test]
fn an_extra_usage_or_limit_signal_stops_the_run() {
    for (event, why) in [
        (rate("allowed_warning"), "overage could be billed"),
        (rate("allowed"), "overage allowed"),
        (
            json!({"type": "rate_limit_event", "rate_limit_info": {"status": "allowed", "overageStatus": "rejected", "isUsingOverage": true}}).to_string(),
            "overage in use",
        ),
        (
            json!({"type": "rate_limit_event", "rate_limit_info": {"status": "rejected", "overageStatus": "rejected", "isUsingOverage": false}}).to_string(),
            "limit reached",
        ),
        (
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allo"#.to_string(),
            "truncated event",
        ),
    ] {
        let d = Tmp::new("overage");
        let o = go(d.p(), fake(d.p(), &[event], "sleep 300"), limits(10_000));
        assert!(matches!(o.stop, Stop::Overage(_)), "{why}: {:?}", o.stop);
        assert!(o.elapsed < Duration::from_secs(4), "{why}");
    }
}

#[test]
fn limits_for_the_next_run_come_from_the_budget() {
    let s = budget::spent(&[]);
    let l = Limits::for_next(&BUDGET_0204, &s).expect("room");
    assert_eq!(
        (
            l.timeout,
            l.timeout_is_total,
            l.billable_room,
            l.cache_read_room
        ),
        (Duration::from_secs(1_800), false, 1_500_000, 15_000_000)
    );
    let rec = |ms: u64, b: u64, cr: u64| {
        json!({"model_request": true, "attempt_id": "a", "command_id": "c", "duration_ms": ms,
               "usage": {"run": {"state": "measured", "tokens": {"input": b, "output": 0, "cache_creation": 0, "cache_read": cr}}}})
    };
    let s = budget::spent(&[rec(13_000_000, 1_000_000, 10_000_000)]);
    let l = Limits::for_next(&BUDGET_X02, &s).expect("room");
    assert_eq!(
        (
            l.timeout,
            l.timeout_is_total,
            l.billable_room,
            l.cache_read_room
        ),
        (Duration::from_secs(1_400), true, 2_000_000, 20_000_000)
    );
    // no room: no limits
    let six: Vec<_> = (0..6).map(|_| rec(1_000, 1, 0)).collect();
    assert!(Limits::for_next(&BUDGET_X02, &budget::spent(&six)).is_none());
}

#[test]
fn a_run_without_measurement_blocks_the_next_call() {
    // the guard observed usage, but the run ended without a result event (killed): its record is
    // unmeasured, and the pre-run check stops the budget — observed in-run sums never replace it
    let killed = json!({"model_request": true, "attempt_id": "a", "command_id": "c", "duration_ms": 1_000,
                        "usage": {"run": {"state": "unmeasured", "reason": "no result event (stopped by guard)"}}});
    let ok_stream = rate("rejected");
    let obs = budget::Observation {
        stream: &ok_stream,
        age_seconds: 1,
    };
    assert!(matches!(
        budget::next(&BUDGET_0204, &[killed], Some(obs)),
        Decision::Stop { .. }
    ));
}

#[test]
fn children_left_behind_by_a_normal_exit_are_cleaned_up() {
    let d = Tmp::new("orphan");
    let pid = d.p().join("child.pid");
    let tail = format!("sleep 300 & echo $! > {}\nexit 0", pid.display());
    let o = go(d.p(), fake(d.p(), &[], &tail), limits(10_000));
    assert_eq!((o.stop.clone(), o.exit_code), (Stop::Exited, Some(0)));
    assert!(gone(&pid), "a background child outlived a normal exit");
}
