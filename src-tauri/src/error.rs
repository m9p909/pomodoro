use serde::Serialize;
use specta::Type;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub enum AppError {
    Validation(String),
    NotFound(String),
    Storage(String),
    Timer(String),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation(m) | Self::NotFound(m) | Self::Storage(m) | Self::Timer(m) => {
                f.write_str(m)
            }
        }
    }
}

impl std::error::Error for AppError {}

impl From<rusqlite::Error> for AppError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Storage(value.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::Storage(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_as_tagged_kind_and_message() {
        let value = serde_json::to_value(AppError::Validation("title is required".into())).unwrap();
        assert_eq!(
            value,
            serde_json::json!({ "kind": "validation", "message": "title is required" })
        );
    }
}
