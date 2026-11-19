#!/usr/bin/env python3
"""Rejudge P09 from preserved events, fixture receipts and Kubernetes status."""

from datetime import datetime
import json
from pathlib import Path
import sys

ROLES = ("development", "guardrail", "product")
LOSS_METRICS = (
    "tetragon_observer_ringbuf_events_lost_total",
    "tetragon_observer_ringbuf_queue_events_lost_total",
    "tetragon_events_export_failed_total",
    "tetragon_export_ratelimit_events_dropped_total",
)


def document(path):
    return json.loads(path.read_text())


def lines(path):
    return [json.loads(line) for line in path.read_text().splitlines() if line]


def container_id(pod):
    status, = pod["status"]["containerStatuses"]
    return status["containerID"].split("://", 1)[1]


def bounds(process, pod, action, marker):
    live = process.get("pod") or {}
    container = live.get("container") or {}
    metadata = pod["metadata"]
    labels = live.get("pod_labels") or {}
    observed_id = container.get("id", "").removeprefix("containerd://")
    # Tetragon records the invoked path, while /proc/self/exe resolves the image symlink.
    binary = process.get("binary")
    binary_matches = binary == action["binary"] or (
        binary == "/usr/local/bin/python3" and action["binary"] == "/usr/local/bin/python3.13"
    )
    if (live.get("uid") != metadata["uid"] or live.get("namespace") != metadata["namespace"]
            or live.get("name") != metadata["name"] or observed_id != container_id(pod)
            or container.get("name") != "fixture" or container.get("pid") != action["pid"]
            or process.get("uid") != action["uid"] or not binary_matches
            or marker not in process.get("arguments", "").split()
            or any(labels.get(k) != metadata["labels"][k] for k in (
                "fixture-role", "fixture-repo", "fixture-assignment", "fixture-generation", "fixture-session"
            )) or type(process.get("pid")) is not int or not process.get("exec_id")
            or not process.get("start_time")):
        return None
    observed_ms = int(datetime.fromisoformat(process["start_time"]).timestamp() * 1000)
    expected_ns = action["sample_wall_ns"] - action["sample_boot_ns"] + (
        action["start_ticks"] * 1_000_000_000 // action["clock_ticks"]
    )
    if abs(observed_ms - expected_ns // 1_000_000) > 250:
        return None
    return (process["exec_id"], process["pid"], process["start_time"])


def action_hook(event, case):
    args = event.get("args", [])
    fn = event.get("function_name", "")
    if case["action"] == "open":
        return (fn.endswith("sys_openat") and event.get("policy_name") == "observe-p09"
                and any(arg.get("string_arg") == case["destination"] for arg in args))
    return (fn == "tcp_connect" and event.get("policy_name") == "observe-fixture"
            and any(arg.get("sock_arg", {}).get("daddr") == case["destination"]
                    and arg["sock_arg"].get("dport") == 18080 for arg in args))


def evidence_matches(events, pod, case, action):
    records = []
    for event in events:
        for field in ("process_exec", "process_kprobe"):
            observed = event.get(field)
            if observed is None:
                continue
            if event.get("node_name") != pod["spec"]["nodeName"]:
                continue
            match = bounds(observed.get("process") or {}, pod, action, case["marker"])
            if match is not None and (field == "process_exec" or action_hook(observed, case)):
                records.append((field, match))
    starts = [identity for kind, identity in records if kind == "process_exec"]
    hooks = [identity for kind, identity in records if kind == "process_kprobe"]
    return starts, hooks


def metrics(path):
    observed = {}
    for line in path.read_text().splitlines():
        if not line or line.startswith("#"):
            continue
        sample, _, value = line.partition(" ")
        if sample.split("{", 1)[0] in LOSS_METRICS:
            observed[sample] = float(value)
    return observed


def loss_delta(directory, start, end, node):
    a = metrics(directory / f"{start}-{node}-metrics.txt")
    b = metrics(directory / f"{end}-{node}-metrics.txt")
    return {
        "samples": {key: b[key] - a[key] for key in sorted(a.keys() & b.keys())},
        "added": {key: b[key] for key in sorted(b.keys() - a.keys())},
        "missing": sorted(a.keys() - b.keys()),
        "missing_metrics": sorted(set(LOSS_METRICS) - {key.split("{", 1)[0] for key in a.keys() & b.keys()}),
    }


def loss_clear(delta):
    return (not delta["missing"] and not delta["missing_metrics"]
            and all(value == 0 for value in (*delta["samples"].values(), *delta["added"].values())))


def assignment_matches(pod, identity):
    metadata = pod["metadata"]
    return (all(metadata["labels"].get(key) == value for key, value in identity.items())
            and metadata.get("annotations", {}).get("ieo-poc/session") == identity["fixture-session"])


def judge(directory):
    cases = document(directory / "cases.json")
    pods = document(directory / "pods-before.json")
    later_pods = document(directory / "pods-after.json")
    before = document(directory / "before-agents.json")
    healthy = document(directory / "healthy-agents.json")
    outage = document(directory / "outage-agents.json")
    outage_after = document(directory / "outage-after-action-agents.json")
    restored = document(directory / "restored-agents.json")
    recovered = document(directory / "recovered-agents.json")
    by_node = {node: lines(directory / f"healthy-{node}-events.jsonl") for node in healthy}
    selector = document(directory / "policy.json")["spec"]["podSelector"]["matchLabels"]
    loss = {node: loss_delta(directory, "before", "healthy", node) for node in healthy}
    dev_node = pods["development"]["spec"]["nodeName"]
    recovery_loss = loss_delta(directory, "restored", "recovered", dev_node)
    recovery_coverage = (
        dev_node in restored and dev_node in recovered
        and restored[dev_node]["metadata"]["uid"] == recovered[dev_node]["metadata"]["uid"]
        and loss_clear(recovery_loss)
    )
    roles_stable = all(
        later_pods[role]["metadata"]["uid"] == pods[role]["metadata"]["uid"]
        and container_id(later_pods[role]) == container_id(pods[role])
        and later_pods[role]["status"]["containerStatuses"][0]["restartCount"] == 0
        for role in (*ROLES, "control")
    )
    coverage = (
        set(before) == set(healthy) == set(recovered) and len(healthy) == 4
        and all(before[node]["metadata"]["uid"] == healthy[node]["metadata"]["uid"] for node in healthy)
        and all(loss_clear(data) for data in loss.values())
    )
    outage_verified = (
        set(outage) == set(outage_after) == set(healthy) - {dev_node}
        and recovered[dev_node]["metadata"]["uid"] != healthy[dev_node]["metadata"]["uid"]
    )
    rows = {}
    for case in cases:
        label, actor = case["label"], pods[case["role"]]
        start, result = lines(directory / f"{label}.jsonl")
        attributes = set(start) - {"phase"}
        known = (assignment_matches(actor, case["identity"])
                 and assignment_matches(later_pods[case["role"]], case["identity"])
                 and start["phase"] == "attempt" and result["phase"] == "result"
                 and all(result.get(key) == start[key] for key in attributes)
                 and all(start[key] == case[key] for key in ("marker", "action", "destination"))
                 and start["role"] == case["role"] and start["pod_uid"] == actor["metadata"]["uid"]
                 and start["uid"] == actor["spec"]["securityContext"]["runAsUser"])
        selected = all(actor["metadata"]["labels"].get(k) == v for k, v in selector.items())
        receiver = case["receiver"]
        if receiver is not None:
            pod = pods[receiver]
            receipts = lines(directory / f"{receiver}.jsonl")
            matches = [e for e in receipts if e.get("event") == "received" and e.get("request_id") == case["marker"]
                       and e.get("peer") == actor["status"]["podIP"] and e.get("pod_uid") == pod["metadata"]["uid"]]
            known = known and len(matches) == 1 and pod["status"]["podIP"] == case["destination"]

        node = actor["spec"]["nodeName"]
        raw = by_node[node]
        if label == "outage-post":
            raw = lines(directory / f"recovered-{node}-events.jsonl")
        elif label == "observer-offline":
            raw = [event for other in outage_after for event in lines(directory / f"outage-after-action-{other}-events.jsonl")]
        starts, hooks = evidence_matches(raw, actor, case, start)
        if label == "outside-policy":
            verdict = "INCONCLUSIVE_POLICY_SCOPE" if known and not selected and len(starts) == 1 and not hooks else "FAIL"
        elif label == "observer-offline":
            verdict = "INCONCLUSIVE_OBSERVER_OFFLINE" if known and selected and outage_verified and not starts and not hooks else "FAIL"
        elif not coverage or (label == "outage-post" and not recovery_coverage):
            verdict = "INCONCLUSIVE_LOSS_COVERAGE"
        else:
            verdict = "PASS" if known and selected and len(starts) == 1 and hooks == starts else "FAIL"
        rows[label] = {"verdict": verdict, "exec": starts, "hooks": hooks, "marker": case["marker"]}
        if label == "development-allowed":
            target_case, target_action = case, start
        elif label == "foreign-event":
            tokens = {identity[0] for identity in (*starts, *hooks)}
            foreign = [event for event in raw
                       if any(payload.get("process", {}).get("exec_id") in tokens for field, payload in event.items()
                              if field in ("process_exec", "process_kprobe"))]
            wrong_starts, wrong_hooks = evidence_matches(foreign, pods["development"], target_case, target_action)
            if verdict == "PASS" and case["marker"] == target_case["marker"] and not wrong_starts and not wrong_hooks:
                rows["foreign-substitution"] = {"verdict": "REJECTED_FOREIGN_POD", "marker": case["marker"]}
            else:
                rows["foreign-substitution"] = {"verdict": "FAIL", "marker": case["marker"]}

    expected = {f"{role}-{action}": "PASS" for role in ROLES for action in ("pre", "allowed", "comparator", "post")}
    expected.update({"outside-policy": "INCONCLUSIVE_POLICY_SCOPE", "foreign-event": "PASS",
                     "foreign-substitution": "REJECTED_FOREIGN_POD", "outage-pre": "PASS",
                     "observer-offline": "INCONCLUSIVE_OBSERVER_OFFLINE", "outage-post": "PASS"})
    ok = roles_stable and coverage and outage_verified and {name: row["verdict"] for name, row in rows.items()} == expected
    return {"verdict": "PASS" if ok else "FAIL", "role_identity_stable": roles_stable,
            "healthy_coverage": coverage, "recovery_coverage": recovery_coverage, "recovery_loss": recovery_loss, "observer_outage_verified": outage_verified, "loss": loss, "rows": rows}


if __name__ == "__main__":
    result = judge(Path(sys.argv[1]))
    print(json.dumps(result, indent=2, sort_keys=True))
    raise SystemExit(result["verdict"] != "PASS")
