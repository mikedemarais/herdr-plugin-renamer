//! Human-readable task-title cleanup and deterministic fallback generation.

const MAX_WORDS: usize = 6;
const MAX_LEN: usize = 60;

/// Preserve spaces and capitalization while removing surrounding punctuation,
/// excess words, control characters, and excess length.
pub fn sanitize_title(raw: &str) -> String {
    let words = raw
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("")
        .split_whitespace()
        .map(|word| word.trim_matches(|ch: char| !ch.is_alphanumeric()))
        .filter(|word| !word.is_empty())
        .take(MAX_WORDS)
        .collect::<Vec<_>>();
    let title = words.join(" ");
    if title.chars().count() <= MAX_LEN {
        title
    } else {
        format!(
            "{}…",
            title
                .chars()
                .take(MAX_LEN - 1)
                .collect::<String>()
                .trim_end()
        )
    }
}

/// Derive a readable emergency fallback from the first prompt line.
pub fn fallback_title_from_prompt(prompt: &str) -> String {
    let first_line = prompt
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("");
    let title = sanitize_title(first_line);
    if title.is_empty() {
        "Agent task".to_string()
    } else {
        title
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_human_title_spaces() {
        assert_eq!(
            sanitize_title("Review Hermes Routing Research."),
            "Review Hermes Routing Research"
        );
    }

    #[test]
    fn caps_words_and_length() {
        assert_eq!(
            sanitize_title("one two three four five six seven"),
            "one two three four five six"
        );
        assert!(sanitize_title(&"a".repeat(80)).ends_with('…'));
    }

    #[test]
    fn fallback_never_empty() {
        assert_eq!(fallback_title_from_prompt("!!!"), "Agent task");
    }
}
