#!/usr/bin/env python3
"""01-06 test lab: inventory, evidence-preserving archive, reset and re-initialisation of the PoC lab (no model calls).

Scope guard: only paths under /opt/rust/ieo_poc_probe and podman resources named ieo-probe-* are touched.
Credentials (/opt/rust/ieo_poc_secrets, role cred/ copies, e-secrets) are never archived or printed.

Usage (.venv-poc/bin/python -I probes/lab/lab.py ...):
  inventory [--out FILE]            current lab state (names, modes, owners, counts, digests of names; no contents)
  archive --dest DIR                tar of lab state except credentials, with sha256 and a name manifest
  reset --yes --archived MANIFEST   remove ieo-probe-* containers/networks and lab data dirs (keeps bin/, e-secrets/);
                                    needs the MANIFEST of the archive just taken (or --no-archive-needed)
  check-init --evidence DIR         judge E/E1b evidence of an init against the expected markers
  init --evidence DIR               probe.py setup + probe2.py e + e1b (no model calls) -> initial state
  verify --inventory FILE           check an inventory against the documented initial state
"""
import argparse, hashlib, json, os, re, shlex, subprocess, sys
from datetime import datetime, timezone
from pathlib import Path

LAB = Path("/opt/rust/ieo_poc_probe")
SECRETS = Path("/opt/rust/ieo_poc_secrets")
PREFIX = "ieo-probe-"
REPO = Path(__file__).resolve().parents[2]
PY = sys.executable
DATA_DIRS = ["roles", "roles-recreated", "testrepos", "snapshots", "snapshots-g", "snapshots-galt", "forge", "rx-data", "g2-src", "g3-cfg", "seed-a", "seed-b"]
KEEP = ["bin", "e-secrets"]
NEVER_ARCHIVE = ["*/cred", "*/cred/*", "e-secrets", "e-secrets/*", "*.credentials.json", "*credentials*"]


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
    # resolve once (PR #19 automated review P2): a relative --dest must mean the same place for mkdir, tar and hashing
    dest = Path(a.dest).expanduser().resolve()
    if dest == LAB or LAB in dest.parents:
        sys.exit("archive destination must be outside the lab")
    if dest == REPO or REPO in dest.parents:
        sys.exit("archive destination must be outside the repository (it holds role session records)")
    dest.mkdir(parents=True, exist_ok=True)
    os.chmod(dest, 0o700)
    tar = dest / "lab-state.tar.gz"
    present = [d for d in DATA_DIRS if (LAB / d).exists()]
    excl = " ".join("--exclude=" + shlex.quote(x) for x in NEVER_ARCHIVE)
    q = shlex.quote(str(tar))
    unshare(f"cd {shlex.quote(str(LAB))} && tar --numeric-owner {excl} -czf {q} {' '.join(present)} && chmod 0600 {q}")
    listing = unshare(f"tar -tzvf {q} --numeric-owner").stdout
    if any(seg in line for line in listing.splitlines() for seg in ("/cred/", "e-secrets", "credentials")):
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
    # evidence first: reset needs the MANIFEST of an archive taken just before, unless explicitly waived
    if not a.no_archive_needed:
        if not a.archived:
            sys.exit("reset needs --archived <MANIFEST.json> (or --no-archive-needed when the state is regenerable)")
        m = json.loads(Path(a.archived).read_text())
        tar = Path(m["archive"])
        if not tar.exists() or hashlib.sha256(tar.read_bytes()).hexdigest() != m["sha256"]:
            sys.exit("archive in MANIFEST missing or sha256 mismatch")
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


def child_env():
    """Pin the probes to the guarded lab (PR #19 automated review P1): probe.py/probe2.py read IEO_PROBE_BASE /
    IEO_PROBE_SECRETS; an inherited override must never move setup/E/E1b outside /opt/rust/ieo_poc_probe."""
    env = dict(os.environ)
    want = {"IEO_PROBE_BASE": str(LAB / "roles"), "IEO_PROBE_SECRETS": str(SECRETS)}
    for k, v in want.items():
        if env.get(k) not in (None, v):
            sys.exit(f"SCOPE GUARD: inherited {k} differs from the lab ({env[k]!r}); unset it")
        env[k] = v
    return env


# expected decisive markers per E/E1b test (the 01-04 expectations); "absent" markers must not appear
EXPECT_TESTS = {
    "E1-dev-push-work": (["PUSH_WORK_OK"], []), "E1-dev-push-main": (["PUSH_MAIN_REJECTED"], ["PUSH_MAIN_OK"]),
    "E1-gd-read-snapshot": (["READ_OK"], []), "E1-gd-write-snapshot": (["WRITE_DENIED", "REPO_ABSENT"], ["WRITE_OK", "REPO_VISIBLE"]),
    "E2-coord-insert": (["COORD_OK"], []), "E2-reader-select": (["READER_SELECT_OK"], []),
    "E2-reader-insert": (["READER_INSERT_DENIED"], ["READER_INSERT_OK"]), "E2-no-password": (["NOPW_DENIED"], ["NOPW_OK"]),
    "E2-development-reach": (["REACH_DENIED"], ["REACH_OK"]), "E2-guardrail-reach": (["REACH_DENIED"], ["REACH_OK"]),
    "E3-post-new": (["^201$"], []), "E3-post-replay": (["^200$", '"replay":true'], []), "E3-post-conflict": (["^409$"], []),
    "E3-get": (["HTTP/1.1 200", "X-Stored-Count: 1"], []), "E3-no-token": (["^401$"], []), "E3-other-token": (["^401$"], []),
    "E1b-dev-push-work": (["PUSH_WORK_OK"], []), "E1b-dev-push-main": (["PUSH_MAIN_REJECTED"], ["PUSH_MAIN_OK"]),
    "E1b-dev-force-main": (["FORCE_MAIN_REJECTED"], ["FORCE_MAIN_OK"]), "E1b-dev-delete-main": (["DELETE_MAIN_REJECTED"], ["DELETE_MAIN_OK"]),
    "E1b-dev-repo-files": (["ABSENT /srv/a.git$", "ABSENT /srv/a.git/hooks/pre-receive"], ["VISIBLE /srv/a.git"]),
    "E1b-gd-reach": (["REACH_DENIED"], ["REACH_OK"]), "E1b-main-unchanged": ([], []),
}


