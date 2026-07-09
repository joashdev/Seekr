pub mod config;
pub mod db;

use clap::{Args, Parser, Subcommand};
use std::io;
use std::path::PathBuf;

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
    Search {
        /// Free-text query to look up.
        query: String,
    },
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
        Some(Command::Search { query }) => {
            Ok(format!("`seekr search {query}` is not implemented yet."))
        }
        Some(Command::Here) => Ok("`seekr here` is not implemented yet.".to_string()),
        Some(Command::Failed) => Ok("`seekr failed` is not implemented yet.".to_string()),
        Some(Command::Import { path }) => Ok(format!(
            "`seekr import {}` is not implemented yet.",
            path.display()
        )),
        Some(Command::Capture(args)) => capture(args),
        Some(Command::Stats) => config::stats_report(),
    }
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

#[cfg(test)]
mod tests {
    use super::{dispatch, CaptureArgs, Cli, Command};
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
                Some(Command::Search {
                    query: "docker".to_string(),
                }),
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
                    command: Some(Command::Search {
                        query: "docker".to_string(),
                    }),
                },
                "`seekr search docker` is not implemented yet.",
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
            (
                Cli {
                    command: Some(Command::Import {
                        path: PathBuf::from("~/.zsh_history"),
                    }),
                },
                "`seekr import ~/.zsh_history` is not implemented yet.",
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
