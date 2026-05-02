use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;
use utoipa::ToSchema;

/// Top-level API error response.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ErrorResponse {
    /// Structured error payload.
    pub error: ErrorBody,
}

/// Structured API error body.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ErrorBody {
    /// Machine-readable error code.
    pub code: String,
    /// Human-readable error message.
    pub message: String,
    /// Additional structured details for clients.
    #[schema(value_type = Object)]
    pub details: BTreeMap<String, Value>,
}
