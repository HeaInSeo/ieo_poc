//! `ieo-coord migrate` | `ieo-coord serve --listen ADDR --tokens FILE --manifest FILE
//!  --policy FILE [--bindings FILE] [--workers N]`. The DB URL is read from `IEO_DB_URL`.

use ieo_coord::api::{Ctx, migrate};
use ieo_coord::auth::Credentials;
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
    let url = std::env::var("IEO_DB_URL").map_err(|_| "IEO_DB_URL is not set".to_string())?;
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
            let manifest = read_json(&need("--manifest")?)?;
            let bindings = match arg(&args, "--bindings") {
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
            }
            let server = tiny_http::Server::http(&listen).map_err(|e| format!("listen {listen}: {e}"))?;
            eprintln!("ieo-coord listening on {listen}; policy {}", policy.digest);
            let app = App {
                ctx: Ctx { policy, manifest, bindings },
                creds,
            };
            serve(Arc::new(server), Arc::new(app), &cfg, workers)
        }
        _ => Err("usage: ieo-coord migrate | serve --listen ADDR --tokens FILE --manifest FILE --policy FILE [--bindings FILE] [--workers N]".into()),
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("ieo-coord: {e}");
        std::process::exit(2);
    }
}
