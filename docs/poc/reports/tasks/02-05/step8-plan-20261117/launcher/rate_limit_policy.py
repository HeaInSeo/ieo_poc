"""02-05: the no-extra-billing rule for one rate_limit_event, for the launcher scripts.

The same rule as the supervisor's `budget::rate_limit_verdict` (Rust; user decision 2026-11-17,
warning, docs/poc/User_Decisions_2026-11-17_warning.md). The shared case table
crates/ieo-supervisor/tests/fixtures/rate_limit_cases.json is checked against both
(crates/ieo-supervisor/tests/rate_limit_policy.rs), so the two cannot drift apart.

verdict(event) -> ("allowed", None) | ("warning", text) | ("stop", reason)
Loaded by absolute path (python3 -I does not put the script's directory on sys.path):
    spec = importlib.util.spec_from_file_location("rate_limit_policy", <dir>/rate_limit_policy.py)
"""
import json

LIMIT_STATUS = ("allowed", "allowed_warning", "rejected")
RATE_LIMIT_TYPES = ("five_hour", "seven_day", "seven_day_opus", "seven_day_sonnet",
                    "seven_day_overage_included", "overage")


def _j(v):
    # the same rendering as serde_json's Value Display for the values that occur here
    return "null" if v is None else json.dumps(v, ensure_ascii=False, separators=(",", ":"))


def verdict(event):
    info = event.get("rate_limit_info") if isinstance(event, dict) else None
    if not isinstance(info, dict):
        return ("stop", "rate_limit_info missing")
    status = info.get("status")
    if not isinstance(status, str) or status not in LIMIT_STATUS:
        return ("stop", f"rate limit status {_j(status)} (missing or unknown)")
    if status == "rejected":
        return ("stop", 'rate limit status "rejected" (subscription usage refused)')
    if info.get("overageStatus") != "rejected":
        return ("stop", f'overageStatus {_j(info.get("overageStatus"))} (only "rejected" excludes paid extra usage)')
    if info.get("isUsingOverage") is not False:
        return ("stop", f'isUsingOverage {_j(info.get("isUsingOverage"))} (extra usage in use or unknown)')
    for k in ("overageInUse", "overageEnabled"):
        if info.get(k) is not None and info.get(k) is not False:
            return ("stop", f"{k} {_j(info.get(k))} (paid extra usage possible or unknown)")
    if info.get("errorCode") is not None:
        return ("stop", f"errorCode {_j(info.get('errorCode'))}")
    kind = info.get("rateLimitType")
    if kind is not None:
        if kind in ("overage", "seven_day_overage_included"):
            return ("stop", f'rateLimitType {_j(kind)} with overageStatus "rejected" (contradictory)')
        if not isinstance(kind, str) or kind not in RATE_LIMIT_TYPES:
            return ("stop", f"rateLimitType {_j(kind)} (unknown)")
    warned = []
    if status == "allowed_warning":
        warned.append(f'{kind if isinstance(kind, str) else "limit"} {_j(info.get("utilization"))} of the limit '
                      f'(threshold {_j(info.get("surpassedThreshold"))}, resets {_j(info.get("resetsAt"))})')
    windows = info.get("unifiedWindows")
    if windows is not None:
        if not isinstance(windows, dict):
            return ("stop", "unifiedWindows is not an object")
        for name in sorted(windows):
            w = windows[name]
            if not isinstance(w, dict):
                return ("stop", f"unifiedWindows.{name} is not an object")
            st = w.get("status")
            if st == "allowed":
                continue
            if st == "allowed_warning":
                warned.append(f'window {name} {_j(w.get("utilization"))} (threshold {_j(w.get("surpassedThreshold"))}, '
                              f'resets {_j(w.get("resetsAt"))})')
                continue
            if st == "rejected":
                return ("stop", f'unifiedWindows.{name} status "rejected"')
            return ("stop", f"unifiedWindows.{name} status {_j(st)} (missing or unknown)")
    if not warned:
        return ("allowed", None)
    return ("warning", "rate limit warning (allowed_warning; paid extra usage off): " + "; ".join(warned))


def _no_constant(name):
    # NaN, Infinity and -Infinity are not JSON (serde_json refuses them; Python accepts them)
    raise ValueError(f"not JSON: {name}")


def split_lines(text):
    """The lines of a stream as the supervisor reads them (Rust str::lines): split on "\n" only
    (never on U+2028, U+0085 and the like), one trailing "\r" removed, no empty last line."""
    parts = text.split("\n")
    if parts and parts[-1] == "":
        parts.pop()
    return [p[:-1] if p.endswith("\r") else p for p in parts]


def stream_verdicts(lines):
    """[(line number, verdict)] for every rate_limit_event of a stream; a line that mentions
    rate_limit_event but is not JSON is a stop (it may have carried a breaking value). `lines`
    is a list (use split_lines) or the stream text itself."""
    if isinstance(lines, str):
        lines = split_lines(lines)
    out = []
    for n, line in enumerate(lines, 1):
        try:
            e = json.loads(line, parse_constant=_no_constant)
        except ValueError:
            if "rate_limit_event" in line:
                out.append((n, ("stop", f"line {n}: unreadable rate_limit_event")))
            continue
        if isinstance(e, dict) and e.get("type") == "rate_limit_event":
            out.append((n, verdict(e)))
    return out
