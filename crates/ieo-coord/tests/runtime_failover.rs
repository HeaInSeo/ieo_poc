use ieo_coord::failover::*;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

const NOW: u64 = 1000;
static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

struct Journal(PathBuf);
impl Journal {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "ieo-runtime-failover-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Journal {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Send {
    runtime: Runtime,
    attempt: u64,
    session: String,
    stage: String,
    last_completed_stage: Option<String>,
    checkpoint: Checkpoint,
    seconds: u64,
    billable: u64,
    cache_read: u64,
}

struct World {
    checkpoint: Checkpoint,
    sends: Vec<Send>,
    expected_approval: String,
    verified: bool,
    paid_usage: bool,
    apply_effect: bool,
    proof_not_sent: bool,
    measured_ms: u64,
}

struct Evidence(Arc<Mutex<World>>);
impl Authority for Evidence {
    fn permits(&self, p: &Plan, cp: &Checkpoint) -> bool {
        let w = self.0.lock().unwrap();
        w.expected_approval == p.approval && &w.checkpoint == cp
    }

    fn submit_if_permitted(&self, p: &Plan, d: &Dispatch, submit: Box<dyn FnOnce() + '_>) -> bool {
        if d.approval != p.approval || !self.permits(p, &d.checkpoint) {
            return false;
        }
        submit();
        true
    }

    fn reconcile(&self, attempt: &Dispatch) -> Resolution {
        let w = self.0.lock().unwrap();
        let recorded = w.sends.iter().any(|s| {
            s.attempt == attempt.attempt
                && s.session == attempt.new_session
                && s.runtime == attempt.runtime
                && s.stage == attempt.stage
                && s.checkpoint == attempt.checkpoint
        });
        if !recorded && w.proof_not_sent && w.checkpoint == attempt.checkpoint {
            return Resolution::NotSent;
        }
        if !w.verified || !recorded {
            return Resolution::Unknown;
        }
        let usage = Usage {
            duration_ms: w.measured_ms,
            input: 2,
            output: 3,
            cache_creation: 1,
            cache_read: 4,
            cost_microusd: u64::from(w.paid_usage),
        };
        if w.checkpoint == attempt.checkpoint {
            return Resolution::NoEffect { usage };
        }
        if w.checkpoint.completed != attempt.checkpoint.completed + 1 {
            return Resolution::Unknown;
        }
        Resolution::Completed {
            checkpoint: w.checkpoint.clone(),
            usage,
        }
    }
}

#[derive(Clone)]
struct Fake {
    runner: Runner,
    world: Arc<Mutex<World>>,
    status: &'static str,
    observed_at: u64,
    extra_usage_disabled: bool,
    revoke_on_observe: bool,
    crash_before_send: bool,
}
impl Adapter for Fake {
    fn runner(&self) -> &Runner {
        &self.runner
    }
    fn observe(&self) -> Option<Observation> {
        if self.revoke_on_observe {
            self.world.lock().unwrap().expected_approval = "revoked".into();
        }
        let signal = match self.runner.runtime {
            Runtime::Claude => serde_json::json!({
                "type":"rate_limit_event",
                "rate_limit_info":{"status":self.status,"overageStatus":"rejected","isUsingOverage":false}
            }),
            Runtime::Codex => serde_json::json!({
                "format":"ieo-codex-quota-fixture/v0","status":self.status
            }),
        };
        Some(Observation {
            group: self.runner.group.clone(),
            observed_at: self.observed_at,
            extra_usage_disabled: self.extra_usage_disabled,
            signal: signal.to_string(),
        })
    }
    fn submit(&mut self, d: &Dispatch) {
        assert!(!self.crash_before_send, "fixture crash before send");
        let mut w = self.world.lock().unwrap();
        assert_eq!(d.assignment, "A17");
        assert_eq!(d.role, "development");
        assert_eq!(d.permissions, "role-profile-digest");
        assert_eq!(d.branch, "refs/heads/feat/a17");
        assert_eq!(d.runtime, self.runner.runtime);
        assert_eq!(d.checkpoint, w.checkpoint);
        assert_eq!(d.max_extra_microusd, 0);
        w.sends.push(Send {
            runtime: d.runtime,
            attempt: d.attempt,
            session: d.new_session.clone(),
            stage: d.stage.clone(),
            last_completed_stage: d.last_completed_stage.clone(),
            checkpoint: d.checkpoint.clone(),
            seconds: d.seconds,
            billable: d.billable,
            cache_read: d.cache_read,
        });
        if !w.apply_effect {
            return;
        }
        w.checkpoint.completed += 1;
        let completed = w.checkpoint.completed;
        w.checkpoint.head = format!("{completed:040x}");
        w.checkpoint.verification = format!("verified-stage-{completed}");
    }
}

