//! 02-01 behaviour on fixed inputs and real local processes (no model calls).
//! Model streams are fixed files: a real 01-04 stream (attempt 23, child agent)
//! and hand-made variants (missing result, missing usage, truncated, is_error).

use ieo_evidence::aggregate::summarize;
use ieo_evidence::record::{
    Applicability, EvidenceRecord, Expected, JudgmentStatus, Measure, ProcessKind, SessionMode,
    Tokens,
};
use ieo_evidence::run::{RunSpec, execute};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

const T23: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/t23-child.jsonl"
);
static N: AtomicU32 = AtomicU32::new(0);

fn tmp() -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "ieo-evidence-test-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn spec(out: &Path, cmd: &str) -> RunSpec {
    RunSpec {
        task_id: "02-01".into(),
        attempt_id: "a1".into(),
        command_id: "c1".into(),
        role: "development".into(),
        retry_of: None,
        argv: vec!["sh".into(), "-c".into(), cmd.into()],
        cwd: out.to_path_buf(),
        env_allow: vec!["PATH".into()],
        timeout: Some(Duration::from_secs(20)),
        grace: Duration::from_secs(1),
        model_request: false,
        session_mode: SessionMode::New,
        resume_base: None,
        required_artifacts: vec![],
        required_markers: vec![],
        expected: Expected::Success,
        out_root: out.join("ev"),
        secrets: vec![],
        child_transcripts: Default::default(),
    }
}

/// The 01-04 T23 child made one model response; its transcript (not kept by 01-04) is rebuilt
/// with that one final response, which equals the reported `tool_use_result.usage`.
const T23_AGENT: &str = "af06f3de7793d2534";
fn t23_transcript(out: &Path) -> PathBuf {
    fixture(
        out,
        "agent-t23.jsonl",
        &format!(
            "{}\n",
            serde_json::json!({"type": "assistant", "isSidechain": true, "agentId": T23_AGENT,
                "message": {"id": "msg_t23_child_1", "stop_reason": "end_turn",
                    "usage": {"input_tokens": 2, "output_tokens": 9,
                              "cache_creation_input_tokens": 844, "cache_read_input_tokens": 0}}})
        ),
    )
}

fn with_t23_child(out: &Path, mut s: RunSpec) -> RunSpec {
    s.child_transcripts
        .insert(T23_AGENT.into(), t23_transcript(out));
    s
}

fn fixture(out: &Path, name: &str, text: &str) -> PathBuf {
    let p = out.join(name);
    std::fs::write(&p, text).unwrap();
    p
}

fn real_stream() -> String {
    std::fs::read_to_string(T23).unwrap()
}

fn tokens(m: &Measure) -> Tokens {
    m.tokens().cloned().expect("measured")
}

fn stored(out: &Path, s: &RunSpec) -> EvidenceRecord {
    let d = ieo_evidence::run::evidence_dir(&s.out_root, &s.task_id, &s.attempt_id, &s.command_id);
    let _ = out;
    serde_json::from_str(&std::fs::read_to_string(d.join("record.json")).unwrap()).unwrap()
}

#[test]
fn normal_success_records_everything() {
    let out = tmp();
    let mut s = spec(&out, "echo RESULT-OK; echo warn >&2; printf data > art.txt");
    s.required_markers = vec!["^RESULT-OK$".into()];
    s.required_artifacts = vec!["art.txt".into()];
    let r = execute(&s).unwrap();
    assert_eq!(r.judgment.status, JudgmentStatus::Succeeded);
    assert!(r.judgment.counts_as_success);
    assert_eq!(r.process.exit_code, Some(0));
    assert_eq!(r.usage.applicability, Applicability::NotApplicable);
    assert!(r.result_event.is_none());
    // stdout/stderr kept and hashed; artifact hashed
    let d = out.join("ev/02-01/a1/c1");
    let so = std::fs::read(d.join("stdout")).unwrap();
    assert_eq!(so, b"RESULT-OK\n");
    assert_eq!(r.stdout.sha256, hex_sha(&so));
    assert_eq!(std::fs::read(d.join("stderr")).unwrap(), b"warn\n");
    assert_eq!(
        r.artifacts[0].sha256.as_deref(),
        Some(hex_sha(b"data").as_str())
    );
    assert_eq!(stored(&out, &s), r);
}

fn hex_sha(b: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(b))
}

#[test]
fn nonzero_exit_is_failure_with_stderr_kept() {
    let out = tmp();
    let r = execute(&spec(&out, "echo boom >&2; exit 3")).unwrap();
    assert_eq!(r.judgment.status, JudgmentStatus::Failed);
    assert!(!r.judgment.counts_as_success);
    assert!(r.judgment.reasons.iter().any(|x| x.contains("Some(3)")));
    assert_eq!(r.stderr.bytes, 5);
}

#[test]
fn launch_failure_still_writes_record() {
    let out = tmp();
    let mut s = spec(&out, "");
    s.argv = vec!["/nonexistent/ieo-no-such-binary".into()];
    let r = execute(&s).unwrap();
    assert_eq!(r.process.kind, ProcessKind::LaunchFailed);
    assert_eq!(r.judgment.status, JudgmentStatus::Failed);
    assert!(r.process.launch_error.is_some());
    assert_eq!(stored(&out, &s), r);
}

#[test]
fn timeout_sends_term_and_is_failure() {
    let out = tmp();
    let mut s = spec(&out, "echo started; sleep 30");
    s.timeout = Some(Duration::from_secs(1));
    let r = execute(&s).unwrap();
    assert_eq!(r.process.kind, ProcessKind::TimedOut);
    assert!(r.process.term_sent);
    assert!(!r.process.kill_sent, "sh dies on TERM");
    assert_eq!(r.judgment.status, JudgmentStatus::Failed);
    assert!(r.duration_ms < 5_000, "{}", r.duration_ms);
    // partial output kept
    assert_eq!(r.stdout.bytes, 8);
}

