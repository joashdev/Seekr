#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
tmp=$(mktemp -d "${TMPDIR:-/tmp}/seekr-smoke.XXXXXX")
trap 'rm -rf "$tmp"' EXIT INT TERM

export SEEKR_CONFIG_DIR="$tmp/config"
export SEEKR_DATA_DIR="$tmp/data"

seekr() {
  cargo run --quiet --manifest-path "$root/Cargo.toml" -- "$@"
}

contains() {
  case "$1" in
    *"$2"*) ;;
    *) printf 'expected output to contain: %s\n' "$2" >&2; exit 1 ;;
  esac
}

not_contains() {
  case "$1" in
    *"$2"*) printf 'expected output not to contain: %s\n' "$2" >&2; exit 1 ;;
    *) ;;
  esac
}

import_output=$(seekr import "$root/tests/fixtures/zsh_history.sample")
contains "$import_output" "Import complete:"
contains "$(seekr search cargo)" "cargo test"

seekr capture \
  --command-text "docker compose up" \
  --cwd "$tmp/project" \
  --executed-at 1800000000 \
  --exit-code 1 \
  --shell zsh \
  --git-repo "$tmp/project" \
  --git-branch smoke

seekr capture --command-text "docker wrong cwd" --cwd "$tmp/other" --executed-at 1800000001 --exit-code 1 --shell zsh --git-repo "$tmp/project" --git-branch smoke
seekr capture --command-text "docker wrong repo" --cwd "$tmp/project" --executed-at 1800000002 --exit-code 1 --shell zsh --git-repo "$tmp/other" --git-branch smoke
seekr capture --command-text "docker wrong branch" --cwd "$tmp/project" --executed-at 1800000003 --exit-code 1 --shell zsh --git-repo "$tmp/project" --git-branch other
seekr capture --command-text "docker succeeded" --cwd "$tmp/project" --executed-at 1800000004 --exit-code 0 --shell zsh --git-repo "$tmp/project" --git-branch smoke

filtered=$(seekr search docker --cwd "$tmp/project" --repo "$tmp/project" --branch smoke --failed)
contains "$filtered" "docker compose up"
not_contains "$filtered" "docker wrong cwd"
not_contains "$filtered" "docker wrong repo"
not_contains "$filtered" "docker wrong branch"
not_contains "$filtered" "docker succeeded"

failed=$(seekr failed)
contains "$failed" "docker compose up"
not_contains "$failed" "docker succeeded"

mkdir -p "$SEEKR_CONFIG_DIR"
cat >"$SEEKR_CONFIG_DIR/config.toml" <<'EOF'
[privacy]
redaction_enabled = true
ignore_commands = ["ls", "cd", "pwd", "clear"]
EOF

seekr capture \
  --command-text "deploy password=smoke-secret" \
  --cwd "$tmp/project" \
  --executed-at 1800000005 \
  --exit-code 0 \
  --shell zsh

redacted=$(seekr search deploy)
contains "$redacted" "password=<REDACTED>"
case "$redacted" in
  *smoke-secret*) printf 'redaction exposed the fixture secret\n' >&2; exit 1 ;;
esac

stats=$(seekr stats)
contains "$stats" "Database: ok"
contains "$stats" "FTS index: ok"
contains "$stats" "Redaction: enabled"

zsh_hook=$(seekr init zsh)
bash_hook=$(seekr init bash)
contains "$zsh_hook" "action == rerun"
contains "$bash_hook" "action == rerun"

printf 'Seekr smoke test passed using isolated data in %s\n' "$tmp"
