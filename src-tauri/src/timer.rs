use std::collections::{HashMap, HashSet};

use rusqlite::{Connection, OptionalExtension};
use uuid::Uuid;

use crate::domain::{
    AlertKind, FOCUS_MS, LONG_BREAK_MS, SHORT_BREAK_MS, SLEEP_GAP_MS, TimerAlert, TimerPhase,
    TimerSnapshot, TimerState,
};
use crate::error::AppError;
use crate::repo::focus::{
    clear_credit, finish_session, insert_active_session, recover_active_sessions,
    update_checkpoint, upsert_focus_time_tx,
};
use crate::repo::settings::{
    increment_completed_focus_count, load_settings, set_muted as store_muted,
};

const UNASSIGNED_BUCKET: &str = "unassigned";
const DATA_EVENT_INTERVAL_MS: i64 = 5_000;
const PERSISTENCE_WARNING: &str = "Focus time could not be saved. Retrying automatically.";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PendingKey {
    session_id: String,
    todo_id: Option<String>,
    local_day: String,
}

#[derive(Debug, Clone)]
pub struct TickOutcome {
    pub snapshot: TimerSnapshot,
    pub alert: Option<TimerAlert>,
    pub data_changed: bool,
}

enum FocusAdvance {
    Advanced,
    SleepPaused,
    Completed,
}

pub struct TimerCore {
    state: TimerState,
    phase: Option<TimerPhase>,
    elapsed_ms: i64,
    session_id: Option<String>,
    attributed_todo_id: Option<String>,
    credit_todo_id: Option<String>,
    last_mono_ms: Option<u64>,
    completed_focus_count: i64,
    muted: bool,
    persistence_warning: Option<String>,
    pending: HashMap<PendingKey, i64>,
    last_data_event_ms: u64,
}

impl TimerCore {
    pub fn load(conn: &mut Connection) -> Result<Self, AppError> {
        recover_active_sessions(conn)?;
        let settings = load_settings(conn)?;

        Ok(Self {
            state: TimerState::Idle,
            phase: None,
            elapsed_ms: 0,
            session_id: None,
            attributed_todo_id: None,
            credit_todo_id: None,
            last_mono_ms: None,
            completed_focus_count: settings.completed_focus_count,
            muted: settings.muted,
            persistence_warning: None,
            pending: HashMap::new(),
            last_data_event_ms: 0,
        })
    }

    pub fn snapshot(&self, conn: &Connection) -> Result<TimerSnapshot, AppError> {
        let total_ms = self.phase_total_ms();
        let remaining_ms = (total_ms - self.elapsed_ms).max(0);
        let attributed_todo_title = match self.attributed_todo_id.as_deref() {
            Some(id) => conn
                .query_row("SELECT title FROM todos WHERE id = ?1", [id], |row| {
                    row.get(0)
                })
                .optional()?,
            None => None,
        };

        Ok(TimerSnapshot {
            state: self.state.clone(),
            phase: self.phase.clone(),
            remaining_ms,
            total_ms,
            attributed_todo_id: self.attributed_todo_id.clone(),
            attributed_todo_title,
            completed_focus_count: self.completed_focus_count,
            muted: self.muted,
            persistence_warning: self.persistence_warning.clone(),
        })
    }

    pub fn start_focus(
        &mut self,
        conn: &mut Connection,
        todo_id: Option<String>,
        utc_ms: i64,
        mono_ms: u64,
        local_day: &str,
    ) -> Result<(), AppError> {
        match self.state {
            TimerState::BreakRunning | TimerState::BreakPaused => {
                return Err(AppError::Timer(
                    "Skip the break before starting focus".to_string(),
                ));
            }
            TimerState::FocusRunning | TimerState::FocusPaused => {
                if self.attributed_todo_id == todo_id {
                    if self.state == TimerState::FocusPaused {
                        self.state = TimerState::FocusRunning;
                        self.last_mono_ms = Some(mono_ms);
                    }
                    return Ok(());
                }

                if self.state == TimerState::FocusRunning {
                    if matches!(
                        self.advance_focus(conn, utc_ms, mono_ms, local_day)?,
                        FocusAdvance::Completed
                    ) {
                        return self.start_focus(conn, todo_id, utc_ms, mono_ms, local_day);
                    }
                } else {
                    let _ = self.flush_pending(conn, utc_ms);
                }

                if let Some(session_id) = self.session_id.as_deref() {
                    clear_credit(conn, session_id)?;
                }

                self.attributed_todo_id = todo_id;
                self.credit_todo_id = None;
                self.state = TimerState::FocusRunning;
                self.last_mono_ms = Some(mono_ms);
            }
            TimerState::Idle | TimerState::BreakPending => {
                let session_id = Uuid::new_v4().to_string();
                insert_active_session(conn, &session_id, utc_ms, todo_id.as_deref())?;

                self.state = TimerState::FocusRunning;
                self.phase = Some(TimerPhase::Focus);
                self.elapsed_ms = 0;
                self.session_id = Some(session_id);
                self.attributed_todo_id = todo_id.clone();
                self.credit_todo_id = todo_id;
                self.last_mono_ms = Some(mono_ms);
                self.last_data_event_ms = 0;
            }
        }

        Ok(())
    }

