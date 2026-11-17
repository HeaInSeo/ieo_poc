//! Settlement of a guard-stopped run from the CLI's own exit accounting (user decision
//! 2026-11-17, `docs/poc/User_Decisions_2026-11-17_r1-settlement.md`, (가) and (A)).
//!
//! The case it exists for is 02-05 `s8-r1`: the run guard stopped the run on a
//! `rate_limit_event` that the rule of that time refused (`allowed_warning`) and that the rule
//! now in force passes (runtime contract rev 19 §20 item 31). The stream has no result event, so
//! the run is unmeasured and blocks the budget. The session transcript the CLI kept has the
//! final usage of every message and, written at exit, its session totals (`cost-state`).
//!
//! A settlement is a **separate line** of the ledger (`ieo-role-run-settlement/v0`). The original
//! record is never changed. On every ledger read ([`crate::caller::records`]) the settlement is
//! checked again from the files it names, and applied to the run it settles:
//!
//! * the run is eligible ([`eligible`]): a model run of the ledger, stopped by the guard on a
//!   rate-limit signal (`Overage`), unmeasured, without a result event, without child runs,
//!   whose stop reasons are only "no result event", "usage unmeasured" and "this run's signal"
//!   and whose kept stream (same SHA-256) has rate-limit events that the current rule all
//!   passes, at least one of them a warning. Any other stop (a real `rejected`, extra usage, a
//!   time or token cap, a lost answer, a failed cleanup, an environment that is not
//!   subscription-only) is never settled here;
//! * the transcript copy kept in the ledger directory (`children/<attempt>/settlement-transcript.jsonl`)
//!   has the recorded SHA-256, and its accounting ([`transcript_accounting`]) equals the usage
//!   the settlement states;
//! * one settlement per run, of an earlier run, never of a corrected one.
//!
//! Applied, the run counts once with the settled usage (`usage.run.state: measured`,
//! `source: transcript_exit_accounting`) and **stays a failed run**: `stop_required` stays true
//! with the reason "no result event" (the guard's stop left no result). The earlier values are
//! kept under `before_settlement`. That remaining reason may be released by a person's spare
//! marker only for a settled run (`caller::spare_releasable`): the special case the decision
//! scopes to this kind of run. The observed lower bound is never used as the usage.

use crate::budget::{self, RateLimitVerdict};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const SETTLEMENT_FORMAT: &str = "ieo-role-run-settlement/v0";
pub const SOURCE: &str = "transcript_exit_accounting";
/// The policy a settled run carries; `caller::spare_releasable` releases a guard-induced
/// "no result event" only for it.
pub const POLICY: &str = "guard_stop_on_old_rule_warning";
const NO_RESULT: &str = "no result event";

/// What a session transcript says about its usage (no content kept).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accounting {
    /// input, output, cache_creation, cache_read of the distinct messages (last entry of each id)
    pub tokens: [u64; 4],
    pub api_requests: u64,
    pub messages: u64,
    pub entries: u64,
    pub sha256: String,
    pub bytes: u64,
}

impl Accounting {
    pub fn tokens_json(&self) -> Value {
        json!({"input": self.tokens[0], "output": self.tokens[1],
               "cache_creation": self.tokens[2], "cache_read": self.tokens[3]})
    }
    pub fn billable(&self) -> u64 {
        self.tokens[0] + self.tokens[1] + self.tokens[2]
    }
}

fn n(v: &Value, k: &str) -> Result<u64, String> {
    v[k].as_u64()
        .ok_or(format!("{k} missing or not a whole number"))
}

