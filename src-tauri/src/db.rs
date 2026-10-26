#![allow(dead_code)]

use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;

use crate::error::AppError;

const SCHEMA_VERSION: i64 = 1;

const SCHEMA_V1: &str = r#"
CREATE TABLE todos (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    estimate_pomodoros INTEGER CHECK (estimate_pomodoros IS NULL OR estimate_pomodoros > 0),
    sort_order INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    completed_at INTEGER,
    deleted_at INTEGER
);

CREATE TABLE focus_sessions (
    id TEXT PRIMARY KEY,
    started_at INTEGER NOT NULL,
    ended_at INTEGER,
    last_checkpoint_at INTEGER NOT NULL,
    outcome TEXT NOT NULL CHECK (outcome IN ('active', 'completed', 'abandoned')),
    credit_todo_id TEXT REFERENCES todos(id)
);

CREATE TABLE focus_time (
    session_id TEXT NOT NULL REFERENCES focus_sessions(id) ON DELETE CASCADE,
    todo_id TEXT REFERENCES todos(id),
    bucket_key TEXT NOT NULL,
    local_day TEXT NOT NULL,
    active_ms INTEGER NOT NULL CHECK (active_ms >= 0),
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (session_id, bucket_key, local_day)
);

CREATE INDEX idx_focus_time_day ON focus_time(local_day);
CREATE INDEX idx_focus_time_todo_day ON focus_time(todo_id, local_day);
CREATE INDEX idx_focus_sessions_credit ON focus_sessions(outcome, credit_todo_id);

CREATE TABLE settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    muted INTEGER NOT NULL,
    completed_focus_count INTEGER NOT NULL
);

INSERT INTO settings (id, muted, completed_focus_count) VALUES (1, 0, 0);
"#;

pub fn open(path: impl AsRef<Path>) -> Result<Connection, AppError> {
    let mut conn = Connection::open(path)?;
    configure(&conn, true)?;
    migrate(&mut conn)?;
    Ok(conn)
}

pub fn open_in_memory() -> Result<Connection, AppError> {
    let mut conn = Connection::open_in_memory()?;
    configure(&conn, false)?;
    migrate(&mut conn)?;
    Ok(conn)
}

fn configure(conn: &Connection, enable_wal: bool) -> Result<(), AppError> {
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.busy_timeout(Duration::from_secs(5))?;

    if enable_wal {
        let journal_mode: String =
            conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
        if !journal_mode.eq_ignore_ascii_case("wal") {
            return Err(AppError::Storage(format!(
                "SQLite refused WAL journal mode (got {journal_mode})"
            )));
        }
    }

    Ok(())
}

pub fn migrate(conn: &mut Connection) -> Result<(), AppError> {
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;

    match version {
        0 => {
            let tx = conn.transaction()?;
            tx.execute_batch(SCHEMA_V1)?;
            tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
            tx.commit()?;
            Ok(())
        }
        SCHEMA_VERSION => Ok(()),
        other => Err(AppError::Storage(format!(
            "unsupported SQLite schema version {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn migrates_schema_and_upgrades_user_version() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();

        let version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);

        let tables: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table'
                   AND name IN ('todos', 'focus_sessions', 'focus_time', 'settings')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tables, 4);

        let settings: (i64, i64) = conn
            .query_row(
                "SELECT muted, completed_focus_count FROM settings WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(settings, (0, 0));
    }

    #[test]
    fn migrate_is_idempotent() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        migrate(&mut conn).unwrap();

        let version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn open_configures_wal_foreign_keys_and_busy_timeout() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open(dir.path().join("pomodoro.sqlite3")).unwrap();

        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(journal_mode.to_ascii_lowercase(), "wal");

        let foreign_keys: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(foreign_keys, 1);

        let busy_timeout: i64 = conn
            .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
            .unwrap();
        assert_eq!(busy_timeout, 5_000);
    }

    #[test]
    fn open_in_memory_enables_foreign_keys_and_busy_timeout() {
        let conn = open_in_memory().unwrap();

        let foreign_keys: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(foreign_keys, 1);

        let busy_timeout: i64 = conn
            .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
            .unwrap();
        assert_eq!(busy_timeout, 5_000);
    }
}
