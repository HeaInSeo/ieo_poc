//! The official recovery after a crash (02-05 step 7b; user decision 2026-11-16 (나),
//! docs/poc/User_Decisions_2026-11-16_recovery.md §2; runtime contract revision 15 §18).
//!
//! A run that did not finish leaves its role's last checkpoint at `takeover` or `error`. Nothing
//! resumes over it and no new session starts over it (runtime contract §6, §13 item 10, review
//! c11e734 M-1). A **person** may approve one new session over exactly that checkpoint:
//!
//! * `POST /v0/repos/{repo}/roles/{role}/recoveries` (approveRecovery, `Idempotency-Key`): only a
//!   `user` principal with `roles:control` for the repo; a service or agent credential (the
//!   role's own included) is refused. The approval names the assignment, generation and the
//!   unfinished checkpoint (sequence and digest) and gives a reason. It is checked against the
//!   supervisor's status (not running, the local checkpoint is the central one, no pending
//!   external request) and the coordinator's record (the named checkpoint is the latest and
//!   unfinished, the assignment generation is the current head, no delivery of the repo with an
//!   unknown outcome), and for development against the GitHub head (step 7's
//!   [`crate::api::github_head_check`]; the GitHub records are evidence-service input, not a live
//!   GitHub read). A refusal records nothing (409). What cannot be confirmed — the supervisor not
//!   observed, a pending external request, a delivery of unknown outcome, a GitHub head that does
//!   not match (a crashed run that pushed included) — is recorded as `needs_confirmation` with
//!   its evidence: a hold that nothing may use. A person reconciles and approves again.
//! * `deliverAssignment` with `recovery_id`: everything is checked again at use time (the
//!   checkpoint, generation and GitHub head may have changed since the approval); the call is
//!   always a `start` of the delivery's new session, never a resume. The use is committed right
//!   before the start is sent: one approval starts at most one run, also under duplicate
//!   requests, lost answers and restarts. The usual limits and budget check apply: an approval
//!   neither resets usage nor approves a call.

use crate::api::{
    Abort, Ctx, R, Req, Resp, TxGuard, github_head_check, history, i64_of, idem_key, is_sha,
    latest_row, lock, need_client_scope, need_role, ok, one, only_keys, parse_body,
    pending_deliveries, problem, run_tx, str_field,
};
use crate::auth::{Caller, Kind};
use postgres::Transaction;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// The recovery id of a request (repo, role, Idempotency-Key).
fn recovery_id(repo: &str, role: &str, key: &str) -> String {
    let h = hex::encode(Sha256::digest(format!("{repo}\0{role}\0{key}").as_bytes()));
    format!("R-{}", &h[..32])
}

/// Who may approve: a person (`user`) with `roles:control` for the repo. A service credential
/// with the scope is not a person; an agent credential (the role's own) never approves.
pub fn need_approver(who: &Caller, repo: &str, role: &str) -> Result<(), Resp> {
    need_role(role)?;
    need_client_scope(who, "roles:control", false)?;
    if !who.in_repo(repo) {
        return Err(problem(
            403,
            "REPO_SCOPE_DENIED",
            "repo not in this credential's scope",
        ));
    }
    if who.kind != Kind::User {
        return Err(problem(
            403,
            "HUMAN_APPROVAL_REQUIRED",
            "a recovery is approved by a person (a user credential), not a service or agent",
        ));
    }
    Ok(())
}

fn target_mismatch(field: &str, message: &str) -> Resp {
    one(
        409,
        "RECOVERY_TARGET_MISMATCH",
        field,
        message,
        "read the latest checkpoint and the current assignment and name them",
    )
}

