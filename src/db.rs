use rusqlite::{params, types::Value, Connection};
use std::fs;
use std::io;
use std::path::Path;

const SCHEMA_VERSION: i64 = 1;
const MIGRATION_001: &str = "
CREATE TABLE IF NOT EXISTS commands (
    id INTEGER PRIMARY KEY,
    command_text TEXT NOT NULL,
    normalized_text TEXT NOT NULL,
    cwd TEXT NOT NULL,
    executed_at INTEGER NOT NULL,
    exit_code INTEGER NOT NULL,
    shell TEXT,
    duration_ms INTEGER,
    hostname TEXT,
    git_repo TEXT,
    git_branch TEXT
);

CREATE VIRTUAL TABLE IF NOT EXISTS commands_fts USING fts5(
    command_text,
    normalized_text,
    content='commands',
    content_rowid='id'
);

CREATE TRIGGER IF NOT EXISTS commands_ai AFTER INSERT ON commands BEGIN
    INSERT INTO commands_fts(rowid, command_text, normalized_text)
    VALUES (new.id, new.command_text, new.normalized_text);
END;

CREATE TRIGGER IF NOT EXISTS commands_ad AFTER DELETE ON commands BEGIN
    INSERT INTO commands_fts(commands_fts, rowid, command_text, normalized_text)
    VALUES ('delete', old.id, old.command_text, old.normalized_text);
END;

CREATE TRIGGER IF NOT EXISTS commands_au AFTER UPDATE ON commands BEGIN
    INSERT INTO commands_fts(commands_fts, rowid, command_text, normalized_text)
    VALUES ('delete', old.id, old.command_text, old.normalized_text);
    INSERT INTO commands_fts(rowid, command_text, normalized_text)
    VALUES (new.id, new.command_text, new.normalized_text);
END;

CREATE INDEX IF NOT EXISTS idx_commands_cwd ON commands (cwd);
CREATE INDEX IF NOT EXISTS idx_commands_git_repo ON commands (git_repo);
CREATE INDEX IF NOT EXISTS idx_commands_git_branch ON commands (git_branch);
CREATE INDEX IF NOT EXISTS idx_commands_executed_at ON commands (executed_at);
CREATE INDEX IF NOT EXISTS idx_commands_exit_code ON commands (exit_code);

INSERT INTO commands_fts(commands_fts) VALUES ('rebuild');
";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRecord {
    pub command_text: String,
    pub normalized_text: String,
    pub cwd: String,
    pub executed_at: i64,
    pub exit_code: i64,
    pub shell: Option<String>,
    pub duration_ms: Option<i64>,
    pub hostname: Option<String>,
    pub git_repo: Option<String>,
    pub git_branch: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchFilters {
    pub cwd: Option<String>,
    pub repo: Option<String>,
    pub branch: Option<String>,
    pub failed: Option<bool>,
    pub since: Option<i64>,
    pub before: Option<i64>,
}

impl CommandRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        command_text: String,
        cwd: String,
        executed_at: i64,
        exit_code: i64,
        shell: Option<String>,
        duration_ms: Option<i64>,
        hostname: Option<String>,
        git_repo: Option<String>,
        git_branch: Option<String>,
    ) -> io::Result<Self> {
        let normalized_text = normalize_command_text(&command_text);
        if command_text.trim().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "command text cannot be empty",
            ));
        }
        if executed_at < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "executed_at cannot be negative",
            ));
        }
        if exit_code < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "exit_code cannot be negative",
            ));
        }
        if duration_ms.is_some_and(|duration| duration < 0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "duration_ms cannot be negative",
            ));
        }

        Ok(Self {
            command_text,
            normalized_text,
            cwd,
            executed_at,
            exit_code,
            shell,
            duration_ms,
            hostname,
            git_repo,
            git_branch,
        })
    }
}

