//! The no-extra-billing rule for `rate_limit_event`s (user decision 2026-11-17, warning;
//! `docs/poc/User_Decisions_2026-11-17_warning.md`), without model calls:
//!
//! * one case table (`tests/fixtures/rate_limit_cases.json`) is judged by the supervisor's rule
//!   ([`budget::rate_limit_verdict`]) and by the launcher's copy of it
//!   (`docs/poc/reports/tasks/02-05/step8-plan-20261117/launcher/rate_limit_policy.py`, run with
//!   `python3`): both must give the expected kind and the same text, so the run guard, the
//!   observation check, the caller's record and the post-run check cannot drift apart;
//! * the real event of 02-05 `s8-r1` (CLI 2.1.322, 2026-11-17 12:19 UTC) passes every path as a
//!   warning now; the policy before this decision stopped on it;
//! * the status display (provider) judges like the budget.

use ieo_supervisor::budget::{self, BUDGET_0205, Decision, Observation, RateLimitVerdict};
use ieo_supervisor::provider;
use serde_json::{Value, json};

const CASES: &str = include_str!("fixtures/rate_limit_cases.json");
const S8_R1_EVENT: &str = include_str!("fixtures/s8-r1-rate-limit-event.jsonl");
const POLICY_PY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/poc/reports/tasks/02-05/step8-plan-20261117/launcher/rate_limit_policy.py"
);

fn kind(v: &RateLimitVerdict) -> (&'static str, Option<String>) {
    match v {
        RateLimitVerdict::Allowed => ("allowed", None),
        RateLimitVerdict::Warning(w) => ("warning", Some(w.clone())),
        RateLimitVerdict::Stop(s) => ("stop", Some(s.clone())),
    }
}

fn cases() -> Vec<Value> {
    serde_json::from_str::<Value>(CASES)
        .expect("cases")
        .as_array()
        .expect("array")
        .clone()
}

fn fresh(stream: &str) -> Option<Observation<'_>> {
    Some(Observation {
        stream,
        age_seconds: 60,
    })
}

#[test]
fn every_case_has_its_expected_verdict() {
    let cs = cases();
    assert!(cs.len() >= 30);
    for c in &cs {
        let (k, _) = kind(&budget::rate_limit_verdict(&c["event"]));
        assert_eq!(k, c["expected"], "{}", c["name"]);
        // the three paths agree with the verdict
        let line = c["event"].to_string();
        let stop = k == "stop";
        assert_eq!(budget::stream_stop(&line).is_some(), stop, "{}", c["name"]);
        assert_eq!(
            budget::observation_check(fresh(&line)).is_err(),
            stop,
            "{}",
            c["name"]
        );
        assert_eq!(
            budget::overage_check(&c["event"]).is_err(),
            stop,
            "{}",
            c["name"]
        );
        assert_eq!(
            budget::rate_limit_warnings(&line).len(),
            usize::from(k == "warning"),
            "{}",
            c["name"]
        );
    }
}

#[test]
fn the_launcher_rule_is_the_supervisor_rule() {
    // python3 is on every runner of this repository (call_sh.rs needs it too)
    let script = r#"
import importlib.util, json, sys
spec = importlib.util.spec_from_file_location("p", sys.argv[1])
p = importlib.util.module_from_spec(spec); spec.loader.exec_module(p)
cases = json.load(sys.stdin)
print(json.dumps([list(p.verdict(c["event"])) for c in cases]))
"#;
    let mut child = std::process::Command::new("python3")
        .args(["-I", "-c", script, POLICY_PY])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("python3");
    {
        use std::io::Write;
        let mut i = child.stdin.take().expect("stdin");
        i.write_all(CASES.as_bytes()).expect("write");
    }
    let out = child.wait_with_output().expect("python3 output");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let py: Vec<(String, Option<String>)> =
        serde_json::from_slice(&out.stdout).expect("python verdicts");
    let cs = cases();
    assert_eq!(py.len(), cs.len());
    for (c, (pk, pt)) in cs.iter().zip(py) {
        let (rk, rt) = kind(&budget::rate_limit_verdict(&c["event"]));
        assert_eq!((rk, rt), (pk.as_str(), pt), "{}", c["name"]);
    }
}

