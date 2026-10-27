use std::collections::HashSet;

use rusqlite::{Connection, OptionalExtension, Row, params};
use uuid::Uuid;

use crate::domain::{Todo, validate_estimate, validate_title};
use crate::error::AppError;

const TODO_SELECT: &str = "
SELECT
    t.id,
    t.title,
    t.estimate_pomodoros,
    t.sort_order,
    t.completed_at,
    t.created_at,
    t.updated_at,
    (
        SELECT COUNT(*)
        FROM focus_sessions fs
        WHERE fs.outcome = 'completed'
          AND fs.credit_todo_id = t.id
    ) AS completed_pomodoros
FROM todos t
";

pub fn list_todos(conn: &Connection) -> Result<Vec<Todo>, AppError> {
    let sql =
        format!("{TODO_SELECT} WHERE t.deleted_at IS NULL ORDER BY t.sort_order, t.created_at");
    let mut stmt = conn.prepare(&sql)?;
    let todos = stmt
        .query_map([], todo_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(todos)
}

pub fn get_todo(conn: &Connection, id: &str) -> Result<Todo, AppError> {
    let sql = format!("{TODO_SELECT} WHERE t.id = ?1 AND t.deleted_at IS NULL");
    let mut stmt = conn.prepare(&sql)?;
    let todo = stmt
        .query_row([id], todo_from_row)
        .optional()?
        .ok_or_else(|| not_found(id))?;

    Ok(todo)
}

pub fn create_todo(
    conn: &Connection,
    title: &str,
    estimate_pomodoros: Option<i64>,
    now: i64,
) -> Result<Todo, AppError> {
    let title = validate_title(title)?;
    let estimate_pomodoros = validate_estimate(estimate_pomodoros)?;
    let id = Uuid::new_v4().to_string();
    let sort_order: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM todos",
        [],
        |row| row.get(0),
    )?;

    conn.execute(
        "INSERT INTO todos
         (id, title, estimate_pomodoros, sort_order, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![id, title, estimate_pomodoros, sort_order, now],
    )?;

    get_todo(conn, &id)
}

pub fn update_todo(
    conn: &Connection,
    id: &str,
    title: &str,
    estimate_pomodoros: Option<i64>,
    now: i64,
) -> Result<Todo, AppError> {
    let title = validate_title(title)?;
    let estimate_pomodoros = validate_estimate(estimate_pomodoros)?;

    let changed = conn.execute(
        "UPDATE todos
         SET title = ?1, estimate_pomodoros = ?2, updated_at = ?3
         WHERE id = ?4 AND deleted_at IS NULL",
        params![title, estimate_pomodoros, now, id],
    )?;
    if changed == 0 {
        return Err(not_found(id));
    }

    get_todo(conn, id)
}

pub fn set_completed(
    conn: &Connection,
    id: &str,
    completed: bool,
    now: i64,
) -> Result<Todo, AppError> {
    let completed_at = completed.then_some(now);
    let changed = conn.execute(
        "UPDATE todos
         SET completed_at = ?1, updated_at = ?2
         WHERE id = ?3 AND deleted_at IS NULL",
        params![completed_at, now, id],
    )?;
    if changed == 0 {
        return Err(not_found(id));
    }

    get_todo(conn, id)
}

pub fn soft_delete(conn: &Connection, id: &str, now: i64) -> Result<(), AppError> {
    let changed = conn.execute(
        "UPDATE todos
         SET deleted_at = ?1, updated_at = ?1
         WHERE id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;
    if changed == 0 {
        return Err(not_found(id));
    }

    Ok(())
}

pub fn reorder_todos(conn: &Connection, ids: &[String], now: i64) -> Result<Vec<Todo>, AppError> {
    let unique_ids: HashSet<&str> = ids.iter().map(String::as_str).collect();
    if unique_ids.len() != ids.len() {
        return Err(AppError::Validation(
            "Reorder must not contain duplicate todo ids".to_string(),
        ));
    }

    let tx = conn.unchecked_transaction()?;
    let active_ids: HashSet<String> = {
        let mut stmt = tx.prepare("SELECT id FROM todos WHERE deleted_at IS NULL")?;
        stmt.query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?
            .into_iter()
            .collect()
    };

    if active_ids.len() != unique_ids.len() || !unique_ids.iter().all(|id| active_ids.contains(*id))
    {
        return Err(AppError::Validation(
            "Reorder must include every active todo exactly once".to_string(),
        ));
    }

    for (sort_order, id) in ids.iter().enumerate() {
        let changed = tx.execute(
            "UPDATE todos
             SET sort_order = ?1, updated_at = ?2
             WHERE id = ?3 AND deleted_at IS NULL",
            params![sort_order as i64, now, id],
        )?;
        if changed != 1 {
            return Err(AppError::Storage(format!(
                "failed to update sort order for todo {id}"
            )));
        }
    }
    tx.commit()?;

    list_todos(conn)
}

fn todo_from_row(row: &Row<'_>) -> rusqlite::Result<Todo> {
    Ok(Todo {
        id: row.get(0)?,
        title: row.get(1)?,
        estimate_pomodoros: row.get(2)?,
        sort_order: row.get(3)?,
        completed_at: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
        completed_pomodoros: row.get(7)?,
    })
}

fn not_found(id: &str) -> AppError {
    AppError::NotFound(format!("Todo {id} not found"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn creates_lists_and_soft_deletes_todos() {
        let conn = db::open_in_memory().unwrap();
        let a = create_todo(&conn, "First", Some(3), 1000).unwrap();
        let b = create_todo(&conn, "Second", None, 1001).unwrap();
        assert_eq!(a.title, "First");
        assert_eq!(a.estimate_pomodoros, Some(3));
        assert_eq!(b.estimate_pomodoros, None);
        assert_eq!(a.sort_order, 0);
        assert_eq!(b.sort_order, 1);
        assert_eq!(list_todos(&conn).unwrap().len(), 2);
        soft_delete(&conn, &a.id, 2000).unwrap();
        let remaining = list_todos(&conn).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, b.id);
        assert!(get_todo(&conn, &a.id).is_err());

        let stored: (String, i64) = conn
            .query_row(
                "SELECT title, deleted_at FROM todos WHERE id = ?1",
                [&a.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(stored, ("First".to_string(), 2000));
    }

    #[test]
    fn reorder_requires_exact_active_set() {
        let conn = db::open_in_memory().unwrap();
        let a = create_todo(&conn, "A", None, 1).unwrap();
        let b = create_todo(&conn, "B", None, 2).unwrap();
        let ordered = reorder_todos(&conn, &[b.id.clone(), a.id.clone()], 3).unwrap();
        assert_eq!(ordered[0].id, b.id);
        assert_eq!(ordered[1].id, a.id);
        assert!(reorder_todos(&conn, std::slice::from_ref(&a.id), 4).is_err());
        assert!(
            reorder_todos(&conn, &[a.id.clone(), a.id.clone()], 4).is_err(),
            "duplicate ids must be rejected"
        );
        assert!(
            reorder_todos(
                &conn,
                &[a.id.clone(), b.id.clone(), "not-active".to_string()],
                4
            )
            .is_err(),
            "ids outside the active set must be rejected"
        );
    }

    #[test]
    fn reorder_rolls_back_if_a_write_fails() {
        let conn = db::open_in_memory().unwrap();
        let a = create_todo(&conn, "A", None, 1).unwrap();
        let b = create_todo(&conn, "B", None, 2).unwrap();
        conn.execute_batch(&format!(
            "CREATE TRIGGER fail_reorder BEFORE UPDATE OF sort_order ON todos
             WHEN NEW.id = '{}' AND NEW.sort_order = 1
             BEGIN
                 SELECT RAISE(ABORT, 'forced reorder failure');
             END;",
            a.id
        ))
        .unwrap();

        assert!(reorder_todos(&conn, &[b.id.clone(), a.id.clone()], 3).is_err());

        let orders: Vec<(String, i64)> = {
            let mut stmt = conn
                .prepare("SELECT id, sort_order FROM todos ORDER BY sort_order")
                .unwrap();
            stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap()
        };
        assert_eq!(orders, vec![(a.id.clone(), 0), (b.id.clone(), 1)]);
        assert_eq!(get_todo(&conn, &b.id).unwrap().sort_order, 1);
        assert_eq!(get_todo(&conn, &a.id).unwrap().sort_order, 0);
    }

    #[test]
    fn validates_and_updates_todos() {
        let conn = db::open_in_memory().unwrap();
        assert!(create_todo(&conn, "   ", None, 1).is_err());
        assert!(create_todo(&conn, "Invalid", Some(0), 1).is_err());
        assert!(create_todo(&conn, "Invalid", Some(-2), 1).is_err());

        let created = create_todo(&conn, "  Draft  ", Some(2), 10).unwrap();
        assert_eq!(created.title, "Draft");

        let updated = update_todo(&conn, &created.id, "  Final  ", None, 20).unwrap();
        assert_eq!(updated.title, "Final");
        assert_eq!(updated.estimate_pomodoros, None);
        assert_eq!(updated.updated_at, 20);
        assert!(update_todo(&conn, "missing", "Final", None, 20).is_err());

        soft_delete(&conn, &created.id, 30).unwrap();
        assert!(update_todo(&conn, &created.id, "Final", None, 40).is_err());
    }

    #[test]
    fn completion_and_credited_count_reflect_only_completed_sessions() {
        let conn = db::open_in_memory().unwrap();
        let todo = create_todo(&conn, "Task", Some(2), 1).unwrap();
        let other = create_todo(&conn, "Other", None, 2).unwrap();

        let insert_session = |id: &str, outcome: &str, credit: Option<&str>| {
            conn.execute(
                "INSERT INTO focus_sessions
                 (id, started_at, ended_at, last_checkpoint_at, outcome, credit_todo_id)
                 VALUES (?1, 0, 0, 0, ?2, ?3)",
                rusqlite::params![id, outcome, credit],
            )
            .unwrap();
        };
        insert_session("completed", "completed", Some(&todo.id));
        insert_session("active", "active", Some(&todo.id));
        insert_session("abandoned", "abandoned", Some(&todo.id));
        insert_session("other", "completed", Some(&other.id));

        let listed = list_todos(&conn).unwrap();
        assert_eq!(listed[0].completed_pomodoros, 1);
        assert_eq!(listed[1].completed_pomodoros, 1);

        let completed = set_completed(&conn, &todo.id, true, 50).unwrap();
        assert_eq!(completed.completed_at, Some(50));
        assert_eq!(completed.updated_at, 50);
        let reopened = set_completed(&conn, &todo.id, false, 60).unwrap();
        assert_eq!(reopened.completed_at, None);
        assert_eq!(reopened.updated_at, 60);

        soft_delete(&conn, &todo.id, 70).unwrap();
        assert!(set_completed(&conn, &todo.id, true, 80).is_err());
        assert!(soft_delete(&conn, "missing", 90).is_err());
    }
}
