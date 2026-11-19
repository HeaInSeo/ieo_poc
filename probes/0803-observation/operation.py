#!/usr/bin/env python3
"""Run one known action inside a disposable fixture Pod."""

import json
import os
from pathlib import Path
import socket
import sys
import time


marker, action, destination = sys.argv[1:]
start_ticks = int(Path("/proc/self/stat").read_text().rsplit(")", 1)[1].split()[19])
boot_ns = time.clock_gettime_ns(time.CLOCK_BOOTTIME)
wall_ns = time.time_ns()
identity = {
    "marker": marker, "action": action, "destination": destination,
    "uid": os.getuid(), "pid": os.getpid(), "binary": os.readlink("/proc/self/exe"),
    "pod_uid": os.environ["POD_UID"], "role": os.environ["FIXTURE_ROLE"],
    "clock_ticks": os.sysconf("SC_CLK_TCK"), "start_ticks": start_ticks,
    "sample_boot_ns": boot_ns, "sample_wall_ns": wall_ns,
}
print(json.dumps({"phase": "attempt", **identity}), flush=True)

if action == "open":
    directory = os.open("/fixture", os.O_RDONLY | os.O_DIRECTORY)
    try:
        with os.fdopen(os.open(destination, os.O_RDONLY, dir_fd=directory)) as canary:
            if canary.read() != "p09 known fixture\n":
                raise ValueError("incorrect canary payload")
    finally:
        os.close(directory)
elif action == "connect":
    message = (marker + "\n").encode("ascii")
    with socket.create_connection((destination, 18080), timeout=3) as client:
        client.sendall(message)
        with client.makefile("rb") as reply:
            if reply.readline(64) != message:
                raise ValueError("receiver did not echo the fixture marker")
else:
    raise ValueError("unknown fixture action")

print(json.dumps({"phase": "result", **identity}), flush=True)
