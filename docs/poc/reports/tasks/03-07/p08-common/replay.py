#!/usr/bin/env python3
"""Compare candidate-selected checks with the approved P22-B policy."""
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import shutil
import subprocess

HERE = Path(__file__).resolve().parent
TASKS = HERE.parents[1]
VERIFY = "/usr/local/bin/ieo-policy-verify"
CANDIDATE = {
    "go": ["go", "test", "-count=1", "-json", "-run", "^TestCandidateSmoke$", "./..."],
    "rust": ["cargo", "test", "--offline", "--locked", "tests::candidate_smoke", "--", "--exact"],
}
MUTATIONS = {
    "go": ("double.go", "\treturn 2 * value", "\treturn value",
           "double_test.go", '\nfunc TestCandidateSmoke(t *testing.T) {\n\tif got := Double(0); got != 0 {\n\t\tt.Fatalf("Double(0) = %d; want 0", got)\n\t}\n}\n'),
    "rust": ("src/lib.rs", "    2 * value", "    value", "src/lib.rs",
             "    #[test]\n    fn candidate_smoke() {\n        assert_eq!(super::double(0), 0);\n    }\n"),
}


def run(argv, **options):
    return subprocess.run(argv, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          text=True, timeout=180, **options)


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def hashes(directory):
    return {str(path.relative_to(directory)): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in sorted(directory.rglob("*")) if path.is_file()}


def stage(output):
    cases = {}
    for stack, mutation in MUTATIONS.items():
        source = TASKS / "03-03/go-rust-20261118/fixtures" / stack
        normal = shutil.copytree(source, output / f"{stack}-normal")
        defective = shutil.copytree(source, output / f"{stack}-defective")
        implementation, old, new, tests, smoke = mutation
        path = defective / implementation
        original = path.read_text()
        require(original.count(old) == 1, f"fixture changed: {path}")
        path.write_text(original.replace(old, new))
        path = defective / tests
        if stack == "rust":
            path.write_text(path.read_text().removesuffix("}\n") + "\n" + smoke + "}\n")
        else:
            path.write_text(path.read_text() + smoke)
        (defective / "candidate-profile.json").write_text(json.dumps(CANDIDATE[stack]) + "\n")
        cases[stack] = normal, defective
    return cases


