# 02-02 evaluation summary: candidate ieo-evidence @ 1785cb7

- Candidate: commit `1785cb76e1b5456150865453a990b9b910d56a9e`, built with `cargo build --offline --locked --release -p ieo-evidence` from `git archive`. Binary sha256 `8125cc4a370ec0c52ba1c45b1446b091f32179e8d884d3f76b5f25564211ce16` (see `build.txt`).
- Frozen inputs: manifest.json sha256 `ec486cc51cfb50aa41127a51fcaeb2c97139baca1a59a58f50ed8a6ac34be57c`, committed in 61a27aa. All 42 files were re-verified before the evaluation (`freeze-verify.txt`, MATCH).
- Evaluation window: 2026-11-12T13:55:01.235Z to 13:55:19.399Z. Each set was run once; the exact commands are in `eval-times.txt`. The validation set was not re-run.

| Set | Determined cases | Passed | Failed | Undetermined cases excluded | Undetermined fields excluded | Fields checked / failed |
|---|---|---|---|---|---|---|
| public | 5 | 5 | 0 | 0 | 3 | 68 / 0 |
| authoring | 14 | 14 | 0 | 0 | 19 | 234 / 0 |
| validation | 34 | 33 | 1 | 1 (V-35) | 52 | 628 / 1 |

## Undetermined (excluded)

- V-35 (whole case, not executed): a natural exit racing the timeout deadline (open item P3-6). The approved rules do not fix which of exited/timed_out is recorded.
- Fields (all sets: 3 + 19 + 52 = 74):
  - wording of the launch error, reasons, attribution detail and redaction placeholder
  - `secret_sources`, `usage.by_model`, `runtime_version`
  - `signal` for timed_out, `exit_code` for signaled, `stray_group_killed` when the leader ignores TERM
  - session measure for a new session with empty modelUsage
  - echo of a rejected `resume_base`
  - judgment where the only open question is unmeasured usage
  - V-33 judgment/exit (P3-2: subtype not success, is_error false)
  - V-34 judgment/exit (P3-4: model request with `--expect timeout`)

  Details are in AUTHOR_NOTES.md.

## Failures

| Case | Field | Expected | Observed | Classification |
|---|---|---|---|---|
| V-22 | `tree` ($tree_not_contains over OUT) | none of 3 listed strings in any evidence file | one string found in `vt/a22/c/record.json` | **scorer-suspect** (case authoring error), not a candidate defect |

### V-22: why it is scorer-suspect

V-22 asserted that three strings never appear under OUT:
1. the secret value
2. its SHA-256 hex
3. the tail of a `sk-ant-…` token (`Qw3rTy…_ab`)

The token was printed to stderr, and that copy is masked. But to keep the token out of argv, I deliberately wrote it split in the shell script (`k="sk-""ant-oat01-<tail>"`). The script text is argv[2], and the record stores argv. Split that way, the tail is not in `sk-ant-` form, so a correct implementation leaves it unmasked in `record.json`. My own `argv_replacements = 2` expectation already assumed exactly that.

The authoring error was putting string 3 in the tree list. All V-22 count fields passed:
- stdout_replacements 3, stderr_replacements 2, argv_replacements 2
- secret_value_seen true
- marker found, succeeded, exit 0

A separate, clearly labelled diagnostic used different values in the same shape. It was not a re-run of the validation case; it is kept in `diag-secret/` and its output is quoted in the `diag-secret` paragraph below. It showed:
- the secret value and its SHA hex appear in no file
- the token tail appears only in `record.json` argv, inside the split literal
- the real token in stderr is masked as `sk-ant-[MASKED]`

The frozen scorer and set were not edited. Corrected result for V-22 if string 3 were removed: PASS. That is an inference and was not re-scored.

## Diagnostic only (not scoring)

`diag-secret/` holds the one manual invocation described above. Command shape: the V-22 command with secret `Diag-secret-VALUE-0001` and token tail `Zz9yXx0123456789_cd`. It printed:
- grep hits: value none, SHA none, tail only in `record.json`
- argv `[..., "[SECRET-VALUE-REDACTED]", "[SECRET-SHA256-REDACTED]"]`
- stderr `e=[SECRET-VALUE-REDACTED] k=sk-ant-[MASKED]`

Observation for the coordinator (not a failure): on `run` the candidate prints a one-line JSON summary on its own stdout.
