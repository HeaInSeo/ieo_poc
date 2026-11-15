//! Claude Code `--output-format stream-json` parsing: result event and usage.
//!
//! Field sources were observed in the 01-04 probes (runtime contract rev 6 §4.2):
//! `result.usage` is the parent, `result.modelUsage` the whole session
//! including children, and each child's usage arrives in the Agent tool result
//! (`tool_use_result.usage`, keyed by `agentId`).
//!
//! **Children (02-04 D3, CLI 2.1.322).** `tool_use_result.usage` (and `totalTokens`) is the
//! child's *last* model response only, not the child's total; the child's `assistant` events in
//! the live stream are streaming snapshots (lower bounds). A child's total is therefore taken
//! from its transcript (`projects/<cwd>/<session>/subagents/agent-<agentId>.jsonl`): the final
//! usage of every response, one per `message.id` ([`child_transcript_total`]). Without a
//! transcript the child stays unmeasured (the reported last-response figure is kept as
//! information), and a resumed run whose share cannot be matched to parent + children is
//! unmeasured unless compaction explains the difference.

use crate::record::{
    Applicability, ChildUsage, Measure, PartialObservation, ResultEvent, ResumeBase, SessionMode,
    Tokens, Usage,
};
use serde_json::Value;
use std::collections::BTreeMap;

/// Tool names that start a child agent.
const AGENT_TOOLS: &[&str] = &["Agent", "Task"];

pub struct Parsed {
    pub result: ResultEvent,
    pub usage: Usage,
}

fn u64_field(v: &Value, key: &str) -> Option<u64> {
    v.get(key).and_then(Value::as_u64)
}

/// Tokens from an API-style usage object (`input_tokens` …). All four fields
/// must be present; a missing one makes the whole value unmeasured.
fn api_tokens(v: &Value) -> Result<Tokens, String> {
    let get = |k: &str| u64_field(v, k).ok_or_else(|| format!("usage.{k} 없음"));
    Ok(Tokens {
        input: get("input_tokens")?,
        output: get("output_tokens")?,
        cache_read: get("cache_read_input_tokens")?,
        cache_creation: get("cache_creation_input_tokens")?,
    })
}

/// Tokens from one `modelUsage` entry (`inputTokens` …).
fn model_tokens(v: &Value) -> Result<Tokens, String> {
    let get = |k: &str| u64_field(v, k).ok_or_else(|| format!("modelUsage.*.{k} 없음"));
    Ok(Tokens {
        input: get("inputTokens")?,
        output: get("outputTokens")?,
        cache_read: get("cacheReadInputTokens")?,
        cache_creation: get("cacheCreationInputTokens")?,
    })
}

/// A child's total from its transcript: the final usage of every model response, once per
/// `message.id`.
///
/// Fails closed: an unreadable line, a response without an id, a response without a final entry
/// (`stop_reason` set), two different final entries for one id, an entry of another agent, or no
/// response at all is an error (the child is then unmeasured). Identical repeated final entries
/// count once; non-final entries of a response are ignored. Returns the total, the number of
/// responses and the last response's usage (in file order).
pub fn child_transcript_total(
    agent_id: &str,
    transcript: &[u8],
) -> Result<(Tokens, usize, Tokens), String> {
    let mut finals: BTreeMap<String, Tokens> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut seen: BTreeMap<String, bool> = BTreeMap::new();
    for (n, line) in transcript.split(|b| *b == b'\n').enumerate() {
        if line.trim_ascii().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_slice(line)
            .map_err(|_| format!("자식 전사 {}행을 읽을 수 없음", n + 1))?;
        if v.get("type").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        let Some(usage) = v.pointer("/message/usage") else {
            continue;
        };
        if let Some(a) = v.get("agentId").and_then(Value::as_str)
            && a != agent_id
        {
            return Err(format!("자식 전사 {}행이 다른 자식({a})의 것", n + 1));
        }
        let id = v
            .pointer("/message/id")
            .and_then(Value::as_str)
            .ok_or(format!("자식 전사 {}행의 응답 id 없음", n + 1))?
            .to_owned();
        if !seen.contains_key(&id) {
            order.push(id.clone());
        }
        let is_final = v
            .pointer("/message/stop_reason")
            .is_some_and(|r| !r.is_null());
        seen.entry(id.clone()).or_insert(false);
        if !is_final {
            continue;
        }
        seen.insert(id.clone(), true);
        let t = api_tokens(usage).map_err(|e| format!("자식 전사 응답 {id}의 {e}"))?;
        match finals.get(&id) {
            Some(prev) if prev != &t => {
                return Err(format!(
                    "자식 전사 응답 {id}의 확정 사용량이 둘 이상이고 서로 다름"
                ));
            }
            _ => {
                finals.insert(id, t);
            }
        }
    }
    if let Some((id, _)) = seen.iter().find(|(_, f)| !**f) {
        return Err(format!(
            "자식 전사 응답 {id}의 확정 사용량 없음(진행 중 기록뿐)"
        ));
    }
    let last = order
        .last()
        .and_then(|id| finals.get(id))
        .cloned()
        .ok_or("자식 전사에 모델 응답 없음")?;
    let mut total = Tokens::default();
    for t in finals.values() {
        total.add(t);
    }
    Ok((total, finals.len(), last))
}

