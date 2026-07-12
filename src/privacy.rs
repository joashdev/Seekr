use crate::config::PrivacyConfig;
use regex::Regex;
use std::sync::LazyLock;

static REDACTION_PATTERNS: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
    let specs: &[(&str, &str)] = &[
        (
            r"(?i)(?:\b|(_))(password|passwd|pass|pwd|secret|token|access_token|refresh_token|api[_-]?key|apikey|api[_-]?secret)\s*=\s*\S+",
            "${1}${2}=<REDACTED>",
        ),
        (
            r"(?i)--(password|passwd|pass|secret|token|api[_-]?key|apikey)\s+\S+",
            "--$1 <REDACTED>",
        ),
        (r#"(?i)(bearer\s+)[^\s"']+"#, "${1}<REDACTED>"),
    ];

    specs
        .iter()
        .map(|(pat, rep)| {
            (
                Regex::new(pat).expect("redaction regex should compile"),
                *rep,
            )
        })
        .collect()
});

/// Returns `None` if the command should be ignored (matches an ignore pattern).
/// Returns `Some(text)` otherwise — redacted if privacy is enabled, raw if not.
pub fn filter_command(command_text: &str, config: &PrivacyConfig) -> Option<String> {
    let first_token = command_text.split_whitespace().next().unwrap_or("");

    // ponytail: simple first-token match covers ls, cd, pwd, clear, and all user patterns.
    if config
        .ignore_commands
        .iter()
        .any(|pattern| first_token == pattern.as_str())
    {
        return None;
    }

    let text = if config.redaction_enabled {
        redact(command_text)
    } else {
        command_text.to_string()
    };

    Some(text)
}

fn redact(command: &str) -> String {
    let mut result = command.to_string();

    for (regex, replacement) in REDACTION_PATTERNS.iter() {
        result = regex.replace_all(&result, *replacement).to_string();
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PrivacyConfig;

    fn default_privacy() -> PrivacyConfig {
        PrivacyConfig::default()
    }

    fn redaction_enabled_privacy() -> PrivacyConfig {
        PrivacyConfig {
            redaction_enabled: true,
            ..PrivacyConfig::default()
        }
    }

    #[test]
    fn default_config_preserves_raw_commands() {
        let config = default_privacy();

        assert_eq!(
            filter_command("cargo test --verbose", &config),
            Some("cargo test --verbose".to_string())
        );
        assert_eq!(
            filter_command("git push origin main", &config),
            Some("git push origin main".to_string())
        );
    }

    #[test]
    fn default_config_suppresses_noisy_commands() {
        let config = default_privacy();

        for noisy in ["ls", "cd", "pwd", "clear"] {
            assert_eq!(filter_command(noisy, &config), None);
        }
        for noisy_with_args in ["ls -la", "cd /tmp", "pwd -P", "clear -x"] {
            assert_eq!(filter_command(noisy_with_args, &config), None);
        }
    }

    #[test]
    fn custom_ignore_patterns_are_respected() {
        let config = PrivacyConfig {
            redaction_enabled: false,
            ignore_commands: vec!["docker".to_string(), "kubectl".to_string()],
        };

        assert_eq!(filter_command("docker compose up", &config), None);
        assert_eq!(filter_command("kubectl get pods", &config), None);
        assert_eq!(
            filter_command("cargo test", &config),
            Some("cargo test".to_string())
        );
    }

    #[test]
    fn redaction_masks_password_assignments() {
        let config = redaction_enabled_privacy();

        assert_eq!(
            filter_command("export PASSWORD=mysecret123", &config),
            Some("export PASSWORD=<REDACTED>".to_string())
        );
        assert_eq!(
            filter_command("MYSQL_PWD=secret mysql -u root", &config),
            Some("MYSQL_PWD=<REDACTED> mysql -u root".to_string())
        );
    }

    #[test]
    fn redaction_masks_token_assignments() {
        let config = redaction_enabled_privacy();

        assert_eq!(
            filter_command("GITHUB_TOKEN=ghp_abc123 gh pr list", &config),
            Some("GITHUB_TOKEN=<REDACTED> gh pr list".to_string())
        );
        assert_eq!(
            filter_command("export TOKEN=abc123def456", &config),
            Some("export TOKEN=<REDACTED>".to_string())
        );
        assert_eq!(
            filter_command(
                "ACCESS_TOKEN=secret123 curl https://api.example.com",
                &config
            ),
            Some("ACCESS_TOKEN=<REDACTED> curl https://api.example.com".to_string())
        );
    }

    #[test]
    fn redaction_masks_api_key_assignments() {
        let config = redaction_enabled_privacy();

        assert_eq!(
            filter_command("API_KEY=sk-abc123def456 python script.py", &config),
            Some("API_KEY=<REDACTED> python script.py".to_string())
        );
        assert_eq!(
            filter_command("apikey=abc123def456 curl api.example.com", &config),
            Some("apikey=<REDACTED> curl api.example.com".to_string())
        );
        assert_eq!(
            filter_command("api_secret=shh-dont-tell node server.js", &config),
            Some("api_secret=<REDACTED> node server.js".to_string())
        );
    }

    #[test]
    fn redaction_masks_bearer_tokens() {
        let config = redaction_enabled_privacy();

        assert_eq!(
            filter_command(
                r#"curl -H "Authorization: Bearer abcdef123456" https://api.example.com"#,
                &config,
            ),
            Some(
                r#"curl -H "Authorization: Bearer <REDACTED>" https://api.example.com"#.to_string()
            )
        );
    }

    #[test]
    fn redaction_masks_cli_flag_secrets() {
        let config = redaction_enabled_privacy();

        assert_eq!(
            filter_command("mysql --password secret123 -u root", &config),
            Some("mysql --password <REDACTED> -u root".to_string())
        );
        assert_eq!(
            filter_command("gh auth login --token ghp_abc123", &config),
            Some("gh auth login --token <REDACTED>".to_string())
        );
        assert_eq!(
            filter_command("deploy --api-key abc123def456 --env prod", &config),
            Some("deploy --api-key <REDACTED> --env prod".to_string())
        );
    }

    #[test]
    fn redaction_disabled_preserves_secrets_in_raw_text() {
        let config = default_privacy();
        let command = "export PASSWORD=mysecret123 && deploy --token ghp_abc";

        assert_eq!(filter_command(command, &config), Some(command.to_string()));
    }

    #[test]
    fn redaction_handles_multiple_secrets_in_one_command() {
        let config = redaction_enabled_privacy();

        assert_eq!(
            filter_command(
                "PASSWORD=foo TOKEN=bar curl -H 'Authorization: Bearer baz' https://api.example.com",
                &config,
            ),
            Some(
                "PASSWORD=<REDACTED> TOKEN=<REDACTED> curl -H 'Authorization: Bearer <REDACTED>' https://api.example.com".to_string()
            )
        );
    }
}
