//! Execute one command and write its evidence directory.

use crate::judge::{Inputs, judge};
use crate::record::{
    ArtifactCheck, EvidenceRecord, Expected, MarkerCheck, ProcessKind, ProcessOutcome,
    RECORD_FORMAT, Redaction, ResumeBase, SessionMode, StreamFile, Usage,
};
use crate::redact::Redactor;
use crate::stream;
use crate::time;
use regex::bytes::Regex;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

#[derive(Clone, Debug)]
pub struct RunSpec {
    pub task_id: String,
    pub attempt_id: String,
    pub command_id: String,
    pub role: String,
    pub retry_of: Option<String>,
    pub argv: Vec<String>,
    pub cwd: PathBuf,
    /// Environment variable names copied from this process; nothing else is passed.
    pub env_allow: Vec<String>,
    pub timeout: Option<Duration>,
    /// Time between SIGTERM and SIGKILL.
    pub grace: Duration,
    /// stdout is Claude Code stream-json: a result event is required and usage parsed.
    pub model_request: bool,
    /// Required for model requests: new session or resume.
    pub session_mode: SessionMode,
    /// For a resume: the record (record.json) of the run whose session is resumed.
    pub resume_base: Option<PathBuf>,
    pub required_artifacts: Vec<PathBuf>,
    pub required_markers: Vec<String>,
    pub expected: Expected,
    pub out_root: PathBuf,
    /// Secret values (in memory only) to mask.
    pub secrets: Vec<Vec<u8>>,
}

pub const ROLES: &[&str] = &["development", "guardrail", "review", "coordinator", "other"];

#[derive(Debug)]
pub enum RunError {
    /// The request itself is invalid; nothing was executed or written.
    Invalid(String),
    /// Writing evidence failed (the command may have run).
    Io(String),
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::Invalid(m) => write!(f, "잘못된 요청: {m}"),
            RunError::Io(m) => write!(f, "증거 기록 실패: {m}"),
        }
    }
}

fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s != "."
        && s != ".."
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

pub fn evidence_dir(out_root: &Path, task: &str, attempt: &str, command: &str) -> PathBuf {
    out_root.join(task).join(attempt).join(command)
}

