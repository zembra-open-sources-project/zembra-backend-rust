use axum::Json;
use axum::extract::State;

use crate::dto::workspaces::{ListWorkspacesResponse, WorkspaceSummary};
use crate::error::ApiError;
use crate::repositories::workspaces::{WorkspacesRepository, workspace_short_hash};

/// Lists local workspaces with visible note summaries.
///
/// # Arguments
///
/// * `state` - Shared application state.
///
/// # Returns
///
/// Returns ordered workspace summaries.
#[utoipa::path(
    get,
    path = "/workspaces",
    tag = "workspaces",
    responses(
        (status = 200, description = "Workspace summaries ordered by visible note count", body = ListWorkspacesResponse),
        (status = 500, description = "Database error", body = crate::dto::error::ErrorResponse)
    )
)]
pub async fn list_workspaces(
    State(state): State<crate::app::AppState>,
) -> Result<Json<ListWorkspacesResponse>, ApiError> {
    let repository = WorkspacesRepository::new(state.database.pool);
    let rows = repository.list_summaries().await?;
    let workspaces = rows
        .into_iter()
        .map(|row| WorkspaceSummary {
            short_hash: workspace_short_hash(&row.workspace_id),
            workspace_id: row.workspace_id,
            visible_note_count: row.visible_note_count,
            latest_note_created_at: row.latest_note_created_at,
        })
        .collect();

    Ok(Json(ListWorkspacesResponse { workspaces }))
}
