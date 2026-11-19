#!/usr/bin/env python3
"""Synthetic actions and receiver evidence for the ieo-0802 network only."""

import argparse
import ipaddress
import json
import os
from pathlib import Path
import re
import signal
import socket
import subprocess
import time


PORT = 18080
POD_NETWORK = ipaddress.IPv4Network("10.244.0.0/16")
READY = Path("/run/fixture/ready")
CANARY = Path("/fixture/ieo-0802-canary.txt")
TIMEOUT = 2


def lab_address(text):
    try:
        address = ipaddress.IPv4Address(text)
    except ipaddress.AddressValueError as error:
        raise argparse.ArgumentTypeError("expected a numeric IPv4 Pod address") from error
    if address not in POD_NETWORK or address in (POD_NETWORK.network_address, POD_NETWORK.broadcast_address):
        raise argparse.ArgumentTypeError("destination is outside the fixture Pod network")
    return str(address)


def request_id(value):
    if re.fullmatch(r"0802-[a-z0-9-]{1,40}", value) is None:
        raise argparse.ArgumentTypeError("expected an 0802- synthetic request ID")
    return value


def emit(event, **fields):
    fields.update(event=event, time_ns=time.time_ns(), pid=os.getpid(), uid=os.getuid())
    fields.update({key.lower(): os.environ.get(key) for key in ("POD_UID", "POD_NAME", "NODE_NAME", "FIXTURE_ROLE")})
    fields["process_start_ticks"] = Path("/proc/self/stat").read_text().rsplit(")", 1)[1].split()[19]
    print(json.dumps(fields, sort_keys=True), flush=True)


def payload(value):
    return (value + "\n").encode("ascii")


def request(address, identifier):
    message = payload(identifier)
    with socket.create_connection(address, timeout=TIMEOUT) as client:
        client.sendall(message)
        with client.makefile("rb") as reply:
            if reply.readline(64) != message:
                raise ValueError("receiver returned an invalid reply")


def accept_one(listener):
    connection, peer = listener.accept()
    with connection:
        emit("accepted", peer=peer[0])
        connection.settimeout(TIMEOUT)
        try:
            with connection.makefile("rb") as stream:
                line = stream.readline(64)
            identifier = request_id(line.decode("ascii").rstrip("\n"))
            if line != payload(identifier):
                raise ValueError("invalid framing")
            emit("received", request_id=identifier, peer=peer[0])
            connection.sendall(line)
        except (OSError, ValueError, argparse.ArgumentTypeError) as error:
            emit("receiver_error", peer=peer[0], error=type(error).__name__)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    for action in ("idle", "serve"):
        sub.add_parser(action)
    for action in ("open", "exec", "connect"):
        command = sub.add_parser(action)
        command.add_argument("id", type=request_id)
        if action == "connect":
            command.add_argument("address", type=lab_address)
    args = parser.parse_args()
    if args.action == "idle":
        signal.pause()
        return 0
    if args.action == "serve":
        with socket.create_server(("0.0.0.0", PORT)) as listener:
            READY.touch()
            emit("listening", port=PORT)
            while True:
                accept_one(listener)

    emit("attempt", action=args.action, request_id=args.id, address=getattr(args, "address", None))
    try:
        if args.action == "connect":
            request((args.address, PORT), args.id)
        elif args.action == "open":
            directory = os.open(CANARY.parent, os.O_RDONLY | os.O_DIRECTORY)
            try:
                # Supplying dir_fd selects openat even when the pathname is absolute.
                with os.fdopen(os.open(CANARY, os.O_RDONLY, dir_fd=directory)) as canary:
                    if canary.read() != "ieo-0802 synthetic canary\n":
                        raise ValueError("unexpected canary")
            finally:
                os.close(directory)
        else:
            subprocess.run(["/bin/true"], check=True)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        emit("result", action=args.action, request_id=args.id, ok=False, error=type(error).__name__)
        return 1
    emit("result", action=args.action, request_id=args.id, ok=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
