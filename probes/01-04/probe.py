#!/usr/bin/env python3
"""01-04 first probes (P1 concurrency, P2 isolation, P3 quota structure).

Plan: docs/poc/reports/tasks/01-04/first-probes-20261112/PROBE_PLAN.md
Budget: 30 model-request attempts, 7200 s wall clock (from first attempt), 300 s per attempt.
Token values are never printed or written by this script; only SHA-256 prefixes.

Usage: python3 -I probes/01-04/probe.py {setup|p1|p2|p3|status} [--evidence DIR]
"""
import argparse, hashlib, json, os, re, subprocess, sys, time, uuid
from datetime import datetime, timezone
from pathlib import Path

ROLES = {"development": 20001, "guardrail": 20002}
# overridable only for the harness dry run with dummy tokens (no model calls); real runs use the defaults
SECRETS = Path(os.environ.get("IEO_PROBE_SECRETS", "/opt/rust/ieo_poc_secrets"))
BASE = Path(os.environ.get("IEO_PROBE_BASE", "/opt/rust/ieo_poc_probe/roles"))  # role volumes (owned by mapped sub-UIDs)
CLAUDE_BIN = Path("/opt/rust/ieo_poc_probe/bin/claude-2.1.322")  # 0755 copy of ~/.local/share/claude/versions/2.1.322 (same SHA-256)
IMAGE = "docker.io/library/golang:1.26.9-bookworm"
MAX_ATTEMPTS, WALL, PER_ATTEMPT = 30, 7200, 300
TOKEN_RE = re.compile(r"sk-ant-[A-Za-z0-9_\-]{8,}")
DEFAULT_EVIDENCE = Path(__file__).resolve().parents[2] / "docs/poc/reports/tasks/01-04/first-probes-20261112/evidence"


def now():
    return datetime.now(timezone.utc).isoformat(timespec="milliseconds")


def fp(b: bytes) -> str:
    return hashlib.sha256(b).hexdigest()[:12]


def run(cmd, **kw):
    return subprocess.run(cmd, capture_output=True, text=True, **kw)


class Ledger:
    def __init__(self, ev: Path):
        self.ev = ev
        self.path = ev / "budget.json"
        self.state = json.loads(self.path.read_text()) if self.path.exists() else {"attempts": [], "first_attempt_at": None, "cli_runs": []}
        self.tokens = {r: (SECRETS / f"{r}.token").read_bytes().strip() for r in ROLES if (SECRETS / f"{r}.token").exists()}

    def save(self):
        self.path.write_text(json.dumps(self.state, ensure_ascii=False, indent=2) + "\n")

    def remaining(self):
        n = MAX_ATTEMPTS - len(self.state["attempts"])
        if self.state["first_attempt_at"] is None:
            return n, WALL
        return n, WALL - (time.time() - self.state["first_attempt_at"])

    def reserve(self, k: int, label: str):
        n, w = self.remaining()
        if n < k or w < PER_ATTEMPT:
            sys.exit(f"BUDGET: need {k} attempts and {PER_ATTEMPT}s; have {n} attempts, {int(w)}s ({label}) -> stop, record unobservable")
        if self.state["first_attempt_at"] is None:
            self.state["first_attempt_at"] = time.time()
        ids = []
        for _ in range(k):
            ids.append(len(self.state["attempts"]) + 1)
            self.state["attempts"].append({"n": ids[-1], "label": label, "reserved_at": now()})
        self.save()
        return ids

    def scrub(self, text: str) -> str:
        """Refuse to persist anything containing a token; mask token-shaped strings."""
        for r, t in self.tokens.items():
            if t and t.decode(errors="ignore") in text:
                raise SystemExit(f"SECRET GUARD: {r} token value found in output; nothing written")
        # full SHA-256 of a token is not recorded either (PROBE_PLAN §0: 12-char prefix only; review of PR #12 P2-1)
        for t in self.tokens.values():
            h = hashlib.sha256(t).hexdigest()
            text = text.replace(h, h[:12] + "[sha256-truncated]")
        return TOKEN_RE.sub("sk-ant-[MASKED]", text)


def role_dir(role):
    return BASE / role


