# Review 4 of PR #40: 02-04 run guard (correction 3, last)

- Head reviewed: `23fee807e907917cb8dc463930c39da99d67bf77` (confirmed with `gh pr view 40 --json headRefOid,mergeStateStatus`: head matches, `mergeStateStatus` BLOCKED, i.e. awaiting review), base `f582403`. Correction diff checked: `git diff 5136d66 23fee80`.
- **Verdict: APPROVE.** No blocking defect. 2 Low and 4 Info items remain as follow-ups for the adapter step.

## What I ran

All work was done in a scratch `git worktree` at the head, with its own `CARGO_TARGET_DIR`. Both were removed afterwards.

- `cargo clippy -p ieo-supervisor --all-targets -- -D warnings`: clean.
- `cargo test -p ieo-supervisor`: 25 (`run_guard`) + 20 pass. I also ran 3 `cargo test` invocations in parallel: all passed.
- The `run_guard` binary ran 20 times in sequence, then 5 x 8 in parallel (40 runs): 0 failures. No `sleep 300`, fake processes or `/tmp/ieo-run-*` directories were left behind.
- PR `mutants.py`, run from a scratch copy with the `.venv-poc` python: 27/27 killed, with the same test names as in `mutants.txt`. Its tree was clean afterwards. `run.rs` and the tests are identical between `1ccd75d`, where the mutants evidence was recorded, and the head.
- My extra mutants (Y1 to Y8) and direct `charge` probes (P1 to P6) used throwaway files, which were not committed.

## Status of the 5136d66 findings

| # | Status | Notes |
|---|---|---|
| 1 Medium: a stopped run is measured below the observed bound | Resolved | `run::charge` charges every run with `stop != Exited` as unmeasured, whatever `result` it wrote. An exited run is charged unmeasured when the collector's billable or cache_read is below the guard's observed bound, or when a token field is missing or not a number. Two tests use the real collector and `budget::next`: P1 (zero-usage `error_during_execution` result, giving `Decision::Stop`) and R26 (a complete real `result` written after a TERM is still not trusted). The module doc and PROGRESS §1 row 4 and §2 bullet 2 now state the rule. |
| 2 Low: stale PROGRESS statements | Resolved | The broken-`kill` path is described as tested. The §1 table cites the new tests, and the copy is described as per line with lossy decoding and `\r` trimmed. One residual wording point: see finding 4. |
| 3 Low: an error return loses the cleanup report | Resolved | `with_cleanup` appends "cleanup failed: processes of the run may remain" on all three `Err` paths after spawn. It is tested with `/dev/full` plus a broken `kill`, and killed by R28 and my Y8. The pre-reader "no stdout pipe" return does not carry it, but that path is unreachable because stdout is always piped. |
| 4 Low: the 2 s post-stop copy bound is untested | Resolved | There is a `setsid` pipe-holder test with `elapsed < 5 s` for a 0.5 s timeout and 0.5 s grace. It kills R25 (formerly X8, the unbounded pump). |
| 5, 6 Info | Resolved as stated | |

Earlier findings did not regress:
- R1 to R22 and R24 are still killed;
- the drain and the copy are still bounded;
- evidence files are still created before spawn;
- the group-gone check through `/proc` and the leader fallback are unchanged.

## Soundness of `run::charge`

- **Field names.** `charge` reads `state == "measured"` and `tokens.{input, output, cache_creation, cache_read}`. These match `ieo_evidence::record::Measure` (serde `tag = "state"`, `snake_case`) and `Tokens`. They are the same fields and the same billable definition (input + output + cache_creation) as `budget::spent`. The collector's `usage.run` includes children and CLI-internal calls (`record.rs` doc on `Usage::run`), and so does the guard's observed sum. The comparison is like for like.
- **Fail closed.** The checks:
  - Any stop other than `Exited` gives unmeasured. Y5 and Y6 are killed.
  - A collector figure below the observed bound gives unmeasured. R27, Y2 and Y3 are killed.
  - Missing or non-numeric fields give unmeasured (P4).
  - A collector `unmeasured` passes through unchanged.
  - A non-object or `null` collector value is returned as is, and `budget::spent` then counts it as unmeasured (P3).

  The unmeasured value keeps `guard_observed` and `collector_reported` for the report, and never turns into 0.
