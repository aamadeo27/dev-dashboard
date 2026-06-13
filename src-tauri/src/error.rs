use serde::ser::SerializeStruct;
use thiserror::Error;

/// Application-level error type. All Tauri commands return `AppResult<T>`.
///
/// Serializes to `{ "code": "SCREAMING_SNAKE_CASE", "message": "...", "details": null }`.
/// The `details` field is always `null` in v1; reserved for structured context in future.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Already exists: {0}")]
    AlreadyExists(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Git error: {0}")]
    Git(String),

    #[error("CLI error: {0}")]
    Cli(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl AppError {
    /// Returns the SCREAMING_SNAKE_CASE code string for the variant.
    pub fn code(&self) -> &'static str {
        match self {
            AppError::NotFound(_) => "NOT_FOUND",
            AppError::AlreadyExists(_) => "ALREADY_EXISTS",
            AppError::Io(_) => "IO",
            AppError::Git(_) => "GIT",
            AppError::Cli(_) => "CLI",
            AppError::Parse(_) => "PARSE",
            AppError::InvalidInput(_) => "INVALID_INPUT",
            AppError::PermissionDenied(_) => "PERMISSION_DENIED",
            AppError::Internal(_) => "INTERNAL",
        }
    }
}

/// Manual `Serialize` impl — produces `{ "code": "...", "message": "...", "details": null }`.
impl serde::Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_struct("AppError", 3)?;
        s.serialize_field("code", self.code())?;
        s.serialize_field("message", &self.to_string())?;
        s.serialize_field("details", &Option::<serde_json::Value>::None)?;
        s.end()
    }
}

impl From<git2::Error> for AppError {
    fn from(e: git2::Error) -> Self {
        AppError::Git(e.message().to_owned())
    }
}

/// Convenience alias used throughout the Rust core.
pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_serializes_correct_code() {
        let err = AppError::NotFound("project-123".to_string());
        let v = serde_json::to_value(&err).expect("serialize");
        assert_eq!(v["code"], "NOT_FOUND");
        assert!(v["message"].as_str().unwrap().contains("project-123"));
        assert!(v["details"].is_null());
    }

    #[test]
    fn internal_serializes_correct_code() {
        let err = AppError::Internal("unexpected state".to_string());
        let v = serde_json::to_value(&err).expect("serialize");
        assert_eq!(v["code"], "INTERNAL");
        assert!(v["message"].as_str().unwrap().contains("unexpected state"));
        assert!(v["details"].is_null());
    }

    #[test]
    fn io_error_from_std_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let app_err: AppError = io_err.into();
        // Should be the Io variant
        assert!(matches!(app_err, AppError::Io(_)));
        let v = serde_json::to_value(&app_err).expect("serialize");
        assert_eq!(v["code"], "IO");
        assert!(v["message"].as_str().unwrap().contains("I/O error"));
    }

    #[test]
    fn all_variant_codes_are_correct() {
        let cases: &[(&str, AppError)] = &[
            ("NOT_FOUND", AppError::NotFound("x".into())),
            ("ALREADY_EXISTS", AppError::AlreadyExists("x".into())),
            ("GIT", AppError::Git("x".into())),
            ("CLI", AppError::Cli("x".into())),
            ("PARSE", AppError::Parse("x".into())),
            ("INVALID_INPUT", AppError::InvalidInput("x".into())),
            ("PERMISSION_DENIED", AppError::PermissionDenied("x".into())),
            ("INTERNAL", AppError::Internal("x".into())),
        ];
        for (expected_code, err) in cases {
            let v = serde_json::to_value(err).expect("serialize");
            assert_eq!(
                v["code"], *expected_code,
                "wrong code for variant: {}",
                expected_code
            );
        }
    }

    /// Verify that `From<git2::Error>` converts to the `Git` variant and that
    /// the resulting serialized `code` is `"GIT"`. This is the only test that
    /// exercises the manual `From<git2::Error>` impl (not covered by the
    /// table-driven test above, which constructs `Git` directly from a String).
    #[test]
    fn git_error_from_git2() {
        // git2::Error::from_str is the simplest constructor for a git2::Error
        // that does not require a live repository.
        let git2_err = git2::Error::from_str("repository not found");
        let app_err: AppError = git2_err.into();

        // Must resolve to the Git variant.
        assert!(matches!(app_err, AppError::Git(_)));

        let v = serde_json::to_value(&app_err).expect("serialize");
        assert_eq!(v["code"], "GIT");
        // The message() call strips whitespace; the word "repository" must survive.
        assert!(
            v["message"].as_str().unwrap().contains("repository"),
            "expected git error message to contain 'repository', got: {}",
            v["message"]
        );
        assert!(v["details"].is_null());
    }

    #[test]
    fn serialized_shape_has_three_fields() {
        let err = AppError::NotFound("test".into());
        let v = serde_json::to_value(&err).expect("serialize");
        let obj = v.as_object().expect("should be object");
        assert_eq!(
            obj.len(),
            3,
            "expected exactly 3 fields: code, message, details"
        );
        assert!(obj.contains_key("code"));
        assert!(obj.contains_key("message"));
        assert!(obj.contains_key("details"));
    }
}
