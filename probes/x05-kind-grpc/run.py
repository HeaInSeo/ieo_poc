#!/usr/bin/env python3
"""Run and evaluate the frozen X-05 criteria against the dedicated kind cluster."""

import argparse
from collections import Counter
from datetime import datetime, timezone
import json
from pathlib import Path
import queue
import subprocess
import threading
import time
from urllib.parse import urlsplit

CONTEXT = "kind-ieo-x05"
NAMESPACE = "x05-probe"
SELECTOR = "app=x05-grpc-server"
MANIFEST = Path(__file__).with_name("workload.yaml")
FIXTURE_VALUE = "x05-fixture-value"
SAMPLE_COUNT = 300


class TrialFailure(Exception):
    pass


class EnvironmentBlocked(Exception):
    pass


def require(condition, message):
    if not condition:
        raise TrialFailure(message)


class Recorder:
    def __init__(self, path):
        self.path = path
        self.epoch = time.monotonic()
        self.log = (path / "run.jsonl").open("x", encoding="utf-8")

    def emit(self, event, **data):
        entry = {"utc": datetime.now(timezone.utc).isoformat(),
                 "elapsed_s": round(time.monotonic() - self.epoch, 3),
                 "event": event, **data}
        self.log.write(json.dumps(entry) + "\n")
        self.log.flush()
        print(json.dumps(entry), flush=True)

    def snapshot(self, name, data):
        (self.path / f"{name}.json").write_text(json.dumps(data, indent=2) + "\n")


class Client:
    def __init__(self, command, output):
        self.events = []
        self.incoming = queue.Queue()
        self.process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                        stderr=output, text=True, encoding="utf-8")
        self.thread = threading.Thread(target=self.collect, args=(output.name,), daemon=True)
        self.thread.start()

    def collect(self, stderr_path):
        try:
            with Path(stderr_path).with_name("client.jsonl").open("x", encoding="utf-8") as log:
                for line in self.process.stdout:
                    event = json.loads(line)
                    event["received_monotonic"] = time.monotonic()
                    log.write(json.dumps(event) + "\n")
                    log.flush()
                    self.incoming.put(event)
        finally:
            self.incoming.put(None)

    def send(self, command):
        self.process.stdin.write(command + "\n")
        self.process.stdin.flush()

    def wait(self, predicate, deadline, description):
        while True:
            result = predicate(self.events)
            if result is not None:
                return result
            try:
                event = self.incoming.get(timeout=max(0, deadline - time.monotonic()))
            except queue.Empty as error:
                raise TrialFailure(f"timed out: {description}") from error
            require(event is not None, f"client exited while waiting for {description}")
            self.events.append(event)
            require(event["received_monotonic"] <= deadline, f"timed out: {description}")

    def mark(self, name):
        self.send(f"mark {name}")
        return self.wait(lambda events: last_event(events, "mark", name=name),
                         time.monotonic() + 5, name)

    def close(self):
        if self.process.poll() is None:
            try:
                self.send("quit")
                self.process.wait(timeout=3)
            except (BrokenPipeError, subprocess.TimeoutExpired):
                self.process.terminate()
                try:
                    self.process.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    self.process.kill()
                    self.process.wait()
        self.thread.join(timeout=3)
        self.process.stdin.close()
        self.process.stdout.close()


def last_event(events, kind, **data):
    return next((event for event in reversed(events)
                 if event["event"] == kind
                 and all(event["data"].get(key) == value for key, value in data.items())), None)


def samples(events):
    return [event for event in events if event["event"] == "read" and event["data"]["index"] is not None]


def reply(event):
    return event["data"]["outcome"].get("reply", {})


def ready_pods(pods, slices, replicas, revision, workers):
    expected = {}
    if len(pods["items"]) != replicas:
        return None
    for pod in pods["items"]:
        metadata = pod["metadata"]
        status = pod["status"]
        if (metadata.get("deletionTimestamp") or metadata["labels"]["revision"] != revision
                or not any(c["type"] == "Ready" and c["status"] == "True"
                           for c in status.get("conditions", []))):
            return None
        node = pod["spec"]["nodeName"]
        require(node in workers, f"server scheduled outside the three workers: {node}")
        expected[metadata["uid"]] = {
            "pod": metadata["name"], "ip": status["podIP"], "node": node, "revision": revision,
        }
    if len({pod["node"] for pod in expected.values()}) != replicas:
        return None
    endpoints = {
        (endpoint.get("targetRef", {}).get("uid"), address)
        for item in slices["items"] if item["addressType"] == "IPv4"
        for endpoint in item["endpoints"] if endpoint.get("conditions", {}).get("ready") is True
        for address in endpoint["addresses"]
    }
    return expected if endpoints == {(uid, pod["ip"]) for uid, pod in expected.items()} else None


