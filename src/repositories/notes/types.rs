use sqlx::FromRow;

use crate::models::note::NoteRecord;
use crate::models::note_link::NoteLinkRecord;

/// Input data used to create a note.
#[derive(Debug, Clone)]
pub struct CreateNoteInput {
    /// Note body content.
    pub content: String,
    /// Optional normalized field name.
    pub field: Option<String>,
    /// Normalized tag names.
    pub tags: Vec<String>,
    /// Role that created the note.
    pub role: String,
    /// Optional device identifier for the initial revision.
    pub device_id: Option<String>,
    /// Normalized outgoing note links parsed by the client.
    pub links: Vec<NoteLinkInput>,
}

/// Result returned after note creation.
#[derive(Debug, Clone)]
pub struct CreatedNote {
    /// Persisted note record.
    pub note: NoteRecord,
    /// Resolved field name.
    pub field: Option<String>,
    /// Resolved tag names.
    pub tags: Vec<String>,
    /// Persisted outgoing note links.
    pub links: Vec<NoteLinkRecord>,
}

/// Input data used to update a note.
#[derive(Debug, Clone)]
pub struct UpdateNoteInput {
    /// New note body content.
    pub content: String,
    /// Optional device identifier for the update revision.
    pub device_id: Option<String>,
    /// Optional normalized field name; absent keeps the current field.
    pub field: Option<String>,
    /// Optional normalized replacement tags; absent keeps current tags.
    pub tags: Option<Vec<String>>,
    /// Optional normalized replacement links; absent keeps current links.
    pub links: Option<Vec<NoteLinkInput>>,
}

/// Input data used to create or replace a note link.
#[derive(Debug, Clone)]
pub struct NoteLinkInput {
    /// Target note full ID or unique prefix.
    pub target_note_ref: String,
    /// Optional link text parsed by the client.
    pub anchor_text: Option<String>,
    /// Optional zero-based position parsed by the client.
    pub position: Option<i64>,
}

/// Aggregated note count for one local calendar date.
#[derive(Debug, Clone, FromRow)]
pub struct DailyNoteCountRow {
    /// Server-local date in `YYYY-MM-DD` format.
    pub date: String,
    /// Number of visible notes created on the date.
    pub count: i64,
}
