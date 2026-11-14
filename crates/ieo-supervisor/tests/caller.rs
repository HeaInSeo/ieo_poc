//! Host-side caller (02-04) against role services with a fake `claude`, no model calls.

use ieo_supervisor::budget;
use ieo_supervisor::caller::{self, Adapter, Call, Called, Plan, RoleTarget};
use ieo_supervisor::serve::{Config, Service};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

const TOKEN: &str = "adapter-test-token";

struct Tmp(PathBuf);
impl Tmp {
    fn new(name: &str) -> Tmp {
        let d = std::env::temp_dir().join(format!(
            "ieo-caller-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&d).expect("tmp");
        Tmp(d)
    }
}
impl Drop for Tmp {
    fn drop(&mut self) {
        // children a failing test may have left (the fake records them)
        if let Ok(p) = std::fs::read_to_string(self.0.join("child.pid")) {
            let _ = std::process::Command::new("kill")
                .args(["-KILL", p.trim()])
                .status();
        }
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A fake `claude` whose session totals grow like the CLI's: the cumulative per session is kept
/// in a file next to the script, and `modelUsage` reports it. Each run uses 3/7/20/10
/// (input/output/cache_read/cache_creation). Prompt words: OVERAGE (a breaking rate-limit
/// event), CHILD (leaves a background child), SLEEP.
fn fake_claude(dir: &Path) -> PathBuf {
    let p = dir.join("fake-claude");
    let cum = dir.join("cum");
    std::fs::write(
        &p,
        format!(
            r#"#!/bin/sh
prompt=$(head -c 4096)
sid=$(printf '%s\n' "$@" | awk 'prev=="--session-id"||prev=="--resume"{{print; exit}} {{prev=$0}}')
f="{cum}-$sid"
set -- $(cat "$f" 2>/dev/null || echo 0 0 0 0)
i=$(($1+3)); o=$(($2+7)); r=$(($3+20)); c=$(($4+10)); echo "$i $o $r $c" > "$f"
printf '{{"type":"system","subtype":"init","session_id":"%s"}}\n' "$sid"
printf '{{"type":"assistant","message":{{"id":"m-%s-%s","usage":{{"input_tokens":3,"output_tokens":5,"cache_creation_input_tokens":10,"cache_read_input_tokens":20}}}}}}\n' "$sid" "$i"
case "$prompt" in
  *OVERAGE*) printf '{{"type":"rate_limit_event","rate_limit_info":{{"status":"allowed","overageStatus":"allowed","isUsingOverage":false}}}}\n'; sleep 30;;
  *) printf '{{"type":"rate_limit_event","rate_limit_info":{{"status":"allowed","overageStatus":"rejected","isUsingOverage":false}}}}\n';;
esac
case "$prompt" in *CHILD*) sleep 300 & echo $! > "{dir}/child.pid";; esac
case "$prompt" in *SLEEP*) sleep 60;; esac
printf '{{"type":"result","subtype":"success","is_error":false,"session_id":"%s","usage":{{"input_tokens":3,"output_tokens":7,"cache_creation_input_tokens":10,"cache_read_input_tokens":20}},"modelUsage":{{"m":{{"inputTokens":%s,"outputTokens":%s,"cacheReadInputTokens":%s,"cacheCreationInputTokens":%s}}}},"total_cost_usd":0.01}}\n' "$sid" "$i" "$o" "$r" "$c"
"#,
            cum = cum.display(),
            dir = dir.display()
        ),
    )
    .expect("fake");
    let _ = std::process::Command::new("chmod")
        .arg("+x")
        .arg(&p)
        .status();
    p
}

fn share(role: &str) -> Value {
    if role == "development" {
        json!({"max_runs": 8, "total_seconds": 9_600, "billable_tokens": 1_000_000, "cache_read_tokens": 10_000_000})
    } else {
        json!({"max_runs": 4, "total_seconds": 4_800, "billable_tokens": 500_000, "cache_read_tokens": 5_000_000})
    }
}

