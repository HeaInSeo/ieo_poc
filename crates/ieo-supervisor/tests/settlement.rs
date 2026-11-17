//! Settlement of a guard-stopped run from its session transcript (user decision 2026-11-17
//! r1-settlement (가)/(A)), without model calls: a ledger shaped like 02-05 `s8-r1` (the guard
//! stopped the run on the old rule's `allowed_warning`; no result event; unmeasured) with a
//! synthetic stream and transcript of the same structure (the real transcript stays outside
//! the repository).

use ieo_supervisor::{budget, caller, settlement};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const SID: &str = "5c991c71-b6e2-4cfb-bd76-6bb18a36e090";
const WARN: &str = include_str!("fixtures/s8-r1-rate-limit-event.jsonl");
const OLD_STOP: &str = "Overage(\"rate limit status \\\"allowed_warning\\\" (not allowed)\")";

struct Tmp(PathBuf);
impl Tmp {
    fn new(name: &str) -> Tmp {
        let d = std::env::temp_dir().join(format!(
            "ieo-settle-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&d).expect("tmp");
        Tmp(d)
    }
}
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn sha(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

fn plan(root: &Path) -> caller::Plan {
    std::fs::write(root.join("adapter.token"), "t").expect("tok");
    caller::Plan::from_json(
        &json!({"budget": "02-05", "ledger_dir": root.join("ledger"),
        "roles": {"development": {"url": "dev", "adapter_token_file": root.join("adapter.token")},
                  "guardrail": {"url": "gr", "adapter_token_file": root.join("adapter.token")}}}),
    )
    .expect("plan")
}

fn assistant_stream(output: u64) -> String {
    json!({"type": "assistant", "message": {"id": "msg_1", "usage": {"input_tokens": 2,
        "output_tokens": output, "cache_creation_input_tokens": 1411, "cache_read_input_tokens": 3223}},
        "session_id": SID})
    .to_string()
}

/// The kept stream: init, two partial snapshots of one message, a tool result, `event`.
fn stream(event: &str) -> String {
    [
        json!({"type": "system", "subtype": "init", "session_id": SID, "tools": ["Glob", "Grep", "Read"]})
            .to_string(),
        assistant_stream(16),
        json!({"type": "user", "session_id": SID}).to_string(),
        assistant_stream(16),
        event.trim_end().to_string(),
        json!({"type": "user", "session_id": SID}).to_string(),
    ]
    .join("\n")
        + "\n"
}

/// A transcript like the CLI's: one request, one message written twice (final usage), the
/// exit accounting, then the last tool result.
fn transcript(cost: Option<[u64; 4]>, requests: usize) -> String {
    let e = |v: Value| {
        let mut v = v;
        v["sessionId"] = json!(SID);
        v.to_string()
    };
    let msg = || {
        e(
            json!({"type": "assistant", "isSidechain": false, "timestamp": "2026-11-17T12:19:48.592Z",
            "message": {"id": "msg_1", "usage": {"input_tokens": 2, "output_tokens": 130,
            "cache_creation_input_tokens": 1411, "cache_read_input_tokens": 3223}}}),
        )
    };
    let mut l = vec![e(json!({"type": "custom-title"}))];
    for _ in 0..requests {
        l.push(e(
            json!({"type": "api-request", "timestamp": "2026-11-17T12:19:47.084Z"}),
        ));
    }
    l.push(msg());
    l.push(e(json!({"type": "user", "isSidechain": false})));
    l.push(msg());
    if let Some([i, o, cc, cr]) = cost {
        l.push(e(
            json!({"type": "cost-state", "modelUsage": {"claude-sonnet-5-5": {
            "inputTokens": i, "outputTokens": o, "cacheCreationInputTokens": cc,
            "cacheReadInputTokens": cr}}}),
        ));
    }
    l.push(e(json!({"type": "user", "isSidechain": false})));
    l.join("\n") + "\n"
}

const EXIT: [u64; 4] = [2, 130, 1411, 3223];

/// An edit of the R1-like record.
type Patch = Box<dyn Fn(&mut Value)>;

struct Ledger {
    t: Tmp,
    p: caller::Plan,
    copy: PathBuf,
    copy_sha: String,
    report: PathBuf,
    /// the test pin of this ledger's run, for a debug `ieo-role-call` (`IEO_SETTLEMENT_TEST_PINS`)
    pin: String,
}

const DECISION: &str = settlement::DECISION;

/// A ledger with one R1-like record (`patch` edits it) over a stream ending in `event`, and an
/// evidence copy of `tr`.
fn ledger(name: &str, event: &str, tr: &str, patch: impl Fn(&mut Value)) -> Ledger {
    let t = Tmp::new(name);
    let p = plan(&t.0);
    let streams = t.0.join("ledger/streams");
    std::fs::create_dir_all(&streams).expect("dirs");
    let s = stream(event);
    let sp = streams.join("s8-r1.jsonl");
    std::fs::write(&sp, &s).expect("stream");
    let mut r = json!({"format": caller::RECORD_FORMAT, "attempt_id": "s8-r1", "budget": "02-05",
        "role": "development", "command_id": "development-start", "task_id": "90-01",
        "model_request": true, "duration_ms": 4119, "children": [],
        "session": {"mode": "new", "session_id": SID, "resume_base_attempt": null},
        "outcome": {"stop": OLD_STOP, "stopped_by_guard": true, "exit_code": 143, "result": null,
            "observed": {"billable_lower_bound": 1429, "cache_read_lower_bound": 3223,
                         "incomplete_lines": 0, "messages": 1}},
        "result_event": {"present": false},
        "usage": {"run": {"state": "unmeasured", "reason": "stopped by the run guard"}},
        "stream": {"path": sp, "bytes": s.len(), "sha256": sha(s.as_bytes())},
        "limits": {"billable_room": 150000, "cache_read_room": 1500000, "grace_ms": 6000,
                   "timeout_is_total": false, "timeout_ms": 594000},
        "stop_required": true,
        "stop_reason": "no result event; usage unmeasured: \"stopped by the run guard\"; this run's signal: rate limit status \"allowed_warning\" (not allowed)"});
    // the decision pins 02-05 s8-r1 of its session and its real kept stream; a synthetic
    // ledger's run gets a test pin for exactly its stream (debug builds only)
    let stream_sha = sha(s.as_bytes());
    settlement::allow_test_pin(settlement::Pin {
        budget: "02-05".into(),
        attempt: "s8-r1".into(),
        session: SID.into(),
        stream_sha256: Some(stream_sha.clone()),
    });
    let pin = format!("02-05|s8-r1|{SID}|{stream_sha}");
    patch(&mut r);
    std::fs::write(t.0.join("ledger/records.jsonl"), format!("{r}\n")).expect("records");
    let copy = t.0.join("evidence-copy.jsonl");
    std::fs::write(&copy, tr).expect("copy");
    let copy_sha = sha(tr.as_bytes());
    let report = t.0.join("verification.md");
    std::fs::write(&report, "# independent verification (test)\n").expect("report");
    Ledger {
        t,
        p,
        copy,
        copy_sha,
        report,
        pin,
    }
}

fn r1_ledger(name: &str) -> Ledger {
    ledger(name, WARN, &transcript(Some(EXIT), 1), |_| {})
}

fn settle(l: &Ledger, dry: bool) -> Result<Value, String> {
    caller::settle(
        &l.p,
        "s8-r1",
        &l.copy,
        "/role-volume/transcript.jsonl",
        &l.copy_sha,
        DECISION,
        &check(l),
        dry,
    )
}

/// The independent check a settlement follows (verdict as the s8-r1 verification gave it): the
/// report file (absolute) and its SHA-256.
fn check(l: &Ledger) -> Value {
    json!({"evidence": l.report, "sha256": sha(&std::fs::read(&l.report).expect("report")),
           "verdict": "CONFIRMED WITH CAVEAT"})
}

fn lines(l: &Ledger) -> Vec<Value> {
    std::fs::read_to_string(l.t.0.join("ledger/records.jsonl"))
        .expect("records")
        .lines()
        .map(|x| serde_json::from_str(x).expect("json"))
        .collect()
}

fn marker() -> Value {
    json!({"spare": {"reason": "stopped on the old rule's warning; rerun after the rule fix",
        "replaces": "s8-r1", "evidence_ref": "RESUME_PLAN.md",
        "by": {"kind": "user", "name": "heain"}, "approved_at": "2026-11-18T00:00:00Z"}})
}

#[test]
fn the_transcript_accounting_counts_each_message_once_and_must_equal_the_exit_accounting() {
    let a = settlement::transcript_accounting(transcript(Some(EXIT), 1).as_bytes(), SID)
        .expect("accounting");
    assert_eq!(
        (a.tokens, a.api_requests, a.messages, a.billable()),
        (EXIT, 1, 1, Some(1543))
    );
    for (tr, why) in [
        (transcript(None, 1), "no cost-state"),
        (
            transcript(Some([2, 130, 1411, 3222]), 1),
            "exit accounting differs",
        ),
        (
            transcript(Some([2, 131, 1411, 3223]), 1),
            "exit accounting differs",
        ),
        (transcript(Some(EXIT), 0), "no api-request"),
        (
            // nothing requested, nothing used, an exit accounting of zero: still not enough
            [
                json!({"type": "custom-title"}),
                json!({"type": "cost-state", "modelUsage": {"m": {
                "inputTokens": 0, "outputTokens": 0, "cacheCreationInputTokens": 0,
                "cacheReadInputTokens": 0}}}),
            ]
            .iter()
            .map(|v| {
                let mut v = v.clone();
                v["sessionId"] = json!(SID);
                v.to_string() + "\n"
            })
            .collect::<String>(),
            "no request at all",
        ),
        (transcript(Some(EXIT), 2), "two requests, one message"),
        (
            transcript(Some(EXIT), 1).replace(SID, "other-session"),
            "another session",
        ),
        (
            transcript(Some(EXIT), 1).replacen("\"isSidechain\":false", "\"isSidechain\":true", 1),
            "a side-chain entry",
        ),
        (transcript(Some(EXIT), 1) + "{not json\n", "a broken line"),
        (
            transcript(Some(EXIT), 1).replace("\"output_tokens\":130", "\"output_tokens\":\"130\""),
            "a token count that is no number",
        ),
    ] {
        assert!(
            settlement::transcript_accounting(tr.as_bytes(), SID).is_err(),
            "{why}"
        );
    }
}

#[test]
fn a_settled_r1_counts_once_with_the_transcript_usage_and_stays_failed() {
    let l = r1_ledger("ok");
    // a dry run writes nothing
    let d = settle(&l, true).expect("dry run");
    assert_eq!(d["usage_run"]["tokens"]["output"], 130);
    assert_eq!(lines(&l).len(), 1);
    assert!(!l.t.0.join("ledger/children").exists());
    let s = settle(&l, false).expect("settle");
    assert_eq!(s["format"], settlement::SETTLEMENT_FORMAT);
    assert_eq!(s["source"], "transcript_exit_accounting");
    // the verifier's caveat travels with the record
    assert_eq!(s["verification"]["uncovered"], settlement::UNCOVERED);
    assert!(
        s["verification"]["residual"]
            .as_str()
            .is_some_and(|r| r.contains("SIGTERM"))
    );
    assert_eq!(s["independent_check"]["verdict"], "CONFIRMED WITH CAVEAT");
    assert_eq!(s["evidence"]["sha256"], l.copy_sha.as_str());
    assert_eq!(s["verification"]["api_requests"], 1);
    // the original line is untouched; one line is added
    let raw = lines(&l);
    assert_eq!(raw.len(), 2);
    assert_eq!(raw[0]["usage"]["run"]["state"], "unmeasured");
    let recs = caller::records(&l.p).expect("records");
    assert_eq!(recs.len(), 1, "the settlement line is no run");
    let r = &recs[0];
    assert_eq!(r["usage"]["run"]["state"], "measured");
    assert_eq!(
        r["usage"]["run"]["tokens"],
        json!({"input": 2, "output": 130, "cache_creation": 1411, "cache_read": 3223})
    );
    // the failure stays: the guard's stop left no result
    assert_eq!(
        (r["stop_required"].clone(), r["stop_reason"].clone()),
        (json!(true), json!("no result event"))
    );
    assert_eq!(r["before_settlement"]["usage_run"]["state"], "unmeasured");
    // counted once, never with the lower bound 1,429
    let sp = budget::spent(&recs);
    assert_eq!(
        (
            sp.runs,
            sp.seconds,
            sp.billable_tokens,
            sp.cache_read_tokens,
            sp.unmeasured_runs.len()
        ),
        (1, 5, 1543, 3223, 0)
    );
    // the spare (A): a person's marker may release this stop, once
    assert!(caller::spare_releasable(r).is_ok());
    let raw = caller::raw_records(&l.p).expect("raw");
    let g = caller::spare_gate(&l.p, "development", &recs, &raw, &marker()).expect("spare");
    assert_eq!(g.expect("marker")["replaces"], "s8-r1");
    // a second settlement is refused, nothing written
    assert!(
        settle(&l, false)
            .expect_err("again")
            .contains("already settled")
    );
    assert_eq!(lines(&l).len(), 2);
}

#[test]
fn a_changed_or_repeated_or_misdirected_settlement_makes_the_ledger_unusable() {
    let l = r1_ledger("tamper");
    settle(&l, false).expect("settle");
    let copy =
        l.t.0
            .join("ledger/children/s8-r1/settlement-transcript.jsonl");
    let rec = l.t.0.join("ledger/records.jsonl");
    let orig_rec = std::fs::read_to_string(&rec).expect("r");
    let orig_copy = std::fs::read(&copy).expect("c");
    // the kept copy changed
    std::fs::write(
        &copy,
        transcript(Some(EXIT), 1).replace("\"output_tokens\":130", "\"output_tokens\":13"),
    )
    .expect("w");
    assert!(caller::records(&l.p).is_err(), "changed copy");
    std::fs::write(&copy, &orig_copy).expect("w");
    assert!(caller::records(&l.p).is_ok());
    let raw = lines(&l);
    let edit = |f: &dyn Fn(&mut Value)| {
        let mut s = raw[1].clone();
        f(&mut s);
        std::fs::write(&rec, format!("{}\n{s}\n", raw[0])).expect("w");
        let r = caller::records(&l.p);
        std::fs::write(&rec, &orig_rec).expect("w");
        r
    };
    for (why, f) in [
        (
            "other usage",
            &(|s: &mut Value| s["usage_run"]["tokens"]["output"] = json!(16))
                as &dyn Fn(&mut Value),
        ),
        ("another attempt", &|s: &mut Value| {
            s["settles"] = json!("s8-r2")
        }),
        ("another session", &|s: &mut Value| {
            s["session_id"] = json!("x")
        }),
        ("another stream", &|s: &mut Value| {
            s["original_stream_sha256"] = json!("00")
        }),
        ("no decision", &|s: &mut Value| s["decision"] = json!("")),
        ("no independent check", &|s: &mut Value| {
            s["independent_check"] = Value::Null
        }),
        ("a verdict that does not confirm", &|s: &mut Value| {
            s["independent_check"]["verdict"] = json!("REFUTED")
        }),
        ("the caveat dropped", &|s: &mut Value| {
            s["verification"]["uncovered"] = Value::Null
        }),
        ("the residual text changed", &|s: &mut Value| {
            s["verification"]["residual"] = json!("none")
        }),
        ("another source", &|s: &mut Value| {
            s["source"] = json!("observed_lower_bound")
        }),
        ("another copy digest", &|s: &mut Value| {
            s["evidence"]["sha256"] = json!("00")
        }),
        ("a relative copy path", &|s: &mut Value| {
            s["evidence"]["ledger_copy"] = json!("children/x")
        }),
        ("another request count", &|s: &mut Value| {
            s["verification"]["api_requests"] = json!(2)
        }),
    ] {
        assert!(edit(f).is_err(), "{why}");
    }
    // a duplicate line, a settlement before its run, a forged run line
    std::fs::write(&rec, format!("{}\n{}\n{}\n", raw[0], raw[1], raw[1])).expect("w");
    assert!(
        caller::records(&l.p)
            .expect_err("dup")
            .contains("more than one settlement")
    );
    std::fs::write(&rec, format!("{}\n{}\n", raw[1], raw[0])).expect("w");
    assert!(caller::records(&l.p).is_err(), "settlement before the run");
    let mut forged = raw[0].clone();
    forged["settlement"] = json!({"policy": settlement::POLICY});
    std::fs::write(&rec, format!("{forged}\n")).expect("w");
    assert!(
        caller::records(&l.p)
            .expect_err("forged")
            .contains("settlement fields")
    );
    // a correction of a settled run
    let corr =
        json!({"format": caller::CORRECTION_FORMAT, "corrects": "s8-r1", "model_request": false});
    std::fs::write(&rec, format!("{}\n{}\n{corr}\n", raw[0], raw[1])).expect("w");
    assert!(
        caller::records(&l.p)
            .expect_err("corr")
            .contains("settled already")
    );
}

#[test]
fn not_enough_evidence_writes_nothing() {
    for (name, tr, sha_ok) in [
        ("nocost", transcript(None, 1), true),
        ("diff", transcript(Some([2, 130, 1411, 3000]), 1), true),
        ("nosha", transcript(Some(EXIT), 1), false),
    ] {
        let l = ledger(name, WARN, &tr, |_| {});
        let r = if sha_ok {
            settle(&l, false)
        } else {
            caller::settle(
                &l.p,
                "s8-r1",
                &l.copy,
                "/x",
                &"0".repeat(64),
                "d",
                &check(&l),
                false,
            )
        };
        assert!(r.is_err(), "{name}");
        assert_eq!(lines(&l).len(), 1, "{name}");
        assert!(
            !l.t.0
                .join("ledger/children/s8-r1/settlement-transcript.jsonl")
                .exists(),
            "{name}"
        );
        // review 758ac03 O-1: no empty directory is left behind either
        assert!(!l.t.0.join("ledger/children").exists(), "{name}");
        assert_eq!(
            caller::records(&l.p).expect("records")[0]["usage"]["run"]["state"],
            "unmeasured",
            "{name}"
        );
    }
    // an accounting below the guard's observed lower bound is refused (the lower bound is never
    // the usage, and the usage is never below it)
    let low = transcript(Some([2, 130, 1411, 3000]), 1).replace(
        "\"cache_read_input_tokens\":3223",
        "\"cache_read_input_tokens\":3000",
    );
    assert!(settlement::transcript_accounting(low.as_bytes(), SID).is_ok());
    let l = ledger("low", WARN, &low, |_| {});
    assert!(settle(&l, false).expect_err("low").contains("lower bound"));
    assert_eq!(lines(&l).len(), 1);
    // a run carrying a correction mark is not settled
    let l = ledger("corrected", WARN, &transcript(Some(EXIT), 1), |r| {
        r["corrected_by"] = json!({"decision": "x"})
    });
    let s = {
        let ok = r1_ledger("for-line");
        settle(&ok, false).expect("settle");
        lines(&ok)[1].clone()
    };
    let rec = l.t.0.join("ledger/records.jsonl");
    let first = std::fs::read_to_string(&rec).expect("r");
    let mut s = s;
    s["evidence"]["ledger_copy"] = json!(l.copy);
    std::fs::write(&rec, format!("{first}{s}\n")).expect("w");
    assert!(
        caller::records(&l.p)
            .expect_err("corrected")
            .contains("corrected already")
    );
    let l = r1_ledger("noargs");
    assert!(
        caller::settle(
            &l.p,
            "s8-r1",
            &l.copy,
            "/x",
            &l.copy_sha,
            "",
            &check(&l),
            false
        )
        .is_err()
    );
    // an independent check needs its evidence, a SHA-256 and a confirming verdict
    for bad in [
        json!({"evidence": "", "sha256": "a".repeat(64), "verdict": "CONFIRMED"}),
        json!({"evidence": "r.md", "sha256": "abc", "verdict": "CONFIRMED"}),
        json!({"evidence": "r.md", "sha256": "A".repeat(64), "verdict": "CONFIRMED"}),
        json!({"evidence": "r.md", "sha256": "a".repeat(64), "verdict": "NOT CONFIRMED"}),
        json!({"evidence": "r.md", "sha256": "a".repeat(64)}),
        json!("independent check #1"),
    ] {
        assert!(
            caller::settle(&l.p, "s8-r1", &l.copy, "/x", &l.copy_sha, "d", &bad, false).is_err(),
            "{bad}"
        );
    }
    let mut ok = check(&l);
    ok["verdict"] = json!("CONFIRMED");
    assert!(
        caller::settle(
            &l.p,
            "s8-r1",
            &l.copy,
            "/x",
            &l.copy_sha,
            DECISION,
            &ok,
            true
        )
        .is_ok()
    );
    // another decision, a report that is not there or changed: refused
    assert!(
        caller::settle(
            &l.p,
            "s8-r1",
            &l.copy,
            "/x",
            &l.copy_sha,
            "d",
            &check(&l),
            true
        )
        .is_err()
    );
    let mut gone = check(&l);
    gone["evidence"] = json!(l.t.0.join("no-such-report.md"));
    assert!(
        caller::settle(
            &l.p,
            "s8-r1",
            &l.copy,
            "/x",
            &l.copy_sha,
            DECISION,
            &gone,
            true
        )
        .is_err()
    );
    let mut other = check(&l);
    other["sha256"] = json!("b".repeat(64));
    assert!(
        caller::settle(
            &l.p,
            "s8-r1",
            &l.copy,
            "/x",
            &l.copy_sha,
            DECISION,
            &other,
            true
        )
        .is_err()
    );
    assert!(
        caller::settle(
            &l.p,
            "s8-r9",
            &l.copy,
            "/x",
            &l.copy_sha,
            "d",
            &check(&l),
            false
        )
        .is_err()
    );
    // a call in flight
    std::fs::write(l.t.0.join("ledger/inflight.json"), "{}").expect("w");
    assert!(
        settle(&l, false)
            .expect_err("inflight")
            .contains("in flight")
    );
}

/// The special case is scoped (decision §2.3): only a guard stop on the old rule's warning whose
/// signals pass the current rule. A real reject, extra usage, a time or token cap, another
/// unmeasured run, another stop reason: never settled, and never released by a spare.
#[test]
fn other_stops_are_never_settled_and_never_released_by_a_spare() {
    let mut rejected: Value = serde_json::from_str(WARN.trim_end()).expect("e");
    rejected["rate_limit_info"]["status"] = json!("rejected");
    let mut overage: Value = serde_json::from_str(WARN.trim_end()).expect("e");
    overage["rate_limit_info"]["overageStatus"] = json!("allowed");
    let allowed = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","overageStatus":"rejected","isUsingOverage":false}}"#;
    let tr = transcript(Some(EXIT), 1);
    let cases: Vec<(&str, String, Patch)> = vec![
        (
            "a warning, then a real reject",
            format!("{}\n{rejected}", WARN.trim_end()),
            Box::new(|_| {}),
        ),
        (
            "a warning, then extra usage",
            format!("{}\n{overage}", WARN.trim_end()),
            Box::new(|_| {}),
        ),
        ("real reject", rejected.to_string(), Box::new(|_| {})),
        ("extra usage", overage.to_string(), Box::new(|_| {})),
        (
            "no warning (an old-rule stop on something else)",
            allowed.into(),
            Box::new(|_| {}),
        ),
        (
            "time cap",
            WARN.into(),
            Box::new(|r| r["outcome"]["stop"] = json!("Timeout")),
        ),
        (
            "token cap",
            WARN.into(),
            Box::new(|r| r["outcome"]["stop"] = json!("BillableCap")),
        ),
        (
            "cache_read cap",
            WARN.into(),
            Box::new(|r| r["outcome"]["stop"] = json!("CacheReadCap")),
        ),
        (
            "not the guard (a lost answer)",
            WARN.into(),
            Box::new(|r| r["outcome"]["stopped_by_guard"] = json!(false)),
        ),
        (
            "another stop reason",
            WARN.into(),
            Box::new(|r| {
                r["stop_reason"] = json!(format!(
                    "{}; process group cleanup failed",
                    r["stop_reason"].as_str().unwrap_or("")
                ));
            }),
        ),
        (
            "measured already",
            WARN.into(),
            Box::new(|r| r["usage"]["run"]["state"] = json!("measured")),
        ),
        (
            "a result event",
            WARN.into(),
            Box::new(|r| r["result_event"]["present"] = json!(true)),
        ),
        (
            "a child run",
            WARN.into(),
            Box::new(|r| r["children"] = json!([{"agent_id": "c1"}])),
        ),
        (
            "a spare run",
            WARN.into(),
            Box::new(|r| r["spare"] = json!({"replaces": "x"})),
        ),
        (
            "another stream",
            WARN.into(),
            Box::new(|r| r["stream"]["sha256"] = json!("00")),
        ),
    ];
    for (why, event, patch) in cases {
        let l = ledger("scope", &event, &tr, patch);
        assert!(settle(&l, false).is_err(), "{why}");
        assert_eq!(lines(&l).len(), 1, "{why}");
        let recs = caller::records(&l.p).expect("records");
        let raw = caller::raw_records(&l.p).expect("raw");
        assert!(
            caller::spare_gate(&l.p, "development", &recs, &raw, &marker()).is_err(),
            "{why}: a spare must not release it"
        );
    }
    // a guard-stopped run counted as measured without a settlement is not released either
    let mut r: Value = lines(&r1_ledger("forge"))[0].clone();
    r["usage"]["run"] = json!({"state": "measured", "tokens": {"input": 1, "output": 1, "cache_creation": 0, "cache_read": 0}});
    r["stop_reason"] = json!("no result event");
    assert!(caller::spare_releasable(&r).is_err());
    // an unsettled R1 is not released
    let l = r1_ledger("unsettled");
    let recs = caller::records(&l.p).expect("records");
    assert!(caller::spare_releasable(&recs[0]).is_err());
}

/// The CLI writer: `ieo-role-call settle` (dry run prints, a run appends; refusal exit 1).
#[test]
fn the_settle_command_writes_only_when_everything_holds() {
    let l = r1_ledger("cli");
    let plan_file = l.t.0.join("plan.json");
    std::fs::write(&plan_file, json!({"budget": "02-05", "ledger_dir": l.t.0.join("ledger"),
        "roles": {"development": {"url": "dev", "adapter_token_file": l.t.0.join("adapter.token")}}}).to_string())
        .expect("plan");
    let report = l.report.clone();
    let run = |extra: &[&str], sha: &str| {
        let mut c = std::process::Command::new(env!("CARGO_BIN_EXE_ieo-role-call"));
        c.args(["settle", "--plan"])
            .arg(&plan_file)
            .args(["--attempt", "s8-r1", "--transcript"])
            .arg(&l.copy)
            .args([
                "--original-path",
                "/role/x.jsonl",
                "--original-sha256",
                sha,
                "--decision",
                DECISION,
                "--independent-check",
            ])
            .arg(&report)
            .args(["--independent-verdict", "CONFIRMED WITH CAVEAT"])
            .env("IEO_SETTLEMENT_TEST_PINS", &l.pin)
            .args(extra);
        c.output().expect("run")
    };
    let o = run(&["--dry-run"], &l.copy_sha);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(lines(&l).len(), 1);
    let o = run(&[], &"1".repeat(64));
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(lines(&l).len(), 1);
    let o = run(&[], &l.copy_sha);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let out: Value =
        serde_json::from_slice(o.stdout.split(|b| *b == b'\n').next().unwrap_or(b"{}"))
            .expect("summary");
    assert_eq!(
        (out["settled"].clone(), out["written"].clone()),
        (json!("s8-r1"), json!(true))
    );
    assert_eq!(lines(&l).len(), 2);
}

/// PR #90 review d971f59, the scope and provenance on every read: only the decision's run
/// (L-4), the copy exactly in the ledger and never through a symlink (L-1), the copy equal to the
/// original and the report as recorded (L-2), the run's own limits (L-6), no overflow (L-7).
#[test]
fn only_the_pinned_run_with_its_evidence_in_place_is_settled() {
    // L-4: the decision's pin itself: 02-05 s8-r1, its session, its real stream
    let real = json!({"budget": "02-05", "attempt_id": "s8-r1",
        "session": {"session_id": settlement::PIN_SESSION},
        "stream": {"sha256": settlement::PIN_STREAM_SHA256}});
    assert!(settlement::pinned(&real).is_ok());
    for (k, v) in [
        ("budget", json!("02-04")),
        ("attempt_id", json!("s8-r2")),
        (
            "session",
            json!({"session_id": "00000000-0000-0000-0000-000000000000"}),
        ),
        ("stream", json!({"sha256": "0".repeat(64)})),
    ] {
        let mut r = real.clone();
        r[k] = v;
        assert!(settlement::pinned(&r).is_err(), "{k}");
    }
    // another attempt or session of the same ledger: not settled, and no spare releases it
    for (why, patch) in [
        (
            "another attempt",
            Box::new(|r: &mut Value| r["attempt_id"] = json!("s8-r9")) as Patch,
        ),
        (
            "another session",
            Box::new(|r: &mut Value| {
                r["session"]["session_id"] = json!("11111111-1111-1111-1111-111111111111")
            }),
        ),
    ] {
        let l = ledger("pin", WARN, &transcript(Some(EXIT), 1), patch);
        let a = lines(&l)[0]["attempt_id"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        assert!(
            caller::settle(
                &l.p,
                &a,
                &l.copy,
                "/x",
                &l.copy_sha,
                DECISION,
                &check(&l),
                false
            )
            .is_err(),
            "{why}"
        );
        let mut forged = lines(&l)[0].clone();
        forged["usage"]["run"] = json!({"state": "measured", "tokens": {"input": 1, "output": 1, "cache_creation": 0, "cache_read": 0}});
        forged["stop_reason"] = json!("no result event");
        forged["settlement"] = json!({"policy": settlement::POLICY});
        assert!(
            caller::spare_releasable(&forged).is_err(),
            "{why}: no spare release"
        );
    }
    // L-6: a settlement above the run's own limits is refused
    let l = ledger("limits", WARN, &transcript(Some(EXIT), 1), |r| {
        r["limits"]["billable_room"] = json!(1500)
    });
    assert!(
        settle(&l, false)
            .expect_err("limits")
            .contains("own limits")
    );
    let l = ledger("nolimits", WARN, &transcript(Some(EXIT), 1), |r| {
        r["limits"] = Value::Null
    });
    assert!(settle(&l, false).is_err());
    // L-7: token counts that overflow are never settled
    let huge = transcript(Some([u64::MAX - 1, 130, 1411, 3223]), 1).replace(
        "\"input_tokens\":2",
        &format!("\"input_tokens\":{}", u64::MAX - 1),
    );
    let acc = settlement::transcript_accounting(huge.as_bytes(), SID).expect("accounting");
    assert_eq!(acc.billable(), None);
    let l = ledger("overflow", WARN, &huge, |_| {});
    assert!(
        settle(&l, false)
            .expect_err("overflow")
            .contains("overflow")
    );
    // L-1: the copy must be exactly <ledger>/children/s8-r1/settlement-transcript.jsonl
    let l = r1_ledger("copy");
    settle(&l, false).expect("settle");
    let raw = lines(&l);
    let rec = l.t.0.join("ledger/records.jsonl");
    let copy =
        l.t.0
            .join("ledger/children/s8-r1/settlement-transcript.jsonl");
    let elsewhere = l.t.0.join("elsewhere.jsonl");
    std::fs::copy(&copy, &elsewhere).expect("copy");
    let mut s = raw[1].clone();
    s["evidence"]["ledger_copy"] = json!(elsewhere);
    std::fs::write(&rec, format!("{}\n{s}\n", raw[0])).expect("w");
    assert!(
        caller::records(&l.p)
            .expect_err("elsewhere")
            .contains("lives in the ledger")
    );
    std::fs::write(&rec, format!("{}\n{}\n", raw[0], raw[1])).expect("w");
    assert!(caller::records(&l.p).is_ok());
    std::fs::remove_file(&copy).expect("rm");
    std::os::unix::fs::symlink(&elsewhere, &copy).expect("symlink");
    assert!(
        caller::records(&l.p)
            .expect_err("symlink")
            .contains("symlink")
    );
    std::fs::remove_file(&copy).expect("rm");
    std::fs::copy(&elsewhere, &copy).expect("copy back");
    assert!(caller::records(&l.p).is_ok());
    // L-2: the copy's two digests must agree; the report must be the recorded one
    let mut s = raw[1].clone();
    s["evidence"]["original_sha256"] = json!("0".repeat(64));
    std::fs::write(&rec, format!("{}\n{s}\n", raw[0])).expect("w");
    assert!(
        caller::records(&l.p)
            .expect_err("orig")
            .contains("not the original")
    );
    std::fs::write(&rec, format!("{}\n{}\n", raw[0], raw[1])).expect("w");
    // N-1: the report is checked in the ledger; its original may change or go away
    std::fs::write(&l.report, "# changed\n").expect("w");
    std::fs::remove_file(&l.report).expect("rm");
    assert!(
        caller::records(&l.p).is_ok(),
        "the original report is provenance only"
    );
    let rcopy = l.t.0.join("ledger/children/s8-r1/independent-check.md");
    assert_eq!(
        raw[1]["independent_check"]["ledger_copy"],
        json!(std::fs::canonicalize(&rcopy).expect("c"))
    );
    let kept = std::fs::read(&rcopy).expect("report copy");
    std::fs::write(&rcopy, "# changed\n").expect("w");
    assert!(
        caller::records(&l.p)
            .expect_err("report")
            .contains("SHA-256")
    );
    std::fs::remove_file(&rcopy).expect("rm");
    assert!(caller::records(&l.p).expect_err("gone").contains("missing"));
    std::fs::write(&rcopy, &kept).expect("w");
    assert!(caller::records(&l.p).is_ok());
    // the writer never writes through a symlink at the copy path
    let l = r1_ledger("wsym");
    std::fs::create_dir_all(l.t.0.join("ledger/children/s8-r1")).expect("dir");
    std::os::unix::fs::symlink(
        l.t.0.join("target.jsonl"),
        l.t.0
            .join("ledger/children/s8-r1/settlement-transcript.jsonl"),
    )
    .expect("symlink");
    assert!(settle(&l, false).is_err());
    assert!(
        !l.t.0.join("target.jsonl").exists(),
        "nothing written through the symlink"
    );
    assert_eq!(lines(&l).len(), 1);
    // the kept stream file changed after the record was written (the record's digest still
    // matches the pin): refused
    let l = r1_ledger("streamfile");
    let sp = l.t.0.join("ledger/streams/s8-r1.jsonl");
    let mut text = std::fs::read_to_string(&sp).expect("stream");
    text.push_str("{\"type\":\"user\"}\n");
    std::fs::write(&sp, text).expect("w");
    assert!(
        settle(&l, false)
            .expect_err("stream")
            .contains("kept stream differs")
    );
    // L-3: a dry run creates no copy anywhere in the ledger
    let l = r1_ledger("dry");
    settle(&l, true).expect("dry");
    assert!(!l.t.0.join("ledger/children").exists());
}

/// PR #90 review 7b904de N-2/N-3: the ledger directory is taken by its canonical path (the CLI
/// and the coordinator agree however their plans spell it), and nothing inside the ledger may be
/// a symlink (`children`, `children/<attempt>`, the files).
#[test]
fn the_ledger_is_one_place_however_it_is_spelled_and_has_no_symlink_inside() {
    // settled through an alias of the ledger directory, read through the real path, and back
    let l = r1_ledger("alias");
    let alias = l.t.0.join("ledger-alias");
    std::os::unix::fs::symlink(l.t.0.join("ledger"), &alias).expect("alias");
    let p_alias = caller::Plan::from_json(&json!({"budget": "02-05", "ledger_dir": alias,
        "roles": {"development": {"url": "dev", "adapter_token_file": l.t.0.join("adapter.token")}}}))
    .expect("plan");
    let p_dots = caller::Plan::from_json(&json!({"budget": "02-05",
        "ledger_dir": l.t.0.join("ledger/../ledger"),
        "roles": {"development": {"url": "dev", "adapter_token_file": l.t.0.join("adapter.token")}}}))
    .expect("plan");
    caller::settle(
        &p_alias,
        "s8-r1",
        &l.copy,
        "/x",
        &l.copy_sha,
        DECISION,
        &check(&l),
        false,
    )
    .expect("settle through the alias");
    for (why, p) in [("real path", &l.p), ("alias", &p_alias), ("dots", &p_dots)] {
        let r = caller::records(p).expect(why);
        assert_eq!(r[0]["settlement"]["policy"], settlement::POLICY, "{why}");
    }
    // N-3: children/s8-r1 moved out of the ledger and linked back: refused
    let attempt_dir = l.t.0.join("ledger/children/s8-r1");
    let outside = l.t.0.join("outside-s8-r1");
    std::fs::rename(&attempt_dir, &outside).expect("mv");
    std::os::unix::fs::symlink(&outside, &attempt_dir).expect("link");
    assert!(
        caller::records(&l.p)
            .expect_err("attempt dir")
            .contains("symlink")
    );
    std::fs::remove_file(&attempt_dir).expect("rm");
    std::fs::rename(&outside, &attempt_dir).expect("mv back");
    assert!(caller::records(&l.p).is_ok());
    // children itself linked from outside: refused
    let children = l.t.0.join("ledger/children");
    let outside = l.t.0.join("outside-children");
    std::fs::rename(&children, &outside).expect("mv");
    std::os::unix::fs::symlink(&outside, &children).expect("link");
    assert!(
        caller::records(&l.p)
            .expect_err("children")
            .contains("symlink")
    );
    std::fs::remove_file(&children).expect("rm");
    std::fs::rename(&outside, &children).expect("mv back");
    assert!(caller::records(&l.p).is_ok());
    // the writer refuses a symlinked children/<attempt> too, and writes nothing through it
    let l = r1_ledger("wdir");
    let outside = l.t.0.join("elsewhere");
    std::fs::create_dir_all(&outside).expect("dir");
    std::fs::create_dir_all(l.t.0.join("ledger/children")).expect("dir");
    std::os::unix::fs::symlink(&outside, l.t.0.join("ledger/children/s8-r1")).expect("link");
    assert!(settle(&l, false).is_err());
    assert_eq!(std::fs::read_dir(&outside).expect("ls").count(), 0);
    assert_eq!(lines(&l).len(), 1);
}

/// `ieo-role-call records`: the effective records (the settled run as it counts), read only;
/// exit 2 while a call is in flight.
#[test]
fn the_records_command_prints_the_settled_run() {
    let l = r1_ledger("records");
    let plan_file = l.t.0.join("plan.json");
    std::fs::write(
        &plan_file,
        json!({"budget": "02-05", "ledger_dir": l.t.0.join("ledger"),
        "roles": {"development": {"url": "dev", "adapter_token_file": l.t.0.join("adapter.token")}}})
        .to_string(),
    )
    .expect("plan");
    let records = || {
        std::process::Command::new(env!("CARGO_BIN_EXE_ieo-role-call"))
            .args(["records", "--plan"])
            .arg(&plan_file)
            .env("IEO_SETTLEMENT_TEST_PINS", &l.pin)
            .output()
            .expect("run")
    };
    let first = |o: &std::process::Output| -> Value {
        serde_json::from_slice(o.stdout.split(|b| *b == b'\n').next().unwrap_or(b"{}"))
            .expect("record")
    };
    let o = records();
    assert!(o.status.success());
    assert_eq!(first(&o)["usage"]["run"]["state"], "unmeasured");
    settle(&l, false).expect("settle");
    let o = records();
    assert!(o.status.success());
    let out = String::from_utf8_lossy(&o.stdout);
    assert_eq!(
        out.lines().count(),
        1,
        "the settlement line is no record of its own"
    );
    let r = first(&o);
    assert_eq!(r["usage"]["run"]["state"], "measured");
    assert_eq!(r["settlement"]["policy"], settlement::POLICY);
    std::fs::write(l.t.0.join("ledger/inflight.json"), "{}").expect("w");
    assert_eq!(records().status.code(), Some(2));
}

/// PR #90 review 758ac03 O-1: the writer checks `children` and `children/<attempt>` before it
/// creates anything; a refused settlement leaves no directory, in the ledger or through a
/// symlink outside it.
#[test]
fn a_refused_settlement_creates_no_directory_anywhere() {
    // a symlinked children: refused before anything is created, nothing appears outside
    let l = r1_ledger("o1-children");
    let outside = l.t.0.join("outside-children");
    std::fs::create_dir_all(&outside).expect("dir");
    std::os::unix::fs::symlink(&outside, l.t.0.join("ledger/children")).expect("link");
    assert!(
        settle(&l, false)
            .expect_err("children")
            .contains("nothing created")
    );
    assert_eq!(
        std::fs::read_dir(&outside).expect("ls").count(),
        0,
        "no s8-r1 created outside"
    );
    assert_eq!(lines(&l).len(), 1);
    // a refusal after the directories were made (the report copy is a different file): both
    // the created copy and the created directories are removed again
    let l = r1_ledger("o1-cleanup");
    std::fs::create_dir_all(l.t.0.join("ledger/children/s8-r1")).expect("dir");
    std::fs::write(
        l.t.0.join("ledger/children/s8-r1/independent-check.md"),
        "other",
    )
    .expect("w");
    assert!(settle(&l, false).is_err());
    assert!(
        !l.t.0
            .join("ledger/children/s8-r1/settlement-transcript.jsonl")
            .exists()
    );
    assert!(
        l.t.0.join("ledger/children/s8-r1").exists(),
        "directories it did not create stay"
    );
    std::fs::remove_dir_all(l.t.0.join("ledger/children")).expect("rm");
    let l2 = ledger("o1-late", WARN, &transcript(Some(EXIT), 1), |r| {
        r["limits"]["billable_room"] = json!(1500)
    });
    assert!(settle(&l2, false).is_err());
    assert!(
        !l2.t.0.join("ledger/children").exists(),
        "created directories removed"
    );
}
