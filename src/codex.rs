//! A bounded headless Codex call that turns the first prompt into a concise title.

use std::env;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(30);
const PROMPT_LIMIT: usize = 2000;

/// Run Codex without user config, hooks, writes, or session persistence.
pub fn generate_title(prompt: &str, output_file: &Path) -> Option<String> {
    let bin = env::var("HERDR_NAMING_CODEX_BIN").unwrap_or_else(|_| "codex".to_string());
    let truncated: String = prompt.chars().take(PROMPT_LIMIT).collect();
    let full_prompt = format!(
        "Output only a concise 3-6 word human-readable chat title in title case \
         (spaces, not kebab-case), with no quotes, ending punctuation, or prose, \
         summarizing this coding task:\n\n{truncated}"
    );

    let _ = std::fs::remove_file(output_file);
    let mut child = Command::new(bin)
        .args([
            "exec",
            "--skip-git-repo-check",
            "--ignore-user-config",
            "--ephemeral",
            "-s",
            "read-only",
            "-m",
            "gpt-5.6-luna",
            "-c",
            "model_reasoning_effort=low",
            "-o",
        ])
        .arg(output_file)
        .arg(&full_prompt)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    if !wait_with_timeout(&mut child, TIMEOUT) {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    }

    let raw = std::fs::read_to_string(output_file).ok()?;
    let title = crate::slug::sanitize_title(&raw);
    (!title.is_empty()).then_some(title)
}

fn wait_with_timeout(child: &mut Child, timeout: Duration) -> bool {
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if start.elapsed() >= timeout => return false,
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(_) => return false,
        }
    }
}
