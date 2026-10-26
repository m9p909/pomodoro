#![allow(dead_code)]

use rusqlite::{Connection, params};

use crate::error::AppError;

pub fn insert_active_session(
    conn: &Connection,
    id: &str,
    started_at: i64,
    credit_todo_id: Option<&str>,
) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO focus_sessions
         (id, started_at, ended_at, last_checkpoint_at, outcome, credit_todo_id)
         VALUES (?1, ?2, NULL, ?2, 'active', ?3)",
        params![id, started_at, credit_todo_id],
    )?;

    Ok(())
}

pub fn finish_session(
    conn: &Connection,
    id: &str,
    outcome: &str,
    ended_at: i64,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE focus_sessions
         SET outcome = ?2, ended_at = ?3, last_checkpoint_at = ?3
         WHERE id = ?1",
        params![id, outcome, ended_at],
    )?;

    Ok(())
}

pub fn upsert_focus_time_tx(
    tx: &Connection,
    session_id: &str,
    todo_id: Option<&str>,
    bucket_key: &str,
    day: &str,
    delta_ms: i64,
    now: i64,
) -> Result<(), AppError> {
    tx.execute(
        "INSERT INTO focus_time (session_id, todo_id, bucket_key, local_day, active_ms, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(session_id, bucket_key, local_day)
         DO UPDATE SET active_ms = active_ms + excluded.active_ms, updated_at = excluded.updated_at",
        params![session_id, todo_id, bucket_key, day, delta_ms, now],
    )?;

    Ok(())
}

pub fn update_checkpoint(conn: &Connection, session_id: &str, now: i64) -> Result<(), AppError> {
    conn.execute(
        "UPDATE focus_sessions SET last_checkpoint_at = ?2
         WHERE id = ?1 AND outcome = 'active'",
        params![session_id, now],
    )?;

    Ok(())
}

pub fn clear_credit(conn: &Connection, session_id: &str) -> Result<(), AppError> {
    conn.execute(
        "UPDATE focus_sessions SET credit_todo_id = NULL WHERE id = ?1",
        params![session_id],
    )?;

    Ok(())
}

pub fn completed_credit_count(conn: &Connection, todo_id: &str) -> Result<i64, AppError> {
    let count = conn.query_row(
        "SELECT COUNT(*)
         FROM focus_sessions
         WHERE outcome = 'completed' AND credit_todo_id = ?1",
        [todo_id],
        |row| row.get(0),
    )?;

    Ok(count)
}

pub fn recover_active_sessions(conn: &Connection) -> Result<usize, AppError> {
    let changed = conn.execute(
        "UPDATE focus_sessions
         SET outcome = 'abandoned', ended_at = last_checkpoint_at
         WHERE outcome = 'active'",
        [],
    )?;

    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::repo::todos::create_todo;

    #[test]
    fn upserts_buckets_and_updates_checkpoints() {
        let conn = db::open_in_memory().unwrap();
        let todo = create_todo(&conn, "Task", None, 0).unwrap();
        insert_active_session(&conn, "s1", 100, Some(&todo.id)).unwrap();

        upsert_focus_time_tx(
            &conn,
            "s1",
            Some(&todo.id),
            &todo.id,
            "2026-10-26",
            1_000,
            200,
        )
        .unwrap();
        upsert_focus_time_tx(
            &conn,
            "s1",
            Some(&todo.id),
            &todo.id,
            "2026-10-26",
            2_000,
            300,
        )
        .unwrap();
        upsert_focus_time_tx(&conn, "s1", None, "unassigned", "2026-10-26", 500, 400).unwrap();
        update_checkpoint(&conn, "s1", 400).unwrap();

        let todo_ms: i64 = conn
            .query_row(
                "SELECT active_ms FROM focus_time WHERE session_id = 's1' AND todo_id = ?1",
                [&todo.id],
                |row| row.get(0),
            )
            .unwrap();
        let unassigned_ms: i64 = conn
            .query_row(
                "SELECT active_ms FROM focus_time
                 WHERE session_id = 's1' AND todo_id IS NULL AND bucket_key = 'unassigned'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let checkpoint: i64 = conn
            .query_row(
                "SELECT last_checkpoint_at FROM focus_sessions WHERE id = 's1'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(todo_ms, 3_000);
        assert_eq!(unassigned_ms, 500);
        assert_eq!(checkpoint, 400);
    }

    #[test]
    fn checkpoint_updates_do_not_move_past_end_for_finished_sessions() {
        let conn = db::open_in_memory().unwrap();
        insert_active_session(&conn, "finished", 100, None).unwrap();
        finish_session(&conn, "finished", "completed", 200).unwrap();

        update_checkpoint(&conn, "finished", 300).unwrap();

        let checkpoint: i64 = conn
            .query_row(
                "SELECT last_checkpoint_at FROM focus_sessions WHERE id = 'finished'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(checkpoint, 200);
    }

    #[test]
    fn recovery_abandons_active_sessions_at_last_checkpoint() {
        let conn = db::open_in_memory().unwrap();
        insert_active_session(&conn, "active", 100, None).unwrap();
        insert_active_session(&conn, "other", 100, None).unwrap();
        finish_session(&conn, "other", "completed", 150).unwrap();
        update_checkpoint(&conn, "active", 250).unwrap();

        assert_eq!(recover_active_sessions(&conn).unwrap(), 1);

        let active: (String, Option<i64>) = conn
            .query_row(
                "SELECT outcome, ended_at FROM focus_sessions WHERE id = 'active'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let other: String = conn
            .query_row(
                "SELECT outcome FROM focus_sessions WHERE id = 'other'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(active, ("abandoned".to_string(), Some(250)));
        assert_eq!(other, "completed");
    }

    #[test]
    fn completed_credit_count_only_counts_completed_sessions() {
        let conn = db::open_in_memory().unwrap();
        let todo = create_todo(&conn, "Task", None, 0).unwrap();
        insert_active_session(&conn, "completed", 100, Some(&todo.id)).unwrap();
        finish_session(&conn, "completed", "completed", 200).unwrap();
        insert_active_session(&conn, "abandoned", 100, Some(&todo.id)).unwrap();
        finish_session(&conn, "abandoned", "abandoned", 200).unwrap();
        insert_active_session(&conn, "active", 100, Some(&todo.id)).unwrap();

        assert_eq!(completed_credit_count(&conn, &todo.id).unwrap(), 1);
    }
}
