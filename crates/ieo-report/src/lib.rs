//! Progress report v0 (task 01-08) — reference implementation of the parts of
//! the contract that are decidable without a server:
//!
//! * [`validate_report`]: JSON Schema + the semantic rules the schema cannot express
//!   (same rule set as `docs/poc/scripts/validate_contracts.py`; known differences in the
//!   Python tool are listed in the 01-08 contract §6);
//! * [`authorize_claim`], [`authorize_repro`], [`may_read_sprint`], [`may_read_repro`]:
//!   credential checks, run **before** body validation (401 → 403 → 400/409/422);
//! * [`render_human`]: the five-part human report, headline states the evidence level;
//! * [`notification`]: re-notify on material change only;
//! * [`completion_summary`], [`review_currency`]: completion wording per scope, derived from facts.

use ieo_spec::schema::format_rejections;
use ieo_spec::{Code, Rejection};
use jsonschema::Validator;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::sync::OnceLock;

pub const PROGRESS_REPORT_SCHEMA: &str =
    include_str!("../../../docs/poc/ProgressReport_v0.schema.json");

fn validator() -> &'static Validator {
    static V: OnceLock<Validator> = OnceLock::new();
    V.get_or_init(|| {
        let schema: Value = serde_json::from_str(PROGRESS_REPORT_SCHEMA).unwrap_or(Value::Null);
        match jsonschema::draft202012::options()
            .should_validate_formats(true)
            .build(&schema)
        {
            Ok(v) => v,
            Err(e) => panic!("ProgressReport schema does not compile: {e}"),
        }
    })
}

fn semantic(field: &str, message: impl Into<String>, correction: &str) -> Rejection {
    Rejection::new(Code::ReportSemanticViolation, field, message, correction)
}

