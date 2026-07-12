pub mod config;
pub mod db;

use clap::{Args, Parser, Subcommand};
use std::env;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

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
        Some(Command::Here) => here(),
        Some(Command::Failed) => failed(),
        Some(Command::Import { path }) => Ok(format!(
            "`seekr import {}` is not implemented yet.",
            path.display()
        )),
        Some(Command::Capture(args)) => capture(args),
        Some(Command::Stats) => config::stats_report(),
    }
}

fn search(args: SearchArgs) -> io::Result<String> {
    let paths = config::ResolvedPaths::from_env()?;
    let connection = db::open(&paths.database_file())?;
    let records =
        db::filtered_command_records(&connection, Some(&args.query), &args.filters(), args.limit)?;

    render_records(
        records,
        format!("No local commands found for {:?}.", args.query),
    )
}

fn here() -> io::Result<String> {
    let cwd = env::current_dir()?;
    let paths = config::ResolvedPaths::from_env()?;
    let connection = db::open(&paths.database_file())?;
    let mut records = db::filtered_command_records(
        &connection,
        None,
        &db::SearchFilters {
            cwd: Some(cwd.to_string_lossy().into_owned()),
            ..db::SearchFilters::default()
        },
        10,
    )?;

    if records.is_empty() {
        if let Some(repo) = git_context(&cwd).repo {
            records = db::filtered_command_records(
                &connection,
                None,
                &db::SearchFilters {
                    repo: Some(repo),
                    ..db::SearchFilters::default()
                },
                10,
            )?;
        }
    }

    render_records(
        records,
        "No local commands found for the current context.".to_string(),
    )
}

fn failed() -> io::Result<String> {
    let paths = config::ResolvedPaths::from_env()?;
    let connection = db::open(&paths.database_file())?;
    let records = db::filtered_command_records(
        &connection,
        None,
        &db::SearchFilters {
            failed: Some(true),
            ..db::SearchFilters::default()
        },
        10,
    )?;

    render_records(records, "No local failed commands found.".to_string())
}

fn render_records(records: Vec<db::CommandRecord>, empty_message: String) -> io::Result<String> {
    if records.is_empty() {
        return Ok(empty_message);
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
    use super::{dispatch, git_context, CaptureArgs, Cli, Command, ProcessCommand, SearchArgs};
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
    fn dispatches_remaining_placeholder_messages() {
        let cases = [
            (
                Cli { command: None },
                "Seekr TUI is not implemented yet. Network access remains disabled on this path.",
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
        assert!(output.contains("timestamp: 1720000000"));
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
