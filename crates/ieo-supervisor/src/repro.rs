//! One-shot, non-model execution for an independent reproduction.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

const INPUT_LIMIT: u64 = 16 * 1024 * 1024;
const LOG_LIMIT: usize = 128 * 1024;

pub struct Runner {
    pub repository: PathBuf,
    pub bundles: PathBuf,
    pub state: PathBuf,
    pub image: String,
}

pub struct Job<'a> {
    pub key: &'a str,
    pub sha: &'a str,
    pub bundle: &'a str,
    pub command: &'a [Value],
    pub timeout: u64,
}

fn local(executable: &str) -> Command {
    let mut command = Command::new("/usr/bin/timeout");
    command.args(["--kill-after=2s", "20s", executable]);
    command
        .env_remove("CONTAINER_HOST")
        .env_remove("CONTAINER_CONNECTION");
    command.stdin(Stdio::null());
    command
}

fn podman() -> Command {
    let mut command = local("/usr/bin/podman");
    command.arg("--remote=false");
    command
}

fn run(command: &mut Command) -> Result<Output, String> {
    let out = command.output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("local adapter exited {}", out.status));
    }
    Ok(out)
}

fn start_if_rootless(probe: Result<Output, String>, start: impl FnOnce()) -> Result<(), String> {
    let observed = probe?;
    if observed.stdout.trim_ascii() != b"true" {
        return Err("Podman rootless mode was not confirmed".into());
    }
    start();
    Ok(())
}

impl Runner {
    pub fn new(
        repository: &Path,
        bundles: &Path,
        state: &Path,
        image: String,
    ) -> Result<Self, String> {
        let trusted_dir = |path: &Path| -> Result<PathBuf, String> {
            let path = path.canonicalize().map_err(|e| e.to_string())?;
            if !path.is_dir()
                || path
                    .to_str()
                    .is_none_or(|s| s.contains(':') || s.contains(','))
            {
                return Err("invalid reproduction directory".into());
            }
            Ok(path)
        };
        if !image.strip_prefix("sha256:").is_some_and(sha256) {
            return Err("runner image must be a full local image ID".into());
        }
        Ok(Self {
            repository: trusted_dir(repository)?,
            bundles: trusted_dir(bundles)?,
            state: trusted_dir(state)?,
            image,
        })
    }

    fn inspect(key: &str) -> Option<Value> {
        let out = run(podman().args(["container", "inspect", key])).ok()?;
        let response: Value = serde_json::from_slice(&out.stdout).ok()?;
        response.as_array()?.first().cloned()
    }