#[test]
fn timeout_escalates_to_kill_when_term_ignored() {
    let out = tmp();
    let mut s = spec(&out, "trap '' TERM; sleep 30 & wait; sleep 30");
    s.timeout = Some(Duration::from_secs(1));
    s.grace = Duration::from_secs(1);
    let r = execute(&s).unwrap();
    assert_eq!(r.process.kind, ProcessKind::TimedOut);
    assert!(r.process.term_sent && r.process.kill_sent);
    assert!(r.duration_ms < 8_000, "{}", r.duration_ms);
}

#[test]
fn intentional_abort_judged_by_expected_outcome() {
    let out = tmp();
    let mut s = spec(&out, "sleep 30");
    s.timeout = Some(Duration::from_secs(1));
    s.expected = Expected::Timeout;
    let r = execute(&s).unwrap();
    assert_eq!(r.judgment.status, JudgmentStatus::ExpectedMatched);
    assert!(r.judgment.counts_as_success);
    // the same expectation on a run that finishes is not matched
    let mut s2 = spec(&out, "true");
    s2.command_id = "c2".into();
    s2.expected = Expected::Timeout;
    assert_eq!(
        execute(&s2).unwrap().judgment.status,
        JudgmentStatus::ExpectedNotMatched
    );
}

#[test]
fn deny_test_needs_nonzero_exit_and_its_marker() {
    let out = tmp();
    let mut s = spec(&out, "echo WRITE_DENIED; exit 1");
    s.expected = Expected::NonzeroExit;
    s.required_markers = vec!["WRITE_DENIED".into()];
    assert_eq!(
        execute(&s).unwrap().judgment.status,
        JudgmentStatus::ExpectedMatched
    );
    // denied as expected but without the marker: the cause is not shown -> not matched
    let mut s2 = spec(&out, "exit 1");
    s2.command_id = "c2".into();
    s2.expected = Expected::NonzeroExit;
    s2.required_markers = vec!["WRITE_DENIED".into()];
    assert_eq!(
        execute(&s2).unwrap().judgment.status,
        JudgmentStatus::ExpectedNotMatched
    );
    // the write succeeded (exit 0): not matched
    let mut s3 = spec(&out, "echo WRITE_OK");
    s3.command_id = "c3".into();
    s3.expected = Expected::NonzeroExit;
    s3.required_markers = vec!["WRITE_DENIED".into()];
    assert_eq!(
        execute(&s3).unwrap().judgment.status,
        JudgmentStatus::ExpectedNotMatched
    );
}

#[test]
fn exit_zero_without_required_evidence_is_incomplete() {
    let out = tmp();
    let mut s = spec(&out, "echo hello");
    s.required_artifacts = vec!["missing.txt".into()];
    s.required_markers = vec!["^DONE$".into()];
    let r = execute(&s).unwrap();
    assert_eq!(r.process.exit_code, Some(0));
    assert_eq!(r.judgment.status, JudgmentStatus::Incomplete);
    assert!(!r.judgment.counts_as_success);
    assert_eq!(r.judgment.reasons.len(), 2);
}

#[test]
fn model_run_real_stream_measures_parent_child_session() {
    let out = tmp();
    let f = fixture(&out, "t23.jsonl", &real_stream());
    let mut s = with_t23_child(&out, spec(&out, &format!("cat {}", f.display())));
    s.model_request = true;
    let r = execute(&s).unwrap();
    assert_eq!(r.judgment.status, JudgmentStatus::Succeeded);
    let re = r.result_event.as_ref().unwrap();
    assert!(re.present);
    assert_eq!(re.num_turns, Some(2));
    assert_eq!(re.runtime_version.as_deref(), Some("2.1.322"));
    let session = tokens(&r.usage.session);
    let parent = tokens(&r.usage.parent);
    assert_eq!(
        session,
        Tokens {
            input: 6,
            output: 166,
            cache_read: 7907,
            cache_creation: 3730
        }
    );
    assert_eq!(
        parent,
        Tokens {
            input: 4,
            output: 157,
            cache_read: 7907,
            cache_creation: 2886
        }
    );
    assert_eq!(r.usage.children.len(), 1);
    assert_eq!(
        tokens(&r.usage.children[0].usage),
        Tokens {
            input: 2,
            output: 9,
            cache_read: 0,
            cache_creation: 844
        }
    );
    assert_eq!(
        r.usage.children[0].agent_id.as_deref(),
        Some("af06f3de7793d2534")
    );
    assert_eq!(r.usage.attribution_check, "consistent");
    assert!(r.usage.cost_usd_reported.is_some());
    assert_eq!(r.usage.internal_retries, "unmeasured");
}

#[test]
fn model_run_exit_zero_without_result_event_is_incomplete() {
    let out = tmp();
    // first 6 lines of the real stream: the run ended before its result event
    let truncated: String = real_stream()
        .lines()
        .take(6)
        .map(|l| format!("{l}\n"))
        .collect();
    let f = fixture(&out, "trunc.jsonl", &truncated);
    let mut s = spec(&out, &format!("cat {}", f.display()));
    s.model_request = true;
    let r = execute(&s).unwrap();
    assert_eq!(r.process.exit_code, Some(0));
    assert_eq!(r.judgment.status, JudgmentStatus::Incomplete);
    assert!(r.judgment.reasons.iter().any(|x| x.contains("결과 사건")));
    assert!(matches!(r.usage.session, Measure::Unmeasured { .. }));
    assert!(matches!(r.usage.parent, Measure::Unmeasured { .. }));
    // the started child never returned: unmeasured, not 0
    assert!(matches!(
        r.usage.children[0].usage,
        Measure::Unmeasured { .. }
    ));
    // streaming snapshots kept for the record, never summed
    assert!(!r.usage.partial_observations.is_empty());
    let sum = summarize(&[r]);
    assert_eq!(sum.usage.completeness, "incomplete");
    assert_eq!(sum.usage.measured_runs, 0);
    assert_eq!(sum.usage.unmeasured.len(), 1);
}

