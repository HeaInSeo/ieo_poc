//! Assignment-level time and token limits (02-05 step 5, PLAN §3.2 step 5; carry-over C7 / 02-04
//! acceptance item 1a). Definitions: user decision 2026-11-16 (L01 token definition (가), time
//! definition (i)), `docs/poc/User_Decisions_2026-11-16_l01-step5.md`.
//!
//! * **Limits.** An assignment generation's `limits {elapsed_seconds, total_tokens}`: the
//!   assignment request's values, else the sprint revision's (never above them; 422
//!   `LIMITS_ABOVE_SPEC`). The current head's limits apply to the whole assignment.
//! * **Consumption.** Every delivered run is measured and charged after it ends by the shared
//!   budget library (02-01 meter: a resumed run is the cumulative difference, a child counts by
//!   its transcript; runtime contract §11·§12). Its ledger record is copied here
//!   (`assignment_run_usage`) and summed over **all generations** of the assignment:
//!   `total_tokens` = input + output + cache_creation (cache_read is recorded, not counted),
//!   `elapsed_seconds` = the sum of the runs' durations (each rounded up to a second; not wall
//!   clock). All four token counts are kept per run.
//! * **Unmeasured or missing.** A run whose ledger record is unmeasured (a guard stop, a lost
//!   answer, a failed measurement), or whose record is missing although its call was sent (or
//!   was settled before), is never counted as 0 and never lowered (review 97ff0bc M-1): the assignment's next delivery is refused (409
//!   `ASSIGNMENT_USAGE_UNMEASURED`) until a person corrects the ledger.
//! * **Enforcement.** A delivery is refused (409 `ASSIGNMENT_LIMIT_REACHED`, nothing recorded
//!   apart from one limit event) when no token room is left or the time left is not more than
//!   the run guard's stop grace. Otherwise the run's limits are the smallest of the assignment's
//!   remaining amounts, the budget's and the role share's ([`ieo_supervisor::caller`]), computed
//!   here, outside the role containers. No decision request is created (that would widen policy
//!   G5: a new policy version); the operational state is "awaiting a person's decision".

use crate::api::{Resp, i64_of, one};
use postgres::Transaction;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// The ledger a budget plan reads: `<budget>@<ledger_dir>` (rows are bound to it).
pub(crate) fn ledger_id(plan_json: &Value) -> Option<String> {
    Some(format!(
        "{}@{}",
        plan_json["budget"].as_str()?,
        plan_json["ledger_dir"].as_str()?
    ))
}

/// Copy what the ledger says about the assignment's delivered attempts (all generations) into
/// `assignment_run_usage`, adding a row whenever a delivery's settled state changes (the newest
/// row by `seq` counts). `recs` are the ledger's effective records read while no call was running
/// (`caller::records_when_idle` or the budget check's own read under the ledger lock); `ledger` is
/// that ledger's identity.
///
/// * Only a delivery that sent a call (a result with a sent digest) is matched, and only with a
///   record of the same attempt and role: a refused admission never takes another caller's record
///   (review 97ff0bc I-2).
/// * A record with `model_request: false` naming the attempt is an explicit entry by a person:
///   the run counts nothing (`withdrawn`).
/// * No record while a call was (or may have been) started (newest result `accepted`/`unknown`),
///   or a record settled before that is gone now: `missing` — it stops the assignment and keeps
///   the last known counts. A missing record never lowers consumption (review 97ff0bc M-1).
/// * An empty ledger (no records at all) when rows were settled before says nothing: no row is
///   added (a truncated or moved file, a ledger not mounted).
/// * Rows of another ledger are never continued from this one: nothing is settled and
///   [`state`] reports the mismatch.
pub(crate) fn settle(
    tx: &mut Transaction,
    repo: &str,
    role: &str,
    aid: &str,
    recs: &[Value],
    ledger: &str,
) -> Result<(), Resp> {
    let other: bool = tx
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM assignment_run_usage
                            WHERE assignment_id = $1 AND ledger <> $2)",
            &[&aid, &ledger],
        )?
        .get(0);
    let any: bool = tx
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM assignment_run_usage WHERE assignment_id = $1)",
            &[&aid],
        )?
        .get(0);
    if other || (any && recs.is_empty()) {
        return Ok(());
    }
    let rows = tx.query(
        "SELECT d.delivery_id, d.generation, d.attempt_id,
                EXISTS (SELECT 1 FROM role_delivery_results r WHERE r.delivery_id = d.delivery_id
                          AND r.sent_sha256 IS NOT NULL),
                (SELECT outcome FROM role_delivery_results r WHERE r.delivery_id = d.delivery_id
                 ORDER BY n DESC LIMIT 1),
                u.record_sha256, u.seconds, u.input_tokens, u.output_tokens,
                u.cache_creation_tokens, u.cache_read_tokens, u.state
         FROM role_deliveries d
         LEFT JOIN LATERAL (SELECT * FROM assignment_run_usage x WHERE x.delivery_id = d.delivery_id
                            ORDER BY x.seq DESC LIMIT 1) u ON true
         WHERE d.assignment_id = $1 AND d.repo_id = $2 AND d.role = $3 ORDER BY d.seq",
        &[&aid, &repo, &role],
    )?;
    for r in rows {
        let (id, generation, attempt): (String, i64, String) = (r.get(0), r.get(1), r.get(2));
        let sent: bool = r.get(3);
        let outcome: Option<String> = r.get(4);
        let stored: Option<String> = r.get(5);
        if !sent {
            continue;
        }
        let rec = recs
            .iter()
            .find(|x| x["attempt_id"] == attempt.as_str() && x["role"] == role);
        let started = matches!(outcome.as_deref(), Some("accepted" | "unknown"));
        let row = match rec {
            Some(rec) => usage_of(rec),
            None if stored.is_some() || started => {
                // keep the last known counts: a missing record never lowers consumption
                let n = |i: usize| r.get::<_, Option<i64>>(i).map(|x| x.max(0) as u64);
                let prev = (n(7), n(8), n(9), n(10));
                let tokens = match prev {
                    (Some(a), Some(b), Some(c), Some(d)) => Some([a, b, c, d]),
                    _ => None,
                };
                Usage {
                    sha: "missing".into(),
                    state: "missing",
                    seconds: n(6),
                    tokens,
                    reason: Some(
                        "the budget ledger has no record of this attempt: a person reconciles                          the ledger (the record back, or an entry naming the attempt)"
                            .into(),
                    ),
                }
            }
            None => continue,
        };
        if stored.as_deref() == Some(row.sha.as_str()) {
            continue;
        }
        let t = |i: usize| row.tokens.map(|t| i64_of(t[i]));
        tx.execute(
            "INSERT INTO assignment_run_usage (delivery_id, record_sha256, ledger, assignment_id,
                 generation, attempt_id, state, seconds, input_tokens, output_tokens,
                 cache_creation_tokens, cache_read_tokens, reason)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
            &[
                &id,
                &row.sha,
                &ledger,
                &aid,
                &generation,
                &attempt,
                &row.state,
                &row.seconds.map(i64_of),
                &t(0),
                &t(1),
                &t(2),
                &t(3),
                &row.reason,
            ],
        )?;
    }
    Ok(())
}

