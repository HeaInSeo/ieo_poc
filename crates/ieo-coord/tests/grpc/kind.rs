//! X-06 driver for two real ieo-coord processes on the disposable kind cluster.

use super::*;
use std::net::ToSocketAddrs;
use std::path::Path;

const CONFIG_SUFFIX: &str = "x06-fixture";
const PG: &str = "pg.x06.svc.cluster.local";
const GRPC_HOST: &str = "grpc-coord.x06.svc.cluster.local";
const REPRO_ID: &str = "repro-x06-response-lost";
const CALL_LIMIT: Duration = Duration::from_secs(5);

#[ignore = "writes config for the disposable X-06 kind trial"]
#[test]
fn write_config() {
    let out = std::env::var("X06_CONFIG_OUT").expect("X06_CONFIG_OUT");
    let out = Path::new(&out);
    let (app, _, creds) = fixture(&postgres::Config::new(), CONFIG_SUFFIX);
    for (name, value) in [
        ("tokens.json", creds),
        ("manifest.json", app.ctx.manifest),
        ("bindings.json", app.ctx.bindings),
        ("roles.json", app.ctx.roles),
    ] {
        std::fs::write(out.join(name), serde_json::to_vec(&value).expect("json")).expect("write");
    }
    std::fs::write(out.join("policy.json"), POLICY).expect("policy");
}

fn external(name: &str) -> Env {
    let mut cfg = postgres::Config::new();
    cfg.host(PG)
        .user("x06")
        .password("x06-ephemeral-fixture")
        .dbname(name)
        .connect_timeout(CALL_LIMIT);
    let (app, tokens, _) = fixture(&cfg, CONFIG_SUFFIX);
    Env {
        _permit: Permit::acquire(),
        addr: format!("{name}-coord.x06.svc.cluster.local:8080"),
        base: None,
        dbname: name.to_owned(),
        cfg,
        tokens,
        app: Arc::new(app),
    }
}

fn transported(
    g: &G,
    cl: &mut SupervisorServiceClient<Channel>,
    e: &Env,
    who: Option<&str>,
    c: &Call,
    deadline: Option<Duration>,
) -> Result<(u16, Value), Status> {
    prepare(e, c);
    let started = Instant::now();
    let answer = g.rt.block_on(async {
        tokio::time::timeout(CALL_LIMIT, G::send(cl, e, who, c, deadline))
            .await
            .expect("product RPC within 5 seconds")
    });
    assert!(started.elapsed() <= CALL_LIMIT);
    answer
}

fn duplicate_assignment(e: &Env) {
    let mut db = e.db();
    let assignment_state = |db: &mut postgres::Client| {
        db.query_one(
            "SELECT jsonb_build_array(
                (SELECT jsonb_agg(to_jsonb(a) ORDER BY a.assignment_id, a.generation) FROM assignments a),
                (SELECT jsonb_agg(to_jsonb(h) ORDER BY h.repo_id, h.role) FROM assignment_heads h)
            )",
            &[],
        )
        .expect("assignment state")
        .get::<_, Value>(0)
    };
    let before = (snapshot(&mut db), assignment_state(&mut db));
    let replay = assign(e, "development", "A1", 0, "01-01", SHA1, Value::Null);
    assert_eq!(
        (replay.0, replay.1["outcome"].as_str()),
        (200, Some("already_issued"))
    );
    let conflict = assign(e, "development", "B1", 0, "01-01", SHA2, Value::Null);
    assert_eq!(
        (conflict.0, code(&conflict.1).as_str()),
        (409, "OWNERSHIP_CONFLICT")
    );
    assert_eq!(before, (snapshot(&mut db), assignment_state(&mut db)));
}

fn rollback(g: &G, client: &mut SupervisorServiceClient<Channel>, e: &Env) {
    let digest = spec("repo-a", 5, 3)["digest"].as_str().unwrap().to_owned();
    let c = cp(
        5,
        cp_raw(5, 1, &digest, json!({"session_id":"s3", "target_sha":SHA2})),
    );
    let mut db = e.db();
    let before = snapshot(&mut db);
    let lock = hold_checkpoint_lock(e);
    let rejection = transported(
        g,
        client,
        e,
        Some("sup-dev-a"),
        &c,
        Some(Duration::from_millis(600)),
    )
    .expect_err("deadline must reject checkpoint");
    release(lock);
    assert_eq!(rejection.code(), Code::DeadlineExceeded, "{rejection:?}");
    assert!(rejection.message().contains("rolled back"), "{rejection:?}");
    assert_eq!(before, snapshot(&mut db));
    let head = transported(
        g,
        client,
        e,
        Some("sup-dev-a"),
        &latest("repo-a", "development"),
        None,
    )
    .expect("checkpoint head after rollback");
    assert_eq!(head.1["seq"], 4);
    eprintln!("rollback: seq 5 rejected by server and DB unchanged, seq 4 confirmed");
}