fn service(root: &Path, role: &str, kill: Option<&str>) -> Arc<Service> {
    let r = root.join(role);
    let etc = r.join("etc");
    std::fs::create_dir_all(etc.join("template")).expect("etc");
    std::fs::write(
        etc.join("template/settings.json"),
        r#"{"autoMemoryEnabled": false}"#,
    )
    .expect("template");
    let cred = r.join("cred");
    std::fs::create_dir_all(&cred).expect("cred");
    std::fs::write(cred.join("oauth_token"), "fake-oauth-token\n").expect("token");
    std::fs::write(
        cred.join("adapter.sha256"),
        hex::encode(Sha256::digest(TOKEN.as_bytes())),
    )
    .expect("adapter");
    let mut v = json!({"role": role, "repo": "sori", "state": r.join("state"), "etc": etc,
        "cred": cred, "claude": fake_claude(root), "budget": "02-04", "share": share(role)});
    if let Some(k) = kill {
        v["kill"] = json!(k);
    }
    Arc::new(Service::new(Config::from_json(&v).expect("config")))
}

/// In-process transport: the role is chosen by URL.
struct InProc(BTreeMap<String, Arc<Service>>);
impl Adapter for InProc {
    fn send(
        &self,
        role: &RoleTarget,
        method: &str,
        path: &str,
        body: Option<&Value>,
    ) -> Result<(u16, Value), String> {
        let svc = self.0.get(&role.url).ok_or("no such service")?;
        let token = std::fs::read_to_string(&role.adapter_token_file).map_err(|e| e.to_string())?;
        let b = body
            .map(|b| serde_json::to_vec(b).expect("j"))
            .unwrap_or_default();
        let r = svc.handle(method, path, Some(token.trim()), &b);
        Ok((r.status, r.body))
    }
}

