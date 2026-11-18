import fcntl
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import textwrap
import unittest

ADAPTER = Path(__file__).with_name("adapter.py")
WORKER = r"""
import json, os, sys

def emit(value):
    print(json.dumps(value), flush=True)

assert sys.argv[1:5] == ["app-server", "--strict-config", "--listen", "stdio://"]
settings = dict(entry.split("=", 1) for entry in sys.argv[6::2])
assert json.loads(settings["forced_login_method"]) == "chatgpt"
assert json.loads(settings["model"]) == "fixture-model"
assert "OPENAI_API_KEY" not in os.environ
assert "TEST_MODE" not in os.environ
home = os.environ["HOME"]
with open(home + "/worker-launches", "a") as trace:
    trace.write("launch\n")
mode = open(home + "/mode").read().strip()
for line in sys.stdin:
    request = json.loads(line)
    method = request["method"]
    params = request.get("params", {})
    id_ = request.get("id")
    trace_path = home + "/methods"
    with open(trace_path, "a") as trace:
        trace.write(method + "\n")
    if method == "initialized":
        continue
    if method == "initialize":
        result = {}
    elif method == "config/read":
        result = {"config": {"forced_login_method": "chatgpt", "sandbox_mode": "read-only",
                 "approval_policy": "never", "web_search": "disabled", "mcp_servers": {},
                 "model": "fixture-model", "model_provider": "openai",
                 "features": {"shell_tool": mode == "bad_config", "plugins": False}}}
    elif method == "account/read":
        assert params == {"refreshToken": False}
        result = {"account": {"type": "apiKey" if mode == "api" else "chatgpt"}}
    elif method == "account/rateLimits/read":
        result = {"rateLimits":{"primary":{"usedPercent":18}}}
    elif method == "thread/start":
        assert params["ephemeral"] and params["sandbox"] == "read-only"
        result = {"thread": {"id": "thread-1", "turns": [], "ephemeral": True},
                  "cwd": params["cwd"], "model": params["model"],
                  "modelProvider": "openai", "approvalPolicy": "never", "sandbox": {"type": "readOnly"}}
    elif method == "turn/start":
        assert request["id"] == "turn-D-fixture"
        assert params == {"threadId": "thread-1", "clientUserMessageId": "D-fixture",
                          "input": [{"type": "text", "text": "VERIFY checkpoint"}],
                          "approvalPolicy": "never", "sandboxPolicy": {"type": "readOnly", "networkAccess": False}}
        path = home + "/state/D-fixture/state.json"
        intent = json.load(open(path))
        assert intent["status"] == "intent" and intent["turn_id"] is None
        if mode == "lost":
            sys.exit(0)
        if mode == "server_request":
            emit({"id": 99, "method": "fixture/tool/request", "params": {}})
            continue
        turn = {"id": "turn-1", "status": "inProgress", "items": []}
        early = {**turn, "id": "foreign" if mode == "early_conflict" else turn["id"]}
        emit({"method": "turn/started", "params": {"threadId": "thread-1", "turn": early}})
        emit({"id": id_, "result": {"turn": turn}})
        usage = {"inputTokens": 10, "cachedInputTokens": 2, "outputTokens": 5,
                 "reasoningOutputTokens": 1, "totalTokens": 15}
        for counts in (usage, {**usage, "totalTokens": 1} if mode == "regression" else usage):
            emit({"method": "thread/tokenUsage/updated", "params": {
                "threadId": "thread-1", "turnId": "foreign" if mode == "foreign" else turn["id"],
                "tokenUsage": {"total": counts, "last": counts}}})
        emit({"method": "turn/completed", "params": {"threadId": "thread-1",
              "turn": {**turn, "status": "failed" if mode == "failed" else "completed"}}})
        continue
    else:
        raise AssertionError(method)
    emit({"id": id_, "result": result})
"""


