use std::collections::BTreeMap;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::Value;

/// Application-level error type used by executable entrypoints.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// Configuration loading or parsing failed.
    #[error("configuration error: {0}")]
    Config(#[from] config::ConfigError),
    /// Database connection or migration failed.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    /// TCP socket binding failed.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// Command line arguments are invalid.
    #[error("{0}")]
    Cli(String),
    /// Service initialization failed.
    #[error("service initialization error: {0}")]
    ServiceInit(#[from] crate::service_init::ServiceInitError),
    /// User configuration initialization failed.
    #[error("config initialization error: {0}")]
    ConfigInit(#[from] crate::config_init::ConfigInitError),
    /// Global initialization failed.
    #[error("initialization error: {0}")]
    Init(#[from] crate::init::GlobalInitError),
}

/// HTTP API error type converted into JSON responses.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// Request JSON was syntactically invalid.
    #[error("Invalid JSON request body.")]
    InvalidJson,
    /// Request validation failed.
    #[error("Request validation failed.")]
    Validation,
    /// Note reference is too short to resolve safely.
    #[error("Note reference must be at least 4 characters.")]
    NoteReferenceTooShort,
    /// Note reference contains non-hex characters.
    #[error("Note reference must be a hexadecimal string.")]
    InvalidNoteReference,
    /// Requested record was not found.
    #[error("{0}")]
    RecordNotFound(String),
    /// Note reference matched multiple records.
    #[error("{0}")]
    AmbiguousNoteReference(String),
    /// Request conflicts with the current resource state.
    #[error("{0}")]
    Conflict(String),
    /// Database is not available for serving requests.
    #[allow(dead_code)]
    #[error("Database is not initialized.")]
    DatabaseNotInitialized,
    /// SQLite operation failed.
    #[error("Database operation failed.")]
    Database(#[from] sqlx::Error),
    /// Synchronization is disabled by runtime configuration.
    #[error("Synchronization is disabled.")]
    SyncDisabled,
    /// Synchronization operation failed.
    #[error("Synchronization failed: {0}")]
    SyncFailed(String),
    /// Runtime configuration validation failed.
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    /// Runtime configuration file operation failed.
    #[error("Configuration file operation failed.")]
    ConfigIo(#[from] std::io::Error),
}

impl ApiError {
    /// Returns the HTTP status code for this API error.
    ///
    /// # Returns
    ///
    /// Returns a status code aligned with the public error contract.
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::InvalidJson => StatusCode::BAD_REQUEST,
            Self::Validation | Self::NoteReferenceTooShort | Self::InvalidNoteReference => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            Self::RecordNotFound(_) => StatusCode::NOT_FOUND,
            Self::AmbiguousNoteReference(_) | Self::Conflict(_) => StatusCode::CONFLICT,
            Self::DatabaseNotInitialized => StatusCode::SERVICE_UNAVAILABLE,
            Self::SyncDisabled => StatusCode::SERVICE_UNAVAILABLE,
            Self::InvalidConfig(_) => StatusCode::BAD_REQUEST,
            Self::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::SyncFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::ConfigIo(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Returns the machine-readable error code for this API error.
    ///
    /// # Returns
    ///
    /// Returns a stable error code for clients.
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidJson => "invalid_json",
            Self::Validation => "validation_error",
            Self::NoteReferenceTooShort => "note_reference_too_short",
            Self::InvalidNoteReference => "invalid_note_reference",
            Self::RecordNotFound(_) => "record_not_found",
            Self::AmbiguousNoteReference(_) => "ambiguous_note_reference",
            Self::Conflict(_) => "conflict",
            Self::DatabaseNotInitialized => "database_not_initialized",
            Self::Database(_) => "database_error",
            Self::SyncDisabled => "sync_disabled",
            Self::SyncFailed(_) => "sync_failed",
            Self::InvalidConfig(_) => "invalid_config",
            Self::ConfigIo(_) => "config_io_failed",
        }
    }
}

impl IntoResponse for ApiError {
    /// Converts an API error into a JSON HTTP response.
    ///
    /// # Returns
    ///
    /// Returns a response with the error contract body and matching status code.
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = crate::dto::error::ErrorResponse {
            error: crate::dto::error::ErrorBody {
                code: self.code().to_string(),
                message: self.to_string(),
                details: BTreeMap::<String, Value>::new(),
            },
        };

        (status, Json(body)).into_response()
    }
}
