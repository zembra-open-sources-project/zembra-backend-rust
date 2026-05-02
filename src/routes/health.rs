use axum::{Router, routing::get};

/// Builds routes for service health checks.
///
/// # Returns
///
/// Returns a router exposing infrastructure health endpoints.
pub fn router() -> Router<crate::app::AppState> {
    Router::new().route("/health", get(crate::handlers::health::health))
}