struct Scenario {
    journal: Journal,
    world: Arc<Mutex<World>>,
    plan: Plan,
    initial: Checkpoint,
    now: u64,
    claude: Fake,
    codex: Fake,
}

impl Scenario {
    fn new(primary: Runtime) -> Self {
        let initial = Checkpoint {
            head: "a".repeat(40),
            completed: 0,
            verification: "initial-verified".into(),
        };
        let world = Arc::new(Mutex::new(World {
            checkpoint: initial.clone(),
            sends: vec![],
            expected_approval: "reviewed-revision-4".into(),
            verified: true,
            paid_usage: false,
            apply_effect: true,
            proof_not_sent: false,
            measured_ms: 1001,
        }));
        let fake = |runtime, group: &str, status| Fake {
            runner: Runner {
                runtime,
                group: group.into(),
                role: "development".into(),
                permissions: "role-profile-digest".into(),
            },
            world: world.clone(),
            status,
            observed_at: NOW,
            extra_usage_disabled: true,
            revoke_on_observe: false,
            crash_before_send: false,
        };
        let claude = fake(Runtime::Claude, "claude-sub", "allowed");
        let codex = fake(Runtime::Codex, "codex-sub", "available");
        Self {
            journal: Journal::new(),
            world,
            initial: initial.clone(),
            now: NOW,
            claude,
            codex,
            plan: Plan {
                assignment: "A17".into(),
                role: "development".into(),
                approval: "reviewed-revision-4".into(),
                permissions: "role-profile-digest".into(),
                branch: "refs/heads/feat/a17".into(),
                head: initial.head,
                stages: vec!["prepare".into(), "verify".into()],
                primary,
                ceiling: Ceiling {
                    runs: 2,
                    seconds: 30,
                    per_run_seconds: 30,
                    billable: 50,
                    cache_read: 80,
                },
            },
        }
    }
    fn advance(&mut self) -> Decision {
        advance(
            &self.journal.0,
            &self.plan,
            &self.initial,
            &Evidence(self.world.clone()),
            &mut [&mut self.claude, &mut self.codex],
            self.now,
        )
        .unwrap()
    }
    fn exhaust(&mut self, runtime: Runtime) {
        self.now += 1;
        match runtime {
            Runtime::Claude => {
                self.claude.observed_at = self.now;
                self.claude.status = "rejected";
            }
            Runtime::Codex => {
                self.codex.observed_at = self.now;
                self.codex.status = "exhausted";
            }
        }
    }
    fn exhaust_both(&mut self) {
        self.exhaust(Runtime::Claude);
        self.exhaust(Runtime::Codex);
    }
}

#[test]
fn swaps_both_directions_and_only_sends_the_remaining_stage_in_a_new_session() {
    for (first, second) in [
        (Runtime::Claude, Runtime::Codex),
        (Runtime::Codex, Runtime::Claude),
    ] {
        let mut s = Scenario::new(first);
        assert_eq!(s.advance(), Decision::Dispatched(first));
        s.exhaust(first);
        assert_eq!(s.advance(), Decision::Dispatched(second));
        assert_eq!(s.advance(), Decision::Complete);
        let w = s.world.lock().unwrap();
        assert_eq!(w.sends.len(), 2);
        assert_eq!(w.sends[0].stage, "prepare");
        let next = &w.sends[1];
        assert_eq!(next.stage, "verify");
        assert_eq!(next.last_completed_stage.as_deref(), Some("prepare"));
        assert_ne!(next.session, w.sends[0].session);
        assert_eq!(next.checkpoint.head, format!("{:040x}", 1));
        assert_eq!(next.checkpoint.verification, "verified-stage-1");
        assert_eq!((next.seconds, next.billable, next.cache_read), (28, 44, 76));
    }
}

