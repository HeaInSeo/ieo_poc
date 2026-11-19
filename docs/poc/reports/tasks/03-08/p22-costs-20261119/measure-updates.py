#!/usr/bin/env python3
"""Measure local additive common-policy and Go fixture layer updates."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import tempfile
import time
import tomllib

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("local_costs", HERE / "measure-local.py")
assert spec and spec.loader
local = importlib.util.module_from_spec(spec)
spec.loader.exec_module(local)
COMMON = "docs/poc/reports/tasks/03-02/common-policy-20261118/COMMON_POLICY.md"
BUNDLE = "/opt/ieo-policy/bundle.sha256"


def blob_sizes(images):
    return {entry["digest"]: entry["size"] for i in images
            for entry in (i["exported"], i["config"], *i["layers"])}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source_store", type=Path, help="read-only image store used by P22-B")
    parser.add_argument("baseline", type=Path, help="output from measure-local.py")
    parser.add_argument("out", type=Path)
    args = parser.parse_args()
    source, baseline, out = (p.resolve() for p in (args.source_store, args.baseline, args.out))
    assert all(p.is_relative_to("/tmp") and p != Path("/tmp") for p in (source, baseline, out))
    storage_input = tomllib.loads((source / "storage.conf").read_text())["storage"]
    assert Path(storage_input["rootless_storage_path"]).resolve() == source / "graph"
    read_only_stores = [str(source / "graph"), *storage_input["options"]["additionalimagestores"]]
    old = local.load(baseline / "objects.json")["images"]
    rows = local.load(baseline / "readiness.json")
    assert len(rows) == 16 and all(row["status"] == "PASS" for row in rows)
    out.mkdir()
    logs = out / "logs"
    logs.mkdir()
    container_store = Path(tempfile.mkdtemp(prefix="ieo-u22-", dir="/tmp"))
    (container_store / "run").mkdir()
    (container_store / "tmp").mkdir()
    storage = container_store / "storage.conf"
    storage.write_text(f"""[storage]