def counts(stack, output):
    if stack == "go":
        outcomes = [event["Action"] for line in output.splitlines()
                    if line.startswith("{") and (event := json.loads(line)).get("Test")
                    and event["Action"] in {"pass", "fail", "skip"}]
        return [outcomes.count(outcome) for outcome in ("pass", "fail", "skip")]
    matches = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;", output)
    require(len(matches) == 1, "missing or ambiguous Rust summary")
    return list(map(int, matches[0]))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("store", type=Path, help="isolated Podman store with the admitted images")
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    store, output = args.store.resolve(), args.output.resolve()
    require(all(path.is_relative_to("/tmp") and path != Path("/tmp") for path in (store, output)),
            "store and new evidence directory must be under /tmp")
    output.mkdir()
    env = {**os.environ, "CONTAINERS_STORAGE_CONF": str(store / "storage.conf")}
    prefix = ["podman", "--runroot", str(store / "run")]

    def podman(*options, check=True):
        return run([*prefix, *options], env=env, check=check)

    info = json.loads(podman("info", "--format", "json").stdout)
    require(info["host"]["security"]["rootless"] and
            Path(info["store"]["graphRoot"]).resolve() == store / "graph",
            "expected dedicated rootless store")
    pin = json.loads((TASKS / "03-06/p22-basic-20261119/inputs.json").read_text())
    image = json.loads(podman("image", "inspect",
                             "localhost/ieo-policy-0305-20261119-r5:combined").stdout)[0]
    require(image["Digest"] == pin["images"]["combined"], "image digest differs from 03-06")
    require((image["Os"], image["Architecture"]) == ("linux", "amd64"), "wrong image platform")
    profile = (TASKS / "03-05/policy-images-20261119/profiles.json").read_text()
    policy = json.loads(profile)
    require(policy["version"] == pin["approval"]["profile_version"] and
            policy["catalog"] == pin["approval"]["catalog_version"], "wrong approved profile version")
    containers = set()
    results = []

    def observe(label, fixture=None, entrypoint=VERIFY, command=("info",)):
        mounts = [] if fixture is None else ["-v", f"{fixture}:/work:ro,Z", "-w", "/work"]
        options = ["create", "--pull=never", "--network=none", "--read-only",
                   "--cap-drop=all", "--security-opt=no-new-privileges", "--user", "20002:20002",
                   "--tmpfs", "/tmp:rw,exec,nosuid,nodev,mode=1777",
                   "--entrypoint", entrypoint, *mounts, image["Id"], *command]
        before = hashes(fixture) if fixture else {}
        cid = podman(*options).stdout.strip()
        try:
            result = podman("start", "--attach", cid, check=False)
        finally:
            podman("rm", "--force", cid)
        (output / f"{label}.stdout").write_text(result.stdout)
        (output / f"{label}.stderr").write_text(result.stderr)
        require(cid and cid not in containers, f"container reused: {label}")
        containers.add(cid)
        require(not fixture or before == hashes(fixture), f"fixture changed: {label}")
        results.append({"case": label, "container": cid, "rc": result.returncode,
                        "inputs": before, "command": [entrypoint, *command]})
        return result

    cases = stage(output)
    versions = []
    for generation in (1, 2):
        tools = observe(f"tools-{generation}")
        require(tools.returncode == 0 and tools.stdout.startswith(profile),
                "wrong tool/profile baseline")
        versions.append((tools.stdout, tools.stderr))
        for stack, (normal, defective) in cases.items():
            required, = policy["profiles"][stack]["required_tests"]
            failure = {"go": 1, "rust": 101}[stack]
            good = observe(f"{stack}-normal-{generation}", normal, command=(stack,))
            bad = observe(f"{stack}-protected-{generation}", defective, command=(stack,))
            require(good.returncode == 0 and f"PASS profile={stack}" in good.stdout,
                    f"{stack}: normal candidate failed")
            require(bad.returncode == failure and required in bad.stdout and
                    ("Double(3) = 3; want 6" if stack == "go" else "left: 3") in
                    (bad.stdout + bad.stderr), f"{stack}: protected contract did not fail")
            for result, row in zip((good, bad), results[-2:]):
                row["pass_fail_skip"] = counts(stack, result.stdout)
                row["required"] = required
                row["checks"] = [line for line in result.stderr.splitlines() if line.startswith("+ ")]
            require(results[-2]["pass_fail_skip"] == [1, 0, 0] and
                    results[-1]["pass_fail_skip"] == [1, 1, 0], f"{stack}: wrong protected counts")
            if generation == 1:
                command = json.loads((defective / "candidate-profile.json").read_text())
                selected = observe(f"{stack}-candidate", defective, "/usr/bin/env",
                                   ("PATH=/opt/ieo-tools/go/bin:/opt/ieo-tools/rust/bin:/usr/bin:/bin",
                                    "HOME=/tmp", "GOCACHE=/tmp/go-cache", "GOTOOLCHAIN=local",
                                    "CARGO_TARGET_DIR=/tmp/target", "CARGO_HOME=/tmp/cargo", *command))
                results[-1]["pass_fail_skip"] = counts(stack, selected.stdout)
                results[-1]["omitted_required"] = [required]
                require(selected.returncode == 0 and required not in selected.stdout and
                        results[-1]["pass_fail_skip"] == [1, 0, 0], f"{stack}: invalid bypass control")
    require(versions[0] == versions[1], "tool versions or approved profile changed")
    for stack in cases:
        for variant in ("normal", "protected"):
            baseline, fresh = [next(row for row in results if row["case"] == f"{stack}-{variant}-{n}")
                               for n in (1, 2)]
            require(all(baseline[key] == fresh[key] for key in
                        ("rc", "inputs", "command", "checks", "pass_fail_skip", "required")),
                    f"{stack}-{variant}: recreation changed verification")
    (output / "results.json").write_text(json.dumps(
        {"image_id": image["Id"], "digest": image["Digest"], "approval": pin["approval"],
         "cases": results}, indent=2) + "\n")
    print(f"{len(results)}/{len(results)} container observations matched expectations")


if __name__ == "__main__":
    main()
