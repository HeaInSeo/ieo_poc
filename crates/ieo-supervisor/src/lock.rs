//! One running process per session (runtime contract §5-5, rev 6 §10 item 4).
//!
//! The lock is an advisory OS file lock (`flock`, via `File::try_lock`) on a supervisor-owned
//! file in the role volume. The kernel releases it when the holder's file descriptor closes,
//! including when the process dies, so there is no stale-lock takeover, no pid-reuse question
//! and no window between creating the file and writing the pid. The pid in the file is only
//! information for the `status` answer; it is written after the lock is held and may be empty
//! while the holder is starting. The file is never removed (removing it would let a second
//! taker lock a new inode while the first still holds the old one).
//!
//! Limit (§5-5, §2.1): development code in the same container can delete the lock file; the
//! effect stays inside the development role's own sessions.

use std::fs::{File, OpenOptions, TryLockError};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

#[derive(Debug)]
pub struct SessionLock {
    _file: File,
}

#[derive(Debug, PartialEq, Eq)]
pub enum LockError {
    /// The session is already running; `pid` as written by the holder (None while it starts).
    Running {
        pid: Option<u32>,
    },
    Io(String),
}

impl SessionLock {
    pub fn acquire(dir: &Path, session_id: &str, pid: u32) -> Result<SessionLock, LockError> {
        let io = |e: std::io::Error| LockError::Io(e.to_string());
        let path = dir.join(format!("{session_id}.lock"));
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(io)?;
        match f.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                let mut s = String::new();
                let _ = f.read_to_string(&mut s);
                return Err(LockError::Running {
                    pid: s.trim().parse().ok(),
                });
            }
            Err(TryLockError::Error(e)) => return Err(io(e)),
        }
        f.set_len(0).map_err(io)?;
        f.seek(SeekFrom::Start(0)).map_err(io)?;
        f.write_all(pid.to_string().as_bytes()).map_err(io)?;
        f.flush().map_err(io)?;
        Ok(SessionLock { _file: f })
    }
}
