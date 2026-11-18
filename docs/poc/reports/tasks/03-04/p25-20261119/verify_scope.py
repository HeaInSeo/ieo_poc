"""Local P25 index checks for the independently selected initial use scope."""
from copy import deepcopy
import json
from pathlib import Path


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[5]
DOCS = "docs/poc/reports/tasks/"
COMMON = DOCS + "03-02/common-policy-20261118/"
STACKS = DOCS + "03-03/go-rust-20261118/"
ROLES = {"development", "reviewer", "coordinator"}
REPOSITORY = "HeaInSeo/ieo_poc"
PROFILE = {"id": "poc-acceptance-v1.3.3", "revision": 4}

# Initial use scope from the frozen expectation, resolved against approved T02 versions.
ITEMS = {
    "CAT-ROLE": ("common", "03-02", COMMON + "COMMON_POLICY.md", "0.1.0", ROLES),
    "CAT-RECOVERY": ("common", "03-02", COMMON + "COMMON_POLICY.md", "0.1.0", ROLES),
    "CAT-GO": ("go", "03-03", STACKS + "GO_POLICY.md", "0.2.0", {"development", "reviewer"}),
    "CAT-RUST": ("rust", "03-03", STACKS + "RUST_POLICY.md", "0.2.0", {"development", "reviewer"}),
    "CAT-GITHUB-IEO-POC": (
        "repository", "03-02", COMMON + "IEO_POC_GITHUB_PROFILE.md", "0.1.0", ROLES
    ),
}
COMMANDS = {
    "CAT-GO": {
        "gofmt -l .",
        "go vet -mod=readonly ./...",
        "go test -mod=readonly -count=1 -json ./...",
        "go test -mod=readonly -count=1 -shuffle=2 -v ./...",
        "go test -mod=readonly -race -count=1 ./...",
    },
    "CAT-RUST": {
        "cargo fmt --all -- --check",
        "cargo clippy --locked --all-targets -- -D warnings",
        "cargo test --locked --all-targets",
    },
}
TOOLS = {
    "CAT-GO": ("Go 1.26.10", "GCC 8"),
    "CAT-RUST": ("Rust/Cargo 1.99.1", "rustfmt 1.10.0-stable", "Clippy 0.1.99"),
}
TEXT = (
    "title", "purpose", "applies_when", "excludes_when", "tools", "environment", "permissions",
    "expected_evidence", "evidence_limits", "cost_basis",
)
VERDICTS = {"pass", "fail", "inconclusive", "not_applicable"}


def assert_input(condition, message):
    if not condition:
        raise ValueError(message)


def link(path, name):
    rel = Path(path)
    full = (ROOT / rel).resolve()
    assert_input(not rel.is_absolute() and full.is_relative_to(ROOT) and full.is_file(),
                 f"{name}: broken reference {path}")
    return full


def verify(catalog, manifest):
    assert_input(catalog["version"] == "0.2.0" and catalog["profile"] == PROFILE, "version/profile")
    plan_tasks = {t["task_id"]: t for sprint in manifest["sprints"] for t in sprint["tasks"]}
    entries = {e["id"]: e for e in catalog["items"]}
    assert_input(len(entries) == len(catalog["items"]) and set(entries) == ITEMS.keys(),
                 "missing/extra/duplicate item")
    for name, (scope, task_id, detail_path, revision, roles) in ITEMS.items():
        e = entries[name]
        assert_input(all(isinstance(e.get(f), str) and e[f].strip() for f in TEXT), f"{name}: required field")
        assert_input(e["version"] == "0.2.0" and e["readiness"] == "draft", f"{name}: version")
        assert_input(e["scope"] == scope and set(e["roles"]) == roles, f"{name}: scope/role")
        assert_input(e.get("repository") == (REPOSITORY if scope == "repository" else None),
                     f"{name}: repository binding")
        task = plan_tasks[task_id]
        acceptance = task["acceptance"]
        assert_input(e["requirement"] in task["requirement_ids"] and e["acceptance"] in acceptance["item_ids"],
                     f"{name}: requirement linkage")
        assert_input(acceptance["profile_id"] == PROFILE["id"] and acceptance["profile_revision"] == PROFILE["revision"],
                     f"{name}: approved profile mismatch")
        detail = link(e["detail"], name)
        assert_input(e["detail"] == detail_path and e["detail_revision"] == revision
                     and revision in detail.read_text().splitlines()[0], f"{name}: detail version/subject")
        assert_input(e["sources"], f"{name}: missing source")
        for source in e["sources"]:
            link(source, name)
        link(e["cost_basis"], name)
        assert_input(e["verdicts"].keys() == VERDICTS and all(e["verdicts"].values()), f"{name}: verdict fields")
        if name in COMMANDS:
            assert_input(set(e["method"]["commands"]) == COMMANDS[name], f"{name}: incorrect command/options")
            assert_input(all(tool in e["tools"] for tool in TOOLS[name]), f"{name}: wrong tool version")
        else:
            assert_input(e["method"]["procedure"].strip(), f"{name}: missing procedure")
    return len(entries), len(TEXT) * len(entries)


