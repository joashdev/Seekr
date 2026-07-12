use crate::config::{self, ResolvedPaths};
use crate::db;
use std::io;

pub fn stats_report() -> io::Result<String> {
    let paths = ResolvedPaths::from_env()?;
    let config = config::load(&paths)?;
    let database_path = paths.database_file();

    let connection = db::open(&database_path).ok();

    let mut output = String::from("Seekr Stats\n===========\n\n");

    output.push_str("Database:\n");
    output.push_str(&format!("  Path: {}\n", database_path.display()));

    if connection.is_some() {
        if let Ok(metadata) = std::fs::metadata(&database_path) {
            let size = metadata.len();
            let size_str = if size < 1024 {
                format!("{size} B")
            } else if size < 1024 * 1024 {
                format!("{:.1} KB", size as f64 / 1024.0)
            } else {
                format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
            };
            output.push_str(&format!("  Size: {size_str}\n"));
        }
    }

    output.push_str("\nCommands:\n");

    if let Some(ref conn) = connection {
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM commands", [], |row| row.get(0))
            .unwrap_or(0);
        output.push_str(&format!("  Total stored: {total}\n"));

        let fts_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM commands_fts", [], |row| row.get(0))
            .unwrap_or(0);
        output.push_str(&format!("  Indexed (FTS): {fts_count}\n"));

        let failed: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM commands WHERE exit_code != 0",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        output.push_str(&format!("  Failed commands: {failed}\n"));

        let (earliest, latest): (Option<i64>, Option<i64>) = conn
            .query_row(
                "SELECT MIN(executed_at), MAX(executed_at) FROM commands",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap_or((None, None));

        match (earliest, latest) {
            (Some(e), Some(l)) => output.push_str(&format!("  Time range: {e} to {l}\n")),
            _ => output.push_str("  Time range: n/a\n"),
        }
    } else {
        output.push_str(
            "  Total stored: 0 (database unavailable)\n  \
             Indexed (FTS): 0\n  \
             Failed commands: 0\n  \
             Time range: n/a\n",
        );
    }

    output.push_str("\nCollapse:\n");

    if let Some(ref conn) = connection {
        let unique_groups: i64 = conn
            .query_row(
                "SELECT COUNT(DISTINCT normalized_text) FROM commands",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        output.push_str(&format!("  Unique groups: {unique_groups}\n"));
    } else {
        output.push_str("  Unique groups: n/a\n");
    }

    output.push_str("\nPrivacy:\n");
    output.push_str(&format!(
        "  Redaction: {}\n",
        if config.privacy.redaction_enabled {
            "enabled"
        } else {
            "disabled"
        }
    ));
    output.push_str(&format!(
        "  Ignore rules: {}\n",
        config.privacy.ignore_commands.len()
    ));

    output.push_str("\nHealth:\n");

    if let Some(ref conn) = connection {
        output.push_str("  Database: ok\n");

        let fts_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'commands_fts'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|count| count > 0)
            .unwrap_or(false);

        if fts_exists {
            output.push_str("  FTS index: ok\n");
        } else {
            output.push_str("  FTS index: missing\n");
        }

        let schema_version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap_or(-1);

        if schema_version == db::SCHEMA_VERSION {
            output.push_str("  Migrations: up to date\n");
        } else {
            output.push_str(&format!(
                "  Migrations: version {schema_version} (expected {})\n",
                db::SCHEMA_VERSION
            ));
        }
    } else {
        output.push_str("  Database: unavailable\n");
        output.push_str("  FTS index: unknown\n");
        output.push_str("  Migrations: unknown\n");
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{self, CommandRecord};
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn stats_with_known_data_reports_counts_and_time_range() {
        let _guard = crate::config::env_lock().lock().expect("env lock");
        let root = temp_root("stats-known");
        let config_dir = root.join("config");
        let data_dir = root.join("data");
        let original_config_dir = env::var_os("SEEKR_CONFIG_DIR");
        let original_data_dir = env::var_os("SEEKR_DATA_DIR");

        unsafe {
            env::set_var("SEEKR_CONFIG_DIR", &config_dir);
            env::set_var("SEEKR_DATA_DIR", &data_dir);
        }

        let connection = db::open(&data_dir.join("seekr.db")).expect("database should initialize");
        for (command_text, executed_at, exit_code) in [
            ("cargo test", 1_720_000_000, 0),
            ("cargo test", 1_720_000_001, 0),
            ("docker compose up", 1_720_000_002, 1),
        ] {
            let record = CommandRecord::new(
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
            db::insert_command_record(&connection, &record).expect("record should insert");
        }

        let output = stats_report().expect("stats should render");

        restore_env("SEEKR_CONFIG_DIR", original_config_dir);
        restore_env("SEEKR_DATA_DIR", original_data_dir);

        assert!(output.contains("Seekr Stats"));
        assert!(output.contains("Database:"));
        assert!(output.contains(&format!("Path: {}", data_dir.join("seekr.db").display())));
        assert!(output.contains("Commands:"));
        assert!(output.contains("Total stored: 3"));
        assert!(output.contains("Indexed (FTS): 3"));
        assert!(output.contains("Failed commands: 1"));
        assert!(output.contains("Time range: 1720000000 to 1720000002"));
        assert!(output.contains("Collapse:"));
        assert!(output.contains("Unique groups: 2"));
        assert!(output.contains("Privacy:"));
        assert!(output.contains("Redaction: disabled"));
        assert!(output.contains("Ignore rules: 4"));
        assert!(output.contains("Health:"));
        assert!(output.contains("Database: ok"));
        assert!(output.contains("FTS index: ok"));
        assert!(output.contains("Migrations: up to date"));
    }

    #[test]
    fn stats_handles_empty_database_gracefully() {
        let _guard = crate::config::env_lock().lock().expect("env lock");
        let root = temp_root("stats-empty");
        let config_dir = root.join("config");
        let data_dir = root.join("data");
        let original_config_dir = env::var_os("SEEKR_CONFIG_DIR");
        let original_data_dir = env::var_os("SEEKR_DATA_DIR");

        unsafe {
            env::set_var("SEEKR_CONFIG_DIR", &config_dir);
            env::set_var("SEEKR_DATA_DIR", &data_dir);
        }

        let output = stats_report().expect("stats should render");

        restore_env("SEEKR_CONFIG_DIR", original_config_dir);
        restore_env("SEEKR_DATA_DIR", original_data_dir);

        assert!(output.contains("Total stored: 0"));
        assert!(output.contains("Indexed (FTS): 0"));
        assert!(output.contains("Failed commands: 0"));
        assert!(output.contains("Time range: n/a"));
        assert!(output.contains("Unique groups: 0"));
        assert!(output.contains("Database: ok"));
        assert!(output.contains("FTS index: ok"));
    }

    #[test]
    fn stats_reports_privacy_state_without_secret_values() {
        let _guard = crate::config::env_lock().lock().expect("env lock");
        let root = temp_root("stats-privacy");
        let config_dir = root.join("config");
        let data_dir = root.join("data");
        let original_config_dir = env::var_os("SEEKR_CONFIG_DIR");
        let original_data_dir = env::var_os("SEEKR_DATA_DIR");

        fs::create_dir_all(&config_dir).expect("config dir");
        fs::write(
            config_dir.join("config.toml"),
            "[privacy]\nredaction_enabled = true\nignore_commands = [\"docker\", \"kubectl\"]\n",
        )
        .expect("config file");

        unsafe {
            env::set_var("SEEKR_CONFIG_DIR", &config_dir);
            env::set_var("SEEKR_DATA_DIR", &data_dir);
        }

        let output = stats_report().expect("stats should render");

        restore_env("SEEKR_CONFIG_DIR", original_config_dir);
        restore_env("SEEKR_DATA_DIR", original_data_dir);

        assert!(output.contains("Redaction: enabled"));
        assert!(output.contains("Ignore rules: 2"));
        assert!(!output.contains("docker"));
        assert!(!output.contains("kubectl"));
    }

    #[test]
    fn stats_redaction_reports_enabled_disabled_only() {
        let _guard = crate::config::env_lock().lock().expect("env lock");
        let root = temp_root("stats-redaction");
        let config_dir = root.join("config");
        let data_dir = root.join("data");
        let original_config_dir = env::var_os("SEEKR_CONFIG_DIR");
        let original_data_dir = env::var_os("SEEKR_DATA_DIR");

        for (enabled, expected) in [(false, "disabled"), (true, "enabled")] {
            let config_str = format!("[privacy]\nredaction_enabled = {enabled}\n");
            fs::create_dir_all(&config_dir).expect("config dir");
            fs::write(config_dir.join("config.toml"), config_str).expect("config file");

            unsafe {
                env::set_var("SEEKR_CONFIG_DIR", &config_dir);
                env::set_var("SEEKR_DATA_DIR", &data_dir);
            }

            let output = stats_report().expect("stats should render");

            restore_env("SEEKR_CONFIG_DIR", original_config_dir.clone());
            restore_env("SEEKR_DATA_DIR", original_data_dir.clone());

            assert!(output.contains(&format!("Redaction: {expected}")));
            assert!(!output.contains("<REDACTED>"));
        }
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

    fn restore_env(name: &str, value: Option<std::ffi::OsString>) {
        unsafe {
            match value {
                Some(value) => env::set_var(name, value),
                None => env::remove_var(name),
            }
        }
    }
}
