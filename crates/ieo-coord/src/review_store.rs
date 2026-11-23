use crate::api::{
    R, is_sha, lock, need_client_scope, ok, only_keys, parse_body, problem, str_field,
};
use crate::auth::{Caller, Kind};
use crate::review_lifecycle::{
    ReviewActivity, ReviewError, ReviewLifecycle, ReviewPhase, ReviewTimeout, ReviewVerdict,
};
use postgres::Transaction;
use serde_json::{Value, json};

fn apply(state: &mut ReviewLifecycle, event: &Value, can_replace: bool) -> Result<(), ReviewError> {
    let sha = event["head_sha"].as_str().unwrap_or_default();
    let round = state.review_round();
    match event["action"].as_str() {
        Some("open") if state.phase() == ReviewPhase::Development => Ok(()),
        Some("submit") => state.submit(sha, round + 1),
        Some("head_changed") => state.observe_head_change(sha, round + 1),
        Some("invalidate") => state.invalidate_head(sha),
        Some("reassign") => state.reassign_invalidated(sha, round + 1),
        Some("timeout") => state.expire(sha, can_replace),
        Some("replace") => state.replace_expired(sha, round + 1),
        Some("approve") => state.resolve(sha, round, ReviewVerdict::Approve),
        Some("reject") => state.resolve(sha, round, ReviewVerdict::Reject),
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
        let replay = apply(
            &mut state,
            &request,
            response["replacement_candidate"] == true,
        );
        let valid = match response["outcome"].as_str() {
            Some("recorded") => replay.is_ok(),
            Some("blocked") => matches!(
                replay,
                Err(ReviewError::ReviewLimit | ReviewError::InvalidGeneration)
            ),
            _ => false,
        };
        if !valid
            || response["phase"] != format!("{:?}", state.phase())
            || response["approved_head"] != json!(state.approved_head())
            || response["unauthorized_head_change"] != state.unauthorized_head_change()
        {
            return Err(problem(
                500,
                "REVIEW_LEDGER_INCONSISTENT",
                "stored transition could not be replayed",
            ));
        }
    }
    Ok(Some(state))
}

pub(crate) fn record(tx: &mut Transaction, who: &Caller, body: &[u8]) -> R {
    record_inner(tx, who, parse_body(body)?, false)
}

