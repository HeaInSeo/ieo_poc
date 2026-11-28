**F03 — Fail.** REQ-11 is testable through the source counters and stored provenance. REQ-12 and REQ-13 agree with the expected response codes, and resetting the database and counters before each run makes the assertions repeatable. AC-14 checks storage only at the end of each run, leaving a gap at the rejection steps.

**Blocker — AC-14, covering REQ-12 and REQ-13.** The publisher’s unverified submission could store a second archive using the cached fixture count and still return `422`. The reader’s submission could delete that second archive and return `403`. Every stated assertion would pass even though the unverified archive was available between requests.

**Measurable correction.** Read back storage immediately after the `201` and record the sole archive’s ID, status, count, and source identifier. Immediately after each `422` and `403`, assert that storage still contains exactly that record with those same values. Retain the per-run source-counter assertions.
