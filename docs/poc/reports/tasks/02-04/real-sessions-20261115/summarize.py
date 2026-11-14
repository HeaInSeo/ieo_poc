"""Summary of one 02-04 real-session record and its stream, for the public report (v2).

Keeps what the acceptance checks need (init fields, tool uses and denials, result, compaction,
rate-limit status, usage, children); never copies the raw stream, which stays in the ledger
directory.

v2 (2026-11-15): children are reported with their source and a status that separates
"confirmed none" from "the record cannot say" (a record written before kept child transcripts
has no top-level `children` field; that is not "no child"). Every summary names the record and
correction lines it was built from (line number and SHA-256) and this generator (version and
SHA-256).

Usage: summarize.py <records.jsonl> <attempt>
"""
import hashlib
import json
import os
import sys

VERSION = "2"
CORRECTION = "ieo-role-run-correction/v0"


def _sha(b):
    return hashlib.sha256(b).hexdigest()


def children_view(r, correction):
    """Children of a run: what its stream showed (the collector's reading in the record) and
    which transcripts were kept, each with its source. Status: present | confirmed_none |
    unknown (never "none" for a record that cannot say)."""
    usage = r.get("usage") if isinstance(r.get("usage"), dict) else None
    observed = usage.get("children") if usage and isinstance(usage.get("children"), list) else None
    if observed is None:
        status = "unknown"
        observed_view = {"source": "not in this record (no usage.children)", "children": None}
    else:
        status = "present" if observed else "confirmed_none"
        observed_view = {
            "source": "record usage.children (the collector's reading of the run's stream: Agent tool_use and its result)",
            "children": [{"tool_use_id": c.get("tool_use_id"), "agent_id": c.get("agent_id"),
                          "usage_state": (c.get("usage") or {}).get("state"),
                          "usage_source": (c.get("usage") or {}).get("source"),
                          "reported_last_response": c.get("reported_last_response")}
                         for c in observed],
        }
    if isinstance(r.get("children"), list):
        kept = {"source": "record children (kept by the role service and fetched by the caller)",
                "transcripts": [{k: c.get(k) for k in ("agent_id", "bytes", "sha256")} for c in r["children"]]}
    elif correction and isinstance(correction.get("child_transcripts"), list):
        kept = {"source": "correction child_transcripts (the record predates kept transcripts)",
                "transcripts": [{k: c.get(k) for k in ("agent_id", "bytes", "sha256")}
                                for c in correction["child_transcripts"]]}
    else:
        kept = {"source": "not recorded in this record format", "transcripts": None}
    return {"status": status, "observed_in_stream": observed_view, "kept_transcripts": kept}


def summarize(raw_lines, attempt, read_stream=True, generator_sha=None):
    """`raw_lines`: the ledger lines as text. Returns the summary dict."""
    lines = [(i + 1, l, json.loads(l)) for i, l in enumerate(raw_lines) if l.strip()]
    rec = next(((n, l, v) for n, l, v in lines if v.get("format") != CORRECTION and v.get("attempt_id") == attempt), None)
    if rec is None:
        raise SystemExit(f"no record of {attempt}")
    rn, rl, r = rec
    cor = next(((n, l, v) for n, l, v in lines if v.get("format") == CORRECTION and v.get("corrects") == attempt), None)
    correction = cor[2] if cor else None
    events, stream_read_sha = [], None
    path = (r.get("stream") or {}).get("path")
    if read_stream and path and os.path.exists(path):
        b = open(path, "rb").read()
        stream_read_sha = _sha(b)
        for l in b.decode("utf-8", "replace").splitlines():
            try:
                v = json.loads(l)
            except ValueError:
                continue
            if isinstance(v, dict):
                events.append(v)
    init = next((e for e in events if e.get("type") == "system" and e.get("subtype") == "init"), {})
    tool_uses, denials = [], []
    for e in events:
        msg = e.get("message")
        content = msg.get("content") if isinstance(msg, dict) else None
        for c in content if isinstance(content, list) else []:
            if not isinstance(c, dict):
                continue
            if c.get("type") == "tool_use":
                inp = c.get("input") or {}
                tool_uses.append({"name": c.get("name"), "file_path": inp.get("file_path"),
                                  "subagent_type": inp.get("subagent_type"),
                                  "parent": e.get("parent_tool_use_id")})
            if c.get("type") == "tool_result" and c.get("is_error"):
                t = c.get("content")
                t = t if isinstance(t, str) else json.dumps(t)
                denials.append(t[:200])
    result = next((e for e in reversed(events) if e.get("type") == "result"), {})
    out = {
        "attempt_id": r["attempt_id"], "role": r.get("role"), "command_id": r.get("command_id"),
        "session": r.get("session"), "duration_ms": r.get("duration_ms"),
        "stop_required": r.get("stop_required"), "stop_reason": r.get("stop_reason"),
        "outcome": {k: (r.get("outcome") or {}).get(k) for k in
                    ["stop", "stopped_by_guard", "exit_code", "cleanup_failed", "tamper", "config_cleanup", "env_names"]},
        "init": {k: init.get(k) for k in ["session_id", "tools", "mcp_servers", "agents", "skills", "plugins",
                                           "memory_paths", "apiKeySource", "model", "permissionMode", "cwd"]},
        "init_slash_commands": len(init.get("slash_commands") or []),
        "tool_uses": tool_uses, "tool_errors": denials,
        "result": {"subtype": result.get("subtype"), "is_error": result.get("is_error"),
                   "session_id": result.get("session_id"), "text": (result.get("result") or "")[:400]},
        "compact_boundaries": sum(1 for e in events if e.get("type") == "system" and e.get("subtype") == "compact_boundary"),
        "rate_limit": [{k: (e.get("rate_limit_info") or {}).get(k) for k in ["status", "overageStatus", "isUsingOverage"]}
                       for e in events if e.get("type") == "rate_limit_event"],
        "usage_run": (r.get("usage") or {}).get("run"),
        "usage_session": (r.get("usage") or {}).get("session"),
        "stream": {k: (r.get("stream") or {}).get(k) for k in ["bytes", "sha256"]},
        "children": children_view(r, correction),
        "provenance": {
            "generator": f"summarize.py v{VERSION}", "generator_sha256": generator_sha,
            "record_line": rn, "record_line_sha256": _sha(rl.encode()),
            "correction_line": cor[0] if cor else None,
            "correction_line_sha256": _sha(cor[1].encode()) if cor else None,
            "stream_read": stream_read_sha is not None,
            "stream_matches_record": (stream_read_sha == (r.get("stream") or {}).get("sha256")) if stream_read_sha else None,
        },
    }
    if correction:
        # the original record above is kept as it was; the correction is shown next to it
        after = correction.get("after") or {}
        out["correction"] = {"decision": correction.get("decision"), "corrected_at": correction.get("corrected_at"),
                             "usage_run": (after.get("usage") or {}).get("run"),
                             "attribution_check": (after.get("usage") or {}).get("attribution_check"),
                             "stop_required": after.get("stop_required")}
    return out


if __name__ == "__main__":
    me = _sha(open(os.path.abspath(__file__), "rb").read())
    print(json.dumps(summarize(open(sys.argv[1]).read().splitlines(), sys.argv[2], generator_sha=me),
                     ensure_ascii=False, indent=1))
