# Seekr

Seekr is a local-first CLI and TUI for capturing, indexing, and recalling terminal commands.

It is designed as a smarter `Ctrl-R` for developers, operators, and power users who want command history with richer context than their shell usually keeps: working directory, repository, branch, timestamp, and exit status.

## MVP Scope

Seekr is currently in planning/bootstrap form. The MVP is scoped around:

- Rust CLI and TUI application.
- Local SQLite storage through `rusqlite`.
- SQLite FTS5-backed command search.
- zsh-first shell capture, with minimal bash support planned.
- Import from `.zsh_history`.
- Contextual search filters such as current directory, repository, time window, and failed commands.
- Reuse actions for copying or staging commands back into the shell prompt.
- Local privacy controls, ignore rules, and optional redaction.

## Planned Command Surface

```bash
seekr
seekr search docker
seekr here
seekr failed
seekr import ~/.zsh_history
seekr stats
```

The intended short alias is:

```bash
sk
```

## Product Principles

- Local-first by default.
- No cloud dependency in the MVP.
- No AI or natural-language command generation.
- Raw commands are stored for fast, runnable recall unless optional redaction is enabled.
- Repeated commands should be collapsed to reduce noise while preserving useful metadata.

## Implementation Plan

The implementation backlog lives in [`tasks/backlog.md`](tasks/backlog.md). Each numbered file in [`tasks/`](tasks/) is written to be executed independently with the `task-runner` skill.

## License

Seekr is released under the MIT License. See [`LICENSE`](LICENSE) for details.
