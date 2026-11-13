"""02-03 mutation check: each mutant disables one rule; the test suite must fail.
Run from the repo root at a clean commit; every file is restored with git checkout."""
import subprocess, sys, os, pathlib
API='crates/ieo-coord/src/api.rs'
M=[
 ("M01 ownership CAS removed", API, "if book.current != expected {", "if false && book.current != expected {"),
 ("M02 generation need not follow head", API, "let follows = book.current == Some((aid.clone(), generation - 1));", "let follows = true;"),
 ("M03 prerequisites ignored (G1)", API, "        if !met {", "        if false && !met {"),
 ("M04 attempt ceiling off by one (G4)", API, "if issued > ceiling {", "if issued > ceiling + 1 {"),
 ("M05 CI evidence from any SHA (G2)", API, "AND kind = ANY($2) AND sha = $3", "AND kind = ANY($2) AND (sha = $3 OR true)"),
 ("M06 replay counts not re-evaluated (N3)", API, "disposition == \"current\" && head.is_some_and(|(a, g)| *a == aid && *g == generation);", "disposition == \"current\";"),
 ("M07 old guardrail generation counted (N4)", API, "AND e.generation = $3", "AND (e.generation = $3 OR true)"),
 ("M08 GitHub self-review counted (G3)", API, "AND body->>'reviewer' <> body->>'author'", "AND true"),
 ("M09 registrant may self-review (G3)", API, "if registrant == who.name {", "if false {"),
 ("M10 semantic review gate removed", API, "if rev_row.get::<_, &str>(2) != \"semantic_review_passed\" {", "if false {"),
 ("M11 run record passes on any judgment", API, "body[\"record\"][\"judgment\"][\"counts_as_success\"] == true", "true"),
 ("M12 current reject does not block", API, "(v == \"reject\" || v == \"needs_confirmation\")", "false"),
 ("M13 refused request commits", API, "let _ = tx.rollback();", "let _ = tx.commit();"),
 ("M14 per-sprint event lock removed", API, "    lock(tx, &format!(\"sprint:{}\", e.sprint_id))?;\n", ""),
 ("M15 claim task/sprint consistency removed", API, "if body[field] != recorded {", "if false {"),
 ("M16 optional outputs judged (G7)", API, "if o[\"required\"] != true {", "if false {"),
 ("M17 decision request not once (G5)", API, "        if reached {", "        if false {"),
 ("M18 append-only triggers removed", "crates/ieo-coord/src/schema.sql", "$$ BEGIN RAISE EXCEPTION 'append-only table %', TG_TABLE_NAME; END $$;", "$$ BEGIN RETURN NEW; END $$;"),
 ("M19 registry trusts digest string (PR10 N2)", "crates/ieo-spec/src/check.rs", "if spec_digest(spec) != digest {", "if false {"),
 ("M20 completion summary prefix only (01-08 N2)", "crates/ieo-report/src/lib.rs", "        .filter(|(_, ok)| *ok)\n", "        .take_while(|(_, ok)| *ok)\n"),
 ("M21 sprint may move to another repo (review F1)", API, "&& Some(r.get::<_, &str>(0)) != spec[\"target\"][\"repo_id\"].as_str()", "&& false"),
 ("M22 GitHub reviews of any SHA (review F2)", API, "WHERE repo_id = $1 AND kind = 'review' AND sha = $2 AND body->>'reviewer' <> body->>'author'", "WHERE repo_id = $1 AND kind = 'review' AND (sha = $2 OR true) AND body->>'reviewer' <> body->>'author'"),
 ("M23 commented review clears changes_requested (review F2)", API, "AND body->>'state' IN ('approved', 'changes_requested', 'dismissed')", "AND body->>'state' IN ('approved', 'changes_requested', 'dismissed', 'commented')"),
 ("M24 decision request once per task, not per ceiling (review F6)", API, "if reached == Some(ceiling) {", "if reached.is_some() {"),
 ("M25 promoter may review (review F10)", API, "if registrant == who.name || registered_it {", "if registrant == who.name {"),
 ("M26 stale note dropped when blocked (review F9)", API, "        stale.dedup();\n", "        stale.clear();\n"),
]
env=dict(os.environ)
out=[]
for name,f,a,b in M:
    p=pathlib.Path(f); s=p.read_text()
    if s.count(a)!=1:
        out.append(f"{name}: PATTERN COUNT {s.count(a)}"); continue
    p.write_text(s.replace(a,b))
    r=subprocess.run(["cargo","test","-q","--locked","--all-targets"],capture_output=True,text=True,env=env)
    failed=[l.split(' ---')[0] for l in (r.stdout+r.stderr).splitlines() if l.endswith('--- FAILED')]
    status="killed" if r.returncode!=0 else "SURVIVED"
    if r.returncode!=0 and not failed:
        failed=["(build/other failure)"]
    out.append(f"{name}: {status} {failed[:4]}")
    subprocess.run(["git","checkout","--",f],check=True)
    print(out[-1],flush=True)
print("dirty after run:", subprocess.run(["git","status","--porcelain","crates"],capture_output=True,text=True).stdout.strip() or "none")
