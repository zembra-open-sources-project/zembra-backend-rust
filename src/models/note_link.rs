use serde::Serialize;
use sqlx::FromRow;
use utoipa::ToSchema;

/// Database record for a note-to-note link.
#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
pub struct NoteLinkRecord {
    /// Stable link identifier.
    pub id: String,
    /// Identifier of the note that contains the link.
    pub source_note_id: String,
    /// Identifier of the note referenced by the source note.
    pub target_note_id: String,
    /// Optional link text parsed by the client.
    pub anchor_text: Option<String>,
    /// Optional zero-based character position parsed by the client.
    pub position: Option<i64>,
    /// Unix timestamp for link creation.
    pub created_at: i64,
}