def check_init(ev: Path):
    """Judge the generated E/E1b evidence against EXPECT_TESTS (PR #19 automated review P2): init succeeds only if
    every test shows its expected markers, none of the forbidden ones, and the DB schema init returned 0."""
    res = []
    e = json.loads((ev / "E.json").read_text()); e1b = json.loads((ev / "E1b.json").read_text())
    res.append({"id": "E2 schema init rc", "ok": e["info"].get("pg_init_rc") == 0, "got": e["info"].get("pg_init_rc")})
    tests = {t["id"]: t for t in e["tests"] + e1b["tests"]}
    for tid, (need, forbid) in EXPECT_TESTS.items():
        t = tests.get(tid)
        if t is None:
            res.append({"id": tid, "ok": False, "got": "missing"}); continue
        out = t.get("output", "")
        if tid == "E1b-main-unchanged":
            need = [r"refs/heads/main " + re.escape(e1b["seed_main"])]
        miss = [m for m in need if not re.search(m, out, re.M)]
        bad = [m for m in forbid if re.search(m, out, re.M)]
        res.append({"id": tid, "ok": not miss and not bad, "missing": miss, "forbidden_seen": bad})
    extra = sorted(set(tests) - set(EXPECT_TESTS))
    res.append({"id": "no unexpected tests", "ok": not extra, "got": extra})
    return {"all_ok": all(r["ok"] for r in res), "checks": res}


def cmd_check_init(a):
    out = check_init(Path(a.evidence).resolve())
    (Path(a.evidence) / "init-check.json").write_text(json.dumps(out, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({"all_ok": out["all_ok"], "failed": [r["id"] for r in out["checks"] if not r["ok"]]}))
    sys.exit(0 if out["all_ok"] else 1)


def cmd_init(a):
    ev = Path(a.evidence).resolve()
    ev.mkdir(parents=True, exist_ok=True)
    py = [PY, "-I"]
    env = child_env()
    steps = [("setup", py + [str(REPO / "probes/01-04/probe.py"), "setup", "--evidence", str(ev)]),
             ("e", py + [str(REPO / "probes/01-04/probe2.py"), "e", "--evidence", str(ev)]),
             ("e1b", py + [str(REPO / "probes/01-04/probe2.py"), "e1b", "--evidence", str(ev)])]
    log = []
    for name, cmd in steps:
        r = sh(cmd, check=False, env=env)
        log.append({"step": name, "rc": r.returncode, "tail": (r.stdout + r.stderr)[-1500:]})
        if r.returncode:
            (ev / "init-log.json").write_text(json.dumps(log, ensure_ascii=False, indent=2) + "\n")
            sys.exit(f"init step {name} failed")
    for c in ("ieo-probe-pg", "ieo-probe-rx"):  # leave the lab at rest: services stopped, data kept
        sh(["podman", "stop", "-t", "5", c], check=False)
    (ev / "init-log.json").write_text(json.dumps(log, ensure_ascii=False, indent=2) + "\n")
    chk = check_init(ev)
    (ev / "init-check.json").write_text(json.dumps(chk, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({"steps": [{k: x[k] for k in ("step", "rc")} for x in log], "behaviour_ok": chk["all_ok"]}))
    if not chk["all_ok"]:
        sys.exit("init: E/E1b behaviour does not match the expected initial state -> not initialised")


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
    out = {"at": now(), "inventory": a.inventory, "checks": res, "all_ok": all(x["ok"] for x in res)}
    print(json.dumps(out, ensure_ascii=False, indent=2))
    sys.exit(0 if out["all_ok"] else 1)


def main():
    ap = argparse.ArgumentParser()
    sp = ap.add_subparsers(dest="cmd", required=True)
    p = sp.add_parser("inventory"); p.add_argument("--out")
    p = sp.add_parser("archive"); p.add_argument("--dest", required=True)
    p = sp.add_parser("reset"); p.add_argument("--yes", action="store_true"); p.add_argument("--archived"); p.add_argument("--no-archive-needed", action="store_true")
    p = sp.add_parser("check-init"); p.add_argument("--evidence", required=True)
    p = sp.add_parser("init"); p.add_argument("--evidence", required=True)
    p = sp.add_parser("verify"); p.add_argument("--inventory", required=True)
    a = ap.parse_args()
    {"inventory": cmd_inventory, "archive": cmd_archive, "reset": cmd_reset, "init": cmd_init, "check-init": cmd_check_init, "verify": cmd_verify}[a.cmd](a)


if __name__ == "__main__":
    main()
