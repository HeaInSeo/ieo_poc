//! Notion REST API reader (read-only). The token is read from a file by the caller and is
//! only sent in the `Authorization` header; it is never logged or written anywhere.

use crate::NotionPage;
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::Duration;

pub const NOTION_VERSION: &str = "2022-06-28";

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(20))
        .build()
}

fn get(base: &str, token: &str, path: &str) -> Result<Value, String> {
    let url = format!("{}{path}", base.trim_end_matches('/'));
    match agent()
        .get(&url)
        .set("Authorization", &format!("Bearer {token}"))
        .set("Notion-Version", NOTION_VERSION)
        .call()
    {
        Ok(r) => r
            .into_json::<Value>()
            .map_err(|e| format!("Notion response is not JSON: {e}")),
        Err(ureq::Error::Status(code, r)) => {
            let body: Value = r.into_json().unwrap_or(Value::Null);
            Err(format!(
                "Notion API {path} → HTTP {code} {}",
                body["code"].as_str().unwrap_or("")
            ))
        }
        Err(e) => Err(format!("Notion API {path} unreachable: {}", e.kind())),
    }
}

fn rich(v: &Value) -> String {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|t| t["plain_text"].as_str())
        .collect()
}

/// A property value as plain text (same shape the connector snapshot produces).
fn property_text(p: &Value) -> String {
    let t = p["type"].as_str().unwrap_or_default();
    let v = &p[t];
    match t {
        "title" | "rich_text" => rich(v),
        "select" | "status" => v["name"].as_str().unwrap_or_default().to_string(),
        "multi_select" => v
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|o| o["name"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        "date" => v["start"].as_str().unwrap_or_default().to_string(),
        "checkbox" => if v == true { "__YES__" } else { "__NO__" }.to_string(),
        "url" | "email" | "phone_number" => v.as_str().unwrap_or_default().to_string(),
        "number" => {
            if v.is_null() {
                String::new()
            } else {
                v.to_string()
            }
        }
        "relation" => v
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|o| o["id"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn children(base: &str, token: &str, id: &str) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let q = match &cursor {
            Some(c) => format!("/v1/blocks/{id}/children?page_size=100&start_cursor={c}"),
            None => format!("/v1/blocks/{id}/children?page_size=100"),
        };
        let page = get(base, token, &q)?;
        out.extend(page["results"].as_array().into_iter().flatten().cloned());
        match (page["has_more"].as_bool(), page["next_cursor"].as_str()) {
            (Some(true), Some(c)) => cursor = Some(c.to_string()),
            _ => return Ok(out),
        }
    }
}

/// Body blocks as the same text form the connector returns (headings, paragraphs, tables).
fn render(base: &str, token: &str, blocks: &[Value]) -> Result<String, String> {
    let mut out = String::new();
    for b in blocks {
        let t = b["type"].as_str().unwrap_or_default();
        let text = rich(&b[t]["rich_text"]);
        match t {
            "heading_1" => out += &format!("# {text}\n\n"),
            "heading_2" => out += &format!("## {text}\n\n"),
            "heading_3" => out += &format!("### {text}\n\n"),
            "paragraph" | "quote" | "callout" => out += &format!("{text}\n\n"),
            "bulleted_list_item" | "numbered_list_item" | "to_do" => out += &format!("- {text}\n"),
            "code" => out += &format!("```\n{text}\n```\n\n"),
            "table" => {
                let id = b["id"].as_str().unwrap_or_default();
                out += "<table>\n";
                for row in children(base, token, id)? {
                    out += "<tr>\n";
                    for cell in row["table_row"]["cells"].as_array().into_iter().flatten() {
                        out += &format!("<td>\n{}\n</td>\n", rich(cell));
                    }
                    out += "</tr>\n";
                }
                out += "</table>\n\n";
            }
            _ => {}
        }
    }
    Ok(out.trim().to_string())
}

/// Read one page (properties + body) through the REST API.
pub fn fetch_page(base: &str, token: &str, page_id: &str) -> Result<NotionPage, String> {
    let page = get(base, token, &format!("/v1/pages/{page_id}"))?;
    let properties: BTreeMap<String, String> = page["properties"]
        .as_object()
        .ok_or("page has no properties")?
        .iter()
        .map(|(k, v)| (k.clone(), property_text(v)))
        .collect();
    let blocks = children(base, token, page_id)?;
    Ok(NotionPage {
        page_id: crate::page_id_from(page_id).unwrap_or_else(|| page_id.to_string()),
        url: page["url"].as_str().unwrap_or_default().to_string(),
        last_edited_time: page["last_edited_time"]
            .as_str()
            .ok_or("page has no last_edited_time")?
            .to_string(),
        properties,
        content: render(base, token, &blocks)?,
        fetched_via: "notion-rest-api".into(),
    })
}
