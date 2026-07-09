use rusqlite::Connection;
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

fn ensure_parent_dir(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent().filter(|path| !path.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::open;
    use rusqlite::{params, Connection};
    use std::collections::HashSet;
    use std::env;
    use std::fs;
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
