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
pub async fn list_tags(
    State(state): State<crate::app::AppState>,
    Query(query): Query<ListTaxonomyQuery>,
) -> Result<Json<ListTagsResponse>, ApiError> {
    let repository = TaxonomyRepository::new(state.database.pool);
    let tags = repository.list_tags(query.resolved_limit()).await?;
    let names = tags.iter().map(|tag| tag.name.clone()).collect();

    Ok(Json(ListTagsResponse { tags, names }))
}