/// The usage of one session from its transcript (the CLI's `projects/<cwd>/<session>.jsonl`),
/// fail-closed: every line is JSON and names the session; no child (side-chain) entries; the
/// distinct assistant messages (last entry of each `message.id`) are as many as the
/// `api-request` entries (at least one); and the last `cost-state` entry's `modelUsage`, summed
/// over models, equals the messages' sum in all four token kinds. Anything else is not enough
/// to settle on.
pub fn transcript_accounting(bytes: &[u8], session_id: &str) -> Result<Accounting, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "transcript is not UTF-8".to_string())?;
    let mut msgs: BTreeMap<String, [u64; 4]> = BTreeMap::new();
    let (mut requests, mut entries) = (0u64, 0u64);
    let mut cost: Option<Value> = None;
    for (i, line) in text.split('\n').enumerate() {
        if line.is_empty() {
            continue;
        }
        entries += 1;
        let e: Value = serde_json::from_str(line)
            .map_err(|_| format!("transcript line {}: not JSON", i + 1))?;
        if e["sessionId"] != session_id {
            return Err(format!(
                "transcript line {}: session {} is not {session_id}",
                i + 1,
                e["sessionId"]
            ));
        }
        if e["isSidechain"] == true || !e["agentId"].is_null() {
            return Err(format!(
                "transcript line {}: a child (side-chain) entry: not settled here",
                i + 1
            ));
        }
        match e["type"].as_str() {
            Some("api-request") => requests += 1,
            Some("assistant") => {
                let m = &e["message"];
                let id = m["id"].as_str().filter(|s| !s.is_empty()).ok_or(format!(
                    "transcript line {}: assistant without message.id",
                    i + 1
                ))?;
                let u = &m["usage"];
                let t = [
                    n(u, "input_tokens")?,
                    n(u, "output_tokens")?,
                    n(u, "cache_creation_input_tokens")?,
                    n(u, "cache_read_input_tokens")?,
                ];
                // the last entry of a message carries its final usage
                msgs.insert(id.to_owned(), t);
            }
            Some("cost-state") => cost = Some(e),
            _ => {}
        }
    }
    if requests == 0 {
        return Err("no api-request entry: not enough to settle on".into());
    }
    if msgs.len() as u64 != requests {
        return Err(format!(
            "{} api-request entries but {} distinct assistant messages",
            requests,
            msgs.len()
        ));
    }
    let Some(cost) = cost else {
        return Err(
            "no cost-state entry (the CLI's exit accounting): not enough to settle on".into(),
        );
    };
    let models = cost["modelUsage"]
        .as_object()
        .filter(|m| !m.is_empty())
        .ok_or("cost-state without modelUsage")?;
    let mut exit = [0u64; 4];
    for (name, m) in models {
        let k = [
            "inputTokens",
            "outputTokens",
            "cacheCreationInputTokens",
            "cacheReadInputTokens",
        ];
        for (j, key) in k.iter().enumerate() {
            exit[j] = exit[j]
                .checked_add(
                    m[*key]
                        .as_u64()
                        .ok_or(format!("cost-state {name}: {key}"))?,
                )
                .ok_or("overflow")?;
        }
    }
    let mut sum = [0u64; 4];
    for t in msgs.values() {
        for j in 0..4 {
            sum[j] = sum[j].checked_add(t[j]).ok_or("overflow")?;
        }
    }
    if sum != exit {
        return Err(format!(
            "the messages' usage {sum:?} differs from the CLI's exit accounting {exit:?}"
        ));
    }
    Ok(Accounting {
        tokens: sum,
        api_requests: requests,
        messages: msgs.len() as u64,
        entries,
        sha256: hex::encode(Sha256::digest(bytes)),
        bytes: bytes.len() as u64,
    })
}

