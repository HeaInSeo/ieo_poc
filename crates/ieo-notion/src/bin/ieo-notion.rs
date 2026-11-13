//! ieo-notion (X-01)
//!
//! ieo-notion snapshot --from-connector FILE --out FILE
//!     Convert a saved connector fetch result into an ieo-notion-snapshot/v0 file.
//! ieo-notion import (--snapshot FILE | --page ID_OR_URL --notion-token-file F [--api-base URL])
//!     --test-id ID --coordinator URL --coordinator-token-file F [--dry-run]
//!     Read the page, derive the import request and POST it to the coordinator.
//!     If the page cannot be read, nothing is sent (the coordinator DB is untouched).
//!
//! Exit codes: 0 registered or replayed, 1 refused by the coordinator, 2 read/usage error.

use ieo_notion::{
    NotionPage, api, from_connector_fetch, from_snapshot, import_request, page_id_from, to_snapshot,
};
use serde_json::Value;
use std::time::Duration;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn read_json(path: &str) -> Result<Value, String> {
    let b = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    serde_json::from_slice(&b).map_err(|e| format!("{path}: {e}"))
}

fn read_secret(path: &str) -> Result<String, String> {
    let s = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let s = s.trim().to_string();
    if s.is_empty() {
        return Err(format!("{path} is empty"));
    }
    Ok(s)
}

fn now_utc() -> String {
    // seconds precision is enough for observed_at; avoids a date crate
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let days = secs / 86_400;
    let (h, m, s) = ((secs % 86_400) / 3600, (secs % 3600) / 60, secs % 60);
    // civil from days (Howard Hinnant)
    let z = i64::try_from(days).unwrap_or_default() + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

fn read_page(args: &[String]) -> Result<NotionPage, String> {
    if let Some(f) = arg(args, "--snapshot") {
        return from_snapshot(&read_json(&f)?);
    }
    let page = arg(args, "--page").ok_or("--snapshot or --page is required")?;
    let id = page_id_from(&page).ok_or("--page is not a Notion page id or URL")?;
    let token = read_secret(
        &arg(args, "--notion-token-file").ok_or("--notion-token-file is required with --page")?,
    )?;
    let base = arg(args, "--api-base").unwrap_or_else(|| "https://api.notion.com".into());
    api::fetch_page(&base, &token, &id)
}

fn run() -> Result<i32, String> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("snapshot") => {
            let src = arg(&args, "--from-connector").ok_or("--from-connector is required")?;
            let out = arg(&args, "--out").ok_or("--out is required")?;
            let p = from_connector_fetch(&read_json(&src)?)?;
            let body = serde_json::to_vec_pretty(&to_snapshot(&p)).map_err(|e| e.to_string())?;
            std::fs::write(&out, body).map_err(|e| format!("{out}: {e}"))?;
            println!("snapshot {} page {} digest {}", out, p.page_id, ieo_notion::content_digest(&p));
            Ok(0)
        }
        Some("import") => {
            let test_id = arg(&args, "--test-id").ok_or("--test-id is required")?;
            let page = read_page(&args)?;
            let req = import_request(&page, &test_id, &now_utc())?;
            if args.iter().any(|a| a == "--dry-run") {
                println!("{}", serde_json::to_string_pretty(&req).map_err(|e| e.to_string())?);
                return Ok(0);
            }
            let coord = arg(&args, "--coordinator").ok_or("--coordinator is required")?;
            let token = read_secret(&arg(&args, "--coordinator-token-file").ok_or("--coordinator-token-file is required")?)?;
            let url = format!("{}/v0/imports/notion", coord.trim_end_matches('/'));
            let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(30)).build();
            let (status, body): (u16, Value) = match agent
                .post(&url)
                .set("Authorization", &format!("Bearer {token}"))
                .send_json(req)
            {
                Ok(r) => (r.status(), r.into_json().unwrap_or(Value::Null)),
                Err(ureq::Error::Status(c, r)) => (c, r.into_json().unwrap_or(Value::Null)),
                Err(e) => return Err(format!("coordinator unreachable: {}", e.kind())),
            };
            let summary = serde_json::json!({
                "http_status": status,
                "test_id": body["import"]["test_id"],
                "replay": body["replay"],
                "registration": body["registration"],
                "unresolved": body["conversion"]["unresolved_bindings"],
                "refusal": body.get("rejections").or_else(|| body.get("code")),
            });
            println!("{}", serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?);
            Ok(if (200..300).contains(&status) { 0 } else { 1 })
        }
        _ => Err("usage: ieo-notion snapshot --from-connector F --out F | import (--snapshot F | --page ID --notion-token-file F [--api-base URL]) --test-id ID --coordinator URL --coordinator-token-file F [--dry-run]".into()),
    }
}

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("ieo-notion: {e}");
            std::process::exit(2);
        }
    }
}
