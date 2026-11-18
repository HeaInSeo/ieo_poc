//! Receipt validation for the explicitly enabled isolated Codex role.
use ieo_evidence::record::Tokens;
use serde_json::Value;

pub(crate) fn receipt(
    stream: &str,
    delivery: &str,
    attempt: &str,
) -> Result<(Value, Option<Tokens>), String> {
    let mut lines = stream.lines();
    let r: Value = serde_json::from_str(lines.next().ok_or("missing Codex trial receipt")?)
        .map_err(|_| "invalid Codex trial receipt")?;
    if lines.next().is_some()
        || r["type"] != "ieo_codex_trial_receipt"
        || r["auth"] != "chatgpt"
        || r["delivery_id"] != delivery
        || r["attempt_id"] != attempt
    {
        return Err("Codex receipt does not match the admitted trial".into());
    }
    let thread = r["thread_id"].as_str().filter(|s| !s.is_empty());
    let turn = r["turn_id"].as_str().filter(|s| !s.is_empty());
    if thread.is_none()
        || turn.is_none()
        || r["result"]["id"].as_str() != turn
        || r["result"]["status"] != "completed"
    {
        return Err("Codex terminal turn is missing or does not match".into());
    }
    if r["usage"].is_null() {
        return Ok((r, None));
    }
    let usage = &r["usage"];
    let counter = |k: &str| {
        usage[k]
            .as_u64()
            .ok_or_else(|| format!("missing Codex {k}"))
    };
    let input = counter("inputTokens")?;
    let output = counter("outputTokens")?;
    let cached = counter("cachedInputTokens")?;
    let reasoning = counter("reasoningOutputTokens")?;
    let total = counter("totalTokens")?;
    let cache_creation = match usage.get("cacheWriteInputTokens") {
        None => 0, // The App Server protocol defaults this optional field to zero.
        Some(_) => counter("cacheWriteInputTokens")?,
    };
    let uncached = input
        .checked_sub(cached)
        .and_then(|n| n.checked_sub(cache_creation))
        .ok_or("Codex input breakdown exceeds total input")?;
    if input.checked_add(output) != Some(total) || reasoning > output {
        return Err("inconsistent Codex usage totals".into());
    }
    Ok((
        r,
        Some(Tokens {
            input: uncached,
            output,
            cache_read: cached,
            cache_creation,
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn codex_trial_only_charges_a_bound_complete_receipt() {
        let mut v = json!({
            "type":"ieo_codex_trial_receipt", "auth":"chatgpt", "delivery_id":"D-1",
            "attempt_id":"a", "thread_id":"th", "turn_id":"tu",
            "result":{"id":"tu","status":"completed"},
            "usage":{"inputTokens":30,"cachedInputTokens":10,"cacheWriteInputTokens":2,
                "outputTokens":9,"reasoningOutputTokens":3,"totalTokens":39}
        });
        assert_eq!(
            receipt(&v.to_string(), "D-1", "a").unwrap().1.unwrap(),
            Tokens {
                input: 18,
                output: 9,
                cache_read: 10,
                cache_creation: 2
            }
        );
        assert!(receipt(&v.to_string(), "D-2", "a").is_err());
        assert!(receipt(&format!("{v}\n{v}"), "D-1", "a").is_err());
        v["usage"]["totalTokens"] = Value::Null;
        assert!(receipt(&v.to_string(), "D-1", "a").is_err());
    }
}
