"""Local P25 index checks for the independently selected initial use scope."""
from copy import deepcopy
import json
from pathlib import Path


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[5]
DOCS = "docs/poc/reports/tasks/"
COMMON = DOCS + "03-02/common-policy-20261118/"
STACKS = DOCS + "03-03/go-rust-20261118/"
VALIDATION = DOCS + "03-04/p25-20261119/"
HISTORIC_CATALOG = DOCS + "03-01/catalog-contract-20261118/catalog.json"
RUNTIME = DOCS + "01-02/runtime-contract-20261112/Runtime_Contract_v0.md"
ROLES = {"development", "reviewer", "coordinator"}
REPOSITORY = "HeaInSeo/ieo_poc"
PROFILE = {"id": "poc-acceptance-v1.3.3", "revision": 4}

# Initial use scope from the frozen expectation, resolved against approved T02 versions.
ITEMS = {
    "CAT-ROLE": ("common", "03-02", COMMON + "COMMON_POLICY.md", "0.1.0", ROLES),
    "CAT-RECOVERY": ("common", "03-02", COMMON + "COMMON_POLICY.md", "0.1.0", ROLES),
    "CAT-GO": ("go", "03-03", VALIDATION + "GO_POLICY.md", "0.2.1", {"development", "reviewer"}),
    "CAT-RUST": ("rust", "03-03", VALIDATION + "RUST_POLICY.md", "0.2.1", {"development", "reviewer"}),
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
    "CAT-ROLE": "Runtime Contract rev 20",
    "CAT-RECOVERY": "ieo-evidence 02-01 검수본 및 역할 계약 rev 20",
    "CAT-GITHUB-IEO-POC": "GitHub Actions 및 gh 읽기 관측; rust-toolchain.toml",
    "CAT-GO": "Go 1.26.10; GCC 8",
    "CAT-RUST": "Rust/Cargo 1.99.1; rustfmt 1.10.0-stable; Clippy 0.1.99",
}
PERMISSIONS = {
    "CAT-ROLE": "배정의 기존 역할 범위",
    "CAT-RECOVERY": "필요 증거 읽기",
    "CAT-GO": "소스 읽기, 임시 쓰기",
    "CAT-RUST": "소스 읽기, 임시 쓰기",
    "CAT-GITHUB-IEO-POC": "읽기 관측, 승인된 자만 병합",
}
SOURCES = {
    "CAT-ROLE": {HISTORIC_CATALOG, COMMON + "ACCEPTANCE.md", RUNTIME},
    "CAT-RECOVERY": {HISTORIC_CATALOG, DOCS + "02-01/collector-20261112/REPORT.md", RUNTIME},
    "CAT-GO": {HISTORIC_CATALOG, STACKS + "ACCEPTANCE.md"},
    "CAT-RUST": {HISTORIC_CATALOG, "rust-toolchain.toml"},
    "CAT-GITHUB-IEO-POC": {".github/workflows/ci.yml", COMMON + "github-main-observation.json"},
}
TEXT = (
    "title", "purpose", "tools", "environment", "permissions",
    "expected_evidence", "evidence_limits", "cost_basis",
)
VERDICTS = {
    "pass": "required_evidence_matched",
    "fail": "observed_violation_or_required_skip",
    "inconclusive": "required_evidence_unobserved",
    "not_applicable": "approved_scope_exclusion",
}
PROCEDURES = {
    "CAT-ROLE": (
        ("submit_change_and_evidence", "development"),
        ("independent_approval_at_head", "reviewer"),
        ("confirm_current_assignment_and_head", "coordinator"),
    ),
    "CAT-RECOVERY": (
        ("preserve_failure_and_attempt_id", "development"),
        ("confirm_expected_failure_and_scope", "reviewer"),
        ("reconcile_retained_result_and_effect", "coordinator"),
    ),
    "CAT-GITHUB-IEO-POC": (
        ("submit_pr_checks_and_head", "development"),
        ("independently_approve_head", "reviewer"),
        ("compare_required_checks_and_confirm_merge_sha", "coordinator"),
    ),
}


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
    assert_input(catalog["version"] == "0.2.2" and catalog["profile"] == PROFILE, "version/profile")
    plan_tasks = {t["task_id"]: t for sprint in manifest["sprints"] for t in sprint["tasks"]}
    assert_input("**revision 20**" in link(RUNTIME, "common runtime").read_text(),
                 "approved runtime version unavailable")
    entries = {e["id"]: e for e in catalog["items"]}
    assert_input(len(entries) == len(catalog["items"]) and set(entries) == ITEMS.keys(),
                 "missing/extra/duplicate item")
    for name, (scope, task_id, detail_path, revision, roles) in ITEMS.items():
        e = entries[name]
        assert_input(all(isinstance(e.get(f), str) and e[f].strip() for f in TEXT), f"{name}: required field")
        assert_input(e["permissions"] == PERMISSIONS[name], f"{name}: role permission authority")
        permitted_stack = [scope] if scope in ("go", "rust") else ["go", "rust"]
        repo = REPOSITORY if scope == "repository" else None
        assert_input(e["applies_when"] == {"stacks": permitted_stack, "repository": repo},
                     f"{name}: uncertified applicability")
        exclusions = {"other_stack", "unassigned_role"} | ({"other_repo"} if repo else set())
        assert_input(set(e["excludes_when"]) == exclusions, f"{name}: uncertified exclusion")
        assert_input(e["version"] == "0.2.2" and e["readiness"] == "draft", f"{name}: version")
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
        assert_input(set(e["sources"]) == SOURCES[name], f"{name}: wrong source binding")
        for source in e["sources"]:
            link(source, name)
        cost_dir = STACKS if scope in ("go", "rust") else COMMON
        assert_input(e["cost_basis"] == cost_dir + "REPORT.md", f"{name}: wrong cost evidence")
        link(e["cost_basis"], name)
        assert_input(e["verdicts"] == VERDICTS, f"{name}: wrong verdict rules")
        assert_input(e["tools"] == TOOLS[name], f"{name}: wrong tool version")
        if name in COMMANDS:
            assert_input(set(e["method"]["commands"]) == COMMANDS[name], f"{name}: incorrect command/options")
        else:
            procedure = tuple((step["action"], step["role"]) for step in e["method"]["procedure"])
            assert_input(procedure == PROCEDURES[name], f"{name}: invalid procedure authority")
    return len(entries), len(TEXT) * len(entries)


