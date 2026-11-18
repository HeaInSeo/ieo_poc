"""Exercise the final candidate in a disposable package root."""
from copy import deepcopy
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


def check(root):
    poc = root / "docs/poc"
    ledger = poc / "progress/state.json"
    original = json.loads(ledger.read_bytes())
    files = ("STATUS.md", "TASKS.md", "RESULTS.md", "RESUME.md", "events.jsonl")

    def views():
        return [(ledger.parent / name).read_bytes() for name in files]

    def execute(script, *args):
        return subprocess.run([sys.executable, "-I", poc / "scripts" / script, "--root", poc, *args],
                              capture_output=True, text=True)

    def render():
        return execute("render_progress.py")

    def set_state(state):
        ledger.write_text(json.dumps(state, ensure_ascii=False, indent=2) + "\n")

    def row(state, id_):
        return next(e for e in state["experiments"] if e["experiment_id"] == id_)

    base = views()
    for track, replacement in (("P36", "P15"), ("P30", "FX-UNKNOWN")):
        state = deepcopy(original)
        row(state, track)["experiment_id"] = replacement
        set_state(state)
        result = render()
        assert result.returncode == 1 and "등록 실험 ID·트랙" in result.stderr and views() == base
        print(f"PASS same-count {track} -> {replacement} rejected before writes")
    state = deepcopy(original)
    row(state, "P01")["track"] = "followup"
    row(state, "P32")["track"] = "main"
    set_state(state)
    result = render()
    assert result.returncode == 1 and "등록 실험 ID·트랙" in result.stderr and views() == base
    print("PASS P01/P32 track exchange rejected before writes")

    set_state(original)
    assert render().returncode == 0
    blank = execute("validate_package.py")
    assert blank.returncode == 0, blank.stderr
    before = json.loads(blank.stdout)
    assert before["ledger_run_revisions"] == dict(synthetic=0, actual=0, unclassified=0)
    print("PASS frozen package has zero run revisions")

    run_proposals = [poc / "reports/tasks/09-02/final-independent/prior-run.json",
                     poc / "reports/tasks/09-02/independent-retest/p33-retest-run-proposal.json"]
    state = deepcopy(original)
    p33 = row(state, "P33")
    p33["runs"] = [json.loads(path.read_bytes()) for path in run_proposals]
    set_state(state)
    assert render().returncode == 0
    populated = execute("validate_package.py")
    assert populated.returncode == 0, populated.stderr
    after = json.loads(populated.stdout)
    assert after["ledger_run_revisions"] == dict(synthetic=2, actual=0, unclassified=0)
    assert after["runtime_probe"] == after["human_trials"] == "NOT_RUN"
    assert before["progress"] == after["progress"]
    status = (ledger.parent / "STATUS.md").read_text()
    assert "| 주계획 실험 | 0/31 |" in status and "| 후속 후보 | 0/5 |" in status
    results = (ledger.parent / "RESULTS.md").read_text()
    assert all(run["run_id"] in results for run in p33["runs"])
    print("PASS both independent P33 runs accepted; counts 2 synthetic / 0 actual / 0 unclassified")
    print("PASS no change to task progress, evaluation closure, or runtime/human trial status")
    populated_views = views()

    for field, value, error in (
        ("latest_report", "reports/nonexistent.txt", "실제 파일이 없는"),
        ("revision", 0, "잘못된 실험 회차"),
        ("run_id", p33["runs"][0]["run_id"], "중복 실험 회차"),
    ):
        altered = deepcopy(state)
        row(altered, "P33")["runs"][1][field] = value
        set_state(altered)
        result = render()
        package = execute("validate_package.py")
        assert result.returncode == 1 and error in result.stderr and views() == populated_views
        assert package.returncode != 0 and error in package.stderr
        print(f"PASS invalid P33 {field} rejected by renderer and package")


if __name__ == "__main__":
    repo = Path(__file__).resolve().parents[6]
    with tempfile.TemporaryDirectory(prefix="ieo-p33-scope-") as tmp:
        root = Path(tmp) / "package"
        shutil.copytree(repo, root, ignore=shutil.ignore_patterns(".git", "__pycache__"))
        check(root)