/// The uses of one checkpoint with their delivery's newest outcome. A use consumes the
/// checkpoint when its delivery may have run: `accepted`, `unknown` or no result yet. One whose
/// delivery ran nothing (`refused`, `not_called`, `budget_refused`, or `not_accepted`: the
/// re-query found the attempt unknown to the supervisor) does not (review 185d12b M-1).
fn checkpoint_uses(
    tx: &mut Transaction,
    repo: &str,
    role: &str,
    seq: i64,
) -> Result<Vec<Value>, Resp> {
    Ok(tx
        .query(
            "SELECT u.recovery_id, u.delivery_id,
                    (SELECT outcome FROM role_delivery_results r WHERE r.delivery_id = u.delivery_id
                     ORDER BY n DESC LIMIT 1)
             FROM role_recovery_uses u
             WHERE u.repo_id = $1 AND u.role = $2 AND u.checkpoint_seq = $3 ORDER BY u.used_at",
            &[&repo, &role, &seq],
        )?
        .iter()
        .map(|r| {
            let outcome: Option<String> = r.get(2);
            let consumes = matches!(outcome.as_deref(), None | Some("accepted" | "unknown"));
            json!({"recovery_id": r.get::<_, String>(0), "delivery_id": r.get::<_, String>(1),
                   "outcome": outcome, "consumes": consumes})
        })
        .collect())
}

/// The use that consumed the checkpoint, other than `this_delivery`'s, if any.
fn consumed_by(uses: &[Value], this_delivery: Option<&str>) -> Option<Value> {
    uses.iter()
        .find(|u| u["consumes"] == true && this_delivery.is_none_or(|d| u["delivery_id"] != d))
        .cloned()
}

/// The checks shared by the approval and the use: `Err` is a refusal (nothing recorded), `Ok`
/// gives the hold reasons (empty: approvable), the facts read and the GitHub comparison.
struct Assessed {
    holds: Vec<String>,
    facts: Value,
    github: Value,
    phase: String,
}

