#!/usr/bin/env python3
"""Measure sealed OCI object bytes and cold/warm first checks from a local layout."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import tempfile
import time

HERE = Path(__file__).resolve().parent
BASIC = HERE.parent.parent / "03-06/p22-basic-20261119"
PIN = "24eb5ec20af93de1f99d7fa219773fb9f0b43687"


def load(path):
    return json.loads(path.read_text())


def run(argv, *, env=None, cwd=None, timeout=240):
    return subprocess.run(argv, env=env, cwd=cwd, timeout=timeout, capture_output=True)


def checked(argv, *, env=None, cwd=None):
    result = run(argv, env=env, cwd=cwd)
    if result.returncode:
        raise RuntimeError(f"{shlex.join(argv)}: {result.stderr.decode(errors='replace')}")
    return result.stdout


def read_oci(layout, approved):
    descriptor, = load(layout / "index.json")["manifests"]

    def checked_blob(entry):
        digest = entry["digest"]
        algorithm, name = digest.split(":")
        assert algorithm == "sha256" and len(name) == 64 and name.isalnum()
        path = layout / "blobs" / algorithm / name
        assert path.stat().st_size == entry["size"]
        with path.open("rb") as stream:
            assert hashlib.file_digest(stream, "sha256").hexdigest() == name
        return path

    manifest = load(checked_blob(descriptor))
    config = load(checked_blob(manifest["config"]))
    assert manifest["config"]["digest"] == f"sha256:{approved['id']}", layout
    assert (config["os"], config["architecture"]) == ("linux", "amd64")
    assert len(config["rootfs"]["diff_ids"]) == len(manifest["layers"])
    for layer in manifest["layers"]:
        checked_blob(layer)
    return {"approved": approved, "exported": descriptor, "config": manifest["config"],
            "layers": manifest["layers"], "diff_ids": config["rootfs"]["diff_ids"]}


def objects(*images):
    seen = {}
    for image in images:
        for entry in (image["exported"], image["config"], *image["layers"]):
            old = seen.setdefault(entry["digest"], entry["size"])
            assert old == entry["size"]
    return {"count": len(seen), "bytes": sum(seen.values())}


def measure(layout, image, stack, fixture, repo_sha, label, logs):
    cold = Path(tempfile.mkdtemp(prefix="ieo-c22-", dir="/tmp"))
    conf = cold / "storage.conf"
    (cold / "run").mkdir()
    (cold / "tmp").mkdir()
    conf.write_text(f"""[storage]
