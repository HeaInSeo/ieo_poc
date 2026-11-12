//! Secret masking before anything is written.

use regex::bytes::Regex;
use sha2::{Digest, Sha256};

/// Shortest secret value accepted; shorter values would mask ordinary text.
pub const MIN_SECRET_LEN: usize = 8;

pub struct Redactor {
    values: Vec<Vec<u8>>,
    hashes: Vec<Vec<u8>>,
    token_pattern: Regex,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Masked {
    pub bytes: Vec<u8>,
    pub replacements: u32,
    pub secret_value_seen: bool,
}

impl Redactor {
    /// `secrets` are secret values held in memory only. Values are trimmed of
    /// surrounding whitespace (token files usually end with a newline).
    pub fn new(secrets: &[Vec<u8>]) -> Result<Self, String> {
        let mut values = Vec::new();
        let mut hashes = Vec::new();
        for s in secrets {
            let v = s.trim_ascii().to_vec();
            if v.len() < MIN_SECRET_LEN {
                return Err(format!(
                    "비밀 값이 {MIN_SECRET_LEN}바이트보다 짧아 가릴 수 없음(값은 출력하지 않음)"
                ));
            }
            hashes.push(hex::encode(Sha256::digest(&v)).into_bytes());
            values.push(v);
        }
        // Longest first so a value containing another is masked whole.
        values.sort_by_key(|v| std::cmp::Reverse(v.len()));
        Ok(Redactor {
            values,
            hashes,
            // Anthropic credential shapes (OAuth and API keys), as in the 01-04 harness.
            token_pattern: Regex::new(r"sk-ant-[A-Za-z0-9_\-]{8,}").expect("static regex"),
        })
    }

    pub fn source_count(&self) -> u32 {
        self.values.len() as u32
    }

    pub fn mask(&self, input: &[u8]) -> Masked {
        let mut bytes = input.to_vec();
        let mut replacements = 0;
        let mut seen = false;
        for v in &self.values {
            let n = replace_all(&mut bytes, v, b"[SECRET-VALUE-REDACTED]");
            seen |= n > 0;
            replacements += n;
        }
        for h in &self.hashes {
            let n = replace_all(&mut bytes, h, b"[SECRET-SHA256-REDACTED]");
            seen |= n > 0;
            replacements += n;
        }
        let n = self.token_pattern.find_iter(&bytes).count() as u32;
        if n > 0 {
            bytes = self
                .token_pattern
                .replace_all(&bytes, &b"sk-ant-[MASKED]"[..])
                .into_owned();
            replacements += n;
        }
        Masked {
            bytes,
            replacements,
            secret_value_seen: seen,
        }
    }
}

fn replace_all(hay: &mut Vec<u8>, needle: &[u8], with: &[u8]) -> u32 {
    if needle.is_empty() || hay.len() < needle.len() {
        return 0;
    }
    let mut out = Vec::with_capacity(hay.len());
    let mut i = 0;
    let mut n = 0;
    while i < hay.len() {
        if hay[i..].starts_with(needle) {
            out.extend_from_slice(with);
            i += needle.len();
            n += 1;
        } else {
            out.push(hay[i]);
            i += 1;
        }
    }
    *hay = out;
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_value_hash_and_token_shape() {
        let secret = b"super-secret-value-123\n".to_vec();
        let r = Redactor::new(&[secret]).unwrap();
        let h = hex::encode(Sha256::digest(b"super-secret-value-123"));
        let text = format!("a super-secret-value-123 b {h} c sk-ant-oat01-ABCDEFGHIJKL d");
        let m = r.mask(text.as_bytes());
        let s = String::from_utf8(m.bytes).unwrap();
        assert!(!s.contains("super-secret-value-123"));
        assert!(!s.contains(&h));
        assert!(!s.contains("ABCDEFGHIJKL"));
        assert_eq!(m.replacements, 3);
        assert!(m.secret_value_seen);
    }

    #[test]
    fn rejects_short_secret() {
        assert!(Redactor::new(&[b"abc".to_vec()]).is_err());
    }

    #[test]
    fn clean_text_untouched() {
        let r = Redactor::new(&[]).unwrap();
        let m = r.mask(b"nothing here");
        assert_eq!(m.bytes, b"nothing here");
        assert_eq!(m.replacements, 0);
        assert!(!m.secret_value_seen);
    }
}