#[allow(clippy::too_many_arguments)]
fn assess(
    tx: &mut Transaction,
    repo: &str,
    role: &str,
    aid: &str,
    generation: i64,
    seq: i64,
    sha: &str,
    status: &Result<Value, String>,
    this_delivery: Option<&str>,
) -> Result<Assessed, Resp> {
    // the same lock as the checkpoint reports and compare: the record does not move meanwhile
    lock(tx, &format!("checkpoint:{repo}:{role}"))?;
    let head = tx
        .query_opt(
            "SELECT assignment_id, generation FROM assignment_heads WHERE repo_id = $1 AND role = $2",
            &[&repo, &role],
        )?
        .map(|h| (h.get::<_, String>(0), h.get::<_, i64>(1)));
    let Some(head) = head else {
        return Err(target_mismatch(
            "assignment_id",
            "no current assignment for this repo and role",
        ));
    };
    if head.0 != aid || head.1 != generation {
        return Err(target_mismatch(
            "assignment_id",
            &format!(
                "{aid}/{generation} is not the current assignment {}/{}",
                head.0, head.1
            ),
        ));
    }
    let Some(central) = latest_row(tx, repo, role)? else {
        return Err(target_mismatch(
            "checkpoint_seq",
            "the coordinator holds no checkpoint of this role: nothing to recover",
        ));
    };
    if central["seq"].as_i64() != Some(seq) || central["checkpoint_sha256"] != sha {
        return Err(target_mismatch(
            "checkpoint_seq",
            &format!(
                "checkpoint {seq} ({sha}) is not the latest central checkpoint {} ({})",
                central["seq"], central["checkpoint_sha256"]
            ),
        ));
    }
    let phase = central["phase"].as_str().unwrap_or_default().to_string();
    if phase != "takeover" && phase != "error" {
        return Err(target_mismatch(
            "checkpoint_seq",
            &format!("checkpoint {seq} is `{phase}`: its run finished, nothing to recover"),
        ));
    }
    if central["assignment_id"] != aid
        || central["generation"]
            .as_i64()
            .is_none_or(|g| g > generation)
    {
        return Err(target_mismatch(
            "checkpoint_seq",
            &format!(
                "checkpoint {seq} belongs to {}/{}, not to {aid} up to generation {generation}",
                central["assignment_id"], central["generation"]
            ),
        ));
    }
    // guardrail: the review target must still be the development target (step 7, item 24)
    if role == "guardrail" {
        let t = |tx: &mut Transaction, r: &str| -> Result<Option<String>, Resp> {
            Ok(tx
                .query_opt(
                    "SELECT a.target_sha FROM assignment_heads h JOIN assignments a
                       ON (a.assignment_id, a.generation) = (h.assignment_id, h.generation)
                     WHERE h.repo_id = $1 AND h.role = $2",
                    &[&repo, &r],
                )?
                .map(|r| r.get(0)))
        };
        let (review, dev) = (t(tx, "guardrail")?, t(tx, "development")?);
        if review.is_none() || review != dev {
            return Err(target_mismatch(
                "assignment_id",
                "the review target is not the current development assignment's target SHA: a new \
                 review assignment is needed",
            ));
        }
    }
    let mut holds = Vec::new();
    // an earlier use of this checkpoint whose delivery may have run: a run may have started over
    // it; a person confirms (review 185d12b M-1)
    let uses = checkpoint_uses(tx, repo, role, seq)?;
    if let Some(u) = consumed_by(&uses, this_delivery) {
        holds.push(format!(
            "checkpoint {seq} was used by recovery {} (delivery {}, outcome {}): a run may have \
             started over it",
            u["recovery_id"].as_str().unwrap_or_default(),
            u["delivery_id"].as_str().unwrap_or_default(),
            u["outcome"].as_str().unwrap_or("none recorded")
        ));
    }
    // the supervisor: the old run must not be running; its end and its external requests must
    // be confirmable
    let supervisor = match status {
        Err(e) => {
            holds.push(format!(
                "the supervisor's status was not observed ({e}): the run's end cannot be confirmed"
            ));
            Value::Null
        }
        Ok(s) => {
            if !s["running"].is_null() {
                return Err(one(
                    409,
                    "RECOVERY_RUNNING",
                    "role",
                    "the role is running an attempt: a recovery is refused while a run is in \
                     progress",
                    "wait for the run to end (or stop it) and approve again",
                ));
            }
            if s["checkpoint_seq"].as_i64() != Some(seq)
                || s["checkpoint_sha256"].as_str() != Some(sha)
            {
                holds.push(format!(
                    "the role's local checkpoint ({} {}) is not the central one: the role's state \
                     is not confirmed",
                    s["checkpoint_seq"], s["checkpoint_sha256"]
                ));
            }
            let pending = &s["safe_shutdown"]["pending_external_requests"];
            if pending.as_array().is_none_or(|l| !l.is_empty()) {
                holds.push(format!(
                    "the role's external requests are not confirmed settled \
                     (pending_external_requests: {pending})"
                ));
            }
            json!({"running": s["running"], "checkpoint_seq": s["checkpoint_seq"],
                   "checkpoint_sha256": s["checkpoint_sha256"],
                   "pending_external_requests": pending,
                   "direct_network_requests": s["safe_shutdown"]["direct_network_requests"],
                   "active_children": s["safe_shutdown"]["active_children"]})
        }
    };
    // the coordinator's own calls to this role whose outcome is unknown
    let deliveries: Vec<Value> = pending_deliveries(tx, repo)?
        .into_iter()
        .filter(|d| {
            d["target"]
                .as_str()
                .is_some_and(|t| t.starts_with(&format!("{role} ")))
                && this_delivery.is_none_or(|x| d["request_id"] != x)
        })
        .collect();
    if !deliveries.is_empty() {
        holds.push(format!(
            "{} delivery(ies) to this role have no known outcome",
            deliveries.len()
        ));
    }
    // development: step 7's GitHub head check of the unfinished checkpoint
    let github = if role == "development" {
        let cp: Value = tx
            .query_one(
                "SELECT checkpoint FROM role_checkpoints WHERE repo_id = $1 AND role = $2 AND seq = $3",
                &[&repo, &role, &seq],
            )?
            .get(0);
        let (_, why, g) = github_head_check(tx, repo, &head, &cp)?;
        if g["result"] != "match" {
            holds.push(format!(
                "GitHub head check: {} ({}); a person reconciles the branch first",
                g["result"].as_str().unwrap_or_default(),
                why.unwrap_or_default()
            ));
        }
        g
    } else {
        Value::Null
    };
    Ok(Assessed {
        holds,
        facts: json!({"central": central, "supervisor": supervisor, "earlier_uses": uses,
            "coordinator_pending_deliveries": deliveries,
            // children are not tracked until 04-05: recorded, not a hold (decision 2026-11-15
            // item 3, runtime contract §16 item 20)
            "limits": ["active_children not tracked (04-05)",
                       "the model's direct network requests are not observable"]}),
        github,
        phase,
    })
}

