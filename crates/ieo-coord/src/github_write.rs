//! Coordinator-owned GitHub writes. Agents receive no GitHub write token.
use crate::api::{
    Abort, Ctx, Req, Resp, TxGuard, ci_ok, is_sha, lock, ok, only_keys, parse_body, problem,
    run_tx, str_field,
};
use crate::auth::{Caller, Kind};
use postgres::{Client, Transaction};
use serde_json::{Value, json};
use std::time::Duration;

pub struct Config {
    endpoint: String,
    repos: Value,
}

fn name(value: &str) -> bool {
    !matches!(value, "" | "." | "..")
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
}

impl Config {
    pub fn github(repos: Value) -> Result<Self, String> {
        if !repos.is_object() {
            return Err("GitHub write configuration must be keyed by repo ID".into());
        }
        Ok(Self {
            endpoint: "https://api.github.com".into(),
            repos,
        })
    }

    /// Test transport targets a disposable loopback HTTP fixture.
    pub fn loopback(port: u16, repos: Value) -> Self {
        Self {
            endpoint: format!("http://127.0.0.1:{port}"),
            repos,
        }
    }
}

#[derive(Clone, Copy)]
struct Candidate<'a> {
    repo: &'a str,
    sprint: &'a str,
    task: &'a str,
    pr: i64,
    sha: &'a str,
    assignment: &'a str,
    generation: i64,
}

fn positive(value: &Value, field: &str, minimum: i64) -> Result<i64, Resp> {
    value[field]
        .as_i64()
        .filter(|n| *n >= minimum)
        .ok_or_else(|| problem(400, "SCHEMA_VIOLATION", &format!("invalid {field}")))
}