/// Strict RFC 3339 `date-time` with an explicit offset. Returns nanoseconds since
/// 1970-01-01T00:00:00Z for ordering. Rules: 4-digit year 0001–9999, real calendar
/// day (leap years), hour 00–23, minute 00–59, second 00–59 (no leap second in v0),
/// optional fraction (1–9 digits), offset `Z`/`z` or `±HH:MM` with HH 00–23, MM 00–59;
/// `T`/`t` separator.
pub fn parse_ts(s: &str) -> Option<i128> {
    let b = s.as_bytes();
    let digits = |r: std::ops::Range<usize>| -> Option<i64> {
        let t = b.get(r)?;
        if t.iter().all(u8::is_ascii_digit) {
            std::str::from_utf8(t).ok()?.parse().ok()
        } else {
            None
        }
    };
    if b.len() < 20
        || b[4] != b'-'
        || b[7] != b'-'
        || !matches!(b[10], b'T' | b't')
        || b[13] != b':'
        || b[16] != b':'
    {
        return None;
    }
    let (y, mo, d) = (digits(0..4)?, digits(5..7)?, digits(8..10)?);
    let (h, mi, se) = (digits(11..13)?, digits(14..16)?, digits(17..19)?);
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let mdays = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if y < 1
        || !(1..=12).contains(&mo)
        || d < 1
        || d > mdays[(mo - 1) as usize]
        || h > 23
        || mi > 59
        || se > 59
    {
        return None;
    }
    let mut i = 19;
    let mut nanos: i128 = 0;
    if b.get(i) == Some(&b'.') {
        let n = b[i + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
        if n == 0 || n > 9 {
            return None;
        }
        let frac: i128 = std::str::from_utf8(&b[i + 1..i + 1 + n])
            .ok()?
            .parse()
            .ok()?;
        nanos = frac * 10i128.pow((9 - n) as u32);
        i += 1 + n;
    }
    let rest = &b[i..];
    let off: i64 = match rest {
        [b'Z'] | [b'z'] => 0,
        [sign @ (b'+' | b'-'), h1, h2, b':', m1, m2]
            if [h1, h2, m1, m2].iter().all(|c| c.is_ascii_digit()) =>
        {
            let oh = i64::from((h1 - b'0') * 10 + (h2 - b'0'));
            let om = i64::from((m1 - b'0') * 10 + (m2 - b'0'));
            if oh > 23 || om > 59 {
                return None;
            }
            let v = oh * 3600 + om * 60;
            if *sign == b'-' { -v } else { v }
        }
        _ => return None,
    };
    let (yy, mm) = if mo <= 2 {
        (y - 1, mo + 9)
    } else {
        (y, mo - 3)
    };
    let era = yy.div_euclid(400);
    let yoe = yy - era * 400;
    let doy = (153 * mm + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let secs = days * 86_400 + h * 3600 + mi * 60 + se - off;
    Some(i128::from(secs) * 1_000_000_000 + nanos)
}

/// Schema + semantic checks. Empty = valid.
pub fn validate_report(r: &Value) -> Vec<Rejection> {
    let mut out = format_rejections(validator(), r);
    if !out.is_empty() {
        return out;
    }
    let ts = |v: &Value| v.as_str().and_then(parse_ts);
    let ts_fields = [
        ("observed_at", &r["observed_at"]),
        ("next_check_at", &r["next_check_at"]),
        ("decision.deadline_at", &r["decision"]["deadline_at"]),
        ("body.deadline_at", &r["body"]["deadline_at"]),
        ("schedule.deadline_at", &r["schedule"]["deadline_at"]),
        (
            "schedule.forecast.completion_range.earliest_at",
            &r["schedule"]["forecast"]["completion_range"]["earliest_at"],
        ),
        (
            "schedule.forecast.completion_range.latest_at",
            &r["schedule"]["forecast"]["completion_range"]["latest_at"],
        ),
    ];
    for (f, v) in ts_fields {
        if let Some(s) = v.as_str()
            && parse_ts(s).is_none()
        {
            out.push(semantic(
                f,
                format!("시간대가 있는 RFC 3339 시각이 아닙니다: {s}"),
                "예: 2026-11-12T09:00:00+09:00",
            ));
        }
    }
    let s = &r["schedule"];
    let f = &s["forecast"];
    if f["state"] == "estimated"
        && let (Some(e), Some(l)) = (
            ts(&f["completion_range"]["earliest_at"]),
            ts(&f["completion_range"]["latest_at"]),
        )
        && e > l
    {
        out.push(semantic(
            "schedule.forecast.completion_range",
            "예상 완료 범위의 앞뒤가 바뀌었습니다.",
            "earliest_at ≤ latest_at",
        ));
    }
    if s["deadline_state"] == "set"
        && s["status"] == "late"
        && let (Some(o), Some(d)) = (ts(&r["observed_at"]), ts(&s["deadline_at"]))
        && o <= d
    {
        out.push(semantic(
            "schedule.status",
            "기한 전인데 지연(late)으로 표시했습니다.",
            "기한 전이면 at_risk 또는 on_track",
        ));
    }
    let b = &r["body"];
    match r["report_type"].as_str() {
        Some("guardrail_review") => {
            if b["verdict"] == "reject" && b["findings"].as_array().is_none_or(Vec::is_empty) {
                out.push(semantic(
                    "body.findings",
                    "반려에는 발견이 하나 이상 필요합니다.",
                    "재현 가능한 발견을 추가하세요.",
                ));
            }
        }
        Some("decision_request") => {
            let d = &r["decision"];
            for k in [
                "question",
                "recommendation",
                "deadline_at",
                "default_action",
            ] {
                if d[k] != b[k] {
                    out.push(semantic(
                        &format!("decision.{k}"),
                        format!("decision.{k}와 body.{k}가 다릅니다."),
                        "같은 질문·선택을 한 곳의 값으로 맞추세요.",
                    ));
                }
            }
            let labels: Vec<String> = b["options"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|o| {
                    format!(
                        "{}: {}",
                        o["label"].as_str().unwrap_or_default(),
                        o["impact"].as_str().unwrap_or_default()
                    )
                })
                .collect();
            let given: Vec<String> = d["options"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect();
            if labels != given {
                out.push(semantic(
                    "decision.options",
                    "decision.options가 body.options의 `label: impact`와 다릅니다.",
                    "body 선택지에서 생성하세요.",
                ));
            }
            let rec = b["recommendation"].as_str().unwrap_or_default();
            if !b["options"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|o| o["label"] == rec)
            {
                out.push(semantic(
                    "body.recommendation",
                    "권고가 선택지에 없습니다.",
                    "선택지 label 중 하나를 권고하세요.",
                ));
            }
        }
        _ => {}
    }
    out
}

// ---------------------------------------------------------------------------
// Credentials and authorization
// ---------------------------------------------------------------------------

/// Server-side identity behind a bearer token. Body names are never trusted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Principal {
    /// Person or service with read/register scopes over a set of repos.
    Client {
        repos: BTreeSet<String>,
        can_register: bool,
    },
    /// Role agent bound to exactly one (repo, role); only `agent:*` scopes.
    Agent { repo_id: String, role: String },
}

/// One assignment the coordinator legitimately issued to a (repo, role), from its DB.
/// `target_sha` is the coordinator's record, never a value taken from an agent body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedAssignment {
    pub assignment_id: String,
    pub generation: u64,
    pub target_sha: String,
}

/// The coordinator's assignment history for one (repo, role): every issued
/// (assignment, generation) and which one is current (at most one).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssignmentBook {
    pub issued: Vec<IssuedAssignment>,
    pub current: Option<(String, u64)>,
}

