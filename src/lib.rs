pub mod config;
pub mod db;

use clap::{Parser, Subcommand};
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
    /// Show local usage and index health statistics.
    Stats,
}

pub fn dispatch(cli: Cli) -> String {
    match cli.command {
        None => "Seekr TUI is not implemented yet. Network access remains disabled on this path."
            .to_string(),
        Some(Command::Search { query }) => {
            format!("`seekr search {query}` is not implemented yet.")
        }
        Some(Command::Here) => "`seekr here` is not implemented yet.".to_string(),
        Some(Command::Failed) => "`seekr failed` is not implemented yet.".to_string(),
        Some(Command::Import { path }) => {
            format!("`seekr import {}` is not implemented yet.", path.display())
        }
        Some(Command::Stats) => config::stats_report()
            .unwrap_or_else(|error| format!("Failed to load Seekr config paths: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::{dispatch, Cli, Command};
    use clap::{CommandFactory, Parser};
    use std::env;
    use std::ffi::OsString;
    use std::fs;
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
            assert_eq!(dispatch(cli), expected_message);
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
        });

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
