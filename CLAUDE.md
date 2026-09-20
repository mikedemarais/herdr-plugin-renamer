# Herdr Plugin Renamer

A deliberately narrow Herdr plugin: generate one human-readable `$task` title
from an agent session's first prompt. Do not broaden it into pane, tab, workspace,
or Git branch renaming.

## Architecture

- `main.rs`: event admission, detached cold phase, session-scoped claims/cache,
  cached-title replay, retryable fallback, and bounded opt-in diagnostics.
- `codex.rs`: one bounded authenticated `codex exec` call using
  `gpt-5.6-luna`, low reasoning, read-only and ephemeral execution.
- `transcript.rs`: first-prompt extraction for Claude Code, Codex, and Pi.
- `slug.rs`: despite the retained upstream filename, human-title sanitization and
  deterministic fallback generation.
- `herdr.rs`: resolve the native agent session and publish only `$task` metadata.
- `context.rs`: admit only `pane.agent_status_changed` events entering `working`.

## Invariants

- Never rename panes, displayed agents, tabs, workspaces, or Git branches.
- Never mark a fallback title done; a later working transition must retry Codex.
- A done session must re-report its cached title so Herdr restarts do not lose it.
- Preserve existing state. Read legacy `.slug` caches and migrate them to `.title`.
- Keep debug logging off by default, bounded, and free of prompt excerpts.
- Fail open: plugin failures must never block Herdr.

## Validation

```sh
cargo fmt --check
cargo test
cargo build --release
```
