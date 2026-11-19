#!/usr/bin/env python3
"""Opt-in Codex App Server stdio adapter for an externally admitted trial."""

import argparse
from contextlib import contextmanager
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import selectors
import stat
import struct
import subprocess
import sys
import tempfile
import time

MAX_FRAME = 4 * 1024 * 1024
RPC_SECONDS = 115
SETTINGS = {
    "forced_login_method": "chatgpt",
    "cli_auth_credentials_store": "file",
    "approval_policy": "never",
    "sandbox_mode": "read-only",
    "web_search": "disabled",
    "features.shell_tool": False,
    "features.plugins": False,
}
COUNTERS = ("inputTokens", "cachedInputTokens", "outputTokens",
            "reasoningOutputTokens", "totalTokens")
PASSIVE = {"thread/started", "thread/status/changed", "account/rateLimits/updated",
           "item/started", "item/completed", "item/agentMessage/delta",
           "item/reasoning/summaryTextDelta", "item/reasoning/textDelta",
           "item/reasoning/summaryPartAdded", "turn/diff/updated", "turn/plan/updated"}
SAFE_ITEMS = {"userMessage", "agentMessage", "reasoning"}


class TrialError(Exception):
    pass


def require(condition, message):
    if not condition:
        raise TrialError(message)


def sync_directory(path):
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def store(path, value):
    with tempfile.NamedTemporaryFile(dir=path.parent, delete=False, mode="w") as file:
        name = file.name
        try:
            json.dump(value, file, separators=(",", ":"))
            file.write("\n")
            file.flush()
            os.fsync(file.fileno())
            os.replace(name, path)
            sync_directory(path.parent)
        finally:
            if os.path.exists(name):
                os.unlink(name)


@contextmanager
def reserve(root, delivery):
    info = root.stat()
    require(info.st_uid == os.getuid() and stat.S_IMODE(info.st_mode) == 0o700
            and stat.S_ISDIR(info.st_mode), "state must be an existing owner-only directory")
    lock = os.open(root / (delivery + ".lock"),
                   os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
    try:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise TrialError("delivery already active; reconcile only") from None
        target = root / delivery
        require(not os.path.lexists(target), "existing delivery intent; reconcile only")
        yield target
    finally:
        os.close(lock)


def immutable_auth(path, expected):
    """Reject writable aliases: the mount pins the inode and fs-verity pins its bytes."""
    require(re.fullmatch(r"[0-9a-f]{64}", expected or ""), "missing assigned credential digest")
    require(sys.platform == "linux" and os.uname().machine in {"x86_64", "aarch64"},
            "immutable credential verification is unavailable")
    require(path.parent.resolve(strict=True) == path.parent, "credential home must be canonical")
    with os.fdopen(os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK), "rb") as file:
        require(stat.S_ISREG(os.fstat(file.fileno()).st_mode), "credential must be a regular file")
        fdinfo = Path(f"/proc/self/fdinfo/{file.fileno()}").read_bytes()
        mount = re.search(rb"^mnt_id:\s+([0-9]+)$", fdinfo, re.MULTILINE)
        require(mount is not None, "credential mount cannot be identified")
        mountinfo = Path("/proc/self/mountinfo").read_bytes().splitlines()
        entry = next((line.split() for line in mountinfo if line.split()[0] == mount[1]), [])
        mount_path = re.sub(rb"\\([0-7]{3})", lambda m: bytes([int(m[1], 8)]), entry[4]) if entry else b""
        require(entry and mount_path == os.fsencode(path) and b"ro" in entry[5].split(b","),
                "credential must be pinned by an exact read-only file mount")
        require(os.statvfs(path.parent).f_flag & os.ST_RDONLY,
                "credential home must be mounted read-only")
        # Linux generic _IOWR('f', 134, struct fsverity_digest); this is an
        # inspection operation over the open inode, never a verity enable operation.
        measurement = bytearray(struct.pack("=HH", 0, 64) + bytes(64))
        try:
            fcntl.ioctl(file, 0xC0046686, measurement, True)
        except OSError:
            raise TrialError("credential inode is not protected by fs-verity") from None
        digest = hashlib.sha256()
        while chunk := file.read(65536):
            digest.update(chunk)
        require(digest.hexdigest() == expected, "assigned credential digest differs")


def launch_guard(args, home):
    codex_home = Path(home["CODEX_HOME"])
    require(not os.path.lexists(codex_home / "config.toml"), "unapproved credential home config")
    immutable_auth(codex_home / "auth.json", args.auth_sha256)
    check_approval(args)


def approval_current(args):
    return int(time.time()) < (args.authorization_expires_unix or 0)


