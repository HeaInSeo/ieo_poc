#!/usr/bin/env python3
"""Run the P11 admission pair for each development and Guardrail fixture."""

import argparse
from copy import deepcopy
import json
from pathlib import Path
import subprocess
import sys
from urllib.parse import urlsplit

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "0802-cluster-baseline"))
from manifests import NS, initial


def inputs():
    pods = {entry["metadata"]["name"]: entry for entry in initial() if entry["kind"] == "Pod"}
    for role in ("development", "guardrail"):
        good = deepcopy(pods[role])
        good["metadata"]["name"] = f"p11-admission-{role}"
        bad = deepcopy(good)
        bad["spec"]["containers"][0]["securityContext"]["allowPrivilegeEscalation"] = True
        yield role, good, bad


def verify(kubeconfig, evidence):
    k = ["kubectl", "--kubeconfig", kubeconfig, "--context", f"kind-{NS}", "--request-timeout=15s"]

    def get(*args):
        return json.loads(subprocess.run([*k, *args, "-o", "json"], text=True, capture_output=True,
                                         check=True, timeout=25).stdout)

    cluster, = get("config", "view", "--minify")["clusters"]
    if cluster["name"] != f"kind-{NS}" or urlsplit(cluster["cluster"]["server"]).hostname != "127.0.0.1":
        raise ValueError("P11 requires the dedicated loopback fixture cluster")
    namespace = get("get", "namespace", NS)
    labels = namespace["metadata"]["labels"]
    if (labels.get("pod-security.kubernetes.io/enforce"), labels.get(
            "pod-security.kubernetes.io/enforce-version")) != ("restricted", "v1.35"):
        raise ValueError("expected Restricted v1.35 admission on the fixture namespace")

    evidence.mkdir()
    (evidence / "namespace.json").write_text(json.dumps(namespace, indent=2) + "\n")
    for role, normal, invalid in inputs():
        name = normal["metadata"]["name"]
        for case, body in (("normal", normal), ("invalid", invalid)):
            data = json.dumps(body, indent=2) + "\n"
            (evidence / f"{role}-{case}-request.json").write_text(data)
            result = subprocess.run([*k, "-n", NS, "create", "--dry-run=server", "-f", "-", "-o", "json"],
                                    input=data, text=True, capture_output=True, timeout=25)
            (evidence / f"{role}-{case}-stdout.txt").write_text(result.stdout)
            (evidence / f"{role}-{case}-stderr.txt").write_text(result.stderr)
            if case == "normal":
                if result.returncode or json.loads(result.stdout)["metadata"]["name"] != name:
                    raise RuntimeError(f"{role}: authorized fixture did not pass admission")
            elif (result.returncode == 0 or 'restricted:v1.35' not in result.stderr
                  or 'allowPrivilegeEscalation' not in result.stderr):
                raise RuntimeError(f"{role}: altered fixture did not get the expected PSA denial")
        persisted = get("-n", NS, "get", "pods", "--field-selector", f"metadata.name={name}")
        if persisted["items"]:
            raise RuntimeError(f"{role}: server dry-run left an object")
        print(f"{role}: normal admitted, one-field variant denied, no persisted object")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kubeconfig")
    parser.add_argument("evidence", type=Path)
    arguments = parser.parse_args()
    verify(arguments.kubeconfig, arguments.evidence)
