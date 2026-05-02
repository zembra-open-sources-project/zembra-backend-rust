use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Json, response::IntoResponse};
use serde::Deserialize;
use validator::Validate;

use crate::dto::notes::{
    BatchCreateNotesRequest, CreateNoteRequest, ListNoteRevisionsResponse, ListNoteTagsResponse,
    ListNotesResponse, UpdateNoteRequest,
};
use crate::error::ApiError;
use crate::services::notes::NotesService;

/// Query parameters for listing notes.
#[derive(Debug, Deserialize)]
pub struct ListNotesQuery {
    /// Maximum number of notes to return.
    pub limit: Option<i64>,
}

/// Lists active notes.
///
/// # Arguments
///
/// * `state` - Shared application state.
/// * `query` - Optional list query parameters.
///
/// # Returns
///
/// Returns active notes ordered by update time.
pub async fn list_notes(
    State(state): State<crate::app::AppState>,
    Query(query): Query<ListNotesQuery>,
) -> Result<Json<ListNotesResponse>, ApiError> {
    let service = NotesService::new(state.database.pool);
    let notes = service.list_notes(query.limit).await?;

    Ok(Json(ListNotesResponse { notes }))
}

/// Creates a note.
///
/// # Arguments
///
/// * `state` - Shared application state.
/// * `payload` - JSON request body.
///
/// # Returns
///
/// Returns a `201 Created` response with the created note.
pub async fn create_note(
    State(state): State<crate::app::AppState>,
    payload: Result<Json<CreateNoteRequest>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Json(request) = payload.map_err(|_| ApiError::InvalidJson)?;
    request.validate().map_err(|_| ApiError::Validation)?;

    let service = NotesService::new(state.database.pool);
    let response = service.create_note(request).await?;

    Ok((StatusCode::CREATED, Json(response)))
}

/// Creates notes in a batch transaction.
///
/// # Arguments
///
/// * `state` - Shared application state.
/// * `payload` - JSON request body.
///
/// # Returns
///
/// Returns a `201 Created` response with created notes.
pub async fn create_notes_batch(
    State(state): State<crate::app::AppState>,
    payload: Result<Json<BatchCreateNotesRequest>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Json(request) = payload.map_err(|_| ApiError::InvalidJson)?;
    request.validate().map_err(|_| ApiError::Validation)?;

    let service = NotesService::new(state.database.pool);
    let response = service.create_notes_batch(request.items).await?;

    Ok((StatusCode::CREATED, Json(response)))
}

/// Reads a note by reference.
///
/// # Arguments
///
/// * `state` - Shared application state.
/// * `note_ref` - Full note ID or unique prefix.
///
/// # Returns
///
/// Returns the matching note.
pub async fn get_note(
    State(state): State<crate::app::AppState>,
    Path(note_ref): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let service = NotesService::new(state.database.pool);
    let note = service.get_note(&note_ref).await?;

    Ok(Json(note))
}

/// Updates a note by reference.
///
/// # Arguments
///
/// * `state` - Shared application state.
/// * `note_ref` - Full note ID or unique prefix.
/// * `payload` - JSON request body.
///
/// # Returns
///
/// Returns the updated note.
pub async fn update_note(
    State(state): State<crate::app::AppState>,
    Path(note_ref): Path<String>,
    payload: Result<Json<UpdateNoteRequest>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Json(request) = payload.map_err(|_| ApiError::InvalidJson)?;
    request.validate().map_err(|_| ApiError::Validation)?;

    let service = NotesService::new(state.database.pool);
    let note = service.update_note(&note_ref, request).await?;

    Ok(Json(note))
}

/// Archives a note by reference.
///
/// # Arguments
///
/// * `state` - Shared application state.
/// * `note_ref` - Full note ID or unique prefix.
///
/// # Returns
///
/// Returns the archived note.
pub async fn archive_note(
    State(state): State<crate::app::AppState>,
    Path(note_ref): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let service = NotesService::new(state.database.pool);
    let note = service.archive_note(&note_ref).await?;

    Ok(Json(note))
}

/// Soft deletes a note by reference.
///
/// # Arguments
///
/// * `state` - Shared application state.
/// * `note_ref` - Full note ID or unique prefix.
///
/// # Returns
///
/// Returns `204 No Content` after deletion.
pub async fn delete_note(
    State(state): State<crate::app::AppState>,
    Path(note_ref): Path<String>,
) -> Result<StatusCode, ApiError> {
    let service = NotesService::new(state.database.pool);
    service.delete_note(&note_ref).await?;

    Ok(StatusCode::NO_CONTENT)
}

/// Lists revisions for a note.
///
/// # Arguments
///
/// * `state` - Shared application state.
/// * `note_ref` - Full note ID or unique prefix.
///
/// # Returns
///
/// Returns note revisions.
pub async fn list_note_revisions(
    State(state): State<crate::app::AppState>,
    Path(note_ref): Path<String>,
) -> Result<Json<ListNoteRevisionsResponse>, ApiError> {
    let service = NotesService::new(state.database.pool);
    let revisions = service.list_note_revisions(&note_ref).await?;

    Ok(Json(ListNoteRevisionsResponse { revisions }))
}

/// Lists tags for a note.
///
/// # Arguments
///
/// * `state` - Shared application state.
/// * `note_ref` - Full note ID or unique prefix.
///
/// # Returns
///
/// Returns note tags.
pub async fn list_note_tags(
    State(state): State<crate::app::AppState>,
    Path(note_ref): Path<String>,
) -> Result<Json<ListNoteTagsResponse>, ApiError> {
    let service = NotesService::new(state.database.pool);
    let tags = service.list_note_tags(&note_ref).await?;

    Ok(Json(ListNoteTagsResponse { tags }))
}

/// Adds a tag to a note.
///
/// # Arguments
///
/// * `state` - Shared application state.
/// * `(note_ref, tag_name)` - Note reference and tag name.
///
/// # Returns
///
/// Returns the associated tag.
pub async fn add_tag_to_note(
    State(state): State<crate::app::AppState>,
    Path((note_ref, tag_name)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let service = NotesService::new(state.database.pool);
    let tag = service.add_tag_to_note(&note_ref, &tag_name).await?;

    Ok(Json(tag))
}

/// Removes a tag from a note.
///
/// # Arguments
///
/// * `state` - Shared application state.
/// * `(note_ref, tag_name)` - Note reference and tag name.
///
/// # Returns
///
/// Returns `204 No Content` after removal.
pub async fn remove_tag_from_note(
    State(state): State<crate::app::AppState>,
    Path((note_ref, tag_name)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let service = NotesService::new(state.database.pool);
    service.remove_tag_from_note(&note_ref, &tag_name).await?;

    Ok(StatusCode::NO_CONTENT)
}
