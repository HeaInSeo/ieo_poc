"""One fixed marker to one fixture IPv4 receiver."""

import ipaddress
import json
import os
from pathlib import Path
import socket
import sys
import time

MARKER = "0802-p10-fixture"
PORT = 18080


def main():
    marker, text = sys.argv[1:]
    destination = ipaddress.IPv4Address(text)
    if marker != MARKER or destination not in ipaddress.IPv4Network("10.244.0.0/16"):
        raise ValueError("outside fixed test scope")
    action = {
        "marker": marker, "destination": str(destination), "action": "connect",
        "uid": os.getuid(), "pid": os.getpid(), "binary": os.readlink("/proc/self/exe"),
        "pod_uid": os.environ["POD_UID"], "role": os.environ["FIXTURE_ROLE"],
        "clock_ticks": os.sysconf("SC_CLK_TCK"),
        "start_ticks": int(Path("/proc/self/stat").read_text().rsplit(")", 1)[1].split()[19]),
        "sample_boot_ns": time.clock_gettime_ns(time.CLOCK_BOOTTIME),
        "sample_wall_ns": time.time_ns(),
    }
    print(json.dumps({"phase": "attempt", **action}), flush=True)
    stage = "connect"
    try:
        with socket.create_connection((str(destination), PORT), timeout=3) as connection:
            stage = "echo"
            wire = (MARKER + "\n").encode("ascii")
            connection.sendall(wire)
            with connection.makefile("rb") as reply:
                if reply.readline(64) != wire:
                    raise ValueError("marker echo mismatch")
    except (OSError, ValueError) as error:
        print(json.dumps({"phase": "result", "ok": False, "stage": stage,
                          "error": type(error).__name__, **action}), flush=True)
        return 1
    print(json.dumps({"phase": "result", "ok": True, "stage": "echo", **action}), flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
