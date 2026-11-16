//! The connection of one assignment generation (02-05 step 4, PLAN §3.2 step 4):
//! `GET /v0/coordinator/assignments/{assignment_id}/generations/{generation}/chain`
//! (getAssignmentChain). HTTP only, read only, one transaction. Service or user principals with
//! `coordinator:assign` or `roles:control` whose repos include the assignment's repo; anything
//! else that names an existing assignment is answered like an unknown one (404).
//!
//! sprint revision (sources, digest) → assignment (generation, target SHA, limits, issuer,
//! policy digest) → deliveries (request, judgment, answer, times) → takeovers → first actions →
//! the generation's checkpoints and tamper events → agent reports of the assignment, and the
//! latest session status of the (repo, role).
//!
//! **Verified and claim.** Only what the coordinator itself recorded or did is `verified`: the
//! sprint revision, the assignment and the deliveries (what it sent, what it judged, what it was
//! answered). Everything a supervisor or an agent reported — takeovers, first actions,
//! checkpoints, tamper events, agent reports, session status — is a `claim` (runtime contract
//! §2.1: the supervisor runs as the model's UID). Every section says which it is. Claims are not
//! required inputs of the completion judgment; the only claim it reads is the current
//! guardrail's `guardrail_review` (G3, policy 0.6.0; step-4 REPORT §1.6). Supervisor claims
//! (takeovers, first actions, checkpoints, session status) are never judgment inputs.

use crate::api::{R, i64_of, ok, problem};
use crate::auth::{Caller, Kind};
use postgres::Transaction;
use serde_json::{Value, json};

fn need_reader(who: &Caller) -> Result<(), crate::api::Resp> {
    if who.kind == Kind::Agent {
        return Err(problem(
            403,
            "SCOPE_DENIED",
            "agent credentials cannot use coordinator endpoints",
        ));
    }
    if !(who.has("coordinator:assign") || who.has("roles:control")) {
        return Err(problem(
            403,
            "SCOPE_DENIED",
            "scope `coordinator:assign` or `roles:control` required",
        ));
    }
    Ok(())
}