def check_approval(args):
    if not args.enable_trial:
        return
    require(approval_current(args), "trial authorization or billing observation expired")
    pin = args.approval_sha256
    require(args.approval and re.fullmatch(r"[0-9a-f]{64}", pin or ""),
            "missing assigned approval digest")
    require(hashlib.sha256(Path(args.approval).read_bytes()).hexdigest() == pin,
            "assigned approval changed")
    require(approval_current(args), "trial authorization or billing observation expired")


class Pipe:
    def __init__(self, process, deadline):
        self.rx = process.stdout.fileno()
        self.tx = process.stdin.fileno()
        for fd in (self.rx, self.tx):
            os.set_blocking(fd, False)
        self.buffer = bytearray()
        self.deadline = deadline

    def wait(self, fd, event):
        with selectors.DefaultSelector() as poll:
            poll.register(fd, event)
            remaining = self.deadline - time.monotonic()
            require(remaining > 0 and poll.select(remaining), "stdio deadline expired")

    def send(self, message, may_send=lambda: True):
        wire = memoryview(json.dumps(message, separators=(",", ":")).encode() + b"\n")
        require(len(wire) <= MAX_FRAME, "oversize outbound frame")
        while wire:
            self.wait(self.tx, selectors.EVENT_WRITE)
            require(may_send(), "trial authorization or billing observation expired")
            try:
                wire = wire[os.write(self.tx, wire):]
            except BlockingIOError:
                continue

    def receive(self):
        while b"\n" not in self.buffer:
            require(len(self.buffer) <= MAX_FRAME, "oversize inbound frame")
            self.wait(self.rx, selectors.EVENT_READ)
            try:
                chunk = os.read(self.rx, 65536)
            except BlockingIOError:
                continue
            require(bool(chunk), "stdio closed")
            self.buffer.extend(chunk)
        line, _, rest = self.buffer.partition(b"\n")
        self.buffer = rest
        require(len(line) <= MAX_FRAME, "oversize inbound frame")
        try:
            item = json.loads(line)
        except (ValueError, UnicodeError):
            raise TrialError("invalid inbound JSON") from None
        require(isinstance(item, dict), "invalid inbound message")
        require(not ("id" in item and "method" in item), "server request requires external review")
        return item

    def call(self, number, method, params):
        request = {"id": number, "method": method}
        if params is not None:
            request["params"] = params
        self.send(request)
        while True:
            item = self.receive()
            if "id" in item:
                require(item["id"] == number and isinstance(item.get("result"), dict),
                        f"{method} response missing or ambiguous")
                return item["result"]
            name = item.get("method")
            if name == "remoteControl/status/changed":
                require(item.get("params", {}).get("status") == "disabled",
                        "remote control is not disabled")
                continue
            if name == "account/updated" and method == "account/read":
                require(item.get("params", {}).get("authMode") == "chatgpt",
                        "subscription changed during account read")
                continue
            label = name if isinstance(name, str) and re.fullmatch(r"[A-Za-z/._-]{1,80}", name) else "unknown"
            require(name in PASSIVE, f"unexpected notification {label} during {method}")
            require(item["method"] not in {"item/started", "item/completed"},
                    f"unexpected item during {method}")


def verify_subscription(pipe, cwd, model):
    pipe.call(1, "initialize", {"clientInfo": {"name": "ieo-codex-trial", "version": "0.1"}})
    pipe.send({"method": "initialized"})
    config = pipe.call(2, "config/read", {"cwd": cwd, "includeLayers": False})["config"]
    require(all(config.get(key) == SETTINGS[key] for key in
                ("forced_login_method", "approval_policy", "sandbox_mode", "web_search"))
            and all(config.get("features", {}).get(key) is False for key in ("shell_tool", "plugins"))
            and not config.get("mcp_servers") and config.get("model") == model
            and config.get("model_provider") == "openai", "effective configuration differs from trial policy")
    account = pipe.call(3, "account/read", {"refreshToken": False}).get("account")
    require(isinstance(account, dict) and account.get("type") == "chatgpt",
            "ChatGPT subscription identity not confirmed")
    return account


def prepare(pipe, cwd, model):
    verify_subscription(pipe, cwd, model)
    response = pipe.call(4, "thread/start", {"cwd": cwd, "model": model, "modelProvider": "openai",
                         "approvalPolicy": "never", "sandbox": "read-only", "ephemeral": True})
    thread = response["thread"]
    sandbox = response["sandbox"]
    require(isinstance(thread.get("id"), str) and thread["id"]
            and thread.get("turns") == [] and thread.get("ephemeral") is True
            and response.get("cwd") == cwd and response.get("model") == model
            and response.get("modelProvider") == "openai" and response.get("approvalPolicy") == "never"
            and sandbox.get("type") == "readOnly" and sandbox.get("networkAccess", False) is False,
            "thread configuration differs from trial policy")
    return thread["id"]


