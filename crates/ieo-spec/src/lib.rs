//! SprintSpec v0 registration contract (task 01-07) — reference implementation.
//!
//! Layers, kept separate on purpose (plan §1.4):
//! 1. format: JSON Schema (`SprintSpec_v0.schema.json`), reported as `MISSING_FIELD` / `SCHEMA_VIOLATION`;
//! 2. references: digest, duplicate IDs, undefined prerequisites, cycles, profile items, binding state;
//! 3. semantic review (P01) is *not* done here; an accepted check only means
//!    "may proceed to semantic review", never execution intake or completion.

pub mod check;
pub mod convert;
pub mod digest;
pub mod notion_url;
pub mod rejection;
pub mod schema;

pub use check::{CheckMode, CheckOutcome, CheckState, Registry, check_registration};
pub use convert::{ConversionContext, ConversionRecord, convert_human_input};
pub use digest::{canonical_json, spec_digest};
pub use rejection::{Code, Rejection};