#[test]
fn intentional_model_abort_expects_no_result() {
    let out = tmp();
    let truncated: String = real_stream()
        .lines()
        .take(2)
        .map(|l| format!("{l}\n"))
        .collect();
    let f = fixture(&out, "abort.jsonl", &truncated);
    let mut s = spec(&out, &format!("cat {}; sleep 30", f.display()));
    s.model_request = true;
    s.expected = Expected::NoResult;
    s.timeout = Some(Duration::from_secs(1));
    let r = execute(&s).unwrap();
    assert_eq!(r.judgment.status, JudgmentStatus::ExpectedMatched);
    // usage of the aborted run is still unmeasured, not 0, and counted in totals
    assert!(matches!(r.usage.session, Measure::Unmeasured { .. }));
    assert_eq!(summarize(&[r]).usage.completeness, "incomplete");
}

#[test]
fn result_without_usage_fields_is_unmeasured() {
    let out = tmp();
    let f = fixture(
        &out,
        "nousage.jsonl",
        "{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"s\"}\n{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"num_turns\":1,\"session_id\":\"s\"}\nnot json\n",
    );
    let mut s = spec(&out, &format!("cat {}", f.display()));
    s.model_request = true;
    let r = execute(&s).unwrap();
    // the command itself succeeded; its usage is unmeasured
    assert_eq!(r.judgment.status, JudgmentStatus::Succeeded);
    assert!(
        matches!(&r.usage.session, Measure::Unmeasured { reason } if reason.contains("modelUsage"))
    );
    assert!(matches!(&r.usage.parent, Measure::Unmeasured { reason } if reason.contains("usage")));
    assert_eq!(r.result_event.as_ref().unwrap().non_json_lines, 1);
    let sum = summarize(&[r]);
    assert_eq!(sum.usage.completeness, "incomplete");
    assert_eq!(
        sum.usage.tokens_measured_sum, None,
        "nothing measured: no sum, not 0"
    );
    assert_eq!(sum.usage.measured_runs, 0);
}

#[test]
fn model_usage_missing_falls_back_to_parent_plus_children() {
    let out = tmp();
    let text: String = real_stream()
        .lines()
        .map(|l| {
            let mut v: serde_json::Value = serde_json::from_str(l).unwrap();
            if v["type"] == "result" {
                v.as_object_mut().unwrap().remove("modelUsage");
            }
            format!("{v}\n")
        })
        .collect();
    let f = fixture(&out, "nomodel.jsonl", &text);
    let mut s = with_t23_child(&out, spec(&out, &format!("cat {}", f.display())));
    s.model_request = true;
    let r = execute(&s).unwrap();
    let Measure::Measured { tokens, source } = &r.usage.session else {
        panic!()
    };
    assert_eq!(
        *tokens,
        Tokens {
            input: 6,
            output: 166,
            cache_read: 7907,
            cache_creation: 3730
        }
    );
    assert!(source.contains("result.usage"));
    assert_eq!(r.usage.attribution_check, "not_checked");
}

#[test]
fn attribution_mismatch_is_reported() {
    let out = tmp();
    let text: String = real_stream()
        .lines()
        .map(|l| {
            let mut v: serde_json::Value = serde_json::from_str(l).unwrap();
            if v["type"] == "result" {
                v["usage"]["output_tokens"] = 100.into();
            }
            format!("{v}\n")
        })
        .collect();
    let f = fixture(&out, "mismatch.jsonl", &text);
    let mut s = with_t23_child(&out, spec(&out, &format!("cat {}", f.display())));
    s.model_request = true;
    let r = execute(&s).unwrap();
    assert_eq!(r.usage.attribution_check, "mismatch");
    assert_eq!(summarize(&[r]).usage.attribution_mismatches.len(), 1);
}

#[test]
fn result_is_error_is_failure_unless_expected() {
    let out = tmp();
    let text = real_stream().replace("\"is_error\":false", "\"is_error\":true");
    let f = fixture(&out, "err.jsonl", &text);
    let mut s = spec(&out, &format!("cat {}", f.display()));
    s.model_request = true;
    assert_eq!(execute(&s).unwrap().judgment.status, JudgmentStatus::Failed);
    s.command_id = "c2".into();
    s.expected = Expected::ResultError;
    assert_eq!(
        execute(&s).unwrap().judgment.status,
        JudgmentStatus::ExpectedMatched
    );
}

#[test]
fn aggregate_includes_children_failures_retries_and_reviews() {
    let out = tmp();
    let f = fixture(&out, "t23.jsonl", &real_stream());
    let mut recs = vec![];
    // development run that succeeded
    let mut s = with_t23_child(&out, spec(&out, &format!("cat {}", f.display())));
    s.model_request = true;
    recs.push(execute(&s).unwrap());
    // the same stream from a run that then failed (exit 2): usage still counted
    let mut s = with_t23_child(&out, spec(&out, &format!("cat {}; exit 2", f.display())));
    s.model_request = true;
    s.attempt_id = "a2".into();
    let failed = execute(&s).unwrap();
    assert_eq!(failed.judgment.status, JudgmentStatus::Failed);
    recs.push(failed);
    // its retry
    let mut s = with_t23_child(&out, spec(&out, &format!("cat {}", f.display())));
    s.model_request = true;
    s.attempt_id = "a3".into();
    s.retry_of = Some("a2".into());
    recs.push(execute(&s).unwrap());
    // a review run
    let mut s = with_t23_child(&out, spec(&out, &format!("cat {}", f.display())));
    s.model_request = true;
    s.attempt_id = "r1".into();
    s.role = "review".into();
    recs.push(execute(&s).unwrap());
    // a plain command: not a model run, not unmeasured
    let mut s = spec(&out, "true");
    s.attempt_id = "p1".into();
    recs.push(execute(&s).unwrap());
    let sum = summarize(&recs);
    assert_eq!(sum.records, 5);
    assert_eq!(sum.retries, 1);
    assert_eq!(sum.usage.model_runs, 4);
    assert_eq!(sum.usage.measured_runs, 4);
    assert_eq!(sum.usage.completeness, "complete");
    assert_eq!(sum.usage.children_seen, 4);
    assert_eq!(sum.usage.children_measured, 4);
    assert_eq!(
        sum.usage.tokens_measured_sum.unwrap(),
        Tokens {
            input: 24,
            output: 664,
            cache_read: 31628,
            cache_creation: 14920
        }
    );
    assert_eq!(sum.usage_by_role["review"].measured_runs, 1);
    assert_eq!(sum.usage_by_role["development"].model_runs, 3);
    assert_eq!(sum.by_judgment["failed"], 1);
    assert!(sum.usage.cost_usd_reported_sum.is_some());
}

