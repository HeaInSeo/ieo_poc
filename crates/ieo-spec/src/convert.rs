//! Human input (`HumanSprintInput_v0`) → SprintSpec v0 draft (Human_Sprint_Authoring_v0.md, steps 1–5).
//!
//! The converter never approves anything: it fills IDs/digest mechanically,
//! keeps the user's wording, and reports what still needs binding or semantic
//! review. Its output is always `registration_state = "draft"`.

use crate::digest::spec_digest;
use crate::rejection::{Code, Rejection};
use crate::schema::{format_rejections, human_input_validator};
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// Values bound by the coordinator/repo setup, not written by the user.
#[derive(Debug, Clone)]
pub struct ConversionContext {
    pub sprint_id: String,
    /// Two-digit sprint number used for task IDs (`NN-01`, `NN-02`, …).
    pub sprint_number: u8,
    pub revision: u64,
    pub plan_version: String,
    pub repository_url: Option<String>,
    pub timezone: String,
    pub track: String,
    pub entry_condition: Option<String>,
    pub acceptance_profile_id: String,
    pub acceptance_profile_revision: u64,
    /// Item IDs present in that approved profile.
    pub profile_items: BTreeSet<String>,
    /// Approved delegation block (`allowed`/`forbidden`/`approval_scope_ref`).
    pub delegation: Value,
    /// `limits` if already frozen; `None` → nulls + unresolved binding.
    pub limits: Option<Value>,
    pub human_validation: Value,
    pub sources: Value,
    /// Registered task IDs a human `after` entry may cite directly, with their plan version.
    pub registered_tasks: BTreeMap<String, String>,
    /// Reference recorded in each task's `references` (where the human input lives).
    pub input_ref: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConversionRecord {
    pub input_digest: String,
    pub spec: Value,
    /// Human task name → assigned task ID (kept so the original wording stays traceable).
    pub name_to_task_id: BTreeMap<String, String>,
    /// Human fields SprintSpec v0 cannot represent; preserved here, listed as a contract gap.
    pub preserved_unrepresented: Value,
    /// Tasks whose `done_when` is not covered by an approved profile item → semantic review / approval.
    pub semantic_review_required: Vec<String>,
    pub unresolved_bindings: Vec<String>,
}

pub fn convert_human_input(
    input: &Value,
    ctx: &ConversionContext,
) -> Result<ConversionRecord, Vec<Rejection>> {
    let format = format_rejections(human_input_validator(), input);
    if !format.is_empty() {
        return Err(format);
    }
    let tasks = input["tasks"].as_array().cloned().unwrap_or_default();
    let mut rej = Vec::new();

    // Step 1: stable IDs, in input order.
    let mut name_to_id: BTreeMap<String, String> = BTreeMap::new();
    for (i, t) in tasks.iter().enumerate() {
        let name = t["name"].as_str().unwrap_or_default().to_string();
        let id = format!("{:02}-{:02}", ctx.sprint_number, i + 1);
        if name_to_id.insert(name.clone(), id).is_some() {
            rej.push(Rejection::new(
                Code::DuplicateTaskName,
                format!("tasks[{i}].name"),
                format!("작업 이름 ‘{name}’이(가) 두 번 나옵니다."),
                "작업 이름을 서로 다르게 고치세요.",
            ));
        }
    }
    // Resolve `after` by name, or by an already-registered task ID.
    let mut prereqs: Vec<Vec<(String, String)>> = Vec::new();
    for (i, t) in tasks.iter().enumerate() {
        let mut list = Vec::new();
        for (j, a) in t["after"].as_array().into_iter().flatten().enumerate() {
            let a = a.as_str().unwrap_or_default();
            if let Some(id) = name_to_id.get(a) {
                list.push((id.clone(), ctx.plan_version.clone()));
            } else if let Some(ver) = ctx.registered_tasks.get(a) {
                list.push((a.to_string(), ver.clone()));
            } else {
                rej.push(Rejection::new(
                    Code::UnresolvedPrerequisite,
                    format!("tasks[{i}].after[{j}]"),
                    format!("‘{a}’라는 선행이 없습니다."),
                    "등록된 작업 이름/ID로 고치거나 새 작업을 먼저 추가하세요.",
                ));
            }
        }
        prereqs.push(list);
    }
    // Step 4 precheck: estimate range order (schema cannot express min ≤ max).
    for (i, t) in tasks.iter().enumerate() {
        let d = &t["expected_duration"];
        if let (Some(lo), Some(hi)) = (d["min_minutes"].as_u64(), d["max_minutes"].as_u64())
            && lo > hi
        {
            rej.push(Rejection::new(
                Code::EstimateRangeReversed,
                format!("tasks[{i}].expected_duration"),
                format!("최소 {lo}분이 최대 {hi}분보다 큽니다."),
                "min_minutes ≤ max_minutes가 되게 고치세요. 근거가 없으면 이 필드를 빼서 미산정으로 두세요.",
            ));
        }
    }
    // Cycles among the human tasks.
    let ids: Vec<String> = tasks
        .iter()
        .map(|t| {
            name_to_id
                .get(t["name"].as_str().unwrap_or_default())
                .cloned()
                .unwrap_or_default()
        })
        .collect();
    if let Some(i) = first_cycle_index(&ids, &prereqs) {
        rej.push(Rejection::new(
            Code::DependencyCycle,
            format!("tasks[{i}].after"),
            "작업 선행 관계가 순환합니다.",
            "순환하는 선행 중 하나를 빼세요.",
        ));
    }
    if !rej.is_empty() {
        rej.sort_by(|a, b| (a.code, &a.field).cmp(&(b.code, &b.field)));
        return Err(rej);
    }

    // Steps 2–4: build tasks.
    let mut semantic = Vec::new();
    let mut spec_tasks = Vec::new();
    for (i, t) in tasks.iter().enumerate() {
        let id = &ids[i];
        let item = format!("{id}.accept");
        if !ctx.profile_items.contains(&item) {
            semantic.push(id.clone());
        }
        let estimate = match t.get("expected_duration") {
            Some(d) => json!({
                "state": "measured",
                "range_seconds": {
                    "min": d["min_minutes"].as_u64().unwrap_or_default() * 60,
                    "max": d["max_minutes"].as_u64().unwrap_or_default() * 60,
                },
                "evidence": d["evidence"].clone(),
                "reason": null,
            }),
            None => json!({
                "state": "unestimated",
                "range_seconds": null,
                "evidence": [],
                "reason": "사용자 입력에 실측 근거가 있는 예상 범위가 없다.",
            }),
        };
        spec_tasks.push(json!({
            "task_id": id,
            "kind": "구현",
            "requirement_ids": [format!("{}-REQ-{}", ctx.sprint_id, &id[3..])],
            "prerequisites": prereqs[i].iter().map(|(p, v)| json!({
                "task_id": p, "plan_version": v, "required_state": "accepted"
            })).collect::<Vec<_>>(),
            "outputs": [{"artifact_id": format!("{id}-output"), "description": t["output"].clone(), "required": true}],
            "verification": t["how_to_check"].clone(),
            "acceptance": {
                "profile_id": ctx.acceptance_profile_id,
                "profile_revision": ctx.acceptance_profile_revision,
                "item_ids": [item],
                "criteria": t["done_when"].clone(),
            },
            "exit_condition": t["done_when"].clone(),
            "references": [ctx.input_ref],
            "legacy_v1_2_id": null,
            "status": "미착수",
            "evidence": [],
            "estimate": estimate,
        }));
    }

    // Step 3: bindings. Unknown values stay null + listed; nothing is invented.
    let mut unresolved = Vec::new();
    if ctx.repository_url.is_none() {
        unresolved.push("실제 repository_url".to_string());
    }
    let limits = ctx.limits.clone().unwrap_or_else(|| {
        unresolved.push("시간/토큰/교정 한도".to_string());
        json!({"elapsed_seconds": null, "total_tokens": null, "correction_attempts": null,
               "unresolved_reason": "한도가 아직 동결되지 않았다."})
    });
    let entry = ctx.entry_condition.clone().unwrap_or_else(|| {
        unresolved.push("착수 조건".to_string());
        "미지정(사용자 입력·바인딩 없음)".to_string()
    });
    if !semantic.is_empty() {
        unresolved.push(format!(
            "승인 프로파일에 없는 완료 기준: {}",
            semantic.join(", ")
        ));
    }
    let d = &input["deadline"];
    let deadline_state = d["state"].as_str().unwrap_or("undecided");
    let schedule = if deadline_state == "set" {
        json!({"timezone": ctx.timezone, "planned_start_at": null, "deadline_at": d["at"].clone(),
               "entry_condition": entry, "unresolved_reason": null, "deadline_state": "set"})
    } else {
        json!({"timezone": ctx.timezone, "planned_start_at": null, "deadline_at": null,
               "entry_condition": entry, "unresolved_reason": d["reason"].clone(), "deadline_state": deadline_state})
    };

    let mut spec = Map::new();
    spec.insert("schema_version".into(), json!("console.sprint.v0"));
    spec.insert("sprint_id".into(), json!(ctx.sprint_id));
    spec.insert("revision".into(), json!(ctx.revision));
    spec.insert("plan_version".into(), json!(ctx.plan_version));
    spec.insert("registration_state".into(), json!("draft"));
    spec.insert(
        "target".into(),
        json!({"repo_id": input["repo"].clone(), "repository_url": ctx.repository_url}),
    );
    spec.insert("title".into(), input["title"].clone());
    spec.insert("track".into(), json!(ctx.track));
    spec.insert("schedule".into(), schedule);
    spec.insert("delegation".into(), ctx.delegation.clone());
    spec.insert("limits".into(), limits);
    spec.insert("human_validation".into(), ctx.human_validation.clone());
    spec.insert("sources".into(), ctx.sources.clone());
    spec.insert("unresolved_bindings".into(), json!(unresolved));
    spec.insert("exit_condition".into(), input["goal"].clone());
    spec.insert("tasks".into(), Value::Array(spec_tasks));
    spec.insert("contract_revision".into(), json!(4));
    let mut spec = Value::Object(spec);
    // Step 5: digest last.
    let digest = spec_digest(&spec);
    spec["digest"] = json!(digest);

    Ok(ConversionRecord {
        input_digest: spec_digest(input),
        spec,
        name_to_task_id: name_to_id,
        preserved_unrepresented: json!({
            "out_of_scope": input.get("out_of_scope").cloned().unwrap_or(json!([])),
            "decisions": input.get("decisions").cloned().unwrap_or(json!([])),
        }),
        semantic_review_required: semantic,
        unresolved_bindings: unresolved,
    })
}

fn first_cycle_index(ids: &[String], prereqs: &[Vec<(String, String)>]) -> Option<usize> {
    let index: BTreeMap<&str, usize> = ids
        .iter()
        .enumerate()
        .map(|(i, s)| (s.as_str(), i))
        .collect();
    // 0 = unvisited, 1 = active, 2 = done
    fn visit(
        n: usize,
        index: &BTreeMap<&str, usize>,
        prereqs: &[Vec<(String, String)>],
        mark: &mut [u8],
    ) -> bool {
        match mark[n] {
            1 => return true,
            2 => return false,
            _ => {}
        }
        mark[n] = 1;
        for (p, _) in &prereqs[n] {
            if let Some(&m) = index.get(p.as_str())
                && visit(m, index, prereqs, mark)
            {
                return true;
            }
        }
        mark[n] = 2;
        false
    }
    let mut mark = vec![0u8; ids.len()];
    (0..ids.len()).find(|&i| visit(i, &index, prereqs, &mut mark))
}
