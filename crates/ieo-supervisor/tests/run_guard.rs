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
        // only a failing test can leave processes behind: kill the fake's whole group and the
        // recorded children (pids are not reused while their owner test is still running)
        if std::thread::panicking()
            && let Ok(rd) = std::fs::read_dir(&self.0)
        {
            for e in rd.flatten() {
                let path = e.path();
                if path.extension().is_some_and(|x| x == "pid")
                    && let Ok(pid) = std::fs::read_to_string(&path)
                {
                    let pid = pid.trim().to_string();
                    let target = if path.file_name().is_some_and(|n| n == "leader.pid") {
                        format!("-{pid}")
                    } else {
                        pid
                    };
                    let _ = Command::new("kill")
                        .args(["-s", "KILL", "--", &target])
                        .status();
                }
            }
        }
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
    // the leader records itself, so a failing test can still kill it (Tmp::drop)
    let mut body = format!(
        "#!/bin/sh\necho $$ > {}\n",
        dir.join("leader.pid").display()
    );
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
    assert!(!o.cleanup_failed);
    // the end of the run is noticed at once, not at the deadline
    assert!(o.elapsed < Duration::from_secs(3), "{:?}", o.elapsed);
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

fn ok_obs(stream: &str) -> Option<budget::Observation<'_>> {
    Some(budget::Observation {
        stream,
        age_seconds: 1,
    })
}

fn rec(ms: u64, b: u64, cr: u64) -> serde_json::Value {
    json!({"model_request": true, "attempt_id": "a", "command_id": "c", "duration_ms": ms,
           "usage": {"run": {"state": "measured", "tokens": {"input": b, "output": 0, "cache_creation": 0, "cache_read": cr}}}})
}

#[test]
fn limits_for_the_next_run_come_from_the_pre_run_decision() {
    let ok = rate("rejected");
    let l = Limits::for_next(&BUDGET_0204, &[], ok_obs(&ok)).expect("room");
    // the stop grace is taken out of the window, so the stop completes within the limit
    assert_eq!(
        (
            l.timeout,
            l.timeout_is_total,
            l.billable_room,
            l.cache_read_room,
            l.grace
        ),
        (
            Duration::from_secs(1_790),
            false,
            1_500_000,
            15_000_000,
            run::GRACE
        )
    );
    let l = Limits::for_next(
        &BUDGET_X02,
        &[rec(13_000_000, 1_000_000, 10_000_000)],
        ok_obs(&ok),
    )
    .expect("room");
    assert_eq!(
        (
            l.timeout,
            l.timeout_is_total,
            l.billable_room,
            l.cache_read_room
        ),
        (Duration::from_secs(1_390), true, 2_000_000, 20_000_000)
    );
    // exactly the per-run limit left is not "total"
    let l = Limits::for_next(&BUDGET_X02, &[rec(10_800_000, 1, 0)], ok_obs(&ok)).expect("room");
    assert_eq!(
        (l.timeout, l.timeout_is_total),
        (Duration::from_secs(3_590), false)
    );
    // refused whenever the pre-run decision refuses, and without room
    let unmeasured = json!({"model_request": true, "attempt_id": "a", "command_id": "c", "duration_ms": 1,
                            "usage": {"run": {"state": "unmeasured", "reason": "x"}}});
    let bad = rate("allowed_warning");
    let cases: Vec<(
        Vec<serde_json::Value>,
        Option<budget::Observation<'_>>,
        &str,
    )> = vec![
        (vec![unmeasured], ok_obs(&ok), "unmeasured run"),
        (
            vec![json!({"duration_ms": 1})],
            ok_obs(&ok),
            "unclassified record",
        ),
        (
            (0..6).map(|_| rec(1_000, 1, 0)).collect(),
            ok_obs(&ok),
            "no runs left",
        ),
        (vec![], ok_obs(&bad), "extra usage possible"),
        (vec![], None, "no observation"),
        (
            vec![rec(14_395_000, 1, 0)],
            ok_obs(&ok),
            "time left within the grace",
        ),
    ];
    for (records, obs, why) in cases {
        assert!(
            Limits::for_next(&BUDGET_X02, &records, obs).is_err(),
            "{why}"
        );
    }
    assert!(matches!(
        Limits::for_next(&BUDGET_X02, &[], None),
        Err(Decision::Hold { .. })
    ));
}

