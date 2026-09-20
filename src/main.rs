//! Generate one stable, human-readable `$task` title from an agent's first prompt.
//! The plugin never renames panes, agents, branches, workspaces, or tabs.

mod codex;
mod context;
mod herdr;
mod slug;
mod transcript;

use std::env;
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CLAIM_TTL: Duration = Duration::from_secs(120);
const SESSION_POLL_ATTEMPTS: u32 = 12;
const SESSION_POLL_DELAY: Duration = Duration::from_millis(500);
const PROMPT_POLL_ATTEMPTS: u32 = 20;
const PROMPT_POLL_DELAY: Duration = Duration::from_millis(750);
const MAX_DEBUG_LOG_BYTES: u64 = 256 * 1024;

fn main() {
    if env::args().nth(1).as_deref() == Some("--replay") {
        replay_cached_titles();
    } else if env::var("HERDR_NAMING_PHASE").as_deref() == Ok("cold") {
        cold_phase();
    } else {
        hot_phase();
    }
}

fn replay_cached_titles() {
    let state_dir = state_dir();
    for session in herdr::active_sessions() {
        let key = marker_key_for_session(&session.agent, &session.id);
        let title_cache = state_dir.join(format!("{key}.title"));
        let legacy_cache = state_dir.join(format!("{key}.slug"));
        if let Some(title) = read_cached_title(&title_cache, &legacy_cache) {
            let _ = herdr::report_task(&session.pane_id, &title);
        }
    }
}

fn hot_phase() {
    let event_json = match env::var("HERDR_PLUGIN_EVENT_JSON") {
        Ok(value) => value,
        Err(_) => return,
    };
    let eligible = match context::evaluate(&event_json) {
        Some(eligible) => eligible,
        None => return,
    };
    let marker_key = marker_key_for_pane(&eligible.pane_id);
    let state_dir = state_dir();
    migrate_legacy_marker_state(&state_dir);
    let claim = state_dir.join(format!("{marker_key}.claim"));
    if claim_is_fresh(&claim) {
        return;
    }
    let _ = std::fs::create_dir_all(&state_dir);
    let _ = std::fs::write(&claim, now_secs().to_string());
    spawn_cold_phase(&eligible.pane_id, &marker_key);
}

fn cold_phase() {
    let pane_id = env::var("HN_PANE_ID").unwrap_or_default();
    let marker_key = env::var("HN_MARKER_KEY").unwrap_or_else(|_| marker_key_for_pane(&pane_id));
    let state_dir = state_dir();
    let claim = state_dir.join(format!("{marker_key}.claim"));

    let (agent, session_id) =
        match herdr::poll_agent_session(&pane_id, SESSION_POLL_ATTEMPTS, SESSION_POLL_DELAY) {
            Some(session) => session,
            None => {
                debug_log(&state_dir, "no agent session after polling");
                let _ = std::fs::remove_file(&claim);
                return;
            }
        };
    let session_key = marker_key_for_session(&agent, &session_id);
    let done = state_dir.join(format!("{session_key}.done"));
    let title_cache = state_dir.join(format!("{session_key}.title"));
    let legacy_cache = state_dir.join(format!("{session_key}.slug"));

    if done.exists() {
        if let Some(title) = read_cached_title(&title_cache, &legacy_cache) {
            let _ = herdr::report_task(&pane_id, &title);
            let _ = std::fs::remove_file(&claim);
            return;
        }
        // A completion marker without its cache cannot restore metadata.
        let _ = std::fs::remove_file(&done);
    }

    let prompt = match poll_first_prompt(&agent, &session_id) {
        Some(prompt) => prompt,
        None => {
            debug_log(&state_dir, "no first prompt after polling");
            let _ = std::fs::remove_file(&claim);
            return;
        }
    };

    let candidate = state_dir.join(format!("{session_key}.candidate"));
    let generated = codex::generate_title(&prompt, &candidate);
    let _ = std::fs::remove_file(&candidate);
    let (title, generated_by_model) = match generated {
        Some(title) => (title, true),
        None => {
            debug_log(
                &state_dir,
                "Codex naming failed; publishing retryable fallback",
            );
            (slug::fallback_title_from_prompt(&prompt), false)
        }
    };

    let _ = std::fs::create_dir_all(&state_dir);
    let _ = std::fs::write(&title_cache, format!("{title}\n"));
    let _ = herdr::report_task(&pane_id, &title);
    if generated_by_model {
        let _ = std::fs::write(&done, now_secs().to_string());
    }
    let _ = std::fs::remove_file(&claim);
}