fn sha256_hex(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

fn signal_group(pgid: u32, sig: &str) {
    // `kill -s SIG -- -PGID` signals the whole process group created for the command.
    let _ = Command::new("kill")
        .args(["-s", sig, "--", &format!("-{pgid}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn spawn_reader<R: Read + Send + 'static>(mut r: R) -> mpsc::Receiver<Vec<u8>> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = r.read_to_end(&mut buf);
        let _ = tx.send(buf);
    });
    rx
}

pub fn execute(spec: &RunSpec) -> Result<EvidenceRecord, RunError> {
    for (name, v) in [
        ("task_id", &spec.task_id),
        ("attempt_id", &spec.attempt_id),
        ("command_id", &spec.command_id),
    ] {
        if !valid_id(v) {
            return Err(RunError::Invalid(format!(
                "{name}는 영숫자·-·_·.만 허용: {v:?}"
            )));
        }
    }
    if !ROLES.contains(&spec.role.as_str()) {
        return Err(RunError::Invalid(format!(
            "알 수 없는 역할 {:?}",
            spec.role
        )));
    }
    if spec.argv.is_empty() {
        return Err(RunError::Invalid("명령이 비어 있음".into()));
    }
    let markers: Vec<(String, Regex)> = spec
        .required_markers
        .iter()
        .map(|p| {
            Regex::new(&format!("(?m){p}"))
                .map(|r| (p.clone(), r))
                .map_err(|e| RunError::Invalid(format!("표식 정규식 {p:?}: {e}")))
        })
        .collect::<Result<_, _>>()?;
    let redactor = Redactor::new(&spec.secrets).map_err(RunError::Invalid)?;
    let base = load_base(spec)?;
    let dir = evidence_dir(
        &spec.out_root,
        &spec.task_id,
        &spec.attempt_id,
        &spec.command_id,
    );
    if dir.exists() {
        return Err(RunError::Invalid(format!(
            "증거 디렉터리가 이미 있음(덮어쓰지 않음): {}",
            dir.display()
        )));
    }
    fs::create_dir_all(&dir).map_err(|e| RunError::Io(format!("{}: {e}", dir.display())))?;

    let mut cmd = Command::new(&spec.argv[0]);
    cmd.args(&spec.argv[1..])
        .current_dir(&spec.cwd)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let mut env_names = Vec::new();
    for name in &spec.env_allow {
        if let Some(v) = std::env::var_os(name) {
            cmd.env(name, v);
            env_names.push(name.clone());
        }
    }
    env_names.sort();

    let started_wall = SystemTime::now();
    let started = Instant::now();
    let mut process = ProcessOutcome {
        kind: ProcessKind::Exited,
        exit_code: None,
        signal: None,
        timeout_ms: spec.timeout.map(|d| d.as_millis() as u64),
        term_sent: false,
        kill_sent: false,
        stray_group_killed: false,
        launch_error: None,
    };
    let (out_raw, err_raw) = match cmd.spawn() {
        Err(e) => {
            process.kind = ProcessKind::LaunchFailed;
            process.launch_error = Some(e.to_string());
            (Vec::new(), Vec::new())
        }
        Ok(mut child) => {
            let pgid = child.id();
            let out_rx = spawn_reader(child.stdout.take().expect("piped"));
            let err_rx = spawn_reader(child.stderr.take().expect("piped"));
            let mut term_at: Option<Instant> = None;
            let status = loop {
                match child.try_wait() {
                    Ok(Some(st)) => break Some(st),
                    Ok(None) => {}
                    Err(_) => break None,
                }
                let now = Instant::now();
                if let Some(t) = spec.timeout
                    && term_at.is_none()
                    && now.duration_since(started) >= t
                {
                    signal_group(pgid, "TERM");
                    process.term_sent = true;
                    term_at = Some(now);
                }
                if let Some(t0) = term_at
                    && !process.kill_sent
                    && now.duration_since(t0) >= spec.grace
                {
                    signal_group(pgid, "KILL");
                    let _ = child.kill();
                    process.kill_sent = true;
                }
                thread::sleep(Duration::from_millis(20));
            };
            if process.term_sent {
                process.kind = ProcessKind::TimedOut;
            }
            match status {
                Some(st) => {
                    process.exit_code = st.code();
                    process.signal = st.signal();
                    if process.kind != ProcessKind::TimedOut && st.signal().is_some() {
                        process.kind = ProcessKind::Signaled;
                    }
                }
                None => {
                    process.kind = ProcessKind::LaunchFailed;
                    process.launch_error = Some("종료 상태를 얻지 못함".into());
                }
            }
            // Group members that outlive the command keep the pipes open; give them
            // a moment, then kill the group and record it.
            let collect = |rx: &mpsc::Receiver<Vec<u8>>, p: &mut ProcessOutcome| match rx
                .recv_timeout(Duration::from_secs(2))
            {
                Ok(b) => b,
                Err(_) => {
                    signal_group(pgid, "KILL");
                    p.stray_group_killed = true;
                    p.kill_sent = true;
                    rx.recv_timeout(Duration::from_secs(10)).unwrap_or_default()
                }
            };
            let o = collect(&out_rx, &mut process);
            let e = collect(&err_rx, &mut process);
            (o, e)
        }
    };
    let duration_ms = started.elapsed().as_millis() as u64;
    let ended_at = time::now();

    // Mask before anything is written.
    let out = redactor.mask(&out_raw);
    let err = redactor.mask(&err_raw);
    let mut argv_repl = 0;
    let mut argv_seen = false;
    let argv: Vec<String> = spec
        .argv
        .iter()
        .map(|a| {
            let m = redactor.mask(a.as_bytes());
            argv_repl += m.replacements;
            argv_seen |= m.secret_value_seen;
            String::from_utf8_lossy(&m.bytes).into_owned()
        })
        .collect();
    drop(out_raw);
    drop(err_raw);
    let write = |name: &str, b: &[u8]| -> Result<StreamFile, RunError> {
        fs::write(dir.join(name), b).map_err(|e| RunError::Io(format!("{name}: {e}")))?;
        Ok(StreamFile {
            file: name.into(),
            bytes: b.len() as u64,
            sha256: sha256_hex(b),
        })
    };
    let stdout = write("stdout", &out.bytes)?;
    let stderr = write("stderr", &err.bytes)?;

    let artifacts: Vec<ArtifactCheck> = spec
        .required_artifacts
        .iter()
        .map(|p| {
            let full = if p.is_absolute() {
                p.clone()
            } else {
                spec.cwd.join(p)
            };
            match fs::read(&full) {
                Ok(b) if full.is_file() => ArtifactCheck {
                    path: p.display().to_string(),
                    present: true,
                    bytes: Some(b.len() as u64),
                    sha256: Some(sha256_hex(&b)),
                },
                _ => ArtifactCheck {
                    path: p.display().to_string(),
                    present: false,
                    bytes: None,
                    sha256: None,
                },
            }
        })
        .collect();
    // Markers are searched in the masked stdout (what the evidence shows).
    let marker_checks: Vec<MarkerCheck> = markers
        .iter()
        .map(|(p, r)| MarkerCheck {
            pattern: p.clone(),
            found: r.is_match(&out.bytes),
        })
        .collect();
    let (result_event, usage) = if spec.model_request {
        let parsed = stream::parse(&out.bytes, spec.session_mode, base);
        (Some(parsed.result), parsed.usage)
    } else {
        (None, Usage::not_applicable())
    };
    let judgment = judge(&Inputs {
        expected: spec.expected,
        model_request: spec.model_request,
        process: &process,
        result: result_event.as_ref(),
        artifacts: &artifacts,
        markers: &marker_checks,
    });
    let record = EvidenceRecord {
        format: RECORD_FORMAT.into(),
        task_id: spec.task_id.clone(),
        attempt_id: spec.attempt_id.clone(),
        command_id: spec.command_id.clone(),
        role: spec.role.clone(),
        retry_of: spec.retry_of.clone(),
        model_request: spec.model_request,
        argv,
        cwd: spec.cwd.display().to_string(),
        env_names,
        started_at: time::rfc3339(started_wall),
        ended_at,
        duration_ms,
        process,
        stdout,
        stderr,
        redaction: Redaction {
            secret_sources: redactor.source_count(),
            stdout_replacements: out.replacements,
            stderr_replacements: err.replacements,
            argv_replacements: argv_repl,
            secret_value_seen: out.secret_value_seen || err.secret_value_seen || argv_seen,
        },
        artifacts,
        markers: marker_checks,
        result_event,
        usage,
        judgment,
    };
    let json = serde_json::to_string_pretty(&record).map_err(|e| RunError::Io(e.to_string()))?;
    fs::write(dir.join("record.json"), json + "\n")
        .map_err(|e| RunError::Io(format!("record.json: {e}")))?;
    Ok(record)
}

/// Validate session mode and load the resume base before anything runs.
fn load_base(spec: &RunSpec) -> Result<Option<ResumeBase>, RunError> {
    if !spec.model_request {
        if spec.resume_base.is_some() {
            return Err(RunError::Invalid(
                "모델 요청이 아닌 명령에 재개 기준 지정".into(),
            ));
        }
        return Ok(None);
    }
    match (spec.session_mode, &spec.resume_base) {
        (SessionMode::Unknown, _) => Err(RunError::Invalid(
            "모델 요청은 세션 방식(new|resume)을 지정해야 함".into(),
        )),
        (SessionMode::New, Some(_)) => Err(RunError::Invalid("새 세션에 재개 기준 지정".into())),
        (SessionMode::New, None) | (SessionMode::Resume, None) => Ok(None),
        (SessionMode::Resume, Some(p)) => {
            let text = fs::read_to_string(p)
                .map_err(|e| RunError::Invalid(format!("재개 기준 {}: {e}", p.display())))?;
            let r: EvidenceRecord = serde_json::from_str(&text)
                .map_err(|e| RunError::Invalid(format!("재개 기준 {}: {e}", p.display())))?;
            if !r.model_request {
                return Err(RunError::Invalid(
                    "재개 기준이 모델 요청 기록이 아님".into(),
                ));
            }
            Ok(Some(ResumeBase {
                task_id: r.task_id,
                attempt_id: r.attempt_id,
                command_id: r.command_id,
                session_id: r.result_event.and_then(|e| e.session_id),
                session: r.usage.session,
                cost_usd_reported: r.usage.cost_usd_reported,
            }))
        }
    }
}
