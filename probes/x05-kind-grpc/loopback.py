#!/usr/bin/env python3
"""Exercise the fixture on localhost when kind is unavailable."""

import argparse
import json
import os
from pathlib import Path
import socket
import subprocess
import time

from run import (
    Client, FIXTURE_VALUE, SAMPLE_COUNT, Recorder, check_stream, check_window,
    last_event, reply, require, samples, stream_items,
)


def start(binary, port, uid, evidence):
    with (evidence / f"server-{uid}.jsonl").open("x", encoding="utf-8") as stdout:
        return subprocess.Popen(
            [binary, "server"],
            env={**os.environ, "POD_NAME": uid, "POD_UID": uid,
                 "PROBE_REVISION": "loopback", "PROBE_LISTEN": f"127.0.0.1:{port}"},
            stdout=stdout,
        )


def read_log(path):
    return [json.loads(line) for line in path.read_text().splitlines()]


def five_items(events, request_id):
    items = stream_items(events, request_id)
    return items if len(items) >= 5 else None


def run(binary, recorder):
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]

    evidence = recorder.path
    os.environ["PROBE_SERVICE"] = "localhost"
    os.environ["PROBE_PORT"] = str(port)
    server = start(binary, port, "first", evidence)
    try:
        with (evidence / "client.stderr").open("x", encoding="utf-8") as stderr:
            client = Client([binary, "client"], stderr)
            try:
                client.wait(lambda events: last_event(events, "addresses", current=[f"127.0.0.1:{port}"]),
                            time.monotonic() + 5, "localhost discovery")
                client.wait(lambda events: next((e for e in samples(events) if reply(e)), None),
                            time.monotonic() + 5, "initial connection")
                client.send("watch original 0")
                items = client.wait(lambda events: five_items(events, "original"),
                                    time.monotonic() + 10, "initial stream")
                backend = {"first": {"pod": "first", "revision": "loopback"}}
                check_stream(items, 1, backend)
                killed = time.monotonic()
                server.kill()
                server.wait(timeout=2)
                end = client.wait(lambda events: last_event(events, "watch_end", request_id="original"),
                                  killed + 10, "stream interruption")
                items = stream_items(client.events, "original")
                check_stream(items, 1, backend)
                seq = items[-1]["seq"]
                recorder.emit("watch_interrupted", last_seq=seq, terminal=end["data"],
                              elapsed_ms=round((end["received_monotonic"] - killed) * 1000))

                server = start(binary, port, "second", evidence)
                client.wait(
                    lambda events: next((e for e in samples(events) if reply(e).get("uid") == "second"), None),
                    time.monotonic() + 10, "reconnect to same address",
                )
                client.send("close-watch")
                client.send(f"watch resumed {seq}")
                resumed = client.wait(lambda events: five_items(events, "resumed"),
                                      time.monotonic() + 10, "resumed stream")
                check_stream(resumed, seq + 1, {"second": {"pod": "second", "revision": "loopback"}}, "first")
                client.send("close-watch")
                recorder.emit("watch_resumed", sequences=[item["seq"] for item in resumed])

                client.send("hold reissue-loopback")
                second_log = evidence / "server-second.jsonl"
                deadline = time.monotonic() + 5
                while not read_log(second_log):
                    require(time.monotonic() < deadline, "held request did not reach server")
                    time.sleep(0.02)
                initial = read_log(second_log)
                require(initial == [{"event": "read_started", "request_id": "reissue-loopback",
                                     "uid": "second", "hold": True}], "original hold was not unique")
                killed = time.monotonic()
                server.kill()
                server.wait(timeout=2)
                failed = client.wait(lambda events: last_event(events, "read", request_id="reissue-loopback", hold=True),
                                     killed + 10, "unary interruption")
                outcome = failed["data"]["outcome"]
                require("code" in outcome, f"expected transport failure: {outcome}")
                recorder.emit("unary_interrupted", outcome=outcome,
                              elapsed_ms=round((failed["received_monotonic"] - killed) * 1000))

                server = start(binary, port, "third", evidence)
                client.wait(lambda events: next((e for e in samples(events) if reply(e).get("uid") == "third"), None),
                            time.monotonic() + 10, "reconnect for reissue")
                third_log = evidence / "server-third.jsonl"
                require(not read_log(third_log), "read replayed before explicit reissue")
                client.send("read reissue-loopback")
                retried = client.wait(
                    lambda events: last_event(events, "read", request_id="reissue-loopback", hold=False),
                    time.monotonic() + 3, "unary reissue",
                )
                response = reply(retried)
                require(response.get("uid") == "third" and response["request_id"] == "reissue-loopback"
                        and response["value"] == FIXTURE_VALUE, f"wrong reissue: {retried}")
                require(read_log(third_log) == [{"event": "read_started", "request_id": "reissue-loopback",
                                                 "uid": "third", "hold": False}], "reissue was not unique")
                recorder.emit("unary_reissued", reply=response)

                mark = client.mark("loopback-window")
                index = mark["data"]["next_index"]

                def complete(events):
                    window = [e for e in samples(events) if index <= e["data"]["index"] < index + SAMPLE_COUNT]
                    return window if len(window) == SAMPLE_COUNT else None

                window = client.wait(complete, time.monotonic() + 34, "300 results")
                counts = check_window(window, {"third": {"pod": "third", "revision": "loopback"}}, index)
                recorder.emit("window", counts=counts,
                              issue_span_ms=window[-1]["data"]["start_ms"] - window[0]["data"]["start_ms"])
            finally:
                client.close()
    finally:
        if server.poll() is None:
            server.kill()
            server.wait(timeout=2)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("evidence", type=Path)
    args = parser.parse_args()
    args.evidence.mkdir()
    recorder = Recorder(args.evidence)
    try:
        run(args.binary.resolve(), recorder)
        recorder.emit("result", verdict="PASS", scope="loopback")
    finally:
        recorder.log.close()


if __name__ == "__main__":
    main()
