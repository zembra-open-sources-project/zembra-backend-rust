use serde::Serialize;
use sqlx::FromRow;
use utoipa::ToSchema;

/// Database record for a note.
#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
pub struct NoteRecord {
    /// Stable note identifier.
    pub id: String,
    /// Note body content.
    pub content: String,
    /// Role that created the note.
    pub role: String,
    /// Optional field identifier for this note.
    pub field_id: Option<String>,
    /// Unix timestamp for note creation.
    pub created_at: i64,
    /// Unix timestamp for the last note update.
    pub updated_at: i64,
    /// Optional Unix timestamp marking archival.
    pub archived_at: Option<i64>,
    /// Optional Unix timestamp marking soft deletion.
    pub deleted_at: Option<i64>,
    /// Optional identifier of the current revision.
    pub current_revision_id: Option<String>,
}
