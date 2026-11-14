//! 02-04 repo supervisor core, without model calls (runtime contract v0 rev 6/7, 01-02).
//!
//! * [`args`]: fixed role arguments (§4.1; file writes as `Edit(//…)`, never `Write(` — §10 item 1)
//! * [`env`]: the allow-list environment, applied to a command after clearing it (§4.3)
//! * [`configdir`]: `CLAUDE_CONFIG_DIR` directive digest, tamper detection and cleanup without following links, fail-closed on read errors (§5-6, §10 item 3)
//! * [`lock`]: one running process per session, as an OS file lock (§5-5, §10 item 4)
//! * [`checkpoint`]: the §5-1 record and the reconnect decision (§5-2/5/6, §6; resume key = session_id only, §10 item 5)
//! * [`budget`]: model-call budget guard (user decision 2026-11-13 ①) checked between runs, with the no-overage precondition and an in-stream stop signal
//!
//! * [`serve`]: the role container's PID 1 service (adapter start/reconnect/stop/status)
//! * [`run`]: in-run enforcement (time, observed tokens, extra-usage signals) and process-group cleanup
//!
//! Pods, PID 1 and the adapter wiring around a real `claude` process are separate steps of 02-04.

pub mod args;
pub mod budget;
pub mod caller;
pub mod checkpoint;
pub mod configdir;
pub mod env;
pub mod lock;
pub mod run;
pub mod serve;
