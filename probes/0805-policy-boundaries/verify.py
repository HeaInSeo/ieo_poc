#!/usr/bin/env python3
"""Exercise P11 boundaries on the accepted, freshly initialized 08-02 fixture."""

import json
from pathlib import Path
import secrets
import subprocess
import sys
import time

from admission import NS, verify as admission_check
from app import ROLES
from manifests import SCRIPT, actor_egress, obj, pod


def verify(config, evidence):
    evidence.mkdir()
    # This checks the dedicated kubeconfig context and Restricted label before any P11 mutations.
    admission_check(str(config), evidence / "admission")
    command = ["kubectl", "--kubeconfig", str(config), "--context", f"kind-{NS}", "--request-timeout=20s", "-n", NS]

    def kubectl(*args, data=None, timeout=25, check=True):
        result = subprocess.run([*command, *args], input=data, capture_output=True, text=True, timeout=timeout)
        if check and result.returncode != 0:
            raise RuntimeError(f"kubectl {args[0]} exited {result.returncode}: {result.stderr}")
        return result

    def write(name, data):
        (evidence / name).write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")

    def get(*args):
        return json.loads(kubectl("get", *args, "-o", "json").stdout)

    def apply(items):
        data = json.dumps({"apiVersion": "v1", "kind": "List", "items": items})
        kubectl("apply", "-f", "-", data=data)

    fixtures = {entry["metadata"]["name"]: entry for entry in get("pods")["items"]}
    expected_roles = {"development": 10001, "guardrail": 10002}
    for role, uid in expected_roles.items():
        active = fixtures[role]
        container, = active["status"]["containerStatuses"]
        assert container["ready"] and container["restartCount"] == 0
        assert active["spec"]["securityContext"]["runAsUser"] == uid
        assert active["spec"]["automountServiceAccountToken"] is False
        assert active["spec"]["serviceAccountName"] == role
    assert fixtures["development"]["metadata"]["uid"] != fixtures["guardrail"]["metadata"]["uid"]
    write("role-pods.json", [fixtures[role] for role in ROLES])

    def network_phase(phase, enforce):
        active = {policy["metadata"]["name"]: policy["spec"] for policy in get("networkpolicies")["items"]}
        expected = {policy["metadata"]["name"]: policy["spec"] for policy in actor_egress(enforce)}
        assert all(active[name] == spec for name, spec in expected.items())
        write(f"network-{phase}.json", active)
        rows = []
        for source in ROLES:
            for role in ROLES:
                target = f"receiver-{role}"
                identifier = f"0802-p11-{phase}-{source[:3]}-{role[:3]}"
                before = kubectl("logs", target).stdout
                (evidence / f"{identifier}-before.jsonl").write_text(before)
                result = kubectl("exec", source, "--", *SCRIPT, "connect", identifier,
                                 fixtures[target]["status"]["podIP"], check=False)
                (evidence / f"{identifier}-client.jsonl").write_text(result.stdout)
                (evidence / f"{identifier}-stderr.txt").write_text(result.stderr)
                after = kubectl("logs", target).stdout
                (evidence / f"{identifier}-after.jsonl").write_text(after)
                assert after.startswith(before)
                new = [json.loads(line) for line in after[len(before):].splitlines()]
                attempt, outcome = [json.loads(line) for line in result.stdout.splitlines()]
                assert (attempt["event"], outcome["event"]) == ("attempt", "result")
                assert attempt["request_id"] == outcome["request_id"] == identifier
                assert attempt["pod_uid"] == fixtures[source]["metadata"]["uid"]
                allow = not enforce or source == role
                assert outcome["ok"] is allow
                if allow:
                    assert result.returncode == 0
                    assert [event["event"] for event in new] == ["accepted", "received"]
                    assert new[1]["request_id"] == identifier
                else:
                    assert result.returncode != 0 and outcome["error"] == "TimeoutError"
                    assert not new, f"unexpected receiver activity for {identifier}"
                rows.append({"source": source, "destination": target, "phase": phase, "allowed": allow,
                             "accepted": sum(e["event"] == "accepted" for e in new)})
        return rows

    paths = network_phase("baseline", False)
    apply(actor_egress(True))
    time.sleep(12)
    paths += network_phase("enforced", True)
    apply(actor_egress(False))
    time.sleep(12)
    paths += network_phase("restored", False)
    write("network-results.json", paths)

    # Credentials exist only for the synthetic application. Tokens are sent over stdin,
    # and the Secret response and manifest are excluded from the evidence bundle.
    tokens = {role: secrets.token_urlsafe(32) for role in ROLES}
    source = Path(__file__).with_name("app.py").read_text()
    application = pod("p11-application", "application", 10020, f"{NS}-worker3", server=True)
    spec = application["spec"]
    spec["volumes"].extend([{"name": "app", "configMap": {"name": "p11-app"}},
                            {"name": "auth", "secret": {"secretName": "p11-auth", "defaultMode": 288}}])
    container = spec["containers"][0]
    container["command"] = ["python3", "-I", "-B", "/p11/app.py", "serve"]
    container["volumeMounts"].extend([{"name": "app", "mountPath": "/p11", "readOnly": True},
                                      {"name": "auth", "mountPath": "/p11-auth", "readOnly": True}])
    apply([obj("ConfigMap", "p11-app", data={"app.py": source}),
           obj("Secret", "p11-auth", stringData=tokens), application])
    kubectl("wait", "--request-timeout=0", "--for=condition=Ready", "pod/p11-application", "--timeout=120s", timeout=130)
    app_pod = get("pod", "p11-application")
    write("application-pod.json", app_pod)
    address = app_pod["status"]["podIP"]

    # The same driver code executes in both role Pods on the existing image. Sending it via stdin
    # on a separate invocation avoids mounting either token or modifying the accepted role Pods.
    code = source.split('if __name__ == "__main__":', 1)[0] + '\nquery(*sys.argv[1:])\n'
    write_cases = []

    def query(actor, method, owner, status, body, value=None):
        args = [address, method, owner] + ([value] if value is not None else [])
        response = json.loads(kubectl("exec", "-i", actor, "--", "python3", "-I", "-B", "-c", code,
                                      *args, data=tokens[actor] + "\n").stdout)
        write_cases.append({"actor": actor, "method": method, "owner": owner, **response})
        assert response == {"status": status, "body": body}, write_cases[-1]

    for role in ROLES:
        other = next(name for name in ROLES if name != role)
        initial_value = f"synthetic-{other}-initial"
        query(other, "GET", other, 200, {"value": initial_value})
        query(role, "GET", other, 403, {"error": "denied"})
        query(role, "PUT", other, 403, {"error": "denied"}, "synthetic-cross-write")
        query(other, "GET", other, 200, {"value": initial_value})
    for role in ROLES:
        value = f"synthetic-{role}-updated"
        query(role, "PUT", role, 200, {"value": value}, value)
        query(role, "GET", role, 200, {"value": value})
    write("application-results.json", write_cases)
    log = kubectl("logs", "p11-application").stdout
    (evidence / "application-events.jsonl").write_text(log)
    actual = [json.loads(line) for line in log.splitlines()]
    assert actual[0] == {"event": "listening"}
    assert [(e["subject"], e["method"], e["object"], e["status"]) for e in actual[1:]] == [
        (e["actor"], e["method"], e["owner"], e["status"]) for e in write_cases
    ]
    print("P11 scoped checks PASS: PSA 4/4, network 12/12, application 12/12")
    write("result.json", {"psa": 4, "network": len(paths), "application": len(write_cases), "result": "PASS",
                          "file_credential_isolation": "UNVERIFIED_NO_P07_EVIDENCE"})


if __name__ == "__main__":
    verify(Path(sys.argv[1]), Path(sys.argv[2]))
