//! X-01 importer tests. Fixtures are synthetic: no real Notion queue text is stored in
//! the public repository.

use ieo_notion::{
    NotionPage, api, content_digest, extract, from_connector_fetch, from_snapshot, import_request,
    page_id_from, to_snapshot,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

const PAGE: &str = "0123456789abcdef0123456789abcdef";

fn body(gated: bool) -> String {
    let gate = if gated {
        "\n## 선행·해제 사건\n\nKEEP\\_GATED. 선행 A1 수용 대기.\n"
    } else {
        ""
    };
    format!(
        "## 목표와 등록 기준\n\n가상 부품의 경계 검사를 제공한다.\n\n## 필수 인수 판정\n\n<table header-row=\"true\">\n<tr>\n<td>\n판정 ID·입력\n</td>\n<td>\n기대\n</td>\n<td>\n증거\n</td>\n</tr>\n<tr>\n<td>\nX-01: 정상 입력\n</td>\n<td>\n수용한다 / 실패: 거절\n</td>\n<td>\n정상 결과\n</td>\n</tr>\n<tr>\n<td>\nX-02: 13/14 \\[경계\\]\n</td>\n<td>\n불일치를 거절\n</td>\n<td>\n변이 결과\n</td>\n</tr>\n</table>\n{gate}"
    )
}

fn page(queue: &str, state: &str, block: &str) -> NotionPage {
    let mut properties = BTreeMap::new();
    properties.insert("작업 ID".into(), "OP-SYNTH-0001".into());
    properties.insert("제목".into(), "\\[합성\\] 가상 부품".into());
    properties.insert("저장소 식별자".into(), "example/widget extra words".into());
    properties.insert("큐 구획".into(), queue.into());
    properties.insert("작업 상태".into(), state.into());
    properties.insert("차단 대상".into(), block.into());
    NotionPage {
        page_id: PAGE.into(),
        url: format!("https://app.notion.com/p/{PAGE}"),
        last_edited_time: "2026-11-01T00:00:00.000Z".into(),
        properties,
        content: body(!block.is_empty()),
        fetched_via: "test".into(),
    }
}

#[test]
fn ready_page_extracts_without_unresolved_reasons() {
    let e = extract(&page("현재", "실행재개", ""));
    assert_eq!(e.task_id.as_deref(), Some("OP-SYNTH-0001"));
    assert_eq!(e.title.as_deref(), Some("[합성] 가상 부품"));
    assert_eq!(e.repo.as_deref(), Some("example/widget"));
    assert_eq!(e.goal.as_deref(), Some("가상 부품의 경계 검사를 제공한다."));
    assert_eq!(e.criteria.len(), 2);
    assert_eq!(e.criteria[1].id, "X-02");
    assert_eq!(e.criteria[1].input, "13/14 [경계]");
    assert!(
        e.unresolved.is_empty() && e.gates.is_empty(),
        "{:?} {:?}",
        e.unresolved,
        e.gates
    );
}

#[test]
fn queue_gates_become_unresolved_reasons() {
    let e = extract(&page("조건 대기", "결정대기", "KEEP_GATED / 선행 A1"));
    // C2: queue gates are recorded apart from missing facts
    assert!(e.unresolved.is_empty(), "{:?}", e.unresolved);
    assert_eq!(e.gates.len(), 3, "{:?}", e.gates);
    assert!(e.gates[0].contains("조건 대기"));
    assert!(e.gates[1].contains("결정대기"));
    assert!(e.gates[2].starts_with("Notion 차단 대상 미해제: KEEP_GATED"));
    let r = import_request(&page("조건 대기", "결정대기", "KEEP_GATED"), "T-1", "x").expect("req");
    assert_eq!(r["gates"].as_array().map(Vec::len), Some(3));
    assert_eq!(r["unresolved"], json!([]));
}

#[test]
fn missing_facts_are_named_not_invented() {
    let mut p = page("현재", "실행재개", "");
    p.content = "본문에 목표와 표가 없다.".into();
    p.properties.remove("작업 ID");
    let e = extract(&p);
    assert!(e.unresolved.iter().any(|u| u.contains("작업 ID")));
    assert!(e.unresolved.iter().any(|u| u.contains("목표")));
    assert!(e.unresolved.iter().any(|u| u.contains("수용 기준")));
    let r = import_request(&p, "T-1", "2026-11-13T00:00:00Z").expect("still a draft");
    assert_eq!(r["input"]["tasks"].as_array().map(Vec::len), Some(1));
    assert!(
        r["input"]["tasks"][0]["done_when"]
            .as_str()
            .unwrap_or_default()
            .contains("읽지 못함")
    );
    // no repo → no binding → nothing to register
    p.properties
        .insert("저장소 식별자".into(), "not a repo".into());
    assert!(import_request(&p, "T-1", "x").is_err());
}

#[test]
fn digest_tracks_content_not_read_path() {
    let a = page("현재", "실행재개", "");
    let mut b = a.clone();
    b.fetched_via = "other".into();
    assert_eq!(content_digest(&a), content_digest(&b));
    b.content.push('.');
    assert_ne!(content_digest(&a), content_digest(&b));
    let mut c = a.clone();
    c.properties.insert("작업 상태".into(), "종료".into());
    assert_ne!(content_digest(&a), content_digest(&c));
}

#[test]
fn snapshot_round_trips_and_detects_tampering() {
    let p = page("현재", "실행재개", "");
    let mut s = to_snapshot(&p);
    assert_eq!(from_snapshot(&s).as_ref(), Ok(&p));
    s["content"] = json!("tampered");
    assert!(from_snapshot(&s).is_err());
}

#[test]
fn connector_fetch_result_is_parsed() {
    let props = json!({"작업 ID": "OP-SYNTH-0001", "제목": "합성", "저장소 식별자": "example/widget",
                       "큐 구획": "현재", "작업 상태": "실행재개", "차단 대상": "", "저장소": ["Widget"]});
    let text = format!(
        "Here is the result\n<page url=\"https://app.notion.com/p/{PAGE}\">\n<properties>\n{props}\n</properties>\n<content>\n{}\n</content>\n</page>",
        body(false)
    );
    let v = json!({"text": text, "page_last_edited_at": "2026-11-01T00:00:00.000Z"});
    let p = from_connector_fetch(&v).expect("parse");
    assert_eq!(p.page_id, PAGE);
    assert_eq!(p.properties["저장소"], "Widget");
    assert_eq!(extract(&p).criteria.len(), 2);
    assert!(from_connector_fetch(&json!({"text": "no blocks"})).is_err());
}

#[test]
fn page_ids_from_urls() {
    assert_eq!(
        page_id_from(&format!("https://app.notion.com/p/{PAGE}?pvs=204")).as_deref(),
        Some(PAGE)
    );
    assert_eq!(
        page_id_from("01234567-89ab-cdef-0123-456789abcdef").as_deref(),
        Some(PAGE)
    );
    assert_eq!(page_id_from("short"), None);
}

// --- REST API reader against a local mock ---

fn mock(status: u16, seen: Arc<Mutex<Vec<String>>>) -> String {
    let server = tiny_http::Server::http("127.0.0.1:0").expect("bind");
    let addr = server
        .server_addr()
        .to_ip()
        .map(|a| a.to_string())
        .expect("addr");
    std::thread::spawn(move || {
        for req in server.incoming_requests() {
            let auth = req
                .headers()
                .iter()
                .find(|h| h.field.equiv("Authorization"))
                .map(|h| h.value.to_string())
                .unwrap_or_default();
            if let Ok(mut s) = seen.lock() {
                s.push(format!("{} {}", req.url(), auth));
            }
            let url = req.url().to_string();
            let body: Value = if url.starts_with(&format!("/v1/pages/{PAGE}")) {
                json!({"url": format!("https://www.notion.so/{PAGE}"), "last_edited_time": "2026-11-02T00:00:00.000Z",
                       "properties": {
                           "작업 ID": {"type": "rich_text", "rich_text": [{"plain_text": "OP-SYNTH-0001"}]},
                           "제목": {"type": "title", "title": [{"plain_text": "합성 "}, {"plain_text": "부품"}]},
                           "저장소 식별자": {"type": "rich_text", "rich_text": [{"plain_text": "example/widget"}]},
                           "큐 구획": {"type": "select", "select": {"name": "현재"}},
                           "작업 상태": {"type": "select", "select": {"name": "실행재개"}},
                           "차단 대상": {"type": "rich_text", "rich_text": []},
                           "외부 전달": {"type": "checkbox", "checkbox": false}}})
            } else if url.starts_with(&format!("/v1/blocks/{PAGE}/children"))
                && !url.contains("start_cursor")
            {
                json!({"results": [
                    {"type": "heading_2", "heading_2": {"rich_text": [{"plain_text": "목표와 등록 기준"}]}},
                    {"type": "paragraph", "paragraph": {"rich_text": [{"plain_text": "가상 부품의 경계 검사를 제공한다."}]}}],
                       "has_more": true, "next_cursor": "c2"})
            } else if url.contains("start_cursor=c2") {
                json!({"results": [
                    {"type": "heading_2", "heading_2": {"rich_text": [{"plain_text": "필수 인수 판정"}]}},
                    {"type": "table", "id": "tbl1", "table": {}}], "has_more": false, "next_cursor": null})
            } else if url.starts_with("/v1/blocks/tbl1/children") {
                let row = |a: &str, b: &str, c: &str| {
                    json!({"type": "table_row", "table_row": {"cells": [
                    [{"plain_text": a}], [{"plain_text": b}], [{"plain_text": c}]]}})
                };
                json!({"results": [row("판정", "기대", "증거"), row("X-01: 정상", "수용", "결과")], "has_more": false})
            } else {
                json!({"object": "error", "code": "object_not_found"})
            };
            let code = if body["object"] == "error" {
                404
            } else {
                status
            };
            let _ = req
                .respond(tiny_http::Response::from_string(body.to_string()).with_status_code(code));
        }
    });
    format!("http://{addr}")
}

#[test]
fn rest_api_reader_produces_the_same_shape() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let base = mock(200, Arc::clone(&seen));
    let p = api::fetch_page(&base, "secret-token-value", PAGE).expect("fetch");
    assert_eq!(p.fetched_via, "notion-rest-api");
    assert_eq!(p.properties["제목"], "합성 부품");
    assert_eq!(p.properties["외부 전달"], "__NO__");
    let e = extract(&p);
    assert_eq!(e.repo.as_deref(), Some("example/widget"));
    assert_eq!(e.goal.as_deref(), Some("가상 부품의 경계 검사를 제공한다."));
    assert_eq!(e.criteria.len(), 1, "{}", p.content);
    assert!(e.unresolved.is_empty(), "{:?}", e.unresolved);
    let calls = seen.lock().map(|s| s.clone()).unwrap_or_default();
    assert!(
        calls
            .iter()
            .all(|c| c.ends_with("Bearer secret-token-value")),
        "{calls:?}"
    );
    assert!(
        calls.iter().any(|c| c.contains("start_cursor=c2")),
        "pagination followed"
    );
}

#[test]
fn notion_failures_are_errors_and_never_leak_the_token() {
    let base = mock(500, Arc::new(Mutex::new(Vec::new())));
    let err = api::fetch_page(&base, "secret-token-value", PAGE).expect_err("500");
    assert!(
        err.contains("HTTP 500") && !err.contains("secret-token-value"),
        "{err}"
    );
    let err = api::fetch_page(
        &base,
        "secret-token-value",
        "ffffffffffffffffffffffffffffffff",
    )
    .expect_err("404");
    assert!(
        err.contains("HTTP 404") && err.contains("object_not_found"),
        "{err}"
    );
    let err =
        api::fetch_page("http://127.0.0.1:9", "secret-token-value", PAGE).expect_err("unreachable");
    assert!(
        err.contains("unreachable") && !err.contains("secret-token-value"),
        "{err}"
    );
}
