use axum::{Json, extract::State};
use serde::Serialize;
use utoipa::ToSchema;

/// Response body returned by the health endpoint.
#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    /// Machine-readable service status.
    pub status: &'static str,
    /// Stable service name exposed to clients.
    pub service: &'static str,
    /// Whether the SQLite database can answer basic queries.
    pub database_initialized: bool,
    /// Cargo package version compiled into the running backend.
    pub version: &'static str,
    /// Versioning policy declared in repository TOML metadata.
    pub version_policy: String,
    /// Release channel declared in repository TOML metadata.
    pub release_channel: String,
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
#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses(
        (status = 200, description = "Service health status", body = HealthResponse)
    )
)]
pub async fn health(State(state): State<crate::app::AppState>) -> Json<HealthResponse> {
    let version = crate::version::version_info();

    Json(HealthResponse {
        status: "ok",
        service: "zembra-server",
        database_initialized: state.database.is_initialized().await,
        version: version.version,
        version_policy: version.version_policy.clone(),
        release_channel: version.release_channel.clone(),
    })
}
