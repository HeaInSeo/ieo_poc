#!/usr/bin/env python3
"""Two-owner synthetic HTTP fixture. Never logs bearer values or stores real data."""

from http.client import HTTPConnection
from http.server import BaseHTTPRequestHandler, HTTPServer
import ipaddress
import json
from pathlib import Path
from secrets import compare_digest
import sys


ROLES = ("development", "guardrail")
PORT = 18080


def emit(**fields):
    print(json.dumps(fields, sort_keys=True), flush=True)


class Handler(BaseHTTPRequestHandler):
    def log_message(self, _format, *_args):
        pass

    def respond(self, method):
        role = self.path.removeprefix("/objects/")
        bearer = self.headers.get("Authorization", "")
        subject = next((name for name, token in self.server.tokens.items() if compare_digest(bearer, token)), None)
        if subject is None:
            status, body = 401, {"error": "unauthenticated"}
        elif role not in ROLES:
            status, body = 404, {"error": "not-found"}
        elif subject != role:
            status, body = 403, {"error": "denied"}
        elif method == "PUT":
            try:
                size = int(self.headers.get("Content-Length", "0"))
                if not 0 < size <= 128:
                    raise ValueError("unexpected body length")
                value = json.loads(self.rfile.read(size))["value"]
                if not isinstance(value, str) or not value.startswith("synthetic-") or len(value) > 64:
                    raise ValueError("unexpected fixture value")
            except (ValueError, KeyError, TypeError):
                status, body = 400, {"error": "bad-input"}
            else:
                self.server.objects[role] = value
                status, body = 200, {"value": value}
        else:
            status, body = 200, {"value": self.server.objects[role]}
        wire = json.dumps(body).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(wire)))
        self.end_headers()
        self.wfile.write(wire)
        emit(event="request", method=method, subject=subject, object=role, status=status, peer=self.client_address[0])

    def do_GET(self):
        self.respond("GET")

    def do_PUT(self):
        self.respond("PUT")


def serve():
    with HTTPServer(("0.0.0.0", PORT), Handler) as server:
        server.tokens = {role: "Bearer " + Path(f"/p11-auth/{role}").read_text() for role in ROLES}
        server.objects = {role: f"synthetic-{role}-initial" for role in ROLES}
        Path("/run/fixture/ready").touch()
        emit(event="listening")
        server.serve_forever()


def query(address, method, role, value=None):
    address = ipaddress.IPv4Address(address)
    if address not in ipaddress.IPv4Network("10.244.0.0/16") or role not in ROLES or method not in ("GET", "PUT"):
        raise ValueError("expected a synthetic role and private fixture endpoint")
    headers = {"Authorization": "Bearer " + sys.stdin.readline().rstrip("\n"), "Content-Type": "application/json"}
    body = json.dumps({"value": value}).encode() if method == "PUT" else None
    connection = HTTPConnection(str(address), PORT, timeout=2)
    try:
        connection.request(method, f"/objects/{role}", body=body, headers=headers)
        with connection.getresponse() as response:
            emit(status=response.status, body=json.load(response))
    finally:
        connection.close()


if __name__ == "__main__":
    if sys.argv[1:] == ["serve"]:
        serve()
    else:
        query(*sys.argv[1:])
