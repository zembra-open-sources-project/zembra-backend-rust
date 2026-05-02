use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

use crate::models::note::NoteRecord;

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
