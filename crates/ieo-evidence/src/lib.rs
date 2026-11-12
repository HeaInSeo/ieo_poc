//! 02-01 (T03): command execution, evidence collection and usage metering.
//!
//! One command run produces one evidence directory
//! `<out>/<task_id>/<attempt_id>/<command_id>/` with the redacted `stdout`,
//! `stderr` and a `record.json` in the format `ieo-evidence-record/v0`
//! (`contracts/evidence-record-v0.schema.json`).
//!
//! Rules carried over from 01-04 and the runtime contract (rev 6):
//! - exit code 0 alone is never success: a required result event, artifact
//!   or marker that is missing makes the run `incomplete`;
//! - an intentional deny/abort test is judged against its own expected outcome;
//! - usage that was not observed is `unmeasured` with a reason, never 0;
//!   sums over runs say which runs they exclude;
//! - secret values never reach the evidence files (they are masked before
//!   anything is written; only counts of replacements are recorded);
//! - the wrapper never retries; a retry is a new run linked with `retry_of`.

pub mod aggregate;
pub mod judge;
pub mod record;
pub mod redact;
pub mod run;
pub mod stream;
pub mod time;
