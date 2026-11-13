"""X-01 C2 mutation check (run from repo root at a clean commit with IEO_TEST_DB_URL set)."""
import subprocess, pathlib, sys
ONLY = sys.argv[1:]  # optional name prefixes, e.g. "C9 "
API = 'crates/ieo-coord/src/api.rs'
LIB = 'crates/ieo-notion/src/lib.rs'
M = [
 ("C1 registration ignores the Notion gate", API, "    if mode == CheckMode::Executable\n        && ieo_spec::schema::format_rejections", "    if false\n        && ieo_spec::schema::format_rejections"),
 ("C2 assignment ignores the Notion gate", API, "    notion_gate(tx, &spec, &sprint_id, &repo)?;\n", ""),
 ("C3 missing facts do not block", API, "        if missing.as_array().is_some_and(|m| !m.is_empty()) {", "        if false {"),
 ("C4 an exception applies to any test sprint and page of the repo (review 7afd4d1 G2)", API, "WHERE test_id = $1 AND repo_id = $2\n                           AND lifts = 'queue_gates' AND page_id = $3 AND content_digest = $4", "WHERE (test_id = $1 OR true) AND repo_id = $2\n                           AND lifts = 'queue_gates' AND (page_id = $3 OR true) AND content_digest = $4"),
 ("C4a exception not filtered by test sprint (page binding remains)", API, "WHERE test_id = $1 AND repo_id = $2\n                           AND lifts = 'queue_gates' AND page_id = $3", "WHERE (test_id = $1 OR true) AND repo_id = $2\n                           AND lifts = 'queue_gates' AND page_id = $3"),
 ("C4b exception not bound to the page (test sprint filter remains)", API, "AND lifts = 'queue_gates' AND page_id = $3 AND content_digest = $4", "AND lifts = 'queue_gates' AND (page_id = $3 OR true) AND content_digest = $4"),
 ("C5 registrars may write import records", API, "    need_client_scope(who, \"notion:import\", true)?;", "    need_client_scope(who, \"sprints:register\", true)?;"),
 ("C6 legacy rows: unknown reasons not fail-closed", API, "        } else if !BINDING.contains(&r.as_str()) {", "        } else if false {"),
 ("C7 importer files a gate as a missing fact", LIB, "        gates.push(format!(\"Notion 차단 대상 미해제: {}\", truncate(&b, 400)));", "        unresolved.push(format!(\"Notion 차단 대상 미해제: {}\", truncate(&b, 400)));"),
 ("C8 gates stored empty (release forged)", API, "            &json!(gates),\n", "            &json!([]),\n"),
 ("C9 page ids in sources not recognised (review 7afd4d1 G1)", API, "WHERE page_id = ANY($1)\",", "WHERE page_id = ANY($1) AND false\","),
 ("C9b no digest fallback for a source without a page reference", API, "WHERE content_digest = $1\",", "WHERE content_digest = $1 AND false\","),
 ("C10 replay of an earlier content state (review F2)", API, "           AND (test_id, sprint_revision) = (SELECT test_id, sprint_revision FROM notion_imports\n                 WHERE page_id = $1 ORDER BY imported_at DESC, sprint_revision DESC LIMIT 1)\",", "\","),
 ("C11 exception not bound to the approved content (review F4)", API, "AND page_id = $3 AND content_digest = $4", "AND page_id = $3 AND (content_digest = $4 OR true)"),
 ("C12 a page may be imported under a second test id (review 7afd4d1 G3)", API, "WHERE page_id = $1 AND test_id <> $2 LIMIT 1\",", "WHERE page_id = $1 AND test_id <> $2 AND false LIMIT 1\","),
]
for name, f, a, b in M:
    if ONLY and not any(name.startswith(o) for o in ONLY):
        continue
    p = pathlib.Path(f); s = p.read_text()
    if s.count(a) != 1:
        print(f"{name}: PATTERN COUNT {s.count(a)}", flush=True); continue
    p.write_text(s.replace(a, b))
    r = subprocess.run(["cargo", "test", "-q", "--locked", "--all-targets"], capture_output=True, text=True)
    failed = [l.split(' ---')[0] for l in (r.stdout + r.stderr).splitlines() if l.endswith('--- FAILED')]
    if r.returncode != 0 and not failed:
        failed = ["(build/other failure)"]
    print(f"{name}: {'killed' if r.returncode else 'SURVIVED'} {failed[:4]}", flush=True)
    subprocess.run(["git", "checkout", "--", f], check=True)
print("dirty after run:", subprocess.run(["git", "status", "--porcelain", "crates"], capture_output=True, text=True).stdout.strip() or "none")
