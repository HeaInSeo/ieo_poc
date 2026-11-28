| Card | Verdict |
|---|---|
| B17 | Fail |
| D42 | Pass |
| F03 | Fail |
| H26 | Fail |

### B17

- **REQ-13 — Publication authority is unspecified.** An administrator credential could publish a verified archive while all AC-14 checks still pass. Two implementations could therefore grant different publication rights and both satisfy the stated test. Name the roles allowed to publish. If publication is publisher-only, require HTTP 403 and zero new records for a verified submission from any non-publisher role, including a role other than reader if one exists.

### D42

REQ-12, REQ-13, and AC-14 agree on who can publish and which verification state is accepted. AC-14 specifies concrete credentials, HTTP responses, and storage outcomes. Its coverage is limited to publication and authorization. REQ-11’s synthetic-only constraint has no corresponding check in AC-14.

### F03

- **AC-14 (REQ-12, REQ-13) — The acceptance criterion has no measurable outcome.** An implementation could return HTTP 201 and store an unverified archive submitted by a publisher, while a reviewer testing only successful publication could still call the feature satisfactory. Replace AC-14 with tests that start from empty storage and assert publisher plus verified → HTTP 201 and one stored archive, publisher plus unverified → HTTP 422 and storage still at one, and reader plus verified → HTTP 403 and storage still at one.

### H26

- **REQ-12 (conflicts with AC-14) — Verified publication is both permitted and forbidden.** If a publisher submits a verified archive and receives HTTP 201 with one new stored record, the implementation passes AC-14 and violates REQ-12’s prohibition. Rejecting the same submission would fail AC-14. To match the stated acceptance behavior, change REQ-12 to permit verified publication by the publisher and prohibit unverified publication. Then assert HTTP 201 with one new record for the verified submission and HTTP 422 with zero additional records for the unverified submission.