impl AssignmentBook {
    fn lookup(&self, body: &Value) -> Option<(&IssuedAssignment, bool)> {
        let id = body["assignment_id"].as_str()?;
        let generation = body["generation"].as_u64()?;
        let a = self
            .issued
            .iter()
            .find(|a| a.assignment_id == id && a.generation == generation)?;
        let is_current = self
            .current
            .as_ref()
            .is_some_and(|(cid, cg)| cid == id && *cg == generation);
        Some((a, is_current))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Denied {
    /// 403: wrong repo for this credential.
    RepoScopeDenied,
    /// 403: role/report type not allowed for this credential.
    RoleMismatch,
    /// 403: agent tried to submit coordinator/verified content.
    VerifiedFromAgentDenied,
    /// 403: agent credentials cannot use client read endpoints (§1.2: roles do not see each other).
    AgentReadDenied,
    /// 409 `STALE_ASSIGNMENT`: a *new* execution request (repro) for a non-current
    /// assignment/generation, or a claim naming an assignment never issued to this (repo, role).
    StaleAssignment,
    /// 409 `EVENT_CONTENT_CONFLICT`: the same event/request ID was already stored (for this
    /// repo and role) with different content.
    #[serde(rename = "EVENT_CONTENT_CONFLICT")]
    ContentConflict,
    /// 409 `ASSIGNMENT_SHA_MISMATCH`: the body names a `target_sha` other than the SHA the
    /// coordinator recorded for that assignment; one claim never carries two SHAs.
    AssignmentShaMismatch,
}

impl Denied {
    pub fn http_status(self) -> u16 {
        match self {
            Denied::StaleAssignment | Denied::ContentConflict | Denied::AssignmentShaMismatch => {
                409
            }
            _ => 403,
        }
    }
}

/// What the coordinator does with an authorized claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimDisposition {
    /// 202: claim for the current assignment/generation; may feed the current judgment.
    Current { target_sha: String },
    /// 202: a late claim from a legitimately issued older assignment or generation.
    /// Stored attributed to that assignment and its SHA, never used for the
    /// current judgment and never reactivating the old assignment (§1.9).
    PreservedLate {
        assignment_id: String,
        generation: u64,
        target_sha: String,
    },
    /// 200: the same event ID with identical content is already stored. The stored
    /// record is returned unchanged; no state change, whatever the assignment's status now.
    Replay,
}

impl ClaimDisposition {
    pub fn http_status(&self) -> u16 {
        match self {
            ClaimDisposition::Replay => 200,
            _ => 202,
        }
    }
    /// Whether this claim may be an input to the current judgment.
    pub fn counts_for_current_judgment(&self) -> bool {
        matches!(self, ClaimDisposition::Current { .. })
    }
}

/// `POST /v0/agent/reports`. Runs on the parsed body before schema validation.
/// `stored` is the record already stored under the same `event_id` (and `event_revision`), if any.
pub fn authorize_claim(
    r: &Value,
    who: &Principal,
    book: &AssignmentBook,
    stored: Option<&Value>,
) -> Result<ClaimDisposition, Denied> {
    let Principal::Agent { repo_id, role } = who else {
        return Err(Denied::RoleMismatch);
    };
    if r["repo_id"].as_str() != Some(repo_id.as_str()) {
        return Err(Denied::RepoScopeDenied);
    }
    if r["channel"] != "agent_to_coordinator" || r["evidence_level"] != "claim" {
        return Err(Denied::VerifiedFromAgentDenied);
    }
    let allowed = match role.as_str() {
        "development" => "development_progress",
        "guardrail" => "guardrail_review",
        _ => return Err(Denied::RoleMismatch),
    };
    if r["actor"]["role"].as_str() != Some(role.as_str()) || r["report_type"] != allowed {
        return Err(Denied::RoleMismatch);
    }
    // Same-ID replay is answered from storage before the assignment check, so a
    // retry after the assignment moved on is a re-query, not a new stale claim.
    if let Some(s) = stored {
        return if s == r {
            Ok(ClaimDisposition::Replay)
        } else {
            Err(Denied::ContentConflict)
        };
    }
    let found = book.lookup(r);
    // A body SHA, when present, must be the assignment's recorded SHA (current or late alike).
    if let (Some((a, _)), Some(body_sha)) = (found, r["body"]["target_sha"].as_str())
        && body_sha != a.target_sha
    {
        return Err(Denied::AssignmentShaMismatch);
    }
    match found {
        Some((a, true)) => Ok(ClaimDisposition::Current {
            target_sha: a.target_sha.clone(),
        }),
        Some((a, false)) => Ok(ClaimDisposition::PreservedLate {
            assignment_id: a.assignment_id.clone(),
            generation: a.generation,
            target_sha: a.target_sha.clone(),
        }),
        None => Err(Denied::StaleAssignment),
    }
}

/// What the coordinator does with an authorized repro request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReproDisposition {
    /// 202: new request for the current assignment; queued for the isolated runner.
    Queue { target_sha: String },
    /// 200: same request ID with identical content already exists; return its stored
    /// status/result. Never re-queues, even if the assignment is no longer current.
    Replay,
}

