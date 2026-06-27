use axum::Router;
use axum::routing::{get, post};

/// Builds routes for field and tag lookup.
///
/// # Returns
///
/// Returns a router exposing taxonomy endpoints.
pub fn router() -> Router<crate::app::AppState> {
    Router::new()
        .route("/fields", get(crate::handlers::taxonomy::list_fields))
        .route(
            "/fields/delete",
            post(crate::handlers::taxonomy::delete_field),
        )
        .route("/tags", get(crate::handlers::taxonomy::list_tags))
}
