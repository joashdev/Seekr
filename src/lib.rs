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
        Some(Command::Stats) => "`seekr stats` is not implemented yet.".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{dispatch, Cli, Command};
    use clap::{CommandFactory, Parser};
    use std::path::PathBuf;

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
            (
                Cli {
                    command: Some(Command::Stats),
                },
                "`seekr stats` is not implemented yet.",
            ),
        ];

        for (cli, expected_message) in cases {
            assert_eq!(dispatch(cli), expected_message);
        }
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
}