#[test]
fn the_real_s8_r1_event_is_now_a_warning_on_every_path() {
    let line = S8_R1_EVENT.trim_end();
    let ev: Value = serde_json::from_str(line).expect("event");
    // what the CLI 2.1.322 sent: a 7-day warning at 86 %, paid extra usage off (out of credits)
    assert_eq!(ev["rate_limit_info"]["status"], "allowed_warning");
    assert_eq!(ev["rate_limit_info"]["overageStatus"], "rejected");
    assert_eq!(ev["rate_limit_info"]["isUsingOverage"], false);
    let RateLimitVerdict::Warning(w) = budget::rate_limit_verdict(&ev) else {
        panic!("not a warning");
    };
    assert_eq!(
        w,
        "rate limit warning (allowed_warning; paid extra usage off): seven_day 0.86 of the limit \
         (threshold 0.75, resets 1795334400); window seven_day 0.86 (threshold 0.75, resets \
         1795334400)"
    );
    // run guard: no stop (the old rule stopped here: "rate limit status \"allowed_warning\"")
    assert_eq!(budget::stream_stop(line), None);
    // observation check before a call: the stream is a valid observation within 7 days
    assert!(budget::observation_check(fresh(line)).is_ok());
    assert!(
        budget::observation_check(Some(Observation {
            stream: line,
            age_seconds: budget::OBSERVATION_MAX_AGE_SECONDS + 1
        }))
        .is_err(),
        "the 7-day validity stays"
    );
    assert!(matches!(
        budget::next(&BUDGET_0205, &[], fresh(line)),
        Decision::Start { .. }
    ));
    // status display: available, with the warning beside it
    let c = provider::classify(line);
    assert_eq!(c["value"], "available");
    assert_eq!(c["warning"], json!(w));
}

#[test]
fn a_later_warning_never_clears_an_earlier_stop() {
    let warn = S8_R1_EVENT.trim_end();
    let mut rejected: Value = serde_json::from_str(warn).expect("e");
    rejected["rate_limit_info"]["status"] = json!("rejected");
    let mut overage: Value = serde_json::from_str(warn).expect("e");
    overage["rate_limit_info"]["overageStatus"] = json!("allowed");
    let mut missing: Value = serde_json::from_str(warn).expect("e");
    missing["rate_limit_info"]
        .as_object_mut()
        .expect("o")
        .remove("isUsingOverage");
    for (stop, display) in [
        (rejected, "usage_limit"),
        (overage, "unknown"),
        (missing, "unknown"),
    ] {
        let stop = stop.to_string();
        for stream in [format!("{warn}\n{stop}\n"), format!("{stop}\n{warn}\n")] {
            assert!(
                budget::observation_check(fresh(&stream)).is_err(),
                "{stream}"
            );
            assert!(matches!(
                budget::next(&BUDGET_0205, &[], fresh(&stream)),
                Decision::Hold { .. }
            ));
            // the status display keeps the stop too
            assert_eq!(provider::classify(&stream)["value"], display, "{stream}");
            assert!(provider::classify(&stream)["warning"].is_null());
        }
        assert!(budget::stream_stop(&stop).is_some());
    }
}

/// The whole kept stream of s8-r1, when its path is given (`IEO_S8R1_STREAM`; the raw stream
/// stays in the ledger outside the repository). Without it this test checks nothing.
#[test]
fn the_kept_s8_r1_stream_replays_without_a_stop() {
    let Ok(p) = std::env::var("IEO_S8R1_STREAM") else {
        return;
    };
    let text = std::fs::read_to_string(&p).expect("stream");
    let events = budget::rate_limit_events(&text).expect("events");
    assert_eq!(events.len(), 1);
    for line in text.lines() {
        assert_eq!(budget::stream_stop(line), None, "{line}");
    }
    assert!(budget::observation_check(fresh(&text)).is_ok());
    assert_eq!(budget::rate_limit_warnings(&text).len(), 1);
    let c = provider::classify(&text);
    assert_eq!(c["value"], "available");
    assert!(c["warning"].is_string());
    println!(
        "replayed {} ({} lines): guard no stop, observation ok, 1 warning, display available",
        p,
        text.lines().count()
    );
}
