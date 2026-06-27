use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Query, State};
use validator::Validate;

use crate::dto::taxonomy::{
    DeleteFieldRequest, DeleteFieldResponse, ListFieldsResponse, ListTagsResponse,
    ListTaxonomyQuery,
};
use crate::error::ApiError;
use crate::repositories::taxonomy::{DeleteFieldError, TaxonomyRepository};
use crate::repositories::workspaces::WorkspacesRepository;

/// Lists fields ordered by name.
///
/// # Arguments
///
/// * `state` - Shared application state.
/// * `query` - List query parameters.
///
/// # Returns
///
/// Returns fields and their names.
#[utoipa::path(
    get,
    path = "/fields",
    tag = "taxonomy",
    params(crate::dto::taxonomy::ListTaxonomyQuery),
    responses(
        (status = 200, description = "Fields ordered by name", body = ListFieldsResponse),
        (status = 500, description = "Database error", body = crate::dto::error::ErrorResponse)
    )
)]
pub async fn list_fields(
    State(state): State<crate::app::AppState>,
    Query(query): Query<ListTaxonomyQuery>,
) -> Result<Json<ListFieldsResponse>, ApiError> {
    let repository = TaxonomyRepository::new(state.database.pool);
    let fields = repository.list_fields(query.resolved_limit()).await?;
    let names = fields.iter().map(|field| field.name.clone()).collect();

    Ok(Json(ListFieldsResponse { fields, names }))
}

/// Deletes a field when no visible notes still use it.
///
/// # Arguments
///
/// * `state` - Shared application state.
/// * `payload` - JSON request body.
///
/// # Returns
///
/// Returns the deleted field identifier.
#[utoipa::path(
    post,
    path = "/fields/delete",
    tag = "taxonomy",
    request_body = DeleteFieldRequest,
    responses(
        (status = 200, description = "Field deleted", body = DeleteFieldResponse),
        (status = 400, description = "Invalid JSON", body = crate::dto::error::ErrorResponse),
        (status = 404, description = "Workspace or field not found", body = crate::dto::error::ErrorResponse),
        (status = 409, description = "Field is still used by visible notes", body = crate::dto::error::ErrorResponse),
        (status = 422, description = "Validation error", body = crate::dto::error::ErrorResponse),
        (status = 500, description = "Database error", body = crate::dto::error::ErrorResponse)
    )
)]
pub async fn delete_field(
    State(state): State<crate::app::AppState>,
    payload: Result<Json<DeleteFieldRequest>, JsonRejection>,
) -> Result<Json<DeleteFieldResponse>, ApiError> {
    let Json(request) = payload.map_err(|_| ApiError::InvalidJson)?;
    request.validate().map_err(|_| ApiError::Validation)?;

    let workspace_repository = WorkspacesRepository::new(state.database.pool.clone());
    workspace_repository
        .ensure_active(&request.workspace_id)
        .await?;

    let field_id = request.field_id.clone();
    let repository = TaxonomyRepository::new(state.database.pool);
    repository
        .delete_unused_field(&request.workspace_id, &request.field_id)
        .await
        .map_err(delete_field_error)?;

    Ok(Json(DeleteFieldResponse {
        field_id,
        deleted: true,
    }))
}

/// Lists tags ordered by name.
///
/// # Arguments
///
/// * `state` - Shared application state.
/// * `query` - List query parameters.
///
/// # Returns
///
/// Returns tags and their names.
#[utoipa::path(
    get,
    path = "/tags",
    tag = "taxonomy",
    params(crate::dto::taxonomy::ListTaxonomyQuery),
    responses(
        (status = 200, description = "Tags ordered by name", body = ListTagsResponse),
        (status = 500, description = "Database error", body = crate::dto::error::ErrorResponse)
    )
)]
pub async fn list_tags(
    State(state): State<crate::app::AppState>,
    Query(query): Query<ListTaxonomyQuery>,
) -> Result<Json<ListTagsResponse>, ApiError> {
    let repository = TaxonomyRepository::new(state.database.pool);
    let tags = repository.list_tags(query.resolved_limit()).await?;
    let names = tags.iter().map(|tag| tag.path.clone()).collect();

    Ok(Json(ListTagsResponse { tags, names }))
}

/// Maps taxonomy field deletion errors into HTTP API errors.
///
/// # Arguments
///
/// * `error` - Repository deletion error.
///
/// # Returns
///
/// Returns the public API error for the deletion failure.
fn delete_field_error(error: DeleteFieldError) -> ApiError {
    match error {
        DeleteFieldError::NotFound {
            workspace_id,
            field_id,
        } => ApiError::RecordNotFound(format!(
            "Field \"{field_id}\" did not match any field in workspace \"{workspace_id}\"."
        )),
        DeleteFieldError::InUse {
            field_id,
            visible_note_count,
        } => ApiError::Conflict(format!(
            "Field \"{field_id}\" is still used by {visible_note_count} visible note(s)."
        )),
        DeleteFieldError::Database(error) => ApiError::Database(error),
    }
}