/// Parse a stream. `mode` and `base` say whether the CLI's session totals
/// include earlier runs (resume) and what those earlier totals were.
/// Without child transcripts every child is unmeasured (see the module note).
pub fn parse(stdout: &[u8], mode: SessionMode, base: Option<ResumeBase>) -> Parsed {
    parse_with_children(stdout, mode, base, &BTreeMap::new())
}

/// [`parse`] with child transcripts by `agentId` (the bytes of `agent-<agentId>.jsonl`).
pub fn parse_with_children(
    stdout: &[u8],
    mode: SessionMode,
    base: Option<ResumeBase>,
    transcripts: &BTreeMap<String, Vec<u8>>,
) -> Parsed {
    let reads = transcripts
        .iter()
        .map(|(a, b)| (a.clone(), Ok(b.clone())))
        .collect();
    parse_with_child_reads(stdout, mode, base, &reads)
}

/// [`parse_with_children`] with the result of reading each given transcript: a transcript that
/// was given but could not be read leaves its child unmeasured with the read error as the
/// reason, not as "no transcript" (review 608b367 F8).
pub fn parse_with_child_reads(
    stdout: &[u8],
    mode: SessionMode,
    base: Option<ResumeBase>,
    transcripts: &BTreeMap<String, Result<Vec<u8>, String>>,
) -> Parsed {
    let mut events = Vec::new();
    let mut non_json = 0u32;
    for line in stdout.split(|b| *b == b'\n') {
        if line.trim_ascii().is_empty() {
            continue;
        }
        match serde_json::from_slice::<Value>(line) {
            Ok(v) if v.is_object() => events.push(v),
            _ => non_json += 1,
        }
    }
    let ty = |e: &Value| e.get("type").and_then(Value::as_str).map(str::to_owned);
    let results: Vec<&Value> = events
        .iter()
        .filter(|e| ty(e).as_deref() == Some("result"))
        .collect();
    let chosen = results
        .iter()
        .rev()
        .find(|e| u64_field(e, "num_turns").unwrap_or(0) >= 1)
        .or(results.last())
        .copied();
    let init = events.iter().find(|e| {
        ty(e).as_deref() == Some("system")
            && e.get("subtype").and_then(Value::as_str) == Some("init")
    });
    let rate_limit_statuses = events
        .iter()
        .filter(|e| ty(e).as_deref() == Some("rate_limit_event"))
        .map(|e| {
            e.pointer("/rate_limit_info/status")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_owned()
        })
        .collect();
    let s = |v: Option<&Value>, k: &str| {
        v.and_then(|e| e.get(k))
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    let result = ResultEvent {
        count: results.len() as u32,
        present: chosen.is_some(),
        subtype: s(chosen, "subtype"),
        is_error: chosen
            .and_then(|e| e.get("is_error"))
            .and_then(Value::as_bool),
        session_id: s(chosen, "session_id").or_else(|| s(init, "session_id")),
        num_turns: chosen.and_then(|e| u64_field(e, "num_turns")),
        stop_reason: s(chosen, "stop_reason"),
        runtime_version: s(init, "claude_code_version"),
        non_json_lines: non_json,
        rate_limit_statuses,
        compact_boundaries: events
            .iter()
            .filter(|e| {
                ty(e).as_deref() == Some("system")
                    && e.get("subtype").and_then(Value::as_str) == Some("compact_boundary")
            })
            .count() as u32,
    };

    // Children: every Agent tool_use in the parent stream, matched to its result.
    let mut child_ids: Vec<String> = Vec::new();
    for e in &events {
        if ty(e).as_deref() != Some("assistant")
            || e.get("parent_tool_use_id").is_some_and(|p| !p.is_null())
        {
            continue;
        }
        if let Some(content) = e.pointer("/message/content").and_then(Value::as_array) {
            for c in content {
                if c.get("type").and_then(Value::as_str) == Some("tool_use")
                    && c.get("name")
                        .and_then(Value::as_str)
                        .is_some_and(|n| AGENT_TOOLS.contains(&n))
                    && let Some(id) = c.get("id").and_then(Value::as_str)
                    && !child_ids.iter().any(|x| x == id)
                {
                    child_ids.push(id.to_owned());
                }
            }
        }
    }
    let mut child_results: BTreeMap<String, &Value> = BTreeMap::new();
    for e in &events {
        if ty(e).as_deref() != Some("user") {
            continue;
        }
        let Some(tur) = e.get("tool_use_result").filter(|v| v.is_object()) else {
            continue;
        };
        if let Some(content) = e.pointer("/message/content").and_then(Value::as_array) {
            for c in content {
                if c.get("type").and_then(Value::as_str) == Some("tool_result")
                    && let Some(id) = c.get("tool_use_id").and_then(Value::as_str)
                {
                    child_results.insert(id.to_owned(), tur);
                }
            }
        }
    }
    let children: Vec<ChildUsage> = child_ids
        .iter()
        .map(|id| match child_results.get(id) {
            None => ChildUsage {
                tool_use_id: id.clone(),
                agent_id: None,
                usage: Measure::unmeasured("자식 결과(tool_result) 없음: 중단·실패 또는 기록 누락"),
                reported_last_response: None,
            },
            Some(tur) => {
                let agent_id = tur
                    .get("agentId")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let reported = tur.get("usage").map(api_tokens);
                let usage = match (&agent_id, &reported) {
                    (_, Some(Err(e))) => Measure::unmeasured(format!("자식 tool_use_result의 {e}")),
                    (None, _) => Measure::unmeasured("자식 tool_use_result에 agentId 없음"),
                    (Some(a), _) => match transcripts.get(a) {
                        None => Measure::unmeasured(
                            "자식 전사 없음: tool_use_result.usage는 마지막 응답만일 수 있어 자식 전체로 쓰지 않음",
                        ),
                        Some(Err(e)) => {
                            Measure::unmeasured(format!("자식 전사를 읽지 못함: {e}"))
                        }
                        Some(Ok(bytes)) => match child_transcript_total(a, bytes) {
                            Err(e) => Measure::unmeasured(e),
                            // the transcript must agree with what the stream reported
                            Ok((_, _, last)) if matches!(&reported, Some(Ok(r)) if r != &last) => {
                                Measure::unmeasured(
                                    "자식 전사의 마지막 응답이 tool_use_result.usage와 다름",
                                )
                            }
                            Ok((total, n, _)) => Measure::Measured {
                                tokens: total,
                                source: format!(
                                    "자식 전사: 응답 {n}개의 확정 사용량 합(message.id별 1회)"
                                ),
                            },
                        },
                    },
                };
                ChildUsage {
                    tool_use_id: id.clone(),
                    agent_id,
                    usage,
                    reported_last_response: match reported {
                        Some(Ok(t)) => Some(t),
                        _ => None,
                    },
                }
            }
        })
        .collect();

    let missing = "결과 사건 없음(중단·시간 초과·출력 누락)";
    let parent = match chosen {
        None => Measure::unmeasured(missing),
        Some(r) => match r.get("usage").map(api_tokens) {
            Some(Ok(t)) => Measure::Measured {
                tokens: t,
                source: "result.usage".into(),
            },
            Some(Err(e)) => Measure::unmeasured(format!("result.{e}")),
            None => Measure::unmeasured("result.usage 없음"),
        },
    };
    let mut by_model = BTreeMap::new();
    let model_usage = chosen.and_then(|r| r.get("modelUsage"));
    let session_from_models = match model_usage.and_then(Value::as_object) {
        None => Err("result.modelUsage 없음".to_owned()),
        Some(m) if m.is_empty() => Err("result.modelUsage 비어 있음".to_owned()),
        Some(m) => {
            let mut total = Tokens::default();
            let mut err = None;
            for (model, v) in m {
                match model_tokens(v) {
                    Ok(t) => {
                        total.add(&t);
                        by_model.insert(model.clone(), t);
                    }
                    Err(e) => err = Some(e),
                }
            }
            err.map_or(Ok(total), Err)
        }
    };
    let all_children: Option<Tokens> = children.iter().try_fold(Tokens::default(), |mut acc, c| {
        acc.add(c.usage.tokens()?);
        Some(acc)
    });
    let session = match (&session_from_models, chosen) {
        (_, None) => Measure::unmeasured(missing),
        (Ok(t), _) => Measure::Measured {
            tokens: t.clone(),
            source: "result.modelUsage".into(),
        },
        // result.usage is per run; it equals the session only for a new session.
        (Err(e), _) if mode != SessionMode::New => Measure::unmeasured(format!(
            "{e}; 새 세션이 아니어서 부모·자식 합으로 대신할 수 없음"
        )),
        (Err(e), _) => match (parent.tokens(), &all_children) {
            (Some(p), Some(c)) => {
                let mut t = p.clone();
                t.add(c);
                Measure::Measured {
                    tokens: t,
                    source: format!("result.usage + 자식 전사 합 ({e})"),
                }
            }
            _ => Measure::unmeasured(format!("{e}; 부모·자식 사용량으로도 합산 불가")),
        },
    };
    let cost = chosen
        .and_then(|r| r.get("total_cost_usd"))
        .and_then(Value::as_f64);
    let (mut run, mut run_cost_usd) = run_share(
        &session,
        cost,
        mode,
        base.as_ref(),
        result.session_id.as_deref(),
    );
    // Only an independent session total (modelUsage) can be checked against
    // parent + children; the fallback is that sum itself.
    let run_checked = run.tokens().filter(|_| session_from_models.is_ok());
    let (attribution_check, attribution_detail) = match (
        run_checked,
        parent.tokens(),
        &all_children,
    ) {
        (Some(r), Some(p), Some(c)) => {
            let mut sum = p.clone();
            sum.add(c);
            if &sum == r {
                ("consistent".to_owned(), None)
            } else {
                (
                    "mismatch".to_owned(),
                    Some(format!(
                        "회차 사용량 {r:?} != 부모+자식 {sum:?}: 부모·자식에 귀속되지 않은 모델 호출(예: 압축)"
                    )),
                )
            }
        }
        _ => ("not_checked".to_owned(), None),
    };
    // A resumed run's share is only as good as its base. Without compaction (or
    // another known unattributed call) the share must equal parent + children;
    // otherwise the base was not the run actually resumed (01-04 L2 after the
    // concurrent N4a/N4b branch) and the share is withdrawn, not reported.
    // An unchecked share (a child without a confirmed total) is withdrawn the same way: the
    // share is not trusted on the session difference alone (02-04 D3).
    if mode == SessionMode::Resume && result.compact_boundaries == 0 && run.tokens().is_some() {
        match attribution_check.as_str() {
            "mismatch" => {
                run = Measure::unmeasured(
                    "재개 회차 몫이 부모+자식과 다르고 압축 사건 없음: 기준 회차가 실제 재개 대상이 아니거나 귀속되지 않은 모델 호출이 있음",
                );
                run_cost_usd = None;
            }
            "not_checked" => {
                run = Measure::unmeasured(
                    "재개 회차 몫을 부모+자식과 대조할 수 없음(확정되지 않은 자식 사용량 등)",
                );
                run_cost_usd = None;
            }
            _ => {}
        }
    }
    // With compaction the share may exceed parent + children (compaction calls are not in
    // either) but never fall below it, and every part must be confirmed (review 608b367 F5).
    if mode == SessionMode::Resume && result.compact_boundaries > 0 && run.tokens().is_some() {
        let below = match (run.tokens(), parent.tokens(), &all_children) {
            (Some(r), Some(p), Some(c)) => {
                let mut sum = p.clone();
                sum.add(c);
                r.input < sum.input
                    || r.output < sum.output
                    || r.cache_read < sum.cache_read
                    || r.cache_creation < sum.cache_creation
            }
            _ => true,
        };
        if below {
            run = Measure::unmeasured(
                "압축이 있는 재개 회차의 몫이 부모+자식보다 작거나 대조할 수 없음(확정되지 않은 자식 등)",
            );
            run_cost_usd = None;
        }
    }

    let mut partial_observations = Vec::new();
    if chosen.is_none() {
        let mut last: BTreeMap<String, PartialObservation> = BTreeMap::new();
        for e in &events {
            if ty(e).as_deref() != Some("assistant") {
                continue;
            }
            let (Some(id), Some(Ok(t))) = (
                e.pointer("/message/id").and_then(Value::as_str),
                e.pointer("/message/usage").map(api_tokens),
            ) else {
                continue;
            };
            last.insert(
                id.to_owned(),
                PartialObservation {
                    message_id: id.to_owned(),
                    parent_tool_use_id: e
                        .get("parent_tool_use_id")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    tokens: t,
                },
            );
        }
        partial_observations = last.into_values().collect();
    }

    let usage = Usage {
        applicability: Applicability::Model,
        session_mode: mode,
        resume_base: base,
        run,
        session,
        parent,
        children,
        attribution_check,
        attribution_detail,
        by_model,
        cost_usd_reported: cost,
        run_cost_usd,
        partial_observations,
        internal_retries: "unmeasured".into(),
    };
    Parsed { result, usage }
}

/// This run's share of the session totals.
fn run_share(
    session: &Measure,
    cost: Option<f64>,
    mode: SessionMode,
    base: Option<&ResumeBase>,
    session_id: Option<&str>,
) -> (Measure, Option<f64>) {
    let total = match session {
        Measure::Unmeasured { reason } => return (Measure::unmeasured(reason.clone()), None),
        Measure::Measured { tokens, source } => (tokens, source),
    };
    match (mode, base) {
        (SessionMode::New, _) => (
            Measure::Measured {
                tokens: total.0.clone(),
                source: format!("새 세션: {}", total.1),
            },
            cost,
        ),
        (SessionMode::Unknown, _) => (
            Measure::unmeasured(
                "세션 방식(새/재개) 미지정: 세션 누적에서 이번 회차 몫을 정할 수 없음",
            ),
            None,
        ),
        (SessionMode::Resume, None) => (
            Measure::unmeasured(
                "재개 실행인데 기준 회차 기록 없음: 세션 누적에서 이번 회차 몫을 분리할 수 없음",
            ),
            None,
        ),
        (SessionMode::Resume, Some(b)) => {
            if b.session_id.is_none() || b.session_id.as_deref() != session_id {
                return (
                    Measure::unmeasured(format!(
                        "기준 회차 session_id {:?} != 이번 {:?}",
                        b.session_id, session_id
                    )),
                    None,
                );
            }
            let Some(bt) = b.session.tokens() else {
                return (Measure::unmeasured("기준 회차의 세션 누적이 미계측"), None);
            };
            let t = total.0;
            let d = |a: u64, b: u64| a.checked_sub(b);
            match (
                d(t.input, bt.input),
                d(t.output, bt.output),
                d(t.cache_read, bt.cache_read),
                d(t.cache_creation, bt.cache_creation),
            ) {
                (Some(input), Some(output), Some(cache_read), Some(cache_creation)) => (
                    Measure::Measured {
                        tokens: Tokens {
                            input,
                            output,
                            cache_read,
                            cache_creation,
                        },
                        source: format!(
                            "재개: {} − 기준 회차 {}/{}/{} 누적",
                            total.1, b.task_id, b.attempt_id, b.command_id
                        ),
                    },
                    match (cost, b.cost_usd_reported) {
                        (Some(c), Some(bc)) if c >= bc => Some(c - bc),
                        _ => None,
                    },
                ),
                _ => (
                    Measure::unmeasured(
                        "세션 누적이 기준 회차보다 작음: 기준이 직전 회차가 아님(동시 재개·분기 가능)",
                    ),
                    None,
                ),
            }
        }
    }
}
