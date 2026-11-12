//! Progress report v0 (task 01-08) — reference implementation of the parts of
//! the contract that are decidable without a server:
//!
//! * [`validate_report`]: JSON Schema + the semantic rules the schema cannot express
//!   (same rules as `docs/poc/scripts/validate_contracts.py`);
//! * [`authorize_claim`]: what an agent credential may submit to `POST /v0/agent/reports`;
//! * [`render_human`]: the five-part human report (summary → facts/impact → next → decision → evidence);
//! * [`notification`]: re-notify only on a real change, never on a bare re-observation;
//! * [`review_currency`]: completion wording keeps "CI passed" and "approved at current SHA" apart.

use ieo_spec::schema::format_rejections;
use ieo_spec::{Code, Rejection};
use jsonschema::Validator;
use serde::Serialize;
use serde_json::Value;
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

/// RFC 3339 with an explicit offset; seconds 00–59 (no leap second in v0).
pub fn parse_ts(s: &str) -> Option<(i64, i64)> {
    // Returns (seconds since an arbitrary epoch in UTC, offset seconds) for ordering only.
    let b = s.as_bytes();
    if b.len() < 20 {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<i64> {
        let t = s.get(r)?;
        if t.bytes().all(|c| c.is_ascii_digit()) {
            t.parse().ok()
        } else {
            None
        }
    };
    let (y, mo, d, h, mi, se) = (
        num(0..4)?,
        num(5..7)?,
        num(8..10)?,
        num(11..13)?,
        num(14..16)?,
        num(17..19)?,
    );
    if b[4] != b'-'
        || b[7] != b'-'
        || !matches!(b[10], b'T' | b't')
        || b[13] != b':'
        || b[16] != b':'
    {
        return None;
    }
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
    if !(1..=12).contains(&mo)
        || d < 1
        || d > mdays[(mo - 1) as usize]
        || h > 23
        || mi > 59
        || se > 59
    {
        return None;
    }
    let mut rest = &s[19..];
    if let Some(r) = rest.strip_prefix('.') {
        let n = r.bytes().take_while(u8::is_ascii_digit).count();
        if n == 0 {
            return None;
        }
        rest = &r[n..];
    }
    let off = match rest {
        "Z" | "z" => 0,
        _ if rest.len() == 6
            && matches!(rest.as_bytes()[0], b'+' | b'-')
            && rest.as_bytes()[3] == b':' =>
        {
            let oh: i64 = rest.get(1..3)?.parse().ok()?;
            let om: i64 = rest.get(4..6)?.parse().ok()?;
            if oh > 23 || om > 59 {
                return None;
            }
            let v = oh * 3600 + om * 60;
            if rest.starts_with('-') { -v } else { v }
        }
        _ => return None,
    };
    // days from civil (Howard Hinnant)
    let (yy, mm) = if mo <= 2 {
        (y - 1, mo + 9)
    } else {
        (y, mo - 3)
    };
    let era = yy.div_euclid(400);
    let yoe = yy - era * 400;
    let doy = (153 * mm + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe;
    Some((days * 86_400 + h * 3600 + mi * 60 + se - off, off))
}

/// Schema + semantic checks. Empty = valid.
pub fn validate_report(r: &Value) -> Vec<Rejection> {
    let mut out = format_rejections(validator(), r);
    if !out.is_empty() {
        return out;
    }
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
    if f["state"] == "estimated" {
        let e = f["completion_range"]["earliest_at"]
            .as_str()
            .and_then(parse_ts);
        let l = f["completion_range"]["latest_at"]
            .as_str()
            .and_then(parse_ts);
        if let (Some(e), Some(l)) = (e, l)
            && e.0 > l.0
        {
            out.push(semantic(
                "schedule.forecast.completion_range",
                "예상 완료 범위의 앞뒤가 바뀌었습니다.",
                "earliest_at ≤ latest_at",
            ));
        }
    }
    if s["deadline_state"] == "set" && s["status"] == "late" {
        let obs = r["observed_at"].as_str().and_then(parse_ts);
        let dl = s["deadline_at"].as_str().and_then(parse_ts);
        if let (Some(o), Some(d)) = (obs, dl)
            && o.0 <= d.0
        {
            out.push(semantic(
                "schedule.status",
                "기한 전인데 지연(late)으로 표시했습니다.",
                "기한 전이면 at_risk 또는 on_track",
            ));
        }
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

/// Server-side identity of the caller of `POST /v0/agent/reports`.
#[derive(Debug, Clone)]
pub struct AgentCredential {
    pub repo_id: String,
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ClaimDenied {
    RepoScopeDenied,
    RoleMismatch,
    VerifiedFromAgentDenied,
}

/// 403 decision for an agent submission (run after [`validate_report`]). Names in the body are not trusted.
pub fn authorize_claim(r: &Value, cred: &AgentCredential) -> Result<(), ClaimDenied> {
    if r["repo_id"].as_str() != Some(cred.repo_id.as_str()) {
        return Err(ClaimDenied::RepoScopeDenied);
    }
    if r["channel"] != "agent_to_coordinator" || r["evidence_level"] != "claim" {
        return Err(ClaimDenied::VerifiedFromAgentDenied);
    }
    let allowed = match cred.role.as_str() {
        "development" => "development_progress",
        "guardrail" => "guardrail_review",
        _ => return Err(ClaimDenied::RoleMismatch),
    };
    if r["actor"]["role"].as_str() != Some(cred.role.as_str()) || r["report_type"] != allowed {
        return Err(ClaimDenied::RoleMismatch);
    }
    Ok(())
}

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

/// Five-part human report (design log “사람이 읽고 결정하는 운영 보고”).
pub fn render_human(r: &Value) -> String {
    let decision = r["decision"]["required"] == true;
    let kind = if decision {
        "결정 필요"
    } else {
        "상태 변경"
    };
    let task = r["task_id"].as_str().unwrap_or("스프린트 전체");
    let mut s = format!(
        "**{kind} · {} {} · {}/{} · 사용자 결정 {}**\n\n",
        r["repo_id"].as_str().unwrap_or_default(),
        task,
        r["phase"].as_str().unwrap_or_default(),
        r["operational_state"].as_str().unwrap_or_default(),
        if decision { "필요" } else { "불필요" },
    );
    // 2. facts and impact
    s += "확인한 사실과 영향\n";
    s += &format!(
        "- 근거 수준: {} ({}), 관측 {}\n",
        r["evidence_level"].as_str().unwrap_or_default(),
        r["channel"].as_str().unwrap_or_default(),
        r["observed_at"].as_str().unwrap_or_default()
    );
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
        if let Some(why) = r["body"]["rationale"].as_str() {
            s += &format!(" — {why}");
        }
        s += "\n";
        match d["deadline_at"].as_str() {
            Some(t) => s += &format!("- 결정 시한: {t}\n"),
            None => {
                s += &format!(
                    "- 결정 시한: 없음({})\n",
                    r["body"]["no_deadline_reason"]
                        .as_str()
                        .unwrap_or("이유 미기재")
                )
            }
        }
        s += &format!(
            "- 응답이 없을 때 기본 조치: {}\n",
            d["default_action"].as_str().unwrap_or_default()
        );
    } else {
        s += "- 없음\n";
    }
    // 5. evidence
    s += "\n확인 근거\n";
    s += &format!(
        "- 사건 {} r{} · 작성 {}({}) · 스프린트 {} r{}\n",
        r["event_id"].as_str().unwrap_or_default(),
        r["event_revision"],
        r["actor"]["role"].as_str().unwrap_or_default(),
        r["actor"]["name"].as_str().unwrap_or_default(),
        r["sprint_id"].as_str().unwrap_or_default(),
        r["sprint_revision"]
    );
    if let Some(sha) = r["body"]["target_sha"].as_str() {
        s += &format!("- 대상 SHA: {sha}\n");
    }
    s += &format!("- 증거: {}\n", list(&r["evidence"]));
    s
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Notification {
    /// Send to the user (new event, state change, first decision, or material change).
    Notify(Vec<&'static str>),
    /// Store the observation only; no new notification.
    Suppress,
}

/// Re-notification rule. `prev` is the last *notified* revision of the same event.
pub fn notification(prev: Option<&Value>, next: &Value) -> Notification {
    let Some(p) = prev else {
        return Notification::Notify(vec!["new_event"]);
    };
    let mut why = Vec::new();
    let changed =
        |path: &[&str]| path.iter().fold(p, |v, k| &v[*k]) != path.iter().fold(next, |v, k| &v[*k]);
    if p["decision"]["required"] != true && next["decision"]["required"] == true {
        why.push("decision_newly_required");
    }
    for (label, path) in [
        ("phase", &["phase"][..]),
        ("operational_state", &["operational_state"][..]),
        ("new_evidence", &["evidence"][..]),
        ("impact_scope", &["affected_tasks"][..]),
        ("resume_condition", &["resume_condition"][..]),
        ("decision_deadline", &["decision", "deadline_at"][..]),
        (
            "decision_question_or_options",
            &["decision", "question"][..],
        ),
        ("decision_question_or_options", &["decision", "options"][..]),
        ("unknown_facts", &["unknown_facts"][..]),
        ("schedule_status", &["schedule", "status"][..]),
    ] {
        if changed(path) && !why.contains(&label) {
            why.push(label);
        }
    }
    // observed_at / next_check_at / event_revision alone are re-observations.
    if why.is_empty() {
        Notification::Suppress
    } else {
        Notification::Notify(why)
    }
}

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

#[cfg(test)]
mod tests {
    use super::parse_ts;

    #[test]
    fn timestamps() {
        assert!(parse_ts("2026-11-12T09:00:00+09:00").is_some());
        assert!(parse_ts("2026-11-12t00:00:00.5Z").is_some());
        assert!(parse_ts("2026-11-12T09:00:00").is_none(), "offset required");
        assert!(parse_ts("2026-02-30T00:00:00Z").is_none());
        assert!(
            parse_ts("2026-11-12T23:59:60Z").is_none(),
            "no leap second in v0"
        );
        assert!(parse_ts("not-a-date").is_none());
        assert_eq!(
            parse_ts("2026-11-12T09:00:00+09:00").map(|x| x.0),
            parse_ts("2026-11-12T00:00:00Z").map(|x| x.0)
        );
    }
}
