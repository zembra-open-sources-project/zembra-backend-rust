use sqlx::SqlitePool;

use crate::dto::notes::{
    BatchCreateNotesResponse, CreateNoteRequest, NoteMetadata, NoteResponse, RecentNotesRequest,
    UpdateNoteRequest,
};
use crate::error::ApiError;
use crate::models::note::NoteRecord;
use crate::models::revision::NoteRevisionRecord;
use crate::models::tag::TagRecord;
use crate::repositories::notes::{CreateNoteInput, CreatedNote, NotesRepository};

/// Service for note business workflows.
#[derive(Debug, Clone)]
pub struct NotesService {
    /// Repository used by note workflows.
    repository: NotesRepository,
}

impl NotesService {
    /// Creates a notes service backed by a SQLite pool.
    ///
    /// # Arguments
    ///
    /// * `pool` - Shared SQLite pool.
    ///
    /// # Returns
    ///
    /// Returns a notes service.
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            repository: NotesRepository::new(pool),
        }
    }

    /// Creates a single note.
    ///
    /// # Arguments
    ///
    /// * `request` - Validated create-note request.
    ///
    /// # Returns
    ///
    /// Returns the created note response.
    pub async fn create_note(&self, request: CreateNoteRequest) -> Result<NoteResponse, ApiError> {
        let input = normalize_create_request(request)?;
        let created = self.repository.create_note(input).await?;

        Ok(created_note_to_response(created))
    }

    /// Creates notes in a single transaction.
    ///
    /// # Arguments
    ///
    /// * `items` - Validated create-note requests.
    ///
    /// # Returns
    ///
    /// Returns all created notes.
    pub async fn create_notes_batch(
        &self,
        items: Vec<CreateNoteRequest>,
    ) -> Result<BatchCreateNotesResponse, ApiError> {
        let inputs = items
            .into_iter()
            .map(normalize_create_request)
            .collect::<Result<Vec<_>, _>>()?;
        let notes = self
            .repository
            .create_notes_batch(inputs)
            .await?
            .into_iter()
            .map(created_note_to_response)
            .collect();

        Ok(BatchCreateNotesResponse { notes })
    }

    /// Lists active notes.
    ///
    /// # Arguments
    ///
    /// * `limit` - Optional maximum record count.
    ///
    /// # Returns
    ///
    /// Returns active note records.
    pub async fn list_notes(&self, limit: Option<i64>) -> Result<Vec<NoteRecord>, ApiError> {
        self.repository.list_notes(limit).await
    }

    /// Lists recent notes for Web presentation.
    ///
    /// # Arguments
    ///
    /// * `request` - Validated recent-notes request.
    ///
    /// # Returns
    ///
    /// Returns non-deleted and non-archived note records ordered by update time.
    pub async fn recent_notes(
        &self,
        request: RecentNotesRequest,
    ) -> Result<Vec<NoteRecord>, ApiError> {
        let limit = request.limit.unwrap_or(50);
        self.repository
            .list_recent_notes(limit, request.note_uuid.as_deref())
            .await
    }

    /// Reads a note by reference.
    ///
    /// # Arguments
    ///
    /// * `note_ref` - Full note ID or unique prefix.
    ///
    /// # Returns
    ///
    /// Returns the matching note.
    pub async fn get_note(&self, note_ref: &str) -> Result<NoteRecord, ApiError> {
        self.repository.get_note_by_ref(note_ref).await
    }

    /// Updates note content.
    ///
    /// # Arguments
    ///
    /// * `note_ref` - Full note ID or unique prefix.
    /// * `request` - Update-note request.
    ///
    /// # Returns
    ///
    /// Returns the updated note.
    pub async fn update_note(
        &self,
        note_ref: &str,
        request: UpdateNoteRequest,
    ) -> Result<NoteRecord, ApiError> {
        let content = normalize_required_text(&request.content)?;
        self.repository
            .update_note_content(note_ref, &content, request.device_id.as_deref())
            .await
    }

    /// Archives a note.
    ///
    /// # Arguments
    ///
    /// * `note_ref` - Full note ID or unique prefix.
    ///
    /// # Returns
    ///
    /// Returns the archived note.
    pub async fn archive_note(&self, note_ref: &str) -> Result<NoteRecord, ApiError> {
        self.repository.archive_note(note_ref).await
    }

    /// Soft deletes a note.
    ///
    /// # Arguments
    ///
    /// * `note_ref` - Full note ID or unique prefix.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after deletion.
    pub async fn delete_note(&self, note_ref: &str) -> Result<(), ApiError> {
        self.repository.delete_note(note_ref).await
    }

    /// Lists note revisions.
    ///
    /// # Arguments
    ///
    /// * `note_ref` - Full note ID or unique prefix.
    ///
    /// # Returns
    ///
    /// Returns note revisions.
    pub async fn list_note_revisions(
        &self,
        note_ref: &str,
    ) -> Result<Vec<NoteRevisionRecord>, ApiError> {
        self.repository.list_note_revisions(note_ref).await
    }

    /// Lists note tags.
    ///
    /// # Arguments
    ///
    /// * `note_ref` - Full note ID or unique prefix.
    ///
    /// # Returns
    ///
    /// Returns note tags.
    pub async fn list_note_tags(&self, note_ref: &str) -> Result<Vec<TagRecord>, ApiError> {
        self.repository.list_note_tags(note_ref).await
    }

    /// Adds a tag to a note.
    ///
    /// # Arguments
    ///
    /// * `note_ref` - Full note ID or unique prefix.
    /// * `tag_name` - Tag name to add.
    ///
    /// # Returns
    ///
    /// Returns the associated tag.
    pub async fn add_tag_to_note(
        &self,
        note_ref: &str,
        tag_name: &str,
    ) -> Result<TagRecord, ApiError> {
        let tag_name = normalize_required_text(tag_name)?;
        self.repository.add_tag_to_note(note_ref, &tag_name).await
    }

    /// Removes a tag from a note.
    ///
    /// # Arguments
    ///
    /// * `note_ref` - Full note ID or unique prefix.
    /// * `tag_name` - Tag name to remove.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after the association is removed.
    pub async fn remove_tag_from_note(
        &self,
        note_ref: &str,
        tag_name: &str,
    ) -> Result<(), ApiError> {
        let tag_name = normalize_required_text(tag_name)?;
        self.repository
            .remove_tag_from_note(note_ref, &tag_name)
            .await
    }
}

