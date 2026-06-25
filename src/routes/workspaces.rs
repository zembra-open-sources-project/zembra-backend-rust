use axum::Router;
use axum::routing::get;

/// Builds routes for workspace metadata.
///
/// # Returns
///
/// Returns a router exposing workspace endpoints.
pub fn router() -> Router<crate::app::AppState> {
    Router::new().route(
        "/workspaces",
        get(crate::handlers::workspaces::list_workspaces),
    )
}
