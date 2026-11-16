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

/// The caller tests run one at a time: the services spawn processes, and a fork elsewhere in
/// the test process briefly holds copies of other tests' descriptors (review f90f5b5 N1).
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

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
/// event), CHILD (leaves a background child), SLEEP, AGENTRUN (one child agent c1), with
/// AGENTTWO a second child c2 like c1.
fn fake_claude(dir: &Path) -> PathBuf {
    let p = dir.join("fake-claude");
    let cum = dir.join("cum");
    std::fs::write(
        &p,
        format!(
            r#"#!/bin/sh
prompt=$(head -c 4096)
err=false; case "$prompt" in *AUTHFAIL*) err=true;; esac
sid=$(printf '%s\n' "$@" | awk 'prev=="--session-id"||prev=="--resume"{{print; exit}} {{prev=$0}}')
f="{cum}-$sid"
set -- $(cat "$f" 2>/dev/null || echo 0 0 0 0)
case "$prompt" in
  *AGENTTWO*) i=$(($1+7)); o=$(($2+21)); r=$(($3+34)); c=$(($4+30));;
  *AGENTRUN*) i=$(($1+5)); o=$(($2+14)); r=$(($3+27)); c=$(($4+20));;
  *) i=$(($1+3)); o=$(($2+7)); r=$(($3+20)); c=$(($4+10));;
esac
echo "$i $o $r $c" > "$f"
printf '{{"type":"system","subtype":"init","session_id":"%s"}}\n' "$sid"
printf '{{"type":"assistant","message":{{"id":"m-%s-%s","usage":{{"input_tokens":3,"output_tokens":5,"cache_creation_input_tokens":10,"cache_read_input_tokens":20}}}}}}\n' "$sid" "$i"
case "$prompt" in
  *OVERAGE*) printf '{{"type":"rate_limit_event","rate_limit_info":{{"status":"allowed","overageStatus":"allowed","isUsingOverage":false}}}}\n'; sleep 30;;
  *) printf '{{"type":"rate_limit_event","rate_limit_info":{{"status":"allowed","overageStatus":"rejected","isUsingOverage":false}}}}\n';;
esac
case "$prompt" in *CHILD*) sleep 300 & echo $! > "{dir}/child.pid";; esac
case "$prompt" in *AGENTRUN*)
  # a child with two responses: its transcript has both, tool_use_result only the last (D3)
  t="$CLAUDE_CONFIG_DIR/projects/-state-work-x/$sid/subagents"; mkdir -p "$t"
  printf '{{"type":"assistant","isSidechain":true,"agentId":"c1","message":{{"id":"c1-r1","stop_reason":"tool_use","usage":{{"input_tokens":1,"output_tokens":5,"cache_creation_input_tokens":7,"cache_read_input_tokens":0}}}}}}
{{"type":"assistant","isSidechain":true,"agentId":"c1","message":{{"id":"c1-r2","stop_reason":"end_turn","usage":{{"input_tokens":1,"output_tokens":2,"cache_creation_input_tokens":3,"cache_read_input_tokens":7}}}}}}
' > "$t/agent-c1.jsonl"
  printf '{{"type":"assistant","message":{{"id":"m-agent-%s","usage":{{"input_tokens":0,"output_tokens":1,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}},"content":[{{"type":"tool_use","id":"tu-%s","name":"Agent"}}]}}}}
' "$i" "$i"
  printf '{{"type":"user","message":{{"content":[{{"type":"tool_result","tool_use_id":"tu-%s"}}]}},"tool_use_result":{{"agentId":"c1","usage":{{"input_tokens":1,"output_tokens":2,"cache_creation_input_tokens":3,"cache_read_input_tokens":7}}}}}}
' "$i";;
esac
case "$prompt" in *AGENTTWO*)
  sed 's/c1/c2/g' "$t/agent-c1.jsonl" > "$t/agent-c2.jsonl"
  printf '{{"type":"assistant","message":{{"id":"m-agent2-%s","usage":{{"input_tokens":0,"output_tokens":0,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}},"content":[{{"type":"tool_use","id":"tv-%s","name":"Agent"}}]}}}}
' "$i" "$i"
  printf '{{"type":"user","message":{{"content":[{{"type":"tool_result","tool_use_id":"tv-%s"}}]}},"tool_use_result":{{"agentId":"c2","usage":{{"input_tokens":1,"output_tokens":2,"cache_creation_input_tokens":3,"cache_read_input_tokens":7}}}}}}
' "$i";;
esac
case "$prompt" in *SLEEP*) sleep 60;; esac
case "$prompt" in *NORESULT*) exit 0;; esac
printf '{{"type":"result","subtype":"success","is_error":%s,"session_id":"%s","usage":{{"input_tokens":3,"output_tokens":7,"cache_creation_input_tokens":10,"cache_read_input_tokens":20}},"modelUsage":{{"m":{{"inputTokens":%s,"outputTokens":%s,"cacheReadInputTokens":%s,"cacheCreationInputTokens":%s}}}},"total_cost_usd":0.01}}\n' "$err" "$sid" "$i" "$o" "$r" "$c"
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
        "cred": cred, "claude": fake_claude(root), "quota_group_id": "test-shared", "checkpoint_central": "local_only", "budget": "02-04", "share": share(role)});
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
    let _serial = serial();
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
    let _serial = serial();
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
    let _serial = serial();
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
    let _serial = serial();
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
    let _serial = serial();
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
    recorded(
        caller::call(
            &p,
            &call_of("development", "reconnect", "d2", "s8", "more"),
            &a,
            POLL,
        )
        .expect("c"),
    );
    // the checkpoint names an older known run, not the newest run of the session
    let cp = t.0.join("development/state/checkpoint.json");
    let mut v: Value = serde_json::from_slice(&std::fs::read(&cp).expect("cp")).expect("j");
    v["session_cumulative_attempt"] = json!("d1");
    std::fs::write(&cp, serde_json::to_vec(&v).expect("j")).expect("w");
    let r = recorded(
        caller::call(
            &p,
            &call_of("development", "reconnect", "d3", "s9", "more"),
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
    // an unmeasured run asks for a person (and would stop the budget)
    assert_eq!(r["stop_required"], true);
    assert!(
        r["stop_reason"]
            .as_str()
            .is_some_and(|x| x.contains("usage unmeasured")),
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
    // a checkpoint naming a run the caller does not know (it survives restarts)
    let t = Tmp::new("base-unknown");
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
    let cp = t.0.join("development/state/checkpoint.json");
    let mut v: Value = serde_json::from_slice(&std::fs::read(&cp).expect("cp")).expect("j");
    v["session_cumulative_attempt"] = json!("elsewhere");
    std::fs::write(&cp, serde_json::to_vec(&v).expect("j")).expect("w");
    let why = not_started(
        caller::call(
            &p,
            &call_of("development", "reconnect", "d2", "s9", "more"),
            &a,
            POLL,
        )
        .expect("c"),
    );
    assert!(why.contains("checkpoint names attempt elsewhere"), "{why}");
}

#[test]
fn plans_and_records_are_checked() {
    let _serial = serial();
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
    let _serial = serial();
    let t = Tmp::new("http");
    let svc_cfg = {
        let s = service(&t.0, "development", None);
        drop(s);
        let r = t.0.join("development");
        Config::from_json(
            &json!({"role": "development", "repo": "sori", "state": r.join("state"),
            "etc": r.join("etc"), "cred": r.join("cred"), "claude": t.0.join("fake-claude"),
            "quota_group_id": "test-shared", "checkpoint_central": "local_only", "budget": "02-04", "share": share("development")}),
        )
        .expect("cfg")
    };
    // a free port: serve's own bind error is reported, and another port is tried
    let mut cfg = Some(svc_cfg);
    let mut port = 0;
    for _ in 0..5 {
        port = std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|l| l.local_addr())
            .expect("port")
            .port();
        let (tx, rx) = std::sync::mpsc::channel();
        let c = cfg.take().expect("cfg");
        let keep = c.clone();
        std::thread::spawn(move || {
            let _ = tx.send(ieo_supervisor::serve::serve(
                c,
                &format!("127.0.0.1:{port}"),
            ));
        });
        let t0 = std::time::Instant::now();
        let up = loop {
            if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
                break true;
            }
            if let Ok(r) = rx.try_recv() {
                eprintln!("serve on {port} ended: {r:?}");
                break false;
            }
            assert!(
                t0.elapsed() < Duration::from_secs(10),
                "serve did not listen on {port}"
            );
            std::thread::sleep(Duration::from_millis(50));
        };
        if up {
            break;
        }
        cfg = Some(keep);
    }
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
    // 02-05 step 7b (review 185d12b L-2): the 02-04 host caller passes a person's recovery id
    // for a start over a run that did not finish
    std::fs::write(t.0.join("adapter.token"), TOKEN).expect("w");
    let cp_path = t.0.join("development/state/checkpoint.json");
    let mut cp: Value = serde_json::from_slice(&std::fs::read(&cp_path).expect("cp")).expect("j");
    cp["phase"] = json!("takeover");
    std::fs::write(&cp_path, serde_json::to_vec_pretty(&cp).expect("j")).expect("w");
    let plan_v = json!({"budget": "02-04", "ledger_dir": t.0.join("ledger"),
        "first_observation": {"stream": t.0.join("first-observation.jsonl"),
                              "observed_unix": now() - 3600},
        "roles": {"development": {"url": url, "adapter_token_file": t.0.join("adapter.token"),
                                  "share": share("development")}}});
    std::fs::write(t.0.join("plan.json"), plan_v.to_string()).expect("plan");
    std::fs::write(t.0.join("prompt.txt"), "hi").expect("prompt");
    std::fs::write(
        t.0.join("coordinator.json"),
        call_of("development", "start", "x", "x", "x")
            .coordinator
            .to_string(),
    )
    .expect("coordinator");
    let cli = |attempt: &str, session: &str, recovery: Option<&str>| {
        let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_ieo-role-call"));
        cmd.arg("--plan")
            .arg(t.0.join("plan.json"))
            .args(["--task", "02-04", "--role", "development", "--op", "start"])
            .args(["--attempt", attempt, "--session", session])
            .arg("--prompt-file")
            .arg(t.0.join("prompt.txt"))
            .arg("--coordinator-file")
            .arg(t.0.join("coordinator.json"));
        if let Some(r) = recovery {
            cmd.args(["--recovery-id", r]);
        }
        let out = cmd.output().expect("ieo-role-call");
        (
            out.status.code(),
            String::from_utf8_lossy(&out.stdout).to_string(),
        )
    };
    let (code, out) = cli("d3", "s3", None);
    assert_eq!(code, Some(2), "{out}");
    assert!(out.contains("recovery_id"), "{out}");
    let (code, out) = cli("d3", "s3", Some("../x"));
    assert_eq!(code, Some(1), "{out}");
    // the spare flags (02-05 budget, user decision 2026-11-17 (나)): both or neither; this
    // 02-04 budget has no spare, so the marker is refused before anything starts
    let spare_cli = |args: &[&str]| {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_ieo-role-call"))
            .arg("--plan")
            .arg(t.0.join("plan.json"))
            .args(["--task", "02-04", "--role", "development", "--op", "start"])
            .args([
                "--attempt",
                "d3",
                "--session",
                "s3",
                "--recovery-id",
                "R-cli-1",
            ])
            .arg("--prompt-file")
            .arg(t.0.join("prompt.txt"))
            .arg("--coordinator-file")
            .arg(t.0.join("coordinator.json"))
            .args(args)
            .output()
            .expect("ieo-role-call");
        (
            out.status.code(),
            String::from_utf8_lossy(&out.stdout).to_string(),
        )
    };
    let (code, out) = spare_cli(&["--spare-reason", "env"]);
    assert_eq!(code, Some(1), "{out}");
    let (code, out) = spare_cli(&["--spare-reason", "env", "--replaces", "d1"]);
    assert_eq!(code, Some(2), "{out}");
    assert!(
        out.contains("has no spare run in the 02-04 budget"),
        "{out}"
    );
    let (code, out) = cli("d3", "s3", Some("R-cli-1"));
    assert_eq!(code, Some(0), "{out}");
    let cp: Value = serde_json::from_slice(&std::fs::read(&cp_path).expect("cp")).expect("j");
    assert_eq!(
        (cp["phase"].as_str(), cp["recovery_id"].as_str()),
        (Some("after_run"), Some("R-cli-1")),
        "{cp}"
    );
}

