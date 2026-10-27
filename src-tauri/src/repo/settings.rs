use rusqlite::Connection;

use crate::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoredSettings {
    pub muted: bool,
    pub completed_focus_count: i64,
}

pub fn load_settings(conn: &Connection) -> Result<StoredSettings, AppError> {
    conn.execute(
        "INSERT OR IGNORE INTO settings (id, muted, completed_focus_count)
         VALUES (1, 0, 0)",
        [],
    )?;

    let settings = conn.query_row(
        "SELECT muted, completed_focus_count FROM settings WHERE id = 1",
        [],
        |row| {
            Ok(StoredSettings {
                muted: row.get(0)?,
                completed_focus_count: row.get(1)?,
            })
        },
    )?;

    Ok(settings)
}

pub fn set_muted(conn: &Connection, muted: bool) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO settings (id, muted, completed_focus_count)
         VALUES (1, ?1, 0)
         ON CONFLICT(id) DO UPDATE SET muted = excluded.muted",
        [muted],
    )?;

    Ok(())
}

pub fn increment_completed_focus_count(conn: &Connection) -> Result<i64, AppError> {
    let count = conn.query_row(
        "INSERT INTO settings (id, muted, completed_focus_count)
         VALUES (1, 0, 1)
         ON CONFLICT(id) DO UPDATE
             SET completed_focus_count = completed_focus_count + 1
         RETURNING completed_focus_count",
        [],
        |row| row.get(0),
    )?;

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn loads_defaults_and_increments_cycle() {
        let conn = db::open_in_memory().unwrap();
        let settings = load_settings(&conn).unwrap();
        assert!(!settings.muted);
        assert_eq!(settings.completed_focus_count, 0);
        assert_eq!(increment_completed_focus_count(&conn).unwrap(), 1);
        assert_eq!(increment_completed_focus_count(&conn).unwrap(), 2);
        set_muted(&conn, true).unwrap();
        assert!(load_settings(&conn).unwrap().muted);
    }

    #[test]
    fn load_restores_defaults_without_overwriting_existing_values() {
        let conn = db::open_in_memory().unwrap();
        conn.execute("DELETE FROM settings", []).unwrap();

        let restored = load_settings(&conn).unwrap();
        assert!(!restored.muted);
        assert_eq!(restored.completed_focus_count, 0);

        conn.execute(
            "UPDATE settings SET muted = 1, completed_focus_count = 7 WHERE id = 1",
            [],
        )
        .unwrap();
        let existing = load_settings(&conn).unwrap();
        assert!(existing.muted);
        assert_eq!(existing.completed_focus_count, 7);

        set_muted(&conn, false).unwrap();
        let muted_off = load_settings(&conn).unwrap();
        assert!(!muted_off.muted);
        assert_eq!(muted_off.completed_focus_count, 7);
    }
}
