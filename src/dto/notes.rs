use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::models::note::NoteRecord;
use crate::models::revision::NoteRevisionRecord;
use crate::models::tag::TagRecord;

/// Query parameters used by the notes list endpoint.
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ListNotesQuery {
    /// Maximum number of notes to return.
    pub limit: Option<i64>,
}

/// Request body used by the recent notes endpoint.
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct RecentNotesRequest {
    /// Maximum number of notes to return.
    #[validate(range(min = 1, max = 100))]
    pub limit: Option<i64>,
    /// Optional full note ID used as a pagination cursor.
    #[validate(length(equal = 32), custom(function = "validate_hex_note_uuid"))]
    pub note_uuid: Option<String>,
}

/// Request body for creating a single note.
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct CreateNoteRequest {
    /// Note body content.
    #[validate(length(min = 1))]
    pub content: String,
    /// Optional field name to associate with the note.
    pub field: Option<String>,
    /// Optional tag names to associate with the note.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Role that created the note.
    #[serde(default = "default_note_role")]
    pub role: String,
    /// Optional device identifier for the initial revision.
    pub device_id: Option<String>,
}

/// Request body for batch note creation.
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct BatchCreateNotesRequest {
    /// Notes to create in one transaction.
    #[validate(nested)]
    pub items: Vec<CreateNoteRequest>,
}

/// Response body for single-note creation and note reads.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct NoteResponse {
    /// Persisted note record.
    pub note: NoteRecord,
    /// User-facing metadata resolved during creation.
    pub metadata: NoteMetadata,
}

/// Response body for batch note creation.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BatchCreateNotesResponse {
    /// Created notes and metadata.
    pub notes: Vec<NoteResponse>,
}

/// Response body for listing notes.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ListNotesResponse {
    /// Note records.
    pub notes: Vec<NoteRecord>,
}

/// Response body for listing note revisions.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ListNoteRevisionsResponse {
    /// Note revision records.
    pub revisions: Vec<NoteRevisionRecord>,
}

/// Response body for listing note tags.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ListNoteTagsResponse {
    /// Tag records associated with the note.
    pub tags: Vec<TagRecord>,
}

/// User-facing metadata associated with a note response.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct NoteMetadata {
    /// Resolved field name.
    pub field: Option<String>,
    /// Resolved tag names.
    pub tags: Vec<String>,
    /// Role that created the note.
    pub role: String,
}

/// Request body for updating a note.
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct UpdateNoteRequest {
    /// New note body content.
    #[validate(length(min = 1))]
    pub content: String,
    /// Optional device identifier for the update revision.
    pub device_id: Option<String>,
}

/// Returns the default note role for create requests.
///
/// # Returns
///
/// Returns `Human`, matching schema v0.2.0 defaults.
pub fn default_note_role() -> String {
    "Human".to_string()
}

/// Validates a note UUID string as lowercase or uppercase hexadecimal.
///
/// # Arguments
///
/// * `note_uuid` - Note ID candidate from a request body.
///
/// # Returns
///
/// Returns `Ok(())` when the value contains only hexadecimal characters.
fn validate_hex_note_uuid(note_uuid: &str) -> Result<(), validator::ValidationError> {
    if note_uuid
        .chars()
        .all(|character| character.is_ascii_hexdigit())
    {
        Ok(())
    } else {
        Err(validator::ValidationError::new("hex_note_uuid"))
    }
}
