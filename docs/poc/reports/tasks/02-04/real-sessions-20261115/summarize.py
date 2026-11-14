"""Summary of one 02-04 real-session record and its stream, for the public report.

Keeps what the acceptance checks need (init fields, tool uses and denials, result, compaction,
rate-limit status, usage); never copies the raw stream, which stays in the ledger directory.
Usage: summarize.py <records.jsonl> <attempt>
"""
import json, sys

recs = [json.loads(l) for l in open(sys.argv[1]) if l.strip()]
r = next(x for x in recs if x.get("attempt_id") == sys.argv[2])
events = []
path = (r.get("stream") or {}).get("path")
if path:
    for l in open(path):
        try:
            v = json.loads(l)
        except ValueError:
            continue
        if isinstance(v, dict):
            events.append(v)
init = next((e for e in events if e.get("type") == "system" and e.get("subtype") == "init"), {})
tool_uses, denials = [], []
for e in events:
    for c in ((e.get("message") or {}).get("content") or []) if isinstance((e.get("message") or {}).get("content"), list) else []:
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
    "attempt_id": r["attempt_id"], "role": r["role"], "command_id": r["command_id"],
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
}
print(json.dumps(out, ensure_ascii=False, indent=1))