fn authorize(
    tx: &mut Transaction,
    candidate: Candidate<'_>,
    merging: bool,
    policy_digest: &str,
    required_checks: &[String],
) -> Result<String, Resp> {
    let Candidate {
        repo,
        sprint,
        task,
        pr,
        sha,
        assignment,
        generation,
    } = candidate;
    lock(tx, &format!("scope:{repo}:development"))?;
    lock(tx, &format!("scope:{repo}:guardrail"))?;
    lock(tx, &format!("delivery-evidence:{repo}"))?;
    let found = tx.query_opt(
        "SELECT a.target_sha, a.request->>'branch_ref', s.review_state, a.policy_digest
         FROM assignment_heads h JOIN assignments a USING (assignment_id,generation)
         JOIN sprint_revisions s ON (s.sprint_id,s.revision)=(a.sprint_id,a.sprint_revision)
         WHERE h.repo_id=$1 AND h.role='development' AND a.sprint_id=$2 AND a.task_id=$3
           AND a.assignment_id=$4 AND a.generation=$5",
        &[&repo, &sprint, &task, &assignment, &generation],
    )?;
    let Some(found) = found else {
        return Err(problem(
            409,
            "GITHUB_ASSIGNMENT_MISMATCH",
            "current development assignment required",
        ));
    };
    let target: String = found.get(0);
    let branch: Option<String> = found.get(1);
    let branch = branch
        .as_deref()
        .and_then(|r| r.strip_prefix("refs/heads/"))
        .filter(|r| *r != "main" && r.split('/').all(name))
        .ok_or_else(|| {
            problem(
                409,
                "GITHUB_BRANCH_MISMATCH",
                "assignment needs an allowed topic branch",
            )
        })?;
    if crate::review_store::pull_request(tx, repo, sprint, task)? != Some(pr) {
        return Err(problem(
            409,
            "REVIEW_PR_MISMATCH",
            "request does not match the recorded pull request",
        ));
    }
    if found.get::<_, String>(2) != "semantic_review_passed"
        || found.get::<_, String>(3) != policy_digest
    {
        return Err(problem(
            409,
            "GITHUB_REVIEW_REQUIRED",
            "the assignment revision has no semantic approval",
        ));
    }
    let state = crate::review_store::load(tx, repo, sprint, task)?
        .ok_or_else(|| problem(409, "GITHUB_REVIEW_REQUIRED", "no review lifecycle"))?;
    if merging {
        let rejected: bool = tx.query_one(
            "SELECT EXISTS (SELECT 1 FROM (
               SELECT DISTINCT ON (body->>'reviewer') body->>'state' AS state FROM evidence
               WHERE repo_id=$1 AND sha=$2 AND kind='review' AND body->>'pull_request'=$3::bigint::text
                 AND body->>'reviewer'<>body->>'author'
                 AND body->>'state' IN ('approved','changes_requested','dismissed')
               ORDER BY body->>'reviewer', seq DESC
             ) latest WHERE state='changes_requested')",
            &[&repo, &sha, &pr],
        )?.get(0);
        let live_claim = tx.query_opt(
            "SELECT e.report->'body'->>'verdict' FROM events e
             JOIN assignment_heads h ON h.repo_id=e.scope_repo AND h.role=e.scope_role
               AND h.assignment_id=e.assignment_id AND h.generation=e.generation
             WHERE e.scope_repo=$1 AND e.scope_role='guardrail' AND e.sprint_id=$2 AND e.task_id=$3
               AND e.attributed_sha=$4 AND e.disposition='current' AND e.report_type='guardrail_review'
             ORDER BY e.sprint_seq DESC LIMIT 1",
            &[&repo, &sprint, &task, &sha],
        )?.is_some_and(|row| row.get::<_, Option<String>>(0).as_deref() == Some("approve"));
        let required_pass: bool = tx.query_one(
            "SELECT NOT EXISTS (SELECT 1 FROM unnest($4::text[]) required(source_ref)
               WHERE (SELECT body->>'conclusion' FROM evidence
                      WHERE repo_id=$1 AND sha=$2 AND kind='ci_run'
                        AND body->>'pull_request'=$3::bigint::text AND body->>'source_ref'=required.source_ref
                      ORDER BY seq DESC LIMIT 1) IS DISTINCT FROM 'success')",
            &[&repo, &sha, &pr, &required_checks],
        )?.get(0);
        if target != sha
            || rejected
            || !live_claim
            || !required_pass
            || crate::review_store::blocks_judgment(tx, repo, sprint, task, sha)?
            || !ci_ok(tx, repo, &["ci_run"], sha, Some(pr), &mut Vec::new())?
        {
            return Err(problem(
                409,
                "GITHUB_MERGE_DENIED",
                "current head requires approval and successful PR checks",
            ));
        }
    } else if !state.developer_push_allowed() {
        return Err(problem(
            409,
            "GITHUB_PUSH_DENIED",
            "review phase does not permit a push",
        ));
    }
    Ok(branch.to_owned())
}