// --- review c4620d2 ---

type HookFn = dyn Fn(&str, &str, Result<(u16, Value), String>) -> Result<(u16, Value), String>;

/// Wraps the in-process transport and rewrites (or breaks) answers.
struct Hook<'a> {
    inner: &'a InProc,
    f: Box<HookFn>,
}
impl Adapter for Hook<'_> {
    fn send(
        &self,
        role: &RoleTarget,
        method: &str,
        path: &str,
        body: Option<&Value>,
    ) -> Result<(u16, Value), String> {
        (self.f)(method, path, self.inner.send(role, method, path, body))
    }
}

fn hook<'a>(
    inner: &'a InProc,
    f: impl Fn(&str, &str, Result<(u16, Value), String>) -> Result<(u16, Value), String> + 'static,
) -> Hook<'a> {
    Hook {
        inner,
        f: Box::new(f),
    }
}

/// Rewrites the finished attempt in status answers.
fn last_rewritten(inner: &InProc, f: impl Fn(&mut Value) + 'static) -> Hook<'_> {
    hook(inner, move |m, p, r| {
        let mut r = r?;
        if m == "GET"
            && p == "/v0/role/status"
            && r.1["running"].is_null()
            && r.1["last"].is_object()
        {
            f(&mut r.1["last"]);
        }
        Ok(r)
    })
}

#[test]
fn the_environment_the_stop_and_the_guard_observation_decide_the_charge() {
    let _serial = serial();
    let t = Tmp::new("authority");
    let (a, p) = both(&t);
    // an API-key variable in the run's environment: a person looks
    let h = last_rewritten(&a, |l| {
        if let Some(v) = l["env_names"].as_array_mut() {
            v.push(json!("ANTHROPIC_API_KEY"));
        }
    });
    let r = recorded(
        caller::call(
            &p,
            &call_of("development", "start", "d1", "s1", "hi"),
            &h,
            POLL,
        )
        .expect("c"),
    );
    assert_eq!(r["stop_required"], true);
    assert!(
        r["stop_reason"]
            .as_str()
            .is_some_and(|s| s.contains("environment")),
        "{r}"
    );
    // a guard-stopped run is unmeasured even when the collector measured it
    let t = Tmp::new("authority2");
    let (a, p) = both(&t);
    let h = last_rewritten(&a, |l| l["stop"] = json!("RunTimeout"));
    let r = recorded(
        caller::call(
            &p,
            &call_of("development", "start", "d1", "s1", "hi"),
            &h,
            POLL,
        )
        .expect("c"),
    );
    assert_eq!(
        (
            r["usage"]["collector_run"]["state"].as_str(),
            r["usage"]["run"]["state"].as_str()
        ),
        (Some("measured"), Some("unmeasured")),
        "{}",
        r["usage"]
    );
    // a missing guard observation is not zero
    let t = Tmp::new("authority3");
    let (a, p) = both(&t);
    let h = last_rewritten(&a, |l| l["observed"] = Value::Null);
    let r = recorded(
        caller::call(
            &p,
            &call_of("development", "start", "d1", "s1", "hi"),
            &h,
            POLL,
        )
        .expect("c"),
    );
    assert_eq!(r["usage"]["run"]["state"], "unmeasured", "{}", r["usage"]);
}

#[test]
fn a_failed_result_asks_for_a_person() {
    let _serial = serial();
    // M1: e.g. an authentication failure on the first call of a role
    let t = Tmp::new("authfail");
    let (a, p) = both(&t);
    let r = recorded(
        caller::call(
            &p,
            &call_of("development", "start", "d1", "s1", "AUTHFAIL"),
            &a,
            POLL,
        )
        .expect("c"),
    );
    assert_eq!(r["stop_required"], true);
    assert!(
        r["stop_reason"]
            .as_str()
            .is_some_and(|s| s.contains("is_error true")),
        "{r}"
    );
    assert_eq!(r["outcome"]["result"]["is_error"], true);
}

#[test]
fn the_role_share_bounds_runs_and_time() {
    let _serial = serial();
    let t = Tmp::new("share");
    let mut m = BTreeMap::new();
    m.insert("dev".to_string(), service(&t.0, "development", None));
    let a = InProc(m);
    std::fs::write(t.0.join("adapter.token"), TOKEN).expect("tok");
    std::fs::write(t.0.join("first.jsonl"), rate("rejected")).expect("obs");
    let v = json!({"budget": "02-04", "ledger_dir": t.0.join("ledger"),
        "first_observation": {"stream": t.0.join("first.jsonl"), "observed_unix": now() - 60},
        "roles": {"development": {"url": "dev", "adapter_token_file": t.0.join("adapter.token"),
            "share": {"max_runs": 1, "total_seconds": 700, "billable_tokens": 1000, "cache_read_tokens": 1000}}}});
    let p = Plan::from_json(&v).expect("plan");
    let l = caller::limits_for(&p, "development", &[]).expect("limits");
    assert_eq!(
        (l.timeout + l.grace, l.timeout_is_total, l.billable_room),
        (Duration::from_secs(700), true, 1000)
    );
    recorded(
        caller::call(
            &p,
            &call_of("development", "start", "d1", "s1", "hi"),
            &a,
            POLL,
        )
        .expect("c"),
    );
    let why = not_started(
        caller::call(
            &p,
            &call_of("development", "start", "d2", "s2", "hi"),
            &a,
            POLL,
        )
        .expect("c"),
    );
    assert!(why.contains("1 of 1 runs"), "{why}");
}

#[test]
fn a_running_role_or_an_unknown_last_attempt_starts_nothing() {
    let _serial = serial();
    let t = Tmp::new("before");
    let (a, p) = both(&t);
    // only the first status answer says "running" (a missing check must not hang the test)
    let first = std::sync::atomic::AtomicBool::new(true);
    let h = hook(&a, move |m, p, r| {
        let mut r = r?;
        if m == "GET"
            && p == "/v0/role/status"
            && first.swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            r.1["running"] = json!({"attempt_id": "elsewhere"});
        }
        Ok(r)
    });
    assert!(
        not_started(
            caller::call(
                &p,
                &call_of("development", "start", "d1", "s1", "hi"),
                &h,
                POLL
            )
            .expect("c")
        )
        .contains("running")
    );
    // the service ran an attempt this ledger does not know
    recorded(
        caller::call(
            &p,
            &call_of("development", "start", "d1", "s1", "hi"),
            &a,
            POLL,
        )
        .expect("c"),
    );
    let mut v = json!({"budget": "02-04", "ledger_dir": t.0.join("other-ledger"), "roles": {
        "development": {"url": "dev", "adapter_token_file": t.0.join("adapter.token"), "share": share("development")}}});
    v["first_observation"] =
        json!({"stream": t.0.join("first-observation.jsonl"), "observed_unix": now() - 60});
    let other = Plan::from_json(&v).expect("plan");
    let why = not_started(
        caller::call(
            &other,
            &call_of("development", "reconnect", "d2", "s2", "hi"),
            &a,
            POLL,
        )
        .expect("c"),
    );
    assert!(why.contains("d1 has no caller record"), "{why}");
}

