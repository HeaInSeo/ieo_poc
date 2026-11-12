//! ieo-evidence: run a command and collect evidence, summarize records,
//! or parse an existing stream-json file.
//!
//! ```text
//! ieo-evidence run --out DIR --task T --attempt A --command C [--role R] [--retry-of A0]
//!     [--timeout-s N] [--grace-s N] [--model-request --session new|resume [--resume-base RECORD.json]] [--expect E] [--artifact PATH]...
//!     [--marker REGEX]... [--env NAME]... [--secret-file PATH]... [--cwd DIR] -- ARGV...
//! ieo-evidence summarize RECORD.json|DIR...
//! ieo-evidence inspect-stream FILE
//! ```
//! Exit status of `run`: 0 when the run counts as success (succeeded or matched
//! its expected outcome), 1 otherwise, 2 for a usage or evidence-write error.

use ieo_evidence::aggregate::summarize;
use ieo_evidence::record::{EvidenceRecord, Expected, SessionMode};
use ieo_evidence::run::{RunSpec, execute};
use ieo_evidence::stream;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

fn usage() -> ExitCode {
    eprintln!(
        "usage: ieo-evidence run --out DIR --task T --attempt A --command C [options] -- ARGV...\n       ieo-evidence summarize RECORD.json|DIR...\n       ieo-evidence inspect-stream FILE"
    );
    ExitCode::from(2)
}

fn parse_expected(s: &str) -> Option<Expected> {
    Some(match s {
        "success" => Expected::Success,
        "nonzero_exit" => Expected::NonzeroExit,
        "timeout" => Expected::Timeout,
        "no_result" => Expected::NoResult,
        "result_error" => Expected::ResultError,
        _ => return None,
    })
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    let split = args
        .iter()
        .position(|a| a == "--")
        .ok_or("명령 앞에 -- 가 필요함")?;
    let (opts, argv) = (&args[..split], &args[split + 1..]);
    let mut spec = RunSpec {
        task_id: String::new(),
        attempt_id: String::new(),
        command_id: String::new(),
        role: "other".into(),
        retry_of: None,
        argv: argv.to_vec(),
        cwd: std::env::current_dir().map_err(|e| e.to_string())?,
        env_allow: vec![],
        timeout: None,
        grace: Duration::from_secs(10),
        model_request: false,
        session_mode: SessionMode::Unknown,
        resume_base: None,
        required_artifacts: vec![],
        required_markers: vec![],
        expected: Expected::Success,
        out_root: PathBuf::new(),
        secrets: vec![],
    };
    let mut i = 0;
    while i < opts.len() {
        let flag = opts[i].as_str();
        if flag == "--model-request" {
            spec.model_request = true;
            i += 1;
            continue;
        }
        let v = opts
            .get(i + 1)
            .ok_or_else(|| format!("{flag} 값 없음"))?
            .clone();
        let secs = |v: &str| {
            v.parse::<u64>()
                .map(Duration::from_secs)
                .map_err(|_| format!("{flag}: 정수 초가 아님"))
        };
        match flag {
            "--out" => spec.out_root = PathBuf::from(v),
            "--task" => spec.task_id = v,
            "--attempt" => spec.attempt_id = v,
            "--command" => spec.command_id = v,
            "--role" => spec.role = v,
            "--retry-of" => spec.retry_of = Some(v),
            "--timeout-s" => spec.timeout = Some(secs(&v)?),
            "--grace-s" => spec.grace = secs(&v)?,
            "--expect" => {
                spec.expected = parse_expected(&v).ok_or_else(|| format!("--expect {v}?"))?
            }
            "--session" => {
                spec.session_mode = match v.as_str() {
                    "new" => SessionMode::New,
                    "resume" => SessionMode::Resume,
                    _ => return Err(format!("--session {v}?")),
                }
            }
            "--resume-base" => spec.resume_base = Some(PathBuf::from(v)),
            "--artifact" => spec.required_artifacts.push(PathBuf::from(v)),
            "--marker" => spec.required_markers.push(v),
            "--env" => spec.env_allow.push(v),
            "--cwd" => spec.cwd = PathBuf::from(v),
            "--secret-file" => spec
                .secrets
                .push(std::fs::read(&v).map_err(|e| format!("--secret-file 읽기 실패: {e}"))?),
            _ => return Err(format!("알 수 없는 옵션 {flag}")),
        }
        i += 2;
    }
    if spec.out_root.as_os_str().is_empty() {
        return Err("--out 필요".into());
    }
    let rec = execute(&spec).map_err(|e| e.to_string())?;
    // Short line for the operator; the record is the evidence.
    println!(
        "{}",
        serde_json::json!({
            "task_id": rec.task_id, "attempt_id": rec.attempt_id, "command_id": rec.command_id,
            "judgment": rec.judgment.status, "counts_as_success": rec.judgment.counts_as_success,
            "reasons": rec.judgment.reasons, "process": rec.process.kind, "exit_code": rec.process.exit_code,
            "duration_ms": rec.duration_ms, "usage_applicability": rec.usage.applicability, "usage_run": rec.usage.run,
        })
    );
    Ok(if rec.judgment.counts_as_success {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn collect(p: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if p.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(p)?.collect::<Result<_, _>>()?;
        entries.sort_by_key(|e| e.path());
        for e in entries {
            collect(&e.path(), out)?;
        }
    } else if p.file_name().is_some_and(|n| n == "record.json") {
        out.push(p.to_path_buf());
    }
    Ok(())
}

fn summarize_cmd(args: &[String]) -> Result<ExitCode, String> {
    if args.is_empty() {
        return Err("record.json 또는 디렉터리 필요".into());
    }
    let mut files = vec![];
    for a in args {
        let p = Path::new(a);
        if !p.exists() {
            return Err(format!("없음: {a}"));
        }
        collect(p, &mut files).map_err(|e| e.to_string())?;
    }
    let mut records = vec![];
    for f in &files {
        let text = std::fs::read_to_string(f).map_err(|e| format!("{}: {e}", f.display()))?;
        let r: EvidenceRecord =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", f.display()))?;
        records.push(r);
    }
    let s = summarize(&records);
    println!(
        "{}",
        serde_json::to_string_pretty(&s).map_err(|e| e.to_string())?
    );
    Ok(ExitCode::SUCCESS)
}

fn inspect(args: &[String]) -> Result<ExitCode, String> {
    let [f] = args else {
        return Err("파일 하나 필요".into());
    };
    let b = std::fs::read(f).map_err(|e| format!("{f}: {e}"))?;
    let p = stream::parse(&b, SessionMode::Unknown, None);
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"result_event": p.result, "usage": p.usage})
        )
        .map_err(|e| e.to_string())?
    );
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let r = match args.first().map(String::as_str) {
        Some("run") => run(&args[1..]),
        Some("summarize") => summarize_cmd(&args[1..]),
        Some("inspect-stream") => inspect(&args[1..]),
        _ => return usage(),
    };
    r.unwrap_or_else(|e| {
        eprintln!("ieo-evidence: {e}");
        ExitCode::from(2)
    })
}