#[test]
fn unresolved_effect_blocks_after_restart_then_verified_effect_skips_the_stage() {
    let mut s = Scenario::new(Runtime::Claude);
    assert_eq!(s.advance(), Decision::Dispatched(Runtime::Claude));
    s.exhaust(Runtime::Claude);
    s.world.lock().unwrap().verified = false;
    // advance reconstructs state from disk; nothing depends on the first invocation's memory.
    assert_eq!(s.advance(), Decision::AwaitingReconciliation);
    assert_eq!(s.advance(), Decision::AwaitingReconciliation);
    assert_eq!(s.world.lock().unwrap().sends.len(), 1);
    s.world.lock().unwrap().verified = true;
    assert_eq!(s.advance(), Decision::Dispatched(Runtime::Codex));
    assert_eq!(s.world.lock().unwrap().sends[1].stage, "verify");
}

#[test]
fn both_exhausted_wait_and_recovery_submits_once() {
    let mut s = Scenario::new(Runtime::Claude);
    assert_eq!(s.advance(), Decision::Dispatched(Runtime::Claude));
    s.exhaust_both();
    for _ in 0..2 {
        assert_eq!(s.advance(), Decision::WaitingForQuota);
    }
    assert_eq!(s.world.lock().unwrap().sends.len(), 1);
    s.codex.status = "available";
    assert_eq!(s.advance(), Decision::QuotaUnknown);
    assert_eq!(s.world.lock().unwrap().sends.len(), 1);
    s.now += 1;
    s.codex.observed_at = s.now;
    assert_eq!(s.advance(), Decision::Dispatched(Runtime::Codex));
    assert_eq!(s.advance(), Decision::Complete);
    assert_eq!(s.advance(), Decision::Complete);
    assert_eq!(s.world.lock().unwrap().sends.len(), 2);
}

#[test]
fn budget_approval_and_role_constraints_override_alternate_capacity() {
    let mut exhausted = Scenario::new(Runtime::Claude);
    exhausted.plan.ceiling.runs = 1;
    assert_eq!(exhausted.advance(), Decision::Dispatched(Runtime::Claude));
    exhausted.claude.status = "rejected";
    assert_eq!(exhausted.advance(), Decision::BlockedBudget);
    assert_eq!(exhausted.world.lock().unwrap().sends.len(), 1);

    let mut revoked = Scenario::new(Runtime::Claude);
    assert_eq!(revoked.advance(), Decision::Dispatched(Runtime::Claude));
    revoked.exhaust_both();
    assert_eq!(revoked.advance(), Decision::WaitingForQuota);
    revoked.codex.status = "available";
    revoked.world.lock().unwrap().expected_approval = "superseded".into();
    assert_eq!(revoked.advance(), Decision::BlockedApproval);
    assert_eq!(revoked.world.lock().unwrap().sends.len(), 1);

    let mut wrong_role = Scenario::new(Runtime::Claude);
    wrong_role.exhaust(Runtime::Claude);
    wrong_role.codex.runner.role = "guardrail".into();
    assert_eq!(wrong_role.advance(), Decision::BlockedApproval);
    assert!(wrong_role.world.lock().unwrap().sends.is_empty());
}

#[test]
fn unknown_and_billable_overage_fail_closed() {
    let mut s = Scenario::new(Runtime::Claude);
    s.claude.observed_at = 0;
    assert_eq!(s.advance(), Decision::QuotaUnknown);
    s.claude.observed_at = NOW;
    s.exhaust(Runtime::Claude);
    s.codex.extra_usage_disabled = false;
    assert_eq!(s.advance(), Decision::QuotaUnknown);
    assert!(s.world.lock().unwrap().sends.is_empty());
    s.codex.extra_usage_disabled = true;
    s.now += 1;
    s.codex.observed_at = s.now;
    s.world.lock().unwrap().paid_usage = true;
    assert_eq!(s.advance(), Decision::Dispatched(Runtime::Codex));
    assert_eq!(s.advance(), Decision::BlockedBudget);
    assert_eq!(s.world.lock().unwrap().sends.len(), 1);
}