#[test]
fn every_failure_after_the_start_is_recorded_and_blocks() {
    let _serial = serial();
    // H1: the start's answer lost (the run did start)
    let t = Tmp::new("lost-post");
    let (a, p) = both(&t);
    let h = hook(&a, |m, _p, r| {
        if m == "POST" {
            r.and(Err("connection reset".into()))
        } else {
            r
        }
    });
    let r = recorded(
        caller::call(
            &p,
            &call_of("development", "start", "d1", "s1", "hi"),
            &h,
            POLL,
        )
        .expect("c"),
    );
    assert_eq!(
        (
            r["stop_required"].as_bool(),
            r["usage"]["run"]["state"].as_str()
        ),
        (Some(true), Some("unmeasured")),
        "{r}"
    );
    assert!(r["duration_ms"].is_null());
    assert!(
        !caller::records_path(&p)
            .with_file_name("inflight.json")
            .exists()
    );
    assert!(
        not_started(
            caller::call(
                &p,
                &call_of("guardrail", "start", "g1", "sg", "x"),
                &a,
                POLL
            )
            .expect("c")
        )
        .contains("asks for a person")
    );
    // the stdout fetch fails
    let t = Tmp::new("lost-stdout");
    let (a, p) = both(&t);
    let h = hook(&a, |_m, path, r| {
        if path.ends_with("/stdout") {
            Err("timeout".into())
        } else {
            r
        }
    });
    let r = recorded(
        caller::call(
            &p,
            &call_of("development", "start", "d1", "s1", "hi"),
            &h,
            POLL,
        )
        .expect("c"),
    );
    assert!(
        r["stop_reason"]
            .as_str()
            .is_some_and(|s| s.contains("stdout fetch")),
        "{r}"
    );
    // the status polls fail until the deadline: a stop, then the run is given up as unknown
    let t = Tmp::new("lost-status");
    let (a, _) = both(&t);
    let mut v = json!({"budget": "02-04", "ledger_dir": t.0.join("ledger"), "roles": {
        "development": {"url": "dev", "adapter_token_file": t.0.join("adapter.token"),
                        "share": share("development"), "max_window_seconds": 7}}});
    v["first_observation"] =
        json!({"stream": t.0.join("first-observation.jsonl"), "observed_unix": now() - 60});
    let p = Plan::from_json(&v).expect("plan");
    let polls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let n = polls.clone();
    let h = hook(&a, move |m, path, r| {
        if m == "GET"
            && path == "/v0/role/status"
            && n.fetch_add(1, std::sync::atomic::Ordering::SeqCst) > 0
        {
            return Err("unreachable".into());
        }
        r
    });
    let t0 = std::time::Instant::now();
    let r = recorded(
        caller::call_with(
            &p,
            &call_of("development", "start", "d1", "s1", "hi"),
            &h,
            POLL,
            Duration::from_millis(300),
        )
        .expect("c"),
    );
    assert!(
        r["stop_reason"]
            .as_str()
            .is_some_and(|s| s.contains("did not end")),
        "{r}"
    );
    assert!(t0.elapsed() < Duration::from_secs(15));
}

#[test]
fn a_call_left_in_flight_blocks_until_reconciled() {
    let _serial = serial();
    // H1: a caller that died after its start request leaves the marker behind
    let t = Tmp::new("inflight");
    let (a, p) = both(&t);
    std::fs::create_dir_all(t.0.join("ledger")).expect("d");
    std::fs::write(
        t.0.join("ledger/inflight.json"),
        "{\"attempt_id\":\"d0\"}\n",
    )
    .expect("w");
    let why = not_started(
        caller::call(
            &p,
            &call_of("development", "start", "d1", "s1", "hi"),
            &a,
            POLL,
        )
        .expect("c"),
    );
    assert!(why.contains("in flight") && why.contains("d0"), "{why}");
    assert!(!t.0.join("development/state/runs/d1").exists());
}

#[test]
fn an_observation_from_the_future_holds_the_call() {
    let _serial = serial();
    // L1: a clock behind the observation would make it look fresh
    let t = Tmp::new("clock");
    let mut m = BTreeMap::new();
    m.insert("dev".to_string(), service(&t.0, "development", None));
    let a = InProc(m);
    std::fs::write(t.0.join("adapter.token"), TOKEN).expect("tok");
    std::fs::write(t.0.join("first.jsonl"), rate("rejected")).expect("obs");
    let v = json!({"budget": "02-04", "ledger_dir": t.0.join("ledger"),
        "first_observation": {"stream": t.0.join("first.jsonl"), "observed_unix": now() + 3600},
        "roles": {"development": {"url": "dev", "adapter_token_file": t.0.join("adapter.token"), "share": share("development")}}});
    let p = Plan::from_json(&v).expect("plan");
    let why = not_started(
        caller::call(
            &p,
            &call_of("development", "start", "d1", "s1", "hi"),
            &a,
            POLL,
        )
        .expect("c"),
    );
    assert!(why.contains("clock"), "{why}");
}

// --- review f90f5b5 ---

#[test]
fn a_refused_start_clears_the_marker_and_records_nothing() {
    let _serial = serial();
    let t = Tmp::new("refused");
    let (a, p) = both(&t);
    // the service refuses before anything runs (an unsafe session id)
    let why = not_started(
        caller::call(
            &p,
            &call_of("development", "start", "d1", "-bad", "hi"),
            &a,
            POLL,
        )
        .expect("c"),
    );
    assert!(why.contains("SCHEMA_VIOLATION"), "{why}");
    assert!(!t.0.join("ledger/inflight.json").exists());
    assert!(caller::records(&p).expect("r").is_empty());
    // the next call is free to start
    recorded(
        caller::call(
            &p,
            &call_of("development", "start", "d2", "s2", "hi"),
            &a,
            POLL,
        )
        .expect("c"),
    );
}

#[test]
fn a_run_without_a_result_or_with_an_unsaved_stream_asks_for_a_person() {
    let _serial = serial();
    let t = Tmp::new("noresult");
    let (a, p) = both(&t);
    let r = recorded(
        caller::call(
            &p,
            &call_of("development", "start", "d1", "s1", "NORESULT"),
            &a,
            POLL,
        )
        .expect("c"),
    );
    assert!(
        r["stop_reason"]
            .as_str()
            .is_some_and(|x| x.contains("no result event")),
        "{r}"
    );
    // the stream cannot be saved (a file where the streams directory goes)
    let t = Tmp::new("nosave");
    let (a, p) = both(&t);
    std::fs::create_dir_all(t.0.join("ledger")).expect("d");
    std::fs::write(t.0.join("ledger/streams"), "not a directory").expect("w");
    let r = recorded(
        caller::call(
            &p,
            &call_of("development", "start", "d1", "s1", "hi"),
            &a,
            POLL,
        )
        .expect("c"),
    );
    assert!(
        r["stop_reason"]
            .as_str()
            .is_some_and(|x| x.contains("stream save")),
        "{r}"
    );
    assert!(
        !t.0.join("ledger/inflight.json").exists(),
        "the failure is recorded, so the marker goes"
    );
}

#[test]
fn a_second_caller_is_refused() {
    let _serial = serial();
    let t = Tmp::new("lock");
    let (a, p) = both(&t);
    std::fs::create_dir_all(t.0.join("ledger")).expect("d");
    let held = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(t.0.join("ledger/caller.lock"))
        .expect("lock");
    held.lock().expect("held");
    let why = not_started(
        caller::call(
            &p,
            &call_of("development", "start", "d1", "s1", "hi"),
            &a,
            POLL,
        )
        .expect("c"),
    );
    assert!(why.contains("another caller"), "{why}");
    assert!(!t.0.join("development/state/runs/d1").exists());
}

// --- 02-04 D3: children's totals from their transcripts; re-evaluation ---

#[test]
fn a_resumed_run_with_a_child_is_measured_from_the_child_transcript() {
    let _serial = serial();
    let t = Tmp::new("agent");
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
    let r = recorded(
        caller::call(
            &p,
            &call_of("development", "reconnect", "d2", "s9", "AGENTRUN"),
            &a,
            POLL,
        )
        .expect("c"),
    );
    assert_eq!(r["stop_required"], false, "{r}");
    assert_eq!(
        r["usage"]["attribution_check"], "consistent",
        "{}",
        r["usage"]
    );
    // parent 3/7/20/10 + child 2/7/7/10 (both responses), not the last response 1/2/7/3
    assert_eq!(
        r["usage"]["run"]["tokens"],
        json!({"input": 5, "output": 14, "cache_read": 27, "cache_creation": 20})
    );
    assert_eq!(
        r["usage"]["children"][0]["usage"]["tokens"],
        json!({"input": 2, "output": 7, "cache_read": 7, "cache_creation": 10})
    );
    // the transcript is kept in the private ledger with its digest
    let kept = r["children"][0]["path"].as_str().expect("kept");
    assert!(kept.starts_with(t.0.join("ledger/children/d2").to_str().expect("s")));
    assert!(r["children"][0]["sha256"].is_string());
}

/// A run whose child transcript could not be fetched: unmeasured, recorded, a person decides.
fn unmeasured_child_run(t: &Tmp) -> (InProc, Plan) {
    let (a, p) = both(t);
    recorded(
        caller::call(
            &p,
            &call_of("development", "start", "d1", "s1", "hi"),
            &a,
            POLL,
        )
        .expect("c"),
    );
    let h = hook(&a, |_m, path, r| {
        if path.contains("/children/") {
            Ok((404, json!({"code": "NOT_FOUND"})))
        } else {
            r
        }
    });
    let r = recorded(
        caller::call(
            &p,
            &call_of("development", "reconnect", "d2", "s9", "AGENTRUN"),
            &h,
            POLL,
        )
        .expect("c"),
    );
    assert_eq!(r["usage"]["run"]["state"], "unmeasured", "{}", r["usage"]);
    assert!(
        r["stop_reason"]
            .as_str()
            .is_some_and(|x| x.starts_with("usage unmeasured")),
        "{r}"
    );
    (a, p)
}