fn response_loss(
    g: &G,
    direct: &mut SupervisorServiceClient<Channel>,
    e: &Env,
    pod: std::net::SocketAddr,
) {
    let relay = relay(g, pod);
    let mut interrupted =
        SupervisorServiceClient::new(g.channel(&format!("http://{}", relay.addr)));
    let read = Call::GetRepro(REPRO_ID.into());
    let warm = answer(transported(
        g,
        &mut interrupted,
        e,
        Some("gr-a"),
        &read,
        None,
    ));
    assert_eq!(warm.0, 404);
    let mut db = e.db();
    let records = |db: &mut postgres::Client| {
        db.query_one(
            "SELECT count(*) FROM repro_requests WHERE request_id = $1",
            &[&REPRO_ID],
        )
        .expect("read committed state")
        .get::<_, i64>(0)
    };
    assert_eq!(records(&mut db), 0);

    relay.blackhole.store(true, Ordering::SeqCst);
    let write = Call::Repro(repro_msg(REPRO_ID, 1, SHA2));
    let started = Instant::now();
    let missing = g.rt.block_on(async {
        tokio::time::timeout(
            Duration::from_millis(1500),
            G::send(&mut interrupted, e, Some("gr-a"), &write, Some(CALL_LIMIT)),
        )
        .await
    });
    assert!(missing.is_err(), "response reached interrupted client");
    assert!(
        started.elapsed() <= Duration::from_secs(2),
        "loss observation exceeded 2 seconds"
    );
    assert_eq!(
        records(&mut db),
        1,
        "one independently committed key before reissue"
    );
    let applied = snapshot(&mut db);
    let observed = transported(g, direct, e, Some("gr-a"), &read, None).expect("confirm by key");
    assert_eq!(observed.0, 200);
    assert_eq!(observed.1["request_id"], REPRO_ID);
    assert_eq!(
        applied,
        snapshot(&mut db),
        "confirmation read changed state"
    );
    let reissued =
        transported(g, direct, e, Some("gr-a"), &write, None).expect("one explicit reissue");
    assert_eq!(normalize(reissued), normalize(observed));
    assert_eq!(records(&mut db), 1);
    assert_eq!(applied, snapshot(&mut db), "explicit replay changed state");
    eprintln!(
        "response-loss: {REPRO_ID} committed once, read confirmed, one explicit replay returned 200"
    );
}

#[ignore = "requires the X-06 kind network and untouched fixture databases"]
#[test]
fn product_paths() {
    assert_eq!(std::env::var("X06_ISOLATED_KIND").as_deref(), Ok("ieo-x06"));
    let h = external("http");
    let r = external("grpc");
    let addresses: Vec<_> = (GRPC_HOST, 50051)
        .to_socket_addrs()
        .expect("headless DNS")
        .collect();
    assert_eq!(
        addresses.len(),
        1,
        "one dedicated coordinator Pod: {addresses:?}"
    );
    assert_eq!(
        addresses[0].ip().to_string(),
        std::env::var("X06_GRPC_POD_IP").expect("Pod IP")
    );
    eprintln!(
        "coordinator: pod_uid={} address={}",
        std::env::var("X06_GRPC_POD_UID").expect("Pod UID"),
        addresses[0],
    );
    let g = G {
        rt: tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap(),
        server: None,
        url: format!("http://{GRPC_HOST}:50051"),
        db_down: Arc::new(AtomicBool::new(false)),
        app_name: String::new(),
    };
    let mut cl = g.client();
    let started = Instant::now();
    let n = compare_transports(
        &h,
        &r,
        |who, call| normalize(answer(transported(&g, &mut cl, &r, who, call, None))),
        |label, rpc| {
            if label == "assign A1/G1 gen 0" {
                for e in [&h, &r] {
                    duplicate_assignment(e);
                }
                let via_grpc = rpc(Some("sup-dev-a"), &own("repo-a", "development"));
                assert_eq!(via_grpc.0, 200);
                assert_eq!(via_grpc.1["assignment_id"], "A1");
                assert_eq!(via_grpc.1["generation"], 0);
                eprintln!("assignment: replay 200, conflict 409, A1 generation 0 unchanged");
            }
        },
    );
    assert_eq!(n, 157);
    assert!(started.elapsed() <= Duration::from_secs(120));
    eprintln!(
        "comparison: {n} matched HTTP/gRPC cases, {} ms",
        started.elapsed().as_millis()
    );
    rollback(&g, &mut cl, &r);
    response_loss(&g, &mut cl, &r, addresses[0]);
}
