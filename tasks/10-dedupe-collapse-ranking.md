# 10 - Collapse Duplicate Commands in Search Results

**Status:** [x]

**Type:** feat

**Summary:** Collapse repeated commands in search results so noisy repeated history does not bury useful recall. The collapsed result should preserve enough metadata to remain useful for context-aware search.

**Dependencies:** 05, 07

**Deliverables:**
- Duplicate detection based on normalized command text and relevant context.
- Collapsed search result models including repeat count, most recent timestamp, most recent cwd, exit status summary, repo, and branch when available.
- Search output that displays collapsed results clearly.
- Ranking that balances FTS relevance, recency, contextual closeness, and duplicate count without adding AI behavior.
- Tests covering duplicate insertion, collapsed display data, and ranking behavior.

**Acceptance Criteria:**
- [x] Identical repeated commands appear as a single collapsed search result by default.
- [x] Collapsed results expose repeat count and most recent execution metadata.
- [x] Failed and successful runs remain distinguishable enough for `seekr failed`.
- [x] Context filters still work correctly with collapsed results.
- [x] The raw command selected for reuse remains exact and runnable.
- [x] Tests cover collapsed search with and without contextual filters.

**Verification Commands:**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo run -- search git
cargo run -- failed
```

**Notes:** This is MVP dedupe/collapse, not the broader v0.2 ranking overhaul mentioned in the PRD.
