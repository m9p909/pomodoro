use std::sync::{Arc, Mutex};

use chrono::NaiveDate;
use rusqlite::Connection;
use tauri::AppHandle;

use crate::clock::Clock;
use crate::domain::{Bootstrap, DataSnapshot, TimerSnapshot, TimerState, Todo};
use crate::error::AppError;
use crate::repo::{analytics, todos};
use crate::timer::{TickOutcome, TimerCore};

pub struct AppState {
    pub inner: Arc<Mutex<AppInner>>,
    pub app: AppHandle,
}

pub struct AppInner {
    conn: Connection,
    timer: TimerCore,
    clock: Arc<dyn Clock>,
}

impl AppInner {
    #[allow(dead_code)]
    pub fn new(mut conn: Connection, clock: Arc<dyn Clock>) -> Result<Self, AppError> {
        let timer = TimerCore::load(&mut conn)?;
        Ok(Self { conn, timer, clock })
    }

    pub fn bootstrap(&mut self) -> Result<Bootstrap, AppError> {
        let todos = todos::list_todos(&self.conn)?;
        let timer = self.timer.snapshot(&self.conn)?;
        let analytics = analytics::get_analytics(&self.conn, self.local_date()?)?;
        Ok(Bootstrap {
            todos,
            timer,
            analytics,
        })
    }

    pub fn refresh_data(&mut self) -> Result<DataSnapshot, AppError> {
        let todos = todos::list_todos(&self.conn)?;
        let analytics = analytics::get_analytics(&self.conn, self.local_date()?)?;
        Ok(DataSnapshot { todos, analytics })
    }

    pub fn create_todo(
        &mut self,
        title: &str,
        estimate_pomodoros: Option<i64>,
    ) -> Result<Todo, AppError> {
        todos::create_todo(&self.conn, title, estimate_pomodoros, self.clock.utc_ms())
    }

    pub fn update_todo(
        &mut self,
        id: &str,
        title: &str,
        estimate_pomodoros: Option<i64>,
    ) -> Result<Todo, AppError> {
        todos::update_todo(
            &self.conn,
            id,
            title,
            estimate_pomodoros,
            self.clock.utc_ms(),
        )
    }

    pub fn set_todo_completed(&mut self, id: &str, completed: bool) -> Result<Todo, AppError> {
        todos::set_completed(&self.conn, id, completed, self.clock.utc_ms())
    }

    pub fn delete_todo(&mut self, id: &str) -> Result<(), AppError> {
        let attributed = self
            .timer
            .snapshot(&self.conn)?
            .attributed_todo_id
            .as_deref()
            == Some(id);
        if attributed {
            self.timer.end_focus(&mut self.conn, self.clock.utc_ms())?;
        }
        todos::soft_delete(&self.conn, id, self.clock.utc_ms())
    }

    pub fn reorder_todos(&mut self, ids: &[String]) -> Result<Vec<Todo>, AppError> {
        todos::reorder_todos(&self.conn, ids, self.clock.utc_ms())
    }

    pub fn start_focus(&mut self, todo_id: Option<String>) -> Result<TimerSnapshot, AppError> {
        let (utc_ms, mono_ms, day) = self.now();
        self.timer
            .start_focus(&mut self.conn, todo_id, utc_ms, mono_ms, &day)?;
        self.timer.snapshot(&self.conn)
    }

    pub fn pause_timer(&mut self) -> Result<TimerSnapshot, AppError> {
        let (utc_ms, _, _) = self.now();
        self.timer.pause(&mut self.conn, utc_ms)?;
        self.timer.snapshot(&self.conn)
    }

    pub fn resume_timer(&mut self) -> Result<TimerSnapshot, AppError> {
        let (_, mono_ms, _) = self.now();
        self.timer.resume(mono_ms)?;
        self.timer.snapshot(&self.conn)
    }

    pub fn end_focus(&mut self) -> Result<TimerSnapshot, AppError> {
        let (utc_ms, _, _) = self.now();
        self.timer.end_focus(&mut self.conn, utc_ms)?;
        self.timer.snapshot(&self.conn)
    }

    pub fn start_break(&mut self) -> Result<TimerSnapshot, AppError> {
        let (_, mono_ms, _) = self.now();
        self.timer.start_break(mono_ms)?;
        self.timer.snapshot(&self.conn)
    }

    pub fn skip_break(&mut self) -> Result<TimerSnapshot, AppError> {
        self.timer.skip_break()?;
        self.timer.snapshot(&self.conn)
    }

    pub fn set_muted(&mut self, muted: bool) -> Result<TimerSnapshot, AppError> {
        self.timer.set_muted(&mut self.conn, muted)?;
        self.timer.snapshot(&self.conn)
    }

    #[allow(dead_code)]
    pub fn tick(&mut self) -> Result<TickOutcome, AppError> {
        let (utc_ms, mono_ms, day) = self.now();
        self.timer.tick(&mut self.conn, utc_ms, mono_ms, &day)
    }

