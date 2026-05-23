use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;

/// Default device used by the backend until explicit device registration exists.
pub const DEFAULT_DEVICE_ID: &str = "local-backend";

/// Input used to record a local synchronization change.
#[derive(Debug, Clone)]
pub struct SyncChangeInput {
    /// Entity type affected by this change.
    pub entity_type: &'static str,
    /// Entity identifier affected by this change.
    pub entity_id: String,
    /// Operation applied to the entity.
    pub operation: &'static str,
    /// Optional base revision identifier.
    pub base_revision_id: Option<String>,
    /// Optional new revision identifier.
    pub new_revision_id: Option<String>,
    /// JSON payload containing the entity snapshot or relation change.
    pub payload: Value,
}

/// Synchronization change row used by tests and sync services.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct SyncChangeRecord {
    /// Unique change identifier.
    pub id: String,
    /// Workspace that owns this change.
    pub workspace_id: String,
    /// Device that produced this change.
    pub device_id: String,
    /// Entity type affected by this change.
    pub entity_type: String,
    /// Entity identifier affected by this change.
    pub entity_id: String,
    /// Operation applied to the entity.
    pub operation: String,
    /// Optional base revision identifier.
    pub base_revision_id: Option<String>,
    /// Optional new revision identifier.
    pub new_revision_id: Option<String>,
    /// JSON payload stored as text in SQLite.
    pub payload: String,
    /// Unix timestamp for change creation.
    pub created_at: i64,
    /// Unix timestamp for local application.
    pub applied_at: Option<i64>,
    /// Unix timestamp for Supabase commit.
    pub supabase_committed_at: Option<i64>,
}

/// Sync cursor row for one workspace, device, and direction.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct SyncStateRecord {
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
