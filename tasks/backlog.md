# Seekr MVP Backlog

| Task ID | Title | Type | Depends On | Status | Summary |
|---|---|---|---|---|---|
| 01 | Bootstrap Rust CLI crate and command surface | feat | None | [x] | Create the Rust project skeleton, install the MVP dependencies, and expose the documented `seekr` command surface with safe placeholder behavior. |
| 02 | Add config and local storage foundation | feat | 01 | [x] | Establish local-only config/data paths, load default config, and expose path inspection through the CLI. |
| 03 | Create SQLite schema and migration runner | feat | 02 | [x] | Add the SQLite database schema, FTS5 table, migration runner, and tests proving a fresh database initializes correctly. |
| 04 | Implement command persistence and capture entrypoint | feat | 03 | [x] | Persist command records through a store layer and add a CLI capture command suitable for shell hooks. |
| 05 | Implement FTS-backed CLI search | feat | 04 | [x] | Make `seekr search <query>` return ranked command results from SQLite FTS with useful metadata. |
| 06 | Import `.zsh_history` files | feat | 04 | [x] | Parse zsh history formats, import commands into SQLite, and report import counts without requiring shell hooks. |
| 07 | Add contextual filters and first-class shortcuts | feat | 05 | [x] | Support `here`, `failed`, and metadata filters for cwd, repo, branch, exit status, and time windows. |
| 08 | Add shell hook generation for zsh and bash | feat | 04 | [x] | Generate shell setup snippets that capture commands, cwd, timestamp, exit code, shell, duration, hostname, git repo, and branch. |
| 09 | Add privacy controls, ignore rules, and redaction | feat | 04, 06, 08 | [ ] | Apply configurable ignore patterns, noisy command suppression, and optional redaction to capture and import paths. |
| 10 | Collapse duplicate commands in search results | feat | 05, 07 | [x] | Collapse repeated commands while preserving recency, count, status, and contextual metadata for ranking and display. |
| 11 | Build interactive TUI search flow | feat | 05, 07, 10 | [ ] | Make `seekr` launch a responsive ratatui interface with fuzzy search, live results, navigation, and command preview. |
| 12 | Implement copy, insert, and rerun reuse actions | feat | 08, 11 | [ ] | Add safe reuse actions for copying, inserting into the prompt for review, and explicitly rerunning selected commands. |
| 13 | Implement stats command and local health reporting | feat | 03, 05, 09, 10 | [x] | Implement `seekr stats` with local database, indexing, privacy, and usage health information. |
| 14 | Add end-to-end smoke coverage and setup documentation | feat | 01, 06, 08, 09, 11, 12, 13 | [ ] | Add scripted smoke coverage and concise setup documentation for the MVP workflows. |
