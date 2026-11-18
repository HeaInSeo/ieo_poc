"""Offline black-box checks against EXPECTED.md. All writes go to a temporary copy."""
import argparse
from copy import deepcopy
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

POC = Path(__file__).resolve().parents[4]
FIX = Path("reports/tasks/09-01/result-export-20261118/fixtures")
VIEWS = ("STATUS.md", "TASKS.md", "RESULTS.md", "RESUME.md", "events.jsonl")
checks = []


def check(name, passed, observed=""):
    checks.append(bool(passed))
    print(f"{'PASS' if passed else 'FAIL'} {name} {observed}".rstrip())


def read(path):
    return json.loads(path.read_bytes())


def write(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def invoke(*args):
    return subprocess.run(args, capture_output=True, text=True, check=False)


def run(root, tmp, binaries):
    progress = root / "progress"
    ledger = progress / "state.json"
    base = read(ledger)
    neutral = deepcopy(base)
    neutral.update(snapshot_id="P33-SYNTHETIC-0", events=[], extra_tasks=[])
    for task in neutral["tasks"]:
        task.update(status="미착수", verification_level="unverified", evidence=[],
                    latest_report=None, result_state="미실행")
    for experiment in neutral["experiments"]:
        experiment.update(evaluation_closed=False, runs=[], latest_report=None)
    write(ledger, neutral)

    def render(*args):
        return invoke(sys.executable, "-I", root / "scripts/render_progress.py", "--root", root, *args)

    def views():
        return {name: (progress / name).read_bytes() for name in VIEWS}

    def status():
        return (progress / "STATUS.md").read_text()

    def task(state, key):
        return next(row for row in state["tasks"] if row["task_id"] == key)

    report_ref = "reports/p33-independent.txt"
    report = root / report_ref
    report.write_text("Synthetic fixture. One run, measured cost 17 units.\n")
    report_bytes = report.read_bytes()
    check("N1 initial separate counts", render().returncode == 0 and all(
        row in status() for row in ("| 주계획 | 0/73 |", "| 후속 실험 | 0/12 |",
                                    "| 주계획 실험 | 0/31 |", "| 후속 후보 | 0/5 |")))
    check("N1 no extra milestone or P15 rows", all(
        not row.startswith(("| P15 ·", "| S0 ·", "| S1 ·", "| S2 ·", "| S3 ·"))
        for row in (progress / "TASKS.md").read_text().splitlines()))

    mixed = deepcopy(neutral)
    mixed["snapshot_id"] = "P33-SYNTHETIC-1"
    task(mixed, "09-01").update(status="검수 완료", evidence=[report_ref],
                                latest_report=report_ref, verification_level="verified")
    task(mixed, "09-02").update(status="조건 대기", result_state="INCONCLUSIVE · 보류")
    task(mixed, "09-03").update(status="종료 미충족", result_state="FAIL · 기각")
    exp = next(row for row in mixed["experiments"] if row["experiment_id"] == "P33")
    exp.update(evaluation_closed=True, adoption="기각", verdict="FAIL", latest_report=report_ref,
               runs=[{"run_id": "p33-001", "revision": 1, "cost": 17, "latest_report": report_ref}])
    write(ledger, mixed)
    check("N2 task and experiment counts independent", render().returncode == 0 and all(
        row in status() for row in ("| 주계획 | 1/73 |", "| 주계획 실험 | 1/31 |",
                                    "| 후속 실험 | 0/12 |")))
    check("N2 transition remains unverified", "| S3 | 미확인 |" in status())
    pristine = views()
    state_bytes = ledger.read_bytes()
    check("N3 repeated render stable", render().returncode == 0 and pristine == views()
          and ledger.read_bytes() == state_bytes and report.read_bytes() == report_bytes)

    def case(name, change):
        altered = deepcopy(mixed)
        change(altered)
        write(ledger, altered)
        result = render()
        check(name, result.returncode != 0 and views() == pristine, f"rc={result.returncode}")
        write(ledger, mixed)
        render()

    case("N2 no evidence rejected", lambda s: task(s, "09-01").update(evidence=[]))
    case("N2 empty evidence object rejected", lambda s: task(s, "09-01").update(
        evidence=[{}], latest_report=None))
    case("N3 duplicate task rejected", lambda s: s["tasks"].append(deepcopy(s["tasks"][0])))
    case("N3 duplicate run rejected", lambda s: next(
        e for e in s["experiments"] if e["experiment_id"] == "P33")["runs"].append(deepcopy(exp["runs"][0])))
    case("N3 changed denominator alone rejected", lambda s: s["baselines"].update(main_tasks=72))
    def shrink(s):
        s["tasks"].remove(task(s, "09-06"))
        for row in s["experiments"]:
            row["task_ids"] = [key for key in row["task_ids"] if key != "09-06"]
        s["baselines"]["main_tasks"] = 72
    case("N3 changed denominator with matching row removal rejected", shrink)
    def replace_task(s):
        task(s, "09-06")["task_id"] = "99-99"
        for row in s["experiments"]:
            row["task_ids"] = ["99-99" if key == "09-06" else key for key in row["task_ids"]]
    case("N3 unknown same-count replacement rejected", replace_task)

    write(ledger, neutral)
    render()
    old = views()
    write(ledger, mixed)
    crash = """
import os, pathlib, runpy, sys
replace = pathlib.Path.replace
def interrupt(src, dst):
    replace(src, dst)
    os._exit(73)
pathlib.Path.replace = interrupt
script, root = sys.argv[1:]
sys.argv = [script, '--root', root]
runpy.run_path(script, run_name='__main__')
"""
    interrupted = invoke(sys.executable, "-I", "-c", crash, root / "scripts/render_progress.py", root)
    partial = views()
    check("N4 forced process exit leaves mixed snapshots", interrupted.returncode == 73
          and partial["STATUS.md"] == pristine["STATUS.md"] and partial["TASKS.md"] == old["TASKS.md"]
          and render("--check").returncode == 1)
    check("N4 regeneration byte-exact and inputs unchanged", render().returncode == 0
          and render("--check").returncode == 0 and views() == pristine and ledger.read_bytes() == state_bytes
          and report.read_bytes() == report_bytes)
    report.unlink()
    check("N5 missing report rejected", render().returncode == 1 and views() == pristine)
    report.write_bytes(report_bytes)
    report.chmod(0)
    try:
        if os.access(report, os.R_OK):
            raise RuntimeError("run as an unprivileged user to exercise EACCES")
        result = render()
        check("N5 unreadable existing report rejected", result.returncode != 0, f"rc={result.returncode}")
    finally:
        report.chmod(0o600)

    write(ledger, neutral)
    export(root, tmp, binaries, ledger, neutral, shrink, replace_task)


def export(root, tmp, binaries, ledger, neutral, shrink, replace_task):
    bundle = read(root / FIX / "input.json")
    inp = tmp / "input.json"

    def cli(data, name="export"):
        write(inp, data)
        return invoke(sys.executable, root / "scripts/export_results.py", "--root", root, "--input", inp,
                      "--authority", root / FIX / "authority.json", "--out", tmp / name,
                      "--collector", binaries / "ieo-evidence", "--spec", binaries / "ieo-spec")

    first = cli(bundle)
    if first.returncode:
        raise RuntimeError(first.stderr)
    folder = Path(first.stdout.strip())
    result = read(folder / "export.json")
    spec = read(folder / "sprint.json")
    rows = {row["candidate"]: row for row in result["runs"]}
    work = result["work"]
    gates = result["goals"]
    progress = result["progress"]
    check("N1 exporter historical denominator separate", progress["historical"]["accepted"] == 0
          and progress["historical"]["total"] == 76
          and progress["main"] == {"accepted": 0, "total": 73}
          and progress["followup"] == {"accepted": 0, "total": 12})
    check("P1 adoption and partial scope", gates["runtime"]["covered_scopes"] == ["base"]
          and gates["partial"]["covered_scopes"] == ["worker"] and work["90-03"]["state"] == "조건 대기")
    check("P2 verdict split", rows["missing"]["verdict"] == "INCONCLUSIVE"
          and rows["failure"]["verdict"] == "FAIL" and work["90-04"]["state"] == "조건 대기")
    check("P3 optional rejection and unresolved required work", rows["optional"]["adoption"] == "기각"
          and "partial" in result["required_goals_unresolved"])
    bad_asset = result["assets"]["no-provenance"]
    check("P4 invalid provenance cannot integrate", bad_asset["status"] not in {"편입 대기", "검증됨"}
          and "missing_origin" in bad_asset["issues"]
          and result["assets"]["meter"]["status"] in {"편입 대기", "검증됨"})
    check("P5 family budget stops retry", result["budgets"]["runtime"]["attempts"] == 3
          and result["budgets"]["runtime"]["automatic_retry"] is False)
    check("P6 stale scope remains unproven independently of base", gates["stale"]["covered_scopes"] == []
          and gates["runtime"]["covered_scopes"] == ["base"])
    check("P8 draft without approval or execution", spec["registration_state"] == "draft"
          and result["approval"]["execution"] == "not_started")
    before = {p.relative_to(folder): hashlib.sha256(p.read_bytes()).hexdigest()
              for p in folder.rglob("*") if p.is_file()}
    replay = cli(bundle)
    after = {p.relative_to(folder): hashlib.sha256(p.read_bytes()).hexdigest()
             for p in folder.rglob("*") if p.is_file()}
    check("P8 same input same package bytes", replay.returncode == 0 and replay.stdout == first.stdout
          and before == after)

    updated = deepcopy(bundle)
    updated["goals"][0]["target"]["spec_revision"] = "p33-independent-new-spec"
    new = cli(updated)
    if new.returncode:
        raise RuntimeError(new.stderr)
    newer = read(Path(new.stdout.strip()) / "export.json")
    check("P8 changed target creates a new draft and preserves prior bytes",
          newer["revision"] == 2 and newer["previous"]["input_digest"] == result["input_digest"]
          and newer["changes"]["reverify_work"] == ["90-01", "90-07"]
          and before == {p.relative_to(folder): hashlib.sha256(p.read_bytes()).hexdigest()
                         for p in folder.rglob("*") if p.is_file()},
          str(newer["changes"]["reverify_work"]))
    removed = deepcopy(bundle)
    removed["work"] = [row for row in removed["work"] if row["task_id"] != "90-03"]
    missing = cli(removed, "required-scope")
    if missing.returncode:
        raise RuntimeError(missing.stderr)
    missing_path = Path(missing.stdout.strip())
    check("P7 omitted required work remains unready",
          "partial" in read(missing_path / "export.json")["required_goals_unresolved"]
          and read(missing_path / "sprint.json")["registration_state"] == "draft")
    for name, transform in (("row and denominator", shrink), ("unknown same-count ID", replace_task)):
        state = deepcopy(neutral)
        transform(state)
        write(ledger, state)
        trial = cli(bundle, "invalid-ledger")
        check(f"N3 exporter rejects {name}", trial.returncode == 1
              and "ledger ID/track mismatch" in trial.stderr)
    write(ledger, neutral)

    for name, transform in (
        ("P7 cycle rejected", lambda x: x["work"][0]["after"].append(x["work"][0]["task_id"])),
        ("P7 absent prerequisite rejected", lambda x: x["work"][0]["after"].append("99-99")),
        ("P7 duplicated result rejected", lambda x: x["runs"].append(deepcopy(x["runs"][0]))),
    ):
        altered = deepcopy(bundle)
        transform(altered)
        trial = cli(altered, "negative")
        check(name, trial.returncode == 1, trial.stderr.strip().splitlines()[-1] if trial.stderr else "")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binaries", type=Path)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="p33-independent-") as sandbox:
        tmp = Path(sandbox)
        isolated = tmp / "docs/poc"
        shutil.copytree(POC, isolated)
        shutil.copytree(POC.parent.parent / "contracts", tmp / "contracts")
        run(isolated, tmp, args.binaries.resolve())
    print(f"RESULT {sum(checks)}/{len(checks)}")
    sys.exit(0 if all(checks) else 1)
