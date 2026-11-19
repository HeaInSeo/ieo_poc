#!/usr/bin/env python3
"""Check the proposed manifests and run a local loopback fixture smoke test."""

from contextlib import redirect_stdout
from io import StringIO
import argparse
import json
import socket
from threading import Thread

from fixture import accept_one, lab_address, request
from manifests import ACTOR, NS, PORT, ROLES, actor_egress, initial, observe


def matches(labels, selector):
    return all(labels.get(key) == value for key, value in selector.items())


def main():
    objects = initial()
    pods = [obj for obj in objects if obj["kind"] == "Pod"]
    assert len(pods) == 8
    for pod in pods:
        spec = pod["spec"]
        security = spec["securityContext"]
        assert security["runAsNonRoot"] and security["runAsUser"] > 0
        assert security["seccompProfile"] == {"type": "RuntimeDefault"}
        assert spec["automountServiceAccountToken"] is False
        assert not any(spec.get(field) for field in ("hostNetwork", "hostPID", "hostIPC"))
        assert all(set(volume) <= {"name", "configMap", "emptyDir"} for volume in spec["volumes"])
        for container in spec["containers"]:
            assert "@sha256:" in container["image"]
            security = container["securityContext"]
            assert security == {"allowPrivilegeEscalation": False, "readOnlyRootFilesystem": True,
                                "capabilities": {"drop": ["ALL"]}}
    assert len({pod["spec"]["securityContext"]["runAsUser"] for pod in pods}) == 8

    policies = {p["metadata"]["name"]: p["spec"] for p in objects if p["kind"] == "NetworkPolicy"}
    assert len(policies) == 6 and policies["egress-receiver"]["egress"] == []
    replacement = {p["metadata"]["name"]: p["spec"] for p in actor_egress(True)}
    assert set(replacement) == {f"egress-{role}" for role in ROLES}
    assert all(policies[name]["podSelector"] == spec["podSelector"] for name, spec in replacement.items())
    assert all(spec["egress"][0]["ports"] == [{"port": PORT, "protocol": "TCP"}]
               for spec in replacement.values())

    actors = {pod["metadata"]["labels"]["fixture-role"]: pod["metadata"]["labels"] for pod in pods
              if "fixture-role" in pod["metadata"]["labels"]}
    receivers = {pod["metadata"]["labels"]["fixture-route"]: pod["metadata"]["labels"] for pod in pods
                 if "fixture-route" in pod["metadata"]["labels"]}
    assert set(actors) == {*ROLES, "control"} and set(receivers) == {*ROLES, "denied"}
    for enforced, active in ((False, policies), (True, {**policies, **replacement})):
        for role, labels in actors.items():
            selected = [p for p in active.values() if "egress" in p and matches(labels, p["podSelector"]["matchLabels"])]
            assert len(selected) == 1
            peer = selected[0]["egress"][0]["to"][0]["podSelector"]["matchLabels"]
            assert set(selected[0]["egress"][0]["to"][0]) == {"podSelector"}
            for route, labels in receivers.items():
                assert matches(labels, peer) == (not enforced or role == "control" or route == role)
    print("fixture pods 8; expected egress cases 32/32 (manifest semantics only)")

    policy = observe()[0]
    assert policy["metadata"]["namespace"] == NS and policy["spec"]["podSelector"]["matchLabels"] == ACTOR
    assert {name for name, labels in actors.items() if matches(labels, ACTOR)} == set(ROLES)
    assert "matchActions" not in json.dumps(policy)
    for address in ("127.0.0.1", "192.0.2.1", "2001:db8::1", "example.invalid"):
        try:
            lab_address(address)
        except argparse.ArgumentTypeError:
            continue
        raise AssertionError(f"non-lab address accepted: {address}")
    assert lab_address("10.244.1.20") == "10.244.1.20"
    print("namespaced observe-only selector; destination guard cases 5/5")

    bind = next(obj for obj in objects if obj["kind"] == "RoleBinding")
    marker_rule = next(obj for obj in objects if obj["kind"] == "Role")["rules"]
    assert bind["subjects"] == [{"kind": "ServiceAccount", "name": "promoter", "namespace": NS}]
    assert marker_rule == [{"apiGroups": [""], "resources": ["configmaps"],
                            "resourceNames": ["promotion-slot"], "verbs": ["get", "patch"]}]
    print("only the synthetic promoter can update the empty promotion marker by fixture RBAC")

    log = StringIO()
    with socket.create_server(("127.0.0.1", 0)) as listener, redirect_stdout(log):
        listener.settimeout(3)
        address = listener.getsockname()
        server = Thread(target=accept_one, args=(listener,))
        server.start()
        request(address, "0802-loopback")
        server.join(3)
        assert not server.is_alive()
    entries = [json.loads(line) for line in log.getvalue().splitlines()]
    assert [entry["event"] for entry in entries] == ["accepted", "received"]
    assert entries[1]["request_id"] == "0802-loopback"
    print("local loopback request, exact echo and accept/receive evidence PASS")


if __name__ == "__main__":
    main()
