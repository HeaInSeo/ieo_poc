//! 02-03 integration tests against a real PostgreSQL and the real HTTP server.
//!
//! `IEO_TEST_DB_URL` (key=value or URL form) names a server where the test user may create
//! databases. Each test creates its own database and server on 127.0.0.1:0. Without the
//! variable the tests fail under CI (`CI` set) and are skipped with a message elsewhere.

use ieo_coord::api::migrate;
use ieo_coord::auth::{Credentials, token_digest};
use ieo_coord::policy;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::Arc;

mod common;
use common::*;

// ---------------------------------------------------------------------------

#[test]
fn health_is_liveness_only_and_needs_no_credential() {
    let Some(e) = start() else { return };
    let (st, b) = e.call(None, "GET", "/v0/health", None, None);
    assert_eq!(st, 200);
    assert_eq!(b["status"], "up");
    assert!(
        b["scope"]
            .as_str()
            .unwrap_or_default()
            .contains("not a P04 result")
    );
    assert_eq!(b["policy_digest"], policy::PINNED_POLICY_DIGEST);
}

#[test]
fn authentication_and_scope_refusals_change_nothing() {
    let Some(e) = start() else { return };
    let s = spec("repo-a", 5, 1);
    let before = e.counts();
    assert_eq!(
        e.call(None, "POST", "/v0/sprints", Some("k-00000001"), Some(&s))
            .0,
        401
    );
    assert_eq!(
        e.call(
            Some("not-a-token"),
            "GET",
            "/v0/sprints/POC-01/state",
            None,
            None
        )
        .0,
        401
    );
    let (st, b) = e.post_k("dev-a", "/v0/sprints", "k-00000002", &s);
    assert_eq!((st, code(&b)), (403, "AGENT_READ_DENIED".into()));
    let (st, b) = e.get("gr-a", "/v0/sprints/POC-01/state");
    assert_eq!((st, code(&b)), (403, "AGENT_READ_DENIED".into()));
    let (st, b) = e.post_k("bob", "/v0/sprints", "k-00000003", &s);
    assert_eq!((st, code(&b)), (403, "SCOPE_DENIED".into()));
    let (st, b) = e.post_k("carol", "/v0/sprints", "k-00000004", &s);
    assert_eq!((st, code(&b)), (403, "REPO_SCOPE_DENIED".into()));
    let (st, b) = e.post("dev-a", "/v0/coordinator/judgments", &json!({}));
    assert_eq!((st, code(&b)), (403, "SCOPE_DENIED".into()));
    assert_eq!(e.counts(), before, "refusals leave every table unchanged");
}

#[test]
fn registration_is_format_intake_and_semantic_review_is_separate() {
    let Some(e) = start() else { return };
    let s = spec("repo-a", 5, 1);
    let (st, r) = e.post_k("alice", "/v0/sprints?mode=executable", "reg-key-0001", &s);
    assert_eq!(st, 201, "{r}");
    assert_eq!(r["state"], "ACCEPTED_FOR_SEMANTIC_REVIEW");
    // idempotent replay and key conflict
    assert_eq!(
        e.post_k("alice", "/v0/sprints?mode=executable", "reg-key-0001", &s)
            .0,
        200
    );
    let (st, b) = e.post_k("alice", "/v0/sprints?mode=draft", "reg-key-0001", &s);
    assert_eq!((st, code(&b)), (409, "IDEMPOTENCY_KEY_CONFLICT".into()));
    let (st, rev) = e.get("alice", "/v0/sprints/POC-01/revisions/5");
    assert_eq!(st, 200);
    assert_eq!(rev["review_state"], "pending_semantic_review");
    assert_eq!(rev["digest"], s["digest"]);
    // a sprint of another repo is not visible
    assert_eq!(e.get("carol", "/v0/sprints/POC-01/revisions/5").0, 404);
    // format-valid but not semantically reviewed → no assignment
    let (st, b) = assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null);
    assert_eq!((st, code(&b)), (422, "SEMANTIC_REVIEW_NOT_PASSED".into()));
    // the registrant cannot review its own registration; a review names the exact digest
    let (st, b) = e.post(
        "carol",
        "/v0/sprints/POC-01/revisions/5/semantic-review",
        &json!({"decision": "passed", "reason": "x", "digest": s["digest"]}),
    );
    assert_eq!(st, 404, "carol's repo scope does not include repo-a: {b}");
    let wrong = json!({"decision": "passed", "reason": "ok", "digest": "0".repeat(64)});
    let (st, b) = e.post(
        "bob",
        "/v0/sprints/POC-01/revisions/5/semantic-review",
        &wrong,
    );
    assert_eq!((st, code(&b)), (409, "DIGEST_MISMATCH".into()));
    let good = json!({"decision": "passed", "reason": "ok", "digest": s["digest"]});
    assert_eq!(
        e.post(
            "bob",
            "/v0/sprints/POC-01/revisions/5/semantic-review",
            &good
        )
        .0,
        201
    );
    assert_eq!(
        e.post(
            "bob",
            "/v0/sprints/POC-01/revisions/5/semantic-review",
            &good
        )
        .0,
        200
    );
    let other = json!({"decision": "rejected", "reason": "changed mind", "digest": s["digest"]});
    assert_eq!(
        e.post(
            "bob",
            "/v0/sprints/POC-01/revisions/5/semantic-review",
            &other
        )
        .0,
        409
    );
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
}

#[test]
fn self_review_is_refused() {
    let Some(e) = start() else { return };
    let s = spec("repo-b", 5, 1);
    assert_eq!(
        e.post_k("carol", "/v0/sprints?mode=executable", "reg-key-0002", &s)
            .0,
        201
    );
    let (st, b) = e.post(
        "carol",
        "/v0/sprints/POC-01/revisions/5/semantic-review",
        &json!({"decision": "passed", "reason": "mine", "digest": s["digest"]}),
    );
    assert_eq!((st, code(&b)), (403, "SELF_REVIEW_DENIED".into()));
}

#[test]
fn draft_and_superseded_revisions_are_not_assignable() {
    let Some(e) = start() else { return };
    let s5 = spec("repo-a", 5, 1);
    assert_eq!(e.post_k("alice", "/v0/sprints", "reg-key-d001", &s5).0, 201);
    assert_eq!(
        e.post(
            "bob",
            "/v0/sprints/POC-01/revisions/5/semantic-review",
            &json!({"decision": "passed", "reason": "ok", "digest": s5["digest"]})
        )
        .0,
        201
    );
    let (st, b) = assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null);
    assert_eq!((st, code(&b)), (422, "UNBOUND_DRAFT".into()));
    let s6 = spec("repo-a", 6, 1);
    assert_eq!(
        e.post_k("alice", "/v0/sprints?mode=executable", "reg-key-d002", &s6)
            .0,
        201
    );
    // old revision: promotion refused, assignment refused, replay is historical
    let (st, b) = e.post_k("alice", "/v0/sprints?mode=executable", "reg-key-d003", &s5);
    assert_eq!((st, code(&b)), (409, "REVISION_SUPERSEDED".into()));
    let (st, b) = assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null);
    assert_eq!((st, code(&b)), (409, "REVISION_SUPERSEDED".into()));
    let (st, b) = e.post_k("alice", "/v0/sprints", "reg-key-d001", &s5);
    assert_eq!(
        (st, b["state"].clone(), b["superseded_by"].clone()),
        (200, json!("SUPERSEDED"), json!(6))
    );
}

#[test]
fn ownership_head_is_compare_and_swap() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 3);
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    // replay: historical answer, no change
    let (st, b) = assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null);
    assert_eq!(
        (st, b["outcome"].clone(), b["is_current"].clone()),
        (200, json!("already_issued"), json!(true))
    );
    // a second issuer that did not see A1 loses
    let (st, b) = assign(&e, "development", "B1", 0, "01-01", SHA2, Value::Null);
    assert_eq!((st, code(&b)), (409, "OWNERSHIP_CONFLICT".into()));
    // generations must follow the head directly
    let (st, b) = assign(
        &e,
        "development",
        "A1",
        2,
        "01-01",
        SHA2,
        json!({"assignment_id": "A1", "generation": 0}),
    );
    assert_eq!((st, code(&b)), (409, "OWNERSHIP_CONFLICT".into()));
    let (st, b) = assign(
        &e,
        "development",
        "A1",
        1,
        "01-02",
        SHA2,
        json!({"assignment_id": "A1", "generation": 0}),
    );
    assert_eq!(
        (st, code(&b)),
        (422, "REPORT_SEMANTIC_VIOLATION".into()),
        "a generation keeps its task"
    );
    assert_eq!(
        assign(
            &e,
            "development",
            "A1",
            1,
            "01-01",
            SHA2,
            json!({"assignment_id": "A1", "generation": 0})
        )
        .0,
        201
    );
    // an old (assignment, generation) replay never reactivates it
    let (st, b) = assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null);
    assert_eq!((st, b["is_current"].clone()), (200, json!(false)));
    let head: (String, i64) = {
        let mut db = e.db();
        let r = db.query_one("SELECT assignment_id, generation FROM assignment_heads WHERE repo_id='repo-a' AND role='development'", &[]).expect("head");
        (r.get(0), r.get(1))
    };
    assert_eq!(head, ("A1".to_string(), 1));
    // a used assignment_id cannot restart at generation 0
    let (st, b) = assign(
        &e,
        "development",
        "A1",
        0,
        "01-01",
        SHA3,
        json!({"assignment_id": "A1", "generation": 1}),
    );
    assert_eq!((st, code(&b)), (409, "EVENT_CONTENT_CONFLICT".into()));
}

#[test]
fn prerequisites_need_a_verified_accepted_verdict() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 2);
    let (st, b) = assign(&e, "development", "A2", 0, "01-02", SHA1, Value::Null);
    assert_eq!((st, code(&b)), (409, "PREREQUISITE_UNMET".into()), "{b}");
    // a claim of completion does not satisfy the prerequisite
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    let mut done = report("development", "dev-done-1", "A1", 0, "01-01", SHA1);
    done["operational_state"] = json!("완료");
    assert_eq!(e.post("dev-a", "/v0/agent/reports", &done).0, 202);
    let (st, b) = assign(
        &e,
        "development",
        "A2",
        0,
        "01-02",
        SHA2,
        json!({"assignment_id": "A1", "generation": 0}),
    );
    assert_eq!((st, code(&b)), (409, "PREREQUISITE_UNMET".into()));
    // a not_accepted verdict does not either
    let (st, v) = judge(&e, "j-early", "01-01");
    assert_eq!(
        (st, v["outcome"].clone()),
        (201, json!("not_accepted")),
        "{v}"
    );
    assert_eq!(
        assign(
            &e,
            "development",
            "A2",
            0,
            "01-02",
            SHA2,
            json!({"assignment_id": "A1", "generation": 0})
        )
        .0,
        409
    );
    // evidence + current guardrail approval → accepted → prerequisite met
    assert_eq!(
        assign(&e, "guardrail", "G1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    assert_eq!(
        e.post(
            "gr-a",
            "/v0/agent/reports",
            &report("guardrail", "gr-ok-1", "G1", 0, "01-01", SHA1)
        )
        .0,
        202
    );
    full_evidence(&e, SHA1, "p");
    let (st, v) = judge(&e, "j-final", "01-01");
    assert_eq!((st, v["outcome"].clone()), (201, json!("accepted")), "{v}");
    assert_eq!(
        assign(
            &e,
            "development",
            "A2",
            0,
            "01-02",
            SHA2,
            json!({"assignment_id": "A1", "generation": 0})
        )
        .0,
        201
    );
}

#[test]
fn claims_late_duplicate_stale_and_mismatched() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 3);
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    let c1 = report("development", "dev-1", "A1", 0, "01-01", SHA1);
    let (st, r) = e.post("dev-a", "/v0/agent/reports", &c1);
    assert_eq!(
        (
            st,
            r["disposition"].clone(),
            r["counts_for_current_judgment"].clone()
        ),
        (202, json!("current"), json!(true)),
        "{r}"
    );
    // duplicate: same content → 200, other content → 409
    assert_eq!(e.post("dev-a", "/v0/agent/reports", &c1).0, 200);
    let mut c1x = c1.clone();
    c1x["next_action"] = json!("different");
    assert_eq!(
        code(&e.post("dev-a", "/v0/agent/reports", &c1x).1),
        "EVENT_CONTENT_CONFLICT"
    );
    // generation bump
    assert_eq!(
        assign(
            &e,
            "development",
            "A1",
            1,
            "01-01",
            SHA2,
            json!({"assignment_id": "A1", "generation": 0})
        )
        .0,
        201
    );
    // replay of the earlier current claim: 200, re-evaluated → no longer counts (PR #10 N3)
    let (st, r) = e.post("dev-a", "/v0/agent/reports", &c1);
    assert_eq!(
        (
            st,
            r["disposition"].clone(),
            r["counts_for_current_judgment"].clone()
        ),
        (200, json!("current"), json!(false))
    );
    // a late claim for gen 0 is preserved with gen 0's SHA and excluded
    let late = report("development", "dev-late", "A1", 0, "01-01", SHA1);
    let (st, r) = e.post("dev-a", "/v0/agent/reports", &late);
    assert_eq!(
        (
            st,
            r["disposition"].clone(),
            r["attributed_target_sha"].clone(),
            r["counts_for_current_judgment"].clone()
        ),
        (202, json!("preserved_late"), json!(SHA1), json!(false))
    );
    let before = e.counts();
    // never issued → 409; SHA of another generation → 409; wrong task → 422; other repo → 403
    assert_eq!(
        code(
            &e.post(
                "dev-a",
                "/v0/agent/reports",
                &report("development", "dev-x", "ZZ", 0, "01-01", SHA1)
            )
            .1
        ),
        "STALE_ASSIGNMENT"
    );
    assert_eq!(
        code(
            &e.post(
                "dev-a",
                "/v0/agent/reports",
                &report("development", "dev-y", "A1", 1, "01-01", SHA1)
            )
            .1
        ),
        "ASSIGNMENT_SHA_MISMATCH"
    );
    let (st, b) = e.post(
        "dev-a",
        "/v0/agent/reports",
        &report("development", "dev-z", "A1", 1, "01-02", SHA2),
    );
    assert_eq!((st, code(&b)), (422, "REPORT_SEMANTIC_VIOLATION".into()));
    let (st, b) = e.post(
        "dev-b",
        "/v0/agent/reports",
        &report("development", "dev-w", "A1", 1, "01-01", SHA2),
    );
    assert_eq!((st, code(&b)), (403, "REPO_SCOPE_DENIED".into()));
    let mut verified = report("development", "dev-v", "A1", 1, "01-01", SHA2);
    verified["evidence_level"] = json!("verified");
    assert_eq!(
        code(&e.post("dev-a", "/v0/agent/reports", &verified).1),
        "VERIFIED_FROM_AGENT_DENIED"
    );
    let gr_as_dev = report("guardrail", "gr-as-dev", "A1", 1, "01-01", SHA2);
    assert_eq!(
        code(&e.post("dev-a", "/v0/agent/reports", &gr_as_dev).1),
        "ROLE_MISMATCH"
    );
    let mut bad = report("development", "dev-bad", "A1", 1, "01-01", SHA2);
    bad["phase"] = json!("없는 단계");
    assert_eq!(e.post("dev-a", "/v0/agent/reports", &bad).0, 400);
    assert_eq!(
        e.counts(),
        before,
        "refused claims leave every table unchanged"
    );
    // read model: the late claim is history, not current state
    let (_, st) = e.get("alice", "/v0/sprints/POC-01/state");
    let ids: Vec<&str> = st["latest_claims"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["event_id"].as_str())
        .collect();
    assert_eq!(ids, vec!["dev-1"], "{st}");
    assert_eq!(st["verified"], json!([]));
}

#[test]
fn repro_requests_new_must_be_current_replays_are_not_requeued() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 3);
    assert_eq!(
        assign(&e, "guardrail", "G1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    let req = |id: &str, generation: u64, sha: &str| {
        json!({"request_id": id, "repo_id": "repo-a", "assignment_id": "G1",
        "generation": generation, "target_sha": sha, "bundle_digest": "a".repeat(64), "command": ["cargo", "test"],
        "expected": "test fails on the bug"})
    };
    let (st, r) = e.post(
        "gr-a",
        "/v0/agent/repro-requests",
        &req("repro-0001", 0, SHA1),
    );
    assert_eq!((st, r["state"].clone()), (202, json!("queued")));
    assert_eq!(
        assign(
            &e,
            "guardrail",
            "G1",
            1,
            "01-01",
            SHA2,
            json!({"assignment_id": "G1", "generation": 0})
        )
        .0,
        201
    );
    let before = e.counts();
    assert_eq!(
        e.post(
            "gr-a",
            "/v0/agent/repro-requests",
            &req("repro-0001", 0, SHA1)
        )
        .0,
        200
    );
    assert_eq!(
        code(
            &e.post(
                "gr-a",
                "/v0/agent/repro-requests",
                &req("repro-0002", 0, SHA1)
            )
            .1
        ),
        "STALE_ASSIGNMENT"
    );
    assert_eq!(
        code(
            &e.post(
                "gr-a",
                "/v0/agent/repro-requests",
                &req("repro-0003", 1, SHA1)
            )
            .1
        ),
        "STALE_ASSIGNMENT"
    );
    assert_eq!(
        e.post(
            "dev-a",
            "/v0/agent/repro-requests",
            &req("repro-0004", 1, SHA2)
        )
        .0,
        403
    );
    assert_eq!(e.counts(), before);
    assert_eq!(
        e.post(
            "gr-a",
            "/v0/agent/repro-requests",
            &req("repro-0005", 1, SHA2)
        )
        .0,
        202
    );
    assert_eq!(e.get("gr-a", "/v0/agent/repro-requests/repro-0001").0, 200);
    assert_eq!(e.get("gr-b", "/v0/agent/repro-requests/repro-0001").0, 404);
    assert_eq!(e.get("dev-a", "/v0/agent/repro-requests/repro-0001").0, 403);
    assert_eq!(e.get("alice", "/v0/agent/repro-requests/repro-0001").0, 403);
}