def check_window(window, expected, index):
    ordered = sorted(window, key=lambda event: event["data"]["index"])
    ids = [event["data"]["index"] for event in ordered]
    require(ids == list(range(index, index + SAMPLE_COUNT)), "window sample IDs are incomplete or duplicated")
    issued = [event["data"]["start_ms"] for event in ordered]
    finished = [event["at_ms"] for event in ordered]
    require(29_000 <= issued[-1] - issued[0] <= 31_000, "window issue span outside 29–31 seconds")
    require(max(finished) - issued[0] <= 33_000, "window took longer than 33 seconds")
    counts = Counter()
    for event in ordered:
        data = event["data"]
        response = reply(event)
        pod = expected.get(response.get("uid"))
        require(data["request_id"] == f"sample-{data['index']}" and pod
                and response["request_id"] == data["request_id"]
                and response["pod"] == pod["pod"] and response["revision"] == pod["revision"]
                and response["value"] == FIXTURE_VALUE and response["seq"] == 0,
                f"bad sample {data}")
        counts[response["uid"]] += 1
    for uid in expected:
        proportion = counts[uid] / SAMPLE_COUNT
        require(abs(proportion - 1 / len(expected)) <= 0.10 + 1e-12,
                f"distribution outside ±10 percentage points on {uid}: {dict(counts)}")
    return counts


def stream_items(events, request_id):
    return [event["data"] for event in events if event["event"] == "watch_item"
            and event["data"]["request_id"] == request_id]


def check_stream(items, first_seq, expected, excluded=None):
    require(len(items) >= 5, "fewer than five stream items")
    uid = items[0]["uid"]
    require(uid != excluded and uid in expected, f"unexpected stream server {uid}")
    for seq, item in enumerate(items, first_seq):
        require(item["seq"] == seq and item["uid"] == uid and item["pod"] == expected[uid]["pod"]
                and item["revision"] == expected[uid]["revision"] and item["value"] == FIXTURE_VALUE,
                f"discontinuous stream at {item}, expected sequence {seq} on {uid}")
    return uid


