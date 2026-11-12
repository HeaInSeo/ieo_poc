"""Generate public/authoring/validation case sets. Expected values come from the approved rules
(R-* references below) and the independent stream reader tools/oracle_stream.py."""
import hashlib, json, os, shlex, sys
sys.path.insert(0, os.path.dirname(__file__))
import oracle_stream as O

EVAL = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FXD = os.path.join(EVAL, "fixtures")
def fx(name): return O.read(os.path.join(FXD, name))
def sha(b): return hashlib.sha256(b if isinstance(b, bytes) else b.encode()).hexdigest()
def fsha(name): return sha(open(os.path.join(FXD, name), "rb").read())
def fsize(name): return os.path.getsize(os.path.join(FXD, name))

# rule references
R_RUN = "02-01 REPORT §1 row 실행(run.rs)"
R_LOG = "02-01 REPORT §1 row 원본 로그·digest"
R_SEC = "02-01 REPORT §1 row 비밀"
R_JDG = "02-01 REPORT §1 row 판정(judge.rs)"
R_USE = "02-01 REPORT §1 row 사용량(stream.rs)"
R_ATT = "02-01 REPORT §1 row 귀속 대조"
R_AGG = "02-01 REPORT §1 row 합산(aggregate.rs)"
R_ID = "02-01 REPORT §1 intro (one dir per command, no overwrite) + task brief item 6 (exit 2 = request/evidence-write error)"
R_CLI = "task brief item 6 (CLI exit: 0 success/expected_matched, 1 otherwise, 2 request error)"
R_C7 = "Runtime_Contract_v0 §11 item 7 + §4.2 row 자식 사건 귀속"
R_R3 = "02-01 REPORT §3 (resume share / base cases)"
R_P22 = "review-pr21-273907b §2 P2-2 + REPORT §1 run row (output_incomplete -> incomplete)"
R_P23 = "review-pr21-273907b §2 P2-3 (TERM-ignoring member KILLed after grace, stray_group_killed)"
R_P24 = "review-pr21-273907b §2 P2-4 + REPORT §1 비밀 row (refusal exit 2, before dir creation, value not printed)"
R_FIX = "task brief item 5 (carried fixes: 2 s drain, output_incomplete on kill with open pipes, shared 2 s deadline, summarize duplicate refusal exit 2)"
R_SCH = "contracts/evidence-record-v0.schema.json"
R_MRK = "02-01 REPORT §5 item 4 (markers searched in stdout after masking)"
R_REV = "review-pr21-2df3c35 §3.4 (tokens_measured_sum null when nothing measured; key name from accepted review)"

def tk(t): return {"input": t["input"], "output": t["output"], "cache_read": t["cache_read"], "cache_creation": t["cache_creation"]}
def T(i, o, cr, cc): return {"input": i, "output": o, "cache_read": cr, "cache_creation": cc}

def run(t, a, c, argv, *opts):
    return shlex.join(["{BIN}", "run", "--out", "{OUT}", "--task", t, "--attempt", a, "--command", c, *opts, "--", *argv])
def summ(*paths): return shlex.join(["{BIN}", "summarize", *paths])
def rec(t, a, c, ptr): return f"record:{t}/{a}/{c}#{ptr}"
SH = "/bin/sh"
CAT = "/bin/cat"

class Case:
    def __init__(self, cid, setname, family, title):
        self.d = {"id": cid, "set": setname, "family": family, "title_for_author": title, "setup": {"files": [], "env": {}},
                  "invocations": [], "post": [], "expected": {}, "basis": {}, "undetermined_fields": {}, "status": "determined"}
    def file(self, path, **kw): self.d["setup"]["files"].append({"path": path, **kw}); return self
    def env(self, **kw): self.d["setup"]["env"].update(kw); return self
    def inv(self, s): self.d["invocations"].append(s); return len(self.d["invocations"]) - 1
    def post(self, s): self.d["post"].append(s); return len(self.d["post"]) - 1
    def exp(self, path, val, basis):
        assert path not in self.d["expected"], path
        self.d["expected"][path] = val; self.d["basis"][path] = basis; return self
    def und(self, path, reason):
        if path in self.d["expected"] or path + " (value text)" in self.d["undetermined_fields"]:
            path = path + " (value text)"   # the exact text is open; the determined check on the same field stays
        self.d["undetermined_fields"][path] = reason; return self
    def undetermined(self, reason): self.d["status"] = "undetermined"; self.d["undetermined_reason"] = reason; return self

def judge(c, t, a, cc, status, counts, inv, cli, basis=R_JDG):
    c.exp(rec(t, a, cc, "/judgment/status"), status, basis)
    c.exp(rec(t, a, cc, "/judgment/counts_as_success"), counts, basis)
    c.exp(f"exit[{inv}]", cli, R_CLI)

def model_stream_expect(c, t, a, cc, name, mode, base=None, run_state="measured", run_tokens=None, attribution=None, judge_status=None, inv=None, cli=None, base_cost=None, check_session=True, check_cost=True):
    s = fx(name)
    p = lambda ptr: rec(t, a, cc, ptr)
    c.exp(p("/model_request"), True, R_SCH + " allOf model_request")
    c.exp(p("/result_event/count"), s["count"], R_USE + " (result events counted); oracle count=%d" % s["count"])
    c.exp(p("/result_event/present"), s["present"], R_USE)
    if s["present"]:
        for k in ["subtype", "is_error", "session_id", "num_turns", "stop_reason"]:
            c.exp(p("/result_event/" + k), s[k], R_USE + " selected result = last with num_turns>=1 else last; oracle")
    c.exp(p("/result_event/non_json_lines"), s["nonjson"], R_USE + "; oracle counts unparsable lines")
    c.exp(p("/result_event/rate_limit_statuses"), s["rate_limit"], R_JDG + " (rate_limit_event status list); oracle")
    c.exp(p("/result_event/compact_boundaries"), s["compact"], R_ATT + "; oracle counts system/compact_boundary")
    c.exp(p("/usage/applicability"), "model", R_SCH)
    c.exp(p("/usage/session_mode"), mode, R_SCH + " + CLI --session")
    c.exp(p("/usage/internal_retries"), "unmeasured", R_USE + " (internal_retries: unmeasured)")
    if s["present"]:
        if s["parent"] is not None:
            c.exp(p("/usage/parent/state"), "measured", R_USE + " parent=result.usage")
            c.exp(p("/usage/parent/tokens"), tk(s["parent"]), R_USE + " parent=result.usage; oracle %s" % s["parent"])
        else:
            c.exp(p("/usage/parent/state"), "unmeasured", R_USE + " (missing one of four fields -> unmeasured)")
        if check_session:
            if s["session"] is not None:
                c.exp(p("/usage/session/state"), "measured", R_USE + " session=sum(modelUsage)")
                c.exp(p("/usage/session/tokens"), tk(s["session"]), R_USE + " session=sum(modelUsage); oracle %s" % s["session"])
            else:
                c.exp(p("/usage/session/state"), "unmeasured", R_USE + " (modelUsage missing a field / absent)")
        if check_cost and s.get("cost") is not None:
            c.exp(p("/usage/cost_usd_reported"), {"$approx": s["cost"]}, R_C7 + " total_cost_usd as reported (cumulative)")
    else:
        for k in ["parent", "session"]:
            c.exp(p(f"/usage/{k}/state"), "unmeasured", R_USE + " (no result event -> run/parent/session unmeasured)")
    if mode == "new":
        c.exp(p("/usage/resume_base"), None, R_SCH + " allOf new => resume_base null")
    c.exp(p("/usage/run/state"), run_state, R_C7 + "; " + R_USE)
    if run_state == "measured" and run_tokens is not None:
        c.exp(p("/usage/run/tokens"), run_tokens, R_C7 + " run = session (new) or session - base session (resume); oracle")
        if s.get("cost") is not None:
            rc = s["cost"] - (base_cost or 0.0)
            c.exp(p("/usage/run_cost_usd"), {"$approx": rc}, R_C7 + " total_cost_usd cumulative; run cost = cost - base cost = %r" % rc)
    if attribution:
        c.exp(p("/usage/attribution_check"), attribution, R_ATT)
        if attribution == "mismatch":
            c.exp(p("/usage/attribution_detail"), {"$not_null": True}, R_ATT + " (difference recorded)")
    if judge_status:
        judge(c, t, a, cc, judge_status, judge_status in ("succeeded", "expected_matched"), inv, cli)
    c.und(p("/usage/by_model"), "per-model breakdown content (cumulative vs run share) not fixed by rules")
    c.und(p("/result_event/runtime_version"), "source event/field for runtime_version not fixed by rules")
    return s

