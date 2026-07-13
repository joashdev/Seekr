# 11 - Build Interactive TUI Search Flow

**Status:** [x]

**Type:** feat

**Summary:** Make `seekr` with no subcommand launch the MVP interactive TUI. The TUI should support fast fuzzy recall with live filtering, keyboard navigation, and a command preview.

**Dependencies:** 05, 07, 10

**Deliverables:**
- A ratatui-based TUI launched by `seekr` with no subcommand.
- Search input with live filtering against existing command history.
- Keyboard navigation through results.
- Command preview showing raw command and metadata such as cwd, timestamp, exit code, repo, and branch.
- Empty, loading, and error states that keep the terminal usable.
- TUI tests for state transitions and rendering where practical.

**Acceptance Criteria:**
- [x] Running `seekr` opens the TUI instead of printing a placeholder.
- [x] Typing in the TUI updates results without restarting the app.
- [x] Results can be navigated from the keyboard.
- [x] The selected command preview includes raw command text and useful metadata.
- [x] Exiting the TUI restores the terminal state.
- [x] Automated tests cover TUI state logic, and manual verification covers terminal rendering.

**Verification Commands:**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo run -- --help
```

**Manual Verification Checklist:**

1. Import or capture at least three commands.
2. Run `cargo run --` to launch the TUI.
3. Type a search term and confirm results update live.
4. Move the selection up and down.
5. Confirm the preview shows command text and metadata.
6. Quit and confirm the terminal prompt is restored cleanly.

**Notes:** Keep visual design functional and restrained. This is a terminal productivity tool, not a marketing surface.
