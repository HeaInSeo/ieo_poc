//! Approved development handoffs shared by committed verdict dispatch and periodic recovery.
use crate::api::{
    self, Abort, Ctx, R, Req, Resp, TxGuard, find_task, issue_assignment, lock, need_client_scope,
    ok, only_keys, parse_body, problem, run_tx, str_field,
};
use crate::auth::{Caller, Kind};
use postgres::{Client, Transaction};
use serde_json::{Value, json};

pub(crate) fn plan(tx: &mut Transaction, ctx: &Ctx, who: &Caller, req: &Req) -> R {
    need_client_scope(who, "coordinator:assign", false)?;
    let body = parse_body(req.body)?;
    only_keys(&body, &["plan_id", "trigger_task", "assignment"])?;
    let id = str_field(&body, "plan_id", 128)?;
    let trigger = str_field(&body, "trigger_task", 32)?;
    let assignment = &body["assignment"];
    let repo = str_field(assignment, "repo_id", 256)?;
    if !who.in_repo(repo) {
        return Err(problem(
            403,
            "REPO_SCOPE_DENIED",
            "followup outside credential's repo",
        ));
    }
    let sprint = str_field(assignment, "sprint_id", 256)?;
    let task = str_field(assignment, "task_id", 32)?;
    str_field(assignment, "assignment_id", 128)?;
    if assignment["role"] != "development"
        || assignment["generation"].as_u64() != Some(0)
        || !api::is_sha(str_field(assignment, "target_sha", 64)?)
        || assignment["expected_current"].is_null()
    {
        return Err(problem(
            400,
            "SCHEMA_VIOLATION",
            "followup needs a new development assignment and its expected predecessor",
        ));
    }
    let expected = &assignment["expected_current"];
    str_field(expected, "assignment_id", 128)?;
    let generation = expected["generation"]
        .as_i64()
        .filter(|n| *n >= 0)
        .ok_or_else(|| problem(400, "SCHEMA_VIOLATION", "invalid predecessor generation"))?;
    let revision = assignment["sprint_revision"]
        .as_i64()
        .filter(|r| *r > 0)
        .ok_or_else(|| problem(400, "SCHEMA_VIOLATION", "invalid sprint revision"))?;
    lock(tx, &format!("followup-plan:{repo}"))?;
    if let Some(row) = tx.query_opt(
        "SELECT trigger_task, assignment, approved_by, disposition
         FROM followup_plans WHERE repo_id=$1 AND plan_id=$2",
        &[&repo, &id],
    )? {
        if row.get::<_, &str>(0) == trigger
            && row.get::<_, Value>(1) == *assignment
            && row.get::<_, &str>(2) == who.name
        {
            return ok(200, row.get(3));
        }
        return Err(problem(
            409,
            "EVENT_CONTENT_CONFLICT",
            "followup id already registered",
        ));
    }
    let registered = tx.query_opt(
        "SELECT spec FROM sprint_revisions WHERE repo_id=$1 AND sprint_id=$2 AND revision=$3",
        &[&repo, &sprint, &revision],
    )?;
    let is_dependency = registered.is_some_and(|row| {
        let spec: Value = row.get(0);
        find_task(&spec, task).is_some_and(|t| {
            t["prerequisites"].as_array().is_some_and(|ps| {
                ps.iter()
                    .any(|p| p["task_id"] == trigger && p["required_state"] == "accepted")
            })
        })
    });
    let bound = tx
        .query_one(
            "SELECT EXISTS(SELECT 1 FROM assignments WHERE repo_id=$1 AND role='development'
         AND task_id=$2 AND assignment_id=$3 AND generation=$4)",
            &[
                &repo,
                &trigger,
                &expected["assignment_id"].as_str(),
                &generation,
            ],
        )?
        .get::<_, bool>(0);
    if !is_dependency || !bound {
        return Err(problem(
            422,
            "UNKNOWN_TASK",
            "followup must name a required predecessor and its exact assignment",
        ));
    }
    if tx.execute(
        "INSERT INTO followup_plans(repo_id,plan_id,trigger_task,sprint_id,sprint_revision,task_id,approved_by,policy_digest,assignment)
         VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT DO NOTHING",
        &[&repo, &id, &trigger, &sprint, &revision, &task, &who.name, &ctx.policy.digest, &assignment],
    )? == 0 {
        return Err(problem(409, "EVENT_CONTENT_CONFLICT", "successor already registered for that revision"));
    }
    ok(201, json!({"state":"waiting"}))
}