/// `getAssignmentChain`.
pub(crate) fn assignment_chain(tx: &mut Transaction, who: &Caller, aid: &str, gen_s: &str) -> R {
    need_reader(who)?;
    let Some(generation) = gen_s
        .parse::<u64>()
        .ok()
        .filter(|_| gen_s.bytes().all(|b| b.is_ascii_digit()))
    else {
        return Err(crate::api::one(
            400,
            "SCHEMA_VIOLATION",
            "generation",
            "generation is a non-negative integer",
            "name the generation as issued",
        ));
    };
    let g = i64_of(generation);
    let row = tx.query_opt(
        "SELECT a.repo_id, a.role, a.sprint_id, a.sprint_revision, a.task_id, a.target_sha,
                a.issued_by, a.policy_digest, iso(a.issued_at), s.digest, s.spec,
                EXISTS (SELECT 1 FROM assignment_heads h WHERE h.repo_id = a.repo_id
                          AND h.role = a.role AND h.assignment_id = a.assignment_id
                          AND h.generation = a.generation)
         FROM assignments a
         JOIN sprint_revisions s ON (s.sprint_id, s.revision) = (a.sprint_id, a.sprint_revision)
         WHERE a.assignment_id = $1 AND a.generation = $2",
        &[&aid, &g],
    )?;
    let Some(a) = row.filter(|r| who.in_repo(r.get::<_, &str>(0))) else {
        return Err(problem(
            404,
            "NOT_FOUND",
            "unknown assignment generation or not visible to this principal",
        ));
    };
    let (repo, role): (String, String) = (a.get(0), a.get(1));
    let spec: Value = a.get(10);
    let sources: Vec<Value> = spec["sources"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|x| {
            json!({"kind": x["kind"], "ref": x["ref"], "revision": x["revision"],
                   "digest": x["digest"], "observed_at": x["observed_at"]})
        })
        .collect();
    let limit = |k: &str| spec["limits"][k].as_u64().map_or(Value::Null, |n| json!(n));
    let sprint = json!({"evidence_level": "verified", "sprint_id": a.get::<_, String>(2),
        "revision": a.get::<_, i64>(3), "digest": a.get::<_, String>(9), "sources": sources});
    // the assignment's limits and what its runs used, all generations (02-05 step 5)
    let usage = crate::limits::state(tx, aid, g, None)?;
    let lim = |k: &str| usage["limits"][k].clone();
    let assignment = json!({"evidence_level": "verified", "assignment_id": aid,
        "generation": generation, "repo_id": repo, "role": role,
        "task_id": a.get::<_, String>(4), "target_sha": a.get::<_, String>(5),
        "limits": {"elapsed_seconds": lim("elapsed_seconds"), "total_tokens": lim("total_tokens"),
                   "correction_attempts": limit("correction_attempts")},
        "issued_by": a.get::<_, String>(6), "policy_digest": a.get::<_, String>(7),
        "issued_at": a.get::<_, String>(8), "current": a.get::<_, bool>(11)});

    // deliveries: what the coordinator sent and was answered (verified)
    let mut deliveries = Vec::new();
    for d in tx.query(
        "SELECT delivery_id, request_id, attempt_id, session_id, logical_name, coordinator_record,
                target_sha, cause, requested_by, iso(requested_at)
         FROM role_deliveries WHERE assignment_id = $1 AND generation = $2 ORDER BY seq",
        &[&aid, &g],
    )? {
        let id: String = d.get(0);
        let results: Vec<Value> = tx
            .query(
                "SELECT n, decision, reasons, op, outcome, limits, sent_sha256, supervisor_status,
                        supervisor_body, requeried, iso(recorded_at)
                 FROM role_delivery_results WHERE delivery_id = $1 ORDER BY n",
                &[&id],
            )?
            .iter()
            .map(|r| {
                json!({"n": r.get::<_, i32>(0), "decision": r.get::<_, Option<String>>(1),
                    "reasons": r.get::<_, Value>(2), "op": r.get::<_, Option<String>>(3),
                    "outcome": r.get::<_, String>(4), "limits": r.get::<_, Option<Value>>(5),
                    "sent_sha256": r.get::<_, Option<String>>(6),
                    "supervisor": r.get::<_, Option<i32>>(7).map(|st| json!({"http_status": st,
                        "body": r.get::<_, Option<Value>>(8)})),
                    "requeried": r.get::<_, bool>(9), "recorded_at": r.get::<_, String>(10)})
            })
            .collect();
        // the time the supervisor's answer (or its loss) was recorded
        let responded_at = results
            .iter()
            .find(|r| {
                matches!(
                    r["outcome"].as_str(),
                    Some("accepted" | "refused" | "unknown")
                )
            })
            .map_or(Value::Null, |r| r["recorded_at"].clone());
        let outcome = results.last().map_or(Value::Null, |r| r["outcome"].clone());
        deliveries.push(json!({"evidence_level": "verified", "delivery_id": id,
            "request_id": d.get::<_, String>(1), "attempt_id": d.get::<_, String>(2),
            "session_id": d.get::<_, String>(3), "logical_name": d.get::<_, String>(4),
            "coordinator_record": d.get::<_, Value>(5), "target_sha": d.get::<_, String>(6),
            "cause": d.get::<_, String>(7), "requested_by": d.get::<_, String>(8),
            "requested_at": d.get::<_, String>(9), "responded_at": responded_at,
            "outcome": outcome, "results": results}));
    }

    // takeovers: stored with their checkpoint, linked to a delivery (claim)
    let takeovers: Vec<Value> = tx
        .query(
            "SELECT delivery_id, seq, checkpoint_sha256, session_id, sprint_id, sprint_revision,
                    spec_digest, reported_by, iso(received_at)
             FROM role_takeovers WHERE assignment_id = $1 AND generation = $2 ORDER BY seq",
            &[&aid, &g],
        )?
        .iter()
        .map(|r| {
            json!({"evidence_level": "claim", "delivery_id": r.get::<_, String>(0),
                "seq": r.get::<_, i64>(1), "checkpoint_sha256": r.get::<_, String>(2),
                "session_id": r.get::<_, String>(3), "applied": {"sprint_id": r.get::<_, String>(4),
                "sprint_revision": r.get::<_, i64>(5), "spec_digest": r.get::<_, String>(6)},
                "reported_by": r.get::<_, String>(7), "received_at": r.get::<_, String>(8)})
        })
        .collect();

    // first actions (claim): the supervisor's report, or the run's later checkpoint
    let first_actions: Vec<Value> = tx
        .query(
            "SELECT f.attempt_id, f.session_id, f.event_offset, iso(f.observed_at), f.kind, f.source,
                    f.current_assignment, f.reported_by, iso(f.received_at),
                    (SELECT d.delivery_id FROM role_deliveries d WHERE d.repo_id = f.repo_id
                       AND d.role = f.role AND d.attempt_id = f.attempt_id)
             FROM role_first_actions f WHERE f.assignment_id = $1 AND f.generation = $2
             ORDER BY f.received_at, f.attempt_id",
            &[&aid, &g],
        )?
        .iter()
        .map(|r| {
            json!({"evidence_level": "claim", "attempt_id": r.get::<_, String>(0),
                "session_id": r.get::<_, String>(1), "event_offset": r.get::<_, i64>(2),
                "observed_at": r.get::<_, String>(3), "kind": r.get::<_, String>(4),
                "source": r.get::<_, String>(5), "current_assignment": r.get::<_, bool>(6),
                "reported_by": r.get::<_, String>(7), "received_at": r.get::<_, String>(8),
                "delivery_id": r.get::<_, Option<String>>(9)})
        })
        .collect();

    // the generation's checkpoints and the tamper events reported with them (claim)
    let checkpoints: Vec<Value> = tx
        .query(
            "SELECT seq, phase, session_id, checkpoint_sha256, current_assignment, reported_by,
                    iso(received_at), checkpoint->'delivery_id', checkpoint->'first_action',
                    checkpoint->'attempt_id'
             FROM role_checkpoints WHERE assignment_id = $1 AND generation = $2 ORDER BY seq",
            &[&aid, &g],
        )?
        .iter()
        .map(|r| {
            json!({"evidence_level": "claim", "seq": r.get::<_, i64>(0),
                "phase": r.get::<_, String>(1), "session_id": r.get::<_, String>(2),
                "checkpoint_sha256": r.get::<_, String>(3),
                "current_assignment": r.get::<_, bool>(4), "reported_by": r.get::<_, String>(5),
                "received_at": r.get::<_, String>(6),
                "delivery_id": r.get::<_, Option<Value>>(7).unwrap_or(Value::Null),
                "first_action": r.get::<_, Option<Value>>(8).unwrap_or(Value::Null),
                "attempt_id": r.get::<_, Option<Value>>(9).unwrap_or(Value::Null)})
        })
        .collect();
    let tamper_events: Vec<Value> = tx
        .query(
            "SELECT t.seq, t.new_session_id, t.old_session_id, t.checkpoint_digest,
                    t.observed_digest, t.reported_by, iso(t.received_at)
             FROM role_tamper_events t
             JOIN role_checkpoints c ON (c.repo_id, c.role, c.seq) = (t.repo_id, t.role, t.seq)
             WHERE c.assignment_id = $1 AND c.generation = $2 ORDER BY t.seq",
            &[&aid, &g],
        )?
        .iter()
        .map(|r| {
            json!({"evidence_level": "claim", "seq": r.get::<_, i64>(0),
                "new_session_id": r.get::<_, String>(1), "old_session_id": r.get::<_, String>(2),
                "checkpoint_digest": r.get::<_, String>(3),
                "observed_digest": r.get::<_, String>(4), "reported_by": r.get::<_, String>(5),
                "received_at": r.get::<_, String>(6)})
        })
        .collect();

    // the agent reports of this assignment generation (report v0 claims, separate field)
    let agent_reports: Vec<Value> = tx
        .query(
            "SELECT scope_role, event_id, event_revision, report_type, disposition, attributed_sha,
                    iso(stored_at)
             FROM events WHERE assignment_id = $1 AND generation = $2 AND evidence_level = 'claim'
             ORDER BY sprint_id, sprint_seq",
            &[&aid, &g],
        )?
        .iter()
        .map(|r| {
            json!({"evidence_level": "claim", "role": r.get::<_, String>(0),
                "event_id": r.get::<_, String>(1), "event_revision": r.get::<_, i64>(2),
                "report_type": r.get::<_, String>(3), "disposition": r.get::<_, Option<String>>(4),
                "attributed_sha": r.get::<_, Option<String>>(5),
                "stored_at": r.get::<_, String>(6)})
        })
        .collect();

    // the latest session status of the (repo, role) (never a stale-on-arrival one)
    let session_status = tx
        .query_opt(
            "SELECT status, iso(observed_at), iso(received_at), assignment_id, generation,
                    current_assignment, reported_by
             FROM role_session_status WHERE repo_id = $1 AND role = $2 AND NOT stale_on_arrival
             ORDER BY observed_at DESC, received_at DESC LIMIT 1",
            &[&repo, &role],
        )?
        .map_or(Value::Null, |r| {
            let names = r.get::<_, Option<String>>(3).as_deref() == Some(aid)
                && r.get::<_, Option<i64>>(4) == Some(g);
            json!({"evidence_level": "claim", "status": r.get::<_, Value>(0),
                "observed_at": r.get::<_, String>(1), "received_at": r.get::<_, String>(2),
                "names_this_generation": names,
                "current_assignment": r.get::<_, Option<bool>>(5),
                "reported_by": r.get::<_, String>(6)})
        });

    // the settled runs of the assignment (newest row per delivery) and its limit refusals
    let usage_runs: Vec<Value> = tx
        .query(
            "SELECT DISTINCT ON (delivery_id) delivery_id, generation, attempt_id, state, seconds,
                    input_tokens, output_tokens, cache_creation_tokens, cache_read_tokens, reason,
                    record_sha256, iso(settled_at)
             FROM assignment_run_usage WHERE assignment_id = $1
             ORDER BY delivery_id, settled_at DESC",
            &[&aid],
        )?
        .iter()
        .map(|r| {
            json!({"evidence_level": "verified", "delivery_id": r.get::<_, String>(0),
                "generation": r.get::<_, i64>(1), "attempt_id": r.get::<_, String>(2),
                "state": r.get::<_, String>(3), "seconds": r.get::<_, Option<i64>>(4),
                "tokens": {"input": r.get::<_, Option<i64>>(5),
                           "output": r.get::<_, Option<i64>>(6),
                           "cache_creation": r.get::<_, Option<i64>>(7),
                           "cache_read": r.get::<_, Option<i64>>(8)},
                "reason": r.get::<_, Option<String>>(9),
                "record_sha256": r.get::<_, String>(10), "settled_at": r.get::<_, String>(11)})
        })
        .collect();
    let limit_events: Vec<Value> = tx
        .query(
            "SELECT generation, code, settled_runs, reason, requested_by, iso(recorded_at)
             FROM assignment_limit_events WHERE assignment_id = $1
             ORDER BY recorded_at, generation, code",
            &[&aid],
        )?
        .iter()
        .map(|r| {
            json!({"generation": r.get::<_, i64>(0), "code": r.get::<_, String>(1),
                "settled_runs": r.get::<_, i64>(2), "reason": r.get::<_, String>(3),
                "requested_by": r.get::<_, String>(4), "recorded_at": r.get::<_, String>(5)})
        })
        .collect();
    let mut usage = usage;
    usage["operational_state"] = json!(match crate::limits::room(&usage) {
        Ok(_) => "open",
        Err(_) => "awaiting_human_decision",
    });

    // one link per delivery: what was sent, whether it was taken over, and the run's first
    // action (recorded, `none` as its after_run says, or null: not known)
    let links: Vec<Value> = deliveries
        .iter()
        .map(|d| {
            let id = &d["delivery_id"];
            let takeover = takeovers.iter().find(|t| t["delivery_id"] == *id);
            let fa = first_actions
                .iter()
                .find(|f| f["delivery_id"] == *id)
                .map(|f| {
                    json!({"event_offset": f["event_offset"], "observed_at": f["observed_at"],
                           "kind": f["kind"], "source": f["source"]})
                })
                .or_else(|| {
                    checkpoints
                        .iter()
                        .any(|c| {
                            c["delivery_id"] == *id
                                && c["attempt_id"] == d["attempt_id"]
                                && c["first_action"] == "none"
                        })
                        .then(|| json!("none"))
                })
                .unwrap_or(Value::Null);
            // a first action the run's checkpoint carries that differs from the record
            let conflict = first_actions
                .iter()
                .filter(|f| f["delivery_id"] == *id)
                .any(|f| {
                    checkpoints.iter().any(|c| {
                        c["delivery_id"] == *id
                            && c["first_action"].is_object()
                            && (c["first_action"]["event_offset"] != f["event_offset"]
                                || c["first_action"]["kind"] != f["kind"])
                    })
                });
            json!({"delivery_id": id, "attempt_id": d["attempt_id"], "outcome": d["outcome"],
                   "taken_over": takeover.is_some(),
                   "takeover_seq": takeover.map_or(Value::Null, |t| t["seq"].clone()),
                   "first_action": fa, "first_action_conflict": conflict})
        })
        .collect();
    ok(
        200,
        json!({"assignment_id": aid, "generation": generation, "repo_id": repo, "role": role,
            "evidence_levels": {
                "verified": ["sprint", "assignment", "deliveries", "usage", "usage_runs",
                         "limit_events"],
                "claim": ["takeovers", "first_actions", "checkpoints", "tamper_events",
                          "agent_reports", "session_status"]},
            "sprint": sprint, "assignment": assignment, "deliveries": deliveries,
            "takeovers": takeovers, "first_actions": first_actions, "checkpoints": checkpoints,
            "tamper_events": tamper_events, "agent_reports": agent_reports,
            "session_status": session_status, "links": links,
            "usage": usage, "usage_runs": usage_runs, "limit_events": limit_events}),
    )
}
