use crate::api::{
    R, is_sha, lock, need_client_scope, ok, only_keys, parse_body, problem, str_field,
};
use crate::auth::{Caller, Kind};
use crate::review_lifecycle::{ReviewError, ReviewLifecycle, ReviewPhase, ReviewVerdict};
use postgres::Transaction;
use serde_json::{Value, json};

fn apply(state: &mut ReviewLifecycle, event: &Value) -> Result<(), ReviewError> {
    let sha = event["head_sha"].as_str().unwrap_or_default();
    let generation = event["generation"]
        .as_u64()
        .ok_or(ReviewError::InvalidGeneration)?;
    match event["action"].as_str() {
        Some("submit") => state.submit(sha, generation),
        Some("head_changed") => state.observe_head_change(sha, generation),
        Some("approve") => state.resolve(sha, generation, ReviewVerdict::Approve),
        Some("reject") => state.resolve(sha, generation, ReviewVerdict::Reject),
        _ => Err(ReviewError::WrongPhase),
    }
}

pub(crate) fn load(
    tx: &mut Transaction,
    repo: &str,
    sprint: &str,
    task: &str,
) -> Result<Option<ReviewLifecycle>, crate::api::Resp> {
    let rows = tx.query(
        "SELECT request, response FROM review_transitions
         WHERE repo_id=$1 AND sprint_id=$2 AND task_id=$3 ORDER BY seq",
        &[&repo, &sprint, &task],
    )?;
    if rows.is_empty() {
        return Ok(None);
    }
    let mut state = ReviewLifecycle::default();
    for row in rows {
        let request: Value = row.get(0);
        let response: Value = row.get(1);
        if response["outcome"] == "preserved_late" {
            continue;
        }
        // A head change can invalidate the previous approval even at the limit.
        // A stale verdict is deliberately replayed as a no-op.
        let _ = apply(&mut state, &request);
    }
    Ok(Some(state))
}

