"""02-04 run guard mutation check (repo root, clean tree; python 3.7+)."""
import subprocess, pathlib, sys
F = 'crates/ieo-supervisor/src/run.rs'
M = [
 ("R1 no final group kill", "    };\n    if !signal_group(kill, pgid, \"KILL\") {\n        cleanup_failed = true;\n    }", "    };"),
 ("R2 no SIGKILL after the grace", "    if !signal_group(kill, pgid, \"KILL\") {\n        *failed = true;\n    }\n    let _ = child.kill();", ""),
 ("R3 ignore extra-usage signals", "    if let Some(why) = crate::budget::stream_stop(line) {", "    if let Some(why) = None::<String>.filter(|_| crate::budget::stream_stop(line).is_some()) {"),
 ("R4 no token cap", "        if usage.observed.billable >= limits.billable_room {", "        if false && usage.observed.billable >= limits.billable_room {"),
 ("R5 no cache_read cap", "        if usage.observed.cache_read >= limits.cache_read_room {", "        if false && usage.observed.cache_read >= limits.cache_read_room {"),
 ("R6 sum instead of max per message", "            *slot = (*slot).max(v);", "            *slot += v;"),
 ("R7 no deadline", "        if now >= deadline {", "        if false && now >= deadline {"),
 ("R8 total timeout reported as run timeout", "        if limits.timeout_is_total {\n            Stop::TotalTimeout", "        if false {\n            Stop::TotalTimeout"),
 ("R9 no 200 ms poll (receive until the deadline)", "        match rx.recv_timeout((deadline - now).min(Duration::from_millis(200))) {", "        match rx.recv_timeout(deadline - now) {"),
 ("R10 SIGKILL first instead of SIGTERM", "    if !signal_group(kill, pgid, \"TERM\") {", "    if !signal_group(kill, pgid, \"KILL\") {"),
 ("R11 grace ignored", "    let until = Instant::now() + grace;", "    let until = Instant::now();"),
 ("R12 closed stream waits for the leader instead of stopping", "                        return Ok(Watch::Stopped(Stop::StreamLost(", "                        return Ok(Watch::Exited(child.wait()?));\n                        #[allow(unreachable_code)]\n                        return Ok(Watch::Stopped(Stop::StreamLost("),
 ("R13 no drain after exit", "            return drained(s, rx, out, usage, limits, deadline);", "            return Ok(Watch::Exited(s));"),
 ("R14 total when exactly the per-run limit is left", "            timeout_is_total: timeout_seconds < budget.per_run_seconds,", "            timeout_is_total: timeout_seconds <= budget.per_run_seconds,"),
 ("R15 for_next ignores the pre-run decision", "        let Decision::Start { timeout_seconds } = decision else {\n            return Err(decision);\n        };", "        let timeout_seconds = match decision {\n            Decision::Start { timeout_seconds } => timeout_seconds,\n            _ => budget.per_run_seconds,\n        };"),
 ("R16 grace not taken out of the window", "            timeout: window - GRACE,", "            timeout: window,"),
 ("R17 non-UTF-8 bytes end the stream", "                    let line = String::from_utf8_lossy(&buf)", "                    if std::str::from_utf8(&buf).is_err() {\n                        let _ = tx.send(Event::Broken);\n                        break;\n                    }\n                    let line = String::from_utf8_lossy(&buf)"),
 ("R18 lines without an id merged", "                format!(\"\\u{0}unnamed-{}\", self.unnamed)", "                String::new()"),
 ("R19 exit noticed only between lines", "        if let Some(s) = child.try_wait()? {\n            return drained(", "        if false && let Some(s) = child.try_wait()? {\n            return drained("),
 ("R20 a failed group kill counted as delivered", "        _ => !group_alive(pgid),", "        _ => true,"),
 ("R21 no direct kill of the leader", "    let _ = child.kill();\n    match child.wait() {", "    match child.wait() {"),
 ("R22 output after a stop not copied", "        copier.pump(DRAIN);", "        let _ = DRAIN;"),
 ("R24 a window equal to the grace allowed", "        if window <= GRACE {", "        if window < GRACE {"),
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