fn read_cached_title(title_cache: &Path, legacy_cache: &Path) -> Option<String> {
    let (raw, migrated) = match std::fs::read_to_string(title_cache) {
        Ok(raw) => (raw, false),
        Err(_) => (std::fs::read_to_string(legacy_cache).ok()?, true),
    };
    let title = slug::sanitize_title(&raw);
    if title.is_empty() {
        return None;
    }
    if migrated {
        let _ = std::fs::write(title_cache, format!("{title}\n"));
    }
    Some(title)
}

fn poll_first_prompt(agent: &str, session_id: &str) -> Option<String> {
    for attempt in 0..PROMPT_POLL_ATTEMPTS {
        if let Some(prompt) = transcript::read_first_prompt(agent, session_id) {
            return Some(prompt);
        }
        if attempt + 1 < PROMPT_POLL_ATTEMPTS {
            std::thread::sleep(PROMPT_POLL_DELAY);
        }
    }
    None
}

fn spawn_cold_phase(pane_id: &str, marker_key: &str) {
    let exe = match env::current_exe() {
        Ok(exe) => exe,
        Err(_) => return,
    };
    let mut command = Command::new(exe);
    command
        .env("HERDR_NAMING_PHASE", "cold")
        .env("HN_PANE_ID", pane_id)
        .env("HN_MARKER_KEY", marker_key)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // SAFETY: setsid only detaches the child into a new process session.
    unsafe {
        command.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    let _ = command.spawn();
}

fn state_dir() -> PathBuf {
    env::var("HERDR_PLUGIN_STATE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp/herdr-plugin-renamer"))
}

fn marker_key_for_pane(pane_id: &str) -> String {
    let safe = pane_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("claim-pane-{safe}")
}

fn marker_key_for_session(agent: &str, session_id: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in agent
        .bytes()
        .chain(std::iter::once(0))
        .chain(session_id.bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let safe_agent = agent
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("session-{safe_agent}-{hash:016x}")
}

fn migrate_legacy_marker_state(state_dir: &Path) {
    let sentinel = state_dir.join("state-v2.migrated");
    if sentinel.exists() {
        return;
    }
    let _ = std::fs::create_dir_all(state_dir);
    if let Ok(entries) = std::fs::read_dir(state_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("pane-")
                && (name.ends_with(".claim") || name.ends_with(".done") || name.ends_with(".slug"))
            {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    let _ = std::fs::write(sentinel, "2");
}

fn claim_is_fresh(marker: &Path) -> bool {
    std::fs::metadata(marker)
        .and_then(|metadata| metadata.modified())
        .and_then(|modified| modified.elapsed().map_err(std::io::Error::other))
        .map(|age| age < CLAIM_TTL)
        .unwrap_or(false)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn debug_log(state_dir: &Path, message: &str) {
    if env::var("HERDR_NAMING_DEBUG").as_deref() != Ok("true") {
        return;
    }
    let _ = std::fs::create_dir_all(state_dir);
    let path = state_dir.join("debug.log");
    if std::fs::metadata(&path)
        .map(|metadata| metadata.len() >= MAX_DEBUG_LOG_BYTES)
        .unwrap_or(false)
    {
        let _ = std::fs::write(&path, "");
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(
            file,
            "{} [pid {}] {message}",
            now_secs(),
            std::process::id()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_keys_are_stable_and_distinct() {
        assert_eq!(marker_key_for_pane("w5V:p1"), "claim-pane-w5V_p1");
        assert_eq!(
            marker_key_for_session("pi", "/tmp/first"),
            marker_key_for_session("pi", "/tmp/first")
        );
        assert_ne!(
            marker_key_for_session("pi", "/tmp/first"),
            marker_key_for_session("pi", "/tmp/second")
        );
    }

    #[test]
    fn cached_legacy_title_is_migrated() {
        let dir = std::env::temp_dir().join(format!("herdr-title-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let title = dir.join("session.title");
        let legacy = dir.join("session.slug");
        std::fs::write(&legacy, "Review Hermes Routing\n").unwrap();
        assert_eq!(
            read_cached_title(&title, &legacy).as_deref(),
            Some("Review Hermes Routing")
        );
        assert!(title.exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
