"""Create frozen fixtures: copies of 01-04 real streams (opaque names) + synthetic stream-json."""
import json, os, shutil, sys
REPO = sys.argv[1]
FX = sys.argv[2]
B = os.path.join(REPO, "docs/poc/reports/tasks/01-04")
REAL = {
    "fx-01.jsonl": "first-probes-20261112/evidence/attempts/01-P1-development",
    "fx-02.jsonl": "remaining-probes-20261112/evidence/attempts/23-T-development",
    "fx-03.jsonl": "remaining-probes-20261112/evidence/attempts/11-N1-development",
    "fx-04.jsonl": "remaining-probes-20261112/evidence/attempts/12-N2-development",
    "fx-05.jsonl": "remaining-probes-20261112/evidence/attempts/13-N3-development",
    "fx-06.jsonl": "remaining-probes-20261112/evidence/attempts/14-N4a-development",
    "fx-07.jsonl": "remaining-probes-20261112/evidence/attempts/15-N4b-development",
    "fx-08.jsonl": "remaining-probes-20261112/evidence/attempts/17-L2-development",
    "fx-09.jsonl": "remaining-probes-20261112/evidence/attempts/18-L3-development",
    "fx-10.jsonl": "remaining-probes-20261112/evidence/attempts/24-R1-development",
    "fx-11.jsonl": "remaining-probes-20261112/evidence/attempts/25-R2-development",
    "fx-12.jsonl": "first-probes-20261112/evidence/attempts/05-P2c-development",
    "fx-13.jsonl": "first-probes-20261112/evidence/attempts/04-P1-guardrail",
    "fx-14.jsonl": "first-probes-20261112/evidence/attempts/02-P1-guardrail",
}
os.makedirs(FX, exist_ok=True)
src_map = {}
for k, v in REAL.items():
    shutil.copyfile(os.path.join(B, v, "stdout.jsonl"), os.path.join(FX, k))
    src_map[k] = "docs/poc/reports/tasks/01-04/" + v + "/stdout.jsonl"

def J(o): return json.dumps(o, separators=(",", ":"))
def init(sid, model="claude-syn-1"):
    return {"type": "system", "subtype": "init", "cwd": "/state/work", "session_id": sid, "tools": ["Agent"], "model": model, "permissionMode": "dontAsk", "claude_code_version": "2.1.322"}
def asst(sid, mid, content, usage, parent=None, model="claude-syn-1"):
    u = None if usage is None else {"input_tokens": usage[0], "output_tokens": usage[1], "cache_read_input_tokens": usage[2], "cache_creation_input_tokens": usage[3]}
    m = {"model": model, "id": mid, "type": "message", "role": "assistant", "content": content, "stop_reason": None}
    if u is not None: m["usage"] = u
    return {"type": "assistant", "message": m, "parent_tool_use_id": parent, "session_id": sid}
def rl(sid, status):
    return {"type": "rate_limit_event", "rate_limit_info": {"status": status, "rateLimitType": "five_hour"}, "session_id": sid}
def usage4(t, drop=None):
    d = {"input_tokens": t[0], "cache_creation_input_tokens": t[3], "cache_read_input_tokens": t[2], "output_tokens": t[1], "service_tier": "standard"}
    if drop: d.pop(drop)
    return d
def mu(model, t, cost, drop=None):
    d = {"inputTokens": t[0], "outputTokens": t[1], "cacheReadInputTokens": t[2], "cacheCreationInputTokens": t[3], "costUSD": cost}
    if drop: d.pop(drop)
    return {model: d}
def result(sid, usage, model_usage, cost, num_turns=1, is_error=False, subtype="success", stop="end_turn"):
    r = {"type": "result", "subtype": subtype, "is_error": is_error, "num_turns": num_turns, "stop_reason": stop, "session_id": sid, "total_cost_usd": cost, "result": "done"}
    if usage is not None: r["usage"] = usage
    if model_usage is not None: r["modelUsage"] = model_usage
    return r
def write(name, events, tail=""):
    with open(os.path.join(FX, name), "w") as f:
        for e in events: f.write(J(e) + "\n")
        f.write(tail)

