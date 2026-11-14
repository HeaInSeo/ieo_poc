"""02-04 run guard mutation check (repo root, clean tree; python 3.7+)."""
import subprocess, pathlib, sys
F = 'crates/ieo-supervisor/src/run.rs'
M = [
 ("R1 no final group kill", "    // no process of the run outlives it\n    signal_group(pgid, \"KILL\");", "    // no process of the run outlives it"),
 ("R2 no SIGKILL after grace", "                signal_group(pgid, \"KILL\");\n                break child.wait()?;", "                break child.wait()?;"),
 ("R3 ignore extra-usage signals", "                if let Some(why) = crate::budget::stream_stop(&line) {", "                if let Some(why) = None::<String>.filter(|_| crate::budget::stream_stop(&line).is_some()) {"),
 ("R4 no token cap", "                    if observed.billable >= limits.billable_room {", "                    if false && observed.billable >= limits.billable_room {"),
 ("R5 no cache_read cap", "                    if observed.cache_read >= limits.cache_read_room {", "                    if false && observed.cache_read >= limits.cache_read_room {"),
 ("R6 sum instead of max per message", "                        e[i] = e[i].max(u[i]);", "                        e[i] += u[i];"),
 ("R7 no deadline", "        if now >= deadline {", "        if false && now >= deadline {"),
 ("R8 total timeout reported as run timeout", "            stop = Some(if limits.timeout_is_total {\n                Stop::TotalTimeout\n            } else {\n                Stop::RunTimeout\n            });\n            break;", "            stop = Some(if false {\n                Stop::TotalTimeout\n            } else {\n                Stop::RunTimeout\n            });\n            break;"),
]
only = sys.argv[1:]
p = pathlib.Path(F); src = p.read_text()
try:
    for name, a, b in M:
        if only and not any(name.startswith(o) for o in only):
            continue
        if src.count(a) != 1:
            print(f"{name}: PATTERN COUNT {src.count(a)}", flush=True); continue
        p.write_text(src.replace(a, b))
        r = subprocess.run(["timeout", "300", "cargo", "test", "-q", "-p", "ieo-supervisor", "--test", "run_guard"], capture_output=True, text=True)
        failed = [l.replace('---- ', '').split(' stdout')[0] for l in (r.stdout + r.stderr).splitlines() if l.startswith('---- ')]
        how = failed[:3] if failed else (['(timeout 300 s)'] if r.returncode == 124 else ['(build/other failure)'] if r.returncode else [])
        print(f"{name}: {'killed' if r.returncode else 'SURVIVED'} {how}", flush=True)
finally:
    p.write_text(src)
print("dirty after run:", subprocess.run(["git", "status", "--porcelain", "crates"], capture_output=True, text=True).stdout.strip() or "none")
