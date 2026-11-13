//! X-01: read one Notion task, derive a HumanSprintInput v0 and the import request for the
//! coordinator (`POST /v0/imports/notion`).
//!
//! Two sources produce the same [`NotionPage`]:
//! * a snapshot of a page fetched through a Notion connector ([`from_connector_fetch`]);
//! * the Notion REST API with an integration token ([`api::fetch_page`]).
//!
//! Nothing here writes to Notion. A page that does not state everything a sprint needs, or
//! whose queue gate is not released, is still registered — as a draft with the reasons in
//! `unresolved` — and never as executable.

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub mod api;

/// One Notion page, normalised: properties as plain strings, body as Notion-flavoured text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotionPage {
    pub page_id: String,
    pub url: String,
    pub last_edited_time: String,
    pub properties: BTreeMap<String, String>,
    pub content: String,
    /// How it was read (`notion-connector-snapshot`, `notion-rest-api`).
    pub fetched_via: String,
}

fn plain(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().map(plain).collect::<Vec<_>>().join("\n"),
        other => other.to_string(),
    }
}

/// Undo the connector's Markdown escapes (`\[`, `\_`, …).
pub fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\\'
            && let Some(&n) = it.peek()
            && "[]_*`#<>()".contains(n)
        {
            out.push(n);
            it.next();
            continue;
        }
        out.push(c);
    }
    out
}

/// Page id (32 hex) from a Notion URL or a dashed UUID.
pub fn page_id_from(s: &str) -> Option<String> {
    let tail = s.split(['?', '#']).next()?;
    let hex: String = tail
        .rsplit(['/', '-'])
        .next()
        .map(str::to_string)
        .filter(|t| t.len() == 32)
        .unwrap_or_else(|| tail.chars().filter(char::is_ascii_hexdigit).collect());
    let hex = if hex.len() >= 32 {
        hex[hex.len() - 32..].to_string()
    } else {
        hex
    };
    (hex.len() == 32 && hex.chars().all(|c| c.is_ascii_hexdigit())).then(|| hex.to_lowercase())
}

/// A connector `fetch` result: `{"text": "...<page url=…><properties>{…}</properties>…<content>…</content>",
/// "page_last_edited_at": "…", "url": "…"}`.
pub fn from_connector_fetch(v: &Value) -> Result<NotionPage, String> {
    let text = v["text"].as_str().ok_or("connector result has no text")?;
    let between = |a: &str, b: &str| -> Option<&str> {
        let i = text.find(a)? + a.len();
        let j = text[i..].find(b)? + i;
        Some(&text[i..j])
    };
    let props_raw = between("<properties>", "</properties>").ok_or("no <properties> block")?;
    let props: Value =
        serde_json::from_str(props_raw.trim()).map_err(|e| format!("properties JSON: {e}"))?;
    let properties = props
        .as_object()
        .ok_or("properties is not an object")?
        .iter()
        .map(|(k, v)| (k.clone(), plain(v)))
        .collect();
    let content = between("<content>", "</content>")
        .ok_or("no <content> block")?
        .trim()
        .to_string();
    let url = between("<page url=\"", "\"")
        .map(str::to_string)
        .or_else(|| v["url"].as_str().map(str::to_string))
        .ok_or("no page url")?;
    let page_id = page_id_from(&url).ok_or("no page id in url")?;
    let last_edited_time = v["page_last_edited_at"]
        .as_str()
        .ok_or("no page_last_edited_at")?
        .to_string();
    Ok(NotionPage {
        page_id,
        url,
        last_edited_time,
        properties,
        content,
        fetched_via: "notion-connector-snapshot".into(),
    })
}