/// `POST /v0/agent/repro-requests`: Guardrail of that repo. A new request must be for
/// the current assignment (a stale one would start new work); an identical re-submission
/// of an existing request is an idempotent re-query.
pub fn authorize_repro(
    req: &Value,
    who: &Principal,
    book: &AssignmentBook,
    stored: Option<&Value>,
) -> Result<ReproDisposition, Denied> {
    let Principal::Agent { repo_id, role } = who else {
        return Err(Denied::RoleMismatch);
    };
    if role != "guardrail" {
        return Err(Denied::RoleMismatch);
    }
    if req["repo_id"].as_str() != Some(repo_id.as_str()) {
        return Err(Denied::RepoScopeDenied);
    }
    if let Some(s) = stored {
        return if s == req {
            Ok(ReproDisposition::Replay)
        } else {
            Err(Denied::ContentConflict)
        };
    }
    match book.lookup(req) {
        // the runner checks out the assignment's SHA from the coordinator DB; a body naming
        // another SHA would be new work outside the current assignment
        Some((a, true)) if req["target_sha"].as_str() == Some(a.target_sha.as_str()) => {
            Ok(ReproDisposition::Queue {
                target_sha: a.target_sha.clone(),
            })
        }
        _ => Err(Denied::StaleAssignment),
    }
}