#[test]
fn verdict_uses_github_and_run_evidence_for_the_recorded_sha_only() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 3);
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    assert_eq!(
        assign(&e, "guardrail", "G1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    // evidence for another SHA never fills the head's scopes (G2)
    full_evidence(&e, SHA2, "other");
    assert_eq!(
        e.post(
            "gr-a",
            "/v0/agent/reports",
            &report("guardrail", "gr-1", "G1", 0, "01-01", SHA1)
        )
        .0,
        202
    );
    let (_, v) = judge(&e, "j1", "01-01");
    assert_eq!(v["outcome"], "not_accepted");
    assert_eq!(v["facts"]["ci_passed_on_head"], false);
    assert_eq!(v["facts"]["merged_sha"], Value::Null);
    // a failing collector run at the head blocks CI even with a green check (02-01 judgment)
    full_evidence(&e, SHA1, "head");
    let rec = json!({"format": "ieo-evidence-record-v0", "judgment": {"expected": "success", "status": "failed",
                     "reasons": ["exit 1"], "counts_as_success": false}});
    assert_eq!(
        evidence(
            &e,
            "run-1",
            "run_record",
            json!({"sha": SHA1, "source_ref": "collector/run-1", "record": rec})
        )
        .0,
        201
    );
    let (_, v) = judge(&e, "j2", "01-01");
    assert_eq!(
        (
            v["outcome"].clone(),
            v["facts"]["ci_passed_on_head"].clone()
        ),
        (json!("not_accepted"), json!(false))
    );
    let ok_rec = json!({"format": "ieo-evidence-record-v0", "judgment": {"expected": "success", "status": "succeeded",
                        "reasons": [], "counts_as_success": true}});
    assert_eq!(
        evidence(
            &e,
            "run-2",
            "run_record",
            json!({"sha": SHA1, "source_ref": "collector/run-1", "record": ok_rec})
        )
        .0,
        201
    );
    let (st, v) = judge(&e, "j3", "01-01");
    assert_eq!((st, v["outcome"].clone()), (201, json!("accepted")), "{v}");
    // G7: the optional output is listed as not judged
    assert!(
        v["facts"]["excluded_not_judged"]
            .to_string()
            .contains("01-01-notes"),
        "{v}"
    );
    // the verdict, its outbox entry and the judgment row exist together
    let mut db = e.db();
    let n: i64 = db.query_one("SELECT count(*) FROM judgments j JOIN outbox o ON o.sprint_id = j.event_sprint AND o.sprint_seq = j.event_seq WHERE j.judgment_id = 'j3'", &[]).expect("q").get(0);
    assert_eq!(n, 1);
    // judgment replay and conflict
    assert_eq!(judge(&e, "j3", "01-01").0, 200);
    assert_eq!(code(&judge(&e, "j3", "01-02").1), "EVENT_CONTENT_CONFLICT");
    // read model: verified separate from claims; delivery follows the outbox
    let (_, st) = e.get("alice", "/v0/sprints/POC-01/state");
    assert_eq!(st["verified"].as_array().map(Vec::len), Some(1));
    assert_eq!(st["verified"][0]["evidence_level"], "verified");
    assert_eq!(st["delivery"], "pending");
    let (_, ob) = e.get("courier", "/v0/coordinator/outbox?state=pending");
    // the entry of the latest verdict (j3) is the last one
    let id = ob["items"]
        .as_array()
        .and_then(|a| a.last())
        .and_then(|i| i["outbox_id"].as_i64())
        .unwrap_or_default();
    assert_eq!(
        e.post(
            "courier",
            &format!("/v0/coordinator/outbox/{id}/delivery"),
            &json!({"state": "sent"})
        )
        .0,
        200
    );
    assert_eq!(
        code(
            &e.post(
                "courier",
                &format!("/v0/coordinator/outbox/{id}/delivery"),
                &json!({"state": "unknown"})
            )
            .1
        ),
        "OUTBOX_STATE_CONFLICT"
    );
    assert_eq!(
        e.get("alice", "/v0/sprints/POC-01/state").1["delivery"],
        "sent"
    );
}

#[test]
fn review_inputs_are_current_independent_and_for_the_head() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 3);
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    assert_eq!(
        assign(&e, "guardrail", "G1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    full_evidence(&e, SHA1, "h");
    // approval by gen 0 of the guardrail, then the guardrail moves to gen 1 (same SHA):
    // the gen-0 approval is no longer a current judgment input (PR #10 N4)
    assert_eq!(
        e.post(
            "gr-a",
            "/v0/agent/reports",
            &report("guardrail", "gr-g0", "G1", 0, "01-01", SHA1)
        )
        .0,
        202
    );
    assert_eq!(judge(&e, "jA", "01-01").1["outcome"], "accepted");
    assert_eq!(
        assign(
            &e,
            "guardrail",
            "G1",
            1,
            "01-01",
            SHA1,
            json!({"assignment_id": "G1", "generation": 0})
        )
        .0,
        201
    );
    let (_, v) = judge(&e, "jB", "01-01");
    assert_eq!(v["outcome"], "not_accepted", "{v}");
    assert!(
        v["summary"]
            .as_str()
            .unwrap_or_default()
            .contains("미완료: 현재 SHA 1111111 독립 검토 승인"),
        "{v}"
    );
    // 01-08 N2: the confirmed merge is still shown although the approval is missing
    assert!(
        v["summary"]
            .as_str()
            .unwrap_or_default()
            .contains("병합 4444444"),
        "{v}"
    );
    // a GitHub self-review does not count; an independent one does
    assert_eq!(evidence(&e, "rv-self", "review", json!({"sha": SHA1, "reviewer": "dev", "author": "dev", "state": "approved", "source_ref": "pull/1#r1"})).0, 201);
    assert_eq!(judge(&e, "jC", "01-01").1["outcome"], "not_accepted");
    assert_eq!(evidence(&e, "rv-ind", "review", json!({"sha": SHA1, "reviewer": "rev", "author": "dev", "state": "approved", "source_ref": "pull/1#r2"})).0, 201);
    assert_eq!(judge(&e, "jD", "01-01").1["outcome"], "accepted");
    // a current reject at the head blocks the approval
    let mut rej = report("guardrail", "gr-g1-reject", "G1", 1, "01-01", SHA1);
    rej["body"]["verdict"] = json!("reject");
    rej["body"]["findings"] = json!([{"finding_id": "F1", "violated_condition": "acceptance",
        "reproduction": {"command": "cargo test", "input": "case 1"}, "expected": "pass", "actual": "fail",
        "severity": "P2", "confidence": "reproduced", "original_log_ref": "logs/1.txt"}]);
    assert_eq!(e.post("gr-a", "/v0/agent/reports", &rej).0, 202);
    assert_eq!(judge(&e, "jE", "01-01").1["outcome"], "not_accepted");
    // approval of an older SHA is not an approval of the head
    assert_eq!(
        assign(
            &e,
            "development",
            "A1",
            1,
            "01-01",
            SHA2,
            json!({"assignment_id": "A1", "generation": 0})
        )
        .0,
        201
    );
    let (_, v) = judge(&e, "jF", "01-01");
    assert_eq!(
        (v["outcome"].clone(), v["head_sha"].clone()),
        (json!("not_accepted"), json!(SHA2))
    );
}

#[test]
fn attempt_ceiling_is_cumulative_and_records_one_decision_request() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 1);
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    assert_eq!(
        assign(
            &e,
            "development",
            "A1",
            1,
            "01-01",
            SHA2,
            json!({"assignment_id": "A1", "generation": 0})
        )
        .0,
        201
    );
    // a fresh assignment_id does not reset the count (G4)
    let (st, b) = assign(
        &e,
        "development",
        "B1",
        0,
        "01-01",
        SHA3,
        json!({"assignment_id": "A1", "generation": 1}),
    );
    assert_eq!(
        (st, b["outcome"].clone()),
        (202, json!("limit_reached")),
        "{b}"
    );
    let before = e.counts();
    let (st, b) = assign(
        &e,
        "development",
        "A1",
        2,
        "01-01",
        SHA3,
        json!({"assignment_id": "A1", "generation": 1}),
    );
    assert_eq!((st, code(&b)), (409, "LIMIT_REACHED".into()));
    assert_eq!(e.counts(), before, "the decision request is recorded once");
    // the owner keeps the assignment; the decision request is verified and in the outbox
    let mut db = e.db();
    let head: String = db
        .query_one(
            "SELECT assignment_id FROM assignment_heads WHERE role = 'development'",
            &[],
        )
        .expect("h")
        .get(0);
    assert_eq!(head, "A1");
    let (_, st) = e.get("alice", "/v0/sprints/POC-01/state");
    assert_eq!(st["verified"][0]["report_type"], "decision_request", "{st}");
    assert_eq!(st["verified"][0]["decision"]["default_action"], "정지 유지");
    assert_eq!(st["delivery"], "pending");
    // other roles count separately
    assert_eq!(
        assign(&e, "guardrail", "G1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
}

#[test]
fn event_pages_are_append_only_and_reads_change_nothing() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 3);
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    for i in 0..5 {
        assert_eq!(
            e.post(
                "dev-a",
                "/v0/agent/reports",
                &report("development", &format!("ev-{i}"), "A1", 0, "01-01", SHA1)
            )
            .0,
            202
        );
    }
    let before = e.counts();
    let (st, p1) = e.get("alice", "/v0/sprints/POC-01/events?limit=2");
    assert_eq!(
        (st, p1["has_more"].clone(), p1["next_cursor"].clone()),
        (200, json!(true), json!("s2"))
    );
    let (_, p2) = e.get("alice", "/v0/sprints/POC-01/events?limit=2&cursor=s2");
    assert_eq!(p2["events"][0]["event_id"], "ev-2");
    let (_, p3) = e.get("alice", "/v0/sprints/POC-01/events?limit=2&cursor=s4");
    assert_eq!(
        (
            p3["events"].as_array().map(Vec::len),
            p3["has_more"].clone()
        ),
        (Some(1), json!(false))
    );
    let (_, p4) = e.get("alice", "/v0/sprints/POC-01/events?cursor=s5");
    assert_eq!(
        (p4["events"].clone(), p4["next_cursor"].clone()),
        (json!([]), json!("s5"))
    );
    for bad in ["s9", "x1", "s-1", "s01"] {
        let (st, b) = e.get("alice", &format!("/v0/sprints/POC-01/events?cursor={bad}"));
        assert_eq!((st, code(&b)), (400, "INVALID_CURSOR".into()), "{bad}");
    }
    assert_eq!(e.get("alice", "/v0/sprints/POC-01/events?limit=0").0, 400);
    assert_eq!(e.counts(), before, "reads create no rows");
    // history tables refuse UPDATE/DELETE at the database level
    let mut db = e.db();
    assert!(
        db.execute("UPDATE events SET report = '{}'::jsonb", &[])
            .is_err()
    );
    assert!(db.execute("DELETE FROM history", &[]).is_err());
    assert!(db.execute("DELETE FROM assignments", &[]).is_err());
}

#[test]
fn concurrent_duplicates_and_competing_issuers() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 5);
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    let e = Arc::new(e);
    // 8 identical claims at once: one store, seven replays, one row
    let c = report("development", "dup-1", "A1", 0, "01-01", SHA1);
    let hs: Vec<_> = (0..8)
        .map(|_| {
            let (e, c) = (Arc::clone(&e), c.clone());
            std::thread::spawn(move || e.post("dev-a", "/v0/agent/reports", &c).0)
        })
        .collect();
    let mut st: Vec<u16> = hs.into_iter().map(|h| h.join().unwrap_or(0)).collect();
    st.sort_unstable();
    assert_eq!(st, vec![200, 200, 200, 200, 200, 200, 200, 202]);
    // 8 issuers racing for generation 1 of the same head: exactly one wins
    let hs: Vec<_> = (0..8)
        .map(|i| {
            let e = Arc::clone(&e);
            std::thread::spawn(move || {
                let sha = format!("{:040x}", 0xabc0 + i);
                assign(
                    &e,
                    "development",
                    "A1",
                    1,
                    "01-01",
                    &sha,
                    json!({"assignment_id": "A1", "generation": 0}),
                )
                .0
            })
        })
        .collect();
    let mut st: Vec<u16> = hs.into_iter().map(|h| h.join().unwrap_or(0)).collect();
    st.sort_unstable();
    assert_eq!(st, vec![201, 409, 409, 409, 409, 409, 409, 409]);
    // concurrent registrations of the same new revision with different keys: one 201
    let s7 = spec("repo-a", 7, 5);
    let hs: Vec<_> = (0..6)
        .map(|i| {
            let (e, s) = (Arc::clone(&e), s7.clone());
            std::thread::spawn(move || {
                e.post_k(
                    "alice",
                    "/v0/sprints?mode=executable",
                    &format!("race-key-{i:04}"),
                    &s,
                )
                .0
            })
        })
        .collect();
    let mut st: Vec<u16> = hs.into_iter().map(|h| h.join().unwrap_or(0)).collect();
    st.sort_unstable();
    assert_eq!(st, vec![200, 200, 200, 200, 200, 201]);
    let mut db = e.db();
    let n: i64 = db
        .query_one("SELECT count(*) FROM events WHERE event_id = 'dup-1'", &[])
        .expect("q")
        .get(0);
    assert_eq!(n, 1);
    let seqs: Vec<i64> = db
        .query(
            "SELECT sprint_seq FROM events WHERE sprint_id = 'POC-01' ORDER BY sprint_seq",
            &[],
        )
        .expect("q")
        .iter()
        .map(|r| r.get(0))
        .collect();
    assert_eq!(
        seqs,
        (1..=i64::try_from(seqs.len()).unwrap_or_default()).collect::<Vec<_>>(),
        "gap-free per-sprint order"
    );
}

#[test]
fn independent_repos_progress_separately() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 3);
    // repo-b's agent cannot touch repo-a's assignment; repo-a's head is unaffected
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    let (st, _) = e.post(
        "dev-b",
        "/v0/agent/reports",
        &report("development", "x-1", "A1", 0, "01-01", SHA1),
    );
    assert_eq!(st, 403);
    let (st, b) = e.post(
        "assigner",
        "/v0/coordinator/assignments",
        &json!({"repo_id": "repo-b", "role": "development", "assignment_id": "BB", "generation": 0,
                "sprint_id": "POC-01", "sprint_revision": 5, "task_id": "01-01", "target_sha": SHA1,
                "expected_current": null}),
    );
    assert_eq!((st, code(&b)), (422, "REPO_MISMATCH".into()));
}

#[test]
fn human_input_conversion_needs_an_approved_binding() {
    let Some(e) = start() else { return };
    let input = json!({"title": "설명용 예시", "repo": "repo-a", "goal": "설명용 목표",
        "out_of_scope": ["새 기능"], "decisions": ["납기 변경 없음"],
        "deadline": {"state": "none", "at": null, "reason": "기한 없음"},
        "tasks": [{"name": "검사 준비", "output": "시험 입력", "done_when": "입력 확정", "how_to_check": "목록 대조"}]});
    let (st, b) = e.post_k("alice", "/v0/sprint-inputs", "conv-key-01", &input);
    assert_eq!(st, 201, "{b}");
    // the server applied the approved binding; the result is a draft awaiting review
    assert_eq!(b["registration"]["mode"], "draft");
    assert_eq!(b["registration"]["state"], "ACCEPTED_FOR_SEMANTIC_REVIEW");
    assert_eq!(b["conversion"]["spec"]["sprint_id"], "HUM-01");
    assert_eq!(b["conversion"]["spec"]["tasks"][0]["task_id"], "11-01");
    // out_of_scope is preserved, never turned into a judged item (G7)
    assert!(
        b["conversion"]["preserved_unrepresented"]
            .to_string()
            .contains("새 기능"),
        "{b}"
    );
    assert_eq!(
        e.post_k("alice", "/v0/sprint-inputs", "conv-key-01", &input)
            .0,
        200
    );
    // a draft from conversion is not assignable
    let (st, c) = e.post(
        "assigner",
        "/v0/coordinator/assignments",
        &json!({"repo_id": "repo-a", "role": "development", "assignment_id": "H1", "generation": 0,
                "sprint_id": "HUM-01", "sprint_revision": 1, "task_id": "11-01", "target_sha": SHA1,
                "expected_current": null}),
    );
    assert_eq!((st, code(&c)), (422, "UNBOUND_DRAFT".into()));
    let mut other = input.clone();
    other["repo"] = json!("repo-b");
    let (st, b) = e.post_k("carol", "/v0/sprint-inputs", "conv-key-02", &other);
    assert_eq!((st, code(&b)), (422, "UNBOUND_DRAFT".into()));
}

#[test]
fn policy_is_pinned_and_fails_closed() {
    assert_eq!(policy::digest_bytes(POLICY), policy::PINNED_POLICY_DIGEST);
    assert!(policy::load(POLICY).is_ok());
    let mut changed = POLICY.to_vec();
    changed.push(b'\n');
    assert!(policy::load(&changed).is_err());
    let v: Value = serde_json::from_slice(POLICY).unwrap_or(Value::Null);
    let ids: Vec<&str> = v["rules"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| r["rule_id"].as_str())
        .collect();
    assert_eq!(ids.len(), policy::IMPLEMENTED_RULES.len());
    assert!(
        v["numeric_values"]
            .as_str()
            .unwrap_or_default()
            .contains("sets no numeric limit")
    );
}

#[test]
fn credentials_refuse_inconsistent_principals() {
    let d = token_digest("t");
    let bad = [
        json!({"name": "a", "token_sha256": d, "kind": "agent", "scopes": ["agent:report"], "repo_ids": ["r1", "r2"], "role": "development"}),
        json!({"name": "a", "token_sha256": d, "kind": "agent", "scopes": ["sprints:read"], "repo_ids": ["r1"], "role": "development"}),
        json!({"name": "a", "token_sha256": d, "kind": "agent", "scopes": ["agent:repro"], "repo_ids": ["r1"], "role": "development"}),
        json!({"name": "a", "token_sha256": d, "kind": "user", "scopes": ["judge:verify"], "repo_ids": ["r1"]}),
        json!({"name": "a", "token_sha256": d, "kind": "user", "scopes": ["agent:report"], "repo_ids": ["r1"]}),
        json!({"name": "a", "token_sha256": "ABC", "kind": "user", "scopes": [], "repo_ids": []}),
    ];
    // C2: the import scope is service-only, the exception scope user-only (review 9a421b7 F6)
    let bad: Vec<Value> = bad.into_iter().chain([
        json!({"name": "a", "token_sha256": d, "kind": "user", "scopes": ["notion:import"], "repo_ids": ["r1"]}),
        json!({"name": "a", "token_sha256": d, "kind": "service", "scopes": ["exceptions:approve"], "repo_ids": ["r1"]}),
    ]).collect();
    for p in [
        json!({"name": "a", "token_sha256": d, "kind": "service", "scopes": ["notion:import"], "repo_ids": ["r1"]}),
        json!({"name": "a", "token_sha256": d, "kind": "user", "scopes": ["exceptions:approve"], "repo_ids": ["r1"]}),
    ] {
        assert!(
            Credentials::from_json(&json!({"principals": [p]})).is_ok(),
            "{p}"
        );
    }
    for p in bad {
        assert!(
            Credentials::from_json(&json!({"principals": [p]})).is_err(),
            "{p}"
        );
    }
    let c = Credentials::from_json(&json!({"principals": [
        {"name": "s", "token_sha256": d, "kind": "service", "scopes": ["judge:verify"], "repo_ids": ["r1"]}]}))
        .expect("valid");
    assert!(c.authenticate(Some("Bearer t")).is_some());
    assert!(c.authenticate(Some("Bearer u")).is_none());
    assert!(c.authenticate(Some("t")).is_none());
    assert!(c.authenticate(None).is_none());
}

#[test]
fn every_emitted_code_is_declared_in_the_openapi_contract() {
    let src = include_str!("../src/api.rs");
    let api = include_str!("../../../contracts/ieo-api-v0.openapi.yaml");
    let declared: std::collections::BTreeSet<&str> = api
        .split(|c: char| !(c.is_ascii_uppercase() || c == '_'))
        .filter(|w| w.len() >= 6)
        .collect();
    let emitted: std::collections::BTreeSet<&str> = src
        .split('"')
        .skip(1)
        .step_by(2)
        .filter(|w| w.len() >= 6 && w.chars().all(|c| c.is_ascii_uppercase() || c == '_'))
        .collect();
    assert!(emitted.len() >= 20, "{emitted:?}");
    let missing: Vec<&&str> = emitted.iter().filter(|c| !declared.contains(**c)).collect();
    assert!(missing.is_empty(), "codes not in OpenAPI: {missing:?}");
}

#[test]
fn a_failure_after_the_first_write_rolls_back_the_whole_request() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 3);
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    full_evidence(&e, SHA1, "f");
    // fault injection: the outbox insert fails after the verdict event was written
    let mut db = e.db();
    db.batch_execute(
        "CREATE FUNCTION fail_outbox() RETURNS trigger LANGUAGE plpgsql AS
           $$ BEGIN RAISE EXCEPTION 'injected outbox failure'; END $$;
         CREATE TRIGGER outbox_fault BEFORE INSERT ON outbox FOR EACH ROW EXECUTE FUNCTION fail_outbox();",
    )
    .expect("fault");
    let before = e.counts();
    let (st, b) = judge(&e, "j-fault", "01-01");
    assert_eq!((st, code(&b)), (500, "INTERNAL".into()));
    assert_eq!(
        e.counts(),
        before,
        "no verdict event, judgment or history row without its outbox entry"
    );
    db.batch_execute("DROP TRIGGER outbox_fault ON outbox")
        .expect("unfault");
    assert_eq!(judge(&e, "j-fault", "01-01").0, 201);
}

#[test]
fn development_and_guardrail_claims_race_into_one_ordered_stream() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 3);
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    assert_eq!(
        assign(&e, "guardrail", "G1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    let e = Arc::new(e);
    // two scopes (separate scope locks) append to the same sprint at once
    let hs: Vec<_> = (0..24)
        .map(|i| {
            let e = Arc::clone(&e);
            std::thread::spawn(move || {
                if i % 2 == 0 {
                    e.post(
                        "dev-a",
                        "/v0/agent/reports",
                        &report("development", &format!("rd-{i}"), "A1", 0, "01-01", SHA1),
                    )
                    .0
                } else {
                    e.post(
                        "gr-a",
                        "/v0/agent/reports",
                        &report("guardrail", &format!("rg-{i}"), "G1", 0, "01-01", SHA1),
                    )
                    .0
                }
            })
        })
        .collect();
    let st: Vec<u16> = hs.into_iter().map(|h| h.join().unwrap_or(0)).collect();
    assert!(st.iter().all(|s| *s == 202), "{st:?}");
    let mut db = e.db();
    let seqs: Vec<i64> = db
        .query(
            "SELECT sprint_seq FROM events WHERE sprint_id = 'POC-01' ORDER BY sprint_seq",
            &[],
        )
        .expect("q")
        .iter()
        .map(|r| r.get(0))
        .collect();
    assert_eq!(seqs, (1..=24).collect::<Vec<i64>>());
}

// --- review 1e98f17 corrections ---

#[test]
fn a_sprint_cannot_be_moved_to_another_repo() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 3);
    // F1: carol (repo-b) registers a newer revision of repo-a's sprint under repo-b
    let hijack = spec("repo-b", 6, 3);
    let before = e.counts();
    let (st, b) = e.post_k(
        "carol",
        "/v0/sprints?mode=executable",
        "hijack-key-01",
        &hijack,
    );
    assert_eq!((st, code(&b)), (409, "SPRINT_REPO_CONFLICT".into()), "{b}");
    assert_eq!(e.counts(), before);
    assert_eq!(e.get("alice", "/v0/sprints/POC-01/state").0, 200);
    assert_eq!(e.get("carol", "/v0/sprints/POC-01/events").0, 404);
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
}

