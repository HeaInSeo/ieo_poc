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

/// Billing observations are short-lived and cannot extend the central approval.
pub(crate) fn approval_expiry(approval: &Value, now: u64) -> Result<u64, String> {
    const BILLING_FRESH_SECONDS: u64 = 300;
    let billing = &approval["billing"];
    let expiry = approval["expires_unix"]
        .as_u64()
        .zip(billing["verified_unix"].as_u64().filter(|t| *t <= now))
        .and_then(|(approval_end, observed)| {
            observed
                .checked_add(BILLING_FRESH_SECONDS)
                .map(|billing_end| approval_end.min(billing_end))
        });
    match expiry {
        Some(end)
            if now < end
                && approval["max_dispatches"] == 1
                && approval["additional_usd"] == 0
                && billing["auto_reload_disabled"] == true
                && billing["paid_paths_blocked"] == true
                && billing["api_fallback_blocked"] == true =>
        {
            Ok(end)
        }
        _ => Err("expired or unverified USD 0 controls for the assigned role".into()),
    }
}

pub(crate) fn check_subscription(v: &Value, limit_id: &str, now: u64) -> Result<(), String> {
    let bucket = &v["rateLimitsByLimitId"][limit_id];
    let credits = &bucket["credits"];
    let available = |w: &Value| {
        w["usedPercent"].as_u64().is_some_and(|n| n < 100)
            && w["resetsAt"].as_u64().is_some_and(|t| now < t)
    };
    if v["account"]["type"] != "chatgpt"
        || bucket["limitId"] != limit_id
        || !available(&bucket["primary"])
        || !bucket
            .get("secondary")
            .is_some_and(|w| w.is_null() || available(w))
        || !bucket
            .get("rateLimitReachedType")
            .is_some_and(Value::is_null)
        || credits["hasCredits"] != false
        || credits["unlimited"] != false
        || credits["balance"]
            .as_str()
            .and_then(|s| s.parse::<f64>().ok())
            != Some(0.0)
        || bucket["spendControlReached"] != false
    {
        return Err("Codex requires a fresh available ChatGPT bucket with zero credits".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn codex_trial_billing_is_time_bounded() {
        let mut approval = json!({"expires_unix":1000, "max_dispatches":1, "additional_usd":0,
            "billing":{"verified_unix":400, "auto_reload_disabled":true, "paid_paths_blocked":true,
                       "api_fallback_blocked":true}});
        assert!(approval_expiry(&approval, 399).is_err());
        assert_eq!(approval_expiry(&approval, 400), Ok(700));
        assert_eq!(approval_expiry(&approval, 699), Ok(700));
        assert!(approval_expiry(&approval, 700).is_err());
        approval["expires_unix"] = json!(600);
        assert_eq!(approval_expiry(&approval, 599), Ok(600));
        assert!(approval_expiry(&approval, 600).is_err());
        approval["billing"]["verified_unix"] = Value::Null;
        assert!(approval_expiry(&approval, 450).is_err());
    }

    #[test]
    fn codex_trial_preflight_refuses_wrong_auth_quota_or_credits() {
        let good = json!({"account":{"type":"chatgpt"},"rateLimitsByLimitId":{"codex":{
            "limitId":"codex","primary":{"usedPercent":19,"resetsAt":200},
            "secondary":null,"rateLimitReachedType":null,"spendControlReached":false,
            "credits":{"hasCredits":false,"unlimited":false,"balance":"0"}
        }}});
        assert!(check_subscription(&good, "codex", 100).is_ok());
        assert!(check_subscription(&good, "other", 100).is_err());
        assert!(check_subscription(&good, "codex", 200).is_err());
        let mut bad = good.clone();
        bad["account"]["type"] = json!("apiKey");
        assert!(check_subscription(&bad, "codex", 100).is_err());
        for (field, value) in [
            ("balance", json!("1")),
            ("hasCredits", json!(true)),
            ("unlimited", json!(true)),
        ] {
            let mut bad = good.clone();
            bad["rateLimitsByLimitId"]["codex"]["credits"][field] = value;
            assert!(check_subscription(&bad, "codex", 100).is_err());
        }
        let mut bad = good;
        bad["rateLimitsByLimitId"]["codex"]["primary"]["usedPercent"] = json!(100);
        assert!(check_subscription(&bad, "codex", 100).is_err());
    }

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
        v.as_object_mut().unwrap().remove("usage");
        let (terminal, usage) = receipt(&v.to_string(), "D-1", "a").unwrap();
        assert_eq!(terminal["result"]["id"], "tu");
        assert!(usage.is_none());
    }
}