/// `GET /v0/agent/repro-requests/{id}`: only the same repo's Guardrail.
pub fn may_read_repro(who: &Principal, request_repo: &str) -> Result<(), Denied> {
    match who {
        Principal::Agent { repo_id, role } if role == "guardrail" && repo_id == request_repo => {
            Ok(())
        }
        Principal::Agent { role, .. } if role != "guardrail" => Err(Denied::RoleMismatch),
        _ => Err(Denied::RepoScopeDenied),
    }
}

/// `GET /v0/sprints/...`: clients with that repo in scope; never agent credentials.
pub fn may_read_sprint(who: &Principal, repo: &str) -> Result<(), Denied> {
    match who {
        Principal::Agent { .. } => Err(Denied::AgentReadDenied),
        Principal::Client { repos, .. } if repos.contains(repo) => Ok(()),
        Principal::Client { .. } => Err(Denied::RepoScopeDenied),
    }
}

// ---------------------------------------------------------------------------
// Human rendering
// ---------------------------------------------------------------------------

fn list(v: &Value) -> String {
    let items: Vec<&str> = v
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if items.is_empty() {
        "없음".into()
    } else {
        items.join(", ")
    }
}

fn or_none(v: &Value) -> String {
    v.as_str()
        .map_or_else(|| "없음".to_string(), str::to_string)
}