#[test]
fn secrets_never_reach_evidence_files() {
    let out = tmp();
    let secret = "s3cr3t-token-value-0123456789";
    let sf = fixture(&out, "secret.txt", &format!("{secret}\n"));
    let mut s = spec(
        &out,
        &format!("echo {secret}; echo {secret} >&2; echo sk-ant-oat01-AAAAAAAAAAAAAAAA"),
    );
    s.argv.push(secret.into());
    s.secrets = vec![std::fs::read(&sf).unwrap()];
    let r = execute(&s).unwrap();
    assert!(r.redaction.secret_value_seen);
    assert_eq!(r.redaction.stdout_replacements, 2);
    assert!(r.redaction.stderr_replacements >= 1);
    assert!(r.redaction.argv_replacements >= 2);
    let d = out.join("ev/02-01/a1/c1");
    for f in ["stdout", "stderr", "record.json"] {
        let text = std::fs::read_to_string(d.join(f)).unwrap();
        assert!(!text.contains(secret), "{f}");
        assert!(!text.contains("AAAAAAAAAAAAAAAA"), "{f}");
    }
}

#[test]
fn environment_is_cleared_to_the_allow_list() {
    let out = tmp();
    let mut s = spec(&out, "env | cut -d= -f1 | sort");
    s.env_allow = vec!["PATH".into(), "IEO_SURELY_UNSET_VAR".into()];
    let r = execute(&s).unwrap();
    let d = out.join("ev/02-01/a1/c1");
    let names = std::fs::read_to_string(d.join("stdout")).unwrap();
    assert!(!names.contains("HOME\n"), "{names}");
    assert!(!names.contains("CARGO"), "{names}");
    // only names actually passed are recorded, never values
    assert_eq!(r.env_names, vec!["PATH".to_owned()]);
    let path_value = std::env::var("PATH").unwrap();
    let rec = std::fs::read_to_string(d.join("record.json")).unwrap();
    assert!(!rec.contains(&path_value));
}

#[test]
fn evidence_is_never_overwritten_and_ids_are_checked() {
    let out = tmp();
    let s = spec(&out, "true");
    execute(&s).unwrap();
    assert!(execute(&s).is_err());
    let mut bad = spec(&out, "true");
    bad.attempt_id = "../x".into();
    assert!(execute(&bad).is_err());
    let mut bad = spec(&out, "true");
    bad.role = "admin".into();
    assert!(execute(&bad).is_err());
}

#[test]
fn stray_background_process_does_not_hang_the_collector() {
    let out = tmp();
    // `sleep` inherits stdout: it still holds the pipe when killed after the drain
    // window, so the stored output may be cut -> incomplete, never success.
    let r = execute(&spec(&out, "sleep 30 & echo x")).unwrap();
    assert!(r.process.stray_group_killed);
    assert!(r.process.output_incomplete);
    assert!(r.duration_ms < 8_000, "{}", r.duration_ms);
    assert_eq!(r.judgment.status, JudgmentStatus::Incomplete);
}

#[test]
fn every_record_matches_the_schema() {
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../contracts/evidence-record-v0.schema.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let v = jsonschema::validator_for(&schema).unwrap();
    let out = tmp();
    let f = fixture(&out, "t23.jsonl", &real_stream());
    let trunc: String = real_stream()
        .lines()
        .take(6)
        .map(|l| format!("{l}\n"))
        .collect();
    let g = fixture(&out, "tr.jsonl", &trunc);
    let cases: Vec<(&str, bool, Option<u64>)> = vec![
        ("echo ok", false, None),
        ("exit 4", false, None),
        ("sleep 30", false, Some(1)),
        (
            &*Box::leak(format!("cat {}", f.display()).into_boxed_str()),
            true,
            None,
        ),
        (
            &*Box::leak(format!("cat {}", g.display()).into_boxed_str()),
            true,
            None,
        ),
    ];
    for (i, (cmd, model, to)) in cases.into_iter().enumerate() {
        let mut s = spec(&out, cmd);
        s.command_id = format!("c{i}");
        s.model_request = model;
        s.timeout = to.map(Duration::from_secs);
        let r = execute(&s).unwrap();
        let j = serde_json::to_value(&r).unwrap();
        let errs: Vec<String> = v.iter_errors(&j).map(|e| e.to_string()).collect();
        assert!(errs.is_empty(), "{cmd}: {errs:?}");
    }
    // a hand-forged record claiming success with exit 1 is rejected by the schema
    let mut s = spec(&out, "true");
    s.command_id = "forge".into();
    let mut j = serde_json::to_value(execute(&s).unwrap()).unwrap();
    j["process"]["exit_code"] = 1.into();
    assert!(!v.is_valid(&j));
    // unmeasured cannot carry tokens; measured cannot omit them
    let mut j2 = j.clone();
    j2["process"]["exit_code"] = 0.into();
    j2["usage"]["session"] = serde_json::json!({"state": "unmeasured", "reason": "x", "tokens": {"input": 0, "output": 0, "cache_read": 0, "cache_creation": 0}});
    assert!(!v.is_valid(&j2));
}

