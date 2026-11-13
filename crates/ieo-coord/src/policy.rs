//! Judgment policy (freeze item L07, decision D12): a machine-readable file pinned by
//! version and SHA-256. The coordinator refuses to start on any other bytes, and every
//! assignment and verdict records the digest it was made under.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const POLICY_ID: &str = "ieo-judgment-policy";
pub const POLICY_VERSION: &str = "0.4.0";
/// SHA-256 of `contracts/judgment-policy-v0.json` (file bytes). A test recomputes it.
pub const PINNED_POLICY_DIGEST: &str =
    "7b54ae55fdee811222c30844d5ee9d673ab479478184eb341d74659c2689b759";

/// Rules this implementation enforces. The file must list exactly these.
pub const IMPLEMENTED_RULES: [&str; 7] = [
    "G1.prerequisite_evidence",
    "G2.sha_attribution",
    "G3.independent_judgment",
    "G4.cumulative_attempts",
    "G5.limit_reached",
    "RG2.approval_scope",
    "G7.exclusion_scope",
];

#[derive(Debug, Clone)]
pub struct Policy {
    pub digest: String,
}

pub fn digest_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Verify the policy bytes against the pinned identity. Fails closed.
pub fn load(bytes: &[u8]) -> Result<Policy, String> {
    let digest = digest_bytes(bytes);
    if digest != PINNED_POLICY_DIGEST {
        return Err(format!(
            "judgment policy digest {digest} is not the pinned {PINNED_POLICY_DIGEST}"
        ));
    }
    let v: Value = serde_json::from_slice(bytes).map_err(|e| format!("policy JSON: {e}"))?;
    if v["policy_id"] != POLICY_ID || v["version"] != POLICY_VERSION {
        return Err("policy id/version differ from the pinned ones".into());
    }
    let listed: BTreeSet<&str> = v["rules"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| r["rule_id"].as_str())
        .collect();
    let implemented: BTreeSet<&str> = IMPLEMENTED_RULES.into_iter().collect();
    if listed != implemented {
        return Err(format!(
            "policy rules {listed:?} differ from implemented {implemented:?}"
        ));
    }
    Ok(Policy { digest })
}
