# Seekr PRD

## Overview

Seekr is a local-first CLI and TUI for capturing, indexing, and recalling terminal commands. Its core promise is simple: make past commands easy to find and reuse, with richer context than shell history and a smoother search experience than `Ctrl-R`.

Positioning:

> Seekr - smarter Ctrl-R for your terminal.

Primary command surface:

```bash
seekr
seekr search docker
seekr here
seekr failed
seekr import ~/.zsh_history
seekr stats
```

Short alias:

```bash
sk
```

## Problem

Terminal users routinely lose useful commands or waste time reconstructing them. Native shell history is inconsistent across environments, difficult to search, and often stripped of important context like working directory, success or failure, and project association.

Common failure modes:

- History limits or retention behavior cause older commands to disappear.
- Multiple terminals and sessions overwrite or fragment history.
- Built-in reverse search is fast for exact recall, but weak for contextual recall.
- Users cannot easily answer questions like "what was the command I used last week to port-forward Postgres in the billing repo?"
- Sensitive commands create trust concerns for any tooling that stores command history.

## Goals

- Persist terminal command history beyond shell-native limitations.
- Make commands easy to find by keyword, project, directory, timeframe, and outcome.
- Provide a fast default TUI experience for fuzzy recall.
- Keep the product local-first and privacy-respecting.
- Reduce repeated command reconstruction for developers, operators, and power users.
- Differentiate from generic history tools through project-aware search and better recall ergonomics.
- Keep the product strictly non-AI and focused on deterministic local recall.

## Non-Goals

- Cloud sync in the initial release.
- Team sharing or collaboration features in the MVP.
- Full shell replacement or terminal emulation.
- IDE plugin support in the MVP.
- Natural-language command generation or recall.
- Capturing every shell and platform at launch; initial support can start with zsh.

## Target Users

- Software engineers who frequently jump across repos, environments, and infra tasks.
- DevOps and platform engineers who rely on long, hard-to-remember operational commands.
- Power users who live in the terminal and want stronger recall than shell history provides.
- Privacy-conscious users who do not want command history sent to a hosted service.

## User Stories

- As a developer, I want to search for a command by keyword so I can reuse it without reconstructing it.
- As a developer, I want to search within the current repo or directory so results stay relevant to my task.
- As an operator, I want to filter to failed commands so I can debug what went wrong.
- As a user, I want to launch an interactive TUI and fuzzy-search history quickly from the keyboard.
- As a privacy-conscious user, I want optional redaction controls so I can decide whether speed or secrecy matters more for my setup.
- As a new user, I want to import existing shell history so Seekr is immediately useful.
- As a repeat user, I want to copy or rerun a selected command directly from search results.

## Functional Requirements

### 1. Command capture

- Seekr must capture executed commands from shell hooks.
- Initial shell integration must support zsh and bash.
- Captured records must include:
  - raw command text
  - current working directory
  - timestamp
  - exit code
- The system should additionally capture useful context when available:
  - shell type
  - duration
  - hostname
  - git repository
  - git branch

### 2. Local storage and indexing

- Seekr must store command records locally on the user's machine.
- The MVP storage engine should be SQLite.
- Search indexing should use SQLite FTS5 or equivalent full-text search capability.
- Storage should support efficient filtering by metadata fields such as cwd, repo, branch, and exit status.

### 3. Search and recall

- `seekr` with no subcommand must open an interactive TUI.
- The TUI must support fuzzy search with live filtering and command preview.
- Users must be able to search by free text from the CLI, for example `seekr search docker`.
- Users must be able to filter results by context, including:
  - current directory or project
  - failed vs successful commands
  - time window
  - repository
- The product should support contextual queries such as "here" and "failed" as first-class commands.

### 4. Reuse actions

- Users must be able to copy a selected command to the clipboard.
- Users must be able to rerun a selected command.
- The UI should make it clear whether an action copies or stages the command for execution.
- Selected commands should be insertable back into the user's terminal prompt for review, editing, and manual execution.

### 5. Import

- Seekr must support importing existing shell history files.
- Initial import targets should include `.zsh_history`.
- The design should leave room for `.bash_history` import shortly after launch.

### 6. Privacy and filtering

- Seekr must be local-first with no cloud dependency in the MVP.
- Redaction should be off by default to preserve fast, runnable recall.
- The system should offer optional redaction as a user-toggle for privacy-sensitive setups.
- The system must support configurable ignore rules for commands and patterns.
- Users must be able to configure noisy command suppression, such as `ls`, `cd`, `pwd`, and `clear`.
- The system should collapse repeated commands to reduce noise, while preserving enough metadata to keep results useful.
- The product must remain fully local-only, including suggestions and ranking behavior.

## Non-Functional Requirements

- Fast startup and responsive search on large local histories.
- Works reliably without network access.
- Minimal installation friction and a simple shell setup flow.
- Single-user local storage by default.
- Secure handling of redaction and local persistence.
- Cross-platform architecture should be considered, even if launch support is narrower.

## Success Metrics

### MVP product metrics

- At least 80% of captured commands are successfully indexed and searchable in normal use.
- Median time to retrieve a previously used command is under 5 seconds in the TUI.
- At least 60% of early users report finding a command faster than with shell history alone.
- Optional redaction behavior catches high-risk obvious secret patterns when enabled in test coverage and smoke tests.

### Adoption and engagement signals

- Weekly active usage of `seekr` or `sk` after setup.
- Repeated usage of search, copy, and rerun actions.
- Import completion rate for new users.
- Low uninstall or disable rate after initial onboarding.

## Risks

- Competitive overlap with Atuin and other shell history tools may weaken differentiation.
- Storing raw commands by default improves reuse speed but increases the chance that sensitive data is retained locally.
- Shell integration can be fragile across shell configurations and plugin ecosystems.
- Reuse flows can introduce user trust and safety concerns if execution behavior is too easy or unclear.
- Capturing too much low-value command noise can make search quality feel worse, not better.

## Rollout

### v0.1 MVP

- zsh command capture
- local SQLite storage
- FTS-backed search
- interactive TUI
- copy selected command
- insert selected command back into the prompt for review/edit/Enter-to-run
- ignore and redact config
- import from `.zsh_history`

### v0.2

- better deduping and ranking
- stronger repo-aware filters
- improved onboarding and shell init helpers

### Later exploration

- command suggestions scoped from tight to broad, starting with current directory, then repo, then broader local context
- timeline views per project
- optional sync or backup

## Product Decisions

- Seekr is not AI-driven and will not include natural-language recall.
- The product remains local-only.
- Initial shell support targets zsh and bash.
- Commands should be stored raw for display/execution and normalized separately for search.
- Redaction is off by default and available as an optional toggle.
- Repeated commands should be collapsed rather than shown as many duplicate results.
- The product name remains Seekr, with `sk` as the default alias.
- After selecting a command, the preferred reuse flow is to place it back into the terminal prompt so the user can inspect, edit, and press Enter manually.
- Command suggestions should be scoped from tight to broad: current directory first, then repo, then broader local context.

## Recommended Technical Direction

- Language: Rust
- Local database: SQLite via `rusqlite`
- Search: SQLite FTS5
- TUI: `ratatui`

This stack matches the product's needs for performance, low overhead, portability, and a strong single-binary CLI experience.
