use axum::Json;
use axum::extract::{Query, State};

use crate::dto::taxonomy::{ListFieldsResponse, ListTagsResponse, ListTaxonomyQuery};
use crate::error::ApiError;
use crate::repositories::taxonomy::TaxonomyRepository;

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
