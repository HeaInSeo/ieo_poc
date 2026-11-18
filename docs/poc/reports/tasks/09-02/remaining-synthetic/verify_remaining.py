"""Offline synthetic 09-02 checks. Keep raw inputs, CLI results and stdout/stderr."""
from copy import deepcopy
from hashlib import sha256
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[5]
FIX = "reports/tasks/09-01/result-export-20261118/fixtures"
NEW = "reports/p33-remaining-fixtures"
EXPECTED = {
    "P12": "04-09 07-04", "P19": "06-02 08-07", "P20": "06-03 06-04",
    "P24": "06-05 06-06", "P22": "03-06 03-08", "P23": "05-01 05-02 05-03",
    "P26": "07-02 07-03", "P27": "07-06 07-07", "P28": "04-11 04-12 07-05",
    "P29": "09-01 09-02 10-01 10-05", "P30": "09-01 09-03 10-02 10-05",
    "P31": "09-01 09-04 10-03 10-05", "P32": "09-01 09-05 10-04 10-05",
    "P33": "05-04 11-03",
}
checks = []


def read(path):
    return json.loads(path.read_bytes())


def write(path, obj):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(obj, ensure_ascii=False, indent=2) + "\n")


def check(name, ok, observed=None):
    checks.append(bool(ok))
    print(f"{'PASS' if ok else 'FAIL'} {name}", json.dumps(observed, ensure_ascii=False))


def digest(path):
    return sha256(path.read_bytes()).hexdigest()