driver="overlay"
graphroot="{container_store}/graph"
rootless_storage_path="{container_store}/graph"
runroot="{container_store}/run"
[storage.options]
additionalimagestores={json.dumps(read_only_stores)}
""")
    env = {**os.environ, "CONTAINERS_STORAGE_CONF": str(storage), "TMPDIR": str(container_store / "tmp")}
    prefix = ["podman", "--runroot", str(container_store / "run"), "--tmpdir", str(container_store / "tmp")]

    def podman(*options):
        return local.checked([*prefix, *options], env=env)

    def capture(label, *options):
        result = local.run([*prefix, *options], env=env, timeout=300)
        (logs / f"{label}.stdout").write_bytes(result.stdout)
        (logs / f"{label}.stderr").write_bytes(result.stderr)
        if result.returncode:
            raise RuntimeError(f"{label} exited {result.returncode}")
        return result.stdout

    actual = json.loads(podman("info", "--format", "{{json .Store}}"))
    assert Path(actual["graphRoot"]).resolve() == container_store / "graph"
    assert Path(actual["runRoot"]).resolve() == container_store / "run"
    for variant, image in old.items():
        current, = json.loads(podman("image", "inspect", image["config"]["digest"]))
        assert current["Digest"] == image["approved"]["digest"], variant

    runtime = ("run", "--rm", "--pull=never", "--network=none", "--read-only",
               "--cap-drop=all", "--security-opt=no-new-privileges", "--user", "20001:20001")
    common = podman(*runtime, "--entrypoint", "/usr/bin/cat", old["combined"]["config"]["digest"],
                    f"/opt/ieo-policy/source/{COMMON}")
    manifest = podman(*runtime, "--entrypoint", "/usr/bin/cat", old["combined"]["config"]["digest"], BUNDLE)
    previous = hashlib.sha256(common).hexdigest()
    updated_common = common + b"\n<!-- P22-C local common-policy update comparison -->\n"
    updated_hash = hashlib.sha256(updated_common).hexdigest()
    needle = f"{previous}  ./{COMMON}".encode()
    assert manifest.count(needle) == 1
    updated_manifest = manifest.replace(needle, f"{updated_hash}  ./{COMMON}".encode())

    results = {}
    for kind, variants in (("common", ("go", "rust", "combined")), ("go", ("go", "combined"))):
        for variant in variants:
            name = f"{kind}-{variant}"
            image = old[variant]
            work = out / name
            work.mkdir()
            overlay = work / "context/overlay"
            overlay.mkdir(parents=True)
            if kind == "common":
                target = overlay / "opt/ieo-policy/source" / COMMON
                target.parent.mkdir(parents=True)
                target.write_bytes(updated_common)
                (overlay / BUNDLE.lstrip("/")).write_bytes(updated_manifest)
            else:
                target = overlay / "opt/ieo-tools/go/P22-C-UPDATE.txt"
                target.parent.mkdir(parents=True)
                target.write_text("local synthetic Go payload revision\n")
            (overlay.parent / "Containerfile").write_text(f"FROM {image['config']['digest']}\nCOPY overlay/ /\n")
            tag = f"localhost/ieo-p22-c-update:{name}"
            start = time.monotonic()
            capture(f"{name}-build", "build", "--network=none", "--pull=never", "--timestamp", "0",
                    "--tag", tag, str(overlay.parent))
            build_seconds = round(time.monotonic() - start, 3)
            tagged, = json.loads(podman("image", "inspect", tag))
            try:
                capture(f"{name}-save", "save", "--format", "oci-dir", "--output", str(work / "oci"), tag)
                updated = local.read_oci(work / "oci", {"id": tagged["Id"].removeprefix("sha256:"), "digest": tagged["Digest"]})
                assert updated["exported"]["digest"] != image["exported"]["digest"]
                old_layers = [d["digest"] for d in image["layers"]]
                assert [d["digest"] for d in updated["layers"][:-1]] == old_layers
                assert updated["diff_ids"][:-1] == image["diff_ids"]
                stacks = ("go", "rust") if variant == "combined" else (variant,)
                verification = {}
                for stack in stacks:
                    fixture = baseline / f"fixtures/{stack}-a"
                    start_verify = time.monotonic()
                    result = capture(f"{name}-{stack}-verify", *runtime,
                                     "--tmpfs", "/tmp:rw,exec,nosuid,nodev,mode=1777,size=512m",
                                     "-v", f"{fixture}:/work:ro,Z", updated["config"]["digest"], stack)
                    assert f"PASS profile={stack} version=0.1.0 catalog=0.2.2".encode() in result.splitlines()
                    verification[stack] = round(time.monotonic() - start_verify, 3)
                results[name] = {"image": updated, "build_seconds": build_seconds,
                                 "tested_stacks": stacks, "verify_seconds": verification}
                (out / "images.json").write_text(json.dumps(results, indent=2) + "\n")
            finally:
                podman("image", "rm", tag)
                if (work / "oci").is_dir():
                    shutil.rmtree(work / "oci")
    assert podman("ps", "--all", "--quiet") == b""
    shutil.rmtree(container_store)
    changes = {}
    for kind in ("common", "go"):
        for label, variants in (("A", ("combined",)), ("B", ("go", "rust"))):
            old_set = blob_sizes(old[v] for v in variants)
            new_set = blob_sizes(results.get(f"{kind}-{v}", {"image": old[v]})["image"] for v in variants)
            affected = [v for v in variants if f"{kind}-{v}" in results]
            changes[f"{kind}-{label}"] = {
                "images_changed": affected,
                "new_oci_bytes": sum(size for digest, size in new_set.items() if digest not in old_set),
                "build_seconds": sum(results[f"{kind}-{v}"]["build_seconds"] for v in affected),
                "stacks_reverified": sorted({s for v in affected for s in results[f"{kind}-{v}"]["tested_stacks"]}),
                "verify_seconds": round(sum(sum(results[f"{kind}-{v}"]["verify_seconds"].values()) for v in affected), 3),
            }
    (out / "changes.json").write_text(json.dumps(changes, indent=2) + "\n")
    print(json.dumps(changes, indent=2))


if __name__ == "__main__":
    main()
