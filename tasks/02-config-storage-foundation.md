# 02 - Add Config and Local Storage Foundation

**Status:** [x]

**Type:** feat

**Summary:** Establish Seekr's local-only config and data path conventions, default configuration values, and path inspection behavior. This unlocks SQLite persistence, privacy settings, and shell integration without hardcoding machine-specific paths.

**Dependencies:** 01

**Deliverables:**
- A config module that resolves Seekr config and data directories using platform-appropriate local paths.
- Environment variable overrides for tests and development, such as `SEEKR_CONFIG_DIR` and `SEEKR_DATA_DIR`.
- A default config model with redaction disabled by default, ignore rules for noisy commands, and space for future shell preferences.
- Config loading that tolerates a missing config file by using defaults.
- Config persistence support for writing an initial config file when requested by later tasks.
- A CLI-accessible way to inspect resolved paths, such as a `seekr stats` placeholder section or an internal command hidden from normal help.

**Acceptance Criteria:**
- [x] Config and data paths are resolved without network access.
- [x] Missing config files fall back to defaults.
- [x] Test overrides can isolate all filesystem writes under a temp directory.
- [x] Default noisy command ignore candidates include `ls`, `cd`, `pwd`, and `clear` but are only applied by later ingestion tasks.
- [x] Tests cover default config loading, environment overrides, and config file parsing.
- [x] Existing CLI parsing tests from task 01 still pass.

**Verification Commands:**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo run -- stats
```

**Notes:** Do not introduce cloud, sync, account, or team settings. The PRD's MVP is strictly single-user and local-first.