# fx-20: new session with one Agent child. parent (3,40,1000,200), child (1,5,300,50) -> session (4,45,1300,250)
s = "5a1c0000-0000-4000-8000-000000000020"
write("fx-20.jsonl", [
    init(s), rl(s, "allowed"),
    asst(s, "msg_p20a", [{"type": "tool_use", "id": "toolu_syn20A", "name": "Agent", "input": {"prompt": "hi"}}], (2, 20, 500, 150)),
    {"type": "user", "message": {"role": "user", "content": [{"type": "text", "text": "hi"}]}, "parent_tool_use_id": "toolu_syn20A", "agent_id": "agsyn20", "session_id": s},
    asst(s, "msg_c20a", [{"type": "text", "text": "CHILD-DONE-20"}], (1, 5, 300, 50), parent="toolu_syn20A"),
    {"type": "user", "message": {"role": "user", "content": [{"tool_use_id": "toolu_syn20A", "type": "tool_result", "content": [{"type": "text", "text": "CHILD-DONE-20"}]}]},
     "parent_tool_use_id": None, "session_id": s,
     "tool_use_result": {"status": "completed", "agentId": "agsyn20", "content": [{"type": "text", "text": "CHILD-DONE-20"}], "totalTokens": 356,
                         "usage": usage4((1, 5, 300, 50))}},
    asst(s, "msg_p20b", [{"type": "text", "text": "SYN20-FINAL"}], (1, 20, 500, 50)),
    result(s, usage4((3, 40, 1000, 200)), mu("claude-syn-1", (4, 45, 1300, 250), 0.0042), 0.0042, num_turns=2),
])
# fx-21: truncated (no result), two parent messages + cut line. (authoring)
s = "5a1c0000-0000-4000-8000-000000000021"
write("fx-21.jsonl", [init(s), rl(s, "allowed"),
    asst(s, "msg_t21a", [{"type": "text", "text": "working"}], (5, 17, 900, 120)),
    asst(s, "msg_t21b", [{"type": "text", "text": "still working"}], (1, 3, 1020, 0))],
    tail='{"type":"assistant","message":{"id":"msg_t21c","usage":{"input_tok')
# fx-22: new session, result has neither usage nor modelUsage (authoring)
s = "5a1c0000-0000-4000-8000-000000000022"
write("fx-22.jsonl", [init(s), asst(s, "msg_u22a", [{"type": "text", "text": "OK22"}], (2, 4, 100, 10)),
    result(s, None, None, 0.0011)])
# fx-23: truncated (validation): three messages, last cut, no result
s = "5a1c0000-0000-4000-8000-000000000023"
write("fx-23.jsonl", [init(s), rl(s, "allowed"),
    asst(s, "msg_t23a", [{"type": "text", "text": "a"}], (3, 11, 500, 60)),
    asst(s, "msg_t23b", [{"type": "tool_use", "id": "toolu_t23", "name": "Bash", "input": {"command": "ls"}}], (2, 7, 800, 0)),
    {"type": "user", "message": {"role": "user", "content": [{"tool_use_id": "toolu_t23", "type": "tool_result", "content": "x"}]}, "parent_tool_use_id": None, "session_id": s},
    asst(s, "msg_t23c", [{"type": "text", "text": "b"}], (1, 2, 870, 9))],
    tail='{"type":"result","subtype":"succ')
# fx-24: result usage lacks cache_read_input_tokens; modelUsage lacks outputTokens (validation)
s = "5a1c0000-0000-4000-8000-000000000024"
write("fx-24.jsonl", [init(s), asst(s, "msg_u24a", [{"type": "text", "text": "OK24"}], (3, 9, 700, 30)),
    result(s, usage4((3, 9, 700, 30), drop="cache_read_input_tokens"), mu("claude-syn-1", (3, 9, 700, 30), 0.0021, drop="outputTokens"), 0.0021)])
# fx-25: rate limit warning, otherwise clean success (validation)
s = "5a1c0000-0000-4000-8000-000000000025"
write("fx-25.jsonl", [init(s), rl(s, "allowed_warning"),
    asst(s, "msg_r25a", [{"type": "text", "text": "RL25-ANSWER"}], (2, 8, 640, 33)),
    result(s, usage4((2, 8, 640, 33)), mu("claude-syn-1", (2, 8, 640, 33), 0.0009), 0.0009)])
# fx-26: subtype error_max_turns but is_error false (judgment undetermined, P3-2)
s = "5a1c0000-0000-4000-8000-000000000026"
write("fx-26.jsonl", [init(s), asst(s, "msg_m26a", [{"type": "text", "text": "x"}], (2, 6, 300, 20)),
    result(s, usage4((2, 6, 300, 20)), mu("claude-syn-1", (2, 6, 300, 20), 0.0005), 0.0005, num_turns=3, subtype="error_max_turns", stop=None)])
# fx-27: two models in modelUsage, new session, no child (validation): parent (5,30,1200,90); models A (5,30,1200,90) + B (7,11,0,400)
s = "5a1c0000-0000-4000-8000-000000000027"
mm = mu("claude-syn-1", (5, 30, 1200, 90), 0.003); mm.update(mu("claude-syn-haiku", (7, 11, 0, 400), 0.0004))
write("fx-27.jsonl", [init(s), asst(s, "msg_x27a", [{"type": "text", "text": "TWO-MODELS-27"}], (5, 30, 1200, 90)),
    result(s, usage4((5, 30, 1200, 90)), mm, 0.0034)])
json.dump(src_map, open(os.path.join(FX, "SOURCES.json"), "w"), indent=1)
print("ok")