#[test]
fn concurrent_resume_requests_send_only_one_remaining_stage() {
    let mut s = Scenario::new(Runtime::Claude);
    assert_eq!(s.advance(), Decision::Dispatched(Runtime::Claude));
    s.exhaust_both();
    assert_eq!(s.advance(), Decision::WaitingForQuota);
    s.now += 1;
    s.codex.observed_at = s.now;
    s.codex.status = "available";
    let results = std::thread::scope(|scope| {
        let run = || {
            advance(
                &s.journal.0,
                &s.plan,
                &s.initial,
                &Evidence(s.world.clone()),
                &mut [&mut s.claude.clone(), &mut s.codex.clone()],
                s.now,
            )
            .unwrap()
        };
        let first = scope.spawn(run);
        let second = scope.spawn(run);
        [first.join().unwrap(), second.join().unwrap()]
    });
    assert!(results.contains(&Decision::Dispatched(Runtime::Codex)));
    assert!(results.contains(&Decision::Complete));
    assert_eq!(s.world.lock().unwrap().sends.len(), 2);
}

#[test]
fn confirmed_no_effect_requires_observed_recovery_and_charges_the_first_attempt() {
    let mut s = Scenario::new(Runtime::Claude);
    s.world.lock().unwrap().apply_effect = false;
    assert_eq!(s.advance(), Decision::Dispatched(Runtime::Claude));
    assert_eq!(s.advance(), Decision::AwaitingQuotaRecovery);
    s.exhaust_both();
    assert_eq!(s.advance(), Decision::WaitingForQuota);
    s.now += 1;
    s.claude.status = "allowed";
    s.claude.observed_at = s.now;
    s.world.lock().unwrap().apply_effect = true;
    assert_eq!(s.advance(), Decision::Dispatched(Runtime::Claude));
    assert_eq!(s.advance(), Decision::BlockedBudget);
    let w = s.world.lock().unwrap();
    assert_eq!(w.sends.len(), 2);
    assert_eq!(w.sends[1].stage, "prepare");
    assert_eq!((w.sends[1].seconds, w.sends[1].billable), (28, 44));
    assert_ne!(w.sends[0].session, w.sends[1].session);
}

#[test]
fn exhausted_pair_can_resume_when_only_the_recovered_runtime_has_fresh_evidence() {
    let mut s = Scenario::new(Runtime::Codex);
    s.exhaust_both();
    assert_eq!(s.advance(), Decision::WaitingForQuota);
    assert!(s.world.lock().unwrap().sends.is_empty());
    s.now += 601;
    s.claude.observed_at = s.now;
    s.claude.status = "allowed";
    assert_eq!(s.advance(), Decision::Dispatched(Runtime::Claude));
    assert_eq!(s.world.lock().unwrap().sends.len(), 1);
}

#[test]
fn exhausted_time_or_tokens_block_even_with_an_available_alternate() {
    for set_cap in [
        (|cap: &mut Ceiling| cap.seconds = 8) as fn(&mut Ceiling),
        |cap| cap.billable = 6,
        |cap| cap.cache_read = 4,
    ] {
        let mut s = Scenario::new(Runtime::Claude);
        set_cap(&mut s.plan.ceiling);
        assert_eq!(s.advance(), Decision::Dispatched(Runtime::Claude));
        s.exhaust(Runtime::Claude);
        assert_eq!(s.advance(), Decision::BlockedBudget);
        assert_eq!(s.world.lock().unwrap().sends.len(), 1);
    }
}

#[test]
fn a_changed_worktree_head_blocks_recovery() {
    let mut s = Scenario::new(Runtime::Claude);
    assert_eq!(s.advance(), Decision::Dispatched(Runtime::Claude));
    s.exhaust_both();
    assert_eq!(s.advance(), Decision::WaitingForQuota);
    s.now += 1;
    s.codex.observed_at = s.now;
    s.codex.status = "available";
    s.world.lock().unwrap().checkpoint.head = "b".repeat(40);
    assert_eq!(s.advance(), Decision::BlockedApproval);
    assert_eq!(s.world.lock().unwrap().sends.len(), 1);
}