/// Five-part human report (design log 「사람이 읽고 결정하는 운영 보고」).
/// The headline always states the evidence level; an agent claim of `완료` is
/// shown as "완료 주장(판정 아님)".
pub fn render_human(r: &Value) -> String {
    let decision = r["decision"]["required"] == true;
    let claim = r["evidence_level"] == "claim";
    let kind = if decision {
        "결정 필요"
    } else {
        "상태 변경"
    };
    let level = if claim {
        "claim · 판정 아님"
    } else {
        "verified 판정"
    };
    let task = r["task_id"].as_str().unwrap_or("스프린트 전체");
    let op = r["operational_state"].as_str().unwrap_or_default();
    let op = if claim && op == "완료" {
        "완료 주장(판정 아님)"
    } else {
        op
    };
    let mut s = format!(
        "**[{level}] {kind} · {} {} · {}/{} · 사용자 결정 {}**\n\n",
        r["repo_id"].as_str().unwrap_or_default(),
        task,
        r["phase"].as_str().unwrap_or_default(),
        op,
        if decision { "필요" } else { "불필요" },
    );
    let b = &r["body"];
    // 2. facts and impact
    s += "확인한 사실과 영향\n";
    s += &format!(
        "- 작성 {}({}), 관측 {}\n",
        r["actor"]["role"].as_str().unwrap_or_default(),
        r["evidence_level"].as_str().unwrap_or_default(),
        r["observed_at"].as_str().unwrap_or_default()
    );
    match r["report_type"].as_str() {
        Some("coordinator_verdict") => {
            s += &format!("- 증거 대조 결과: {}\n", or_none(&b["evidence_comparison"]));
        }
        Some("guardrail_review") => {
            let n = b["findings"].as_array().map_or(0, Vec::len);
            s += &format!(
                "- 검토 의견(claim): {} · 발견 {n}건 · 대상 SHA {}\n",
                or_none(&b["verdict"]),
                or_none(&b["target_sha"])
            );
        }
        Some("development_progress") => {
            s += &format!("- 개발 보고(claim): {}\n", or_none(&b["change_summary"]));
            if let Some(why) = b["blocked_reason"].as_str() {
                s += &format!("- 막힌 이유: {why}\n");
            }
        }
        _ => {}
    }
    if let Some(reason) = r["reason_code"].as_str() {
        s += &format!("- 대기 사유: {reason}\n");
    }
    s += &format!("- 아직 모르는 사실: {}\n", list(&r["unknown_facts"]));
    s += &format!("- 영향받는 작업: {}\n", list(&r["affected_tasks"]));
    let sc = &r["schedule"];
    s += &format!(
        "- 일정: 기한 {} ({}), 상태 {}, 남은 작업 {}\n",
        sc["deadline_state"].as_str().unwrap_or_default(),
        or_none(&sc["deadline_at"]),
        sc["status"].as_str().unwrap_or_default(),
        sc["remaining_tasks"].as_u64().map_or_else(
            || format!("미산정({})", or_none(&sc["remaining_reason"])),
            |n| n.to_string()
        )
    );
    if sc["delay_reasons"]
        .as_array()
        .is_some_and(|a| !a.is_empty())
    {
        s += &format!(
            "- 지연 사유: {} · 지연 영향 작업: {}\n",
            list(&sc["delay_reasons"]),
            list(&sc["affected_tasks"])
        );
    }
    match sc["forecast"]["state"].as_str() {
        Some("estimated") => {
            s += &format!(
                "- 예상 완료 범위: {} ~ {}\n",
                or_none(&sc["forecast"]["completion_range"]["earliest_at"]),
                or_none(&sc["forecast"]["completion_range"]["latest_at"])
            )
        }
        _ => {
            s += &format!(
                "- 예상 완료: 미산정({})\n",
                or_none(&sc["forecast"]["reason"])
            )
        }
    }
    // 3. next
    s += "\n다음 행동\n";
    s += &format!(
        "- {} — 담당 {}\n",
        r["next_action"].as_str().unwrap_or_default(),
        r["next_owner"].as_str().unwrap_or_default()
    );
    s += &format!("- 재개 조건: {}\n", or_none(&r["resume_condition"]));
    s += &format!(
        "- 다음 확인 시각(해결 예상 시각 아님): {}\n",
        or_none(&r["next_check_at"])
    );
    // 4. decision
    s += "\n결정 요청\n";
    if decision {
        let d = &r["decision"];
        s += &format!("- 질문: {}\n", or_none(&d["question"]));
        for o in d["options"].as_array().into_iter().flatten() {
            s += &format!("  - {}\n", o.as_str().unwrap_or_default());
        }
        s += &format!("- 권고: {}", or_none(&d["recommendation"]));
        if let Some(why) = b["rationale"].as_str() {
            s += &format!(" — {why}");
        }
        s += "\n";
        match d["deadline_at"].as_str() {
            Some(t) => s += &format!("- 결정 시한: {t}\n"),
            None => {
                s += &format!(
                    "- 결정 시한: 없음({})\n",
                    b["no_deadline_reason"].as_str().unwrap_or("이유 미기재")
                )
            }
        }
        s += &format!(
            "- 시한이 지나거나 응답이 없으면 적용할 기본 조치: {}\n",
            d["default_action"].as_str().unwrap_or_default()
        );
    } else {
        s += "- 없음\n";
    }
    // 5. evidence
    s += "\n확인 근거\n";
    s += &format!(
        "- 사건 {} r{} · 작성 {}({}) · 스프린트 {} r{} · 배정 {} 세대 {}\n",
        r["event_id"].as_str().unwrap_or_default(),
        r["event_revision"],
        r["actor"]["role"].as_str().unwrap_or_default(),
        r["actor"]["name"].as_str().unwrap_or_default(),
        r["sprint_id"].as_str().unwrap_or_default(),
        r["sprint_revision"],
        or_none(&r["assignment_id"]),
        r["generation"]
    );
    if let Some(sha) = b["target_sha"].as_str() {
        s += &format!("- 대상 SHA: {sha}\n");
    }
    s += &format!("- 증거: {}\n", list(&r["evidence"]));
    s
}