/// One recovery as stored, with its use (if any).
pub(crate) fn load(tx: &mut Transaction, id: &str) -> Result<Option<Value>, Resp> {
    let Some(r) = tx.query_opt(
        "SELECT recovery_id, repo_id, role, request_id, assignment_id, generation, checkpoint_seq,
                checkpoint_sha256, checkpoint_phase, state, reasons, facts, github, approved_by,
                approver_kind, reason, iso(approved_at)
         FROM role_recoveries WHERE recovery_id = $1",
        &[&id],
    )?
    else {
        return Ok(None);
    };
    let used = tx
        .query_opt(
            "SELECT delivery_id, iso(used_at), github FROM role_recovery_uses WHERE recovery_id = $1",
            &[&id],
        )?
        .map(|u| {
            json!({"delivery_id": u.get::<_, String>(0), "used_at": u.get::<_, String>(1),
                   "github": u.get::<_, Option<Value>>(2)})
        });
    Ok(Some(json!({
        "recovery_id": r.get::<_, String>(0), "repo_id": r.get::<_, String>(1),
        "role": r.get::<_, String>(2), "request_id": r.get::<_, String>(3),
        "assignment_id": r.get::<_, String>(4), "generation": r.get::<_, i64>(5),
        "checkpoint": {"seq": r.get::<_, i64>(6), "checkpoint_sha256": r.get::<_, String>(7),
                       "phase": r.get::<_, String>(8)},
        "state": r.get::<_, String>(9), "reasons": r.get::<_, Value>(10),
        "facts": r.get::<_, Value>(11), "github": r.get::<_, Option<Value>>(12),
        "approved_by": r.get::<_, String>(13), "approver_kind": r.get::<_, String>(14),
        "reason": r.get::<_, String>(15), "approved_at": r.get::<_, String>(16),
        "evidence_level": "verified", "used": used})))
}

fn check_body(req: &Req) -> Result<(Value, String, i64, i64, String, String), Resp> {
    let body = parse_body(req.body)?;
    only_keys(
        &body,
        &[
            "assignment_id",
            "generation",
            "checkpoint_seq",
            "checkpoint_sha256",
            "reason",
        ],
    )?;
    let aid = str_field(&body, "assignment_id", 128)?.to_string();
    let n = |k: &str| -> Result<i64, Resp> {
        body[k].as_u64().map(i64_of).ok_or_else(|| {
            one(
                400,
                "MISSING_FIELD",
                k,
                &format!("`{k}` must be a non-negative integer"),
                &format!("supply `{k}`"),
            )
        })
    };
    let (generation, seq) = (n("generation")?, n("checkpoint_seq")?);
    let sha = str_field(&body, "checkpoint_sha256", 64)?.to_string();
    if sha.len() != 64 || !is_sha(&sha) {
        return Err(one(
            400,
            "SCHEMA_VIOLATION",
            "checkpoint_sha256",
            "not a lowercase hex SHA-256",
            "name the checkpoint's digest",
        ));
    }
    let reason = str_field(&body, "reason", 2000)?.to_string();
    Ok((body, aid, generation, seq, sha, reason))
}

