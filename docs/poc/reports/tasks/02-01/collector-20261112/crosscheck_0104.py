#!/usr/bin/env python3
"""02-01 real-data cross-check (no model calls).

Replays the 35 real Claude Code streams kept in 01-04 evidence (already masked) through the full
collector (`ieo-evidence run --model-request -- cat stdout.jsonl`) and compares every record with an
independent Python reading of the same raw stream.

- session mode: `resume` when the original 01-04 argv contained --resume/-r, else `new`;
- resume base: the previously replayed run of the same session id, in attempt order (the runtime
  contract §5-5 allows one run per session; 01-04 N4a/N4b broke that on purpose, so their later
  runs may have no valid base and must come out unmeasured, not negative or 0);
- reference: chosen result = last result with num_turns >= 1 else last; session = sum of
  modelUsage; run = session (new) or session - base session (resume, all components >= 0).
- semantic check independent of the subtraction: when a run has no children and no compaction,
  its run share must equal its own result.usage.
Usage: python3 -I crosscheck_0104.py BIN REPO_ROOT OUT_DIR
"""
import json, subprocess, sys
from pathlib import Path

binary, repo, outdir = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3])
base = repo / "docs/poc/reports/tasks/01-04"
K = ("input", "output", "cache_read", "cache_creation")
API = ("input_tokens", "output_tokens", "cache_read_input_tokens", "cache_creation_input_tokens")
MOD = ("inputTokens", "outputTokens", "cacheReadInputTokens", "cacheCreationInputTokens")


def ref(path):
    ev = []
    for line in path.read_text().splitlines():
        try:
            v = json.loads(line)
            if isinstance(v, dict):
                ev.append(v)
        except ValueError:
            pass
    rs = [e for e in ev if e.get("type") == "result"]
    good = [e for e in rs if (e.get("num_turns") or 0) >= 1]
    r = good[-1] if good else (rs[-1] if rs else None)
    if not r:
        return None
    mu = r.get("modelUsage") or {}
    return {"sid": r.get("session_id"), "turns": r.get("num_turns"), "is_error": r.get("is_error"),
            "session": [sum(m[k] for m in mu.values()) for k in MOD] if mu else None,
            "parent": [r["usage"][k] for k in API] if r.get("usage") else None,
            "cost": r.get("total_cost_usd"),
            "agents": sum(1 for e in ev if e.get("type") == "assistant" and not e.get("parent_tool_use_id")
                          for c in (e.get("message") or {}).get("content") or [] if isinstance(c, dict)
                          and c.get("type") == "tool_use" and c.get("name") in ("Agent", "Task")),
            "compact": any(e.get("type") == "system" and e.get("subtype") == "compact_boundary" for e in ev)}


def toks(m):
    return [m["tokens"][k] for k in K] if m["state"] == "measured" else None


streams = sorted(base.glob("*/evidence/attempts/*/stdout.jsonl"), key=lambda p: (p.parts[-5], p.parent.name))
last_by_sid, rows, fails = {}, [], 0
stats = {"new": 0, "resume": 0, "run_measured": 0, "run_unmeasured": 0, "semantic_checked": 0, "semantic_equal": 0}
for i, out in enumerate(streams):
    meta = json.loads((out.parent / "meta.json").read_text())
    argv = " ".join(str(a) for a in meta["argv"])
    mode = "resume" if (" --resume " in f" {argv} " or " -r " in f" {argv} ") else "new"
    stats[mode] += 1
    rf = ref(out)
    base_rec = last_by_sid.get(rf["sid"]) if (rf and mode == "resume") else None
    cmd = [binary, "run", "--out", str(outdir), "--task", "01-04-replay", "--attempt", f"{i:02d}-{out.parent.name}",
           "--command", "replay", "--model-request", "--session", mode, "--env", "PATH"]
    if base_rec:
        cmd += ["--resume-base", str(base_rec[0])]
    cmd += ["--", "cat", str(out)]
    p = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, universal_newlines=True)
    rec_path = outdir / "01-04-replay" / f"{i:02d}-{out.parent.name}" / "replay" / "record.json"
    rec = json.loads(rec_path.read_text())
    u = rec["usage"]
    c = {}
    c["result_present"] = rec["result_event"]["present"] == (rf is not None)
    exp_run = None
    if rf:
        c["session_id"] = rec["result_event"]["session_id"] == rf["sid"]
        c["num_turns"] = rec["result_event"]["num_turns"] == rf["turns"]
        c["session"] = toks(u["session"]) == (rf["session"] if rf["session"] is not None else (rf["parent"] if mode == "new" and rf["agents"] == 0 else None))
        c["parent"] = toks(u["parent"]) == rf["parent"]
        c["cost_exact"] = u["cost_usd_reported"] == rf["cost"]
        if mode == "new":
            # empty modelUsage in a new session: the collector falls back to parent (+ children) as reported
            exp_run = rf["session"] if rf["session"] is not None else (rf["parent"] if rf["agents"] == 0 else None)
        elif base_rec and rf["session"] and base_rec[1]["session"]:
            d = [a - b for a, b in zip(rf["session"], base_rec[1]["session"])]
            exp_run = d if min(d) >= 0 else None
            # withdrawn when the share differs from its own parent usage without children/compaction
            if exp_run and rf["agents"] == 0 and not rf["compact"] and exp_run != rf["parent"]:
                exp_run = None
        c["run"] = toks(u["run"]) == exp_run
        if toks(u["run"]) and rf["agents"] == 0 and not rf["compact"] and rf["turns"]:
            stats["semantic_checked"] += 1
            stats["semantic_equal"] += toks(u["run"]) == rf["parent"]
        last_by_sid[rf["sid"]] = (rec_path, rf)
    else:
        c["run"] = u["run"]["state"] == "unmeasured"
    stats["run_measured" if u["run"]["state"] == "measured" else "run_unmeasured"] += 1
    ok = all(c.values())
    fails += not ok
    rows.append((out.parent.parent.parent.parent.name[:5] + "/" + out.parent.name, mode,
                 base_rec[0].parent.parent.name if base_rec else "-", u["run"]["state"],
                 u["run"].get("reason", "")[:40], u["attribution_check"], rec["judgment"]["status"],
                 ",".join(k for k, v in c.items() if not v) or "-"))
print("stream | mode | base | run | unmeasured reason | attribution | judgment | failed checks")
for r in rows:
    print(" | ".join(str(x) for x in r))
print("\n" + json.dumps(stats, ensure_ascii=False))
print(f"streams {len(rows)} · mismatches {fails}")
sys.exit(1 if fails else 0)
