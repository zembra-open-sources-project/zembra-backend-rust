use axum::Json;
use axum::extract::State;

use crate::dto::sync::{
    SyncConfigResponse, SyncConfigTestResponse, SyncDirectionResponse, SyncRunResponse,
    SyncStatusResponse, TestSyncConfigRequest, UpdateSyncConfigRequest,
};
use crate::error::ApiError;

/// Returns synchronization status without exposing secrets.
///
/// # Arguments
///
/// * `state` - Shared application state.
///
/// # Returns
///
/// Returns sync enabled state and cursor rows.
#[utoipa::path(
    get,
    path = "/sync/status",
    responses(
        (status = 200, description = "Synchronization status", body = SyncStatusResponse),
        (status = 500, description = "Database error", body = crate::dto::error::ErrorResponse)
    ),
    tag = "sync"
)]
pub async fn status(
    State(state): State<crate::app::AppState>,
) -> Result<Json<SyncStatusResponse>, ApiError> {
    state
        .sync
        .status()
        .await
        .map(SyncStatusResponse::from)
        .map(Json)
        .map_err(sync_error_to_api_error)
}

/// Returns persisted synchronization configuration without exposing secrets.
///
/// # Arguments
///
/// * `state` - Shared application state.
///
/// # Returns
///
/// Returns sync settings safe for frontend display.
#[utoipa::path(
    get,
    path = "/sync/config",
    responses(
        (status = 200, description = "Synchronization configuration", body = SyncConfigResponse),
        (status = 500, description = "Configuration read error", body = crate::dto::error::ErrorResponse)
    ),
    tag = "sync"
)]
pub async fn config(
    State(state): State<crate::app::AppState>,
) -> Result<Json<SyncConfigResponse>, ApiError> {
    state.sync_config.read_response().map(Json)
}

/// Persists synchronization configuration and updates runtime settings.
///
/// # Arguments
///
/// * `state` - Shared application state.
/// * `request` - New synchronization configuration.
///
/// # Returns
///
/// Returns saved sync settings without exposing secrets.
#[utoipa::path(
    put,
    path = "/sync/config",
    request_body = UpdateSyncConfigRequest,
    responses(
        (status = 200, description = "Synchronization configuration saved", body = SyncConfigResponse),
        (status = 400, description = "Invalid synchronization configuration", body = crate::dto::error::ErrorResponse),
        (status = 500, description = "Configuration write error", body = crate::dto::error::ErrorResponse)
    ),
    tag = "sync"
)]
pub async fn update_config(
    State(state): State<crate::app::AppState>,
    Json(request): Json<UpdateSyncConfigRequest>,
) -> Result<Json<SyncConfigResponse>, ApiError> {
    let settings = state.sync_config.save(request)?;
    state.sync.update_settings(settings.clone());
    Ok(Json(crate::services::sync_config::sync_config_response(
        settings,
    )))
}

/// Tests synchronization configuration without saving it.
///
/// # Arguments
///
/// * `state` - Shared application state.
/// * `request` - Candidate configuration values.
///
/// # Returns
///
/// Returns a sanitized connectivity result.
#[utoipa::path(
    post,
    path = "/sync/config/test",
    request_body = TestSyncConfigRequest,
    responses(
        (status = 200, description = "Synchronization configuration test result", body = SyncConfigTestResponse),
        (status = 400, description = "Invalid synchronization configuration", body = crate::dto::error::ErrorResponse),
        (status = 500, description = "Configuration read error", body = crate::dto::error::ErrorResponse)
    ),
    tag = "sync"
)]
pub async fn test_config(
    State(state): State<crate::app::AppState>,
    Json(request): Json<TestSyncConfigRequest>,
) -> Result<Json<SyncConfigTestResponse>, ApiError> {
    state.sync_config.test_connection(request).await.map(Json)
}

/// Runs one push and pull synchronization cycle.
///
/// # Arguments
///
/// * `state` - Shared application state.
///
/// # Returns
///
/// Returns a summary of pushed and pulled changes.
#[utoipa::path(
    post,
    path = "/sync/run",
    responses(
        (status = 200, description = "Synchronization cycle finished", body = SyncRunResponse),
        (status = 503, description = "Synchronization disabled", body = crate::dto::error::ErrorResponse),
        (status = 500, description = "Synchronization error", body = crate::dto::error::ErrorResponse)
    ),
    tag = "sync"
)]
pub async fn run(
    State(state): State<crate::app::AppState>,
) -> Result<Json<SyncRunResponse>, ApiError> {
    state
        .sync
        .run_once()
        .await
        .map(SyncRunResponse::from)
        .map(Json)
        .map_err(sync_error_to_api_error)
}

/// Pushes local changes to Supabase.
///
/// # Arguments
///
/// * `state` - Shared application state.
///
/// # Returns
///
/// Returns the number of pushed changes.
#[utoipa::path(
    post,
    path = "/sync/push",
    responses(
        (status = 200, description = "Push finished", body = SyncDirectionResponse),
        (status = 503, description = "Synchronization disabled", body = crate::dto::error::ErrorResponse),
        (status = 500, description = "Synchronization error", body = crate::dto::error::ErrorResponse)
    ),
    tag = "sync"
)]
pub async fn push(
    State(state): State<crate::app::AppState>,
) -> Result<Json<SyncDirectionResponse>, ApiError> {
    state
        .sync
        .push()
        .await
        .map(SyncDirectionResponse::from)
        .map(Json)
        .map_err(sync_error_to_api_error)
}

/// Pulls remote changes from Supabase.
///
/// # Arguments
///
/// * `state` - Shared application state.
///
/// # Returns
///
/// Returns the number of pulled changes.
#[utoipa::path(
    post,
    path = "/sync/pull",
    responses(
        (status = 200, description = "Pull finished", body = SyncDirectionResponse),
        (status = 503, description = "Synchronization disabled", body = crate::dto::error::ErrorResponse),
        (status = 500, description = "Synchronization error", body = crate::dto::error::ErrorResponse)
    ),
    tag = "sync"
)]
pub async fn pull(
    State(state): State<crate::app::AppState>,
) -> Result<Json<SyncDirectionResponse>, ApiError> {
    state
        .sync
        .pull()
        .await
        .map(SyncDirectionResponse::from)
        .map(Json)
        .map_err(sync_error_to_api_error)
}

/// Converts sync service errors into public API errors.
///
/// # Arguments
///
/// * `error` - Sync service error.
///
/// # Returns
///
/// Returns an API error without secrets.
fn sync_error_to_api_error(error: crate::services::sync::SyncError) -> ApiError {
    match error {
        crate::services::sync::SyncError::Disabled => ApiError::SyncDisabled,
        crate::services::sync::SyncError::Database(error) => ApiError::Database(error),
        crate::services::sync::SyncError::Supabase(error) => {
            ApiError::SyncFailed(error.to_string())
        }
        crate::services::sync::SyncError::Conflict { .. }
        | crate::services::sync::SyncError::NotConverged { .. } => {
            ApiError::SyncFailed(error.to_string())
        }
    }
}