pub fn open(path: &Path) -> io::Result<Connection> {
    ensure_parent_dir(path)?;

    let connection = Connection::open(path).map_err(io::Error::other)?;
    run_migrations(&connection).map_err(io::Error::other)?;

    Ok(connection)
}

pub fn run_migrations(connection: &Connection) -> rusqlite::Result<()> {
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;

    if version >= SCHEMA_VERSION {
        return Ok(());
    }

    connection.execute_batch("BEGIN IMMEDIATE;")?;

    let migration_result = (|| -> rusqlite::Result<()> {
        if version < 1 {
            connection.execute_batch(MIGRATION_001)?;
            connection.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }

        Ok(())
    })();

    match migration_result {
        Ok(()) => {
            connection.execute_batch("COMMIT;")?;
            Ok(())
        }
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK;");
            Err(error)
        }
    }
}

pub fn insert_command_record(connection: &Connection, record: &CommandRecord) -> io::Result<i64> {
    connection
        .execute(
            "INSERT INTO commands (
                command_text,
                normalized_text,
                cwd,
                executed_at,
                exit_code,
                shell,
                duration_ms,
                hostname,
                git_repo,
                git_branch
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                &record.command_text,
                &record.normalized_text,
                &record.cwd,
                record.executed_at,
                record.exit_code,
                &record.shell,
                record.duration_ms,
                &record.hostname,
                &record.git_repo,
                &record.git_branch,
            ],
        )
        .map_err(io::Error::other)?;

    Ok(connection.last_insert_rowid())
}

pub fn recent_command_records(
    connection: &Connection,
    limit: usize,
) -> io::Result<Vec<CommandRecord>> {
    let mut statement = connection
        .prepare(
            "SELECT
                command_text,
                normalized_text,
                cwd,
                executed_at,
                exit_code,
                shell,
                duration_ms,
                hostname,
                git_repo,
                git_branch
             FROM commands
             ORDER BY executed_at DESC, id DESC
             LIMIT ?1",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([limit as i64], |row| {
            Ok(CommandRecord {
                command_text: row.get(0)?,
                normalized_text: row.get(1)?,
                cwd: row.get(2)?,
                executed_at: row.get(3)?,
                exit_code: row.get(4)?,
                shell: row.get(5)?,
                duration_ms: row.get(6)?,
                hostname: row.get(7)?,
                git_repo: row.get(8)?,
                git_branch: row.get(9)?,
            })
        })
        .map_err(io::Error::other)?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(io::Error::other)
}

pub fn search_command_records(
    connection: &Connection,
    query: &str,
    limit: usize,
) -> io::Result<Vec<CommandRecord>> {
    filtered_command_records(connection, Some(query), &SearchFilters::default(), limit)
}

