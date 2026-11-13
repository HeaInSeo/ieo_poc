"""X-01 mutation check: each mutant disables one rule; the full test suite must fail.
Run from the repo root at a clean commit with IEO_TEST_DB_URL set; files are restored with git checkout."""
import subprocess, os, pathlib
API = 'crates/ieo-coord/src/api.rs'
LIB = 'crates/ieo-notion/src/lib.rs'
M = [
 ("X1 import replay disabled (same content re-registers)", API, "WHERE page_id = $1 AND content_digest = $2\",", "WHERE page_id = $1 AND content_digest = $2 AND false\","),
 ("X2 running assignment not pinned to its revision", API, "let pinned = generation > 0", "let pinned = false && generation > 0"),
 ("X3 own earlier revision treated as another sprint", API, "WHERE sprint_id <> $1", "WHERE sprint_id <> $1 OR true"),
 ("X4 importer ignores the Notion queue section", LIB, "Some(q) if q == \"현재\" => {}", "Some(_) => {}"),
 ("X5 another page may reuse a test_id", API, "&& row.get::<_, &str>(0) != page_id", "&& false"),
 ("X6 importer's unresolved reasons dropped", API, "for u in &imp.extra_unresolved {", "for u in imp.extra_unresolved.iter().take(0) {"),
 ("X7 snapshot tampering not detected", LIB, "if v[\"content_digest\"].as_str() != Some(content_digest(&p).as_str()) {", "if false {"),
 ("X8 Notion source not recorded on the spec", API, "src.extend(imp.extra_sources.iter().cloned());", "let _ = &imp.extra_sources;"),
 ("X9 existing non-import sprint may be taken over (review R1)", API, "if !imported_before && latest_revision(tx, &test_id)?.is_some() {", "if false {"),
 ("X10 replay answers another repo (review R2)", API, "if stored_repo.as_deref() != Some(repo) {", "if false {"),
 ("X11 pinned continuation ignores latest ceiling (review R3)", API, "ceiling_from_latest = Some(l[\"limits\"][\"correction_attempts\"].clone());", "let _ = &l;"),
]
out = []
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
