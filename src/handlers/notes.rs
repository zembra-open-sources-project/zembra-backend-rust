use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Json, response::IntoResponse};
use validator::Validate;

use crate::dto::notes::{
    BatchCreateNotesRequest, BatchCreateNotesResponse, CreateNoteRequest, DailyNoteCountsResponse,
    FieldNotesResponse, ListNoteRevisionsResponse, ListNoteTagsResponse, ListNotesQuery,
    ListNotesResponse, NoteResponse, NotesByDateQuery, NotesByDateResponse, RandomFieldsQuery,
    RandomNotesQuery, RandomTagsQuery, RecentNotesRequest, TaggedNotesResponse, UpdateNoteRequest,
    WorkspaceQuery,
};
use crate::error::ApiError;
use crate::repositories::workspaces::{WorkspacesRepository, workspace_not_found};
use crate::services::notes::NotesService;

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
#[utoipa::path(
    get,
    path = "/notes",
    tag = "notes",
    params(ListNotesQuery),
    responses(
        (status = 200, description = "Active notes ordered by update time", body = ListNotesResponse),
        (status = 500, description = "Database error", body = crate::dto::error::ErrorResponse)
    )
)]
pub async fn list_notes(
    State(state): State<crate::app::AppState>,
    Query(query): Query<ListNotesQuery>,
) -> Result<Json<ListNotesResponse>, ApiError> {
    let workspace_id = active_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let service = NotesService::new(state.database.pool);
    let notes = service.list_notes(&workspace_id, query.limit).await?;

    Ok(Json(ListNotesResponse { notes }))
}

/// Lists recent notes for Web presentation.
///
/// # Arguments
///
/// * `state` - Shared application state.
/// * `payload` - JSON request body.
///
/// # Returns
///
/// Returns non-deleted and non-archived notes ordered by update time.
#[utoipa::path(
    post,
    path = "/notes/recent",
    tag = "notes",
    params(WorkspaceQuery),
    request_body = RecentNotesRequest,
    responses(
        (status = 200, description = "Recent non-archived notes ordered by update time", body = ListNotesResponse),
        (status = 400, description = "Invalid JSON", body = crate::dto::error::ErrorResponse),
        (status = 422, description = "Validation error", body = crate::dto::error::ErrorResponse),
        (status = 500, description = "Database error", body = crate::dto::error::ErrorResponse)
    )
)]
pub async fn recent_notes(
    State(state): State<crate::app::AppState>,
    Query(query): Query<WorkspaceQuery>,
    payload: Result<Json<RecentNotesRequest>, JsonRejection>,
) -> Result<Json<ListNotesResponse>, ApiError> {
    let Json(request) = payload.map_err(|_| ApiError::InvalidJson)?;
    request.validate().map_err(|_| ApiError::Validation)?;

    let workspace_id = active_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let service = NotesService::new(state.database.pool);
    let notes = service.recent_notes(&workspace_id, request).await?;

    Ok(Json(ListNotesResponse { notes }))
}

/// Counts notes created per day over the past 30 days.
///
/// # Arguments
///
/// * `state` - Shared application state.
///
/// # Returns
///
/// Returns daily visible note counts ordered by date ascending.
#[utoipa::path(
    get,
    path = "/notes/stats/daily-counts",
    tag = "notes",
    params(WorkspaceQuery),
    responses(
        (status = 200, description = "Daily visible note counts for the past 30 days", body = DailyNoteCountsResponse),
        (status = 500, description = "Database error", body = crate::dto::error::ErrorResponse)
    )
)]
pub async fn daily_note_counts(
    State(state): State<crate::app::AppState>,
    Query(query): Query<WorkspaceQuery>,
) -> Result<Json<DailyNoteCountsResponse>, ApiError> {
    let workspace_id = active_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let service = NotesService::new(state.database.pool);
    let response = service.daily_note_counts(&workspace_id).await?;

    Ok(Json(response))
}