/// Real 01-04 resume chain R1 (new) → R2 (resume): the CLI's modelUsage in R2
/// is cumulative over R1 and R2; the run share must be R2's own usage.
const R1: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/r1-new.jsonl");
const R2: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/r2-resume.jsonl"
);

fn replay(
    out: &Path,
    attempt: &str,
    file: &str,
    mode: SessionMode,
    base: Option<PathBuf>,
) -> EvidenceRecord {
    let mut s = spec(out, &format!("cat {file}"));
    s.attempt_id = attempt.into();
    s.model_request = true;
    s.session_mode = mode;
    s.resume_base = base;
    execute(&s).unwrap()
}

#[test]
fn resumed_session_usage_is_split_from_the_cumulative_total() {
    let out = tmp();
    let r1 = replay(&out, "r1", R1, SessionMode::New, None);
    assert_eq!(
        tokens(&r1.usage.run),
        Tokens {
            input: 2,
            output: 612,
            cache_read: 649,
            cache_creation: 1800
        }
    );
    let base = out.join("ev/02-01/r1/c1/record.json");
    let r2 = replay(&out, "r2", R2, SessionMode::Resume, Some(base.clone()));
    // session totals as reported: cumulative R1 + R2
    assert_eq!(
        tokens(&r2.usage.session),
        Tokens {
            input: 6,
            output: 626,
            cache_read: 3098,
            cache_creation: 2386
        }
    );
    // run share: R2 alone, equal to its own result.usage (no children, no compaction)
    assert_eq!(
        tokens(&r2.usage.run),
        Tokens {
            input: 4,
            output: 14,
            cache_read: 2449,
            cache_creation: 586
        }
    );
    assert_eq!(r2.usage.attribution_check, "consistent");
    assert!(r2.usage.run_cost_usd.unwrap() < r2.usage.cost_usd_reported.unwrap());
    // the sum over both runs counts R1 once
    let sum = summarize(&[r1, r2.clone()]);
    assert_eq!(
        sum.usage.tokens_measured_sum.unwrap(),
        Tokens {
            input: 6,
            output: 626,
            cache_read: 3098,
            cache_creation: 2386
        }
    );
    assert_eq!(sum.usage.completeness, "complete");

    // resume without a base: unmeasured, never the cumulative value
    let r3 = replay(&out, "r3", R2, SessionMode::Resume, None);
    assert!(
        matches!(&r3.usage.run, Measure::Unmeasured { reason } if reason.contains("기준 회차"))
    );
    assert_eq!(summarize(&[r3]).usage.completeness, "incomplete");
    // base of another session: unmeasured
    let other = out.join("ev/02-01/a1/c1/record.json");
    let mut s = spec(&out, &format!("cat {T23}"));
    s.model_request = true;
    execute(&s).unwrap();
    let r4 = replay(&out, "r4", R2, SessionMode::Resume, Some(other));
    assert!(
        matches!(&r4.usage.run, Measure::Unmeasured { reason } if reason.contains("session_id"))
    );
    // a base that is not the run actually resumed (R2 as base of the earlier R1 stream, same
    // session id): totals would decrease -> unmeasured, never negative or clamped to 0
    let r5 = replay(
        &out,
        "r5",
        R1,
        SessionMode::Resume,
        Some(out.join("ev/02-01/r2/c1/record.json")),
    );
    assert!(
        matches!(&r5.usage.run, Measure::Unmeasured { reason } if reason.contains("기준 회차보다 작음"))
    );
}

#[test]
fn model_request_must_state_session_mode() {
    let out = tmp();
    let mut s = spec(&out, &format!("cat {R1}"));
    s.model_request = true;
    s.session_mode = SessionMode::Unknown;
    assert!(execute(&s).is_err());
    let mut s = spec(&out, &format!("cat {R1}"));
    s.model_request = true;
    s.resume_base = Some(out.join("x.json"));
    assert!(execute(&s).is_err(), "new session with a base");
}

