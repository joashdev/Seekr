pub mod config;
pub mod db;
pub mod privacy;
pub mod stats;
pub mod tui;

use clap::{Args, Parser, Subcommand, ValueEnum};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::time::{SystemTime, UNIX_EPOCH};

const CLI_ABOUT: &str =
    "Seekr is a local-first CLI and future TUI for recalling terminal commands.";
const CLI_AFTER_HELP: &str =
    "The generated zsh integration installs `sk` and Ctrl-R command recall.";

pub(crate) fn format_relative_timestamp(executed_at: i64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(i64::MAX, |duration| {
            i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
        });
    format_relative_timestamp_at(executed_at, now)
}

fn format_relative_timestamp_at(executed_at: i64, now: i64) -> String {
    let seconds = executed_at.abs_diff(now);
    if seconds < 60 {
        return "just now".to_string();
    }

    let (value, unit) = if seconds < 60 * 60 {
        (seconds / 60, "m")
    } else if seconds < 24 * 60 * 60 {
        (seconds / (60 * 60), "h")
    } else if seconds < 7 * 24 * 60 * 60 {
        (seconds / (24 * 60 * 60), "d")
    } else {
        (seconds / (7 * 24 * 60 * 60), "w")
    };

    if executed_at > now {
        format!("in {value}{unit}")
    } else {
        format!("{value}{unit} ago")
    }
}

#[derive(Debug, Parser, PartialEq, Eq)]
#[command(
    name = "seekr",
    version,
    about = CLI_ABOUT,
    long_about = None,
    after_help = CLI_AFTER_HELP
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum Command {
    /// Search for previously captured commands.
    Search(SearchArgs),
    /// Limit results to the current directory or repository context.
    Here,
    /// Limit results to failed commands.
    Failed,
    /// Import shell history from a local file.
    Import {
        /// Path to a shell history file, such as ~/.zsh_history.
        path: PathBuf,
    },
    /// Print shell hook code; add it to the matching shell startup file or eval its output.
    Init(InitArgs),
    #[command(hide = true)]
    Capture(CaptureArgs),
    /// Show local usage and index health statistics.
    Stats,
}

#[derive(Debug, Args, PartialEq, Eq)]
pub struct SearchArgs {
    /// Free-text query to look up.
    pub query: String,
    /// Maximum number of matching commands to show.
    #[arg(
        short,
        long,
        default_value_t = 10,
        value_parser = clap::builder::RangedU64ValueParser::<usize>::new().range(1..=100)
    )]
    pub limit: usize,
    /// Restrict results to this working directory.
    #[arg(long)]
    pub cwd: Option<String>,
    /// Restrict results to this git repository.
    #[arg(long)]
    pub repo: Option<String>,
    /// Restrict results to this git branch.
    #[arg(long)]
    pub branch: Option<String>,
    /// Restrict results to non-zero exit codes.
    #[arg(long, conflicts_with = "successful")]
    pub failed: bool,
    /// Restrict results to successful commands.
    #[arg(long, conflicts_with = "failed")]
    pub successful: bool,
    /// Restrict results to commands captured at or after this Unix timestamp.
    #[arg(long)]
    pub since: Option<i64>,
    /// Restrict results to commands captured at or before this Unix timestamp.
    #[arg(long)]
    pub before: Option<i64>,
}

impl Default for SearchArgs {
    fn default() -> Self {
        Self {
            query: String::new(),
            limit: 10,
            cwd: None,
            repo: None,
            branch: None,
            failed: false,
            successful: false,
            since: None,
            before: None,
        }
    }
}

impl SearchArgs {
    fn filters(&self) -> db::SearchFilters {
        db::SearchFilters {
            cwd: self.cwd.clone(),
            repo: self.repo.clone(),
            branch: self.branch.clone(),
            failed: self
                .failed
                .then_some(true)
                .or(self.successful.then_some(false)),
            since: self.since,
            before: self.before,
        }
    }
}

#[derive(Debug, Args, PartialEq, Eq)]
pub struct InitArgs {
    /// Shell to integrate.
    pub shell: HookShell,
}

#[derive(Debug, Clone, ValueEnum, PartialEq, Eq)]
pub enum HookShell {
    Zsh,
    Bash,
}

#[derive(Debug, Args, PartialEq, Eq)]
pub struct CaptureArgs {
    #[arg(long = "command-text")]
    pub command_text: String,
    #[arg(long)]
    pub cwd: String,
    #[arg(long = "executed-at")]
    pub executed_at: i64,
    #[arg(long = "exit-code")]
    pub exit_code: i64,
    #[arg(long)]
    pub shell: Option<String>,
    #[arg(long = "duration-ms")]
    pub duration_ms: Option<i64>,
    #[arg(long)]
    pub hostname: Option<String>,
    #[arg(long = "git-repo")]
    pub git_repo: Option<String>,
    #[arg(long = "git-branch")]
    pub git_branch: Option<String>,
}

pub fn dispatch(cli: Cli) -> io::Result<String> {
    match cli.command {
        None => tui::run(),
        Some(Command::Search(args)) => search(args),
        Some(Command::Here) => here(),
        Some(Command::Failed) => failed(),
        Some(Command::Import { path }) => import(path),
        Some(Command::Init(args)) => Ok(shell_hook(args.shell).to_string()),
        Some(Command::Capture(args)) => capture(args),
        Some(Command::Stats) => stats::stats_report(),
    }
}

fn shell_hook(shell: HookShell) -> &'static str {
    match shell {
        HookShell::Zsh => ZSH_HOOK,
        HookShell::Bash => BASH_HOOK,
    }
}

const ZSH_HOOK: &str = r#"# Add this to ~/.zshrc, or run: eval "$(seekr init zsh)"
autoload -Uz add-zsh-hook
zmodload zsh/datetime

typeset -g SEEKR_COMMAND=""
typeset -g SEEKR_STARTED_AT_MS=0
typeset -g SEEKR_EXECUTED_AT=0
typeset -g SEEKR_CWD=""
typeset -g SEEKR_HOSTNAME=""
typeset -g SEEKR_GIT_REPO=""
typeset -g SEEKR_GIT_BRANCH=""