struct Usage {
    sha: String,
    /// measured, unmeasured, missing or withdrawn
    state: &'static str,
    seconds: Option<u64>,
    /// input, output, cache_creation, cache_read
    tokens: Option<[u64; 4]>,
    reason: Option<String>,
}

/// What one ledger record says the run used (the charge `budget::spent` reads).
fn usage_of(rec: &Value) -> Usage {
    let sha = hex::encode(Sha256::digest(serde_json::to_vec(rec).unwrap_or_default()));
    if rec["model_request"] == false {
        return Usage {
            sha,
            state: "withdrawn",
            seconds: None,
            tokens: None,
            reason: Some(
                rec["reason"]
                    .as_str()
                    .unwrap_or("a ledger entry says no model request ran")
                    .to_string(),
            ),
        };
    }
    let seconds = rec["duration_ms"].as_u64().map(|ms| ms.div_ceil(1000));
    let run = &rec["usage"]["run"];
    let t = &run["tokens"];
    let n = |k: &str| t[k].as_u64();
    let tokens = (run["state"] == "measured")
        .then(|| {
            Some([
                n("input")?,
                n("output")?,
                n("cache_creation")?,
                n("cache_read")?,
            ])
        })
        .flatten();
    let measured = tokens.is_some() && seconds.is_some();
    let reason = (!measured).then(|| {
        run["reason"]
            .as_str()
            .or(rec["stop_reason"].as_str())
            .unwrap_or("the run is not measured")
            .to_string()
    });
    Usage {
        sha,
        state: if measured { "measured" } else { "unmeasured" },
        seconds,
        tokens: tokens.filter(|_| measured),
        reason,
    }
}