    pub fn pause(&mut self, conn: &mut Connection, utc_ms: i64) -> Result<(), AppError> {
        match self.state {
            TimerState::FocusRunning => {
                let _ = self.flush_pending(conn, utc_ms);
                self.state = TimerState::FocusPaused;
                self.last_mono_ms = None;
                Ok(())
            }
            TimerState::BreakRunning => {
                self.state = TimerState::BreakPaused;
                self.last_mono_ms = None;
                Ok(())
            }
            _ => Err(AppError::Timer("Timer is not running".to_string())),
        }
    }

    pub fn resume(&mut self, mono_ms: u64) -> Result<(), AppError> {
        match self.state {
            TimerState::FocusPaused => {
                self.state = TimerState::FocusRunning;
                self.last_mono_ms = Some(mono_ms);
                Ok(())
            }
            TimerState::BreakPaused => {
                self.state = TimerState::BreakRunning;
                self.last_mono_ms = Some(mono_ms);
                Ok(())
            }
            _ => Err(AppError::Timer("Timer is not paused".to_string())),
        }
    }

    pub fn end_focus(&mut self, conn: &mut Connection, utc_ms: i64) -> Result<(), AppError> {
        match self.state {
            TimerState::FocusRunning | TimerState::FocusPaused => {
                let _ = self.flush_pending(conn, utc_ms);
                if let Some(session_id) = self.session_id.clone()
                    && finish_session(conn, &session_id, "abandoned", utc_ms)? != 1
                {
                    return Err(AppError::Storage(format!(
                        "Focus session {session_id} could not be abandoned"
                    )));
                }

                self.state = TimerState::Idle;
                self.phase = None;
                self.elapsed_ms = 0;
                self.session_id = None;
                self.attributed_todo_id = None;
                self.credit_todo_id = None;
                self.last_mono_ms = None;
                self.last_data_event_ms = 0;
                Ok(())
            }
            _ => Err(AppError::Timer("Focus is not active".to_string())),
        }
    }

    pub fn start_break(&mut self, mono_ms: u64) -> Result<(), AppError> {
        if self.state != TimerState::BreakPending {
            return Err(AppError::Timer("No break is pending".to_string()));
        }

        self.state = TimerState::BreakRunning;
        self.elapsed_ms = 0;
        self.last_mono_ms = Some(mono_ms);
        self.last_data_event_ms = 0;
        Ok(())
    }

    pub fn skip_break(&mut self) -> Result<(), AppError> {
        match self.state {
            TimerState::BreakPending | TimerState::BreakRunning | TimerState::BreakPaused => {
                self.state = TimerState::Idle;
                self.phase = None;
                self.elapsed_ms = 0;
                self.session_id = None;
                self.attributed_todo_id = None;
                self.credit_todo_id = None;
                self.last_mono_ms = None;
                self.last_data_event_ms = 0;
                Ok(())
            }
            _ => Err(AppError::Timer("No break is active".to_string())),
        }
    }

    pub fn set_muted(&mut self, conn: &mut Connection, muted: bool) -> Result<(), AppError> {
        store_muted(conn, muted)?;
        self.muted = muted;
        Ok(())
    }

    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    pub fn flush_all(&mut self, conn: &mut Connection, now_utc: i64) -> Result<(), AppError> {
        self.flush_pending(conn, now_utc)?;
        if self.has_pending() {
            return Err(AppError::Storage(
                "Focus time could not be saved before closing".to_string(),
            ));
        }
        Ok(())
    }

