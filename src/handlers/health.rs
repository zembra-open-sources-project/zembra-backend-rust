use axum::{Json, extract::State};
use serde::Serialize;

/// Response body returned by the health endpoint.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    /// Machine-readable service status.
    pub status: &'static str,
    /// Stable service name exposed to clients.
    pub service: &'static str,
    /// Whether the SQLite database can answer basic queries.
    pub database_initialized: bool,
}

/// Returns the current service health status.
///
/// # Arguments
///
/// * `state` - Shared application state containing the database handle.
///
/// # Returns
///
/// Returns a JSON response indicating that the service process is healthy.
pub async fn health(State(state): State<crate::app::AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "zembra-server",
        database_initialized: state.database.is_initialized().await,
    })
}
