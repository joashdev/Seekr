# 03 - Create SQLite Schema and Migration Runner

**Status:** [ ]

**Type:** feat

**Summary:** Add SQLite storage initialization for captured commands and search indexing. This task should create a durable schema with FTS5 support and enough metadata columns for the PRD's MVP filters.

**Dependencies:** 02

**Deliverables:**
- A database module using `rusqlite`.
- A migration runner that creates or upgrades the local SQLite database.
- A `commands` table storing raw command text, normalized search text, cwd, timestamp, exit code, shell, duration, hostname, git repo, and git branch.
- A SQLite FTS5 table linked to command text for full-text search.
- Indexes for metadata filters such as cwd, repo, branch, timestamp, and exit code.
- Tests that initialize a fresh database and verify the expected tables, indexes, and FTS5 support.

**Acceptance Criteria:**
- [ ] Opening the database path from task 02 creates parent directories when needed.
- [ ] Running migrations multiple times is safe and idempotent.
- [ ] The schema stores raw command text separately from normalized text used for search.
- [ ] FTS5 insertion and query can run in a test database.
- [ ] Metadata indexes exist for cwd, repo, branch, timestamp, and exit code filtering.
- [ ] No network access or external services are required.

**Verification Commands:**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo run -- stats
```

**Notes:** Prefer bundled SQLite features if needed so FTS5 is available consistently in local development and CI.