#[test]
fn github_reviews_count_only_at_the_head_and_comments_do_not_clear_changes_requested() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 3);
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    full_evidence(&e, SHA1, "h");
    let rv = |id: &str, sha: &str, reviewer: &str, state: &str| {
        evidence(
            &e,
            id,
            "review",
            json!({"sha": sha, "reviewer": reviewer, "author": "dev",
            "state": state, "source_ref": format!("pull/1#{id}")}),
        )
    };
    assert_eq!(rv("r1-ok", SHA1, "r1", "approved").0, 201);
    assert_eq!(rv("r2-cr", SHA1, "r2", "changes_requested").0, 201);
    assert_eq!(judge(&e, "g1", "01-01").1["outcome"], "not_accepted");
    // F2 repro 1: a later COMMENTED review by r2 does not clear its changes_requested
    assert_eq!(rv("r2-cm", SHA1, "r2", "commented").0, 201);
    assert_eq!(judge(&e, "g2", "01-01").1["outcome"], "not_accepted");
    // F2 repro 2: r2 approving another PR's SHA does not clear it either
    assert_eq!(rv("r2-other", SHA3, "r2", "approved").0, 201);
    let (_, v) = judge(&e, "g3", "01-01");
    assert_eq!(v["outcome"], "not_accepted", "{v}");
    // a dismissed changes_requested no longer blocks
    assert_eq!(rv("r2-dis", SHA1, "r2", "dismissed").0, 201);
    assert_eq!(judge(&e, "g4", "01-01").1["outcome"], "accepted");
    // F9: when a reject blocks, an approval of the task's earlier SHA is still named as not used
    assert_eq!(
        assign(
            &e,
            "development",
            "A1",
            1,
            "01-01",
            SHA2,
            json!({"assignment_id": "A1", "generation": 0})
        )
        .0,
        201
    );
    full_evidence(&e, SHA2, "h2");
    assert_eq!(rv("r3-cr", SHA2, "r3", "changes_requested").0, 201);
    let (_, v) = judge(&e, "g5", "01-01");
    let summary = v["summary"].as_str().unwrap_or_default();
    assert!(
        v["outcome"] == "not_accepted"
            && summary.contains("이전 SHA 승인 1111111은 현재 판정에 쓰지 않음"),
        "{v}"
    );
}

#[test]
fn a_raised_ceiling_reached_again_asks_again() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 1);
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    assert_eq!(
        assign(
            &e,
            "development",
            "A1",
            1,
            "01-01",
            SHA2,
            json!({"assignment_id": "A1", "generation": 0})
        )
        .0,
        201
    );
    let (_, b) = assign(
        &e,
        "development",
        "A1",
        2,
        "01-01",
        SHA3,
        json!({"assignment_id": "A1", "generation": 1}),
    );
    assert_eq!(b["outcome"], "limit_reached");
    // the user raises the ceiling with a newer revision (ceiling 2)
    let s6 = spec("repo-a", 6, 2);
    assert_eq!(
        e.post_k("alice", "/v0/sprints?mode=executable", "reg-key-r6", &s6)
            .0,
        201
    );
    assert_eq!(
        e.post(
            "bob",
            "/v0/sprints/POC-01/revisions/6/semantic-review",
            &json!({"decision": "passed", "reason": "ok", "digest": s6["digest"]})
        )
        .0,
        201
    );
    let body = |g: u64, sha: &str| {
        json!({"repo_id": "repo-a", "role": "development", "assignment_id": "A1",
        "generation": g, "sprint_id": "POC-01", "sprint_revision": 6, "task_id": "01-01", "target_sha": sha,
        "expected_current": {"assignment_id": "A1", "generation": g - 1}})
    };
    assert_eq!(
        e.post("assigner", "/v0/coordinator/assignments", &body(2, SHA3))
            .0,
        201
    );
    let (st, b) = e.post("assigner", "/v0/coordinator/assignments", &body(3, MERGE));
    assert_eq!(
        (st, b["outcome"].clone()),
        (202, json!("limit_reached")),
        "{b}"
    );
    assert_eq!(
        code(
            &e.post("assigner", "/v0/coordinator/assignments", &body(3, MERGE))
                .1
        ),
        "LIMIT_REACHED"
    );
    let mut db = e.db();
    let n: i64 = db
        .query_one(
            "SELECT count(*) FROM events WHERE report_type = 'decision_request'",
            &[],
        )
        .expect("q")
        .get(0);
    assert_eq!(n, 2);
    // review 0fa839d N1: back to an already-asked ceiling (1) → 409, never 500, nothing new
    let s7 = spec("repo-a", 7, 1);
    assert_eq!(
        e.post_k("alice", "/v0/sprints?mode=executable", "reg-key-r7", &s7)
            .0,
        201
    );
    assert_eq!(
        e.post(
            "bob",
            "/v0/sprints/POC-01/revisions/7/semantic-review",
            &json!({"decision": "passed", "reason": "ok", "digest": s7["digest"]})
        )
        .0,
        201
    );
    let mut b7 = body(3, MERGE);
    b7["sprint_revision"] = json!(7);
    let before = e.counts();
    for _ in 0..2 {
        let (st, b) = e.post("assigner", "/v0/coordinator/assignments", &b7);
        assert_eq!((st, code(&b)), (409, "LIMIT_REACHED".into()), "{b}");
    }
    assert_eq!(e.counts(), before);
}

#[test]
fn whoever_registered_or_promoted_a_revision_cannot_review_it() {
    let Some(e) = start() else { return };
    let s = spec("repo-a", 5, 1);
    assert_eq!(e.post_k("alice", "/v0/sprints", "reg-key-p01", &s).0, 201);
    assert_eq!(
        e.post_k("erin", "/v0/sprints?mode=executable", "reg-key-p02", &s)
            .0,
        201
    );
    let review = json!({"decision": "passed", "reason": "ok", "digest": s["digest"]});
    let (st, b) = e.post(
        "erin",
        "/v0/sprints/POC-01/revisions/5/semantic-review",
        &review,
    );
    assert_eq!((st, code(&b)), (403, "SELF_REVIEW_DENIED".into()));
    assert_eq!(
        e.post(
            "bob",
            "/v0/sprints/POC-01/revisions/5/semantic-review",
            &review
        )
        .0,
        201
    );
}

// --- review 0fa839d corrections ---

#[test]
fn verdicts_judge_the_assigned_revision_and_a_changed_task_needs_a_new_verdict() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 3);
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    assert_eq!(
        assign(&e, "guardrail", "G1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    assert_eq!(
        e.post(
            "gr-a",
            "/v0/agent/reports",
            &report("guardrail", "gr-1", "G1", 0, "01-01", SHA1)
        )
        .0,
        202
    );
    full_evidence(&e, SHA1, "v");
    // revision 6 adds a required output to 01-01
    let mut s6 = spec("repo-a", 6, 3);
    if let Some(o) = s6["tasks"][0]["outputs"].as_array_mut() {
        o.push(
            json!({"artifact_id": "01-01-extra", "description": "added in r6", "required": true}),
        );
    }
    s6["digest"] = json!(ieo_spec::spec_digest(&s6));
    assert_eq!(
        e.post_k("alice", "/v0/sprints?mode=executable", "reg-key-v6", &s6)
            .0,
        201
    );
    assert_eq!(
        e.post(
            "bob",
            "/v0/sprints/POC-01/revisions/6/semantic-review",
            &json!({"decision": "passed", "reason": "ok", "digest": s6["digest"]})
        )
        .0,
        201
    );
    // F3: the assignment came from revision 5, so its outputs are judged
    let (_, v) = judge(&e, "jv", "01-01");
    assert_eq!(v["outcome"], "accepted", "{v}");
    // N3: but 01-01 changed in revision 6, so that verdict no longer satisfies 01-02's prerequisite
    let a2 = json!({"repo_id": "repo-a", "role": "development", "assignment_id": "A2", "generation": 0,
                    "sprint_id": "POC-01", "sprint_revision": 6, "task_id": "01-02", "target_sha": SHA2,
                    "expected_current": {"assignment_id": "A1", "generation": 0}});
    let (st, b) = e.post("assigner", "/v0/coordinator/assignments", &a2);
    assert_eq!((st, code(&b)), (409, "PREREQUISITE_UNMET".into()), "{b}");
}

#[test]
fn one_assignment_id_raced_by_two_roles_never_errors() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 50); // ceiling high enough that 10 rounds never reach it
    let e = Arc::new(e);
    for round in 0..10 {
        let aid = format!("RACE-{round}");
        let (dev_exp, gr_exp) = if round == 0 {
            (Value::Null, Value::Null)
        } else {
            let mut db = e.db();
            let mut head = |role: &str| -> Value {
                db.query_opt("SELECT assignment_id, generation FROM assignment_heads WHERE role = $1", &[&role])
                    .expect("q")
                    .map_or(Value::Null, |r| json!({"assignment_id": r.get::<_, String>(0), "generation": r.get::<_, i64>(1)}))
            };
            (head("development"), head("guardrail"))
        };
        let hs: Vec<_> = [("development", dev_exp), ("guardrail", gr_exp)]
            .into_iter()
            .map(|(role, exp)| {
                let (e, aid) = (Arc::clone(&e), aid.clone());
                std::thread::spawn(move || assign(&e, role, &aid, 0, "01-01", SHA1, exp).0)
            })
            .collect();
        let mut st: Vec<u16> = hs.into_iter().map(|h| h.join().unwrap_or(0)).collect();
        st.sort_unstable();
        assert_eq!(st, vec![201, 409], "round {round}");
    }
}

// --- review 4bc8081 correction ---

#[test]
fn label_and_estimate_changes_keep_a_prerequisite_verdict_but_definition_changes_void_it() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 3);
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    assert_eq!(
        assign(&e, "guardrail", "G1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    assert_eq!(
        e.post(
            "gr-a",
            "/v0/agent/reports",
            &report("guardrail", "gr-1", "G1", 0, "01-01", SHA1)
        )
        .0,
        202
    );
    full_evidence(&e, SHA1, "l");
    assert_eq!(judge(&e, "jl", "01-01").1["outcome"], "accepted");
    let review = |rev: u64, s: &Value| {
        e.post(
            "bob",
            &format!("/v0/sprints/POC-01/revisions/{rev}/semantic-review"),
            &json!({"decision": "passed", "reason": "ok", "digest": s["digest"]}),
        )
        .0
    };
    let a2 = |rev: u64, aid: &str| {
        json!({"repo_id": "repo-a", "role": "development", "assignment_id": aid,
        "generation": 0, "sprint_id": "POC-01", "sprint_revision": rev, "task_id": "01-02", "target_sha": SHA2,
        "expected_current": {"assignment_id": "A1", "generation": 0}})
    };
    // R1: revision 6 changes only the status label and estimate of 01-01 → verdict still counts
    let mut s6 = spec("repo-a", 6, 3);
    s6["tasks"][0]["status"] = json!("검수 완료");
    s6["tasks"][0]["estimate"] = json!({"state": "measured", "range_seconds": {"min": 60, "max": 120},
                                        "evidence": ["fixture://measured"], "reason": null});
    s6["digest"] = json!(ieo_spec::spec_digest(&s6));
    let (st, b) = e.post_k("alice", "/v0/sprints?mode=executable", "reg-key-l6", &s6);
    assert_eq!(st, 201, "{b}");
    assert_eq!(review(6, &s6), 201);
    // revision 7 changes 01-01's verification → the verdict no longer counts
    let mut s7 = s6.clone();
    s7["revision"] = json!(7);
    s7["tasks"][0]["verification"] = json!("다른 검증 방법");
    s7["digest"] = json!(ieo_spec::spec_digest(&s7));
    let (st, b) = e.post("assigner", "/v0/coordinator/assignments", &a2(6, "A2"));
    assert_eq!(
        st, 201,
        "label/estimate change must not void the verdict: {b}"
    );
    assert_eq!(
        e.post_k("alice", "/v0/sprints?mode=executable", "reg-key-l7", &s7)
            .0,
        201
    );
    assert_eq!(review(7, &s7), 201);
    let a3 = json!({"repo_id": "repo-a", "role": "development", "assignment_id": "A3", "generation": 0,
        "sprint_id": "POC-01", "sprint_revision": 7, "task_id": "01-02", "target_sha": SHA3,
        "expected_current": {"assignment_id": "A2", "generation": 0}});
    let (st, b) = e.post("assigner", "/v0/coordinator/assignments", &a3);
    assert_eq!((st, code(&b)), (409, "PREREQUISITE_UNMET".into()), "{b}");
}

// --- X-01 Notion import ---

fn notion_import(test_id: &str, digest_char: char, unresolved: Value) -> Value {
    json!({
        "test_id": test_id,
        "source": {"system": "notion", "page_id": "0123456789abcdef0123456789abcdef",
                   "url": "https://app.notion.com/p/0123456789abcdef0123456789abcdef",
                   "last_edited_time": "2026-11-01T00:00:00.000Z",
                   "content_digest": digest_char.to_string().repeat(64),
                   "source_task_id": "OP-SYNTH-0001", "fetched_via": "test",
                   "observed_at": "2026-11-13T00:00:00Z"},
        "input": {"title": "합성 작업", "repo": "repo-a", "goal": "합성 목표",
                  "out_of_scope": [], "decisions": [],
                  "deadline": {"state": "undecided", "at": null, "reason": "원본에 기한 없음"},
                  "tasks": [{"name": "X-01 정상", "output": "결과", "done_when": "수용",
                             "how_to_check": "X-01: 정상"}]},
        "unresolved": unresolved
    })
}

#[test]
fn notion_import_registers_a_draft_once_and_revises_on_change() {
    let Some(e) = start() else { return };
    let gate = json!(["Notion 큐 구획 '조건 대기': 현재 실행 대상이 아님"]);
    let (st, r) = e.post(
        "importer",
        "/v0/imports/notion",
        &notion_import("POCX-T01", 'a', gate.clone()),
    );
    assert_eq!(st, 201, "{r}");
    assert_eq!(r["registration"]["mode"], "draft");
    assert_eq!(r["registration"]["revision"], 1);
    assert_eq!(r["import"]["source_task_id"], "OP-SYNTH-0001");
    let (_, rev1) = e.get("alice", "/v0/sprints/POCX-T01/revisions/1");
    assert_eq!(rev1["registration_state"], "draft");
    let spec = &rev1["spec"];
    assert!(
        spec["unresolved_bindings"]
            .to_string()
            .contains("조건 대기"),
        "{spec}"
    );
    assert!(
        spec["sources"].as_array().is_some_and(|s| s
            .iter()
            .any(|x| x["kind"] == "notion_page" && x["digest"] == "a".repeat(64))),
        "{spec}"
    );
    // same page, same content: replay, nothing new
    let before = e.counts();
    let (st, r) = e.post(
        "importer",
        "/v0/imports/notion",
        &notion_import("POCX-T01", 'a', gate.clone()),
    );
    assert_eq!(
        (
            st,
            r["replay"].clone(),
            r["registration"]["revision"].clone()
        ),
        (200, json!(true), json!(1))
    );
    assert_eq!(e.counts(), before);
    // changed content: revision 2; revision 1 is kept unchanged
    let (st, r) = e.post(
        "importer",
        "/v0/imports/notion",
        &notion_import("POCX-T01", 'b', gate.clone()),
    );
    assert_eq!(
        (st, r["registration"]["revision"].clone()),
        (201, json!(2)),
        "{r}"
    );
    assert_eq!(
        e.get("alice", "/v0/sprints/POCX-T01/revisions/1").1["digest"],
        rev1["digest"]
    );
    // content that returns to the first state is a new revision (review 9a421b7 F2), not a replay
    let (st, r) = e.post(
        "importer",
        "/v0/imports/notion",
        &notion_import("POCX-T01", 'a', gate.clone()),
    );
    assert_eq!(
        (
            st,
            r["registration"]["revision"].clone(),
            r["replay"].clone()
        ),
        (201, json!(3), json!(false)),
        "{r}"
    );
    // the same content under another test id, or another page under this test id → 409
    assert_eq!(
        code(
            &e.post(
                "importer",
                "/v0/imports/notion",
                &notion_import("POCX-T02", 'a', gate.clone())
            )
            .1
        ),
        "EVENT_CONTENT_CONFLICT"
    );
    let mut other = notion_import("POCX-T01", 'c', gate);
    other["source"]["page_id"] = json!("fedcba9876543210fedcba9876543210");
    other["source"]["url"] = json!("https://app.notion.com/p/fedcba9876543210fedcba9876543210");
    assert_eq!(
        code(&e.post("importer", "/v0/imports/notion", &other).1),
        "EVENT_CONTENT_CONFLICT"
    );
    // agents and other repos cannot import
    assert_eq!(
        e.post(
            "dev-a",
            "/v0/imports/notion",
            &notion_import("POCX-T03", 'd', json!([]))
        )
        .0,
        403
    );
    assert_eq!(
        code(
            &e.post(
                "importer-b",
                "/v0/imports/notion",
                &notion_import("POCX-T03", 'd', json!([]))
            )
            .1
        ),
        "REPO_SCOPE_DENIED"
    );
}

#[test]
fn an_imported_draft_with_unresolved_reasons_cannot_execute() {
    let Some(e) = start() else { return };
    let (st, r) = e.post(
        "importer",
        "/v0/imports/notion",
        &notion_import_gated(
            "POCX-T01",
            'a',
            json!(["Notion 차단 대상 미해제: 선행 A1"]),
            json!([]),
        ),
    );
    assert_eq!(st, 201, "{r}");
    let spec = e.get("alice", "/v0/sprints/POCX-T01/revisions/1").1["spec"].clone();
    // assigning the draft is refused (draft revisions are checked before the import gate)
    let (st, b) = e.post("assigner", "/v0/coordinator/assignments",
        &json!({"repo_id": "repo-a", "role": "development", "assignment_id": "N1", "generation": 0,
                "sprint_id": "POCX-T01", "sprint_revision": 1, "task_id": "11-01", "target_sha": SHA1,
                "expected_current": null}));
    assert_eq!((st, code(&b)), (422, "UNBOUND_DRAFT".into()), "{b}");
    // promoting the stored draft as executable is refused. Since 02-05 U2 the 01-07 binding
    // check runs before the import gate (C2), so the first code is the binding's 422; before
    // 02-05 it was the gate's 409 `IMPORT_GATED`. Either way it is refused and nothing changes.
    let before = e.counts();
    let (st, b) = e.post_k(
        "alice",
        "/v0/sprints?mode=executable",
        "promote-key-01",
        &spec,
    );
    assert_eq!((st, code(&b)), (422, "UNBOUND_DRAFT".into()), "{b}");
    assert_eq!(e.counts(), before);
    // flipping a copy to ready while reasons remain fails the format check (other sprint id)
    let mut ready = spec.clone();
    ready["registration_state"] = json!("ready");
    ready["sprint_id"] = json!("COPY-OF-IMPORT");
    ready["digest"] = json!(ieo_spec::spec_digest(&ready));
    let (st, b) = e.post_k(
        "alice",
        "/v0/sprints?mode=executable",
        "promote-key-02",
        &ready,
    );
    assert_eq!(st, 400, "{b}");
}

#[test]
fn a_running_assignment_stays_on_its_revision() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 3);
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    // the source changes: revision 6 becomes the latest
    let s6 = spec("repo-a", 6, 3);
    assert_eq!(
        e.post_k("alice", "/v0/sprints?mode=executable", "reg-key-pin6", &s6)
            .0,
        201
    );
    // the running assignment continues on revision 5 (its own next generation)
    assert_eq!(
        assign(
            &e,
            "development",
            "A1",
            1,
            "01-01",
            SHA2,
            json!({"assignment_id": "A1", "generation": 0})
        )
        .0,
        201
    );
    assert_eq!(
        e.post(
            "dev-a",
            "/v0/agent/reports",
            &report("development", "pin-1", "A1", 1, "01-01", SHA2)
        )
        .0,
        202
    );
    // a new assignment cannot start from the superseded revision
    let (st, b) = assign(
        &e,
        "development",
        "B1",
        0,
        "01-01",
        SHA3,
        json!({"assignment_id": "A1", "generation": 1}),
    );
    assert_eq!((st, code(&b)), (409, "REVISION_SUPERSEDED".into()));
    // and a generation cannot jump to an older revision it was not on
    let mut db = e.db();
    let rev: i64 = db
        .query_one(
            "SELECT sprint_revision FROM assignments WHERE assignment_id = 'A1' AND generation = 1",
            &[],
        )
        .expect("q")
        .get(0);
    assert_eq!(rev, 5);
}

#[test]
fn import_sprint_numbers_and_own_revisions_do_not_conflict() {
    let Some(e) = start() else { return };
    let (st, r) = e.post(
        "importer",
        "/v0/imports/notion",
        &notion_import("POCX-T01", 'a', json!([])),
    );
    assert_eq!(st, 201, "{r}");
    assert_eq!(r["conversion"]["spec"]["tasks"][0]["task_id"], "11-01");
    // another Notion page under another test id takes the next free number of the pool
    let mut second = notion_import("POCX-T02", 'e', json!([]));
    second["source"]["page_id"] = json!("fedcba9876543210fedcba9876543210");
    second["source"]["url"] = json!("https://app.notion.com/p/fedcba9876543210fedcba9876543210");
    let (st, r) = e.post("importer", "/v0/imports/notion", &second);
    assert_eq!(st, 201, "{r}");
    assert_eq!(r["conversion"]["spec"]["tasks"][0]["task_id"], "12-01");
    // the first test sprint keeps its number on a new revision
    let (st, r) = e.post(
        "importer",
        "/v0/imports/notion",
        &notion_import("POCX-T01", 'f', json!([])),
    );
    assert_eq!(
        (st, r["conversion"]["spec"]["tasks"][0]["task_id"].clone()),
        (201, json!("11-01")),
        "{r}"
    );
    // pool exhausted → refused, nothing stored
    let before = e.counts();
    let mut third = notion_import("POCX-T03", '0', json!([]));
    third["source"]["page_id"] = json!("00000000000000000000000000000000");
    third["source"]["url"] = json!("https://app.notion.com/p/00000000000000000000000000000000");
    assert_eq!(
        code(&e.post("importer", "/v0/imports/notion", &third).1),
        "UNBOUND_DRAFT"
    );
    assert_eq!(e.counts(), before);
}