_seekr_select() {
  local result action command_text
  result=$(command seekr) || return
  [[ $result == *$'\n'* ]] || return
  action=${result%%$'\n'*}
  command_text=${result#*$'\n'}
  reply=("$action" "$command_text")
}

_seekr_widget() {
  local -a reply
  local action command_text
  _seekr_select || return
  action=${reply[1]}
  command_text=${reply[2]}
  BUFFER=$command_text
  CURSOR=${#BUFFER}
  [[ $action == rerun ]] && zle accept-line
}

zle -N seekr-history _seekr_widget
bindkey '^R' seekr-history

unalias sk 2>/dev/null
sk() {
  SEEKR_COMMAND=""
  if (( $# )); then
    command seekr "$@"
    return
  fi
  local -a reply
  local action command_text
  _seekr_select || return
  action=${reply[1]}
  command_text=${reply[2]}
  if [[ $action == rerun ]]; then
    _seekr_preexec "$command_text"
    builtin eval -- "$command_text"
  else
    print -rz -- "$command_text"
  fi
}

_seekr_preexec() {
  SEEKR_COMMAND=$1
  SEEKR_CWD=$PWD
  SEEKR_HOSTNAME=$(hostname)
  SEEKR_GIT_REPO=$(git rev-parse --show-toplevel 2>/dev/null) || SEEKR_GIT_REPO=""
  SEEKR_GIT_BRANCH=$(git symbolic-ref --quiet --short HEAD 2>/dev/null) || SEEKR_GIT_BRANCH=""
  printf -v SEEKR_STARTED_AT_MS '%.0f' "$(( EPOCHREALTIME * 1000 ))"
  SEEKR_EXECUTED_AT=$(( SEEKR_STARTED_AT_MS / 1000 ))
}

_seekr_precmd() {
  local exit_code=$?
  local finished_at_ms duration_ms
  local -a args

  [[ -n $SEEKR_COMMAND ]] || return
  printf -v finished_at_ms '%.0f' "$(( EPOCHREALTIME * 1000 ))"
  duration_ms=$(( finished_at_ms - SEEKR_STARTED_AT_MS ))
  args=(
    --command-text "$SEEKR_COMMAND"
    --cwd "$SEEKR_CWD"
    --executed-at "$SEEKR_EXECUTED_AT"
    --exit-code "$exit_code"
    --shell zsh
    --duration-ms "$duration_ms"
    --hostname "$SEEKR_HOSTNAME"
  )
  [[ -n $SEEKR_GIT_REPO ]] && args+=(--git-repo "$SEEKR_GIT_REPO")
  [[ -n $SEEKR_GIT_BRANCH ]] && args+=(--git-branch "$SEEKR_GIT_BRANCH")
  seekr capture "${args[@]}" >/dev/null 2>&1
  SEEKR_COMMAND=""
}

add-zsh-hook preexec _seekr_preexec
add-zsh-hook precmd _seekr_precmd
"#;

const BASH_HOOK: &str = r#"# Add this to ~/.bashrc, or run: eval "$(seekr init bash)"
# Bash exposes submitted compound commands through history, not DEBUG. Commands
# excluded by HISTCONTROL/HISTIGNORE fall back to their first simple command.
# Seekr must own the DEBUG trap to snapshot command-start metadata. If another
# integration also needs DEBUG, source Seekr first and let that integration chain it.
SEEKR_COMMAND=""
SEEKR_STARTED_AT_MS=0
SEEKR_EXECUTED_AT=0
SEEKR_CWD=""
SEEKR_HOSTNAME=""
SEEKR_GIT_REPO=""
SEEKR_GIT_BRANCH=""
SEEKR_LAST_HISTORY=""
SEEKR_READY=0

_seekr_readline() {
  local result action command_text
  result=$(command seekr) || return
  [[ $result == *$'\n'* ]] || return
  action=${result%%$'\n'*}
  command_text=${result#*$'\n'}
  if [[ $action == rerun ]]; then
    _seekr_rerun "$command_text"
  else
    READLINE_LINE=$command_text
    READLINE_POINT=${#READLINE_LINE}
  fi
}

bind -x '"\C-r":_seekr_readline'
case "$PROMPT_COMMAND" in
  '_seekr_precmd "$?"') SEEKR_PROMPT_COMMAND=${SEEKR_PROMPT_COMMAND:-} ;;
  *) SEEKR_PROMPT_COMMAND=$PROMPT_COMMAND ;;
esac

_seekr_now_ms() {
  perl -MTime::HiRes=time -e 'printf "%.0f\n", time * 1000'
}

_seekr_preexec() {
  [[ $SEEKR_READY == 1 ]] || return
  _seekr_start "$BASH_COMMAND"
}

_seekr_start() {
  local command=$1

  SEEKR_READY=0
  SEEKR_COMMAND=$command
  SEEKR_CWD=$PWD
  SEEKR_HOSTNAME=$(hostname)
  SEEKR_GIT_REPO=$(git rev-parse --show-toplevel 2>/dev/null) || SEEKR_GIT_REPO=""
  SEEKR_GIT_BRANCH=$(git symbolic-ref --quiet --short HEAD 2>/dev/null) || SEEKR_GIT_BRANCH=""
  SEEKR_STARTED_AT_MS=$(_seekr_now_ms)
  SEEKR_EXECUTED_AT=$(( SEEKR_STARTED_AT_MS / 1000 ))
}

_seekr_capture() {
  local exit_code=$1
  local finished_at_ms duration_ms
  local -a args

  finished_at_ms=$(_seekr_now_ms)
  duration_ms=$(( finished_at_ms - SEEKR_STARTED_AT_MS ))
  args=(
    --command-text "$SEEKR_COMMAND"
    --cwd "$SEEKR_CWD"
    --executed-at "$SEEKR_EXECUTED_AT"
    --exit-code "$exit_code"
    --shell bash
    --duration-ms "$duration_ms"
    --hostname "$SEEKR_HOSTNAME"
  )
  [[ -n $SEEKR_GIT_REPO ]] && args+=(--git-repo "$SEEKR_GIT_REPO")
  [[ -n $SEEKR_GIT_BRANCH ]] && args+=(--git-branch "$SEEKR_GIT_BRANCH")
  seekr capture "${args[@]}" >/dev/null 2>&1
}

_seekr_rerun() {
  local command_text=$1 exit_code

  _seekr_start "$command_text"
  builtin eval -- "$command_text"
  exit_code=$?
  _seekr_capture "$exit_code"
  SEEKR_COMMAND=""
  SEEKR_READY=1
  return "$exit_code"
}

_seekr_precmd() {
  local exit_code=$1
  local history_line
  local HISTTIMEFORMAT=

  trap - DEBUG
  if [[ -n $SEEKR_PROMPT_COMMAND ]]; then
    (exit "$exit_code")
    eval "$SEEKR_PROMPT_COMMAND"
  fi

  if [[ -n $SEEKR_COMMAND ]]; then
    history_line=$(builtin history 1)
    if [[ $history_line != "$SEEKR_LAST_HISTORY" && $history_line =~ ^[[:space:]]*([0-9]+)[[:space:]]+(.*)$ ]]; then
      SEEKR_COMMAND=${BASH_REMATCH[2]}
    fi
    _seekr_capture "$exit_code"
  fi
  SEEKR_COMMAND=""
  SEEKR_LAST_HISTORY=$(builtin history 1)
  SEEKR_READY=1
  trap '_seekr_preexec' DEBUG
}

trap '_seekr_preexec' DEBUG
PROMPT_COMMAND='_seekr_precmd "$?"'
"#;

fn search(args: SearchArgs) -> io::Result<String> {
    let paths = config::ResolvedPaths::from_env()?;
    let connection = db::open(&paths.database_file())?;
    let collapsed = db::filtered_collapsed_records(
        &connection,
        Some(&args.query),
        &args.filters(),
        args.limit,
    )?;

    render_collapsed_records(
        collapsed,
        format!("No local commands found for {:?}.", args.query),
    )
}

fn here() -> io::Result<String> {
    let cwd = env::current_dir()?;
    let paths = config::ResolvedPaths::from_env()?;
    let connection = db::open(&paths.database_file())?;
    let collapsed = db::filtered_collapsed_records(
        &connection,
        None,
        &db::SearchFilters {
            cwd: Some(cwd.to_string_lossy().into_owned()),
            ..db::SearchFilters::default()
        },
        10,
    )?;

    if collapsed.is_empty() {
        if let Some(repo) = git_context(&cwd).repo {
            let collapsed = db::filtered_collapsed_records(
                &connection,
                None,
                &db::SearchFilters {
                    repo: Some(repo),
                    ..db::SearchFilters::default()
                },
                10,
            )?;
            return render_collapsed_records(
                collapsed,
                "No local commands found for the current context.".to_string(),
            );
        }
    }

    render_collapsed_records(
        collapsed,
        "No local commands found for the current context.".to_string(),
    )
}

fn failed() -> io::Result<String> {
    let paths = config::ResolvedPaths::from_env()?;
    let connection = db::open(&paths.database_file())?;
    let collapsed = db::filtered_collapsed_records(
        &connection,
        None,
        &db::SearchFilters {
            failed: Some(true),
            ..db::SearchFilters::default()
        },
        10,
    )?;

    render_collapsed_records(collapsed, "No local failed commands found.".to_string())
}

fn render_collapsed_records(
    collapsed: Vec<db::CollapsedRecord>,
    empty_message: String,
) -> io::Result<String> {
    if collapsed.is_empty() {
        return Ok(empty_message);
    }

    Ok(collapsed
        .iter()
        .map(|record| {
            let repeat = if record.repeat_count > 1 {
                format!(" | repeats: {}", record.repeat_count)
            } else {
                String::new()
            };
            let exit = if record.all_same_exit {
                format!("{}", record.most_recent_exit_code)
            } else {
                format!("mixed (most recent: {})", record.most_recent_exit_code)
            };
            let mut metadata = format!(
                "  cwd: {} | last used: {} | exit: {}{}",
                record.most_recent_cwd,
                format_relative_timestamp(record.most_recent_executed_at),
                exit,
                repeat
            );
            if let Some(repo) = &record.repo {
                metadata.push_str(&format!(" | repo: {repo}"));
            }
            if let Some(branch) = &record.branch {
                metadata.push_str(&format!(" | branch: {branch}"));
            }
            format!("{}\n{metadata}", record.command_text)
        })
        .collect::<Vec<_>>()
        .join("\n\n"))
}

#[derive(Debug, Default, PartialEq, Eq)]
struct GitContext {
    repo: Option<String>,
    branch: Option<String>,
}

fn git_context(cwd: &Path) -> GitContext {
    GitContext {
        repo: git_output(cwd, &["rev-parse", "--show-toplevel"]),
        branch: git_output(cwd, &["rev-parse", "--abbrev-ref", "HEAD"]),
    }
}

fn git_output(cwd: &Path, args: &[&str]) -> Option<String> {
    let output = ProcessCommand::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let value = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn capture(args: CaptureArgs) -> io::Result<String> {
    let paths = config::ResolvedPaths::from_env()?;
    let config = config::load(&paths)?;

    let command_text = match privacy::filter_command(&args.command_text, &config.privacy) {
        Some(filtered) => filtered,
        None => return Ok(String::new()),
    };

    let record = db::CommandRecord::new(
        command_text,
        args.cwd,
        args.executed_at,
        args.exit_code,
        args.shell,
        args.duration_ms,
        args.hostname,
        args.git_repo,
        args.git_branch,
    )?;
    let connection = db::open(&paths.database_file())?;
    db::insert_command_record(&connection, &record)?;
    Ok(String::new())
}

#[derive(Debug, PartialEq, Eq)]
struct HistoryEntry {
    command_text: String,
    executed_at: i64,
    duration_ms: Option<i64>,
}

#[derive(Debug, PartialEq, Eq)]
enum ParsedHistoryEntry {
    Entry(HistoryEntry),
    Skipped,
    Failed,
}

fn import(path: PathBuf) -> io::Result<String> {
    let history = fs::read(path)?;
    let fallback_timestamp = i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    )
    .map_err(io::Error::other)?;
    let cwd = std::env::current_dir()?.display().to_string();
    let paths = config::ResolvedPaths::from_env()?;
    let config = config::load(&paths)?;
    let connection = db::open(&paths.database_file())?;
    let mut inserted = 0;
    let mut skipped = 0;
    let mut failed = 0;

    for entry in parse_zsh_history(&history, fallback_timestamp) {
        let ParsedHistoryEntry::Entry(entry) = entry else {
            if matches!(entry, ParsedHistoryEntry::Skipped) {
                skipped += 1;
            } else {
                failed += 1;
            }
            continue;
        };

        if entry.command_text.trim().is_empty() {
            skipped += 1;
            continue;
        }

        let command_text = match privacy::filter_command(&entry.command_text, &config.privacy) {
            Some(filtered) => filtered,
            None => {
                skipped += 1;
                continue;
            }
        };

        let record = match db::CommandRecord::new(
            command_text,
            cwd.clone(),
            entry.executed_at,
            0,
            Some("zsh".to_string()),
            entry.duration_ms,
            None,
            None,
            None,
        ) {
            Ok(record) => record,
            Err(_) => {
                failed += 1;
                continue;
            }
        };

        if db::insert_command_record(&connection, &record).is_ok() {
            inserted += 1;
        } else {
            failed += 1;
        }
    }

    Ok(format!(
        "Import complete: inserted: {inserted}, skipped: {skipped}, failed: {failed}."
    ))
}

fn parse_zsh_history(history: &[u8], fallback_timestamp: i64) -> Vec<ParsedHistoryEntry> {
    let mut entries = Vec::new();

    for line in history.split_inclusive(|byte| *byte == b'\n') {
        let line = line.strip_suffix(b"\n").unwrap_or(line);
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        let Ok(line) = std::str::from_utf8(line) else {
            entries.push(ParsedHistoryEntry::Failed);
            continue;
        };

        let extended_metadata = extended_history_metadata(line);

        if let Some(ParsedHistoryEntry::Entry(entry)) = entries.last_mut() {
            if entry.command_text.ends_with('\\') && extended_metadata.is_none() {
                entry.command_text.push('\n');
                entry.command_text.push_str(line);
                continue;
            }
        }

        if line.trim().is_empty() {
            entries.push(ParsedHistoryEntry::Skipped);
        } else if let Some(metadata) = extended_metadata {
            entries.push(parse_extended_history_entry(metadata));
        } else {
            entries.push(ParsedHistoryEntry::Entry(HistoryEntry {
                command_text: line.to_string(),
                executed_at: fallback_timestamp,
                duration_ms: None,
            }));
        }
    }

    entries
}

fn extended_history_metadata(line: &str) -> Option<&str> {
    line.strip_prefix(": ").filter(|metadata| {
        metadata
            .split_once(';')
            .is_some_and(|(metadata, _)| metadata.contains(':'))
    })
}

fn parse_extended_history_entry(metadata: &str) -> ParsedHistoryEntry {
    let Some((metadata, command_text)) = metadata.split_once(';') else {
        return ParsedHistoryEntry::Failed;
    };
    let Some((timestamp, duration)) = metadata.split_once(':') else {
        return ParsedHistoryEntry::Failed;
    };
    let (Ok(executed_at), Some(duration_ms)) = (
        timestamp.parse::<i64>(),
        duration
            .parse::<i64>()
            .ok()
            .filter(|duration| *duration >= 0)
            .and_then(|duration| duration.checked_mul(1_000)),
    ) else {
        return ParsedHistoryEntry::Failed;
    };
    if executed_at < 0 {
        return ParsedHistoryEntry::Failed;
    }

    ParsedHistoryEntry::Entry(HistoryEntry {
        command_text: command_text.to_string(),
        executed_at,
        duration_ms: Some(duration_ms),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        dispatch, git_context, parse_zsh_history, CaptureArgs, Cli, Command, HistoryEntry,
        HookShell, InitArgs, ParsedHistoryEntry, ProcessCommand, SearchArgs,
    };
    use clap::{CommandFactory, Parser};
    use std::env;
    use std::ffi::OsString;
    use std::fs;
    use std::io::{self, Write};
    use std::path::PathBuf;
    use std::process::Stdio;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parses_mvp_command_surface() {
        let cases = [
            (vec!["seekr"], None),
            (
                vec!["seekr", "search", "docker"],
                Some(Command::Search(SearchArgs {
                    query: "docker".to_string(),
                    limit: 10,
                    ..SearchArgs::default()
                })),
            ),
            (
                vec!["seekr", "search", "docker", "--limit", "2"],
                Some(Command::Search(SearchArgs {
                    query: "docker".to_string(),
                    limit: 2,
                    ..SearchArgs::default()
                })),
            ),
            (
                vec![
                    "seekr", "search", "docker", "--cwd", "/tmp/app", "--repo", "app", "--branch",
                    "main", "--failed", "--since", "100", "--before", "200",
                ],
                Some(Command::Search(SearchArgs {
                    query: "docker".to_string(),
                    limit: 10,
                    cwd: Some("/tmp/app".to_string()),
                    repo: Some("app".to_string()),
                    branch: Some("main".to_string()),
                    failed: true,
                    since: Some(100),
                    before: Some(200),
                    ..SearchArgs::default()
                })),
            ),
            (
                vec!["seekr", "search", "docker", "--successful"],
                Some(Command::Search(SearchArgs {
                    query: "docker".to_string(),
                    limit: 10,
                    successful: true,
                    ..SearchArgs::default()
                })),
            ),
            (vec!["seekr", "here"], Some(Command::Here)),
            (vec!["seekr", "failed"], Some(Command::Failed)),
            (
                vec!["seekr", "import", "~/.zsh_history"],
                Some(Command::Import {
                    path: PathBuf::from("~/.zsh_history"),
                }),
            ),
            (
                vec!["seekr", "init", "zsh"],
                Some(Command::Init(InitArgs {
                    shell: HookShell::Zsh,
                })),
            ),
            (
                vec![
                    "seekr",
                    "capture",
                    "--command-text",
                    "cargo test",
                    "--cwd",
                    "/tmp/project",
                    "--executed-at",
                    "1720000000",
                    "--exit-code",
                    "0",
                ],
                Some(Command::Capture(CaptureArgs {
                    command_text: "cargo test".to_string(),
                    cwd: "/tmp/project".to_string(),
                    executed_at: 1_720_000_000,
                    exit_code: 0,
                    shell: None,
                    duration_ms: None,
                    hostname: None,
                    git_repo: None,
                    git_branch: None,
                })),
            ),
            (vec!["seekr", "stats"], Some(Command::Stats)),
        ];

        for (args, expected_command) in cases {
            let cli = Cli::try_parse_from(args).expect("MVP command should parse");

            assert_eq!(cli.command, expected_command);
        }
    }

    #[test]
    fn bare_seekr_parses_as_no_subcommand() {
        let cli = Cli::try_parse_from(["seekr"]).expect("bare seekr should parse");
        assert_eq!(cli.command, None);
    }

    #[test]
    fn empty_search_reports_local_only_message() {
        let _guard = crate::config::env_lock().lock().expect("env lock");
        let root = temp_root("search-empty");
        let config_dir = root.join("config");
        let data_dir = root.join("data");
        let original_config_dir = env::var_os("SEEKR_CONFIG_DIR");
        let original_data_dir = env::var_os("SEEKR_DATA_DIR");

        unsafe {
            env::set_var("SEEKR_CONFIG_DIR", &config_dir);
            env::set_var("SEEKR_DATA_DIR", &data_dir);
        }

        let output = dispatch(Cli {
            command: Some(Command::Search(SearchArgs {
                query: "docker".to_string(),
                limit: 10,
                ..SearchArgs::default()
            })),
        })
        .expect("empty search should succeed");

        restore_env("SEEKR_CONFIG_DIR", original_config_dir);
        restore_env("SEEKR_DATA_DIR", original_data_dir);

        assert_eq!(output, "No local commands found for \"docker\".");
    }

    #[test]
    fn stats_reports_local_paths_and_defaults() {
        let _guard = crate::config::env_lock().lock().expect("env lock");
        let root = temp_root("stats");
        let config_dir = root.join("config");
        let data_dir = root.join("data");
        let original_config_dir = env::var_os("SEEKR_CONFIG_DIR");
        let original_data_dir = env::var_os("SEEKR_DATA_DIR");

        unsafe {
            env::set_var("SEEKR_CONFIG_DIR", &config_dir);
            env::set_var("SEEKR_DATA_DIR", &data_dir);
        }

        let output = dispatch(Cli {
            command: Some(Command::Stats),
        })
        .expect("stats should render");

        restore_env("SEEKR_CONFIG_DIR", original_config_dir);
        restore_env("SEEKR_DATA_DIR", original_data_dir);

        assert!(output.contains("Seekr Stats"));
        assert!(output.contains("Database:"));
        assert!(output.contains(&format!("Path: {}", data_dir.join("seekr.db").display())));
        assert!(output.contains("Redaction: disabled"));
        assert!(output.contains("Ignore rules: 4"));
        assert!(output.contains("Database: missing"));
        assert!(!data_dir.exists());
    }

    #[test]
    fn includes_zsh_integration_in_help() {
        let mut command = Cli::command();
        let mut help = Vec::new();

        command
            .write_long_help(&mut help)
            .expect("help rendering should succeed");

        let help = String::from_utf8(help).expect("help should be utf8");

        assert!(help.contains("generated zsh integration"));
        assert!(help.contains("search"));
        assert!(help.contains("here"));
        assert!(help.contains("failed"));
        assert!(help.contains("import"));
        assert!(help.contains("init"));
        assert!(help.contains("stats"));
        assert!(!help
            .lines()
            .any(|line| line.trim_start().starts_with("capture")));
    }

    #[test]
    fn formats_command_timestamps_for_people() {
        assert_eq!(
            super::format_relative_timestamp_at(1_000, 1_000),
            "just now"
        );
        assert_eq!(super::format_relative_timestamp_at(940, 1_000), "1m ago");
        assert_eq!(
            super::format_relative_timestamp_at(1_000 - 3 * 60 * 60, 1_000),
            "3h ago"
        );
        assert_eq!(
            super::format_relative_timestamp_at(1_000 - 2 * 24 * 60 * 60, 1_000),
            "2d ago"
        );
        assert_eq!(
            super::format_relative_timestamp_at(1_000 + 5 * 60, 1_000),
            "in 5m"
        );
    }

    #[test]
    fn zsh_hook_uses_lifecycle_hooks_and_capture_metadata() {
        let output = dispatch(Cli {
            command: Some(Command::Init(InitArgs {
                shell: HookShell::Zsh,
            })),
        })
        .expect("zsh hook should render");

        assert!(output.contains("add-zsh-hook preexec _seekr_preexec"));
        assert!(output.contains("add-zsh-hook precmd _seekr_precmd"));
        assert!(output.contains("zmodload zsh/datetime"));
        assert!(output.contains("SEEKR_CWD=$PWD"));
        assert!(output.contains("EPOCHREALTIME * 1000"));
        assert!(output.contains("bindkey '^R' seekr-history"));
        assert!(output.contains("sk()"));
        assert!(output.contains("print -rz -- \"$command_text\""));
        assert!(output.contains("BUFFER=$command_text"));
        assert!(output.contains("[[ $action == rerun ]] && zle accept-line"));
        assert_capture_fields(&output, "zsh");
        assert!(output.contains("~/.zshrc"));
        assert_shell_syntax("zsh", &output);
    }

    #[cfg(unix)]
    #[test]
    fn zsh_sk_stages_raw_multiline_commands_without_printing_protocol() {
        use std::os::unix::fs::PermissionsExt;

        let hook = dispatch(Cli {
            command: Some(Command::Init(InitArgs {
                shell: HookShell::Zsh,
            })),
        })
        .expect("zsh hook should render");
        let root = temp_root("zsh-sk");
        let seekr = root.join("seekr");
        let selection = root.join("selection");
        let selected = "printf '%s\\n' \\\n  'hello world'";
        fs::write(
            &seekr,
            "#!/bin/sh\nprintf 'insert\\n'\ncat \"$SEEKR_TEST_SELECTION\"\n",
        )
        .expect("seekr stub should write");
        fs::set_permissions(&seekr, fs::Permissions::from_mode(0o755))
            .expect("seekr stub should be executable");
        fs::write(&selection, selected).expect("selection fixture should write");

        let input = r#"eval "$SEEKR_TEST_HOOK"
sk
read -rz staged
print -rn -- "$staged"
"#;
        let path = format!(
            "{}:{}",
            root.display(),
            env::var("PATH").expect("PATH should be set")
        );
        let mut child = ProcessCommand::new("zsh")
            .args(["-f"])
            .env("PATH", path)
            .env("SEEKR_TEST_HOOK", hook)
            .env("SEEKR_TEST_SELECTION", &selection)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("zsh should start");
        child
            .stdin
            .take()
            .expect("zsh stdin")
            .write_all(input.as_bytes())
            .expect("zsh input should write");
        let output = child.wait_with_output().expect("zsh should finish");

        assert!(
            output.status.success(),
            "zsh failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout), selected);
    }

    #[cfg(unix)]
    #[test]
    fn zsh_sk_captures_selected_commands_instead_of_the_wrapper() {
        use std::os::unix::fs::PermissionsExt;

        let hook = dispatch(Cli {
            command: Some(Command::Init(InitArgs {
                shell: HookShell::Zsh,
            })),
        })
        .expect("zsh hook should render");
        let root = temp_root("zsh-sk-capture");
        let seekr = root.join("seekr");
        let insert_selection = root.join("insert-selection");
        let rerun_selection = root.join("rerun-selection");
        let captures = root.join("captures");
        let insert_command = "printf '%s\\n' \\\n  'insert marker'";
        let rerun_command = "printf '%s\\n' \\\n  'rerun marker'\nreturn 7";
        fs::write(
            &seekr,
            r#"#!/bin/sh
if [ "$1" = capture ]; then
  shift
  printf 'CAPTURE\n' >> "$SEEKR_TEST_CAPTURES"
  printf 'ARG=<%s>\n' "$@" >> "$SEEKR_TEST_CAPTURES"
  exit 0
fi
printf '%s\n' "$SEEKR_TEST_ACTION"
cat "$SEEKR_TEST_SELECTION"
"#,
        )
        .expect("seekr stub should write");
        fs::set_permissions(&seekr, fs::Permissions::from_mode(0o755))
            .expect("seekr stub should be executable");
        fs::write(&insert_selection, insert_command).expect("insert fixture should write");
        fs::write(&rerun_selection, rerun_command).expect("rerun fixture should write");

        let input = r#"eval "$SEEKR_TEST_HOOK"
preexec_functions=()
export SEEKR_TEST_ACTION=insert
export SEEKR_TEST_SELECTION="$SEEKR_TEST_INSERT_SELECTION"
_seekr_preexec sk
sk
insert_status=$?
(exit "$insert_status")
_seekr_precmd
read -rz staged
print -r -- "INSERT_STATUS=$insert_status"
print -r -- "STAGED=$staged"
export SEEKR_TEST_ACTION=rerun
export SEEKR_TEST_SELECTION="$SEEKR_TEST_RERUN_SELECTION"
_seekr_preexec sk
sk
rerun_status=$?
(exit "$rerun_status")
_seekr_precmd
print -r -- "RERUN_STATUS=$rerun_status"
"#;
        let path = format!(
            "{}:{}",
            root.display(),
            env::var("PATH").expect("PATH should be set")
        );
        let mut child = ProcessCommand::new("zsh")
            .args(["-f"])
            .env("PATH", path)
            .env("SEEKR_TEST_HOOK", hook)
            .env("SEEKR_TEST_INSERT_SELECTION", &insert_selection)
            .env("SEEKR_TEST_RERUN_SELECTION", &rerun_selection)
            .env("SEEKR_TEST_CAPTURES", &captures)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("zsh should start");
        child
            .stdin
            .take()
            .expect("zsh stdin")
            .write_all(input.as_bytes())
            .expect("zsh input should write");
        let output = child.wait_with_output().expect("zsh should finish");

        assert!(
            output.status.success(),
            "zsh failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            format!("INSERT_STATUS=0\nSTAGED={insert_command}\nrerun marker\nRERUN_STATUS=7\n")
        );
        let captures = fs::read_to_string(captures).expect("rerun should be captured");
        assert_eq!(captures.matches("CAPTURE\n").count(), 1);
        assert!(captures.contains(&format!("ARG=<--command-text>\nARG=<{rerun_command}>\n")));
        assert!(captures.contains("ARG=<--exit-code>\nARG=<7>\n"));
        assert!(!captures.contains("ARG=<sk>"));
    }

    #[cfg(unix)]
    #[test]
    fn zsh_sk_forwards_cli_arguments() {
        use std::os::unix::fs::PermissionsExt;

        let hook = dispatch(Cli {
            command: Some(Command::Init(InitArgs {
                shell: HookShell::Zsh,
            })),
        })
        .expect("zsh hook should render");
        let root = temp_root("zsh-sk-args");
        let seekr = root.join("seekr");
        fs::write(&seekr, "#!/bin/sh\nprintf '<%s>\\n' \"$@\"\n").expect("seekr stub should write");
        fs::set_permissions(&seekr, fs::Permissions::from_mode(0o755))
            .expect("seekr stub should be executable");

        let path = format!(
            "{}:{}",
            root.display(),
            env::var("PATH").expect("PATH should be set")
        );
        let output = ProcessCommand::new("zsh")
            .args(["-fc", "eval \"$SEEKR_TEST_HOOK\"; sk --version"])
            .env("PATH", path)
            .env("SEEKR_TEST_HOOK", hook)
            .output()
            .expect("zsh should finish");

        assert!(
            output.status.success(),
            "zsh failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout), "<--version>\n");
    }

    #[test]
    fn bash_hook_uses_debug_trap_and_prompt_command() {
        let output = dispatch(Cli {
            command: Some(Command::Init(InitArgs {
                shell: HookShell::Bash,
            })),
        })
        .expect("bash hook should render");

        assert!(output.contains("trap '_seekr_preexec' DEBUG"));
        assert!(output.contains("history_line=$(builtin history 1)"));
        assert!(output.contains("[[ $SEEKR_READY == 1 ]] || return"));
        assert!(output.contains("SEEKR_PROMPT_COMMAND=$PROMPT_COMMAND"));
        assert!(output.contains("PROMPT_COMMAND='_seekr_precmd \"$?\"'"));
        assert!(output.contains("Seekr must own the DEBUG trap"));
        assert!(output.contains("bind -x '\"\\C-r\":_seekr_readline'"));
        assert!(output.contains("READLINE_LINE=$command_text"));
        assert!(output.contains("_seekr_rerun \"$command_text\""));
        assert!(output.contains("_seekr_start \"$command_text\""));
        assert!(output.contains("_seekr_capture \"$exit_code\""));
        assert_capture_fields(&output, "bash");
        assert!(output.contains("~/.bashrc"));
        assert_shell_syntax("bash", &output);
    }

    #[test]
    fn bash_hook_captures_one_full_history_line_and_preserves_prompt_status() {
        let hook = dispatch(Cli {
            command: Some(Command::Init(InitArgs {
                shell: HookShell::Bash,
            })),
        })
        .expect("bash hook should render");
        let root = temp_root("bash-hook");
        let log = root.join("captures");
        let command = "cd /tmp; sleep 0.05; printf alpha | sed s/alpha/beta/; printf gamma";
        let input = format!(
            r#"seekr() {{ printf 'ARG=<%s>\n' "$@" >> "$SEEKR_TEST_LOG"; printf 'END\n' >> "$SEEKR_TEST_LOG"; }}
PROMPT_COMMAND='printf "OLD_STATUS=%s\n" "$?"'
eval "$SEEKR_TEST_HOOK"
eval "$SEEKR_TEST_HOOK"
{command}
false
exit
"#
        );
        let mut child = ProcessCommand::new("bash")
            .args(["--noprofile", "--norc", "-i"])
            .env("SEEKR_TEST_HOOK", hook)
            .env("SEEKR_TEST_LOG", &log)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("bash should start");
        child
            .stdin
            .take()
            .expect("bash stdin")
            .write_all(input.as_bytes())
            .expect("bash input should write");
        let output = child.wait_with_output().expect("bash should finish");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let captures = fs::read_to_string(log).expect("capture log should exist");

        assert!(stdout.contains("OLD_STATUS=1"));
        assert_eq!(captures.matches("END\n").count(), 2);
        assert!(captures.contains(&format!("ARG=<--command-text>\nARG=<{command}>")));
        assert!(captures.contains(&format!(
            "ARG=<--cwd>\nARG=<{}>",
            env!("CARGO_MANIFEST_DIR")
        )));
    }

    #[test]
    fn bash_hook_captures_explicit_reruns() {
        let hook = dispatch(Cli {
            command: Some(Command::Init(InitArgs {
                shell: HookShell::Bash,
            })),
        })
        .expect("bash hook should render");
        let root = temp_root("bash-rerun");
        let log = root.join("captures");
        let input = r#"seekr() { printf 'ARG=<%s>\n' "$@" >> "$SEEKR_TEST_LOG"; printf 'END\n' >> "$SEEKR_TEST_LOG"; }
eval "$SEEKR_TEST_HOOK"
_seekr_rerun 'printf rerun-marker'
"#;
        let output = ProcessCommand::new("bash")
            .args(["--noprofile", "--norc"])
            .env("SEEKR_TEST_HOOK", hook)
            .env("SEEKR_TEST_LOG", &log)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("bash should start");
        output
            .stdin
            .as_ref()
            .expect("bash stdin")
            .write_all(input.as_bytes())
            .expect("bash input should write");
        let output = output.wait_with_output().expect("bash should finish");
        let captures = fs::read_to_string(log).expect("capture log should exist");

        assert_eq!(String::from_utf8_lossy(&output.stdout), "rerun-marker");
        assert_eq!(captures.matches("END\n").count(), 1);
        assert!(captures.contains("ARG=<--command-text>\nARG=<printf rerun-marker>"));
    }

    fn assert_capture_fields(output: &str, shell: &str) {
        for field in [
            "seekr capture",
            "--command-text",
            "--cwd",
            "--executed-at",
            "--exit-code",
            "--duration-ms",
            "--hostname",
            "--git-repo",
            "--git-branch",
        ] {
            assert!(output.contains(field), "missing {field}");
        }
        assert!(output.contains(&format!("--shell {shell}")));
    }

    fn assert_shell_syntax(shell: &str, hook: &str) {
        let mut child = ProcessCommand::new(shell)
            .arg("-n")
            .stdin(Stdio::piped())
            .spawn()
            .unwrap_or_else(|error| panic!("{shell} should start: {error}"));
        child
            .stdin
            .take()
            .expect("shell stdin")
            .write_all(hook.as_bytes())
            .expect("hook should write");
        assert!(child.wait().expect("shell should finish").success());
    }

    #[test]
    fn malformed_capture_numbers_are_rejected_by_cli_parser() {
        let error = Cli::try_parse_from([
            "seekr",
            "capture",
            "--command-text",
            "cargo test",
            "--cwd",
            "/tmp/project",
            "--executed-at",
            "not-a-timestamp",
            "--exit-code",
            "0",
        ])
        .expect_err("invalid timestamp should fail");
        assert_eq!(error.kind(), clap::error::ErrorKind::ValueValidation);

        let error = Cli::try_parse_from([
            "seekr",
            "capture",
            "--command-text",
            "cargo test",
            "--cwd",
            "/tmp/project",
            "--executed-at",
            "1720000000",
            "--exit-code",
            "not-an-exit-code",
        ])
        .expect_err("invalid exit code should fail");
        assert_eq!(error.kind(), clap::error::ErrorKind::ValueValidation);
    }

    #[test]
    fn invalid_search_limits_are_rejected_by_cli_parser() {
        for limit in ["0", "101", "18446744073709551615"] {
            let error = Cli::try_parse_from(["seekr", "search", "docker", "--limit", limit])
                .expect_err("invalid search limit should fail");
            assert_eq!(error.kind(), clap::error::ErrorKind::ValueValidation);
        }
    }

    #[test]
    fn capture_persists_command_metadata() {
        let _guard = crate::config::env_lock().lock().expect("env lock");
        let root = temp_root("capture");
        let config_dir = root.join("config");
        let data_dir = root.join("data");
        let original_config_dir = env::var_os("SEEKR_CONFIG_DIR");
        let original_data_dir = env::var_os("SEEKR_DATA_DIR");

        unsafe {
            env::set_var("SEEKR_CONFIG_DIR", &config_dir);
            env::set_var("SEEKR_DATA_DIR", &data_dir);
        }

        let output = dispatch(Cli {
            command: Some(Command::Capture(CaptureArgs {
                command_text: "gh pr checkout 123 && cargo   test".to_string(),
                cwd: "/tmp/project".to_string(),
                executed_at: 1_720_000_000,
                exit_code: 0,
                shell: Some("zsh".to_string()),
                duration_ms: Some(250),
                hostname: Some("seekr-host".to_string()),
                git_repo: Some("seekr".to_string()),
                git_branch: Some("feat/task-04".to_string()),
            })),
        })
        .expect("capture should succeed");

        let connection = crate::db::open(&data_dir.join("seekr.db")).expect("database should open");
        let records =
            crate::db::recent_command_records(&connection, 5).expect("records should load");

        restore_env("SEEKR_CONFIG_DIR", original_config_dir);
        restore_env("SEEKR_DATA_DIR", original_data_dir);

        assert!(output.is_empty());
        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0].command_text,
            "gh pr checkout 123 && cargo   test"
        );
        assert_eq!(records[0].normalized_text, "gh pr checkout 123 cargo test");
        assert_eq!(records[0].shell.as_deref(), Some("zsh"));
        assert_eq!(records[0].duration_ms, Some(250));
        assert_eq!(records[0].git_repo.as_deref(), Some("seekr"));
        assert_eq!(records[0].git_branch.as_deref(), Some("feat/task-04"));
    }

    #[test]
    fn empty_capture_command_is_rejected_without_persisting() {
        let _guard = crate::config::env_lock().lock().expect("env lock");
        let root = temp_root("capture-empty");
        let config_dir = root.join("config");
        let data_dir = root.join("data");
        let original_config_dir = env::var_os("SEEKR_CONFIG_DIR");
        let original_data_dir = env::var_os("SEEKR_DATA_DIR");

        unsafe {
            env::set_var("SEEKR_CONFIG_DIR", &config_dir);
            env::set_var("SEEKR_DATA_DIR", &data_dir);
        }

        let error = dispatch(Cli {
            command: Some(Command::Capture(CaptureArgs {
                command_text: "   ".to_string(),
                cwd: "/tmp/project".to_string(),
                executed_at: 1_720_000_000,
                exit_code: 0,
                shell: None,
                duration_ms: None,
                hostname: None,
                git_repo: None,
                git_branch: None,
            })),
        })
        .expect_err("blank capture should fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);

        let connection = crate::db::open(&data_dir.join("seekr.db")).expect("database should open");
        let records =
            crate::db::recent_command_records(&connection, 5).expect("records should load");

        restore_env("SEEKR_CONFIG_DIR", original_config_dir);
        restore_env("SEEKR_DATA_DIR", original_data_dir);

        assert!(records.is_empty());
    }

    #[test]
    fn parses_zsh_history_fixture_formats() {
        let entries = parse_zsh_history(
            include_bytes!("../tests/fixtures/zsh_history.sample"),
            1_700_000_003,
        );

        assert_eq!(
            entries,
            vec![
                ParsedHistoryEntry::Entry(HistoryEntry {
                    command_text: "git status".to_string(),
                    executed_at: 1_700_000_000,
                    duration_ms: Some(0),
                }),
                ParsedHistoryEntry::Entry(HistoryEntry {
                    command_text: "cargo test".to_string(),
                    executed_at: 1_700_000_001,
                    duration_ms: Some(2_000),
                }),
                ParsedHistoryEntry::Entry(HistoryEntry {
                    command_text: "plain history command".to_string(),
                    executed_at: 1_700_000_003,
                    duration_ms: None,
                }),
                ParsedHistoryEntry::Skipped,
                ParsedHistoryEntry::Failed,
                ParsedHistoryEntry::Entry(HistoryEntry {
                    command_text: "printf 'first \\\nsecond'".to_string(),
                    executed_at: 1_700_000_002,
                    duration_ms: Some(1_000),
                }),
                ParsedHistoryEntry::Entry(HistoryEntry {
                    command_text: "printf 'metadata-looking \\\n: continuation'".to_string(),
                    executed_at: 1_700_000_005,
                    duration_ms: Some(0),
                }),
                ParsedHistoryEntry::Entry(HistoryEntry {
                    command_text: "echo \\".to_string(),
                    executed_at: 1_700_000_006,
                    duration_ms: Some(0),
                }),
                ParsedHistoryEntry::Entry(HistoryEntry {
                    command_text: "git log".to_string(),
                    executed_at: 1_700_000_007,
                    duration_ms: Some(0),
                }),
                ParsedHistoryEntry::Entry(HistoryEntry {
                    command_text: ": plain colon command".to_string(),
                    executed_at: 1_700_000_003,
                    duration_ms: None,
                }),
                ParsedHistoryEntry::Entry(HistoryEntry {
                    command_text: String::new(),
                    executed_at: 1_700_000_004,
                    duration_ms: Some(0),
                }),
            ]
        );
    }

    #[test]
    fn import_persists_fixture_records_and_reports_counts() {
        let _guard = crate::config::env_lock().lock().expect("env lock");
        let root = temp_root("import");
        let config_dir = root.join("config");
        let data_dir = root.join("data");
        let original_config_dir = env::var_os("SEEKR_CONFIG_DIR");
        let original_data_dir = env::var_os("SEEKR_DATA_DIR");

        unsafe {
            env::set_var("SEEKR_CONFIG_DIR", &config_dir);
            env::set_var("SEEKR_DATA_DIR", &data_dir);
        }

        let output = dispatch(Cli {
            command: Some(Command::Import {
                path: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/zsh_history.sample"),
            }),
        })
        .expect("history import should succeed");
        let connection = crate::db::open(&data_dir.join("seekr.db")).expect("database should open");
        let records =
            crate::db::recent_command_records(&connection, 10).expect("records should load");

        restore_env("SEEKR_CONFIG_DIR", original_config_dir);
        restore_env("SEEKR_DATA_DIR", original_data_dir);

        assert_eq!(
            output,
            "Import complete: inserted: 8, skipped: 2, failed: 1."
        );
        assert_eq!(records.len(), 8);
        assert!(records.iter().any(|record| {
            record.command_text == "git status"
                && record.executed_at == 1_700_000_000
                && record.duration_ms == Some(0)
                && record.shell.as_deref() == Some("zsh")
        }));
        assert!(records.iter().any(|record| {
            record.command_text == "plain history command" && record.duration_ms.is_none()
        }));
        assert!(records
            .iter()
            .any(|record| record.command_text == "printf 'first \\\nsecond'"));
        assert!(records.iter().any(|record| {
            record.command_text == "printf 'metadata-looking \\\n: continuation'"
        }));
        assert!(records
            .iter()
            .any(|record| record.command_text == "echo \\"));
        assert!(records
            .iter()
            .any(|record| record.command_text == "git log"));
        assert!(records
            .iter()
            .any(|record| record.command_text == ": plain colon command"));
    }

    #[test]
    fn import_counts_invalid_utf8_line_as_failed() {
        let _guard = crate::config::env_lock().lock().expect("env lock");
        let root = temp_root("import-invalid-utf8");
        let config_dir = root.join("config");
        let data_dir = root.join("data");
        let history_file = root.join(".zsh_history");
        let original_config_dir = env::var_os("SEEKR_CONFIG_DIR");
        let original_data_dir = env::var_os("SEEKR_DATA_DIR");
        fs::write(
            &history_file,
            b"git status\ninvalid \xff line\ncargo test\n",
        )
        .expect("history fixture should write");

        unsafe {
            env::set_var("SEEKR_CONFIG_DIR", &config_dir);
            env::set_var("SEEKR_DATA_DIR", &data_dir);
        }

        let output = dispatch(Cli {
            command: Some(Command::Import { path: history_file }),
        })
        .expect("valid history lines should still import");
        let connection = crate::db::open(&data_dir.join("seekr.db")).expect("database should open");
        let records =
            crate::db::recent_command_records(&connection, 10).expect("records should load");

        restore_env("SEEKR_CONFIG_DIR", original_config_dir);
        restore_env("SEEKR_DATA_DIR", original_data_dir);

        assert_eq!(
            output,
            "Import complete: inserted: 2, skipped: 0, failed: 1."
        );
        assert_eq!(records.len(), 2);
    }

    #[test]
    fn search_outputs_raw_command_and_metadata() {
        let _guard = crate::config::env_lock().lock().expect("env lock");
        let root = temp_root("search");
        let config_dir = root.join("config");
        let data_dir = root.join("data");
        let original_config_dir = env::var_os("SEEKR_CONFIG_DIR");
        let original_data_dir = env::var_os("SEEKR_DATA_DIR");

        unsafe {
            env::set_var("SEEKR_CONFIG_DIR", &config_dir);
            env::set_var("SEEKR_DATA_DIR", &data_dir);
        }

        let connection = crate::db::open(&data_dir.join("seekr.db")).expect("database should open");
        let record = crate::db::CommandRecord::new(
            "docker compose up && cargo test".to_string(),
            "/tmp/project".to_string(),
            1_720_000_000,
            0,
            None,
            None,
            None,
            Some("seekr".to_string()),
            Some("main".to_string()),
        )
        .expect("record should validate");
        crate::db::insert_command_record(&connection, &record).expect("record should insert");

        let output = dispatch(Cli {
            command: Some(Command::Search(SearchArgs {
                query: "docker".to_string(),
                limit: 10,
                ..SearchArgs::default()
            })),
        })
        .expect("search should succeed");

        restore_env("SEEKR_CONFIG_DIR", original_config_dir);
        restore_env("SEEKR_DATA_DIR", original_data_dir);

        assert!(output.contains("docker compose up && cargo test"));
        assert!(output.contains("cwd: /tmp/project"));
        assert!(output.contains("last used:"));
        assert!(!output.contains("timestamp:"));
        assert!(output.contains("exit: 0"));
        assert!(output.contains("repo: seekr"));
        assert!(output.contains("branch: main"));
    }

    #[test]
    fn shortcuts_return_contextual_and_failed_records() {
        let _guard = crate::config::env_lock().lock().expect("env lock");
        let root = temp_root("shortcuts");
        let config_dir = root.join("config");
        let data_dir = root.join("data");
        let original_config_dir = env::var_os("SEEKR_CONFIG_DIR");
        let original_data_dir = env::var_os("SEEKR_DATA_DIR");
        let original_cwd = env::current_dir().expect("current directory");
        let repo_path = root.join("repo");
        let git_init = ProcessCommand::new("git")
            .args(["init", "--quiet"])
            .arg(&repo_path)
            .status()
            .expect("git should run");
        assert!(git_init.success());
        env::set_current_dir(repo_path).expect("fixture should become current directory");
        let cwd_path = env::current_dir().expect("fixture current directory");
        let cwd = cwd_path.to_string_lossy().into_owned();
        let repo = git_context(&cwd_path)
            .repo
            .expect("fixture should be a git repo");

        unsafe {
            env::set_var("SEEKR_CONFIG_DIR", &config_dir);
            env::set_var("SEEKR_DATA_DIR", &data_dir);
        }

        let connection = crate::db::open(&data_dir.join("seekr.db")).expect("database should open");
        for (command_text, command_cwd, exit_code) in [
            ("cargo test", cwd.as_str(), 0),
            ("docker compose up", "/tmp/other", 1),
        ] {
            let record = crate::db::CommandRecord::new(
                command_text.to_string(),
                command_cwd.to_string(),
                1_720_000_000,
                exit_code,
                None,
                None,
                None,
                None,
                None,
            )
            .expect("record should validate");
            crate::db::insert_command_record(&connection, &record).expect("record should insert");
        }
        let record = crate::db::CommandRecord::new(
            "git status".to_string(),
            "/tmp/repo-context".to_string(),
            1_720_000_001,
            0,
            None,
            None,
            None,
            Some(repo),
            None,
        )
        .expect("record should validate");
        crate::db::insert_command_record(&connection, &record).expect("record should insert");

        let here = dispatch(Cli {
            command: Some(Command::Here),
        })
        .expect("here should search");
        let failed = dispatch(Cli {
            command: Some(Command::Failed),
        })
        .expect("failed should search");
        connection
            .execute("DELETE FROM commands WHERE cwd = ?1", [&cwd])
            .expect("current directory record should delete");
        let repo_here = dispatch(Cli {
            command: Some(Command::Here),
        })
        .expect("here should fall back to repo");

        restore_env("SEEKR_CONFIG_DIR", original_config_dir);
        restore_env("SEEKR_DATA_DIR", original_data_dir);
        env::set_current_dir(original_cwd).expect("current directory should restore");

        assert!(here.contains("cargo test"));
        assert!(!here.contains("docker compose up"));
        assert!(failed.contains("docker compose up"));
        assert!(!failed.contains("cargo test"));
        assert!(repo_here.contains("git status"));
    }

    #[test]
    fn git_context_is_empty_outside_a_repository() {
        let context = git_context(&temp_root("outside-git"));

        assert_eq!(context.repo, None);
        assert_eq!(context.branch, None);
    }

    #[test]
    fn default_capture_preserves_raw_commands() {
        let _guard = crate::config::env_lock().lock().expect("env lock");
        let root = temp_root("capture-privacy-default");
        let config_dir = root.join("config");
        let data_dir = root.join("data");
        let original_config_dir = env::var_os("SEEKR_CONFIG_DIR");
        let original_data_dir = env::var_os("SEEKR_DATA_DIR");

        unsafe {
            env::set_var("SEEKR_CONFIG_DIR", &config_dir);
            env::set_var("SEEKR_DATA_DIR", &data_dir);
        }

        let _ = dispatch(Cli {
            command: Some(Command::Capture(CaptureArgs {
                command_text: "export PASSWORD=mysecret123 && deploy".to_string(),
                cwd: "/tmp/project".to_string(),
                executed_at: 1_720_000_000,
                exit_code: 0,
                shell: None,
                duration_ms: None,
                hostname: None,
                git_repo: None,
                git_branch: None,
            })),
        })
        .expect("capture should succeed");

        let connection = crate::db::open(&data_dir.join("seekr.db")).expect("database should open");
        let records =
            crate::db::recent_command_records(&connection, 5).expect("records should load");

        restore_env("SEEKR_CONFIG_DIR", original_config_dir);
        restore_env("SEEKR_DATA_DIR", original_data_dir);

        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0].command_text,
            "export PASSWORD=mysecret123 && deploy"
        );
    }

    #[test]
    fn enabled_redaction_masks_secrets_in_capture() {
        let _guard = crate::config::env_lock().lock().expect("env lock");
        let root = temp_root("capture-redact");
        let config_dir = root.join("config");
        let data_dir = root.join("data");
        let original_config_dir = env::var_os("SEEKR_CONFIG_DIR");
        let original_data_dir = env::var_os("SEEKR_DATA_DIR");
        fs::create_dir_all(&config_dir).expect("config dir");
        fs::write(
            config_dir.join("config.toml"),
            "[privacy]\nredaction_enabled = true\n",
        )
        .expect("config file");

        unsafe {
            env::set_var("SEEKR_CONFIG_DIR", &config_dir);
            env::set_var("SEEKR_DATA_DIR", &data_dir);
        }

        let _ = dispatch(Cli {
            command: Some(Command::Capture(CaptureArgs {
                command_text: "export PASSWORD=mysecret123 && deploy".to_string(),
                cwd: "/tmp/project".to_string(),
                executed_at: 1_720_000_000,
                exit_code: 0,
                shell: None,
                duration_ms: None,
                hostname: None,
                git_repo: None,
                git_branch: None,
            })),
        })
        .expect("capture should succeed");

        let connection = crate::db::open(&data_dir.join("seekr.db")).expect("database should open");
        let records =
            crate::db::recent_command_records(&connection, 5).expect("records should load");

        restore_env("SEEKR_CONFIG_DIR", original_config_dir);
        restore_env("SEEKR_DATA_DIR", original_data_dir);

        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0].command_text,
            "export PASSWORD=<REDACTED> && deploy"
        );
    }

    #[test]
    fn noisy_commands_are_suppressed_in_capture() {
        let _guard = crate::config::env_lock().lock().expect("env lock");
        let root = temp_root("capture-noise");
        let config_dir = root.join("config");
        let data_dir = root.join("data");
        let original_config_dir = env::var_os("SEEKR_CONFIG_DIR");
        let original_data_dir = env::var_os("SEEKR_DATA_DIR");

        unsafe {
            env::set_var("SEEKR_CONFIG_DIR", &config_dir);
            env::set_var("SEEKR_DATA_DIR", &data_dir);
        }

        for noisy in ["ls -la", "cd /tmp", "pwd", "clear"] {
            let _ = dispatch(Cli {
                command: Some(Command::Capture(CaptureArgs {
                    command_text: noisy.to_string(),
                    cwd: "/tmp/project".to_string(),
                    executed_at: 1_720_000_000,
                    exit_code: 0,
                    shell: None,
                    duration_ms: None,
                    hostname: None,
                    git_repo: None,
                    git_branch: None,
                })),
            })
            .expect("capture should succeed");
        }

        let connection = crate::db::open(&data_dir.join("seekr.db")).expect("database should open");
        let records =
            crate::db::recent_command_records(&connection, 10).expect("records should load");

        restore_env("SEEKR_CONFIG_DIR", original_config_dir);
        restore_env("SEEKR_DATA_DIR", original_data_dir);

        assert!(records.is_empty());
    }

    #[test]
    fn custom_ignore_pattern_suppresses_capture() {
        let _guard = crate::config::env_lock().lock().expect("env lock");
        let root = temp_root("capture-custom-ignore");
        let config_dir = root.join("config");
        let data_dir = root.join("data");
        let original_config_dir = env::var_os("SEEKR_CONFIG_DIR");
        let original_data_dir = env::var_os("SEEKR_DATA_DIR");
        fs::create_dir_all(&config_dir).expect("config dir");
        fs::write(
            config_dir.join("config.toml"),
            "[privacy]\nignore_commands = [\"docker\", \"kubectl\"]\n",
        )
        .expect("config file");

        unsafe {
            env::set_var("SEEKR_CONFIG_DIR", &config_dir);
            env::set_var("SEEKR_DATA_DIR", &data_dir);
        }

        let _ = dispatch(Cli {
            command: Some(Command::Capture(CaptureArgs {
                command_text: "docker compose up".to_string(),
                cwd: "/tmp/project".to_string(),
                executed_at: 1_720_000_000,
                exit_code: 0,
                shell: None,
                duration_ms: None,
                hostname: None,
                git_repo: None,
                git_branch: None,
            })),
        })
        .expect("capture should succeed");

        let _ = dispatch(Cli {
            command: Some(Command::Capture(CaptureArgs {
                command_text: "cargo test".to_string(),
                cwd: "/tmp/project".to_string(),
                executed_at: 1_720_000_000,
                exit_code: 0,
                shell: None,
                duration_ms: None,
                hostname: None,
                git_repo: None,
                git_branch: None,
            })),
        })
        .expect("capture should succeed");

        let connection = crate::db::open(&data_dir.join("seekr.db")).expect("database should open");
        let records =
            crate::db::recent_command_records(&connection, 5).expect("records should load");

        restore_env("SEEKR_CONFIG_DIR", original_config_dir);
        restore_env("SEEKR_DATA_DIR", original_data_dir);

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].command_text, "cargo test");
    }

    #[test]
    fn import_with_redaction_enabled_masks_secrets_and_suppresses_noise() {
        let _guard = crate::config::env_lock().lock().expect("env lock");
        let root = temp_root("import-privacy");
        let config_dir = root.join("config");
        let data_dir = root.join("data");
        let original_config_dir = env::var_os("SEEKR_CONFIG_DIR");
        let original_data_dir = env::var_os("SEEKR_DATA_DIR");
        fs::create_dir_all(&config_dir).expect("config dir");
        fs::write(
            config_dir.join("config.toml"),
            "[privacy]\nredaction_enabled = true\n",
        )
        .expect("config file");

        unsafe {
            env::set_var("SEEKR_CONFIG_DIR", &config_dir);
            env::set_var("SEEKR_DATA_DIR", &data_dir);
        }

        let output = dispatch(Cli {
            command: Some(Command::Import {
                path: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/zsh_history_with_secrets.sample"),
            }),
        })
        .expect("import should succeed");
        let connection = crate::db::open(&data_dir.join("seekr.db")).expect("database should open");
        let records =
            crate::db::recent_command_records(&connection, 20).expect("records should load");

        restore_env("SEEKR_CONFIG_DIR", original_config_dir);
        restore_env("SEEKR_DATA_DIR", original_data_dir);

        // 11 entries, minus 2 noisy (ls -la, cd /tmp) = 9 inserted
        assert_eq!(
            output,
            "Import complete: inserted: 9, skipped: 2, failed: 0."
        );
        assert_eq!(records.len(), 9);

        assert!(records
            .iter()
            .any(|r| r.command_text == "export PASSWORD=<REDACTED>"));
        assert!(records
            .iter()
            .any(|r| r.command_text.contains("Bearer <REDACTED>")));
        assert!(records
            .iter()
            .any(|r| r.command_text == "API_KEY=<REDACTED> python script.py"));
        assert!(records
            .iter()
            .any(|r| r.command_text == "AWS_ACCESS_KEY_ID=<REDACTED> aws s3 ls"));
        assert!(records
            .iter()
            .any(|r| r.command_text == "AWS_SECRET_ACCESS_KEY=<REDACTED> aws s3 ls"));
        assert!(records
            .iter()
            .any(|r| r.command_text == "git push --token <REDACTED>"));
        assert!(records
            .iter()
            .any(|r| r.command_text == "npm config set _authToken=<REDACTED>"));
        assert!(records
            .iter()
            .any(|r| r.command_text == "plain safe command"));
        assert!(records.iter().any(|r| r.command_text == "gh pr status"));
    }

    #[test]
    fn import_with_defaults_preserves_raw_commands() {
        let _guard = crate::config::env_lock().lock().expect("env lock");
        let root = temp_root("import-defaults");
        let config_dir = root.join("config");
        let data_dir = root.join("data");
        let original_config_dir = env::var_os("SEEKR_CONFIG_DIR");
        let original_data_dir = env::var_os("SEEKR_DATA_DIR");

        unsafe {
            env::set_var("SEEKR_CONFIG_DIR", &config_dir);
            env::set_var("SEEKR_DATA_DIR", &data_dir);
        }

        let output = dispatch(Cli {
            command: Some(Command::Import {
                path: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/zsh_history_with_secrets.sample"),
            }),
        })
        .expect("import should succeed");
        let connection = crate::db::open(&data_dir.join("seekr.db")).expect("database should open");
        let records =
            crate::db::recent_command_records(&connection, 20).expect("records should load");

        restore_env("SEEKR_CONFIG_DIR", original_config_dir);
        restore_env("SEEKR_DATA_DIR", original_data_dir);

        // 11 entries, minus 2 noisy (ls -la, cd /tmp) = 9 inserted
        assert_eq!(
            output,
            "Import complete: inserted: 9, skipped: 2, failed: 0."
        );
        assert_eq!(records.len(), 9);

        assert!(records
            .iter()
            .any(|r| r.command_text == "export PASSWORD=mysecret123"));
        assert!(records
            .iter()
            .any(|r| r.command_text.contains("Bearer abcdef123456")));
        assert!(records
            .iter()
            .any(|r| r.command_text == "AWS_SECRET_ACCESS_KEY=wJalrXutnFEMI/K7MDENG aws s3 ls"));
        assert!(records
            .iter()
            .any(|r| r.command_text == "plain safe command"));
    }

    #[test]
    fn search_does_not_leak_redacted_secrets() {
        let _guard = crate::config::env_lock().lock().expect("env lock");
        let root = temp_root("search-redacted");
        let config_dir = root.join("config");
        let data_dir = root.join("data");
        let original_config_dir = env::var_os("SEEKR_CONFIG_DIR");
        let original_data_dir = env::var_os("SEEKR_DATA_DIR");
        fs::create_dir_all(&config_dir).expect("config dir");
        fs::write(
            config_dir.join("config.toml"),
            "[privacy]\nredaction_enabled = true\n",
        )
        .expect("config file");

        unsafe {
            env::set_var("SEEKR_CONFIG_DIR", &config_dir);
            env::set_var("SEEKR_DATA_DIR", &data_dir);
        }

        let _ = dispatch(Cli {
            command: Some(Command::Capture(CaptureArgs {
                command_text: "export PASSWORD=mysecret123".to_string(),
                cwd: "/tmp/project".to_string(),
                executed_at: 1_720_000_000,
                exit_code: 0,
                shell: None,
                duration_ms: None,
                hostname: None,
                git_repo: None,
                git_branch: None,
            })),
        })
        .expect("capture should succeed");

        let output = dispatch(Cli {
            command: Some(Command::Search(SearchArgs {
                query: "password".to_string(),
                limit: 10,
                ..SearchArgs::default()
            })),
        })
        .expect("search should succeed");

        restore_env("SEEKR_CONFIG_DIR", original_config_dir);
        restore_env("SEEKR_DATA_DIR", original_data_dir);

        assert!(!output.contains("mysecret123"));
        assert!(output.contains("PASSWORD=<REDACTED>"));
    }

    #[test]
    fn collapsed_search_shows_repeat_count_and_exit_summary() {
        let _guard = crate::config::env_lock().lock().expect("env lock");
        let root = temp_root("search-collapse");
        let config_dir = root.join("config");
        let data_dir = root.join("data");
        let original_config_dir = env::var_os("SEEKR_CONFIG_DIR");
        let original_data_dir = env::var_os("SEEKR_DATA_DIR");

        unsafe {
            env::set_var("SEEKR_CONFIG_DIR", &config_dir);
            env::set_var("SEEKR_DATA_DIR", &data_dir);
        }

        let connection = crate::db::open(&data_dir.join("seekr.db")).expect("database should open");
        for (executed_at, exit_code) in [(1_720_000_000, 0), (1_720_000_001, 0), (1_720_000_002, 1)]
        {
            let record = crate::db::CommandRecord::new(
                "cargo build".to_string(),
                "/tmp/project".to_string(),
                executed_at,
                exit_code,
                None,
                None,
                None,
                None,
                None,
            )
            .expect("record should validate");
            crate::db::insert_command_record(&connection, &record).expect("record should insert");
        }

        let output = dispatch(Cli {
            command: Some(Command::Search(SearchArgs {
                query: "cargo".to_string(),
                limit: 10,
                ..SearchArgs::default()
            })),
        })
        .expect("search should succeed");

        restore_env("SEEKR_CONFIG_DIR", original_config_dir);
        restore_env("SEEKR_DATA_DIR", original_data_dir);

        assert!(output.contains("cargo build"));
        assert!(output.contains("repeats: 3"));
        assert!(output.contains("exit: mixed (most recent: 1)"));
        assert!(output.contains("cwd: /tmp/project"));
        assert!(output.contains("last used:"));
        assert!(!output.contains("timestamp:"));
    }

    #[test]
    fn collapsed_search_same_exit_codes_show_single_value() {
        let _guard = crate::config::env_lock().lock().expect("env lock");
        let root = temp_root("search-collapse-same");
        let config_dir = root.join("config");
        let data_dir = root.join("data");
        let original_config_dir = env::var_os("SEEKR_CONFIG_DIR");
        let original_data_dir = env::var_os("SEEKR_DATA_DIR");

        unsafe {
            env::set_var("SEEKR_CONFIG_DIR", &config_dir);
            env::set_var("SEEKR_DATA_DIR", &data_dir);
        }

        let connection = crate::db::open(&data_dir.join("seekr.db")).expect("database should open");
        for executed_at in [1_720_000_000, 1_720_000_001] {
            let record = crate::db::CommandRecord::new(
                "git push".to_string(),
                "/tmp/project".to_string(),
                executed_at,
                0,
                None,
                None,
                None,
                None,
                None,
            )
            .expect("record should validate");
            crate::db::insert_command_record(&connection, &record).expect("record should insert");
        }

        let output = dispatch(Cli {
            command: Some(Command::Search(SearchArgs {
                query: "git".to_string(),
                limit: 10,
                ..SearchArgs::default()
            })),
        })
        .expect("search should succeed");

        restore_env("SEEKR_CONFIG_DIR", original_config_dir);
        restore_env("SEEKR_DATA_DIR", original_data_dir);

        assert!(output.contains("git push"));
        assert!(output.contains("repeats: 2"));
        assert!(output.contains("exit: 0"));
        assert!(!output.contains("mixed"));
    }

    #[test]
    fn collapsed_failed_shows_only_failed_commands() {
        let _guard = crate::config::env_lock().lock().expect("env lock");
        let root = temp_root("failed-collapse");
        let config_dir = root.join("config");
        let data_dir = root.join("data");
        let original_config_dir = env::var_os("SEEKR_CONFIG_DIR");
        let original_data_dir = env::var_os("SEEKR_DATA_DIR");

        unsafe {
            env::set_var("SEEKR_CONFIG_DIR", &config_dir);
            env::set_var("SEEKR_DATA_DIR", &data_dir);
        }

        let connection = crate::db::open(&data_dir.join("seekr.db")).expect("database should open");
        for (command_text, executed_at, exit_code) in [
            ("docker compose up", 1_720_000_000, 0),
            ("docker compose up", 1_720_000_001, 1),
            ("docker compose up", 1_720_000_002, 1),
        ] {
            let record = crate::db::CommandRecord::new(
                command_text.to_string(),
                "/tmp/project".to_string(),
                executed_at,
                exit_code,
                None,
                None,
                None,
                None,
                None,
            )
            .expect("record should validate");
            crate::db::insert_command_record(&connection, &record).expect("record should insert");
        }

        let output = dispatch(Cli {
            command: Some(Command::Failed),
        })
        .expect("failed should search");

        restore_env("SEEKR_CONFIG_DIR", original_config_dir);
        restore_env("SEEKR_DATA_DIR", original_data_dir);

        assert!(output.contains("docker compose up"));
        assert!(output.contains("repeats: 2"));
        assert!(output.contains("exit: 1"));
        assert!(!output.contains("exit: 0"));
    }

    #[test]
    fn collapsed_search_with_contextual_filters_still_collapses() {
        let _guard = crate::config::env_lock().lock().expect("env lock");
        let root = temp_root("collapse-filters");
        let config_dir = root.join("config");
        let data_dir = root.join("data");
        let original_config_dir = env::var_os("SEEKR_CONFIG_DIR");
        let original_data_dir = env::var_os("SEEKR_DATA_DIR");

        unsafe {
            env::set_var("SEEKR_CONFIG_DIR", &config_dir);
            env::set_var("SEEKR_DATA_DIR", &data_dir);
        }

        let connection = crate::db::open(&data_dir.join("seekr.db")).expect("database should open");
        for (command_text, cwd, executed_at, exit_code) in [
            ("npm install", "/tmp/app-a", 1_720_000_000, 0),
            ("npm install", "/tmp/app-a", 1_720_000_001, 0),
            ("npm install", "/tmp/app-b", 1_720_000_002, 0),
        ] {
            let record = crate::db::CommandRecord::new(
                command_text.to_string(),
                cwd.to_string(),
                executed_at,
                exit_code,
                None,
                None,
                None,
                None,
                None,
            )
            .expect("record should validate");
            crate::db::insert_command_record(&connection, &record).expect("record should insert");
        }

        let output = dispatch(Cli {
            command: Some(Command::Search(SearchArgs {
                query: "npm".to_string(),
                limit: 10,
                cwd: Some("/tmp/app-a".to_string()),
                ..SearchArgs::default()
            })),
        })
        .expect("search should succeed");

        restore_env("SEEKR_CONFIG_DIR", original_config_dir);
        restore_env("SEEKR_DATA_DIR", original_data_dir);

        assert!(output.contains("npm install"));
        assert!(output.contains("repeats: 2"));
        assert!(output.contains("cwd: /tmp/app-a"));
        assert!(!output.contains("/tmp/app-b"));
    }

    fn temp_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = env::temp_dir().join(format!("seekr-{label}-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).expect("temp dir");
        path
    }

    fn restore_env(name: &str, value: Option<OsString>) {
        unsafe {
            match value {
                Some(value) => env::set_var(name, value),
                None => env::remove_var(name),
            }
        }
    }
}
