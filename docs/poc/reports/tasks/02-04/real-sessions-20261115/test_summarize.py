"""Regression tests for summarize.py (no files, no streams): a record written before kept child
transcripts must never read as "no child" (02-04 D3, review 29690b5 L2).
Run: python test_summarize.py (CI: from docs/poc)."""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from summarize import summarize  # noqa: E402

CORR = "ieo-role-run-correction/v0"


def rec(attempt, **kw):
    r = {"format": "ieo-role-run/v0", "attempt_id": attempt, "role": "development",
         "stream": {"path": "/nonexistent", "sha256": "x", "bytes": 1}}
    r.update(kw)
    return json.dumps(r)


CHILD = {"tool_use_id": "tu1", "agent_id": "a1", "reported_last_response": {"input": 2},
         "usage": {"state": "unmeasured", "reason": "자식 tool_use_result.usage"}}
fails = 0


def check(name, cond, detail=""):
    global fails
    print(("PASS " if cond else "FAIL ") + name + ("" if cond else f": {detail}"))
    fails += not cond


# the old D3 shape: no top-level `children`, a child in usage.children, a correction with the transcript
old_d3 = rec("D3", usage={"run": {"state": "unmeasured"}, "children": [CHILD]})
corr = json.dumps({"format": CORR, "corrects": "D3", "decision": "d",
                   "child_transcripts": [{"agent_id": "a1", "bytes": 10, "sha256": "s", "path": "/p"}],
                   "after": {"usage": {"run": {"state": "measured"}, "attribution_check": "consistent"}, "stop_required": False}})
s = summarize([rec("D1", usage={"children": []}), old_d3, corr], "D3", read_stream=False)
c = s["children"]
check("old D3 record: a child, not none", c["status"] == "present", c)
check("old D3 record: the child from the record's stream reading", c["observed_in_stream"]["children"][0]["agent_id"] == "a1", c)
check("old D3 record: kept transcript from the correction, with its source",
      c["kept_transcripts"]["source"].startswith("correction") and c["kept_transcripts"]["transcripts"][0]["sha256"] == "s", c)
check("provenance: record and correction lines", s["provenance"]["record_line"] == 2 and s["provenance"]["correction_line"] == 3, s["provenance"])
check("correction shown next to the original", s["correction"]["usage_run"] == {"state": "measured"} and s["usage_run"] == {"state": "unmeasured"}, s)

# no child in the stream reading: confirmed none
s = summarize([rec("D1", usage={"children": []})], "D1", read_stream=False)
check("empty usage.children: confirmed none", s["children"]["status"] == "confirmed_none", s["children"])
check("confirmed none: no kept transcripts recorded, said so",
      s["children"]["kept_transcripts"]["source"] == "not recorded in this record format", s["children"])

# a record that cannot say (no usage.children at all, e.g. a failure record)
s = summarize([rec("X", usage={"run": {"state": "unmeasured"}})], "X", read_stream=False)
check("no usage.children: unknown, never none", s["children"]["status"] == "unknown", s["children"])

# a new-format record keeps its own transcripts
s = summarize([rec("D7", usage={"children": [CHILD]}, children=[{"agent_id": "a1", "bytes": 5, "sha256": "k", "path": "/q"}])],
              "D7", read_stream=False)
check("new record: kept transcripts from the record",
      s["children"]["kept_transcripts"]["source"].startswith("record children") and s["children"]["kept_transcripts"]["transcripts"][0]["sha256"] == "k",
      s["children"])
# the real D3 record and its correction, from the public ledger copy (review 56882cb L5)
pub = os.path.join(os.path.dirname(os.path.abspath(__file__)), "evidence", "real", "records.jsonl")
real = open(pub).read().splitlines()
s = summarize(real, "D3", read_stream=False)
c = s["children"]
check("public D3 record: a child, not none", c["status"] == "present", c)
check("public D3 record: agent a13188dc0170f8901 from the record's stream reading",
      [x["agent_id"] for x in c["observed_in_stream"]["children"]] == ["a13188dc0170f8901"], c)
check("public D3 record: kept transcript from the correction (8e509473…)",
      c["kept_transcripts"]["source"].startswith("correction") and c["kept_transcripts"]["transcripts"][0]["sha256"].startswith("8e509473"), c)
check("public D3 record: original unmeasured, correction measured",
      s["usage_run"]["state"] == "unmeasured" and s["correction"]["usage_run"]["state"] == "measured", s)
s = summarize(real, "D4", read_stream=False)
check("public D4 record: confirmed none", s["children"]["status"] == "confirmed_none", s["children"])
print("RESULT", "PASS" if not fails else "FAIL", fails)
sys.exit(1 if fails else 0)
