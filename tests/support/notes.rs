//! Note test data builders and database helpers.

use zembra_backend_rust::app::AppState;
use zembra_backend_rust::dto::notes::{CreateNoteRequest, NoteLinkRequest};

/// Builder for creating notes in integration tests.
#[derive(Debug, Clone)]
pub struct TestNoteBuilder {
    /// Note body content to create.
    content: String,
    /// Optional field name to associate with the note.
    field: Option<String>,
    /// Tag names to associate with the note.
    tags: Vec<String>,
    /// Client-parsed outgoing links to persist with the note.
    links: Vec<NoteLinkRequest>,
    /// Role to send in the create request.
    role: String,
    /// Optional device identifier for the initial revision.
    device_id: Option<String>,
}

impl TestNoteBuilder {
    /// Creates a note builder with default Human role and no metadata.
    ///
    /// # Arguments
    ///
    /// * `content` - Note body content.
    ///
    /// # Returns
    ///
    /// Returns a builder ready to create the note through `NotesService`.
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            field: None,
            tags: Vec::new(),
            links: Vec::new(),
            role: "Human".to_string(),
            device_id: None,
        }
    }

    /// Adds a field to the note under construction.
    ///
    /// # Arguments
    ///
    /// * `field` - Field name to associate with the note.
    ///
    /// # Returns
    ///
    /// Returns the updated builder.
    pub fn field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }

    /// Adds tags to the note under construction.
    ///
    /// # Arguments
    ///
    /// * `tags` - Tag names to associate with the note.
    ///
    /// # Returns
    ///
    /// Returns the updated builder.
    pub fn tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    /// Adds an outgoing note link to the note under construction.
    ///
    /// # Arguments
    ///
    /// * `target_note_ref` - Target note full ID or unique prefix.
    /// * `anchor_text` - Optional client-parsed anchor text.
    /// * `position` - Optional client-parsed position.
    ///
    /// # Returns
    ///
    /// Returns the updated builder.
    pub fn link(
        mut self,
        target_note_ref: impl Into<String>,
        anchor_text: Option<String>,
        position: Option<i64>,
    ) -> Self {
        self.links.push(NoteLinkRequest {
            target_note_ref: target_note_ref.into(),
            anchor_text,
            position,
        });
        self
    }

    /// Creates the note through the notes service.
    ///
    /// # Arguments
    ///
    /// * `state` - Shared application state.
    ///
    /// # Returns
    ///
    /// Returns the created note ID.
    pub async fn create(self, state: &AppState) -> String {
        let service =
            zembra_backend_rust::services::notes::NotesService::new(state.database.pool.clone());
        service
            .create_note(CreateNoteRequest {
                content: self.content,
                field: self.field,
                tags: self.tags,
                role: self.role,
                device_id: self.device_id,
                links: self.links,
            })
            .await
            .unwrap()
            .note
            .id
    }
}

/// Creates a note with default metadata through the notes service.
///
/// # Arguments
///
/// * `state` - Shared application state.
/// * `content` - Note body content.
///
/// # Returns
///
/// Returns the created note ID.
pub async fn create_note(state: &AppState, content: &str) -> String {
    TestNoteBuilder::new(content).create(state).await
}

/// Creates a note with tags through the notes service.
///
/// # Arguments
///
/// * `state` - Shared application state.
/// * `content` - Note body content.
/// * `tags` - Tag names to associate with the note.
///
/// # Returns
///
/// Returns the created note ID.
pub async fn create_tagged_note(state: &AppState, content: &str, tags: Vec<String>) -> String {
    TestNoteBuilder::new(content).tags(tags).create(state).await
}

/// Creates a note with a field through the notes service.
///
/// # Arguments
///
/// * `state` - Shared application state.
/// * `content` - Note body content.
/// * `field` - Field name to associate with the note.
///
/// # Returns
///
/// Returns the created note ID.
pub async fn create_field_note(state: &AppState, content: &str, field: &str) -> String {
    TestNoteBuilder::new(content)
        .field(field)
        .create(state)
        .await
}

/// Updates a note timestamp directly for deterministic ordering tests.
///
/// # Arguments
///
/// * `state` - Shared application state.
/// * `note_id` - Note ID to update.
/// * `updated_at` - Timestamp value to write.
pub async fn set_updated_at(state: &AppState, note_id: &str, updated_at: i64) {
    sqlx::query("UPDATE notes SET updated_at = ? WHERE id = ?")
        .bind(updated_at)
        .bind(note_id)
        .execute(&state.database.pool)
        .await
        .unwrap();
}

/// Updates a note creation timestamp directly for deterministic statistics tests.
///
/// # Arguments
///
/// * `state` - Shared application state.
/// * `note_id` - Note ID to update.
/// * `created_at` - Timestamp value to write.
pub async fn set_created_at(state: &AppState, note_id: &str, created_at: i64) {
    sqlx::query("UPDATE notes SET created_at = ?, updated_at = ? WHERE id = ?")
        .bind(created_at)
        .bind(created_at)
        .bind(note_id)
        .execute(&state.database.pool)
        .await
        .unwrap();
}