pub fn filtered_command_records(
    connection: &Connection,
    query: Option<&str>,
    filters: &SearchFilters,
    limit: usize,
) -> io::Result<Vec<CommandRecord>> {
    let limit = i64::try_from(limit)
        .ok()
        .filter(|limit| (1..=100).contains(limit))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "limit must be between 1 and 100",
            )
        })?;
    if filters.since.is_some_and(|since| since < 0)
        || filters.before.is_some_and(|before| before < 0)
        || matches!((filters.since, filters.before), (Some(since), Some(before)) if since > before)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid time window",
        ));
    }

    let query_provided = query.is_some();
    let query = query
        .map(normalize_command_text)
        .filter(|query| !query.is_empty())
        .map(|query| {
            query
                .split_whitespace()
                .map(|term| format!("\"{term}\""))
                .collect::<Vec<_>>()
                .join(" ")
        });
    if query_provided && query.is_none() {
        return Ok(Vec::new());
    }
    let has_query = query.is_some();

    let mut sql = String::from(
        "SELECT
            commands.command_text,
            commands.normalized_text,
            commands.cwd,
            commands.executed_at,
            commands.exit_code,
            commands.shell,
            commands.duration_ms,
            commands.hostname,
            commands.git_repo,
            commands.git_branch
         FROM ",
    );
    if has_query {
        sql.push_str("commands_fts JOIN commands ON commands.id = commands_fts.rowid");
    } else {
        sql.push_str("commands");
    }

    let mut clauses: Vec<String> = Vec::new();
    let mut values = Vec::new();
    if let Some(query) = query {
        clauses.push("commands_fts MATCH ?".to_string());
        values.push(Value::Text(query));
    }
    for (column, value) in [
        ("commands.cwd", filters.cwd.as_ref()),
        ("commands.git_repo", filters.repo.as_ref()),
        ("commands.git_branch", filters.branch.as_ref()),
    ] {
        if let Some(value) = value {
            clauses.push(format!("{column} = ?"));
            values.push(Value::Text(value.clone()));
        }
    }
    if let Some(failed) = filters.failed {
        clauses.push(if failed {
            "commands.exit_code != 0".to_string()
        } else {
            "commands.exit_code = 0".to_string()
        });
    }
    if let Some(since) = filters.since {
        clauses.push("commands.executed_at >= ?".to_string());
        values.push(Value::Integer(since));
    }
    if let Some(before) = filters.before {
        clauses.push("commands.executed_at <= ?".to_string());
        values.push(Value::Integer(before));
    }
    if !clauses.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&clauses.join(" AND "));
    }
    sql.push_str(if has_query {
        " ORDER BY bm25(commands_fts), commands.executed_at DESC, commands.id DESC LIMIT ?"
    } else {
        " ORDER BY commands.executed_at DESC, commands.id DESC LIMIT ?"
    });
    values.push(Value::Integer(limit));

    let mut statement = connection.prepare(&sql).map_err(io::Error::other)?;
    let rows = statement
        .query_map(rusqlite::params_from_iter(values), |row| {
            Ok(CommandRecord {
                command_text: row.get(0)?,
                normalized_text: row.get(1)?,
                cwd: row.get(2)?,
                executed_at: row.get(3)?,
                exit_code: row.get(4)?,
                shell: row.get(5)?,
                duration_ms: row.get(6)?,
                hostname: row.get(7)?,
                git_repo: row.get(8)?,
                git_branch: row.get(9)?,
            })
        })
        .map_err(io::Error::other)?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(io::Error::other)
}

fn ensure_parent_dir(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent().filter(|path| !path.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }

    Ok(())
}