/// Normalizes a create-note request into repository input.
///
/// # Arguments
///
/// * `request` - Create-note request.
///
/// # Returns
///
/// Returns normalized repository input.
fn normalize_create_request(request: CreateNoteRequest) -> Result<CreateNoteInput, ApiError> {
    let content = normalize_required_text(&request.content)?;
    let role = normalize_role(&request.role)?;
    let field = request
        .field
        .as_deref()
        .map(normalize_required_text)
        .transpose()?;
    let tags = normalize_tags(request.tags);

    Ok(CreateNoteInput {
        content,
        field,
        tags,
        role,
        device_id: request.device_id,
    })
}

/// Converts a repository creation result into an API response.
///
/// # Arguments
///
/// * `created` - Repository creation result.
///
/// # Returns
///
/// Returns API note response.
fn created_note_to_response(created: CreatedNote) -> NoteResponse {
    NoteResponse {
        metadata: NoteMetadata {
            field: created.field,
            tags: created.tags,
            role: created.note.role.clone(),
        },
        note: created.note,
    }
}

/// Normalizes a required text field.
///
/// # Arguments
///
/// * `value` - Raw text value.
///
/// # Returns
///
/// Returns trimmed text or a validation error.
fn normalize_required_text(value: &str) -> Result<String, ApiError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ApiError::Validation);
    }

    Ok(trimmed.to_string())
}

/// Normalizes a note role.
///
/// # Arguments
///
/// * `role` - Raw role value.
///
/// # Returns
///
/// Returns a valid role string.
fn normalize_role(role: &str) -> Result<String, ApiError> {
    match role {
        "Human" | "Agent" => Ok(role.to_string()),
        _ => Err(ApiError::Validation),
    }
}

/// Normalizes tag names by trimming, removing blanks, and deduplicating.
///
/// # Arguments
///
/// * `tags` - Raw tag names.
///
/// # Returns
///
/// Returns normalized tag names preserving first-seen order.
fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();
    for tag in tags {
        let trimmed = tag.trim();
        if !trimmed.is_empty() && !normalized.iter().any(|item: &String| item == trimmed) {
            normalized.push(trimmed.to_string());
        }
    }

    normalized
}
