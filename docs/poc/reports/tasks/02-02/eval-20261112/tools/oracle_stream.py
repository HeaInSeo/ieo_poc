"""Independent reader of Claude Code stream-json (expectation owner's own computation).
Rules: REPORT 02-01 §1 row 사용량(stream.rs) / 귀속 대조; Runtime_Contract_v0 §11 item 7."""
import json, sys

def tok_from_usage(u):
    keys = ["input_tokens", "output_tokens", "cache_read_input_tokens", "cache_creation_input_tokens"]
    if not isinstance(u, dict) or any(k not in u for k in keys):
        return None
    return dict(input=u[keys[0]], output=u[keys[1]], cache_read=u[keys[2]], cache_creation=u[keys[3]])

def tok_from_mu(mu):
    if not isinstance(mu, dict) or not mu:
        return None
    keys = ["inputTokens", "outputTokens", "cacheReadInputTokens", "cacheCreationInputTokens"]
    t = dict(input=0, output=0, cache_read=0, cache_creation=0)
    for m in mu.values():
        if any(k not in m for k in keys):
            return None
        t["input"] += m[keys[0]]; t["output"] += m[keys[1]]
        t["cache_read"] += m[keys[2]]; t["cache_creation"] += m[keys[3]]
    return t

def read(path):
    results, nonjson, rl, compact, children, agent_uses = [], 0, [], 0, {}, []
    for line in open(path, encoding="utf-8", errors="replace").read().splitlines():
        if not line.strip():
            continue
        try:
            o = json.loads(line)
        except Exception:
            nonjson += 1; continue
        if not isinstance(o, dict):
            continue
        t = o.get("type")
        if t == "result":
            results.append(o)
        elif t == "rate_limit_event":
            rl.append((o.get("rate_limit_info") or {}).get("status"))
        elif t == "system" and o.get("subtype") == "compact_boundary":
            compact += 1
        elif t == "assistant" and o.get("parent_tool_use_id") is None:
            for c in (o.get("message") or {}).get("content") or []:
                if isinstance(c, dict) and c.get("type") == "tool_use" and c.get("name") in ("Agent", "Task"):
                    agent_uses.append(c.get("id"))
        elif t == "user" and isinstance(o.get("tool_use_result"), dict):
            tur = o["tool_use_result"]
            for c in (o.get("message") or {}).get("content") or []:
                if isinstance(c, dict) and c.get("type") == "tool_result":
                    if "usage" in tur or "agentId" in tur:
                        children[c.get("tool_use_id")] = dict(agent_id=tur.get("agentId"), tokens=tok_from_usage(tur.get("usage")))
    sel = None
    for r in results:
        if isinstance(r.get("num_turns"), int) and r["num_turns"] >= 1:
            sel = r
    if sel is None and results:
        sel = results[-1]
    out = dict(count=len(results), present=sel is not None, nonjson=nonjson, rate_limit=rl, compact=compact,
               agent_uses=agent_uses, children=children)
    if sel is not None:
        out.update(subtype=sel.get("subtype"), is_error=sel.get("is_error"), session_id=sel.get("session_id"),
                   num_turns=sel.get("num_turns"), stop_reason=sel.get("stop_reason"),
                   parent=tok_from_usage(sel.get("usage")), session=tok_from_mu(sel.get("modelUsage")),
                   cost=sel.get("total_cost_usd"))
    return out

def sub(a, b):
    return {k: a[k] - b[k] for k in a}

def add(a, b):
    return {k: a[k] + b[k] for k in a}

if __name__ == "__main__":
    for p in sys.argv[1:]:
        r = read(p)
        print(p.split("/")[-2], json.dumps({k: v for k, v in r.items()}, ensure_ascii=False))
