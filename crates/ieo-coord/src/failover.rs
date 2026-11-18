//! Isolated coordinator policy for cross-runtime continuation. The host supplies trusted,
//! read-only authority and quota evidence. This module has no live adapters or server route.

use ieo_supervisor::{budget, provider};
use serde_json::{Value, json};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Runtime {
    Claude,
    Codex,
}

impl Runtime {
    fn name(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Quota {
    Available,
    Exhausted,
    Unknown,
}

pub struct Observation {
    pub group: String,
    pub observed_at: u64,
    pub extra_usage_disabled: bool,
    pub signal: String,
}

fn classify(runtime: Runtime, signal: &str) -> Quota {
    match runtime {
        Runtime::Claude => {
            let status = provider::classify(signal);
            match status["value"].as_str() {
                Some("usage_limit")
                    if status["source"]["status"] == "rejected"
                        && status["source"]["isUsingOverage"] == false
                        && status["unreadable_lines"] == 0 =>
                {
                    Quota::Exhausted
                }
                Some("available") if status["unreadable_lines"] == 0 => Quota::Available,
                _ => Quota::Unknown,
            }
        }
        Runtime::Codex => {
            let Ok(value) = serde_json::from_str::<Value>(signal) else {
                return Quota::Unknown;
            };
            if value["format"] != "ieo-codex-quota-fixture/v0" {
                return Quota::Unknown;
            }
            match value["status"].as_str() {
                Some("available") => Quota::Available,
                Some("exhausted") => Quota::Exhausted,
                _ => Quota::Unknown,
            }
        }
    }
}

#[derive(Clone)]
pub struct Runner {
    pub runtime: Runtime,
    pub group: String,
    pub role: String,
    pub permissions: String,
}

#[derive(Clone, Copy)]
pub struct Ceiling {
    pub runs: u64,
    pub seconds: u64,
    pub per_run_seconds: u64,
    pub billable: u64,
    pub cache_read: u64,
}

pub struct Plan {
    pub assignment: String,
    pub role: String,
    pub approval: String,
    pub permissions: String,
    pub branch: String,
    pub head: String,
    pub stages: Vec<String>,
    pub primary: Runtime,
    pub ceiling: Ceiling,
}

impl Plan {
    fn identity(&self) -> Value {
        let c = self.ceiling;
        json!({
            "assignment": self.assignment, "role": self.role, "approval": self.approval,
            "permissions": self.permissions, "branch": self.branch, "head": self.head,
            "stages": self.stages, "primary": self.primary.name(),
            "caps": [c.runs, c.seconds, c.per_run_seconds, c.billable, c.cache_read]
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checkpoint {
    pub head: String,
    pub completed: usize,
    pub verification: String,
}

pub struct Dispatch {
    pub runtime: Runtime,
    pub attempt: u64,
    pub approval: String,
    /// The adapter must create this coordinator-issued session, never reconnect another CLI.
    pub new_session: String,
    pub assignment: String,
    pub role: String,
    pub permissions: String,
    pub branch: String,
    pub checkpoint: Checkpoint,
    pub last_completed_stage: Option<String>,
    pub stage: String,
    pub seconds: u64,
    pub billable: u64,
    pub cache_read: u64,
    pub max_extra_microusd: u64,
}

#[derive(Clone)]
pub struct Usage {
    pub duration_ms: u64,
    pub input: u64,
    pub output: u64,
    pub cache_creation: u64,
    pub cache_read: u64,
    pub cost_microusd: u64,
}

pub enum Resolution {
    Unknown,
    /// Independent evidence confirms the intent never reached the runner and had no effect.
    NotSent,
    /// The authority independently confirms the ledger and actual effect for this attempt.
    Completed {
        checkpoint: Checkpoint,
        usage: Usage,
    },
    /// The authority independently confirms no effect and measured usage for this attempt.
    NoEffect {
        usage: Usage,
    },
}

pub trait Authority {
    /// Verify current assignment, approval/review, branch, head and effect against the checkpoint.
    fn permits(&self, plan: &Plan, checkpoint: &Checkpoint) -> bool;
    /// Validate the approval revision, worktree and role at submission. A live implementation
    /// must hold the authorization fence through submission, including against revocation.
    /// Return false only if submit was never invoked. Keep uncertainty as a pending intent.
    fn submit_if_permitted(
        &self,
        plan: &Plan,
        dispatch: &Dispatch,
        submit: Box<dyn FnOnce() + '_>,
    ) -> bool;
    /// Correlate the exact attempt with the ledger and actual worktree/remote effect.
    fn reconcile(&self, attempt: &Dispatch) -> Resolution;
}

pub trait Adapter {
    fn runner(&self) -> &Runner;
    /// Must not make a model call.
    fn observe(&self) -> Option<Observation>;
    /// Submit exactly once. Even a lost reply must be reconciled by the authority.
    fn submit(&mut self, dispatch: &Dispatch);
}

#[derive(Debug, Eq, PartialEq)]
pub enum Decision {
    Dispatched(Runtime),
    Complete,
    WaitingForQuota,
    QuotaUnknown,
    BlockedApproval,
    BlockedBudget,
    AwaitingReconciliation,
    AwaitingQuotaRecovery,
}

fn checkpoint_value(checkpoint: &Checkpoint) -> Value {
    json!([
        checkpoint.head,
        checkpoint.completed,
        checkpoint.verification
    ])
}

fn checkpoint_from(value: &Value) -> Option<Checkpoint> {
    Some(Checkpoint {
        head: value.get(0)?.as_str()?.to_owned(),
        completed: usize::try_from(value.get(1)?.as_u64()?).ok()?,
        verification: value.get(2)?.as_str()?.to_owned(),
    })
}

fn invalid() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "invalid or changed failover journal",
    )
}

fn persist(dir: &Path, state: &Value) -> std::io::Result<()> {
    let next = dir.join("next.json");
    let mut file = File::create(&next)?;
    serde_json::to_writer(&mut file, state)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    fs::rename(next, dir.join("journal.json"))?;
    File::open(dir)?.sync_all()
}

fn make_dispatch(
    plan: &Plan,
    checkpoint: &Checkpoint,
    runtime: Runtime,
    attempt: u64,
    remaining: [u64; 3],
) -> Dispatch {
    Dispatch {
        runtime,
        attempt,
        approval: plan.approval.clone(),
        new_session: format!("{}-failover-{attempt}", plan.assignment),
        assignment: plan.assignment.clone(),
        role: plan.role.clone(),
        permissions: plan.permissions.clone(),
        branch: plan.branch.clone(),
        checkpoint: checkpoint.clone(),
        last_completed_stage: checkpoint
            .completed
            .checked_sub(1)
            .map(|i| plan.stages[i].clone()),
        stage: plan.stages[checkpoint.completed].clone(),
        seconds: remaining[0],
        billable: remaining[1],
        cache_read: remaining[2],
        max_extra_microusd: 0,
    }
}

fn quota(adapter: &dyn Adapter, now: u64, history: &mut Value) -> Quota {
    let Some(o) = adapter.observe() else {
        return Quota::Unknown;
    };
    if o.group != adapter.runner().group
        || now
            .checked_sub(o.observed_at)
            .is_none_or(|age| age > provider::DEFAULT_MAX_AGE_SECONDS)
    {
        return Quota::Unknown;
    }
    let verdict = if o.extra_usage_disabled {
        classify(adapter.runner().runtime, &o.signal)
    } else {
        Quota::Unknown
    };
    let label = match verdict {
        Quota::Available => "available",
        Quota::Exhausted => "exhausted",
        Quota::Unknown => "unknown",
    };
    if let Some(previous) = history["observed_at"].as_u64() {
        if o.observed_at < previous {
            return Quota::Unknown;
        }
        if o.observed_at == previous && history["status"] != label {
            history["status"] = json!("unknown");
            return Quota::Unknown;
        }
    }
    history["observed_at"] = json!(o.observed_at);
    history["status"] = json!(label);
    if verdict == Quota::Exhausted {
        history["exhausted_at"] = json!(o.observed_at);
    }
    verdict
}

fn usage_breached(a: &Value) -> bool {
    if a["model_request"] == false {
        return false;
    }
    let usage = budget::spent(std::slice::from_ref(a));
    [
        usage.seconds,
        usage.billable_tokens,
        usage.cache_read_tokens,
    ]
    .into_iter()
    .enumerate()
    .any(|(i, used)| a["remaining"][i].as_u64().is_none_or(|cap| used > cap))
        || a["cost_microusd"].as_u64() != Some(0)
}

/// Advance at most one dispatch in a private, pre-existing journal directory owned by the caller.
/// The directory must be dedicated to this assignment and excluded from runner write access.
pub fn advance(
    dir: &Path,
    plan: &Plan,
    initial: &Checkpoint,
    authority: &dyn Authority,
    adapters: &mut [&mut dyn Adapter; 2],
    now: u64,
) -> std::io::Result<Decision> {
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("journal.lock"))?;
    lock.lock()?;
    let identity = plan.identity();
    let bindings = json!(
        adapters
            .iter()
            .map(|a| {
                let r = a.runner();
                json!([r.runtime.name(), r.group, r.role, r.permissions])
            })
            .collect::<Vec<_>>()
    );
    let mut state = match fs::read(dir.join("journal.json")) {
        Ok(bytes) => serde_json::from_slice(&bytes)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({
            "plan": identity, "runners": bindings, "initial": checkpoint_value(initial),
            "checkpoint": checkpoint_value(initial), "attempts": [],
            "quota": {"claude": {}, "codex": {}}, "waiting": false
        }),
        Err(e) => return Err(e),
    };
    if plan.approval.is_empty()
        || plan.permissions.is_empty()
        || plan.branch.is_empty()
        || plan.head.is_empty()
        || state["plan"] != identity
        || state["runners"] != bindings
        || state["initial"] != checkpoint_value(initial)
        || initial.head != plan.head
        || adapters[0].runner().runtime == adapters[1].runner().runtime
        || adapters[0].runner().group == adapters[1].runner().group
        || adapters.iter().any(|a| a.runner().group.is_empty())
    {
        return Err(invalid());
    }

    let mut checkpoint = checkpoint_from(&state["checkpoint"]).ok_or_else(invalid)?;
    if checkpoint.completed > plan.stages.len()
        || checkpoint.completed < initial.completed
        || checkpoint.verification.is_empty()
    {
        return Err(invalid());
    }
    let attempts = state["attempts"].as_array_mut().ok_or_else(invalid)?;
    if let Some(last) = attempts.last_mut()
        && last["result"].is_null()
    {
        if checkpoint.completed == plan.stages.len() {
            return Err(invalid());
        }
        let runtime = adapters
            .iter()
            .map(|a| a.runner().runtime)
            .find(|r| last["runtime"] == r.name())
            .ok_or_else(invalid)?;
        let cap = |i| {
            last["remaining"]
                .get(i)
                .and_then(Value::as_u64)
                .ok_or_else(invalid)
        };
        let dispatch = make_dispatch(
            plan,
            &checkpoint,
            runtime,
            last["id"].as_u64().ok_or_else(invalid)?,
            [cap(0usize)?, cap(1usize)?, cap(2usize)?],
        );
        let (result, usage) = match authority.reconcile(&dispatch) {
            Resolution::Unknown => return Ok(Decision::AwaitingReconciliation),
            Resolution::Completed {
                checkpoint: next,
                usage,
            } if next.completed == checkpoint.completed + 1
                && !next.head.is_empty()
                && !next.verification.is_empty() =>
            {
                checkpoint = next;
                ("completed", Some(usage))
            }
            Resolution::Completed { .. } => return Ok(Decision::AwaitingReconciliation),
            Resolution::NoEffect { usage } => ("no_effect", Some(usage)),
            Resolution::NotSent => ("not_sent", None),
        };
        last["result"] = json!(result);
        last["model_request"] = json!(usage.is_some());
        if let Some(usage) = usage {
            last["duration_ms"] = json!(usage.duration_ms);
            last["cost_microusd"] = json!(usage.cost_microusd);
            last["usage"] = json!({"run": {"state": "measured", "tokens": {
                "input": usage.input, "output": usage.output, "cache_creation": usage.cache_creation,
                "cache_read": usage.cache_read
            }}});
        }
        state["checkpoint"] = checkpoint_value(&checkpoint);
        persist(dir, &state)?;
    }

    if !authority.permits(plan, &checkpoint) {
        return Ok(Decision::BlockedApproval);
    }
    let attempts = state["attempts"].as_array_mut().ok_or_else(invalid)?;
    let spent = budget::spent(attempts);
    let c = plan.ceiling;
    let seconds = c
        .seconds
        .saturating_sub(spent.seconds)
        .min(c.per_run_seconds);
    let billable = c.billable.saturating_sub(spent.billable_tokens);
    let cache_read = c.cache_read.saturating_sub(spent.cache_read_tokens);
    if spent.runs > c.runs
        || spent.seconds > c.seconds
        || spent.billable_tokens > c.billable
        || spent.cache_read_tokens > c.cache_read
        || !spent.unmeasured_runs.is_empty()
        || !spent.unclassified_records.is_empty()
        || attempts.iter().any(usage_breached)
    {
        return Ok(Decision::BlockedBudget);
    }
    if checkpoint.completed == plan.stages.len() {
        return Ok(Decision::Complete);
    }
    if spent.runs >= c.runs
        || seconds <= ieo_supervisor::run::GRACE.as_secs()
        || billable == 0
        || cache_read == 0
    {
        return Ok(Decision::BlockedBudget);
    }

    // Stay on the last runtime until it is explicitly exhausted. A recovery observation must
    // be newer than the last exhaustion, including across process restarts.
    let last_sent = attempts.iter().rfind(|a| a["model_request"] == true);
    let preferred = last_sent.map_or(plan.primary.name(), |a| {
        a["runtime"].as_str().unwrap_or_default()
    });
    let primary = adapters
        .iter()
        .position(|a| a.runner().runtime.name() == preferred)
        .ok_or_else(invalid)?;
    let no_effect = last_sent
        .filter(|a| a["result"] == "no_effect")
        .and_then(|a| a["submitted_at"].as_u64());
    let waiting = state["waiting"].as_bool().ok_or_else(invalid)?;
    let observations = state["quota"].as_object_mut().ok_or_else(invalid)?;
    let observe = |index: usize, observations: &mut serde_json::Map<String, Value>| {
        let adapter = &*adapters[index];
        let last = observations
            .get_mut(adapter.runner().runtime.name())
            .ok_or_else(invalid)?;
        Ok::<_, std::io::Error>(quota(adapter, now, last))
    };
    let selection = match observe(primary, observations)? {
        Quota::Available
            if no_effect.is_some_and(|sent| {
                observations[adapters[primary].runner().runtime.name()]["exhausted_at"]
                    .as_u64()
                    .is_none_or(|exhausted| exhausted < sent)
            }) =>
        {
            Err(Decision::AwaitingQuotaRecovery)
        }
        Quota::Available => Ok(primary),
        Quota::Unknown if !waiting => Err(Decision::QuotaUnknown),
        status => match observe(1 - primary, observations)? {
            Quota::Available => Ok(1 - primary),
            Quota::Exhausted if status == Quota::Exhausted || waiting => {
                Err(Decision::WaitingForQuota)
            }
            Quota::Exhausted => Err(Decision::QuotaUnknown),
            Quota::Unknown => Err(Decision::QuotaUnknown),
        },
    };
    if selection == Err(Decision::WaitingForQuota) {
        state["waiting"] = json!(true);
    }
    persist(dir, &state)?;
    let index = match selection {
        Ok(index) => index,
        Err(decision) => return Ok(decision),
    };
    let attempts = state["attempts"].as_array_mut().ok_or_else(invalid)?;
    let runner = adapters[index].runner();
    if runner.role != plan.role || runner.permissions != plan.permissions {
        return Ok(Decision::BlockedApproval);
    }
    let runtime = runner.runtime;
    let attempt = u64::try_from(attempts.len())
        .map_err(|_| invalid())?
        .checked_add(1)
        .ok_or_else(invalid)?;
    let remaining = [seconds, billable, cache_read];
    let dispatch = make_dispatch(plan, &checkpoint, runtime, attempt, remaining);
    attempts.push(
        json!({"id": attempt, "runtime": runtime.name(), "remaining": remaining,
        "stage": dispatch.stage, "session": dispatch.new_session, "result": null, "submitted_at": now}),
    );
    state["waiting"] = json!(false);
    persist(dir, &state)?;
    if !authority.submit_if_permitted(
        plan,
        &dispatch,
        Box::new(|| adapters[index].submit(&dispatch)),
    ) {
        let attempt = state["attempts"]
            .as_array_mut()
            .and_then(|a| a.last_mut())
            .ok_or_else(invalid)?;
        attempt["result"] = json!("not_sent");
        attempt["model_request"] = json!(false);
        persist(dir, &state)?;
        return Ok(Decision::BlockedApproval);
    }
    Ok(Decision::Dispatched(runtime))
}
