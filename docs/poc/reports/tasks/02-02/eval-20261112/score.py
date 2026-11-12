#!/usr/bin/env python3
"""02-02 independent scorer for ieo-evidence (stdlib only, Python 3.12).

usage: score.py --set SETFILE --bin CANDIDATE --out RESULT.json [--fixtures DIR] [--work DIR] [--keep] [--only ID,...]

The candidate only ever receives the case inputs (command lines, fixture files, env values).
Expected values stay inside this process and are compared after the invocations finish.

Target syntax in "expected":
  exit[i]                 CLI exit status of invocation i (int, or "timeout"/"spawn_error")
  cli_stdout[i] / cli_stderr[i]   text printed by the candidate itself
  post[i]                 exit status of post-check shell command i
  record:T/A/C#/ptr       JSON pointer into OUT/T/A/C/record.json ("" = whole record)
  recordtext:T/A/C        raw text of that record.json
  file:REL                bytes of OUT/REL (text compare / contains)
  exists:REL              whether OUT/REL exists
  summary[i]              stdout text of invocation i; summary[i]#/ptr JSON pointer; summary[i]~key breadth-first key search
  tree                    all files under OUT (for $tree_not_contains)
Spec: a literal (compared by JSON equality, floats within 1e-12) or an object of $-operators (all must hold):
  $approx(+$tol) $gte $lte $len $len_gte $not_null $set_eq $regex $text $contains $contains_all $not_contains $has_keys $tree_not_contains
"""
import argparse, base64, json, math, os, re, shlex, shutil, subprocess, sys, tempfile, time
from collections import deque
from datetime import datetime, timezone

MISSING = object()
INV_TIMEOUT_S = 90


def jeq(a, b):
    if isinstance(a, bool) or isinstance(b, bool):
        return type(a) is type(b) and a == b
    if isinstance(a, (int, float)) and isinstance(b, (int, float)):
        if isinstance(a, int) and isinstance(b, int):
            return a == b
        return math.isclose(a, b, rel_tol=0, abs_tol=1e-12)
    if isinstance(a, dict) and isinstance(b, dict):
        return a.keys() == b.keys() and all(jeq(a[k], b[k]) for k in a)
    if isinstance(a, list) and isinstance(b, list):
        return len(a) == len(b) and all(jeq(x, y) for x, y in zip(a, b))
    return a == b


def ptr_get(doc, ptr):
    if ptr == "":
        return doc
    cur = doc
    for raw in ptr.lstrip("/").split("/"):
        tok = raw.replace("~1", "/").replace("~0", "~")
        if isinstance(cur, dict):
            if tok not in cur:
                return MISSING
            cur = cur[tok]
        elif isinstance(cur, list):
            if not tok.isdigit() or int(tok) >= len(cur):
                return MISSING
            cur = cur[int(tok)]
        else:
            return MISSING
    return cur


def bfs_key(doc, key):
    q = deque([doc])
    while q:
        n = q.popleft()
        if isinstance(n, dict):
            if key in n:
                return n[key]
            q.extend(n.values())
        elif isinstance(n, list):
            q.extend(n)
    return MISSING


def as_text(v):
    if isinstance(v, bytes):
        return v.decode("utf-8", "replace")
    if isinstance(v, str):
        return v
    return json.dumps(v, ensure_ascii=False)


def check(value, spec):
    """returns None if ok, else a short description of the failure"""
    if value is MISSING:
        return "missing"
    if not (isinstance(spec, dict) and spec and all(k.startswith("$") for k in spec)):
        return None if jeq(value, spec) else "got %s" % json.dumps(value, ensure_ascii=False, default=str)[:300]
    errs = []
    for op, arg in spec.items():
        ok = True
        try:
            if op == "$approx":
                ok = isinstance(value, (int, float)) and not isinstance(value, bool) and math.isclose(value, arg, rel_tol=0, abs_tol=spec.get("$tol", 1e-12))
            elif op == "$tol":
                continue
            elif op == "$gte":
                ok = isinstance(value, (int, float)) and value >= arg
            elif op == "$lte":
                ok = isinstance(value, (int, float)) and value <= arg
            elif op == "$len":
                ok = hasattr(value, "__len__") and len(value) == arg
            elif op == "$len_gte":
                ok = hasattr(value, "__len__") and len(value) >= arg
            elif op == "$not_null":
                ok = (value is not None) == bool(arg)
            elif op == "$set_eq":
                ok = isinstance(value, list) and sorted(map(str, value)) == sorted(map(str, arg)) and len(value) == len(arg)
            elif op == "$regex":
                ok = isinstance(value, str) and re.search(arg, value) is not None
            elif op == "$text":
                ok = isinstance(value, (bytes, str)) and as_text(value) == arg and (not isinstance(value, bytes) or value == arg.encode())
            elif op == "$contains":
                ok = arg in as_text(value)
            elif op == "$contains_all":
                ok = all(a in as_text(value) for a in arg)
            elif op == "$not_contains":
                ok = arg not in as_text(value)
            elif op == "$has_keys":
                ok = isinstance(value, dict) and all(k in value for k in arg)
                if not ok and isinstance(value, dict):
                    errs.append("missing keys %s" % [k for k in arg if k not in value]); continue
            elif op == "$tree_not_contains":
                hits = [(p, a) for p, data in value for a in arg if a.encode() in data]
                ok = not hits
                if not ok:
                    errs.append("secret-like value found in %d file(s): %s" % (len(hits), sorted({p for p, _ in hits})[:5])); continue
            else:
                errs.append("unknown operator %s" % op); continue
        except Exception as e:  # never crash on odd candidate output
            ok = False; errs.append("%s error %s" % (op, e)); continue
        if not ok:
            shown = value if not isinstance(value, list) or op != "$tree_not_contains" else "<tree>"
            errs.append("%s %s failed; got %s" % (op, json.dumps(arg, ensure_ascii=False)[:120], as_text(shown)[:300]))
    return "; ".join(errs) if errs else None


