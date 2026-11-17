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
//! * the run is **the one the decision names** ([`pinned`]: 02-05 `s8-r1`, its session and its
//!   kept stream; review d971f59 L-4), and it is eligible ([`eligible`]): a model run of the ledger, stopped by the guard on a
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
use std::path::{Path, PathBuf};

pub const SETTLEMENT_FORMAT: &str = "ieo-role-run-settlement/v0";
pub const SOURCE: &str = "transcript_exit_accounting";
/// The policy a settled run carries; `caller::spare_releasable` releases a guard-induced
/// "no result event" only for it.
pub const POLICY: &str = "guard_stop_on_old_rule_warning";
const NO_RESULT: &str = "no result event";
/// The verdicts of an independent check a settlement may follow (decision §2.1).
pub const VERDICTS: [&str; 2] = ["CONFIRMED", "CONFIRMED WITH CAVEAT"];
/// What client-side evidence never covers (independent verification of s8-r1, its caveat).
pub const UNCOVERED: &str = "unlogged_in_flight_request_at_sigterm";
pub const RESIDUAL: &str = "settled from the session transcript; one api-request logged \
    (requestRef = api-request id, 1:1 with the one message); a prompt_snapshot for a next \
    request was written at its end with no api-request after it; a request sent just before \
    SIGTERM whose log write was lost cannot be ruled out client-side; provider-side billing is \
    not visible";

/// The decision a settlement follows (it is the only one that allows it).
pub const DECISION: &str = "docs/poc/User_Decisions_2026-11-17_r1-settlement.md";

/// The one run the decision allows to be settled (user decision 2026-11-17 r1-settlement §2.3:
/// "이번 s8-r1에 한정한 특례"; PR #90 review d971f59 L-4): the 02-05 budget's `s8-r1`, its
/// session and its kept stream (SHA-256 of the ledger's `streams/s8-r1.jsonl`, recorded in the
/// run's record). No other run is ever settled, and no other guard stop is released by a spare.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pin {
    pub budget: String,
    pub attempt: String,
    pub session: String,
    /// `None` only for a test pin (any stream of that run)
    pub stream_sha256: Option<String>,
}
pub const PIN_BUDGET: &str = "02-05";
pub const PIN_ATTEMPT: &str = "s8-r1";
pub const PIN_SESSION: &str = "5c991c71-b6e2-4cfb-bd76-6bb18a36e090";
pub const PIN_STREAM_SHA256: &str =
    "252609c73723fc8bff77ff50ad6a2c57f85ee818f4cfd2dfe685e0f11ff34c94";

static TEST_PINS: std::sync::Mutex<Vec<Pin>> = std::sync::Mutex::new(Vec::new());

/// A pin for synthetic ledgers (tests, dry runs), honoured only in a build with debug
/// assertions; a release build (what is deployed) knows only the decision's pin. Tests in the
/// same process use this; a debug binary also reads `IEO_SETTLEMENT_TEST_PINS`
/// (`budget|attempt|session|stream-sha256-or-*`, `;`-separated).
#[doc(hidden)]
pub fn allow_test_pin(p: Pin) {
    if cfg!(debug_assertions) {
        TEST_PINS.lock().unwrap_or_else(|e| e.into_inner()).push(p);
    }
}

/// The pins in force: the decision's, and in a debug build the test pins.
pub fn pins() -> Vec<Pin> {
    let mut v = vec![Pin {
        budget: PIN_BUDGET.into(),
        attempt: PIN_ATTEMPT.into(),
        session: PIN_SESSION.into(),
        stream_sha256: Some(PIN_STREAM_SHA256.into()),
    }];
    if cfg!(debug_assertions) {
        v.extend(
            TEST_PINS
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .iter()
                .cloned(),
        );
        if let Ok(e) = std::env::var("IEO_SETTLEMENT_TEST_PINS") {
            for p in e.split(';').filter(|p| !p.is_empty()) {
                let f: Vec<&str> = p.split('|').collect();
                if let [b, a, s, h] = f.as_slice() {
                    v.push(Pin {
                        budget: (*b).into(),
                        attempt: (*a).into(),
                        session: (*s).into(),
                        stream_sha256: (*h != "*").then(|| (*h).to_string()),
                    });
                }
            }
        }
    }
    v
}

