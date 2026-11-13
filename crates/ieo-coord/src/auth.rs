//! Bearer credentials. The server keeps only SHA-256 digests of tokens; a token value is
//! never stored, logged or echoed. Names inside request bodies are never authorization.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    User,
    Service,
    Agent,
}

/// Server-side identity behind a bearer token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caller {
    pub name: String,
    pub kind: Kind,
    pub scopes: BTreeSet<String>,
    pub repos: BTreeSet<String>,
    /// Agents only: the one role the credential is bound to.
    pub role: Option<String>,
}

impl Caller {
    pub fn has(&self, scope: &str) -> bool {
        self.scopes.contains(scope)
    }

    pub fn in_repo(&self, repo: &str) -> bool {
        self.repos.contains(repo)
    }

    /// The 01-08 reference principal used by the claim/repro/read authorization rules.
    pub fn principal(&self) -> ieo_report::Principal {
        match (self.kind, &self.role) {
            (Kind::Agent, Some(role)) => ieo_report::Principal::Agent {
                repo_id: self.repos.iter().next().cloned().unwrap_or_default(),
                role: role.clone(),
            },
            _ => ieo_report::Principal::Client {
                repos: self.repos.clone(),
                can_register: self.has("sprints:register"),
            },
        }
    }
}

pub fn token_digest(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

const AGENT_SCOPES: [&str; 2] = ["agent:report", "agent:repro"];
const CLIENT_SCOPES: [&str; 7] = [
    "sprints:register",
    "sprints:read",
    "review:semantic",
    "coordinator:assign",
    "evidence:ingest",
    "judge:verify",
    "outbox:deliver",
];

/// Token digest → caller. Loaded once at start-up from a JSON file of digests.
#[derive(Debug, Default, Clone)]
pub struct Credentials {
    by_digest: BTreeMap<String, Caller>,
}

impl Credentials {
    /// `{"principals": [{"name", "token_sha256", "kind": "user|service|agent", "scopes": [],
    /// "repo_ids": [], "role"?}]}`. Refuses inconsistent entries instead of guessing:
    /// an agent has exactly one repo, a role and only `agent:*` scopes; other kinds never
    /// carry `agent:*` scopes or a role; `judge:verify` belongs to services only (policy G3).
    pub fn from_json(v: &Value) -> Result<Self, String> {
        let mut by_digest = BTreeMap::new();
        let mut names = BTreeSet::new();
        for (i, p) in v["principals"]
            .as_array()
            .ok_or("principals must be an array")?
            .iter()
            .enumerate()
        {
            let s = |k: &str| p[k].as_str().map(str::to_string);
            let list = |k: &str| -> BTreeSet<String> {
                p[k].as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            };
            let name = s("name").ok_or(format!("principals[{i}].name missing"))?;
            let digest = s("token_sha256").ok_or(format!("{name}: token_sha256 missing"))?;
            if digest.len() != 64
                || !digest
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            {
                return Err(format!("{name}: token_sha256 must be 64 lowercase hex"));
            }
            let kind = match s("kind").as_deref() {
                Some("user") => Kind::User,
                Some("service") => Kind::Service,
                Some("agent") => Kind::Agent,
                _ => return Err(format!("{name}: kind must be user|service|agent")),
            };
            let scopes = list("scopes");
            let repos = list("repo_ids");
            let role = s("role");
            match kind {
                Kind::Agent => {
                    if repos.len() != 1 {
                        return Err(format!("{name}: an agent is bound to exactly one repo"));
                    }
                    if !matches!(role.as_deref(), Some("development" | "guardrail")) {
                        return Err(format!("{name}: an agent needs role development|guardrail"));
                    }
                    if scopes.iter().any(|x| !AGENT_SCOPES.contains(&x.as_str())) {
                        return Err(format!("{name}: agents carry only agent:* scopes"));
                    }
                    if scopes.contains("agent:repro") && role.as_deref() != Some("guardrail") {
                        return Err(format!("{name}: agent:repro is for guardrail only"));
                    }
                }
                _ => {
                    if role.is_some() {
                        return Err(format!("{name}: only agents have a role"));
                    }
                    if scopes.iter().any(|x| !CLIENT_SCOPES.contains(&x.as_str())) {
                        return Err(format!("{name}: unknown or agent scope"));
                    }
                    if scopes.contains("judge:verify") && kind != Kind::Service {
                        return Err(format!("{name}: judge:verify is for services only"));
                    }
                }
            }
            if !names.insert(name.clone()) {
                return Err(format!("{name}: duplicate principal name"));
            }
            let caller = Caller {
                name,
                kind,
                scopes,
                repos,
                role,
            };
            if by_digest.insert(digest, caller).is_some() {
                return Err(format!("principals[{i}]: duplicate token digest"));
            }
        }
        Ok(Self { by_digest })
    }

    /// `Authorization: Bearer <token>` → caller, or `None` (401).
    pub fn authenticate(&self, header: Option<&str>) -> Option<&Caller> {
        let token = header?.strip_prefix("Bearer ")?.trim();
        if token.is_empty() {
            return None;
        }
        self.by_digest.get(&token_digest(token))
    }
}