#[test]
fn converting_a_sprint_again_is_a_new_revision_not_a_task_conflict() {
    let Some(e) = start() else { return };
    let input = |goal: &str| {
        json!({"title": "예시", "repo": "repo-a", "goal": goal,
        "deadline": {"state": "none", "at": null, "reason": "기한 없음"},
        "tasks": [{"name": "검사 준비", "output": "시험 입력", "done_when": "입력 확정", "how_to_check": "목록 대조"}]})
    };
    assert_eq!(
        e.post_k(
            "alice",
            "/v0/sprint-inputs",
            "conv-rev-01",
            &input("목표 1")
        )
        .0,
        201
    );
    let (st, r) = e.post_k(
        "alice",
        "/v0/sprint-inputs",
        "conv-rev-02",
        &input("목표 2"),
    );
    assert_eq!(
        (st, r["registration"]["revision"].clone()),
        (201, json!(2)),
        "{r}"
    );
}

// --- review b8b619a corrections ---

#[test]
fn a_test_id_cannot_take_over_an_existing_sprint_or_the_notion_id() {
    let Some(e) = start() else { return };
    let input = json!({"title": "예시", "repo": "repo-a", "goal": "목표",
        "deadline": {"state": "none", "at": null, "reason": "기한 없음"},
        "tasks": [{"name": "검사 준비", "output": "시험 입력", "done_when": "입력 확정", "how_to_check": "목록 대조"}]});
    assert_eq!(
        e.post_k("alice", "/v0/sprint-inputs", "conv-r1-01", &input)
            .0,
        201
    );
    let before = e.counts();
    // R1: an existing sprint not created by a Notion import
    let (st, b) = e.post(
        "importer",
        "/v0/imports/notion",
        &notion_import("HUM-01", 'a', json!([])),
    );
    assert_eq!(
        (st, code(&b)),
        (409, "EVENT_CONTENT_CONFLICT".into()),
        "{b}"
    );
    // the test id must differ from the Notion task id
    let (st, b) = e.post(
        "importer",
        "/v0/imports/notion",
        &notion_import("OP-SYNTH-0001", 'a', json!([])),
    );
    assert_eq!((st, code(&b)), (400, "SCHEMA_VIOLATION".into()), "{b}");
    assert_eq!(e.counts(), before);
}

#[test]
fn an_import_replay_answers_only_within_its_repo() {
    let Some(e) = start() else { return };
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &notion_import("POCX-T01", 'a', json!([]))
        )
        .0,
        201
    );
    // R2: carol (repo-b) sends the same page, digest and test id under her repo
    let mut theirs = notion_import("POCX-T01", 'a', json!([]));
    theirs["input"]["repo"] = json!("repo-b");
    let (st, b) = e.post("importer-b", "/v0/imports/notion", &theirs);
    assert_eq!(
        (st, code(&b)),
        (409, "EVENT_CONTENT_CONFLICT".into()),
        "{b}"
    );
    assert!(
        b.get("conversion").is_none() && b.get("registration").is_none(),
        "no stored content: {b}"
    );
}

#[test]
fn a_pinned_continuation_obeys_the_latest_attempt_ceiling() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 3);
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    // R3: the newer revision lowers the ceiling to 0
    let s6 = spec("repo-a", 6, 0);
    assert_eq!(
        e.post_k("alice", "/v0/sprints?mode=executable", "reg-key-r3", &s6)
            .0,
        201
    );
    // only an executable, semantically reviewed revision sets the ceiling of a pinned run
    assert_eq!(
        e.post(
            "bob",
            "/v0/sprints/POC-01/revisions/6/semantic-review",
            &json!({"decision": "passed", "reason": "ok", "digest": s6["digest"]})
        )
        .0,
        201
    );
    let (st, b) = assign(
        &e,
        "development",
        "A1",
        1,
        "01-01",
        SHA2,
        json!({"assignment_id": "A1", "generation": 0}),
    );
    assert_eq!(
        (st, b["outcome"].clone()),
        (202, json!("limit_reached")),
        "{b}"
    );
    let (st, b) = assign(
        &e,
        "development",
        "A1",
        1,
        "01-01",
        SHA2,
        json!({"assignment_id": "A1", "generation": 0}),
    );
    assert_eq!((st, code(&b)), (409, "LIMIT_REACHED".into()), "{b}");
}

// --- review 38b671b corrections ---

#[test]
fn a_draft_revision_never_changes_a_running_execution() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 0);
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    // S1a: a newer *draft* revision raises the ceiling to 5 — the run stays at ceiling 0
    let s6 = spec("repo-a", 6, 5);
    assert_eq!(
        e.post_k("alice", "/v0/sprints?mode=draft", "reg-key-s1a", &s6)
            .0,
        201
    );
    let (st, b) = assign(
        &e,
        "development",
        "A1",
        1,
        "01-01",
        SHA2,
        json!({"assignment_id": "A1", "generation": 0}),
    );
    assert_eq!(
        (st, b["outcome"].clone()),
        (202, json!("limit_reached")),
        "{b}"
    );
}

#[test]
fn a_draft_with_unbound_limits_does_not_block_a_running_execution() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 3);
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    // S1b: a newer draft with null limits (e.g. a re-imported Notion task)
    let mut s6 = spec("repo-a", 6, 3);
    s6["registration_state"] = json!("draft");
    s6["limits"] = json!({"elapsed_seconds": null, "total_tokens": null, "correction_attempts": null,
                          "unresolved_reason": "한도 미정"});
    s6["unresolved_bindings"] = json!(["limits"]);
    s6["digest"] = json!(ieo_spec::spec_digest(&s6));
    let (st, b) = e.post_k("alice", "/v0/sprints?mode=draft", "reg-key-s1b", &s6);
    assert_eq!(st, 201, "{b}");
    let (st, b) = assign(
        &e,
        "development",
        "A1",
        1,
        "01-01",
        SHA2,
        json!({"assignment_id": "A1", "generation": 0}),
    );
    assert_eq!(st, 201, "pinned run continues under its own ceiling: {b}");
}

#[test]
fn an_import_owned_sprint_changes_only_through_imports() {
    let Some(e) = start() else { return };
    let (st, r) = e.post(
        "importer",
        "/v0/imports/notion",
        &notion_import("POCX-T01", 'a', json!([])),
    );
    assert_eq!(st, 201, "{r}");
    // S2: registering a spec over the import's sprint id is refused
    let mut s = spec("repo-a", 2, 3);
    s["sprint_id"] = json!("POCX-T01");
    s["digest"] = json!(ieo_spec::spec_digest(&s));
    let before = e.counts();
    let (st, b) = e.post_k("alice", "/v0/sprints?mode=executable", "reg-key-s2", &s);
    assert_eq!(
        (st, code(&b)),
        (409, "EVENT_CONTENT_CONFLICT".into()),
        "{b}"
    );
    assert_eq!(e.counts(), before);
}

// --- review 7a48a4f corrections ---

/// The import's latest revision, re-registered as executable with bound limits and no reasons.
fn derived_executable(e: &Env, sprint_id: &str) -> Value {
    let rev = e
        .get("alice", &format!("/v0/sprints/{sprint_id}/revisions/1"))
        .1;
    let mut s = rev["spec"].clone();
    s["revision"] = json!(2);
    s["registration_state"] = json!("ready");
    s["target"]["repository_url"] = json!("https://example.invalid/repo-a");
    s["limits"] = json!({"elapsed_seconds": 3600, "total_tokens": 100000, "correction_attempts": 1,
                         "unresolved_reason": null});
    s["unresolved_bindings"] = json!([]);
    s["digest"] = json!(ieo_spec::spec_digest(&s));
    s
}

#[test]
fn an_imported_draft_can_be_promoted_only_with_its_own_tasks_and_source() {
    let Some(e) = start() else { return };
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &notion_import("POCX-T01", 'a', json!([]))
        )
        .0,
        201
    );
    // T1: derived from the latest import (same tasks + notion source), limits bound → executable
    let good = derived_executable(&e, "POCX-T01");
    let mut changed = good.clone();
    changed["tasks"][0]["exit_condition"] = json!("다른 종료 조건");
    changed["digest"] = json!(ieo_spec::spec_digest(&changed));
    let before = e.counts();
    let (st, b) = e.post_k(
        "alice",
        "/v0/sprints?mode=executable",
        "promote-t1-02",
        &changed,
    );
    assert_eq!(
        (st, code(&b)),
        (409, "EVENT_CONTENT_CONFLICT".into()),
        "{b}"
    );
    let mut other_source = good.clone();
    other_source["sources"] = json!([other_source["sources"][0].clone()]);
    other_source["digest"] = json!(ieo_spec::spec_digest(&other_source));
    let (st, b) = e.post_k(
        "alice",
        "/v0/sprints?mode=executable",
        "promote-t1-03",
        &other_source,
    );
    assert_eq!(
        (st, code(&b)),
        (409, "EVENT_CONTENT_CONFLICT".into()),
        "{b}"
    );
    assert_eq!(e.counts(), before);
    let (st, b) = e.post_k(
        "alice",
        "/v0/sprints?mode=executable",
        "promote-t1-01",
        &good,
    );
    assert_eq!(
        (st, b["mode"].clone(), b["revision"].clone()),
        (201, json!("executable"), json!(2)),
        "{b}"
    );
    // T2: a malformed body is a format error (400), not an ownership conflict
    let mut bad = good.clone();
    bad.as_object_mut().map(|m| m.remove("title"));
    assert_eq!(
        e.post_k("alice", "/v0/sprints", "promote-t2-01", &bad).0,
        400
    );
    // T5: another repo hits the repo rule first
    let mut theirs = good.clone();
    theirs["target"]["repo_id"] = json!("repo-b");
    theirs["revision"] = json!(3);
    theirs["digest"] = json!(ieo_spec::spec_digest(&theirs));
    let (st, b) = e.post_k("carol", "/v0/sprints", "promote-t5-01", &theirs);
    assert_eq!((st, code(&b)), (409, "SPRINT_REPO_CONFLICT".into()), "{b}");
}

#[test]
fn a_binding_sprint_id_is_reserved_against_imports() {
    let Some(e) = start() else { return };
    // T3/Y1: HUM-01 is repo-a's binding sprint and does not exist yet
    let (st, b) = e.post(
        "importer",
        "/v0/imports/notion",
        &notion_import("HUM-01", 'a', json!([])),
    );
    assert_eq!(
        (st, code(&b)),
        (409, "EVENT_CONTENT_CONFLICT".into()),
        "{b}"
    );
}

#[test]
fn only_an_executable_reviewed_newer_revision_sets_a_pinned_ceiling() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 0);
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    // T3/Y3: executable but not reviewed (ceiling 5) → ignored
    let s6 = spec("repo-a", 6, 5);
    assert_eq!(
        e.post_k("alice", "/v0/sprints?mode=executable", "reg-key-y3", &s6)
            .0,
        201
    );
    // T3/Y4: reviewed but draft-only (ceiling 5) → ignored
    let s7 = spec("repo-a", 7, 5);
    assert_eq!(
        e.post_k("alice", "/v0/sprints?mode=draft", "reg-key-y4", &s7)
            .0,
        201
    );
    assert_eq!(
        e.post(
            "bob",
            "/v0/sprints/POC-01/revisions/7/semantic-review",
            &json!({"decision": "passed", "reason": "ok", "digest": s7["digest"]})
        )
        .0,
        201
    );
    let (st, b) = assign(
        &e,
        "development",
        "A1",
        1,
        "01-01",
        SHA2,
        json!({"assignment_id": "A1", "generation": 0}),
    );
    assert_eq!(
        (st, b["outcome"].clone()),
        (202, json!("limit_reached")),
        "{b}"
    );
}

// --- X-01 follow-up C2: execution gate for imported tasks ---

fn notion_import_gated(test_id: &str, digest_char: char, gates: Value, missing: Value) -> Value {
    let mut b = notion_import(test_id, digest_char, missing);
    b["gates"] = gates;
    b
}

fn queue_gates() -> Value {
    json!([
        "Notion 큐 구획 '조건 대기': 현재 실행 대상이 아님",
        "Notion 차단 대상 미해제: 선행 A1"
    ])
}

fn exception(id: &str, test_id: &str, repo: &str) -> Value {
    json!({"exception_id": id, "test_id": test_id, "repo_id": repo, "lifts": "queue_gates",
           "scope": {"branch": format!("poc/{test_id}"), "purpose": "PoC 시험", "merge_to_default": false},
           "decision_ref": "User_Decisions_2026-11-13_x02.md ③"})
}

/// Register the derived spec executable, review it and assign it; returns the three statuses.
fn promote_review_assign(e: &Env, derived: &Value, key: &str) -> (u16, Value, u16, u16) {
    let (st1, b1) = e.post_k("alice", "/v0/sprints?mode=executable", key, derived);
    let rev = derived["revision"].as_i64().unwrap_or_default();
    let st2 = e
        .post(
            "bob",
            &format!(
                "/v0/sprints/{}/revisions/{rev}/semantic-review",
                derived["sprint_id"].as_str().unwrap_or_default()
            ),
            &json!({"decision": "passed", "reason": "ok", "digest": derived["digest"]}),
        )
        .0;
    let st3 = e.post("assigner", "/v0/coordinator/assignments",
        &json!({"repo_id": "repo-a", "role": "development", "assignment_id": format!("A-{key}"), "generation": 0,
                "sprint_id": derived["sprint_id"], "sprint_revision": rev, "task_id": "11-01", "target_sha": SHA1,
                "expected_current": null})).0;
    (st1, b1, st2, st3)
}

#[test]
fn c2_gated_import_cannot_execute_even_with_reasons_removed() {
    let Some(e) = start() else { return };
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &notion_import_gated("POCX-T01", 'a', queue_gates(), json!([]))
        )
        .0,
        201
    );
    // the registrar removes every reason and binds limits (the reproduced bypass)
    let derived = derived_executable(&e, "POCX-T01");
    assert_eq!(derived["unresolved_bindings"], json!([]));
    let before = e.counts();
    let (st, b) = e.post_k(
        "alice",
        "/v0/sprints?mode=executable",
        "c2-a-01-key",
        &derived,
    );
    assert_eq!((st, code(&b)), (409, "IMPORT_GATED".into()), "{b}");
    assert_eq!(e.counts(), before);
    // a draft registration of the same derived spec is still allowed (no execution)
    let mut draft = derived.clone();
    draft["registration_state"] = json!("draft");
    draft["digest"] = json!(ieo_spec::spec_digest(&draft));
    assert_eq!(
        e.post_k("alice", "/v0/sprints", "c2-a-02-key", &draft).0,
        201
    );
}

#[test]
fn c2_claimed_release_without_a_trusted_record_is_refused() {
    let Some(e) = start() else { return };
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &notion_import_gated("POCX-T01", 'a', queue_gates(), json!([]))
        )
        .0,
        201
    );
    // a registrar cannot write a "released" import record (only the import service can)
    let (st, b) = e.post(
        "alice",
        "/v0/imports/notion",
        &notion_import_gated("POCX-T01", 'b', json!([]), json!([])),
    );
    assert_eq!((st, code(&b)), (403, "SCOPE_DENIED".into()), "{b}");
    // a release claimed in editable spec text changes nothing
    let mut claimed = derived_executable(&e, "POCX-T01");
    claimed["title"] = json!("선행 해제됨(확인함) — 실행 가능");
    claimed["digest"] = json!(ieo_spec::spec_digest(&claimed));
    let (st, b) = e.post_k(
        "alice",
        "/v0/sprints?mode=executable",
        "c2-c-01-key",
        &claimed,
    );
    assert_eq!((st, code(&b)), (409, "IMPORT_GATED".into()), "{b}");
    // only a person can approve an exception
    let (st, _) = e.post(
        "importer",
        "/v0/coordinator/exceptions",
        &exception("EX-1", "POCX-T01", "repo-a"),
    );
    assert_eq!(st, 403);
    let (st, _) = e.post(
        "alice",
        "/v0/coordinator/exceptions",
        &exception("EX-1", "POCX-T01", "repo-a"),
    );
    assert_eq!(st, 403);
}

#[test]
fn c2_a_user_exception_applies_only_to_its_test_and_repo() {
    let Some(e) = start() else { return };
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &notion_import_gated("POCX-T01", 'a', queue_gates(), json!([]))
        )
        .0,
        201
    );
    let mut second = notion_import_gated("POCX-T02", 'e', queue_gates(), json!([]));
    second["source"]["page_id"] = json!("fedcba9876543210fedcba9876543210");
    second["source"]["url"] = json!("https://app.notion.com/p/fedcba9876543210fedcba9876543210");
    assert_eq!(e.post("importer", "/v0/imports/notion", &second).0, 201);
    // the exception for T02 does not unlock T01; one for another repo is refused
    assert_eq!(
        e.post(
            "uma",
            "/v0/coordinator/exceptions",
            &exception("EX-T02", "POCX-T02", "repo-a")
        )
        .0,
        201
    );
    assert_eq!(
        e.post(
            "uma",
            "/v0/coordinator/exceptions",
            &exception("EX-B", "POCX-T01", "repo-b")
        )
        .0,
        403
    );
    let (st, b) = e.post_k(
        "alice",
        "/v0/sprints?mode=executable",
        "c2-d-01-key",
        &derived_executable(&e, "POCX-T01"),
    );
    assert_eq!((st, code(&b)), (409, "IMPORT_GATED".into()), "{b}");
    // the exception for T01 unlocks T01 only: executable, reviewed, assigned
    assert_eq!(
        e.post(
            "uma",
            "/v0/coordinator/exceptions",
            &exception("EX-T01", "POCX-T01", "repo-a")
        )
        .0,
        201
    );
    assert_eq!(
        e.post(
            "uma",
            "/v0/coordinator/exceptions",
            &exception("EX-T01", "POCX-T01", "repo-a")
        )
        .0,
        200
    );
    let (st1, b1, st2, st3) =
        promote_review_assign(&e, &derived_executable(&e, "POCX-T01"), "c2-d-02-key");
    assert_eq!((st1, st2, st3), (201, 201, 201), "{b1}");
    // the imported record still carries its gates (the exception is a separate record)
    let mut db = e.db();
    let g: Value = db
        .query_one(
            "SELECT gates FROM notion_imports WHERE test_id = 'POCX-T01'",
            &[],
        )
        .expect("q")
        .get(0);
    assert_eq!(g, queue_gates());
}

#[test]
fn c2_an_exception_does_not_lift_missing_inputs() {
    let Some(e) = start() else { return };
    let missing = json!(["수용 기준(필수 인수 판정 표)을 원본에서 읽지 못함"]);
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &notion_import_gated("POCX-T01", 'a', queue_gates(), missing)
        )
        .0,
        201
    );
    assert_eq!(
        e.post(
            "uma",
            "/v0/coordinator/exceptions",
            &exception("EX-T01", "POCX-T01", "repo-a")
        )
        .0,
        201
    );
    let (st, b) = e.post_k(
        "alice",
        "/v0/sprints?mode=executable",
        "c2-e-01-key",
        &derived_executable(&e, "POCX-T01"),
    );
    assert_eq!((st, code(&b)), (409, "IMPORT_INCOMPLETE".into()), "{b}");
}

#[test]
fn c2_a_trusted_release_and_review_allow_execution() {
    let Some(e) = start() else { return };
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &notion_import_gated("POCX-T01", 'a', queue_gates(), json!([]))
        )
        .0,
        201
    );
    // the import service records the released state (new content, no gates) → revision 2
    let (st, r) = e.post(
        "importer",
        "/v0/imports/notion",
        &notion_import_gated("POCX-T01", 'b', json!([]), json!([])),
    );
    assert_eq!(
        (st, r["registration"]["revision"].clone()),
        (201, json!(2)),
        "{r}"
    );
    let rev = e.get("alice", "/v0/sprints/POCX-T01/revisions/2").1;
    let mut d = rev["spec"].clone();
    d["revision"] = json!(3);
    d["registration_state"] = json!("ready");
    d["target"]["repository_url"] = json!("https://example.invalid/repo-a");
    d["limits"] = json!({"elapsed_seconds": 3600, "total_tokens": 100000, "correction_attempts": 1, "unresolved_reason": null});
    d["unresolved_bindings"] = json!([]);
    d["digest"] = json!(ieo_spec::spec_digest(&d));
    let (st1, b1, st2, st3) = promote_review_assign(&e, &d, "c2-f-01-key");
    assert_eq!((st1, st2, st3), (201, 201, 201), "{b1}");
}

// --- review 9a421b7 corrections ---

fn ready_from(e: &Env, sprint_id: &str, from_rev: i64, new_rev: i64) -> Value {
    let rev = e
        .get(
            "alice",
            &format!("/v0/sprints/{sprint_id}/revisions/{from_rev}"),
        )
        .1;
    let mut d = rev["spec"].clone();
    d["revision"] = json!(new_rev);
    d["registration_state"] = json!("ready");
    d["target"]["repository_url"] = json!("https://example.invalid/repo-a");
    d["limits"] = json!({"elapsed_seconds": 3600, "total_tokens": 100000, "correction_attempts": 1,
                         "unresolved_reason": null});
    d["unresolved_bindings"] = json!([]);
    d["digest"] = json!(ieo_spec::spec_digest(&d));
    d
}

