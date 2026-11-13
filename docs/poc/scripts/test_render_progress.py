#!/usr/bin/env python3
"""01-06 verification of scripts/render_progress.py on temporary copies of docs/poc (never the real ledger).

Cases: normal render and idempotence, hand-edit detection, invalid ledger (no write), evidence-link rules,
interrupted generation and regeneration, separate main/follow-up counts, no timer or network use;
02-01: the same snapshot_id/ledger digest in every view and no combined main+follow-up progress rate.
Run from anywhere: python3 -I docs/poc/scripts/test_render_progress.py   (prints PASS/FAIL lines, exit 0 on all PASS)
"""
import hashlib, json, shutil, subprocess, sys, tempfile
from pathlib import Path

POC = Path(__file__).resolve().parents[1]
SCRIPT = POC / "scripts/render_progress.py"
VIEWS = ("STATUS.md", "TASKS.md", "RESULTS.md", "RESUME.md", "events.jsonl")
fails = []


def check(name, cond, detail=""):
    print(("PASS " if cond else "FAIL ") + name + (f"  ({detail})" if detail and not cond else ""))
    if not cond:
        fails.append(name)


def run(root, *extra):
    r = subprocess.run([sys.executable, "-I", str(root / "scripts/render_progress.py"), "--root", str(root), *extra], capture_output=True, text=True)
    return r.returncode, r.stdout + r.stderr


def views_digest(root):
    h = hashlib.sha256()
    for v in VIEWS:
        p = root / "progress" / v
        h.update(v.encode() + b"\0" + (p.read_bytes() if p.exists() else b"<missing>") + b"\0")
    return h.hexdigest()


def fresh(td):
    root = Path(td) / "poc"
    shutil.copytree(POC, root, ignore=shutil.ignore_patterns("__pycache__", "*.pyc"))
    return root


def mutate(root, fn):
    p = root / "progress/state.json"
    s = json.loads(p.read_text())
    fn(s)
    p.write_text(json.dumps(s, ensure_ascii=False, indent=2) + "\n")


with tempfile.TemporaryDirectory() as td:
    root = fresh(td)
    # 1. normal: the committed views match the ledger; a render is byte-identical (idempotent)
    rc, out = run(root, "--check")
    check("committed_views_match_ledger", rc == 0, out)
    before = views_digest(root)
    rc, out = run(root)
    check("render_ok", rc == 0, out)
    check("render_idempotent", views_digest(root) == before)
    st = json.loads((root / "progress/state.json").read_text())
    status = (root / "progress/STATUS.md").read_text()
    main_done = sum(t["track"] == "main" and t["status"] == "검수 완료" for t in st["tasks"])
    fu_done = sum(t["track"] == "followup" and t["status"] == "검수 완료" for t in st["tasks"])
    check("main_and_followup_counted_separately",
          f"| 주계획 | {main_done}/{st['baselines']['main_tasks']} |" in status and f"| 후속 실험 | {fu_done}/{st['baselines']['followup_tasks']} |" in status)
    check("ledger_digest_in_views", hashlib.sha256((root / "progress/state.json").read_bytes()).hexdigest() in status)
    # 02-01: every generated view carries the same snapshot_id and ledger digest; no combined progress
    stamp = f"snapshot `{st['snapshot_id']}` · 원장 SHA-256 `{hashlib.sha256((root / 'progress/state.json').read_bytes()).hexdigest()}`"
    views = {v: (root / "progress" / v).read_text() for v in ("STATUS.md", "TASKS.md", "RESULTS.md", "RESUME.md")}
    check("same_snapshot_and_digest_in_all_views", all(stamp in t for t in views.values()),
          ",".join(v for v, t in views.items() if stamp not in t))
    total = st["baselines"]["main_tasks"] + st["baselines"]["followup_tasks"]
    check("no_combined_progress_rate",
          not any(f"/{total} |" in t or f"/{total})" in t or "합산 진행률 |" in t for t in views.values()))

    # 2. hand edit of a view is detected by --check (exit 1, names the file)
    p = root / "progress/TASKS.md"
    p.write_text(p.read_text() + "\nhand edit\n")
    rc, out = run(root, "--check")
    check("hand_edit_detected", rc == 1 and "TASKS.md" in out, out)
    run(root)
    check("rerender_restores_after_hand_edit", run(root, "--check")[0] == 0)

with tempfile.TemporaryDirectory() as td:
    root = fresh(td)
    # X-01 (amendment A3): an accepted extra task is listed apart and never changes 73/12 counts
    st0 = json.loads((root / "progress/state.json").read_text())
    main_done0 = sum(t["track"] == "main" and t["status"] == "검수 완료" for t in st0["tasks"])
    def accept_extra(s):
        s.setdefault("extra_tasks", []).append({"task_id": "X-99", "title": "시험 추가 작업", "track": "extra",
            "kind": "구현", "status": "검수 완료", "verification_level": "verified",
            "evidence": ["https://example.invalid/x"], "prerequisites": []})
    mutate(root, accept_extra)
    rc, out = run(root)
    status = (root / "progress/STATUS.md").read_text()
    check("extra_task_listed_apart", rc == 0 and "X-99" in status and "추가 작업" in (root / "progress/TASKS.md").read_text(), out)
    check("extra_task_not_counted", f"| 주계획 | {main_done0}/73 |" in status, status[:600])
    mutate(root, lambda s: s["extra_tasks"][-1].update({"task_id": "01-01"}))
    rc, out = run(root)
    check("extra_task_id_collision_rejected", rc == 1, out)
    mutate(root, lambda s: s["extra_tasks"][-1].update({"task_id": "X-99", "evidence": []}))
    rc, out = run(root)
    check("extra_task_accepted_without_evidence_rejected", rc == 1, out)