#[test]
fn a_run_is_corrected_once_from_its_kept_records_and_not_counted_again() {
    let _serial = serial();
    let t = Tmp::new("reeval");
    let (a, p) = unmeasured_child_run(&t);
    // blocked until the decision
    assert!(
        not_started(
            caller::call(
                &p,
                &call_of("guardrail", "start", "g1", "sg", "x"),
                &a,
                POLL
            )
            .expect("c")
        )
        .contains("asks for a person")
    );
    let src =
        t.0.join("development/state/runs/d2/children/agent-c1.jsonl");
    let mut tr = std::collections::BTreeMap::new();
    // the last response only does not reconcile: nothing is written
    let last_only = t.0.join("last-only.jsonl");
    let full = std::fs::read_to_string(&src).expect("kept by the service");
    std::fs::write(&last_only, full.lines().nth(1).expect("r2")).expect("w");
    tr.insert("c1".to_string(), last_only);
    assert!(caller::reevaluate(&p, "d2", &tr, "decision-x").is_err());
    assert_eq!(caller::raw_records(&p).expect("r").len(), 2);
    assert!(
        !t.0.join("ledger/children/d2").exists(),
        "a refusal copies nothing"
    );
    // the full transcript does
    tr.insert("c1".to_string(), src);
    let c = caller::reevaluate(&p, "d2", &tr, "decision-x").expect("correct");
    assert_eq!(
        c["after"]["usage"]["run"]["tokens"],
        json!({"input": 5, "output": 14, "cache_read": 27, "cache_creation": 20})
    );
    assert_eq!(c["before"]["usage_run"]["state"], "unmeasured");
    // the original line stays; the effective record is corrected; runs are not counted again
    let raw = caller::raw_records(&p).expect("raw");
    assert_eq!(raw.len(), 3);
    assert_eq!(raw[1]["usage"]["run"]["state"], "unmeasured");
    let recs = caller::records(&p).expect("eff");
    assert_eq!(recs.len(), 2);
    assert_eq!(recs[1]["corrected_by"]["decision"], "decision-x");
    let s = budget::spent(&recs);
    assert_eq!(
        (s.runs, s.unmeasured_runs.len(), s.billable_tokens),
        (2, 0, 20 + 39)
    );
    // once only
    assert!(
        caller::reevaluate(&p, "d2", &tr, "decision-y")
            .expect_err("twice")
            .contains("already")
    );
    // and the series continues: the next resume takes the corrected run as its base
    let r = recorded(
        caller::call(
            &p,
            &call_of("development", "reconnect", "d3", "s8", "more"),
            &a,
            POLL,
        )
        .expect("c"),
    );
    assert_eq!(
        (
            r["session"]["resume_base_attempt"].as_str(),
            r["usage"]["run"]["state"].as_str()
        ),
        (Some("d2"), Some("measured")),
        "{r}"
    );
}

#[test]
fn a_correction_must_match_the_run_it_corrects() {
    let _serial = serial();
    let t = Tmp::new("reeval-bad");
    let (_a, p) = unmeasured_child_run(&t);
    let mut tr = std::collections::BTreeMap::new();
    tr.insert(
        "c1".to_string(),
        t.0.join("development/state/runs/d2/children/agent-c1.jsonl"),
    );
    // a changed stream file
    let rec = &caller::raw_records(&p).expect("r")[1];
    let sp = rec["stream"]["path"].as_str().expect("p").to_string();
    let orig = std::fs::read(&sp).expect("s");
    std::fs::write(&sp, [orig.clone(), b"\n".to_vec()].concat()).expect("w");
    assert!(
        caller::reevaluate(&p, "d2", &tr, "x")
            .expect_err("digest")
            .contains("digest")
    );
    std::fs::write(&sp, &orig).expect("w");
    // a run that is not stopped for usage alone, or an unknown run
    assert!(caller::reevaluate(&p, "d1", &tr, "x").is_err());
    assert!(caller::reevaluate(&p, "zz", &tr, "x").is_err());
    // forged correction lines make the ledger unusable
    let path = caller::records_path(&p);
    let good = std::fs::read_to_string(&path).expect("l");
    let forged = json!({"format": caller::CORRECTION_FORMAT, "corrects": "nope", "original_stream_sha256": "x",
                        "after": {"usage": {}, "stop_required": false}});
    std::fs::write(&path, format!("{good}{forged}\n")).expect("w");
    assert!(caller::records(&p).is_err());
    let wrong_digest = json!({"format": caller::CORRECTION_FORMAT, "corrects": "d2", "original_stream_sha256": "x",
                              "after": {"usage": {}, "stop_required": false}});
    std::fs::write(&path, format!("{good}{wrong_digest}\n")).expect("w");
    assert!(caller::records(&p).is_err());
}

// --- review 608b367 ---

fn rewrite_line(path: &std::path::Path, attempt: &str, f: impl Fn(&mut Value)) {
    let text = std::fs::read_to_string(path).expect("ledger");
    let out: String = text
        .lines()
        .map(|l| {
            let mut v: Value = serde_json::from_str(l).expect("line");
            if v["attempt_id"] == attempt || v["corrects"] == attempt {
                f(&mut v);
            }
            format!("{v}\n")
        })
        .collect();
    std::fs::write(path, out).expect("w");
}

#[test]
fn a_run_stopped_for_more_than_its_usage_is_not_corrected() {
    // F2: another reason besides the usage keeps the stop
    let _serial = serial();
    let t = Tmp::new("reeval-reasons");
    let (_a, p) = unmeasured_child_run(&t);
    rewrite_line(&caller::records_path(&p), "d2", |v| {
        let r = v["stop_reason"].as_str().unwrap_or_default().to_string();
        v["stop_reason"] = json!(format!("{r}; process-group cleanup failed"));
    });
    let mut tr = std::collections::BTreeMap::new();
    tr.insert(
        "c1".to_string(),
        t.0.join("development/state/runs/d2/children/agent-c1.jsonl"),
    );
    assert!(
        caller::reevaluate(&p, "d2", &tr, "x")
            .expect_err("reasons")
            .contains("only for unmeasured usage")
    );
}

#[test]
fn a_correction_is_measured_again_when_read() {
    // F1: an edited correction, or a changed transcript behind it, makes the ledger unusable
    let _serial = serial();
    let t = Tmp::new("reeval-edit");
    let (_a, p) = unmeasured_child_run(&t);
    let src =
        t.0.join("development/state/runs/d2/children/agent-c1.jsonl");
    let kept = t.0.join("kept-c1.jsonl");
    std::fs::copy(&src, &kept).expect("copy");
    let mut tr = std::collections::BTreeMap::new();
    tr.insert("c1".to_string(), kept.clone());
    caller::reevaluate(&p, "d2", &tr, "decision-x").expect("correct");
    assert!(caller::records(&p).is_ok());
    let path = caller::records_path(&p);
    let good = std::fs::read_to_string(&path).expect("l");
    // the charge zeroed by hand
    rewrite_line(&path, "d2", |v| {
        if v["format"] == caller::CORRECTION_FORMAT {
            v["after"]["usage"]["run"]["tokens"] =
                json!({"input": 0, "output": 0, "cache_read": 0, "cache_creation": 0});
        }
    });
    assert!(caller::records(&p).expect_err("zeroed").contains("differs"));
    std::fs::write(&path, &good).expect("restore");
    // the transcript it names (the copy kept with the ledger, by absolute path) changed
    let in_ledger = t.0.join("ledger/children/d2/agent-c1.jsonl");
    assert!(in_ledger.is_file(), "the transcript goes with the ledger");
    let raw = caller::raw_records(&p).expect("raw");
    assert_eq!(raw[2]["child_transcripts"][0]["path"], json!(in_ledger));
    let orig = std::fs::read_to_string(&in_ledger).expect("k");
    std::fs::write(&in_ledger, orig.lines().nth(1).expect("r2")).expect("w");
    assert!(caller::records(&p).is_err());
    std::fs::write(&in_ledger, &orig).expect("restore");
    assert!(caller::records(&p).is_ok());
    // the source it was copied from no longer matters
    std::fs::remove_file(&kept).expect("rm");
    assert!(caller::records(&p).is_ok());
    // a relative path in a correction is refused (it would only work from one directory)
    rewrite_line(&path, "d2", |v| {
        if v["format"] == caller::CORRECTION_FORMAT {
            v["child_transcripts"][0]["path"] = json!("children/d2/agent-c1.jsonl");
        }
    });
    assert!(
        caller::records(&p)
            .expect_err("relative")
            .contains("not absolute")
    );
    std::fs::write(&path, &good).expect("restore");
    // the transcripts it records must be the ones measured
    rewrite_line(&path, "d2", |v| {
        if v["format"] == caller::CORRECTION_FORMAT {
            v["child_transcripts"][0]["sha256"] = json!("0".repeat(64));
        }
    });
    assert!(caller::records(&p).is_err());
    std::fs::write(&path, &good).expect("restore");
    // a decision left empty
    rewrite_line(&path, "d2", |v| {
        if v["format"] == caller::CORRECTION_FORMAT {
            v["decision"] = json!("");
        }
    });
    assert!(caller::records(&p).is_err());
    std::fs::write(&path, &good).expect("restore");
    assert!(caller::records(&p).is_ok());
}

