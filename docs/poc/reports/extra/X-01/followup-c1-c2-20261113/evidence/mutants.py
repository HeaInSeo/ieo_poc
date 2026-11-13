"""X-01 C2 mutation check (run from repo root at a clean commit with IEO_TEST_DB_URL set)."""
import subprocess, pathlib
API = 'crates/ieo-coord/src/api.rs'
LIB = 'crates/ieo-notion/src/lib.rs'
M = [
 ("C1 registration ignores the import gate", API, "            import_gate(tx, sid, &owner)?;\n", ""),
 ("C2 assignment ignores the import gate", API, "    import_gate(tx, &sprint_id, &repo)?;\n", ""),
 ("C3 missing facts do not block", API, "    if missing.as_array().is_some_and(|m| !m.is_empty()) {", "    if false {"),
 ("C4 an exception applies to any test/repo", API, "WHERE test_id = $1 AND repo_id = $2\n                   AND lifts = 'queue_gates'", "WHERE (test_id = $1 AND repo_id = $2 OR true)\n                   AND lifts = 'queue_gates'"),
 ("C5 registrars may write import records", API, "    need_client_scope(who, \"notion:import\", true)?;", "    need_client_scope(who, \"sprints:register\", true)?;"),
 ("C6 legacy rows: unknown reasons not fail-closed", API, "        } else if !BINDING.contains(&r.as_str()) {", "        } else if false {"),
 ("C7 importer files a gate as a missing fact", LIB, "        gates.push(format!(\"Notion 차단 대상 미해제: {}\", truncate(&b, 400)));", "        unresolved.push(format!(\"Notion 차단 대상 미해제: {}\", truncate(&b, 400)));"),
 ("C8 gates stored empty (release forged)", API, "            &json!(gates),\n", "            &json!([]),\n"),
]
for name, f, a, b in M:
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
