#!/usr/bin/env python3
"""Render the isolated 08-02 fixture and its mutually exclusive policy phases."""

import argparse
import json
from pathlib import Path

from fixture import CANARY, PORT


NS = "ieo-0802"
IMAGE = "docker.io/library/python@sha256:0d8324e4e3453ec8586b886b95e8fc901689f936c413fdf6ae7d93a32a95956e"
SCRIPT = ["python3", "-I", "-B", "/fixture/fixture.py"]
ROLES = ("development", "guardrail", "product")
ACTOR = {"fixture": NS, "fixture-kind": "actor"}
RECEIVER = {"fixture": NS, "fixture-kind": "receiver"}
SOURCE = {"fixture": NS, "fixture-source": "true"}


def obj(kind, name, api="v1", *, namespaced=True, labels=None, **body):
    meta = {"name": name}
    if namespaced:
        meta["namespace"] = NS
    if labels:
        meta["labels"] = labels
    return dict(apiVersion=api, kind=kind, metadata=meta, **body)


def account(name):
    return obj("ServiceAccount", name, automountServiceAccountToken=False)


def pod(name, role, uid, node, server=False):
    if server:
        labels = {**RECEIVER, "fixture-route": role}
    else:
        labels = {**SOURCE, "fixture-kind": "control" if role == "control" else "actor", "fixture-role": role}
    env = [{"name": key, "valueFrom": {"fieldRef": {"fieldPath": field}}} for key, field in (
        ("POD_UID", "metadata.uid"), ("POD_NAME", "metadata.name"), ("NODE_NAME", "spec.nodeName"),
    )]
    env.append({"name": "FIXTURE_ROLE", "value": role})
    container = {
        "name": "fixture", "image": IMAGE, "command": SCRIPT + (["serve"] if server else ["idle"]),
        "env": env, "securityContext": {"allowPrivilegeEscalation": False, "readOnlyRootFilesystem": True,
                                          "capabilities": {"drop": ["ALL"]}},
        "resources": {"requests": {"cpu": "10m", "memory": "16Mi"},
                      "limits": {"cpu": "200m", "memory": "64Mi"}},
        "volumeMounts": [{"name": "fixture", "mountPath": "/fixture", "readOnly": True}],
    }
    volumes = [{"name": "fixture", "configMap": {"name": "fixture"}}]
    if server:
        container["readinessProbe"] = {
            "exec": {"command": ["/bin/busybox", "test", "-f", "/run/fixture/ready"]},
            "timeoutSeconds": 5,
        }
        container["volumeMounts"].append({"name": "ready", "mountPath": "/run/fixture"})
        volumes.append({"name": "ready", "emptyDir": {"sizeLimit": "1Mi"}})
    return obj("Pod", name, labels=labels, spec={
        "serviceAccountName": "receiver" if server else role, "automountServiceAccountToken": False,
        "enableServiceLinks": False, "restartPolicy": "Never",
        "nodeSelector": {"kubernetes.io/hostname": node},
        "securityContext": {"runAsNonRoot": True, "runAsUser": uid, "runAsGroup": uid, "fsGroup": uid,
                            "seccompProfile": {"type": "RuntimeDefault"}},
        "containers": [container], "volumes": volumes,
    })


def network(name, selector, direction, peer=None):
    field = "to" if direction == "Egress" else "from"
    rules = [] if peer is None else [{field: [{"podSelector": {"matchLabels": peer}}],
                                     "ports": [{"port": PORT, "protocol": "TCP"}]}]
    return obj("NetworkPolicy", name, "networking.k8s.io/v1", spec={
        "podSelector": {"matchLabels": selector}, "policyTypes": [direction], direction.lower(): rules,
    })


def network_specs(items):
    # The Kubernetes API omits empty ingress/egress rule slices. Policy types still select deny.
    return {item["metadata"]["name"]: {
        key: value for key, value in item["spec"].items()
        if key not in ("ingress", "egress") or value != []
    } for item in items}

def actor_egress(enforce):
    # Replace each role's rule by name. A second egress policy would union the allowed peers.
    return [network(f"egress-{role}", {**SOURCE, "fixture-role": role}, "Egress",
                    {**RECEIVER, "fixture-route": role} if enforce else RECEIVER) for role in ROLES]


def initial():
    namespace = obj("Namespace", NS, namespaced=False, labels={
        "pod-security.kubernetes.io/enforce": "restricted", "pod-security.kubernetes.io/enforce-version": "v1.35",
    })
    fixture = obj("ConfigMap", "fixture", data={
        "fixture.py": Path(__file__).with_name("fixture.py").read_text(), CANARY.name: "ieo-0802 synthetic canary\n",
    })
    role = obj("Role", "promotion-marker", "rbac.authorization.k8s.io/v1", rules=[{
        "apiGroups": [""], "resources": ["configmaps"], "resourceNames": ["promotion-slot"], "verbs": ["get", "patch"],
    }])
    binding = obj("RoleBinding", "promotion-marker", "rbac.authorization.k8s.io/v1",
                  roleRef={"apiGroup": "rbac.authorization.k8s.io", "kind": "Role", "name": "promotion-marker"},
                  subjects=[{"kind": "ServiceAccount", "name": "promoter", "namespace": NS}])
    words = ("", "2", "3")
    return [namespace, fixture, obj("ConfigMap", "promotion-slot", data={"digest": "", "state": "empty"}),
            *[account(name) for name in (*ROLES, "control", "receiver", "promoter", "publisher", "puller")],
            role, binding, *actor_egress(False),
            network("egress-control", {**SOURCE, "fixture-role": "control"}, "Egress", RECEIVER),
            network("ingress-receiver", RECEIVER, "Ingress", SOURCE), network("egress-receiver", RECEIVER, "Egress"),
            *[pod(role_name, role_name, 10001 + index, f"{NS}-worker{suffix}")
              for index, (role_name, suffix) in enumerate(zip(ROLES, words))],
            pod("control", "control", 10004, f"{NS}-worker"),
            *[pod(f"receiver-{route}", route, 10011 + index, f"{NS}-worker3", server=True)
              for index, route in enumerate((*ROLES, "denied"))]]


def observe():
    return [obj("TracingPolicyNamespaced", "observe-fixture", "cilium.io/v1alpha1", spec={
        "podSelector": {"matchLabels": ACTOR},
        "kprobes": [
            {"call": "tcp_connect", "syscall": False, "args": [{"index": 0, "type": "sock"}]},
            {"call": "sys_openat", "syscall": True, "args": [{"index": 1, "type": "string"}],
             "selectors": [{"matchArgs": [{"index": 1, "operator": "Equal", "values": [str(CANARY)]}]}]},
        ],
    })]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("phase", choices=("initial", "observe", "enforce"))
    phase = parser.parse_args().phase
    items = initial() if phase == "initial" else observe() if phase == "observe" else actor_egress(True)
    print(json.dumps({"apiVersion": "v1", "kind": "List", "items": items}, indent=2))


if __name__ == "__main__":
    main()
