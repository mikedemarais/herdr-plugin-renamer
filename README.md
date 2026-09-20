# herdr-plugin-renamer

A narrow [Herdr](https://herdr.dev) plugin that publishes a stable, human-readable
`$task` title for Claude Code, Codex, and Pi sessions.

For each native agent session it reads the first prompt, asks the authenticated
Codex CLI (`gpt-5.6-luna`, low reasoning) for a concise 3–6 word title, and
reports only the `task` metadata token. It does **not** rename panes, displayed
agents, tabs, workspaces, or Git branches.

## Install

```sh
herdr plugin install mikedemarais/herdr-plugin-renamer --yes
herdr integration install claude
herdr integration install codex
herdr integration install pi
```

Configure the Agents sidebar:

```toml
[ui.sidebar.agents]
rows = [
  ["state_icon", "$task"],
  ["agent", "workspace"],
]
```

Requirements: Herdr 0.7.4+, Rust at install time, and an authenticated `codex`
CLI on `PATH` with access to `gpt-5.6-luna`.

## Behavior

- Titles are generated once from the first prompt and cached per native session.
- Cached titles are re-reported after Herdr restarts.
- A failed Codex call publishes a readable local fallback but does not mark the
  session complete, so a later `working` transition retries Codex.
- Codex runs with `--ignore-user-config`, `--ephemeral`, and a read-only sandbox.
- Debug logging is off by default. Set `HERDR_NAMING_DEBUG=true` to enable a
  bounded 256 KiB diagnostic log without prompt excerpts.
- Legacy `.slug` caches are read and migrated to `.title` files.

## Development

```sh
cargo fmt --check
cargo test
cargo build --release
```

License: MIT.