def podman_role_cmd(role, inner: str, extra_mounts=(), name=None):
    uid = ROLES[role]
    d = role_dir(role)
    cmd = ["podman", "run", "--rm", "--user", f"{uid}:{uid}", "--network", "slirp4netns",
           "--security-opt", "no-new-privileges", "--cap-drop", "ALL",
           # ":z" = shared SELinux label, so that SELinux never decides role-to-role access;
           # only UID ownership and modes do (P2 must not mistake an SELinux denial for UID isolation)
           "-v", f"{d}/state:/state:z", "-v", f"{d}/cred:/run/ieo/cred:ro,z",
           "-v", f"{CLAUDE_BIN}:/opt/claude/claude:ro,z",
           "-v", f"{d}/etc:/etc/ieo/role:ro,z",
           "--env", "HOME=/state/home", "--env", "CLAUDE_CONFIG_DIR=/state/claude", "--env", "LANG=C.UTF-8",
           "--workdir", "/state/work"]
    for m in extra_mounts:
        cmd += ["-v", m]
    if name:
        cmd += ["--name", name]
    return cmd + [IMAGE, "bash", "-c", inner]


def claude_inner(session_id: str, prompt_role: str, extra=""):
    # token read inside the container; env names recorded, values never printed
    return ("set -u; export CLAUDE_CODE_OAUTH_TOKEN=\"$(cat /run/ieo/cred/oauth_token)\"; "
            "echo \"#ENV_NAMES $(env | cut -d= -f1 | sort | tr '\\n' ' ')\" >&2; "
            "echo \"#UID $(id -u) GID $(id -g)\" >&2; "
            "echo \"#T_START $(date +%s.%N)\" >&2; "
            f"timeout -k 10 290 /opt/claude/claude -p 'Reply with exactly: ok-{prompt_role}' "
            "--output-format stream-json --verbose "
            f"--session-id {session_id} --tools '' --strict-mcp-config --mcp-config /etc/ieo/role/mcp.json "
            "--forward-subagent-text --setting-sources user --permission-mode dontAsk " + extra +
            "; rc=$?; echo \"#T_END $(date +%s.%N)\" >&2; exit $rc")


def record_attempt(led: Ledger, n: int, probe: str, role: str, argv, started, ended, proc_rc, out, err, extra=None):
    d = led.ev / "attempts" / f"{n:02d}-{probe}-{role}"
    d.mkdir(parents=True, exist_ok=True)
    out, err = led.scrub(out), led.scrub(err)
    (d / "stdout.jsonl").write_text(out)
    (d / "stderr.txt").write_text(err)
    events = [json.loads(l) for l in out.splitlines() if l.strip().startswith("{")]
    marks = dict(re.findall(r"#(T_START|T_END) ([0-9.]+)", err))
    results = [e for e in events if e.get("type") == "result"]
    result = next((e for e in reversed(results) if (e.get("num_turns") or 0) >= 1), results[-1] if results else None)
    meta = {"attempt": n, "probe": probe, "role": role, "argv": led.scrub(json.dumps(argv)),
            "started_at": started, "ended_at": ended, "exit_code": proc_rc,
            "claude_process_interval_epoch": [float(marks["T_START"]) if "T_START" in marks else None, float(marks["T_END"]) if "T_END" in marks else None], "timeout": proc_rc in (124, 137),
            "token_fingerprint": fp(led.tokens.get(role, b"")) if role in led.tokens else None,
            "result_event": {k: result.get(k) for k in ("subtype", "is_error", "session_id", "num_turns", "duration_ms", "usage", "total_cost_usd") if result and k in result} if result else None,
            "rate_limit_events": [e for e in events if e.get("type") == "rate_limit_event"],
            "tool_use_events": sum(1 for e in events for c in (e.get("message", {}) or {}).get("content", []) or [] if isinstance(c, dict) and c.get("type") == "tool_use"),
            "parent_tool_use_events": sum(1 for e in events if e.get("parent_tool_use_id")),
            **(extra or {})}
    (d / "meta.json").write_text(json.dumps(meta, ensure_ascii=False, indent=2) + "\n")
    a = led.state["attempts"][n - 1]
    a.update({"probe": probe, "role": role, "started_at": started, "ended_at": ended, "exit_code": proc_rc})
    led.save()
    return meta


