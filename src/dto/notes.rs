use serde::{Deserialize, Deserializer, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::models::field::FieldRecord;
use crate::models::note::NoteRecord;
use crate::models::note_link::NoteLinkRecord;
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
    /// Optional note creator role filter.
    pub role: Option<String>,
}

/// Query parameters used by the random tags endpoint.
#[derive(Debug, Clone, Deserialize, Validate, IntoParams)]
pub struct RandomTagsQuery {
    /// Number of random tags to return.
    #[validate(range(min = 1, max = 20))]
    pub n: Option<i64>,
    /// Maximum cumulative number of notes to return across tags.
    #[validate(range(min = 1, max = 100))]
    pub count: Option<i64>,
}

/// Query parameters used by the random fields endpoint.
#[derive(Debug, Clone, Deserialize, Validate, IntoParams)]
pub struct RandomFieldsQuery {
    /// Number of random fields to return.
    #[validate(range(min = 1, max = 20))]
    pub n: Option<i64>,
    /// Maximum cumulative number of notes to return across fields.
    #[validate(range(min = 1, max = 100))]
    pub count: Option<i64>,
}

/// Query parameters used by the random notes endpoint.
#[derive(Debug, Clone, Deserialize, Validate, IntoParams)]
pub struct RandomNotesQuery {
    /// Number of random notes to return.
    #[validate(range(min = 1, max = 50))]
    pub n: i64,
}

/// Query parameters used by the notes-by-date endpoint.
#[derive(Debug, Clone, Deserialize, Validate, IntoParams)]
pub struct NotesByDateQuery {
    /// Server-local creation date in `YYYY-MM-DD` format.
    #[validate(required, custom(function = "validate_note_date"))]
    pub date: Option<String>,
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
    /// Client-parsed outgoing note links.
    #[serde(default)]
    #[validate(nested)]
    pub links: Vec<NoteLinkRequest>,
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

/// Response body for daily note count statistics.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DailyNoteCountsResponse {
    /// Daily note count buckets ordered by date ascending.
    pub days: Vec<DailyNoteCount>,
}

/// Response body for listing notes created on one date.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct NotesByDateResponse {
    /// Server-local date in `YYYY-MM-DD` format.
    pub date: String,
    /// Visible notes created on the date.
    pub notes: Vec<NoteRecord>,
}

/// Note count for a server-local calendar date.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DailyNoteCount {
    /// Server-local date in `YYYY-MM-DD` format.
    pub date: String,
    /// Number of visible notes created on the date.
    pub count: i64,
}

/// Response body for random tagged notes.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TaggedNotesResponse {
    /// Random tag groups with their visible notes.
    pub tagged_notes: Vec<TaggedNotesGroup>,
}

/// Notes grouped under one randomly selected tag.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TaggedNotesGroup {
    /// Randomly selected tag.
    pub tag: TagRecord,
    /// Visible notes associated with the tag.
    pub notes: Vec<NoteRecord>,
}

/// Response body for random field notes.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FieldNotesResponse {
    /// Random field groups with their visible notes.
    pub field_notes: Vec<FieldNotesGroup>,
}

/// Notes grouped under one randomly selected field.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FieldNotesGroup {
    /// Randomly selected field.
    pub field: FieldRecord,
    /// Visible notes associated with the field.
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
    /// Links from this note to other visible notes.
    pub outgoing_links: Vec<NoteLinkRecord>,
    /// Links from other visible notes to this note.
    pub backlinks: Vec<NoteLinkRecord>,
}

/// Client-parsed outgoing link for note create and update requests.
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct NoteLinkRequest {
    /// Target note full ID or unique hexadecimal prefix.
    #[validate(length(min = 4), custom(function = "validate_hex_note_uuid"))]
    pub target_note_ref: String,
    /// Optional link text parsed by the client.
    pub anchor_text: Option<String>,
    /// Optional zero-based position parsed by the client.
    #[validate(range(min = 0))]
    pub position: Option<i64>,
}

/// Request body for updating a note.
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct UpdateNoteRequest {
    /// New note body content.
    #[validate(length(min = 1))]
    pub content: String,
    /// Optional device identifier for the update revision.
    pub device_id: Option<String>,
    /// Optional field update; absent keeps the current field, null selects inbox.
    #[serde(default, deserialize_with = "deserialize_optional_field_update")]
    pub field: Option<Option<String>>,
    /// Optional replacement tag list; absent keeps current tags.
    pub tags: Option<Vec<String>>,
    /// Optional replacement outgoing links; absent keeps current links.
    #[validate(nested)]
    pub links: Option<Vec<NoteLinkRequest>>,
}

/// Filter for recent notes by creator role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecentNotesRoleFilter {
    /// Return notes created by humans.
    Human,
    /// Return notes created by agents.
    Agent,
    /// Return notes created by humans and agents.
    Both,
}

impl RecentNotesRoleFilter {
    /// Parses a recent-notes role filter from a request value.
    ///
    /// # Arguments
    ///
    /// * `role` - Optional raw role filter from the request body.
    ///
    /// # Returns
    ///
    /// Returns the parsed role filter, defaulting to `Both` when absent.
    pub fn from_request(role: Option<&str>) -> Result<Self, crate::error::ApiError> {
        match role.map(str::to_ascii_lowercase).as_deref() {
            None | Some("both") => Ok(Self::Both),
            Some("human") => Ok(Self::Human),
            Some("agent") => Ok(Self::Agent),
            Some(_) => Err(crate::error::ApiError::Validation),
        }
    }

    /// Returns the stored database role value for a filtering variant.
    ///
    /// # Returns
    ///
    /// Returns `Some` for concrete role filters and `None` for `Both`.
    pub fn stored_role(self) -> Option<&'static str> {
        match self {
            Self::Human => Some("Human"),
            Self::Agent => Some("Agent"),
            Self::Both => None,
        }
    }
}

/// Returns the default note role for create requests.
///
/// # Returns
///
/// Returns `Human`, matching shared schema defaults.
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

/// Validates a server-local note date string.
///
/// # Arguments
///
/// * `date` - Date candidate from query parameters.
///
/// # Returns
///
/// Returns `Ok(())` when the value is a valid `YYYY-MM-DD` calendar date.
fn validate_note_date(date: &str) -> Result<(), validator::ValidationError> {
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map(|_| ())
        .map_err(|_| validator::ValidationError::new("note_date"))
}

/// Deserializes a field update while preserving absent, null, and string states.
///
/// # Arguments
///
/// * `deserializer` - Serde deserializer for the field value.
///
/// # Returns
///
/// Returns `None` when absent, `Some(None)` for JSON null, and `Some(Some(value))` for strings.
fn deserialize_optional_field_update<'de, D>(
    deserializer: D,
) -> Result<Option<Option<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer).map(Some)
}
