use serde::{Deserialize, Serialize};

/// Complete local or remote data snapshot for the synchronized tables.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncTableSnapshot {
    /// Workspace rows keyed by `id`.
    pub workspaces: Vec<WorkspaceSnapshotRow>,
    /// Device rows keyed by `id`.
    pub devices: Vec<DeviceSnapshotRow>,
    /// Field rows keyed by `id`.
    pub fields: Vec<FieldSnapshotRow>,
    /// Tag rows keyed by `id`.
    pub tags: Vec<TagSnapshotRow>,
    /// Note rows keyed by `id`.
    pub notes: Vec<NoteSnapshotRow>,
    /// Note revision rows keyed by `id`.
    pub note_revisions: Vec<NoteRevisionSnapshotRow>,
    /// Note tag relation rows keyed by `workspace_id`, `note_id`, and `tag_id`.
    pub note_tags: Vec<NoteTagSnapshotRow>,
    /// Note link rows keyed by `id`.
    pub note_links: Vec<NoteLinkSnapshotRow>,
    /// Synchronization change rows keyed by `id`.
    pub sync_changes: Vec<SyncChangeSnapshotRow>,
}

/// Workspace snapshot row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct WorkspaceSnapshotRow {
    /// Workspace identifier.
    pub id: String,
    /// Optional workspace display name.
    pub workspace_name: Option<String>,
    /// Unix timestamp when the workspace was created.
    pub created_at: i64,
    /// Unix timestamp when the workspace was last updated.
    pub updated_at: i64,
    /// Optional archive timestamp.
    pub archived_at: Option<i64>,
    /// Optional deletion timestamp.
    pub deleted_at: Option<i64>,
}

/// Device snapshot row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct DeviceSnapshotRow {
    /// Device identifier.
    pub id: String,
    /// Workspace that owns the device.
    pub workspace_id: String,
    /// Device display name.
    pub name: String,
    /// Device platform string.
    pub platform: String,
    /// Unix timestamp when the device was created.
    pub created_at: i64,
    /// Optional last-seen timestamp.
    pub last_seen_at: Option<i64>,
    /// Whether synchronization is enabled for this device.
    pub sync_enabled: bool,
    /// Optional last-synced timestamp.
    pub last_synced_at: Option<i64>,
}

/// Field snapshot row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct FieldSnapshotRow {
    /// Field identifier.
    pub id: String,
    /// Workspace that owns the field.
    pub workspace_id: String,
    /// Field display name.
    pub name: String,
    /// Unix timestamp when the field was created.
    pub created_at: i64,
}

/// Tag snapshot row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct TagSnapshotRow {
    /// Tag identifier.
    pub id: String,
    /// Workspace that owns the tag.
    pub workspace_id: String,
    /// Tag display name.
    pub name: String,
    /// Optional parent tag identifier.
    pub parent_tag_id: Option<String>,
    /// Materialized tag path.
    pub path: String,
    /// Zero-based depth inside the tag tree.
    pub depth: i64,
    /// Unix timestamp when the tag was created.
    pub created_at: i64,
}

/// Note snapshot row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct NoteSnapshotRow {
    /// Note identifier.
    pub id: String,
    /// Workspace that owns the note.
    pub workspace_id: String,
    /// Note body content.
    pub content: String,
    /// Note role.
    pub role: String,
    /// Optional field identifier.
    pub field_id: Option<String>,
    /// Unix timestamp when the note was created.
    pub created_at: i64,
    /// Unix timestamp when the note was last updated.
    pub updated_at: i64,
    /// Optional archive timestamp.
    pub archived_at: Option<i64>,
    /// Optional deletion timestamp.
    pub deleted_at: Option<i64>,
    /// Optional current revision identifier.
    pub current_revision_id: Option<String>,
    /// Optional last synchronization change identifier.
    pub last_change_id: Option<String>,
    /// Conflict status for the note.
    pub conflict_status: String,
}

/// Note revision snapshot row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct NoteRevisionSnapshotRow {
    /// Revision identifier.
    pub id: String,
    /// Workspace that owns the revision.
    pub workspace_id: String,
    /// Note identifier for this revision.
    pub note_id: String,
    /// Revision content.
    pub content: String,
    /// Optional revision title.
    pub title: Option<String>,
    /// Optional device that produced the revision.
    pub device_id: Option<String>,
    /// Unix timestamp when the revision was created.
    pub created_at: i64,
    /// Optional base revision identifier.
    pub base_revision_id: Option<String>,
    /// Optional sync change identifier.
    pub change_id: Option<String>,
}

/// Note tag relation snapshot row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct NoteTagSnapshotRow {
    /// Workspace that owns the relation.
    pub workspace_id: String,
    /// Note identifier in the relation.
    pub note_id: String,
    /// Tag identifier in the relation.
    pub tag_id: String,
    /// Unix timestamp when the relation was created.
    pub created_at: i64,
}

/// Note link snapshot row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct NoteLinkSnapshotRow {
    /// Link identifier.
    pub id: String,
    /// Workspace that owns the link.
    pub workspace_id: String,
    /// Source note identifier.
    pub source_note_id: String,
    /// Target note identifier.
    pub target_note_id: String,
    /// Optional anchor text for the link.
    pub anchor_text: Option<String>,
    /// Optional position of the anchor in source content.
    pub position: Option<i64>,
    /// Unix timestamp when the link was created.
    pub created_at: i64,
}

/// Sync change snapshot row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct SyncChangeSnapshotRow {
    /// Change identifier.
    pub id: String,
    /// Workspace that owns the change.
    pub workspace_id: String,
    /// Device that produced the change.
    pub device_id: String,
    /// Entity type affected by the change.
    pub entity_type: String,
    /// Entity identifier affected by the change.
    pub entity_id: String,
    /// Operation applied by the change.
    pub operation: String,
    /// Optional base revision identifier.
    pub base_revision_id: Option<String>,
    /// Optional new revision identifier.
    pub new_revision_id: Option<String>,
    /// JSON payload text.
    pub payload: String,
    /// Unix timestamp when the change was created.
    pub created_at: i64,
    /// Optional timestamp when the change was applied locally.
    pub applied_at: Option<i64>,
    /// Optional timestamp when the change was committed to Supabase.
    pub supabase_committed_at: Option<i64>,
}