#[test]
fn c2_a_copy_under_another_sprint_id_is_still_gated() {
    let Some(e) = start() else { return };
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &notion_import_gated("POCX-T01", 'a', queue_gates(), json!([]))
        )
        .0,
        201
    );
    // F1: same tasks and the gated notion source, under another sprint id and task id
    let mut copy = derived_executable(&e, "POCX-T01");
    copy["sprint_id"] = json!("POCX-Z02");
    copy["revision"] = json!(1);
    copy["tasks"][0]["task_id"] = json!("13-01");
    copy["digest"] = json!(ieo_spec::spec_digest(&copy));
    let before = e.counts();
    let (st, b) = e.post_k("alice", "/v0/sprints?mode=executable", "f1-copy-key", &copy);
    assert_eq!((st, code(&b)), (409, "IMPORT_GATED".into()), "{b}");
    assert_eq!(e.counts(), before);
    // an exception for the import's own test sprint does not cover the copy
    assert_eq!(
        e.post(
            "uma",
            "/v0/coordinator/exceptions",
            &exception("EX-T01", "POCX-T01", "repo-a")
        )
        .0,
        201
    );
    let (st, b) = e.post_k(
        "alice",
        "/v0/sprints?mode=executable",
        "f1-copy-key2",
        &copy,
    );
    assert_eq!((st, code(&b)), (409, "IMPORT_GATED".into()), "{b}");
}

#[test]
fn c2_content_returning_to_a_gated_state_gates_again() {
    let Some(e) = start() else { return };
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &notion_import_gated("POCX-T01", 'a', queue_gates(), json!([]))
        )
        .0,
        201
    );
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &notion_import_gated("POCX-T01", 'b', json!([]), json!([]))
        )
        .0,
        201
    );
    // F2: back to the gated content A → new revision 3, gated again
    let (st, r) = e.post(
        "importer",
        "/v0/imports/notion",
        &notion_import_gated("POCX-T01", 'a', queue_gates(), json!([])),
    );
    assert_eq!(
        (st, r["registration"]["revision"].clone()),
        (201, json!(3)),
        "{r}"
    );
    let d = ready_from(&e, "POCX-T01", 3, 4);
    let (st, b) = e.post_k("alice", "/v0/sprints?mode=executable", "f2-regate-key", &d);
    assert_eq!((st, code(&b)), (409, "IMPORT_GATED".into()), "{b}");
}

#[test]
fn c2_an_exception_covers_only_the_content_it_approved() {
    let Some(e) = start() else { return };
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &notion_import_gated("POCX-T01", 'a', queue_gates(), json!([]))
        )
        .0,
        201
    );
    assert_eq!(
        e.post(
            "uma",
            "/v0/coordinator/exceptions",
            &exception("EX-A", "POCX-T01", "repo-a")
        )
        .0,
        201
    );
    // F4: a later import with different, still-gated content needs a new approval
    let gated_b2 = json!(["Notion 차단 대상 미해제: 새 선행 B2"]);
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &notion_import_gated("POCX-T01", 'c', gated_b2, json!([]))
        )
        .0,
        201
    );
    let d = ready_from(&e, "POCX-T01", 2, 3);
    let (st, b) = e.post_k(
        "alice",
        "/v0/sprints?mode=executable",
        "f4-new-gate-key",
        &d,
    );
    assert_eq!((st, code(&b)), (409, "IMPORT_GATED".into()), "{b}");
    assert_eq!(
        e.post(
            "uma",
            "/v0/coordinator/exceptions",
            &exception("EX-C", "POCX-T01", "repo-a")
        )
        .0,
        201
    );
    let (st1, b1, st2, st3) = promote_review_assign(&e, &d, "f4-after-key");
    assert_eq!((st1, st2, st3), (201, 201, 201), "{b1}");
}

#[test]
fn c2_a_re_gating_import_stops_a_running_assignment() {
    let Some(e) = start() else { return };
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &notion_import_gated("POCX-T01", 'a', queue_gates(), json!([]))
        )
        .0,
        201
    );
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &notion_import_gated("POCX-T01", 'b', json!([]), json!([]))
        )
        .0,
        201
    );
    let d = ready_from(&e, "POCX-T01", 2, 3);
    let (st1, b1, st2, st3) = promote_review_assign(&e, &d, "regate-run-key");
    assert_eq!((st1, st2, st3), (201, 201, 201), "{b1}");
    // the Notion task becomes gated again → the running assignment's next generation is refused
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &notion_import_gated("POCX-T01", 'd', queue_gates(), json!([]))
        )
        .0,
        201
    );
    let (st, b) = e.post("assigner", "/v0/coordinator/assignments",
        &json!({"repo_id": "repo-a", "role": "development", "assignment_id": "A-regate-run-key", "generation": 1,
                "sprint_id": "POCX-T01", "sprint_revision": 3, "task_id": "11-01", "target_sha": SHA2,
                "expected_current": {"assignment_id": "A-regate-run-key", "generation": 0}}));
    assert_eq!((st, code(&b)), (409, "IMPORT_GATED".into()), "{b}");
}

// --- review 7afd4d1 G1–G4 ---

type SourceEdit = Box<dyn Fn(&mut Value)>;

fn on_page(mut b: Value, page: &str) -> Value {
    b["source"]["page_id"] = json!(page);
    b["source"]["url"] = json!(format!("https://app.notion.com/p/Synthetic-Task-{page}"));
    b
}

/// A copy of POCX-T01's derived spec under another sprint and task id, with `edit` applied to its
/// Notion source.
fn copy_of_t01(e: &Env, sprint: &str, edit: impl Fn(&mut Value)) -> Value {
    let mut copy = derived_executable(e, "POCX-T01");
    copy["sprint_id"] = json!(sprint);
    copy["revision"] = json!(1);
    copy["tasks"][0]["task_id"] = json!("13-01");
    for s in copy["sources"].as_array_mut().into_iter().flatten() {
        if s["kind"] == "notion_page" {
            edit(s);
        }
    }
    copy["digest"] = json!(ieo_spec::spec_digest(&copy));
    copy
}

#[test]
fn g1_a_copy_with_a_relabelled_kind_or_another_digest_is_still_gated() {
    let Some(e) = start() else { return };
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &notion_import_gated("POCX-T01", 'a', queue_gates(), json!([]))
        )
        .0,
        201
    );
    let variants: [(&str, SourceEdit); 4] = [
        ("relabelled kind", Box::new(|s| s["kind"] = json!("notion"))),
        (
            "other digest",
            Box::new(|s| s["digest"] = json!("f".repeat(64))),
        ),
        (
            "both, dashed id in another field",
            Box::new(|s| {
                s["kind"] = json!("document");
                s["digest"] = json!("f".repeat(64));
                s["ref"] = json!("fixture://moved");
                s["revision"] = json!("01234567-89ab-cdef-0123-456789abcdef");
            }),
        ),
        (
            "reference dropped, digest kept",
            Box::new(|s| {
                s["kind"] = json!("document");
                s["ref"] = json!("fixture://moved");
            }),
        ),
    ];
    for (i, (name, edit)) in variants.iter().enumerate() {
        let copy = copy_of_t01(&e, &format!("POCX-Z0{i}"), edit);
        let before = e.counts();
        let (st, b) = e.post_k(
            "alice",
            "/v0/sprints?mode=executable",
            &format!("g1-copy-key-{i}"),
            &copy,
        );
        assert_eq!((st, code(&b)), (409, "IMPORT_GATED".into()), "{name}: {b}");
        assert_eq!(e.counts(), before, "{name}");
    }
    // documented residual (REPORT §2.3): a copy that drops both the page reference and the
    // digest is not recognised by the server; the registrar and the semantic review own it
    let copy = copy_of_t01(&e, "POCX-Z09", |s| {
        s["kind"] = json!("document");
        s["ref"] = json!("fixture://moved");
        s["digest"] = json!("f".repeat(64));
    });
    let (st, b) = e.post_k(
        "alice",
        "/v0/sprints?mode=executable",
        "g1-residual-key",
        &copy,
    );
    assert_eq!(st, 201, "{b}");
}

#[test]
fn g2_identical_content_on_two_pages_is_gated_and_excepted_per_page() {
    let Some(e) = start() else { return };
    const Q: &str = "fedcba9876543210fedcba9876543210";
    // P (POCX-T01) and Q (POCX-T02) with the same gated content digest
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &notion_import_gated("POCX-T01", 'a', queue_gates(), json!([]))
        )
        .0,
        201
    );
    let (st, r) = e.post(
        "importer",
        "/v0/imports/notion",
        &on_page(
            notion_import_gated("POCX-T02", 'a', queue_gates(), json!([])),
            Q,
        ),
    );
    assert_eq!(st, 201, "{r}");
    // T02's own exception covers T02 (no over-block through page P's digest)
    let (st, x) = e.post(
        "uma",
        "/v0/coordinator/exceptions",
        &exception("EX-T02", "POCX-T02", "repo-a"),
    );
    assert_eq!((st, x["approved_page_id"].clone()), (201, json!(Q)), "{x}");
    let d2 = derived_executable(&e, "POCX-T02");
    let (st, b) = e.post_k("alice", "/v0/sprints?mode=executable", "g2-t02-key", &d2);
    assert_eq!(st, 201, "{b}");
    // Q is released; P stays gated and T02's exception never covers P
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &on_page(
                notion_import_gated("POCX-T02", 'c', json!([]), json!([])),
                Q
            )
        )
        .0,
        201
    );
    let d1 = derived_executable(&e, "POCX-T01");
    let (st, b) = e.post_k("alice", "/v0/sprints?mode=executable", "g2-t01-key", &d1);
    assert_eq!((st, code(&b)), (409, "IMPORT_GATED".into()), "{b}");
}

#[test]
fn g3_a_page_belongs_to_one_test_id() {
    let Some(e) = start() else { return };
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &notion_import("POCX-T01", 'a', json!([]))
        )
        .0,
        201
    );
    for (digest, key) in [('a', "same content"), ('b', "new content")] {
        let before = e.counts();
        let (st, b) = e.post(
            "importer",
            "/v0/imports/notion",
            &notion_import("POCX-T09", digest, json!([])),
        );
        assert_eq!(
            (st, code(&b)),
            (409, "EVENT_CONTENT_CONFLICT".into()),
            "{key}: {b}"
        );
        assert_eq!(e.counts(), before, "{key}");
    }
}

#[test]
fn g4_migration_from_earlier_keys_keeps_a_page_imported_under_two_test_ids() {
    let Some(e) = start() else { return };
    const Q: &str = "fedcba9876543210fedcba9876543210";
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &notion_import("POCX-T01", 'a', json!([]))
        )
        .0,
        201
    );
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &on_page(notion_import("POCX-T02", 'b', json!([])), Q)
        )
        .0,
        201
    );
    let mut db = e.db();
    // the state an earlier schema allowed: one page under two test ids (both revision 1)
    for old_pk in ["page_id, content_digest", "test_id, sprint_revision"] {
        db.batch_execute(&format!(
            "ALTER TABLE notion_imports DISABLE TRIGGER notion_imports_append_only;
             ALTER TABLE notion_imports DROP CONSTRAINT notion_imports_pkey;
             UPDATE notion_imports SET page_id = '0123456789abcdef0123456789abcdef';
             ALTER TABLE notion_imports ADD PRIMARY KEY ({old_pk});
             ALTER TABLE notion_imports ENABLE TRIGGER notion_imports_append_only;"
        ))
        .expect("old schema");
        migrate(&mut db).expect("migrate");
        migrate(&mut db).expect("migrate again");
        let cols: Vec<String> = db
            .query(
                "SELECT a.attname::text FROM pg_constraint c JOIN pg_attribute a
                   ON a.attrelid = c.conrelid AND a.attnum = ANY (c.conkey)
                 WHERE c.conname = 'notion_imports_pkey' ORDER BY a.attname",
                &[],
            )
            .expect("pk")
            .iter()
            .map(|r| r.get(0))
            .collect();
        assert_eq!(cols, vec!["sprint_revision", "test_id"], "from ({old_pk})");
    }
    // the 7afd4d1 key (page_id, sprint_revision) on data it allows migrates too
    db.batch_execute(
        "ALTER TABLE notion_imports DISABLE TRIGGER notion_imports_append_only;
         DELETE FROM notion_imports WHERE test_id = 'POCX-T02';
         ALTER TABLE notion_imports DROP CONSTRAINT notion_imports_pkey;
         ALTER TABLE notion_imports ADD PRIMARY KEY (page_id, sprint_revision);
         ALTER TABLE notion_imports ENABLE TRIGGER notion_imports_append_only;",
    )
    .expect("7afd4d1 schema");
    migrate(&mut db).expect("migrate");
    let n: i64 = db
        .query_one(
            "SELECT count(*) FROM pg_constraint c JOIN pg_attribute a
               ON a.attrelid = c.conrelid AND a.attnum = ANY (c.conkey)
             WHERE c.conname = 'notion_imports_pkey' AND a.attname = 'page_id'",
            &[],
        )
        .expect("pk")
        .get(0);
    assert_eq!(n, 0);
}

// --- review e908a8c H1–H3 ---

#[test]
fn h1_a_reference_rewritten_to_another_imported_page_does_not_hide_the_cited_content() {
    let Some(e) = start() else { return };
    const Q: &str = "fedcba9876543210fedcba9876543210";
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &notion_import_gated("POCX-T01", 'a', queue_gates(), json!([]))
        )
        .0,
        201
    );
    // another page, released (no gates), with its own content
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &on_page(
                notion_import_gated("POCX-T02", 'b', json!([]), json!([])),
                Q
            )
        )
        .0,
        201
    );
    let q_url = format!("https://app.notion.com/p/Synthetic-Task-{Q}");
    // the reference now names Q, the cited content is still P's gated import
    let copy = copy_of_t01(&e, "POCX-Z10", |s| s["ref"] = json!(q_url));
    let before = e.counts();
    let (st, b) = e.post_k("alice", "/v0/sprints?mode=executable", "h1-copy-key", &copy);
    assert_eq!((st, code(&b)), (409, "IMPORT_GATED".into()), "{b}");
    assert_eq!(e.counts(), before);
    // documented residual (REPORT §2.3): reference and digest both rewritten to Q's import
    let copy = copy_of_t01(&e, "POCX-Z11", |s| {
        s["ref"] = json!(q_url);
        s["digest"] = json!("b".repeat(64));
    });
    let (st, b) = e.post_k(
        "alice",
        "/v0/sprints?mode=executable",
        "h1-residual-key",
        &copy,
    );
    assert_eq!(st, 201, "{b}");
}

#[test]
fn h2_every_reader_takes_the_same_latest_import() {
    let Some(e) = start() else { return };
    for (digest, gates) in [('a', queue_gates()), ('b', json!([])), ('c', queue_gates())] {
        assert_eq!(
            e.post(
                "importer",
                "/v0/imports/notion",
                &notion_import_gated("POCX-T01", digest, gates, json!([]))
            )
            .0,
            201
        );
    }
    let mut db = e.db();
    // new rows are stamped after the registry lock: import order = revision order
    let order: Vec<i64> = db
        .query(
            "SELECT sprint_revision FROM notion_imports WHERE test_id = 'POCX-T01' ORDER BY imported_at",
            &[],
        )
        .expect("order")
        .iter()
        .map(|r| r.get(0))
        .collect();
    assert_eq!(order, vec![1, 2, 3]);
    // a clock that went backwards: revision 3 stamped before revision 2. Every reader must still
    // take revision 3 (review f0c4e87 J2: revision first, never an older release).
    db.batch_execute(
        "ALTER TABLE notion_imports DISABLE TRIGGER notion_imports_append_only;
         UPDATE notion_imports SET imported_at = (SELECT imported_at FROM notion_imports
           WHERE test_id = 'POCX-T01' AND sprint_revision = 2) - interval '1 millisecond'
         WHERE test_id = 'POCX-T01' AND sprint_revision = 3;
         ALTER TABLE notion_imports ENABLE TRIGGER notion_imports_append_only;",
    )
    .expect("invert");
    // revision 3 is gated: its derivation is refused
    let from3 = ready_from(&e, "POCX-T01", 3, 4);
    let (st, b) = e.post_k(
        "alice",
        "/v0/sprints?mode=executable",
        "h2-from3-key",
        &from3,
    );
    assert_ne!(st, 201, "{b}");
    // a spec derived from the older release (revision 2) is no longer the latest derivation:
    // refused as stale before the gate (revision 3 is the latest import, whatever the clock says)
    let from2 = ready_from(&e, "POCX-T01", 2, 4);
    let (st, b) = e.post_k(
        "alice",
        "/v0/sprints?mode=executable",
        "h2-from2-key",
        &from2,
    );
    assert_eq!(
        (st, code(&b)),
        (409, "EVENT_CONTENT_CONFLICT".into()),
        "{b}"
    );
    // the exception binds the same import the gate reads (revision 3's content)
    let (st, x) = e.post(
        "uma",
        "/v0/coordinator/exceptions",
        &exception("EX-H2", "POCX-T01", "repo-a"),
    );
    assert_eq!(
        (st, x["approved_content_digest"].clone()),
        (201, json!("c".repeat(64))),
        "{x}"
    );
}

#[test]
fn h3_an_exception_recorded_without_a_page_never_matches() {
    let Some(e) = start() else { return };
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &notion_import_gated("POCX-T01", 'a', queue_gates(), json!([]))
        )
        .0,
        201
    );
    // a row as recorded before correction 2 (no page_id), otherwise matching in every field
    e.db()
        .execute(
            "INSERT INTO exec_exceptions (exception_id, test_id, repo_id, lifts, scope, decision_ref,
                request, approved_by, content_digest, page_id)
             VALUES ('EX-LEGACY', 'POCX-T01', 'repo-a', 'queue_gates', '{}', 'legacy', '{}', 'uma', $1, NULL)",
            &[&"a".repeat(64)],
        )
        .expect("legacy exception");
    let d = derived_executable(&e, "POCX-T01");
    let (st, b) = e.post_k("alice", "/v0/sprints?mode=executable", "h3-legacy-key", &d);
    assert_eq!((st, code(&b)), (409, "IMPORT_GATED".into()), "{b}");
}

// --- review aa38041 I1–I2 ---

#[test]
fn i1_only_the_named_pages_latest_content_hides_the_cited_digest() {
    let Some(e) = start() else { return };
    const Q: &str = "fedcba9876543210fedcba9876543210";
    // P (T01) gated with content a; Q (T02) once had the same content a, then was released as c
    for (body, why) in [
        (
            notion_import_gated("POCX-T01", 'a', queue_gates(), json!([])),
            "P",
        ),
        (
            on_page(
                notion_import_gated("POCX-T02", 'a', queue_gates(), json!([])),
                Q,
            ),
            "Q gated a",
        ),
        (
            on_page(
                notion_import_gated("POCX-T02", 'c', json!([]), json!([])),
                Q,
            ),
            "Q released c",
        ),
    ] {
        assert_eq!(
            e.post("importer", "/v0/imports/notion", &body).0,
            201,
            "{why}"
        );
    }
    let q_url = format!("https://app.notion.com/p/Synthetic-Task-{Q}");
    let copy = copy_of_t01(&e, "POCX-Z12", |s| s["ref"] = json!(q_url));
    let before = e.counts();
    let (st, b) = e.post_k("alice", "/v0/sprints?mode=executable", "i1-copy-key", &copy);
    assert_eq!((st, code(&b)), (409, "IMPORT_GATED".into()), "{b}");
    assert_eq!(e.counts(), before);
}

/// Imports P as POCX-T01: revision 1 released (content b), revision 2 gated (content a); then
/// stamps revision 2 before revision 1 (a clock that went backwards), so by import time the
/// released revision 1 looks newer. Readers must take revision 2 (review f0c4e87 J2).
fn inverted_latest(e: &Env) {
    for (digest, gates) in [('b', json!([])), ('a', queue_gates())] {
        assert_eq!(
            e.post(
                "importer",
                "/v0/imports/notion",
                &notion_import_gated("POCX-T01", digest, gates, json!([]))
            )
            .0,
            201
        );
    }
    e.db()
        .batch_execute(
            "ALTER TABLE notion_imports DISABLE TRIGGER notion_imports_append_only;
             UPDATE notion_imports SET imported_at = (SELECT imported_at FROM notion_imports
               WHERE test_id = 'POCX-T01' AND sprint_revision = 1) - interval '1 millisecond'
             WHERE test_id = 'POCX-T01' AND sprint_revision = 2;
             ALTER TABLE notion_imports ENABLE TRIGGER notion_imports_append_only;",
        )
        .expect("invert");
}

#[test]
fn i2_the_gate_reads_the_highest_revision_whatever_the_clock() {
    let Some(e) = start() else { return };
    inverted_latest(&e);
    // a copy citing P's page: the gate must read revision 2 (gated), not the later-stamped revision 1 (released)
    let mut copy = ready_from(&e, "POCX-T01", 2, 1);
    copy["sprint_id"] = json!("POCX-Z13");
    copy["tasks"][0]["task_id"] = json!("13-01");
    copy["digest"] = json!(ieo_spec::spec_digest(&copy));
    let (st, b) = e.post_k("alice", "/v0/sprints?mode=executable", "i2-gate-key", &copy);
    assert_eq!((st, code(&b)), (409, "IMPORT_GATED".into()), "{b}");
}

#[test]
fn i2_a_replay_matches_the_highest_revision_whatever_the_clock() {
    let Some(e) = start() else { return };
    inverted_latest(&e);
    // content a is the highest revision: a replay, not a new revision
    let (st, r) = e.post(
        "importer",
        "/v0/imports/notion",
        &notion_import_gated("POCX-T01", 'a', queue_gates(), json!([])),
    );
    assert_eq!((st, r["replay"].clone()), (200, json!(true)), "{r}");
}

