//! Provider status (runtime contract §7 `provider_status_or_unknown`, 02-04 decision item 4).
//!
//! Raw provider signals in a run's stream-json are mapped to the contract's status values:
//!
//! | value | raw signal (CLI 2.1.322 SDK schema) |
//! | --- | --- |
//! | `auth_or_billing` | an `assistant` event whose top-level `error` is `authentication_failed`, `oauth_org_not_allowed`, `account_on_hold`, `verification_required`, `billing_error` or `cloud_credential_error` |
//! | `usage_limit` | a `rate_limit_event` with `rate_limit_info.status` `rejected`, or `isUsingOverage: true` (the subscription limit is reached; going on would be paid extra usage, which is not allowed) |
//! | `available` | the latest `rate_limit_event` has `status` `allowed` or `allowed_warning`, `isUsingOverage: false`, and no error above came after it |
//! | `unknown` | anything else: no signal, an unreadable or unrecognised one, a transient error (`rate_limit` without a rejected window, `overloaded`, `server_error`), or a signal that is not fresh |
//!
//! Precedence within one run: `auth_or_billing` > `usage_limit` > the latest availability
//! signal. A later `allowed` event does not clear an auth/billing error or a rejected window of
//! the same run; a transient error after an `allowed` event makes the run's status `unknown`
//! (availability was not observed after it).
//!
//! **Time.** stream-json events carry no timestamp. A signal is known only to lie between the
//! run's start and its end (or now, while it runs). Its age is counted from the run's **start**
//! (the conservative end), and a status older than the configured maximum age is reported as
//! `unknown` with the last observation kept beside it. Nothing observed before the service
//! (re)started is current: after a restart the status is `unknown` until a new run observes one.
//! A run longer than the maximum age reports `unknown` while it runs, even when its signal is
//! recent (conservative), and a start time in the future is not fresh.
//! "Not observed" is never `available` and never zero usage.

use serde_json::{Value, json};

/// Default maximum age (seconds) of an observation reported as current.
pub const DEFAULT_MAX_AGE_SECONDS: u64 = 600;

const AUTH_OR_BILLING: [&str; 6] = [
    "authentication_failed",
    "oauth_org_not_allowed",
    "account_on_hold",
    "verification_required",
    "billing_error",
    "cloud_credential_error",
];
const TRANSIENT: [&str; 3] = ["rate_limit", "overloaded", "server_error"];

