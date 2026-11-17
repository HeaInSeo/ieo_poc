"""02-05 step 8: the checks after each run (EXECUTION_PLAN.md §6.2, §6.3), and the G1 prompt.

check-run.py <ledger dir> <out dir> <attempt> <role> <assignment>   exit 0 = every check passed
check-run.py --g1-prompt <ledger dir> <G1 head file> <out file>     the numbered claims, ≤ 2 KB

Reads only the 02-05 ledger (records and the run streams the caller kept), the answers run-step8.sh
saved (role status, coordinator role status, chain) and prints no credential.
"""
import json
import math
import os
import sys

CAPS = {"runs": 4, "seconds": 2400, "billable": 200_000, "cache_read": 2_000_000}  # BUDGET_0205
SHARES = {"development": {"runs": 3, "seconds": 1800, "billable": 150_000, "cache_read": 1_500_000},
          "guardrail": {"runs": 1, "seconds": 600, "billable": 50_000, "cache_read": 500_000}}
LIMITS = {"S8-DEV-A1": {"seconds": 1800, "billable": 150_000}, "S8-GR-A1": {"seconds": 600, "billable": 50_000}}
OFFERED = {"development": {"Read", "Grep", "Glob"}, "guardrail": {"Read", "Grep", "Glob", "Write", "Edit"}}
USED = {"development": {"Read", "Grep", "Glob"}, "guardrail": {"Read", "Grep", "Glob"}}
G1_CAP = 2048


def records(ledger):
    with open(os.path.join(ledger, "records.jsonl")) as f:
        return [json.loads(l) for l in f if l.strip()]


def events(path):
    with open(path) as f:
        for line in f:
            try:
                yield json.loads(line)
            except ValueError:
                continue


def result_text(rec):
    text = None
    for e in events(rec["stream"]["path"]):
        if e.get("type") == "result":
            text = e.get("result")
    return text


def g1_prompt(ledger, head, out):
    by = {r["attempt_id"]: r for r in records(ledger)}
    lines = []
    for a in ("s8-r1", "s8-r2"):
        t = result_text(by[a]) if a in by else None
        lines += [l.strip() for l in (t or "").splitlines() if l.strip()]
    claims = "\n".join(f"{i}. {l}" for i, l in enumerate(lines, 1))
    if not claims:
        print("no developer answer to review")
        return 1
    if len(claims.encode()) > G1_CAP:
        print(f"the developer answers are {len(claims.encode())} bytes, over {G1_CAP}")
        return 1
    with open(head) as f:
        body = f.read().strip() + "\n\n" + claims + "\n"
    with open(out, "w") as f:
        f.write(body)
    print(f"G1 prompt: {len(lines)} numbered claims, {len(claims.encode())} bytes")
    return 0


def tokens(r):
    t = r["usage"]["run"].get("tokens") or {}
    return t.get("input", 0) + t.get("output", 0) + t.get("cache_creation", 0), t.get("cache_read", 0)