def counters(value):
    require(isinstance(value, dict) and all(key in value for key in COUNTERS),
            "incomplete usage counters")
    keys = COUNTERS + (("cacheWriteInputTokens",) if "cacheWriteInputTokens" in value else ())
    result = {key: value[key] for key in keys}
    require(all(type(number) is int and number >= 0 for number in result.values()),
            "invalid usage counters")
    return result


def observe(pipe, root, state):
    last = None
    with (root / "events.jsonl").open("x") as events:
        while True:
            item = pipe.receive()
            events.write(json.dumps(item, separators=(",", ":")) + "\n")
            events.flush()
            os.fsync(events.fileno())
            if "id" in item:
                require(item["id"] == state["rpc_id"] and state["turn_id"] is None
                        and isinstance(item.get("result"), dict), "ambiguous turn acknowledgement")
                turn = item["result"]["turn"]
                require(isinstance(turn.get("id"), str) and turn["id"]
                        and turn.get("status") in {"inProgress", "completed"},
                        "invalid turn acknowledgement")
                require(state.get("early_turn_id", turn["id"]) == turn["id"],
                        "early turn conflicts with acknowledgement")
                state["turn_id"] = turn["id"]
                store(root / "state.json", state)
                continue

            method = item.get("method")
            params = item.get("params")
            require(isinstance(params, dict), "invalid notification")
            if method in {"turn/started", "turn/completed"}:
                require(params.get("threadId") == state["thread_id"], "foreign thread")
                turn = params["turn"]
                if method == "turn/started" and state["turn_id"] is None:
                    # An early notification cannot bind the RPC. Check it against the ACK later.
                    early = state.setdefault("early_turn_id", turn["id"])
                    require(early == turn["id"], "conflicting early turn")
                    continue
                require(state["turn_id"] is not None and turn.get("id") == state["turn_id"],
                        "unbound or foreign turn")
                if method == "turn/completed":
                    require(turn.get("status") == "completed" and not turn.get("error"),
                            "turn did not complete successfully")
                    require(isinstance(turn.get("items"), list)
                            and all(i.get("type") in SAFE_ITEMS for i in turn["items"]),
                            "completed turn contains unexpected items")
                    return turn, last
            elif method == "thread/tokenUsage/updated":
                require(params.get("threadId") == state["thread_id"]
                        and state["turn_id"] is not None and params.get("turnId") == state["turn_id"],
                        "unbound or foreign usage")
                current = counters(params["tokenUsage"]["total"])
                require(last is None or (current.keys() == last.keys()
                        and all(current[k] >= last[k] for k in current)), "conflicting usage counters")
                last = current
            else:
                require(method in PASSIVE, "unsupported notification during trial")
                if method in {"item/started", "item/completed"}:
                    require(params.get("threadId") == state["thread_id"]
                            and state["turn_id"] is not None
                            and params.get("turnId") == state["turn_id"]
                            and params["item"].get("type") in SAFE_ITEMS, "unbound or disallowed item")


def trial(process, directory, delivery, attempt, cwd, model, prompt, deadline, args, home):
    pipe = Pipe(process, deadline)
    thread = prepare(pipe, cwd, model)
    state = {"delivery_id": delivery, "attempt_id": attempt, "thread_id": thread,
             "turn_id": None, "rpc_id": "turn-" + delivery, "status": "intent",
             "prompt_sha256": hashlib.sha256(prompt.encode()).hexdigest()}
    directory.mkdir(mode=0o700)
    sync_directory(directory.parent)
    store(directory / "state.json", state)
    try:
        launch_guard(args, home)

        def current():
            check_approval(args)
            return True

        pipe.send({"id": state["rpc_id"], "method": "turn/start", "params": {
            "threadId": thread, "clientUserMessageId": delivery,
            "input": [{"type": "text", "text": prompt}],
            "approvalPolicy": "never", "sandboxPolicy": {"type": "readOnly", "networkAccess": False},
        }}, current)
        turn, observed = observe(pipe, directory, state)
        receipt = {"type": "ieo_codex_trial_receipt", "delivery_id": delivery, "attempt_id": attempt,
                   "thread_id": thread, "turn_id": state["turn_id"], "result": turn, "auth": "chatgpt",
                   "observed_usage": observed}
        # Snapshots do not establish final/authoritative usage. Never put them in the receipt.
        state.update(status="completed_observed", usage_final=False, observed_total=observed, receipt=receipt)
        store(directory / "state.json", state)
        return receipt
    except (TrialError, OSError, KeyError, TypeError, AttributeError):
        state.update(status="unknown", usage_final=False)
        store(directory / "state.json", state)
        raise


