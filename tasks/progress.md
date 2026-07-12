# Seekr MVP Progress

- 2026-07-12 - Task 07: Added indexed contextual search filters plus `here` and `failed` shortcuts with safe git context detection.
- 2026-07-12 - Task 08: Added zsh and bash shell-hook generation that captures command metadata through `seekr capture`.
- 2026-07-11 - Task 06: Added `.zsh_history` import for extended, plain, and backslash-continued commands with SQLite persistence and import counts.
- 2026-07-11 - Task 05: Added ranked FTS-backed CLI search with result limits and local command metadata output.
- 2026-07-09 - Task 04: Added a hidden capture entrypoint plus SQLite insert/recent helpers that preserve raw commands and populate normalized search text.
- 2026-07-09 - Task 03: Added SQLite initialization with idempotent migrations, metadata indexes, and FTS-backed command search schema coverage.
- 2026-07-08 - Task 01: Bootstrapped the Rust CLI crate with the MVP command surface, placeholder handlers, and parsing tests.
- 2026-07-08 - Task 02: Added local config/data path resolution, default config load-save support, and `seekr stats` path inspection.