fn evaluate(tx: &mut Transaction, ctx: &Ctx, repo: &str, id: &str, event: Option<&str>) -> R {
    let row = tx.query_one(
        "SELECT assignment, approved_by, disposition, assigned_at IS NOT NULL, trigger_task, policy_digest
         FROM followup_plans WHERE repo_id=$1 AND plan_id=$2 FOR UPDATE",
        &[&repo, &id],
    )?;
    let body: Value = row.get(0);
    let issuer: &str = row.get(1);
    let previous: Value = row.get(2);
    if row.get::<_, bool>(3) {
        return ok(200, previous);
    }
    let trigger: &str = row.get(4);
    lock(tx, &format!("scope:{repo}:development"))?;
    lock(tx, "registry")?;
    let expected = &body["expected_current"];
    let accepted = tx.query_opt(
        "SELECT e.event_id, j.outcome, e.stored_at::text AS ready_at
         FROM judgments j JOIN events e ON e.sprint_id=j.event_sprint AND e.sprint_seq=j.event_seq
         WHERE j.repo_id=$1 AND j.task_id=$2 AND j.policy_digest=$3
           AND e.assignment_id=$4 AND e.generation=$5
         ORDER BY j.event_seq DESC LIMIT 1",
        &[
            &repo,
            &trigger,
            &ctx.policy.digest,
            &expected["assignment_id"].as_str(),
            &expected["generation"].as_i64(),
        ],
    )?;
    let accepted = accepted.filter(|row| row.get::<_, &str>(1) == "accepted");
    let observed: String = tx.query_one("SELECT clock_timestamp()::text", &[])?.get(0);
    let disposition = if row.get::<_, &str>(5) != ctx.policy.digest {
        json!({"state":"waiting","reason":"policy_changed"})
    } else if let Some(evidence) = accepted {
        let mut attempt = tx.transaction()?;
        let outcome = issue_assignment(&mut attempt, ctx, issuer, None, body.clone());
        match outcome {
            Ok(r) if r.status == 201 => {
                attempt.commit()?;
                json!({"state":"assigned","assignment_id":body["assignment_id"],"generation":0,
                    "cause":if event.is_some() {"event"} else {"reconcile"},
                    "event_id":event,"prerequisite_event":evidence.get::<_, &str>(0),
                    "prerequisite_at":evidence.get::<_, &str>(2),"observed_at":observed})
            }
            Ok(r) if r.status == 202 && r.body["outcome"] == "limit_reached" => {
                attempt.commit()?;
                json!({"state":"waiting","reason":r.body})
            }
            Ok(r) if r.status == 200 => {
                attempt.rollback()?;
                json!({"state":"assigned","assignment_id":body["assignment_id"],"generation":0,"cause":"existing"})
            }
            Ok(r) | Err(r) if r.status < 500 => {
                attempt.rollback()?;
                if previous["reason"]["outcome"] == "limit_reached"
                    && r.body["rejections"][0]["code"] == "LIMIT_REACHED"
                {
                    previous.clone()
                } else {
                    json!({"state":"waiting","reason":r.body})
                }
            }
            Ok(r) | Err(r) => {
                attempt.rollback()?;
                return Err(r);
            }
        }
    } else {
        json!({"state":"waiting","reason":"predecessor_not_accepted"})
    };
    if disposition != previous {
        tx.execute(
            "UPDATE followup_plans SET disposition=$3,
               assigned_at=CASE WHEN $3::jsonb->>'state'='assigned' THEN clock_timestamp() END
             WHERE repo_id=$1 AND plan_id=$2",
            &[&repo, &id, &disposition],
        )?;
    }
    ok(200, disposition)
}