- **Charged below what was used?** Not for a stopped run. For an exited run, the collector's `result` figure is authoritative by design, and the guard's lower bound acts as a floor. One minor difference: `charge` saturates while `spent` uses `checked_add`. An overflow is therefore accepted by `charge` and rejected by `spent`, so the end result is still unmeasured.
- **A legitimate exited run wrongly blocked?** Only if the collector's run share were below the start-of-message usage the stream showed. The collector's run share includes children and, for a resumed session, is the session total minus the base. Neither should fall below the in-stream lower bound for this run. If it ever does, the result is a conservative stop that needs a person's decision, not a budget overrun. That is acceptable.

## Findings

### 1. Low: an exited run with `cleanup_failed` is charged as measured, and the next call is allowed

Probe P1: `Outcome { stop: Exited, cleanup_failed: true }` with a collector figure at or above the observed bound gives `charge` = measured. In that state, members of the group may still be running, for example a background process that a tool call started. Their model use would not be counted, and nothing in `charge` or `budget::next` blocks the next call.

Reaching this needs the group `kill` to fail while members are alive, which is very unlikely with the system `kill`. A `setsid` descendant is not detected at all (already documented). Follow-up for the adapter: treat `cleanup_failed` (and any `Err` from `run`) as "stop and report", or make `charge` return unmeasured when `cleanup_failed` is set.

### 2. Low: test gaps in `charge`

Extra mutants against `charge`:

| Mutant | Result |
|---|---|
| Y1: the cache_read comparison is dropped | **SURVIVED**. No test has a collector cache_read below the observed one. P2 shows the code is right. |
| Y4: a missing token field is read as 0 | SURVIVED. P4 shows the code is right, and `spent` also rejects missing fields. |
| Y7: a non-measured collector value is turned into measured zero (an added-code mutant) | SURVIVED. No test calls `charge` with an unmeasured collector value. |
| Y2: billable not compared | killed |
| Y3: `>` instead of `>=` | killed |
| Y5: a stopped run with a measured `result` is trusted | killed |
| Y6: only token-cap stops are untrusted | killed |
| Y8: cleanup flag dropped on the `watch` error path | killed |

The code is correct in each surviving case. Only the pinning is missing. Follow-up: add a cache_read-below case and an unmeasured pass-through case.

### 3. Info: `charge` must actually be used

`charge` is a helper, and nothing yet forces the adapter to write its value, rather than the collector's `usage.run`, into the record that `budget::spent` reads. The adapter step should make that the only path, and test it end to end.

### 4. Info: PROGRESS §2, last bullet

The bullet says "종료하며 쓴 `result`가 남으면 계측기가 그 회차를 계측할 수 있다". That is true of the collector. Read alone, it suggests the run is measured for the budget, which §2 bullet 2 and `charge` contradict. Suggest adding "(예산에는 `run::charge`가 미계측으로 넣는다)".

### 5. Info: shape of the `charge` return value

`charge` returns the collector value unchanged when it is not an object (P3: `null`). `spent` handles that safely, but the record would then hold a `usage.run` that is not a `Measure`. Returning an explicit `unmeasured` for any state other than `measured` would keep the record schema valid.

### 6. Info: incomplete assistant lines

On an exited run with `incomplete_lines > 0`, the observed floor is understated (missing fields count as 0), and `charge` trusts the collector (P5). That is reasonable, because `result` is authoritative. It would help if the report showed `incomplete_lines`, and it already does, under `guard_observed`, only when the run is unmeasured.

## Are the §3 claims proven?

| §3 requirement | Proven by |
|---|---|
| time limits with child cleanup | R1, R2, R7, R9 to R11, R19 to R21 and R25 are killed. There are tests for a broken `kill`, an error return and a `setsid` pipe holder. |
| stop on observed usage at the cap | R4, R5 and R6 are killed, and the boundary is tested. |
| stop on extra-usage or limit signals | R3 is killed. |
| block the next call when measurement fails | A guard stop gives unmeasured, both before `result` and when a `result` is written on the way out (zero or complete). Both go through the real collector, `charge` and `budget::next`. R26, R27, Y5 and Y6 are killed. |
| positive control | A real 01-04 stream is measured, `charge` equals the collector's figure, and the next call is allowed. |

## Honesty of the docs

The module doc and PROGRESS state:
- observe-then-stop, not a provider limit;
- usage as a lower bound, with fixture numbers;
- final usage only from `result`;
- a stopped run always unmeasured, even with a late `result`;
- an unmeasured run blocks until a person decides;
- the `setsid` limit;
- a time overrun of tens of ms.

The model-call count of 0 and "11/73 unchanged" are correct. 27/27 mutants and 25 tests are reproduced. The only wording point is finding 4.

## Verdict

**APPROVE.** Findings 1 and 3 belong to the adapter step, before the first real budgeted call. Findings 2, 4, 5 and 6 are optional clean-ups.