def inspect_references(root, out, view):
    historical = (root / "01_plan_and_review/Console_Coordinator_PoC_Validation_Plan_v1.2.md").read_text()
    section = historical.split("### 9.1 ", 1)[1].split("### 9.12 ", 1)[0]
    rows = {}
    for line in section.splitlines():
        if re.match(r"^\| \d\d-\d\d · ", line):
            fields = line.split("|")
            id_ = fields[1].strip().split()[0]
            assert id_ not in rows
            rows[id_] = {"title": fields[1].strip(), "refs": fields[5].strip()}
    table = (root / "V1_3_Changes_and_Validation.md").read_text()
    documented = dict(re.findall(
        r"^\| v1\.2/(\d\d-\d\d) \| (?:주계획|후속) v1\.3\.1/([\w-]+) \|", table, re.M))
    exported = {row["legacy_v1_2_id"]: row["task_id"] for row in view["poc_tasks"] if row["legacy_v1_2_id"]}
    state = read(root / "progress/state.json")
    experiments = {e["experiment_id"]: e for e in state["experiments"]}
    universe = {key: e["track"] for key, e in experiments.items()}
    main = read(root / "Console_Coordinator_Sprints_v0.json")
    current = {t["task_id"]: t for s in main["sprints"] for t in s["tasks"]}
    followup = {t["id"]: t for t in read(root / "Followup_Execution_v1.3.3.json")["tasks"]}
    mapped = [{
        "v1.2": old, **row, "documented": documented.get(old),
        "exported": exported.get(old),
        "registered_title": (current | followup).get(exported.get(old), {}).get("title"),
        "registered_refs": (current | followup).get(exported.get(old), {}).get("references"),
    } for old, row in sorted(rows.items())]
    write(out / "v1.2-crosswalk.json", mapped)
    check("R5 all 76 numbered tasks mapped once",
          len(rows) == len(exported) == 76 and set(rows) == set(exported) and documented == exported,
          {"historical": len(rows), "documented": len(documented), "exported": len(exported)})
    for key in (f"T{i:02}" for i in range(1, 10)):
        ids = [old for old, row in rows.items() if re.search(rf"\b{key}\b", row["refs"])]
        check(f"R5 {key} all original reference rows mapped", bool(ids) and all(old in exported for old in ids), ids)
    split_details = {}
    for label, ids in EXPECTED.items():
        old_ids = ids.split()
        expected_track = "followup" if label in ("P29", "P30", "P31", "P32") else "main"
        destinations = [exported.get(old) for old in old_ids]
        valid = all(dest in (followup if expected_track == "followup" else current) for dest in destinations)
        linked = experiments[label]["task_ids"]
        direct = destinations if expected_track == "main" else destinations[1:3]
        via = {} if expected_track == "main" else {
            "shared_source": destinations[0], "combination": destinations[3],
            "candidate_depends_on_source": destinations[0] in followup[direct[0]]["deps"],
            "evaluation_depends_on_candidate": direct[0] in followup[direct[1]]["deps"],
            "combination_depends_on_evaluation": direct[1] in followup[destinations[3]]["deps"],
        }
        split_details[label] = {"mapping": dict(zip(old_ids, destinations)), "ledger_links": linked, "via": via}
        check(f"R5 {label} split links preserve track and unique experiment row",
              valid and universe.get(label) == expected_track and set(direct) <= set(linked)
              and all(value is True for key, value in via.items() if "depends" in key),
              split_details[label])
    write(out / "v1.2-split-references.json", split_details)
    history = view["progress"]["historical"]
    historic_experiments = re.findall(r"^### (P\d\d) ·", historical, re.M)
    check("R5 P15 excluded; S0–S3 milestones and denominators separate",
          "P15" not in universe and len(set(historic_experiments) - {"P15"}) == 32
          and all(f"S{i}" in state["milestones"] and f"S{i}" not in universe for i in range(4))
          and state["baselines"] == dict(main_tasks=73, followup_tasks=12, main_experiments=31, followup_candidates=5)
          and history["version"] == "v1.2" and history["total"] == 76
          and exported.get("05-04") == "09-02" and exported.get("11-03") == "09-05",
          {"historical_experiments": len(set(historic_experiments) - {"P15"}), "baselines": state["baselines"]})
    state_path = root / "progress/state.json"
    original = state_path.read_bytes()
    report = f"{NEW}/split-test.txt"
    (root / NEW).mkdir(exist_ok=True)
    (root / report).write_text("Synthetic split-scope counters, no actual trial.\n")
    for key in EXPECTED:
        e = experiments[key]
        e["runs"] = [
            {"run_id": f"{key}-split-synthetic", "revision": revision,
             "execution_kind": "synthetic", "latest_report": report}
            for revision in (1, 2)
        ]
    script = root / "scripts/render_progress.py"

    def snapshot(name):
        write(state_path, state)
        rendered = subprocess.run([sys.executable, "-I", script, "--root", root], capture_output=True, text=True)
        (out / f"{name}-renderer-stderr.txt").write_text(rendered.stderr)
        assert rendered.returncode == 0, rendered.stderr
        status = (root / "progress/STATUS.md").read_text()
        (out / f"{name}-status.txt").write_text(status)
        return status

    try:
        partial = snapshot("split-partial")
        check("R5 split/repeated rounds alone do not close any evaluation",
              "| 주계획 실험 | 0/31 |" in partial and "| 후속 후보 | 0/5 |" in partial)
        for key in set(EXPECTED) - {"P33"}:
            experiments[key]["evaluation_closed"] = True
        closed = snapshot("split-declared-closed")
        check("R5 each declared split closure counts once; P33 remains open",
              "| 주계획 실험 | 9/31 |" in closed and "| 후속 후보 | 4/5 |" in closed
              and f"| 주계획 | {view['progress']['main']['accepted']}/73 |" in closed
              and f"| 후속 실험 | {view['progress']['followup']['accepted']}/12 |" in closed)
    finally:
        state_path.write_bytes(original)