#[test]
fn i2_an_import_is_stamped_after_the_registry_lock() {
    let Some(e) = start() else { return };
    let mut holder = e.db();
    let mut tx = holder.transaction().expect("tx");
    tx.execute(
        "SELECT pg_advisory_xact_lock(hashtextextended('registry', 0))",
        &[],
    )
    .expect("hold registry lock");
    std::thread::scope(|sc| {
        let req = sc.spawn(|| {
            e.post(
                "importer",
                "/v0/imports/notion",
                &notion_import("POCX-T01", 'a', json!([])),
            )
            .0
        });
        // wait until the import's transaction is actually queued on the lock (review f0c4e87 J3)
        let mut watcher = e.db();
        let t0 = std::time::Instant::now();
        while watcher
            .query_one(
                "SELECT count(*) FROM pg_locks WHERE locktype = 'advisory' AND NOT granted
                   AND database = (SELECT oid FROM pg_database WHERE datname = current_database())",
                &[],
            )
            .expect("pg_locks")
            .get::<_, i64>(0)
            == 0
        {
            assert!(
                t0.elapsed().as_secs() < 10,
                "the import never waited on the lock"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let released_at: std::time::SystemTime = tx
            .query_one("SELECT clock_timestamp()", &[])
            .expect("clock")
            .get(0);
        tx.commit().expect("release");
        assert_eq!(req.join().expect("join"), 201);
        let stamped: std::time::SystemTime = e
            .db()
            .query_one(
                "SELECT imported_at FROM notion_imports WHERE test_id = 'POCX-T01'",
                &[],
            )
            .expect("row")
            .get(0);
        assert!(
            stamped >= released_at,
            "imported_at precedes the lock release: stamped at transaction start"
        );
    });
}

// --- review 6352413 K1 ---

#[test]
fn k1_a_page_imported_under_two_test_ids_fails_closed() {
    let Some(e) = start() else { return };
    const Q: &str = "fedcba9876543210fedcba9876543210";
    // T01 on P: gated then released; T02 on Q: gated (both allowed today)
    for (body, why) in [
        (
            notion_import_gated("POCX-T01", 'a', queue_gates(), json!([])),
            "P gated",
        ),
        (
            notion_import_gated("POCX-T01", 'b', json!([]), json!([])),
            "P released",
        ),
        (
            on_page(
                notion_import_gated("POCX-T02", 'c', queue_gates(), json!([])),
                Q,
            ),
            "Q gated",
        ),
    ] {
        assert_eq!(
            e.post("importer", "/v0/imports/notion", &body).0,
            201,
            "{why}"
        );
    }
    // legacy state (allowed before a page was bound to one test id): Q's row relabelled to P
    e.db()
        .batch_execute(&format!(
            "ALTER TABLE notion_imports DISABLE TRIGGER notion_imports_append_only;
             UPDATE notion_imports SET page_id = '0123456789abcdef0123456789abcdef' WHERE page_id = '{Q}';
             ALTER TABLE notion_imports ENABLE TRIGGER notion_imports_append_only;"
        ))
        .expect("legacy two test ids");
    // revision numbers of two test ids cannot be compared: the page is refused, never released
    let before = e.counts();
    for (sprint, key) in [("POCX-T01", "k1-t01-key"), ("POCX-T02", "k1-t02-key")] {
        let d = ready_from(&e, sprint, if sprint == "POCX-T01" { 2 } else { 1 }, 3);
        let (st, b) = e.post_k("alice", "/v0/sprints?mode=executable", key, &d);
        assert_eq!(
            (st, code(&b)),
            (409, "IMPORT_GATED".into()),
            "{sprint}: {b}"
        );
    }
    assert_eq!(e.counts(), before);
}

// --- review f571d1e L3 ---

#[test]
fn l3_page_ids_are_accepted_only_in_canonical_form() {
    let Some(e) = start() else { return };
    for (page, why) in [
        ("01234567-89ab-cdef-0123-456789abcdef", "dashed"),
        ("0123456789ABCDEF0123456789ABCDEF", "upper case"),
        ("0123456789abcdef0123456789abcde", "31 characters"),
        ("0123456789abcdef0123456789abcdeg", "not hex"),
    ] {
        let mut b = notion_import("POCX-T09", 'a', json!([]));
        b["source"]["page_id"] = json!(page);
        let before = e.counts();
        let (st, r) = e.post("importer", "/v0/imports/notion", &b);
        assert_eq!(
            (st, code(&r)),
            (400, "SCHEMA_VIOLATION".into()),
            "{why}: {r}"
        );
        assert_eq!(e.counts(), before, "{why}");
    }
    // the canonical form of the same page is accepted
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &notion_import("POCX-T09", 'a', json!([]))
        )
        .0,
        201
    );
}

// --- X-02 prep: the URL must name the imported page (user decision 2026-11-14 §5) ---

#[test]
fn an_import_whose_url_names_another_page_is_refused() {
    let Some(e) = start() else { return };
    for (url, why) in [
        (
            "https://app.notion.com/p/fedcba9876543210fedcba9876543210",
            "another page",
        ),
        (
            "https://app.notion.com/p/Task-fedcba9876543210fedcba9876543210",
            "another page with a title",
        ),
        ("https://app.notion.com/p/notion", "no page id"),
        (
            "https://app.notion.com/p/0123456789abcdef0123456789abcdef/sub",
            "page id not in the last segment",
        ),
    ] {
        let mut b = notion_import("POCX-T01", 'a', json!([]));
        b["source"]["url"] = json!(url);
        let before = e.counts();
        let (st, r) = e.post("importer", "/v0/imports/notion", &b);
        assert_eq!(
            (st, code(&r)),
            (400, "SCHEMA_VIOLATION".into()),
            "{why}: {r}"
        );
        assert_eq!(e.counts(), before, "{why}");
    }
    // the same page in the forms Notion uses is accepted
    for (i, url) in [
        "https://app.notion.com/p/0123456789abcdef0123456789abcdef",
        "https://www.notion.so/Synthetic-Task-0123456789abcdef0123456789abcdef?pvs=4",
        "https://www.notion.so/01234567-89ab-cdef-0123-456789abcdef#section",
        "https://www.notion.so/my-workspace/Task-0123456789abcdef0123456789abcdef",
        "https://www.notion.so/0123456789ABCDEF0123456789ABCDEF",
    ]
    .iter()
    .enumerate()
    {
        let mut b = notion_import("POCX-T01", 'a', json!([]));
        b["source"]["url"] = json!(url);
        let (st, r) = e.post("importer", "/v0/imports/notion", &b);
        assert!(st == 201 || (i > 0 && st == 200), "{url}: {st} {r}");
    }
}

#[test]
fn repo_scope_role_control_needs_its_scope_and_reports_unreachable_roles() {
    let Some(env) = start() else { return };
    // without the scope, as an agent, or for another repo: refused
    let (st, b) = env.get("uma", "/v0/repos/repo-a/roles");
    assert_eq!((st, b["code"].as_str()), (403, Some("SCOPE_DENIED")), "{b}");
    let (st, _) = env.post("operator", "/v0/repos/repo-b/stop", &json!({}));
    assert_eq!(st, 403);
    // the operator: both roles reported, unreachable, and the shutdown not confirmed
    let (st, b) = env.post("operator", "/v0/repos/repo-a/stop", &json!({}));
    assert_eq!(st, 200, "{b}");
    assert_eq!(b["safe_shutdown_confirmed"], false, "{b}");
    assert_eq!(b["roles"]["development"]["reachable"], false, "{b}");
    assert_eq!(b["roles"]["guardrail"]["reachable"], false, "{b}");
    assert!(
        b["unconfirmed"].as_array().is_some_and(|u| u.len() == 2),
        "{b}"
    );
    let (st, b) = env.get("operator", "/v0/repos/repo-z/roles");
    assert_eq!(st, 403, "{b}");
    // a credential with the scope but no repos reaches no repo (review adf760e B1)
    let (st, b) = env.get("operator-norepo", "/v0/repos/repo-a/roles");
    assert_eq!((st, b["code"].as_str()), (403, Some("SCOPE_DENIED")), "{b}");
    let (st, _) = env.post("operator-norepo", "/v0/repos/repo-a/stop", &json!({}));
    assert_eq!(st, 403);
    // an agent credential is refused
    let (st, b) = env.post("dev-a", "/v0/repos/repo-a/stop", &json!({}));
    assert_eq!((st, b["code"].as_str()), (403, Some("SCOPE_DENIED")), "{b}");
}

// --- 02-04 decision item 9: the central checkpoint record ---

const CP: &str = "/v0/repos/repo-a/roles/development/checkpoints";

#[test]
fn central_checkpoints_keep_sequence_identity_and_tamper_reports() {
    let Some(e) = start() else { return };
    let s = ready_sprint(&e, 3);
    let digest = s["digest"].as_str().expect("digest").to_string();
    let report = |who: &str, seq: u64, raw: &str| {
        ensure_delivery(&e, raw.as_bytes());
        e.post(who, CP, &json!({"seq": seq, "checkpoint_json": raw}))
    };
    // no assignment yet
    let c1 = checkpoint(1, 0, &digest, json!({"phase": "takeover"}));
    let (st, b) = report("sup-dev-a", 1, &c1);
    assert_eq!((st, code(&b)), (409, "NO_CURRENT_ASSIGNMENT".into()), "{b}");
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    // credentials: the model's report credential, another role's supervisor, a client
    for (who, want) in [
        ("dev-a", "SCOPE_DENIED"),
        ("sup-gr-a", "ROLE_MISMATCH"),
        ("operator", "SCOPE_DENIED"),
    ] {
        let (st, b) = report(who, 1, &c1);
        assert_eq!((st, code(&b)), (403, want.into()), "{who}: {b}");
    }
    let (st, b) = e.get("sup-dev-a", &format!("{CP}/latest"));
    assert_eq!((st, code(&b)), (404, "NO_CHECKPOINT".into()), "{b}");
    // a gap before the first
    let (st, b) = report("sup-dev-a", 2, &checkpoint(2, 0, &digest, json!({})));
    assert_eq!(
        (st, code(&b)),
        (409, "CHECKPOINT_SEQUENCE_GAP".into()),
        "{b}"
    );
    // stored; the digest is the coordinator's own over the exact bytes
    let (st, b) = report("sup-dev-a", 1, &c1);
    assert_eq!(st, 201, "{b}");
    assert_eq!(
        b["checkpoint_sha256"],
        hex::encode(Sha256::digest(c1.as_bytes()))
    );
    // duplicate: same bytes → 200, nothing new; other bytes for the same seq → conflict
    let before = e.counts();
    let (st, b) = report("sup-dev-a", 1, &c1);
    assert_eq!((st, b["duplicate"].as_bool()), (200, Some(true)), "{b}");
    let (st, b) = report(
        "sup-dev-a",
        1,
        &checkpoint(1, 0, &digest, json!({"phase": "error"})),
    );
    assert_eq!(
        (st, code(&b)),
        (409, "EVENT_CONTENT_CONFLICT".into()),
        "{b}"
    );
    // identity against the current assignment: target SHA, spec digest, sprint revision,
    // generation, another assignment, another repo, a seq that is not the checkpoint's own
    for (extra, want) in [
        (json!({"target_sha": SHA2}), "ASSIGNMENT_SHA_MISMATCH"),
        (json!({"spec_digest": "0".repeat(64)}), "DIGEST_MISMATCH"),
        (json!({"sprint_revision": 4}), "CHECKPOINT_MISMATCH"),
        (json!({"generation": 5}), "CHECKPOINT_MISMATCH"),
        (json!({"assignment_id": "A9"}), "CHECKPOINT_MISMATCH"),
        (json!({"repo_id": "repo-b"}), "REPO_MISMATCH"),
        (json!({"seq": 3}), "SCHEMA_VIOLATION"),
        (json!({"phase": "running"}), "SCHEMA_VIOLATION"),
        (json!({"format": "other"}), "SCHEMA_VIOLATION"),
    ] {
        let (st, b) = report("sup-dev-a", 2, &checkpoint(2, 0, &digest, extra.clone()));
        assert_eq!(code(&b), want, "{extra}: {st} {b}");
    }
    assert_eq!(e.counts(), before, "refused reports leave no row");
    // a tamper detection travels with the new session's checkpoints; recorded once
    let tamper = json!({"config_tamper_detected": {"old_session_id": "s1", "new_session_id": "s2",
        "checkpoint_digest": "a".repeat(64), "observed_digest": "b".repeat(64)}, "session_id": "s2"});
    let mut t2 = tamper.clone();
    t2["phase"] = json!("takeover");
    let (st, b) = report("sup-dev-a", 2, &checkpoint(2, 0, &digest, t2));
    assert_eq!(
        (st, b["tamper_recorded"].as_bool()),
        (201, Some(true)),
        "{b}"
    );
    // latest says the seq-2 takeover's own report recorded the event (review 3724bc9 I-1)
    let (st, b) = e.get("operator", &format!("{CP}/latest"));
    assert_eq!(
        (st, b["seq"].as_u64(), b["tamper_recorded"].as_bool()),
        (200, Some(2), Some(true)),
        "{b}"
    );
    let (st, b) = report("sup-dev-a", 3, &checkpoint(3, 0, &digest, tamper.clone()));
    assert_eq!(
        (st, b["tamper_recorded"].as_bool()),
        (201, Some(false)),
        "{b}"
    );
    // a duplicate says whether the stored report of its sequence recorded the event; latest
    // says it for the latest checkpoint (review 681c1d3 L-1)
    let mut t2 = tamper.clone();
    t2["phase"] = json!("takeover");
    let (st, b) = report("sup-dev-a", 2, &checkpoint(2, 0, &digest, t2));
    assert_eq!(
        (st, b["duplicate"].as_bool(), b["tamper_recorded"].as_bool()),
        (200, Some(true), Some(true)),
        "{b}"
    );
    let (st, b) = report("sup-dev-a", 3, &checkpoint(3, 0, &digest, tamper.clone()));
    assert_eq!(
        (st, b["duplicate"].as_bool(), b["tamper_recorded"].as_bool()),
        (200, Some(true), Some(false)),
        "{b}"
    );
    let (st, b) = e.get("operator", &format!("{CP}/latest"));
    assert_eq!(
        (st, b["tamper_recorded"].as_bool()),
        (200, Some(false)),
        "{b}"
    );
    let mut bad = tamper.clone();
    bad["config_tamper_detected"]["new_session_id"] = json!("other");
    let (st, b) = report("sup-dev-a", 4, &checkpoint(4, 0, &digest, bad));
    assert_eq!((st, code(&b)), (400, "SCHEMA_VIOLATION".into()), "{b}");
    let (st, b) = e.get("operator", &format!("{CP}/latest"));
    assert_eq!(st, 200, "{b}");
    assert_eq!(
        (
            b["seq"].as_u64(),
            b["tamper_events"].as_i64(),
            b["session_id"].as_str()
        ),
        (Some(3), Some(1), Some("s2")),
        "{b}"
    );
    assert_eq!(e.get("operator-norepo", &format!("{CP}/latest")).0, 403);
    // a new generation of the assignment: the old one's checkpoints are stale
    let (st, b) = assign(
        &e,
        "development",
        "A1",
        1,
        "01-01",
        SHA1,
        json!({"assignment_id": "A1", "generation": 0}),
    );
    assert_eq!(st, 201, "{b}");
    let (st, b) = report(
        "sup-dev-a",
        4,
        &checkpoint(4, 0, &digest, json!({"session_id": "s2"})),
    );
    assert_eq!((st, code(&b)), (409, "STALE_ASSIGNMENT".into()), "{b}");
    assert_eq!(
        report(
            "sup-dev-a",
            4,
            &checkpoint(4, 1, &digest, json!({"session_id": "s2"}))
        )
        .0,
        201
    );
    // and the stored rows cannot be changed
    let mut db = e.db();
    assert!(
        db.execute(
            "UPDATE role_checkpoints SET phase = 'error' WHERE seq = 1",
            &[]
        )
        .is_err()
    );
    assert!(db.execute("DELETE FROM role_tamper_events", &[]).is_err());
}

#[test]
fn resume_judgment_compares_local_central_and_assignment() {
    use ieo_coord::api::judge_resume;
    let central = json!({"seq": 4, "checkpoint_sha256": "aa", "assignment_id": "A1", "generation": 2, "phase": "after_run"});
    let head = ("A1".to_string(), 2i64);
    let local = |seq: Value, sha: Value| json!({"running": null, "checkpoint_seq": seq, "checkpoint_sha256": sha});
    let d = |c: Option<&Value>, h: Option<&(String, i64)>, l: &Value| judge_resume(c, h, Ok(l)).0;
    assert_eq!(
        d(Some(&central), Some(&head), &local(json!(4), json!("aa"))),
        "resume_allowed"
    );
    assert_eq!(
        d(None, Some(&head), &local(Value::Null, Value::Null)),
        "no_checkpoint"
    );
    for l in [
        local(json!(4), json!("bb")),
        local(json!(3), json!("aa")),
        local(json!(5), json!("aa")),
        local(Value::Null, Value::Null),
    ] {
        assert_eq!(
            d(Some(&central), Some(&head), &l),
            "external_reconciliation_required",
            "{l}"
        );
    }
    assert_eq!(
        d(None, Some(&head), &local(json!(1), json!("aa"))),
        "external_reconciliation_required"
    );
    assert_eq!(
        d(
            Some(&central),
            Some(&("A1".into(), 3)),
            &local(json!(4), json!("aa"))
        ),
        "stale_generation"
    );
    assert_eq!(
        d(
            Some(&central),
            Some(&("A2".into(), 2)),
            &local(json!(4), json!("aa"))
        ),
        "external_reconciliation_required"
    );
    assert_eq!(
        d(Some(&central), None, &local(json!(4), json!("aa"))),
        "external_reconciliation_required"
    );
    let mut takeover = central.clone();
    takeover["phase"] = json!("takeover");
    assert_eq!(
        d(Some(&takeover), Some(&head), &local(json!(4), json!("aa"))),
        "external_reconciliation_required"
    );
    let mut running = local(json!(4), json!("aa"));
    running["running"] = json!({"attempt_id": "x"});
    assert_eq!(d(Some(&central), Some(&head), &running), "already_running");
    assert_eq!(
        judge_resume(Some(&central), Some(&head), Err(&"down".to_string())).0,
        "not_observed"
    );
}

// --- 02-05 step 1: U2 check order (X-01 review 74bd796 U2; PR #66 review 3c017a4 N-1, N-2) ---
//
// registerSprint: sprint repo (RG2, C2 reservation) → 01-07 checks → import ownership → import
// gate → store. Every case below was refused before 02-05 and is refused now; only the first code
// of a request that fails several checks may differ. Each refusal leaves every row count as it was.

/// A refused request: (status, first code); asserts that no row of any table changed.
fn refused(e: &Env, who: &str, path: &str, key: &str, body: &Value) -> (u16, String) {
    let before = e.counts();
    let (st, b) = e.post_k(who, path, key, body);
    // one line per case for the step-1 evidence log (`--nocapture`)
    eprintln!("U2-CASE {key} -> {st} {}", code(&b));
    assert!(st >= 400, "{b}");
    assert_eq!(e.counts(), before, "{b}");
    (st, code(&b))
}

fn redigest(mut s: Value) -> Value {
    s["digest"] = json!(ieo_spec::spec_digest(&s));
    s
}

/// POCX-T01 imported once (no gates) and its derived spec registered executable as revision 3;
/// returns that derived spec.
fn imported_with_derived_rev3(e: &Env) -> Value {
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &notion_import("POCX-T01", 'a', json!([]))
        )
        .0,
        201
    );
    let mut good = derived_executable(e, "POCX-T01");
    good["revision"] = json!(3);
    let good = redigest(good);
    let (st, b) = e.post_k("alice", "/v0/sprints?mode=executable", "u2-good-03", &good);
    assert_eq!(
        (st, b["state"].clone()),
        (201, json!("ACCEPTED_FOR_SEMANTIC_REVIEW")),
        "{b}"
    );
    good
}

/// `good` with other tasks (not derived from the import) at `revision`.
fn not_derived(good: &Value, revision: u64) -> Value {
    let mut s = good.clone();
    s["tasks"][0]["exit_condition"] = json!("다른 종료 조건");
    s["revision"] = json!(revision);
    redigest(s)
}

#[test]
fn u2_an_older_revision_of_an_imported_sprint_gets_the_revision_code_first() {
    let Some(e) = start() else { return };
    let good = imported_with_derived_rev3(&e);
    // (a) not derived, below the latest revision: was 409 EVENT_CONTENT_CONFLICT
    let stale = not_derived(&good, 2);
    assert_eq!(
        refused(
            &e,
            "alice",
            "/v0/sprints?mode=executable",
            "u2-a-0001",
            &stale
        ),
        (409, "REVISION_SUPERSEDED".into())
    );
    assert_eq!(
        refused(&e, "alice", "/v0/sprints", "u2-a-0002", &stale),
        (409, "REVISION_SUPERSEDED".into())
    );
    // (b) not derived, same number as a stored revision, other digest: was EVENT_CONTENT_CONFLICT
    let same_number = not_derived(&good, 3);
    assert_eq!(
        refused(
            &e,
            "alice",
            "/v0/sprints?mode=executable",
            "u2-b-0001",
            &same_number
        ),
        (409, "REVISION_CONFLICT".into())
    );
    // unchanged: not derived, a new revision that passes 01-07 → ownership 409
    let fresh = not_derived(&good, 4);
    assert_eq!(
        refused(
            &e,
            "alice",
            "/v0/sprints?mode=executable",
            "u2-o-0001",
            &fresh
        ),
        (409, "EVENT_CONTENT_CONFLICT".into())
    );
}

