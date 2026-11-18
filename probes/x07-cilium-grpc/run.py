#!/usr/bin/env python3
"""Run and evaluate the frozen X-07 Cilium kind criteria."""

from collections import Counter
import json
from pathlib import Path
import sys
import time
from urllib.parse import urlsplit

# Run the X-05 scenario and adjudication on Cilium with a distinct frontend.
sys.path.insert(0, str(Path(__file__).resolve().parent.with_name("x05-kind-grpc")))
from run import Client, FIXTURE_VALUE, Trial, last_event, main, reply, require, samples

CONTEXT = "kind-ieo-x07"
NAMESPACE = "x07-probe"
BASE_WORKLOAD = Path(__file__).resolve().parent.with_name("x05-kind-grpc") / "workload.yaml"


def service_ip(service):
    address = service["spec"]["clusterIP"]
    require(address != "None", "expected a ClusterIP")
    return f"{address}:50051"


def one_connection(events, records, address):
    first = min(e["data"]["start_ms"] for e in records)
    end = max(e["at_ms"] for e in records)
    connects = [e for e in events if e["event"] == "connect" and e["at_ms"] <= end]
    require(connects and connects[-1]["at_ms"] <= first,
            f"connection changed during the fixed window: {connects[-2:]}")
    require(all(e["data"]["address"] == address for e in connects), "TCP peer left the Service VIP")
    return connects[-1]["data"]["local"]


