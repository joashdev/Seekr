# 13 - Implement Stats Command and Local Health Reporting

**Status:** [ ]

**Type:** feat

**Summary:** Implement `seekr stats` so users can understand their local database, indexing status, and privacy configuration without inspecting SQLite directly.

**Dependencies:** 03, 05, 09, 10

**Deliverables:**
- `seekr stats` output showing database path, total stored commands, indexed commands, collapsed duplicate groups, failed command count, and earliest/latest timestamps.
- Privacy status output showing whether redaction is enabled and how many ignore rules are configured.
- Basic local health checks for missing database, migration status, and FTS availability.
- Tests for stats calculations against a temp database.

**Acceptance Criteria:**
- [ ] `seekr stats` prints useful local-only health information.
- [ ] Stats include total command count and indexed command count.
- [ ] Stats include duplicate/collapsed group information after task 10.
- [ ] Stats include failed command count and history time range when data exists.
- [ ] Stats report redaction enabled/disabled without printing secret values.
- [ ] Missing or empty databases are handled gracefully.

**Verification Commands:**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo run -- stats
```

**Notes:** Do not add telemetry or success-metric reporting. This command is only for local visibility.