fn normalize_command_text(command_text: &str) -> String {
    command_text
        .chars()
        .map(|ch| match ch {
            'a'..='z' | '0'..='9' | '-' | '_' | '/' | '.' => ch,
            'A'..='Z' => ch.to_ascii_lowercase(),
            _ if ch.is_alphanumeric() => ch.to_lowercase().next().unwrap_or(ch),
            _ => ' ',
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::{
        filtered_command_records, insert_command_record, open, recent_command_records,
        search_command_records, CommandRecord, SearchFilters,
    };
    use rusqlite::{params, Connection};
    use std::collections::HashSet;
    use std::env;
    use std::fs;
    use std::io;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn opening_database_creates_parent_dirs_and_schema() {
        let root = temp_root("db-schema");
        let database_path = root.join("nested/data/seekr.db");

        let connection = open(&database_path).expect("database should initialize");

        assert!(database_path.exists());
        assert!(database_path.parent().expect("db parent").is_dir());

        let tables = names_for(&connection, "table");

        assert!(tables.contains("commands"));
        assert!(tables.contains("commands_fts"));

        let indexes = names_for(&connection, "index");

        for index in [
            "idx_commands_cwd",
            "idx_commands_git_repo",
            "idx_commands_git_branch",
            "idx_commands_executed_at",
            "idx_commands_exit_code",
        ] {
            assert!(indexes.contains(index), "missing index {index}");
        }
    }

    #[test]
    fn migrations_are_idempotent_and_fts_queries_use_normalized_text() {
        let root = temp_root("db-fts");
        let database_path = root.join("seekr.db");
        let connection = open(&database_path).expect("database should initialize");

        super::run_migrations(&connection).expect("migration rerun should succeed");
        super::run_migrations(&connection).expect("migration rerun should stay idempotent");

        let version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .expect("schema version");
        assert_eq!(version, 1);

        connection
            .execute(
                "INSERT INTO commands (
                    command_text,
                    normalized_text,
                    cwd,
                    executed_at,
                    exit_code,
                    shell,
                    duration_ms,
                    hostname,
                    git_repo,
                    git_branch
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    "gh pr checkout 123 && cargo test",
                    "gh pr checkout 123 cargo test",
                    "/tmp/project",
                    1_720_000_000_i64,
                    0_i64,
                    "zsh",
                    250_i64,
                    "seekr-host",
                    "seekr",
                    "feat/task-03"
                ],
            )
            .expect("command insert should succeed");

        let (command_text, normalized_text): (String, String) = connection
            .query_row(
                "SELECT command_text, normalized_text FROM commands LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("command row");

        assert_eq!(command_text, "gh pr checkout 123 && cargo test");
        assert_eq!(normalized_text, "gh pr checkout 123 cargo test");
        assert_ne!(command_text, normalized_text);

        let matches: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM commands_fts WHERE commands_fts MATCH 'normalized_text: checkout'",
                [],
                |row| row.get(0),
            )
            .expect("fts query should succeed");

        assert_eq!(matches, 1);
    }

    #[test]
    fn command_record_insert_and_recent_fetch_preserve_raw_and_normalized_text() {
        let root = temp_root("db-store");
        let database_path = root.join("seekr.db");
        let connection = open(&database_path).expect("database should initialize");
        let record = CommandRecord::new(
            "gh pr checkout 123 && cargo   test".to_string(),
            "/tmp/project".to_string(),
            1_720_000_001,
            0,
            Some("zsh".to_string()),
            Some(250),
            Some("seekr-host".to_string()),
            Some("seekr".to_string()),
            Some("feat/task-04".to_string()),
        )
        .expect("record should validate");

        insert_command_record(&connection, &record).expect("insert should succeed");

        let records = recent_command_records(&connection, 5).expect("recent records should load");

        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0].command_text,
            "gh pr checkout 123 && cargo   test"
        );
        assert_eq!(records[0].normalized_text, "gh pr checkout 123 cargo test");
        assert_eq!(records[0].cwd, "/tmp/project");
        assert_eq!(records[0].git_branch.as_deref(), Some("feat/task-04"));
    }

    #[test]
    fn fts_search_returns_raw_commands_in_ranked_recency_order() {
        let root = temp_root("db-search");
        let database_path = root.join("seekr.db");
        let connection = open(&database_path).expect("database should initialize");

        for (command_text, executed_at) in [
            ("docker compose up", 1_720_000_000),
            ("docker compose up", 1_720_000_001),
            ("cargo test", 1_720_000_002),
        ] {
            let record = CommandRecord::new(
                command_text.to_string(),
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
            insert_command_record(&connection, &record).expect("record should insert");
        }

        let records =
            search_command_records(&connection, "docker", 1).expect("search should succeed");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].command_text, "docker compose up");
        assert_eq!(records[0].executed_at, 1_720_000_001);

        let records =
            search_command_records(&connection, "docker", 5).expect("search should succeed");
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].executed_at, 1_720_000_001);
        assert_eq!(records[1].executed_at, 1_720_000_000);
    }

    #[test]
    fn fts_search_handles_command_punctuation() {
        let root = temp_root("db-search-punctuation");
        let connection = open(&root.join("seekr.db")).expect("database should initialize");

        for command_text in ["git-status", "foo/bar", "foo.bar", "git status --short"] {
            let record = CommandRecord::new(
                command_text.to_string(),
                "/tmp/project".to_string(),
                1_720_000_000,
                0,
                None,
                None,
                None,
                None,
                None,
            )
            .expect("record should validate");
            insert_command_record(&connection, &record).expect("record should insert");

            let records = search_command_records(&connection, command_text, 10)
                .expect("punctuated search should succeed");
            assert!(records
                .iter()
                .any(|record| record.command_text == command_text));
        }
    }

    #[test]
    fn fts_search_rejects_invalid_limits() {
        let root = temp_root("db-search-limit");
        let connection = open(&root.join("seekr.db")).expect("database should initialize");

        for limit in [0, usize::MAX] {
            let error = search_command_records(&connection, "docker", limit)
                .expect_err("invalid limit should fail");
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        }
    }

    #[test]
    fn filters_search_results_by_metadata_and_time_window() {
        let root = temp_root("db-filtered-search");
        let connection = open(&root.join("seekr.db")).expect("database should initialize");

        for (command_text, cwd, executed_at, exit_code, repo, branch) in [
            (
                "docker compose up",
                "/tmp/app",
                100,
                0,
                Some("app"),
                Some("main"),
            ),
            (
                "docker compose logs",
                "/tmp/app",
                200,
                1,
                Some("app"),
                Some("fix"),
            ),
            (
                "docker compose down",
                "/tmp/other",
                300,
                1,
                Some("other"),
                Some("main"),
            ),
        ] {
            let record = CommandRecord::new(
                command_text.to_string(),
                cwd.to_string(),
                executed_at,
                exit_code,
                None,
                None,
                None,
                repo.map(str::to_string),
                branch.map(str::to_string),
            )
            .expect("record should validate");
            insert_command_record(&connection, &record).expect("record should insert");
        }

        let records = filtered_command_records(
            &connection,
            Some("docker"),
            &SearchFilters {
                cwd: Some("/tmp/app".to_string()),
                repo: Some("app".to_string()),
                branch: Some("fix".to_string()),
                failed: Some(true),
                since: Some(150),
                before: Some(250),
            },
            10,
        )
        .expect("filters should search");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].command_text, "docker compose logs");

        let successful = filtered_command_records(
            &connection,
            Some("docker"),
            &SearchFilters {
                failed: Some(false),
                ..SearchFilters::default()
            },
            10,
        )
        .expect("successful filter should search");
        assert_eq!(successful.len(), 1);
        assert_eq!(successful[0].command_text, "docker compose up");
    }

    #[test]
    fn command_record_rejects_invalid_input() {
        let empty = CommandRecord::new(
            "   ".to_string(),
            "/tmp/project".to_string(),
            1_720_000_001,
            0,
            None,
            None,
            None,
            None,
            None,
        )
        .expect_err("blank command should fail");
        assert_eq!(empty.kind(), io::ErrorKind::InvalidInput);

        let negative_duration = CommandRecord::new(
            "cargo test".to_string(),
            "/tmp/project".to_string(),
            1_720_000_001,
            0,
            None,
            Some(-1),
            None,
            None,
            None,
        )
        .expect_err("negative duration should fail");
        assert_eq!(negative_duration.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn punctuation_only_commands_are_still_valid_raw_commands() {
        let record = CommandRecord::new(
            ":".to_string(),
            "/tmp/project".to_string(),
            1_720_000_001,
            0,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("shell builtins should still persist");

        assert_eq!(record.command_text, ":");
        assert!(record.normalized_text.is_empty());
    }

    fn names_for(connection: &Connection, object_type: &str) -> HashSet<String> {
        let mut statement = connection
            .prepare("SELECT name FROM sqlite_master WHERE type = ?1")
            .expect("sqlite master query");
        let rows = statement
            .query_map([object_type], |row| row.get::<_, String>(0))
            .expect("object names");

        rows.map(|row| row.expect("sqlite name")).collect()
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
}