#[test]
fn reevaluation_checks_the_decision_the_kept_digest_and_the_base_session() {
    let _serial = serial();
    let t = Tmp::new("reeval-checks");
    let (_a, p) = unmeasured_child_run(&t);
    let src =
        t.0.join("development/state/runs/d2/children/agent-c1.jsonl");
    let mut tr = std::collections::BTreeMap::new();
    tr.insert("c1".to_string(), src.clone());
    // N3: a blank decision on the run that could be corrected
    assert!(
        caller::reevaluate(&p, "d2", &tr, " ")
            .expect_err("blank")
            .contains("decision")
    );
    let path = caller::records_path(&p);
    let good = std::fs::read_to_string(&path).expect("l");
    // F4: the run recorded a different digest for its child transcript
    rewrite_line(&path, "d2", |v| {
        v["children"] = json!([{"agent_id": "c1", "sha256": "0".repeat(64)}])
    });
    assert!(
        caller::reevaluate(&p, "d2", &tr, "x")
            .expect_err("digest")
            .contains("differs from the one the run kept")
    );
    std::fs::write(&path, &good).expect("restore");
    // F3: the base belongs to another session
    rewrite_line(&path, "d1", |v| v["session"]["session_id"] = json!("other"));
    assert!(
        caller::reevaluate(&p, "d2", &tr, "x")
            .expect_err("base")
            .contains("another session")
    );
    std::fs::write(&path, &good).expect("restore");
    // and with everything in order it holds, from a relative source path too (stored absolute)
    let cwd = std::env::current_dir().expect("cwd");
    let rel = pathdiff(&src, &cwd);
    tr.insert("c1".to_string(), rel);
    let c = caller::reevaluate(&p, "d2", &tr, "x").expect("correct");
    assert!(
        std::path::Path::new(c["child_transcripts"][0]["path"].as_str().expect("p")).is_absolute()
    );
    assert!(caller::records(&p).is_ok());
}

/// `to` relative to `from` (both absolute), for a relative-path argument.
fn pathdiff(to: &Path, from: &Path) -> PathBuf {
    let t: Vec<_> = to.components().collect();
    let f: Vec<_> = from.components().collect();
    let common = t.iter().zip(&f).take_while(|(a, b)| a == b).count();
    let mut p = PathBuf::new();
    for _ in common..f.len() {
        p.push("..");
    }
    for c in &t[common..] {
        p.push(c);
    }
    p
}

// --- X-08: the 02-04 caller carry-overs (decision item 8) ---

fn mode(p: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).expect("meta").permissions().mode() & 0o777
}

/// A plan over `dev`/`gr` (both roles) with its ledger at `ledger`.
fn plan_at(t: &Tmp, ledger: &Path) -> Plan {
    let v = json!({"budget": "02-04", "ledger_dir": ledger,
        "first_observation": {"stream": t.0.join("first-observation.jsonl"), "observed_unix": now() - 60},
        "roles": {
            "development": {"url": "dev", "adapter_token_file": t.0.join("adapter.token"), "share": share("development")},
            "guardrail": {"url": "gr", "adapter_token_file": t.0.join("adapter.token"), "share": share("guardrail")}}});
    Plan::from_json(&v).expect("plan")
}

