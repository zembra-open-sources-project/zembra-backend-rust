use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Sync cursor DTO returned by status APIs.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SyncStateResponse {
    /// Workspace that owns this cursor.
    pub workspace_id: String,
    /// Device that owns this cursor.
    pub device_id: String,
    /// Cursor direction.
    pub scope: String,
    /// Last processed change timestamp.
    pub last_change_created_at: i64,
    /// Last processed change identifier.
    pub last_change_id: String,
    /// Last successful sync timestamp.
    pub last_success_at: Option<i64>,
    /// Last failed sync timestamp.
    pub last_error_at: Option<i64>,
    /// Last failed sync message.
    pub last_error_message: Option<String>,
}

/// Sync status response.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SyncStatusResponse {
    /// Whether synchronization is enabled.
    pub enabled: bool,
    /// Local sync cursor rows.
    pub states: Vec<SyncStateResponse>,
}

/// Response returned by a manual sync run.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SyncRunResponse {
    /// Number of local changes pushed.
    pub pushed: usize,
    /// Number of remote changes pulled.
    pub pulled: usize,
}

/// Response returned by one sync direction.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SyncDirectionResponse {
    /// Number of changes processed.
    pub processed: usize,
}

/// Sync configuration response that never exposes the Supabase secret key.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SyncConfigResponse {
    /// Whether synchronization is enabled.
    pub enabled: bool,
    /// Delay in seconds between background synchronization attempts.
    pub interval_seconds: u64,
    /// Supabase project URL used by the backend REST client.
    pub supabase_url: String,
    /// Whether a Supabase secret key is currently configured.
    pub secret_key_configured: bool,
}

/// Request used to persist synchronization settings.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateSyncConfigRequest {
    /// Whether synchronization should be enabled.
    pub enabled: bool,
    /// Delay in seconds between background synchronization attempts.
    pub interval_seconds: u64,
    /// Supabase project URL used by the backend REST client.
    pub supabase_url: String,
    /// Optional new Supabase secret key.
    pub secret_key: Option<String>,
}

/// Request used to test synchronization settings without persisting them.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct TestSyncConfigRequest {
    /// Optional Supabase project URL used only for this test.
    pub supabase_url: Option<String>,
    /// Optional Supabase secret key used only for this test.
    pub secret_key: Option<String>,
}

/// Response returned by a Supabase configuration connectivity test.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SyncConfigTestResponse {
    /// Whether the Supabase REST endpoint accepted the test request.
    pub ok: bool,
    /// Sanitized test result message.
    pub message: String,
}

impl From<crate::repositories::sync::SyncStateRecord> for SyncStateResponse {
    /// Converts a repository sync state into an API response.
    ///
    /// # Returns
    ///
    /// Returns a sync state response.
    fn from(state: crate::repositories::sync::SyncStateRecord) -> Self {
        Self {
            workspace_id: state.workspace_id,
            device_id: state.device_id,
            scope: state.scope,
            last_change_created_at: state.last_change_created_at,
            last_change_id: state.last_change_id,
            last_success_at: state.last_success_at,
            last_error_at: state.last_error_at,
            last_error_message: state.last_error_message,
        }
    }
}

impl From<crate::services::sync::SyncStatus> for SyncStatusResponse {
    /// Converts a sync service status into an API response.
    ///
    /// # Returns
    ///
    /// Returns a sync status response.
    fn from(status: crate::services::sync::SyncStatus) -> Self {
        Self {
            enabled: status.enabled,
            states: status.states.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<crate::services::sync::SyncRunSummary> for SyncRunResponse {
    /// Converts a sync run summary into an API response.
    ///
    /// # Returns
    ///
    /// Returns a sync run response.
    fn from(summary: crate::services::sync::SyncRunSummary) -> Self {
        Self {
            pushed: summary.pushed,
            pulled: summary.pulled,
        }
    }
}

impl From<crate::services::sync::SyncDirectionSummary> for SyncDirectionResponse {
    /// Converts a sync direction summary into an API response.
    ///
    /// # Returns
    ///
    /// Returns a sync direction response.
    fn from(summary: crate::services::sync::SyncDirectionSummary) -> Self {
        Self {
            processed: summary.processed,
        }
    }
}