class CiliumTrial(Trial):
    """Run and evaluate the frozen X-07 Cilium kind criteria."""

    def __init__(self, kubeconfig, recorder):
        super().__init__(kubeconfig, recorder, CONTEXT, NAMESPACE)

    def client_command(self, service="probe"):
        return self.command + ["-n", self.namespace, "exec", "-i", "client", "--",
                               "/ieo-x05-probe", "client-vip",
                               f"{service}.{self.namespace}.svc.cluster.local."]

    def bootstrap(self):
        config = json.loads(self.kubectl("config", "view", "--minify", "-o", "json"))
        target = config["clusters"][0]
        require(target["name"] == CONTEXT and urlsplit(target["cluster"]["server"]).hostname == "127.0.0.1",
                "kubeconfig must contain the dedicated loopback kind context")
        nodes = self.get("nodes")
        self.workers = {"ieo-x07-worker", "ieo-x07-worker2", "ieo-x07-worker3"}
        require({n["metadata"]["name"] for n in nodes["items"]} == self.workers | {"ieo-x07-control-plane"},
                "expected the dedicated four-node cluster")
        require(all(any(c["type"] == "Ready" and c["status"] == "True"
                        for c in n["status"]["conditions"]) for n in nodes["items"]), "nodes not Ready")
        daemonsets = self.get("daemonsets", "-A")
        ready = {ds["metadata"]["name"]: ds["status"].get("numberReady") for ds in daemonsets["items"]}
        require(ready.get("cilium") == ready.get("cilium-envoy") == 4,
                f"Cilium agent and Envoy are not Ready on four nodes: {ready}")
        require("kindnet" not in ready and "kube-proxy" not in ready, "unexpected datapath")
        settings = self.get("configmap", "cilium-config", namespace="kube-system")
        require(settings["data"].get("kube-proxy-replacement") == "true"
                and settings["data"].get("loadbalancer-l7") == "envoy", "unexpected Cilium flags")
        self.recorder.snapshot("cluster", {"nodes": nodes, "daemonsets": daemonsets, "config": settings,
                                           "version": json.loads(self.kubectl("version", "-o", "json"))})

        overlay = self.recorder.path / "workload.yaml"
        text = BASE_WORKLOAD.read_text()
        require(text.count("  clusterIP: None\n") == 1, "X-05 fixture Service layout changed")
        overlay.write_text(text.replace("x05-probe", NAMESPACE).replace("  clusterIP: None\n", ""))
        self.kubectl("create", "-f", str(overlay), timeout=30)
        self.kubectl("expose", "deployment", "probe", "--name=probe-control", "--port=50051",
                     "--target-port=50051", "--type=ClusterIP")
        self.kubectl("annotate", "service", "probe", "service.cilium.io/lb-l7=enabled",
                     "service.cilium.io/lb-l7-algorithm=round_robin")
        proxy_deadline = time.monotonic() + 120
        self.kubectl("wait", "--for=condition=Ready", "pod/client", "--timeout=120s", timeout=120)
        self.initial_client = self.get("pod", "client")
        self.recorder.snapshot("client-pod", self.initial_client)
        svc = self.get("service", "probe")
        self.recorder.snapshot("service", svc)
        self.address = service_ip(svc)
        control = self.get("service", "probe-control")
        self.recorder.snapshot("service-control", control)
        require(svc["spec"]["selector"] == control["spec"]["selector"], "control uses different Pods")
        require(not control["metadata"].get("annotations", {}).get("service.cilium.io/lb-l7"),
                "L7 is enabled on the control")
        self.control_address = service_ip(control)
        self.observe_proxy(proxy_deadline)

    def observe_proxy(self, deadline):
        node = self.initial_client["spec"]["nodeName"]
        pods = self.get("pods", "-l", "k8s-app=cilium", "--field-selector", f"spec.nodeName={node}",
                        namespace="kube-system")["items"]
        require(len(pods) == 1, f"no distinct Cilium agent for client node {node}")
        agent = pods[0]["metadata"]["name"]
        seen = False
        while time.monotonic() < deadline:
            objects = self.get("ciliumenvoyconfigs")
            matching = [
                obj for obj in objects["items"]
                if any(s["name"] == "probe" and s.get("namespace", NAMESPACE) == NAMESPACE
                       for s in obj["spec"].get("services", []))
            ]
            if len(matching) == 1:
                self.recorder.snapshot("generated-envoy-config", matching[0])
                cluster = next((r for r in matching[0]["spec"]["resources"]
                                if r.get("@type", "").endswith(".Cluster")), None)
                require(cluster and cluster.get("type") == "EDS"
                        and cluster.get("lb_policy", cluster.get("lbPolicy", "ROUND_ROBIN")) == "ROUND_ROBIN",
                        "expected a round-robin EDS cluster")
                config = json.dumps(cluster).replace("_", "").lower()
                require("http2protocoloptions" in config, "expected upstream HTTP/2 settings")
                seen = True
                try:
                    active = self.kubectl("exec", agent, "-c", "cilium-agent", "--", "cilium-dbg",
                                          "envoy", "admin", "config", "clusters", namespace="kube-system")
                except RuntimeError:
                    time.sleep(1)
                    continue
                (self.recorder.path / "active-envoy-clusters.txt").write_text(active)
                if f"{NAMESPACE}/probe" in active and "http2protocoloptions" in active.replace("_", "").lower():
                    self.recorder.emit("proxy", node=node, agent=agent, vip=self.address, algorithm="round_robin")
                    return
            time.sleep(1)
        require(False, f"active Envoy cluster unverified (CiliumEnvoyConfig present: {seen})")

    def before_trial(self):
        _, expected = self.stable("control", 2, "v1", set(), set(), time.monotonic() + 120)
        dest = self.recorder.path / "control"
        dest.mkdir()
        with (dest / "client.stderr").open("x", encoding="utf-8") as log:
            client = Client(self.client_command("probe-control"), log)
            try:
                client.wait(lambda events: next((e for e in samples(events) if reply(e)), None),
                            time.monotonic() + 10, "successful control connection")
                mark = client.mark("control")
                index = mark["data"]["next_index"]

                def complete(events):
                    data = [e for e in samples(events) if index <= e["data"]["index"] < index + 30]
                    return data if len(data) >= 30 else None

                window = client.wait(complete, time.monotonic() + 6, "30 control reads")
                records = sorted(window, key=lambda e: e["data"]["index"])
                require([e["data"]["index"] for e in records] == list(range(index, index + 30)),
                        "control sample IDs were repeated or skipped")
                uid = reply(records[0]).get("uid")
                require(uid in expected, "unexpected control endpoint")
                for e in records:
                    response = reply(e)
                    require(response.get("uid") == uid and response["request_id"] == e["data"]["request_id"]
                            and response["value"] == FIXTURE_VALUE, f"incorrect control result: {e}")
                require(2_800 <= records[-1]["data"]["start_ms"] - records[0]["data"]["start_ms"] <= 3_100,
                        "control issue span outside 2.8–3.1 seconds")
                require(last_event(client.events, "frontend", address=self.control_address),
                        "control DNS did not resolve to the Service VIP")
                connection = one_connection(client.events, window, self.control_address)
                self.recorder.emit("control", counts=Counter(reply(e)["uid"] for e in records),
                                   local=connection)
            finally:
                client.close()

    def frontend_matches(self, events, expected, deadline):
        return any(e["event"] == "frontend" and e["data"]["address"] == self.address
                   and e["received_monotonic"] <= deadline for e in events)

    def window(self, stage, observed, expected, boundary=None):
        records = super().window(stage, observed, expected, boundary)
        connection = self.check(stage, one_connection, self.client.events, records, self.address)
        if connection:
            self.recorder.emit("window_connection", stage=stage, local=connection, vip=self.address)


if __name__ == "__main__":
    raise SystemExit(main(CiliumTrial))
