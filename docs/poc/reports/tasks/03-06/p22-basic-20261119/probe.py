"""Check a single synthetic role in the approved image and run its verifier."""
import errno
import hashlib
import json
import os
import socket
import subprocess
import sys
from pathlib import Path

CATALOG = Path("/opt/ieo-policy/source/docs/poc/reports/tasks/03-04/p25-20261119")
VERIFY = Path("/usr/local/bin/ieo-policy-verify")
PROFILE = Path("/opt/ieo-policy/profiles.json")
ASSETS = ("workspace", "session", "credential", "approval")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def denied(path, action):
    try:
        if action == "read":
            path.read_bytes()
        else:
            path.write_bytes(b"P22 synthetic write probe")
    except OSError as exc:
        if exc.errno not in {errno.ENOENT, errno.EACCES, errno.EPERM, errno.EROFS}:
            raise
        return errno.errorcode[exc.errno]
    raise RuntimeError(f"unexpected {action} allowed: {path}")


def main():
    role, peer, uid, stack, head, policy_digest, profile_digest, runner_digest = sys.argv[1:]
    assert (os.getuid(), os.getgid()) == (int(uid), int(uid)), "wrong role identity"
    git = ["/usr/bin/git", "-c", "safe.directory=/work"]
    actual = subprocess.check_output([*git, "rev-parse", "HEAD"], cwd="/work", text=True).strip()
    assert actual == head
    assert subprocess.check_output([*git, "status", "--porcelain"], cwd="/work", text=True) == ""
    assert digest(CATALOG / "INDEX.md") == policy_digest
    assert digest(PROFILE) == profile_digest and digest(VERIFY) == runner_digest
    catalog = json.loads((CATALOG / "catalog-use.json").read_text())
    assert catalog["version"] == "0.2.2"
    detail = f"{stack.upper()}_POLICY.md"
    assert detail in (CATALOG / "INDEX.md").read_text() and (CATALOG / detail).is_file()
    profile = json.loads(PROFILE.read_text())
    assert profile["version"] == "0.1.0" and profile["profiles"][stack]["required_tests"]
    assert {name for _, name in socket.if_nameindex()} <= {"lo"}, "unexpected interface"
    status = dict(line.split(":", 1) for line in Path("/proc/self/status").read_text().splitlines() if ":" in line)
    assert int(status["CapEff"].strip(), 16) == 0
    assert status["NoNewPrivs"].strip() == "1"

    for asset in ASSETS:
        own = Path("/roles") / role / asset
        assert own.read_text() == f"p22-synthetic:{role}:{asset}\n", "own fixture inaccessible"
    rejects = {f"peer_{asset}_{action}": denied(Path("/roles") / peer / asset, action)
               for asset in ASSETS for action in ("read", "write")}
    rejects["policy_write"] = denied(PROFILE, "write")
    rejects["source_write"] = denied(Path("/work") / ".p22-write", "write")
    marker = Path("/role-state/attempt")
    assert not marker.exists(), "new container contains a previous state marker"
    marker.write_text(role)
    assert marker.read_text() == role

    print("P22_ROLE " + json.dumps({"uid": int(uid), "head": head, "role": role, "own_state": True,
                                    "catalog": catalog["version"], "profile": profile["version"],
                                    "network": "loopback-only", "denials": rejects}), flush=True)
    fake = Path("/tmp/p22-fake-tools")
    fake.mkdir()
    for name in ("go", "gofmt", "cargo", "rustc"):
        tool = fake / name
        tool.write_text("#!/bin/sh\necho P22_FAKE_TOOL_EXECUTED >&2\nexit 97\n")
        tool.chmod(0o755)
    env = {**os.environ, "PATH": f"{fake}:/usr/bin:/bin", "RUSTC": str(fake / "rustc"),
           "GOFLAGS": "-run=^NoP22Tests$"}
    return subprocess.run([str(VERIFY), stack], env=env, check=False).returncode


if __name__ == "__main__":
    sys.exit(main())
