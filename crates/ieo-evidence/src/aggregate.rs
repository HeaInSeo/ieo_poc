//! Totals over runs: parent and children, failed, aborted, retried and review
//! runs are all included. Unmeasured runs are listed, never counted as 0.

use crate::record::{Applicability, EvidenceRecord, JudgmentStatus, Measure, Tokens};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SUMMARY_FORMAT: &str = "ieo-evidence-summary/v0";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Unmeasured {
    pub task_id: String,
    pub attempt_id: String,
    pub command_id: String,
    pub role: String,
    pub reason: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct UsageTotal {
    /// complete: every model run measured · incomplete: some unmeasured (the
    /// sum is then a sum over measured runs only, not a total) · no_model_runs.
    pub completeness: String,
    pub model_runs: u32,
    pub measured_runs: u32,
    /// Sum over measured runs only; null when no run was measured (never 0).
    pub tokens_measured_sum: Option<Tokens>,
    pub unmeasured: Vec<Unmeasured>,
    /// Child agents seen across model runs and how many had measured usage.
    pub children_seen: u32,
    pub children_measured: u32,
    /// Sum of CLI-reported cost estimates; null unless every model run reported one.
    pub cost_usd_reported_sum: Option<f64>,
    pub attribution_mismatches: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Summary {
    pub format: String,
    pub records: u32,
    pub task_ids: Vec<String>,
    pub by_judgment: BTreeMap<String, u32>,
    pub by_role: BTreeMap<String, u32>,
    pub retries: u32,
    pub timed_out: u32,
    pub wall_ms_sum: u64,
    pub usage: UsageTotal,
    pub usage_by_role: BTreeMap<String, UsageTotal>,
}

fn status_name(s: JudgmentStatus) -> &'static str {
    match s {
        JudgmentStatus::Succeeded => "succeeded",
        JudgmentStatus::Failed => "failed",
        JudgmentStatus::Incomplete => "incomplete",
        JudgmentStatus::ExpectedMatched => "expected_matched",
        JudgmentStatus::ExpectedNotMatched => "expected_not_matched",
    }
}

fn usage_total<'a>(records: impl Iterator<Item = &'a EvidenceRecord>) -> UsageTotal {
    let mut t = UsageTotal {
        completeness: "no_model_runs".into(),
        model_runs: 0,
        measured_runs: 0,
        tokens_measured_sum: None,
        unmeasured: vec![],
        children_seen: 0,
        children_measured: 0,
        cost_usd_reported_sum: Some(0.0),
        attribution_mismatches: vec![],
    };
    for r in records {
        if r.usage.applicability != Applicability::Model {
            continue;
        }
        t.model_runs += 1;
        t.children_seen += r.usage.children.len() as u32;
        t.children_measured += r
            .usage
            .children
            .iter()
            .filter(|c| c.usage.tokens().is_some())
            .count() as u32;
        match &r.usage.run {
            Measure::Measured { tokens, .. } => {
                t.measured_runs += 1;
                t.tokens_measured_sum
                    .get_or_insert_with(Tokens::default)
                    .add(tokens);
            }
            Measure::Unmeasured { reason } => t.unmeasured.push(Unmeasured {
                task_id: r.task_id.clone(),
                attempt_id: r.attempt_id.clone(),
                command_id: r.command_id.clone(),
                role: r.role.clone(),
                reason: reason.clone(),
            }),
        }
        t.cost_usd_reported_sum = match (t.cost_usd_reported_sum, r.usage.run_cost_usd) {
            (Some(a), Some(b)) => Some(a + b),
            _ => None,
        };
        if r.usage.attribution_check == "mismatch" {
            t.attribution_mismatches
                .push(format!("{}/{}/{}", r.task_id, r.attempt_id, r.command_id));
        }
    }
    if t.model_runs == 0 {
        t.cost_usd_reported_sum = None;
    } else {
        t.completeness = if t.unmeasured.is_empty() {
            "complete".into()
        } else {
            "incomplete".into()
        };
    }
    t
}

pub fn summarize(records: &[EvidenceRecord]) -> Summary {
    let mut by_judgment = BTreeMap::new();
    let mut by_role = BTreeMap::new();
    let mut task_ids: Vec<String> = records.iter().map(|r| r.task_id.clone()).collect();
    task_ids.sort();
    task_ids.dedup();
    for r in records {
        *by_judgment
            .entry(status_name(r.judgment.status).to_owned())
            .or_insert(0) += 1;
        *by_role.entry(r.role.clone()).or_insert(0) += 1;
    }
    let usage_by_role = by_role
        .keys()
        .map(|role| {
            (
                role.clone(),
                usage_total(records.iter().filter(|r| &r.role == role)),
            )
        })
        .collect();
    Summary {
        format: SUMMARY_FORMAT.into(),
        records: records.len() as u32,
        task_ids,
        by_judgment,
        by_role,
        retries: records.iter().filter(|r| r.retry_of.is_some()).count() as u32,
        timed_out: records
            .iter()
            .filter(|r| r.process.kind == crate::record::ProcessKind::TimedOut)
            .count() as u32,
        wall_ms_sum: records.iter().map(|r| r.duration_ms).sum(),
        usage: usage_total(records.iter()),
        usage_by_role,
    }
}