class Tests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        self.state = self.root / "state"
        self.state.mkdir(mode=0o700)
        self.worker = self.root / "mock"
        self.worker.write_text("#!" + sys.executable + "\n" + textwrap.dedent(WORKER))
        self.worker.chmod(0o700)

    def invoke(self, mode, opt_in=True, envelope=False, preflight=False):
        (self.root / "mode").write_text(mode)
        cmd = [sys.executable, "-I", str(ADAPTER), "--delivery-id", "D-fixture", "--attempt-id", "a1",
               "--cwd", str(self.root), "--state", str(self.state),
               "--codex", str(self.worker), "--model", "fixture-model"]
        if preflight:
            cmd.append("--preflight-only")
        elif opt_in:
            cmd.append("--enable-trial")
        value = "VERIFY checkpoint"
        if envelope:
            value = {"type": "user", "message": {"content": [{"type": "text", "text": value}]}}
        return subprocess.run(cmd, input=json.dumps(value) + "\n", text=True, capture_output=True,
                              env={**os.environ, "HOME": str(self.root), "CODEX_HOME": str(self.root),
                                   "OPENAI_API_KEY": "synthetic", "TEST_MODE": "synthetic"}, timeout=8)

    def read_state(self):
        return json.loads((self.state / "D-fixture/state.json").read_text())

    def test_preflight_reads_account_and_limits_without_reserving_or_starting(self):
        result = self.invoke("ok", preflight=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout)["rateLimits"]["primary"]["usedPercent"], 18)
        self.assertEqual((self.root / "methods").read_text().splitlines(), [
            "initialize", "initialized", "config/read", "account/read", "account/rateLimits/read"])
        self.assertEqual(list(self.state.iterdir()), [])

    def test_receipt_omits_unproven_final_usage_and_retains_raw(self):
        result = self.invoke("ok", envelope=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(len(result.stdout.splitlines()), 1)
        receipt = json.loads(result.stdout)
        self.assertEqual(receipt.pop("observed_usage")["totalTokens"], 15)
        self.assertEqual(receipt, {"type": "ieo_codex_trial_receipt", "delivery_id": "D-fixture",
                         "attempt_id": "a1", "thread_id": "thread-1", "turn_id": "turn-1",
                         "result": {"id": "turn-1", "status": "completed", "items": []}, "auth": "chatgpt"})
        saved = self.read_state()
        self.assertFalse(saved["usage_final"])
        self.assertEqual(saved["observed_total"]["totalTokens"], 15)
        self.assertEqual(len((self.state / "D-fixture/events.jsonl").read_text().splitlines()), 5)
        again = self.invoke("ok")
        self.assertNotEqual(again.returncode, 0)
        self.assertEqual(again.stdout, "")
        self.assertEqual((self.root / "worker-launches").read_text(), "launch\n")
        self.assertEqual((self.root / "methods").read_text().splitlines().count("turn/start"), 1)

    def test_lost_reply_preserves_unknown_and_prevents_second_send(self):
        first = self.invoke("lost")
        self.assertNotEqual(first.returncode, 0)
        self.assertFalse(first.stdout)
        self.assertEqual(self.read_state()["status"], "unknown")
        self.assertIsNone(self.read_state()["turn_id"])
        self.assertIn("existing delivery intent", self.invoke("ok").stderr)
        self.assertEqual((self.root / "methods").read_text().splitlines().count("turn/start"), 1)

    def test_busy_delivery_lock_refuses_without_process(self):
        with (self.state / "D-fixture.lock").open("w") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            result = self.invoke("ok")
            self.assertIn("delivery already active", result.stderr)
            self.assertFalse((self.root / "worker-launches").exists())

    def test_partial_intent_directory_is_conservatively_never_reused(self):
        (self.state / "D-fixture").mkdir(mode=0o700)
        result = self.invoke("ok")
        self.assertIn("existing delivery intent", result.stderr)
        self.assertFalse((self.root / "worker-launches").exists())

    def test_no_opt_in_no_process(self):
        result = self.invoke("ok", opt_in=False)
        self.assertIn("explicit opt-in", result.stderr)
        self.assertFalse((self.root / "worker-launches").exists())

    def test_wrong_config_or_auth_prevents_turn(self):
        for mode in ("bad_config", "api"):
            with self.subTest(mode=mode):
                self.assertNotEqual(self.invoke(mode).returncode, 0)
                self.assertNotIn("turn/start", (self.root / "methods").read_text())
                self.assertFalse((self.state / "D-fixture").exists())

    def test_fail_closed_on_ambiguous_or_failed_turn(self):
        for mode in ("foreign", "regression", "failed", "early_conflict", "server_request"):
            with self.subTest(mode=mode):
                result = self.invoke(mode)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse(result.stdout)
                self.assertEqual(self.read_state()["status"], "unknown")
                for path in (self.state / "D-fixture").iterdir():
                    path.unlink()
                (self.state / "D-fixture").rmdir()


if __name__ == "__main__":
    unittest.main(verbosity=2)
