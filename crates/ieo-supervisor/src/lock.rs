//! One running process per session (runtime contract §5-5, rev 6 §10 item 4).
//!
//! The lock is a supervisor-owned file in the role volume holding the owner pid. A second
//! `reconnect` for a session whose lock holder is alive does not start `--resume`; it answers
//! with the holder (`status`). A lock whose holder no longer exists is stale and is replaced.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct SessionLock {
    path: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
pub enum LockError {
    /// The session is already running in `pid`.
    Running {
        pid: u32,
    },
    Io(String),
}

fn alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

impl SessionLock {
    pub fn acquire(dir: &Path, session_id: &str, pid: u32) -> Result<SessionLock, LockError> {
        let path = dir.join(format!("{session_id}.lock"));
        for _ in 0..2 {
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut f) => {
                    f.write_all(pid.to_string().as_bytes())
                        .map_err(|e| LockError::Io(e.to_string()))?;
                    return Ok(SessionLock { path });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    let holder = std::fs::read_to_string(&path)
                        .ok()
                        .and_then(|s| s.trim().parse::<u32>().ok());
                    match holder {
                        Some(p) if alive(p) => return Err(LockError::Running { pid: p }),
                        // stale (holder gone) or unreadable: replace once
                        _ => {
                            std::fs::remove_file(&path)
                                .map_err(|e| LockError::Io(e.to_string()))?;
                        }
                    }
                }
                Err(e) => return Err(LockError::Io(e.to_string())),
            }
        }
        Err(LockError::Io("lock could not be taken".into()))
    }
}

impl Drop for SessionLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