    /// Prepare and verify the isolated container before it may be started.
    pub fn prepare(&self, job: &Job<'_>) -> Result<(), String> {
        let stage = self.state.join(job.key);
        fs::create_dir(&stage).map_err(|e| e.to_string())?;
        let input = stage.join("input");
        fs::create_dir(&input).map_err(|e| e.to_string())?;
        if !sha256(job.bundle)
            || !matches!(job.sha.len(), 40 | 64)
            || !job
                .sha
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("invalid candidate or bundle identity".into());
        }
        let bundle = fs::OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
            .open(self.bundles.join(job.bundle))
            .map_err(|e| e.to_string())?;
        if !bundle.metadata().map_err(|e| e.to_string())?.is_file() {
            return Err("bundle is not a regular file".into());
        }
        let mut bytes = Vec::new();
        bundle
            .take(INPUT_LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > INPUT_LIMIT || hex::encode(Sha256::digest(&bytes)) != job.bundle {
            return Err("bundle is oversized or its digest differs".into());
        }
        fs::File::create(input.join("bundle"))
            .and_then(|mut file| file.write_all(&bytes))
            .map_err(|e| e.to_string())?;
        let output = input.join("source.tar");
        let git = || {
            let mut command = local("/usr/bin/git");
            command
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_ATTR_NOSYSTEM", "1")
                .env(
                    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
                    self.repository.join("objects"),
                );
            command
        };
        let scratch = stage.join("git");
        let object_format = if job.sha.len() == 64 {
            "--object-format=sha256"
        } else {
            "--object-format=sha1"
        };
        run(git()
            .args(["init", "--bare", "--template=", object_format])
            .arg(&scratch))?;
        fs::create_dir_all(scratch.join("info")).map_err(|e| e.to_string())?;
        // A candidate's export-ignore / export-subst settings do not redefine checkout contents.
        fs::write(
            scratch.join("info/attributes"),
            "* -export-ignore -export-subst\n",
        )
        .map_err(|e| e.to_string())?;
        let in_scratch = || {
            let mut command = git();
            command.arg("--git-dir").arg(&scratch);
            command
        };
        if run(in_scratch().args(["cat-file", "-t", job.sha]))?.stdout != b"commit\n" {
            return Err("target object is not a commit".into());
        }
        let mut archive_file = fs::File::create(&output).map_err(|e| e.to_string())?;
        let mut archive = in_scratch()
            .args(["archive", "--format=tar", job.sha])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let limit = 128 * 1024 * 1024;
        let copied = std::io::copy(
            &mut archive.stdout.take().expect("archive pipe").take(limit + 1),
            &mut archive_file,
        );
        if !matches!(&copied, Ok(size) if *size <= limit) {
            let _ = archive.kill();
        }
        if !archive.wait().is_ok_and(|status| status.success())
            || !matches!(copied, Ok(size) if size <= limit)
        {
            return Err("candidate archive unavailable or oversized".into());
        }
        let mount = format!("{}:/inputs:ro,Z", input.display());
        let mut create = podman();
        create.args([
            "create",
            "--pull=never",
            "--network=none",
            "--http-proxy=false",
            "--unsetenv-all",
            "--read-only",
            "--read-only-tmpfs=false",
            "--image-volume=ignore",
            "--cap-drop=ALL",
            "--security-opt=no-new-privileges",
            "--pids-limit=0",
            "--ulimit=nproc=16:16",
            "--ulimit=nofile=128:128",
            "--ulimit=fsize=33554432:33554432",
            "--userns=keep-id",
            "--user=65534:65534",
            "--pid=private",
            "--tmpfs=/work:rw,size=128m,mode=1777",
            "--tmpfs=/tmp:rw,size=32m,mode=1777",
            "--log-driver=k8s-file",
            "--log-opt=max-size=128kb",
            "--entrypoint=/bin/sh",
            "--env=PATH=/usr/bin:/bin",
            "--env=REPRO_BUNDLE=/inputs/bundle",
            "--name",
            job.key,
            "--timeout",
            &job.timeout.to_string(),
            "--volume",
            &mount,
            &self.image,
            "-ec",
            "ulimit -v 131072; tar -xf /inputs/source.tar -C /work && cd /work && exec \"$@\"",
            "repro",
        ]);
        for arg in job.command {
            create.arg(arg.as_str().ok_or("invalid command argument")?);
        }
        run(&mut create)?;
        let container = Self::inspect(job.key).ok_or("created container cannot be inspected")?;
        let mounts = container["Mounts"]
            .as_array()
            .ok_or("container mounts unavailable")?;
        // Check the effective mounts before the only start. In particular, an
        // implicit host secret mount must never be visible to reviewed code.
        if container["Image"] != self.image.trim_start_matches("sha256:")
            || mounts.len() != 1
            || mounts[0]["Source"].as_str() != input.to_str()
            || mounts[0]["Destination"] != "/inputs"
            || mounts[0]["RW"] != false
        {
            return Err("unexpected image or mount in reproduction container".into());
        }
        Ok(())
    }

    /// The only start. A missing response remains subject to read-only observation.
    pub fn start(&self, key: &str) -> Result<(), String> {
        start_if_rootless(
            run(podman().args(["info", "--format", "{{.Host.Security.Rootless}}"])),
            || {
                let _ = run(podman().args(["start", key]));
            },
        )
    }

    /// Inspect only the original isolated run. Absence or ambiguous status
    /// never authorizes a new start, even after coordinator restart.
    pub fn observe(&self, key: &str) -> Option<Value> {
        let container = Self::inspect(key)?;
        let state = &container["State"];
        if container["Image"] != self.image.trim_start_matches("sha256:")
            || state["Status"] != "exited"
            || state["StartedAt"].as_str()?.starts_with("0001-")
            || state["FinishedAt"].as_str()?.starts_with("0001-")
        {
            return None;
        }
        let code = state["ExitCode"].as_i64()?;
        let out = run(podman().args(["logs", key])).ok()?;
        let encode = |bytes: &[u8]| hex::encode(&bytes[..bytes.len().min(LOG_LIMIT)]);
        Some(json!({
            "state": if state["OOMKilled"] == true { "runner_failed" } else { "finished" },
            "runner_image_digest": self.image,
            "exit_code": code,
            "finished_at": state["FinishedAt"],
            "stdout": encode(&out.stdout),
            "stderr": encode(&out.stderr),
        }))
    }

    pub fn cleanup(&self, key: &str) {
        let _ = run(podman().args(["rm", key]));
        let _ = fs::remove_dir_all(self.state.join(key));
    }
}

pub fn sha256(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;

    #[test]
    fn starts_only_after_confirmed_rootless_probe() {
        let report = |stdout: &[u8]| {
            Ok(Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: stdout.to_vec(),
                stderr: vec![],
            })
        };
        let starts = std::cell::Cell::new(0);
        for probe in [report(b"false\n"), report(b""), Err("probe failed".into())] {
            assert!(start_if_rootless(probe, || starts.set(starts.get() + 1)).is_err());
        }
        assert_eq!(starts.get(), 0);
        assert!(start_if_rootless(report(b"true\n"), || starts.set(starts.get() + 1)).is_ok());
        assert_eq!(starts.get(), 1);
    }
}
