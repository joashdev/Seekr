# 06 - Import `.zsh_history` Files

**Status:** [ ]

**Type:** feat

**Summary:** Implement `seekr import <path>` for existing `.zsh_history` files so a new user can populate Seekr without waiting for shell hooks to collect history. The importer should handle common zsh history formats and report useful counts.

**Dependencies:** 04

**Deliverables:**
- A zsh history parser for extended history lines such as `: 1700000000:0;git status`.
- Support for plain one-command-per-line history entries when extended metadata is absent.
- Import logic that writes parsed commands through the same persistence path as capture.
- Import summary output including inserted, skipped, and failed counts.
- A committed fixture at `tests/fixtures/zsh_history.sample` for verification and regression tests.
- Tests with fixture content covering extended history, plain history, multiline commands when feasible, and malformed lines.

**Acceptance Criteria:**
- [ ] `seekr import <path>` imports valid zsh history records into SQLite.
- [ ] Extended zsh timestamps and durations are preserved when present.
- [ ] Plain history lines import with reasonable fallback metadata.
- [ ] Malformed lines do not abort the entire import.
- [ ] Import output reports inserted, skipped, and failed counts.
- [ ] The implementation leaves room for `.bash_history` import later without adding bash support in this task.

**Verification Commands:**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
SEEKR_CONFIG_DIR=/tmp/seekr-task06-config SEEKR_DATA_DIR=/tmp/seekr-task06-data cargo run -- import tests/fixtures/zsh_history.sample
SEEKR_CONFIG_DIR=/tmp/seekr-task06-config SEEKR_DATA_DIR=/tmp/seekr-task06-data cargo run -- search git
```

**Notes:** Do not apply redaction or ignore rules here unless task 09 has already been completed in the branch this task is running on. Task 09 will wire privacy behavior through import and capture.
