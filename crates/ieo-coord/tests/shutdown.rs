//! The coordinator as PID 1 (02-04): SIGTERM stops it cleanly (exit 0) while it serves; a
//! request in progress is answered first. Uses `IEO_TEST_DB_URL` (key=value form) like the other
//! coordinator tests; skipped without it outside CI.

use postgres::NoTls;
use std::io::{Read, Write};
use std::time::{Duration, Instant};

fn base() -> Option<String> {
    match std::env::var("IEO_TEST_DB_URL") {
        Ok(u) => Some(u),
        Err(_) => {
            assert!(
                std::env::var("CI").is_err(),
                "IEO_TEST_DB_URL must be set in CI"
            );
            eprintln!("SKIP: IEO_TEST_DB_URL not set");
            None
        }
    }
}

fn get(port: u16, path: &str) -> Option<String> {
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).ok()?;
    s.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    write!(
        s,
        "GET {path} HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n"
    )
    .ok()?;
    let mut out = String::new();
    s.read_to_string(&mut out).ok()?;
    Some(out)
}

#[test]
fn sigterm_stops_the_coordinator_with_exit_zero() {
    let Some(url) = base() else { return };
    let cfg: postgres::Config = url.parse().expect("IEO_TEST_DB_URL");
    let dbname = format!("ieo_t_shutdown_{}", std::process::id());
    let mut admin = cfg.connect(NoTls).expect("connect");
    let _ = admin.batch_execute(&format!("DROP DATABASE IF EXISTS {dbname}"));
    admin
        .batch_execute(&format!("CREATE DATABASE {dbname}"))
        .expect("create db");
    let dir = std::env::temp_dir().join(format!("ieo-coord-shutdown-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    let tokens = dir.join("tokens.json");
    std::fs::write(&tokens, r#"{"principals": []}"#).expect("tokens");
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .expect("port")
        .port();
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_ieo-coord"))
        .args([
            "serve",
            "--listen",
            &format!("127.0.0.1:{port}"),
            "--workers",
            "3",
        ])
        .arg("--tokens")
        .arg(&tokens)
        .arg("--manifest")
        .arg(format!(
            "{root}/docs/poc/Console_Coordinator_Sprints_v0.json"
        ))
        .arg("--policy")
        .arg(format!("{root}/contracts/judgment-policy-v0.json"))
        // the URL from a file (as a mounted secret in the coordinator container)
        .env_remove("IEO_DB_URL")
        .env("IEO_DB_URL_FILE", {
            let f = dir.join("db-url");
            std::fs::write(&f, format!("{url} dbname={dbname}\n")).expect("url file");
            f
        })
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn");
    // serving: an unauthenticated request is answered (401), not dropped
    let t0 = Instant::now();
    let answer = loop {
        if let Some(a) = get(port, "/v0/sprints") {
            break a;
        }
        assert!(
            t0.elapsed() < Duration::from_secs(20),
            "the coordinator did not listen"
        );
        std::thread::sleep(Duration::from_millis(100));
    };
    assert!(answer.starts_with("HTTP/1.1 401"), "{answer}");
    let _ = std::process::Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status();
    let t0 = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait().expect("wait") {
            break s;
        }
        if t0.elapsed() > Duration::from_secs(10) {
            let _ = child.kill();
            panic!("SIGTERM did not stop the coordinator within 10 s");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let mut err = String::new();
    let _ = child.stderr.take().map(|mut e| e.read_to_string(&mut err));
    assert_eq!(status.code(), Some(0), "{err}");
    assert!(
        err.contains("stopping (signal)") && err.contains("stopped"),
        "{err}"
    );
    // and it no longer listens
    assert!(get(port, "/v0/sprints").is_none());
    let _ = std::fs::remove_dir_all(&dir);
    let _ = admin.batch_execute(&format!("DROP DATABASE IF EXISTS {dbname}"));
}

#[test]
fn a_binding_on_an_imported_test_id_stops_the_start() {
    // X-01 U3: the startup check refuses a repo binding whose sprint_id a Notion import owns
    let Some(url) = base() else { return };
    let cfg: postgres::Config = url.parse().expect("IEO_TEST_DB_URL");
    let dbname = format!("ieo_t_bindcheck_{}", std::process::id());
    let mut admin = cfg.connect(NoTls).expect("connect");
    let _ = admin.batch_execute(&format!("DROP DATABASE IF EXISTS {dbname}"));
    admin
        .batch_execute(&format!("CREATE DATABASE {dbname}"))
        .expect("create db");
    let mut c2 = cfg.clone();
    c2.dbname(&dbname);
    let mut db = c2.connect(NoTls).expect("connect test db");
    ieo_coord::api::migrate(&mut db).expect("migrate");
    // a draft revision of the test sprint and its import row (fixture rows for the check only)
    db.batch_execute(&format!(
        "INSERT INTO sprint_revisions (sprint_id, revision, repo_id, digest, spec, mode, registration_state, \
         registered_by) VALUES ('POCX-T-01', 1, 'repo-x', '{}', '{{}}', 'draft', 'draft', 'svc')",
        "b".repeat(64)
    ))
    .expect("revision row");
    db.batch_execute(&format!(
        "INSERT INTO notion_imports (page_id, content_digest, test_id, url, last_edited_time, source, \
         sprint_revision, response, imported_by) VALUES ('p', '{}', 'POCX-T-01', 'u', 't', '{{}}', 1, '{{}}', 'svc')",
        "a".repeat(64)
    ))
    .expect("import row");
    let dir = std::env::temp_dir().join(format!("ieo-coord-bindcheck-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    std::fs::write(dir.join("tokens.json"), r#"{"principals": []}"#).expect("tokens");
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let run = |sprint: &str| {
        std::fs::write(
            dir.join("bindings.json"),
            format!(r#"{{"repo-a": {{"sprint_id": "{sprint}"}}}}"#),
        )
        .expect("bindings");
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|l| l.local_addr())
            .expect("port")
            .port();
        let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_ieo-coord"))
            .args(["serve", "--listen", &format!("127.0.0.1:{port}")])
            .arg("--tokens")
            .arg(dir.join("tokens.json"))
            .arg("--bindings")
            .arg(dir.join("bindings.json"))
            .arg("--manifest")
            .arg(format!(
                "{root}/docs/poc/Console_Coordinator_Sprints_v0.json"
            ))
            .arg("--policy")
            .arg(format!("{root}/contracts/judgment-policy-v0.json"))
            .env("IEO_DB_URL", format!("{url} dbname={dbname}"))
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("spawn");
        let t0 = Instant::now();
        loop {
            if let Some(s) = child.try_wait().expect("wait") {
                let mut err = String::new();
                let _ = child.stderr.take().map(|mut e| e.read_to_string(&mut err));
                return (Some(s.code()), err);
            }
            if get(port, "/v0/sprints").is_some() || t0.elapsed() > Duration::from_secs(15) {
                let _ = std::process::Command::new("kill")
                    .args(["-TERM", &child.id().to_string()])
                    .status();
                let _ = child.wait();
                return (None, String::new());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    };
    let (exit, err) = run("POCX-T-01");
    assert_eq!(exit, Some(Some(2)), "{err}");
    assert!(
        err.contains("owned by a Notion import") && err.contains("repo-a: POCX-T-01"),
        "{err}"
    );
    // another sprint id starts normally (it listened, then was stopped)
    let (exit, _) = run("HUM-01");
    assert_eq!(exit, None, "a binding without a clash must start");
    let _ = std::fs::remove_dir_all(&dir);
    drop(db);
    let _ = admin.batch_execute(&format!("DROP DATABASE IF EXISTS {dbname}"));
}