/// `approveRecovery`. Order: 404 role → 403 (scope, repo, a person) → 400 Idempotency-Key and
/// body → the same key: the stored approval (200) or 409 → the supervisor's status (outside the
/// transaction) → one transaction: refusal 409 (nothing recorded) or the approval / hold (201).
pub fn approve(
    db: &mut postgres::Client,
    ctx: &Ctx,
    who: &Caller,
    repo: &str,
    role: &str,
    req: &Req,
    guard: &dyn TxGuard,
) -> Result<Resp, Abort> {
    let run = |db: &mut postgres::Client, f: &dyn Fn(&mut Transaction) -> R| run_tx(db, guard, f);
    let pre = (|| {
        need_approver(who, repo, role)?;
        let key = idem_key(req)?;
        let body = check_body(req)?;
        Ok::<_, Resp>((key, body))
    })();
    let (key, (body, aid, generation, seq, sha, reason)) = match pre {
        Ok(x) => x,
        Err(r) => return Ok(r),
    };
    let id = recovery_id(repo, role, key);
    let replay = run(db, &|tx| {
        lock(tx, &format!("recovery:{repo}:{role}"))?;
        match tx.query_opt(
            "SELECT body FROM role_recoveries WHERE repo_id = $1 AND role = $2 AND request_id = $3",
            &[&repo, &role, &key],
        )? {
            Some(r) if r.get::<_, Value>(0) == body => ok(200, load(tx, &id)?.unwrap_or_default()),
            Some(_) => Err(one(
                409,
                "IDEMPOTENCY_KEY_CONFLICT",
                "Idempotency-Key",
                "this Idempotency-Key was used for another recovery",
                "use a new Idempotency-Key",
            )),
            None => ok(204, Value::Null),
        }
    })?;
    if replay.status != 204 {
        return Ok(replay);
    }
    let status = crate::api::role_status(ctx, repo, role);
    run(db, &|tx| {
        lock(tx, &format!("recovery:{repo}:{role}"))?;
        // the same key again, under the lock: a concurrent identical retry gets the stored
        // approval, never a key error (review 185d12b I-2)
        if let Some(r) = tx.query_opt(
            "SELECT body FROM role_recoveries WHERE repo_id = $1 AND role = $2 AND request_id = $3",
            &[&repo, &role, &key],
        )? {
            return if r.get::<_, Value>(0) == body {
                ok(200, load(tx, &id)?.unwrap_or_default())
            } else {
                Err(one(
                    409,
                    "IDEMPOTENCY_KEY_CONFLICT",
                    "Idempotency-Key",
                    "this Idempotency-Key was used for another recovery",
                    "use a new Idempotency-Key",
                ))
            };
        }
        let a = assess(tx, repo, role, &aid, generation, seq, &sha, &status, None)?;
        let state = if a.holds.is_empty() {
            "approved"
        } else {
            "needs_confirmation"
        };
        let github = Some(a.github.clone()).filter(|g| !g.is_null());
        tx.execute(
            "INSERT INTO role_recoveries (recovery_id, repo_id, role, request_id, body,
                 assignment_id, generation, checkpoint_seq, checkpoint_sha256, checkpoint_phase,
                 state, reasons, facts, github, approved_by, approver_kind, reason)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, 'user', $16)",
            &[
                &id,
                &repo,
                &role,
                &key,
                &body,
                &aid,
                &generation,
                &seq,
                &sha,
                &a.phase,
                &state,
                &json!(a.holds),
                &a.facts,
                &github,
                &who.name,
                &reason,
            ],
        )?;
        history(
            tx,
            &who.name,
            if state == "approved" {
                "recovery.approved"
            } else {
                "recovery.held"
            },
            &json!({"recovery_id": id, "repo_id": repo, "role": role, "checkpoint_seq": seq}),
        )?;
        ok(201, load(tx, &id)?.unwrap_or_default())
    })
}

/// `GET /v0/repos/{repo}/roles/{role}/recoveries/{id}` (`roles:control`, any client kind).
pub fn get(tx: &mut Transaction, who: &Caller, repo: &str, role: &str, id: &str) -> R {
    need_role(role)?;
    need_client_scope(who, "roles:control", false)?;
    if !who.in_repo(repo) {
        return Err(problem(
            403,
            "REPO_SCOPE_DENIED",
            "repo not in this credential's scope",
        ));
    }
    match load(tx, id)? {
        Some(r) if r["repo_id"] == repo && r["role"] == role => ok(200, r),
        _ => Err(problem(404, "NOT_FOUND", "no such recovery")),
    }
}