/// Whether a recorded run is one a settlement may settle (see the module documentation).
/// `stream` is the kept stream's text; its SHA-256 must be the record's.
pub fn eligible(r: &Value, stream: &str) -> Result<(), String> {
    let a = r["attempt_id"].as_str().unwrap_or("?");
    if r["format"] != crate::caller::RECORD_FORMAT || r["model_request"] != true {
        return Err(format!("{a} is not a model run record"));
    }
    if r["usage"]["run"]["state"] != "unmeasured" {
        return Err(format!("{a} is not unmeasured"));
    }
    if r["outcome"]["stopped_by_guard"] != true
        || !r["outcome"]["stop"]
            .as_str()
            .is_some_and(|s| s.starts_with("Overage("))
    {
        return Err(format!(
            "{a} was not stopped by the run guard on a rate-limit signal (stop {})",
            r["outcome"]["stop"]
        ));
    }
    if r["result_event"]["present"] != false || !r["outcome"]["result"].is_null() {
        return Err(format!("{a} has a result event"));
    }
    if r["children"].as_array().is_none_or(|c| !c.is_empty()) {
        return Err(format!("{a} has child runs: not settled here"));
    }
    if !r["spare"].is_null() {
        return Err(format!("{a} is a spare run"));
    }
    let reason = r["stop_reason"].as_str().unwrap_or_default();
    let parts: Vec<&str> = reason.split("; ").collect();
    let mut signal = false;
    for p in &parts {
        if *p == NO_RESULT || p.starts_with("usage unmeasured: ") {
            continue;
        }
        if p.starts_with("this run's signal: ") {
            signal = true;
            continue;
        }
        return Err(format!("{a} stopped for another reason: {p}"));
    }
    if !signal || !parts.contains(&NO_RESULT) {
        return Err(format!(
            "{a} is not a guard stop on a rate-limit signal: {reason}"
        ));
    }
    if hex::encode(Sha256::digest(stream.as_bytes()))
        != r["stream"]["sha256"].as_str().unwrap_or("")
    {
        return Err(format!(
            "{a}: the kept stream differs from the recorded one"
        ));
    }
    let events = budget::rate_limit_events(stream)?;
    let mut warned = false;
    for (i, e) in events.iter().enumerate() {
        match budget::rate_limit_verdict(e) {
            RateLimitVerdict::Stop(why) => {
                return Err(format!(
                    "{a}: event {} is a stop under the current rule: {why}",
                    i + 1
                ));
            }
            RateLimitVerdict::Warning(_) => warned = true,
            RateLimitVerdict::Allowed => {}
        }
    }
    if !warned {
        return Err(format!(
            "{a}: no warning event in its stream (not a stop on the old rule's warning)"
        ));
    }
    Ok(())
}

/// The kept transcript copy of a settlement, read and checked against the recorded digest.
fn read_copy(s: &Value) -> Result<Vec<u8>, String> {
    let p = s["evidence"]["ledger_copy"]
        .as_str()
        .ok_or("settlement without evidence.ledger_copy")?;
    if !std::path::Path::new(p).is_absolute() {
        return Err(format!("settlement copy {p} is not an absolute path"));
    }
    let b = std::fs::read(p).map_err(|e| format!("settlement copy {p}: {e}"))?;
    if s["evidence"]["sha256"] != hex::encode(Sha256::digest(&b)).as_str() {
        return Err(format!(
            "settlement copy {p} differs from its recorded SHA-256"
        ));
    }
    Ok(b)
}

