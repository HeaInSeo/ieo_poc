#!/usr/bin/env python3
"""Run the fixed P10 matrix inside a fresh 08-02 fixture cluster on its disposable VM."""

import ipaddress
import json
from pathlib import Path
import subprocess
import sys
import time
from urllib.parse import urlsplit
from uuid import uuid4

from judge import MARKER, connection, detection

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "0802-cluster-baseline"))
from manifests import IMAGE, NS, actor_egress, initial, observe

sys.path.insert(0, str(HERE.parent / "0803-observation"))
from evaluate import container_id, evidence_matches, loss_clear, loss_delta

CLUSTER = f"kind-{NS}"
NAMES = {"development", "control", "receiver-development", "receiver-denied"}
CASES = (
    ("open-denied", "development", "receiver-denied", True),
    ("open-allowed", "development", "receiver-development", True),
    ("enforced-allowed", "development", "receiver-development", True),
    ("unapplied-before", "control", "receiver-denied", True),
    ("enforced-denied", "development", "receiver-denied", False),
    ("unapplied-after", "control", "receiver-denied", True),
    ("restored-denied", "development", "receiver-denied", True),
)


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def wait(check, label, seconds=120):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        value = check()
        if value:
            return value
        time.sleep(2)
    raise RuntimeError(f"timed out waiting for {label}")


