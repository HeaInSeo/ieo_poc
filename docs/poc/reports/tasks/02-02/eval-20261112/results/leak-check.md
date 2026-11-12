# 02-02 answer-leak checks (within current scope)

## a) Grep of the candidate tree and the repository

Scope:
- the candidate tree is a `git archive 1785cb7` extracted to `build/src`, excluding `target/`
- the repository is a `git archive 61a27aa`, the freeze commit, deleted after the grep

This was grep only; I read no source for understanding. The full term list and hits are in `leak-grep.txt`. There were 91 terms:
- all 35 validation case ids (`V-01`…`V-35`)
- every distinctive input I chose: secret values and the secret's SHA hex, the sk-ant tail, refused-secret values, env names and values, marker strings, printed strings, sleep durations, task/attempt/command ids such as `loose-resume` and `a13-0`
- synthetic stream identifiers: session-id prefix, message ids, model names, agent/tool ids
- distinctive expected token fragments of the synthetic streams

Result: **no validation case id and no input or expected literal I created appears in either tree.** The only hits:
- `WRITE_DENIED` and `ECHO-OK`. These are not mine. They are existing 01-04/02-01 literals (the 01-04 deny probe, the real T23 stream that is fixture fx-02, and 02-01's own tests) that were deliberately reused from the approved inputs. They reveal nothing about expectations.
- `a14-4`: a coincidental substring of a UUID (`…da14-4b13…`) inside a 01-04 real stream.

At 61a27aa the only 02-02 evaluation file is `docs/poc/reports/tasks/02-02/eval-20261112/freeze/manifest.json`, byte-identical to OUT/manifest.json. The sets, scorer and fixtures are not in the repository.

Verdict: **PASS**.

## b) Ordering (UTC)

| Event | Time | Source |
|---|---|---|
| Candidate commit 1785cb7 | 2026-11-12T13:40:27Z (22:40:27+09:00) | `git log -1 --format=%cI 1785cb7` |
| Manifest `created_at` | 2026-11-12T13:53:52Z | manifest.json |
| Freeze commit 61a27aa | 2026-11-12T13:54:14Z (22:54:14+09:00) | `git log -1 --format=%cI 61a27aa` |
| Freeze re-verification | 2026-11-12T13:54:35Z | freeze-verify.txt |
| First evaluation start | 2026-11-12T13:55:01.235Z | eval-times.txt |

The required order holds: candidate < manifest < freeze commit < evaluation.

The last commit touching `crates/ieo-evidence` up to 1785cb7 is 1785cb7 itself. The candidate was built from that exact commit, so no later candidate change could have used the sets.

Verdict: **PASS**.

## c) Content of the frozen manifest

manifest.json contains:
- `created_at`
- the candidate commit
- `rules_used`: document paths
- `not_read`: paths
- a sealing statement
- `files`: relative path and sha256 only
- `counts` per set: cases, determined, undetermined, determined_fields, undetermined_fields and `by_family`

It contains no case content, no inputs and no expected values.

The family labels (for example `secret_masking`, `model_wrong_base`, `leftover_member_drain`) do reveal which behaviour categories are covered and how many cases each has. They do not reveal inputs, values or which way a judgment goes. These categories are the ones listed in the task brief, which the developer already knows. Disclosure: topic coverage only, low.

Verdict: **PASS** (with that disclosure noted).

## d) Limits of these checks

**개발 계정의 정답 접근 차단은 미검증.** The sets, scorer and fixtures were in a scratchpad readable by the same OS account the development agent uses. Strong sealing was deferred by user decision A2 until before 04-07/P07. The checks above show only two things:
1. none of my chosen literals reached the candidate tree or the repository before evaluation;
2. the timing order is correct.

They cannot rule out that the files were read. **Passing these checks is not strong-sealing verification, and it does not guarantee the overall correctness of the development.** The evaluation covers only the behaviours in the case sets, compared on determined fields.
