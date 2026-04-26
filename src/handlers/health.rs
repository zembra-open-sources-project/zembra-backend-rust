use axum::Json;
use serde::Serialize;

/// Response body returned by the health endpoint.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    /// Machine-readable service status.
    pub status: &'static str,
}

/// Returns the current service health status.
///
/// # Returns
///
/// Returns a JSON response indicating that the service process is healthy.
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}
