//! `ieo-evidence-record/v0`: one record per command run.
//! The JSON Schema is `contracts/evidence-record-v0.schema.json`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const RECORD_FORMAT: &str = "ieo-evidence-record/v0";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EvidenceRecord {
    pub format: String,
    /// Traceability: which task, attempt and command produced this evidence.
    pub task_id: String,
    pub attempt_id: String,
    pub command_id: String,
    /// development | guardrail | review | coordinator | other
    pub role: String,
    /// Earlier attempt this run retries. The wrapper never retries by itself.
    pub retry_of: Option<String>,
    /// True when the command sends a model request (`claude -p … stream-json`):
    /// a result event is then required and usage is expected.
    pub model_request: bool,
    /// argv after secret masking.
    pub argv: Vec<String>,
    pub cwd: String,
    /// Names of the environment variables passed (values are never recorded).
    pub env_names: Vec<String>,
    pub started_at: String,
    pub ended_at: String,
    pub duration_ms: u64,
    pub process: ProcessOutcome,
    pub stdout: StreamFile,
    pub stderr: StreamFile,
    pub redaction: Redaction,
    pub artifacts: Vec<ArtifactCheck>,
    pub markers: Vec<MarkerCheck>,
    pub result_event: Option<ResultEvent>,
    pub usage: Usage,
    pub judgment: Judgment,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProcessKind {
    Exited,
    Signaled,
    TimedOut,
    LaunchFailed,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ProcessOutcome {
    pub kind: ProcessKind,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub timeout_ms: Option<u64>,
    /// SIGTERM sent to the process group at the deadline.
    pub term_sent: bool,
    /// SIGKILL sent after the grace period (or to stray group members).
    pub kill_sent: bool,
    /// Members of the command's process group were still alive after the
    /// command (or after TERM and the grace period) and were killed.
    pub stray_group_killed: bool,
    /// The output pipes were still held open (by a descendant outside the
    /// process group) when collection stopped: stored output may be truncated.
    pub output_incomplete: bool,
    pub launch_error: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct StreamFile {
    /// File name inside the evidence directory.
    pub file: String,
    pub bytes: u64,
    /// SHA-256 of the stored (masked) bytes.
    pub sha256: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Redaction {
    /// Number of secret sources given (values are not recorded).
    pub secret_sources: u32,
    pub stdout_replacements: u32,
    pub stderr_replacements: u32,
    pub argv_replacements: u32,
    /// A given secret value (or its SHA-256) appeared somewhere before masking.
    pub secret_value_seen: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ArtifactCheck {
    pub path: String,
    pub present: bool,
    pub bytes: Option<u64>,
    pub sha256: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MarkerCheck {
    /// Regular expression searched in stdout.
    pub pattern: String,
    pub found: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ResultEvent {
    /// Result events in stdout. The judged one is the last with num_turns >= 1,
    /// else the last (a concurrent resume can emit an empty 0-turn result first).
    pub count: u32,
    pub present: bool,
    pub subtype: Option<String>,
    pub is_error: Option<bool>,
    pub session_id: Option<String>,
    pub num_turns: Option<u64>,
    pub stop_reason: Option<String>,
    pub runtime_version: Option<String>,
    /// stdout lines that are not JSON (kept raw in stdout; counted here).
    pub non_json_lines: u32,
    /// `rate_limit_event` statuses seen, in order.
    pub rate_limit_statuses: Vec<String>,
    /// `compact_boundary` system events (compaction model calls are not in
    /// `result.usage`, 01-04 L3).
    pub compact_boundaries: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
pub struct Tokens {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_creation: u64,
}

impl Tokens {
    pub fn add(&mut self, o: &Tokens) {
        self.input += o.input;
        self.output += o.output;
        self.cache_read += o.cache_read;
        self.cache_creation += o.cache_creation;
    }
}

/// A usage value is either measured (with its source field) or unmeasured
/// with a reason. There is no implicit zero.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Measure {
    Measured { tokens: Tokens, source: String },
    Unmeasured { reason: String },
}

impl Measure {
    pub fn tokens(&self) -> Option<&Tokens> {
        match self {
            Measure::Measured { tokens, .. } => Some(tokens),
            Measure::Unmeasured { .. } => None,
        }
    }
    pub fn unmeasured(reason: impl Into<String>) -> Self {
        Measure::Unmeasured {
            reason: reason.into(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ChildUsage {
    /// Agent tool_use id in the parent stream.
    pub tool_use_id: String,
    pub agent_id: Option<String>,
    pub usage: Measure,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PartialObservation {
    pub message_id: String,
    pub parent_tool_use_id: Option<String>,
    pub tokens: Tokens,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Applicability {
    /// The command makes a model request; usage is expected.
    Model,
    /// No model request (plain command); there is no usage to measure.
    NotApplicable,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionMode {
    /// A new session (`--session-id` or none): the session totals are this run's.
    New,
    /// `--resume`: the CLI's session totals include earlier runs of the session.
    Resume,
    /// Not stated (only for parsing a stream outside a run).
    Unknown,
}

/// The earlier run whose session this run resumed.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ResumeBase {
    pub task_id: String,
    pub attempt_id: String,
    pub command_id: String,
    pub session_id: Option<String>,
    /// That run's session totals (cumulative) or why they are unknown.
    pub session: Measure,
    pub cost_usd_reported: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Usage {
    pub applicability: Applicability,
    pub session_mode: SessionMode,
    pub resume_base: Option<ResumeBase>,
    /// This run's usage including children and CLI-internal model calls such
    /// as compaction: the value summed across runs. New session: the session
    /// totals. Resumed session: session totals minus the base run's totals.
    pub run: Measure,
    /// Session totals as reported (`result.modelUsage`, children included).
    /// Cumulative over every run of the session when it was resumed (01-04
    /// N1→N2, R1→R2, G3a→G3b). Falls back to parent + children when
    /// modelUsage is absent and every part is measured.
    pub session: Measure,
    /// Parent turns of this run only (`result.usage`); excludes children and
    /// compaction.
    pub parent: Measure,
    /// One entry per Agent tool_use seen in the parent stream.
    pub children: Vec<ChildUsage>,
    /// consistent | mismatch | not_checked: run vs parent + children. A
    /// mismatch means model calls not attributed to parent or children (for
    /// example compaction, 01-04 L3), reported rather than hidden.
    pub attribution_check: String,
    pub attribution_detail: Option<String>,
    /// Per-model breakdown of `result.modelUsage` (session totals).
    pub by_model: BTreeMap<String, Tokens>,
    /// `result.total_cost_usd` as reported by the CLI (session total): an
    /// estimate, not a bill (subscription credentials; no API spend).
    pub cost_usd_reported: Option<f64>,
    /// This run's share of the reported cost, derived like `run`.
    pub run_cost_usd: Option<f64>,
    /// Without a result event: last usage seen per assistant message. Streaming
    /// snapshots, not a total or a lower bound; never summed.
    pub partial_observations: Vec<PartialObservation>,
    /// CLI-internal and provider-side retries are not observable here.
    pub internal_retries: String,
}

impl Usage {
    pub fn not_applicable() -> Self {
        let na = || Measure::unmeasured("모델 요청이 없는 명령");
        Usage {
            applicability: Applicability::NotApplicable,
            session_mode: SessionMode::Unknown,
            resume_base: None,
            run: na(),
            session: na(),
            parent: na(),
            children: vec![],
            attribution_check: "not_checked".into(),
            attribution_detail: None,
            by_model: BTreeMap::new(),
            cost_usd_reported: None,
            run_cost_usd: None,
            partial_observations: vec![],
            internal_retries: "not_applicable".into(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Expected {
    /// Normal run: exit 0, result event (if model request), artifacts, markers.
    Success,
    /// Intentional deny test: the command must exit non-zero.
    NonzeroExit,
    /// Intentional abort: the deadline must be hit.
    Timeout,
    /// Intentional abort of a model run: no result event may appear.
    NoResult,
    /// Model run whose result event must report is_error (e.g. a denied request).
    ResultError,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JudgmentStatus {
    Succeeded,
    /// The command failed (launch, signal, timeout, non-zero exit, result error).
    Failed,
    /// The command looked fine (exit 0) but required evidence is missing.
    Incomplete,
    ExpectedMatched,
    ExpectedNotMatched,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Judgment {
    pub expected: Expected,
    pub status: JudgmentStatus,
    /// Every reason found; nothing is dropped.
    pub reasons: Vec<String>,
    pub counts_as_success: bool,
}