fn fx(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// Real 01-04 chain after a deliberate concurrent resume (N4a ∥ N4b), then L2 (resume)
/// and L3 (/compact). L2 actually continued N4a's branch; with N4b as base its share is
/// not its own usage and must be withdrawn. L3's compaction usage appears only in the
/// session totals, so its share is measured and the attribution mismatch is reported.
#[test]
fn wrong_resume_base_is_withdrawn_and_compaction_is_counted() {
    let out = tmp();
    // treat N4a as a base snapshot of the session (its session totals are measured)
    let n4a = replay(
        &out,
        "n4a",
        &fx("n4a-resume.jsonl"),
        SessionMode::Resume,
        None,
    );
    assert!(n4a.usage.session.tokens().is_some());
    let n4b = replay(
        &out,
        "n4b",
        &fx("n4b-resume-concurrent.jsonl"),
        SessionMode::Resume,
        Some(out.join("ev/02-01/n4a/c1/record.json")),
    );
    assert!(
        matches!(&n4b.usage.run, Measure::Unmeasured { reason } if reason.contains("기준 회차보다 작음"))
    );
    // L2 with the wrong branch (N4b) as base: totals grow, but the share is not L2's own usage
    let l2_wrong = replay(
        &out,
        "l2w",
        &fx("l2-resume.jsonl"),
        SessionMode::Resume,
        Some(out.join("ev/02-01/n4b/c1/record.json")),
    );
    assert!(
        matches!(&l2_wrong.usage.run, Measure::Unmeasured { reason } if reason.contains("압축 사건 없음"))
    );
    // L2 with the branch it continued (N4a): its own usage
    let l2 = replay(
        &out,
        "l2",
        &fx("l2-resume.jsonl"),
        SessionMode::Resume,
        Some(out.join("ev/02-01/n4a/c1/record.json")),
    );
    assert_eq!(tokens(&l2.usage.run), tokens(&l2.usage.parent));
    assert_eq!(l2.usage.attribution_check, "consistent");
    // L3 (/compact): result.usage is all zero, the compaction calls show only in the totals
    let l3 = replay(
        &out,
        "l3",
        &fx("l3-compact.jsonl"),
        SessionMode::Resume,
        Some(out.join("ev/02-01/l2/c1/record.json")),
    );
    assert_eq!(l3.result_event.as_ref().unwrap().compact_boundaries, 1);
    assert_eq!(tokens(&l3.usage.parent), Tokens::default());
    assert_eq!(
        tokens(&l3.usage.run),
        Tokens {
            input: 2020,
            output: 1354,
            cache_read: 2761,
            cache_creation: 1019
        }
    );
    assert_eq!(l3.usage.attribution_check, "mismatch");
    assert_eq!(summarize(&[l3]).usage.attribution_mismatches.len(), 1);
}

/// PR #21 review P2-2: a descendant outside the process group keeps stdout open.
/// The bytes already written must be kept and the run must not count as success.
#[test]
fn output_held_by_detached_descendant_is_kept_and_flagged() {
    let out = tmp();
    // `sleep 1` lets setsid run before the leader exits; otherwise the collector may kill the
    // child while it is still in the group (correct, but then nothing holds the pipe).
    let mut s = spec(&out, "echo hello-marker; setsid sleep 20 & sleep 1; exit 0");
    s.required_markers = vec!["hello-marker".into()];
    let r = execute(&s).unwrap();
    assert!(r.process.output_incomplete);
    assert_eq!(r.stdout.bytes, 13, "already-read output kept");
    assert!(r.markers[0].found);
    assert_eq!(r.judgment.status, JudgmentStatus::Incomplete);
    assert!(!r.judgment.counts_as_success);
    assert!(r.judgment.reasons.iter().any(|x| x.contains("출력 관")));
    assert!(r.duration_ms < 8_000, "{}", r.duration_ms);
}

/// PR #21 review P2-3: after a timeout, a group member that ignores TERM must be
/// killed after the grace period even when the leader already exited.
#[test]
fn group_member_ignoring_term_is_killed_after_grace() {
    let out = tmp();
    // POSIX sh (dash on CI): no `exec -a`; a per-process sleep length identifies the member.
    let secs = 4000 + std::process::id() % 1000;
    let marker = format!("sleep {secs}");
    let mut s = spec(
        &out,
        &format!("(trap '' TERM; exec {marker}) >/dev/null 2>&1 & sleep 60"),
    );
    s.timeout = Some(Duration::from_secs(1));
    s.grace = Duration::from_secs(1);
    s.expected = Expected::Timeout;
    let r = execute(&s).unwrap();
    assert_eq!(r.process.kind, ProcessKind::TimedOut);
    assert!(r.process.term_sent && r.process.kill_sent && r.process.stray_group_killed);
    std::thread::sleep(Duration::from_millis(300));
    let ps = std::process::Command::new("pgrep")
        .args(["-f", &marker])
        .output()
        .unwrap();
    assert!(ps.stdout.is_empty(), "TERM-ignoring member survived");
}

/// A member left behind by a normal exit (not holding the pipes) is killed and recorded,
/// without turning the run into a failure.
#[test]
fn leftover_member_after_normal_exit_is_killed_and_recorded() {
    let out = tmp();
    let r = execute(&spec(&out, "sleep 30 >/dev/null 2>&1 & echo done")).unwrap();
    assert!(r.process.stray_group_killed);
    assert_eq!(r.judgment.status, JudgmentStatus::Succeeded);
    assert!(
        r.judgment
            .reasons
            .iter()
            .any(|x| x.contains("남은 프로세스"))
    );
}

/// Review 273907b P3-1: a group member that writes shortly after the leader exits
/// is given the drain window; its output is kept.
#[test]
fn late_output_within_drain_window_is_kept() {
    let out = tmp();
    let r = execute(&spec(&out, "(sleep 0.3; echo late-line) & echo early")).unwrap();
    let d = out.join("ev/02-01/a1/c1");
    assert_eq!(
        std::fs::read_to_string(d.join("stdout")).unwrap(),
        "early\nlate-line\n"
    );
    assert!(!r.process.output_incomplete);
    assert!(!r.process.stray_group_killed);
    assert_eq!(r.judgment.status, JudgmentStatus::Succeeded);
}

/// Review 273907b P3-2: stdout and stderr get the same treatment.
#[test]
fn late_stderr_from_detached_descendant_is_kept_like_stdout() {
    let out = tmp();
    let r = execute(&spec(
        &out,
        "setsid sh -c 'sleep 0.5; echo late-err >&2' & sleep 0.2; exit 0",
    ))
    .unwrap();
    let d = out.join("ev/02-01/a1/c1");
    assert_eq!(
        std::fs::read_to_string(d.join("stderr")).unwrap(),
        "late-err\n"
    );
    assert!(!r.process.output_incomplete);
    assert_eq!(r.judgment.status, JudgmentStatus::Succeeded);
}

// --- 02-04 D3: a child's total comes from its transcript ---

#[test]
fn without_a_transcript_the_child_is_unmeasured_and_its_last_response_kept() {
    let out = tmp();
    let f = fixture(&out, "t23.jsonl", &real_stream());
    let mut s = spec(&out, &format!("cat {}", f.display()));
    s.model_request = true;
    let r = execute(&s).unwrap();
    let c = &r.usage.children[0];
    assert!(
        matches!(&c.usage, Measure::Unmeasured { reason } if reason.contains("자식 전사 없음")),
        "{:?}",
        c.usage
    );
    assert_eq!(
        c.reported_last_response,
        Some(Tokens {
            input: 2,
            output: 9,
            cache_read: 0,
            cache_creation: 844
        })
    );
    // a new session's share is its session total either way
    assert!(r.usage.run.tokens().is_some());
    assert_eq!(r.usage.attribution_check, "not_checked");
}

/// Review 608b367 F8: a `--child-transcript` that cannot be read is reported as a read error,
/// not as "no transcript".
#[test]
fn an_unreadable_transcript_is_reported_as_a_read_error() {
    let out = tmp();
    let f = fixture(&out, "t23.jsonl", &real_stream());
    let mut s = spec(&out, &format!("cat {}", f.display()));
    s.model_request = true;
    let missing = out.join("no-such-transcript.jsonl");
    s.child_transcripts
        .insert(T23_AGENT.into(), missing.clone());
    let r = execute(&s).unwrap();
    let c = &r.usage.children[0];
    assert!(
        matches!(&c.usage, Measure::Unmeasured { reason }
            if reason.contains("자식 전사를 읽지 못함")
                && reason.contains(missing.to_str().unwrap())
                && !reason.contains("자식 전사 없음")),
        "{:?}",
        c.usage
    );
    // fail-closed as before: the run is not checked against its children
    assert_eq!(r.usage.attribution_check, "not_checked");
}

// --- 02-04 D3 replay (de-identified stored record, no model call) ---

const D3: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/d3-child-usage");

fn d3_base() -> ieo_evidence::record::ResumeBase {
    let b: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(format!("{D3}/base-d2.json")).unwrap())
            .unwrap();
    ieo_evidence::record::ResumeBase {
        task_id: "02-04".into(),
        attempt_id: "D2".into(),
        command_id: "development-reconnect".into(),
        session_id: b["session_id"].as_str().map(str::to_owned),
        session: serde_json::from_value(b["session"].clone()).unwrap(),
        cost_usd_reported: b["cost_usd_reported"].as_f64(),
    }
}