class Ctx:
    def __init__(self, out):
        self.out = out
        self.inv = []   # dicts: exit, stdout, stderr
        self.post = []
        self._rec = {}

    def record(self, rel):
        if rel not in self._rec:
            p = os.path.join(self.out, rel, "record.json")
            try:
                self._rec[rel] = json.load(open(p, encoding="utf-8"))
            except FileNotFoundError:
                self._rec[rel] = MISSING
            except Exception as e:
                self._rec[rel] = ("unparsable", str(e))
        return self._rec[rel]

    def resolve(self, target):
        m = re.fullmatch(r"(exit|cli_stdout|cli_stderr|post)\[(\d+)\]", target)
        if m:
            kind, i = m.group(1), int(m.group(2))
            src = self.post if kind == "post" else self.inv
            if i >= len(src):
                return MISSING
            return src[i] if kind == "post" else self.inv[i][{"exit": "exit", "cli_stdout": "stdout", "cli_stderr": "stderr"}[kind]]
        m = re.fullmatch(r"record:([^#]+)#(.*)", target)
        if m:
            r = self.record(m.group(1))
            if r is MISSING:
                return MISSING
            if isinstance(r, tuple):
                return "<record.json unparsable: %s>" % r[1]
            return ptr_get(r, m.group(2))
        if target.startswith("recordtext:"):
            try:
                return open(os.path.join(self.out, target[11:], "record.json"), "rb").read()
            except OSError:
                return MISSING
        if target.startswith("file:"):
            try:
                return open(os.path.join(self.out, target[5:]), "rb").read()
            except OSError:
                return MISSING
        if target.startswith("exists:"):
            return os.path.exists(os.path.join(self.out, target[7:]))
        m = re.fullmatch(r"summary\[(\d+)\](?:(#|~)(.*))?", target)
        if m:
            i = int(m.group(1))
            if i >= len(self.inv):
                return MISSING
            text = self.inv[i]["stdout"]
            if m.group(2) is None:
                return text
            try:
                doc = json.loads(text)
            except Exception:
                return MISSING
            return ptr_get(doc, m.group(3)) if m.group(2) == "#" else bfs_key(doc, m.group(3))
        if target == "tree":
            files = []
            for root, _, names in os.walk(self.out):
                for n in names:
                    p = os.path.join(root, n)
                    try:
                        files.append((os.path.relpath(p, self.out), open(p, "rb").read()))
                    except OSError:
                        pass
            return files
        return MISSING


def subst(s, ph):
    for k, v in ph.items():
        s = s.replace("{" + k + "}", v)
    return s


