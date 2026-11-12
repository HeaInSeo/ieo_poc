#!/usr/bin/env python3
"""Audit 01-04 model attempts: did every attempt used for a judgement carry a result event? (no model calls)"""
import glob, json, os
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2] / "docs/poc/reports/tasks/01-04"
rows = []
for run in ("first-probes-20261112", "remaining-probes-20261112"):
    for d in sorted(glob.glob(str(ROOT / run / "evidence/attempts/*"))):
        m = json.load(open(d + "/meta.json"))
        ev = [json.loads(l) for l in open(d + "/stdout.jsonl") if l.strip().startswith("{")]
        res = [e for e in ev if e.get("type") == "result"]
        init = [e for e in ev if e.get("type") == "system" and e.get("subtype") == "init"]
        rows.append({"run": run, "attempt": os.path.basename(d), "exit_code": m["exit_code"], "result_events": len(res), "init_events": len(init),
                     "results": [[r.get("subtype"), r.get("is_error")] for r in res]})
for run in ("first-probes-20261112", "remaining-probes-20261112"):
    b = json.load(open(ROOT / run / "evidence/budget.json"))
    for a in b["attempts"]:
        if a.get("launched") is False:
            rows.append({"run": run, "attempt": f"{a['n']:02d}-{a.get('probe')}-{a.get('role')}", "exit_code": None, "result_events": None, "init_events": None, "results": "NOT LAUNCHED (reserved only)"})
print(json.dumps(rows, ensure_ascii=False, indent=1))
launched = [r for r in rows if r["result_events"] is not None]
print("launched", len(launched), "| missing result event", [r["attempt"] for r in launched if r["result_events"] == 0],
      "| exit0 without result", [r["attempt"] for r in launched if r["exit_code"] == 0 and r["result_events"] == 0],
      "| non-zero exit", [(r["attempt"], r["exit_code"], r["results"]) for r in launched if r["exit_code"] != 0])