def prompt_from_stdin(deadline):
    fd = sys.stdin.fileno()
    os.set_blocking(fd, False)
    data = bytearray()
    with selectors.DefaultSelector() as poll:
        poll.register(fd, selectors.EVENT_READ)
        while b"\n" not in data:
            remaining = deadline - time.monotonic()
            require(remaining > 0 and poll.select(remaining), "prompt deadline expired")
            try:
                chunk = os.read(fd, min(65536, MAX_FRAME + 1 - len(data)))
            except BlockingIOError:
                continue
            require(bool(chunk), "missing newline-terminated JSON prompt")
            data.extend(chunk)
            require(len(data) <= MAX_FRAME, "oversize prompt")
    line, _, extra = data.partition(b"\n")
    require(not extra.strip(), "expected one JSON prompt line")
    value = json.loads(line)
    if isinstance(value, dict) and set(value) == {"prompt"}:
        value = value["prompt"]
    if isinstance(value, dict) and value.get("type") == "user":
        parts = value["message"]["content"]
        require(isinstance(parts, list) and len(parts) == 1 and parts[0].get("type") == "text",
                "unsupported prompt envelope")
        value = parts[0]["text"]
    require(isinstance(value, str) and bool(value.strip()), "expected one JSON prompt string")
    return value


def stop(process):
    # Keep the child in the launcher's process group so its hard stop can cover
    # both processes even if this adapter has already crashed.
    process.terminate()
    try:
        process.wait(timeout=4)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=1)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("delivery-id", "attempt-id", "cwd", "state", "codex", "model"):
        parser.add_argument("--" + name, required=True)
    parser.add_argument("--auth-sha256")
    parser.add_argument("--approval")
    parser.add_argument("--approval-sha256")
    parser.add_argument("--authorization-expires-unix", type=int)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument("--enable-trial", action="store_true")
    modes.add_argument("--preflight-only", action="store_true")
    args = parser.parse_args()
    os.umask(0o077)
    deadline = time.monotonic() + RPC_SECONDS
    try:
        require(args.enable_trial or args.preflight_only, "trial requires explicit opt-in from the admitted launcher")
        for identifier in (args.delivery_id, args.attempt_id):
            require(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]{0,63}", identifier), "invalid identity")
        require(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]{0,63}", args.model), "invalid pinned model")
        cwd = str(Path(args.cwd).resolve(strict=True))
        require(Path(cwd).is_dir() and Path(args.codex).is_absolute(), "invalid launcher paths")
        home = {k: os.environ.get(k) for k in ("HOME", "CODEX_HOME")}
        require(all(v and Path(v).is_absolute() for v in home.values()), "launcher must set isolated homes")
        env = {**home, "PATH": "/usr/bin:/bin", "LANG": "C.UTF-8"}
        cmd = [args.codex, "app-server", "--strict-config", "--listen", "stdio://"]
        for key, value in {**SETTINGS, "model": args.model, "model_provider": "openai"}.items():
            cmd += ["-c", key + "=" + json.dumps(value)]
        if args.preflight_only:
            if args.auth_sha256 is not None:
                launch_guard(args, home)
            process = subprocess.Popen(cmd, cwd=cwd, env=env, stdin=subprocess.PIPE,
                                       stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
            try:
                pipe = Pipe(process, deadline)
                account = verify_subscription(pipe, cwd, args.model)
                limits = pipe.call(4, "account/rateLimits/read", None)
                print(json.dumps({"account": {k: account.get(k) for k in ("type", "planType")},
                                  "rateLimits": limits.get("rateLimits"),
                                  "rateLimitsByLimitId": limits.get("rateLimitsByLimitId")}))
            finally:
                stop(process)
            return 0
        prompt = prompt_from_stdin(deadline)
        with reserve(Path(args.state), args.delivery_id) as directory:
            launch_guard(args, home)
            process = subprocess.Popen(cmd, cwd=cwd, env=env, stdin=subprocess.PIPE,
                                       stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
            try:
                receipt = trial(process, directory, args.delivery_id, args.attempt_id,
                                cwd, args.model, prompt, deadline, args, home)
            finally:
                stop(process)
        print(json.dumps(receipt, separators=(",", ":")), flush=True)
        return 0
    except (TrialError, OSError, ValueError, KeyError, TypeError, AttributeError, IndexError, subprocess.TimeoutExpired) as exc:
        # Server payloads and authentication data never enter errors emitted to the caller.
        print(str(exc) if isinstance(exc, TrialError) else f"trial stopped: {type(exc).__name__}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