/// At use time (deliverAssignment with `recovery_id`): `Ok(Ok(github))` when the approval may
/// start its one run now, `Ok(Err(why))` when not (the delivery then calls nothing). Everything
/// the approval checked is checked again: the checkpoint, generation and GitHub head may have
/// changed since.
pub fn usable(
    tx: &mut Transaction,
    repo: &str,
    role: &str,
    id: &str,
    status: &Result<Value, String>,
    this_delivery: &str,
) -> Result<Result<Value, String>, Resp> {
    let Some(r) = load(tx, id)? else {
        return Ok(Err(format!("recovery {id}: no such approval")));
    };
    if r["repo_id"] != repo || r["role"] != role {
        return Ok(Err(format!(
            "recovery {id}: approved for another repo or role"
        )));
    }
    if r["state"] != "approved" {
        return Ok(Err(format!(
            "recovery {id}: not approved ({}): a person reconciles and approves again",
            r["state"].as_str().unwrap_or_default()
        )));
    }
    if !r["used"].is_null() {
        return Ok(Err(format!(
            "recovery {id}: already used by delivery {}",
            r["used"]["delivery_id"].as_str().unwrap_or_default()
        )));
    }
    let a = assess(
        tx,
        repo,
        role,
        r["assignment_id"].as_str().unwrap_or_default(),
        r["generation"].as_i64().unwrap_or(-1),
        r["checkpoint"]["seq"].as_i64().unwrap_or(-1),
        r["checkpoint"]["checkpoint_sha256"]
            .as_str()
            .unwrap_or_default(),
        status,
        Some(this_delivery),
    );
    match a {
        Err(refusal) => Ok(Err(format!(
            "recovery {id}: {}",
            refusal.body["rejections"][0]["message"]
                .as_str()
                .or(refusal.body["message"].as_str())
                .unwrap_or("refused")
        ))),
        Ok(a) if !a.holds.is_empty() => Ok(Err(format!(
            "recovery {id}: re-checked at use, cannot be confirmed now: {}",
            a.holds.join("; ")
        ))),
        Ok(a) => {
            let mut g = a.github;
            if g.is_object() {
                // what changed since the approval is visible in the use record
                g["head_at_approval"] = r["github"]["head"].clone();
            }
            Ok(Ok(g))
        }
    }
}

/// The one use of an approval, committed before the start is sent: `Err` when it was used
/// already (or its checkpoint was) — the delivery then calls nothing.
pub fn record_use(
    tx: &mut Transaction,
    repo: &str,
    role: &str,
    id: &str,
    delivery_id: &str,
    github: &Value,
) -> R {
    lock(tx, &format!("recovery:{repo}:{role}"))?;
    let seq: Option<i64> = tx
        .query_opt(
            "SELECT checkpoint_seq FROM role_recoveries WHERE recovery_id = $1",
            &[&id],
        )?
        .map(|r| r.get(0));
    let Some(seq) = seq else {
        return Err(problem(409, "RECOVERY_USED", "no such recovery"));
    };
    // under the recovery lock: another use of the checkpoint that may have run consumes it
    if let Some(u) = consumed_by(&checkpoint_uses(tx, repo, role, seq)?, Some(delivery_id)) {
        return Err(problem(
            409,
            "RECOVERY_USED",
            &format!(
                "checkpoint {seq} was used by recovery {} (delivery {}): a run may have started",
                u["recovery_id"].as_str().unwrap_or_default(),
                u["delivery_id"].as_str().unwrap_or_default()
            ),
        ));
    }
    let n = tx.execute(
        "INSERT INTO role_recovery_uses (recovery_id, delivery_id, repo_id, role, checkpoint_seq,
             github)
         VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT DO NOTHING",
        &[
            &id,
            &delivery_id,
            &repo,
            &role,
            &seq,
            &Some(github.clone()).filter(|g| !g.is_null()),
        ],
    )?;
    if n == 0 {
        return Err(problem(
            409,
            "RECOVERY_USED",
            "this approval was used already",
        ));
    }
    history(
        tx,
        "coordinator",
        "recovery.used",
        &json!({"recovery_id": id, "delivery_id": delivery_id}),
    )?;
    ok(200, json!({"recovery_id": id, "delivery_id": delivery_id}))
}

/// The recovery a delivery used, if any (for the stored delivery and the chain).
pub fn of_delivery(tx: &mut Transaction, delivery_id: &str) -> Result<Value, Resp> {
    Ok(tx
        .query_opt(
            "SELECT u.recovery_id, r.approved_by, u.checkpoint_seq, iso(u.used_at)
             FROM role_recovery_uses u JOIN role_recoveries r USING (recovery_id)
             WHERE u.delivery_id = $1",
            &[&delivery_id],
        )?
        .map_or(Value::Null, |u| {
            json!({"recovery_id": u.get::<_, String>(0), "approved_by": u.get::<_, String>(1),
                   "checkpoint_seq": u.get::<_, i64>(2), "used_at": u.get::<_, String>(3)})
        }))
}
