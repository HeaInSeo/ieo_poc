#!/usr/bin/env python3
"""Replay P22-B on frozen local images in a dedicated Podman store."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import io
import itertools
import json
import os
from pathlib import Path
import re
import secrets
import shlex
import shutil
import subprocess
import tarfile
import time

HERE = Path(__file__).resolve().parent
POLICY = "docs/poc/reports/tasks/03-05/policy-images-20261119"
CATALOG = "docs/poc/reports/tasks/03-04/p25-20261119"
FIXTURES = "docs/poc/reports/tasks/03-03/go-rust-20261118/fixtures"
IMAGE_PREFIX = "localhost/ieo-policy-0305-20261119-r5"
VERIFY = "/usr/local/bin/ieo-policy-verify"
ASSETS = ("workspace", "session", "credential", "approval")


def call(argv, *, cwd=None, env=None, timeout=30, check=True):
    return subprocess.run(argv, cwd=cwd, env=env, timeout=timeout, check=check,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE)


def git(*args, cwd=HERE):
    env = {**os.environ, "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null",
           "GIT_AUTHOR_DATE": "2026-11-19T00:00:00+00:00", "GIT_COMMITTER_DATE": "2026-11-19T00:00:00+00:00"}
    return call(["git", *args], cwd=cwd, env=env).stdout


def create_repo(repo, parent, head, stack, label, defect=False):
    location = parent / f"{stack}-{label}"
    source = f"{FIXTURES}/{stack}"
    with tarfile.open(fileobj=io.BytesIO(git("archive", head, source, cwd=repo))) as archive:
        archive.extractall(parent / "staged", filter="data")
    (parent / "staged" / source).rename(location)
    (location / "README.md").write_text(f"# P22 isolated {stack} repository {label}\n")
    if defect and stack == "go":
        path = location / "double_test.go"
        path.write_text(path.read_text().replace(
            "func TestDouble(t *testing.T) {", 'func TestDouble(t *testing.T) {\n\tt.Skip("P22 control")'))
    if defect and stack == "rust":
        path = location / "src/lib.rs"
        path.write_text(path.read_text().replace("    fn doubles_three()", "    #[ignore]\n    fn doubles_three()"))
    git("init", "-q", "--initial-branch=main", "--template=", cwd=location)
    git("add", ".", cwd=location)
    git("-c", "user.name=P22 fixture", "-c", "user.email=p22@example.invalid",
        "commit", "-q", "-m", f"Freeze {stack} fixture {label}", cwd=location)
    return location, git("rev-parse", "HEAD", cwd=location).decode().strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("store", type=Path, help="temporary directory with storage.conf, graph and run")
    parser.add_argument("output", type=Path, help="new /tmp evidence directory")
    parser.add_argument("--case", action="append", help="run only named cases for diagnosis")
    args = parser.parse_args()
    store, output = args.store.resolve(), args.output.resolve()
    if not all(path.is_relative_to("/tmp") and path != Path("/tmp") for path in (store, output)):
        parser.error("dedicated store and output must be under /tmp")
    output.mkdir()
    (output / "logs").mkdir()
    (output / "fixtures").mkdir()
    probe = output / "probe.py"
    shutil.copyfile(HERE / "probe.py", probe)
    expected = json.loads((HERE / "inputs.json").read_text())
    approval = expected["approval"]
    frozen = approval["head"]
    assert frozen in git("show", "-s", "--format=%P", approval["merge"]).decode().split()
    assert git("show", f"{frozen}:{POLICY}/profiles.json") == git(
        "show", f"{approval['merge']}:{POLICY}/profiles.json")
    pin_paths = (f"{CATALOG}/INDEX.md", f"{POLICY}/profiles.json", f"{POLICY}/verify.py")
    pins = [hashlib.sha256(git("show", f"{frozen}:{path}")).hexdigest() for path in pin_paths]
    env = {**os.environ, "CONTAINERS_STORAGE_CONF": str(store / "storage.conf")}
    prefix = ["podman", "--runroot", str(store / "run")]

    def podman(*options, timeout=30, check=True):
        return call([*prefix, *options], env=env, timeout=timeout, check=check)

    actual_store = json.loads(podman("info", "--format", "{{json .Store}}").stdout)
    assert Path(actual_store["graphRoot"]).resolve() == store / "graph"
    assert Path(actual_store["runRoot"]).resolve() == store / "run"
    images = {}
    for variant, digest in expected["images"].items():
        image = json.loads(podman("image", "inspect", f"{IMAGE_PREFIX}:{variant}").stdout)[0]
        assert image["Digest"] == digest and (image["Os"], image["Architecture"]) == ("linux", "amd64")
        images[variant] = {"id": image["Id"].removeprefix("sha256:"), "digest": digest}

    roles = expected["roles"]
    stacks = expected["stacks"]
    repo_names = expected["repositories"]
    repo_root = Path(git("rev-parse", "--show-toplevel").decode().strip())
    repos = {(stack, name): create_repo(repo_root, output / "fixtures", frozen, stack, name, name == "defect")
             for stack, name in itertools.product(stacks, [*repo_names, "defect"])}
    assert len({entry[1] for entry in repos.values()}) == len(repos)
    nonce = secrets.token_hex(4)
    cases = []
    for stack, name, role, generation in itertools.product(stacks, repo_names, roles, (1, 2)):
        for variant in (stack, "combined"):
            cases.append((f"{stack}-{name}-{role}-{generation}-{variant}", stack, name, role, variant, "normal"))
    for stack in stacks:
        for kind in ("defect", "absent-profile", "incompatible", "absent-policy"):
            variant = {"absent-policy": "combined",
                       "incompatible": "rust" if stack == "go" else "go"}.get(kind, stack)
            cases.append((f"{stack}-{kind}", stack, "defect" if kind == "defect" else "a",
                          "development", variant, kind))

    if args.case:
        if set(args.case) - {case[0] for case in cases}:
            parser.error("unknown case name")
        cases = [case for case in cases if case[0] in args.case]

    def run_case(case):
        label, stack, name, role, variant, kind = case
        peer = next(value for value in roles if value != role)
        uid = str(roles[role])
        fixture, head = repos[(stack, name)]
        root = output / label
        root.mkdir()
        checkout = root / "checkout"
        git("clone", "--local", "--no-hardlinks", "-q", str(fixture), str(checkout), cwd=root)
        for identity in roles:
            assets = root / identity
            assets.mkdir()
            for asset in ASSETS:
                (assets / asset).write_text(f"p22-synthetic:{identity}:{asset}\n")
        attached = kind in {"normal", "defect"}
        assert git("rev-parse", "HEAD", cwd=checkout).decode().strip() == head
        tag = f"p22-{nonce}-{label}"
        command = ["create", "--name", tag, "--hostname", tag, "--pull=never", "--network=none",
                   "--read-only", "--cap-drop=all", "--security-opt=no-new-privileges",
                   "--user", f"{uid}:{uid}", "--workdir", "/work",
                   "--tmpfs", "/tmp:rw,exec,nosuid,nodev,mode=1777,size=512m",
                   "--tmpfs", "/role-state:rw,noexec,nosuid,nodev,mode=1777,size=4m",
                   "-v", f"{checkout}:/work:ro,Z"]
        if attached:
            command += ["-v", f"{root / role}:/roles/{role}:ro,Z", "-v", f"{probe}:/p22-probe.py:ro,z",
                        "--entrypoint", "/usr/bin/python3"]
            launch = ["-I", "/p22-probe.py", role, peer, uid, stack, head, *pins]
        elif kind == "absent-profile":
            command += ["--entrypoint", "/usr/bin/python3"]
            launch = ["-I", VERIFY]
        else:
            launch = [stack]
        if kind == "absent-policy":
            empty_policy = root / "missing-policy"
            empty_policy.mkdir()
            command += ["-v", f"{empty_policy}:/opt/ieo-policy/source/{CATALOG}:ro,Z"]
        command += [f"sha256:{images[variant]['id']}", *launch]
        started = time.monotonic()
        stdout, stderr, detail = b"", b"", ""
        container = None
        try:
            container = podman(*command).stdout.decode().strip()
            assert re.fullmatch("[0-9a-f]{64}", container)
            result = podman("start", "--attach", container, timeout=180, check=False)
            stdout, stderr = result.stdout, result.stderr
            captured = json.loads(podman("container", "inspect", container).stdout)[0]
            assert captured["Image"].removeprefix("sha256:") == images[variant]["id"]
            assert captured["HostConfig"]["ReadonlyRootfs"]
            mounts = {m["Destination"] for m in captured["Mounts"] if m["Type"] == "bind"}
            assert "/work" in mounts
            if attached:
                assert f"/roles/{role}" in mounts and f"/roles/{peer}" not in mounts
            assert all(not m["RW"] for m in captured["Mounts"] if m["Type"] == "bind")
            assert result.returncode == captured["State"]["ExitCode"]
            pattern = {
                "normal": (0, f"PASS profile={stack}"),
                "defect": (1, f"FAIL required {'Go' if stack == 'go' else 'Rust'} test results"),
                "absent-profile": (2, "the following arguments are required: profile"),
                "incompatible": (2, f"{'Go' if stack == 'go' else 'Rust'} is absent from this image"),
                "absent-policy": (1, "INDEX.md: No such file or directory"),
            }[kind]
            actual = stdout + stderr
            assert result.returncode == pattern[0] and pattern[1].encode() in actual, f"expected {pattern}, exit {result.returncode}"
            assert b"P22_FAKE_TOOL_EXECUTED" not in actual, "an untrusted tool ran"
            observation_lines = [line for line in stdout.splitlines() if line.startswith(b"P22_ROLE ")]
            assert len(observation_lines) == int(attached), "role probe was not observed"
            if observation_lines:
                role_evidence = json.loads(observation_lines[0][len(b"P22_ROLE "):])
                assert role_evidence["uid"] == int(uid) and len(role_evidence["denials"]) == 10
                assert role_evidence["role"] == role and role_evidence["head"] == head
            assert git("status", "--porcelain", cwd=checkout) == b""
            verdict = "PASS"
        except (AssertionError, subprocess.SubprocessError, ValueError, KeyError) as exc:
            detail = str(exc)
            if isinstance(exc, subprocess.CalledProcessError):
                stderr += exc.stderr or b""
            verdict = "FAIL"
        finally:
            if container:
                removal = podman("rm", "--force", container, check=False)
                if removal.returncode:
                    stderr += removal.stderr
                    verdict, detail = "FAIL", f"{detail} container cleanup failed".strip()
        (output / "logs" / f"{label}.stdout").write_bytes(stdout)
        (output / "logs" / f"{label}.stderr").write_bytes(stderr)
        return {"id": label, "verdict": verdict, "reason": detail, "role": role, "kind": kind,
                "source": head, "image": images[variant]["digest"], "container": container,
                "elapsed_seconds": round(time.monotonic() - started, 3),
                "create": shlex.join([*prefix, *command])}

    with ThreadPoolExecutor(max_workers=4) as pool:
        reports = list(pool.map(run_case, cases))
    seen = [r["container"] for r in reports if r["container"]]
    assert len(seen) == len(set(seen)), "container was reused"
    info = {"approval": approval, "pins": dict(zip(pin_paths, pins)), "images": images, "cases": reports,
            "model_requests": 0, "additional_usd": 0}
    (output / "results.json").write_text(json.dumps(info, indent=2) + "\n")
    for row in reports:
        print(row["verdict"], row["id"], row["reason"])
    passed = sum(r["verdict"] == "PASS" for r in reports)
    print(f"P22-B comparisons {passed}/{len(reports)}")
    return int(passed != len(reports))


if __name__ == "__main__":
    raise SystemExit(main())