def select(catalog, stack, repo, role):
    selected = []
    for entry in catalog["items"]:
        applies = entry["applies_when"]
        if stack not in applies["stacks"] and "other_stack" in entry["excludes_when"]:
            continue
        if repo != applies["repository"] and "other_repo" in entry["excludes_when"]:
            continue
        if role not in entry["roles"] and "unassigned_role" in entry["excludes_when"]:
            continue
        selected.append(entry["id"])
    return tuple(sorted(selected))


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
        ("development granted reviewer authority", lambda c: c["items"][0].__setitem__(
            "permissions", "개발이 Guardrail 검토를 승인"), "role permission authority"),
        ("unrelated source", lambda c: c["items"][0].__setitem__("sources", ["Cargo.lock"]),
         "wrong source binding"),
        ("unrelated cost", lambda c: c["items"][0].__setitem__("cost_basis", "Cargo.lock"), "wrong cost evidence"),
        ("version prefix collision", lambda c: c["items"][2].__setitem__("tools", "Go 1.26.100; GCC 8"),
         "wrong tool version"),
        ("non-applicable scope changed", lambda c: c["items"][2]["applies_when"].__setitem__("stacks", []),
         "uncertified applicability"),
        ("role exclusion removed", lambda c: c["items"][2]["excludes_when"].remove("unassigned_role"),
         "uncertified exclusion"),
        ("procedure grants approval to development", lambda c: c["items"][0]["method"]["procedure"][1].__setitem__(
            "role", "development"), "invalid procedure authority"),
        ("required skip downgraded to unobserved", lambda c: c["items"][2]["verdicts"].__setitem__(
            "fail", "required_evidence_unobserved"), "wrong verdict rules"),
        ("unapproved common runtime revision", lambda c: c["items"][0].__setitem__("tools", "Runtime Contract rev 21"),
         "wrong tool version"),
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
