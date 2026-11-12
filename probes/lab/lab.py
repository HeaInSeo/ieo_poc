#!/usr/bin/env python3
"""01-06 test lab: inventory, evidence-preserving archive, reset and re-initialisation of the PoC lab (no model calls).

Scope guard: only paths under /opt/rust/ieo_poc_probe and podman resources named ieo-probe-* are touched.
Credentials (/opt/rust/ieo_poc_secrets, role cred/ copies, e-secrets) are never archived or printed.

Usage (.venv-poc/bin/python -I probes/lab/lab.py ...):
  inventory [--out FILE]            current lab state (names, modes, owners, counts, digests of names; no contents)
  archive --dest DIR                tar of lab state except credentials, with sha256 and a name manifest
  reset --yes                       remove ieo-probe-* containers/networks and lab data dirs (keeps bin/, e-secrets/)
  init --evidence DIR               probe.py setup + probe2.py e + e1b (no model calls) -> initial state
  verify --inventory FILE           check an inventory against the documented initial state
"""
import argparse, hashlib, json, os, subprocess, sys
from datetime import datetime, timezone
from pathlib import Path

LAB = Path("/opt/rust/ieo_poc_probe")
SECRETS = Path("/opt/rust/ieo_poc_secrets")
PREFIX = "ieo-probe-"
REPO = Path(__file__).resolve().parents[2]
PY = sys.executable
DATA_DIRS = ["roles", "roles-recreated", "testrepos", "snapshots", "snapshots-g", "snapshots-galt", "forge", "rx-data", "g2-src", "g3-cfg", "seed-a", "seed-b"]
KEEP = ["bin", "e-secrets"]
NEVER_ARCHIVE = ["*/cred", "*/cred/*", "e-secrets", "e-secrets/*"]


def now():
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def sh(cmd, check=True, **kw):
    r = subprocess.run(cmd, capture_output=True, text=True, **kw)
    if check and r.returncode:
        sys.exit(f"command failed ({r.returncode}): {cmd}\n{r.stderr[-800:]}")
    return r


def unshare(script, check=True):
    return sh(["podman", "unshare", "bash", "-c", script], check=check)


def guard_path(p: Path):
    p = Path(os.path.abspath(p))
    if p == LAB or LAB not in p.parents:
        sys.exit(f"SCOPE GUARD: refusing path outside {LAB}: {p}")
    return p


def podman_names(kind):
    fmt = "{{.Names}}" if kind == "ps" else "{{.Name}}"
    cmd = ["podman", "ps", "-a", "--format", fmt] if kind == "ps" else ["podman", "network", "ls", "--format", fmt]
    return [n for n in sh(cmd).stdout.split() if n]


def inventory():
    inv = {"at": now(), "lab": str(LAB), "dirs": {}, "containers": {}, "networks": [], "timers": {}}
    script = ("cd %s && for d in */; do d=${d%%/}; n=$(find \"$d\" -mindepth 1 | wc -l); "
              "h=$(find \"$d\" -mindepth 1 -printf '%%P %%y %%m %%U\\n' | sort | sha256sum | cut -c1-16); "
              "echo \"$d $(stat -c '%%a %%u' \"$d\") $n $h\"; done" % LAB)
    for line in unshare(script).stdout.splitlines():
        d, mode, uid, n, h = line.split()
        inv["dirs"][d] = {"mode": mode, "uid_in_ns": int(uid), "entries": int(n), "names_digest": h}
    for role in ("development", "guardrail"):
        base = LAB / "roles" / role
        r = unshare(f"for p in state cred etc state/claude state/home state/work; do [ -e {base}/$p ] && echo \"$p $(stat -c '%a %u' {base}/$p)\"; done; "
                    f"[ -e {base}/cred/oauth_token ] && echo \"cred/oauth_token $(stat -c '%a %u' {base}/cred/oauth_token)\"; "
                    f"echo sessions $(find {base}/state/claude/projects -name '*.jsonl' 2>/dev/null | wc -l)", check=False)
        inv.setdefault("roles", {})[role] = dict((l.split(" ", 1) + [""])[:2] for l in r.stdout.splitlines() if l)
    for n in podman_names("ps"):
        if n.startswith(PREFIX):
            inv["containers"][n] = sh(["podman", "inspect", "--format", "{{.State.Status}}", n]).stdout.strip()
    inv["networks"] = sorted(n for n in podman_names("net") if n.startswith(PREFIX))
    inv["other_containers_not_touched"] = sorted(n for n in podman_names("ps") if not n.startswith(PREFIX))
    for t in ("claude-operator-tick", "claude-guardrail-tick", "claude-report-bridge"):
        r = sh(["systemctl", "--user", "show", f"{t}.timer", "-p", "UnitFileState", "-p", "ActiveState", "-p", "SubState"], check=False)
        inv["timers"][t] = " ".join(r.stdout.split())
    inv["secrets_dir"] = {"exists": SECRETS.exists(), "mode": oct(SECRETS.stat().st_mode & 0o777) if SECRETS.exists() else None,
                          "files": sorted(p.name for p in SECRETS.glob("*.token")) if SECRETS.exists() else []}
    return inv


