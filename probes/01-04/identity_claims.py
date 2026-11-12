#!/usr/bin/env python3
"""01-04 P3 account identity from the provider-issued policy-limits document (no model request).

`claude auth status --json` with CLAUDE_CODE_OAUTH_TOKEN returns no account/org fields, so P3 reads the
claims of the JWS the CLI received from the provider and cached in each role's config dir
(`policy-limits.json.signature.json`). Only claim *shapes* (uuid-redacted), SHA-256 prefixes and equality
booleans are printed; no claim value is written.

- provenance: capture time, source file SHA-256 prefix / mtime / size, the command line
- signature: ES256 signature checked with openssl against the leaf certificate in `x5c`; the chain is
  checked with `openssl verify` against the system CA store (result recorded as-is)
- claim meaning (optional `--host-login <path to ~/.claude.json>`): equality of `sub`/`aud`/`ws` with the
  host login's accountUuid / organizationUuid, booleans only. The host login is the PoC development tool's
  credential; it is used only to interpret claim names, never as role identity or runtime input.

Run as: podman unshare <venv python> -I probes/01-04/identity_claims.py [--host-login F] <role config dir> ...
(the role volumes are owned by mapped sub-UIDs, so the host user reads them inside its user namespace)
"""
import base64, hashlib, json, os, re, subprocess, sys, tempfile
from datetime import datetime, timezone
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


def der_int(b: bytes) -> bytes:
    b = b.lstrip(b"\0") or b"\0"
    if b[0] & 0x80:
        b = b"\0" + b
    return b"\x02" + bytes([len(b)]) + b


def verify(h: str, p: str, s: str, hdr: dict) -> dict:
    if hdr.get("alg") != "ES256" or not hdr.get("x5c"):
        return {"checked": False, "reason": f"alg={hdr.get('alg')} x5c={'yes' if hdr.get('x5c') else 'no'}"}
    sig = b64(s)
    body = der_int(sig[:32]) + der_int(sig[32:])
    der = b"\x30" + bytes([len(body)]) + body
    with tempfile.TemporaryDirectory() as td:
        t = Path(td)
        pems = []
        for i, c in enumerate(hdr["x5c"]):
            pem = "-----BEGIN CERTIFICATE-----\n" + "\n".join(c[j:j + 64] for j in range(0, len(c), 64)) + "\n-----END CERTIFICATE-----\n"
            (t / f"c{i}.pem").write_text(pem); pems.append(t / f"c{i}.pem")
        subprocess.run(["openssl", "x509", "-in", str(pems[0]), "-pubkey", "-noout", "-out", str(t / "pub.pem")], check=True, capture_output=True)
        (t / "sig.der").write_bytes(der); (t / "data").write_bytes((h + "." + p).encode())
        r = subprocess.run(["openssl", "dgst", "-sha256", "-verify", str(t / "pub.pem"), "-signature", str(t / "sig.der"), str(t / "data")], capture_output=True, text=True)
        subj = subprocess.run(["openssl", "x509", "-in", str(pems[0]), "-noout", "-subject", "-issuer", "-enddate"], capture_output=True, text=True).stdout.strip().splitlines()
        if len(pems) > 1:
            (t / "chain.pem").write_text("".join(x.read_text() for x in pems[1:]))
            ch = subprocess.run(["openssl", "verify", "-untrusted", str(t / "chain.pem"), str(pems[0])], capture_output=True, text=True)
        else:
            ch = subprocess.run(["openssl", "verify", str(pems[0])], capture_output=True, text=True)
        return {"checked": True, "signature_by_leaf": (r.stdout + r.stderr).strip(), "leaf": subj, "x5c_len": len(pems),
                "chain_verify_system_ca": (ch.stdout + ch.stderr).strip().replace(str(pems[0]), "leaf"),
                "leaf_cert_fp": hashlib.sha256(base64.b64decode(hdr["x5c"][0])).hexdigest()[:12], "kid_fp": fp(hdr.get("kid"))}


def src(f: Path) -> dict:
    st = f.stat()
    return {"file": f.name, "sha256_12": hashlib.sha256(f.read_bytes()).hexdigest()[:12], "size": st.st_size,
            "mtime": datetime.fromtimestamp(st.st_mtime, timezone.utc).isoformat(timespec="seconds")}


def main(argv):
    host = None
    if argv and argv[0] == "--host-login":
        host = json.loads(Path(argv[1]).read_text()).get("oauthAccount") or {}
        argv = argv[2:]
    out = {"captured_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
           "command": "podman unshare .venv-poc/bin/python -I probes/01-04/identity_claims.py" + (" --host-login ~/.claude.json" if host is not None else "") + " " + " ".join(argv)}
    claims_raw = {}
    for d in map(Path, argv):
        sf = d / "policy-limits.json.signature.json"
        jws = json.loads(sf.read_text())["jws"]
        h, p, s = jws.split(".")
        hdr, pl = json.loads(b64(h)), json.loads(b64(p))
        local = json.loads((d / ".claude.json").read_text())
        stamp = json.loads((d / "policy-limits.json.stamp.json").read_text())
        role = d.parent.parent.name
        claims_raw[role] = pl
        out[role] = {
            "sources": [src(sf), src(d / "policy-limits.json"), src(d / "policy-limits.json.stamp.json")],
            "jws_alg": hdr.get("alg"), "jws_header_keys": sorted(hdr),
            "signature": verify(h, p, s, hdr),
            "signature_fp": hashlib.sha256(b64(s)).hexdigest()[:12],
            "claims": {k: {"shape": shape(v) if k in ("typ", "aud", "sub", "ws", "plan") else type(v).__name__, "fp": fp(v)} for k, v in pl.items()},
            "local_only": {"claude.json userID fp": fp(local.get("userID")), "stamp.identity fp": fp(stamp.get("identity")), "stamp.kind": stamp.get("kind")},
        }
        if host is not None:
            out[role]["claim_meaning_vs_host_login"] = {
                f"{c}=={k}": (pl.get(c) is not None and pl.get(c) == host.get(k))
                for c in ("sub", "aud", "ws") for k in ("accountUuid", "organizationUuid")}
    roles = list(claims_raw)
    if len(roles) == 2:
        a, b = (claims_raw[r] for r in roles)
        out["comparison"] = {k: ("same" if a.get(k) == b.get(k) else "different") for k in sorted(set(a) | set(b))}
        out["comparison"]["signature"] = "same" if out[roles[0]]["signature_fp"] == out[roles[1]]["signature_fp"] else "different"
    print(json.dumps(out, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main(sys.argv[1:])
