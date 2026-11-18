"""Check catalog joins and pinned source excerpts against the PoC plan."""
import hashlib
import json
from pathlib import Path
import re
import subprocess


HERE = Path(__file__).resolve().parent
POC = HERE.parents[3]
VERSION = "0.1.0"

# Approved design inputs taken from the task manifest, T01 scope table and T02/P25.
# A historical source cannot acquire authority by changing its relation in the map.
REQUIRED_SOURCES = {
    "03-01.accept": {"spec-0301", "scope-0301", "t02-fields", "p25-readiness"},
    "03-02.accept": {"t02-fields"},
    "03-03.accept": {"t02-fields", "p25-readiness"},
}
REQUIREMENT_ANCHORS = {
    "spec-0301": ("Console_Coordinator_Sprints_v0.json", [1366, 1373], "03-01.accept"),
    "scope-0301": (
        "reports/tasks/01-01/scope-20261112/Scope_and_Requirements.md",
        [43, 45], "POC-REQ-03-01",
    ),
    "t02-fields": ("Console_Coordinator_PoC_Validation_Plan.md", [216, 220], "각 카탈로그 항목에는"),
    "p25-readiness": ("Console_Coordinator_PoC_Validation_Plan.md", [664, 675], "카탈로그"),
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def read(path):
    def no_duplicate_keys(pairs):
        result = {}
        for key, value in pairs:
            require(key not in result, f"duplicate JSON key: {key}")
            result[key] = value
        return result

    return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=no_duplicate_keys)


def keyed(rows, fields):
    require(isinstance(rows, list) and rows, f"empty or invalid table: {fields}")
    result = {}
    for row in rows:
        key = tuple(row[field] for field in fields)
        require(key not in result, f"duplicate identity: {key}")
        result[key] = row
    return result


def text_fields(row, fields, label):
    for field in fields.split():
        value = row.get(field)
        require(isinstance(value, str) and value.strip(), f"{label}: missing {field}")


def check_sources(sources, poc):
    for (source_id,), source in sources.items():
        text_fields(source, "id path revision sha256 excerpt scope", source_id)
        path = Path(source["path"])
        resolved = (poc / path).resolve()
        require(not path.is_absolute() and ".." not in path.parts
                and resolved.is_relative_to(poc) and resolved.is_file(),
                f"{source_id}: invalid source path")
        require(re.fullmatch(r"[0-9a-f]{40}", source["revision"]), f"{source_id}: invalid revision")
        data = resolved.read_bytes()
        require(hashlib.sha256(data).hexdigest() == source["sha256"], f"{source_id}: digest mismatch")
        pinned = subprocess.run(
            ["git", "show", f"{source['revision']}:docs/poc/{path.as_posix()}"],
            cwd=poc, capture_output=True, check=False,
        )
        require(pinned.returncode == 0 and pinned.stdout == data, f"{source_id}: revision mismatch")
        lines = data.decode("utf-8").splitlines()
        first, last = source["lines"]
        require(type(first) is int and type(last) is int and 1 <= first <= last <= len(lines),
                f"{source_id}: invalid line range")
        require(source["excerpt"] in "\n".join(lines[first-1:last]), f"{source_id}: excerpt missing")


def verify(catalog, source_map, spec, poc=POC):
    require(catalog["contract_version"] == source_map["contract_version"] == VERSION,
            "contract version mismatch")
    require(catalog["plan_version"] == spec["plan_version"], "plan version mismatch")
    tasks = [task for sprint in spec["sprints"] for task in sprint["tasks"]]
    requirements = set()
    for task in tasks:
        acceptance = task["acceptance"]
        for requirement in task["requirement_ids"]:
            for item in acceptance["item_ids"]:
                requirements.add((requirement, item, acceptance["profile_id"], acceptance["profile_revision"]))

    profile_items = {
        (profile["profile_id"], profile["revision"], item)
        for profile in spec["inspection_profiles"] for item in profile["items"]
    }
    entries = keyed(catalog["entries"], ("id", "version"))
    mappings = keyed(source_map["mappings"], ("item_id", "item_version"))
    sources = keyed(source_map["sources"], ("id",))
    require(entries.keys() == mappings.keys(), "index/source-map identities differ")
    used_sources = set()
    for (item_id, version), entry in entries.items():
        label = f"{item_id}@{version}"
        text_fields(entry, "id version title purpose applies_when excludes_when expected_evidence evidence_limits", label)
        require(re.fullmatch(r"[A-Z][A-Z0-9-]+", item_id), f"{label}: invalid item ID")
        require(re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", version),
                f"{label}: invalid version")
        require(entry["scope"] in {"common", "go", "rust", "repository"}, f"{label}: invalid scope")
        require(("repository" in entry) == (entry["scope"] == "repository"),
                f"{label}: repository binding mismatch")
        if "repository" in entry:
            require(re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", entry["repository"]),
                    f"{label}: invalid repository")
        roles = entry["roles"]
        require(isinstance(roles, list) and roles and len(set(roles)) == len(roles)
                and set(roles) <= {"author", "development", "reviewer"}, f"{label}: invalid roles")
        require(entry["readiness"] in {"contract", "draft"}, f"{label}: invalid readiness")
        req_keys = keyed(entry["requirements"], ("requirement_id", "item_id", "profile_id", "profile_revision"))
        require(all(type(key[-1]) is int for key in req_keys) and req_keys.keys() <= requirements,
                f"{label}: invalid requirement/acceptance binding")

        require(all((profile, rev, item) in profile_items for _, item, profile, rev in req_keys),
                f"{label}: acceptance item absent from profile")

        execution = entry["execution"]
        text_fields(execution, "tools_and_versions environment permissions", label)
        if execution["kind"] == "policy":
            text_fields(execution, "procedure comparison", label)
        else:
            require(execution["kind"] == "command", f"{label}: invalid execution kind")
            text_fields(execution, "command important_options", label)
        text_fields(entry["verdicts"], "pass fail not_applicable", label)
        text_fields(entry["cost"], "basis time resources tokens", label)
        require(entry["cost"]["state"] in {"unestimated", "estimate", "measured"}, f"{label}: invalid cost")

        links = keyed(mappings[item_id, version]["sources"], ("source_id",))
        relations = {link["relation"] for link in links.values()}
        require("requirement" in relations and relations <= {"requirement", "candidate", "observation"},
                f"{label}: invalid source relations")
        require(links.keys() <= sources.keys(), f"{label}: unresolved source")
        required_sources = set()
        for _, item, _, _ in req_keys:
            require(item in REQUIRED_SOURCES, f"{label}: no approved source rule for {item}")
            required_sources.update(REQUIRED_SOURCES[item])
        actual_required = {source_id for (source_id,), link in links.items()
                           if link["relation"] == "requirement"}
        require(actual_required == required_sources, f"{label}: unapproved requirement source")
        for source_id in required_sources:
            source = sources[source_id,]
            locator = (source["path"], source["lines"], source["excerpt"])
            require(locator == REQUIREMENT_ANCHORS[source_id],
                    f"{label}: requirement source anchor mismatch")
        used_sources.update(links)

    require(used_sources == sources.keys(), "unreferenced source")
    check_sources(sources, poc)
    return len(entries), len(sources)


def inputs():
    return (read(HERE / "catalog.json"), read(HERE / "sources.json"),
            read(POC / "Console_Coordinator_Sprints_v0.json"))


if __name__ == "__main__":
    items, sources = verify(*inputs())
    print(f"PASS: {items} catalog entries, {sources} pinned sources")
