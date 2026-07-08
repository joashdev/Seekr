# 05 - Implement FTS-Backed CLI Search

**Status:** [ ]

**Type:** feat

**Summary:** Replace the placeholder `seekr search <query>` behavior with SQLite FTS-backed command search. Results should be useful in plain CLI mode before the TUI exists.

**Dependencies:** 04

**Deliverables:**
- Search query APIs over the SQLite FTS table.
- `seekr search <query>` output showing command text plus useful metadata such as cwd, timestamp, exit code, repo, and branch when available.
- Basic ranking using FTS relevance and recency.
- A result limit with a sensible default and a CLI option to override it.
- Tests covering indexing, searching, empty-result behavior, and result ordering.

**Acceptance Criteria:**
- [ ] Captured commands are searchable by keyword through `seekr search`.
- [ ] Search returns raw command text, not normalized-only text.
- [ ] Empty results print a clear local-only message and exit successfully.
- [ ] Search result limits prevent unbounded output.
- [ ] Tests prove FTS matches inserted command records.
- [ ] No TUI code is required for this task.

**Verification Commands:**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo run -- search docker
```

**Notes:** Keep the CLI output stable enough for smoke tests, but do not over-design a long-term export format unless needed.
