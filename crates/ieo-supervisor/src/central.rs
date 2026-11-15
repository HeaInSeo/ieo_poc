//! Central checkpoint record (runtime contract §2.1, §5-1, §5-2, §5-6; 02-04 decision item 9).
//!
//! The coordinator's record is the canonical one. The supervisor reports every checkpoint it
//! writes (sequence number, exact bytes; the coordinator computes the digest and links the
//! repo, role, assignment, generation, sprint revision, spec digest and target SHA), and
//! compares its local checkpoint with the coordinator's latest record before anything starts:
//!
//! * same sequence, same digest → consistent;
//! * the local one is ahead → the missing reports are sent from the local archive, in order,
//!   and the comparison is repeated (a refusal stops: external reconciliation);
//! * the central one is ahead, the digests differ, the local checkpoint is missing while the
//!   central record has one, or the archive lacks a reported sequence → external
//!   reconciliation (`EXTERNAL_RECONCILIATION_REQUIRED`), nothing runs;
//! * the coordinator cannot be reached → `NOT_OBSERVED`, nothing runs.
//!
//! `local_only` (explicit in the role configuration) turns this off for tests; the status then
//! says the comparison is not made.

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Central {
    /// No central record (tests only; reported as a capability gap).
    LocalOnly,
    /// The coordinator: base URL and the file holding this supervisor's report credential.
    Coordinator { url: String, token_file: PathBuf },
}

/// Why the comparison stops a start: the HTTP status and body the adapter answers with.
#[derive(Debug, Clone, PartialEq)]
pub struct Refusal {
    pub status: u16,
    pub body: Value,
}

fn reconcile(reason: impl Into<String>) -> Refusal {
    Refusal {
        status: 409,
        body: json!({"code": "EXTERNAL_RECONCILIATION_REQUIRED", "reason": reason.into(),
                     "external_reconciliation_required": true}),
    }
}

fn not_observed(reason: impl Into<String>) -> Refusal {
    Refusal {
        status: 503,
        body: json!({"code": "NOT_OBSERVED", "reason": reason.into()}),
    }
}

impl Central {
    pub fn from_json(v: &Value) -> Result<Central, String> {
        match v {
            Value::String(s) if s == "local_only" => Ok(Central::LocalOnly),
            Value::Object(_) => Ok(Central::Coordinator {
                url: v["url"]
                    .as_str()
                    .ok_or("config.checkpoint_central.url missing")?
                    .trim_end_matches('/')
                    .to_string(),
                token_file: v["token_file"]
                    .as_str()
                    .ok_or("config.checkpoint_central.token_file missing")?
                    .into(),
            }),
            _ => Err(
                "config.checkpoint_central must be {url, token_file} or \"local_only\" \
                 (runtime contract §2.1: the coordinator's record is canonical)"
                    .into(),
            ),
        }
    }

    pub fn mode(&self) -> &'static str {
        match self {
            Central::LocalOnly => "local_only",
            Central::Coordinator { .. } => "coordinator",
        }
    }

    fn call(&self, method: &str, path: &str, body: Option<&Value>) -> Result<(u16, Value), String> {
        let Central::Coordinator { url, token_file } = self else {
            return Err("no coordinator configured".into());
        };
        let token = std::fs::read_to_string(token_file)
            .map_err(|e| format!("report credential: {}", e.kind()))?;
        let req = ureq::request(method, &format!("{url}{path}"))
            .timeout(Duration::from_secs(10))
            .set("Authorization", &format!("Bearer {}", token.trim()));
        let r = match body {
            Some(b) => req.send_json(b.clone()),
            None => req.call(),
        };
        let r = match r {
            Ok(r) => r,
            Err(ureq::Error::Status(_, r)) => r,
            Err(e) => return Err(format!("transport: {}", e.kind())),
        };
        let st = r.status();
        let v = r.into_json().unwrap_or(Value::Null);
        Ok((st, v))
    }

    fn path(repo: &str, role: &str) -> String {
        format!("/v0/repos/{repo}/roles/{role}/checkpoints")
    }

    /// The coordinator's latest record (`None` when it has none).
    pub fn latest(&self, repo: &str, role: &str) -> Result<Option<Value>, String> {
        match self.call("GET", &format!("{}/latest", Self::path(repo, role)), None)? {
            (200, v) => Ok(Some(v)),
            (404, v) if v["code"] == "NO_CHECKPOINT" => Ok(None),
            (st, v) => Err(format!("latest answered {st}: {}", code_of(&v))),
        }
    }

    /// Report one checkpoint (its exact bytes). `Ok` for a stored or duplicate report.
    pub fn report(&self, repo: &str, role: &str, seq: u64, raw: &[u8]) -> Result<Value, Refusal> {
        let body = json!({"seq": seq, "checkpoint_json": String::from_utf8_lossy(raw)});
        match self.call("POST", &Self::path(repo, role), Some(&body)) {
            Err(e) => Err(not_observed(format!("checkpoint report {seq}: {e}"))),
            Ok((200 | 201, v)) => Ok(v),
            Ok((st, v)) => Err(reconcile(format!(
                "the coordinator refused checkpoint {seq} ({st} {})",
                code_of(&v)
            ))
            .with("coordinator_answer", v)),
        }
    }
}

