//! `ieo-supervisor serve --config <file> --listen <addr>`: the role container's PID 1 (02-04).

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let get = |k: &str| {
        args.iter()
            .position(|a| a == k)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    if args.get(1).map(String::as_str) != Some("serve") {
        eprintln!("usage: ieo-supervisor serve --config <file> --listen <addr>");
        std::process::exit(2);
    }
    let (Some(config), Some(listen)) = (get("--config"), get("--listen")) else {
        eprintln!("usage: ieo-supervisor serve --config <file> --listen <addr>");
        std::process::exit(2);
    };
    let cfg = std::fs::read(&config)
        .map_err(|e| e.to_string())
        .and_then(|b| serde_json::from_slice(&b).map_err(|e| e.to_string()))
        .and_then(|v| ieo_supervisor::serve::Config::from_json(&v));
    let cfg = match cfg {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ieo-supervisor: config: {e}");
            std::process::exit(2);
        }
    };
    eprintln!("ieo-supervisor: serving {listen}");
    if let Err(e) = ieo_supervisor::serve::serve(cfg, &listen) {
        eprintln!("ieo-supervisor: {e}");
        std::process::exit(1);
    }
}
