"""02-05 step 8 resume: the one permission change the USER makes (RESUME_PLAN.md §9).

python3 -I apply-perm-resume.py <rules.json from make-resume-copies.sh> [settings file]
(default settings file: /opt/rust/src/github.com/HeaInSeo/ieo_poc/.claude/settings.local.json)

Backup (0600, same directory, never overwritten) -> every "remove" line must be in
permissions.allow exactly once and no "add" line yet (or: all already applied: nothing changes)
-> remove them, append the new lines -> JSON written atomically with the file's mode, read back
and checked -> counts printed. Nothing else in the file changes. Exit 0 done or already applied;
3 STOP (nothing changed).
"""
import datetime
import json
import os
import shutil
import sys


def main():
    rules_path = sys.argv[1]
    f = sys.argv[2] if len(sys.argv) > 2 else "/opt/rust/src/github.com/HeaInSeo/ieo_poc/.claude/settings.local.json"
    rules = json.load(open(rules_path))
    remove, add = rules["remove"], rules["add"]
    d = json.load(open(f))
    allow = d.get("permissions", {}).get("allow", [])
    if all(allow.count(a) == 1 for a in add) and not any(r in allow for r in remove if r not in add):
        print(f"already applied: {len(add)} new lines present, no old line left; nothing changed")
        return 0
    missing = [r for r in remove if allow.count(r) != 1]
    present = [a for a in add if a in allow and a not in remove]
    if missing or present:
        print(f"STOP: {len(missing)} old lines not present exactly once, {len(present)} new lines already present; nothing changed")
        for r in missing:
            print("  missing:", r[:160])
        return 3
    stamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ")
    backup = f"{f}.bak-{stamp}"
    if os.path.exists(backup):
        print("STOP: backup exists"); return 3
    shutil.copy2(f, backup); os.chmod(backup, 0o600)
    if open(backup, "rb").read() != open(f, "rb").read():
        print("STOP: backup differs"); return 3
    before = len(allow)
    kept = [a for a in allow if a not in remove]
    d.setdefault("permissions", {})["allow"] = kept + [a for a in add if a not in kept]
    text = json.dumps(d, indent=2, ensure_ascii=False) + "\n"
    json.loads(text)
    tmp = f + ".tmp"
    fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_EXCL, os.stat(f).st_mode & 0o777)
    with os.fdopen(fd, "w") as w:
        w.write(text)
    os.replace(tmp, f)
    a2 = json.load(open(f))["permissions"]["allow"]
    ok = all(a2.count(a) == 1 for a in add) and not any(r in a2 for r in remove if r not in add)
    print(f"backup: {backup}")
    print(f"allow {before} -> {len(a2)} (removed {len(remove)}, added {len(add)}), deny {len(d['permissions'].get('deny', []))}; "
          f"new lines all present once: {ok}; JSON valid")
    return 0 if ok else 3


if __name__ == "__main__":
    sys.exit(main())