fn d3_parse(
    transcript: Option<&str>,
    base: ieo_evidence::record::ResumeBase,
) -> ieo_evidence::stream::Parsed {
    let stream = std::fs::read(format!("{D3}/stream.jsonl")).unwrap();
    let mut t = std::collections::BTreeMap::new();
    if let Some(x) = transcript {
        t.insert("agent-1".to_string(), x.as_bytes().to_vec());
    }
    ieo_evidence::stream::parse_with_children(&stream, SessionMode::Resume, Some(base), &t)
}

fn d3_transcript() -> String {
    std::fs::read_to_string(format!("{D3}/child-transcript.jsonl")).unwrap()
}

const D3_RUN: Tokens = Tokens {
    input: 10,
    output: 224,
    cache_read: 35_662,
    cache_creation: 2_613,
};

#[test]
fn d3_the_child_s_two_responses_reconcile_the_session_increase() {
    let p = d3_parse(Some(&d3_transcript()), d3_base());
    let u = &p.usage;
    // child: both responses (2/61/0/2067 and 2/3/2067/90), not the last one reported
    assert_eq!(
        tokens(&u.children[0].usage),
        Tokens {
            input: 4,
            output: 64,
            cache_read: 2_067,
            cache_creation: 2_157
        }
    );
    assert_eq!(
        u.children[0].reported_last_response,
        Some(Tokens {
            input: 2,
            output: 3,
            cache_read: 2_067,
            cache_creation: 90
        })
    );
    assert_eq!(
        tokens(&u.parent),
        Tokens {
            input: 6,
            output: 160,
            cache_read: 33_595,
            cache_creation: 456
        }
    );
    // parent + child = session increase, all four fields
    assert_eq!(
        u.attribution_check, "consistent",
        "{:?}",
        u.attribution_detail
    );
    assert_eq!(tokens(&u.run), D3_RUN);
}

#[test]
fn d3_without_the_child_total_the_resume_stays_unmeasured() {
    // no transcript: the child is unmeasured, the share cannot be matched
    let p = d3_parse(None, d3_base());
    assert!(
        matches!(&p.usage.run, Measure::Unmeasured { .. }),
        "{:?}",
        p.usage.run
    );
    // the last response only (what tool_use_result reports): parent + child < increase
    let last_only: String = d3_transcript()
        .lines()
        .skip(1)
        .map(|l| format!("{l}\n"))
        .collect();
    let p = d3_parse(Some(&last_only), d3_base());
    assert_eq!(p.usage.attribution_check, "mismatch");
    assert!(matches!(&p.usage.run, Measure::Unmeasured { .. }));
}

#[test]
fn d3_repeated_or_in_progress_entries_count_once() {
    let t = d3_transcript();
    let first = t.lines().next().unwrap();
    // an in-progress entry of the first response (no stop_reason, streaming output 16) and an
    // identical repeat of its final entry
    let mut partial: serde_json::Value = serde_json::from_str(first).unwrap();
    partial["message"]["stop_reason"] = serde_json::Value::Null;
    partial["message"]["usage"]["output_tokens"] = 16.into();
    let noisy = format!("{partial}\n{first}\n{first}\n{t}");
    let p = d3_parse(Some(&noisy), d3_base());
    assert_eq!(tokens(&p.usage.run), D3_RUN);
    assert_eq!(p.usage.attribution_check, "consistent");
}

