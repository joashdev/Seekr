pub mod config;
pub mod db;

use clap::{Args, Parser, Subcommand};
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const CLI_ABOUT: &str =
    "Seekr is a local-first CLI and future TUI for recalling terminal commands.";
const CLI_AFTER_HELP: &str =
    "Planned alias: sk\n\nRunning `seekr` with no subcommand is reserved for the future TUI.";

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
        None => Ok(
            "Seekr TUI is not implemented yet. Network access remains disabled on this path."
                .to_string(),
        ),
        Some(Command::Search(args)) => search(args),
        Some(Command::Here) => Ok("`seekr here` is not implemented yet.".to_string()),
        Some(Command::Failed) => Ok("`seekr failed` is not implemented yet.".to_string()),
        Some(Command::Import { path }) => import(path),
        Some(Command::Capture(args)) => capture(args),
        Some(Command::Stats) => config::stats_report(),
    }
}

fn search(args: SearchArgs) -> io::Result<String> {
    let paths = config::ResolvedPaths::from_env()?;
    let connection = db::open(&paths.database_file())?;
    let records = db::search_command_records(&connection, &args.query, args.limit)?;

    if records.is_empty() {
        return Ok(format!("No local commands found for {:?}.", args.query));
    }

    Ok(records
        .iter()
        .map(|record| {
            let mut metadata = format!(
                "  cwd: {} | timestamp: {} | exit: {}",
                record.cwd, record.executed_at, record.exit_code
            );
            if let Some(repo) = &record.git_repo {
                metadata.push_str(&format!(" | repo: {repo}"));
            }
            if let Some(branch) = &record.git_branch {
                metadata.push_str(&format!(" | branch: {branch}"));
            }
            format!("{}\n{metadata}", record.command_text)
        })
        .collect::<Vec<_>>()
        .join("\n\n"))
}

fn capture(args: CaptureArgs) -> io::Result<String> {
    let record = db::CommandRecord::new(
        args.command_text,
        args.cwd,
        args.executed_at,
        args.exit_code,
        args.shell,
        args.duration_ms,
        args.hostname,
        args.git_repo,
        args.git_branch,
    )?;
    let paths = config::ResolvedPaths::from_env()?;
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

        let record = match db::CommandRecord::new(
            entry.command_text,
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
        dispatch, parse_zsh_history, CaptureArgs, Cli, Command, HistoryEntry, ParsedHistoryEntry,
        SearchArgs,
    };
    use clap::{CommandFactory, Parser};
    use std::env;
    use std::ffi::OsString;
    use std::fs;
    use std::io;
    use std::path::PathBuf;
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
                })),
            ),
            (
                vec!["seekr", "search", "docker", "--limit", "2"],
                Some(Command::Search(SearchArgs {
                    query: "docker".to_string(),
                    limit: 2,
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
    fn dispatches_distinct_placeholder_messages() {
        let cases = [
            (
                Cli { command: None },
                "Seekr TUI is not implemented yet. Network access remains disabled on this path.",
            ),
            (
                Cli {
                    command: Some(Command::Here),
                },
                "`seekr here` is not implemented yet.",
            ),
            (
                Cli {
                    command: Some(Command::Failed),
                },
                "`seekr failed` is not implemented yet.",
            ),
        ];

        for (cli, expected_message) in cases {
            assert_eq!(
                dispatch(cli).expect("placeholder command should dispatch"),
                expected_message
            );
        }
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

        assert!(output.contains("Seekr paths:"));
        assert!(output.contains("config dir:"));
        assert!(output.contains("data dir:"));
        assert!(output.contains("config file:"));
        assert!(output.contains("database:"));
        assert!(output.contains("redaction enabled: false"));
        assert!(output.contains("noisy command ignore candidates: ls, cd, pwd, clear"));
        assert!(data_dir.is_dir());
        assert!(data_dir.join("seekr.db").is_file());
    }

    #[test]
    fn includes_planned_alias_in_help() {
        let mut command = Cli::command();
        let mut help = Vec::new();

        command
            .write_long_help(&mut help)
            .expect("help rendering should succeed");

        let help = String::from_utf8(help).expect("help should be utf8");

        assert!(help.contains("Planned alias: sk"));
        assert!(help.contains("search"));
        assert!(help.contains("here"));
        assert!(help.contains("failed"));
        assert!(help.contains("import"));
        assert!(help.contains("stats"));
        assert!(!help
            .lines()
            .any(|line| line.trim_start().starts_with("capture")));
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
            })),
        })
        .expect("search should succeed");

        restore_env("SEEKR_CONFIG_DIR", original_config_dir);
        restore_env("SEEKR_DATA_DIR", original_data_dir);

        assert!(output.contains("docker compose up && cargo test"));
        assert!(output.contains("cwd: /tmp/project"));
        assert!(output.contains("timestamp: 1720000000"));
        assert!(output.contains("exit: 0"));
        assert!(output.contains("repo: seekr"));
        assert!(output.contains("branch: main"));
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