    pub fn tick(
        &mut self,
        conn: &mut Connection,
        utc_ms: i64,
        mono_ms: u64,
        local_day: &str,
    ) -> Result<TickOutcome, AppError> {
        let mut alert = None;
        let mut data_changed = false;

        match self.state {
            TimerState::FocusRunning => {
                match self.advance_focus(conn, utc_ms, mono_ms, local_day) {
                    Ok(FocusAdvance::SleepPaused) => {
                        return Ok(TickOutcome {
                            snapshot: self.snapshot(conn)?,
                            alert: None,
                            data_changed: true,
                        });
                    }
                    Ok(FocusAdvance::Completed) => {
                        data_changed = true;
                        alert = Some(self.focus_complete_alert());
                    }
                    Ok(FocusAdvance::Advanced) => {}
                    Err(error) => {
                        log::warn!("focus completion failed: {error}");
                        data_changed = true;
                    }
                }

                if self.state == TimerState::FocusRunning
                    && self.elapsed_ms - self.last_data_event_ms as i64 >= DATA_EVENT_INTERVAL_MS
                {
                    self.last_data_event_ms = self.elapsed_ms as u64;
                    data_changed = true;
                }
            }
            TimerState::FocusPaused => {
                let _ = self.flush_pending(conn, utc_ms);
            }
            TimerState::BreakRunning => {
                let delta = self.delta(mono_ms);
                if delta > SLEEP_GAP_MS {
                    self.state = TimerState::BreakPaused;
                    self.last_mono_ms = None;
                    return Ok(TickOutcome {
                        snapshot: self.snapshot(conn)?,
                        alert: None,
                        data_changed: true,
                    });
                }

                self.last_mono_ms = Some(mono_ms);
                let total_ms = self.phase_total_ms();
                let credit = (delta as i64).min((total_ms - self.elapsed_ms).max(0));
                self.elapsed_ms += credit;

                if self.elapsed_ms >= total_ms {
                    self.state = TimerState::Idle;
                    self.phase = None;
                    self.elapsed_ms = 0;
                    self.last_mono_ms = None;
                    self.last_data_event_ms = 0;
                    data_changed = true;
                    alert = Some(TimerAlert {
                        kind: AlertKind::BreakComplete,
                        audible: !self.muted,
                        title: "Break complete".to_string(),
                        body: "Ready to focus again.".to_string(),
                    });
                }
            }
            _ => {}
        }

        Ok(TickOutcome {
            snapshot: self.snapshot(conn)?,
            alert,
            data_changed,
        })
    }

    fn advance_focus(
        &mut self,
        conn: &mut Connection,
        utc_ms: i64,
        mono_ms: u64,
        local_day: &str,
    ) -> Result<FocusAdvance, AppError> {
        if self.elapsed_ms >= FOCUS_MS {
            self.complete_focus(conn, utc_ms)?;
            return Ok(FocusAdvance::Completed);
        }

        let delta = self.delta(mono_ms);
        if delta > SLEEP_GAP_MS {
            let flush_at = utc_ms.saturating_sub(delta as i64);
            let _ = self.flush_pending(conn, flush_at);
            self.state = TimerState::FocusPaused;
            self.last_mono_ms = None;
            return Ok(FocusAdvance::SleepPaused);
        }

        self.last_mono_ms = Some(mono_ms);
        self.accrue_focus(delta, local_day);

        if self.elapsed_ms >= FOCUS_MS {
            self.complete_focus(conn, utc_ms)?;
            return Ok(FocusAdvance::Completed);
        }

        let _ = self.flush_pending(conn, utc_ms);
        Ok(FocusAdvance::Advanced)
    }

    fn accrue_focus(&mut self, delta_ms: u64, local_day: &str) {
        let remaining = (FOCUS_MS - self.elapsed_ms).max(0);
        let credit_ms = (delta_ms as i64).min(remaining);
        if credit_ms <= 0 {
            return;
        }

        let Some(session_id) = self.session_id.clone() else {
            return;
        };

        let key = PendingKey {
            session_id,
            todo_id: self.attributed_todo_id.clone(),
            local_day: local_day.to_string(),
        };
        *self.pending.entry(key).or_insert(0) += credit_ms;
        self.elapsed_ms += credit_ms;
    }

    fn complete_focus(&mut self, conn: &mut Connection, utc_ms: i64) -> Result<(), AppError> {
        let Some(session_id) = self.session_id.clone() else {
            let error = AppError::Storage("No active focus session to complete".to_string());
            self.persistence_warning = Some(PERSISTENCE_WARNING.to_string());
            return Err(error);
        };

        let entries: Vec<(PendingKey, i64)> = self
            .pending
            .iter()
            .map(|(key, delta)| (key.clone(), *delta))
            .collect();

        match Self::complete_focus_tx(conn, &session_id, &entries, utc_ms) {
            Ok(count) => {
                self.pending.clear();
                self.persistence_warning = None;
                self.completed_focus_count = count;
                self.phase = Some(if count % 4 == 0 {
                    TimerPhase::LongBreak
                } else {
                    TimerPhase::ShortBreak
                });
                self.state = TimerState::BreakPending;
                self.elapsed_ms = 0;
                self.session_id = None;
                self.attributed_todo_id = None;
                self.credit_todo_id = None;
                self.last_mono_ms = None;
                self.last_data_event_ms = 0;
                Ok(())
            }
            Err(error) => {
                self.persistence_warning = Some(PERSISTENCE_WARNING.to_string());
                Err(error)
            }
        }
    }

