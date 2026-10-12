//! Errors as JSON `{"error": "…"}` with a fitting status.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::files::FilesError;
use crate::store::StoreError;

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "not logged in")
    }

    /// Details go to the log, not to the client.
    fn internal() -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
    }
}

impl From<StoreError> for ApiError {
    fn from(err: StoreError) -> Self {
        match err {
            StoreError::NotFound(what) => {
                Self::new(StatusCode::NOT_FOUND, format!("{what} not found"))
            }
            StoreError::Invalid(message) => Self::new(StatusCode::BAD_REQUEST, message),
            other => {
                tracing::error!(err = %other, "store error");
                Self::internal()
            }
        }
    }
}

impl From<FilesError> for ApiError {
    fn from(err: FilesError) -> Self {
        match err {
            FilesError::NotFound(what) => {
                Self::new(StatusCode::NOT_FOUND, format!("{what} not found"))
            }
            FilesError::Invalid(message) => Self::new(StatusCode::BAD_REQUEST, message),
            // Paths on the server and the hub's API address stay in the log.
            FilesError::Failed(message) => {
                tracing::error!(%message, "file operation failed");
                Self::internal()
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_file_operation_does_not_reveal_its_details() {
        let failed = ApiError::from(FilesError::Failed("/data/folders/x: denied".into()));
        assert_eq!(failed.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(failed.message, "internal error");
        let invalid = ApiError::from(FilesError::Invalid("not a conflict copy".into()));
        assert_eq!(invalid.status, StatusCode::BAD_REQUEST);
        assert_eq!(invalid.message, "not a conflict copy");
        assert_eq!(
            ApiError::from(FilesError::NotFound("folder")).status,
            StatusCode::NOT_FOUND
        );
    }
}
