//! The work tree's `HEAD` and the pushed SHA of the assigned branch, read from the git files
//! (02-05 step 7, runtime contract revision 14 §17).
//!
//! The supervisor never runs `git` in the role's work tree: the repository is the model's (same
//! UID), and its configuration could run code (hooks, `core.fsmonitor`). It reads only the ref
//! files: `.git/HEAD` (or the `gitdir:` a `.git` file names, and its `commondir`), loose refs and
//! `packed-refs`. A value is accepted only when it is a lowercase hex SHA (40 or 64 characters);
//! anything else — no repository, a symbolic ref that does not resolve, an oversized file — is
//! "not observed" (`None`).
//!
//! The pushed SHA is the remote-tracking ref `refs/remotes/origin/<branch>` that a `git push` to
//! `origin` updates. Both values are claims of the role volume: development code can change them.
//! The coordinator uses them only as the expected value and compares them with the GitHub record
//! (evidence-service input), which is the final basis of a resume.

use std::path::{Path, PathBuf};

/// Ref files larger than this are not read.
const MAX_REF_FILE: u64 = 1 << 20;

fn is_sha(s: &str) -> bool {
    (s.len() == 40 || s.len() == 64)
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn read_small(p: &Path) -> Option<String> {
    let meta = std::fs::metadata(p).ok()?;
    if !meta.is_file() || meta.len() > MAX_REF_FILE {
        return None;
    }
    std::fs::read_to_string(p).ok()
}

/// The git directory and the common directory of the work tree (`.git` directory, or a `.git`
/// file `gitdir: <path>` with an optional `commondir`).
fn dirs(worktree: &Path) -> Option<(PathBuf, PathBuf)> {
    let dot = worktree.join(".git");
    let gitdir = if dot.is_dir() {
        dot
    } else {
        let t = read_small(&dot)?;
        let p = t.trim().strip_prefix("gitdir:")?.trim().to_string();
        let p = PathBuf::from(p);
        if p.is_absolute() { p } else { worktree.join(p) }
    };
    let common = match read_small(&gitdir.join("commondir")) {
        Some(c) => {
            let c = PathBuf::from(c.trim());
            if c.is_absolute() { c } else { gitdir.join(c) }
        }
        None => gitdir.clone(),
    };
    Some((gitdir, common))
}

/// A full ref (`refs/...`) as a SHA: the loose file in the git or common directory, else
/// `packed-refs`. A ref name with `..` or outside `refs/` is not read.
fn resolve(gitdir: &Path, common: &Path, name: &str) -> Option<String> {
    if !name.starts_with("refs/") || name.contains("..") || name.contains('\\') {
        return None;
    }
    for d in [gitdir, common] {
        if let Some(t) = read_small(&d.join(name)) {
            let t = t.trim();
            return is_sha(t).then(|| t.to_string());
        }
    }
    let packed = read_small(&common.join("packed-refs"))?;
    packed.lines().find_map(|l| {
        let (sha, r) = l.split_once(' ')?;
        (r.trim() == name && is_sha(sha)).then(|| sha.to_string())
    })
}

/// The work tree's `HEAD` commit, or `None` when it cannot be observed.
pub fn head(worktree: &Path) -> Option<String> {
    let (gitdir, common) = dirs(worktree)?;
    let t = read_small(&gitdir.join("HEAD"))?;
    let t = t.trim();
    match t.strip_prefix("ref:") {
        Some(r) => resolve(&gitdir, &common, r.trim()),
        None => is_sha(t).then(|| t.to_string()),
    }
}

/// The SHA last pushed to `branch_ref` (`refs/heads/<name>`) on `origin`, as the work tree's
/// remote-tracking ref records it; `None` when nothing was pushed (or it cannot be read).
pub fn pushed(worktree: &Path, branch_ref: &str) -> Option<String> {
    let name = branch_ref.strip_prefix("refs/heads/")?;
    let (gitdir, common) = dirs(worktree)?;
    resolve(&gitdir, &common, &format!("refs/remotes/origin/{name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    #[test]
    fn head_and_pushed_sha_are_read_from_the_ref_files_only() {
        let d = std::env::temp_dir().join(format!("ieo-gitref-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("dir");
        // no repository: nothing observed
        assert_eq!((head(&d), pushed(&d, "refs/heads/feat/x")), (None, None));
        let g = d.join(".git");
        std::fs::create_dir_all(g.join("refs/heads/feat")).expect("refs");
        std::fs::write(g.join("HEAD"), "ref: refs/heads/feat/x\n").expect("HEAD");
        // a symbolic HEAD that does not resolve yet (no commit)
        assert_eq!(head(&d), None);
        std::fs::write(g.join("refs/heads/feat/x"), format!("{A}\n")).expect("ref");
        assert_eq!(head(&d).as_deref(), Some(A));
        // nothing pushed yet; then the remote-tracking ref from packed-refs, then a loose one
        assert_eq!(pushed(&d, "refs/heads/feat/x"), None);
        std::fs::write(
            g.join("packed-refs"),
            format!("# pack-refs with: peeled\n{B} refs/remotes/origin/feat/x\n"),
        )
        .expect("packed");
        assert_eq!(pushed(&d, "refs/heads/feat/x").as_deref(), Some(B));
        std::fs::create_dir_all(g.join("refs/remotes/origin/feat")).expect("remote");
        std::fs::write(g.join("refs/remotes/origin/feat/x"), format!("{A}\n")).expect("loose");
        assert_eq!(pushed(&d, "refs/heads/feat/x").as_deref(), Some(A));
        // not a SHA, not a branch ref, a path escape: not observed
        std::fs::write(g.join("refs/remotes/origin/feat/x"), "not a sha\n").expect("bad");
        assert_eq!(pushed(&d, "refs/heads/feat/x"), None);
        assert_eq!(pushed(&d, "feat/x"), None);
        assert_eq!(pushed(&d, "refs/heads/../../HEAD"), None);
        // a symbolic HEAD that climbs out of refs/ to a file holding a SHA is not followed
        // (review f0e0ca9 L-3)
        std::fs::write(g.join("HEAD-sha"), format!("{A}\n")).expect("HEAD-sha");
        std::fs::write(g.join("HEAD"), "ref: refs/../HEAD-sha\n").expect("HEAD");
        assert_eq!(head(&d), None);
        // a detached HEAD
        std::fs::write(g.join("HEAD"), format!("{B}\n")).expect("detached");
        assert_eq!(head(&d).as_deref(), Some(B));
        // a linked work tree: `.git` file with gitdir and commondir
        let w = d.join("wt");
        std::fs::create_dir_all(&w).expect("wt");
        let wg = g.join("worktrees/wt");
        std::fs::create_dir_all(&wg).expect("wg");
        std::fs::write(w.join(".git"), format!("gitdir: {}\n", wg.display())).expect(".git");
        std::fs::write(wg.join("commondir"), "../..\n").expect("commondir");
        std::fs::write(wg.join("HEAD"), "ref: refs/heads/feat/x\n").expect("HEAD");
        std::fs::write(g.join("refs/heads/feat/x"), format!("{B}\n")).expect("ref");
        assert_eq!(head(&w).as_deref(), Some(B));
        let _ = std::fs::remove_dir_all(&d);
    }
}