#[test]
fn u2_an_imported_sprint_with_a_01_07_error_gets_the_01_07_code_first() {
    let Some(e) = start() else { return };
    let good = imported_with_derived_rev3(&e);
    // (c) not derived and a wrong digest: was 409 EVENT_CONTENT_CONFLICT
    let mut wrong_digest = not_derived(&good, 4);
    wrong_digest["digest"] = json!("0".repeat(64));
    assert_eq!(
        refused(
            &e,
            "alice",
            "/v0/sprints?mode=executable",
            "u2-c-0001",
            &wrong_digest
        ),
        (422, "DIGEST_MISMATCH".into())
    );
    // (c) not derived and an undefined prerequisite: was 409 EVENT_CONTENT_CONFLICT
    let mut bad_ref = not_derived(&good, 4);
    bad_ref["tasks"][0]["prerequisites"] =
        json!([{"task_id": "99-99", "plan_version": "v1.3.3", "required_state": "accepted"}]);
    let bad_ref = redigest(bad_ref);
    assert_eq!(
        refused(
            &e,
            "alice",
            "/v0/sprints?mode=executable",
            "u2-c-0002",
            &bad_ref
        ),
        (422, "UNDEFINED_PREREQUISITE".into())
    );
    // (d) not derived and draft state asked for executable intake: was 409 EVENT_CONTENT_CONFLICT
    let mut unbound = not_derived(&good, 4);
    unbound["registration_state"] = json!("draft");
    let unbound = redigest(unbound);
    assert_eq!(
        refused(
            &e,
            "alice",
            "/v0/sprints?mode=executable",
            "u2-d-0001",
            &unbound
        ),
        (422, "UNBOUND_DRAFT".into())
    );
    // unchanged: a format error stays 400; another repo stays SPRINT_REPO_CONFLICT
    let mut malformed = not_derived(&good, 4);
    malformed.as_object_mut().map(|m| m.remove("title"));
    assert_eq!(
        refused(&e, "alice", "/v0/sprints", "u2-f-0001", &malformed).0,
        400
    );
    let mut theirs = not_derived(&good, 4);
    theirs["target"]["repo_id"] = json!("repo-b");
    let theirs = redigest(theirs);
    assert_eq!(
        refused(&e, "carol", "/v0/sprints", "u2-r-0001", &theirs),
        (409, "SPRINT_REPO_CONFLICT".into())
    );
}

#[test]
fn u2_ownership_covers_every_outcome_the_01_07_checks_accept() {
    let Some(e) = start() else { return };
    // two imports: revision 1 (content a), revision 2 (content b)
    for c in ['a', 'b'] {
        assert_eq!(
            e.post(
                "importer",
                "/v0/imports/notion",
                &notion_import("POCX-T01", c, json!([]))
            )
            .0,
            201
        );
    }
    let rev1 = e.get("alice", "/v0/sprints/POCX-T01/revisions/1").1["spec"].clone();
    let rev2 = e.get("alice", "/v0/sprints/POCX-T01/revisions/2").1["spec"].clone();
    // the earlier import's stored revision re-sent unchanged: 01-07 says Superseded (200), the
    // ownership guard still refuses it (PLAN §2.4 "위치"; was and stays 409, never 200)
    assert_eq!(
        refused(&e, "alice", "/v0/sprints", "u2-m1-0001", &rev1),
        (409, "EVENT_CONTENT_CONFLICT".into())
    );
    // the latest import's stored revision re-sent unchanged: AlreadyRegistered, 200
    let (st, b) = e.post_k("alice", "/v0/sprints", "u2-m1-0002", &rev2);
    assert_eq!(
        (st, b["state"].clone()),
        (200, json!("ALREADY_REGISTERED")),
        "{b}"
    );
    // a derived new revision: 201; the same again under a new key: 200
    let d = ready_from(&e, "POCX-T01", 2, 3);
    let (st, b) = e.post_k("alice", "/v0/sprints?mode=executable", "u2-m1-0003", &d);
    assert_eq!(st, 201, "{b}");
    let before = e.counts();
    let (st, b) = e.post_k("alice", "/v0/sprints?mode=executable", "u2-m1-0004", &d);
    assert_eq!(
        (st, b["state"].clone()),
        (200, json!("ALREADY_REGISTERED")),
        "{b}"
    );
    // a 200 without a write stores only the idempotency record
    let after = e.counts();
    let changed: Vec<_> = before
        .iter()
        .zip(&after)
        .filter(|(x, y)| x != y)
        .map(|(x, _)| x.0.clone())
        .collect();
    assert_eq!(changed, vec!["idempotency".to_string()]);
    // a newer import: the derived revision 3 re-sent is Superseded for 01-07 and no longer
    // derived from the latest import → 409, not 200
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &notion_import("POCX-T01", 'c', json!([]))
        )
        .0,
        201
    );
    assert_eq!(
        refused(&e, "alice", "/v0/sprints?mode=executable", "u2-m1-0005", &d),
        (409, "EVENT_CONTENT_CONFLICT".into())
    );
}

#[test]
fn u2_the_import_gate_follows_the_01_07_checks_and_the_repo_rule() {
    let Some(e) = start() else { return };
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &notion_import_gated("POCX-T01", 'a', queue_gates(), json!([]))
        )
        .0,
        201
    );
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &on_page(
                notion_import_gated("POCX-T02", 'e', json!([]), json!(["기한 미정"])),
                "fedcba9876543210fedcba9876543210"
            )
        )
        .0,
        201
    );
    let gated = derived_executable(&e, "POCX-T01");
    // unchanged: a derived spec without other errors → the gate's 409
    assert_eq!(
        refused(
            &e,
            "alice",
            "/v0/sprints?mode=executable",
            "u2-g-0001",
            &gated
        ),
        (409, "IMPORT_GATED".into())
    );
    // gate + wrong digest: was 409 IMPORT_GATED
    let mut wrong = gated.clone();
    wrong["digest"] = json!("0".repeat(64));
    assert_eq!(
        refused(
            &e,
            "alice",
            "/v0/sprints?mode=executable",
            "u2-g-0002",
            &wrong
        ),
        (422, "DIGEST_MISMATCH".into())
    );
    // missing facts + wrong digest: was 409 IMPORT_INCOMPLETE
    let mut incomplete = derived_executable(&e, "POCX-T02");
    assert_eq!(
        refused(
            &e,
            "alice",
            "/v0/sprints?mode=executable",
            "u2-g-0003",
            &incomplete
        ),
        (409, "IMPORT_INCOMPLETE".into())
    );
    incomplete["digest"] = json!("0".repeat(64));
    assert_eq!(
        refused(
            &e,
            "alice",
            "/v0/sprints?mode=executable",
            "u2-g-0004",
            &incomplete
        ),
        (422, "DIGEST_MISMATCH".into())
    );
    // gate + the sprint belongs to another repo (RG2): was 409 IMPORT_GATED
    let mut theirs = gated.clone();
    theirs["target"]["repo_id"] = json!("repo-b");
    theirs["target"]["repository_url"] = json!("https://example.invalid/repo-b");
    let theirs = redigest(theirs);
    assert_eq!(
        refused(
            &e,
            "carol",
            "/v0/sprints?mode=executable",
            "u2-g-0005",
            &theirs
        ),
        (409, "SPRINT_REPO_CONFLICT".into())
    );
    // gate + a sprint_id reserved for repo-a's binding (C2): was 409 IMPORT_GATED
    let mut reserved = theirs.clone();
    reserved["sprint_id"] = json!("HUM-01");
    reserved["revision"] = json!(1);
    reserved["tasks"][0]["task_id"] = json!("13-01");
    let reserved = redigest(reserved);
    assert_eq!(
        refused(
            &e,
            "carol",
            "/v0/sprints?mode=executable",
            "u2-g-0006",
            &reserved
        ),
        (409, "SPRINT_REPO_CONFLICT".into())
    );
}

#[test]
fn n2_a_re_gated_page_keeps_an_unchanged_executable_copy_refused() {
    // review 3c017a4 N-2: the gate covers AlreadyRegistered and Superseded too (409, never 200)
    let Some(e) = start() else { return };
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &notion_import_gated("POCX-T01", 'a', queue_gates(), json!([]))
        )
        .0,
        201
    );
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &notion_import_gated("POCX-T01", 'b', json!([]), json!([]))
        )
        .0,
        201
    );
    // a copy under another sprint id, registered executable while the page is released
    let mut copy = ready_from(&e, "POCX-T01", 2, 1);
    copy["sprint_id"] = json!("POCX-Z02");
    copy["tasks"][0]["task_id"] = json!("13-01");
    let copy = redigest(copy);
    assert_eq!(
        e.post_k("alice", "/v0/sprints?mode=executable", "n2-copy-01", &copy)
            .0,
        201
    );
    let mut copy2 = copy.clone();
    copy2["revision"] = json!(2);
    copy2["title"] = json!("사본 2");
    let copy2 = redigest(copy2);
    assert_eq!(
        e.post_k("alice", "/v0/sprints?mode=executable", "n2-copy-02", &copy2)
            .0,
        201
    );
    // while released, re-sending is a 200 lookup
    assert_eq!(
        e.post_k("alice", "/v0/sprints?mode=executable", "n2-copy-03", &copy2)
            .0,
        200
    );
    // the page is gated again
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &notion_import_gated("POCX-T01", 'd', queue_gates(), json!([]))
        )
        .0,
        201
    );
    // the latest copy unchanged (AlreadyRegistered for 01-07) → IMPORT_GATED
    assert_eq!(
        refused(
            &e,
            "alice",
            "/v0/sprints?mode=executable",
            "n2-copy-04",
            &copy2
        ),
        (409, "IMPORT_GATED".into())
    );
    // the older copy unchanged (Superseded for 01-07) → IMPORT_GATED
    assert_eq!(
        refused(
            &e,
            "alice",
            "/v0/sprints?mode=executable",
            "n2-copy-05",
            &copy
        ),
        (409, "IMPORT_GATED".into())
    );
    // an idempotent replay of the earlier request is answered first, as before (no new check)
    let (st, b) = e.post_k("alice", "/v0/sprints?mode=executable", "n2-copy-03", &copy2);
    assert_eq!(st, 200, "{b}");
}

// --- 02-05 step 1: C2 (02-03 review 0fa839d N2) — a binding reserves its sprint_id ---

#[test]
fn c2_a_binding_sprint_id_is_reserved_for_its_repo() {
    let Some(e) = start() else { return };
    // HUM-01 is repo-a's binding sprint and is not registered yet
    let mut theirs = spec("repo-b", 1, 3);
    theirs["sprint_id"] = json!("HUM-01");
    let theirs = redigest(theirs);
    assert_eq!(
        refused(&e, "carol", "/v0/sprints", "c2n2-0001", &theirs),
        (409, "SPRINT_REPO_CONFLICT".into())
    );
    assert_eq!(
        refused(
            &e,
            "carol",
            "/v0/sprints?mode=executable",
            "c2n2-0002",
            &theirs
        ),
        (409, "SPRINT_REPO_CONFLICT".into())
    );
    // a draft-state spec (registration_state draft, unbound limits) is reserved the same way
    // (PR #72 review 23877c1 I-2)
    let mut draft = theirs.clone();
    draft["registration_state"] = json!("draft");
    draft["limits"] = json!({"elapsed_seconds": null, "total_tokens": null,
        "correction_attempts": null, "unresolved_reason": "limits are not bound yet"});
    let draft = redigest(draft);
    assert_eq!(
        refused(&e, "carol", "/v0/sprints", "c2n2-0007", &draft),
        (409, "SPRINT_REPO_CONFLICT".into())
    );
    // checked before the format, like RG2: a malformed body naming it is also 409
    let mut malformed = theirs.clone();
    malformed.as_object_mut().map(|m| m.remove("title"));
    assert_eq!(
        refused(&e, "carol", "/v0/sprints", "c2n2-0003", &malformed),
        (409, "SPRINT_REPO_CONFLICT".into())
    );
    // nothing visible to repo-b afterwards
    assert_eq!(e.get("carol", "/v0/sprints/HUM-01/state").0, 404);
    // the binding's own repo registers it (registerSprint and the conversion path)
    let mut ours = spec("repo-a", 1, 3);
    ours["sprint_id"] = json!("HUM-01");
    let ours = redigest(ours);
    let (st, b) = e.post_k("alice", "/v0/sprints", "c2n2-0004", &ours);
    assert_eq!(st, 201, "{b}");
    let input = json!({"title": "설명용 예시", "repo": "repo-a", "goal": "설명용 목표",
        "out_of_scope": [], "decisions": [],
        "deadline": {"state": "none", "at": null, "reason": "기한 없음"},
        "tasks": [{"name": "검사 준비", "output": "시험 입력", "done_when": "입력 확정", "how_to_check": "목록 대조"}]});
    let (st, b) = e.post_k("alice", "/v0/sprint-inputs", "c2n2-0005", &input);
    assert_eq!(
        (
            st,
            b["registration"]["sprint_id"].clone(),
            b["registration"]["revision"].clone()
        ),
        (201, json!("HUM-01"), json!(2)),
        "{b}"
    );
    // once registered, RG2 answers the same for repo-b
    let mut later = spec("repo-b", 3, 3);
    later["sprint_id"] = json!("HUM-01");
    let later = redigest(later);
    assert_eq!(
        refused(&e, "carol", "/v0/sprints", "c2n2-0006", &later),
        (409, "SPRINT_REPO_CONFLICT".into())
    );
}

#[test]
fn c2_the_startup_check_refuses_a_binding_on_another_repos_sprint() {
    let Some(e) = start() else { return };
    let mut db = e.db();
    assert_eq!(
        ieo_coord::api::check_bindings(&mut db, &json!({"repo-a": {"sprint_id": "HUM-01"}})),
        Ok(())
    );
    ready_sprint(&e, 3); // POC-01 for repo-a
    // a binding of repo-b naming repo-a's sprint: RG2 and the reservation would contradict
    let err = ieo_coord::api::check_bindings(&mut db, &json!({"repo-b": {"sprint_id": "POC-01"}}))
        .expect_err("another repo's sprint");
    assert!(
        err.contains("registered for another repo (repo-b: POC-01, owned by repo-a)"),
        "{err}"
    );
    // the owner's own binding is fine
    assert_eq!(
        ieo_coord::api::check_bindings(&mut db, &json!({"repo-a": {"sprint_id": "POC-01"}})),
        Ok(())
    );
    // two bindings naming one sprint_id
    let err = ieo_coord::api::check_bindings(
        &mut db,
        &json!({"repo-a": {"sprint_id": "X-01"}, "repo-b": {"sprint_id": "X-01"}}),
    )
    .expect_err("two bindings");
    assert!(err.contains("named by two repos"), "{err}");
    // an imported test id (X-01 U3, unchanged)
    assert_eq!(
        e.post(
            "importer",
            "/v0/imports/notion",
            &notion_import("POCX-T01", 'a', json!([]))
        )
        .0,
        201
    );
    let err =
        ieo_coord::api::check_bindings(&mut db, &json!({"repo-a": {"sprint_id": "POCX-T01"}}))
            .expect_err("imported");
    assert!(
        err.contains("owned by a Notion import (repo-a: POCX-T01)"),
        "{err}"
    );
}

// --- 02-05 step 4: takeover ↔ delivery, first action, getAssignmentChain, claim separation ---

const CHAIN0: &str = "/v0/coordinator/assignments/A1/generations/0/chain";

/// Row counts of the tables a takeover or a first action can write (a refusal leaves them as
/// they were), the deliveries included.
fn step4_counts(e: &Env) -> Vec<(String, i64)> {
    let mut db = e.db();
    [
        "role_checkpoints",
        "role_takeovers",
        "role_first_actions",
        "role_tamper_events",
        "role_deliveries",
        "role_delivery_results",
        "history",
    ]
    .iter()
    .map(|t| {
        let n: i64 = db
            .query_one(&format!("SELECT count(*) FROM {t}"), &[])
            .expect("count")
            .get(0);
        (t.to_string(), n)
    })
    .collect()
}

/// A delivery as deliverAssignment records it (test fixture, no supervisor), with its results.
fn delivery_row(e: &Env, id: &str, generation: i64, outcomes: &[&str]) {
    let mut db = e.db();
    db.execute(
        "INSERT INTO role_deliveries (delivery_id, repo_id, role, request_id, body, assignment_id,
             generation, attempt_id, session_id, logical_name, coordinator_record, target_sha,
             requested_by)
         VALUES ($1, 'repo-a', 'development', $1, '{}'::jsonb, 'A1', $2, $1, 's', 'n',
                 '{}'::jsonb, $3, 'test-fixture')",
        &[&id, &generation, &SHA1],
    )
    .expect("delivery");
    for (i, o) in outcomes.iter().enumerate() {
        db.execute(
            "INSERT INTO role_delivery_results (delivery_id, n, reasons, outcome)
             VALUES ($1, $2, '[]'::jsonb, $3)",
            &[&id, &(i32::try_from(i).expect("n") + 1), o],
        )
        .expect("result");
    }
}

#[test]
fn a_takeover_is_stored_only_for_a_delivery_of_its_assignment_generation() {
    let Some(e) = start() else { return };
    let s = ready_sprint(&e, 3);
    let digest = s["digest"].as_str().expect("digest").to_string();
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    let report = |seq: u64, raw: &str| {
        e.post(
            "sup-dev-a",
            CP,
            &json!({"seq": seq, "checkpoint_json": raw}),
        )
    };
    let takeover = |delivery: Value| {
        checkpoint(
            1,
            0,
            &digest,
            json!({"phase": "takeover", "delivery_id": delivery}),
        )
    };
    // refusals: none of them leaves a row in any table (deliveries included)
    delivery_row(&e, "D-g1", 0, &[]);
    assert_eq!(
        assign(
            &e,
            "development",
            "A1",
            1,
            "01-01",
            SHA1,
            json!({"assignment_id": "A1", "generation": 0})
        )
        .0,
        201
    );
    delivery_row(&e, "D-old", 0, &["accepted"]);
    delivery_row(&e, "D-nc", 1, &["not_called"]);
    delivery_row(&e, "D-br", 1, &["budget_refused"]);
    delivery_row(&e, "D-rf", 1, &["accepted", "refused"]);
    delivery_row(&e, "D-na", 1, &["unknown", "not_accepted"]);
    // a delivery still waiting for its own call (review 5dbb3d8 M-1)
    delivery_row(&e, "D-wait", 1, &[]);
    let at1 = |delivery: Value| {
        checkpoint(
            1,
            1,
            &digest,
            json!({"phase": "takeover", "delivery_id": delivery}),
        )
    };
    let before = step4_counts(&e);
    for (raw, st, code_want) in [
        // a takeover without a delivery (every 02-04 direct start)
        (at1(Value::Null), 409, "DELIVERY_REQUIRED"),
        (
            checkpoint(
                1,
                1,
                &digest,
                json!({"phase": "takeover", "delivery_id": null}),
            ),
            409,
            "DELIVERY_REQUIRED",
        ),
        // an unknown delivery, one of the older generation, nothing sent
        (at1(json!("D-none")), 409, "DELIVERY_MISMATCH"),
        (at1(json!("D-old")), 409, "DELIVERY_MISMATCH"),
        (at1(json!("D-g1")), 409, "DELIVERY_MISMATCH"),
        (at1(json!("D-nc")), 409, "DELIVERY_MISMATCH"),
        (at1(json!("D-br")), 409, "DELIVERY_MISMATCH"),
        (at1(json!("D-rf")), 409, "DELIVERY_MISMATCH"),
        (at1(json!("D-na")), 409, "DELIVERY_MISMATCH"),
        // the right delivery, but not its call: another attempt, or none (review 5dbb3d8 M-1)
        (
            checkpoint(
                1,
                1,
                &digest,
                json!({"phase": "takeover", "delivery_id": "D-wait", "attempt_id": "other"}),
            ),
            409,
            "DELIVERY_MISMATCH",
        ),
        (
            checkpoint(
                1,
                1,
                &digest,
                json!({"phase": "takeover", "delivery_id": "D-wait", "attempt_id": null}),
            ),
            409,
            "DELIVERY_MISMATCH",
        ),
        (
            checkpoint(
                1,
                1,
                &digest,
                json!({"phase": "takeover", "delivery_id": "D-wait", "attempt_id": 7}),
            ),
            400,
            "SCHEMA_VIOLATION",
        ),
        // format
        (at1(json!(7)), 400, "SCHEMA_VIOLATION"),
        (at1(json!("")), 400, "SCHEMA_VIOLATION"),
        (
            checkpoint(
                1,
                1,
                &digest,
                json!({"phase": "takeover", "delivery_id": "D-x", "first_action": 0}),
            ),
            400,
            "SCHEMA_VIOLATION",
        ),
        // the older generation's takeover with the older generation's delivery: the 02-04
        // conditions refuse it first
        (takeover(json!("D-old")), 409, "STALE_ASSIGNMENT"),
    ] {
        let (got, b) = report(1, &raw);
        assert_eq!((got, code(&b)), (st, code_want.to_string()), "{raw} {b}");
    }
    assert_eq!(step4_counts(&e), before, "refusals leave no row");
    // a delivery without a result yet (the supervisor reports while the coordinator records the
    // answer), accepted, or unknown: stored with its takeover row in the same transaction
    delivery_row(&e, "D-in", 1, &[]);
    delivery_row(&e, "D-ok", 1, &["accepted"]);
    delivery_row(&e, "D-un", 1, &["unknown"]);
    let (st, b) = report(1, &at1(json!("D-in")));
    assert_eq!(st, 201, "{b}");
    let mut db = e.db();
    let row = db
        .query_one(
            "SELECT delivery_id, seq, session_id, sprint_revision, spec_digest, checkpoint_sha256,
                    evidence_level
             FROM role_takeovers",
            &[],
        )
        .expect("takeover");
    assert_eq!(
        (
            row.get::<_, String>(0),
            row.get::<_, i64>(1),
            row.get::<_, String>(2),
            row.get::<_, i64>(3),
            row.get::<_, String>(4),
            row.get::<_, String>(5),
            row.get::<_, String>(6)
        ),
        (
            "D-in".to_string(),
            1,
            "s1".to_string(),
            5,
            digest.clone(),
            b["checkpoint_sha256"].as_str().expect("sha").to_string(),
            "claim".to_string()
        )
    );
    // the same takeover again: a duplicate, no second row
    assert_eq!(report(1, &at1(json!("D-in"))).0, 200);
    // a delivery is taken over once: a second takeover of it is refused and its checkpoint row
    // rolled back with it
    let after_run = checkpoint(2, 1, &digest, json!({"delivery_id": "D-in"}));
    assert_eq!(report(2, &after_run).0, 201);
    let before = step4_counts(&e);
    let again = checkpoint(
        3,
        1,
        &digest,
        json!({"phase": "takeover", "session_id": "s2", "delivery_id": "D-in"}),
    );
    let (st, b) = report(3, &again);
    assert_eq!((st, code(&b)), (409, "DELIVERY_MISMATCH".into()), "{b}");
    assert!(b.to_string().contains("already has a takeover"), "{b}");
    assert_eq!(
        step4_counts(&e),
        before,
        "no checkpoint row without its takeover row"
    );
    for (seq, d) in [(3, "D-ok"), (5, "D-un")] {
        let raw = checkpoint(
            seq,
            1,
            &digest,
            json!({"phase": "takeover", "session_id": format!("s{seq}"), "delivery_id": d}),
        );
        assert_eq!(report(seq, &raw).0, 201, "{d}");
        let close = checkpoint(
            seq + 1,
            1,
            &digest,
            json!({"session_id": format!("s{seq}"), "delivery_id": d}),
        );
        assert_eq!(report(seq + 1, &close).0, 201, "{d}");
    }
    let n: i64 = db
        .query_one("SELECT count(*) FROM role_takeovers", &[])
        .expect("n")
        .get(0);
    assert_eq!(n, 3);
}

