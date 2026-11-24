//! Coordinator-owned GitHub writes. Agents receive no GitHub write token.
use crate::api::{
    Abort, Ctx, Req, Resp, TxGuard, ci_ok, is_sha, lock, ok, only_keys, parse_body, problem,
    run_tx, str_field,
};
use crate::auth::{Caller, Kind};
use postgres::{Client, Transaction};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

#[derive(Clone)]
pub struct Config {
    endpoint: String,
    repos: Value,
    fixture_remote: Option<PathBuf>,
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
            fixture_remote: None,
        })
    }

    /// Test transport targets a disposable loopback HTTP fixture.
    pub fn loopback(port: u16, repos: Value, remote: PathBuf) -> Self {
        Self {
            endpoint: format!("http://127.0.0.1:{port}"),
            repos,
            fixture_remote: Some(remote),
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
        // N5: a PR-level request for changes is an independent merge gate.
        // It can survive a new head and does not contribute to G3 approval.
        let pr_changes_requested = tx
            .query_opt(
                "SELECT body->>'decision', sha FROM evidence
             WHERE repo_id=$1 AND kind='pr_review_state' AND body->>'pull_request'=$2::bigint::text
             ORDER BY seq DESC LIMIT 1",
                &[&repo, &pr],
            )?
            .is_some_and(|row| {
                row.get::<_, &str>(0) == "CHANGES_REQUESTED" || row.get::<_, &str>(1) != sha
            });
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
            || pr_changes_requested
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

fn git(directory: &Path) -> Command {
    let mut command = Command::new("/usr/bin/timeout");
    command
        .args(["--kill-after=2s", "12s", "/usr/bin/git"])
        .current_dir(directory)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ALLOW_PROTOCOL", "file:https")
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "http.followRedirects=false",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

fn push_commit(
    config: &Config,
    slug: &str,
    token: &str,
    branch: &str,
    sha: &str,
    bundle: &Path,
) -> (u16, Value) {
    let Ok(stage) = tempfile::tempdir() else {
        return (
            409,
            json!({"outcome":"refused","reason":"staging_unavailable"}),
        );
    };
    let dir = stage.path();
    if !git(dir)
        .args(["init", "--bare", "."])
        .status()
        .is_ok_and(|s| s.success())
        || !bundle.is_absolute()
        || !git(dir)
            .args(["bundle", "unbundle", "--"])
            .arg(bundle)
            .status()
            .is_ok_and(|s| s.success())
        || !git(dir)
            .args(["cat-file", "-e", &format!("{sha}^{{commit}}")])
            .status()
            .is_ok_and(|s| s.success())
    {
        return (
            409,
            json!({"outcome":"refused","reason":"candidate_bundle_unavailable"}),
        );
    }
    let remote = format!("https://github.com/{slug}.git");
    let mut command = git(dir);
    // Only the isolated Git push process receives the coordinator credential.
    command
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "http.https://github.com/.extraheader")
        .env(
            "GIT_CONFIG_VALUE_0",
            format!("Authorization: Bearer {token}"),
        );
    let target = config
        .fixture_remote
        .as_deref()
        .unwrap_or_else(|| Path::new(&remote));
    let result = command
        .arg("push")
        .arg("--")
        .arg(target)
        .arg(format!("{sha}:refs/heads/{branch}"))
        .status();
    if result.is_ok_and(|status| status.success()) {
        (200, json!({"outcome":"pushed","head_sha":sha}))
    } else {
        // A failed Git client can have lost its response after updating the ref.
        (502, json!({"outcome":"unknown"}))
    }
}

fn github_api<'a>(
    config: &Config,
    slug: &str,
    token: &'a str,
) -> impl Fn(&str, &str) -> ureq::Request + 'a {
    let agent = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout(Duration::from_secs(5))
        .build();
    let root = format!("{}/repos/{slug}", config.endpoint);
    move |method, path: &str| {
        agent
            .request(method, &format!("{root}{path}"))
            .set("Authorization", &format!("Bearer {token}"))
            .set("Accept", "application/vnd.github+json")
            .set("X-GitHub-Api-Version", "2022-11-28")
            .set("User-Agent", "ieo-coordinator")
    }
}
fn execute_remote(
    config: &Config,
    slug: &str,
    token: &str,
    branch: &str,
    candidate: Candidate<'_>,
    merging: bool,
    bundle: Option<&Path>,
) -> (u16, Value) {
    let Candidate { pr, sha, .. } = candidate;
    let request = github_api(config, slug, token);
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
    if !merging {
        return bundle.map_or(
            (
                409,
                json!({"outcome":"refused","reason":"candidate_bundle_unavailable"}),
            ),
            |bundle| push_commit(config, slug, token, branch, sha, bundle),
        );
    }
    let sent = request("PUT", &format!("/pulls/{pr}/merge"))
        .send_json(json!({"sha":sha,"merge_method":"merge"}));
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
    let intent = run_tx(db, guard, &|tx| {
        lock(tx, &format!("github-write:{repo}"))?;
        if let Some(existing) = tx.query_opt(
            "SELECT principal, request, result FROM github_write_intents WHERE repo_id=$1 AND request_id=$2",
            &[&repo, &key],
        )? {
            if existing.get::<_, &str>(0) != who.name || (existing.get::<_, Value>(1)["candidate"] != request || existing.get::<_, Value>(1)["merge"] != merging) {
                return Err(problem(409, "EVENT_CONTENT_CONFLICT", "GitHub write key already used"));
            }
            return ok(200, existing.get::<_, Option<Value>>(2).unwrap_or(json!({"outcome":"unknown"})));
        }
        if let Some(already) = tx.query_opt(
            "SELECT principal, request, result FROM github_write_intents WHERE repo_id=$1 AND pull_request=$2
             AND request->'candidate'->>'head_sha'=$3 AND request->'merge'=to_jsonb($4::boolean)
             AND result->>'outcome' IN ('pushed','merged') ORDER BY request_id LIMIT 1",
            &[&repo, &candidate.pr, &candidate.sha, &merging],
        )? {
            if already.get::<_, &str>(0) != who.name
                || already.get::<_, Value>(1)["candidate"] != request
            {
                return Err(problem(409, "GITHUB_WRITE_RECEIPT_MISMATCH",
                    "confirmed GitHub write belongs to a different principal or assignment"));
            }
            return ok(200, already.get(2));
        }
        let pending = has_unresolved(tx, repo, candidate.pr)?;
        if pending {
            return Err(problem(
                409,
                "GITHUB_WRITE_UNRESOLVED",
                "reconcile the prior GitHub write",
            ));
        }
        let branch = authorize(tx, candidate, merging, &ctx.policy.digest, &required_checks)?;
        let persisted =
            json!({"merge":merging,"candidate":request,"branch":branch,"repository":slug});
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
        let row = tx.query_one(
            "SELECT request, result FROM github_write_intents WHERE repo_id=$1 AND request_id=$2",
            &[&repo, &key],
        )?;
        if let Some(result) = row.get::<_, Option<Value>>(1) {
            return ok(200, result);
        }
        let reserved: Value = row.get(0);
        let (status, result) =
            match authorize(tx, candidate, merging, &ctx.policy.digest, &required_checks) {
                Ok(branch) if reserved["branch"] == branch && reserved["repository"] == slug => {
                    execute_remote(
                        config,
                        slug,
                        &token,
                        &branch,
                        candidate,
                        merging,
                        config.repos[repo]["push_bundle_file"]
                            .as_str()
                            .map(Path::new),
                    )
                }
                _ => (
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

pub(crate) fn has_unresolved(tx: &mut Transaction, repo: &str, pr: i64) -> Result<bool, Resp> {
    Ok(tx
        .query_one(
            "SELECT EXISTS(SELECT 1 FROM github_write_intents WHERE repo_id=$1 AND pull_request=$2
         AND (result IS NULL OR result->>'outcome'='unknown'))",
            &[&repo, &pr],
        )?
        .get(0))
}

/// No mutation is sent to GitHub. A definite result is attached once to the
/// original intent. Negative or incomplete observations never re-enable writes.
pub(crate) fn reconcile(tx: &mut Transaction, ctx: &Ctx, who: &Caller, req: &Req) -> crate::api::R {
    if who.kind != Kind::Service || !who.has("github:merge") {
        return Err(problem(
            403,
            "SCOPE_DENIED",
            "GitHub reconciliation requires a service principal",
        ));
    }
    let body = parse_body(req.body)?;
    only_keys(&body, &["repo_id", "request_id"])?;
    let repo = str_field(&body, "repo_id", 256)?;
    let key = str_field(&body, "request_id", 128)?;
    if !who.in_repo(repo) {
        return Err(problem(
            403,
            "REPO_SCOPE_DENIED",
            "repo not in credential scope",
        ));
    }
    lock(tx, &format!("github-write:{repo}"))?;
    let row = tx.query_opt(
        "SELECT pull_request, request, result FROM github_write_intents WHERE repo_id=$1 AND request_id=$2",
        &[&repo, &key],
    )?.ok_or_else(|| problem(404, "GITHUB_WRITE_UNKNOWN", "no such GitHub write intent"))?;
    if let Some(result) = row.get::<_, Option<Value>>(2)
        && result["outcome"] != "unknown"
    {
        return ok(200, result);
    }
    let pr: i64 = row.get(0);
    let intent: Value = row.get(1);
    let unresolved = || ok(202, json!({"outcome":"unknown","request_id":key}));
    let config = ctx
        .github_writes
        .as_ref()
        .ok_or_else(|| problem(503, "GITHUB_WRITE_DISABLED", "no configured GitHub writer"))?;
    let settings = &config.repos[repo];
    let Some(slug) = intent["repository"]
        .as_str()
        .filter(|slug| settings["repository"] == *slug)
    else {
        return unresolved();
    };
    let Some(branch) = intent["branch"].as_str() else {
        return unresolved();
    };
    let Some(sha) = intent["candidate"]["head_sha"]
        .as_str()
        .filter(|sha| is_sha(sha))
    else {
        return unresolved();
    };
    let merging = intent["merge"] == true;
    let token_file = if merging {
        "merge_token_file"
    } else {
        "push_token_file"
    };
    let Some(token) = settings[token_file]
        .as_str()
        .and_then(|path| std::fs::read_to_string(path).ok())
    else {
        return unresolved();
    };
    let token = token.trim();
    if token.is_empty() || !token.bytes().all(|byte| byte.is_ascii_graphic()) {
        return unresolved();
    }
    let api = github_api(config, slug, token);
    let read = |path: &str| api("GET", path).call().ok()?.into_json::<Value>().ok();
    let Some(remote) = read(&format!("/pulls/{pr}")) else {
        return unresolved();
    };
    if remote["head"]["repo"]["full_name"] != slug
        || remote["base"]["repo"]["full_name"] != slug
        || remote["head"]["ref"] != branch
        || remote["head"]["sha"] != sha
        || remote["base"]["ref"] != "main"
    {
        return unresolved();
    }
    let result = if merging {
        let Some(merged) = remote["merge_commit_sha"]
            .as_str()
            .filter(|sha| is_sha(sha))
        else {
            return unresolved();
        };
        if remote["merged"] != true {
            return unresolved();
        }
        let Some(comparison) = read(&format!("/compare/{merged}...main")) else {
            return unresolved();
        };
        if comparison["base_commit"]["sha"] != merged
            || !matches!(comparison["status"].as_str(), Some("identical" | "ahead"))
        {
            return unresolved();
        }
        // The PR identifies the merge commit. Its second parent identifies the
        // head actually merged, independently of the PR's current head.
        let Some(commit) = read(&format!("/git/commits/{merged}")) else {
            return unresolved();
        };
        if commit["sha"] != merged
            || !commit["parents"].as_array().is_some_and(|parents| {
                parents.len() == 2
                    && parents[0]["sha"].as_str().is_some_and(is_sha)
                    && parents[0]["sha"] != sha
                    && parents[1]["sha"] == sha
            })
        {
            return unresolved();
        }
        json!({"outcome":"merged","head_sha":sha,"merged_sha":merged,"reconciled":true})
    } else {
        let Some(reference) = read(&format!("/git/ref/heads/{branch}")) else {
            return unresolved();
        };
        if reference["ref"] != format!("refs/heads/{branch}") || reference["object"]["sha"] != sha {
            return unresolved();
        }
        json!({"outcome":"pushed","head_sha":sha,"reconciled":true})
    };
    tx.execute(
        "UPDATE github_write_intents SET result=$3 WHERE repo_id=$1 AND request_id=$2
           AND (result IS NULL OR result->>'outcome'='unknown')",
        &[&repo, &key, &result],
    )?;
    ok(200, result)
}
