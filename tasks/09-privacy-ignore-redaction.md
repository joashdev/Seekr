# 09 - Add Privacy Controls, Ignore Rules, and Redaction

**Status:** [ ]

**Type:** feat

**Summary:** Add local privacy controls for command ingestion. Redaction must remain off by default, while configurable ignore rules and optional redaction can be enabled for privacy-sensitive setups.

**Dependencies:** 04, 06, 08

**Deliverables:**
- Config fields for enabled redaction, ignore patterns, and noisy command suppression.
- Default noisy command suppression candidates for `ls`, `cd`, `pwd`, and `clear`.
- Ignore matching applied consistently to shell capture and zsh history import.
- Optional redaction applied consistently to shell capture and zsh history import when enabled.
- Redaction patterns for obvious high-risk values such as tokens, passwords, API keys, and bearer secrets.
- A committed fixture at `tests/fixtures/zsh_history_with_secrets.sample` for privacy verification.
- Tests proving defaults preserve raw runnable commands, while enabled redaction masks obvious secrets.

**Acceptance Criteria:**
- [ ] Redaction is off by default.
- [ ] Raw commands remain runnable when redaction is disabled.
- [ ] Ignore rules can prevent noisy or user-configured commands from being stored.
- [ ] Optional redaction masks obvious secret patterns before persistence.
- [ ] Capture and import use the same privacy filtering path.
- [ ] Tests cover noisy suppression, custom ignore patterns, redaction disabled, and redaction enabled.

**Verification Commands:**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
SEEKR_CONFIG_DIR=/tmp/seekr-task09-config SEEKR_DATA_DIR=/tmp/seekr-task09-data cargo run -- import tests/fixtures/zsh_history_with_secrets.sample
SEEKR_CONFIG_DIR=/tmp/seekr-task09-config SEEKR_DATA_DIR=/tmp/seekr-task09-data cargo run -- search password
```

**Manual Verification Checklist:**

1. Enable redaction in a temporary Seekr config using test data paths.
2. Capture or import a command containing `password=secret-value`.
3. Confirm search results do not display the raw secret.
4. Disable redaction and confirm newly captured non-secret commands remain raw and reusable.

**Notes:** Keep all behavior local. Do not add remote secret scanning, telemetry, or cloud lookups.