/// Check a settlement line against the run it settles (both as written) and return the run as
/// it counts. `stream` is the run's kept stream text.
pub fn apply(run: &Value, s: &Value, stream: &str) -> Result<Value, String> {
    let a = run["attempt_id"].as_str().unwrap_or("?");
    if s["source"] != SOURCE || s["policy"] != POLICY || s["model_request"] != false {
        return Err(format!("settlement of {a}: unknown source or policy"));
    }
    for k in ["decision", "independent_check", "settled_at"] {
        if s[k].as_str().is_none_or(|v| v.trim().is_empty()) {
            return Err(format!("settlement of {a}: {k} missing"));
        }
    }
    if s["original_stream_sha256"] != run["stream"]["sha256"]
        || s["session_id"] != run["session"]["session_id"]
        || s["role"] != run["role"]
        || s["budget"] != run["budget"]
    {
        return Err(format!("settlement of {a} does not match the run"));
    }
    eligible(run, stream).map_err(|e| format!("settlement of {a}: {e}"))?;
    let sid = run["session"]["session_id"].as_str().unwrap_or_default();
    let acc = transcript_accounting(&read_copy(s)?, sid)
        .map_err(|e| format!("settlement of {a}: {e}"))?;
    if s["usage_run"]["tokens"] != acc.tokens_json()
        || s["verification"]["api_requests"] != acc.api_requests
        || s["verification"]["messages"] != acc.messages
    {
        return Err(format!(
            "settlement of {a} differs from its transcript accounting"
        ));
    }
    // never the guard's lower bound: the settled usage is the accounting, and not below it
    let o = &run["outcome"]["observed"];
    if o["billable_lower_bound"]
        .as_u64()
        .is_none_or(|lb| acc.billable() < lb)
        || o["cache_read_lower_bound"]
            .as_u64()
            .is_none_or(|lb| acc.tokens[3] < lb)
    {
        return Err(format!(
            "settlement of {a}: the accounting is below the guard's observed lower bound"
        ));
    }
    let mut r = run.clone();
    r["before_settlement"] = json!({"usage_run": run["usage"]["run"],
        "stop_required": run["stop_required"], "stop_reason": run["stop_reason"]});
    r["usage"]["run"] = json!({"state": "measured", "tokens": acc.tokens_json(), "source": SOURCE});
    // the run stays failed: the guard's stop left no result (user decision §2.2)
    r["stop_required"] = json!(true);
    r["stop_reason"] = json!(NO_RESULT);
    r["settlement"] = json!({"source": SOURCE, "policy": POLICY, "decision": s["decision"],
        "independent_check": s["independent_check"], "settled_at": s["settled_at"],
        "evidence_sha256": s["evidence"]["sha256"]});
    Ok(r)
}

/// The settlement line for `run` from a transcript copy already in the ledger directory
/// (`ledger_copy`, absolute), with its provenance. Checked like a read ([`apply`]) before it is
/// returned.
#[allow(clippy::too_many_arguments)]
pub fn build(
    run: &Value,
    stream: &str,
    ledger_copy: &std::path::Path,
    original_path: &str,
    original_sha256: &str,
    decision: &str,
    independent_check: &str,
) -> Result<Value, String> {
    let a = run["attempt_id"].as_str().unwrap_or("?");
    eligible(run, stream)?;
    let b = std::fs::read(ledger_copy).map_err(|e| format!("{}: {e}", ledger_copy.display()))?;
    let sid = run["session"]["session_id"].as_str().unwrap_or_default();
    let acc = transcript_accounting(&b, sid)?;
    if acc.sha256 != original_sha256 {
        return Err(format!(
            "the copy's SHA-256 {} is not the original's {original_sha256}",
            acc.sha256
        ));
    }
    let signals: Vec<Value> = budget::rate_limit_events(stream)?
        .iter()
        .map(|e| match budget::rate_limit_verdict(e) {
            RateLimitVerdict::Warning(w) => json!({"verdict": "warning", "text": w}),
            RateLimitVerdict::Allowed => json!({"verdict": "allowed"}),
            RateLimitVerdict::Stop(w) => json!({"verdict": "stop", "text": w}),
        })
        .collect();
    let s = json!({"format": SETTLEMENT_FORMAT, "settles": a, "model_request": false,
        "budget": run["budget"], "role": run["role"], "session_id": sid,
        "source": SOURCE, "policy": POLICY,
        "original_stream_sha256": run["stream"]["sha256"],
        "evidence": {"ledger_copy": ledger_copy, "sha256": acc.sha256, "bytes": acc.bytes,
                     "original_path": original_path, "original_sha256": original_sha256},
        "verification": {"api_requests": acc.api_requests, "messages": acc.messages,
            "entries": acc.entries, "exit_accounting_equals_messages": true,
            "signals_under_current_rule": signals,
            "residual": "a request sent after the guard's stop and never written to the \
                transcript cannot be seen on the client side"},
        "usage_run": {"state": "measured", "tokens": acc.tokens_json(), "source": SOURCE},
        "lower_bound_not_used": run["outcome"]["observed"],
        "rule": "runtime contract rev 19 §20 item 31; rev 20 §21 item 32",
        "decision": decision, "independent_check": independent_check,
        "settled_at": ieo_evidence::time::now()});
    apply(run, &s, stream)?;
    Ok(s)
}
