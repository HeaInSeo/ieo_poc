"""Validate the central review record for a single Codex conversion trial."""

import hashlib
import json
from pathlib import Path
import re

SCHEMA = "ieo.codex-trial-prerequisites/v1"
FRESH_SECONDS = 300
RUN_SECONDS = 120
SCOPE = ("assignment_id", "generation", "target_sha", "checkpoint_sha256",
         "model", "limit_id", "auth_sha256", "workspace_id", "claude_account_id",
         "claude_quota_group_id", "claude_window_id")


class InvalidPrerequisites(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise InvalidPrerequisites(message)


def digest(value):
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def pinned_json(path, pin, name):
    require(path and digest(pin), f"missing pinned {name}")
    try:
        raw = Path(path).read_bytes()
    except OSError:
        raise InvalidPrerequisites(f"missing or unreadable {name}") from None
    require(hashlib.sha256(raw).hexdigest() == pin, f"{name} changed")
    value = json.loads(raw)
    require(isinstance(value, dict), f"invalid {name}")
    return value


def fresh_record(record, state, source, now):
    require(isinstance(record, dict) and record.get("state") == state
            and record.get("source") == source and digest(record.get("evidence_sha256")),
            f"missing verified {source} evidence")
    observed = record.get("observed_unix")
    require(type(observed) is int and 0 < observed <= now < observed + FRESH_SECONDS,
            f"stale or future {source} evidence")
    return observed + FRESH_SECONDS


def verify(approval, evidence, now):
    """Return the last exclusive start time and the bound workspace UUID."""
    require(evidence.get("schema") == SCHEMA, "missing supported prerequisite review")
    scope = evidence.get("scope")
    require(isinstance(scope, dict) and set(scope) == set(SCOPE)
            and all(key in approval and scope[key] == approval[key] for key in SCOPE),
            "prerequisite scope does not match approval")
    require(type(scope["generation"]) is int and type(approval["generation"]) is int
            and scope["generation"] >= 0
            and all(isinstance(scope[key], str) and scope[key].strip()
                    for key in SCOPE if key != "generation")
            and digest(scope["auth_sha256"]) and digest(scope["checkpoint_sha256"])
            and re.fullmatch(r"[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}", scope["workspace_id"]),
            "missing or invalid approved execution identity")
    billing = approval.get("billing")
    require(type(approval.get("additional_usd")) is int and approval["additional_usd"] == 0
            and type(approval.get("max_dispatches")) is int and approval["max_dispatches"] == 1
            and isinstance(billing, dict)
            and all(billing.get(key) is True for key in
                    ("auto_reload_disabled", "paid_paths_blocked", "api_fallback_blocked")),
            "missing USD 0 approval")
    expiry, verified = approval.get("expires_unix"), billing.get("verified_unix")
    require(type(expiry) is int and type(verified) is int and 0 < verified <= now,
            "missing current billing approval")
    ends = [expiry, verified + FRESH_SECONDS]
    for key, state, source in (
        ("workspace_credit_spend", "verified_blocked", "workspace_admin_policy"),
        ("concurrent_credit_changes", "verified_excluded", "account_wide_enforced_freeze"),
    ):
        record = evidence.get(key)
        ends.append(fresh_record(record, state, source, now))
        end = record.get("valid_until_unix")
        require(type(end) is int, f"missing enforcement expiry for {key}")
        ends.append(end - RUN_SECONDS)
    claude = evidence.get("claude_quota")
    ends.append(fresh_record(claude, "naturally_exhausted", "operator_usage_screen", now))
    reset = claude.get("reset_unix")
    require(claude.get("limit_kind") == "included_subscription"
            and claude.get("extra_usage") == "disabled" and type(reset) is int,
            "missing included-limit reset or Claude extra-usage exclusion")
    ends.append(reset - RUN_SECONDS)
    end = min(ends)
    require(now < end, "prerequisites expired or execution window crosses reset/control expiry")
    return end, scope["workspace_id"]
