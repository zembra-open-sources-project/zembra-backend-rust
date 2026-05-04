use axum::Router;
use axum::routing::{get, post};

/// Builds routes for synchronization APIs.
///
/// # Returns
///
/// Returns a router exposing sync status and manual trigger endpoints.
pub fn router() -> Router<crate::app::AppState> {
    Router::new()
        .route("/sync/status", get(crate::handlers::sync::status))
        .route("/sync/run", post(crate::handlers::sync::run))
        .route("/sync/push", post(crate::handlers::sync::push))
        .route("/sync/pull", post(crate::handlers::sync::pull))
}
