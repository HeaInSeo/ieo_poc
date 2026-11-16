//! `ieo-coord migrate` | `ieo-coord serve --listen ADDR --tokens FILE --manifest FILE
//!  --policy FILE [--bindings FILE] [--profiles FILE] [--roles FILE] [--workers N]
//!  [--grpc-listen ADDR [--grpc-workers N]]`. The DB URL is read from `IEO_DB_URL`,
//!  or from the file named by `IEO_DB_URL_FILE`.
//!
//! `--grpc-listen` (X-04) also serves the internal gRPC API (`ieo.internal.v0.SupervisorService`
//! and `grpc.health.v1.Health`) from the same process, with the same credentials, policy and
//! database; without it the process is the HTTP server it was.

use ieo_coord::api::{Ctx, migrate};
use ieo_coord::auth::Credentials;
use ieo_coord::grpc::{GrpcConfig, GrpcServer};
use ieo_coord::http::{App, serve};
use ieo_coord::policy;
use postgres::{Config, NoTls};
use std::sync::Arc;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn read_json(path: &str) -> Result<serde_json::Value, String> {
    let b = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    serde_json::from_slice(&b).map_err(|e| format!("{path}: {e}"))
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    // IEO_DB_URL, or IEO_DB_URL_FILE naming a file that holds it (a mounted secret: the value
    // then appears in no environment listing)
    let url = match (
        std::env::var("IEO_DB_URL"),
        std::env::var("IEO_DB_URL_FILE"),
    ) {
        (Ok(u), _) => u,
        (Err(_), Ok(f)) => std::fs::read_to_string(&f)
            .map(|s| s.trim().to_string())
            .map_err(|e| format!("IEO_DB_URL_FILE: {}", e.kind()))?,
        _ => return Err("IEO_DB_URL (or IEO_DB_URL_FILE) is not set".into()),
    };
    let cfg: Config = url
        .parse()
        .map_err(|_| "IEO_DB_URL is not a valid PostgreSQL config".to_string())?;
    match args.get(1).map(String::as_str) {
        Some("migrate") => {
            let mut db = cfg.connect(NoTls).map_err(|e| format!("db connect: {e}"))?;
            migrate(&mut db).map_err(|e| format!("migrate: {e}"))
        }
        Some("serve") => {
            let need = |n: &str| arg(&args, n).ok_or(format!("{n} is required"));
            let policy_path = need("--policy")?;
            let bytes = std::fs::read(&policy_path).map_err(|e| format!("{policy_path}: {e}"))?;
            let policy = policy::load(&bytes)?;
            let creds = Credentials::from_json(&read_json(&need("--tokens")?)?)?;
            let mut manifest = read_json(&need("--manifest")?)?;
            // extra approved acceptance profiles (e.g. a test binding's profile), appended
            if let Some(p) = arg(&args, "--profiles") {
                let extra = read_json(&p)?;
                let list = extra.as_array().ok_or("--profiles must be a JSON array")?.clone();
                match manifest["inspection_profiles"].as_array_mut() {
                    Some(ps) => ps.extend(list),
                    None => manifest["inspection_profiles"] = serde_json::Value::Array(list),
                }
            }
            let bindings = match arg(&args, "--bindings") {
                Some(p) => read_json(&p)?,
                None => serde_json::json!({}),
            };
            let roles = match arg(&args, "--roles") {
                Some(p) => read_json(&p)?,
                None => serde_json::json!({}),
            };
            let workers = arg(&args, "--workers")
                .and_then(|w| w.parse().ok())
                .unwrap_or(4);
            let listen = need("--listen")?;
            {
                let mut db = cfg.connect(NoTls).map_err(|e| format!("db connect: {e}"))?;
                migrate(&mut db).map_err(|e| format!("migrate: {e}"))?;
                // startup check (X-01 U3, 02-05 C2): a binding's sprint_id is neither an imported
                // test id, nor another repo's sprint, nor named by two bindings
                ieo_coord::api::check_bindings(&mut db, &bindings)?;
            }
            let server = tiny_http::Server::http(&listen).map_err(|e| format!("listen {listen}: {e}"))?;
            eprintln!("ieo-coord listening on {listen}; policy {}", policy.digest);
            // the delivery bounds, read once (logged when not the defaults)
            let _ = ieo_coord::delivery::bounds();
            let app = App {
                ctx: Ctx {
                    policy,
                    manifest,
                    bindings,
                    roles,
                },
                creds,
            };
            // PID 1 in the coordinator container (02-04): SIGTERM/SIGINT stop accepting, let the
            // requests in progress finish, and exit 0 (a PID 1 without a handler ignores SIGTERM)
            let server = Arc::new(server);
            let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
            for sig in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT] {
                signal_hook::flag::register(sig, Arc::clone(&stop)).map_err(|e| format!("signal: {e}"))?;
            }
            {
                let server = Arc::clone(&server);
                std::thread::spawn(move || {
                    while !stop.load(std::sync::atomic::Ordering::SeqCst) {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    eprintln!("ieo-coord: stopping (signal)");
                    // one unblock per worker waiting in recv
                    for _ in 0..workers.max(1) {
                        server.unblock();
                    }
                });
            }
            let app = Arc::new(app);
            let grpc = match arg(&args, "--grpc-listen") {
                Some(g) => {
                    let c = cfg.clone();
                    let connect: ieo_coord::grpc::Connect =
                        Arc::new(move || c.connect(NoTls).map_err(|e| e.to_string()));
                    let grpc_cfg = GrpcConfig {
                        workers: arg(&args, "--grpc-workers")
                            .and_then(|w| w.parse().ok())
                            .unwrap_or(4),
                        ..GrpcConfig::default()
                    };
                    let s = GrpcServer::start(&g, Arc::clone(&app), connect, grpc_cfg)?;
                    eprintln!("ieo-coord gRPC listening on {}", s.addr);
                    Some(s)
                }
                None => None,
            };
            let served = serve(server, app, &cfg, workers);
            // the gRPC side stops after the HTTP side (calls in progress finish first)
            if let Some(g) = grpc {
                g.stop();
            }
            served?;
            eprintln!("ieo-coord: stopped");
            Ok(())
        }
        _ => Err("usage: ieo-coord migrate | serve --listen ADDR --tokens FILE --manifest FILE --policy FILE [--bindings FILE] [--profiles FILE] [--workers N] [--grpc-listen ADDR [--grpc-workers N]]".into()),
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("ieo-coord: {e}");
        std::process::exit(2);
    }
}