#[test]
fn d3_unconfirmed_or_conflicting_child_records_stay_unmeasured() {
    let t = d3_transcript();
    let first = t.lines().next().unwrap();
    let mut v: serde_json::Value = serde_json::from_str(first).unwrap();
    // a response with only an in-progress entry
    v["message"]["stop_reason"] = serde_json::Value::Null;
    let only_partial = format!("{v}\n{}\n", t.lines().nth(1).unwrap());
    // two different final entries for one response
    let mut w: serde_json::Value = serde_json::from_str(first).unwrap();
    w["message"]["usage"]["output_tokens"] = 62.into();
    let conflicting = format!("{t}{w}\n");
    // an entry of another agent, and an unreadable line
    let mut x: serde_json::Value = serde_json::from_str(first).unwrap();
    x["agentId"] = "agent-9".into();
    let other_agent = format!("{t}{x}\n");
    let broken = format!("{t}{{not json\n");
    for (name, tr) in [
        ("partial", only_partial),
        ("conflict", conflicting),
        ("other agent", other_agent),
        ("broken", broken),
    ] {
        let p = d3_parse(Some(&tr), d3_base());
        assert!(
            matches!(&p.usage.children[0].usage, Measure::Unmeasured { .. }),
            "{name}"
        );
        assert!(matches!(&p.usage.run, Measure::Unmeasured { .. }), "{name}");
    }
}

#[test]
fn d3_a_wrong_or_larger_base_is_still_refused() {
    // another session's base
    let mut b = d3_base();
    b.session_id = Some("session-other".into());
    assert!(matches!(
        &d3_parse(Some(&d3_transcript()), b).usage.run,
        Measure::Unmeasured { .. }
    ));
    // a base above the session total (cumulative went down)
    let mut b = d3_base();
    b.session = Measure::Measured {
        tokens: Tokens {
            input: 100,
            output: 1_000,
            cache_read: 100_000,
            cache_creation: 100_000,
        },
        source: "test".into(),
    };
    assert!(matches!(
        &d3_parse(Some(&d3_transcript()), b).usage.run,
        Measure::Unmeasured { .. }
    ));
    // an older base (D1's totals would leave D2's usage unattributed)
    let mut b = d3_base();
    b.session = Measure::Measured {
        tokens: Tokens {
            input: 2,
            output: 17,
            cache_read: 11_393,
            cache_creation: 5_231,
        },
        source: "test".into(),
    };
    let p = d3_parse(Some(&d3_transcript()), b);
    assert_eq!(p.usage.attribution_check, "mismatch");
    assert!(matches!(&p.usage.run, Measure::Unmeasured { .. }));
}

// --- review 608b367 ---

#[test]
fn d3_a_transcript_whose_last_response_is_not_the_reported_one_is_refused() {
    // F2: the two responses in the other order: the last is not what tool_use_result reported
    let t = d3_transcript();
    let swapped: String = t.lines().rev().map(|l| format!("{l}\n")).collect();
    let p = d3_parse(Some(&swapped), d3_base());
    assert!(
        matches!(&p.usage.children[0].usage, Measure::Unmeasured { reason } if reason.contains("마지막 응답")),
        "{:?}",
        p.usage.children[0].usage
    );
    assert!(matches!(&p.usage.run, Measure::Unmeasured { .. }));
}

fn d3_with_compaction(
    transcript: Option<&str>,
    parent_output: Option<u64>,
) -> ieo_evidence::stream::Parsed {
    let mut lines: Vec<serde_json::Value> = std::fs::read_to_string(format!("{D3}/stream.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    lines.insert(
        1,
        serde_json::json!({"type": "system", "subtype": "compact_boundary"}),
    );
    if let Some(o) = parent_output {
        for v in lines.iter_mut().filter(|v| v["type"] == "result") {
            v["usage"]["output_tokens"] = o.into();
        }
    }
    let stream: String = lines.iter().map(|v| format!("{v}\n")).collect();
    let mut t = std::collections::BTreeMap::new();
    if let Some(x) = transcript {
        t.insert("agent-1".to_string(), x.as_bytes().to_vec());
    }
    ieo_evidence::stream::parse_with_children(
        stream.as_bytes(),
        SessionMode::Resume,
        Some(d3_base()),
        &t,
    )
}

#[test]
fn with_compaction_a_share_below_its_parts_or_with_an_unconfirmed_child_is_unmeasured() {
    // F5: compaction explains a share above parent + children, nothing else
    assert_eq!(
        tokens(&d3_with_compaction(Some(&d3_transcript()), None).usage.run),
        D3_RUN
    );
    // a parent that used more output than the whole share
    let p = d3_with_compaction(Some(&d3_transcript()), Some(500));
    assert!(
        matches!(&p.usage.run, Measure::Unmeasured { .. }),
        "{:?}",
        p.usage.run
    );
    // an unconfirmed child (no transcript)
    let p = d3_with_compaction(None, None);
    assert!(
        matches!(&p.usage.run, Measure::Unmeasured { .. }),
        "{:?}",
        p.usage.run
    );
    // N3: each of the other three fields below its parts as well
    for (field, v) in [
        ("input_tokens", 50u64),
        ("cache_read_input_tokens", 40_000),
        ("cache_creation_input_tokens", 3_000),
    ] {
        let mut lines: Vec<serde_json::Value> =
            std::fs::read_to_string(format!("{D3}/stream.jsonl"))
                .unwrap()
                .lines()
                .map(|l| serde_json::from_str(l).unwrap())
                .collect();
        lines.insert(
            1,
            serde_json::json!({"type": "system", "subtype": "compact_boundary"}),
        );
        for x in lines.iter_mut().filter(|x| x["type"] == "result") {
            x["usage"][field] = v.into();
        }
        let stream: String = lines.iter().map(|x| format!("{x}\n")).collect();
        let mut t = std::collections::BTreeMap::new();
        t.insert("agent-1".to_string(), d3_transcript().into_bytes());
        let p = ieo_evidence::stream::parse_with_children(
            stream.as_bytes(),
            SessionMode::Resume,
            Some(d3_base()),
            &t,
        );
        assert!(
            matches!(&p.usage.run, Measure::Unmeasured { .. }),
            "{field}: {:?}",
            p.usage.run
        );
    }
    // a share above parent + children with compaction stays measured (compaction calls)
    let p = d3_with_compaction(Some(&d3_transcript()), Some(100));
    assert_eq!(tokens(&p.usage.run), D3_RUN);
    assert_eq!(p.usage.attribution_check, "mismatch");
}