def run(config, evidence):
    out = evidence / "p10"
    out.mkdir()
    command = ["kubectl", "--kubeconfig", config, "--context", CLUSTER, "--request-timeout=15s"]
    generation = "p10-" + uuid4().hex[:12]
    script = (HERE / "client.py").read_text()

    def capture(*args, data=None, namespace=NS, check=True):
        response = subprocess.run([*command, "-n", namespace, *args], input=data,
                                  capture_output=True, text=True, timeout=30)
        if check:
            require(response.returncode == 0, f"kubectl {args[0]} failed: {response.stderr.strip()}")
        return response

    def save(name, data):
        (out / name).write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")

    def get(*args, namespace=NS):
        return json.loads(capture("get", *args, "-o", "json", namespace=namespace).stdout)

    def apply(items, action="apply"):
        capture(action, "-f", "-", data=json.dumps({"apiVersion": "v1", "kind": "List", "items": items}))

    view = json.loads(capture("config", "view", "--minify", "-o", "json").stdout)
    cluster, = view["clusters"]
    require(cluster["name"] == CLUSTER and urlsplit(cluster["cluster"]["server"]).hostname == "127.0.0.1",
            "requires dedicated loopback kind context")
    nodes = get("nodes")["items"]
    require(len(nodes) == 4 and {node["metadata"]["name"] for node in nodes}
            == {f"{NS}-control-plane", f"{NS}-worker", f"{NS}-worker2", f"{NS}-worker3"},
            "unexpected kind node set")
    cilium = get("daemonset", "cilium", namespace="kube-system")["status"]
    require(cilium.get("numberReady") == cilium.get("desiredNumberScheduled") == 4,
            "pinned CNI isn't ready")
    save("nodes.json", nodes)
    save("cilium-status.json", cilium)

    items = []
    for item in initial():
        name = item["metadata"]["name"]
        kind = item["kind"]
        if kind == "Pod":
            if name not in NAMES:
                continue
            if name in {"development", "control"}:
                labels = item["metadata"]["labels"]
                labels.update({"fixture-repo": "ieo-poc-fixture", "fixture-generation": "1",
                               "fixture-assignment": generation, "fixture-session": generation + "-" + name})
                item["metadata"]["annotations"] = {"ieo-poc/session": labels["fixture-session"]}
        elif kind == "ServiceAccount" and name not in {"development", "control", "receiver"}:
            continue
        elif kind == "NetworkPolicy" and name in {"egress-guardrail", "egress-product"}:
            continue
        elif kind not in {"Namespace", "ServiceAccount", "NetworkPolicy"} and name != "fixture":
            continue
        items.append(item)
    # Exclusive create refuses an occupied namespace before any other object is applied.
    namespace, = [item for item in items if item["kind"] == "Namespace"]
    save("fixture.json", items)
    apply([namespace], "create")
    apply([item for item in items if item is not namespace])
    policy, = observe()
    apply([policy])
    save("observation-policy.json", policy)

    def pods():
        entries = get("pods")["items"]
        observed = {p["metadata"]["name"]: p for p in entries}
        if set(observed) != NAMES:
            return None
        for pod in entries:
            statuses = pod["status"].get("containerStatuses", [])
            if (len(statuses) != 1 or not statuses[0].get("ready") or statuses[0]["restartCount"] != 0
                    or pod["spec"].get("automountServiceAccountToken") is not False
                    or pod["spec"]["containers"][0]["image"] != IMAGE
                    or pod["metadata"]["labels"].get("fixture") != NS):
                return None
            ip = pod["status"].get("podIP")
            if not ip or ipaddress.IPv4Address(ip) not in ipaddress.IPv4Network("10.244.0.0/16"):
                return None
        return observed

    bound = wait(pods, "fixture Pods")
    save("pods-before.json", bound)

    def ready_endpoints():
        endpoints = get("ciliumendpoints")["items"]
        return endpoints if {e["metadata"]["name"] for e in endpoints
                             if e.get("status", {}).get("state") == "ready"} == NAMES else None

    save("cilium-endpoints-before.json", wait(ready_endpoints, "fixture Cilium endpoint readiness"))
    dev_node = bound["development"]["spec"]["nodeName"]

    def current_agent():
        agents = get("pods", "-l", "app.kubernetes.io/component=agent,app.kubernetes.io/name=tetragon",
                     namespace="kube-system")["items"]
        matches = [pod for pod in agents if pod["spec"]["nodeName"] == dev_node
                   and not pod["metadata"].get("deletionTimestamp")
                   and any(c["type"] == "Ready" and c["status"] == "True"
                           for c in pod["status"].get("conditions", []))]
        return matches[0] if len(matches) == 1 else None

    observer = wait(current_agent, "development-node observer")
    name = observer["metadata"]["name"]

    def active_observer():
        result = capture("exec", name, "-c", "tetragon", "--", "tetra", "tracingpolicy", "list",
                         "--domain", "k8s", "-o", "json", namespace="kube-system", check=False)
        if result.returncode:
            return None
        data = json.loads(result.stdout)
        if any(p.get("namespace") == NS and p.get("name") == "observe-fixture"
               and p.get("state") == "TP_STATE_ENABLED" for p in data.get("policies", [])):
            return data
        return None

    def snapshot(tag):
        save(tag + "-policies.json", wait(active_observer, tag + " active observation policy"))
        result = capture("get", "--raw", f"/api/v1/namespaces/kube-system/pods/{name}:2112/proxy/metrics",
                         namespace="kube-system")
        (out / f"{tag}-{dev_node}-metrics.txt").write_text(result.stdout)

    def policy_specs(tag, expected):
        active = {entry["metadata"]["name"]: entry["spec"] for entry in get("networkpolicies")["items"]}
        require(active == expected, f"unexpected active NetworkPolicy in {tag}")
        save(f"{tag}-networkpolicies.json", active)

    last_logs = {}

    def one(index):
        label, source, receiver, allowed = CASES[index]
        actor, target = bound[source], bound[receiver]
        before = capture("logs", receiver).stdout
        require(receiver not in last_logs or last_logs[receiver] == before,
                f"unattributed activity at {receiver} between samples")
        result = capture("exec", "-i", source, "--", "python3", "-I", "-B", "-", MARKER,
                         target["status"]["podIP"], data=script, check=False)
        after = capture("logs", receiver).stdout
        for suffix, value in (("before.jsonl", before), ("after.jsonl", after),
                              ("client.jsonl", result.stdout), ("stderr.txt", result.stderr)):
            (out / f"{label}-{suffix}").write_text(value)
        require(after.startswith(before), "receiver logs changed instead of appending")
        last_logs[receiver] = after
        case = {"label": label, "source": source, "receiver": receiver, "allowed": allowed,
                "marker": MARKER, "action": "connect", "destination": target["status"]["podIP"]}
        frames = [json.loads(line) for line in result.stdout.splitlines()]
        delta = [json.loads(line) for line in after[len(before):].splitlines()]
        row = {**case, "rc": result.returncode, "accepted": sum(e["event"] == "accepted" for e in delta),
               "received": sum(e["event"] == "received" for e in delta),
               "pass": connection(case, actor, target, frames, delta, result.returncode)}
        save(f"{label}-result.json", row)
        return row, frames

    baseline = {p["metadata"]["name"]: p["spec"] for p in items if p["kind"] == "NetworkPolicy"}
    policy_specs("open", baseline)
    snapshot("before")
    open_case, open_frames = one(0)
    allowed_case, allowed_frames = one(1)
    starts, hooks, control_seen = [], [], False

    def observed():
        nonlocal starts, hooks, control_seen
        raw = capture("logs", name, "-c", "export-stdout", namespace="kube-system").stdout
        (out / "observer-events.jsonl").write_text(raw)
        events = [json.loads(line) for line in raw.splitlines() if line]
        if len(open_frames) != 2 or len(allowed_frames) != 2:
            return None
        starts, hooks = evidence_matches(events, bound["development"], open_case, open_frames[0])
        control, signals = evidence_matches(events, bound["development"], allowed_case, allowed_frames[0])
        control_seen = len(control) == 1 and signals == control
        return len(starts) == 1 and starts == hooks and control_seen

    try:
        wait(observed, "attributed attempt and observer control", seconds=45)
    except RuntimeError as error:
        (out / "observation-wait.txt").write_text(str(error) + "\n")
    snapshot("after")
    loss = loss_delta(out, "before", "after", dev_node)
    live_observer = current_agent()
    def observer_unchanged(first, last):
        return last is not None and first["metadata"]["uid"] == last["metadata"]["uid"] and (
            [(s["name"], s["containerID"], s["restartCount"]) for s in first["status"]["containerStatuses"]]
            == [(s["name"], s["containerID"], s["restartCount"]) for s in last["status"]["containerStatuses"]]
        )

    detected = detection(control_seen and allowed_case["pass"] and loss_clear(loss)
                         and observer_unchanged(observer, live_observer), open_case["pass"], starts, hooks)
    save("detection.json", {**detected, "loss_delta": loss, "execs": starts, "hooks": hooks})

    replaced = [p for p in actor_egress(True) if p["metadata"]["name"] == "egress-development"]
    expected = {**baseline, "egress-development": replaced[0]["spec"]}
    save("enforced-policy.json", replaced[0])
    rows = [open_case, allowed_case]
    try:
        apply(replaced)
        policy_specs("enforced", expected)
        # Same settling interval as the isolated P11 fixture. Successful bracketing
        # samples and receiver results still determine the verdict.
        time.sleep(12)
        for i in range(2, 6):
            rows.append(one(i)[0])
    finally:
        apply([p for p in items if p["kind"] == "NetworkPolicy"
               and p["metadata"]["name"] == "egress-development"])
    policy_specs("restored", baseline)
    time.sleep(12)
    rows.append(one(6)[0])
    last = pods()
    save("pods-after.json", last)
    stable = last is not None and all(
        p["metadata"]["uid"] == last[key]["metadata"]["uid"]
        and p["status"]["podIP"] == last[key]["status"]["podIP"]
        and container_id(p) == container_id(last[key]) for key, p in bound.items()
    )
    continuous = all(capture("logs", receiver).stdout == last_log for receiver, last_log in last_logs.items())
    blocking = stable and continuous and all(row["pass"] for row in rows)
    result = {"detection": detected, "blocking": "PASS" if blocking else "FAIL", "cases": rows,
              "stable_pods": stable, "continuous_receiver_logs": continuous,
              "verdict": "PASS" if blocking and detected["verdict"] == "VIOLATION" else "FAIL"}
    save("summary.json", result)
    return result["verdict"] == "PASS"


if __name__ == "__main__":
    sys.exit(0 if run(sys.argv[1], Path(sys.argv[2])) else 1)
