# 12 - Implement Copy, Insert, and Rerun Reuse Actions

**Status:** [ ]

**Type:** feat

**Summary:** Add reuse actions for selected commands. The preferred path should stage a command back into the user's prompt for review and editing, while copy and explicit rerun actions remain clearly distinct.

**Dependencies:** 08, 11

**Deliverables:**
- TUI action to copy the selected raw command to the clipboard.
- TUI action to print or return the selected command for shell prompt insertion.
- Shell integration support that can insert a selected command into the active prompt for user review.
- Explicit rerun action that executes only when the user intentionally chooses rerun.
- Clear TUI labels or status messages distinguishing copy, insert/stage, and rerun.
- Tests for action selection and command output behavior, with manual checks for clipboard and prompt insertion.

**Acceptance Criteria:**
- [ ] Copy action places the selected raw command on the clipboard when clipboard support is available.
- [ ] Insert action stages the selected raw command back into the shell prompt without immediately executing it.
- [ ] Rerun action is explicit and visually distinct from insert/stage.
- [ ] Reuse actions operate on raw command text, not redacted or normalized search text unless redaction was applied before persistence.
- [ ] The TUI makes the chosen action clear before returning control to the shell.
- [ ] Tests cover action routing and shell-facing output.

**Verification Commands:**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo run -- --help
cargo run -- init zsh
```

**Manual Verification Checklist:**

1. Launch the TUI with at least one searchable command.
2. Select a command and copy it, then paste somewhere safe to verify clipboard contents.
3. Select a command and insert/stage it into the shell prompt.
4. Confirm the inserted command is editable before execution.
5. Trigger rerun only with the explicit rerun action and confirm the UI communicates that execution will happen.

**Notes:** If clipboard support requires an additional crate, keep it cross-platform where practical and gracefully degrade when no clipboard provider is available.