fn rate(overage: &str) -> String {
    json!({"type": "rate_limit_event", "rate_limit_info": {"status": "allowed",
           "overageStatus": overage, "isUsingOverage": false}})
    .to_string()
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

/// A plan over `urls` (role → url), with a first observation of `first` (None: none).
fn plan(root: &Path, urls: &[(&str, &str)], first: Option<&str>) -> Plan {
    std::fs::write(root.join("adapter.token"), TOKEN).expect("tok");
    let obs = root.join("first-observation.jsonl");
    let mut v = json!({"budget": "02-04", "ledger_dir": root.join("ledger"), "roles": {}});
    for (role, url) in urls {
        v["roles"][*role] = json!({"url": url, "adapter_token_file": root.join("adapter.token"),
                                    "share": share(role)});
    }
    if let Some(line) = first {
        std::fs::write(&obs, format!("{line}\n")).expect("obs");
        v["first_observation"] = json!({"stream": obs, "observed_unix": now() - 3600});
    }
    Plan::from_json(&v).expect("plan")
}

fn call_of(role: &str, op: &str, attempt: &str, session: &str, prompt: &str) -> Call {
    Call {
        task_id: "02-04".into(),
        role: role.into(),
        op: op.into(),
        attempt_id: attempt.into(),
        session_id: session.into(),
        prompt: prompt.into(),
        coordinator: json!({"generation": 0, "assignment_id": "A1", "sprint_id": "POCX",
            "sprint_revision": 3, "spec_digest": "dg", "image_digest": "sha256:img",
            "profile_id": "dev", "profile_revision": 1}),
    }
}

fn recorded(c: Called) -> Value {
    match c {
        Called::Recorded(r) => r,
        Called::NotStarted(why) => panic!("not started: {why}"),
    }
}

fn not_started(c: Called) -> String {
    match c {
        Called::NotStarted(why) => why,
        Called::Recorded(r) => panic!("started: {r}"),
    }
}

const POLL: Duration = Duration::from_millis(50);

fn both(t: &Tmp) -> (InProc, Plan) {
    let mut m = BTreeMap::new();
    m.insert("dev".to_string(), service(&t.0, "development", None));
    m.insert("gr".to_string(), service(&t.0, "guardrail", None));
    let p = plan(
        &t.0,
        &[("development", "dev"), ("guardrail", "gr")],
        Some(&rate("rejected")),
    );
    (InProc(m), p)
}

#[test]
fn a_new_session_then_a_resume_are_measured_and_charged_to_the_shared_budget() {
    let t = Tmp::new("chain");
    let (a, p) = both(&t);
    let r1 = recorded(
        caller::call(
            &p,
            &call_of("development", "start", "d1", "s1", "hi"),
            &a,
            POLL,
        )
        .expect("call"),
    );
    assert_eq!(r1["stop_required"], false, "{r1}");
    assert_eq!(r1["model_request"], true);
    assert_eq!(
        r1["usage"]["run"]["state"], "measured",
        "{}",
        r1["usage"]["run"]
    );
    assert_eq!(
        r1["usage"]["run"]["tokens"],
        json!({"input": 3, "output": 7, "cache_read": 20, "cache_creation": 10})
    );
    assert!(r1["duration_ms"].is_u64());
    // the stream is kept in the ledger directory with its digest
    let sp = r1["stream"]["path"].as_str().expect("path");
    assert!(sp.starts_with(t.0.join("ledger").to_str().expect("s")));
    // the resume: the base is the caller's own record of the run actually resumed
    let r2 = recorded(
        caller::call(
            &p,
            &call_of("development", "reconnect", "d2", "s-unused", "more"),
            &a,
            POLL,
        )
        .expect("call"),
    );
    assert_eq!(
        r2["session"],
        json!({"mode": "resume", "session_id": "s1", "resume_base_attempt": "d1"}),
        "{r2}"
    );
    assert_eq!(r2["usage"]["run"]["state"], "measured", "{}", r2["usage"]);
    assert_eq!(
        r2["usage"]["run"]["tokens"]["input"], 3,
        "share = cumulative 6 − base 3"
    );
    // the Guardrail role in the same budget
    let r3 = recorded(
        caller::call(
            &p,
            &call_of("guardrail", "start", "g1", "sg1", "review"),
            &a,
            POLL,
        )
        .expect("call"),
    );
    assert_eq!(r3["stop_required"], false, "{r3}");
    // the records are what the budget reads: 3 runs, 3 × 20 billable
    let recs = caller::records(&p).expect("records");
    let s = budget::spent(&recs);
    assert_eq!(
        (
            s.runs,
            s.billable_tokens,
            s.cache_read_tokens,
            s.unmeasured_runs.len()
        ),
        (3, 60, 60, 0)
    );
    // the limits of the next call shrink with what is spent, within the role's share
    let l = caller::limits_for(&p, "guardrail", &recs).expect("limits");
    assert_eq!(l.billable_room, 500_000 - 20);
    assert_eq!(l.timeout + l.grace, Duration::from_secs(1_800));
    // a role's window cap
    let mut v = json!({"budget": "02-04", "ledger_dir": t.0.join("ledger"), "roles": {
        "guardrail": {"url": "gr", "adapter_token_file": t.0.join("adapter.token"),
                      "share": share("guardrail"), "max_window_seconds": 600}}});
    v["first_observation"] =
        json!({"stream": t.0.join("first-observation.jsonl"), "observed_unix": now() - 3600});
    let capped = Plan::from_json(&v).expect("plan");
    let l = caller::limits_for(&capped, "guardrail", &recs).expect("limits");
    assert_eq!(
        (l.timeout + l.grace, l.timeout_is_total),
        (Duration::from_secs(600), false)
    );
    // an attempt id is used once
    assert!(
        not_started(
            caller::call(
                &p,
                &call_of("guardrail", "start", "g1", "sg2", "x"),
                &a,
                POLL
            )
            .expect("c")
        )
        .contains("already")
    );
}

#[test]
fn a_call_is_held_without_a_no_overage_observation() {
    let t = Tmp::new("hold");
    let mut m = BTreeMap::new();
    m.insert("dev".to_string(), service(&t.0, "development", None));
    let a = InProc(m);
    for first in [None, Some(rate("allowed")), Some(rate("allowed_warning"))] {
        let p = plan(&t.0, &[("development", "dev")], first.as_deref());
        let why = not_started(
            caller::call(
                &p,
                &call_of("development", "start", "d1", "s1", "hi"),
                &a,
                POLL,
            )
            .expect("call"),
        );
        assert!(why.contains("Hold"), "{why}");
    }
    assert!(!t.0.join("development/state/runs/d1").exists());
}

#[test]
fn a_breaking_signal_during_the_run_stops_the_budget() {
    let t = Tmp::new("overage");
    let (a, p) = both(&t);
    let r = recorded(
        caller::call(
            &p,
            &call_of("development", "start", "d1", "s1", "OVERAGE"),
            &a,
            POLL,
        )
        .expect("call"),
    );
    // the guard stopped the run on the event; the run is unmeasured and a person must look
    assert_eq!(
        r["outcome"]["stop"]
            .as_str()
            .map(|s| s.starts_with("Overage")),
        Some(true),
        "{r}"
    );
    assert_eq!(r["usage"]["run"]["state"], "unmeasured");
    assert_eq!(r["stop_required"], true);
    assert!(
        r["stop_reason"]
            .as_str()
            .is_some_and(|s| s.contains("overageStatus")),
        "{r}"
    );
    let why = not_started(
        caller::call(
            &p,
            &call_of("guardrail", "start", "g1", "sg", "x"),
            &a,
            POLL,
        )
        .expect("c"),
    );
    assert!(why.contains("asks for a person"), "{why}");
}

#[test]
fn a_cleanup_failure_or_a_run_error_asks_for_a_person() {
    let t = Tmp::new("cleanup");
    let bad_kill = t.0.join("kill");
    std::fs::write(&bad_kill, "#!/bin/sh\nexit 1\n").expect("k");
    let _ = std::process::Command::new("chmod")
        .arg("+x")
        .arg(&bad_kill)
        .status();
    let mut m = BTreeMap::new();
    m.insert(
        "dev".to_string(),
        service(&t.0, "development", Some(bad_kill.to_str().expect("s"))),
    );
    let a = InProc(m);
    let p = plan(&t.0, &[("development", "dev")], Some(&rate("rejected")));
    let r = recorded(
        caller::call(
            &p,
            &call_of("development", "start", "d1", "s1", "CHILD"),
            &a,
            POLL,
        )
        .expect("call"),
    );
    assert_eq!(
        (
            r["outcome"]["cleanup_failed"].as_bool(),
            r["stop_required"].as_bool()
        ),
        (Some(true), Some(true)),
        "{r}"
    );
    // a run error (the program is gone): unmeasured, no duration, a person decides
    let t2 = Tmp::new("runerror");
    let mut m = BTreeMap::new();
    let svc = service(&t2.0, "development", None);
    std::fs::remove_file(t2.0.join("fake-claude")).expect("rm");
    m.insert("dev".to_string(), svc);
    let a = InProc(m);
    let p = plan(&t2.0, &[("development", "dev")], Some(&rate("rejected")));
    let r = recorded(
        caller::call(
            &p,
            &call_of("development", "start", "d1", "s1", "hi"),
            &a,
            POLL,
        )
        .expect("call"),
    );
    assert_eq!(r["stop_required"], true);
    assert!(r["duration_ms"].is_null());
    let s = budget::spent(&caller::records(&p).expect("r"));
    assert_eq!((s.runs, s.unmeasured_runs.len()), (1, 1));
}

#[test]
fn a_resume_whose_base_is_not_the_run_resumed_is_unmeasured() {
    let t = Tmp::new("base");
    let (a, p) = both(&t);
    recorded(
        caller::call(
            &p,
            &call_of("development", "start", "d1", "s1", "hi"),
            &a,
            POLL,
        )
        .expect("c"),
    );
    // the service's checkpoint now names a run the caller has no record of
    let cp = t.0.join("development/state/checkpoint.json");
    let mut v: Value = serde_json::from_slice(&std::fs::read(&cp).expect("cp")).expect("j");
    v["session_cumulative_attempt"] = json!("elsewhere");
    std::fs::write(&cp, serde_json::to_vec(&v).expect("j")).expect("w");
    let r = recorded(
        caller::call(
            &p,
            &call_of("development", "reconnect", "d2", "s9", "more"),
            &a,
            POLL,
        )
        .expect("c"),
    );
    assert_eq!(r["session"]["mode"], "resume");
    assert!(r["session"]["resume_base_attempt"].is_null());
    assert_eq!(
        r["usage"]["run"]["state"], "unmeasured",
        "{}",
        r["usage"]["run"]
    );
    // an unmeasured run stops the budget
    let why = not_started(
        caller::call(
            &p,
            &call_of("guardrail", "start", "g1", "sg", "x"),
            &a,
            POLL,
        )
        .expect("c"),
    );
    assert!(why.contains("Stop"), "{why}");
}

#[test]
fn plans_and_records_are_checked() {
    let t = Tmp::new("plan");
    let base = json!({"budget": "02-04", "ledger_dir": t.0.join("ledger"), "roles": {
        "development": {"url": "u", "adapter_token_file": "f", "share": share("development")},
        "guardrail": {"url": "u", "adapter_token_file": "f", "share": share("guardrail")}}});
    assert!(Plan::from_json(&base).is_ok());
    // shares above the approval
    let mut over = base.clone();
    over["roles"]["guardrail"]["share"]["max_runs"] = json!(5);
    assert!(Plan::from_json(&over).expect_err("over").contains("exceed"));
    let mut bad = base.clone();
    bad["roles"]["other"] = bad["roles"]["guardrail"].clone();
    assert!(Plan::from_json(&bad).is_err());
    // an unreadable record line
    let p = Plan::from_json(&base).expect("plan");
    std::fs::create_dir_all(t.0.join("ledger")).expect("d");
    std::fs::write(
        caller::records_path(&p),
        "{\"attempt_id\":\"a\"}\nnot json\n",
    )
    .expect("w");
    assert!(caller::records(&p).expect_err("bad").contains("line 2"));
}

#[test]
fn the_caller_talks_to_the_service_over_http() {
    let t = Tmp::new("http");
    let svc_cfg = {
        let s = service(&t.0, "development", None);
        drop(s);
        let r = t.0.join("development");
        Config::from_json(
            &json!({"role": "development", "repo": "sori", "state": r.join("state"),
            "etc": r.join("etc"), "cred": r.join("cred"), "claude": t.0.join("fake-claude"),
            "budget": "02-04", "share": share("development")}),
        )
        .expect("cfg")
    };
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .expect("port")
        .port();
    std::thread::spawn(move || {
        let _ = ieo_supervisor::serve::serve(svc_cfg, &format!("127.0.0.1:{port}"));
    });
    let url = format!("http://127.0.0.1:{port}");
    let p = plan(&t.0, &[("development", &url)], Some(&rate("rejected")));
    let t0 = std::time::Instant::now();
    let r = loop {
        match caller::call(
            &p,
            &call_of("development", "start", "d1", "s1", "hi"),
            &caller::Http,
            POLL,
        ) {
            Ok(c) => break recorded(c),
            Err(e) if t0.elapsed() < Duration::from_secs(10) && e.contains("transport") => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => panic!("{e}"),
        }
    };
    assert_eq!(
        (
            r["usage"]["run"]["state"].as_str(),
            r["stop_required"].as_bool()
        ),
        (Some("measured"), Some(false)),
        "{r}"
    );
    // a wrong adapter token is refused before anything starts
    std::fs::write(t.0.join("adapter.token"), "wrong").expect("w");
    let why = not_started(
        caller::call(
            &p,
            &call_of("development", "start", "d2", "s2", "hi"),
            &caller::Http,
            POLL,
        )
        .expect("c"),
    );
    assert!(why.contains("401"), "{why}");
}