driver="overlay"
graphroot="{cold}/graph"
rootless_storage_path="{cold}/graph"
runroot="{cold}/run"
[storage.options]
additionalimagestores=[]
""")
    env = {**os.environ, "CONTAINERS_STORAGE_CONF": str(conf), "TMPDIR": str(cold / "tmp")}
    prefix = ["podman", "--runroot", str(cold / "run"), "--tmpdir", str(cold / "tmp")]

    def podman(*args):
        return checked([*prefix, *args], env=env)

    actual = json.loads(podman("info", "--format", "{{json .Store}}"))
    assert Path(actual["graphRoot"]).resolve() == cold / "graph"
    assert Path(actual["runRoot"]).resolve() == cold / "run"
    assert actual["imageStore"]["number"] == 0
    identity = image["config"]["digest"]
    args = [*prefix, "run", "--rm", "--pull=never", "--network=none", "--read-only",
            "--cap-drop=all", "--security-opt=no-new-privileges", "--user", "20001:20001",
            "--tmpfs", "/tmp:rw,exec,nosuid,nodev,mode=1777,size=512m",
            "-v", f"{fixture}:/work:ro,Z", identity, stack]
    data = []
    try:
        for state in ("cold", "warm"):
            started = time.monotonic()
            import_seconds = None
            if state == "cold":
                command = [*prefix, "pull", "--quiet", f"oci:{layout}"]
                load_result = run(command, env=env)
                (logs / f"{label}-pull.log").write_bytes(load_result.stdout + load_result.stderr)
                if load_result.returncode:
                    raise RuntimeError(f"OCI load {label}: {load_result.stderr.decode(errors='replace')}")
                import_seconds = time.monotonic() - started
            current, = json.loads(podman("image", "inspect", identity))
            assert current["Id"].removeprefix("sha256:") == identity.removeprefix("sha256:")
            assert current["RootFS"]["Layers"] == image["diff_ids"]
            run_start = time.monotonic()
            result = run(args, env=env)
            elapsed = time.monotonic()
            (logs / f"{label}-{state}.stdout").write_bytes(result.stdout)
            (logs / f"{label}-{state}.stderr").write_bytes(result.stderr)
            expected = f"PASS profile={stack} version=0.1.0 catalog=0.2.2".encode()
            if result.returncode or expected not in result.stdout.splitlines():
                raise RuntimeError(f"{label} {state}: required verification not observed")
            data.append({
                "label": label, "cache": state, "status": "PASS", "source_sha": repo_sha,
                "image_config": identity, "local_manifest": image["exported"]["digest"],
                "import_seconds": None if import_seconds is None else round(import_seconds, 3),
                "verify_seconds": round(elapsed - run_start, 3), "ready_seconds": round(elapsed - started, 3),
                "transferred_oci_bytes": objects(image)["bytes"] if state == "cold" else 0,
                "verify_argv": args,
            })
    finally:
        assert podman("ps", "--all", "--quiet") == b"", f"unexpected container in {cold}"
        staged = [line.removeprefix("sha256:") for line in
                  podman("images", "--all", "--quiet", "--no-trunc").decode().splitlines()]
        if staged:
            assert set(staged) == {identity.removeprefix("sha256:")}, f"unexpected image in {cold}"
            podman("image", "rm", identity)
        shutil.rmtree(cold)
    return data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("layouts", type=Path)
    parser.add_argument("out", type=Path)
    parser.add_argument("--pilot", action="store_true")
    args = parser.parse_args()
    layouts, out = args.layouts.resolve(), args.out.resolve()
    assert all(p.is_relative_to("/tmp") and p != Path("/tmp") for p in (layouts, out))
    out.mkdir()
    logs = out / "logs"
    logs.mkdir()
    fixtures = out / "fixtures"
    fixtures.mkdir()
    p22 = load(BASIC / "evidence/results.json")
    images = {variant: read_oci(layouts / variant, approved) for variant, approved in p22["images"].items()}
    static = {"images": images, "payload": {variant: objects(data) for variant, data in images.items()},
              "both": {"A": objects(images["combined"]), "B": objects(images["go"], images["rust"])}}
    (out / "objects.json").write_text(json.dumps(static, indent=2) + "\n")

    spec = importlib.util.spec_from_file_location("p22_basic", BASIC / "run-p22.py")
    assert spec and spec.loader
    previous = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(previous)
    repo = Path(checked(["git", "rev-parse", "--show-toplevel"], cwd=HERE).decode().strip())
    samples = {(stack, label): previous.create_repo(repo, fixtures, PIN, stack, label)
               for stack in ("go", "rust") for label in ("a", "b")}
    for (stack, label), (_, head) in samples.items():
        matches = [case for case in p22["cases"] if case["id"].startswith(f"{stack}-{label}-")
                   and case["kind"] == "normal"]
        assert matches and {case["source"] for case in matches} == {head}

    pairs = [(stack, label, variant) for stack in ("go", "rust") for label in ("a", "b")
             for variant in ((stack, "combined") if label == "a" else ("combined", stack))]
    if args.pilot:
        pairs = pairs[:1]
    measurements = []
    for stack, label, variant in pairs:
        fixture, head = samples[(stack, label)]
        key = f"{stack}-{label}-{variant}"
        rows = measure(layouts / variant, images[variant], stack, fixture, head, key, logs)
        measurements.extend(rows)
        print(key, [(row["cache"], row["ready_seconds"]) for row in rows], flush=True)
        (out / "readiness.json").write_text(json.dumps(measurements, indent=2) + "\n")
    assert len(measurements) == (2 if args.pilot else 16)


if __name__ == "__main__":
    main()
