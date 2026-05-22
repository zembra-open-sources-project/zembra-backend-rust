use axum::Router;
use axum::routing::{get, post, put};

/// Builds routes for notes CRUD.
///
/// # Returns
///
/// Returns a router exposing note endpoints.
pub fn router() -> Router<crate::app::AppState> {
    Router::new()
        .route(
            "/notes",
            get(crate::handlers::notes::list_notes).post(crate::handlers::notes::create_note),
        )
        .route("/notes/recent", post(crate::handlers::notes::recent_notes))
        .route(
            "/notes/stats/daily-counts",
            get(crate::handlers::notes::daily_note_counts),
        )
        .route("/notes/by-date", get(crate::handlers::notes::notes_by_date))
        .route(
            "/random/tags",
            get(crate::handlers::notes::random_tagged_notes),
        )
        .route("/random/notes", get(crate::handlers::notes::random_notes))
        .route(
            "/random/fields",
            get(crate::handlers::notes::random_field_notes),
        )
        .route(
            "/notes/batch",
            post(crate::handlers::notes::create_notes_batch),
        )
        .route(
            "/notes/{note_ref}",
            get(crate::handlers::notes::get_note)
                .patch(crate::handlers::notes::update_note)
                .delete(crate::handlers::notes::delete_note),
        )
        .route(
            "/notes/{note_ref}/archive",
            post(crate::handlers::notes::archive_note),
        )
        .route(
            "/notes/{note_ref}/tags",
            get(crate::handlers::notes::list_note_tags),
        )
        .route(
            "/notes/{note_ref}/tags/{tag_name}",
            put(crate::handlers::notes::add_tag_to_note)
                .delete(crate::handlers::notes::remove_tag_from_note),
        )
        .route(
            "/notes/{note_ref}/revisions",
            get(crate::handlers::notes::list_note_revisions),
        )
}
