//! Minimal callbacks into Herdr: resolve a pane's native session and publish `$task`.

use std::env;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

const METADATA_SOURCE: &str = "plugin:herdr-plugin-renamer";

fn herdr_bin() -> String {
    env::var("HERDR_BIN_PATH").unwrap_or_else(|_| "herdr".to_string())
}

fn pane_agent_session(pane_id: &str) -> Option<(String, String)> {
    let output = Command::new(herdr_bin())
        .args(["pane", "get", pane_id])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let session = value
        .pointer("/result/pane/agent_session")
        .or_else(|| value.pointer("/pane/agent_session"))
        .or_else(|| value.get("agent_session"))?;
    let agent = session
        .get("agent")
        .and_then(|value| value.as_str())
        .map(str::to_owned)
        .or_else(|| {
            session
                .get("source")
                .and_then(|value| value.as_str())
                .map(|source| source.trim_start_matches("herdr:").to_owned())
        })?;
    let id = session.get("value")?.as_str()?.to_owned();
    Some((agent, id))
}

pub fn poll_agent_session(
    pane_id: &str,
    attempts: u32,
    delay: Duration,
) -> Option<(String, String)> {
    for attempt in 0..attempts {
        if let Some(session) = pane_agent_session(pane_id) {
            return Some(session);
        }
        if attempt + 1 < attempts {
            sleep(delay);
        }
    }
    None
}

/// Publish only the custom `$task` token. Pane titles and agent labels stay untouched.
pub fn report_task(pane_id: &str, task: &str) -> bool {
    Command::new(herdr_bin())
        .args([
            "pane",
            "report-metadata",
            pane_id,
            "--source",
            METADATA_SOURCE,
            "--token",
            &format!("task={task}"),
        ])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}
