use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;

/// Default device used by the backend until explicit device registration exists.
pub const DEFAULT_DEVICE_ID: &str = "local-backend";

/// Input used to record a local synchronization change.
#[derive(Debug, Clone)]
pub struct SyncChangeInput {
    /// Workspace that owns this change.
    pub workspace_id: String,
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

/// Remote synchronization entity kind used for apply dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RemoteEntityKind {
    /// Field entity.
    Field,
    /// Tag entity.
    Tag,
    /// Note entity.
    Note,
    /// Note revision entity.
    NoteRevision,
    /// Note tag relation.
    NoteTag,
    /// Note link relation.
    NoteLink,
}

impl RemoteEntityKind {
    /// Parses a remote entity kind from a sync change record.
    ///
    /// # Arguments
    ///
    /// * `value` - Raw entity type from a sync change.
    ///
    /// # Returns
    ///
    /// Returns a typed entity kind or `None` for unsupported values.
    pub(super) fn parse(value: &str) -> Option<Self> {
        match value {
            "field" => Some(Self::Field),
            "tag" => Some(Self::Tag),
            "note" => Some(Self::Note),
            "note_revision" => Some(Self::NoteRevision),
            "note_tag" => Some(Self::NoteTag),
            "note_link" => Some(Self::NoteLink),
            _ => None,
        }
    }
}

/// Remote synchronization operation used for apply dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RemoteOperation {
    /// Insert operation.
    Insert,
    /// Update operation.
    Update,
    /// Delete operation.
    Delete,
    /// Restore operation.
    Restore,
    /// Attach relation operation.
    Attach,
    /// Detach relation operation.
    Detach,
}

impl RemoteOperation {
    /// Parses a remote operation from a sync change record.
    ///
    /// # Arguments
    ///
    /// * `value` - Raw operation from a sync change.
    ///
    /// # Returns
    ///
    /// Returns a typed operation or `None` for unsupported values.
    pub(super) fn parse(value: &str) -> Option<Self> {
        match value {
            "insert" => Some(Self::Insert),
            "update" => Some(Self::Update),
            "delete" => Some(Self::Delete),
            "restore" => Some(Self::Restore),
            "attach" => Some(Self::Attach),
            "detach" => Some(Self::Detach),
            _ => None,
        }
    }
}

/// Parsed remote change dispatch key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RemoteChangeKind {
    /// Entity kind affected by the remote change.
    pub entity: RemoteEntityKind,
    /// Operation applied by the remote change.
    pub operation: RemoteOperation,
}

impl TryFrom<&SyncChangeRecord> for RemoteChangeKind {
    type Error = String;

    /// Converts a sync change record into a typed dispatch key.
    ///
    /// # Arguments
    ///
    /// * `change` - Remote sync change metadata.
    ///
    /// # Returns
    ///
    /// Returns a typed dispatch key or the existing unsupported-change message.
    fn try_from(change: &SyncChangeRecord) -> Result<Self, Self::Error> {
        let Some(entity) = RemoteEntityKind::parse(&change.entity_type) else {
            return Err(format!(
                "unsupported remote change {} {}",
                change.entity_type, change.operation
            ));
        };
        let Some(operation) = RemoteOperation::parse(&change.operation) else {
            return Err(format!(
                "unsupported remote change {} {}",
                change.entity_type, change.operation
            ));
        };

        Ok(Self { entity, operation })
    }
}
