//! ieo coordinator (02-03): PostgreSQL schema, authenticated HTTP/JSON API, assignments with
//! ownership heads and permission generations, event IDs, append-only history, outbox and
//! read model. Registration format checks (01-07) are separate from the independent
//! semantic review; verdicts take GitHub and execution evidence as inputs under the pinned
//! judgment policy (L07). A liveness answer is not a P04 result.

pub mod api;
pub mod auth;
pub mod chain;
pub mod delivery;
pub mod failover;
pub mod github_write;
pub mod grpc;
pub mod http;
pub mod limits;
pub mod policy;
pub mod recovery;
pub mod review_lifecycle;
mod review_store;
pub mod roles;
