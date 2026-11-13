//! Minimal HTTP/1.1 front end. Each worker thread owns one PostgreSQL connection.

use crate::api::{Ctx, Req, handle};
use crate::auth::Credentials;
use postgres::{Client, Config, NoTls};
use std::io::Read;
use std::sync::Arc;
use tiny_http::{Header, Response, Server};

/// Largest accepted request body.
pub const MAX_BODY: u64 = 1 << 20;

pub struct App {
    pub ctx: Ctx,
    pub creds: Credentials,
}

fn header<'a>(req: &'a tiny_http::Request, name: &str) -> Option<&'a str> {
    req.headers()
        .iter()
        .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(name))
        .map(|h| h.value.as_str())
}

fn respond(req: tiny_http::Request, status: u16, body: &serde_json::Value) {
    let bytes = serde_json::to_vec(body).unwrap_or_default();
    let mut resp = Response::from_data(bytes).with_status_code(status);
    if let Ok(h) = Header::from_bytes("Content-Type", "application/json") {
        resp = resp.with_header(h);
    }
    let _ = req.respond(resp);
}

fn serve_one(db: &mut Client, app: &App, mut req: tiny_http::Request) {
    let mut body = Vec::new();
    let too_big = req.body_length().is_some_and(|n| n as u64 > MAX_BODY);
    let read = req.as_reader().take(MAX_BODY + 1).read_to_end(&mut body);
    if too_big || read.is_err() || body.len() as u64 > MAX_BODY {
        let b = serde_json::json!({"rejections": [{"code": "SCHEMA_VIOLATION", "field": "",
            "message": "body too large or unreadable", "correction": "send at most 1 MiB",
            "retryable_after_edit": true}]});
        respond(req, 400, &b);
        return;
    }
    let url = req.url().to_string();
    let (path, query) = url.split_once('?').unwrap_or((url.as_str(), ""));
    let method = req.method().as_str().to_string();
    let who = app.creds.authenticate(header(&req, "Authorization"));
    let idem = header(&req, "Idempotency-Key").map(str::to_string);
    let r = handle(
        db,
        &app.ctx,
        who,
        &Req {
            method: &method,
            path,
            query,
            idempotency_key: idem.as_deref(),
            body: &body,
        },
    );
    respond(req, r.status, &r.body);
}

/// Serve until the process ends. The DB config comes from the environment, never from argv.
pub fn serve(
    server: Arc<Server>,
    app: Arc<App>,
    db: &Config,
    workers: usize,
) -> Result<(), String> {
    let mut handles = Vec::new();
    for _ in 0..workers.max(1) {
        let mut db = db.connect(NoTls).map_err(|e| format!("db connect: {e}"))?;
        let server = Arc::clone(&server);
        let app = Arc::clone(&app);
        handles.push(std::thread::spawn(move || {
            while let Ok(req) = server.recv() {
                if db.is_closed() {
                    break;
                }
                serve_one(&mut db, &app, req);
            }
        }));
    }
    for h in handles {
        let _ = h.join();
    }
    Ok(())
}
