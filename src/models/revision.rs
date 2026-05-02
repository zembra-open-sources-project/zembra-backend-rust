use serde::Serialize;
use sqlx::FromRow;
use utoipa::ToSchema;

/// Database record for a note content revision.
#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
pub struct NoteRevisionRecord {
    /// Stable revision identifier.
    pub id: String,
    /// Identifier of the note this revision belongs to.
    pub note_id: String,
    /// Full content snapshot stored for this revision.
    pub content: String,
    /// Optional title snapshot stored for this revision.
    pub title: Option<String>,
    /// Optional device identifier that produced this revision.
    pub device_id: Option<String>,
    /// Unix timestamp for revision creation.
    pub created_at: i64,
}
