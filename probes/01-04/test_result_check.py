#!/usr/bin/env python3
"""No-model regression check for the result-event / decisive-marker rules (harness check 2026-11-12).

Run: .venv-poc/bin/python -I probes/01-04/test_result_check.py   (no podman, no claude, no tokens printed)
"""
import json, sys, tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import probe as P   # noqa: E402
import probe2 as P2  # noqa: E402

ROOT = Path(__file__).resolve().parents[2] / "docs/poc/reports/tasks/01-04"
fails = []


def check(name, cond):
    print(("PASS " if cond else "FAIL ") + name)
    if not cond:
        fails.append(name)


def exits(fn):
    try:
        fn()
    except SystemExit:
        return True
    return False


with tempfile.TemporaryDirectory() as td:
    led = P2.Ledger2(Path(td))
    led.reserve(3, "synthetic")
    argv = ["true"]
    # 1. exit 0, empty stream -> no result event -> not judgeable, stop
    m = P2.record(led, 1, "SYN", "development", argv, "t0", "t1", 0, "", "", {"expected_outcome": "result"}, check_stop=False)
    check("exit0_no_result_marked_unjudgeable", m["result_missing"] and not m["judgeable"])
    check("exit0_no_result_stops", exits(lambda: P2.stop_signals(m)))
    # 2. intentional abort test: no result expected -> judged by its own expectation, no stop
    m2 = P2.record(led, 2, "SYN", "development", argv, "t0", "t1", 137, "", "", {"expected_outcome": "no_result"}, check_stop=False)
    check("intended_no_result_is_judgeable", m2["judgeable"])
    check("intended_no_result_does_not_stop", not exits(lambda: P2.stop_signals(m2)))
    # 3. normal attempt with a result event passes
    ok = '{"type":"system","subtype":"init","session_id":"s"}\n{"type":"result","subtype":"success","is_error":false,"session_id":"s","result":"ok"}\n'
    m3 = P2.record(led, 3, "SYN", "development", argv, "t0", "t1", 0, ok, "", {"expected_outcome": "result"}, check_stop=False)
    check("result_present_judgeable", m3["judgeable"] and not m3["result_missing"])
    check("result_present_no_stop", not exits(lambda: P2.stop_signals(m3)))
    # 4. probe.py round check: missing result event stops even with exit 0
    check("probe_py_round_without_result_stops", exits(lambda: P.stop_on_signal({"development": {"result_event": None, "exit_code": 0, "rate_limit_events": []}}, "SYN")))
    check("probe_py_round_with_result_passes", not exits(lambda: P.stop_on_signal({"development": {"result_event": {"subtype": "success", "is_error": False}, "exit_code": 0, "rate_limit_events": []}}, "SYN")))

# 5. decisive markers: empty / unrelated output is unobservable; every recorded 01-04 no-model test has a marker
check("empty_output_unobservable", not P2.observe("")["observed"])
check("unrelated_output_unobservable", not P2.observe("some text\nexit 0\n")["observed"])
n = 0
for f in ("first-probes-20261112/evidence/P2.json", "remaining-probes-20261112/evidence/E.json", "remaining-probes-20261112/evidence/E1b.json"):
    for t in json.loads((ROOT / f).read_text())["tests"]:
        if "output" in t:
            n += 1
            check(f"recorded_{t['id']}_observed", P2.observe(t["output"])["observed"])
print(f"recorded no-model tests checked: {n}")
print("RESULT", "FAIL" if fails else "PASS", len(fails))
sys.exit(1 if fails else 0)