def run_case(case, binpath, fixtures, workroot, keep):
    tmp = tempfile.mkdtemp(prefix=case["id"] + "-", dir=workroot)
    ph = {"BIN": binpath, "OUT": os.path.join(tmp, "out"), "FIX": os.path.join(tmp, "fix"), "TMP": os.path.join(tmp, "work")}
    for d in ("FIX", "TMP"):
        os.makedirs(ph[d])
    # OUT is not pre-created: the candidate creates it
    ctx = Ctx(ph["OUT"])
    setup_err = None
    try:
        for f in case["setup"].get("files", []):
            path = subst(f["path"], ph)
            os.makedirs(os.path.dirname(path), exist_ok=True)
            if "fixture" in f:
                shutil.copyfile(os.path.join(fixtures, f["fixture"]), path)
            elif "text" in f:
                open(path, "wb").write(f["text"].encode())
            elif "b64" in f:
                open(path, "wb").write(base64.b64decode(f["b64"]))
        for sh in case["setup"].get("shell", []):
            subprocess.run(["/bin/sh", "-c", subst(sh, ph)], check=True, cwd=ph["TMP"], timeout=60)
    except Exception as e:
        setup_err = "setup failed: %s" % e
    env = dict(os.environ)
    env.update(case["setup"].get("env", {}))
    t0 = time.monotonic()
    if setup_err is None:
        for line in case["invocations"]:
            argv = shlex.split(subst(line, ph))
            try:
                p = subprocess.run(argv, cwd=ph["TMP"], env=env, stdin=subprocess.DEVNULL, capture_output=True, timeout=INV_TIMEOUT_S)
                ctx.inv.append({"exit": p.returncode, "stdout": p.stdout.decode("utf-8", "replace"), "stderr": p.stderr.decode("utf-8", "replace")})
            except subprocess.TimeoutExpired as e:
                ctx.inv.append({"exit": "timeout", "stdout": (e.stdout or b"").decode("utf-8", "replace"), "stderr": (e.stderr or b"").decode("utf-8", "replace")})
            except OSError as e:
                ctx.inv.append({"exit": "spawn_error", "stdout": "", "stderr": str(e)})
        for sh in case.get("post", []):
            try:
                ctx.post.append(subprocess.run(["/bin/sh", "-c", subst(sh, ph)], timeout=60).returncode)
            except Exception:
                ctx.post.append("post_error")
    elapsed = round(time.monotonic() - t0, 2)
    und = case.get("undetermined_fields", {})
    fails, checked = [], 0
    for target, spec in case["expected"].items():
        if target in und:
            continue
        checked += 1
        if setup_err:
            fails.append({"field": target, "problem": setup_err}); continue
        try:
            err = check(ctx.resolve(target), spec)
        except Exception as e:
            err = "scorer error: %s" % e
        if err:
            fails.append({"field": target, "problem": err})
    if not keep:
        shutil.rmtree(tmp, ignore_errors=True)
    return {"id": case["id"], "verdict": "PASS" if not fails else "FAIL", "checked_fields": checked, "failed_fields": fails,
            "undetermined_fields": len(und), "exits": [i["exit"] for i in ctx.inv], "seconds": elapsed, "workdir": tmp if keep else None}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--set", required=True)
    ap.add_argument("--bin", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--fixtures")
    ap.add_argument("--work")
    ap.add_argument("--keep", action="store_true")
    ap.add_argument("--only")
    a = ap.parse_args()
    data = json.load(open(a.set, encoding="utf-8"))
    fixtures = a.fixtures or os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(a.set))), "fixtures")
    binpath = os.path.abspath(a.bin)
    workroot = a.work or tempfile.gettempdir()
    os.makedirs(workroot, exist_ok=True)
    only = set(a.only.split(",")) if a.only else None
    results, skipped = [], []
    for case in data["cases"]:
        if only and case["id"] not in only:
            continue
        if case.get("status") == "undetermined":
            skipped.append({"id": case["id"], "reason": case.get("undetermined_reason", "")})
            print("%-6s UNDETERMINED (excluded): %s" % (case["id"], case.get("undetermined_reason", "")))
            continue
        r = run_case(case, binpath, fixtures, workroot, a.keep)
        results.append(r)
        print("%-6s %s  fields %d checked, %d failed, %d undetermined  exits=%s  %.1fs" % (r["id"], r["verdict"], r["checked_fields"], len(r["failed_fields"]), r["undetermined_fields"], r["exits"], r["seconds"]))
        for f in r["failed_fields"]:
            print("         - %s: %s" % (f["field"], f["problem"]))
        sys.stdout.flush()
    passed = sum(r["verdict"] == "PASS" for r in results)
    totals = {"determined_cases": len(results), "passed": passed, "failed": len(results) - passed,
              "undetermined_cases_excluded": len(skipped), "undetermined_fields_excluded": sum(r["undetermined_fields"] for r in results),
              "checked_fields": sum(r["checked_fields"] for r in results), "failed_fields": sum(len(r["failed_fields"]) for r in results)}
    print("TOTAL set=%s determined=%d passed=%d failed=%d | undetermined cases excluded=%d, undetermined fields excluded=%d | fields checked=%d failed=%d"
          % (data.get("set"), totals["determined_cases"], passed, totals["failed"], totals["undetermined_cases_excluded"],
             totals["undetermined_fields_excluded"], totals["checked_fields"], totals["failed_fields"]))
    out = {"scored_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"), "set": data.get("set"), "set_file": os.path.abspath(a.set),
           "bin": binpath, "totals": totals, "cases": results, "undetermined_cases": skipped}
    json.dump(out, open(a.out, "w"), indent=1, ensure_ascii=False)
    return 0 if totals["failed"] == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