    pub fn close_flush(&mut self) -> Result<(), AppError> {
        let (utc_ms, _, _) = self.now();
        if matches!(
            self.timer.snapshot(&self.conn)?.state,
            TimerState::FocusRunning | TimerState::FocusPaused
        ) {
            self.timer.end_focus(&mut self.conn, utc_ms)?;
        }
        self.timer.flush_all(&mut self.conn, utc_ms)
    }

    fn now(&self) -> (i64, u64, String) {
        (
            self.clock.utc_ms(),
            self.clock.mono_ms(),
            self.clock.local_day(),
        )
    }

    fn local_date(&self) -> Result<NaiveDate, AppError> {
        let day = self.clock.local_day();
        NaiveDate::parse_from_str(&day, "%Y-%m-%d")
            .map_err(|error| AppError::Storage(format!("invalid local day {day}: {error}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::FakeClock;
    use crate::db;

    fn test_inner() -> (AppInner, Arc<FakeClock>) {
        let clock = Arc::new(FakeClock::new(1_000, 10_000, "2026-10-26"));
        let inner = AppInner::new(db::open_in_memory().unwrap(), clock.clone()).unwrap();
        (inner, clock)
    }

    #[test]
    fn deleting_active_todo_ends_focus_and_keeps_time() {
        let (mut inner, clock) = test_inner();
        let todo = inner.create_todo("Task", Some(1)).unwrap();
        inner.start_focus(Some(todo.id.clone())).unwrap();
        clock.advance(1_000);
        inner.tick().unwrap();
        inner.delete_todo(&todo.id).unwrap();
        assert_eq!(
            inner.timer.snapshot(&inner.conn).unwrap().state,
            TimerState::Idle
        );
        let total: i64 = inner
            .conn
            .query_row(
                "SELECT COALESCE(SUM(active_ms),0) FROM focus_time",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(total, 1_000);
        let outcome: String = inner
            .conn
            .query_row("SELECT outcome FROM focus_sessions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(outcome, "abandoned");
    }

    #[test]
    fn refresh_returns_todos_and_analytics() {
        let (mut inner, _clock) = test_inner();
        let todo = inner.create_todo("Task", None).unwrap();
        let data = inner.refresh_data().unwrap();
        assert_eq!(data.todos.len(), 1);
        assert_eq!(data.todos[0].id, todo.id);
        assert_eq!(data.analytics.today.total_ms, 0);
        assert_eq!(data.analytics.week.days.len(), 7);
    }

    #[test]
    fn bootstrap_returns_matching_todos_timer_and_analytics() {
        let (mut inner, _clock) = test_inner();
        let todo = inner.create_todo("Task", Some(2)).unwrap();
        inner.start_focus(Some(todo.id.clone())).unwrap();

        let bootstrap = inner.bootstrap().unwrap();

        assert_eq!(bootstrap.todos.len(), 1);
        assert_eq!(bootstrap.todos[0].id, todo.id);
        assert_eq!(bootstrap.timer.state, TimerState::FocusRunning);
        assert_eq!(bootstrap.timer.attributed_todo_id, Some(todo.id));
        assert_eq!(bootstrap.analytics.today.total_ms, 0);
        assert_eq!(bootstrap.analytics.week.days.len(), 7);
    }

    #[test]
    fn close_flush_ends_focus_and_saves_time() {
        let (mut inner, clock) = test_inner();
        let todo = inner.create_todo("Task", Some(1)).unwrap();
        inner.start_focus(Some(todo.id.clone())).unwrap();
        clock.advance(3_000);
        inner.tick().unwrap();

        inner.close_flush().unwrap();

        assert_eq!(
            inner.timer.snapshot(&inner.conn).unwrap().state,
            TimerState::Idle
        );
        let total: i64 = inner
            .conn
            .query_row(
                "SELECT COALESCE(SUM(active_ms),0) FROM focus_time",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(total, 3_000);
    }

    #[test]
    fn close_flush_fails_until_pending_time_can_be_saved() {
        let (mut inner, clock) = test_inner();
        inner
            .conn
            .execute_batch(
                "CREATE TRIGGER fail_focus_time BEFORE INSERT ON focus_time
                 BEGIN
                     SELECT RAISE(ABORT, 'forced checkpoint failure');
                 END;",
            )
            .unwrap();
        let todo = inner.create_todo("Task", Some(1)).unwrap();
        inner.start_focus(Some(todo.id.clone())).unwrap();
        clock.advance(2_000);
        inner.tick().unwrap();

        assert!(inner.close_flush().is_err());
        assert_eq!(
            inner.timer.snapshot(&inner.conn).unwrap().state,
            TimerState::Idle
        );

        inner
            .conn
            .execute_batch("DROP TRIGGER fail_focus_time;")
            .unwrap();
        inner.close_flush().unwrap();

        let total: i64 = inner
            .conn
            .query_row(
                "SELECT COALESCE(SUM(active_ms),0) FROM focus_time",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(total, 2_000);
    }
}