/// The error code of a coordinator answer: `code`, or the first rejection's (review 48b1d0e L-4).
fn code_of(v: &Value) -> String {
    v["code"]
        .as_str()
        .or_else(|| v["rejections"][0]["code"].as_str())
        .unwrap_or("unknown")
        .to_string()
}

impl Refusal {
    fn with(mut self, k: &str, v: Value) -> Refusal {
        self.body[k] = v;
        self
    }
}

/// The archive of every checkpoint written: `<state>/checkpoints/<seq>.json`.
pub fn archive_path(dir: &Path, seq: u64) -> PathBuf {
    dir.join(format!("{seq:020}.json"))
}

/// Highest archived sequence (0 when none).
pub fn archived_max(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|e| {
            e.file_name()
                .to_str()
                .and_then(|n| n.strip_suffix(".json"))
                .and_then(|n| n.parse::<u64>().ok())
        })
        .max()
        .unwrap_or(0)
}

pub fn sha256(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

/// Compare the local checkpoint (`checkpoint.json` bytes, if any) with the coordinator's
/// latest record, sending missing reports from `archive` first. Returns what was compared.
pub fn compare(
    central: &Central,
    repo: &str,
    role: &str,
    local: Option<&[u8]>,
    archive: &Path,
) -> Result<Value, Refusal> {
    if *central == Central::LocalOnly {
        return Ok(json!({"compared": false, "mode": "local_only"}));
    }
    let local = match local {
        None => None,
        Some(b) => {
            let v: Value = serde_json::from_slice(b)
                .map_err(|_| reconcile("the local checkpoint cannot be read"))?;
            let Some(seq) = v["seq"].as_u64() else {
                return Err(reconcile(
                    "the local checkpoint has no sequence number (written before central reporting)",
                ));
            };
            Some((seq, sha256(b)))
        }
    };
    let latest = central
        .latest(repo, role)
        .map_err(|e| not_observed(format!("central checkpoint record: {e}")))?;
    let central_at = latest.as_ref().map(|v| {
        (
            v["seq"].as_u64().unwrap_or(0),
            v["checkpoint_sha256"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
        )
    });
    let (cseq, csha) = central_at.clone().unwrap_or((0, String::new()));
    let Some((lseq, lsha)) = local else {
        if cseq > 0 {
            return Err(reconcile(format!(
                "the coordinator holds checkpoint {cseq} but there is no local checkpoint"
            )));
        }
        return Ok(json!({"compared": true, "central_seq": 0, "local_seq": null, "sent": 0}));
    };
    if cseq > lseq {
        return Err(reconcile(format!(
            "the local checkpoint {lseq} is older than the coordinator's {cseq} (rolled back or replaced)"
        )));
    }
    if cseq == lseq {
        if csha != lsha {
            return Err(reconcile(format!(
                "the local checkpoint {lseq} differs from the coordinator's record (digest)"
            )));
        }
        return Ok(json!({"compared": true, "central_seq": cseq, "local_seq": lseq, "sent": 0}));
    }
    // local is ahead: the coordinator's last record must be in the archive unchanged
    if cseq > 0 {
        let b = std::fs::read(archive_path(archive, cseq))
            .map_err(|_| reconcile(format!("archived checkpoint {cseq} (reported) is missing")))?;
        if sha256(&b) != csha {
            return Err(reconcile(format!(
                "archived checkpoint {cseq} differs from the coordinator's record"
            )));
        }
    }
    let mut sent = 0u64;
    for s in cseq + 1..=lseq {
        let b = std::fs::read(archive_path(archive, s))
            .map_err(|_| reconcile(format!("archived checkpoint {s} is missing")))?;
        if s == lseq && sha256(&b) != lsha {
            return Err(reconcile(format!(
                "archived checkpoint {s} differs from the local checkpoint"
            )));
        }
        central.report(repo, role, s, &b)?;
        sent += 1;
    }
    Ok(json!({"compared": true, "central_seq": cseq, "local_seq": lseq, "sent": sent}))
}
