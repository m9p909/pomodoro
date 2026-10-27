use std::sync::MutexGuard;

use tauri::{AppHandle, Emitter, State};

use crate::domain::{Bootstrap, DataSnapshot, TimerSnapshot, Todo};
use crate::error::AppError;
use crate::state::{AppInner, AppState};

pub const EVENT_TIMER_SNAPSHOT: &str = "timer-snapshot";
pub const EVENT_DATA_CHANGED: &str = "data-changed";
pub const EVENT_TIMER_ALERT: &str = "timer-alert";

fn lock_inner(state: &AppState) -> Result<MutexGuard<'_, AppInner>, AppError> {
    state
        .inner
        .lock()
        .map_err(|_| AppError::Storage("App state lock was poisoned".to_string()))
}

fn emit_data_changed(app: &AppHandle) {
    if let Err(error) = app.emit(EVENT_DATA_CHANGED, ()) {
        log::warn!("failed to emit {EVENT_DATA_CHANGED}: {error}");
    }
}

fn emit_timer_snapshot(app: &AppHandle, snapshot: &TimerSnapshot) {
    if let Err(error) = app.emit(EVENT_TIMER_SNAPSHOT, snapshot.clone()) {
        log::warn!("failed to emit {EVENT_TIMER_SNAPSHOT}: {error}");
    }
}

fn timer_update(app: &AppHandle, snapshot: TimerSnapshot) -> TimerSnapshot {
    emit_timer_snapshot(app, &snapshot);
    emit_data_changed(app);
    snapshot
}

#[tauri::command]
#[specta::specta]
pub fn bootstrap(state: State<'_, AppState>) -> Result<Bootstrap, AppError> {
    lock_inner(state.inner())?.bootstrap()
}

#[tauri::command]
#[specta::specta]
pub fn refresh_data(state: State<'_, AppState>) -> Result<DataSnapshot, AppError> {
    lock_inner(state.inner())?.refresh_data()
}

#[tauri::command]
#[specta::specta]
pub fn create_todo(
    state: State<'_, AppState>,
    title: String,
    estimate_pomodoros: Option<i64>,
) -> Result<Todo, AppError> {
    let todo = {
        let mut inner = lock_inner(state.inner())?;
        inner.create_todo(&title, estimate_pomodoros)?
    };
    emit_data_changed(&state.app);
    Ok(todo)
}

#[tauri::command]
#[specta::specta]
pub fn update_todo(
    state: State<'_, AppState>,
    id: String,
    title: String,
    estimate_pomodoros: Option<i64>,
) -> Result<Todo, AppError> {
    let todo = {
        let mut inner = lock_inner(state.inner())?;
        inner.update_todo(&id, &title, estimate_pomodoros)?
    };
    emit_data_changed(&state.app);
    Ok(todo)
}

#[tauri::command]
#[specta::specta]
pub fn set_todo_completed(
    state: State<'_, AppState>,
    id: String,
    completed: bool,
) -> Result<Todo, AppError> {
    let todo = {
        let mut inner = lock_inner(state.inner())?;
        inner.set_todo_completed(&id, completed)?
    };
    emit_data_changed(&state.app);
    Ok(todo)
}

#[tauri::command]
#[specta::specta]
pub fn delete_todo(state: State<'_, AppState>, id: String) -> Result<TimerSnapshot, AppError> {
    let snapshot = {
        let mut inner = lock_inner(state.inner())?;
        inner.delete_todo(&id)?
    };
    Ok(timer_update(&state.app, snapshot))
}

#[tauri::command]
#[specta::specta]
pub fn reorder_todos(state: State<'_, AppState>, ids: Vec<String>) -> Result<Vec<Todo>, AppError> {
    let todos = {
        let mut inner = lock_inner(state.inner())?;
        inner.reorder_todos(&ids)?
    };
    emit_data_changed(&state.app);
    Ok(todos)
}

#[tauri::command]
#[specta::specta]
pub fn start_focus(
    state: State<'_, AppState>,
    todo_id: Option<String>,
) -> Result<TimerSnapshot, AppError> {
    let snapshot = {
        let mut inner = lock_inner(state.inner())?;
        inner.start_focus(todo_id)?
    };
    Ok(timer_update(&state.app, snapshot))
}

#[tauri::command]
#[specta::specta]
pub fn pause_timer(state: State<'_, AppState>) -> Result<TimerSnapshot, AppError> {
    let snapshot = {
        let mut inner = lock_inner(state.inner())?;
        inner.pause_timer()?
    };
    Ok(timer_update(&state.app, snapshot))
}

#[tauri::command]
#[specta::specta]
pub fn resume_timer(state: State<'_, AppState>) -> Result<TimerSnapshot, AppError> {
    let snapshot = {
        let mut inner = lock_inner(state.inner())?;
        inner.resume_timer()?
    };
    Ok(timer_update(&state.app, snapshot))
}

#[tauri::command]
#[specta::specta]
pub fn end_focus(state: State<'_, AppState>) -> Result<TimerSnapshot, AppError> {
    let snapshot = {
        let mut inner = lock_inner(state.inner())?;
        inner.end_focus()?
    };
    Ok(timer_update(&state.app, snapshot))
}

#[tauri::command]
#[specta::specta]
pub fn start_break(state: State<'_, AppState>) -> Result<TimerSnapshot, AppError> {
    let snapshot = {
        let mut inner = lock_inner(state.inner())?;
        inner.start_break()?
    };
    Ok(timer_update(&state.app, snapshot))
}

#[tauri::command]
#[specta::specta]
pub fn skip_break(state: State<'_, AppState>) -> Result<TimerSnapshot, AppError> {
    let snapshot = {
        let mut inner = lock_inner(state.inner())?;
        inner.skip_break()?
    };
    Ok(timer_update(&state.app, snapshot))
}

#[tauri::command]
#[specta::specta]
pub fn set_muted(state: State<'_, AppState>, muted: bool) -> Result<TimerSnapshot, AppError> {
    let snapshot = {
        let mut inner = lock_inner(state.inner())?;
        inner.set_muted(muted)?
    };
    Ok(timer_update(&state.app, snapshot))
}

#[tauri::command]
#[specta::specta]
pub fn close_app(state: State<'_, AppState>) -> Result<(), AppError> {
    {
        let mut inner = lock_inner(state.inner())?;
        inner.close_flush()?;
    }
    state.app.exit(0);
    Ok(())
}
