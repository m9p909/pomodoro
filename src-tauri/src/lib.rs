mod bindings;
mod clock;
mod commands;
mod db;
mod domain;
mod error;
mod repo;
mod state;
mod timer;

use std::fmt::Display;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{Emitter, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_notification::{NotificationExt, PermissionState};

use crate::clock::SystemClock;
use crate::commands::{EVENT_DATA_CHANGED, EVENT_TIMER_ALERT, EVENT_TIMER_SNAPSHOT};
use crate::state::{AppInner, AppState};

const TICK_INTERVAL: Duration = Duration::from_millis(500);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let specta_builder = bindings::builder();
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(specta_builder.invoke_handler())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            let data_dir = match app.path().app_data_dir() {
                Ok(data_dir) => data_dir,
                Err(error) => {
                    show_startup_failure(
                        app.handle(),
                        Path::new("<app data directory unavailable>"),
                        &error,
                    );
                    return Ok(());
                }
            };
            let db_path = data_dir.join("pomodoro.sqlite");
            if let Err(error) = std::fs::create_dir_all(&data_dir) {
                show_startup_failure(app.handle(), &db_path, &error);
                return Ok(());
            }
            let conn = match db::open(&db_path) {
                Ok(conn) => conn,
                Err(error) => {
                    show_startup_failure(app.handle(), &db_path, &error);
                    return Ok(());
                }
            };
            let inner = match AppInner::new(conn, Arc::new(SystemClock::new())) {
                Ok(inner) => inner,
                Err(error) => {
                    show_startup_failure(app.handle(), &db_path, &error);
                    return Ok(());
                }
            };
            app.manage(AppState {
                inner: Arc::new(Mutex::new(inner)),
                app: app.handle().clone(),
            });

            match app.notification().permission_state() {
                Ok(PermissionState::Granted | PermissionState::Denied) => {}
                Ok(_) => {
                    if let Err(error) = app.notification().request_permission() {
                        log::warn!("failed to request notification permission: {error}");
                    }
                }
                Err(error) => {
                    log::warn!("failed to read notification permission: {error}");
                }
            }

            let handle = app.handle().clone();
            std::thread::spawn(move || tick_loop(handle));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while building tauri application");
}

fn startup_error_message(db_path: &Path, error: &dyn Display) -> String {
    format!(
        "Pomodoro could not open its local database.\n\nDatabase: {}\n\nError: {error}",
        db_path.display()
    )
}

fn show_startup_failure(app: &tauri::AppHandle, db_path: &Path, error: &dyn Display) {
    log::error!("startup failed: {error}");
    let handle = app.clone();
    app.dialog()
        .message(startup_error_message(db_path, error))
        .title("Pomodoro could not start")
        .kind(MessageDialogKind::Error)
        .buttons(MessageDialogButtons::Ok)
        .show(move |_| {
            handle.exit(1);
        });
}

fn tick_loop(app: tauri::AppHandle) {
    loop {
        std::thread::sleep(TICK_INTERVAL);

        let outcome = {
            let state = app.state::<AppState>();
            let mut inner = match state.inner.lock() {
                Ok(inner) => inner,
                Err(_) => {
                    log::error!("app state lock was poisoned; stopping tick loop");
                    return;
                }
            };
            inner.tick()
        };

        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(error) => {
                log::warn!("timer tick failed: {error}");
                continue;
            }
        };

        if let Err(error) = app.emit(EVENT_TIMER_SNAPSHOT, outcome.snapshot) {
            log::warn!("failed to emit {EVENT_TIMER_SNAPSHOT}: {error}");
        }
        if outcome.data_changed
            && let Err(error) = app.emit(EVENT_DATA_CHANGED, ())
        {
            log::warn!("failed to emit {EVENT_DATA_CHANGED}: {error}");
        }
        if let Some(alert) = outcome.alert {
            if let Err(error) = app.emit(EVENT_TIMER_ALERT, alert.clone()) {
                log::warn!("failed to emit {EVENT_TIMER_ALERT}: {error}");
            }
            if alert.audible
                && notification_permission_granted(&app)
                && let Err(error) = app
                    .notification()
                    .builder()
                    .title(alert.title)
                    .body(alert.body)
                    .show()
            {
                log::warn!("failed to show native notification: {error}");
            }
        }
    }
}

fn notification_permission_granted(app: &tauri::AppHandle) -> bool {
    match app.notification().permission_state() {
        Ok(PermissionState::Granted) => true,
        Ok(_) => false,
        Err(error) => {
            log::warn!("failed to read notification permission: {error}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppError;
    use std::path::Path;

    #[test]
    fn startup_error_message_includes_database_path_and_error() {
        let path = Path::new("/tmp/example/pomodoro.sqlite");
        let message = startup_error_message(path, &AppError::Storage("disk full".to_string()));

        assert!(message.contains("/tmp/example/pomodoro.sqlite"));
        assert!(message.contains("disk full"));
    }
}
