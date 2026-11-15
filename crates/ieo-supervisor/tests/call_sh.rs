//! The 02-04 operator script `call.sh` (real-sessions-20261115), restart step R1, against stub
//! role services and a stub `podman` (X-08: reviews 12c9a21 N6, dc11612 L-c3-3/L-c3-4). No
//! container, no model call. Needs bash, curl, flock and python3 (as the CI runner has).

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const TOKEN: &str = "adapter-test-token";

fn call_sh() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/poc/reports/tasks/02-04/real-sessions-20261115/call.sh")
}

struct Tmp(PathBuf);
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A role service's status endpoint: answers the current status to the right bearer, 401
/// otherwise. The 401 body also carries the status fields, so only the HTTP status (`curl -f`)
/// tells it apart (review 681c1d3 I-2).
struct Stub {
    port: u16,
    status: Arc<Mutex<Value>>,
}

fn stub(status: Value) -> Stub {
    let server = tiny_http::Server::http("127.0.0.1:0").expect("bind");
    let port = server.server_addr().to_ip().expect("ip").port();
    let status = Arc::new(Mutex::new(status));
    let s = status.clone();
    std::thread::spawn(move || {
        for req in server.incoming_requests() {
            let authorized = req.headers().iter().any(|h| {
                h.field.equiv("Authorization") && h.value.as_str() == format!("Bearer {TOKEN}")
            });
            let (code, body) = if !authorized {
                let mut b = s.lock().expect("s").clone();
                b["code"] = json!("UNAUTHORIZED");
                (401, b)
            } else if req.url() == "/v0/role/status" {
                (200, s.lock().expect("s").clone())
            } else {
                (404, json!({"code": "NOT_FOUND"}))
            };
            let _ = req
                .respond(tiny_http::Response::from_string(body.to_string()).with_status_code(code));
        }
    });
    Stub { port, status }
}

fn idle(last: &str) -> Value {
    json!({"running": null, "last": {"attempt_id": last},
           "checkpoint": {"session_id": "s1", "phase": "after_run", "session_cumulative_attempt": last}})
}

fn record(attempt: &str, role: &str) -> Value {
    json!({"format": "ieo-role-run/v0", "attempt_id": attempt, "role": role, "stop_required": false})
}

struct Setup {
    t: Tmp,
    dev: Stub,
    gr: Stub,
}

impl Setup {
    fn new(name: &str) -> Setup {
        let d = std::env::temp_dir().join(format!(
            "ieo-callsh-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(d.join("bin")).expect("bin");
        std::fs::create_dir_all(d.join("ledger")).expect("ledger");
        std::fs::create_dir_all(d.join("stub")).expect("stub");
        let t = Tmp(d);
        let w = &t.0;
        std::os::unix::fs::symlink(
            env!("CARGO_BIN_EXE_ieo-role-call"),
            w.join("bin/ieo-role-call"),
        )
        .expect("link");
        std::fs::write(w.join("adapter.token"), TOKEN).expect("token");
        std::fs::write(
            w.join("adapter.header"),
            format!("Authorization: Bearer {TOKEN}\n"),
        )
        .expect("header");
        let dev = stub(idle("D1"));
        let gr = stub(idle("G1"));
        let share = json!({"max_runs": 4, "total_seconds": 4_800, "billable_tokens": 500_000,
                           "cache_read_tokens": 5_000_000});
        let plan = json!({"budget": "02-04", "ledger_dir": w.join("ledger"), "roles": {
            "development": {"url": format!("http://127.0.0.1:{}", dev.port),
                            "adapter_token_file": w.join("adapter.token"), "share": share},
            "guardrail": {"url": format!("http://127.0.0.1:{}", gr.port),
                          "adapter_token_file": w.join("adapter.token"), "share": share}}});
        std::fs::write(w.join("plan.json"), plan.to_string()).expect("plan");
        // podman: the port of each container, a restart that notes whether caller.lock is held
        // while it runs and whether it inherited the lock's descriptor 9 (review 681c1d3 I-1: a
        // long-lived helper holding it would block every later call), and PID 1's command line
        let podman = w.join("stub/podman");
        std::fs::write(
            &podman,
            format!(
                r#"#!/bin/sh
case "$1" in
  port) case "$2" in *development*) echo "0.0.0.0:{dev}";; *) echo "0.0.0.0:{gr}";; esac;;
  restart) if flock -n "{w}/ledger/caller.lock" true; then held=free; else held=held; fi
           if [ -e /proc/self/fd/9 ]; then fd=fd9-inherited; else fd=fd9-closed; fi
           echo "restart $* lock-$held $fd" >> "{w}/restarts.log";;
  exec) printf 'ieo-supervisor\0serve\0';;
esac
"#,
                dev = dev.port,
                gr = gr.port,
                w = w.display()
            ),
        )
        .expect("podman");
        let _ = Command::new("chmod").arg("+x").arg(&podman).status();
        let s = Setup { t, dev, gr };
        s.records(&[record("D1", "development"), record("G1", "guardrail")]);
        s
    }

    fn records(&self, lines: &[Value]) {
        let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
        std::fs::write(self.t.0.join("ledger/records.jsonl"), text).expect("records");
    }

    fn restarts(&self) -> String {
        std::fs::read_to_string(self.t.0.join("restarts.log")).unwrap_or_default()
    }

    /// `call.sh <W> R1`: exit code and output.
    fn r1(&self) -> (i32, String) {
        let path = format!(
            "{}:{}",
            self.t.0.join("stub").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let out = Command::new("bash")
            .arg(call_sh())
            .arg(&self.t.0)
            .arg("R1")
            .env("PATH", path)
            .env("IEO_POC_PY", "python3")
            .env("CALLER_LOCK_WAIT", "1")
            .output()
            .expect("bash");
        (
            out.status.code().unwrap_or(-1),
            format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ),
        )
    }
}