#[test]
fn a_stop_with_sigterm_first_lets_the_process_exit_within_the_grace() {
    let d = Tmp::new("term-first");
    let marker = d.p().join("got-term");
    let tail = format!(
        "trap 'echo term > {}; sleep 0.5; exit 0' TERM\nwhile :; do sleep 0.1; done",
        marker.display()
    );
    let mut l = limits(500);
    l.grace = Duration::from_secs(5);
    let o = go(d.p(), fake(d.p(), &[], &tail), l);
    assert_eq!((o.stop.clone(), o.exit_code), (Stop::RunTimeout, Some(0)));
    assert!(marker.exists(), "SIGTERM was not sent first");
    assert!(
        o.elapsed < Duration::from_secs(3),
        "the grace was not cut short: {:?}",
        o.elapsed
    );
}

#[test]
fn the_drain_after_exit_is_bounded_and_descendants_are_killed() {
    let d = Tmp::new("drain");
    let pid = d.p().join("child.pid");
    // the leader exits; a background child keeps the pipe open and writes forever
    let tail = format!(
        "(while :; do echo noise; sleep 0.1; done) & echo $! > {}\nexit 0",
        pid.display()
    );
    let o = go(d.p(), fake(d.p(), &[], &tail), limits(60_000));
    assert_eq!(o.stop, Stop::Exited);
    assert!(o.elapsed < Duration::from_secs(6), "{:?}", o.elapsed);
    assert!(gone(&pid));
}

#[test]
fn lines_after_the_leader_exits_are_still_checked() {
    let d = Tmp::new("late");
    let pid = d.p().join("child.pid");
    // a descendant writes a usage line after the leader has gone
    let late = assistant("late", 1, 5_000, 0, 0);
    let tail = format!(
        "(sleep 0.5; printf '%s\\n' '{late}'; sleep 300) & echo $! > {}\nexit 0",
        pid.display()
    );
    let mut l = limits(60_000);
    l.billable_room = 1_000;
    let o = go(d.p(), fake(d.p(), &[], &tail), l);
    assert_eq!(o.stop, Stop::TokenCap);
    assert!(gone(&pid));
}

#[test]
fn a_closed_or_unreadable_stream_stops_a_running_process() {
    let d = Tmp::new("closed");
    let o = go(
        d.p(),
        fake(d.p(), &[], "exec >&-\nsleep 300"),
        limits(60_000),
    );
    assert!(matches!(o.stop, Stop::StreamLost(_)), "{:?}", o.stop);
    assert!(o.elapsed < Duration::from_secs(6), "{:?}", o.elapsed);
    // bytes that are not UTF-8 do not blind the guard
    let d = Tmp::new("bytes");
    let tail = format!(
        "printf '\\377\\376 not utf-8\\n'\nprintf '%s\\n' '{}'\nsleep 300",
        assistant("m1", 1, 5_000, 0, 0)
    );
    let mut l = limits(60_000);
    l.billable_room = 1_000;
    let o = go(d.p(), fake(d.p(), &[], &tail), l);
    assert_eq!(o.stop, Stop::TokenCap);
}

