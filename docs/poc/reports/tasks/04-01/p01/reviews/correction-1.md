| Card | Verdict | Basis |
|---|---|---|
| B17 | **Fail** | REQ-13 now identifies the sole publishing role, and AC-14 explicitly tests denial for a reader and a non-publisher administrator. The synthetic-only check lacks a testable definition. |
| F03 | **Fail** | AC-14 now specifies the credentials, response codes, and final record count, with a repeat from an empty database. The synthetic-only check lacks a testable definition. |
| H26 | **Fail** | REQ-12 now agrees with AC-14, and the readback checks that the stored archive is marked verified. The synthetic-only check lacks a testable definition. |

**Blocker in each card — REQ-11 and its check in AC-14.** A synthetic fixture can produce a count of `42`, and a production report can independently contain `42`. If the publisher stores the production-derived value, inspecting the stored number and checking the HTTP responses and record count cannot distinguish the two cases. The test can therefore pass while the archive contains production-derived data.

**Measurable correction.** Define eligible counts by provenance and state how the test observes it. For example, give AC-14 a controlled synthetic source and a production-derived test source, each returning `42` under distinct source identifiers. Assert that the publisher reads only the synthetic source and that the stored archive contains exactly the expected fixture fields and values. This gives the test an observable way to detect a source mix-up even when the numbers match.