def cmd_setup(ev: Path):
    log = []
    def note(c, r):
        log.append({"cmd": c, "rc": r.returncode, "stdout": r.stdout[-4000:], "stderr": r.stderr[-4000:]})
    for r in ROLES:
        if not (SECRETS / f"{r}.token").exists():
            sys.exit(f"missing {SECRETS}/{r}.token (user issues it with `claude setup-token`)")
    st = SECRETS.stat()
    tokens = {r: (SECRETS / f"{r}.token").read_bytes().strip() for r in ROLES}
    info = {"secrets_dir_mode": oct(st.st_mode & 0o777),
            "token_file_modes": {r: oct((SECRETS / f"{r}.token").stat().st_mode & 0o777) for r in ROLES},
            "token_fingerprints": {r: fp(t) for r, t in tokens.items()},
            "tokens_distinct": tokens["development"] != tokens["guardrail"],
            "token_prefix_ok": {r: t.startswith(b"sk-ant-") for r, t in tokens.items()}}
    for r, uid in ROLES.items():
        d = role_dir(r)
        for sub in ("state/home", "state/claude", "state/work", "state/scratch", "cred", "etc"):
            (d / sub).mkdir(parents=True, exist_ok=True)
        (d / "etc/mcp.json").write_text('{"mcpServers":{}}\n')
        # copy the token into the role's cred volume inside the user namespace, then hand ownership to the role UID
        c = ["podman", "unshare", "bash", "-c",
             f"umask 077; cat {SECRETS}/{r}.token > {d}/cred/oauth_token && "
             f"chown -R {uid}:{uid} {d}/state {d}/cred && chmod 0700 {d}/state {d}/cred {d}/state/* && chmod 0400 {d}/cred/oauth_token && "
             f"chmod 0755 {d}/etc && chmod 0444 {d}/etc/mcp.json"]
        rr = run(c); note(" ".join(c), rr)
        if rr.returncode:
            sys.exit(rr.stderr)
    for c in (["podman", "info", "--format", "{{.Host.Security.Rootless}} {{.Host.NetworkBackend}} {{.Host.IDMappings.UIDMap}}"],
              ["grep", "heain", "/etc/subuid"],
              ["stat", "-c", "%n uid=%u gid=%g mode=%a", *[str(role_dir(r) / p) for r in ROLES for p in ("state", "cred")]],
              ["podman", "image", "inspect", "--format", "{{.Id}} {{.RepoDigests}}", IMAGE],
              ["sha256sum", str(CLAUDE_BIN)]):
        note(" ".join(c), run(c))
    (ev / "setup.json").write_text(json.dumps({"at": now(), "info": info, "commands": log}, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(info, indent=2))


def attempt_pair(led, probe, label, extra_by_role=None, sessions=None):
    ids = led.reserve(2, label)
    procs = {}
    sessions = sessions or {r: str(uuid.uuid4()) for r in ROLES}
    for (r, uid), n in zip(ROLES.items(), ids):
        inner = claude_inner(sessions[r], r, (extra_by_role or {}).get(r, ""))
        argv = podman_role_cmd(r, inner)
        procs[r] = (n, argv, now(), subprocess.Popen(argv, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True))
    metas = {}
    for r, (n, argv, started, p) in procs.items():
        try:
            out, err = p.communicate(timeout=PER_ATTEMPT + 30)
        except subprocess.TimeoutExpired:
            p.kill(); out, err = p.communicate()
        metas[r] = record_attempt(led, n, probe, r, argv, started, now(), p.returncode, out, err, {"issued_session_id": sessions[r]})
    return metas


def stop_on_signal(metas, label):
    """Probe_Budget_Rules §4: after any attempt with a non-allowed rate-limit status or an auth/billing/limit error,
    start no new attempt (PR #12 automated review 4547423210). Evidence of the round is already recorded."""
    for r, x in metas.items():
        rl = [e.get("rate_limit_info", {}).get("status") for e in x.get("rate_limit_events", [])]
        bad_rl = [s for s in rl if s and s != "allowed"]
        res = x.get("result_event") or {}
        if not x.get("result_event"):
            # exit 0 without a result event is not success: unobservable, start no new attempt
            sys.exit(f"STOP after {label} ({r}): no result event (exit={x.get('exit_code')}) -> unobservable; no new attempt")
        if bad_rl or (res.get("is_error") and res.get("subtype") != "success") or x.get("exit_code") not in (0,):
            sys.exit(f"STOP after {label} ({r}): rate_limit={bad_rl} result={res.get('subtype')} exit={x.get('exit_code')} -> no new attempt; record per Probe_Budget_Rules §4")


def cmd_p1(ev):
    led = Ledger(ev)
    rounds = []
    for i in (1, 2):
        m = attempt_pair(led, "P1", f"P1 round {i}")
        ok = all(x["result_event"] and not x["result_event"].get("is_error") and x["result_event"].get("session_id") == x["issued_session_id"] for x in m.values())
        iv = {r: x["claude_process_interval_epoch"] for r, x in m.items()}
        # overlap judged from timestamps taken inside the containers right around the claude process
        # (same host clock); host-side Popen/communicate times are kept only for reference
        overlap = None if any(None in v for v in iv.values()) else max(v[0] for v in iv.values()) < min(v[1] for v in iv.values())
        overlap_s = None if overlap is None else round(min(v[1] for v in iv.values()) - max(v[0] for v in iv.values()), 3)
        rounds.append({"round": i, "both_ok_with_issued_session": ok, "claude_intervals_overlap": overlap, "overlap_seconds": overlap_s,
                       "claude_process_intervals_epoch": iv,
                       "host_wrapper_intervals": {r: [x["started_at"], x["ended_at"]] for r, x in m.items()}})
        if not ok:
            break
        stop_on_signal(m, f"P1 round {i}")
    (ev / "P1.json").write_text(json.dumps({"rounds": rounds}, indent=2) + "\n")
    print(json.dumps(rounds, indent=2))


def cmd_p2(ev, files_only=False):
    led = Ledger(ev)
    tests = []
    def t(tid, desc, argv, expect_denied=True):
        r = run(argv)
        txt = led.scrub(r.stdout + r.stderr)
        found = sorted(set(re.findall(r"\b(?:LIST_OK|LIST_DENIED\w*|READ_OK|READ_DENIED\w*|WRITE_OK|WRITE_DENIED|WRITE_INSIDE\w*|INTRUSION_\w+|TOKEN_(?:ABSENT|PRESENT)|ENV_MATCH|FILE_MATCH)\b", txt)))
        tests.append({"id": tid, "desc": desc, "argv": json.loads(led.scrub(json.dumps(argv))), "rc": r.returncode, "output": txt[-3000:], "expect": "denied" if expect_denied else "n/a",
                      "markers": found, "judgement_basis": "markers" if found else "UNOBSERVABLE: no decisive marker (exit code alone is not used)"})
    dev, gd = role_dir("development"), role_dir("guardrail")
    # 2a: guardrail paths do not exist inside the development container
    g = role_dir("guardrail")
    t("2a", "개발 컨테이너에서 Guardrail 자격·상태 경로 목록·읽기·쓰기", podman_role_cmd("development",
      f"ls -la /run/ieo/cred /state; for p in {g} {g}/cred {g}/state; do printf '%s ' $p; (ls $p >/dev/null 2>&1 && echo LIST_OK) || echo LIST_DENIED_OR_ABSENT; done; "
      f"(cat {g}/cred/oauth_token >/dev/null 2>&1 && echo READ_OK) || echo READ_DENIED_OR_ABSENT; "
      f"(mkdir -p {g}/state 2>/dev/null && touch {g}/state/intrusion 2>/dev/null && echo WRITE_INSIDE_CONTAINER_FS_ONLY) || echo WRITE_DENIED; "
      "cat /run/ieo/cred/* >/dev/null && echo OWN_CRED_READABLE; grep -c guardrail /proc/self/mountinfo || true"))
    # a write into the container's own filesystem is not the guardrail volume; confirm on the host it did not appear
    t("2a-host", "2a의 쓰기 시도가 호스트의 Guardrail 볼륨에 나타나지 않음", ["podman", "unshare", "bash", "-c", f"ls -la {g}/state; test -e {g}/state/intrusion && echo INTRUSION_PRESENT || echo INTRUSION_ABSENT"])
    # 2b: both volumes mounted into one test container; cross-UID access in each direction
    for me, other in (("development", "guardrail"), ("guardrail", "development")):
        o = role_dir(other)
        inner = ("for p in /x/cred /x/cred/oauth_token /x/state /x/state/claude /x/state/scratch; do "
                 "printf '%s ' $p; (ls $p >/dev/null 2>&1 && echo LIST_OK) || echo LIST_DENIED; "
                 "printf '%s ' $p; (cat $p >/dev/null 2>&1 && echo READ_OK) || echo READ_DENIED; done; "
                 "(touch /x/state/intrusion 2>/dev/null && echo WRITE_OK) || echo WRITE_DENIED")
        argv = podman_role_cmd(me, inner, extra_mounts=(f"{o}/cred:/x/cred:ro,z", f"{o}/state:/x/state:z"))
        t(f"2b-{me}->{other}", f"{me} UID가 {other} 소유 0700 볼륨에 접근", argv)
        # positive control: the owner UID reaches the same paths in the same kind of container, so a denial above
        # is caused by the UID boundary and not by mounts/SELinux
        own = ("for p in /run/ieo/cred /run/ieo/cred/oauth_token /state /state/claude /state/scratch; do "
               "printf '%s ' $p; (ls $p >/dev/null 2>&1 && echo LIST_OK) || echo LIST_DENIED; "
               "printf '%s ' $p; (test -d $p || cat $p >/dev/null 2>&1) && echo READ_OK || echo READ_DENIED; done; "
               "(touch /state/scratch/owner-write && rm /state/scratch/owner-write && echo WRITE_OK) || echo WRITE_DENIED")
        t(f"2p-{other}-owner", f"양성 대조: {other} UID가 자기 볼륨에 접근", podman_role_cmd(other, own), expect_denied=False)
    # 2d: guardrail token absent from development container (fingerprint comparison inside, no value printed)
    def token_scan(target_hex):
        return podman_role_cmd("development", "export CLAUDE_CODE_OAUTH_TOKEN=\"$(cat /run/ieo/cred/oauth_token)\"; "
                      f"G={target_hex}; hit=0; n=0; "
                      "e=$(printf %s \"$CLAUDE_CODE_OAUTH_TOKEN\" | sha256sum | cut -d' ' -f1); [ \"$e\" = \"$G\" ] && { echo ENV_MATCH; hit=1; }; "
                      "while IFS= read -r -d '' f; do n=$((n+1)); a=$(sha256sum < \"$f\" | cut -d' ' -f1); b=$(head -c -1 \"$f\" 2>/dev/null | sha256sum | cut -d' ' -f1); "
                      "if [ \"$a\" = \"$G\" ] || [ \"$b\" = \"$G\" ]; then echo FILE_MATCH $f; hit=1; fi; done < <(find /state /run/ieo /etc/ieo -type f -readable -print0 2>/dev/null); "
                      "echo files_hashed=$n; [ $hit = 0 ] && echo TOKEN_ABSENT || echo TOKEN_PRESENT")
    # positive control: the same scan finds the development container's own token
    t("2d-control", "양성 대조: 같은 검사로 개발 자신의 토큰은 찾음", token_scan(hashlib.sha256(led.tokens["development"]).hexdigest()), expect_denied=False)
    gfull = hashlib.sha256(led.tokens["guardrail"]).hexdigest()
    # every readable file (whole content, and content without trailing newline) and the token env var are hashed
    # inside the development container and compared with the guardrail token's SHA-256; no value is passed in
    t("2d", "개발 컨테이너의 파일·환경 어디에도 Guardrail 토큰과 같은 값이 없음", token_scan(gfull))
    # 2e: unprivileged host user (same as the dev conversation) reading role volumes
    for r in ROLES:
        d = role_dir(r)
        t(f"2e-{r}", f"호스트 비특권 사용자 heain이 {r} 0700 볼륨 읽기", ["bash", "-c", f"ls {d}/cred {d}/state/claude 2>&1; cat {d}/cred/oauth_token >/dev/null 2>&1 && echo READ_OK || echo READ_DENIED"])
    if files_only:
        (ev / "P2-files.json").write_text(json.dumps({"at": now(), "tests": tests}, ensure_ascii=False, indent=2) + "\n")
        print(json.dumps([{k: x.get(k) for k in ("id", "rc")} for x in tests], indent=2))
        return
    # 2c: development tries to resume the guardrail's P1 session id (one model-request attempt)
    p1 = json.loads((ev / "P1.json").read_text())
    gsid = None
    for a in sorted((ev / "attempts").glob("*-P1-guardrail")):
        gsid = json.loads((a / "meta.json").read_text()).get("issued_session_id")
    n = led.reserve(1, "P2 2c resume other role session")[0]
    inner = (
        "set -u; export CLAUDE_CODE_OAUTH_TOKEN=\"$(cat /run/ieo/cred/oauth_token)\"; "
        f"exec timeout -k 10 290 /opt/claude/claude -p 'Reply with exactly: ok-resume' --output-format stream-json --verbose "
        f"--resume {gsid} --tools '' --strict-mcp-config --mcp-config /etc/ieo/role/mcp.json --setting-sources user --permission-mode dontAsk")
    argv = podman_role_cmd("development", inner)
    started = now(); r = run(argv, timeout=PER_ATTEMPT + 30)
    meta = record_attempt(led, n, "P2c", "development", argv, started, now(), r.returncode, r.stdout, r.stderr, {"target_session_id": gsid})
    tests.append({"id": "2c", "desc": "개발 프로세스가 Guardrail 세션 재개", "attempt": n, "rc": r.returncode,
                  "result_event": meta["result_event"], "stderr_tail": led.scrub(r.stderr)[-1500:]})
    (ev / "P2.json").write_text(json.dumps({"at": now(), "tests": tests, "p1_rounds": p1["rounds"]}, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps([{k: x.get(k) for k in ("id", "rc")} for x in tests], indent=2))


def cmd_p3(ev):
    led = Ledger(ev)
    out = {"auth_status": {}, "rounds": []}
    for r in ROLES:  # CLI run without a model request
        argv = podman_role_cmd(r, "export CLAUDE_CODE_OAUTH_TOKEN=\"$(cat /run/ieo/cred/oauth_token)\"; timeout 60 /opt/claude/claude auth status --json 2>&1 || timeout 60 /opt/claude/claude auth status 2>&1")
        res = run(argv)
        led.state["cli_runs"].append({"label": f"P3 auth status {r}", "at": now(), "rc": res.returncode}); led.save()
        txt = led.scrub(res.stdout + res.stderr)
        try:
            j = json.loads(txt[txt.index("{"):txt.rindex("}") + 1])
        except ValueError:
            j = None
        ident = {k: (fp(str(j[k]).encode()) if j and j.get(k) else None) for k in ("email", "orgId", "accountUuid", "orgName")} if j else None
        out["auth_status"][r] = {"rc": res.returncode, "identity_fingerprints": ident,
                                 "fields_present": sorted(j.keys()) if j else None,
                                 "non_identity": {k: j.get(k) for k in ("authMethod", "apiProvider", "subscriptionType", "planTier") if j and k in j} if j else None,
                                 "raw_if_unparsed": None if j else txt[-1500:]}
    for i in (1, 2):
        m = attempt_pair(led, "P3", f"P3 round {i}")
        out["rounds"].append({r: {"rate_limit_events": x["rate_limit_events"], "result_ok": bool(x["result_event"] and not x["result_event"].get("is_error"))} for r, x in m.items()})
        if i == 1:
            (ev / "P3.json").write_text(json.dumps(out, ensure_ascii=False, indent=2) + "\n")
            stop_on_signal(m, "P3 round 1")
    (ev / "P3.json").write_text(json.dumps(out, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(out, ensure_ascii=False, indent=2)[:6000])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("step", choices=["setup", "p1", "p2", "p2-files", "p3", "status"])
    ap.add_argument("--evidence", type=Path, default=DEFAULT_EVIDENCE)
    a = ap.parse_args()
    a.evidence.mkdir(parents=True, exist_ok=True)
    if a.step == "status":
        led = Ledger(a.evidence); n, w = led.remaining(); print(f"attempts used {MAX_ATTEMPTS - n}/30, wall remaining {int(w)}s"); return
    {"setup": cmd_setup, "p1": cmd_p1, "p2": cmd_p2, "p2-files": lambda e: cmd_p2(e, files_only=True), "p3": cmd_p3}[a.step](a.evidence)


if __name__ == "__main__":
    main()