#[test]
fn a_lost_first_action_report_is_filled_from_the_run_checkpoint() {
    let Some(e) = start() else { return };
    let s = ready_sprint(&e, 3);
    let digest = s["digest"].as_str().expect("digest").to_string();
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    let report = |seq: u64, raw: &str| {
        ensure_delivery(&e, raw.as_bytes());
        e.post(
            "sup-dev-a",
            CP,
            &json!({"seq": seq, "checkpoint_json": raw}),
        )
    };
    let fa_path = "/v0/repos/repo-a/roles/development/first-actions";
    let t1 = checkpoint(1, 0, &digest, json!({"phase": "takeover"}));
    assert_eq!(report(1, &t1).0, 201);
    let d1 = serde_json::from_str::<Value>(&t1).expect("j")["delivery_id"].clone();
    let fa = json!({"attempt_id": d1, "event_offset": 4, "observed_at": "2026-11-16T01:00:00.250Z",
                    "kind": "text"});
    // its own report was lost: the after_run checkpoint carries it and fills the record
    let a1 = checkpoint(
        2,
        0,
        &digest,
        json!({"delivery_id": d1, "first_action": fa}),
    );
    assert_eq!(report(2, &a1).0, 201);
    let body = json!({"assignment_id": "A1", "generation": 0, "attempt_id": d1, "session_id": "s1",
                      "event_offset": 4, "observed_at": "2026-11-16T01:00:00.250Z", "kind": "text"});
    let mut db = e.db();
    let row = db
        .query_one(
            "SELECT source, body, evidence_level FROM role_first_actions",
            &[],
        )
        .expect("fa");
    assert_eq!(
        (
            row.get::<_, String>(0),
            row.get::<_, Value>(1),
            row.get::<_, String>(2)
        ),
        ("checkpoint".into(), body.clone(), "claim".into())
    );
    // the late report of the same action: 200, nothing new; other content: 409, nothing new
    let before = step4_counts(&e);
    let (st, b) = e.post("sup-dev-a", fa_path, &body);
    assert_eq!(st, 200, "{b}");
    let mut other = body.clone();
    other["event_offset"] = json!(5);
    let (st, b) = e.post("sup-dev-a", fa_path, &other);
    assert_eq!(
        (st, code(&b)),
        (409, "EVENT_CONTENT_CONFLICT".into()),
        "{b}"
    );
    assert_eq!(step4_counts(&e), before);
    // a run whose report arrived first: the after_run's copy changes nothing; a run without an
    // action says `none` and records nothing; a checkpoint whose first action differs from the
    // record keeps the record (the chain shows both)
    let t3 = checkpoint(
        3,
        0,
        &digest,
        json!({"phase": "takeover", "session_id": "s3"}),
    );
    assert_eq!(report(3, &t3).0, 201);
    let d3 = serde_json::from_str::<Value>(&t3).expect("j")["delivery_id"].clone();
    let body3 = json!({"assignment_id": "A1", "generation": 0, "attempt_id": d3, "session_id": "s3",
                       "event_offset": 2, "observed_at": "2026-11-16T01:05:00.000Z", "kind": "tool_use"});
    let (st, b) = e.post("sup-dev-a", fa_path, &body3);
    assert_eq!(st, 201, "{b}");
    let mut differs = body3.clone();
    differs["event_offset"] = json!(3);
    let fa3 = json!({"attempt_id": d3, "event_offset": 3, "observed_at": "2026-11-16T01:05:00.000Z",
                     "kind": "tool_use"});
    let before = step4_counts(&e);
    assert_eq!(
        report(
            4,
            &checkpoint(
                4,
                0,
                &digest,
                json!({"session_id": "s3", "delivery_id": d3, "first_action": fa3})
            )
        )
        .0,
        201
    );
    let t5 = checkpoint(
        5,
        0,
        &digest,
        json!({"phase": "takeover", "session_id": "s5"}),
    );
    assert_eq!(report(5, &t5).0, 201);
    let d5 = serde_json::from_str::<Value>(&t5).expect("j")["delivery_id"].clone();
    assert_eq!(
        report(
            6,
            &checkpoint(
                6,
                0,
                &digest,
                json!({"session_id": "s5", "delivery_id": d5, "first_action": "none"})
            )
        )
        .0,
        201
    );
    let n: i64 = db
        .query_one("SELECT count(*) FROM role_first_actions", &[])
        .expect("n")
        .get(0);
    assert_eq!(n, 2, "{before:?}");
    let (st, chain) = e.get("operator", CHAIN0);
    assert_eq!(st, 200, "{chain}");
    let link = |d: &Value| {
        chain["links"]
            .as_array()
            .expect("links")
            .iter()
            .find(|l| l["delivery_id"] == *d)
            .cloned()
            .expect("link")
    };
    assert_eq!(link(&d1)["first_action"]["source"], "checkpoint");
    assert_eq!(link(&d1)["first_action"]["event_offset"], 4);
    assert_eq!(link(&d3)["first_action"]["source"], "report");
    assert_eq!(link(&d3)["first_action_conflict"], true);
    assert_eq!(link(&d5)["first_action"], "none");
    assert_eq!(link(&d5)["first_action_conflict"], false);
    // a resumed session takes over once per run under the same generation: the first action
    // of its second run is recorded (step 2's lookup failed with 500 on two takeover rows)
    let t7 = checkpoint(7, 0, &digest, json!({"phase": "takeover"}));
    assert_eq!(report(7, &t7).0, 201);
    let d7 = serde_json::from_str::<Value>(&t7).expect("j")["delivery_id"].clone();
    let (st, b) = e.post(
        "sup-dev-a",
        fa_path,
        &json!({"assignment_id": "A1", "generation": 0, "attempt_id": d7, "session_id": "s1",
                "event_offset": 2, "observed_at": "2026-11-16T01:10:00Z", "kind": "tool_use"}),
    );
    assert_eq!(st, 201, "{b}");
    assert_eq!(
        report(
            8,
            &checkpoint(
                8,
                0,
                &digest,
                json!({"delivery_id": d7, "first_action": "none"})
            )
        )
        .0,
        201
    );
    // review 5dbb3d8 L-2, I-2: a first action is filled only for a session whose takeover is
    // stored and linked to the delivery that called that attempt
    let before = step4_counts(&e);
    let fa_x = |attempt: &str| {
        json!({"attempt_id": attempt, "event_offset": 1, "observed_at": "2026-11-16T01:20:00Z",
               "kind": "text"})
    };
    // (a) a session never taken over (the after_run is current, so it is stored)
    let a = checkpoint(
        9,
        0,
        &digest,
        json!({"session_id": "s-none", "first_action": fa_x("att-none")}),
    );
    assert_eq!(report(9, &a).0, 201);
    // (b) a session taken over through d1, with a first action of an attempt d1 did not call
    let b = checkpoint(
        10,
        0,
        &digest,
        json!({"delivery_id": d1, "attempt_id": "att-other", "first_action": fa_x("att-other")}),
    );
    assert_eq!(report(10, &b).0, 201);
    // (c) the checkpoint's own attempt differs from the first action's: s5's takeover is linked
    // to d5, which called the first action's attempt and has no first action yet, but this
    // checkpoint belongs to another attempt
    let c = checkpoint(
        11,
        0,
        &digest,
        json!({"session_id": "s5", "delivery_id": d5, "attempt_id": "att-third",
               "first_action": fa_x(d5.as_str().expect("d5"))}),
    );
    assert_eq!(report(11, &c).0, 201);
    let after = step4_counts(&e);
    assert_eq!(
        (after[2].1, after[0].1),
        (before[2].1, before[0].1 + 3),
        "three checkpoints, no first action"
    );
    let (_, ch) = e.get("operator", CHAIN0);
    let none = ["att-none", "att-other", d5.as_str().expect("d5")];
    assert!(
        ch["first_actions"]
            .as_array()
            .expect("fa")
            .iter()
            .all(|f| !none.contains(&f["attempt_id"].as_str().unwrap_or(""))),
        "{ch}"
    );
    // a malformed first action is refused with its checkpoint
    let before = step4_counts(&e);
    for bad in [
        json!({"attempt_id": "x", "event_offset": -1, "observed_at": "2026-11-16T01:00:00Z", "kind": "text"}),
        json!({"attempt_id": "x", "event_offset": 1, "observed_at": "2026-11-16 01:00:00", "kind": "text"}),
        json!({"attempt_id": "x", "event_offset": 1, "observed_at": "2026-11-16T01:00:00Z", "kind": "init"}),
        json!({"attempt_id": "x", "event_offset": 1, "observed_at": "2026-11-16T01:00:00Z", "kind": "text", "extra": 1}),
        json!("some"),
        json!(0),
    ] {
        let (st, b) = report(
            12,
            &checkpoint(
                12,
                0,
                &digest,
                json!({"session_id": "s5", "first_action": bad}),
            ),
        );
        assert_eq!(
            (st, code(&b)),
            (400, "SCHEMA_VIOLATION".into()),
            "{bad} {b}"
        );
    }
    assert_eq!(step4_counts(&e), before);
    // PR #75 review debf413 L-3: a later checkpoint of ANOTHER attempt that names the delivery
    // and says `none` is not that delivery's "no first action": the link stays null (not known)
    let t12 = checkpoint(
        12,
        0,
        &digest,
        json!({"phase": "takeover", "session_id": "s12"}),
    );
    assert_eq!(report(12, &t12).0, 201);
    let d12 = serde_json::from_str::<Value>(&t12).expect("j")["delivery_id"].clone();
    let foreign = checkpoint(
        13,
        0,
        &digest,
        json!({"session_id": "s12", "delivery_id": d12, "attempt_id": "att-foreign",
               "first_action": "none"}),
    );
    assert_eq!(report(13, &foreign).0, 201);
    let (st, ch) = e.get("operator", CHAIN0);
    assert_eq!(st, 200, "{ch}");
    let l12 = ch["links"]
        .as_array()
        .expect("links")
        .iter()
        .find(|l| l["delivery_id"] == d12)
        .cloned()
        .expect("link d12");
    assert_eq!(
        (l12["taken_over"].as_bool(), l12["first_action"].clone()),
        (Some(true), Value::Null),
        "{l12}"
    );
}

#[test]
fn the_assignment_chain_is_read_by_clients_of_its_repo_only() {
    let Some(e) = start() else { return };
    ready_sprint(&e, 3);
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    assert_eq!(e.call(None, "GET", CHAIN0, None, None).0, 401);
    for who in ["sup-dev-a", "dev-a", "alice", "ingest"] {
        let (st, b) = e.get(who, CHAIN0);
        assert_eq!((st, code(&b)), (403, "SCOPE_DENIED".into()), "{who} {b}");
    }
    // another repo's client and an unknown generation: the same 404
    for (who, path) in [
        ("operator-norepo", CHAIN0),
        (
            "operator",
            "/v0/coordinator/assignments/A1/generations/9/chain",
        ),
        (
            "operator",
            "/v0/coordinator/assignments/A9/generations/0/chain",
        ),
    ] {
        let (st, b) = e.get(who, path);
        assert_eq!(
            (st, code(&b)),
            (404, "NOT_FOUND".into()),
            "{who} {path} {b}"
        );
    }
    for g in ["x", "-1", "1.0", "18446744073709551616"] {
        let (st, b) = e.get(
            "operator",
            &format!("/v0/coordinator/assignments/A1/generations/{g}/chain"),
        );
        assert_eq!((st, code(&b)), (400, "SCHEMA_VIOLATION".into()), "{g} {b}");
    }
    for who in ["operator", "assigner"] {
        let (st, b) = e.get(who, CHAIN0);
        assert_eq!(st, 200, "{who} {b}");
        assert_eq!(b["assignment"]["evidence_level"], "verified");
        assert_eq!(b["assignment"]["current"], true);
        assert_eq!(b["assignment"]["issued_by"], "assigner");
        assert_eq!(b["sprint"]["evidence_level"], "verified");
        assert_eq!(b["sprint"]["revision"], 5);
        assert!(b["sprint"]["sources"].is_array(), "{b}");
        for k in [
            "deliveries",
            "takeovers",
            "first_actions",
            "checkpoints",
            "agent_reports",
        ] {
            assert_eq!(b[k], json!([]), "{k}");
        }
        assert!(b["session_status"].is_null());
    }
}

/// PLAN §3.2 step 4: with no agent report, no takeover and no first action the chain and the
/// completion judgment proceed; claims that arrive later are stored as claims and change no
/// judgment.
#[test]
fn claims_are_kept_apart_from_the_completion_judgment() {
    let Some(e) = start() else { return };
    let s = ready_sprint(&e, 3);
    let digest = s["digest"].as_str().expect("digest").to_string();
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    full_evidence(&e, SHA1, "c");
    assert_eq!(
        evidence(
            &e,
            "rv-ind",
            "review",
            json!({"sha": SHA1, "reviewer": "rev", "author": "dev", "state": "approved",
                   "source_ref": "pull/1#r2"})
        )
        .0,
        201
    );
    // nothing reported by any supervisor or agent yet
    let (st, j1) = judge(&e, "jc-1", "01-01");
    assert_eq!(
        (st, j1["outcome"].as_str()),
        (201, Some("accepted")),
        "{j1}"
    );
    let (st, c1) = e.get("operator", CHAIN0);
    assert_eq!(st, 200, "{c1}");
    for k in ["takeovers", "first_actions", "agent_reports", "checkpoints"] {
        assert_eq!(c1[k], json!([]), "{k}");
    }
    // the claims arrive late: a takeover, an after_run carrying the first action, the
    // development agent's progress report, a session status
    let report = |seq: u64, raw: &str| {
        ensure_delivery(&e, raw.as_bytes());
        e.post(
            "sup-dev-a",
            CP,
            &json!({"seq": seq, "checkpoint_json": raw}),
        )
    };
    let t1 = checkpoint(1, 0, &digest, json!({"phase": "takeover"}));
    assert_eq!(report(1, &t1).0, 201);
    let d1 = serde_json::from_str::<Value>(&t1).expect("j")["delivery_id"].clone();
    let fa = json!({"attempt_id": d1, "event_offset": 2, "observed_at": "2026-11-16T02:00:00.100Z",
                    "kind": "tool_use"});
    assert_eq!(
        report(
            2,
            &checkpoint(
                2,
                0,
                &digest,
                json!({"delivery_id": d1, "first_action": fa})
            )
        )
        .0,
        201
    );
    assert_eq!(
        e.post("dev-a", "/v0/agent/reports", &report_body("dev-c1"))
            .0,
        202
    );
    let status = json!({"observed_at": "2026-11-16T02:00:01Z", "lifecycle": "idle", "session_id": "s1",
        "assignment_id": "A1", "generation": 0, "quota_group_id": "test-shared",
        "provider_status_or_unknown": "unknown", "last_actual_action_at": "2026-11-16T02:00:00.100Z",
        "checkpoint_seq": 2, "checkpoint_sha256": "a".repeat(64),
        "pending_external_requests": "not_tracked", "active_children": "not_tracked"});
    let (st, b) = e.call(
        Some("sup-dev-a"),
        "POST",
        "/v0/repos/repo-a/roles/development/session-status",
        Some("status-key-0001"),
        Some(&status),
    );
    assert_eq!(st, 201, "{b}");
    // the same judgment: claims are not judgment inputs
    let (st, j2) = judge(&e, "jc-2", "01-01");
    assert_eq!(st, 201, "{j2}");
    assert_eq!(
        (&j2["outcome"], &j2["facts"]),
        (&j1["outcome"], &j1["facts"]),
        "{j1} {j2}"
    );
    let (st, c2) = e.get("operator", CHAIN0);
    assert_eq!(st, 200, "{c2}");
    assert_eq!(
        c2["evidence_levels"],
        json!({"verified": ["sprint", "assignment", "deliveries", "usage", "usage_runs",
                            "limit_events"],
               "claim": ["takeovers", "first_actions", "checkpoints", "tamper_events",
                         "agent_reports", "session_status"]})
    );
    assert_eq!(c2["agent_reports"].as_array().map(Vec::len), Some(1));
    assert_eq!(c2["agent_reports"][0]["evidence_level"], "claim");
    assert_eq!(c2["agent_reports"][0]["event_id"], "dev-c1");
    assert_eq!(c2["takeovers"][0]["evidence_level"], "claim");
    assert_eq!(c2["first_actions"][0]["evidence_level"], "claim");
    assert_eq!(c2["first_actions"][0]["source"], "checkpoint");
    assert_eq!(c2["session_status"]["evidence_level"], "claim");
    assert_eq!(c2["session_status"]["names_this_generation"], true);
    assert_eq!(
        c2["session_status"]["status"]["last_actual_action_at"],
        "2026-11-16T02:00:00.100Z"
    );
    // the fixture delivery is a coordinator record: verified; its link shows the takeover
    assert_eq!(c2["deliveries"][0]["evidence_level"], "verified");
    assert_eq!(c2["links"][0]["taken_over"], true);
    assert_eq!(c2["links"][0]["takeover_seq"], 1);
}

fn report_body(event_id: &str) -> Value {
    report("development", event_id, "A1", 0, "01-01", SHA1)
}

/// PR #74 review fe924b7 I-5: a statement on the HTTP path is bounded (10 s), advisory-lock
/// waits included. A checkpoint report queued behind a held lock answers 500 after the bound,
/// nothing is stored, and the next report goes through once the lock is released.
#[test]
fn an_http_statement_waits_at_most_the_statement_timeout() {
    let Some(e) = start() else { return };
    let s = ready_sprint(&e, 3);
    let digest = s["digest"].as_str().expect("digest").to_string();
    assert_eq!(
        assign(&e, "development", "A1", 0, "01-01", SHA1, Value::Null).0,
        201
    );
    let raw = checkpoint(1, 0, &digest, json!({"phase": "takeover"}));
    ensure_delivery(&e, raw.as_bytes());
    let mut holder = e.db();
    let mut tx = holder.transaction().expect("tx");
    tx.execute(
        "SELECT pg_advisory_xact_lock(hashtextextended('checkpoint:repo-a:development', 0))",
        &[],
    )
    .expect("hold");
    let before = e.counts();
    let t0 = std::time::Instant::now();
    let (st, b) = e.post("sup-dev-a", CP, &json!({"seq": 1, "checkpoint_json": raw}));
    let took = t0.elapsed();
    assert_eq!(st, 500, "{b}");
    assert!(b.to_string().contains("statement timeout"), "{b}");
    assert!(
        took >= std::time::Duration::from_secs(9) && took < std::time::Duration::from_secs(30),
        "{took:?}"
    );
    assert_eq!(e.counts(), before, "nothing stored");
    tx.rollback().expect("release");
    let (st, b) = e.post("sup-dev-a", CP, &json!({"seq": 1, "checkpoint_json": raw}));
    assert_eq!(st, 201, "{b}");
}
