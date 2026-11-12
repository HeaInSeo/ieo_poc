//! Digest rule (plan §1.4): SHA-256 over the UTF-8 JSON of the spec without the
//! `digest` field, keys sorted, no insignificant whitespace, non-ASCII kept raw
//! (equivalent to Python `json.dumps(v, ensure_ascii=False, sort_keys=True,
//! separators=(',', ':'))`).

use serde_json::Value;
use sha2::{Digest, Sha256};

/// Canonical serialization. `serde_json::Map` is a `BTreeMap` (the
/// `preserve_order` feature is not enabled), so object keys come out sorted by
/// code point, matching Python's `sort_keys=True` for these documents.
pub fn canonical_json(value: &Value) -> String {
    // Serializing a `Value` cannot fail.
    serde_json::to_string(value).unwrap_or_default()
}

/// Digest of a spec, excluding its own top-level `digest` field.
pub fn spec_digest(spec: &Value) -> String {
    let mut copy = spec.clone();
    if let Value::Object(map) = &mut copy {
        map.remove("digest");
    }
    hex::encode(Sha256::digest(canonical_json(&copy).as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn keys_sorted_and_unicode_raw() {
        let v = json!({"b": 1, "a": "한글", "c": {"z": null, "y": [1, 2]}});
        assert_eq!(
            canonical_json(&v),
            r#"{"a":"한글","b":1,"c":{"y":[1,2],"z":null}}"#
        );
    }

    #[test]
    fn digest_ignores_digest_field() {
        let a = json!({"x": 1});
        let b = json!({"x": 1, "digest": "ff"});
        assert_eq!(spec_digest(&a), spec_digest(&b));
    }
}