    fn complete_focus_tx(
        conn: &mut Connection,
        session_id: &str,
        entries: &[(PendingKey, i64)],
        utc_ms: i64,
    ) -> Result<i64, AppError> {
        let tx = conn.transaction()?;
        Self::upsert_pending_time(&tx, entries, utc_ms)?;
        if finish_session(&tx, session_id, "completed", utc_ms)? != 1 {
            return Err(AppError::Storage(format!(
                "Focus session {session_id} could not be completed"
            )));
        }
        let count = increment_completed_focus_count(&tx)?;
        tx.commit()?;
        Ok(count)
    }

    fn focus_complete_alert(&self) -> TimerAlert {
        let (title, body) = match self.phase {
            Some(TimerPhase::LongBreak) => ("Focus complete", "Take a long break."),
            _ => ("Focus complete", "Take a short break."),
        };

        TimerAlert {
            kind: AlertKind::FocusComplete,
            audible: !self.muted,
            title: title.to_string(),
            body: body.to_string(),
        }
    }

    fn flush_pending(&mut self, conn: &mut Connection, now: i64) -> Result<(), AppError> {
        if self.pending.is_empty() {
            self.persistence_warning = None;
            return Ok(());
        }

        let entries: Vec<(PendingKey, i64)> = self
            .pending
            .iter()
            .map(|(key, delta)| (key.clone(), *delta))
            .collect();

        match Self::write_pending(conn, &entries, now) {
            Ok(()) => {
                self.pending.clear();
                self.persistence_warning = None;
                Ok(())
            }
            Err(error) => {
                self.persistence_warning = Some(PERSISTENCE_WARNING.to_string());
                Err(error)
            }
        }
    }

    fn write_pending(
        conn: &mut Connection,
        entries: &[(PendingKey, i64)],
        now: i64,
    ) -> Result<(), AppError> {
        let tx = conn.transaction()?;
        let sessions: HashSet<&str> = entries
            .iter()
            .map(|(key, _)| key.session_id.as_str())
            .collect();

        Self::upsert_pending_time(&tx, entries, now)?;

        for session_id in sessions {
            update_checkpoint(&tx, session_id, now)?;
        }

        tx.commit()?;
        Ok(())
    }

    fn upsert_pending_time(
        conn: &Connection,
        entries: &[(PendingKey, i64)],
        now: i64,
    ) -> Result<(), AppError> {
        for (key, delta) in entries {
            let bucket_key = key.todo_id.as_deref().unwrap_or(UNASSIGNED_BUCKET);
            upsert_focus_time_tx(
                conn,
                &key.session_id,
                key.todo_id.as_deref(),
                bucket_key,
                &key.local_day,
                *delta,
                now,
            )?;
        }

        Ok(())
    }

    fn delta(&self, mono_ms: u64) -> u64 {
        self.last_mono_ms
            .map_or(0, |last| mono_ms.saturating_sub(last))
    }