def run(root, workdir, evidence, binaries):
    fixture = root / FIX
    original = read(fixture / "input.json")
    authority = read(fixture / "authority.json")

    def cli(name, bundle, auth=authority, group=None):
        case = evidence / name
        case.mkdir()
        write(case / "input.json", bundle)
        write(case / "authority.json", auth)
        cmd = [sys.executable, root / "scripts/export_results.py", "--root", root, "--input", case / "input.json",
               "--authority", case / "authority.json", "--out", workdir / (group or name),
               "--collector", binaries / "ieo-evidence", "--spec", binaries / "ieo-spec"]
        p = subprocess.run(cmd, capture_output=True, text=True)
        (case / "stderr.txt").write_text(p.stderr)
        (case / "stdout.txt").write_text(p.stdout)
        (case / "exit.txt").write_text(f"{p.returncode}\n")
        if p.returncode:
            return p, None, None
        folder = Path(p.stdout.strip())
        for artifact in ("export.json", "sprint.json", "DRAFT.md"):
            shutil.copyfile(folder / artifact, case / (artifact + ".txt" if artifact == "DRAFT.md" else artifact))
        return p, read(folder / "export.json"), folder

    def issue(stem, entry, raw, decision, scopes, observed, auth):
        ref = f"{NEW}/{stem}-result.json"
        review_ref = f"{NEW}/{stem}-review.json"
        disposal_ref = f"{NEW}/{stem}-disposition.json"
        obs = f"{NEW}/{stem}-observation.txt"
        (root / NEW).mkdir(exist_ok=True)
        (root / obs).write_text(observed + "\n")
        raw.update(operational_report_ref=review_ref, evidence_refs=[obs])
        raw["verification"]["evidence"] = [obs]
        write(root / ref, raw)
        disposal = read(root / entry["disposition"])
        disposal.update(result_sha256=digest(root / ref), decision=decision, scopes=scopes)
        write(root / disposal_ref, disposal)
        prior = read(root / entry["result"])
        review = read(root / prior["operational_report_ref"])
        review.update(event_revision=raw["revision"], evidence=[
            f"sha256:{digest(root / obs)}  {obs}", f"sha256:{digest(root / ref)}",
            f"sha256:{digest(root / disposal_ref)}",
        ])
        write(root / review_ref, review)
        auth["approvals"][review_ref] = digest(root / review_ref)
        return {**entry, "result": ref, "disposition": disposal_ref}

    _, baseline, folder = cli("baseline", original)
    assert baseline is not None
    inspect_references(root, evidence, baseline)

    bundle, auth = deepcopy(original), deepcopy(authority)
    prior = next(e for e in bundle["runs"] if e["candidate"] == "failure")
    raw = read(root / prior["result"])
    raw.update(run_id="paired-methods", expected="Two standalone methods must also compose correctly")
    entry = issue("combination", prior, raw, "기각", [], "Combined fixture: outputs conflict; standalone control is valid.", auth)
    bundle["runs"].append({**entry, "candidate": "paired-methods", "records": []})
    bundle["work"].append({**bundle["work"][0], "task_id": "90-31", "candidate": "paired-methods", "asset_ids": []})
    _, result, _ = cli("combination-failure", bundle, auth)
    check("R1 combination held; standalone and unrelated retained", result is not None
          and any(r["candidate"] == "paired-methods" and (r["verdict"], r["adoption"]) == ("FAIL", "기각")
                  for r in result["runs"]) and result["work"]["90-31"]["state"] == "조건 대기"
          and result["work"]["90-01"]["state"] == baseline["work"]["90-01"]["state"] == "편입 대기"
          and result["work"]["90-02"] == baseline["work"]["90-02"]
          and result["approval"]["execution"] == "not_started",
          {k: result["work"][k]["state"] for k in ("90-01", "90-02", "90-31")} if result else None)

    bundle, auth = deepcopy(original), deepcopy(authority)
    prior = next(e for e in bundle["runs"] if e["candidate"] == "missing")
    target = {**bundle["work"][3], "task_id": "90-32", "purpose": "integration", "followup": None}
    bundle["work"].append(target)
    _, before, rev1 = cli("before-recovery", bundle, group="recovery")
    assert before
    archived = digest(rev1 / "export.json")
    raw = read(root / prior["result"])
    raw.update(revision=2, previous_run_ref=prior["result"])
    raw["observed"]["covered_scopes"] = ["missing"]
    bundle["runs"].append(issue("recovered", prior, raw, "채택", ["missing"],
                                "Previously missing scope now independently observed: PASS.", auth))
    _, after, rev2 = cli("after-recovery", bundle, auth, group="recovery")
    _, _, replay = cli("recovery-replay", bundle, auth, group="recovery")
    check("R2 verified recovery changes only relevant work, preserves spend and old draft",
          after is not None and before["work"]["90-32"]["state"] == "조건 대기"
          and after["work"]["90-32"]["state"] == "편입 대기"
          and after["budgets"] == before["budgets"] and digest(rev1 / "export.json") == archived
          and rev2 != rev1 and replay == rev2
          and set(after["changes"]["reverify_work"]) == {"90-04", "90-32"},
          after["changes"]["reverify_work"] if after else None)

    bundle = deepcopy(original)
    estimate_ref = f"{NEW}/remaining-work-estimate.txt"
    (root / estimate_ref).write_text("Independent synthetic worksheet: remaining integration 1234–2345 seconds.\n")
    estimate = {"state": "measured", "range_seconds": {"min": 1234, "max": 2345},
                "evidence": [estimate_ref], "reason": None}
    bundle["work"][0]["estimate"] = estimate
    _, result, folder = cli("measured-and-estimated", bundle)
    spec = read(folder / "sprint.json") if folder else None
    draft = (folder / "DRAFT.md").read_text() if folder else ""
    check("R3 structured estimate retained and excluded from measured totals",
          result is not None and result["budgets"] == baseline["budgets"]
          and result["work"]["90-01"]["estimate"] == estimate
          and next(t for t in spec["tasks"] if t["task_id"] == "90-01")["estimate"] == estimate)
    check("R3 reader-facing draft shows range, unit and estimate provenance",
          "1234" in draft and "2345" in draft and estimate_ref in draft)

    for name, relabel in (("same-bytes-other-run", None),
                          ("same-measurement-existing-id", "failure"),
                          ("same-measurement-fresh-id", "synthetic-unobserved-attempt")):
        bundle = deepcopy(original)
        source, other = bundle["runs"][0], bundle["runs"][2]
        record = root / source["records"][0]
        alias = f"{NEW}/{name}.json"
        shutil.copyfile(record, root / alias)
        if relabel:
            data = read(root / alias)
            data["attempt_id"] = relabel
            write(root / alias, data)
        other["records"] = [*other["records"], alias]
        proc, actual, _ = cli(name, bundle)
        check(f"R4 {name} not counted or accepted",
              proc.returncode == 1 and "duplicate evidence" in proc.stderr,
              {"rc": proc.returncode, "records": actual["budgets"]["runtime"]["summary"]["records"] if actual else None,
               "wall_ms": actual["budgets"]["runtime"]["wall_ms"] if actual else None})
    shutil.copytree(root / NEW, evidence / "added-fixtures")


if __name__ == "__main__":
    destination = Path(sys.argv[2]).resolve()
    destination.mkdir()
    with tempfile.TemporaryDirectory(prefix="ieo-p33-remaining-") as tmp:
        work = Path(tmp)
        root = work / "docs/poc"
        def ignore(directory, names):
            return [name for name in names if name == "__pycache__" or Path(directory, name) == destination]

        shutil.copytree(REPO / "docs/poc", root, ignore=ignore)
        shutil.copytree(REPO / "contracts", work / "contracts")
        run(root, work, destination, Path(sys.argv[1]).resolve())
    print(f"RESULT {sum(checks)}/{len(checks)}")
    sys.exit(0 if all(checks) else 1)