pub(crate) fn process(
    db: &mut Client,
    ctx: &Ctx,
    repo: &str,
    trigger: Option<(&str, &str)>,
    guard: &dyn TxGuard,
) -> Result<Resp, Abort> {
    let task = trigger.map(|(task, _)| task);
    let queries = db.query(
        "SELECT p.plan_id FROM followup_plans p
         JOIN sprint_revisions r ON (r.sprint_id,r.revision)=(p.sprint_id,p.sprint_revision)
         WHERE p.repo_id=$1 AND p.assigned_at IS NULL AND ($2::text IS NULL OR EXISTS (
           SELECT 1 FROM jsonb_array_elements(r.spec->'tasks') t
           CROSS JOIN LATERAL jsonb_array_elements(t->'prerequisites') d
           WHERE t->>'task_id'=p.task_id AND d->>'task_id'=$2
         )) ORDER BY p.plan_id",
        &[&repo, &task],
    );
    let rows = match queries {
        Ok(rows) => rows,
        Err(err) => return Ok(err.into()),
    };
    let mut plans = Vec::with_capacity(rows.len());
    for row in rows {
        let id: &str = row.get(0);
        let response = run_tx(db, guard, &|tx| {
            evaluate(tx, ctx, repo, id, trigger.map(|(_, event)| event))
        })?;
        if response.status != 200 {
            return Ok(response);
        }
        plans.push(json!({"plan_id":id,"disposition":response.body}));
    }
    Ok(Resp {
        status: 200,
        body: json!({"plans":plans}),
    })
}

pub(crate) fn reconcile(
    db: &mut Client,
    ctx: &Ctx,
    who: &Caller,
    req: &Req,
    guard: &dyn TxGuard,
) -> Result<Resp, Abort> {
    let input = || -> Result<(String, i64), Resp> {
        need_client_scope(who, "followups:reconcile", false)?;
        if who.kind != Kind::Service {
            return Err(problem(
                403,
                "SCOPE_DENIED",
                "followup watcher requires service credentials",
            ));
        }
        let body = parse_body(req.body)?;
        only_keys(&body, &["repo_id", "stall_seconds"])?;
        let repo = str_field(&body, "repo_id", 256)?;
        if !who.in_repo(repo) {
            return Err(problem(
                403,
                "REPO_SCOPE_DENIED",
                "followup outside credential's repo",
            ));
        }
        let seconds = match body.get("stall_seconds") {
            None => 60,
            Some(v) => v
                .as_i64()
                .filter(|n| (1..=86400).contains(n))
                .ok_or_else(|| {
                    problem(400, "SCHEMA_VIOLATION", "stall_seconds must be 1..86400")
                })?,
        };
        Ok((repo.into(), seconds))
    };
    let (repo, seconds) = match input() {
        Ok(input) => input,
        Err(r) => return Ok(r),
    };
    let mut reply = process(db, ctx, &repo, None, guard)?;
    if reply.status != 200 {
        return Ok(reply);
    }
    // An accepted delivery starts the observation window. A plan that is
    // waiting for approval/budget, or an unconfirmed delivery, has no deadline.
    let stalls = db.query(
        "SELECT d.delivery_id, d.assignment_id, d.attempt_id, iso(result.recorded_at)
         FROM followup_plans p
         JOIN assignment_heads h ON h.repo_id=p.repo_id AND h.role='development'
           AND h.assignment_id=p.disposition->>'assignment_id' AND h.generation=0
         JOIN LATERAL (
           SELECT * FROM role_deliveries d WHERE d.repo_id=h.repo_id AND d.role=h.role
             AND d.assignment_id=h.assignment_id AND d.generation=h.generation
           ORDER BY seq DESC LIMIT 1
         ) d ON true
         JOIN LATERAL (
           SELECT outcome, recorded_at FROM role_delivery_results WHERE delivery_id=d.delivery_id
           ORDER BY n DESC LIMIT 1
         ) result ON true
         WHERE p.repo_id=$1 AND p.assigned_at IS NOT NULL AND result.outcome='accepted'
           AND result.recorded_at <= clock_timestamp() - ($2::bigint * interval '1 second')
           AND NOT EXISTS (SELECT 1 FROM role_first_actions a WHERE a.repo_id=d.repo_id
             AND a.role=d.role AND a.assignment_id=d.assignment_id AND a.generation=d.generation
             AND a.attempt_id=d.attempt_id)
         ORDER BY d.delivery_id",
        &[&repo, &seconds],
    );
    let rows = match stalls {
        Ok(rows) => rows,
        Err(err) => return Ok(err.into()),
    };
    reply.body["stalled"] = json!(
        rows.iter()
            .map(|r| json!({
                "delivery_id":r.get::<_, &str>(0),"assignment_id":r.get::<_, &str>(1),
                "attempt_id":r.get::<_, &str>(2),"accepted_at":r.get::<_, String>(3)
            }))
            .collect::<Vec<_>>()
    );
    Ok(reply)
}
