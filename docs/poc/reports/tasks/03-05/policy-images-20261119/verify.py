#!/usr/bin/python3 -I
"""Run the T04 fixture profiles and preserve the command exit and original output."""

import argparse
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

POLICY = Path("/opt/ieo-policy")
PATH = "/opt/ieo-tools/go/bin:/opt/ieo-tools/rust/bin:/usr/bin:/bin"


def run(argv, env, *, capture=False):
    print(f"+ {shlex.join(argv)}", file=sys.stderr, flush=True)
    if not capture:
        subprocess.run(argv, env=env, check=True)
        return ""
    with tempfile.TemporaryFile() as stdout:
        result = subprocess.run(argv, env=env, stdout=stdout, check=False)
        stdout.seek(0)
        shutil.copyfileobj(stdout, sys.stdout.buffer)
        sys.stdout.buffer.flush()
        result.check_returncode()
        stdout.seek(0)
        return stdout.read().decode("utf-8")


class Rejected(Exception):
    """A required check produced evidence of a violation."""


def require_pass(label, statuses, passed):
    detail = f"required {label} test results: {statuses}"
    if any(outcome != passed for results in statuses.values() for outcome in results):
        raise Rejected(detail)
    if any(results != [passed] for results in statuses.values()):
        raise ValueError(detail)


def go_tests(output, required, package):
    events = [json.loads(line) for line in output.splitlines() if line]
    statuses = {name: [] for name in required}
    for event in events:
        name = event.get("Test")
        if event.get("Package") == package and name in statuses and event.get("Action") in {"pass", "skip", "fail"}:
            statuses[name].append(event["Action"])
    require_pass("Go", statuses, "pass")


def rust_tests(output, required):
    statuses = {name: [] for name in required}
    for name, outcome in re.findall(r"^test (\S+) \.\.\. (ok|ignored|FAILED)(?:,.*)?$", output, re.M):
        if name in statuses:
            statuses[name].append(outcome)
    require_pass("Rust", statuses, "ok")


def verify(stack, profiles, scratch, env):
    profile = profiles[stack]
    required = profile["required_tests"]
    if not required:
        raise ValueError("profile has no required test")
    if stack == "go":
        if not Path("/opt/ieo-tools/go/bin/go").is_file():
            raise ValueError("Go is absent from this image")
        env.update(GOTOOLCHAIN="local", GOENV="off", GOWORK="off", GOPROXY="off",
                   GOSUMDB="off", CGO_ENABLED="1", GOCACHE=f"{scratch}/go-cache",
                   GOMODCACHE=f"{scratch}/go-mod-cache", GOPATH=f"{scratch}/go-path",
                   GOMAXPROCS="2")
        if run(["gofmt", "-l", "."], env, capture=True):
            raise Rejected("Go formatting differences")
        run(["go", "vet", "-mod=readonly", "./..."], env)
        output = run(["go", "test", "-mod=readonly", "-count=1", "-json", "./..."], env, capture=True)
        go_tests(output, required, profile["package"])
    else:
        if not Path("/opt/ieo-tools/rust/bin/cargo").is_file():
            raise ValueError("Rust is absent from this image")
        env.update(CARGO_HOME=f"{scratch}/cargo-home", CARGO_TARGET_DIR=f"{scratch}/target",
                   CARGO_NET_OFFLINE="true", CARGO_BUILD_JOBS="2", CARGO_TERM_COLOR="never")
        run(["cargo", "fmt", "--all", "--", "--check"], env)
        run(["cargo", "clippy", "--offline", "--locked", "--all-targets", "--", "-D", "warnings"], env)
        output = run(["cargo", "test", "--offline", "--locked", "--all-targets"], env, capture=True)
        rust_tests(output, required)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("profile", choices=("info", "go", "rust"))
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(dir="/tmp", prefix="ieo-policy-") as scratch:
        env = {"PATH": PATH, "LC_ALL": "C", "HOME": scratch, "TMPDIR": scratch}
        subprocess.run(["sha256sum", "--quiet", "-c", str(POLICY / "bundle.sha256")],
                       cwd=POLICY / "source", env=env, check=True)
        profile_path = POLICY / "profiles.json"
        profiles = json.loads(profile_path.read_text())
        if args.profile == "info":
            print(profile_path.read_text(), end="", flush=True)
            for command in (["git", "--version"], ["gcc", "-dumpfullversion"], ["python3", "-V"]):
                run(command, env)
            if Path("/opt/ieo-tools/go/bin/go").is_file():
                run(["go", "version"], {**env, "GOTOOLCHAIN": "local", "GOENV": "off"})
            if Path("/opt/ieo-tools/rust/bin/rustc").is_file():
                for tool in ("rustc", "cargo", "rustfmt", "clippy-driver"):
                    run([tool, "-V"], env)
        else:
            verify(args.profile, profiles["profiles"], scratch, env)
            print(f"PASS profile={args.profile} version={profiles['version']} catalog={profiles['catalog']}")


if __name__ == "__main__":
    try:
        main()
    except Rejected as exc:
        print(f"FAIL {exc}", file=sys.stderr)
        sys.exit(1)
    except subprocess.CalledProcessError as exc:
        print(f"FAIL command exit={exc.returncode}", file=sys.stderr)
        sys.exit(exc.returncode if exc.returncode > 0 else 128 - exc.returncode)
    except (OSError, ValueError, KeyError) as exc:
        print(f"INCONCLUSIVE {exc}", file=sys.stderr)
        sys.exit(2)