/// Whether `r` is the run the decision names (budget, attempt, session, kept stream).
pub fn pinned(r: &Value) -> Result<(), String> {
    let hit = pins().iter().any(|p| {
        r["budget"] == p.budget.as_str()
            && r["attempt_id"] == p.attempt.as_str()
            && r["session"]["session_id"] == p.session.as_str()
            && p.stream_sha256
                .as_deref()
                .is_none_or(|h| r["stream"]["sha256"] == h)
    });
    if hit {
        Ok(())
    } else {
        Err(format!(
            "{}/{} is not the run the decision allows to settle (02-05 s8-r1 of session \
             {PIN_SESSION}, its kept stream)",
            r["budget"], r["attempt_id"]
        ))
    }
}

/// The independent check a settlement follows: `{evidence, sha256, verdict}` with a 64-hex
/// SHA-256 of the evidence file and a verdict of [`VERDICTS`].
pub fn independent_check_ok(c: &Value) -> Result<(), String> {
    if c["evidence"].as_str().is_none_or(|e| e.trim().is_empty()) {
        return Err("independent_check.evidence missing".into());
    }
    if c["sha256"].as_str().is_none_or(|h| {
        h.len() != 64
            || !h
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    }) {
        return Err("independent_check.sha256 is not a SHA-256".into());
    }
    if !c["verdict"].as_str().is_some_and(|v| VERDICTS.contains(&v)) {
        return Err(format!(
            "independent_check.verdict {} is not one of {VERDICTS:?}",
            c["verdict"]
        ));
    }
    Ok(())
}

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
    /// input + output + cache_creation; `None` on overflow (never settled)
    pub fn billable(&self) -> Option<u64> {
        self.tokens[0]
            .checked_add(self.tokens[1])?
            .checked_add(self.tokens[2])
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
    pinned(r)?;
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

/// The settlement's files kept in the ledger (`<ledger dir>/children/<attempt>/`): the
/// transcript copy and the independent check's report copy (PR #90 review 7b904de N-1).
pub const COPY_NAME: &str = "settlement-transcript.jsonl";
pub const REPORT_NAME: &str = "independent-check.md";

/// Where a settlement file lives: `<canonical ledger dir>/children/<attempt>/<name>`. The ledger
/// directory is taken by its canonical path, so the writer and every reader (CLI, coordinator)
/// agree however the plan spells it, an alias of the ledger directory included (review 7b904de
/// N-2). Inside the ledger nothing may be a symlink: `children`, `children/<attempt>` and the
/// file are checked with `symlink_metadata` (reviews d971f59 L-1, 7b904de N-3).
pub fn child_path(ledger_dir: &Path, attempt: &str, name: &str) -> Result<PathBuf, String> {
    let base = std::fs::canonicalize(ledger_dir).map_err(|e| format!("ledger dir: {e}"))?;
    Ok(base.join("children").join(attempt).join(name))
}

/// No symlink inside the ledger on the way to `file` (`children`, `children/<attempt>`), and
/// `file` itself, when it exists, a regular file. `Ok(false)` when `file` does not exist.
fn no_symlink_in_ledger(file: &Path) -> Result<bool, String> {
    let attempt_dir = file.parent().ok_or("settlement path without a directory")?;
    let children = attempt_dir
        .parent()
        .ok_or("settlement path without children/")?;
    for d in [children, attempt_dir] {
        match std::fs::symlink_metadata(d) {
            Ok(m) if m.file_type().is_dir() => {}
            Ok(_) => {
                return Err(format!(
                    "{} is not a directory of the ledger (a symlink?)",
                    d.display()
                ));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(format!("{}: {e}", d.display())),
        }
    }
    match std::fs::symlink_metadata(file) {
        Ok(m) if m.file_type().is_file() => Ok(true),
        Ok(_) => Err(format!(
            "{} is not a regular file (a symlink?)",
            file.display()
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(format!("{}: {e}", file.display())),
    }
}

/// Read a settlement file kept in the ledger: the recorded path must lead (canonically) to
/// [`child_path`], with no symlink inside the ledger, and the bytes must have the recorded
/// SHA-256. The one check every reader uses.
pub fn read_in_ledger(
    ledger_dir: &Path,
    attempt: &str,
    name: &str,
    recorded: &Value,
    sha256: &Value,
) -> Result<Vec<u8>, String> {
    let want = child_path(ledger_dir, attempt, name)?;
    let p = recorded
        .as_str()
        .ok_or(format!("settlement without the path of its {name}"))?;
    if !Path::new(p).is_absolute() {
        return Err(format!("settlement file {p} is not an absolute path"));
    }
    if !no_symlink_in_ledger(&want)? {
        return Err(format!("settlement file {} is missing", want.display()));
    }
    let canon = std::fs::canonicalize(p).map_err(|e| format!("settlement file {p}: {e}"))?;
    if canon != want {
        return Err(format!(
            "settlement file {p} is not the ledger's {} (the copy lives in the ledger)",
            want.display()
        ));
    }
    let b = std::fs::read(&want).map_err(|e| format!("settlement file {p}: {e}"))?;
    if *sha256 != hex::encode(Sha256::digest(&b)).as_str() {
        return Err(format!(
            "settlement file {p} differs from its recorded SHA-256"
        ));
    }
    Ok(b)
}

/// The ledger's `children` and `children/<attempt>` for a settlement, each checked with
/// `symlink_metadata` **before** anything is created (PR #90 review 758ac03 O-1): an existing one
/// must be a real directory, a missing one is created (0700, one level at a time). Returns the
/// directories it created, so a failed settlement removes them again (it leaves no empty
/// directory behind and creates nothing through a symlink).
pub fn prepare_dirs(ledger_dir: &Path, attempt: &str) -> Result<Vec<PathBuf>, String> {
    use std::os::unix::fs::DirBuilderExt;
    let base = std::fs::canonicalize(ledger_dir).map_err(|e| format!("ledger dir: {e}"))?;
    let children = base.join("children");
    let dirs = [children.clone(), children.join(attempt)];
    for d in &dirs {
        match std::fs::symlink_metadata(d) {
            Ok(m) if m.file_type().is_dir() => {}
            Ok(_) => {
                return Err(format!(
                    "{} is not a directory of the ledger (a symlink?): nothing created",
                    d.display()
                ));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("{}: {e}", d.display())),
        }
    }
    let mut made = Vec::new();
    for d in &dirs {
        if std::fs::symlink_metadata(d).is_err() {
            if let Err(e) = std::fs::DirBuilder::new().mode(0o700).create(d) {
                remove_made(&made);
                return Err(format!("{}: {e}", d.display()));
            }
            made.push(d.clone());
        }
    }
    Ok(made)
}

/// Remove directories [`prepare_dirs`] created (only when empty), the deepest first.
pub fn remove_made(made: &[PathBuf]) {
    for d in made.iter().rev() {
        let _ = std::fs::remove_dir(d);
    }
}

/// Keep `bytes` as the ledger's settlement file `name` (its directories from [`prepare_dirs`]):
/// created new (0600), never through a symlink; an existing regular file must hold the same bytes. Returns the path
/// and whether it was created (the caller removes what it created when the settlement fails).
pub fn write_in_ledger(
    ledger_dir: &Path,
    attempt: &str,
    name: &str,
    bytes: &[u8],
) -> Result<(PathBuf, bool), String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let to = child_path(ledger_dir, attempt, name)?;
    // the directories come from prepare_dirs (checked before anything is created)
    if no_symlink_in_ledger(&to)? {
        let old = std::fs::read(&to).map_err(|e| format!("{}: {e}", to.display()))?;
        if old != bytes {
            return Err(format!("{} already holds a different file", to.display()));
        }
        return Ok((to, false));
    }
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&to)
        .map_err(|e| format!("settlement copy {}: {e}", to.display()))?;
    if let Err(e) = f.write_all(bytes).and_then(|()| f.sync_all()) {
        // a partial file is never left behind (PR #91 review 4b427c5 F-4)
        drop(f);
        let _ = std::fs::remove_file(&to);
        return Err(format!(
            "settlement copy {}: {e} (partial file removed)",
            to.display()
        ));
    }
    Ok((to, true))
}

/// The independent check's report as given (`bytes`) against the record: its SHA-256 is the
/// recorded one, and the original path is absolute (provenance; it is never read again: every
/// read checks the ledger's copy, review 7b904de N-1).
fn check_report(c: &Value, bytes: &[u8]) -> Result<(), String> {
    independent_check_ok(c)?;
    let p = c["evidence"].as_str().unwrap_or_default();
    if !Path::new(p).is_absolute() {
        return Err(format!(
            "independent_check.evidence {p} is not an absolute path"
        ));
    }
    if c["sha256"] != hex::encode(Sha256::digest(bytes)).as_str() {
        return Err(format!(
            "independent_check report differs from its recorded SHA-256 ({p})"
        ));
    }
    Ok(())
}

/// Check a settlement line against the run it settles (both as written) and return the run as
/// it counts. `stream` is the run's kept stream text; `ledger_dir` the ledger it is read from.
pub fn apply(run: &Value, s: &Value, stream: &str, ledger_dir: &Path) -> Result<Value, String> {
    let a = run["attempt_id"].as_str().unwrap_or("?").to_owned();
    let copy = read_in_ledger(
        ledger_dir,
        &a,
        COPY_NAME,
        &s["evidence"]["ledger_copy"],
        &s["evidence"]["sha256"],
    )?;
    let report = read_in_ledger(
        ledger_dir,
        &a,
        REPORT_NAME,
        &s["independent_check"]["ledger_copy"],
        &s["independent_check"]["sha256"],
    )?;
    apply_bytes(run, s, stream, &copy, &report)
}

/// [`apply`] with the transcript copy's bytes given (a dry run reads them from the evidence
/// copy, writing nothing).
fn apply_bytes(
    run: &Value,
    s: &Value,
    stream: &str,
    copy: &[u8],
    report: &[u8],
) -> Result<Value, String> {
    let a = run["attempt_id"].as_str().unwrap_or("?");
    if s["source"] != SOURCE || s["policy"] != POLICY || s["model_request"] != false {
        return Err(format!("settlement of {a}: unknown source or policy"));
    }
    if s["settled_at"].as_str().is_none_or(|v| v.trim().is_empty()) {
        return Err(format!("settlement of {a}: settled_at missing"));
    }
    if s["decision"] != DECISION {
        return Err(format!(
            "settlement of {a}: decision {} is not {DECISION}",
            s["decision"]
        ));
    }
    check_report(&s["independent_check"], report).map_err(|e| format!("settlement of {a}: {e}"))?;
    // the caveat travels with the settlement (it is never dropped)
    if s["verification"]["uncovered"] != UNCOVERED || s["verification"]["residual"] != RESIDUAL {
        return Err(format!(
            "settlement of {a}: the uncovered residual is missing"
        ));
    }
    if s["original_stream_sha256"] != run["stream"]["sha256"]
        || s["session_id"] != run["session"]["session_id"]
        || s["role"] != run["role"]
        || s["budget"] != run["budget"]
    {
        return Err(format!("settlement of {a} does not match the run"));
    }
    // the copy is the original (review d971f59 L-2): both digests recorded, and equal
    if s["evidence"]["original_sha256"] != s["evidence"]["sha256"]
        || s["evidence"]["sha256"] != hex::encode(Sha256::digest(copy)).as_str()
    {
        return Err(format!(
            "settlement of {a}: the copy is not the original (digests differ)"
        ));
    }
    eligible(run, stream).map_err(|e| format!("settlement of {a}: {e}"))?;
    let sid = run["session"]["session_id"].as_str().unwrap_or_default();
    let acc = transcript_accounting(copy, sid).map_err(|e| format!("settlement of {a}: {e}"))?;
    if s["usage_run"]["tokens"] != acc.tokens_json()
        || s["verification"]["api_requests"] != acc.api_requests
        || s["verification"]["messages"] != acc.messages
    {
        return Err(format!(
            "settlement of {a} differs from its transcript accounting"
        ));
    }
    let billable = acc
        .billable()
        .ok_or(format!("settlement of {a}: token counts overflow"))?;
    // never the guard's lower bound: the settled usage is the accounting, and not below it
    let o = &run["outcome"]["observed"];
    if o["billable_lower_bound"]
        .as_u64()
        .is_none_or(|lb| billable < lb)
        || o["cache_read_lower_bound"]
            .as_u64()
            .is_none_or(|lb| acc.tokens[3] < lb)
    {
        return Err(format!(
            "settlement of {a}: the accounting is below the guard's observed lower bound"
        ));
    }
    // within the run's own limits (review d971f59 L-6): a token overrun is not this case
    let l = &run["limits"];
    if l["billable_room"]
        .as_u64()
        .is_none_or(|room| billable > room)
        || l["cache_read_room"]
            .as_u64()
            .is_none_or(|room| acc.tokens[3] > room)
    {
        return Err(format!(
            "settlement of {a}: the accounting exceeds the run's own limits {l}"
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

/// The settlement line for `run`: the transcript `bytes` (the verified evidence copy) and the
/// independent check's `report` bytes, the paths in the ledger they are (or would be) kept at,
/// and their provenance. Checked like a read before it is returned: against the ledger's files
/// when `on_disk` (the real write, after the copies), against the given bytes otherwise (a dry
/// run writes nothing).
#[allow(clippy::too_many_arguments)]
pub fn build(
    run: &Value,
    stream: &str,
    bytes: &[u8],
    report: &[u8],
    ledger_dir: &Path,
    on_disk: bool,
    original_path: &str,
    original_sha256: &str,
    decision: &str,
    independent_check: &Value,
) -> Result<Value, String> {
    let a = run["attempt_id"].as_str().unwrap_or("?");
    check_report(independent_check, report)?;
    eligible(run, stream)?;
    let sid = run["session"]["session_id"].as_str().unwrap_or_default();
    let acc = transcript_accounting(bytes, sid)?;
    if acc.sha256 != original_sha256 {
        return Err(format!(
            "the copy's SHA-256 {} is not the original's {original_sha256}",
            acc.sha256
        ));
    }
    let ledger_copy = child_path(ledger_dir, a, COPY_NAME)?;
    // the report's original path and digest stay as provenance; reads check the ledger's copy
    let mut check = independent_check.clone();
    check["ledger_copy"] = json!(child_path(ledger_dir, a, REPORT_NAME)?);
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
            "uncovered": UNCOVERED, "residual": RESIDUAL},
        "usage_run": {"state": "measured", "tokens": acc.tokens_json(), "source": SOURCE},
        "lower_bound_not_used": run["outcome"]["observed"],
        "rule": "runtime contract rev 19 §20 item 31; rev 20 §21 item 32",
        "decision": decision, "independent_check": check,
        "settled_at": ieo_evidence::time::now()});
    if on_disk {
        apply(run, &s, stream, ledger_dir)?;
    } else {
        apply_bytes(run, &s, stream, bytes, report)?;
    }
    Ok(s)
}