/// Review 71e408a P1 (and P3): a transcript for an id that is not a child of the run, or an id
/// that is not a plain name, is refused before anything is copied; a run's own stream naming
/// such a child gets no file. Ledger copies are private.
#[test]
fn transcripts_only_for_the_run_s_own_children_and_kept_privately() {
    let _serial = serial();
    let t = Tmp::new("x08-p1");
    let (_a, p) = unmeasured_child_run(&t);
    let src =
        t.0.join("development/state/runs/d2/children/agent-c1.jsonl");
    for extra in ["zz", "../../escape", "c1/../c1", ""] {
        let mut tr = BTreeMap::new();
        tr.insert("c1".to_string(), src.clone());
        tr.insert(extra.to_string(), src.clone());
        let e = caller::reevaluate(&p, "d2", &tr, "x").expect_err(extra);
        assert!(e.contains("not an agent id"), "{extra}: {e}");
        assert!(
            !t.0.join("ledger/children").exists(),
            "{extra}: a refusal copies nothing"
        );
        assert_eq!(caller::raw_records(&p).expect("r").len(), 2);
    }
    let mut tr = BTreeMap::new();
    tr.insert("c1".to_string(), src);
    caller::reevaluate(&p, "d2", &tr, "x").expect("correct");
    let copy = t.0.join("ledger/children/d2/agent-c1.jsonl");
    assert_eq!(
        (mode(&copy), mode(copy.parent().expect("d"))),
        (0o600, 0o700)
    );
    let stream = caller::raw_records(&p).expect("r")[1]["stream"]["path"]
        .as_str()
        .expect("p")
        .to_string();
    assert_eq!(mode(Path::new(&stream)), 0o600);

    // the run's own stream names an id with path separators: no file, the child unmeasured
    let t = Tmp::new("x08-p1-stream");
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
    let h = hook(&a, |_m, path, r| {
        let mut r = r?;
        if path.ends_with("/stdout")
            && let Some(c) = r.1["content"].as_str()
        {
            r.1["content"] = json!(c.replace(r#""agentId":"c1""#, r#""agentId":"../../esc""#));
        }
        Ok(r)
    });
    let r = recorded(
        caller::call(
            &p,
            &call_of("development", "reconnect", "d2", "s9", "AGENTRUN"),
            &h,
            POLL,
        )
        .expect("c"),
    );
    assert_eq!(r["children"][0]["refused"], "unsafe agent id", "{r}");
    assert_eq!(r["usage"]["run"]["state"], "unmeasured", "{}", r["usage"]);
    assert!(!t.0.join("ledger/children").exists());
    assert!(!t.0.join("esc.jsonl").exists());
}

/// Review 71e408a P2: a different transcript already at the copy location is refused even
/// when it would reconcile too (C4), and a ledger the append leaves unreadable is reported at
/// once by the read-back (C6).
#[test]
fn a_correction_refuses_a_different_copy_and_reads_the_ledger_back() {
    let _serial = serial();
    let t = Tmp::new("x08-p2");
    let (_a, p) = unmeasured_child_run(&t);
    let src =
        t.0.join("development/state/runs/d2/children/agent-c1.jsonl");
    let mut tr = BTreeMap::new();
    tr.insert("c1".to_string(), src.clone());
    // the same responses with one more blank line: it reconciles, but it is not the file given
    let copy = t.0.join("ledger/children/d2/agent-c1.jsonl");
    std::fs::create_dir_all(copy.parent().expect("d")).expect("d");
    let different = format!("{}\n", std::fs::read_to_string(&src).expect("s"));
    std::fs::write(&copy, &different).expect("w");
    let mut alt = BTreeMap::new();
    alt.insert("c1".to_string(), copy.clone());
    let probe = Tmp::new("x08-p2-probe");
    let (_b, q) = unmeasured_child_run(&probe);
    let probe_copy = probe.0.join("different.jsonl");
    std::fs::write(&probe_copy, &different).expect("w");
    let mut probe_tr = BTreeMap::new();
    probe_tr.insert("c1".to_string(), probe_copy);
    caller::reevaluate(&q, "d2", &probe_tr, "x").expect("the different file reconciles too");
    let e = caller::reevaluate(&p, "d2", &tr, "x").expect_err("different copy");
    assert!(e.contains("already holds a different transcript"), "{e}");
    assert_eq!(std::fs::read_to_string(&copy).expect("c"), different);
    assert_eq!(caller::raw_records(&p).expect("r").len(), 2);
    std::fs::remove_file(&copy).expect("rm");
    // a last line without its newline: the appended correction joins it, and the read-back
    // reports the broken ledger instead of the next call finding it
    let path = caller::records_path(&p);
    let text = std::fs::read_to_string(&path).expect("l");
    std::fs::write(&path, text.trim_end()).expect("w");
    let e = caller::reevaluate(&p, "d2", &tr, "x").expect_err("read-back");
    assert!(e.contains("unreadable"), "{e}");
}

/// Review 71e408a P4: the ledger names its files by absolute path; a moved ledger directory is
/// refused, and the refusal says why.
#[test]
fn a_moved_ledger_is_refused_with_its_cause() {
    let _serial = serial();
    let t = Tmp::new("x08-p4");
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
    std::fs::rename(t.0.join("ledger"), t.0.join("ledger-moved")).expect("mv");
    let moved = plan_at(&t, &t.0.join("ledger-moved"));
    let why = match caller::call(
        &moved,
        &call_of("development", "reconnect", "d2", "s2", "hi"),
        &a,
        POLL,
    ) {
        Ok(Called::NotStarted(w)) | Err(w) => w,
        Ok(Called::Recorded(r)) => panic!("started: {r}"),
    };
    assert!(why.contains("moved or renamed"), "{why}");
    // a correction's files the same way
    let t = Tmp::new("x08-p4-corr");
    let (_a, p) = unmeasured_child_run(&t);
    let mut tr = BTreeMap::new();
    tr.insert(
        "c1".to_string(),
        t.0.join("development/state/runs/d2/children/agent-c1.jsonl"),
    );
    caller::reevaluate(&p, "d2", &tr, "x").expect("correct");
    std::fs::rename(t.0.join("ledger"), t.0.join("ledger-moved")).expect("mv");
    let e = caller::records(&plan_at(&t, &t.0.join("ledger-moved"))).expect_err("moved");
    assert!(e.contains("moved or renamed"), "{e}");
}

/// Review dc11612 L-c3-2: the short lock wait lets a call through once an earlier holder lets
/// go (the in-process harness case it exists for); a holder past the wait is refused
/// (`a_second_caller_is_refused`).
#[test]
fn a_call_waits_briefly_for_a_released_lock() {
    let _serial = serial();
    let t = Tmp::new("x08-lockwait");
    let (a, p) = both(&t);
    std::fs::create_dir_all(t.0.join("ledger")).expect("d");
    let held = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(t.0.join("ledger/caller.lock"))
        .expect("lock");
    held.lock().expect("held");
    let release = caller::LOCK_WAIT / 3;
    let h = std::thread::spawn(move || {
        std::thread::sleep(release);
        drop(held);
    });
    let t0 = std::time::Instant::now();
    recorded(
        caller::call(
            &p,
            &call_of("development", "start", "d1", "s1", "hi"),
            &a,
            POLL,
        )
        .expect("c"),
    );
    assert!(t0.elapsed() >= release);
    h.join().expect("j");
}

/// Review dc11612 L-c3-3: a run of one role that this ledger does not know (a service's last
/// attempt, or the attempt its persisted checkpoint names after a restart) blocks a call of the
/// other role too.
#[test]
fn an_unknown_run_of_another_role_blocks_the_call() {
    let _serial = serial();
    let t = Tmp::new("x08-crossrole");
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
    // another ledger over the same services: d1 has no record there
    let other = plan_at(&t, &t.0.join("other-ledger"));
    let why = not_started(
        caller::call(
            &other,
            &call_of("guardrail", "start", "g1", "sg", "x"),
            &a,
            POLL,
        )
        .expect("c"),
    );
    assert!(
        why.contains("role development") && why.contains("last attempt d1 has no caller record"),
        "{why}"
    );
    // the development service restarted: no `last`, but its checkpoint still names d1
    let mut m = BTreeMap::new();
    m.insert("dev".to_string(), service(&t.0, "development", None));
    m.insert("gr".to_string(), service(&t.0, "guardrail", None));
    let restarted = InProc(m);
    let why = not_started(
        caller::call(
            &other,
            &call_of("guardrail", "start", "g1", "sg", "x"),
            &restarted,
            POLL,
        )
        .expect("c"),
    );
    assert!(
        why.contains("role development") && why.contains("checkpoint names attempt d1"),
        "{why}"
    );
    assert!(!t.0.join("guardrail/state/runs/g1").exists());
    // the ledger that knows d1 goes on
    recorded(
        caller::call(
            &p,
            &call_of("guardrail", "start", "g1", "sg", "x"),
            &restarted,
            POLL,
        )
        .expect("c"),
    );
}

/// Reviews 12c9a21 N6, dc11612 L-c3-3: `check` (what `call.sh` runs before a restart) refuses
/// what a call refuses, verifies corrections, and writes nothing.
#[test]
fn check_refuses_what_a_call_would_refuse() {
    let _serial = serial();
    let t = Tmp::new("x08-check");
    let (a, p) = both(&t);
    assert_eq!(caller::check(&p, &a).expect("c"), None);
    recorded(
        caller::call(
            &p,
            &call_of("development", "start", "d1", "s1", "hi"),
            &a,
            POLL,
        )
        .expect("c"),
    );
    assert_eq!(caller::check(&p, &a).expect("c"), None);
    let before = std::fs::read(caller::records_path(&p)).expect("r");
    // a call in flight
    std::fs::write(
        t.0.join("ledger/inflight.json"),
        "{\"attempt_id\":\"d2\"}\n",
    )
    .expect("w");
    let why = caller::check(&p, &a).expect("c").expect("refused");
    assert!(why.contains("in flight"), "{why}");
    std::fs::remove_file(t.0.join("ledger/inflight.json")).expect("rm");
    // another role's unknown run
    let other = plan_at(&t, &t.0.join("other-ledger"));
    let why = caller::check(&other, &a).expect("c").expect("refused");
    assert!(why.contains("d1 has no caller record"), "{why}");
    assert!(!t.0.join("other-ledger").exists(), "check writes nothing");
    // a correction that does not hold makes the ledger unreadable (call.sh trusted it)
    let forged = json!({"format": caller::CORRECTION_FORMAT, "corrects": "d1",
        "original_stream_sha256": "x", "after": {"usage": {}, "stop_required": false}});
    let path = caller::records_path(&p);
    std::fs::write(
        &path,
        [before.clone(), format!("{forged}\n").into_bytes()].concat(),
    )
    .expect("w");
    assert!(caller::check(&p, &a).is_err());
    // a record that asks for a person
    rewrite_line(&path, "d1", |v| {
        if v["format"] == caller::RECORD_FORMAT {
            v["stop_required"] = json!(true);
        }
    });
    std::fs::write(&path, {
        let t = std::fs::read_to_string(&path).expect("l");
        t.lines()
            .filter(|l| !l.contains(caller::CORRECTION_FORMAT))
            .map(|l| format!("{l}\n"))
            .collect::<String>()
    })
    .expect("w");
    let why = caller::check(&p, &a).expect("c").expect("refused");
    assert!(why.contains("asks for a person"), "{why}");
    // a running role
    let h = hook(&a, |m, p, r| {
        let mut r = r?;
        if m == "GET" && p == "/v0/role/status" {
            r.1["running"] = json!({"attempt_id": "x"});
        }
        Ok(r)
    });
    std::fs::write(&path, &before).expect("w");
    let why = caller::check(&p, &h).expect("c").expect("refused");
    assert!(why.contains("running"), "{why}");
}

/// Review dc11612 L-new-3 / 02-04 acceptance §4 item 8 ("마감 정지 시험"), part 1: a run that
/// does not end within its window is sent a stop at the caller's deadline (window + margin),
/// once, and the record says so.
#[test]
fn the_caller_sends_a_stop_at_its_deadline() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    let _serial = serial();
    let t = Tmp::new("x08-deadline");
    let mut m = BTreeMap::new();
    m.insert("dev".to_string(), service(&t.0, "development", None));
    let a = InProc(m);
    std::fs::write(t.0.join("adapter.token"), TOKEN).expect("tok");
    std::fs::write(t.0.join("first.jsonl"), rate("rejected")).expect("obs");
    let window = 7u64;
    let v = json!({"budget": "02-04", "ledger_dir": t.0.join("ledger"),
        "first_observation": {"stream": t.0.join("first.jsonl"), "observed_unix": now() - 60},
        "roles": {"development": {"url": "dev", "adapter_token_file": t.0.join("adapter.token"),
                                  "share": share("development"), "max_window_seconds": window}}});
    let p = Plan::from_json(&v).expect("plan");
    // after the start, the status says "running" until a stop is sent (at most 40 s, so a
    // caller that never sends one fails the test instead of hanging it)
    let started = Arc::new(AtomicBool::new(false));
    let stops: Arc<Mutex<Vec<std::time::Instant>>> = Arc::new(Mutex::new(Vec::new()));
    let (s1, s2) = (started.clone(), stops.clone());
    let t0 = std::time::Instant::now();
    let h = hook(&a, move |m, path, r| {
        let mut r = r?;
        if m == "POST" && path == "/v0/role/start" {
            s1.store(true, Ordering::SeqCst);
        }
        if m == "POST" && path == "/v0/role/stop" {
            s2.lock().expect("s").push(std::time::Instant::now());
        }
        if m == "GET"
            && path == "/v0/role/status"
            && s1.load(Ordering::SeqCst)
            && s2.lock().expect("s").is_empty()
            && t0.elapsed() < Duration::from_secs(40)
        {
            r.1["running"] = json!({"attempt_id": "d1"});
        }
        Ok(r)
    });
    let margin = Duration::from_millis(300);
    let r = recorded(
        caller::call_with(
            &p,
            &call_of("development", "start", "d1", "s1", "hi"),
            &h,
            POLL,
            margin,
        )
        .expect("c"),
    );
    let stops = stops.lock().expect("s").clone();
    assert_eq!(stops.len(), 1, "one stop");
    let at = stops[0] - t0;
    assert!(
        at >= Duration::from_secs(window) + margin && at < Duration::from_secs(window + 5),
        "the stop at the deadline, not before: {at:?}"
    );
    assert!(
        r["notes"].as_array().is_some_and(|n| n
            .iter()
            .any(|x| x.as_str() == Some("the caller sent a stop at its deadline"))),
        "{r}"
    );
}

/// Part 2: the role's run guard stops a run at its time limit (fake runtime), and the caller
/// records it as stopped by the guard, unmeasured, asking for a person; nothing further starts.
#[test]
fn a_run_at_its_time_limit_is_stopped_and_recorded() {
    let _serial = serial();
    let t = Tmp::new("x08-timelimit");
    let mut m = BTreeMap::new();
    m.insert("dev".to_string(), service(&t.0, "development", None));
    let a = InProc(m);
    std::fs::write(t.0.join("adapter.token"), TOKEN).expect("tok");
    std::fs::write(t.0.join("first.jsonl"), rate("rejected")).expect("obs");
    // window 8 s: the run gets 8 − 6 (grace) = 2 s
    let v = json!({"budget": "02-04", "ledger_dir": t.0.join("ledger"),
        "first_observation": {"stream": t.0.join("first.jsonl"), "observed_unix": now() - 60},
        "roles": {"development": {"url": "dev", "adapter_token_file": t.0.join("adapter.token"),
                                  "share": share("development"), "max_window_seconds": 8}}});
    let p = Plan::from_json(&v).expect("plan");
    let t0 = std::time::Instant::now();
    let r = recorded(
        caller::call(
            &p,
            &call_of("development", "start", "d1", "s1", "SLEEP"),
            &a,
            POLL,
        )
        .expect("c"),
    );
    assert!(t0.elapsed() < Duration::from_secs(20), "{:?}", t0.elapsed());
    assert_eq!(r["limits"]["timeout_ms"], 2_000, "{r}");
    assert_eq!(r["outcome"]["stop"], "RunTimeout", "{r}");
    assert_eq!(r["outcome"]["stopped_by_guard"], true, "{r}");
    let ms = r["duration_ms"].as_u64().expect("duration");
    assert!((2_000..8_000).contains(&ms), "{ms}");
    assert_eq!(r["usage"]["run"]["state"], "unmeasured", "{}", r["usage"]);
    assert_eq!(r["stop_required"], true);
    assert!(
        r["stop_reason"]
            .as_str()
            .is_some_and(|s| s.contains("usage unmeasured")),
        "{r}"
    );
    assert_eq!(caller::raw_records(&p).expect("r").len(), 1);
    assert!(!t.0.join("ledger/inflight.json").exists());
    let why = not_started(
        caller::call(
            &p,
            &call_of("development", "reconnect", "d2", "s2", "hi"),
            &a,
            POLL,
        )
        .expect("c"),
    );
    assert!(why.contains("asks for a person"), "{why}");
}

/// Review 71e408a P1, the part "a refusal leaves nothing behind" with more than one child: a
/// copy already made is removed again when a later one is refused.
#[test]
fn a_refused_correction_removes_the_copies_it_made() {
    let _serial = serial();
    let t = Tmp::new("x08-cleanup");
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
    // the measured case first: two children, both transcripts fetched, consistent
    let r = recorded(
        caller::call(
            &p,
            &call_of("development", "reconnect", "d2", "s9", "AGENTRUN AGENTTWO"),
            &a,
            POLL,
        )
        .expect("c"),
    );
    assert_eq!(
        r["usage"]["attribution_check"], "consistent",
        "{}",
        r["usage"]
    );
    // the same with the transcripts not fetched: unmeasured, a person decides
    let t = Tmp::new("x08-cleanup2");
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
    let h = hook(&a, |_m, path, r| {
        if path.contains("/children/") {
            Ok((404, json!({"code": "NOT_FOUND"})))
        } else {
            r
        }
    });
    let r = recorded(
        caller::call(
            &p,
            &call_of("development", "reconnect", "d2", "s9", "AGENTRUN AGENTTWO"),
            &h,
            POLL,
        )
        .expect("c"),
    );
    assert_eq!(r["usage"]["run"]["state"], "unmeasured", "{}", r["usage"]);
    let kept = t.0.join("development/state/runs/d2/children");
    let mut tr = BTreeMap::new();
    tr.insert("c1".to_string(), kept.join("agent-c1.jsonl"));
    tr.insert("c2".to_string(), kept.join("agent-c2.jsonl"));
    // c2's place in the ledger already holds another file: c1 is copied first, then refused
    let dir = t.0.join("ledger/children/d2");
    std::fs::create_dir_all(&dir).expect("d");
    std::fs::write(dir.join("agent-c2.jsonl"), "other\n").expect("w");
    let e = caller::reevaluate(&p, "d2", &tr, "x").expect_err("refused");
    assert!(
        e.contains("already holds a different transcript of c2"),
        "{e}"
    );
    assert!(
        !dir.join("agent-c1.jsonl").exists(),
        "the copy made before the refusal is removed"
    );
    assert_eq!(caller::raw_records(&p).expect("r").len(), 2);
    // with the place free, both are copied and the correction holds
    std::fs::remove_file(dir.join("agent-c2.jsonl")).expect("rm");
    caller::reevaluate(&p, "d2", &tr, "x").expect("correct");
    assert!(dir.join("agent-c1.jsonl").exists() && dir.join("agent-c2.jsonl").exists());
}

// --- 02-05 budget (user decision 2026-11-15 (나), §2.1) ---

/// A role service of the 02-05 budget: no share configured, the approved one applies.
fn service_0205(root: &Path, role: &str) -> Arc<Service> {
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
    let v = json!({"role": role, "repo": "sori", "state": r.join("state"), "etc": etc,
        "cred": cred, "claude": fake_claude(root), "quota_group_id": "test-shared",
        "checkpoint_central": "local_only", "budget": "02-05"});
    Arc::new(Service::new(Config::from_json(&v).expect("config")))
}

#[test]
fn the_0205_budget_fixes_the_role_shares_and_never_retries_the_guardrail() {
    let _serial = serial();
    let t = Tmp::new("b0205");
    std::fs::write(t.0.join("adapter.token"), TOKEN).expect("tok");
    let obs = t.0.join("first-observation.jsonl");
    std::fs::write(&obs, format!("{}\n", rate("rejected"))).expect("obs");
    let plan_v = |roles: Value| {
        json!({"budget": "02-05", "ledger_dir": t.0.join("ledger"),
               "first_observation": {"stream": obs, "observed_unix": now() - 3600},
               "roles": roles})
    };
    let role = |url: &str, share: Value| {
        let mut r = json!({"url": url, "adapter_token_file": t.0.join("adapter.token")});
        if !share.is_null() {
            r["share"] = share;
        }
        r
    };
    // the approved shares: omitted, or given exactly; anything else is refused
    let p = Plan::from_json(&plan_v(json!({
        "development": role("dev", Value::Null), "guardrail": role("gr", Value::Null)})))
    .expect("plan");
    let dev = p.roles["development"].share;
    let gr = p.roles["guardrail"].share;
    assert_eq!(
        (
            dev.max_runs,
            dev.total_seconds,
            dev.billable_tokens,
            dev.cache_read_tokens
        ),
        (3, 1_800, 150_000, 1_500_000)
    );
    assert_eq!(
        (
            gr.max_runs,
            gr.total_seconds,
            gr.billable_tokens,
            gr.cache_read_tokens
        ),
        (1, 600, 50_000, 500_000)
    );
    let exact = json!({"max_runs": 3, "total_seconds": 1_800, "billable_tokens": 150_000,
                       "cache_read_tokens": 1_500_000});
    assert!(Plan::from_json(&plan_v(json!({"development": role("dev", exact.clone())}))).is_ok());
    for (k, v) in [
        ("max_runs", 4),
        ("total_seconds", 2_400),
        ("billable_tokens", 150_001),
        ("cache_read_tokens", 1),
    ] {
        let mut more = exact.clone();
        more[k] = json!(v);
        let e = Plan::from_json(&plan_v(json!({"development": role("dev", more)})))
            .expect_err("a share other than the approved one");
        assert!(e.contains("approved share"), "{k}: {e}");
    }
    // the guardrail may not take the development share
    let e = Plan::from_json(&plan_v(json!({"guardrail": role("gr", exact.clone())})))
        .expect_err("guardrail with the development share");
    assert!(e.contains("approved share"), "{e}");
    // the role services take the approved share too; a configured other one is refused
    let e = Config::from_json(&json!({"role": "guardrail", "repo": "sori",
        "state": t.0.join("x"), "etc": t.0.join("x"), "cred": t.0.join("x"), "claude": "x",
        "quota_group_id": "q", "checkpoint_central": "local_only", "budget": "02-05",
        "share": {"max_runs": 2, "total_seconds": 600, "billable_tokens": 50_000,
                  "cache_read_tokens": 500_000}}))
    .expect_err("a guardrail share of 2 runs");
    assert!(e.contains("approved share"), "{e}");
    // the per-run window (timeout + grace) is 600 s; the rooms are the role's shares
    let l = caller::limits_for(&p, "development", &[]).expect("limits");
    assert_eq!(
        (
            (l.timeout + l.grace).as_secs(),
            l.billable_room,
            l.cache_read_room
        ),
        (600, 150_000, 1_500_000)
    );
    let l = caller::limits_for(&p, "guardrail", &[]).expect("limits");
    assert_eq!(
        (
            (l.timeout + l.grace).as_secs(),
            l.billable_room,
            l.cache_read_room
        ),
        (600, 50_000, 500_000)
    );
    // runs: development its 2 planned runs, then without a spare marker no 3rd (user decision
    // 2026-11-17 (나)); the guardrail 1, never retried
    let mut m = BTreeMap::new();
    m.insert("dev".to_string(), service_0205(&t.0, "development"));
    m.insert("gr".to_string(), service_0205(&t.0, "guardrail"));
    let a = InProc(m);
    for (i, attempt) in ["d1", "d2"].iter().enumerate() {
        let r = recorded(
            caller::call(
                &p,
                &call_of("development", "start", attempt, &format!("s{i}"), "hi"),
                &a,
                POLL,
            )
            .expect("call"),
        );
        assert_eq!(r["budget"], "02-05", "{r}");
    }
    let why = not_started(
        caller::call(
            &p,
            &call_of("development", "start", "d3", "s3", "hi"),
            &a,
            POLL,
        )
        .expect("call"),
    );
    assert!(why.contains("planned runs are used"), "{why}");
    recorded(
        caller::call(
            &p,
            &call_of("guardrail", "start", "g1", "g1", "hi"),
            &a,
            POLL,
        )
        .expect("call"),
    );
    let why = not_started(
        caller::call(
            &p,
            &call_of("guardrail", "start", "g2", "g2", "hi"),
            &a,
            POLL,
        )
        .expect("call"),
    );
    assert!(why.contains("guardrail: 1 of 1 runs"), "{why}");
    // the guardrail's own share: one run, never retried, with budget runs left (a fresh ledger)
    let t2 = Tmp::new("b0205-gr");
    std::fs::write(t2.0.join("adapter.token"), TOKEN).expect("tok");
    let mut v = plan_v(json!({"guardrail": role("gr", Value::Null)}));
    v["ledger_dir"] = json!(t2.0.join("ledger"));
    v["roles"]["guardrail"]["adapter_token_file"] = json!(t2.0.join("adapter.token"));
    let p2 = Plan::from_json(&v).expect("plan");
    let mut m = BTreeMap::new();
    m.insert("gr".to_string(), service_0205(&t2.0, "guardrail"));
    let a2 = InProc(m);
    recorded(
        caller::call(
            &p2,
            &call_of("guardrail", "start", "g1", "g1", "hi"),
            &a2,
            POLL,
        )
        .expect("call"),
    );
    let why = not_started(
        caller::call(
            &p2,
            &call_of("guardrail", "start", "g2", "g2", "hi"),
            &a2,
            POLL,
        )
        .expect("call"),
    );
    assert!(why.contains("guardrail: 1 of 1 runs"), "{why}");
}

#[test]
fn a_ledger_belongs_to_one_budget() {
    let _serial = serial();
    let t = Tmp::new("ledger-budget");
    let p = plan(&t.0, &[("development", "dev")], Some(&rate("rejected")));
    // a record of another budget in this budget's ledger: refused, never counted or charged
    std::fs::create_dir_all(t.0.join("ledger")).expect("ledger");
    std::fs::write(
        t.0.join("ledger/records.jsonl"),
        format!(
            "{}\n",
            json!({"format": "ieo-role-run/v0", "attempt_id": "x1", "role": "development",
                   "budget": "02-05", "model_request": true, "duration_ms": 1000,
                   "usage": {"run": {"state": "measured", "tokens": {"input": 1, "output": 1,
                             "cache_creation": 1, "cache_read": 1}}}})
        ),
    )
    .expect("record");
    let e = caller::records(&p).expect_err("another budget's record");
    assert!(e.contains("one ledger per budget"), "{e}");
}

/// The 02-05 spare run (user decision 2026-11-17 (나)): a marker with a reason and the replaced
/// attempt, which must be an earlier failed development run of this budget; one spare only;
/// never for the guardrail; the marker is written to the run's record.
#[test]
fn the_0205_spare_run_needs_a_marker_and_a_failed_development_run() {
    let _serial = serial();
    let t = Tmp::new("b0205-spare");
    std::fs::write(t.0.join("adapter.token"), TOKEN).expect("tok");
    let obs = t.0.join("first-observation.jsonl");
    std::fs::write(&obs, format!("{}\n", rate("rejected"))).expect("obs");
    let p = Plan::from_json(&json!({"budget": "02-05", "ledger_dir": t.0.join("ledger"),
        "first_observation": {"stream": obs, "observed_unix": now() - 3600},
        "roles": {"development": {"url": "dev", "adapter_token_file": t.0.join("adapter.token")},
                  "guardrail": {"url": "gr", "adapter_token_file": t.0.join("adapter.token")}}}))
    .expect("plan");
    let mut m = BTreeMap::new();
    m.insert("dev".to_string(), service_0205(&t.0, "development"));
    m.insert("gr".to_string(), service_0205(&t.0, "guardrail"));
    let a = InProc(m);
    let spare =
        |reason: &str, replaces: &str| json!({"spare": {"reason": reason, "replaces": replaces}});
    let call = |role: &str, attempt: &str, prompt: &str, extra: Value| {
        caller::call_with_extra(
            &p,
            &call_of(role, "start", attempt, attempt, prompt),
            &a,
            POLL,
            Duration::from_secs(60),
            &extra,
        )
        .expect("call")
    };
    // R1 succeeds; a spare that replaces it is refused (it did not fail)
    recorded(call("development", "d1", "hi", json!({})));
    let why = not_started(call("development", "x1", "hi", spare("env", "d1")));
    assert!(why.contains("did not fail"), "{why}");
    // G1; the guardrail has no spare
    recorded(call("guardrail", "g1", "hi", json!({})));
    let why = not_started(call("guardrail", "x2", "hi", spare("env", "d1")));
    assert!(why.contains("has no spare"), "{why}");
    // R2 fails (an error result: a record that asks for a person)
    let r2 = recorded(call("development", "d2", "AUTHFAIL", json!({})));
    assert_eq!(r2["stop_required"], true, "{r2}");
    // no marker: development stops at its 2 planned runs
    let why = not_started(call("development", "x3", "hi", json!({})));
    assert!(why.contains("planned runs are used"), "{why}");
    // a marker naming the guardrail's run, an unknown attempt, or without a reason: refused
    for (extra, want) in [
        (spare("env", "g1"), "is not a development run"),
        (spare("env", "nope"), "0 found"),
        (
            json!({"spare": {"reason": " ", "replaces": "d2"}}),
            "needs a reason",
        ),
        (json!({"spare": {"reason": "env"}}), "needs a reason"),
    ] {
        let why = not_started(call("development", "x4", "hi", extra.clone()));
        assert!(why.contains(want), "{extra}: {why}");
    }
    // the spare: replaces the failed R2 with a reason; recorded with its marker; R2's record no
    // longer stops the budget (the marker is the person's decision about it)
    let r3 = recorded(call(
        "development",
        "d3",
        "hi",
        spare("execution environment defect, cause reported", "d2"),
    ));
    assert_eq!(
        r3["spare"],
        json!({"reason": "execution environment defect, cause reported", "replaces": "d2"}),
        "{r3}"
    );
    let ledger = std::fs::read_to_string(t.0.join("ledger/records.jsonl")).expect("ledger");
    assert!(
        ledger.contains("execution environment defect, cause reported"),
        "the marker is in the ledger"
    );
    // one spare only (also the budget's 4 runs are used now)
    let why = not_started(call("development", "x5", "hi", spare("again", "d2")));
    assert!(why.contains("used already"), "{why}");
    let why = not_started(call("development", "x6", "hi", json!({})));
    assert!(why.contains("planned runs are used"), "{why}");
}

/// The spare may redo R1 too (the run after a failure, not only the third), and R2 then runs
/// without a marker: 2 planned + 1 spare.
#[test]
fn the_0205_spare_may_redo_the_first_planned_run() {
    let _serial = serial();
    let t = Tmp::new("b0205-spare1");
    std::fs::write(t.0.join("adapter.token"), TOKEN).expect("tok");
    let obs = t.0.join("first-observation.jsonl");
    std::fs::write(&obs, format!("{}\n", rate("rejected"))).expect("obs");
    let p = Plan::from_json(&json!({"budget": "02-05", "ledger_dir": t.0.join("ledger"),
        "first_observation": {"stream": obs, "observed_unix": now() - 3600},
        "roles": {"development": {"url": "dev", "adapter_token_file": t.0.join("adapter.token")}}}))
    .expect("plan");
    let mut m = BTreeMap::new();
    m.insert("dev".to_string(), service_0205(&t.0, "development"));
    let a = InProc(m);
    let call = |attempt: &str, prompt: &str, extra: Value| {
        caller::call_with_extra(
            &p,
            &call_of("development", "start", attempt, attempt, prompt),
            &a,
            POLL,
            Duration::from_secs(60),
            &extra,
        )
        .expect("call")
    };
    recorded(call("d1", "AUTHFAIL", json!({})));
    // R1's failure asks for a person: nothing runs without the spare marker
    let why = not_started(call("x1", "hi", json!({})));
    assert!(why.contains("asks for a person"), "{why}");
    recorded(call(
        "d1b",
        "hi",
        json!({"spare": {"reason": "env defect", "replaces": "d1"}}),
    ));
    recorded(call("d2", "hi", json!({})));
    let why = not_started(call("d3", "hi", json!({})));
    assert!(why.contains("planned runs are used"), "{why}");
}

/// The spare gate fails closed on an ambiguous ledger: two records of the replaced attempt, or
/// a replaced record of another budget, refuse (user decision 2026-11-17 (나)).
#[test]
fn the_spare_gate_refuses_ambiguous_records() {
    let t = Tmp::new("spare-gate");
    std::fs::write(t.0.join("adapter.token"), TOKEN).expect("tok");
    let p = Plan::from_json(&json!({"budget": "02-05", "ledger_dir": t.0.join("ledger"),
        "roles": {"development": {"url": "dev", "adapter_token_file": t.0.join("adapter.token")}}}))
    .expect("plan");
    let run = |attempt: &str, budget: &str, failed: bool| {
        json!({"format": "ieo-role-run/v0", "attempt_id": attempt, "role": "development",
               "budget": budget, "model_request": true, "stop_required": failed,
               "outcome": {"result": {"subtype": "success", "is_error": failed}}})
    };
    let recs = vec![run("d1", "02-05", false), run("d2", "02-05", true)];
    let marker = json!({"spare": {"reason": "env", "replaces": "d2"}});
    assert_eq!(
        caller::spare_gate(&p, "development", &recs, &recs, &marker),
        Ok(Some(json!({"reason": "env", "replaces": "d2"})))
    );
    // two records of the replaced attempt
    let raw = vec![
        run("d1", "02-05", false),
        run("d2", "02-05", true),
        run("d2", "02-05", true),
    ];
    let e = caller::spare_gate(&p, "development", &recs, &raw, &marker).expect_err("ambiguous");
    assert!(e.contains("2 found"), "{e}");
    // a replaced run of another budget
    let raw = vec![run("d1", "02-05", false), run("d2", "02-04", true)];
    let e = caller::spare_gate(&p, "development", &recs, &raw, &marker).expect_err("budget");
    assert!(e.contains("of the 02-05 budget"), "{e}");
    // a failure is a non-success result even without stop_required
    let mut soft = run("d2", "02-05", false);
    soft["outcome"]["result"]["subtype"] = json!("error_max_turns");
    let raw = vec![run("d1", "02-05", false), soft];
    assert!(caller::spare_gate(&p, "development", &recs, &raw, &marker).is_ok());
    // a missing result is no clean success either
    let mut none = run("d2", "02-05", false);
    none["outcome"]["result"] = Value::Null;
    let raw = vec![run("d1", "02-05", false), none];
    assert!(caller::spare_gate(&p, "development", &recs, &raw, &marker).is_ok());
}