fn execute_remote(
    config: &Config,
    slug: &str,
    token: &str,
    branch: &str,
    pr: i64,
    sha: &str,
    merging: bool,
) -> (u16, Value) {
    let agent = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout(Duration::from_secs(5))
        .build();
    let root = format!("{}/repos/{slug}", config.endpoint);
    let request = |method, path: &str| {
        agent
            .request(method, &format!("{root}{path}"))
            .set("Authorization", &format!("Bearer {token}"))
            .set("Accept", "application/vnd.github+json")
            .set("X-GitHub-Api-Version", "2022-11-28")
            .set("User-Agent", "ieo-coordinator")
    };
    let Ok(current) = request("GET", &format!("/pulls/{pr}")).call() else {
        return (
            409,
            json!({"outcome":"refused", "reason":"pull_request_unavailable"}),
        );
    };
    let Ok(current) = current.into_json::<Value>() else {
        return (
            409,
            json!({"outcome":"refused", "reason":"pull_request_unreadable"}),
        );
    };
    if current["state"] != "open"
        || current["head"]["repo"]["full_name"] != slug
        || current["base"]["repo"]["full_name"] != slug
        || current["head"]["ref"] != branch
        || current["base"]["ref"] != "main"
    {
        return (
            409,
            json!({"outcome":"refused", "reason":"pull_request_binding_mismatch"}),
        );
    }
    let sent = if merging {
        request("PUT", &format!("/pulls/{pr}/merge")).send_json(json!({"sha":sha}))
    } else {
        request("PATCH", &format!("/git/refs/heads/{branch}"))
            .send_json(json!({"sha":sha,"force":false}))
    };
    match sent {
        Ok(reply) => match reply.into_json::<Value>() {
            Ok(reply)
                if merging
                    && reply["merged"] == true
                    && reply["sha"].as_str().is_some_and(is_sha) =>
            {
                (
                    200,
                    json!({"outcome":"merged", "head_sha":sha, "merged_sha":reply["sha"]}),
                )
            }
            Ok(reply) if !merging && reply["object"]["sha"] == sha => {
                (200, json!({"outcome":"pushed", "head_sha":sha}))
            }
            Ok(reply) if merging && reply["merged"] == false => (
                409,
                json!({"outcome":"refused", "reason":"github_merge_refused"}),
            ),
            _ => (502, json!({"outcome":"unknown"})),
        },
        Err(ureq::Error::Status(status, _)) if (400..500).contains(&status) => {
            (409, json!({"outcome":"refused", "github_status":status}))
        }
        Err(_) => (502, json!({"outcome":"unknown"})),
    }
}

