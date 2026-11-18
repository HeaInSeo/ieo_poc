"""Add observation hooks to a private copy of the frozen 03-03 runner."""
from pathlib import Path
import shutil
import sys


def stage(source: Path, target: Path) -> None:
    shutil.copytree(source, target, ignore=shutil.ignore_patterns("evidence", "*.md", "*.sha256"))
    path = target / "verify-cases.sh"
    script = path.read_text()
    marker = '  /usr/bin/time -f'
    assert script.count(marker) == 1
    script = script.replace(
        marker,
        '''  record="/scratch/results/$name.observed"
  printf '%s\\0' "$@" >"$record.argv"
  date -u +%Y-%m-%dT%H:%M:%S.%NZ >"$record.start"
''' + marker,
    )
    existing = '>"/scratch/results/$name.log" 2>&1 || actual=$?'
    assert script.count(existing) == 1
    script = script.replace(
        existing,
        '''>"$record.stdout" 2>"$record.stderr" || actual=$?
  date -u +%Y-%m-%dT%H:%M:%S.%NZ >"$record.end"
  printf '%s\\n' "$actual" >"$record.exit"
  cat "$record.stdout" "$record.stderr" >"/scratch/results/$name.log"''',
    )
    path.write_text(script)


if __name__ == "__main__":
    stage(Path(sys.argv[1]), Path(sys.argv[2]))
