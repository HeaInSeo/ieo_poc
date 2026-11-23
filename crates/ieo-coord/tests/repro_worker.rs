//! The queue uses a real local container when IEO_TEST_REPRO_IMAGE is supplied.
mod common;
use common::*;
use ieo_coord::repro::tick;
use ieo_supervisor::repro::Runner;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn queued_reproduction_is_isolated_and_reconciled_without_another_start() {
    let Ok(image) = std::env::var("IEO_TEST_REPRO_IMAGE") else {
        eprintln!("SKIP: no pinned local reproduction image");
        return;
    };
    let e = start().expect("test database");
    ready_sprint(&e, 9);
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    let bundles = dir.path().join("bundles");
    let state = dir.path().join("state");
    for path in [&repo, &bundles, &state] {
        fs::create_dir(path).unwrap();
    }
    git(&repo, &["init", "-q"]);
    fs::write(repo.join("test-marker"), "candidate").unwrap();
    fs::write(repo.join(".gitattributes"), "test-marker export-ignore\n").unwrap();
    git(&repo, &["add", ".gitattributes", "test-marker"]);
    git(
        &repo,
        &[
            "-c",
            "user.name=fixture",
            "-c",
            "user.email=fixture@invalid",
            "commit",
            "-qm",
            "candidate",
        ],
    );
    let sha = git(&repo, &["rev-parse", "HEAD"]);
    let digest = hex::encode(Sha256::digest(b"bundle-fixture"));
    fs::write(bundles.join(&digest), b"bundle-fixture").unwrap();
    let runner = Runner::new(&repo.join(".git"), &bundles, &state, image.clone()).unwrap();
    assert_eq!(
        assign(&e, "guardrail", "G1", 0, "01-01", &sha, Value::Null).0,
        201
    );
    let request = |id, generation, command: &[&str], bundle: &str| {
        json!({"request_id":id,"repo_id":"repo-a","assignment_id":"G1","generation":generation,
            "target_sha":sha,"bundle_digest":bundle,"command":command,"expected":"fixture","timeout_seconds":15})
    };
    let submit = |body: &Value| e.post("gr-a", "/v0/agent/repro-requests", body);
    let status = |id| e.get("gr-a", &format!("/v0/agent/repro-requests/{id}")).1;
    let run_key = |id: &str| {
        e.db()
            .query_one(
                "SELECT run_key FROM repro_requests WHERE repo_id='repo-a' AND request_id=$1",
                &[&id],
            )
            .unwrap()
            .get::<_, String>(0)
    };
    let check_finished = |id| {
        let start = Instant::now();
        loop {
            // A new DB connection represents a restarted worker process. Only
            // the durable container and database carry the original run forward.
            tick(&mut e.db(), "repo-a", &runner).unwrap();
            let view = status(id);
            if view["state"] != "running" {
                break view;
            }
            assert!(start.elapsed() < Duration::from_secs(12), "{view}");
            std::thread::sleep(Duration::from_millis(150));
        }
    };
    let good = request(
        "repro-success",
        0,
        &[
            "/bin/sh",
            "-ec",
            "test \"$(cat test-marker)\" = candidate; test \"$(cat \"$REPRO_BUNDLE\")\" = bundle-fixture; test ! -e /run/secrets/oauth_token; test ! -e /run/ieo/cred; test ! -e /inputs/../opt/rust; test \"$(ls /sys/class/net)\" = lo; printf ok",
        ],
        &digest,
    );
    assert_eq!(submit(&good).0, 202);
    assert_eq!(submit(&good).0, 200);
    {
        let mut db = e.db();
        tick(&mut db, "repo-a", &runner).unwrap();
    }
    let key = run_key("repro-success");
    e.db()
        .batch_execute(
            "CREATE FUNCTION lose_repro_result() RETURNS trigger LANGUAGE plpgsql AS
         $$ BEGIN RAISE EXCEPTION 'fixture lost result'; END; $$;
         CREATE TRIGGER lose_repro_result BEFORE UPDATE ON repro_requests
         FOR EACH ROW WHEN (OLD.request_id='repro-success' AND NEW.state='finished')
           EXECUTE FUNCTION lose_repro_result();",
        )
        .unwrap();
    let started = Instant::now();
    loop {
        if tick(&mut e.db(), "repo-a", &runner).is_err() {
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(12));
        std::thread::sleep(Duration::from_millis(150));
    }
    assert_eq!(status("repro-success")["state"], "running");
    e.db()
        .batch_execute("DROP TRIGGER lose_repro_result ON repro_requests")
        .unwrap();
    let finished = check_finished("repro-success");
    assert_eq!(finished["state"], "finished", "{finished}");
    let stderr = finished["raw_log_refs"][1].as_str().unwrap();
    let trace = e.get("gr-a", stderr).1["content"]
        .as_str()
        .and_then(|value| hex::decode(value).ok())
        .unwrap_or_default();
    assert_eq!(
        finished["exit_code"],
        0,
        "{finished} {}",
        String::from_utf8_lossy(&trace)
    );
    assert_eq!(finished["runner_image_digest"], image);
    let refs = finished["raw_log_refs"].as_array().unwrap();
    let stdout = refs[0].as_str().unwrap();
    assert_eq!(e.get("gr-a", stdout).1["content"], "6f6b");
    assert_eq!(e.get("gr-b", stdout).0, 404);
    assert_eq!(submit(&good), (200, finished));
    assert_eq!(run_key("repro-success"), key);

    let failed = request(
        "repro-nonzero",
        0,
        &["/bin/sh", "-c", "printf failure >&2; exit 7"],
        &digest,
    );
    assert_eq!(submit(&failed).0, 202);
    tick(&mut e.db(), "repo-a", &runner).unwrap();
    let result = check_finished("repro-nonzero");
    assert_eq!(result["state"], "finished");
    assert_eq!(result["exit_code"], 7);
    let stderr = result["raw_log_refs"][1].as_str().unwrap();
    assert_eq!(e.get("gr-a", stderr).1["content"], "6661696c757265");

    let bad = request("repro-bad-input", 0, &["/bin/true"], &"a".repeat(64));
    assert_eq!(submit(&bad).0, 202);
    tick(&mut e.db(), "repo-a", &runner).unwrap();
    assert_eq!(status("repro-bad-input")["state"], "runner_failed");

    let unknown = request("repro-unknown", 0, &["/bin/true"], &digest);
    assert_eq!(submit(&unknown).0, 202);
    let unknown_key = "ieo-repro-11111111-1111-4111-8111-111111111111";
    // Durable reservation without proof that the external call did or did not happen.
    e.db().execute("UPDATE repro_requests SET state='running',run_key=$1,runner_image=$2 WHERE request_id='repro-unknown'",
        &[&unknown_key, &image]).unwrap();
    for _ in 0..2 {
        tick(&mut e.db(), "repo-a", &runner).unwrap();
        assert_eq!(submit(&unknown).1["state"], "running");
    }
    assert_eq!(run_key("repro-unknown"), unknown_key);
    assert!(!state.join(unknown_key).exists());

    let stale = request("repro-stale", 0, &["/bin/true"], &digest);
    assert_eq!(submit(&stale).0, 202);
    assert_eq!(
        assign(
            &e,
            "guardrail",
            "G1",
            1,
            "01-01",
            &sha,
            json!({"assignment_id":"G1","generation":0})
        )
        .0,
        201
    );
    tick(&mut e.db(), "repo-a", &runner).unwrap();
    assert_eq!(status("repro-stale")["state"], "rejected_stale_generation");
    assert_eq!(fs::read_dir(&state).unwrap().count(), 0);
}
