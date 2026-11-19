import fcntl
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import textwrap
import time
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
                 "forced_chatgpt_workspace_id": "wrong" if mode == "wrong_workspace" else
                    json.loads(settings.get("forced_chatgpt_workspace_id", "null")),
                 "features": {"shell_tool": mode == "bad_config", "plugins": False}}}
    elif method == "account/read":
        assert params == {"refreshToken": False}
        result = {"account": {"type": "apiKey" if mode == "api" else "chatgpt"}}
    elif method == "account/rateLimits/read":
        result = {"rateLimits":{"primary":{"usedPercent":18}}}
    elif method == "thread/start":
        if mode == "change_approval":
            with open(home + "/approval.json", "w") as file:
                file.write("changed")
        if mode == "change_auth":
            with open(home + "/auth.json", "w") as file:
                file.write("changed")
        if mode == "change_prerequisites":
            with open(home + "/prerequisites.json", "w") as file:
                file.write("changed")
        if mode == "reset_during_thread":
            with open(home + "/clock-offset", "w") as file:
                file.write("90")
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

        self.auth = self.root / "auth.json"
        self.auth.write_text("synthetic credential")
        self.approval = self.root / "approval.json"
        self.prerequisites = self.root / "prerequisites.json"
        self.entry = self.root / "test-entry.py"
        # The mock subprocess can only simulate kernel immutability. It still checks
        # that the adapter uses the approved credential digest at both boundaries.
        self.entry.write_text(
            "import hashlib,os,runpy,sys,time,types\n"
            f"main=runpy.run_path({str(ADAPTER)!r})['main']\n"
            "def fixture_auth(path, expected):\n"
            "    if hashlib.sha256(path.read_bytes()).hexdigest() != expected:\n"
            "        raise main.__globals__['TrialError']('fixture credential changed')\n"
            "main.__globals__['immutable_auth']=fixture_auth\n"
            "offset=os.path.join(os.environ['HOME'], 'clock-offset')\n"
            "main.__globals__['time']=types.SimpleNamespace(monotonic=time.monotonic, "
            "time=lambda: time.time() + (int(open(offset).read()) if os.path.exists(offset) else 0))\n"
            "sys.exit(main())\n")

    def central_fixture(self, mutation=None):
        now = int(time.time())
        scope = {
            "assignment_id": "synthetic-assignment", "generation": 1, "target_sha": "a" * 40,
            "checkpoint_sha256": "b" * 64, "model": "fixture-model", "limit_id": "codex",
            "auth_sha256": hashlib.sha256(self.auth.read_bytes()).hexdigest(),
            "workspace_id": "12345678-1234-1234-1234-123456789012",
            "claude_account_id": "synthetic-claude", "claude_quota_group_id": "synthetic-group",
            "claude_window_id": "synthetic-weekly",
        }
        record = {"evidence_sha256": "c" * 64, "observed_unix": now, "valid_until_unix": now + 600}
        review = {
            "schema": "ieo.codex-trial-prerequisites/v1", "scope": scope.copy(),
            "workspace_credit_spend": {**record, "state": "verified_blocked", "source": "workspace_admin_policy"},
            "concurrent_credit_changes": {**record, "state": "verified_excluded",
                                          "source": "account_wide_enforced_freeze"},
            "claude_quota": {**record, "state": "naturally_exhausted", "source": "operator_usage_screen",
                             "limit_kind": "included_subscription", "extra_usage": "disabled", "reset_unix": now + 180},
        }
        if mutation:
            mutation(review, now)
        self.prerequisites.write_text(json.dumps(review))
        approval = {**scope, "expires_unix": now + 600, "additional_usd": 0, "max_dispatches": 1,
                    "billing": {"verified_unix": now, "auto_reload_disabled": True,
                                "paid_paths_blocked": True, "api_fallback_blocked": True},
                    "prerequisites_sha256": hashlib.sha256(self.prerequisites.read_bytes()).hexdigest()}
        self.approval.write_text(json.dumps(approval))

    def invoke(self, mode, opt_in=True, envelope=False, preflight=False, real_guard=False, expired=False,
               mutation=None, missing_review=False, approved_preflight=False):
        (self.root / "mode").write_text(mode)
        cmd = [sys.executable, "-I", str(ADAPTER if real_guard else self.entry), "--delivery-id", "D-fixture", "--attempt-id", "a1",
               "--cwd", str(self.root), "--state", str(self.state),
               "--codex", str(self.worker), "--model", "fixture-model"]
        if preflight:
            cmd.append("--preflight-only")
        elif opt_in:
            cmd.append("--enable-trial")
        if (opt_in and not preflight) or approved_preflight:
            self.central_fixture(mutation)
            if missing_review:
                self.prerequisites.unlink()
            cmd += ["--prerequisites", str(self.prerequisites), "--auth-sha256", hashlib.sha256(self.auth.read_bytes()).hexdigest(),
                    "--approval", str(self.approval), "--approval-sha256",
                    hashlib.sha256(self.approval.read_bytes()).hexdigest(),
                    "--authorization-expires-unix", str(int(time.time()) + (-1 if expired else 60))]
        value = "VERIFY checkpoint"
        if envelope:
            value = {"type": "user", "message": {"content": [{"type": "text", "text": value}]}}
        return subprocess.run(cmd, input=json.dumps(value) + "\n", text=True, capture_output=True,
                              env={**os.environ, "HOME": str(self.root), "CODEX_HOME": str(self.root),
                                   "OPENAI_API_KEY": "synthetic", "TEST_MODE": "synthetic"}, timeout=8)

    def read_state(self):
        return json.loads((self.state / "D-fixture/state.json").read_text())

    def test_unmounted_credential_refuses_before_child(self):
        result = self.invoke("ok", real_guard=True)
        self.assertIn("exact read-only file mount", result.stderr)
        self.assertFalse((self.root / "worker-launches").exists())

    def test_expired_billing_refuses_before_child(self):
        result = self.invoke("ok", expired=True)
        self.assertIn("billing observation expired", result.stderr)
        self.assertFalse((self.root / "worker-launches").exists())

    def test_credential_or_approval_change_before_turn_refuses(self):
        for mode in ("change_auth", "change_approval", "change_prerequisites", "reset_during_thread"):
            with self.subTest(mode=mode):
                result = self.invoke(mode)
                self.assertNotEqual(result.returncode, 0)
                self.assertNotIn("turn/start", (self.root / "methods").read_text())
                self.assertEqual(self.read_state()["status"], "unknown")
                for path in (self.state / "D-fixture").iterdir():
                    path.unlink()
                (self.state / "D-fixture").rmdir()

    def test_prerequisite_gaps_never_send(self):
        cases = {
            "no workspace credit proof": lambda r, n: r.pop("workspace_credit_spend"),
            "no concurrent freeze": lambda r, n: r.pop("concurrent_credit_changes"),
            "no Claude exhaustion": lambda r, n: r.pop("claude_quota"),
            "stale Claude observation": lambda r, n: r["claude_quota"].update(observed_unix=n-301),
            "Claude still available": lambda r, n: r["claude_quota"].update(state="available"),
            "Claude paid fallback unknown": lambda r, n: r["claude_quota"].update(extra_usage="unknown"),
            "past reset": lambda r, n: r["claude_quota"].update(reset_unix=n-1),
            "start too late to finish before reset": lambda r, n: r["claude_quota"].update(reset_unix=n+120),
            "freeze ends during run": lambda r, n: r["concurrent_credit_changes"].update(valid_until_unix=n+119),
            "workspace changed": lambda r, n: r["scope"].update(workspace_id="other"),
            "Claude window changed": lambda r, n: r["scope"].update(claude_window_id="other"),
        }
        for label, mutation in cases.items():
            with self.subTest(label=label):
                result = self.invoke("ok", mutation=mutation)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse(result.stdout)
                self.assertFalse((self.root / "worker-launches").exists())
        result = self.invoke("ok", missing_review=True)
        self.assertIn("missing or unreadable prerequisite review", result.stderr)
        self.assertFalse((self.root / "worker-launches").exists())

    def test_approved_preflight_proves_current_workspace_without_turn(self):
        result = self.invoke("ok", preflight=True, approved_preflight=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        receipt = json.loads(result.stdout)
        self.assertEqual(receipt["workspace_id"], "12345678-1234-1234-1234-123456789012")
        self.assertGreater(receipt["authorization_expires_unix"], int(time.time()))
        self.assertNotIn("turn/start", (self.root / "methods").read_text())
        self.assertFalse((self.state / "D-fixture").exists())

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
        for mode in ("bad_config", "api", "wrong_workspace"):
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