def check(ledger, out, att, role, aid):
    bad = []
    recs = records(ledger)
    by = {r["attempt_id"]: r for r in recs}
    r = by.get(att)
    if r is None:
        print(f"{att}: no record")
        return 1
    run = r["usage"]["run"]
    if r.get("budget") != "02-05" or r.get("model_request") is not True:
        bad.append("not a 02-05 model run")
    if r.get("stop_required"):
        bad.append(f"stop_required: {r.get('stop_reason')}")
    if run.get("state") != "measured":
        bad.append(f"usage {run.get('state')} (never counted as 0)")
    res = (r.get("outcome") or {}).get("result") or {}
    if res.get("subtype") != "success" or res.get("is_error") is not False:
        bad.append(f"result {res}")
    env = (r.get("outcome") or {}).get("env_names") or []
    if "CLAUDE_CODE_OAUTH_TOKEN" not in env or any(n.startswith("ANTHROPIC_") for n in env):
        bad.append(f"environment not subscription-only: {env}")
    signals, offered, used = [], None, []
    for e in events(r["stream"]["path"]):
        if e.get("type") == "rate_limit_event":
            signals.append(e.get("rate_limit_info") or {})
        if e.get("type") == "system" and e.get("subtype") == "init":
            offered = set(e.get("tools") or [])
        if e.get("type") == "assistant":
            for c in (e.get("message") or {}).get("content") or []:
                if isinstance(c, dict) and c.get("type") == "tool_use":
                    used.append(c.get("name"))
    if not signals or any(s.get("status") != "allowed" or s.get("overageStatus") != "rejected"
                          or s.get("isUsingOverage") is not False for s in signals):
        bad.append(f"rate-limit signals not all allowed/rejected/false: {signals}")
    if offered is None:
        bad.append("no init event")
    elif not offered <= OFFERED[role]:
        bad.append(f"tools offered beyond {sorted(OFFERED[role])}: {sorted(offered - OFFERED[role])}")
    if set(used) - USED[role]:
        bad.append(f"tool uses beyond {sorted(USED[role])}: {used}")
    if att == "s8-r2":
        s1 = (by.get("s8-r1") or {}).get("session", {}).get("session_id")
        if r.get("session", {}).get("mode") != "resume" or r.get("session", {}).get("session_id") != s1:
            bad.append(f"R2 did not resume R1's session: {r.get('session')}")
    total = {"runs": 0, "seconds": 0, "billable": 0, "cache_read": 0}
    per = {k: dict.fromkeys(total, 0) for k in SHARES}
    for x in recs:
        if x.get("model_request") is not True or x.get("budget") != "02-05":
            continue
        if x["usage"]["run"].get("state") != "measured":
            bad.append(f"{x['attempt_id']}: unmeasured")
            continue
        b, c = tokens(x)
        s = math.ceil((x.get("duration_ms") or 0) / 1000)
        for d in (total, per[x["role"]]):
            d["runs"] += 1; d["seconds"] += s; d["billable"] += b; d["cache_read"] += c
    bad += [f"budget {k} {total[k]} > {v}" for k, v in CAPS.items() if total[k] > v]
    for rl, cap in SHARES.items():
        bad += [f"{rl} share {k} {per[rl][k]} > {v}" for k, v in cap.items() if per[rl][k] > v]
    lim = LIMITS[aid]
    if per[role]["seconds"] > lim["seconds"] or per[role]["billable"] > lim["billable"]:
        bad.append(f"{aid} limits exceeded: {per[role]}")
    st = json.load(open(os.path.join(out, f"status-{att}.json")))
    if st.get("safe_shutdown", {}).get("pending_external_requests") != []:
        bad.append(f"pending external requests: {st.get('safe_shutdown', {}).get('pending_external_requests')}")
    if st.get("central_checkpoint", {}).get("mode") == "local_only":
        bad.append("supervisor in local_only mode")
    rs = json.load(open(os.path.join(out, f"roles-{att}.json")))
    if rs.get("quota_groups", {}).get("mismatches"):
        bad.append(f"quota group mismatch: {rs['quota_groups']['mismatches']}")
    ch = json.load(open(os.path.join(out, f"chain-{aid}-{att}.json")))
    link = [l for l in ch.get("links", []) if l.get("attempt_id") == att]
    if not link or not link[0].get("taken_over") or not link[0].get("first_action"):
        bad.append(f"chain: the delivery of {att} is not taken over with a first action: {link}")
    if os.path.exists(os.path.join(ledger, "inflight.json")):
        bad.append("in-flight marker left")
    b, c = tokens(r)
    print(f"{att}: measured, {math.ceil((r.get('duration_ms') or 0) / 1000)} s, billable {b}, cache_read {c}, "
          f"tools offered {sorted(offered or [])}, used {used}; totals {total}")
    for m in bad:
        print(f"CHECK FAILED {att}: {m}")
    return 1 if bad else 0


if __name__ == "__main__":
    if sys.argv[1] == "--g1-prompt":
        sys.exit(g1_prompt(*sys.argv[2:5]))
    sys.exit(check(*sys.argv[1:6]))