// ---------------------------------------------------------------------------
// Re-notification
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Notification {
    /// Send to the user (new event, state change, first decision, or material change).
    Notify(Vec<&'static str>),
    /// Store the observation only; no new notification.
    Suppress,
}

fn at<'a>(v: &'a Value, path: &[&str]) -> &'a Value {
    path.iter().fold(v, |v, k| &v[*k])
}

/// Lists whose order carries no meaning are compared as sets.
fn same_set(a: &Value, b: &Value) -> bool {
    let set = |v: &Value| -> BTreeSet<String> {
        v.as_array()
            .into_iter()
            .flatten()
            .map(|x| x.to_string())
            .collect()
    };
    set(a) == set(b)
}

/// Re-notification rule. `prev` is the last *notified* revision of the same event.
/// Only `observed_at`, `next_check_at`, `event_revision`, list order and the free-text
/// `next_action` wording can change without a notification.
pub fn notification(prev: Option<&Value>, next: &Value) -> Notification {
    let Some(p) = prev else {
        return Notification::Notify(vec!["new_event"]);
    };
    let mut why: Vec<&'static str> = Vec::new();
    let mut add = |label: &'static str| {
        if !why.contains(&label) {
            why.push(label);
        }
    };
    if p["decision"]["required"] != true && next["decision"]["required"] == true {
        add("decision_newly_required");
    }
    if p["decision"]["required"] == true && next["decision"]["required"] != true {
        add("decision_withdrawn");
    }
    let scalar: [(&'static str, &[&str]); 20] = [
        ("phase", &["phase"]),
        ("operational_state", &["operational_state"]),
        ("reason_code", &["reason_code"]),
        ("next_owner", &["next_owner"]),
        ("resume_condition", &["resume_condition"]),
        ("decision", &["decision", "question"]),
        ("decision", &["decision", "options"]),
        ("decision", &["decision", "recommendation"]),
        ("decision", &["decision", "deadline_at"]),
        ("decision", &["decision", "default_action"]),
        ("schedule", &["schedule", "status"]),
        ("schedule", &["schedule", "deadline_state"]),
        ("schedule", &["schedule", "deadline_at"]),
        ("schedule", &["schedule", "remaining_tasks"]),
        ("schedule", &["schedule", "forecast"]),
        ("body_verdict", &["body", "verdict"]),
        ("body_verdict", &["body", "target_sha"]),
        ("body_verdict", &["body", "evidence_comparison"]),
        ("body_verdict", &["body", "findings"]),
        ("body_verdict", &["body", "blocked_reason"]),
    ];
    for (label, path) in scalar {
        if at(p, path) != at(next, path) {
            add(label);
        }
    }
    let sets: [(&'static str, &[&str]); 5] = [
        ("new_evidence", &["evidence"]),
        ("impact_scope", &["affected_tasks"]),
        ("unknown_facts", &["unknown_facts"]),
        ("schedule", &["schedule", "delay_reasons"]),
        ("schedule", &["schedule", "affected_tasks"]),
    ];
    for (label, path) in sets {
        if !same_set(at(p, path), at(next, path)) {
            add(label);
        }
    }
    if why.is_empty() {
        Notification::Suppress
    } else {
        Notification::Notify(why)
    }
}

// ---------------------------------------------------------------------------
// Completion wording
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ReviewCurrency {
    /// An independent approve exists for exactly the current head SHA.
    ApprovedAtCurrentSha,
    /// No approve for the current head (older-SHA approvals do not count).
    ReviewNeededOnCurrentSha { stale_approvals: Vec<String> },
}

/// `reviews` = (target_sha, verdict) from guardrail reports.
pub fn review_currency(current_head: &str, reviews: &[(String, String)]) -> ReviewCurrency {
    if reviews
        .iter()
        .any(|(sha, v)| sha == current_head && v == "approve")
    {
        return ReviewCurrency::ApprovedAtCurrentSha;
    }
    let stale = reviews
        .iter()
        .filter(|(sha, v)| sha != current_head && v == "approve")
        .map(|(s, _)| s.clone())
        .collect();
    ReviewCurrency::ReviewNeededOnCurrentSha {
        stale_approvals: stale,
    }
}

/// Facts the coordinator verified for one task, each a separate completion scope (§1.5).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompletionFacts {
    pub head_sha: String,
    pub ci_passed_on_head: bool,
    pub review: Option<ReviewCurrency>,
    pub merged_sha: Option<String>,
    pub main_ci_passed: bool,
    pub required_outputs_present: bool,
    pub followups_accepted: bool,
}

