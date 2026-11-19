"""Separate the approved fixture's observation and CNI enforcement outcomes."""

MARKER = "0802-p10-fixture"


def connection(case, actor, receiver, client, delta, rc):
    if len(client) != 2 or receiver["status"]["podIP"] != case["destination"]:
        return False
    action, result = client
    if (action.get("phase") != "attempt" or result.get("phase") != "result"
            or action.get("marker") != MARKER or action.get("action") != "connect"
            or action.get("destination") != case["destination"]
            or action.get("pod_uid") != actor["metadata"]["uid"]
            or action.get("role") != case["source"]
            or action.get("uid") != actor["spec"]["securityContext"]["runAsUser"]
            or any(result.get(k) != v for k, v in action.items() if k != "phase")):
        return False
    if not case["allowed"]:
        return (rc != 0 and result.get("ok") is False and result.get("stage") == "connect"
                and result.get("error") in {"TimeoutError", "ConnectionRefusedError"}
                and not delta)
    if rc != 0 or result.get("ok") is not True or result.get("stage") != "echo":
        return False
    if [event.get("event") for event in delta] != ["accepted", "received"]:
        return False
    return (delta[1].get("request_id") == MARKER
            and all(event.get("peer") == actor["status"]["podIP"]
                    and event.get("pod_uid") == receiver["metadata"]["uid"] for event in delta))


def detection(healthy, open_path, execs, hooks):
    if not healthy or not open_path or len(execs) != 1 or hooks != execs:
        return {"verdict": "INCONCLUSIVE", "completion_eligible": False}
    return {"verdict": "VIOLATION", "completion_eligible": False}
