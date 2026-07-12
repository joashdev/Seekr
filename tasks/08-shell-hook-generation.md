# 08 - Add Shell Hook Generation for Zsh and Bash

**Status:** [x]

**Type:** feat

**Summary:** Implement shell setup output that lets users wire Seekr into zsh and bash command lifecycle hooks. The generated hooks should call the capture entrypoint from task 04 and collect the PRD's required metadata.

**Dependencies:** 04

**Deliverables:**
- A command such as `seekr init zsh` that prints zsh integration code.
- A command such as `seekr init bash` that prints bash integration code.
- Hook snippets that capture raw command text, cwd, timestamp, exit code, shell type, duration, hostname, git repo, and git branch when available.
- Documentation in CLI help or comments explaining how to add the generated hook to shell startup files.
- Tests for generated hook output containing the expected capture invocation and metadata fields.

**Acceptance Criteria:**
- [x] zsh hook output uses appropriate zsh lifecycle hooks, such as `preexec` and `precmd`, to capture command and exit status.
- [x] bash hook output uses bash-compatible mechanisms such as `DEBUG` trap and `PROMPT_COMMAND` without requiring a full shell replacement.
- [x] Generated hooks call the Seekr capture entrypoint with command, cwd, timestamp, and exit code.
- [x] Generated hooks include shell, duration, hostname, git repo, and git branch when available.
- [x] Hook generation does not write to shell startup files automatically.
- [x] Tests verify zsh and bash hook output includes required fields.

**Verification Commands:**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo run -- init zsh
cargo run -- init bash
```

**Manual Verification Checklist:**

1. Run `cargo run -- init zsh` and confirm the output is copy-pasteable into a zsh session.
2. Run a simple command such as `echo seekr-smoke` in that instrumented session.
3. Confirm `seekr search seekr-smoke` can find the captured command.

**Notes:** The PRD allows launch support to start narrower, but functional requirements mention both zsh and bash. Keep bash support minimal and explicit if zsh receives the stronger initial path.