/// Completion wording: lists what is done and the first scope that is not.
/// Never says "완료" for the whole task unless every scope holds.
pub fn completion_summary(f: &CompletionFacts) -> String {
    let short = |s: &str| s.chars().take(7).collect::<String>();
    let review_ok = matches!(f.review, Some(ReviewCurrency::ApprovedAtCurrentSha));
    let scopes = [
        (
            format!("현재 SHA {} CI 통과", short(&f.head_sha)),
            f.ci_passed_on_head,
        ),
        (
            format!("현재 SHA {} 독립 검토 승인", short(&f.head_sha)),
            review_ok,
        ),
        (
            format!(
                "병합{}",
                f.merged_sha
                    .as_deref()
                    .map(|m| format!(" {}", short(m)))
                    .unwrap_or_default()
            ),
            f.merged_sha.is_some(),
        ),
        ("병합 SHA의 main CI 통과".to_string(), f.main_ci_passed),
        ("필수 산출물 확인".to_string(), f.required_outputs_present),
        ("후속 작업 인수".to_string(), f.followups_accepted),
    ];
    let done: Vec<&str> = scopes
        .iter()
        .take_while(|(_, ok)| *ok)
        .map(|(t, _)| t.as_str())
        .collect();
    match scopes.iter().find(|(_, ok)| !*ok) {
        None => format!("작업 완료: {}", done.join(" · ")),
        Some((first_missing, _)) => {
            let mut s = if done.is_empty() {
                "확인된 완료 범위 없음".to_string()
            } else {
                format!("{}까지만 확인", done.join(" · "))
            };
            s += &format!(". 미완료: {first_missing}");
            if let Some(ReviewCurrency::ReviewNeededOnCurrentSha { stale_approvals }) = &f.review
                && !stale_approvals.is_empty()
            {
                s += &format!(
                    " (이전 SHA 승인 {}은 현재 판정에 쓰지 않음)",
                    stale_approvals
                        .iter()
                        .map(|x| short(x))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            s
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_ts;

    #[test]
    fn timestamps() {
        let ok = |s: &str| parse_ts(s).is_some();
        assert!(ok("2026-11-12T09:00:00+09:00"));
        assert!(ok("2026-11-12t00:00:00.5z"));
        assert!(ok("2028-02-29T00:00:00Z"));
        assert!(!ok("2026-02-29T00:00:00Z"));
        assert!(!ok("2026-11-12T09:00:00"), "offset required");
        assert!(!ok("2026-11-12T24:00:00Z"));
        assert!(!ok("2026-11-12T23:59:60Z"), "no leap second in v0");
        assert!(!ok("2026-11-12T09:00:00+-1:00"), "signed offset hour");
        assert!(!ok("2026-11-12T09:00:00+05:60"));
        assert!(!ok("0000-01-01T00:00:00Z"));
        assert!(!ok("2026-11-12T09:00:00.Z"));
        assert!(!ok("not-a-date"));
        assert_eq!(
            parse_ts("2026-11-12T09:00:00+09:00"),
            parse_ts("2026-11-12T00:00:00Z")
        );
        assert!(parse_ts("2026-11-12T00:00:00.2Z") > parse_ts("2026-11-12T00:00:00.1Z"));
        assert_eq!(parse_ts("1970-01-01T00:00:00Z"), Some(0));
    }
}
