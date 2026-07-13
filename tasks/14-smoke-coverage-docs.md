# 14 - Add End-to-End Smoke Coverage and Setup Documentation

**Status:** [x]

**Type:** feat

**Summary:** Add durable MVP smoke coverage and concise setup documentation so a future executor can verify the critical Seekr flows end to end. This task should validate the product loop without expanding scope beyond the PRD.

**Dependencies:** 01, 06, 08, 09, 11, 12, 13

**Deliverables:**
- A scripted smoke test or documented test fixture that uses isolated temp config/data directories.
- Smoke coverage for import, capture, search, contextual filters, privacy redaction, stats, and non-interactive reuse output where possible.
- README setup instructions for building Seekr, generating shell hooks, importing `.zsh_history`, launching the TUI, and using the `sk` alias.
- Documentation that clearly states Seekr is local-only and redaction is off by default.
- A short manual smoke checklist for TUI rendering, prompt insertion, clipboard behavior, and explicit rerun.

**Acceptance Criteria:**
- [x] Automated smoke coverage can run without touching the user's real Seekr data.
- [x] Smoke coverage exercises import, capture, search, filters, redaction, and stats.
- [x] README setup instructions include zsh and bash hook generation.
- [x] README explains the intended `sk` alias.
- [x] README states local-only behavior and no cloud dependency.
- [x] Manual smoke checklist covers TUI, copy, insert/stage, and explicit rerun behavior.

**Verification Commands:**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
./scripts/smoke.sh
SEEKR_CONFIG_DIR=/tmp/seekr-task14-config SEEKR_DATA_DIR=/tmp/seekr-task14-data cargo run -- stats
SEEKR_CONFIG_DIR=/tmp/seekr-task14-config SEEKR_DATA_DIR=/tmp/seekr-task14-data cargo run -- init zsh
SEEKR_CONFIG_DIR=/tmp/seekr-task14-config SEEKR_DATA_DIR=/tmp/seekr-task14-data cargo run -- init bash
```

**Manual Verification Checklist:**

1. Follow the README setup steps in a temporary shell session.
2. Import a small zsh history fixture.
3. Search for a known command from the fixture.
4. Launch the TUI and verify live search plus preview.
5. Verify copy, insert/stage, and explicit rerun behavior using safe commands only.

**Notes:** Keep documentation focused on MVP usage. Do not add cloud sync, collaboration, IDE integration, AI recall, or broader roadmap instructions.
