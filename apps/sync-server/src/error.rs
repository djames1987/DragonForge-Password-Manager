use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("sync record not found")]
    NotFound,
    #[error("sync revision conflict: current revision is {current_revision}")]
    Conflict { current_revision: u64 },
    #[error("account already exists")]
    AccountExists,
    #[error("device already exists with different identity")]
    DeviceExists,
    #[error("store failure")]
    Internal,
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("authentication required")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("invalid request: {0}")]
    BadRequest(String),
    #[error("not found")]
    NotFound,
    #[error("revision conflict")]
    Conflict { current_revision: u64 },
    #[error("service unavailable")]
    Unavailable,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorBody {
    code: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_revision: Option<u64>,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message, current_revision) = match self {
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "authentication required".to_owned(),
                None,
            ),
            Self::Forbidden => (
                StatusCode::FORBIDDEN,
                "forbidden",
                "request is not permitted".to_owned(),
                None,
            ),
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, "badRequest", message, None),
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                "notFound",
                "sync object was not found".to_owned(),
                None,
            ),
            Self::Conflict { current_revision } => (
                StatusCode::CONFLICT,
                "revisionConflict",
                "base revision does not match the current server revision".to_owned(),
                Some(current_revision),
            ),
            Self::Unavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "sync service is temporarily unavailable".to_owned(),
                None,
            ),
        };

        (
            status,
            Json(ErrorBody {
                code,
                message,
                current_revision,
            }),
        )
            .into_response()
    }
}

impl From<StoreError> for ApiError {
    fn from(value: StoreError) -> Self {
        match value {
            StoreError::NotFound => Self::NotFound,
            StoreError::Conflict { current_revision } => Self::Conflict { current_revision },
            StoreError::DeviceExists => Self::BadRequest(
                "device ID is already registered with a different identity".to_owned(),
            ),
            StoreError::AccountExists | StoreError::Internal => Self::Unavailable,
        }
    }
}
