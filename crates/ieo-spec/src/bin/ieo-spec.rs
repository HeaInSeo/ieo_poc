//! `ieo-spec` — local CLI over the 01-07 reference checks (not the authenticated API).
//!
//! ```text
//! ieo-spec digest <spec.json>
//! ieo-spec check-manifest <Console_Coordinator_Sprints_v0.json> [--executable]
//! ieo-spec check <spec.json> --manifest <manifest.json> [--executable]
//! ```
//! Exit codes: 0 all accepted, 1 any rejection, 2 usage/IO error.

use ieo_spec::{CheckMode, Registry, check_registration, spec_digest};
use serde_json::{Value, json};
use std::process::ExitCode;

fn load(path: &str) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))
}

fn run(args: &[String]) -> Result<bool, String> {
    let mode = if args.iter().any(|a| a == "--executable") {
        CheckMode::Executable
    } else {
        CheckMode::Draft
    };
    match args.first().map(String::as_str) {
        Some("digest") => {
            let spec = load(args.get(1).ok_or("missing <spec.json>")?)?;
            println!("{}", spec_digest(&spec));
            Ok(true)
        }
        Some("check-manifest") => {
            let manifest = load(args.get(1).ok_or("missing <manifest.json>")?)?;
            let sprints = manifest["sprints"].as_array().cloned().unwrap_or_default();
            let mut all_ok = true;
            let mut results = Vec::new();
            for (i, spec) in sprints.iter().enumerate() {
                // Every other sprint of the manifest is the registry for this one.
                let mut reg = Registry::new();
                reg.add_profiles_from_manifest(&manifest);
                for (j, other) in sprints.iter().enumerate() {
                    if i != j {
                        reg.add_spec(other);
                    }
                }
                let out = check_registration(spec, &reg, mode);
                all_ok &= out.rejections.is_empty();
                results.push(json!({
                    "sprint_id": spec["sprint_id"], "revision": spec["revision"],
                    "http_status": out.http_status(), "outcome": out,
                }));
            }
            println!("{}", serde_json::to_string_pretty(&results).map_err(|e| e.to_string())?);
            Ok(all_ok)
        }
        Some("check") => {
            let spec = load(args.get(1).ok_or("missing <spec.json>")?)?;
            let mpath = args
                .iter()
                .position(|a| a == "--manifest")
                .and_then(|i| args.get(i + 1))
                .ok_or("missing --manifest <manifest.json>")?;
            let manifest = load(mpath)?;
            let mut reg = Registry::new();
            reg.add_profiles_from_manifest(&manifest);
            for other in manifest["sprints"].as_array().into_iter().flatten() {
                if other["sprint_id"] != spec["sprint_id"] {
                    reg.add_spec(other);
                }
            }
            let out = check_registration(&spec, &reg, mode);
            let ok = out.rejections.is_empty();
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({"http_status": out.http_status(), "outcome": out}))
                    .map_err(|e| e.to_string())?
            );
            Ok(ok)
        }
        _ => Err("usage: ieo-spec digest <spec> | check-manifest <manifest> [--executable] | check <spec> --manifest <m> [--executable]".into()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(2)
        }
    }
}
