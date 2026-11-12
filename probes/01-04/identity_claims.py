#!/usr/bin/env python3
"""01-04 P3 account identity from the provider-issued policy-limits document (no model request).

`claude auth status --json` with CLAUDE_CODE_OAUTH_TOKEN returns no account/org fields, so P3 reads the
claims of the JWS the CLI received from the provider and cached in each role's config dir
(`policy-limits.json.signature.json`). Only claim *shapes* (uuid-redacted) and SHA-256 prefixes are printed.
The JWS signature is not verified here (no crypto library in the venv); this is recorded as a limit.

Run as: podman unshare <venv python> -I probes/01-04/identity_claims.py <role config dir> ...
(the role volumes are owned by mapped sub-UIDs, so the host user reads them inside its user namespace)
"""
import base64, hashlib, json, re, sys
from pathlib import Path

UUID = re.compile(r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}")


def fp(v) -> str:
    return hashlib.sha256(json.dumps(v, sort_keys=True).encode()).hexdigest()[:12]


def shape(v):
    if isinstance(v, str):
        return re.sub(r"[A-Za-z0-9_\-]{24,}", "<id>", UUID.sub("<uuid>", v))
    return type(v).__name__


def b64(x: str) -> bytes:
    return base64.urlsafe_b64decode(x + "=" * (-len(x) % 4))


def main(dirs):
    out = {}
    for d in map(Path, dirs):
        jws = json.loads((d / "policy-limits.json.signature.json").read_text())["jws"]
        h, p, _ = jws.split(".")
        hdr, pl = json.loads(b64(h)), json.loads(b64(p))
        local = json.loads((d / ".claude.json").read_text())
        stamp = json.loads((d / "policy-limits.json.stamp.json").read_text())
        out[d.parent.parent.name] = {
            "jws_alg": hdr.get("alg"), "jws_header_keys": sorted(hdr),
            "claims": {k: {"shape": shape(v) if k in ("typ", "aud", "sub", "ws", "plan") else type(v).__name__, "fp": fp(v)} for k, v in pl.items()},
            "local_only": {"claude.json userID fp": fp(local.get("userID")), "stamp.identity fp": fp(stamp.get("identity")), "stamp.kind": stamp.get("kind")},
        }
    roles = list(out)
    if len(roles) == 2:
        a, b = (out[r]["claims"] for r in roles)
        out["comparison"] = {k: ("same" if a.get(k, {}).get("fp") == b.get(k, {}).get("fp") else "different") for k in sorted(set(a) | set(b))}
    print(json.dumps(out, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main(sys.argv[1:])
