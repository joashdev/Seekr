# 07 - Add Contextual Filters and First-Class Shortcuts

**Status:** [ ]

**Type:** feat

**Summary:** Add the contextual search behavior described in the PRD, including `seekr here`, `seekr failed`, and explicit metadata filters for project-aware recall.

**Dependencies:** 05

**Deliverables:**
- Search filter options for cwd, repo, branch, exit status, and time window.
- `seekr here` implemented as a first-class shortcut scoped to the current directory or detected git repository.
- `seekr failed` implemented as a first-class shortcut for non-zero exit codes.
- CLI flags for contextual filtering on `seekr search`, such as cwd, repo, branch, failed/successful, since, and before.
- Git repository and branch detection helpers that work outside a git repo without failing.
- Tests covering filter combinations and shortcut behavior.

**Acceptance Criteria:**
- [ ] `seekr here` prioritizes or filters to commands from the current cwd or current git repo.
- [ ] `seekr failed` returns only commands with non-zero exit codes.
- [ ] `seekr search <query>` can be filtered by cwd, repo, branch, exit status, and time window.
- [ ] Filters use indexed SQLite metadata fields where applicable.
- [ ] Running outside a git repo produces useful results instead of an error.
- [ ] Tests cover successful, failed, cwd, repo, branch, and time-window filtering.

**Verification Commands:**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo run -- here
cargo run -- failed
cargo run -- search docker --failed
```

**Notes:** If exact flag names differ, update CLI help and tests so the supported filter surface is obvious to users and future task runners.