/// Lists notes created on one server-local date.
///
/// # Arguments
///
/// * `state` - Shared application state.
/// * `query` - Notes-by-date query parameters.
///
/// # Returns
///
/// Returns visible notes created on the requested date.
#[utoipa::path(
    get,
    path = "/notes/by-date",
    tag = "notes",
    params(NotesByDateQuery),
    responses(
        (status = 200, description = "Visible notes created on the requested date", body = NotesByDateResponse),
        (status = 422, description = "Validation error", body = crate::dto::error::ErrorResponse),
        (status = 500, description = "Database error", body = crate::dto::error::ErrorResponse)
    )
)]
pub async fn notes_by_date(
    State(state): State<crate::app::AppState>,
    Query(query): Query<NotesByDateQuery>,
) -> Result<Json<NotesByDateResponse>, ApiError> {
    let workspace_id = active_workspace_id(&state, query.workspace_id.as_deref()).await?;
    query.validate().map_err(|_| ApiError::Validation)?;
    let service = NotesService::new(state.database.pool);
    let response = service.notes_by_date(&workspace_id, query).await?;

    Ok(Json(response))
}

/// Lists random visible notes.
///
/// # Arguments
///
/// * `state` - Shared application state.
/// * `query` - Random notes query parameters.
///
/// # Returns
///
/// Returns random non-deleted and non-archived notes.
#[utoipa::path(
    get,
    path = "/random/notes",
    tag = "notes",
    params(RandomNotesQuery),
    responses(
        (status = 200, description = "Random visible notes", body = ListNotesResponse),
        (status = 422, description = "Validation error", body = crate::dto::error::ErrorResponse),
        (status = 500, description = "Database error", body = crate::dto::error::ErrorResponse)
    )
)]
pub async fn random_notes(
    State(state): State<crate::app::AppState>,
    Query(query): Query<RandomNotesQuery>,
) -> Result<Json<ListNotesResponse>, ApiError> {
    let workspace_id = active_workspace_id(&state, query.workspace_id.as_deref()).await?;
    query.validate().map_err(|_| ApiError::Validation)?;
    let service = NotesService::new(state.database.pool);
    let notes = service.random_notes(&workspace_id, query).await?;

    Ok(Json(ListNotesResponse { notes }))
}

/// Lists notes grouped by randomly selected tags.
///
/// # Arguments
///
/// * `state` - Shared application state.
/// * `query` - Random tags query parameters.
///
/// # Returns
///
/// Returns random tag groups with their visible notes.
#[utoipa::path(
    get,
    path = "/random/tags",
    tag = "notes",
    params(RandomTagsQuery),
    responses(
        (status = 200, description = "Random tags with their visible notes", body = TaggedNotesResponse),
        (status = 422, description = "Validation error", body = crate::dto::error::ErrorResponse),
        (status = 500, description = "Database error", body = crate::dto::error::ErrorResponse)
    )
)]
pub async fn random_tagged_notes(
    State(state): State<crate::app::AppState>,
    Query(query): Query<RandomTagsQuery>,
) -> Result<Json<TaggedNotesResponse>, ApiError> {
    let workspace_id = active_workspace_id(&state, query.workspace_id.as_deref()).await?;
    query.validate().map_err(|_| ApiError::Validation)?;
    let service = NotesService::new(state.database.pool);
    let response = service.random_tagged_notes(&workspace_id, query).await?;

    Ok(Json(response))
}