#[test]
fn usage_lines_without_an_id_or_fields_are_counted_separately() {
    let d = Tmp::new("noid");
    let noid =
        json!({"type": "assistant", "message": {"usage": {"input_tokens": 1, "output_tokens": 599,
                      "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0}}})
        .to_string();
    let partial =
        json!({"type": "assistant", "message": {"id": "m9", "usage": {"input_tokens": 3}}})
            .to_string();
    let o = go(
        d.p(),
        fake(d.p(), &[noid.clone(), noid, partial], "exit 0"),
        limits(10_000),
    );
    assert_eq!(
        (
            o.observed.billable,
            o.observed.messages,
            o.observed.incomplete_lines
        ),
        (1_203, 3, 1)
    );
}

#[test]
fn an_unwritable_evidence_path_fails_before_anything_starts() {
    let d = Tmp::new("nopath");
    let marker = d.p().join("started");
    let cmd = fake(
        d.p(),
        &[],
        &format!("echo x > {}\nsleep 300", marker.display()),
    );
    let r = run::run(
        cmd,
        limits(5_000),
        &d.p().join("missing-dir/stdout.jsonl"),
        &d.p().join("stderr.txt"),
    );
    assert!(r.is_err());
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        !marker.exists(),
        "the process was started although evidence cannot be written"
    );
}

/// The record the 02-01 collector makes for a run, in the shape `budget` reads.
fn collected(stdout: &Path, ms: u64) -> serde_json::Value {
    use ieo_evidence::record::SessionMode;
    let bytes = std::fs::read(stdout).expect("stdout");
    let parsed = ieo_evidence::stream::parse(&bytes, SessionMode::New, None);
    json!({"model_request": true, "attempt_id": "a", "command_id": "c", "duration_ms": ms,
           "usage": {"run": serde_json::to_value(&parsed.usage.run).expect("usage")}})
}

#[test]
fn a_guard_stop_leaves_the_run_unmeasured_and_blocks_the_next_call() {
    let ok = rate("rejected");
    // stopped by the guard before its result event: the collector cannot measure it
    let d = Tmp::new("unmeasured");
    let mut l = limits(10_000);
    l.billable_room = 1_000;
    let o = go(
        d.p(),
        fake(d.p(), &[assistant("m1", 1, 2_000, 0, 0)], "sleep 300"),
        l,
    );
    assert_eq!(o.stop, Stop::TokenCap);
    let r = collected(&d.p().join("stdout.jsonl"), o.elapsed.as_millis() as u64);
    assert_eq!(r["usage"]["run"]["state"], "unmeasured", "{r}");
    assert!(matches!(
        budget::next(&BUDGET_0204, std::slice::from_ref(&r), ok_obs(&ok)),
        Decision::Stop { .. }
    ));
    assert!(Limits::for_next(&BUDGET_0204, &[r], ok_obs(&ok)).is_err());
    // positive control: a complete real stream (01-04) is measured and allows the next call
    let real = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/poc/reports/tasks/01-04/first-probes-20261112/evidence/attempts/01-P1-development/stdout.jsonl"
    ))
    .expect("stored 01-04 stream");
    let d = Tmp::new("measured");
    let lines: Vec<String> = real.lines().map(str::to_string).collect();
    let o = go(d.p(), fake(d.p(), &lines, "exit 0"), limits(10_000));
    assert_eq!(o.stop, Stop::Exited);
    let r = collected(&d.p().join("stdout.jsonl"), o.elapsed.as_millis() as u64);
    assert_eq!(r["usage"]["run"]["state"], "measured", "{r}");
    assert!(Limits::for_next(&BUDGET_0204, &[r], ok_obs(&real)).is_ok());
}

#[test]
fn children_left_behind_by_a_normal_exit_are_cleaned_up() {
    let d = Tmp::new("orphan");
    let pid = d.p().join("child.pid");
    // the child keeps stdout open and writes nothing: the exit must still be noticed at once
    let tail = format!("sleep 300 & echo $! > {}\nexit 0", pid.display());
    let o = go(d.p(), fake(d.p(), &[], &tail), limits(30_000));
    assert_eq!((o.stop.clone(), o.exit_code), (Stop::Exited, Some(0)));
    assert!(o.elapsed < Duration::from_secs(6), "{:?}", o.elapsed);
    assert!(gone(&pid), "a background child outlived a normal exit");
}

// --- review d2989f7 ---

#[test]
fn output_written_on_the_way_out_is_kept() {
    // the stopping line and what follows it (a result) are copied
    let d = Tmp::new("after-stop");
    let result =
        json!({"type": "result", "subtype": "success", "usage": {"output_tokens": 9}}).to_string();
    let mut l = limits(10_000);
    l.billable_room = 1_000;
    // the result is written right behind the stopping line (already queued when the guard stops)
    let tail = format!(
        "printf '%s\\n' '{}' '{}'\nsleep 300",
        assistant("m1", 1, 2_000, 0, 0),
        result
    );
    let o = go(d.p(), fake(d.p(), &[], &tail), l);
    assert_eq!(o.stop, Stop::TokenCap);
    let copied = std::fs::read_to_string(d.p().join("stdout.jsonl")).expect("copy");
    assert!(
        copied.contains("\"m1\""),
        "the stopping line was not copied: {copied}"
    );
    assert!(
        copied.contains(&result),
        "a line after the stop was dropped: {copied}"
    );
    // a TERM handler that writes its result during the grace
    let d = Tmp::new("term-result");
    let tail = format!(
        "trap 'printf \"%s\\n\" \"{}\"; exit 0' TERM\nwhile :; do sleep 0.1; done",
        result.replace('"', "\\\"")
    );
    let mut l = limits(500);
    l.grace = Duration::from_secs(5);
    let o = go(d.p(), fake(d.p(), &[], &tail), l);
    assert_eq!(o.stop, Stop::RunTimeout);
    let copied = std::fs::read_to_string(d.p().join("stdout.jsonl")).expect("copy");
    assert!(
        copied.contains("\"result\""),
        "the result written during the grace was dropped: {copied:?}"
    );
}

/// A `kill` that always fails.
fn broken_kill(dir: &Path) -> String {
    let k = dir.join("kill");
    std::fs::write(&k, "#!/bin/sh\nexit 1\n").expect("kill");
    let _ = Command::new("chmod").args(["+x"]).arg(&k).status();
    k.to_string_lossy().to_string()
}

#[test]
fn a_failed_group_kill_is_reported_and_the_leader_is_still_stopped() {
    // normal exit with a background child the broken kill cannot reach: reported
    let d = Tmp::new("badkill");
    let pid = d.p().join("child.pid");
    let tail = format!("sleep 300 & echo $! > {}\nexit 0", pid.display());
    let kill = broken_kill(d.p());
    let o = run::run_with_kill(
        fake(d.p(), &[], &tail),
        limits(10_000),
        &d.p().join("stdout.jsonl"),
        &d.p().join("stderr.txt"),
        &kill,
    )
    .expect("run");
    assert_eq!(o.stop, Stop::Exited);
    assert!(
        o.cleanup_failed,
        "a surviving child was reported as cleaned up"
    );
    let child = std::fs::read_to_string(&pid).expect("pid");
    let _ = Command::new("kill")
        .args(["-s", "KILL", child.trim()])
        .status();
    // a stop with the broken kill: the leader is killed directly, within the watchdog
    let d = Tmp::new("badkill-stop");
    let kill = broken_kill(d.p());
    let (out, err) = (d.p().join("stdout.jsonl"), d.p().join("stderr.txt"));
    let cmd = fake(d.p(), &[], "trap '' TERM\nwhile :; do sleep 0.1; done");
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut l = limits(300);
        l.grace = Duration::from_millis(300);
        let _ = tx.send(run::run_with_kill(cmd, l, &out, &err, &kill));
    });
    let o = rx
        .recv_timeout(Duration::from_secs(15))
        .expect("the leader was not stopped without group signalling")
        .expect("run");
    assert_eq!((o.stop.clone(), o.exit_code), (Stop::RunTimeout, None));
    assert!(o.cleanup_failed);
}

#[test]
fn the_stop_grace_must_fit_in_the_time_left() {
    let ok = rate("rejected");
    // 10 s left = the grace: refused; 11 s left: 1 s of run time
    assert!(Limits::for_next(&BUDGET_X02, &[rec(14_390_000, 1, 0)], ok_obs(&ok)).is_err());
    let l = Limits::for_next(&BUDGET_X02, &[rec(14_389_000, 1, 0)], ok_obs(&ok)).expect("room");
    assert_eq!(
        (l.timeout, l.timeout_is_total),
        (Duration::from_secs(1), true)
    );
}
