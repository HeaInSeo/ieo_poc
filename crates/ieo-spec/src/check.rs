//! Reference layer and binding state.

use crate::digest::spec_digest;
use crate::rejection::{Code, Rejection};
use crate::schema::{format_rejections, sprint_spec_validator};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// What the caller is asking for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckMode {
    /// Store/review a draft: format + reference checks only. Never executable intake.
    Draft,
    /// Register for execution: additionally requires `ready` with no unresolved bindings.
    Executable,
}

#[derive(Debug, Clone)]
struct RegisteredTask {
    sprint_id: String,
    plan_version: String,
    prerequisites: Vec<String>,
}

/// Already-registered state the checker resolves references against.
#[derive(Debug, Default, Clone)]
pub struct Registry {
    tasks: BTreeMap<String, RegisteredTask>,
    revisions: BTreeMap<(String, u64), String>,
    profiles: BTreeMap<(String, u64), BTreeSet<String>>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an approved acceptance profile (`profile_id`, `revision`, item IDs).
    pub fn add_profile(
        &mut self,
        id: &str,
        revision: u64,
        items: impl IntoIterator<Item = String>,
    ) {
        self.profiles
            .insert((id.to_string(), revision), items.into_iter().collect());
    }

    /// Load `inspection_profiles` from a plan manifest (`Console_Coordinator_Sprints_v0.json`).
    pub fn add_profiles_from_manifest(&mut self, manifest: &Value) {
        for p in manifest["inspection_profiles"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let id = p["profile_id"].as_str().unwrap_or_default();
            let rev = p["revision"].as_u64().unwrap_or_default();
            let items = p["items"]
                .as_object()
                .map(|m| m.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default();
            self.add_profile(id, rev, items);
        }
    }

    /// Record a spec as registered (its tasks become resolvable prerequisites).
    pub fn add_spec(&mut self, spec: &Value) {
        let sprint_id = spec["sprint_id"].as_str().unwrap_or_default().to_string();
        let plan_version = spec["plan_version"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        if let (Some(rev), Some(d)) = (spec["revision"].as_u64(), spec["digest"].as_str()) {
            self.revisions
                .insert((sprint_id.clone(), rev), d.to_string());
        }
        for t in spec["tasks"].as_array().into_iter().flatten() {
            let id = t["task_id"].as_str().unwrap_or_default().to_string();
            let prerequisites = t["prerequisites"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|p| p["task_id"].as_str().map(str::to_string))
                .collect();
            self.tasks.insert(
                id,
                RegisteredTask {
                    sprint_id: sprint_id.clone(),
                    plan_version: plan_version.clone(),
                    prerequisites,
                },
            );
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CheckState {
    /// Passed format+reference(+binding) checks; next step is independent semantic review (P01).
    /// This is not execution intake and not completion.
    AcceptedForSemanticReview,
    /// Same sprint_id/revision/digest already registered (idempotent re-submit, HTTP 200).
    AlreadyRegistered,
    Rejected,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckOutcome {
    pub state: CheckState,
    pub computed_digest: Option<String>,
    pub rejections: Vec<Rejection>,
}

impl CheckOutcome {
    /// HTTP status for the registration API: 201 accepted, 200 idempotent, else the most specific rejection status.
    pub fn http_status(&self) -> u16 {
        match self.state {
            CheckState::AcceptedForSemanticReview => 201,
            CheckState::AlreadyRegistered => 200,
            CheckState::Rejected => {
                let codes: BTreeSet<u16> = self
                    .rejections
                    .iter()
                    .map(|r| r.code.http_status())
                    .collect();
                // Format errors first (400), then conflict (409), then reference (422).
                [400, 409, 422]
                    .into_iter()
                    .find(|s| codes.contains(s))
                    .unwrap_or(422)
            }
        }
    }
}

pub fn check_registration(spec: &Value, registry: &Registry, mode: CheckMode) -> CheckOutcome {
    // 1. format. Reference checks on a malformed document would be unreliable, so stop here.
    let format = format_rejections(sprint_spec_validator(), spec);
    if !format.is_empty() {
        return CheckOutcome {
            state: CheckState::Rejected,
            computed_digest: None,
            rejections: format,
        };
    }
    let mut rej = Vec::new();
    let digest = spec_digest(spec);
    let sprint_id = spec["sprint_id"].as_str().unwrap_or_default();
    let revision = spec["revision"].as_u64().unwrap_or_default();

    // 2. digest
    if spec["digest"].as_str() != Some(digest.as_str()) {
        rej.push(Rejection::new(
            Code::DigestMismatch,
            "digest",
            format!("digest가 문서 내용과 다릅니다(계산값 {digest})."),
            "digest는 변환기/등록 도구가 계산합니다. 내용을 바꿨다면 새 revision으로 다시 계산하세요.",
        ));
    }

    // 3. revision identity
    if let Some(existing) = registry.revisions.get(&(sprint_id.to_string(), revision)) {
        if *existing == digest && rej.is_empty() {
            return CheckOutcome {
                state: CheckState::AlreadyRegistered,
                computed_digest: Some(digest),
                rejections: vec![],
            };
        }
        if *existing != digest {
            rej.push(Rejection::new(
                Code::RevisionConflict,
                "revision",
                format!("{sprint_id} revision {revision}은(는) 다른 내용으로 이미 등록됐습니다."),
                "기존 revision을 바꾸지 말고 revision을 올려 새로 제출하세요.",
            ));
        }
    }

    let tasks: Vec<&Value> = spec["tasks"]
        .as_array()
        .map(|a| a.iter().collect())
        .unwrap_or_default();

    // 4. duplicate task IDs
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for (i, t) in tasks.iter().enumerate() {
        let id = t["task_id"].as_str().unwrap_or_default();
        if let Some(first) = seen.insert(id, i) {
            rej.push(Rejection::new(
                Code::DuplicateTaskId,
                format!("tasks[{i}].task_id"),
                format!("작업 ID {id}이(가) tasks[{first}]와 중복됩니다."),
                "작업 ID를 고유하게 고치세요.",
            ));
        } else if let Some(other) = registry.tasks.get(id).filter(|r| r.sprint_id != sprint_id) {
            rej.push(Rejection::new(
                Code::DuplicateTaskId,
                format!("tasks[{i}].task_id"),
                format!(
                    "작업 ID {id}은(는) 이미 {}에 등록돼 있습니다.",
                    other.sprint_id
                ),
                "다른 스프린트와 겹치지 않는 작업 ID를 쓰세요.",
            ));
        }
    }

    // 5. prerequisites resolvable (in this spec, or registered with the required plan version)
    let local: BTreeSet<&str> = seen.keys().copied().collect();
    let own_version = spec["plan_version"].as_str().unwrap_or_default();
    for (i, t) in tasks.iter().enumerate() {
        for (j, p) in t["prerequisites"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            let pid = p["task_id"].as_str().unwrap_or_default();
            let pver = p["plan_version"].as_str().unwrap_or_default();
            // A task of this spec only exists in this spec's plan version.
            let ok = (local.contains(pid) && pver == own_version)
                || registry
                    .tasks
                    .get(pid)
                    .is_some_and(|r| r.sprint_id != sprint_id && r.plan_version == pver);
            if !ok {
                rej.push(Rejection::new(
                    Code::UndefinedPrerequisite,
                    format!("tasks[{i}].prerequisites[{j}].task_id"),
                    format!("선행 작업 {pid}({pver})이(가) 이 명세에도 등록 목록에도 없습니다."),
                    "등록된 작업 ID·계획 버전으로 고치거나 선행 명세를 먼저 등록하세요.",
                ));
            }
        }
    }

    // 6. cycles over this spec + registered graph (excluding older revisions of this sprint)
    let mut graph: BTreeMap<String, Vec<String>> = registry
        .tasks
        .iter()
        .filter(|(_, r)| r.sprint_id != sprint_id)
        .map(|(k, r)| (k.clone(), r.prerequisites.clone()))
        .collect();
    for t in &tasks {
        let id = t["task_id"].as_str().unwrap_or_default().to_string();
        let pre = t["prerequisites"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| p["task_id"].as_str().map(str::to_string))
            .collect();
        graph.insert(id, pre);
    }
    if let Some(cycle) = find_cycle(&graph, &local) {
        let idx = tasks
            .iter()
            .position(|t| t["task_id"].as_str() == Some(cycle[0].as_str()))
            .unwrap_or(0);
        rej.push(Rejection::new(
            Code::DependencyCycle,
            format!("tasks[{idx}].prerequisites"),
            format!("선행 관계가 순환합니다: {}", cycle.join(" → ")),
            "순환의 한 간선을 제거하거나 작업을 나누세요.",
        ));
    }

    // 7. acceptance profile items
    for (i, t) in tasks.iter().enumerate() {
        let a = &t["acceptance"];
        let pid = a["profile_id"].as_str().unwrap_or_default();
        let prev = a["profile_revision"].as_u64().unwrap_or_default();
        match registry.profiles.get(&(pid.to_string(), prev)) {
            None => rej.push(Rejection::new(
                Code::UnknownProfileItem,
                format!("tasks[{i}].acceptance.profile_id"),
                format!("승인 프로파일 {pid} revision {prev}이(가) 없습니다."),
                "승인된 프로파일 ID·revision을 쓰세요. 새 기준은 정책 소유자 승인 뒤 프로파일에 추가됩니다.",
            )),
            Some(items) => {
                for (k, item) in a["item_ids"].as_array().into_iter().flatten().enumerate() {
                    let item = item.as_str().unwrap_or_default();
                    if !items.contains(item) {
                        rej.push(Rejection::new(
                            Code::UnknownProfileItem,
                            format!("tasks[{i}].acceptance.item_ids[{k}]"),
                            format!("프로파일 {pid} r{prev}에 항목 {item}이(가) 없습니다."),
                            "프로파일에 있는 항목을 쓰거나, 새 완료 기준은 의미 검토·승인으로 돌리세요.",
                        ));
                    }
                }
            }
        }
    }

    // 8. binding state (executable intake only)
    if mode == CheckMode::Executable {
        if spec["registration_state"].as_str() != Some("ready") {
            rej.push(Rejection::new(
                Code::UnboundDraft,
                "registration_state",
                "draft 명세는 실행 가능 등록을 할 수 없습니다.",
                "실제 repo·한도·출처 바인딩을 채운 새 revision을 ready로 제출하세요.",
            ));
        }
        for (i, b) in spec["unresolved_bindings"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            rej.push(Rejection::new(
                Code::UnboundDraft,
                format!("unresolved_bindings[{i}]"),
                format!("미바인딩: {}", b.as_str().unwrap_or_default()),
                "이 항목을 바인딩하고 목록에서 제거한 새 revision을 제출하세요.",
            ));
        }
    }

    rej.sort_by(|a, b| (a.code, &a.field).cmp(&(b.code, &b.field)));
    let state = if rej.is_empty() {
        CheckState::AcceptedForSemanticReview
    } else {
        CheckState::Rejected
    };
    CheckOutcome {
        state,
        computed_digest: Some(digest),
        rejections: rej,
    }
}

/// Return one cycle (as a path of task IDs, first repeated at the end) that passes through a local task.
fn find_cycle(
    graph: &BTreeMap<String, Vec<String>>,
    local: &BTreeSet<&str>,
) -> Option<Vec<String>> {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        Active,
        Done,
    }
    fn visit(
        n: &str,
        graph: &BTreeMap<String, Vec<String>>,
        marks: &mut BTreeMap<String, Mark>,
        stack: &mut Vec<String>,
    ) -> Option<Vec<String>> {
        match marks.get(n) {
            Some(Mark::Done) => return None,
            Some(Mark::Active) => {
                let start = stack.iter().position(|s| s == n).unwrap_or(0);
                let mut cyc = stack[start..].to_vec();
                cyc.push(n.to_string());
                return Some(cyc);
            }
            None => {}
        }
        marks.insert(n.to_string(), Mark::Active);
        stack.push(n.to_string());
        for p in graph.get(n).into_iter().flatten() {
            if let Some(c) = visit(p, graph, marks, stack) {
                return Some(c);
            }
        }
        stack.pop();
        marks.insert(n.to_string(), Mark::Done);
        None
    }
    let mut marks = BTreeMap::new();
    for n in local {
        let mut stack = Vec::new();
        if let Some(c) = visit(n, graph, &mut marks, &mut stack) {
            return Some(c);
        }
    }
    None
}