/// Lists notes grouped by randomly selected fields.
///
/// # Arguments
///
/// * `state` - Shared application state.
/// * `query` - Random fields query parameters.
///
/// # Returns
///
/// Returns random field groups with their visible notes.
#[utoipa::path(
    get,
    path = "/random/fields",
    tag = "notes",
    params(RandomFieldsQuery),
    responses(
        (status = 200, description = "Random fields with their visible notes", body = FieldNotesResponse),
        (status = 422, description = "Validation error", body = crate::dto::error::ErrorResponse),
        (status = 500, description = "Database error", body = crate::dto::error::ErrorResponse)
    )
)]
pub async fn random_field_notes(
    State(state): State<crate::app::AppState>,
    Query(query): Query<RandomFieldsQuery>,
) -> Result<Json<FieldNotesResponse>, ApiError> {
    let workspace_id = active_workspace_id(&state, query.workspace_id.as_deref()).await?;
    query.validate().map_err(|_| ApiError::Validation)?;
    let service = NotesService::new(state.database.pool);
    let response = service.random_field_notes(&workspace_id, query).await?;

    Ok(Json(response))
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
#[utoipa::path(
    post,
    path = "/notes",
    tag = "notes",
    params(WorkspaceQuery),
    request_body = CreateNoteRequest,
    responses(
        (status = 201, description = "Note created", body = NoteResponse),
        (status = 400, description = "Invalid JSON", body = crate::dto::error::ErrorResponse),
        (status = 422, description = "Validation error", body = crate::dto::error::ErrorResponse),
        (status = 500, description = "Database error", body = crate::dto::error::ErrorResponse)
    )
)]
pub async fn create_note(
    State(state): State<crate::app::AppState>,
    Query(query): Query<WorkspaceQuery>,
    payload: Result<Json<CreateNoteRequest>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Json(request) = payload.map_err(|_| ApiError::InvalidJson)?;
    request.validate().map_err(|_| ApiError::Validation)?;

    let workspace_id = active_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let service = NotesService::new(state.database.pool);
    let response = service.create_note(&workspace_id, request).await?;

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
#[utoipa::path(
    post,
    path = "/notes/batch",
    tag = "notes",
    params(WorkspaceQuery),
    request_body = BatchCreateNotesRequest,
    responses(
        (status = 201, description = "Notes created", body = BatchCreateNotesResponse),
        (status = 400, description = "Invalid JSON", body = crate::dto::error::ErrorResponse),
        (status = 422, description = "Validation error", body = crate::dto::error::ErrorResponse),
        (status = 500, description = "Database error", body = crate::dto::error::ErrorResponse)
    )
)]
pub async fn create_notes_batch(
    State(state): State<crate::app::AppState>,
    Query(query): Query<WorkspaceQuery>,
    payload: Result<Json<BatchCreateNotesRequest>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Json(request) = payload.map_err(|_| ApiError::InvalidJson)?;
    request.validate().map_err(|_| ApiError::Validation)?;

    let workspace_id = active_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let service = NotesService::new(state.database.pool);
    let response = service
        .create_notes_batch(&workspace_id, request.items)
        .await?;

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
/// Returns the matching note with metadata.
#[utoipa::path(
    get,
    path = "/notes/{note_ref}",
    tag = "notes",
    params(
        WorkspaceQuery,
        ("note_ref" = String, Path, description = "Full 32-character note ID or at least 4-character hex prefix")
    ),
    responses(
        (status = 200, description = "Matching note", body = NoteResponse),
        (status = 404, description = "Note not found", body = crate::dto::error::ErrorResponse),
        (status = 409, description = "Ambiguous note reference", body = crate::dto::error::ErrorResponse),
        (status = 422, description = "Invalid note reference", body = crate::dto::error::ErrorResponse),
        (status = 500, description = "Database error", body = crate::dto::error::ErrorResponse)
    )
)]
pub async fn get_note(
    State(state): State<crate::app::AppState>,
    Path(note_ref): Path<String>,
    Query(query): Query<WorkspaceQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let workspace_id = active_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let service = NotesService::new(state.database.pool);
    let note = service.get_note(&workspace_id, &note_ref).await?;

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
/// Returns the updated note with metadata.
#[utoipa::path(
    patch,
    path = "/notes/{note_ref}",
    tag = "notes",
    params(
        WorkspaceQuery,
        ("note_ref" = String, Path, description = "Full 32-character note ID or at least 4-character hex prefix")
    ),
    request_body = UpdateNoteRequest,
    responses(
        (status = 200, description = "Updated note", body = NoteResponse),
        (status = 400, description = "Invalid JSON", body = crate::dto::error::ErrorResponse),
        (status = 404, description = "Note not found", body = crate::dto::error::ErrorResponse),
        (status = 409, description = "Ambiguous note reference", body = crate::dto::error::ErrorResponse),
        (status = 422, description = "Validation or note reference error", body = crate::dto::error::ErrorResponse),
        (status = 500, description = "Database error", body = crate::dto::error::ErrorResponse)
    )
)]
pub async fn update_note(
    State(state): State<crate::app::AppState>,
    Path(note_ref): Path<String>,
    Query(query): Query<WorkspaceQuery>,
    payload: Result<Json<UpdateNoteRequest>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Json(request) = payload.map_err(|_| ApiError::InvalidJson)?;
    request.validate().map_err(|_| ApiError::Validation)?;

    let workspace_id = active_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let service = NotesService::new(state.database.pool);
    let note = service
        .update_note(&workspace_id, &note_ref, request)
        .await?;

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
#[utoipa::path(
    post,
    path = "/notes/{note_ref}/archive",
    tag = "notes",
    params(
        WorkspaceQuery,
        ("note_ref" = String, Path, description = "Full 32-character note ID or at least 4-character hex prefix")
    ),
    responses(
        (status = 200, description = "Archived note", body = crate::models::note::NoteRecord),
        (status = 404, description = "Note not found", body = crate::dto::error::ErrorResponse),
        (status = 409, description = "Ambiguous note reference", body = crate::dto::error::ErrorResponse),
        (status = 422, description = "Invalid note reference", body = crate::dto::error::ErrorResponse),
        (status = 500, description = "Database error", body = crate::dto::error::ErrorResponse)
    )
)]
pub async fn archive_note(
    State(state): State<crate::app::AppState>,
    Path(note_ref): Path<String>,
    Query(query): Query<WorkspaceQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let workspace_id = active_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let service = NotesService::new(state.database.pool);
    let note = service.archive_note(&workspace_id, &note_ref).await?;

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
#[utoipa::path(
    delete,
    path = "/notes/{note_ref}",
    tag = "notes",
    params(
        WorkspaceQuery,
        ("note_ref" = String, Path, description = "Full 32-character note ID or at least 4-character hex prefix")
    ),
    responses(
        (status = 204, description = "Note soft deleted"),
        (status = 404, description = "Note not found", body = crate::dto::error::ErrorResponse),
        (status = 409, description = "Ambiguous note reference", body = crate::dto::error::ErrorResponse),
        (status = 422, description = "Invalid note reference", body = crate::dto::error::ErrorResponse),
        (status = 500, description = "Database error", body = crate::dto::error::ErrorResponse)
    )
)]
pub async fn delete_note(
    State(state): State<crate::app::AppState>,
    Path(note_ref): Path<String>,
    Query(query): Query<WorkspaceQuery>,
) -> Result<StatusCode, ApiError> {
    let workspace_id = active_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let service = NotesService::new(state.database.pool);
    service.delete_note(&workspace_id, &note_ref).await?;

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
#[utoipa::path(
    get,
    path = "/notes/{note_ref}/revisions",
    tag = "notes",
    params(
        WorkspaceQuery,
        ("note_ref" = String, Path, description = "Full 32-character note ID or at least 4-character hex prefix")
    ),
    responses(
        (status = 200, description = "Note revisions", body = ListNoteRevisionsResponse),
        (status = 404, description = "Note not found", body = crate::dto::error::ErrorResponse),
        (status = 409, description = "Ambiguous note reference", body = crate::dto::error::ErrorResponse),
        (status = 422, description = "Invalid note reference", body = crate::dto::error::ErrorResponse),
        (status = 500, description = "Database error", body = crate::dto::error::ErrorResponse)
    )
)]
pub async fn list_note_revisions(
    State(state): State<crate::app::AppState>,
    Path(note_ref): Path<String>,
    Query(query): Query<WorkspaceQuery>,
) -> Result<Json<ListNoteRevisionsResponse>, ApiError> {
    let workspace_id = active_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let service = NotesService::new(state.database.pool);
    let revisions = service
        .list_note_revisions(&workspace_id, &note_ref)
        .await?;

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
#[utoipa::path(
    get,
    path = "/notes/{note_ref}/tags",
    tag = "notes",
    params(
        WorkspaceQuery,
        ("note_ref" = String, Path, description = "Full 32-character note ID or at least 4-character hex prefix")
    ),
    responses(
        (status = 200, description = "Note tags", body = ListNoteTagsResponse),
        (status = 404, description = "Note not found", body = crate::dto::error::ErrorResponse),
        (status = 409, description = "Ambiguous note reference", body = crate::dto::error::ErrorResponse),
        (status = 422, description = "Invalid note reference", body = crate::dto::error::ErrorResponse),
        (status = 500, description = "Database error", body = crate::dto::error::ErrorResponse)
    )
)]
pub async fn list_note_tags(
    State(state): State<crate::app::AppState>,
    Path(note_ref): Path<String>,
    Query(query): Query<WorkspaceQuery>,
) -> Result<Json<ListNoteTagsResponse>, ApiError> {
    let workspace_id = active_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let service = NotesService::new(state.database.pool);
    let tags = service.list_note_tags(&workspace_id, &note_ref).await?;

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
#[utoipa::path(
    put,
    path = "/notes/{note_ref}/tags/{tag_name}",
    tag = "notes",
    params(
        WorkspaceQuery,
        ("note_ref" = String, Path, description = "Full 32-character note ID or at least 4-character hex prefix"),
        ("tag_name" = String, Path, description = "Tag name to associate")
    ),
    responses(
        (status = 200, description = "Associated tag", body = crate::models::tag::TagRecord),
        (status = 404, description = "Note not found", body = crate::dto::error::ErrorResponse),
        (status = 409, description = "Ambiguous note reference", body = crate::dto::error::ErrorResponse),
        (status = 422, description = "Validation or note reference error", body = crate::dto::error::ErrorResponse),
        (status = 500, description = "Database error", body = crate::dto::error::ErrorResponse)
    )
)]
pub async fn add_tag_to_note(
    State(state): State<crate::app::AppState>,
    Path((note_ref, tag_name)): Path<(String, String)>,
    Query(query): Query<WorkspaceQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let workspace_id = active_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let service = NotesService::new(state.database.pool);
    let tag = service
        .add_tag_to_note(&workspace_id, &note_ref, &tag_name)
        .await?;

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
#[utoipa::path(
    delete,
    path = "/notes/{note_ref}/tags/{tag_name}",
    tag = "notes",
    params(
        WorkspaceQuery,
        ("note_ref" = String, Path, description = "Full 32-character note ID or at least 4-character hex prefix"),
        ("tag_name" = String, Path, description = "Tag name to remove")
    ),
    responses(
        (status = 204, description = "Tag association removed"),
        (status = 404, description = "Note not found", body = crate::dto::error::ErrorResponse),
        (status = 409, description = "Ambiguous note reference", body = crate::dto::error::ErrorResponse),
        (status = 422, description = "Validation or note reference error", body = crate::dto::error::ErrorResponse),
        (status = 500, description = "Database error", body = crate::dto::error::ErrorResponse)
    )
)]
pub async fn remove_tag_from_note(
    State(state): State<crate::app::AppState>,
    Path((note_ref, tag_name)): Path<(String, String)>,
    Query(query): Query<WorkspaceQuery>,
) -> Result<StatusCode, ApiError> {
    let workspace_id = active_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let service = NotesService::new(state.database.pool);
    service
        .remove_tag_from_note(&workspace_id, &note_ref, &tag_name)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

/// Resolves and verifies the active workspace id for a notes request.
///
/// # Arguments
///
/// * `state` - Shared application state.
/// * `workspace_id` - Optional workspace id from the query string.
///
/// # Returns
///
/// Returns the active workspace id or a not-found API error.
async fn active_workspace_id(
    state: &crate::app::AppState,
    workspace_id: Option<&str>,
) -> Result<String, ApiError> {
    let workspace_id = workspace_id
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| workspace_not_found(""))?;
    let repository = WorkspacesRepository::new(state.database.pool.clone());
    repository.ensure_active(workspace_id).await?;

    Ok(workspace_id.to_string())
}