def stdout_exact(c, t, a, cc, data, name="stdout"):
    b = data.encode()
    c.exp(rec(t, a, cc, f"/{name}/bytes"), len(b), R_LOG + "; len(%r)" % data)
    c.exp(rec(t, a, cc, f"/{name}/sha256"), sha(b), R_LOG + "; sha256(%r)" % data)
    c.exp(f"file:{t}/{a}/{cc}/{name}", {"$text": data}, R_LOG + " (raw log stored)")

sets = {"public": [], "authoring": [], "validation": []}
def add(c): sets[c.d["set"]].append(c.d)
SCHEMA_KEYS = ["format", "task_id", "attempt_id", "command_id", "role", "retry_of", "model_request", "argv", "cwd", "env_names", "started_at", "ended_at", "duration_ms", "process", "stdout", "stderr", "redaction", "artifacts", "markers", "result_event", "usage", "judgment"]

# ===================== PUBLIC =====================
c = Case("P-01", "public", "success_markers_artifacts", "exit 0 + artifact + marker")
i = c.inv(run("pt", "a1", "c1", [SH, "-c", "printf 'payload' > {TMP}/art.txt; echo 'BUILD-OK 7'"], "--artifact", "{TMP}/art.txt", "--marker", r"BUILD-OK \d+"))
c.exp(rec("pt", "a1", "c1", ""), {"$has_keys": SCHEMA_KEYS}, R_SCH + " required")
c.exp(rec("pt", "a1", "c1", "/format"), "ieo-evidence-record/v0", R_SCH)
c.exp(rec("pt", "a1", "c1", "/task_id"), "pt", R_SCH)
c.exp(rec("pt", "a1", "c1", "/started_at"), {"$regex": r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d{3}Z$"}, R_SCH + " UTC ms")
c.exp(rec("pt", "a1", "c1", "/model_request"), False, R_SCH)
c.exp(rec("pt", "a1", "c1", "/result_event"), None, R_SCH + " model_request false => null")
c.exp(rec("pt", "a1", "c1", "/usage/applicability"), "not_applicable", R_SCH)
c.exp(rec("pt", "a1", "c1", "/process/kind"), "exited", R_RUN)
c.exp(rec("pt", "a1", "c1", "/process/exit_code"), 0, R_RUN)
c.exp(rec("pt", "a1", "c1", "/artifacts/0/present"), True, R_LOG)
c.exp(rec("pt", "a1", "c1", "/artifacts/0/bytes"), 7, R_LOG + "; len('payload')=7")
c.exp(rec("pt", "a1", "c1", "/artifacts/0/sha256"), sha("payload"), R_LOG + "; sha256('payload')")
c.exp(rec("pt", "a1", "c1", "/markers/0/found"), True, R_JDG)
stdout_exact(c, "pt", "a1", "c1", "BUILD-OK 7\n")
judge(c, "pt", "a1", "c1", "succeeded", True, i, 0)
add(c)

c = Case("P-02", "public", "nonzero_exit", "exit 5 is failure, stderr kept")
i = c.inv(run("pt", "a1", "c2", [SH, "-c", "echo 'bad input' >&2; exit 5"]))
c.exp(rec("pt", "a1", "c2", "/process/kind"), "exited", R_RUN)
c.exp(rec("pt", "a1", "c2", "/process/exit_code"), 5, R_RUN)
stdout_exact(c, "pt", "a1", "c2", "bad input\n", "stderr")
judge(c, "pt", "a1", "c2", "failed", False, i, 1)
add(c)

c = Case("P-03", "public", "launch_failure", "nonexistent program still writes record")
i = c.inv(run("pt", "a1", "c3", ["/nonexistent/ieo-no-such-prog"]))
c.exp(rec("pt", "a1", "c3", "/process/kind"), "launch_failed", R_RUN + " (launch failure recorded)")
c.exp(rec("pt", "a1", "c3", "/process/launch_error"), {"$not_null": True}, R_RUN)
judge(c, "pt", "a1", "c3", "failed", False, i, 1)
c.und(rec("pt", "a1", "c3", "/process/launch_error"), "wording of the OS error text not fixed (only non-null checked)")
add(c)

c = Case("P-04", "public", "model_new_session", "real 01-04 stream, new session")
c.file("{FIX}/s.jsonl", fixture="fx-01.jsonl")
i = c.inv(run("pt", "a1", "c4", [CAT, "{FIX}/s.jsonl"], "--model-request", "--session", "new"))
s = model_stream_expect(c, "pt", "a1", "c4", "fx-01.jsonl", "new", run_tokens=tk(fx("fx-01.jsonl")["session"]), attribution="consistent", judge_status="succeeded", inv=i, cli=0)
c.exp(rec("pt", "a1", "c4", "/stdout/sha256"), fsha("fx-01.jsonl"), R_LOG + " (no secret in stream -> stored bytes identical)")
c.exp(rec("pt", "a1", "c4", "/usage/children"), [], R_USE + " (no Agent tool_use)")
add(c)

c = Case("P-05", "public", "missing_evidence", "exit 0 but required artifact absent")
i = c.inv(run("pt", "a1", "c5", [SH, "-c", "echo done"], "--artifact", "{TMP}/never-created.bin"))
c.exp(rec("pt", "a1", "c5", "/artifacts/0/present"), False, R_LOG)
c.exp(rec("pt", "a1", "c5", "/artifacts/0/bytes"), None, R_SCH + " + " + R_LOG)
c.exp(rec("pt", "a1", "c5", "/artifacts/0/sha256"), None, R_SCH + " + " + R_LOG)
c.exp(rec("pt", "a1", "c5", "/process/exit_code"), 0, R_RUN)
judge(c, "pt", "a1", "c5", "incomplete", False, i, 1)
add(c)

# ===================== AUTHORING =====================
c = Case("A-01", "authoring", "success_markers_artifacts", "two artifacts, marker")
i = c.inv(run("at", "x1", "build", [SH, "-c", "printf 'alpha\\n' > {TMP}/o1; printf '' > {TMP}/o2; echo 'stage complete: 3/3'"],
               "--artifact", "{TMP}/o1", "--artifact", "{TMP}/o2", "--marker", r"stage complete: \d/\d", "--role", "guardrail"))
c.exp(rec("at", "x1", "build", "/role"), "guardrail", R_SCH + " role from --role")
c.exp(rec("at", "x1", "build", "/artifacts/0/bytes"), 6, R_LOG + "; 'alpha\\n'")
c.exp(rec("at", "x1", "build", "/artifacts/0/sha256"), sha("alpha\n"), R_LOG)
c.exp(rec("at", "x1", "build", "/artifacts/1/present"), True, R_LOG)
c.exp(rec("at", "x1", "build", "/artifacts/1/bytes"), 0, R_LOG + "; empty file")
c.exp(rec("at", "x1", "build", "/artifacts/1/sha256"), sha(b""), R_LOG)
c.exp(rec("at", "x1", "build", "/markers/0/found"), True, R_JDG)
judge(c, "at", "x1", "build", "succeeded", True, i, 0)
add(c)

c = Case("A-02", "authoring", "timeout_term_honoured", "expected timeout, TERM honoured")
i = c.inv(run("at", "x1", "slow", ["/bin/sleep", "40"], "--timeout-s", "1", "--grace-s", "4", "--expect", "timeout"))
p = lambda ptr: rec("at", "x1", "slow", ptr)
c.exp(p("/process/kind"), "timed_out", R_RUN)
c.exp(p("/process/timeout_ms"), 1000, R_SCH + "; 1 s")
c.exp(p("/process/term_sent"), True, R_RUN)
c.exp(p("/process/kill_sent"), False, R_RUN + " (no member left after TERM -> no KILL)")
c.exp(p("/duration_ms"), {"$gte": 980, "$lte": 4000}, R_RUN + " (TERM at 1 s; no grace wait when group empty); bounds = timeout-20ms .. timeout+grace")
judge(c, "at", "x1", "slow", "expected_matched", True, i, 0)
c.und(p("/process/signal"), "whether timed_out records the terminating signal is not fixed")
add(c)

c = Case("A-03", "authoring", "missing_evidence", "exit 0 without required marker")
i = c.inv(run("at", "x1", "nomark", [SH, "-c", "echo 'all fine'"], "--marker", "TESTS PASSED"))
c.exp(rec("at", "x1", "nomark", "/markers/0/found"), False, R_JDG)
c.exp(rec("at", "x1", "nomark", "/markers/0/pattern"), "TESTS PASSED", R_SCH)
judge(c, "at", "x1", "nomark", "incomplete", False, i, 1)
add(c)

c = Case("A-04", "authoring", "model_child_usage", "synthetic new session with one Agent child")
c.file("{FIX}/s.jsonl", fixture="fx-20.jsonl")
i = c.inv(run("at", "x2", "agent", [CAT, "{FIX}/s.jsonl"], "--model-request", "--session", "new", "--marker", "SYN20-FINAL"))
s = model_stream_expect(c, "at", "x2", "agent", "fx-20.jsonl", "new", run_tokens=T(4, 45, 1300, 250), attribution="consistent", judge_status="succeeded", inv=i, cli=0)
c.exp(rec("at", "x2", "agent", "/usage/children/0/tool_use_id"), "toolu_syn20A", R_USE + " child per Agent tool_use")
c.exp(rec("at", "x2", "agent", "/usage/children/0/agent_id"), "agsyn20", R_USE)
c.exp(rec("at", "x2", "agent", "/usage/children/0/usage/state"), "measured", R_USE)
c.exp(rec("at", "x2", "agent", "/usage/children/0/usage/tokens"), T(1, 5, 300, 50), R_USE + " tool_use_result.usage; parent(3,40,1000,200)+child(1,5,300,50)=session(4,45,1300,250)")
c.exp(rec("at", "x2", "agent", "/usage/children"), {"$len": 1}, R_USE)
add(c)

c = Case("A-05", "authoring", "model_resume_share", "R1 new -> R2 resume with correct base, summarize")
c.file("{FIX}/r1.jsonl", fixture="fx-10.jsonl").file("{FIX}/r2.jsonl", fixture="fx-11.jsonl")
i0 = c.inv(run("at", "x3", "r1", [CAT, "{FIX}/r1.jsonl"], "--model-request", "--session", "new"))
i1 = c.inv(run("at", "x4", "r2", [CAT, "{FIX}/r2.jsonl"], "--model-request", "--session", "resume", "--resume-base", "{OUT}/at/x3/r1/record.json"))
i2 = c.inv(summ("{OUT}"))
r1, r2 = fx("fx-10.jsonl"), fx("fx-11.jsonl")
model_stream_expect(c, "at", "x3", "r1", "fx-10.jsonl", "new", run_tokens=tk(r1["session"]), attribution="consistent", judge_status="succeeded", inv=i0, cli=0)
model_stream_expect(c, "at", "x4", "r2", "fx-11.jsonl", "resume", run_tokens=O.sub(r2["session"], r1["session"]), attribution="consistent",
                    judge_status="succeeded", inv=i1, cli=0, base_cost=r1["cost"])
c.exp(rec("at", "x4", "r2", "/usage/resume_base/command_id"), "r1", R_SCH + " resume_base ids")
c.exp(rec("at", "x4", "r2", "/usage/resume_base/attempt_id"), "x3", R_SCH)
c.exp(rec("at", "x4", "r2", "/usage/resume_base/session_id"), r1["session_id"], R_SCH)
c.exp(rec("at", "x4", "r2", "/usage/resume_base/session/tokens"), tk(r1["session"]), R_C7)
c.exp(f"summary[{i2}]~completeness", "complete", R_AGG)
c.exp(f"summary[{i2}]~tokens_measured_sum", tk(r2["session"]), R_AGG + " + " + R_REV + "; R1 run + R2 run = R2 cumulative")
add(c)

c = Case("A-06", "authoring", "model_resume_no_base", "resume without base -> run unmeasured")
c.file("{FIX}/n2.jsonl", fixture="fx-04.jsonl")
i = c.inv(run("at", "x5", "n2", [CAT, "{FIX}/n2.jsonl"], "--model-request", "--session", "resume"))
model_stream_expect(c, "at", "x5", "n2", "fx-04.jsonl", "resume", run_state="unmeasured")
c.exp(rec("at", "x5", "n2", "/usage/resume_base"), None, R_C7 + " (no base given)")
c.und(rec("at", "x5", "n2", "/judgment/status"), "whether unmeasured run usage affects judgment is not fixed")
add(c)

c = Case("A-07", "authoring", "model_result_error", "is_error stream with --expect result_error")
c.file("{FIX}/e.jsonl", fixture="fx-12.jsonl")
i = c.inv(run("at", "x6", "err", [CAT, "{FIX}/e.jsonl"], "--model-request", "--session", "new", "--expect", "result_error"))
model_stream_expect(c, "at", "x6", "err", "fx-12.jsonl", "new", run_tokens=T(0, 0, 0, 0), judge_status="expected_matched", inv=i, cli=0, check_session=False)
c.exp(rec("at", "x6", "err", "/judgment/expected"), "result_error", R_JDG)
c.und(rec("at", "x6", "err", "/usage/session"), "new session with empty modelUsage: session measure state not fixed (only run share fixed, REPORT §3 P2c para)")
add(c)

c = Case("A-08", "authoring", "model_truncated", "truncated stream with --expect no_result")
c.file("{FIX}/t.jsonl", fixture="fx-21.jsonl")
i = c.inv(run("at", "x7", "cut", [CAT, "{FIX}/t.jsonl"], "--model-request", "--session", "new", "--expect", "no_result"))
model_stream_expect(c, "at", "x7", "cut", "fx-21.jsonl", "new", run_state="unmeasured", judge_status="expected_matched", inv=i, cli=0)
c.exp(rec("at", "x7", "cut", "/usage/partial_observations"), {"$len": 2}, R_USE + " partial_observations per message, not summed")
c.exp(rec("at", "x7", "cut", "/usage/partial_observations/0/message_id"), "msg_t21a", R_USE)
c.exp(rec("at", "x7", "cut", "/usage/partial_observations/0/tokens"), T(5, 17, 900, 120), R_USE)
c.exp(rec("at", "x7", "cut", "/usage/partial_observations/1/tokens"), T(1, 3, 1020, 0), R_USE)
add(c)

SECRET_A = "Authr-secret-Value-2210"
c = Case("A-09", "authoring", "secret_masking", "secret file value masked in stdout and stderr")
c.file("{TMP}/sec.txt", text=SECRET_A)
i = c.inv(run("at", "x8", "sec", [SH, "-c", "v=$(cat {TMP}/sec.txt); echo \"x=$v\"; echo \"y=$v\" >&2; echo plain-line"], "--secret-file", "{TMP}/sec.txt", "--env", "PATH"))
c.exp(rec("at", "x8", "sec", "/redaction/stdout_replacements"), 1, R_SEC + "; one occurrence in stdout")
c.exp(rec("at", "x8", "sec", "/redaction/stderr_replacements"), 1, R_SEC)
c.exp(rec("at", "x8", "sec", "/redaction/argv_replacements"), 0, R_SEC + "; value not in argv")
c.exp(rec("at", "x8", "sec", "/redaction/secret_value_seen"), True, R_SEC)
c.exp("tree", {"$tree_not_contains": [SECRET_A]}, R_SEC + " (value never in evidence files)")
c.exp("file:at/x8/sec/stdout", {"$contains": "plain-line"}, R_LOG)
judge(c, "at", "x8", "sec", "succeeded", True, i, 0)
c.und(rec("at", "x8", "sec", "/redaction/secret_sources"), "what counts as a source (file, sha form, sk-ant pattern) not fixed")
add(c)

c = Case("A-10", "authoring", "secret_refusal", "secret with quote / non-UTF-8 refused")
c.file("{TMP}/q.txt", text='abc"defghij-1').file("{TMP}/n.txt", b64="cGFzc3dvcmQt/v8tdmFsdWU=")
i0 = c.inv(run("at", "x9", "q", [SH, "-c", "echo hi"], "--secret-file", "{TMP}/q.txt"))
i1 = c.inv(run("at", "x9", "n", [SH, "-c", "echo hi"], "--secret-file", "{TMP}/n.txt"))
c.exp(f"exit[{i0}]", 2, R_P24); c.exp(f"exit[{i1}]", 2, R_P24)
c.exp("exists:at/x9/q", False, R_P24 + " (refused before evidence dir creation)")
c.exp("exists:at/x9/n", False, R_P24)
c.exp(f"cli_stderr[{i0}]", {"$not_contains": 'abc"defghij-1'}, R_P24 + " (value not printed)")
add(c)

c = Case("A-11", "authoring", "env_allowlist", "only allowed env names passed, values not recorded")
c.env(IEO_AUTH_KEEP="keep-val-3391", IEO_AUTH_DROP="drop-val-7710")
i = c.inv(run("at", "y1", "env", [SH, "-c", "printf '%s:%s\\n' \"${IEO_AUTH_KEEP:-none}\" \"${IEO_AUTH_DROP:-none}\""], "--env", "IEO_AUTH_KEEP"))
c.exp(rec("at", "y1", "env", "/env_names"), {"$set_eq": ["IEO_AUTH_KEEP"]}, R_RUN + " (names only)")
stdout_exact(c, "at", "y1", "env", "keep-val-3391:none\n")
c.exp("recordtext:at/y1/env", {"$not_contains": "keep-val-3391"}, R_RUN + " (values not recorded)")
judge(c, "at", "y1", "env", "succeeded", True, i, 0)
add(c)

c = Case("A-12", "authoring", "non_overwrite_and_dup_summary", "same id twice refused; summarize same record twice refused")
i0 = c.inv(run("at", "y2", "same", [SH, "-c", "echo first-run"]))
i1 = c.inv(run("at", "y2", "same", [SH, "-c", "echo second-run"]))
i2 = c.inv(summ("{OUT}/at/y2/same/record.json", "{OUT}/at/y2/same/record.json"))
c.exp(f"exit[{i0}]", 0, R_CLI); c.exp(f"exit[{i1}]", 2, R_ID)
c.exp("file:at/y2/same/stdout", {"$text": "first-run\n"}, R_ID + " (not overwritten)")
c.exp(f"exit[{i2}]", 2, R_FIX + " (duplicate record refused)")
add(c)

c = Case("A-13", "authoring", "detached_holds_pipe", "setsid descendant holds stdout")
i = c.inv(run("at", "y3", "held", [SH, "-c", "echo keep-me-77; setsid sleep 7 & sleep 1; exit 0"], "--env", "PATH", "--marker", r"keep-me-\d+"))
c.exp(rec("at", "y3", "held", "/process/output_incomplete"), True, R_P22)
c.exp(rec("at", "y3", "held", "/stdout/bytes"), 11, R_P22 + " (already-read bytes kept); len('keep-me-77\\n')=11")
c.exp(rec("at", "y3", "held", "/markers/0/found"), True, R_P22)
judge(c, "at", "y3", "held", "incomplete", False, i, 1)
add(c)

c = Case("A-14", "authoring", "model_missing_usage", "result without usage and modelUsage")
c.file("{FIX}/u.jsonl", fixture="fx-22.jsonl")
i0 = c.inv(run("at", "y4", "nouse", [CAT, "{FIX}/u.jsonl"], "--model-request", "--session", "new"))
i1 = c.inv(summ("{OUT}"))
model_stream_expect(c, "at", "y4", "nouse", "fx-22.jsonl", "new", run_state="unmeasured")
c.exp(f"summary[{i1}]~completeness", "incomplete", R_AGG)
c.exp(f"summary[{i1}]~tokens_measured_sum", None, R_AGG + " (no measured run -> null, not 0) + " + R_REV)
c.exp(f"summary[{i1}]", {"$contains": "nouse"}, R_AGG + " (unmeasured runs listed)")
c.und(rec("at", "y4", "nouse", "/judgment/status"), "whether unmeasured usage affects judgment is not fixed")
add(c)

# ===================== VALIDATION =====================
c = Case("V-01", "validation", "success_markers_artifacts", "")
i = c.inv(run("vt", "a01", "c", [SH, "-c", "printf 'hello-1301\\n' > {TMP}/r1.txt; printf 'xyz' > {TMP}/r2.bin; echo RESULT=ok-1301; echo 'phase two done'"],
               "--artifact", "{TMP}/r1.txt", "--artifact", "{TMP}/r2.bin", "--marker", r"RESULT=ok-\d+", "--marker", "phase two done", "--role", "development"))
p = lambda ptr: rec("vt", "a01", "c", ptr)
c.exp(p(""), {"$has_keys": SCHEMA_KEYS}, R_SCH)
c.exp(p("/role"), "development", R_SCH)
c.exp(p("/retry_of"), None, R_SCH + " (no --retry-of)")
c.exp(p("/artifacts/0/bytes"), 11, R_LOG); c.exp(p("/artifacts/0/sha256"), sha("hello-1301\n"), R_LOG)
c.exp(p("/artifacts/1/bytes"), 3, R_LOG); c.exp(p("/artifacts/1/sha256"), sha("xyz"), R_LOG)
c.exp(p("/markers/0/found"), True, R_JDG); c.exp(p("/markers/1/found"), True, R_JDG)
c.exp(p("/process/term_sent"), False, R_RUN); c.exp(p("/process/kill_sent"), False, R_RUN)
c.exp(p("/process/output_incomplete"), False, R_RUN)
stdout_exact(c, "vt", "a01", "c", "RESULT=ok-1301\nphase two done\n")
c.exp(p("/stderr/bytes"), 0, R_LOG)
judge(c, "vt", "a01", "c", "succeeded", True, i, 0)
add(c)

c = Case("V-02", "validation", "nonzero_exit", "")
i = c.inv(run("vt", "a02", "c", [SH, "-c", "echo partial-out; echo 'err-line-42' >&2; exit 42"]))
p = lambda ptr: rec("vt", "a02", "c", ptr)
c.exp(p("/process/exit_code"), 42, R_RUN); c.exp(p("/process/kind"), "exited", R_RUN)
stdout_exact(c, "vt", "a02", "c", "partial-out\n"); stdout_exact(c, "vt", "a02", "c", "err-line-42\n", "stderr")
judge(c, "vt", "a02", "c", "failed", False, i, 1)
add(c)

c = Case("V-03", "validation", "signaled", "")
i = c.inv(run("vt", "a03", "c", [SH, "-c", "echo before-kill; kill -KILL $$"]))
p = lambda ptr: rec("vt", "a03", "c", ptr)
c.exp(p("/process/kind"), "signaled", R_RUN + " / " + R_SCH)
c.exp(p("/process/signal"), 9, R_SCH + "; SIGKILL=9")
stdout_exact(c, "vt", "a03", "c", "before-kill\n")
judge(c, "vt", "a03", "c", "failed", False, i, 1)
c.und(p("/process/exit_code"), "exit_code value for a signaled process (null vs other) not fixed")
add(c)

c = Case("V-04", "validation", "launch_failure", "")
i = c.inv(run("vt", "a04", "c", ["/opt/ieo-absent-dir-9/tool", "--flag"], "--marker", "x"))
c.exp(rec("vt", "a04", "c", "/process/kind"), "launch_failed", R_RUN)
c.exp(rec("vt", "a04", "c", "/process/launch_error"), {"$not_null": True}, R_RUN)
c.exp(rec("vt", "a04", "c", "/process/exit_code"), None, R_SCH + " (no process ran)")
c.exp("exists:vt/a04/c/record.json", True, R_RUN + " (record left on launch failure)")
judge(c, "vt", "a04", "c", "failed", False, i, 1)
add(c)

c = Case("V-05", "validation", "timeout_term_honoured", "")
i = c.inv(run("vt", "a05", "c", ["/bin/sleep", "30"], "--timeout-s", "1", "--grace-s", "5"))
p = lambda ptr: rec("vt", "a05", "c", ptr)
c.exp(p("/process/kind"), "timed_out", R_RUN); c.exp(p("/process/timeout_ms"), 1000, R_SCH)
c.exp(p("/process/term_sent"), True, R_RUN); c.exp(p("/process/kill_sent"), False, R_RUN + " (sleep dies on TERM, group empty)")
c.exp(p("/process/stray_group_killed"), False, R_RUN)
c.exp(p("/duration_ms"), {"$gte": 980, "$lte": 5500}, R_RUN + "; >= timeout, < timeout+grace (+0.5 s slack)")
judge(c, "vt", "a05", "c", "failed", False, i, 1)
add(c)

c = Case("V-06", "validation", "timeout_term_ignored_member", "")
i = c.inv(run("vt", "a06", "c", [SH, "-c", "(trap '' TERM; exec sleep 4723) >/dev/null 2>&1 & sleep 60"], "--env", "PATH", "--timeout-s", "1", "--grace-s", "1", "--expect", "timeout"))
j = c.post("pgrep -f 'sleep 472[3]' >/dev/null")
p = lambda ptr: rec("vt", "a06", "c", ptr)
c.exp(p("/process/kind"), "timed_out", R_P23); c.exp(p("/process/term_sent"), True, R_P23)
c.exp(p("/process/kill_sent"), True, R_P23); c.exp(p("/process/stray_group_killed"), True, R_P23)
c.exp(f"post[{j}]", 1, R_P23 + " (no surviving member; pgrep exit 1 = none)")
judge(c, "vt", "a06", "c", "expected_matched", True, i, 0)
add(c)

c = Case("V-07", "validation", "timeout_term_ignored_leader", "")
i = c.inv(run("vt", "a07", "c", [SH, "-c", "trap '' TERM; sleep 3741; echo after"], "--env", "PATH", "--timeout-s", "1", "--grace-s", "1", "--expect", "timeout"))
j = c.post("pgrep -f 'sleep 374[1]' >/dev/null")
p = lambda ptr: rec("vt", "a07", "c", ptr)
c.exp(p("/process/kind"), "timed_out", R_RUN); c.exp(p("/process/term_sent"), True, R_RUN)
c.exp(p("/process/kill_sent"), True, R_RUN + " (members remain after grace -> KILL)")
c.exp(p("/duration_ms"), {"$gte": 1980}, R_RUN + "; leader dies only at KILL after timeout+grace = 2000 ms (-20 ms)")
c.exp(f"post[{j}]", 1, R_RUN + " / " + R_P23)
judge(c, "vt", "a07", "c", "expected_matched", True, i, 0)
c.und(p("/process/stray_group_killed"), "flag semantics when the leader itself is still alive at KILL not fixed")
add(c)

c = Case("V-08", "validation", "intentional_deny", "")
i = c.inv(run("vt", "a08", "c", [SH, "-c", "if ( : > /proc/version ) 2>/dev/null; then echo WROTE; exit 0; else echo WRITE_DENIED; exit 1; fi"], "--expect", "nonzero_exit", "--marker", "WRITE_DENIED"))
c.exp(rec("vt", "a08", "c", "/process/exit_code"), 1, R_RUN)
c.exp(rec("vt", "a08", "c", "/judgment/expected"), "nonzero_exit", R_JDG)
judge(c, "vt", "a08", "c", "expected_matched", True, i, 0)
add(c)

c = Case("V-09", "validation", "intentional_deny_not_matched", "")
i0 = c.inv(run("vt", "a09", "c1", [SH, "-c", "echo fine; exit 0"], "--expect", "nonzero_exit"))
i1 = c.inv(run("vt", "a09", "c2", [SH, "-c", "echo other-failure; exit 2"], "--expect", "nonzero_exit", "--marker", "ABORTED_BY_POLICY"))
judge(c, "vt", "a09", "c1", "expected_not_matched", False, i0, 1)
judge(c, "vt", "a09", "c2", "expected_not_matched", False, i1, 1)
c.exp(rec("vt", "a09", "c2", "/markers/0/found"), False, R_JDG)
add(c)

c = Case("V-10", "validation", "missing_evidence", "")
i = c.inv(run("vt", "a10", "c", [SH, "-c", "echo something-else"], "--artifact", "{TMP}/never.out", "--marker", r"NEVER_PRINTED_\w+"))
p = lambda ptr: rec("vt", "a10", "c", ptr)
c.exp(p("/artifacts/0/present"), False, R_LOG); c.exp(p("/artifacts/0/sha256"), None, R_SCH)
c.exp(p("/markers/0/found"), False, R_JDG)
c.exp(p("/judgment/reasons"), {"$len_gte": 2}, R_JDG + " (all reasons listed; REPORT §2 '사유 2개 모두')")
judge(c, "vt", "a10", "c", "incomplete", False, i, 1)
c.und(p("/judgment/reasons"), "wording of reasons not fixed (only count lower bound checked)")
add(c)

c = Case("V-11", "validation", "marker_stdout_only", "")
i = c.inv(run("vt", "a11", "c", [SH, "-c", "echo ONLY-ON-ERR-5 >&2; echo plain"], "--marker", "ONLY-ON-ERR-5"))
c.exp(rec("vt", "a11", "c", "/markers/0/found"), False, R_MRK)
judge(c, "vt", "a11", "c", "incomplete", False, i, 1)
add(c)

c = Case("V-12", "validation", "model_child_usage", "")
c.file("{FIX}/t.jsonl", fixture="fx-02.jsonl")
i = c.inv(run("vt", "a12", "c", [CAT, "{FIX}/t.jsonl"], "--model-request", "--session", "new", "--marker", "ECHO-OK"))
s = fx("fx-02.jsonl")
model_stream_expect(c, "vt", "a12", "c", "fx-02.jsonl", "new", run_tokens=tk(s["session"]), attribution="consistent", judge_status="succeeded", inv=i, cli=0)
c.exp(rec("vt", "a12", "c", "/usage/children/0/tool_use_id"), "toolu_013tQgWfRyafKrqQipk3EsZk", R_USE)
c.exp(rec("vt", "a12", "c", "/usage/children/0/agent_id"), "af06f3de7793d2534", R_USE)
c.exp(rec("vt", "a12", "c", "/usage/children/0/usage/tokens"), T(2, 9, 0, 844), R_USE + " + " + R_C7 + "; parent(4,157,7907,2886)+child(2,9,0,844)=modelUsage(6,166,7907,3730)")
c.exp(rec("vt", "a12", "c", "/stdout/sha256"), fsha("fx-02.jsonl"), R_LOG)
c.exp(rec("vt", "a12", "c", "/stdout/bytes"), fsize("fx-02.jsonl"), R_LOG)
add(c)

# chain N1 -> N2 -> N3 -> N4a -> L2 -> L3 ; summarize
c = Case("V-13", "validation", "model_resume_chain_compaction", "")
names = [("n1", "fx-03.jsonl"), ("n2", "fx-04.jsonl"), ("n3", "fx-05.jsonl"), ("n4a", "fx-06.jsonl"), ("l2", "fx-08.jsonl"), ("l3", "fx-09.jsonl")]
prev = None; invs = []
for k, (cc, f) in enumerate(names):
    c.file("{FIX}/%s.jsonl" % cc, fixture=f)
    opts = ["--model-request", "--session", "new"] if prev is None else ["--model-request", "--session", "resume", "--resume-base", "{OUT}/vt/a13-%d/%s/record.json" % (k - 1, prev)]
    invs.append(c.inv(run("vt", "a13-%d" % k, cc, [CAT, "{FIX}/%s.jsonl" % cc], *opts)))
    prev = cc
isum = c.inv(summ("{OUT}"))
S = [fx(f) for _, f in names]
for k, (cc, f) in enumerate(names):
    base = S[k - 1] if k else None
    runt = tk(S[k]["session"]) if k == 0 else O.sub(S[k]["session"], base["session"])
    attrib = "mismatch" if cc == "l3" else "consistent"
    model_stream_expect(c, "vt", "a13-%d" % k, cc, f, "new" if k == 0 else "resume", run_tokens=runt, attribution=attrib,
                        judge_status="succeeded", inv=invs[k], cli=0, base_cost=base["cost"] if base else None)
c.d["basis"][rec("vt", "a13-5", "l3", "/usage/run/tokens")] += "; L3 session(2042,1867,26400,4126)-L2 session(22,513,23639,3107)=(2020,1354,2761,1019) (REPORT §3 compaction usage)"
c.d["basis"][rec("vt", "a13-5", "l3", "/usage/attribution_check")] += " (compaction: run != parent 0 + no child -> mismatch, kept measured)"
c.exp(f"summary[{isum}]~completeness", "complete", R_AGG)
c.exp(f"summary[{isum}]~tokens_measured_sum", tk(S[-1]["session"]), R_AGG + " + " + R_REV + "; sum of run shares = final cumulative (2042,1867,26400,4126)")
add(c)

c = Case("V-14", "validation", "model_wrong_base", "")
names = [("n1", "fx-03.jsonl"), ("n2", "fx-04.jsonl"), ("n3", "fx-05.jsonl"), ("n4a", "fx-06.jsonl"), ("n4b", "fx-07.jsonl"), ("l2", "fx-08.jsonl")]
prev = None; invs = []
for k, (cc, f) in enumerate(names):
    c.file("{FIX}/%s.jsonl" % cc, fixture=f)
    opts = ["--model-request", "--session", "new"] if prev is None else ["--model-request", "--session", "resume", "--resume-base", "{OUT}/vt/a14-%d/%s/record.json" % (k - 1, prev)]
    invs.append(c.inv(run("vt", "a14-%d" % k, cc, [CAT, "{FIX}/%s.jsonl" % cc], *opts)))
    prev = cc
isum = c.inv(summ("{OUT}"))
S = [fx(f) for _, f in names]
model_stream_expect(c, "vt", "a14-4", "n4b", "fx-07.jsonl", "resume", run_state="unmeasured")
c.d["basis"][rec("vt", "a14-4", "n4b", "/usage/run/state")] += "; N4b session (14,171,10275,1615) < base N4a (18,501,20878,2065) -> decreasing -> unmeasured"
model_stream_expect(c, "vt", "a14-5", "l2", "fx-08.jsonl", "resume", run_state="unmeasured")
c.d["basis"][rec("vt", "a14-5", "l2", "/usage/run/state")] += "; L2-N4b=(8,342,13364,1492) != parent (4,12,2761,1042), no compaction -> withdrawn (REPORT §3)"
c.exp(rec("vt", "a14-3", "n4a", "/usage/run/tokens"), O.sub(S[3]["session"], S[2]["session"]), R_C7 + "; (8,342,13364,1528)")
c.exp(f"summary[{isum}]~completeness", "incomplete", R_AGG)
c.exp(f"summary[{isum}]~tokens_measured_sum", tk(S[3]["session"]), R_AGG + " + " + R_REV + "; measured runs n1..n4a sum = N4a cumulative (18,501,20878,2065)")
c.exp(f"summary[{isum}]", {"$contains_all": ["n4b", "a14-5"]}, R_AGG + " (unmeasured runs listed: n4b, and l2 of attempt a14-5)")
for cc, a in [("n4b", "a14-4"), ("l2", "a14-5")]:
    c.und(rec("vt", a, cc, "/usage/resume_base"), "whether a rejected base is still echoed in resume_base not fixed")
    c.und(rec("vt", a, cc, "/judgment/status"), "whether unmeasured run usage affects judgment is not fixed")
add(c)

c = Case("V-15", "validation", "model_resume_branch_base", "")
names = [("n1", "fx-03.jsonl"), ("n2", "fx-04.jsonl"), ("n3", "fx-05.jsonl"), ("n4b", "fx-07.jsonl")]
prev = None; invs = []
for k, (cc, f) in enumerate(names):
    c.file("{FIX}/%s.jsonl" % cc, fixture=f)
    opts = ["--model-request", "--session", "new"] if prev is None else ["--model-request", "--session", "resume", "--resume-base", "{OUT}/vt/a15-%d/%s/record.json" % (k - 1, prev)]
    invs.append(c.inv(run("vt", "a15-%d" % k, cc, [CAT, "{FIX}/%s.jsonl" % cc], *opts)))
    prev = cc
S = [fx(f) for _, f in names]
model_stream_expect(c, "vt", "a15-3", "n4b", "fx-07.jsonl", "resume", run_tokens=O.sub(S[3]["session"], S[2]["session"]), attribution="consistent",
                    judge_status="succeeded", inv=invs[3], cli=0, base_cost=S[2]["cost"])
c.d["basis"][rec("vt", "a15-3", "n4b", "/result_event/count")] += " (two result events; selected = the one with num_turns 1)"
add(c)

c = Case("V-16", "validation", "model_different_session_base", "")
c.file("{FIX}/a.jsonl", fixture="fx-03.jsonl").file("{FIX}/b.jsonl", fixture="fx-11.jsonl")
c.inv(run("vt", "a16-0", "base", [CAT, "{FIX}/a.jsonl"], "--model-request", "--session", "new"))
c.inv(run("vt", "a16-1", "next", [CAT, "{FIX}/b.jsonl"], "--model-request", "--session", "resume", "--resume-base", "{OUT}/vt/a16-0/base/record.json"))
model_stream_expect(c, "vt", "a16-1", "next", "fx-11.jsonl", "resume", run_state="unmeasured")
c.d["basis"][rec("vt", "a16-1", "next", "/usage/run/state")] += "; base session_id differs (REPORT §2 '다른 세션 기준 -> 미계측')"
c.und(rec("vt", "a16-1", "next", "/usage/resume_base"), "whether a rejected base is still echoed not fixed")
add(c)

c = Case("V-17", "validation", "model_result_error", "")
c.file("{FIX}/e.jsonl", fixture="fx-12.jsonl")
i = c.inv(run("vt", "a17", "c", [CAT, "{FIX}/e.jsonl"], "--model-request", "--session", "new"))
model_stream_expect(c, "vt", "a17", "c", "fx-12.jsonl", "new", run_tokens=T(0, 0, 0, 0), judge_status="failed", inv=i, cli=1, check_session=False)
c.d["basis"][rec("vt", "a17", "c", "/usage/run/tokens")] += "; new session, modelUsage empty, result.usage explicit zeros -> measured 0 (REPORT §3 P2c para)"
c.und(rec("vt", "a17", "c", "/usage/session"), "session measure for empty modelUsage in new session not fixed")
add(c)

c = Case("V-18", "validation", "model_result_error_resume", "")
c.file("{FIX}/b.jsonl", fixture="fx-13.jsonl").file("{FIX}/e.jsonl", fixture="fx-12.jsonl")
c.inv(run("vt", "a18-0", "base", [CAT, "{FIX}/b.jsonl"], "--model-request", "--session", "new"))
i = c.inv(run("vt", "a18-1", "err", [CAT, "{FIX}/e.jsonl"], "--model-request", "--session", "resume", "--resume-base", "{OUT}/vt/a18-0/base/record.json", "--expect", "result_error"))
model_stream_expect(c, "vt", "a18-1", "err", "fx-12.jsonl", "resume", run_state="unmeasured", check_session=False, judge_status="expected_matched", inv=i, cli=0)
c.exp(rec("vt", "a18-1", "err", "/usage/session/state"), "unmeasured", R_R3 + " P2c para + review-pr21-2df3c35 P2-1 (run and session unmeasured)")
add(c)

c = Case("V-19", "validation", "model_truncated", "")
c.file("{FIX}/t.jsonl", fixture="fx-23.jsonl")
i0 = c.inv(run("vt", "a19", "c1", [CAT, "{FIX}/t.jsonl"], "--model-request", "--session", "new"))
i1 = c.inv(run("vt", "a19", "c2", [CAT, "{FIX}/t.jsonl"], "--model-request", "--session", "new", "--expect", "no_result"))
for cc, st, inv, cli in [("c1", "incomplete", i0, 1), ("c2", "expected_matched", i1, 0)]:
    model_stream_expect(c, "vt", "a19", cc, "fx-23.jsonl", "new", run_state="unmeasured", judge_status=st, inv=inv, cli=cli)
c.exp(rec("vt", "a19", "c1", "/usage/partial_observations"), {"$len": 3}, R_USE)
c.exp(rec("vt", "a19", "c1", "/usage/partial_observations/1/message_id"), "msg_t23b", R_USE)
c.exp(rec("vt", "a19", "c1", "/usage/partial_observations/2/tokens"), T(1, 2, 870, 9), R_USE)
c.exp(rec("vt", "a19", "c1", "/usage/partial_observations/0/parent_tool_use_id"), None, R_USE)
add(c)

c = Case("V-20", "validation", "model_missing_usage_fields", "")
c.file("{FIX}/m.jsonl", fixture="fx-24.jsonl")
i0 = c.inv(run("vt", "a20", "c", [CAT, "{FIX}/m.jsonl"], "--model-request", "--session", "new"))
i1 = c.inv(summ("{OUT}/vt/a20/c/record.json"))
model_stream_expect(c, "vt", "a20", "c", "fx-24.jsonl", "new", run_state="unmeasured")
c.exp(f"summary[{i1}]~completeness", "incomplete", R_AGG)
c.exp(f"summary[{i1}]~tokens_measured_sum", None, R_AGG + " + " + R_REV)
c.und(rec("vt", "a20", "c", "/judgment/status"), "whether unmeasured usage affects judgment is not fixed")
add(c)

c = Case("V-21", "validation", "model_rate_limit_and_extra_model", "")
c.file("{FIX}/r.jsonl", fixture="fx-25.jsonl").file("{FIX}/x.jsonl", fixture="fx-27.jsonl")
i0 = c.inv(run("vt", "a21", "rl", [CAT, "{FIX}/r.jsonl"], "--model-request", "--session", "new"))
i1 = c.inv(run("vt", "a21", "two", [CAT, "{FIX}/x.jsonl"], "--model-request", "--session", "new"))
model_stream_expect(c, "vt", "a21", "rl", "fx-25.jsonl", "new", run_tokens=T(2, 8, 640, 33), attribution="consistent", judge_status="succeeded", inv=i0, cli=0)
c.exp(rec("vt", "a21", "rl", "/judgment/reasons"), {"$len_gte": 1}, R_JDG + " (non-allowed rate limit signal kept in reasons, non-blocking)")
model_stream_expect(c, "vt", "a21", "two", "fx-27.jsonl", "new", run_tokens=T(12, 41, 1200, 490), attribution="mismatch", judge_status="succeeded", inv=i1, cli=0)
c.d["basis"][rec("vt", "a21", "two", "/usage/run/tokens")] += "; two models summed: (5,30,1200,90)+(7,11,0,400)"
c.d["basis"][rec("vt", "a21", "two", "/usage/attribution_check")] += "; run (12,41,1200,490) != parent (5,30,1200,90) + no children; withdrawal rule applies to resumed runs only"
add(c)

SECRET_V = "Zq7-secret-VALUE-9931"
SHAHEX = sha(SECRET_V)
c = Case("V-22", "validation", "secret_masking", "")
c.file("{TMP}/s.txt", text=SECRET_V)
script = 'printf "a=%s\\nb=%s\\nh=%s\\n" "$1" "$1" "$2"; k="sk-""ant-oat01-Qw3rTy0123456789_ab-CD"; printf "e=%s k=%s\\n" "$1" "$k" >&2; echo tail-ok'
i = c.inv(run("vt", "a22", "c", [SH, "-c", script, "sh", SECRET_V, SHAHEX], "--secret-file", "{TMP}/s.txt", "--marker", "tail-ok"))
p = lambda ptr: rec("vt", "a22", "c", ptr)
c.exp(p("/redaction/stdout_replacements"), 3, R_SEC + "; value x2 + its SHA-256 hex x1 in stdout")
c.exp(p("/redaction/stderr_replacements"), 2, R_SEC + "; value x1 + sk-ant-… form x1 in stderr")
c.exp(p("/redaction/argv_replacements"), 2, R_SEC + "; argv holds value x1 and SHA hex x1 (sk-ant literal split in script)")
c.exp(p("/redaction/secret_value_seen"), True, R_SEC)
c.exp("tree", {"$tree_not_contains": [SECRET_V, SHAHEX, "Qw3rTy0123456789_ab"]}, R_SEC + " (masked before storing; nothing under OUT holds the value)")
c.exp(p("/markers/0/found"), True, R_JDG)
judge(c, "vt", "a22", "c", "succeeded", True, i, 0)
c.und(p("/redaction/secret_sources"), "what counts as a secret source not fixed")
c.und(p("/argv"), "replacement placeholder text not fixed")
add(c)

SECRET_N = "Nl-trail-secret-4417"
c = Case("V-23", "validation", "secret_trailing_newline_and_marker", "")
c.file("{TMP}/s.txt", text=SECRET_N + "\n")
i = c.inv(run("vt", "a23", "c", [SH, "-c", 'echo "token: $1"; echo visible-ok', "sh", SECRET_N], "--secret-file", "{TMP}/s.txt", "--marker", r"Nl-trail-secret-44\d7"))
p = lambda ptr: rec("vt", "a23", "c", ptr)
c.exp(p("/redaction/stdout_replacements"), 1, R_P24 + " (trailing newline trimmed, value masked)")
c.exp(p("/redaction/argv_replacements"), 1, R_SEC)
c.exp(p("/markers/0/found"), False, R_MRK + " (marker equal to secret is not found, intended)")
c.exp("tree", {"$tree_not_contains": [SECRET_N]}, R_SEC)
c.exp("file:vt/a23/c/stdout", {"$contains": "visible-ok"}, R_LOG)
judge(c, "vt", "a23", "c", "incomplete", False, i, 1)
add(c)

c = Case("V-24", "validation", "secret_refusal", "")
c.file("{TMP}/b.txt", text="abcd\\efghij-22").file("{TMP}/s.txt", text="x7Kq2pZ").file("{TMP}/t.txt", text="abc\tdefghij-33")
i0 = c.inv(run("vt", "a24", "bs", [SH, "-c", "echo hi"], "--secret-file", "{TMP}/b.txt"))
i1 = c.inv(run("vt", "a24", "short", [SH, "-c", "echo hi"], "--secret-file", "{TMP}/s.txt"))
i2 = c.inv(run("vt", "a24", "tab", [SH, "-c", "echo hi"], "--secret-file", "{TMP}/t.txt"))
for ii, cc, val in [(i0, "bs", "abcd\\efghij-22"), (i1, "short", "x7Kq2pZ"), (i2, "tab", "abc\tdefghij-33")]:
    c.exp(f"exit[{ii}]", 2, R_P24 + " / " + R_SEC + " (<8 bytes, '\\\\', control char refused)")
    c.exp(f"exists:vt/a24/{cc}", False, R_P24)
    c.exp(f"cli_stderr[{ii}]", {"$not_contains": val}, R_P24)
add(c)

c = Case("V-25", "validation", "env_allowlist", "")
c.env(IEO_T_ALLOWED="allowed-value-5521", IEO_T_SECRETISH="hidden-value-8830")
i = c.inv(run("vt", "a25", "c", [SH, "-c", 'printf "%s|%s|%s\\n" "${IEO_T_ALLOWED:-unset}" "${IEO_T_SECRETISH:-unset}" "${HOME:-unset}"'], "--env", "PATH", "--env", "IEO_T_ALLOWED"))
c.exp(rec("vt", "a25", "c", "/env_names"), {"$set_eq": ["PATH", "IEO_T_ALLOWED"]}, R_RUN)
stdout_exact(c, "vt", "a25", "c", "allowed-value-5521|unset|unset\n")
c.exp("recordtext:vt/a25/c", {"$not_contains": "allowed-value-5521"}, R_RUN + " (values not recorded)")
c.exp("tree", {"$tree_not_contains": ["hidden-value-8830"]}, R_RUN + " (non-allowed variable not passed)")
judge(c, "vt", "a25", "c", "succeeded", True, i, 0)
add(c)

c = Case("V-26", "validation", "non_overwrite", "")
i0 = c.inv(run("vt", "a26", "c", [SH, "-c", "echo original-26; exit 3"]))
i1 = c.inv(run("vt", "a26", "c", [SH, "-c", "echo replaced-26"]))
c.exp(f"exit[{i0}]", 1, R_CLI); c.exp(f"exit[{i1}]", 2, R_ID)
c.exp(rec("vt", "a26", "c", "/process/exit_code"), 3, R_ID + " (first record kept)")
c.exp("file:vt/a26/c/stdout", {"$text": "original-26\n"}, R_ID)
add(c)

c = Case("V-27", "validation", "summarize_mixed_and_duplicates", "")
c.file("{FIX}/e.jsonl", fixture="fx-12.jsonl").file("{FIX}/p.jsonl", fixture="fx-01.jsonl").file("{FIX}/g.jsonl", fixture="fx-14.jsonl").file("{FIX}/t.jsonl", fixture="fx-02.jsonl")
c.inv(run("vs", "a1", "main", [CAT, "{FIX}/e.jsonl"], "--model-request", "--session", "new"))
c.inv(run("vs", "a2", "main", [CAT, "{FIX}/p.jsonl"], "--model-request", "--session", "new", "--retry-of", "a1"))
c.inv(run("vs", "a2", "rev", [CAT, "{FIX}/g.jsonl"], "--model-request", "--session", "new", "--role", "review"))
c.inv(run("vs", "a2", "agent", [CAT, "{FIX}/t.jsonl"], "--model-request", "--session", "new"))
c.inv(run("vs", "a2", "build", [SH, "-c", "echo built"]))
isum = c.inv(summ("{OUT}"))
idup = c.inv(summ("{OUT}", "{OUT}/vs/a2/agent/record.json"))
tot = T(0, 0, 0, 0)
for f in ["fx-01.jsonl", "fx-14.jsonl", "fx-02.jsonl"]:
    tot = O.add(tot, tk(fx(f)["session"]))
c.exp(rec("vs", "a2", "main", "/retry_of"), "a1", R_SCH + " --retry-of")
c.exp(rec("vs", "a2", "rev", "/role"), "review", R_SCH)
c.exp(rec("vs", "a1", "main", "/usage/run/tokens"), T(0, 0, 0, 0), R_R3 + " P2c para (new session explicit zeros)")
c.exp(f"summary[{isum}]~completeness", "complete", R_AGG + " (failed, retry, review, child-bearing runs all measured)")
c.exp(f"summary[{isum}]~tokens_measured_sum", tot, R_AGG + " + " + R_REV + "; 0 + (2,6,649,1900) + (2,9,649,1903) + (6,166,7907,3730) = %s" % tot)
c.exp(f"exit[{idup}]", 2, R_FIX)
add(c)

c = Case("V-28", "validation", "summarize_unmeasured_and_no_model", "")
c.file("{FIX}/n2.jsonl", fixture="fx-04.jsonl").file("{FIX}/r1.jsonl", fixture="fx-10.jsonl")
c.inv(run("vu", "a1", "loose-resume", [CAT, "{FIX}/n2.jsonl"], "--model-request", "--session", "resume"))
c.inv(run("vu", "a1", "fresh", [CAT, "{FIX}/r1.jsonl"], "--model-request", "--session", "new"))
c.inv(run("vn", "a1", "plain1", [SH, "-c", "echo p1"]))
c.inv(run("vn", "a1", "plain2", [SH, "-c", "exit 4"]))
i0 = c.inv(summ("{OUT}/vu"))
i1 = c.inv(summ("{OUT}/vn"))
c.exp(f"summary[{i0}]~completeness", "incomplete", R_AGG)
c.exp(f"summary[{i0}]~tokens_measured_sum", tk(fx("fx-10.jsonl")["session"]), R_AGG + " (sum of measured runs only: R1 (2,612,649,1800))")
c.exp(f"summary[{i0}]", {"$contains": "loose-resume"}, R_AGG + " (unmeasured run listed)")
c.exp(f"summary[{i1}]~completeness", "no_model_runs", R_AGG)
c.exp(f"summary[{i1}]~tokens_measured_sum", None, R_AGG + " + " + R_REV)
add(c)

c = Case("V-29", "validation", "detached_holds_pipe", "")
i = c.inv(run("vt", "a29", "c", [SH, "-c", "echo held-out-29; setsid sleep 8 & sleep 1; exit 0"], "--env", "PATH", "--marker", r"held-out-\d+"))
p = lambda ptr: rec("vt", "a29", "c", ptr)
c.exp(p("/process/output_incomplete"), True, R_P22)
c.exp(p("/process/exit_code"), 0, R_RUN)
c.exp(p("/stdout/bytes"), 12, R_P22 + "; len('held-out-29\\n')")
c.exp(p("/markers/0/found"), True, R_P22)
judge(c, "vt", "a29", "c", "incomplete", False, i, 1)
add(c)

c = Case("V-30", "validation", "leftover_member_drain", "")
i0 = c.inv(run("vt", "a30", "drain", [SH, "-c", "(sleep 0.4; echo late-line-81) & echo early-81"], "--env", "PATH"))
i1 = c.inv(run("vt", "a30", "linger", [SH, "-c", "(sleep 6; echo never-62) & echo early-62"], "--env", "PATH"))
i2 = c.inv(run("vt", "a30", "quiet", [SH, "-c", "sleep 6 >/dev/null 2>&1 & echo ok-63"], "--env", "PATH"))
stdout_exact(c, "vt", "a30", "drain", "early-81\nlate-line-81\n")
c.exp(rec("vt", "a30", "drain", "/process/output_incomplete"), False, R_FIX)
c.exp(rec("vt", "a30", "drain", "/process/stray_group_killed"), False, R_FIX + " (member ends inside 2 s window, not killed)")
judge(c, "vt", "a30", "drain", "succeeded", True, i0, 0)
c.exp(rec("vt", "a30", "linger", "/process/output_incomplete"), True, R_FIX + " (killed while holding pipes)")
c.exp(rec("vt", "a30", "linger", "/process/stray_group_killed"), True, R_RUN + " + " + R_FIX)
c.exp("file:vt/a30/linger/stdout", {"$text": "early-62\n"}, R_FIX)
judge(c, "vt", "a30", "linger", "incomplete", False, i1, 1)
c.exp(rec("vt", "a30", "quiet", "/process/output_incomplete"), False, R_FIX + " (pipes not held)")
c.exp(rec("vt", "a30", "quiet", "/process/stray_group_killed"), True, R_RUN + " (leftover member killed after window, recorded)")
c.exp(rec("vt", "a30", "quiet", "/judgment/reasons"), {"$len_gte": 1}, R_RUN + " (listed in reasons, non-blocking)")
judge(c, "vt", "a30", "quiet", "succeeded", True, i2, 0)
add(c)

c = Case("V-31", "validation", "shared_output_deadline", "")
i = c.inv(run("vt", "a31", "c", [SH, "-c", "setsid /bin/sh -c 'sleep 0.5; echo late-err-74 >&2' & echo out-74"], "--env", "PATH"))
stdout_exact(c, "vt", "a31", "c", "out-74\n"); stdout_exact(c, "vt", "a31", "c", "late-err-74\n", "stderr")
c.exp(rec("vt", "a31", "c", "/process/output_incomplete"), False, R_FIX + " (stdout and stderr share one 2 s deadline) + review-pr21-273907b P3-2")
judge(c, "vt", "a31", "c", "succeeded", True, i, 0)
add(c)

c = Case("V-32", "validation", "request_errors", "")
c.file("{FIX}/p.jsonl", fixture="fx-01.jsonl")
i0 = c.inv(run("vt", "a32", "nosess", [CAT, "{FIX}/p.jsonl"], "--model-request"))
c.exp(f"exit[{i0}]", 2, "REPORT §4 row 실제 프로세스 ('세션 방식 없는 모델 요청 거부') + review-pr21-2df3c35 §3.5 (exit 2)")
add(c)

c = Case("V-33", "validation", "undetermined_rules", "")
c.file("{FIX}/m.jsonl", fixture="fx-26.jsonl")
i = c.inv(run("vt", "a33", "c", [CAT, "{FIX}/m.jsonl"], "--model-request", "--session", "new"))
c.exp(rec("vt", "a33", "c", "/result_event/subtype"), "error_max_turns", R_USE)
c.exp(rec("vt", "a33", "c", "/result_event/is_error"), False, R_USE)
c.exp(rec("vt", "a33", "c", "/result_event/num_turns"), 3, R_USE)
c.und(rec("vt", "a33", "c", "/judgment/status"), "subtype != success with is_error=false: open item P3-2 (02-01 ledger, owner 02-02); not fixed by approved rules")
c.und("exit[0]", "follows judgment, which is undetermined (P3-2)")
add(c)

c = Case("V-34", "validation", "undetermined_rules", "")
c.file("{FIX}/t.jsonl", fixture="fx-23.jsonl")
i = c.inv(run("vt", "a34", "c", [SH, "-c", "cat {FIX}/t.jsonl; sleep 30"], "--env", "PATH", "--model-request", "--session", "new", "--timeout-s", "1", "--grace-s", "1", "--expect", "timeout"))
c.exp(rec("vt", "a34", "c", "/process/kind"), "timed_out", R_RUN)
c.exp(rec("vt", "a34", "c", "/process/term_sent"), True, R_RUN)
c.exp(rec("vt", "a34", "c", "/result_event/present"), False, R_USE)
c.und(rec("vt", "a34", "c", "/judgment/status"), "model request with --expect timeout: open item P3-4 (missing result blocking vs timeout expectation) not fixed")
c.und("exit[0]", "follows undetermined judgment (P3-4)")
add(c)

c = Case("V-35", "validation", "undetermined_rules", "")
c.undetermined("natural exit racing the timeout deadline (open item P3-6): which of exited/timed_out is recorded is not fixed and not reproducible deterministically; case not executed")
add(c)

for name, cases in sets.items():
    with open(os.path.join(EVAL, "sets", name + ".json"), "w") as f:
        json.dump({"set": name, "format": "ieo-0202-caseset/v1", "cases": cases}, f, indent=1, ensure_ascii=False)
    print(name, len(cases), sum(len(c["expected"]) for c in cases), "fields")
