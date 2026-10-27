use tauri_specta::{Builder, ErrorHandlingMode, collect_commands};

pub fn builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .error_handling(ErrorHandlingMode::Throw)
        .dangerously_cast_bigints_to_number()
        .typ::<crate::domain::TimerAlert>()
        .commands(collect_commands![
            crate::commands::bootstrap,
            crate::commands::refresh_data,
            crate::commands::create_todo,
            crate::commands::update_todo,
            crate::commands::set_todo_completed,
            crate::commands::delete_todo,
            crate::commands::reorder_todos,
            crate::commands::start_focus,
            crate::commands::pause_timer,
            crate::commands::resume_timer,
            crate::commands::end_focus,
            crate::commands::start_break,
            crate::commands::skip_break,
            crate::commands::set_muted,
            crate::commands::close_app,
        ])
}

#[cfg(test)]
mod tests {
    #[test]
    fn export_typescript_bindings() {
        let path = "../src/lib/bindings.ts";
        super::builder()
            .export(specta_typescript::Typescript::default(), path)
            .expect("failed to export TypeScript bindings");
    }
}