#[test]
fn the_restart_step_runs_only_when_every_guard_holds() {
    // all clear: one restart, made while the caller's lock was held
    let s = Setup::new("clear");
    let (rc, out) = s.r1();
    assert_eq!(rc, 0, "{out}");
    let log = s.restarts();
    assert_eq!(log.lines().count(), 1, "{log}");
    assert!(
        log.contains("ieo-rt-fixture0204-development")
            && log.contains("lock-held")
            && log.contains("fd9-closed"),
        "{log}"
    );
    let summary: Value = serde_json::from_str(
        &std::fs::read_to_string(s.t.0.join("summaries/R1.json")).expect("summary"),
    )
    .expect("json");
    assert_eq!(summary["before"], "s1 after_run D1", "{summary}");

    // L-c3-3: the other role's service ran an attempt the ledger does not know
    let s = Setup::new("crossrole");
    *s.gr.status.lock().expect("s") = idle("G9");
    let (rc, out) = s.r1();
    assert_eq!(rc, 1, "{out}");
    assert!(out.contains("G9 has no caller record"), "{out}");
    assert_eq!(s.restarts(), "");

    // L-c3-4 (a): the status answer is an error (401): no restart on an empty status
    let s = Setup::new("status401");
    std::fs::write(
        s.t.0.join("adapter.header"),
        "Authorization: Bearer wrong\n",
    )
    .expect("w");
    let (rc, out) = s.r1();
    assert_eq!(rc, 1, "{out}");
    assert!(out.contains("status unavailable"), "{out}");
    assert_eq!(s.restarts(), "");

    // L-c3-4 (b): another caller holds the ledger
    let s = Setup::new("locked");
    let held = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(s.t.0.join("ledger/caller.lock"))
        .expect("lock");
    held.lock().expect("held");
    let t0 = Instant::now();
    let (rc, out) = s.r1();
    assert_eq!(rc, 1, "{out}");
    assert!(out.contains("another caller"), "{out}");
    assert!(t0.elapsed() < Duration::from_secs(10));
    assert_eq!(s.restarts(), "");
    drop(held);

    // N6: a correction line is verified, not trusted (this one does not hold)
    let s = Setup::new("correction");
    let mut d1 = record("D1", "development");
    d1["stop_required"] = json!(true);
    d1["stop_reason"] = json!("usage unmeasured: x");
    s.records(&[
        d1,
        record("G1", "guardrail"),
        json!({"format": "ieo-role-run-correction/v0", "corrects": "D1", "decision": "x",
               "original_stream_sha256": null, "after": {"usage": {}, "stop_required": false}}),
    ]);
    let (rc, out) = s.r1();
    assert_eq!(rc, 1, "{out}");
    assert!(out.contains("restart refused"), "{out}");
    assert_eq!(s.restarts(), "");

    // a call in flight, and a running attempt
    let s = Setup::new("inflight");
    std::fs::write(
        s.t.0.join("ledger/inflight.json"),
        "{\"attempt_id\":\"D2\"}\n",
    )
    .expect("w");
    let (rc, out) = s.r1();
    assert_eq!(rc, 1, "{out}");
    assert!(out.contains("in flight"), "{out}");
    let s = Setup::new("running");
    s.dev.status.lock().expect("s")["running"] = json!({"attempt_id": "D2"});
    let (rc, out) = s.r1();
    assert_eq!(rc, 1, "{out}");
    assert!(out.contains("running"), "{out}");
    assert_eq!(s.restarts(), "");
}
