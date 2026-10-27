use std::collections::HashMap;

use chrono::{Duration, NaiveDate};
use rusqlite::{Connection, params};

use crate::domain::{AnalyticsSnapshot, DailyAnalytics, FocusRow, WeeklyAnalytics, WeeklyRow};
use crate::error::AppError;

const UNASSIGNED_KEY: &str = "unassigned";
const UNASSIGNED_TITLE: &str = "Unassigned";
const WEEK_DAYS: usize = 7;

pub fn get_analytics(conn: &Connection, today: NaiveDate) -> Result<AnalyticsSnapshot, AppError> {
    Ok(AnalyticsSnapshot {
        today: daily_analytics(conn, today)?,
        week: weekly_analytics(conn, today)?,
    })
}

fn daily_analytics(conn: &Connection, date: NaiveDate) -> Result<DailyAnalytics, AppError> {
    let day = format_day(date);
    let mut stmt = conn.prepare(
        "SELECT
             ft.todo_id,
             t.title,
             t.estimate_pomodoros,
             SUM(ft.active_ms) AS active_ms,
             (
                 SELECT COUNT(*)
                 FROM focus_sessions fs
                 WHERE fs.outcome = 'completed'
                   AND fs.credit_todo_id = ft.todo_id
             ) AS completed_pomodoros
         FROM focus_time ft
         LEFT JOIN todos t ON t.id = ft.todo_id
         WHERE ft.local_day = ?1
         GROUP BY ft.todo_id",
    )?;

    let mut rows = stmt
        .query_map([day.as_str()], |row| {
            let todo_id: Option<String> = row.get(0)?;
            let title: Option<String> = row.get(1)?;
            let estimate_pomodoros: Option<i64> = row.get(2)?;
            let active_ms: i64 = row.get(3)?;
            let completed_pomodoros: i64 = row.get(4)?;

            Ok(match todo_id {
                Some(id) => FocusRow {
                    key: id.clone(),
                    todo_id: Some(id),
                    title: title.unwrap_or_default(),
                    active_ms,
                    completed_pomodoros,
                    estimate_pomodoros,
                },
                None => FocusRow {
                    key: UNASSIGNED_KEY.to_string(),
                    todo_id: None,
                    title: UNASSIGNED_TITLE.to_string(),
                    active_ms,
                    completed_pomodoros: 0,
                    estimate_pomodoros: None,
                },
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    rows.retain(|row| row.active_ms > 0);
    rows.sort_by(|a, b| {
        b.active_ms
            .cmp(&a.active_ms)
            .then_with(|| a.title.cmp(&b.title))
            .then_with(|| a.key.cmp(&b.key))
    });

    let total_ms = rows.iter().map(|row| row.active_ms).sum();

    Ok(DailyAnalytics {
        date: day,
        total_ms,
        rows,
    })
}

fn weekly_analytics(conn: &Connection, today: NaiveDate) -> Result<WeeklyAnalytics, AppError> {
    let days: Vec<String> = (0..WEEK_DAYS)
        .map(|offset| {
            let days_ago = (WEEK_DAYS - 1 - offset) as i64;
            format_day(today - Duration::days(days_ago))
        })
        .collect();
    let day_indexes: HashMap<&str, usize> = days
        .iter()
        .enumerate()
        .map(|(index, day)| (day.as_str(), index))
        .collect();

    let mut stmt = conn.prepare(
        "SELECT
             ft.todo_id,
             t.title,
             t.estimate_pomodoros,
             ft.local_day,
             SUM(ft.active_ms) AS active_ms,
             (
                 SELECT COUNT(*)
                 FROM focus_sessions fs
                 WHERE fs.outcome = 'completed'
                   AND fs.credit_todo_id = ft.todo_id
             ) AS completed_pomodoros
         FROM focus_time ft
         LEFT JOIN todos t ON t.id = ft.todo_id
         WHERE ft.local_day BETWEEN ?1 AND ?2
         GROUP BY ft.todo_id, ft.local_day",
    )?;

    let mut by_todo: HashMap<Option<String>, WeeklyRow> = HashMap::new();
    let mut day_totals = vec![0_i64; WEEK_DAYS];

    let mut query = stmt.query(params![days[0], days[WEEK_DAYS - 1]])?;
    while let Some(row) = query.next()? {
        let todo_id: Option<String> = row.get(0)?;
        let title: Option<String> = row.get(1)?;
        let estimate_pomodoros: Option<i64> = row.get(2)?;
        let local_day: String = row.get(3)?;
        let active_ms: i64 = row.get(4)?;
        let completed_pomodoros: i64 = row.get(5)?;

        let Some(&day_index) = day_indexes.get(local_day.as_str()) else {
            continue;
        };
        if active_ms <= 0 {
            continue;
        }

        let unassigned = todo_id.is_none();
        let entry = by_todo.entry(todo_id.clone()).or_insert_with(|| WeeklyRow {
            key: todo_id
                .clone()
                .unwrap_or_else(|| UNASSIGNED_KEY.to_string()),
            todo_id: todo_id.clone(),
            title: if unassigned {
                UNASSIGNED_TITLE.to_string()
            } else {
                title.clone().unwrap_or_default()
            },
            cells: vec![0; WEEK_DAYS],
            total_ms: 0,
            completed_pomodoros: if unassigned { 0 } else { completed_pomodoros },
            estimate_pomodoros: if unassigned { None } else { estimate_pomodoros },
        });

        entry.cells[day_index] += active_ms;
        entry.total_ms += active_ms;
        day_totals[day_index] += active_ms;
    }

    let mut rows: Vec<WeeklyRow> = by_todo.into_values().collect();
    rows.sort_by(|a, b| {
        b.total_ms
            .cmp(&a.total_ms)
            .then_with(|| a.title.cmp(&b.title))
            .then_with(|| a.key.cmp(&b.key))
    });
    let total_ms = rows.iter().map(|row| row.total_ms).sum();

    Ok(WeeklyAnalytics {
        days,
        rows,
        day_totals,
        total_ms,
    })
}

fn format_day(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::repo::todos::{create_todo, soft_delete};

    fn fixed_today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 26).unwrap()
    }

    #[test]
    fn daily_and_week_include_task_unassigned_deleted_and_totals() {
        let conn = db::open_in_memory().unwrap();
        let todo = create_todo(&conn, "Write report", Some(4), 0).unwrap();
        conn.execute("INSERT INTO focus_sessions (id, started_at, ended_at, last_checkpoint_at, outcome, credit_todo_id) VALUES ('s1', 0, 0, 0, 'completed', ?1)", [&todo.id]).unwrap();
        conn.execute(
            "INSERT INTO focus_time VALUES ('s1', ?1, ?1, '2026-10-26', 600000, 0)",
            [&todo.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO focus_time VALUES ('s1', NULL, 'unassigned', '2026-10-25', 300000, 0)",
            [],
        )
        .unwrap();
        let a = get_analytics(&conn, NaiveDate::from_ymd_opt(2026, 10, 26).unwrap()).unwrap();
        assert_eq!(a.today.total_ms, 600000);
        assert_eq!(a.today.rows[0].completed_pomodoros, 1);
        assert_eq!(a.week.days.len(), 7);
        assert_eq!(a.week.days[6], "2026-10-26");
        assert_eq!(a.week.total_ms, 900000);
        assert!(a.week.rows.iter().any(|r| r.key == "unassigned"));
    }

    #[test]
    fn week_excludes_days_before_window_and_keeps_deleted_todo_title() {
        let conn = db::open_in_memory().unwrap();
        let deleted = create_todo(&conn, "Deleted report", Some(2), 0).unwrap();
        let active = create_todo(&conn, "Active task", None, 0).unwrap();

        conn.execute(
            "INSERT INTO focus_sessions
             (id, started_at, ended_at, last_checkpoint_at, outcome, credit_todo_id)
             VALUES ('deleted-completed', 0, 0, 0, 'completed', ?1)",
            [&deleted.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO focus_sessions
             (id, started_at, ended_at, last_checkpoint_at, outcome, credit_todo_id)
             VALUES ('deleted-abandoned', 0, 0, 0, 'abandoned', ?1)",
            [&deleted.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO focus_sessions
             (id, started_at, ended_at, last_checkpoint_at, outcome, credit_todo_id)
             VALUES ('active-session', 0, 0, 0, 'abandoned', ?1)",
            [&active.id],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO focus_time VALUES ('deleted-completed', ?1, ?1, '2026-10-19', 900000, 0)",
            [&deleted.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO focus_time VALUES ('deleted-completed', ?1, ?1, '2026-10-20', 120000, 0)",
            [&deleted.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO focus_time VALUES ('active-session', ?1, ?1, '2026-10-26', 60000, 0)",
            [&active.id],
        )
        .unwrap();

        soft_delete(&conn, &deleted.id, 1).unwrap();

        let a = get_analytics(&conn, fixed_today()).unwrap();

        assert_eq!(
            a.week.days,
            vec![
                "2026-10-20",
                "2026-10-21",
                "2026-10-22",
                "2026-10-23",
                "2026-10-24",
                "2026-10-25",
                "2026-10-26",
            ]
        );
        assert_eq!(a.week.total_ms, 180000);
        assert_eq!(a.week.day_totals, vec![120000, 0, 0, 0, 0, 0, 60000]);
        assert_eq!(a.today.total_ms, 60000);
        assert_eq!(a.today.rows.len(), 1);
        assert_eq!(a.today.rows[0].title, "Active task");
        assert_eq!(a.today.rows[0].completed_pomodoros, 0);

        let deleted_row = a
            .week
            .rows
            .iter()
            .find(|row| row.todo_id.as_deref() == Some(deleted.id.as_str()))
            .unwrap();
        assert_eq!(deleted_row.title, "Deleted report");
        assert_eq!(deleted_row.cells, vec![120000, 0, 0, 0, 0, 0, 0]);
        assert_eq!(deleted_row.total_ms, 120000);
        assert_eq!(deleted_row.completed_pomodoros, 1);
        assert_eq!(deleted_row.estimate_pomodoros, Some(2));
        assert!(!a.week.rows.iter().any(|row| row.total_ms == 900000));
    }

    #[test]
    fn rows_sort_by_time_then_title_and_zero_time_rows_are_omitted() {
        let conn = db::open_in_memory().unwrap();
        let gamma = create_todo(&conn, "Gamma", None, 0).unwrap();
        let beta = create_todo(&conn, "Beta", None, 0).unwrap();
        let alpha = create_todo(&conn, "Alpha", None, 0).unwrap();
        let empty = create_todo(&conn, "Empty", None, 0).unwrap();

        conn.execute(
            "INSERT INTO focus_sessions
             (id, started_at, ended_at, last_checkpoint_at, outcome, credit_todo_id)
             VALUES ('s1', 0, 0, 0, 'abandoned', NULL)",
            [],
        )
        .unwrap();

        for (todo_id, active_ms) in [
            (&gamma.id, 600000),
            (&beta.id, 300000),
            (&alpha.id, 300000),
            (&empty.id, 0),
        ] {
            conn.execute(
                "INSERT INTO focus_time VALUES ('s1', ?1, ?1, '2026-10-26', ?2, 0)",
                params![todo_id, active_ms],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO focus_time VALUES ('s1', NULL, 'unassigned', '2026-10-26', 100000, 0)",
            [],
        )
        .unwrap();

        let a = get_analytics(&conn, fixed_today()).unwrap();
        let keys: Vec<&str> = a.today.rows.iter().map(|row| row.key.as_str()).collect();
        assert_eq!(
            keys,
            vec![
                gamma.id.as_str(),
                alpha.id.as_str(),
                beta.id.as_str(),
                "unassigned"
            ]
        );
        assert_eq!(a.today.total_ms, 1300000);
        assert!(a.week.rows.iter().all(|row| row.title != "Empty"));
    }
}