class Trial:
    def __init__(self, kubeconfig, recorder, context=CONTEXT, namespace=NAMESPACE):
        self.command = ["kubectl", "--kubeconfig", str(kubeconfig), "--context", context]
        self.namespace = namespace
        self.recorder = recorder
        self.failures = []

    def kubectl(self, *args, namespace=None, timeout=8):
        process = subprocess.run(self.command + ["--request-timeout", f"{timeout}s", "-n", namespace or self.namespace, *args],
                                 text=True, encoding="utf-8", capture_output=True, timeout=timeout + 2)
        if process.returncode:
            raise RuntimeError(f"kubectl {args}: {process.stderr.strip()}")
        return process.stdout

    def get(self, *args, namespace=None):
        return json.loads(self.kubectl("get", *args, "-o", "json", namespace=namespace))

    def bootstrap(self):
        config = json.loads(self.kubectl("config", "view", "--minify", "-o", "json"))
        if (config["clusters"][0]["name"] != CONTEXT
                or urlsplit(config["clusters"][0]["cluster"]["server"]).hostname != "127.0.0.1"):
            raise EnvironmentBlocked("kubeconfig must contain the dedicated loopback kind context")
        nodes = self.get("nodes")
        self.workers = {node["metadata"]["name"] for node in nodes["items"]
                        if "node-role.kubernetes.io/control-plane" not in node["metadata"]["labels"]}
        actual = {node["metadata"]["name"] for node in nodes["items"]}
        require(actual == {"ieo-x05-control-plane", "ieo-x05-worker", "ieo-x05-worker2", "ieo-x05-worker3"}
                and len(self.workers) == 3, "kubeconfig does not point to the dedicated four-node cluster")
        require(all(any(c["type"] == "Ready" and c["status"] == "True"
                        for c in node["status"]["conditions"]) for node in nodes["items"]), "node is not Ready")
        daemonsets = self.get("daemonsets", "-A")
        cni = [ds for ds in daemonsets["items"] if ds["metadata"]["name"] == "kindnet"
               and ds["metadata"]["namespace"] == "kube-system"]
        require(len(cni) == 1 and cni[0]["status"].get("numberReady") == 4
                and not any("cilium" in ds["metadata"]["name"] for ds in daemonsets["items"]),
                "default kindnet not Ready on every node, or Cilium found")
        self.recorder.snapshot("cluster", {"nodes": nodes, "daemonsets": daemonsets,
                                           "version": json.loads(self.kubectl("version", "-o", "json"))})
        # create rejects a previously used namespace instead of changing its resources.
        self.kubectl("create", "-f", str(MANIFEST), timeout=30)
        self.kubectl("wait", "--for=condition=Ready", "pod/client", "--timeout=120s", timeout=120)
        self.initial_client = self.get("pod", "client")
        self.recorder.snapshot("client-pod", self.initial_client)
        self.recorder.snapshot("service", self.get("service", "probe"))
        self.recorder.snapshot("coredns", self.get("configmap", "coredns", namespace="kube-system"))

    def stable(self, stage, replicas, revision, survives, removed, deadline):
        state = {}
        while time.monotonic() < deadline:
            pods = self.get("pods", "-l", SELECTOR)
            slices = self.get("endpointslices", "-l", "kubernetes.io/service-name=probe")
            deployment = self.get("deployment", "probe")
            expected = ready_pods(pods, slices, replicas, revision, self.workers)
            state = {"pods": pods, "endpointslices": slices, "deployment": deployment}
            status = deployment["status"]
            if (expected and survives <= expected.keys() and removed.isdisjoint(expected)
                    and deployment["metadata"]["generation"] == status.get("observedGeneration")
                    and status.get("updatedReplicas") == replicas
                    and time.monotonic() <= deadline):
                observed = time.monotonic()
                self.recorder.snapshot(f"{stage}-ready", state)
                self.recorder.emit("ready", stage=stage, pods=expected)
                return observed, expected
            time.sleep(0.25)
        self.recorder.snapshot(f"{stage}-last", state)
        raise TrialFailure(f"{stage}: Kubernetes did not converge within 120 seconds")

    def frontend_matches(self, events, expected, deadline):
        return any(event["event"] == "addresses" and
                   set(event["data"]["current"]) == {f"{pod['ip']}:50051" for pod in expected.values()}
                   for event in events if event["received_monotonic"] <= deadline)

    def window(self, stage, observed, expected, boundary=None):
        self.client.wait(
            lambda events: True if (
                {reply(e).get("uid") for e in samples(events)
                 if observed <= e["received_monotonic"] <= observed + 30}
                >= expected.keys()
                and self.frontend_matches(events, expected, observed + 30)
            ) else None,
            observed + 30, f"{stage}: all Ready endpoints serving and matching a DNS snapshot")
        mark = self.client.mark(stage + "-window")
        index = mark["data"]["next_index"]

        def complete(events):
            window = [e for e in samples(events) if index <= e["data"]["index"] < index + SAMPLE_COUNT]
            return window if len({e["data"]["index"] for e in window}) == SAMPLE_COUNT else None

        window = self.client.wait(complete, time.monotonic() + 34, f"{stage}: 300 results")
        counts = Counter(reply(e).get("uid", "error") for e in window)
        self.recorder.emit("window", stage=stage, first_index=index, counts=counts)
        self.check(stage, check_window, window, expected, index)
        if boundary:
            self.check(stage, self.check_gap, boundary, window)
        return window

    def check(self, stage, function, *args):
        try:
            return function(*args)
        except TrialFailure as error:
            self.failures.append(f"{stage}: {error}")
            self.recorder.emit("criterion_failed", stage=stage, detail=str(error))
            return None

    def check_gap(self, boundary, window):
        first_success = min(e["at_ms"] for e in window if reply(e)) if any(map(reply, window)) else None
        require(first_success is not None, "no successful read in the new window")
        tick_events = samples(self.client.events)
        previous = [e["at_ms"] for e in tick_events if reply(e) and e["at_ms"] <= boundary["at_ms"]]
        require(previous, "no read succeeded before mutation")
        start = max(previous)
        successes = sorted([start, first_success]
                           + [e["at_ms"] for e in tick_events if reply(e) and start < e["at_ms"] < first_success])
        gap = max(b - a for a, b in zip(successes, successes[1:]))
        errors = sum(not reply(e) and start <= e["at_ms"] <= first_success for e in tick_events)
        self.recorder.emit("mutation_unary", stage=boundary["data"]["name"].removesuffix("-before"),
                           max_success_gap_ms=gap, errors=errors)
        require(gap <= 10_000, f"unary success gap was {gap} ms")

    def update(self, stage, *command):
        boundary = self.client.mark(stage + "-before")
        self.recorder.emit("mutation", stage=stage, command=command)
        started = time.monotonic()
        self.kubectl(*command)
        return boundary, started

    def delete(self, stage, uid, pods):
        return self.update(stage, "delete", "pod", pods[uid]["pod"], "--grace-period=0", "--force", "--wait=false")

    def replace(self, stage, started, uid, before):
        revision = before[uid]["revision"]
        return self.stable(stage, 3, revision, before.keys() - {uid}, {uid}, started + 120)

    def watch_replacement(self, before):
        request_id, resume_id = "watch-before", "watch-resume"
        self.client.send(f"watch {request_id} 0")

        def five(events, stream_id):
            items = stream_items(events, stream_id)
            return items if len(items) >= 5 else None

        items = self.client.wait(lambda events: five(events, request_id), time.monotonic() + 10,
                                 "five initial stream items")
        uid = check_stream(items, 1, before)
        boundary, started = self.delete("stream-replacement", uid, before)
        end = self.client.wait(lambda events: last_event(events, "watch_end", request_id=request_id),
                               started + 10, "original stream interruption")
        items = stream_items(self.client.events, request_id)
        self.check("stream-replacement", check_stream, items, 1, before)
        last_seq = items[-1]["seq"]
        self.recorder.emit("watch_interrupted", uid=uid, last_seq=last_seq,
                           elapsed_ms=round((end["received_monotonic"] - started) * 1000))
        require(end["received_monotonic"] >= started, "stream ended before deletion")
        self.client.send("close-watch")
        resume_started = time.monotonic()
        self.client.send(f"watch {resume_id} {last_seq}")
        resumed = self.client.wait(lambda events: five(events, resume_id), resume_started + 10,
                                   "five items from explicitly resumed stream")
        # The surviving two servers or the newly started replacement can serve the resume.
        resumed_uid = resumed[0]["uid"]
        self.client.send("close-watch")
        observed, after = self.replace("stream-replacement", started, uid, before)
        self.check("stream-resume", check_stream, resumed, last_seq + 1, after, uid)
        self.recorder.emit("watch_resumed", uid=resumed_uid, first_seq=resumed[0]["seq"])
        self.window("stream-replacement", observed, after, boundary)
        return after

    def started_reads(self, stage, pods, request_id):
        raw = {uid: self.kubectl("logs", pod["pod"], "-c", "probe") for uid, pod in pods.items()}
        self.recorder.snapshot(stage, raw)
        starts = []
        for uid, log in raw.items():
            for line in log.splitlines():
                entry = json.loads(line)
                if entry.get("request_id") == request_id:
                    require(entry["uid"] == uid and entry["event"] == "read_started", "invalid server evidence")
                    starts.append(entry)
        return starts

    def unary_replacement(self, before):
        request_id = "reissue-read-1"
        self.client.send(f"hold {request_id}")
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            starts = self.started_reads("unary-before-delete", before, request_id)
            if starts:
                break
            time.sleep(0.2)
        else:
            raise TrialFailure("held request did not reach a server")
        require(len(starts) == 1 and starts[0]["hold"], "expected one original held read")
        uid = starts[0]["uid"]
        boundary, started = self.delete("unary-replacement", uid, before)
        failed = self.client.wait(lambda events: last_event(events, "read", request_id=request_id, hold=True),
                                  started + 10, "interrupted original unary")
        require(failed["received_monotonic"] >= started
                and failed["data"]["outcome"].get("code") in {"Cancelled", "Internal", "Unavailable", "Unknown"},
                f"expected transport failure after deletion, got {failed}")
        self.recorder.emit("unary_interrupted", uid=uid, result=failed["data"]["outcome"],
                           elapsed_ms=round((failed["received_monotonic"] - started) * 1000))
        observed, after = self.replace("unary-replacement", started, uid, before)
        self.window("unary-replacement", observed, after, boundary)
        require(not self.started_reads("unary-before-reissue", after, request_id), "unary replayed implicitly")
        self.client.send(f"read {request_id}")
        retried = self.client.wait(lambda events: last_event(events, "read", request_id=request_id, hold=False),
                                   time.monotonic() + 3, "explicit unary reissue")
        response = reply(retried)
        require(response.get("uid") in after and response["uid"] != uid
                and response["request_id"] == request_id and response["value"] == FIXTURE_VALUE
                and response["seq"] == 0 and response["pod"] == after[response["uid"]]["pod"]
                and response["revision"] == after[response["uid"]]["revision"]
                and retried["at_ms"] - retried["data"]["start_ms"] <= 2_000, f"wrong reissue result: {retried}")
        starts = self.started_reads("unary-after-reissue", after, request_id)
        require(len(starts) == 1 and not starts[0]["hold"] and starts[0]["uid"] == response["uid"],
                f"expected exactly one explicit reissue: {starts}")
        self.recorder.emit("unary_reissued", uid=response["uid"], request_id=request_id)

    def before_trial(self):
        """Optional control experiment after the environment is ready."""

    def client_command(self):
        return self.command + ["-n", self.namespace, "exec", "-i", "client", "--", "/ieo-x05-probe", "client"]

    def run(self):
        try:
            self.bootstrap()
        except TrialFailure as error:
            raise EnvironmentBlocked(str(error)) from error
        self.before_trial()
        with (self.recorder.path / "client.stderr").open("x", encoding="utf-8") as stderr:
            self.client = Client(self.client_command(), stderr)
            try:
                observed, initial = self.stable("initial", 2, "v1", set(), set(), time.monotonic() + 120)
                self.window("initial", observed, initial)
                boundary, started = self.update("scale", "scale", "deployment/probe", "--replicas=3")
                observed, scaled = self.stable("scale", 3, "v1", initial.keys(), set(), started + 120)
                self.window("scale", observed, scaled, boundary)

                replaced = self.watch_replacement(scaled)
                boundary, started = self.update("rollout", "patch", "deployment/probe", "--type=merge",
                                                "-p", '{"spec":{"template":{"metadata":{"labels":{"revision":"v2"}}}}}')
                observed, rolled = self.stable("rollout", 3, "v2", set(), replaced.keys(), started + 120)
                self.window("rollout", observed, rolled, boundary)
                self.unary_replacement(rolled)
            finally:
                self.client.close()
            final_client = self.get("pod", "client")
            self.recorder.snapshot("client-pod-final", final_client)
            before = self.initial_client["status"]["containerStatuses"][0]
            after = final_client["status"]["containerStatuses"][0]
            require(self.initial_client["metadata"]["uid"] == final_client["metadata"]["uid"]
                    and before["containerID"] == after["containerID"]
                    and before["restartCount"] == after["restartCount"] == 0,
                    "client pod or container changed during the trial")
        return self.failures


def main(trial_type=Trial):
    parser = argparse.ArgumentParser(description=trial_type.__doc__ or __doc__)
    parser.add_argument("--kubeconfig", required=True, type=Path)
    parser.add_argument("--evidence", required=True, type=Path, help="new directory for evidence")
    args = parser.parse_args()
    args.evidence.mkdir()
    recorder = Recorder(args.evidence)
    trial = trial_type(args.kubeconfig, recorder)
    try:
        failures = trial.run()
    except TrialFailure as error:
        recorder.emit("result", verdict="FAIL", failures=[*trial.failures, str(error)])
        return 1
    except (EnvironmentBlocked, OSError, RuntimeError, subprocess.TimeoutExpired) as error:
        verdict = "FAIL" if trial.failures else "INCONCLUSIVE"
        recorder.emit("result", verdict=verdict, failures=trial.failures, blocker=str(error))
        return 1
    else:
        recorder.emit("result", verdict="FAIL" if failures else "PASS", failures=failures)
        return bool(failures)
    finally:
        recorder.log.close()


if __name__ == "__main__":
    raise SystemExit(main())
