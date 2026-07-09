# 04 - Implement Command Persistence and Capture Entrypoint

**Status:** [x]

**Type:** feat

**Summary:** Implement the core command record store and a CLI capture entrypoint that shell hooks can call after command execution. This is the first vertical slice from command input to local persistence.

**Dependencies:** 03

**Deliverables:**
- A `CommandRecord` domain type with raw command, cwd, timestamp, exit code, shell, duration, hostname, git repo, and git branch fields.
- Store methods for inserting command records and fetching recent records for tests and diagnostics.
- A CLI capture subcommand or internal command that accepts command metadata from shell hooks.
- Normalization of command text for FTS indexing while preserving the raw command for display and reuse.
- Input validation that rejects empty commands and malformed timestamps or exit codes.
- Tests for successful capture, rejected capture, and persisted metadata.

**Acceptance Criteria:**
- [x] A valid capture invocation writes exactly one command record to SQLite.
- [x] Empty commands are ignored or rejected without creating records.
- [x] Raw command text is preserved exactly for later display/execution.
- [x] Normalized command text is populated for search.
- [x] Capture can run with temp config/data directories in tests.
- [x] Existing CLI, config, and database tests continue to pass.

**Verification Commands:**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo run -- stats
```

**Notes:** It is acceptable for the capture command to be hidden from normal help if that keeps the public CLI focused.
