# Plumbing-only fake for the scorer dry check (non-model happy/failed paths). NOT a reference implementation.
import hashlib, json, os, re, subprocess, sys
a = sys.argv[1:]
if a[0] != "run": sys.exit(3)
sep = a.index("--"); opts, argv = a[1:sep], a[sep + 1:]
o = {}; arts = []; marks = []
i = 0
while i < len(opts):
    k = opts[i]
    if k == "--artifact": arts.append(opts[i + 1]); i += 2
    elif k == "--marker": marks.append(opts[i + 1]); i += 2
    else: o[k] = opts[i + 1]; i += 2
d = os.path.join(o["--out"], o["--task"], o["--attempt"], o["--command"])
try: os.makedirs(d)
except FileExistsError: sys.exit(2)
kind, code, err = "exited", None, None
try:
    p = subprocess.run(argv, capture_output=True, env={}); out, er, code = p.stdout, p.stderr, p.returncode
except OSError as e:
    kind, out, er, err = "launch_failed", b"", b"", str(e)
open(os.path.join(d, "stdout"), "wb").write(out); open(os.path.join(d, "stderr"), "wb").write(er)
def sf(n, b): return {"file": n, "bytes": len(b), "sha256": hashlib.sha256(b).hexdigest()}
A = []
for p in arts:
    if os.path.exists(p): b = open(p, "rb").read(); A.append({"path": p, "present": True, "bytes": len(b), "sha256": hashlib.sha256(b).hexdigest()})
    else: A.append({"path": p, "present": False, "bytes": None, "sha256": None})
M = [{"pattern": m, "found": re.search(m, out.decode()) is not None} for m in marks]
if kind != "exited" or code != 0: st = "failed"
elif not all(x["present"] for x in A) or not all(x["found"] for x in M): st = "incomplete"
else: st = "succeeded"
r = {"format": "ieo-evidence-record/v0", "task_id": o["--task"], "attempt_id": o["--attempt"], "command_id": o["--command"], "role": o.get("--role", "development"),
     "retry_of": None, "model_request": False, "argv": argv, "cwd": os.getcwd(), "env_names": [], "started_at": "2026-11-12T00:00:00.000Z", "ended_at": "2026-11-12T00:00:00.000Z",
     "duration_ms": 1, "process": {"kind": kind, "exit_code": code, "signal": None, "timeout_ms": None, "term_sent": False, "kill_sent": False, "stray_group_killed": False, "output_incomplete": False, "launch_error": err},
     "stdout": sf("stdout", out), "stderr": sf("stderr", er), "redaction": {}, "artifacts": A, "markers": M, "result_event": None,
     "usage": {"applicability": "not_applicable"}, "judgment": {"expected": "success", "status": st, "reasons": [], "counts_as_success": st == "succeeded"}}
json.dump(r, open(os.path.join(d, "record.json"), "w"))
sys.exit(0 if st == "succeeded" else 1)