/// SHA-256 over the page's properties and body (canonical JSON, sorted keys). Any edit in
/// Notion changes it; the read path and time do not.
pub fn content_digest(p: &NotionPage) -> String {
    let canon = json!({"properties": p.properties, "content": p.content}).to_string();
    hex::encode(Sha256::digest(canon.as_bytes()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Criterion {
    pub id: String,
    pub input: String,
    pub expected: String,
    pub evidence: String,
}

/// What the importer could read from the page, and what it could not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extracted {
    pub task_id: Option<String>,
    pub title: Option<String>,
    pub repo: Option<String>,
    pub goal: Option<String>,
    pub criteria: Vec<Criterion>,
    pub prerequisites: Option<String>,
    /// Reasons the draft is not executable (missing facts, unreleased queue gates).
    pub unresolved: Vec<String>,
}

fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    unescape(&out.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// Text of the section that starts with `## {heading}` (up to the next `## `).
fn section<'a>(content: &'a str, heading: &str) -> Option<&'a str> {
    let start = content.find(&format!("## {heading}"))?;
    let body = &content[start..];
    let body = &body[body.find('\n').map_or(body.len(), |i| i + 1)..];
    Some(match body.find("\n## ") {
        Some(end) => &body[..end],
        None => body,
    })
}

fn table_rows(s: &str) -> Vec<Vec<String>> {
    s.split("<tr>")
        .skip(1)
        .map(|row| {
            let row = row.split("</tr>").next().unwrap_or_default();
            row.split("<td>")
                .skip(1)
                .map(|c| strip_tags(c.split("</td>").next().unwrap_or_default()))
                .collect()
        })
        .collect()
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}

/// Read the task. Missing facts become `unresolved` reasons, never invented values.
pub fn extract(p: &NotionPage) -> Extracted {
    let prop = |k: &str| {
        p.properties
            .get(k)
            .map(|v| unescape(v.trim()))
            .filter(|v| !v.is_empty())
    };
    let mut unresolved = Vec::new();
    let task_id = prop("작업 ID");
    let title = prop("제목");
    let repo = prop("저장소 식별자")
        .and_then(|r| r.split_whitespace().next().map(str::to_string))
        .filter(|r| {
            let parts: Vec<&str> = r.split('/').collect();
            parts.len() == 2
                && parts.iter().all(|x| {
                    !x.is_empty()
                        && x.chars()
                            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
                })
        });
    let goal = section(&p.content, "목표")
        .and_then(|s| {
            s.lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .map(strip_tags)
        })
        .or_else(|| prop("내용 요약"));
    let mut criteria = Vec::new();
    if let Some(sec) = section(&p.content, "필수 인수 판정") {
        for row in table_rows(sec).into_iter().skip(1) {
            if row.len() >= 3 && !row[0].is_empty() {
                let (id, input) = match row[0].split_once(':') {
                    Some((a, b)) => (a.trim().to_string(), b.trim().to_string()),
                    None => (row[0].clone(), row[0].clone()),
                };
                criteria.push(Criterion {
                    id,
                    input,
                    expected: row[1].clone(),
                    evidence: row[2].clone(),
                });
            }
        }
    }
    let prerequisites = section(&p.content, "선행").map(|s| truncate(&strip_tags(s), 600));
    for (k, v) in [("작업 ID", &task_id), ("제목", &title)] {
        if v.is_none() {
            unresolved.push(format!("Notion 필수 속성 '{k}' 없음"));
        }
    }
    if goal.is_none() {
        unresolved.push("목표를 원본에서 읽지 못함".into());
    }
    if criteria.is_empty() {
        unresolved.push("수용 기준(필수 인수 판정 표)을 원본에서 읽지 못함".into());
    }
    match prop("큐 구획") {
        Some(q) if q == "현재" => {}
        Some(q) => unresolved.push(format!("Notion 큐 구획 '{q}': 현재 실행 대상이 아님")),
        None => unresolved.push("Notion 큐 구획 없음".into()),
    }
    if let Some(s) = prop("작업 상태")
        && s != "실행재개"
    {
        unresolved.push(format!("Notion 작업 상태 '{s}': 실행 재개가 아님"));
    }
    if let Some(b) = prop("차단 대상") {
        unresolved.push(format!("Notion 차단 대상 미해제: {}", truncate(&b, 400)));
    }
    Extracted {
        task_id,
        title,
        repo,
        goal,
        criteria,
        prerequisites,
        unresolved,
    }
}

/// HumanSprintInput v0 derived from the page (no invented deadline, estimates or outputs).
pub fn human_input(e: &Extracted) -> Option<Value> {
    let repo = e.repo.as_ref()?;
    let title = e
        .title
        .clone()
        .or_else(|| e.task_id.clone())
        .unwrap_or_else(|| "(제목 없음)".into());
    let goal = e
        .goal
        .clone()
        .unwrap_or_else(|| "(원본에 목표 없음)".into());
    let tasks: Vec<Value> = if e.criteria.is_empty() {
        vec![
            json!({"name": e.task_id.clone().unwrap_or_else(|| "원본 작업".into()),
                    "output": "(원본에서 산출물을 읽지 못함)",
                    "done_when": "(원본에서 수용 기준을 읽지 못함)",
                    "how_to_check": "(원본에서 확인 방법을 읽지 못함)"}),
        ]
    } else {
        e.criteria
            .iter()
            .map(|c| {
                json!({"name": format!("{} — {}", c.id, truncate(&c.input, 80)),
                       "output": c.evidence, "done_when": c.expected,
                       "how_to_check": format!("{}: {}", c.id, c.input)})
            })
            .collect()
    };
    Some(json!({
        "title": title,
        "repo": repo,
        "goal": goal,
        "out_of_scope": [],
        "decisions": [],
        "deadline": {"state": "undecided", "at": null,
                     "reason": "Notion 원본에 기한 속성이 없음(조건부 기준 일정은 달력 납기가 아님)"},
        "tasks": tasks
    }))
}

/// Body for `POST /v0/imports/notion`, or why it cannot be built (no repo → no binding).
pub fn import_request(p: &NotionPage, test_id: &str, observed_at: &str) -> Result<Value, String> {
    let e = extract(p);
    let input = human_input(&e).ok_or_else(|| {
        "the page names no target repo (`저장소 식별자` owner/name); nothing is registered"
            .to_string()
    })?;
    Ok(json!({
        "test_id": test_id,
        "source": {"system": "notion", "page_id": p.page_id, "url": p.url,
                   "last_edited_time": p.last_edited_time, "content_digest": content_digest(p),
                   "source_task_id": e.task_id.clone().unwrap_or_else(|| "(작업 ID 없음)".into()),
                   "fetched_via": p.fetched_via, "observed_at": observed_at},
        "input": input,
        "unresolved": e.unresolved,
    }))
}

/// Snapshot file format (kept outside the public repository).
pub fn to_snapshot(p: &NotionPage) -> Value {
    json!({"format": "ieo-notion-snapshot/v0", "page_id": p.page_id, "url": p.url,
           "last_edited_time": p.last_edited_time, "properties": p.properties,
           "content": p.content, "fetched_via": p.fetched_via,
           "content_digest": content_digest(p)})
}

pub fn from_snapshot(v: &Value) -> Result<NotionPage, String> {
    if v["format"] != "ieo-notion-snapshot/v0" {
        return Err("not an ieo-notion-snapshot/v0 file".into());
    }
    let s = |k: &str| {
        v[k].as_str()
            .map(str::to_string)
            .ok_or(format!("snapshot.{k} missing"))
    };
    let p = NotionPage {
        page_id: s("page_id")?,
        url: s("url")?,
        last_edited_time: s("last_edited_time")?,
        properties: v["properties"]
            .as_object()
            .ok_or("snapshot.properties missing")?
            .iter()
            .map(|(k, v)| (k.clone(), plain(v)))
            .collect(),
        content: s("content")?,
        fetched_via: s("fetched_via")?,
    };
    if v["content_digest"].as_str() != Some(content_digest(&p).as_str()) {
        return Err("snapshot content_digest does not match its content".into());
    }
    Ok(p)
}
