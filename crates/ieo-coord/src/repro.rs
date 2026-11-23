//! Durable coordinator queue for one-shot supervisor reproductions.
use ieo_supervisor::repro::{Job, Runner};
use postgres::Client;
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

pub fn configure(config: &Value) -> Result<BTreeMap<String, Runner>, String> {
    let repos = config
        .as_object()
        .ok_or("repro configuration must be an object")?;
    repos
        .iter()
        .map(|(repo, entry)| {
            let field = |name: &str| {
                entry[name]
                    .as_str()
                    .ok_or_else(|| format!("{repo}: missing {name}"))
            };
            let runner = Runner::new(
                Path::new(field("repository")?),
                Path::new(field("bundles")?),
                Path::new(field("state")?),
                field("image")?.to_owned(),
            )?;
            Ok((repo.clone(), runner))
        })
        .collect()
}

fn current(
    tx: &mut postgres::Transaction<'_>,
    repo: &str,
    body: &Value,
) -> Result<bool, postgres::Error> {
    Ok(tx.query_one(
        "SELECT EXISTS (SELECT 1 FROM assignment_heads h JOIN assignments a USING (assignment_id,generation)
         WHERE h.repo_id=$1 AND h.role='guardrail' AND a.repo_id=$1 AND a.role='guardrail'
           AND a.assignment_id=$2::jsonb->>'assignment_id' AND a.generation::text=$2::jsonb->>'generation'
           AND a.target_sha=$2::jsonb->>'target_sha')",
        &[&repo, &body],
    )?.get(0))
}

fn reserve(
    db: &mut Client,
    repo: &str,
    image: &str,
) -> Result<Option<(String, String, Value)>, postgres::Error> {
    let mut tx = db.transaction()?;
    tx.query_one(
        "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
        &[&format!("scope:{repo}:guardrail")],
    )?;
    let next = tx.query_opt(
        "SELECT request_id, body FROM repro_requests WHERE repo_id=$1 AND state='queued'
         ORDER BY created_at, request_id LIMIT 1 FOR UPDATE",
        &[&repo],
    )?;
    let Some(next) = next else {
        tx.commit()?;
        return Ok(None);
    };
    let id: String = next.get(0);
    let body: Value = next.get(1);
    let active = current(&mut tx, repo, &body)?;
    let row = tx.query_one(
        "UPDATE repro_requests SET
           state=CASE WHEN $4 THEN 'running' ELSE 'rejected_stale_generation' END,
           run_key=CASE WHEN $4 THEN 'ieo-repro-' || gen_random_uuid()::text END,
           runner_image=CASE WHEN $4 THEN $3 END
         WHERE repo_id=$1 AND request_id=$2 RETURNING run_key",
        &[&repo, &id, &image, &active],
    )?;
    let key: Option<String> = row.get(0);
    tx.commit()?;
    Ok(key.map(|key| (id, key, body)))
}

fn finish(
    db: &mut Client,
    repo: &str,
    id: &str,
    key: &str,
    result: &Value,
) -> Result<bool, postgres::Error> {
    let state = result["state"].as_str().unwrap_or("runner_failed");
    Ok(db.execute(
        "UPDATE repro_requests SET state=$4, result=$5
         WHERE repo_id=$1 AND request_id=$2 AND run_key=$3 AND state='running'",
        &[&repo, &id, &key, &state, result],
    )? == 1)
}

/// Each tick may start one queued request per configured repo. Running rows are
/// read-only reconciled by the original container ID and never dispatched again.
pub fn tick(db: &mut Client, repo: &str, runner: &Runner) -> Result<(), postgres::Error> {
    let pending = db.query(
        "SELECT request_id, run_key FROM repro_requests
         WHERE repo_id=$1 AND state='running' AND runner_image=$2 AND run_key IS NOT NULL",
        &[&repo, &runner.image],
    )?;
    for row in pending {
        let id: &str = row.get(0);
        let key: &str = row.get(1);
        if let Some(result) = runner.observe(key)
            && finish(db, repo, id, key, &result)?
        {
            runner.cleanup(key);
        }
    }
    let Some((id, key, body)) = reserve(db, repo, &runner.image)? else {
        return Ok(());
    };
    let job = Job {
        key: &key,
        sha: body["target_sha"].as_str().unwrap_or_default(),
        bundle: body["bundle_digest"].as_str().unwrap_or_default(),
        command: body["command"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default(),
        timeout: body["timeout_seconds"].as_u64().unwrap_or(60).min(300),
    };
    if let Err(reason) = runner.prepare(&job) {
        let mut tx = db.transaction()?;
        let result = json!({"state":"runner_failed","runner_image_digest":runner.image,
            "finished_at": time_now(&mut tx)?, "reason":reason});
        tx.execute(
            "UPDATE repro_requests SET state='runner_failed', result=$4
             WHERE repo_id=$1 AND request_id=$2 AND run_key=$3 AND state='running'",
            &[&repo, &id, &key, &result],
        )?;
        tx.commit()?;
        runner.cleanup(&key);
        return Ok(());
    }
    // The durable reservation survived preparation. Hold the existing role
    // fence only around the final authorization and the one external start.
    let mut tx = db.transaction()?;
    tx.query_one(
        "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
        &[&format!("scope:{repo}:guardrail")],
    )?;
    if current(&mut tx, repo, &body)? {
        runner.start(&key);
        tx.commit()
    } else {
        tx.execute(
            "UPDATE repro_requests SET state='rejected_stale_generation'
             WHERE repo_id=$1 AND request_id=$2 AND run_key=$3 AND state='running'",
            &[&repo, &id, &key],
        )?;
        tx.commit()?;
        runner.cleanup(&key);
        Ok(())
    }
}

fn time_now(tx: &mut postgres::Transaction<'_>) -> Result<String, postgres::Error> {
    Ok(tx.query_one("SELECT to_char(clock_timestamp() AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')", &[])?.get(0))
}