pub(crate) fn execute(
    db: &mut Client,
    ctx: &Ctx,
    who: &Caller,
    req: &Req,
    guard: &dyn TxGuard,
    merging: bool,
) -> Result<Resp, Abort> {
    let prepare = || -> Result<_, Resp> {
        let authorized = if merging {
            who.kind == Kind::Service && who.has("github:merge")
        } else {
            who.kind == Kind::Agent
                && who.role.as_deref() == Some("development")
                && who.has("github:push")
        };
        if !authorized {
            return Err(problem(403, "SCOPE_DENIED", "GitHub write scope required"));
        }
        let request = parse_body(req.body)?;
        only_keys(
            &request,
            &[
                "repo_id",
                "sprint_id",
                "task_id",
                "pull_request",
                "head_sha",
                "assignment_id",
                "generation",
            ],
        )?;
        Ok(request)
    };
    let request = match prepare() {
        Ok(v) => v,
        Err(r) => return Ok(r),
    };
    let prepare = || -> Result<_, Resp> {
        let repo = str_field(&request, "repo_id", 256)?;
        if !who.in_repo(repo) {
            return Err(problem(
                403,
                "REPO_SCOPE_DENIED",
                "repo not in credential scope",
            ));
        }
        let sha = str_field(&request, "head_sha", 64)?;
        if !is_sha(sha) {
            return Err(problem(400, "SCHEMA_VIOLATION", "invalid head SHA"));
        }
        let candidate = Candidate {
            repo,
            sha,
            sprint: str_field(&request, "sprint_id", 256)?,
            task: str_field(&request, "task_id", 32)?,
            pr: positive(&request, "pull_request", 1)?,
            assignment: str_field(&request, "assignment_id", 128)?,
            generation: positive(&request, "generation", 0)?,
        };
        let key = req
            .idempotency_key
            .filter(|k| !k.is_empty() && k.len() <= 128)
            .ok_or_else(|| {
                problem(
                    400,
                    "IDEMPOTENCY_KEY_REQUIRED",
                    "a bounded Idempotency-Key is required",
                )
            })?;
        let config = ctx
            .github_writes
            .as_ref()
            .ok_or_else(|| problem(503, "GITHUB_WRITE_DISABLED", "no configured GitHub writer"))?;
        let settings = &config.repos[repo];
        let slug = str_field(settings, "repository", 256)?;
        if !slug
            .split_once('/')
            .is_some_and(|(owner, repo)| name(owner) && name(repo))
        {
            return Err(problem(
                503,
                "GITHUB_WRITE_DISABLED",
                "repository mapping is invalid",
            ));
        }
        let token_file = if merging {
            "merge_token_file"
        } else {
            "push_token_file"
        };
        let token = std::fs::read_to_string(str_field(settings, token_file, 4096)?)
            .ok()
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty() && t.bytes().all(|c| c.is_ascii_graphic()))
            .ok_or_else(|| {
                problem(
                    503,
                    "GITHUB_WRITE_DISABLED",
                    "configured GitHub credential is unavailable",
                )
            })?;
        let required_checks = if merging {
            settings["required_checks"]
                .as_array()
                .filter(|items| !items.is_empty())
                .and_then(|items| {
                    items
                        .iter()
                        .map(|item| {
                            item.as_str()
                                .filter(|name| !name.is_empty() && name.len() <= 512)
                                .map(str::to_owned)
                        })
                        .collect::<Option<Vec<_>>>()
                })
                .ok_or_else(|| {
                    problem(
                        503,
                        "GITHUB_WRITE_DISABLED",
                        "merge requires configured CI evidence sources",
                    )
                })?
        } else {
            Vec::new()
        };
        Ok((candidate, key, config, slug, token, required_checks))
    };
    let (candidate, key, config, slug, token, required_checks) = match prepare() {
        Ok(v) => v,
        Err(r) => return Ok(r),
    };
    let repo = candidate.repo;
    let persisted = json!({"merge":merging,"candidate":request});
    let intent = run_tx(db, guard, &|tx| {
        lock(tx, &format!("github-write:{repo}"))?;
        if let Some(existing) = tx.query_opt(
            "SELECT principal, request, result FROM github_write_intents WHERE repo_id=$1 AND request_id=$2",
            &[&repo, &key],
        )? {
            if existing.get::<_, &str>(0) != who.name || existing.get::<_, Value>(1) != persisted {
                return Err(problem(409, "EVENT_CONTENT_CONFLICT", "GitHub write key already used"));
            }
            return ok(200, existing.get::<_, Option<Value>>(2).unwrap_or(json!({"outcome":"unknown"})));
        }
        let pending: bool = tx.query_one(
            "SELECT EXISTS(SELECT 1 FROM github_write_intents WHERE repo_id=$1 AND pull_request=$2
             AND (result IS NULL OR result->>'outcome'='unknown'))",
            &[&repo, &candidate.pr],
        )?.get(0);
        if pending {
            return Err(problem(
                409,
                "GITHUB_WRITE_UNRESOLVED",
                "reconcile the prior GitHub write",
            ));
        }
        authorize(tx, candidate, merging, &ctx.policy.digest, &required_checks)?;
        tx.execute(
            "INSERT INTO github_write_intents (repo_id,request_id,pull_request,principal,request)
             VALUES ($1,$2,$3,$4,$5)",
            &[&repo, &key, &candidate.pr, &who.name, &persisted],
        )?;
        ok(201, json!({"outcome":"reserved"}))
    })?;
    if intent.status != 201 {
        return Ok(intent);
    }
    run_tx(db, guard, &|tx| {
        lock(tx, &format!("github-write:{repo}"))?;
        let (status, result) =
            match authorize(tx, candidate, merging, &ctx.policy.digest, &required_checks) {
                Ok(branch) => execute_remote(
                    config,
                    slug,
                    &token,
                    &branch,
                    candidate.pr,
                    candidate.sha,
                    merging,
                ),
                Err(_) => (
                    409,
                    json!({"outcome":"refused","reason":"authorization_changed"}),
                ),
            };
        tx.execute(
            "UPDATE github_write_intents SET result=$3 WHERE repo_id=$1 AND request_id=$2 AND result IS NULL",
            &[&repo, &key, &result],
        )?;
        ok(status, result)
    })
}
