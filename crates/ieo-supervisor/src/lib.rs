//! 02-04 repo supervisor core, without model calls (runtime contract v0 rev 6/7, 01-02).
//!
//! * [`args`]: fixed role arguments (§4.1; file writes as `Edit(//…)`, never `Write(` — §10 item 1)
//! * [`env`]: empty environment plus the allow-list (§4.3)
//! * [`configdir`]: `CLAUDE_CONFIG_DIR` directive digest, tamper detection and cleanup (§5-6, §10 item 3)
//! * [`lock`]: one running process per session (§5-5, §10 item 4)
//! * [`checkpoint`]: checkpoint record and the reconnect decision (§5-1/2/4/6; resume key = session_id only, §10 item 5)
//! * [`budget`]: model-call budget guard (user decision 2026-11-13 ①) including the no-overage precondition
//!
//! Pods, PID 1 and the adapter wiring around a real `claude` process are separate steps of 02-04.

pub mod args;
pub mod budget;
pub mod checkpoint;
pub mod configdir;
pub mod env;
pub mod lock;
