# 01 - Bootstrap Rust CLI Crate and Command Surface

**Status:** [x]

**Type:** feat

**Summary:** Create the initial Rust CLI application for Seekr, including the binary crate, MVP dependencies, and the documented command surface with safe placeholder behavior. This task should make the repository buildable and give later tasks a stable place to add behavior.

**Dependencies:** None

**Deliverables:**
- A Rust binary crate in the repository root with `Cargo.toml`, `Cargo.lock`, and `src/`.
- A `seekr` binary implemented in Rust.
- CLI parsing for `seekr`, `seekr search <query>`, `seekr here`, `seekr failed`, `seekr import <path>`, and `seekr stats`.
- A planned `sk` alias documented in CLI help or setup notes without requiring package installation yet.
- Dependencies added for the PRD stack: `clap`, `rusqlite` with FTS-capable SQLite support, `ratatui`, and a terminal backend such as `crossterm`.
- Placeholder command handlers that return clear "not implemented yet" messages and successful build/test results.

**Acceptance Criteria:**
- [x] `cargo build` succeeds from the repository root.
- [x] `cargo run -- --help` lists the MVP command surface from the PRD.
- [x] `cargo run -- search docker` parses `docker` as the search query.
- [x] `cargo run -- here`, `cargo run -- failed`, `cargo run -- import ~/.zsh_history`, and `cargo run -- stats` all invoke distinct handlers.
- [x] The default `cargo run --` path is reserved for the future TUI and does not perform network access.
- [x] Unit or integration tests cover CLI parsing for every MVP command.

**Verification Commands:**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo run -- --help
cargo run -- search docker
cargo run -- here
cargo run -- failed
cargo run -- import ~/.zsh_history
cargo run -- stats
```

**Notes:** Keep placeholder behavior explicit and small. Later tasks should replace handlers without needing to rewrite the CLI parser.