with tempfile.TemporaryDirectory() as td:
    root = fresh(td)
    base = views_digest(root)
    # 3. invalid ledger: accepted task without evidence -> exit 1, views untouched
    def no_evidence(s):
        t = next(t for t in s["tasks"] if t["status"] == "검수 완료")
        t["evidence"] = []
    mutate(root, no_evidence)
    rc, out = run(root)
    check("accepted_without_evidence_rejected", rc == 1 and "검수 완료" in out, out)
    check("invalid_ledger_writes_nothing", views_digest(root) == base)

with tempfile.TemporaryDirectory() as td:
    root = fresh(td)
    base = views_digest(root)
    # 4. evidence-link rules: missing local file, http URL, parent path -> rejected; https URL -> accepted
    for name, ref, ok in (("missing_local_evidence_rejected", "reports/no-such-file.md", False),
                          ("http_url_rejected", "http://example.invalid/x", False),
                          ("parent_path_rejected", "../README.md", False),
                          ("absolute_path_rejected", "/etc/hostname", False),
                          ("https_url_accepted", "https://example.invalid/x", True)):
        shutil.copy2(POC / "progress/state.json", root / "progress/state.json")
        mutate(root, lambda s, ref=ref: s["tasks"][0].setdefault("evidence", []).append(ref))
        rc, out = run(root)
        check(name, (rc == 0) == ok, out)
    shutil.copy2(POC / "progress/state.json", root / "progress/state.json")
    run(root)
    check("restored_after_link_cases", views_digest(root) == base)
    # 5. counts must match the baselines (a removed task is rejected)
    mutate(root, lambda s: s["tasks"].pop())
    rc, out = run(root)
    check("baseline_count_mismatch_rejected", rc == 1 and "기준선" in out, out)
    shutil.copy2(POC / "progress/state.json", root / "progress/state.json")

with tempfile.TemporaryDirectory() as td:
    root = fresh(td)
    base = views_digest(root)
    # 6. interrupted generation: change the ledger, then crash after writing temporaries and swapping one view
    mutate(root, lambda s: s.update(snapshot_id=s["snapshot_id"] + "-T"))
    crash = (
        "import runpy,sys,pathlib\n"
        "orig=pathlib.Path.replace\n"
        "n={'k':0}\n"
        "def flaky(self,target):\n"
        "    n['k']+=1\n"
        "    if n['k']==2: raise OSError('simulated crash')\n"
        "    return orig(self,target)\n"
        "pathlib.Path.replace=flaky\n"
        f"sys.argv=['render_progress.py','--root',{str(root)!r}]\n"
        f"runpy.run_path({str(root / 'scripts/render_progress.py')!r},run_name='__main__')\n")
    r = subprocess.run([sys.executable, "-I", "-c", crash], capture_output=True, text=True)
    check("simulated_crash_reported", r.returncode != 0 or "simulated crash" in (r.stdout + r.stderr), r.stdout + r.stderr)
    leftovers = sorted(x.name for x in (root / "progress").glob("*.tmp"))
    check("crash_left_partial_state", bool(leftovers) or views_digest(root) != base)
    rc, out = run(root, "--check")
    check("check_flags_partial_state", rc == 1 and "재생성 필요" in out, out)
    rc, out = run(root)
    check("rerun_after_crash_ok", rc == 0, out)
    check("no_tmp_left_after_rerun", not list((root / "progress").glob("*.tmp")))
    check("check_ok_after_rerun", run(root, "--check")[0] == 0)
    check("views_reflect_new_ledger", "-T`" in (root / "progress/STATUS.md").read_text())

with tempfile.TemporaryDirectory() as td:
    root = fresh(td)
    # 8. a stray temporary from an interrupted run is reported by --check even when every view is current
    (root / "progress/STATUS.md.tmp").write_text("partial")
    rc, out = run(root, "--check")
    check("stray_tmp_flagged_by_check", rc == 1 and "STATUS.md.tmp" in out, out)
    run(root)
    check("render_removes_stray_tmp", not list((root / "progress").glob("*.tmp")) and run(root, "--check")[0] == 0)

# 7. no timer or network: the renderer imports nothing for scheduling or fetching
src = SCRIPT.read_text()
check("no_timer_or_network_in_renderer", not any(w in src for w in ("import time", "sched", "threading", "urllib.request", "http.client", "socket", "subprocess")))

print("RESULT", "FAIL" if fails else "PASS", len(fails))
sys.exit(1 if fails else 0)