/// The assignment's limits, consumption and what is left (all generations; the newest usage row
/// of each delivery). `limits` are the current head generation's when the assignment is the
/// head, else the named generation's.
pub(crate) fn state(
    tx: &mut Transaction,
    aid: &str,
    generation: i64,
    ledger: Option<&str>,
) -> Result<Value, Resp> {
    let lim = tx.query_one(
        "SELECT coalesce(a.limits, jsonb_build_object(
                    'elapsed_seconds', s.spec->'limits'->'elapsed_seconds',
                    'total_tokens', s.spec->'limits'->'total_tokens'))
         FROM assignments a
         JOIN sprint_revisions s ON (s.sprint_id, s.revision) = (a.sprint_id, a.sprint_revision)
         WHERE a.assignment_id = $1 AND a.generation = $2",
        &[&aid, &generation],
    )?;
    let limits: Value = lim.get(0);
    let u = tx.query_one(
        "WITH newest AS (
             SELECT DISTINCT ON (delivery_id) * FROM assignment_run_usage
             WHERE assignment_id = $1 ORDER BY delivery_id, seq DESC)
         SELECT count(*) FILTER (WHERE state <> 'withdrawn'),
                count(*) FILTER (WHERE state IN ('unmeasured', 'missing')),
                coalesce(sum(seconds), 0)::bigint,
                coalesce(sum(input_tokens), 0)::bigint, coalesce(sum(output_tokens), 0)::bigint,
                coalesce(sum(cache_creation_tokens), 0)::bigint,
                coalesce(sum(cache_read_tokens), 0)::bigint
         FROM newest",
        &[&aid],
    )?;
    let g = |i: usize| u.get::<_, i64>(i).max(0) as u64;
    let (runs, unmeasured, seconds) = (g(0), g(1), g(2));
    let (input, output, cc, cr) = (g(3), g(4), g(5), g(6));
    let total = input.saturating_add(output).saturating_add(cc);
    let left = |k: &str, used: u64| {
        limits[k]
            .as_u64()
            .map_or(Value::Null, |l| json!(l.saturating_sub(used)))
    };
    // rows settled from another ledger than the one this delivery would use
    let ledgers: Vec<String> = tx
        .query(
            "SELECT DISTINCT ledger FROM assignment_run_usage WHERE assignment_id = $1 ORDER BY 1",
            &[&aid],
        )?
        .iter()
        .map(|r| r.get(0))
        .collect();
    let mismatch = ledger.is_some_and(|l| ledgers.iter().any(|x| x != l));
    Ok(json!({"evidence_level": "verified", "limits": limits,
        "ledgers": ledgers, "ledger_mismatch": mismatch,
        "runs": runs, "unmeasured_runs": unmeasured,
        "elapsed_seconds": seconds,
        "total_tokens": total,
        "tokens": {"input": input, "output": output, "cache_creation": cc, "cache_read": cr},
        "remaining": {"elapsed_seconds": left("elapsed_seconds", seconds),
                      "total_tokens": left("total_tokens", total)},
        "definition": "total_tokens = input + output + cache_creation; elapsed_seconds = the sum of run durations; all generations (user decision 2026-11-16)"}))
}

/// What the next run of the assignment may use: `Ok((window_seconds, billable))` (`None` = the
/// assignment sets no limit), or the 409 refusal (with the state attached as `usage`).
pub(crate) fn room(st: &Value) -> Result<(Option<u64>, Option<u64>), Resp> {
    let refuse = |code: &str, why: String| {
        let mut r = one(
            409,
            code,
            "assignment_id",
            &why,
            "a person decides: ask the operator",
        );
        r.body["usage"] = st.clone();
        r.body["operational_state"] = json!("awaiting_human_decision");
        r
    };
    if st["ledger_mismatch"] == true {
        return Err(refuse(
            "ASSIGNMENT_USAGE_UNMEASURED",
            format!(
                "this assignment's runs were settled from the ledger {}, not the configured one: \
                 its usage cannot be read here",
                st["ledgers"]
            ),
        ));
    }
    if st["unmeasured_runs"].as_u64().unwrap_or(0) > 0 {
        return Err(refuse(
            "ASSIGNMENT_USAGE_UNMEASURED",
            format!(
                "{} run(s) of this assignment are not measured or their ledger record is missing: never counted as 0",
                st["unmeasured_runs"]
            ),
        ));
    }
    let tokens = st["remaining"]["total_tokens"].as_u64();
    if tokens == Some(0) {
        return Err(refuse(
            "ASSIGNMENT_LIMIT_REACHED",
            format!(
                "total_tokens: {} of {} used",
                st["total_tokens"], st["limits"]["total_tokens"]
            ),
        ));
    }
    let secs = st["remaining"]["elapsed_seconds"].as_u64();
    let grace = ieo_supervisor::run::GRACE.as_secs();
    if secs.is_some_and(|s| s <= grace) {
        return Err(refuse(
            "ASSIGNMENT_LIMIT_REACHED",
            format!(
                "elapsed_seconds: {} of {} used; {} s left is not more than the {grace} s stop grace",
                st["elapsed_seconds"],
                st["limits"]["elapsed_seconds"],
                secs.unwrap_or(0)
            ),
        ));
    }
    Ok((secs, tokens))
}

/// One limit event per (generation, code, settled runs): the refusal stays visible.
pub(crate) fn record_event(
    tx: &mut Transaction,
    aid: &str,
    generation: i64,
    refusal: &Resp,
    who: &str,
) -> Result<(), Resp> {
    let b = &refusal.body;
    let code = b["rejections"][0]["code"].as_str().unwrap_or_default();
    tx.execute(
        "INSERT INTO assignment_limit_events (assignment_id, generation, code, settled_runs, usage,
             reason, requested_by)
         VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT DO NOTHING",
        &[
            &aid,
            &generation,
            &code,
            &i64_of(b["usage"]["runs"].as_u64().unwrap_or(0)),
            &b["usage"],
            &b["rejections"][0]["message"].as_str().unwrap_or_default(),
            &who,
        ],
    )?;
    Ok(())
}