    fn phase_total_ms(&self) -> i64 {
        match self.phase {
            Some(TimerPhase::Focus) | None => FOCUS_MS,
            Some(TimerPhase::ShortBreak) => SHORT_BREAK_MS,
            Some(TimerPhase::LongBreak) => LONG_BREAK_MS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::repo::todos::create_todo;

    struct Harness {
        conn: Connection,
        timer: TimerCore,
        utc: i64,
        mono: u64,
        day: String,
        todo_id: Option<String>,
    }

    impl Harness {
        fn new() -> Self {
            let mut conn = db::open_in_memory().unwrap();
            let todo = create_todo(&conn, "Task", Some(1), 0).unwrap();
            let timer = TimerCore::load(&mut conn).unwrap();
            Self {
                conn,
                timer,
                utc: 1_000,
                mono: 10_000,
                day: "2026-10-26".into(),
                todo_id: Some(todo.id),
            }
        }

        fn start(&mut self) {
            self.timer
                .start_focus(
                    &mut self.conn,
                    self.todo_id.clone(),
                    self.utc,
                    self.mono,
                    &self.day,
                )
                .unwrap();
        }

        fn second_todo(&self) -> String {
            create_todo(&self.conn, "Second", None, 1).unwrap().id
        }

        fn tick(&mut self, ms: u64) -> TickOutcome {
            self.utc += ms as i64;
            self.mono += ms;
            self.timer
                .tick(&mut self.conn, self.utc, self.mono, &self.day)
                .unwrap()
        }

        fn complete_focus(&mut self) -> TickOutcome {
            for _ in 0..1600 {
                let outcome = self.tick(1_000);
                if outcome.alert.is_some() {
                    return outcome;
                }
            }
            panic!("focus did not complete within 1600 ticks");
        }

        fn snapshot(&self) -> TimerSnapshot {
            self.timer.snapshot(&self.conn).unwrap()
        }
    }

    #[test]
    fn start_pause_resume_and_switch_keep_countdown_continuous() {
        let mut h = Harness::new();
        let first = h.todo_id.clone().unwrap();
        let second = h.second_todo();
        h.start();
        h.tick(5_000);
        h.tick(5_000);
        let before_pause = h.snapshot().remaining_ms;

        h.timer.pause(&mut h.conn, h.utc).unwrap();
        h.utc += 30_000;
        h.mono += 30_000;
        h.timer.resume(h.mono).unwrap();
        h.tick(5_000);
        assert_eq!(h.snapshot().remaining_ms, before_pause - 5_000);

        h.timer
            .start_focus(&mut h.conn, Some(second.clone()), h.utc, h.mono, &h.day)
            .unwrap();
        h.tick(2_000);
        let snap = h.snapshot();
        assert_eq!(snap.attributed_todo_id, Some(second));
        assert_eq!(snap.remaining_ms, before_pause - 7_000);

        let first_ms: i64 = h
            .conn
            .query_row(
                "SELECT COALESCE(SUM(active_ms), 0) FROM focus_time WHERE todo_id = ?1",
                [&first],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(first_ms, 15_000);
        let credit: Option<String> = h
            .conn
            .query_row(
                "SELECT credit_todo_id FROM focus_sessions LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(credit, None);
    }

    #[test]
    fn completion_credits_whole_phase_and_opens_short_break() {
        let mut h = Harness::new();
        let todo = h.todo_id.clone().unwrap();
        h.start();
        let outcome = h.complete_focus();
        assert_eq!(outcome.snapshot.state, TimerState::BreakPending);
        assert_eq!(outcome.snapshot.phase, Some(TimerPhase::ShortBreak));
        assert_eq!(outcome.snapshot.completed_focus_count, 1);
        assert_eq!(outcome.alert.unwrap().kind, AlertKind::FocusComplete);
        let outcome: String = h
            .conn
            .query_row("SELECT outcome FROM focus_sessions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(outcome, "completed");
        let credit: Option<String> = h
            .conn
            .query_row("SELECT credit_todo_id FROM focus_sessions", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(credit, Some(todo));
        let total: i64 = h
            .conn
            .query_row(
                "SELECT COALESCE(SUM(active_ms), 0) FROM focus_time",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(total, FOCUS_MS);
    }

    #[test]
    fn fourth_completion_opens_long_break() {
        let mut h = Harness::new();
        h.start();
        for completed in 1..=4 {
            let outcome = h.complete_focus();
            let expected = if completed == 4 {
                TimerPhase::LongBreak
            } else {
                TimerPhase::ShortBreak
            };
            assert_eq!(outcome.snapshot.phase, Some(expected));
            assert_eq!(outcome.snapshot.completed_focus_count, completed);
            if completed < 4 {
                h.start();
            }
        }
    }

    #[test]
    fn task_switch_splits_time_and_forfeits_credit() {
        let mut h = Harness::new();
        let first = h.todo_id.clone().unwrap();
        let second = h.second_todo();
        h.start();
        for _ in 0..300 {
            h.tick(1_000);
        }
        h.timer
            .start_focus(&mut h.conn, Some(second.clone()), h.utc, h.mono, &h.day)
            .unwrap();
        let outcome = h.complete_focus();
        assert_eq!(outcome.snapshot.completed_focus_count, 1);

        let first_ms: i64 = h
            .conn
            .query_row(
                "SELECT COALESCE(SUM(active_ms), 0) FROM focus_time WHERE todo_id = ?1",
                [&first],
                |row| row.get(0),
            )
            .unwrap();
        let second_ms: i64 = h
            .conn
            .query_row(
                "SELECT COALESCE(SUM(active_ms), 0) FROM focus_time WHERE todo_id = ?1",
                [&second],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(first_ms, 300_000);
        assert_eq!(second_ms, 1_200_000);
        let credit: Option<String> = h
            .conn
            .query_row("SELECT credit_todo_id FROM focus_sessions", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(credit, None);
    }

    #[test]
    fn abandon_keeps_time_but_no_cycle_or_estimate_credit() {
        let mut h = Harness::new();
        h.start();
        h.tick(5_000);
        h.timer.end_focus(&mut h.conn, h.utc).unwrap();
        let snap = h.snapshot();
        assert_eq!(snap.state, TimerState::Idle);
        assert_eq!(snap.completed_focus_count, 0);
        let outcome: String = h
            .conn
            .query_row("SELECT outcome FROM focus_sessions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(outcome, "abandoned");
        let credit: Option<String> = h
            .conn
            .query_row("SELECT credit_todo_id FROM focus_sessions", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(credit, h.todo_id);
        let total: i64 = h
            .conn
            .query_row(
                "SELECT COALESCE(SUM(active_ms), 0) FROM focus_time",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(total, 5_000);
    }

    #[test]
    fn skipped_break_returns_to_idle_without_changing_count() {
        let mut h = Harness::new();
        h.start();
        let completed = h.complete_focus().snapshot.completed_focus_count;
        h.timer.skip_break().unwrap();
        let snap = h.snapshot();
        assert_eq!(snap.state, TimerState::Idle);
        assert_eq!(snap.completed_focus_count, completed);
    }

    #[test]
    fn unassigned_focus_uses_unassigned_bucket() {
        let mut h = Harness::new();
        h.timer
            .start_focus(&mut h.conn, None, h.utc, h.mono, &h.day)
            .unwrap();
        h.tick(2_000);
        assert_eq!(h.snapshot().attributed_todo_id, None);
        let (todo_id, bucket_key, active_ms): (Option<String>, String, i64) = h
            .conn
            .query_row(
                "SELECT todo_id, bucket_key, active_ms FROM focus_time",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(todo_id, None);
        assert_eq!(bucket_key, "unassigned");
        assert_eq!(active_ms, 2_000);
    }

    #[test]
    fn focus_time_is_persisted_while_running() {
        let mut h = Harness::new();
        h.start();
        h.tick(2_000);
        let total: i64 = h
            .conn
            .query_row(
                "SELECT COALESCE(SUM(active_ms), 0) FROM focus_time",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(total, 2_000);
    }

    #[test]
    fn sleep_gap_pauses_without_crediting_gap() {
        let mut h = Harness::new();
        h.timer
            .start_focus(&mut h.conn, h.todo_id.clone(), h.utc, h.mono, &h.day)
            .unwrap();
        h.tick(1_000);
        let before = h.timer.snapshot(&h.conn).unwrap().remaining_ms;
        h.utc += 60_000;
        h.mono += 60_000;
        let out = h.timer.tick(&mut h.conn, h.utc, h.mono, &h.day).unwrap();
        assert_eq!(out.snapshot.state, TimerState::FocusPaused);
        assert_eq!(out.snapshot.remaining_ms, before);
        assert!(out.data_changed);
        let total: i64 = h
            .conn
            .query_row(
                "SELECT COALESCE(SUM(active_ms), 0) FROM focus_time",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(total, 1_000);
    }

    #[test]
    fn sleep_gap_pauses_break_without_crediting_gap() {
        let mut h = Harness::new();
        h.start();
        h.complete_focus();
        h.timer.start_break(h.mono).unwrap();
        h.tick(1_000);
        let before = h.snapshot().remaining_ms;
        h.utc += 60_000;
        h.mono += 60_000;
        let out = h.timer.tick(&mut h.conn, h.utc, h.mono, &h.day).unwrap();
        assert_eq!(out.snapshot.state, TimerState::BreakPaused);
        assert_eq!(out.snapshot.remaining_ms, before);
        assert!(out.data_changed);
    }

    #[test]
    fn startup_recovery_abandons_active_sessions() {
        let mut h = Harness::new();
        h.timer
            .start_focus(&mut h.conn, h.todo_id.clone(), h.utc, h.mono, &h.day)
            .unwrap();
        h.tick(1_000);
        let reloaded = TimerCore::load(&mut h.conn).unwrap();
        assert_eq!(reloaded.snapshot(&h.conn).unwrap().state, TimerState::Idle);
        let (outcome, ended_at): (String, Option<i64>) = h
            .conn
            .query_row("SELECT outcome, ended_at FROM focus_sessions", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(outcome, "abandoned");
        assert_eq!(ended_at, Some(2_000));
    }

    #[test]
    fn muted_completion_alert_is_silent() {
        let mut h = Harness::new();
        h.timer.set_muted(&mut h.conn, true).unwrap();
        h.start();
        let outcome = h.complete_focus();
        assert!(!outcome.alert.unwrap().audible);
    }

    #[test]
    fn flush_all_rejects_pending_data_until_it_can_be_saved() {
        let mut h = Harness::new();
        h.conn
            .execute_batch(
                "CREATE TRIGGER fail_focus_time BEFORE INSERT ON focus_time
                 BEGIN
                     SELECT RAISE(ABORT, 'forced checkpoint failure');
                 END;",
            )
            .unwrap();

        h.start();
        h.tick(2_000);
        assert!(h.timer.has_pending());
        assert!(h.timer.flush_all(&mut h.conn, h.utc).is_err());
        assert!(h.timer.has_pending());

        h.conn
            .execute_batch("DROP TRIGGER fail_focus_time;")
            .unwrap();
        h.timer.flush_all(&mut h.conn, h.utc).unwrap();
        assert!(!h.timer.has_pending());

        let total: i64 = h
            .conn
            .query_row(
                "SELECT COALESCE(SUM(active_ms), 0) FROM focus_time",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(total, 2_000);
    }

    #[test]
    fn failed_flush_retries_without_double_counting() {
        let mut h = Harness::new();
        h.conn
            .execute_batch(
                "CREATE TRIGGER fail_focus_time BEFORE INSERT ON focus_time
                 BEGIN
                     SELECT RAISE(ABORT, 'forced checkpoint failure');
                 END;",
            )
            .unwrap();

        h.start();
        h.tick(2_000);
        assert!(h.snapshot().persistence_warning.is_some());
        let total: i64 = h
            .conn
            .query_row(
                "SELECT COALESCE(SUM(active_ms), 0) FROM focus_time",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(total, 0);

        h.conn
            .execute_batch("DROP TRIGGER fail_focus_time;")
            .unwrap();
        h.tick(1_000);

        let snapshot = h.snapshot();
        assert_eq!(snapshot.persistence_warning, None);
        let (rows, total): (i64, i64) = h
            .conn
            .query_row(
                "SELECT COUNT(*), COALESCE(SUM(active_ms), 0) FROM focus_time",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(rows, 1);
        assert_eq!(total, 3_000);
    }

    #[test]
    fn switching_at_the_phase_boundary_credits_the_old_todo() {
        let mut h = Harness::new();
        let first = h.todo_id.clone().unwrap();
        let second = h.second_todo();
        h.start();
        for _ in 0..1499 {
            h.tick(1_000);
        }

        h.utc += 1_000;
        h.mono += 1_000;
        h.timer
            .start_focus(&mut h.conn, Some(second.clone()), h.utc, h.mono, &h.day)
            .unwrap();

        let snapshot = h.snapshot();
        assert_eq!(snapshot.state, TimerState::FocusRunning);
        assert_eq!(snapshot.attributed_todo_id, Some(second.clone()));
        assert_eq!(snapshot.completed_focus_count, 1);

        let credit: Option<String> = h
            .conn
            .query_row(
                "SELECT credit_todo_id FROM focus_sessions WHERE outcome = 'completed'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(credit, Some(first.clone()));

        let first_ms: i64 = h
            .conn
            .query_row(
                "SELECT COALESCE(SUM(active_ms), 0) FROM focus_time WHERE todo_id = ?1",
                [&first],
                |row| row.get(0),
            )
            .unwrap();
        let second_ms: i64 = h
            .conn
            .query_row(
                "SELECT COALESCE(SUM(active_ms), 0) FROM focus_time WHERE todo_id = ?1",
                [&second],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(first_ms, FOCUS_MS);
        assert_eq!(second_ms, 0);
    }

    #[test]
    fn switching_after_a_large_delta_does_not_credit_the_gap() {
        let mut h = Harness::new();
        let first = h.todo_id.clone().unwrap();
        let second = h.second_todo();
        h.start();
        h.tick(1_000);

        h.utc += 60_000;
        h.mono += 60_000;
        h.timer
            .start_focus(&mut h.conn, Some(second.clone()), h.utc, h.mono, &h.day)
            .unwrap();
        h.tick(1_000);

        let snapshot = h.snapshot();
        assert_eq!(snapshot.state, TimerState::FocusRunning);
        assert_eq!(snapshot.attributed_todo_id, Some(second.clone()));

        let first_ms: i64 = h
            .conn
            .query_row(
                "SELECT COALESCE(SUM(active_ms), 0) FROM focus_time WHERE todo_id = ?1",
                [&first],
                |row| row.get(0),
            )
            .unwrap();
        let second_ms: i64 = h
            .conn
            .query_row(
                "SELECT COALESCE(SUM(active_ms), 0) FROM focus_time WHERE todo_id = ?1",
                [&second],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(first_ms, 1_000);
        assert_eq!(second_ms, 1_000);
    }

    #[test]
    fn failed_clear_credit_does_not_double_accrue_the_switch_interval() {
        let mut h = Harness::new();
        let first = h.todo_id.clone().unwrap();
        let second = h.second_todo();
        h.start();
        h.tick(1_000);
        h.conn
            .execute_batch(
                "CREATE TRIGGER fail_clear_credit
                 BEFORE UPDATE OF credit_todo_id ON focus_sessions
                 WHEN NEW.credit_todo_id IS NULL
                 BEGIN
                     SELECT RAISE(ABORT, 'forced clear credit failure');
                 END;",
            )
            .unwrap();

        h.utc += 1_000;
        h.mono += 1_000;
        assert!(
            h.timer
                .start_focus(&mut h.conn, Some(second.clone()), h.utc, h.mono, &h.day)
                .is_err()
        );

        h.conn
            .execute_batch("DROP TRIGGER fail_clear_credit;")
            .unwrap();
        h.utc += 1_000;
        h.mono += 1_000;
        h.timer
            .start_focus(&mut h.conn, Some(second.clone()), h.utc, h.mono, &h.day)
            .unwrap();

        let first_ms: i64 = h
            .conn
            .query_row(
                "SELECT COALESCE(SUM(active_ms), 0) FROM focus_time WHERE todo_id = ?1",
                [&first],
                |row| row.get(0),
            )
            .unwrap();
        let second_ms: i64 = h
            .conn
            .query_row(
                "SELECT COALESCE(SUM(active_ms), 0) FROM focus_time WHERE todo_id = ?1",
                [&second],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(first_ms, 3_000);
        assert_eq!(second_ms, 0);
    }

    #[test]
    fn failed_completion_transaction_keeps_session_active_and_retries() {
        let mut h = Harness::new();
        h.start();
        for _ in 0..1499 {
            h.tick(1_000);
        }
        h.conn
            .execute_batch(
                "CREATE TRIGGER fail_completed_focus_count
                 BEFORE UPDATE ON settings
                 BEGIN
                     SELECT RAISE(ABORT, 'forced settings failure');
                 END;",
            )
            .unwrap();

        let failed = h.tick(1_000);
        assert_eq!(failed.snapshot.state, TimerState::FocusRunning);
        assert_eq!(failed.snapshot.completed_focus_count, 0);
        assert!(failed.snapshot.persistence_warning.is_some());
        assert!(failed.alert.is_none());
        assert!(h.timer.has_pending());

        let (outcome, ended_at): (String, Option<i64>) = h
            .conn
            .query_row("SELECT outcome, ended_at FROM focus_sessions", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        assert_eq!(outcome, "active");
        assert_eq!(ended_at, None);

        let persisted_ms: i64 = h
            .conn
            .query_row(
                "SELECT COALESCE(SUM(active_ms), 0) FROM focus_time",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(persisted_ms, 1_499_000);

        h.conn
            .execute_batch("DROP TRIGGER fail_completed_focus_count;")
            .unwrap();
        let completed = h.tick(1_000);

        assert_eq!(completed.snapshot.state, TimerState::BreakPending);
        assert_eq!(completed.snapshot.completed_focus_count, 1);
        assert_eq!(
            completed.alert.as_ref().map(|alert| &alert.kind),
            Some(&AlertKind::FocusComplete)
        );
        let total_ms: i64 = h
            .conn
            .query_row(
                "SELECT COALESCE(SUM(active_ms), 0) FROM focus_time",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let completed_sessions: i64 = h
            .conn
            .query_row(
                "SELECT COUNT(*) FROM focus_sessions WHERE outcome = 'completed'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(total_ms, FOCUS_MS);
        assert_eq!(completed_sessions, 1);
    }

    #[test]
    fn switching_back_to_the_original_todo_keeps_credit_forfeited() {
        let mut h = Harness::new();
        let first = h.todo_id.clone().unwrap();
        let second = h.second_todo();
        h.start();
        h.tick(1_000);
        h.timer
            .start_focus(&mut h.conn, Some(second), h.utc, h.mono, &h.day)
            .unwrap();
        h.tick(1_000);
        h.timer
            .start_focus(&mut h.conn, Some(first.clone()), h.utc, h.mono, &h.day)
            .unwrap();
        h.tick(1_000);
        h.timer.end_focus(&mut h.conn, h.utc).unwrap();

        let credit: Option<String> = h
            .conn
            .query_row("SELECT credit_todo_id FROM focus_sessions", [], |row| {
                row.get(0)
            })
            .unwrap();
        let first_ms: i64 = h
            .conn
            .query_row(
                "SELECT COALESCE(SUM(active_ms), 0) FROM focus_time WHERE todo_id = ?1",
                [&first],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(credit, None);
        assert_eq!(first_ms, 2_000);
    }
}