/// The status one run's stream shows: `value` plus the raw signal it came from.
pub fn classify(stream: &str) -> Value {
    let mut auth: Option<Value> = None;
    let mut limit: Option<Value> = None;
    let mut latest: Option<(&'static str, Value)> = None;
    let mut unreadable = 0u64;
    for (n, line) in stream.lines().enumerate() {
        let line_no = n as u64 + 1;
        let v = match serde_json::from_str::<Value>(line) {
            Ok(v) => v,
            Err(_) => {
                if line.contains("rate_limit_event") || line.contains("\"error\"") {
                    unreadable += 1;
                    latest = Some(("unknown", json!({"line": line_no, "kind": "unreadable"})));
                }
                continue;
            }
        };
        if v["type"] == "rate_limit_event" {
            let i = &v["rate_limit_info"];
            let raw = json!({"line": line_no, "kind": "rate_limit_event", "status": i["status"],
                "rateLimitType": i["rateLimitType"], "resetsAt": i["resetsAt"],
                "overageStatus": i["overageStatus"], "isUsingOverage": i["isUsingOverage"]});
            let st = i["status"].as_str();
            if st == Some("rejected") || i["isUsingOverage"] == true {
                limit.get_or_insert(raw.clone());
                latest = Some(("usage_limit", raw));
            } else if matches!(st, Some("allowed" | "allowed_warning"))
                && i["isUsingOverage"] == false
            {
                latest = Some(("available", raw));
            } else {
                latest = Some(("unknown", raw));
            }
        } else if v["type"] == "assistant"
            && let Some(e) = v["error"].as_str()
        {
            let raw = json!({"line": line_no, "kind": "assistant_error", "error": e});
            if AUTH_OR_BILLING.contains(&e) {
                auth.get_or_insert(raw.clone());
            } else if TRANSIENT.contains(&e) {
                latest = Some(("unknown", raw));
            }
            // other errors (invalid_request, model_not_found, max_output_tokens, unknown) say
            // nothing about the provider's availability: not mapped
        }
    }
    let (value, source) = if let Some(a) = auth {
        ("auth_or_billing", a)
    } else if let Some(l) = limit {
        ("usage_limit", l)
    } else if let Some((v, s)) = latest {
        (v, s)
    } else {
        ("unknown", json!({"kind": "none"}))
    };
    json!({"value": value, "source": source, "unreadable_lines": unreadable})
}

/// One run's observation: [`classify`] plus when it can have been observed.
pub fn observation(stream: &str, attempt: &str, started_at: &str, ended_at: Option<&str>) -> Value {
    let mut v = classify(stream);
    v["attempt_id"] = json!(attempt);
    v["observed_not_before"] = json!(started_at);
    v["observed_not_after"] = ended_at.map_or(Value::Null, |e| json!(e));
    v
}

/// The §7 status from the current observation (this service's lifetime only). `age_seconds` is
/// measured from `observed_not_before`. Returns `(provider_status_or_unknown, detail)`.
pub fn current(
    obs: Option<&Value>,
    now_unix: u64,
    max_age: u64,
    during_run: bool,
) -> (String, Value) {
    let Some(o) = obs else {
        return (
            "unknown".into(),
            json!({"reason": "nothing observed since this service started", "observation": null}),
        );
    };
    // a start time in the future (clock skew) is not a fresh observation (review 48b1d0e I-4)
    let age = o["observed_not_before"]
        .as_str()
        .and_then(unix_of)
        .filter(|t| *t <= now_unix + 60)
        .map(|t| now_unix.saturating_sub(t));
    let value = o["value"].as_str().unwrap_or("unknown");
    let (out, reason) = match age {
        None => ("unknown", "observation time unreadable".to_string()),
        Some(a) if a > max_age => (
            "unknown",
            format!("observation {a} s old (max {max_age} s); last value {value} not current"),
        ),
        Some(_) if value == "unknown" => {
            ("unknown", "the run showed no usable provider signal".into())
        }
        Some(_) => (
            value,
            if during_run {
                "observed in the running attempt".into()
            } else {
                "observed in the last attempt".into()
            },
        ),
    };
    (
        out.into(),
        json!({"reason": reason, "age_seconds": age, "max_age_seconds": max_age, "observation": o}),
    )
}

/// Seconds since the epoch of an RFC 3339 UTC time `YYYY-MM-DDTHH:MM:SS(.fff)Z`.
pub fn unix_of(t: &str) -> Option<u64> {
    let b = t.as_bytes();
    if b.len() < 20 || !t.ends_with('Z') || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' {
        return None;
    }
    let n = |s: &str| s.parse::<u64>().ok();
    let (y, mo, d) = (n(&t[0..4])?, n(&t[5..7])?, n(&t[8..10])?);
    let (h, mi, s) = (n(&t[11..13])?, n(&t[14..16])?, n(&t[17..19])?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || s > 60 {
        return None;
    }
    // days from civil (Howard Hinnant)
    if y == 0 {
        return None;
    }
    let (y, m) = if mo <= 2 {
        (y - 1, mo + 9)
    } else {
        (y, mo - 3)
    };
    let era = y / 400;
    let yoe = y - era * 400;
    let doy = (153 * m + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + h * 3_600 + mi * 60 + s)
}

/// Now, seconds since the epoch.
pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALLOWED: &str = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","rateLimitType":"five_hour","resetsAt":1,"overageStatus":"rejected","isUsingOverage":false}}"#;

    #[test]
    fn contract_values_from_raw_signals() {
        let c = |s: &str| {
            classify(s)["value"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        };
        assert_eq!(c(ALLOWED), "available");
        assert_eq!(
            c(&ALLOWED.replace("\"allowed\"", "\"allowed_warning\"")),
            "available"
        );
        assert_eq!(
            c(&ALLOWED.replace("\"allowed\"", "\"rejected\"")),
            "usage_limit"
        );
        assert_eq!(
            c(&ALLOWED.replace("\"isUsingOverage\":false", "\"isUsingOverage\":true")),
            "usage_limit"
        );
        for e in AUTH_OR_BILLING {
            let l = format!(r#"{{"type":"assistant","error":"{e}","message":{{}}}}"#);
            assert_eq!(
                c(&format!("{ALLOWED}\n{l}\n{ALLOWED}")),
                "auth_or_billing",
                "{e}"
            );
        }
        for e in TRANSIENT {
            let l = format!(r#"{{"type":"assistant","error":"{e}","message":{{}}}}"#);
            assert_eq!(c(&format!("{ALLOWED}\n{l}")), "unknown", "{e}");
        }
        // other errors say nothing about availability
        assert_eq!(
            c(&format!(
                "{ALLOWED}\n{}",
                r#"{"type":"assistant","error":"max_output_tokens"}"#
            )),
            "available"
        );
        // nothing, an unknown status value, a missing overage flag, a truncated event
        assert_eq!(c(""), "unknown");
        assert_eq!(c(r#"{"type":"result","subtype":"success"}"#), "unknown");
        assert_eq!(c(&ALLOWED.replace("\"allowed\"", "\"paused\"")), "unknown");
        assert_eq!(
            c(&ALLOWED.replace(",\"isUsingOverage\":false", "")),
            "unknown"
        );
        assert_eq!(c(&format!("{ALLOWED}\n{}", &ALLOWED[..40])), "unknown");
        // a rejected window is not cleared by a later allowed event of the same run
        assert_eq!(
            c(&format!(
                "{}\n{ALLOWED}",
                ALLOWED.replace("\"allowed\"", "\"rejected\"")
            )),
            "usage_limit"
        );
    }

    #[test]
    fn freshness_and_restart() {
        let t0 = unix_of("2026-11-15T00:00:00Z").expect("t");
        assert_eq!(unix_of("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(unix_of("2026-11-15T00:00:00.123Z"), Some(t0));
        assert_eq!(unix_of("2026-11-15 00:00:00"), None);
        let o = observation(
            ALLOWED,
            "a1",
            "2026-11-15T00:00:00Z",
            Some("2026-11-15T00:01:00Z"),
        );
        assert_eq!(current(Some(&o), t0 + 60, 600, false).0, "available");
        let (v, d) = current(Some(&o), t0 + 601, 600, false);
        assert_eq!(v, "unknown", "{d}");
        assert_eq!(d["observation"]["value"], "available");
        // after a restart nothing is current
        assert_eq!(current(None, t0, 600, false).0, "unknown");
        // an unreadable time is never fresh
        let mut bad = o.clone();
        bad["observed_not_before"] = json!("yesterday");
        assert_eq!(current(Some(&bad), t0, 600, false).0, "unknown");
        // a start in the future is not fresh; year 0 is not a time (review 48b1d0e I-1, I-4)
        let mut future = o.clone();
        future["observed_not_before"] = json!("2026-11-15T01:00:00Z");
        assert_eq!(current(Some(&future), t0, 600, false).0, "unknown");
        assert_eq!(unix_of("0000-01-01T00:00:00Z"), None);
    }
}
