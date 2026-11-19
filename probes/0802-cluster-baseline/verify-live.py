#!/usr/bin/env python3
"""Verify the initialized 08-02 fixture on its dedicated loopback kind API."""

import json
from pathlib import Path
import subprocess
import sys
import time
from urllib.parse import urlsplit

from manifests import IMAGE, NS, ROLES, SCRIPT, actor_egress, initial, observe


NODES = {f"{NS}-{part}" for part in ("control-plane", "worker", "worker2", "worker3")}


def verify(config, evidence):
    k = ["kubectl", "--kubeconfig", str(config), "--context", f"kind-{NS}", "--request-timeout=15s"]

    def invoke(*args, timeout=25):
        return subprocess.run(k + list(args), capture_output=True, text=True, timeout=timeout)

    def kubectl(*args, timeout=25):
        result = invoke(*args, timeout=timeout)
        if result.returncode:
            raise RuntimeError(f"kubectl {' '.join(args)}: {result.stderr.strip()}")
        return result.stdout

    def write(name, data):
        (evidence / name).write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")

    def get(*args):
        return json.loads(kubectl("get", *args, "-o", "json"))

    def apply(name, items, *flags):
        path = evidence / f"{name}.json"
        write(path.name, {"apiVersion": "v1", "kind": "List", "items": items})
        (evidence / f"{name}-apply.txt").write_text(kubectl("apply", "-f", str(path), *flags))

    context = json.loads(kubectl("config", "view", "--minify", "-o", "json"))["clusters"][0]
    assert context["name"] == f"kind-{NS}" and urlsplit(context["cluster"]["server"]).hostname == "127.0.0.1"
    nodes = get("nodes")
    assert {node["metadata"]["name"] for node in nodes["items"]} == NODES
    assert all(node["status"]["nodeInfo"]["kubeletVersion"] == "v1.35.0" and
               any(c["type"] == "Ready" and c["status"] == "True" for c in node["status"]["conditions"])
               for node in nodes["items"])
    write("nodes.json", nodes)
    agents = get("pods", "-n", "kube-system", "-l", "app.kubernetes.io/component=agent,app.kubernetes.io/name=tetragon")
    assert {pod["spec"]["nodeName"] for pod in agents["items"]} == NODES
    assert len(agents["items"]) == 4
    system = get("daemonsets", "-n", "kube-system")
    ready = {d["metadata"]["name"]: d["status"].get("numberReady", 0) for d in system["items"]}
    assert ready["cilium"] == ready["tetragon"] == 4 and "kube-proxy" not in ready and "kindnet" not in ready
    write("system-daemonsets.json", system)

    apply("initial", initial())
    kubectl("-n", NS, "wait", "--request-timeout=0", "--for=condition=Ready", "pod", "--all", "--timeout=120s", timeout=130)
    fixture = get("pods", "-n", NS)
    expected = {*ROLES, "control", *(f"receiver-{route}" for route in (*ROLES, "denied"))}
    pods = {p["metadata"]["name"]: p for p in fixture["items"]}
    assert set(pods) == expected
    for pod in pods.values():
        status, = pod["status"]["containerStatuses"]
        assert status["ready"] and status["restartCount"] == 0 and status["containerID"] and status["imageID"]
        assert pod["spec"]["containers"][0]["image"] == IMAGE and not pod["spec"].get("imagePullSecrets")
    write("fixture-pods.json", fixture)
    system_pods = get("pods", "-n", "kube-system")
    write("system-pods.json", system_pods)
    for name in ("cilium", "tetragon", "tetragon-operator"):
        group = [p for p in system_pods["items"] if (
            p["metadata"]["name"].startswith("tetragon-operator-") if name == "tetragon-operator"
            else any(owner["kind"] == "DaemonSet" and owner["name"] == name
                     for owner in p["metadata"].get("ownerReferences", []))
        )]
        assert len(group) == (1 if name == "tetragon-operator" else 4), name
        assert all(s["ready"] and s["imageID"] for p in group for s in p["status"]["containerStatuses"]), name

    namespace = get("namespace", NS)
    labels = namespace["metadata"]["labels"]
    assert labels["pod-security.kubernetes.io/enforce"] == "restricted"
    assert labels["pod-security.kubernetes.io/enforce-version"] == "v1.35"
    write("namespace.json", namespace)
    network = get("networkpolicies", "-n", NS)
    assert {x["metadata"]["name"]: x["spec"] for x in network["items"]} == {
        x["metadata"]["name"]: x["spec"] for x in initial() if x["kind"] == "NetworkPolicy"
    }
    write("network-active.json", network)
    apply("enforce-dry-run", actor_egress(True), "--dry-run=server")
    assert get("networkpolicies", "-n", NS)["items"] == network["items"]

    deadline = time.monotonic() + 120
    while True:
        endpoints = get("ciliumendpoints", "-n", NS)
        if {e["metadata"]["name"] for e in endpoints["items"] if e.get("status", {}).get("state") == "ready"} == expected:
            break
        assert time.monotonic() < deadline, "Cilium endpoints did not reach ready"
        time.sleep(2)
    write("cilium-endpoints.json", endpoints)

    apply("observe", observe())
    deadline = time.monotonic() + 120
    for agent in agents["items"]:
        name = agent["metadata"]["name"]
        while True:
            policies = json.loads(kubectl("-n", "kube-system", "exec", name, "-c", "tetragon", "--",
                                          "tetra", "tracingpolicy", "list", "--domain", "k8s", "-o", "json"))
            write(f"{name}-policies.json", policies)
            if any(p.get("name") == "observe-fixture" and p.get("namespace") == NS and
                   p.get("state") == "TP_STATE_ENABLED" for p in policies.get("policies", [])):
                break
            assert time.monotonic() < deadline, f"observation policy not enabled on {name}"
            time.sleep(2)

    receivers = sorted(name for name in pods if name.startswith("receiver-"))

    def receiver_log(name, phase):
        raw = kubectl("-n", NS, "logs", name)
        (evidence / f"{name}-{phase}.jsonl").write_text(raw)
        return [json.loads(line) for line in raw.splitlines()]

    for name in receivers:
        assert [e["event"] for e in receiver_log(name, "initial")] == ["listening"], name
    for source, target, identifier in (("development", "receiver-development", "0802-boot-allow"),
                                        ("control", "receiver-denied", "0802-boot-control")):
        ip = pods[target]["status"]["podIP"]
        result = kubectl("-n", NS, "exec", source, "--", *SCRIPT, "connect", identifier, ip)
        (evidence / f"{source}-bootstrap.jsonl").write_text(result)
        tape = receiver_log(target, "bootstrap")
        assert [e["event"] for e in tape] == ["listening", "accepted", "received"], target
        assert tape[-1]["request_id"] == identifier

    marker = get("configmap", "promotion-slot", "-n", NS)
    assert marker["data"] == {"digest": "", "state": "empty"}
    write("promotion-initial.json", marker)
    assert not kubectl("-n", NS, "get", "secrets", "-o", "name").strip()
    grants = []
    for actor in (*ROLES, "control", "receiver", "publisher", "puller", "promoter"):
        for verb, resource in (("get", "configmaps/promotion-slot"), ("patch", "configmaps/promotion-slot"), ("patch", "pods")):
            allow = actor == "promoter" and resource == "configmaps/promotion-slot"
            result = invoke("-n", NS, "auth", "can-i", verb, resource, f"--as=system:serviceaccount:{NS}:{actor}")
            assert (result.stdout.strip(), result.returncode) == (("yes", 0) if allow else ("no", 1)), result
            grants.append({"subject": actor, "verb": verb, "resource": resource, "allowed": allow})
    write("synthetic-rbac.json", grants)
    for agent in agents["items"]:
        name = agent["metadata"]["name"]
        result = invoke("get", "--raw", f"/api/v1/namespaces/kube-system/pods/{name}:2112/proxy/metrics")
        (evidence / f"{name}-metrics.txt").write_text(result.stdout if result.returncode == 0 else result.stderr)
    write("baseline.json", {"nodes_ready": 4, "cilium_ready": 4, "tetragon_ready": 4, "observer_enabled": 4,
                            "fixture_ready": 8, "receiver_initially_empty": 4, "bootstrap_echoes": 2,
                            "synthetic_checks": len(grants), "namespace_secrets": 0, "result": "PASS"})


if __name__ == "__main__":
    verify(Path(sys.argv[1]), Path(sys.argv[2]))
