# Seekr

Seekr is a local-only CLI and TUI for capturing, indexing, and recalling terminal commands. Commands and configuration stay on this machine; Seekr has no cloud dependency.

It is designed as a smarter `Ctrl-R` for developers, operators, and power users who want command history with richer context than their shell usually keeps: working directory, repository, branch, timestamp, and exit status.

## Setup

Build Seekr and put the binary on your `PATH`:

```bash
cargo build --release
mkdir -p "$HOME/.local/bin"
cp target/release/seekr "$HOME/.local/bin/seekr"
```

Add the generated hook for your shell to its startup file:

```bash
# zsh
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
"$HOME/.local/bin/seekr" init zsh >> ~/.zshrc
source ~/.zshrc

# bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
"$HOME/.local/bin/seekr" init bash >> ~/.bashrc
source ~/.bashrc
```

The zsh hook installs both `Ctrl-R` and an `sk` function. `sk` consumes Seekr's
selection protocol and puts an inserted command onto zsh's editable command
line. Do not replace it with `alias sk=seekr`, which cannot stage the selected
command in the parent shell. The bash hook currently exposes recall through
`Ctrl-R`.

Import existing zsh history, search from the CLI, or launch the TUI:

```bash
seekr import ~/.zsh_history
seekr search docker
sk
```

The shell hook captures future commands locally. Privacy redaction is off by default so stored commands remain runnable. To opt in, create the config file shown by your platform's Seekr config directory with:

```toml
[privacy]
redaction_enabled = true
ignore_commands = ["ls", "cd", "pwd", "clear"]
```

## MVP Scope

The MVP includes:

- Rust CLI and TUI application.
- Local SQLite storage through `rusqlite`.
- SQLite FTS5-backed command search.
- zsh-first shell capture with minimal bash support.
- Import from `.zsh_history`.
- Contextual search filters such as current directory, repository, time window, and failed commands.
- Reuse actions for copying or staging commands back into the shell prompt.
- Local privacy controls, ignore rules, and optional redaction.

## Command Surface

```bash
seekr
seekr search docker
seekr here
seekr failed
seekr import ~/.zsh_history
seekr stats
```

## Product Principles

- Local-only storage and operation.
- No cloud dependency.
- No AI or natural-language command generation.
- Raw commands are stored for fast, runnable recall unless optional redaction is enabled.
- Repeated commands should be collapsed to reduce noise while preserving useful metadata.

## Implementation Plan

The implementation backlog lives in [`tasks/backlog.md`](tasks/backlog.md). Each numbered file in [`tasks/`](tasks/) is written to be executed independently with the `task-runner` skill.

## Smoke Testing

Run the automated smoke coverage from the repository root:

```bash
./scripts/smoke.sh
```

The script creates temporary config and data directories and removes them when it exits. It covers history import, capture, search, contextual and failed filters, redaction, stats, and generation of both shell hooks without touching real Seekr data.

For the interactive behavior, use safe commands and check:

1. Launch `sk`; type a known query and verify the result list and preview update.
2. Press `Ctrl+Y` and verify the raw selected command reaches the clipboard without leaving the TUI.
3. Press `Enter` and verify the command is inserted/staged at the prompt and remains editable rather than executing.
4. Press `Ctrl+E` and verify the visibly labeled explicit rerun action executes the selected safe command.

## Releasing

CI runs formatting, Clippy, tests, and the smoke script on pushes and pull requests to `main`.

Only a repository administrator can create a release tag. To release, update the version in `Cargo.toml`, merge it to `main`, then push the matching tag:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The release workflow builds a Linux x86_64 archive, waits for approval from the protected `release` environment, and publishes the archive with signed SLSA provenance. Published releases and their tags are immutable.

Verify a downloaded release and its assets with the GitHub CLI:

```bash
gh release verify v0.1.0 --repo joashdev/Seekr
gh release verify-asset v0.1.0 seekr-v0.1.0-x86_64-unknown-linux-gnu.tar.gz --repo joashdev/Seekr
```

## License

Seekr is released under the MIT License. See [`LICENSE`](LICENSE) for details.
