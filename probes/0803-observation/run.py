#!/usr/bin/env python3
"""Capture scoped Kubernetes and Tetragon evidence on the baseline kind VM."""

import ipaddress
import json
import os
from pathlib import Path
import re
import runpy
import subprocess
import sys
import time
from urllib.parse import urlsplit
from uuid import uuid4

HERE = Path(__file__).resolve().parent
BASE = HERE.parent / "0802-cluster-baseline"
sys.path.insert(0, str(BASE))
from manifests import ACTOR, NS, ROLES, initial, obj

from evaluate import judge


def write_json(path, obj):
    path.write_text(json.dumps(obj, indent=2, sort_keys=True) + "\n")


def wait_for(check, description):
    deadline = time.monotonic() + 150
    while time.monotonic() < deadline:
        value = check()
        if value:
            return value
        time.sleep(2)
    raise RuntimeError(f"fixture did not reach {description}")


def run(config, evidence):
    kubectl = ["kubectl", "--kubeconfig", config, "--context", f"kind-{NS}", "--request-timeout=15s"]
    body = (HERE / "operation.py").read_text()

    def request(*args, stdin=None, system=False):
        ns = "kube-system" if system else NS
        return subprocess.run([*kubectl, "-n", ns, *args], input=stdin, capture_output=True, text=True, timeout=30)

    def k(*args, **kwargs):
        result = request(*args, **kwargs)
        if result.returncode:
            raise RuntimeError(f"{' '.join(args)}: {result.stderr.strip()}")
        return result.stdout

    def get(*args, system=False):
        return json.loads(k("get", *args, "-o", "json", system=system))

    current = json.loads(k("config", "view", "--minify", "-o", "json"))["clusters"][0]
    if current["name"] != f"kind-{NS}" or urlsplit(current["cluster"]["server"]).hostname != "127.0.0.1":
        raise ValueError("P09 requires the dedicated loopback fixture cluster")

    stamp = os.environ.get("GITHUB_RUN_ID") or uuid4().hex[:10]
    if not re.fullmatch(r"[0-9a-f]{1,18}", stamp):
        raise ValueError("invalid fixture run identifier")
    attempt = os.environ.get("GITHUB_RUN_ATTEMPT", "1")
    if not re.fullmatch(r"\d{1,4}", attempt):
        raise ValueError("invalid fixture run attempt")
    stamp = f"{stamp}-{attempt}"
    path = f"/fixture/p09-{stamp}.txt"
    items = initial()
    for item in items:
        if item["kind"] == "ConfigMap" and item["metadata"]["name"] == "fixture":
            item["data"][Path(path).name] = "p09 known fixture\n"
        if item["kind"] == "Pod" and item["metadata"]["name"] in (*ROLES, "control"):
            role = item["metadata"]["name"]
            item["metadata"]["labels"].update({
                "fixture-repo": "ieo-poc-fixture",
                "fixture-assignment": f"p09-{stamp}-{role}",
                "fixture-generation": "1",
            })
            item["metadata"].setdefault("annotations", {})["ieo-poc/session"] = f"p09-{stamp}-{role}"
    runpy.run_path(str(BASE / "verify-live.py"))["verify"](config, evidence, fixture_items=items)

    output = evidence / "p09"
    output.mkdir()
    pods = {p["metadata"]["name"]: p for p in get("pods")["items"]}
    write_json(output / "pods-before.json", pods)
    policy = obj("TracingPolicyNamespaced", "observe-p09", "cilium.io/v1alpha1", spec={
        "podSelector": {"matchLabels": ACTOR},
        "kprobes": [{"call": "sys_openat", "syscall": True, "args": [{"index": 1, "type": "string"}],
                     "selectors": [{"matchArgs": [{"index": 1, "operator": "Equal", "values": [path]}]}]}],
    })
    write_json(output / "policy.json", policy)
    k("apply", "-f", str(output / "policy.json"))

    def agents():
        selected = get("pods", "-l", "app.kubernetes.io/component=agent,app.kubernetes.io/name=tetragon", system=True)
        return {p["spec"]["nodeName"]: p for p in selected["items"]}

    def snapshot(tag, nodes):
        def ready_observers():
            current_agents = agents()
            if (set(current_agents) == nodes
                    and all(not p["metadata"].get("deletionTimestamp") and
                            any(c["type"] == "Ready" and c["status"] == "True"
                                for c in p["status"].get("conditions", [])) for p in current_agents.values())):
                return current_agents
            return None

        current_agents = wait_for(ready_observers, tag)
        write_json(output / f"{tag}-agents.json", current_agents)
        for node, pod in sorted(current_agents.items()):
            name = pod["metadata"]["name"]

            def active_policies():
                result = request("exec", name, "-c", "tetragon", "--",
                                 "tetra", "tracingpolicy", "list", "--domain", "k8s", "-o", "json", system=True)
                if result.returncode:
                    return None
                document = json.loads(result.stdout)
                names = {p["name"] for p in document.get("policies", [])
                         if p.get("namespace") == NS and p.get("state") == "TP_STATE_ENABLED"}
                if {"observe-fixture", "observe-p09"} <= names:
                    return document
                return None

            write_json(output / f"{tag}-{node}-policies.json", wait_for(active_policies, f"{tag} active policies on {node}"))
            metric = request("get", "--raw", f"/api/v1/namespaces/kube-system/pods/{name}:2112/proxy/metrics", system=True)
            (output / f"{tag}-{node}-metrics.txt").write_text(metric.stdout if metric.returncode == 0 else metric.stderr)
            if tag != "before":
                (output / f"{tag}-{node}-events.jsonl").write_text(k("logs", name, "-c", "export-stdout", system=True))
        return current_agents

    nodes = {p["spec"]["nodeName"] for p in agents().values()}
    if len(nodes) != 4:
        raise ValueError("expected four dedicated observer nodes")
    cases = []

    def run_case(label, role, action, route=None, marker=None):
        receiver = f"receiver-{route}" if route else None
        dest = pods[receiver]["status"]["podIP"] if receiver else path
        if receiver and ipaddress.IPv4Address(dest) not in ipaddress.IPv4Network("10.244.0.0/16"):
            raise ValueError("receiver is outside the fixture Pod network")
        marker = marker or f"0802-p09-{stamp[-10:]}-{len(cases) + 1}"
        case = {"label": label, "role": role, "action": action,
                "destination": dest, "marker": marker, "receiver": receiver}
        cases.append(case)
        write_json(output / "cases.json", cases)
        result = request("exec", "-i", role, "--", "python3", "-I", "-B", "-", marker, action, dest, stdin=body)
        (output / f"{label}.jsonl").write_text(result.stdout)
        if result.returncode:
            (output / f"{label}.stderr").write_text(result.stderr)
            raise RuntimeError(f"fixture operation failed: {label}")
        return marker

    snapshot("before", nodes)
    for role in ROLES:
        run_case(f"{role}-pre", role, "open")
        marker = run_case(f"{role}-allowed", role, "connect", role)
        if role == "development":
            shared_marker = marker
        run_case(f"{role}-comparator", role, "connect", "denied")
        run_case(f"{role}-post", role, "open")
    run_case("outside-policy", "control", "open")
    run_case("foreign-event", "guardrail", "connect", "development", marker=shared_marker)
    run_case("outage-pre", "development", "open")
    snapshot("healthy", nodes)

    dev_node = pods["development"]["spec"]["nodeName"]
    for node in sorted(nodes - {dev_node}):
        k("label", "node", node, "p09-observer=active")
    daemon = get("daemonset", "tetragon", system=True)
    old_selector = daemon["spec"]["template"]["spec"].get("nodeSelector", {})
    old_strategy = daemon["spec"]["updateStrategy"]
    pause = {"spec": {"updateStrategy": {"type": "OnDelete", "rollingUpdate": None}, "template": {"spec": {
        "nodeSelector": {**old_selector, "p09-observer": "active"},
    }}}}
    (output / "observer-stop.txt").write_text(
        k("patch", "daemonset", "tetragon", "--type=merge", "-p", json.dumps(pause), system=True))
    snapshot("outage", nodes - {dev_node})
    run_case("observer-offline", "development", "open")
    snapshot("outage-after-action", nodes - {dev_node})
    restore = {"spec": {"updateStrategy": old_strategy,
                        "template": {"spec": {"nodeSelector": {"p09-observer": None}}}}}
    (output / "observer-start.txt").write_text(
        k("patch", "daemonset", "tetragon", "--type=merge", "-p", json.dumps(restore), system=True))
    snapshot("restored", nodes)
    run_case("outage-post", "development", "open")
    snapshot("recovered", nodes)

    for name in ("development", "guardrail", "product", "denied"):
        receiver = f"receiver-{name}"
        (output / f"{receiver}.jsonl").write_text(k("logs", receiver))
    write_json(output / "pods-after.json", {p["metadata"]["name"]: p for p in get("pods")["items"]})
    summary = judge(output)
    write_json(output / "summary.json", summary)
    print(json.dumps(summary, sort_keys=True))
    if summary["verdict"] != "PASS":
        raise SystemExit(1)


if __name__ == "__main__":
    run(sys.argv[1], Path(sys.argv[2]))