fn record_inner(tx: &mut Transaction, who: &Caller, request: Value, issuing_assignment: bool) -> R {
    need_client_scope(who, "coordinator:assign", false)?;
    if who.kind != Kind::Service {
        return Err(problem(
            403,
            "SCOPE_DENIED",
            "review transitions require a service principal",
        ));
    }
    only_keys(
        &request,
        &[
            "event_id",
            "repo_id",
            "sprint_id",
            "task_id",
            "head_sha",
            "pull_request",
            "assignment_id",
            "generation",
            "action",
            "report_id",
            "report_revision",
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
    let pull_request = request["pull_request"]
        .as_i64()
        .filter(|n| *n > 0)
        .ok_or_else(|| {
            problem(
                400,
                "SCHEMA_VIOLATION",
                "a positive pull_request number is required",
            )
        })?;
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
    if !matches!(
        action,
        "open"
            | "submit"
            | "head_changed"
            | "invalidate"
            | "reassign"
            | "timeout"
            | "replace"
            | "approve"
            | "reject"
    ) {
        return Err(problem(400, "SCHEMA_VIOLATION", "unknown review action"));
    }
    if action == "replace" && !issuing_assignment {
        return Err(problem(
            409,
            "REVIEW_REPLACEMENT_REQUIRES_ASSIGNMENT",
            "issue the replacement through the assignment endpoint",
        ));
    }
    lock(tx, &format!("scope:{repo}:development"))?;
    lock(tx, &format!("scope:{repo}:guardrail"))?;
    if action == "timeout" {
        // Use the same row-before-evidence order as the final delivery fence.
        tx.query_opt(
            "SELECT 1 FROM assignment_heads WHERE repo_id=$1 AND role='guardrail' FOR UPDATE",
            &[&repo],
        )?;
    }
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
    if let Some(row) = tx.query_opt(
        "SELECT (request->>'pull_request')::bigint FROM review_transitions
         WHERE repo_id=$1 AND sprint_id=$2 AND task_id=$3 ORDER BY seq LIMIT 1",
        &[&repo, &sprint, &task],
    )? && row.get::<_, Option<i64>>(0) != Some(pull_request)
    {
        return Err(problem(
            409,
            "REVIEW_PR_MISMATCH",
            "the task is already bound to another pull request",
        ));
    }
    if action == "open" && pull_request_recorded(tx, repo, sprint, task)? {
        return Err(problem(
            409,
            "REVIEW_ALREADY_OPEN",
            "the pull request is already bound",
        ));
    }
    let Some(row) = tx.query_opt(
        "SELECT EXISTS (SELECT 1 FROM assignment_heads h
          WHERE h.repo_id=a.repo_id AND h.role=a.role AND h.assignment_id=a.assignment_id
            AND h.generation=a.generation)
         FROM assignments a WHERE a.repo_id=$1 AND a.role=CASE WHEN $7='open' THEN 'development' ELSE 'guardrail' END AND a.sprint_id=$2
          AND a.task_id=$3 AND a.assignment_id=$4 AND a.generation=$5
          AND (a.target_sha=$6 OR $7='invalidate')",
        &[
            &repo,
            &sprint,
            &task,
            &assignment,
            &generation,
            &head,
            &action,
        ],
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
        let revision = request["report_revision"]
            .as_i64()
            .filter(|r| *r > 0)
            .ok_or_else(|| {
                problem(
                    400,
                    "SCHEMA_VIOLATION",
                    "a positive report_revision is required",
                )
            })?;
        let matched: bool = tx
            .query_one(
                "SELECT EXISTS (SELECT 1 FROM events WHERE scope_repo=$1 AND scope_role='guardrail'
              AND assignment_id=$2 AND generation=$3 AND event_id=$4 AND attributed_sha=$5
              AND report_type='guardrail_review' AND report->'body'->>'verdict'=$6
              AND event_revision=$7 AND event_revision = (SELECT max(later.event_revision) FROM events later
                WHERE later.scope_repo=$1 AND later.scope_role='guardrail' AND later.event_id=$4))",
                &[&repo, &assignment, &generation, &report, &head, &action, &revision],
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
        if !current && action != "invalidate" {
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
    if action == "invalidate"
        && !active_assignment(tx, repo, sprint, task)?
            .is_some_and(|(id, prior)| id == assignment && prior == generation)
    {
        return Err(problem(
            409,
            "REVIEW_ASSIGNMENT_MISMATCH",
            "invalidation must reference the active review",
        ));
    }
    let current = current
        && (!verdict
            || active_assignment(tx, repo, sprint, task)?.is_some_and(|(id, prior_generation)| {
                id == assignment && prior_generation == generation
            }));
    let mut replacement_candidate = false;
    if action == "timeout" {
        if !active_assignment(tx, repo, sprint, task)?
            .is_some_and(|(id, prior)| id == assignment && prior == generation)
        {
            return Err(problem(
                409,
                "REVIEW_ASSIGNMENT_MISMATCH",
                "timeout must reference the active review",
            ));
        }
        let time_and_budget = tx.query_one(
            "SELECT coalesce(extract(epoch FROM clock_timestamp() - max(recorded_at))::double precision, 0),
                    EXISTS (SELECT 1 FROM review_transitions WHERE repo_id=$1 AND sprint_id=$2
                      AND task_id=$3 AND request->>'head_sha'=$4 AND request->>'action'='replace')
             FROM review_transitions WHERE repo_id=$1 AND sprint_id=$2 AND task_id=$3
               AND response->>'outcome'='recorded'
               AND request->>'action' IN ('submit','head_changed','reassign','replace')",
            &[&repo, &sprint, &task, &head],
        )?;
        let activity = ReviewActivity::Unclaimed;
        let elapsed = time_and_budget.get::<_, f64>(0).max(0.0) as u64;
        match state.timeout(elapsed, 1800, 3600, activity, time_and_budget.get(1)) {
            Some(ReviewTimeout::ReplacementCandidate(_)) => replacement_candidate = true,
            Some(ReviewTimeout::Blocked(_)) => {}
            _ => {
                return Err(problem(
                    409,
                    "REVIEW_TIMEOUT_NOT_DUE",
                    "the pending review has not reached the timeout",
                ));
            }
        }
        if !crate::api::pending_deliveries(tx, repo)?.is_empty() {
            return Err(problem(
                409,
                "REVIEW_RECONCILIATION_REQUIRED",
                "resolve outstanding deliveries before fencing the review",
            ));
        }
        let revoked = tx.execute(
            "DELETE FROM assignment_heads WHERE repo_id=$1 AND role='guardrail'
               AND assignment_id=$2 AND generation=$3",
            &[&repo, &assignment, &generation],
        )?;
        if revoked != 1 {
            return Err(problem(
                409,
                "REVIEW_ASSIGNMENT_MISMATCH",
                "active review authority changed",
            ));
        }
    }
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
        apply(&mut state, &request, replacement_candidate)
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
        "replacement_candidate": replacement_candidate,
        "pull_request": pull_request,
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

/// A replacement becomes current and spends its allowance in the same transaction.
/// The caller already holds the development and Guardrail scope locks.
pub(crate) fn reserve_replacement(
    tx: &mut Transaction,
    who: &Caller,
    assignment: &Value,
) -> Result<(), crate::api::Resp> {
    let repo = str_field(assignment, "repo_id", 256)?;
    let sprint = str_field(assignment, "sprint_id", 256)?;
    let task = str_field(assignment, "task_id", 32)?;
    let Some(state) = load(tx, repo, sprint, task)? else {
        return Ok(());
    };
    if state.phase() != ReviewPhase::TimedOut {
        return Ok(());
    }
    let row = tx.query_one(
        "SELECT request->'pull_request', seq FROM review_transitions
         WHERE repo_id=$1 AND sprint_id=$2 AND task_id=$3 AND request->>'action'='timeout'
         ORDER BY seq DESC LIMIT 1",
        &[&repo, &sprint, &task],
    )?;
    let pr: Value = row.get(0);
    let seq: i64 = row.get(1);
    record_inner(
        tx,
        who,
        json!({
            "event_id": format!("replacement-for-timeout-{seq}"),
            "repo_id": repo, "sprint_id": sprint, "task_id": task,
            "head_sha": assignment["target_sha"], "pull_request": pr,
            "assignment_id": assignment["assignment_id"], "generation": assignment["generation"],
            "action": "replace"
        }),
        true,
    )?;
    Ok(())
}

fn active_assignment(
    tx: &mut Transaction,
    repo: &str,
    sprint: &str,
    task: &str,
) -> Result<Option<(String, i64)>, crate::api::Resp> {
    Ok(tx.query_opt(
        "SELECT request->>'assignment_id', (request->>'generation')::bigint FROM review_transitions
         WHERE repo_id=$1 AND sprint_id=$2 AND task_id=$3
           AND request->>'action' IN ('submit','head_changed','reassign','replace')
           AND response->>'outcome'='recorded' ORDER BY seq DESC LIMIT 1",
        &[&repo, &sprint, &task],
    )?.map(|r| (r.get(0), r.get(1))))
}

fn pull_request_recorded(
    tx: &mut Transaction,
    repo: &str,
    sprint: &str,
    task: &str,
) -> Result<bool, crate::api::Resp> {
    Ok(pull_request(tx, repo, sprint, task)?.is_some())
}

pub(crate) fn pull_request(
    tx: &mut Transaction,
    repo: &str,
    sprint: &str,
    task: &str,
) -> Result<Option<i64>, crate::api::Resp> {
    tx.query_opt(
        "SELECT (request->>'pull_request')::bigint FROM review_transitions
         WHERE repo_id=$1 AND sprint_id=$2 AND task_id=$3 ORDER BY seq LIMIT 1",
        &[&repo, &sprint, &task],
    )?
    .map(|row| row.try_get::<_, i64>(0).map_err(Into::into))
    .transpose()
}

pub(crate) fn blocks_judgment(
    tx: &mut Transaction,
    repo: &str,
    sprint: &str,
    task: &str,
    sha: &str,
) -> Result<bool, crate::api::Resp> {
    let Some(state) = load(tx, repo, sprint, task)? else {
        return Ok(false);
    };
    let Some((id, generation)) = active_assignment(tx, repo, sprint, task)? else {
        return Ok(true);
    };
    let current: bool = tx
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM assignment_heads WHERE repo_id=$1
          AND role='guardrail' AND assignment_id=$2 AND generation=$3)",
            &[&repo, &id, &generation],
        )?
        .get(0);
    Ok(!current || state.approved_head() != Some(sha))
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
            let id = str_field(target, "assignment_id", 128)?;
            let active = active_assignment(tx, repo, sprint, task)?.is_some_and(
                |(candidate, generation)| candidate == id && target["generation"] == generation,
            );
            active
                && state.review_delivery_allowed(
                    str_field(target, "target_sha", 64)?,
                    state.review_round(),
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