pub(crate) fn record(tx: &mut Transaction, who: &Caller, body: &[u8]) -> R {
    need_client_scope(who, "coordinator:assign", false)?;
    if who.kind != Kind::Service {
        return Err(problem(
            403,
            "SCOPE_DENIED",
            "review transitions require a service principal",
        ));
    }
    let request = parse_body(body)?;
    only_keys(
        &request,
        &[
            "event_id",
            "repo_id",
            "sprint_id",
            "task_id",
            "head_sha",
            "assignment_id",
            "generation",
            "action",
            "report_id",
        ],
    )?;
    let repo = str_field(&request, "repo_id", 256)?;
    if !who.in_repo(repo) {
        return Err(problem(
            403,
            "REPO_SCOPE_DENIED",
            "repo not in credential scope",
        ));
    }
    let sprint = str_field(&request, "sprint_id", 256)?;
    let task = str_field(&request, "task_id", 32)?;
    let event_id = str_field(&request, "event_id", 128)?;
    let head = str_field(&request, "head_sha", 64)?;
    let assignment = str_field(&request, "assignment_id", 128)?;
    let action = str_field(&request, "action", 24)?;
    let generation = request["generation"].as_i64().filter(|n| *n >= 0);
    let Some(generation) = generation.filter(|_| is_sha(head)) else {
        return Err(problem(
            400,
            "SCHEMA_VIOLATION",
            "expected a valid head SHA and generation",
        ));
    };
    if !matches!(action, "submit" | "head_changed" | "approve" | "reject") {
        return Err(problem(400, "SCHEMA_VIOLATION", "unknown review action"));
    }
    lock(tx, &format!("scope:{repo}:development"))?;
    lock(tx, &format!("scope:{repo}:guardrail"))?;
    lock(tx, &format!("delivery-evidence:{repo}"))?;
    if let Some(row) = tx.query_opt(
        "SELECT request, response, principal FROM review_transitions
         WHERE repo_id=$1 AND sprint_id=$2 AND task_id=$3 AND event_id=$4",
        &[&repo, &sprint, &task, &event_id],
    )? {
        if row.get::<_, Value>(0) == request && row.get::<_, &str>(2) == who.name {
            return ok(200, row.get(1));
        }
        return Err(problem(
            409,
            "EVENT_CONTENT_CONFLICT",
            "review event already recorded",
        ));
    }
    let Some(row) = tx.query_opt(
        "SELECT EXISTS (SELECT 1 FROM assignment_heads h
          WHERE h.repo_id=a.repo_id AND h.role=a.role AND h.assignment_id=a.assignment_id
            AND h.generation=a.generation)
         FROM assignments a WHERE a.repo_id=$1 AND a.role='guardrail' AND a.sprint_id=$2
          AND a.task_id=$3 AND a.assignment_id=$4 AND a.generation=$5 AND a.target_sha=$6",
        &[&repo, &sprint, &task, &assignment, &generation, &head],
    )?
    else {
        return Err(problem(
            409,
            "REVIEW_ASSIGNMENT_MISMATCH",
            "review assignment does not match candidate",
        ));
    };
    let current: bool = row.get(0);
    let verdict = matches!(action, "approve" | "reject");
    if verdict {
        let report = str_field(&request, "report_id", 128)?;
        let matched: bool = tx
            .query_one(
                "SELECT EXISTS (SELECT 1 FROM events WHERE scope_repo=$1 AND scope_role='guardrail'
              AND assignment_id=$2 AND generation=$3 AND event_id=$4 AND attributed_sha=$5
              AND report_type='guardrail_review' AND report->'body'->>'verdict'=$6)",
                &[&repo, &assignment, &generation, &report, &head, &action],
            )?
            .get(0);
        if !matched {
            return Err(problem(
                409,
                "REVIEW_REPORT_MISMATCH",
                "review verdict needs a matching claimed report",
            ));
        }
    } else {
        if !current {
            return Err(problem(
                409,
                "REVIEW_ASSIGNMENT_MISMATCH",
                "review assignment is stale",
            ));
        }
        let development_matches: bool = tx
            .query_one(
                "SELECT EXISTS (SELECT 1 FROM assignment_heads h JOIN assignments a
             ON a.assignment_id=h.assignment_id AND a.generation=h.generation
             WHERE h.repo_id=$1 AND h.role='development' AND a.sprint_id=$2
               AND a.task_id=$3 AND a.target_sha=$4)",
                &[&repo, &sprint, &task, &head],
            )?
            .get(0);
        if !development_matches {
            return Err(problem(
                409,
                "REVIEW_HEAD_MISMATCH",
                "candidate is not the current development head",
            ));
        }
    }
    let mut state = load(tx, repo, sprint, task)?.unwrap_or_default();
    let active_assignment = tx
        .query_opt(
            "SELECT request->>'assignment_id' FROM review_transitions
         WHERE repo_id=$1 AND sprint_id=$2 AND task_id=$3
           AND request->>'action' IN ('submit','head_changed')
           AND response->>'outcome'='recorded' ORDER BY seq DESC LIMIT 1",
            &[&repo, &sprint, &task],
        )?
        .map(|r| r.get::<_, String>(0));
    let current = current && (!verdict || active_assignment.as_deref() == Some(assignment));
    let result = if verdict && !current {
        if state.phase() == ReviewPhase::Development {
            return Err(problem(
                409,
                "REVIEW_TRANSITION_DENIED",
                "review has no recorded submission",
            ));
        }
        Err(ReviewError::StaleGeneration)
    } else {
        apply(&mut state, &request)
    };
    if let Err(error) = result
        && !(action == "head_changed"
            && state.unauthorized_head_change()
            && matches!(
                error,
                ReviewError::ReviewLimit | ReviewError::InvalidGeneration
            ))
        && !(verdict
            && matches!(
                error,
                ReviewError::StaleGeneration | ReviewError::WrongCandidate
            ))
    {
        return Err(problem(
            409,
            "REVIEW_TRANSITION_DENIED",
            &format!("{error:?}"),
        ));
    }
    let response = json!({
        "outcome": match result {
            Ok(()) => "recorded",
            Err(ReviewError::StaleGeneration | ReviewError::WrongCandidate) if verdict => "preserved_late",
            Err(_) => "blocked",
        },
        "phase": format!("{:?}", state.phase()),
        "developer_push_allowed": state.developer_push_allowed(),
        "approved_head": state.approved_head(),
        "unauthorized_head_change": state.unauthorized_head_change(),
    });
    tx.execute(
        "INSERT INTO review_transitions (repo_id,sprint_id,task_id,event_id,principal,request,response)
         VALUES ($1,$2,$3,$4,$5,$6,$7)",
        &[&repo, &sprint, &task, &event_id, &who.name, &request, &response],
    )?;
    ok(201, response)
}

pub(crate) fn check_delivery(
    tx: &mut Transaction,
    repo: &str,
    role: &str,
    target: &Value,
) -> Result<(), crate::api::Resp> {
    let sprint = str_field(target, "sprint_id", 256)?;
    let task = str_field(target, "task_id", 32)?;
    if let Some(state) = load(tx, repo, sprint, task)? {
        let allowed = if role == "development" {
            state.developer_push_allowed()
        } else {
            state.review_delivery_allowed(
                str_field(target, "target_sha", 64)?,
                target["generation"].as_u64().ok_or_else(|| {
                    problem(409, "REVIEW_DELIVERY_DENIED", "missing review generation")
                })?,
            )
        };
        if !allowed {
            return Err(problem(
                409,
                "REVIEW_DELIVERY_DENIED",
                "the current review phase does not authorize this delivery",
            ));
        }
    }
    Ok(())
}