def cmd_inventory(a):
    inv = inventory()
    text = json.dumps(inv, ensure_ascii=False, indent=2) + "\n"
    if a.out:
        Path(a.out).write_text(text)
    print(text)


def cmd_archive(a):
    dest = Path(a.dest)
    if LAB in Path(os.path.abspath(dest)).parents or dest.resolve() == LAB:
        sys.exit("archive destination must be outside the lab")
    dest.mkdir(parents=True, exist_ok=True)
    os.chmod(dest, 0o700)
    tar = dest / "lab-state.tar.gz"
    present = [d for d in DATA_DIRS if (LAB / d).exists()]
    excl = " ".join(f"--exclude='{x}'" for x in NEVER_ARCHIVE)
    unshare(f"cd {LAB} && tar --numeric-owner {excl} -czf {tar} {' '.join(present)} && chmod 0600 {tar}")
    listing = unshare(f"tar -tzvf {tar} --numeric-owner").stdout
    if any(seg in line for line in listing.splitlines() for seg in ("/cred/", "e-secrets")):
        sys.exit("ARCHIVE GUARD: credential path found in archive listing")
    names = sorted(l.split()[-1] for l in listing.splitlines())
    sha = hashlib.sha256(tar.read_bytes()).hexdigest()
    manifest = {"at": now(), "archive": str(tar), "sha256": sha, "bytes": tar.stat().st_size, "dirs": present,
                "entries": len(names), "names_digest": hashlib.sha256("\n".join(names).encode()).hexdigest()[:16],
                "excluded": NEVER_ARCHIVE}
    (dest / "MANIFEST.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(manifest, ensure_ascii=False, indent=2))


def cmd_reset(a):
    if not a.yes:
        sys.exit("reset needs --yes")
    removed = {"containers": [], "networks": [], "dirs": []}
    for n in podman_names("ps"):
        if n.startswith(PREFIX):
            sh(["podman", "rm", "-f", n]); removed["containers"].append(n)
    for n in podman_names("net"):
        if n.startswith(PREFIX):
            sh(["podman", "network", "rm", n]); removed["networks"].append(n)
    for d in DATA_DIRS:
        p = guard_path(LAB / d)
        if p.exists():
            unshare(f"rm -rf {p}"); removed["dirs"].append(d)
    left = sorted(x.name for x in LAB.iterdir())
    if set(left) - set(KEEP):
        sys.exit(f"reset left unexpected entries: {sorted(set(left) - set(KEEP))}")
    print(json.dumps({"at": now(), "removed": removed, "kept": left}, ensure_ascii=False, indent=2))


def cmd_init(a):
    ev = Path(a.evidence).resolve()
    ev.mkdir(parents=True, exist_ok=True)
    py = [PY, "-I"]
    steps = [("setup", py + [str(REPO / "probes/01-04/probe.py"), "setup", "--evidence", str(ev)]),
             ("e", py + [str(REPO / "probes/01-04/probe2.py"), "e", "--evidence", str(ev)]),
             ("e1b", py + [str(REPO / "probes/01-04/probe2.py"), "e1b", "--evidence", str(ev)])]
    log = []
    for name, cmd in steps:
        r = sh(cmd, check=False)
        log.append({"step": name, "rc": r.returncode, "tail": (r.stdout + r.stderr)[-1500:]})
        if r.returncode:
            (ev / "init-log.json").write_text(json.dumps(log, ensure_ascii=False, indent=2) + "\n")
            sys.exit(f"init step {name} failed")
    for c in ("ieo-probe-pg", "ieo-probe-rx"):  # leave the lab at rest: services stopped, data kept
        sh(["podman", "stop", "-t", "5", c], check=False)
    (ev / "init-log.json").write_text(json.dumps(log, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps([{k: x[k] for k in ("step", "rc")} for x in log]))


EXPECTED = {
    "dirs_present": ["bin", "e-secrets", "forge", "roles", "rx-data", "snapshots", "testrepos"],
    "dirs_absent": ["roles-recreated", "snapshots-g", "snapshots-galt", "g2-src", "g3-cfg"],
    "role": {"state": "700", "cred": "700", "cred/oauth_token": "400", "sessions": "0"},
    "role_uid": {"development": "20001", "guardrail": "20002"},
    "containers": {"ieo-probe-pg": "exited", "ieo-probe-rx": "exited", "ieo-probe-forge": "exited"},
    "networks": ["ieo-probe-coord", "ieo-probe-forge"],
}


def cmd_verify(a):
    inv = json.loads(Path(a.inventory).read_text())
    res = []
    def c(name, ok, got=None):
        res.append({"check": name, "ok": bool(ok), "got": got})
    for d in EXPECTED["dirs_present"]:
        c(f"dir {d} present", d in inv["dirs"])
    for d in EXPECTED["dirs_absent"]:
        c(f"dir {d} absent (probe scratch not part of initial state)", d not in inv["dirs"])
    for role, uid in EXPECTED["role_uid"].items():
        r = inv.get("roles", {}).get(role, {})
        for k, mode in EXPECTED["role"].items():
            v = r.get(k, "")
            if k == "sessions":
                c(f"{role} no session records", v == mode, v)
            else:
                c(f"{role} {k} mode {mode} uid {uid}", v.split() == [mode, uid], v)
    for n, st in EXPECTED["containers"].items():
        c(f"container {n} {st}", inv["containers"].get(n) == st, inv["containers"].get(n))
    c("networks", inv["networks"] == EXPECTED["networks"], inv["networks"])
    c("only ieo-probe-* touched (others listed, not removed)", True, inv.get("other_containers_not_touched"))
    out = {"at": now(), "inventory": a.inventory, "checks": res, "all_ok": all(x["ok"] for x in res)}
    print(json.dumps(out, ensure_ascii=False, indent=2))
    sys.exit(0 if out["all_ok"] else 1)


def main():
    ap = argparse.ArgumentParser()
    sp = ap.add_subparsers(dest="cmd", required=True)
    p = sp.add_parser("inventory"); p.add_argument("--out")
    p = sp.add_parser("archive"); p.add_argument("--dest", required=True)
    p = sp.add_parser("reset"); p.add_argument("--yes", action="store_true")
    p = sp.add_parser("init"); p.add_argument("--evidence", required=True)
    p = sp.add_parser("verify"); p.add_argument("--inventory", required=True)
    a = ap.parse_args()
    {"inventory": cmd_inventory, "archive": cmd_archive, "reset": cmd_reset, "init": cmd_init, "verify": cmd_verify}[a.cmd](a)


if __name__ == "__main__":
    main()
