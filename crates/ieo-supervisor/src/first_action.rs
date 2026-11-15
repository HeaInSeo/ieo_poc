//! The first actual action of a run (02-05 step 4, PLAN §3.2 step 4; runtime contract revision
//! 12 §14 item 15).
//!
//! **Definition.** The first `assistant` event of the session's own stream (no
//! `parent_tool_use_id`) **after** the `system`/`init` event that carries a `tool_use` content
//! block or a non-empty `text` block. `kind` is `tool_use` when the event has any `tool_use`
//! block, else `text`. Events before `init`, the `init` event itself, `thinking`-only or empty
//! assistant events, `user`/`result`/`rate_limit_event` lines and child (sub-agent) events are not
//! actions.
//!
//! * `event_offset` is the 0-based line index of that event in the run's `stdout.jsonl` (every
//!   line the guard copies there is counted, in order).
//! * `observed_at` is the supervisor's clock when the guard read the line: the stream has no
//!   event times. It is an observation time, never the start time of the run.
//! * A run with no such event has **no first action** (`"none"` in its `after_run` checkpoint):
//!   never offset 0 or the start time.

use serde_json::Value;
use std::sync::Mutex;

/// One observed first action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub event_offset: u64,
    pub observed_at: String,
    pub kind: &'static str,
}

/// Whether one stream event is an action, and of which kind (`None` for anything else). Only
/// meaningful after `init` (see [`Probe::feed`]).
pub fn action_kind(v: &Value) -> Option<&'static str> {
    if v["type"] != "assistant" || !v["parent_tool_use_id"].is_null() {
        return None;
    }
    let blocks = v["message"]["content"].as_array()?;
    if blocks.iter().any(|b| b["type"] == "tool_use") {
        return Some("tool_use");
    }
    let text = blocks
        .iter()
        .any(|b| b["type"] == "text" && b["text"].as_str().is_some_and(|t| !t.trim().is_empty()));
    text.then_some("text")
}

fn is_init(v: &Value) -> bool {
    v["type"] == "system" && v["subtype"] == "init"
}

/// The first action of a whole stream (offset, kind), for a stream read after the run.
pub fn scan(text: &str) -> Option<(u64, &'static str)> {
    let mut seen_init = false;
    for (i, line) in text.lines().enumerate() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if !seen_init {
            seen_init = is_init(&v);
            continue;
        }
        if let Some(k) = action_kind(&v) {
            return Some((i as u64, k));
        }
    }
    None
}

type Notify = Box<dyn FnOnce(&Found) + Send>;

#[derive(Default)]
struct State {
    lines: u64,
    seen_init: bool,
    found: Option<Found>,
}

/// Fed every line the guard writes to `stdout.jsonl`, in order; records the first action with
/// the time it was read and calls `notify` once (outside the guard's own work: the callback
/// must not block).
pub struct Probe {
    state: Mutex<State>,
    notify: Mutex<Option<Notify>>,
}

impl std::fmt::Debug for Probe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Probe")
            .field("found", &self.found())
            .finish()
    }
}

impl Probe {
    pub fn new(notify: Option<Notify>) -> Probe {
        Probe {
            state: Mutex::new(State::default()),
            notify: Mutex::new(notify),
        }
    }

    /// One line, as written to `stdout.jsonl`.
    pub fn feed(&self, line: &str) {
        let hit = {
            let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            let offset = st.lines;
            st.lines += 1;
            if st.found.is_some() {
                return;
            }
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                return;
            };
            if !st.seen_init {
                st.seen_init = is_init(&v);
                return;
            }
            let Some(kind) = action_kind(&v) else {
                return;
            };
            let f = Found {
                event_offset: offset,
                observed_at: ieo_evidence::time::now(),
                kind,
            };
            st.found = Some(f.clone());
            f
        };
        let cb = self.notify.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(cb) = cb {
            cb(&hit);
        }
    }

    pub fn found(&self) -> Option<Found> {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .found
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INIT: &str = r#"{"type":"system","subtype":"init","session_id":"s"}"#;
    const EMPTY: &str = r#"{"type":"assistant","message":{"id":"m","usage":{}}}"#;
    const THINK: &str =
        r#"{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"hm"}]}}"#;
    const BLANK: &str =
        r#"{"type":"assistant","message":{"content":[{"type":"text","text":"  "}]}}"#;
    const TEXT: &str =
        r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hello"}]}}"#;
    const TOOL: &str = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"x"},{"type":"tool_use","id":"t","name":"Bash","input":{}}]}}"#;
    const CHILD: &str = r#"{"type":"assistant","parent_tool_use_id":"t","message":{"content":[{"type":"text","text":"child"}]}}"#;
    const RL: &str = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed"}}"#;

    fn feed(lines: &[&str]) -> Option<(u64, &'static str)> {
        let p = Probe::new(None);
        for l in lines {
            p.feed(l);
        }
        let live = p.found().map(|f| (f.event_offset, f.kind));
        // the live probe and a scan of the same stream agree
        assert_eq!(live, scan(&lines.join("\n")), "{lines:?}");
        live
    }

    #[test]
    fn the_first_action_is_the_first_tool_use_or_text_after_init() {
        assert_eq!(feed(&[INIT, TEXT]), Some((1, "text")));
        assert_eq!(feed(&[INIT, TOOL]), Some((1, "tool_use")));
        assert_eq!(
            feed(&[INIT, EMPTY, RL, THINK, BLANK, CHILD, TOOL, TEXT]),
            Some((6, "tool_use"))
        );
        // before init nothing counts; init itself is not an action
        assert_eq!(feed(&[TEXT, TOOL, INIT, EMPTY, TEXT]), Some((4, "text")));
        assert_eq!(feed(&[TEXT, TOOL]), None);
        assert_eq!(feed(&[INIT]), None);
        // no action: none (never offset 0)
        assert_eq!(feed(&[INIT, EMPTY, RL, THINK]), None);
        assert_eq!(feed(&[]), None);
        // a line that is not JSON still counts as a line of the stream
        assert_eq!(feed(&[INIT, "not json", TEXT]), Some((2, "text")));
    }

    #[test]
    fn the_callback_runs_once_with_the_observation() {
        let seen = std::sync::Arc::new(Mutex::new(Vec::new()));
        let s2 = std::sync::Arc::clone(&seen);
        let p = Probe::new(Some(Box::new(move |f: &Found| {
            s2.lock().expect("l").push(f.clone())
        })));
        for l in [INIT, EMPTY, TEXT, TOOL] {
            p.feed(l);
        }
        let got = seen.lock().expect("l").clone();
        assert_eq!(got.len(), 1);
        assert_eq!((got[0].event_offset, got[0].kind), (2, "text"));
        assert!(got[0].observed_at.ends_with('Z'));
        assert_eq!(p.found(), Some(got[0].clone()));
    }
}