#[test]
fn revocation_during_quota_observation_blocks_submission() {
    let mut s = Scenario::new(Runtime::Claude);
    s.claude.revoke_on_observe = true;
    assert_eq!(s.advance(), Decision::BlockedApproval);
    assert!(s.world.lock().unwrap().sends.is_empty());
}

#[test]
fn old_and_conflicting_exhaustion_cannot_trigger_failover() {
    let mut s = Scenario::new(Runtime::Claude);
    assert_eq!(s.advance(), Decision::Dispatched(Runtime::Claude));
    s.claude.status = "rejected";
    s.claude.observed_at = NOW - 1;
    assert_eq!(s.advance(), Decision::QuotaUnknown);
    s.claude.observed_at = NOW;
    assert_eq!(s.advance(), Decision::QuotaUnknown);
    assert_eq!(s.advance(), Decision::QuotaUnknown);
    assert_eq!(s.world.lock().unwrap().sends.len(), 1);
    s.exhaust(Runtime::Claude);
    assert_eq!(s.advance(), Decision::Dispatched(Runtime::Codex));
}

#[test]
fn confirmed_per_run_time_breach_blocks_even_if_budget_has_room() {
    let mut s = Scenario::new(Runtime::Claude);
    s.plan.ceiling.per_run_seconds = 10;
    s.world.lock().unwrap().measured_ms = 11000;
    assert_eq!(s.advance(), Decision::Dispatched(Runtime::Claude));
    s.exhaust(Runtime::Claude);
    assert_eq!(s.advance(), Decision::BlockedBudget);
    assert_eq!(s.world.lock().unwrap().sends.len(), 1);
}

#[test]
fn verified_not_sent_does_not_consume_the_only_run() {
    let mut s = Scenario::new(Runtime::Claude);
    s.plan.ceiling.runs = 1;
    s.claude.crash_before_send = true;
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.advance())).is_err());
    assert_eq!(s.advance(), Decision::AwaitingReconciliation);
    assert!(s.world.lock().unwrap().sends.is_empty());
    s.claude.crash_before_send = false;
    s.world.lock().unwrap().proof_not_sent = true;
    assert_eq!(s.advance(), Decision::Dispatched(Runtime::Claude));
    assert_eq!(s.advance(), Decision::BlockedBudget);
    let world = s.world.lock().unwrap();
    assert_eq!(world.sends.len(), 1);
    assert_eq!(world.sends[0].attempt, 2);
    assert_eq!(world.sends[0].seconds, 30);
}

#[test]
fn a_verified_no_send_preserves_recovery_when_only_the_alternate_is_fresh() {
    let mut s = Scenario::new(Runtime::Claude);
    s.plan.ceiling.runs = 1;
    s.exhaust_both();
    assert_eq!(s.advance(), Decision::WaitingForQuota);
    s.now += 601;
    s.codex.status = "available";
    s.codex.observed_at = s.now;
    s.codex.crash_before_send = true;
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.advance())).is_err());
    assert_eq!(s.advance(), Decision::AwaitingReconciliation);
    assert!(s.world.lock().unwrap().sends.is_empty());
    s.codex.crash_before_send = false;
    s.world.lock().unwrap().proof_not_sent = true;
    assert_eq!(s.advance(), Decision::Dispatched(Runtime::Codex));
    assert_eq!(s.world.lock().unwrap().sends.len(), 1);
}

#[test]
fn a_conflict_cancels_exhaustion_as_proof_of_same_runtime_recovery() {
    let mut s = Scenario::new(Runtime::Claude);
    s.world.lock().unwrap().apply_effect = false;
    assert_eq!(s.advance(), Decision::Dispatched(Runtime::Claude));
    s.exhaust_both();
    assert_eq!(s.advance(), Decision::WaitingForQuota);
    s.claude.status = "allowed";
    assert_eq!(s.advance(), Decision::QuotaUnknown);
    s.now += 1;
    s.claude.observed_at = s.now;
    assert_eq!(s.advance(), Decision::AwaitingQuotaRecovery);
    assert_eq!(s.world.lock().unwrap().sends.len(), 1);
}