def select(catalog, stack, repo, role):
    return tuple(sorted(e["id"] for e in catalog["items"] if role in e["roles"]
                        and (e["scope"] in ("common", stack)
                             or e["scope"] == "repository" and e["repository"] == repo)))


def negative_controls(catalog, manifest):
    controls = [
        ("required item", lambda c: c["items"].pop(), "missing/extra/duplicate"),
        ("required field", lambda c: c["items"][0].pop("expected_evidence"), "required field"),
        ("wrong option", lambda c: c["items"][2]["method"]["commands"].__setitem__(0, "gofmt --unknown ."),
         "incorrect command/options"),
        ("omitted important option", lambda c: c["items"][3]["method"]["commands"].__setitem__(
            1, "cargo clippy --locked --all-targets"), "incorrect command/options"),
        ("wrong version", lambda c: c["items"][2].__setitem__("tools", "Go 1.25.5; GCC 8"), "wrong tool version"),
        ("broken reference", lambda c: c["items"][1].__setitem__("detail", "missing.md"), "broken reference"),
        ("wrong role", lambda c: c["items"][0].__setitem__("roles", ["development"]), "scope/role"),
        ("wrong stack", lambda c: c["items"][2].__setitem__("scope", "rust"), "scope/role"),
        ("wrong repository", lambda c: c["items"][4].__setitem__("repository", "HeaInSeo/notion-projection-guard"),
         "repository binding"),
    ]
    for name, mutate, reason in controls:
        changed = deepcopy(catalog)
        mutate(changed)
        try:
            verify(changed, manifest)
        except ValueError as exc:
            assert reason in str(exc), (name, exc)
            print(f"PASS rejected {name}: {exc}")
        else:
            raise AssertionError(f"accepted invalid {name}")
    return len(controls)


if __name__ == "__main__":
    catalog = json.loads((HERE / "catalog-use.json").read_text())
    manifest = json.loads((ROOT / "docs/poc/Console_Coordinator_Sprints_v0.json").read_text())
    n, fields = verify(catalog, manifest)
    print(f"PASS normal catalog: links {n}/{len(ITEMS)}, selected text fields {fields}/{len(TEXT)*len(ITEMS)}")
    controls = negative_controls(catalog, manifest)
    expected = ("CAT-GITHUB-IEO-POC", "CAT-GO", "CAT-RECOVERY", "CAT-ROLE")
    assert select(catalog, "go", REPOSITORY, "development") == expected
    assert select(catalog, "go", REPOSITORY, "development") == expected
    assert select(catalog, "rust", "example/other", "reviewer") == ("CAT-RECOVERY", "CAT-ROLE", "CAT-RUST")
    assert select(catalog, "go", REPOSITORY, "unassigned") == ()
    print("PASS repeat lookup plus stack, repository and role N/A controls")
    print(f"PASS corrected catalog, {controls}/{controls} mutations rejected")
