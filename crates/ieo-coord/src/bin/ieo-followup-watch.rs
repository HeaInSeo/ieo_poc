//! Separate-lifetime periodic recovery client. It holds a repo-scoped watcher
//! token rather than a coordinator DB credential.
use serde_json::json;
use std::{path::Path, time::Duration};

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let (args, once) = args
        .strip_suffix(&["--once".to_owned()])
        .map_or((args.as_slice(), false), |args| (args, true));
    let [url, token_file, repo, interval] = args else {
        return Err(
            "usage: ieo-followup-watch COORDINATOR_URL TOKEN_FILE REPO_ID SECONDS [--once]".into(),
        );
    };
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("coordinator URL must use HTTP or HTTPS".into());
    }
    let seconds = interval
        .parse::<u64>()
        .ok()
        .filter(|seconds| *seconds > 0)
        .ok_or("interval must be positive seconds")?;
    let url = format!(
        "{}/v0/coordinator/followups/reconcile",
        url.trim_end_matches('/')
    );
    let client = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout(Duration::from_secs(15))
        .build();
    let mut last_error = None;
    loop {
        let result = poll(&client, &url, Path::new(token_file), repo);
        if once {
            return result;
        }
        let error = result.err();
        if error != last_error {
            match &error {
                Some(error) => eprintln!("followup watcher unavailable: {error}"),
                None => eprintln!("followup watcher recovered"),
            }
            last_error = error;
        }
        std::thread::sleep(Duration::from_secs(seconds));
    }
}

fn poll(client: &ureq::Agent, url: &str, file: &Path, repo: &str) -> Result<(), String> {
    let raw =
        std::fs::read_to_string(file).map_err(|e| format!("credential file: {}", e.kind()))?;
    let token = raw.trim();
    if token.is_empty() || !token.bytes().all(|b| b.is_ascii_graphic()) {
        return Err("invalid watcher credential".into());
    }
    match client
        .post(url)
        .set("Authorization", &format!("Bearer {token}"))
        .send_json(json!({"repo_id":repo}))
    {
        Ok(response) if response.status() == 200 => Ok(()),
        Ok(response) => Err(format!(
            "unexpected coordinator status {}",
            response.status()
        )),
        Err(ureq::Error::Status(status, _)) => Err(format!("coordinator returned {status}")),
        Err(ureq::Error::Transport(error)) => {
            Err(format!("coordinator transport: {}", error.kind()))
        }
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(2);
    }
}
