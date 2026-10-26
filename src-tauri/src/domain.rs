#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::error::AppError;

pub const FOCUS_MS: i64 = 25 * 60 * 1000;
pub const SHORT_BREAK_MS: i64 = 5 * 60 * 1000;
pub const LONG_BREAK_MS: i64 = 15 * 60 * 1000;
pub const SLEEP_GAP_MS: u64 = 5_000;

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum TimerState {
    Idle,
    FocusRunning,
    FocusPaused,
    BreakPending,
    BreakRunning,
    BreakPaused,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum TimerPhase {
    Focus,
    ShortBreak,
    LongBreak,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum AlertKind {
    FocusComplete,
    BreakComplete,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimerSnapshot {
    pub state: TimerState,
    pub phase: Option<TimerPhase>,
    pub remaining_ms: i64,
    pub total_ms: i64,
    pub attributed_todo_id: Option<String>,
    pub attributed_todo_title: Option<String>,
    pub completed_focus_count: i64,
    pub muted: bool,
    pub persistence_warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Todo {
    pub id: String,
    pub title: String,
    pub estimate_pomodoros: Option<i64>,
    pub sort_order: i64,
    pub completed_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub completed_pomodoros: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FocusRow {
    pub key: String,
    pub todo_id: Option<String>,
    pub title: String,
    pub active_ms: i64,
    pub completed_pomodoros: i64,
    pub estimate_pomodoros: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DailyAnalytics {
    pub date: String,
    pub total_ms: i64,
    pub rows: Vec<FocusRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WeeklyRow {
    pub key: String,
    pub todo_id: Option<String>,
    pub title: String,
    pub cells: Vec<i64>,
    pub total_ms: i64,
    pub completed_pomodoros: i64,
    pub estimate_pomodoros: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WeeklyAnalytics {
    pub days: Vec<String>,
    pub rows: Vec<WeeklyRow>,
    pub day_totals: Vec<i64>,
    pub total_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsSnapshot {
    pub today: DailyAnalytics,
    pub week: WeeklyAnalytics,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DataSnapshot {
    pub todos: Vec<Todo>,
    pub analytics: AnalyticsSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    pub todos: Vec<Todo>,
    pub timer: TimerSnapshot,
    pub analytics: AnalyticsSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimerAlert {
    pub kind: AlertKind,
    pub audible: bool,
    pub title: String,
    pub body: String,
}

pub fn validate_title(title: &str) -> Result<String, AppError> {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation("Title is required".to_string()));
    }
    Ok(trimmed.to_string())
}

pub fn validate_estimate(estimate: Option<i64>) -> Result<Option<i64>, AppError> {
    match estimate {
        Some(value) if value <= 0 => Err(AppError::Validation(
            "Estimate must be greater than zero".to_string(),
        )),
        other => Ok(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_is_trimmed_and_required() {
        assert_eq!(validate_title("  Write tests  ").unwrap(), "Write tests");
        assert!(validate_title("   ").is_err());
    }

    #[test]
    fn estimate_is_optional_but_positive() {
        assert_eq!(validate_estimate(None).unwrap(), None);
        assert_eq!(validate_estimate(Some(3)).unwrap(), Some(3));
        assert!(validate_estimate(Some(0)).is_err());
        assert!(validate_estimate(Some(-1)).is_err());
    }

    #[test]
    fn dto_fields_serialize_as_camel_case() {
        let todo = Todo {
            id: "id".into(),
            title: "Title".into(),
            estimate_pomodoros: Some(1),
            sort_order: 0,
            completed_at: None,
            created_at: 1,
            updated_at: 2,
            completed_pomodoros: 0,
        };
        let value = serde_json::to_value(todo).unwrap();
        assert!(value.get("estimatePomodoros").is_some());
        assert!(value.get("sortOrder").is_some());
        assert!(value.get("completedAt").is_some());
        assert!(value.get("createdAt").is_some());
        assert!(value.get("updatedAt").is_some());
        assert!(value.get("completedPomodoros").is_some());
    }
}
