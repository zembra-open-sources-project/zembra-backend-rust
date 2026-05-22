use std::collections::HashSet;

use sqlx::{FromRow, Sqlite, SqlitePool, Transaction};

use crate::error::ApiError;
use crate::models::field::FieldRecord;
use crate::models::note::NoteRecord;
use crate::models::note_link::NoteLinkRecord;
use crate::models::revision::NoteRevisionRecord;
use crate::models::tag::TagRecord;
use crate::repositories::sync::{SyncChangeInput, record_sync_change_in_transaction};
use crate::repositories::taxonomy::{
    DEFAULT_WORKSPACE_ID, get_or_create_field_in_transaction, get_or_create_tag_in_transaction,
    new_id,
};

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

/// Repository for note data access.
#[derive(Debug, Clone)]
pub struct NotesRepository {
    /// SQLx pool used by repository queries.
    pool: SqlitePool,
}

impl NotesRepository {
    /// Creates a note repository backed by a SQLite pool.
    ///
    /// # Arguments
    ///
    /// * `pool` - Shared SQLite connection pool.
    ///
    /// # Returns
    ///
    /// Returns a repository value.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Creates a single note with initial revision, field, and tags.
    ///
    /// # Arguments
    ///
    /// * `input` - Note creation input.
    ///
    /// # Returns
    ///
    /// Returns the created note and resolved metadata.
    pub async fn create_note(&self, input: CreateNoteInput) -> Result<CreatedNote, ApiError> {
        let mut transaction = self.pool.begin().await?;
        let created = create_note_in_transaction(&mut transaction, input).await?;
        transaction.commit().await?;

        Ok(created)
    }

    /// Creates several notes in one transaction.
    ///
    /// # Arguments
    ///
    /// * `items` - Note creation inputs.
    ///
    /// # Returns
    ///
    /// Returns created notes when all inputs succeed.
    pub async fn create_notes_batch(
        &self,
        items: Vec<CreateNoteInput>,
    ) -> Result<Vec<CreatedNote>, ApiError> {
        let mut transaction = self.pool.begin().await?;
        let mut created_notes = Vec::with_capacity(items.len());

        for item in items {
            created_notes.push(create_note_in_transaction(&mut transaction, item).await?);
        }

        transaction.commit().await?;
        Ok(created_notes)
    }

    /// Lists active notes ordered by update time descending.
    ///
    /// # Arguments
    ///
    /// * `limit` - Optional maximum record count.
    ///
    /// # Returns
    ///
    /// Returns active note records.
    pub async fn list_notes(&self, limit: Option<i64>) -> Result<Vec<NoteRecord>, ApiError> {
        let limit = limit.unwrap_or(50);

        sqlx::query_as::<_, NoteRecord>(
            "SELECT id, content, role, field_id, created_at, updated_at, archived_at, deleted_at, current_revision_id \
             FROM notes WHERE workspace_id = ? AND deleted_at IS NULL AND archived_at IS NULL ORDER BY updated_at DESC LIMIT ?",
        )
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(ApiError::from)
    }

    /// Lists recent active notes ordered by update time descending.
    ///
    /// # Arguments
    ///
    /// * `limit` - Maximum record count.
    /// * `note_uuid` - Optional full note ID used as a cursor.
    ///
    /// # Returns
    ///
    /// Returns non-deleted and non-archived note records.
    pub async fn list_recent_notes(
        &self,
        limit: i64,
        note_uuid: Option<&str>,
    ) -> Result<Vec<NoteRecord>, ApiError> {
        match note_uuid {
            Some(note_uuid) => {
                validate_full_note_uuid(note_uuid)?;
                let cursor = self.get_visible_note_by_id(note_uuid).await?;

                sqlx::query_as::<_, NoteRecord>(
                    "SELECT id, content, role, field_id, created_at, updated_at, archived_at, deleted_at, current_revision_id \
                     FROM notes \
                     WHERE workspace_id = ? \
                     AND deleted_at IS NULL \
                     AND archived_at IS NULL \
                     AND (updated_at < ? OR (updated_at = ? AND id < ?)) \
                     ORDER BY updated_at DESC, id DESC LIMIT ?",
                )
                .bind(DEFAULT_WORKSPACE_ID)
                .bind(cursor.updated_at)
                .bind(cursor.updated_at)
                .bind(&cursor.id)
                .bind(limit)
                .fetch_all(&self.pool)
                .await
                .map_err(ApiError::from)
            }
            None => sqlx::query_as::<_, NoteRecord>(
                "SELECT id, content, role, field_id, created_at, updated_at, archived_at, deleted_at, current_revision_id \
                 FROM notes WHERE workspace_id = ? AND deleted_at IS NULL AND archived_at IS NULL ORDER BY updated_at DESC, id DESC LIMIT ?",
            )
            .bind(DEFAULT_WORKSPACE_ID)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(ApiError::from),
        }
    }

    /// Lists random visible notes.
    ///
    /// # Arguments
    ///
    /// * `limit` - Maximum number of notes to return.
    ///
    /// # Returns
    ///
    /// Returns random non-deleted and non-archived note records.
    pub async fn list_random_notes(&self, limit: i64) -> Result<Vec<NoteRecord>, ApiError> {
        sqlx::query_as::<_, NoteRecord>(
            "SELECT id, content, role, field_id, created_at, updated_at, archived_at, deleted_at, current_revision_id \
             FROM notes \
             WHERE workspace_id = ? \
             AND deleted_at IS NULL \
             AND archived_at IS NULL \
             ORDER BY RANDOM() LIMIT ?",
        )
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(ApiError::from)
    }

    /// Lists visible note counts grouped by server-local creation date.
    ///
    /// # Arguments
    ///
    /// * `start_timestamp` - Inclusive Unix timestamp for the first local day.
    ///
    /// # Returns
    ///
    /// Returns note counts grouped by `YYYY-MM-DD` local date.
    pub async fn daily_note_counts_since(
        &self,
        start_timestamp: i64,
    ) -> Result<Vec<DailyNoteCountRow>, ApiError> {
        sqlx::query_as::<_, DailyNoteCountRow>(
            "SELECT date(created_at, 'unixepoch', 'localtime') AS date, COUNT(*) AS count \
             FROM notes \
             WHERE workspace_id = ? \
             AND deleted_at IS NULL \
             AND archived_at IS NULL \
             AND created_at >= ? \
             GROUP BY date \
             ORDER BY date ASC",
        )
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(start_timestamp)
        .fetch_all(&self.pool)
        .await
        .map_err(ApiError::from)
    }

    /// Lists visible notes created within a timestamp range.
    ///
    /// # Arguments
    ///
    /// * `start_timestamp` - Inclusive Unix timestamp for the local day start.
    /// * `end_timestamp` - Exclusive Unix timestamp for the next local day start.
    ///
    /// # Returns
    ///
    /// Returns non-deleted and non-archived note records created within the range.
    pub async fn list_visible_notes_created_between(
        &self,
        start_timestamp: i64,
        end_timestamp: i64,
    ) -> Result<Vec<NoteRecord>, ApiError> {
        sqlx::query_as::<_, NoteRecord>(
            "SELECT id, content, role, field_id, created_at, updated_at, archived_at, deleted_at, current_revision_id \
             FROM notes \
             WHERE workspace_id = ? \
             AND deleted_at IS NULL \
             AND archived_at IS NULL \
             AND created_at >= ? \
             AND created_at < ? \
             ORDER BY created_at DESC, id DESC",
        )
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(start_timestamp)
        .bind(end_timestamp)
        .fetch_all(&self.pool)
        .await
        .map_err(ApiError::from)
    }

    /// Lists random tags from the default workspace.
    ///
    /// # Arguments
    ///
    /// * `limit` - Maximum number of tags to return.
    ///
    /// # Returns
    ///
    /// Returns randomly ordered tag records.
    pub async fn random_tags(&self, limit: i64) -> Result<Vec<TagRecord>, ApiError> {
        sqlx::query_as::<_, TagRecord>(
            "SELECT id, name, created_at FROM tags WHERE workspace_id = ? ORDER BY RANDOM() LIMIT ?",
        )
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(ApiError::from)
    }

    /// Lists visible notes associated with a tag.
    ///
    /// # Arguments
    ///
    /// * `tag_id` - Exact tag identifier.
    /// * `limit` - Maximum number of notes to return.
    ///
    /// # Returns
    ///
    /// Returns non-deleted and non-archived note records for the tag.
    pub async fn list_visible_notes_by_tag(
        &self,
        tag_id: &str,
        limit: i64,
    ) -> Result<Vec<NoteRecord>, ApiError> {
        sqlx::query_as::<_, NoteRecord>(
            "SELECT notes.id, notes.content, notes.role, notes.field_id, notes.created_at, notes.updated_at, notes.archived_at, notes.deleted_at, notes.current_revision_id \
             FROM notes \
             INNER JOIN note_tags ON notes.workspace_id = note_tags.workspace_id AND notes.id = note_tags.note_id \
             WHERE note_tags.workspace_id = ? \
             AND note_tags.tag_id = ? \
             AND notes.deleted_at IS NULL \
             AND notes.archived_at IS NULL \
             ORDER BY RANDOM() LIMIT ?",
        )
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(tag_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(ApiError::from)
    }

    /// Lists random fields from the default workspace.
    ///
    /// # Arguments
    ///
    /// * `limit` - Maximum number of fields to return.
    ///
    /// # Returns
    ///
    /// Returns randomly ordered field records.
    pub async fn random_fields(&self, limit: i64) -> Result<Vec<FieldRecord>, ApiError> {
        sqlx::query_as::<_, FieldRecord>(
            "SELECT id, name, created_at FROM fields WHERE workspace_id = ? ORDER BY RANDOM() LIMIT ?",
        )
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(ApiError::from)
    }

    /// Lists random visible notes associated with a field.
    ///
    /// # Arguments
    ///
    /// * `field_id` - Exact field identifier.
    /// * `limit` - Maximum number of notes to return.
    ///
    /// # Returns
    ///
    /// Returns non-deleted and non-archived note records for the field.
    pub async fn list_visible_notes_by_field(
        &self,
        field_id: &str,
        limit: i64,
    ) -> Result<Vec<NoteRecord>, ApiError> {
        sqlx::query_as::<_, NoteRecord>(
            "SELECT id, content, role, field_id, created_at, updated_at, archived_at, deleted_at, current_revision_id \
             FROM notes \
             WHERE workspace_id = ? \
             AND field_id = ? \
             AND deleted_at IS NULL \
             AND archived_at IS NULL \
             ORDER BY RANDOM() LIMIT ?",
        )
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(field_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(ApiError::from)
    }

    /// Reads a visible note by exact ID.
    ///
    /// # Arguments
    ///
    /// * `note_id` - Full note ID.
    ///
    /// # Returns
    ///
    /// Returns the matching non-deleted and non-archived note.
    async fn get_visible_note_by_id(&self, note_id: &str) -> Result<NoteRecord, ApiError> {
        sqlx::query_as::<_, NoteRecord>(
            "SELECT id, content, role, field_id, created_at, updated_at, archived_at, deleted_at, current_revision_id \
             FROM notes WHERE workspace_id = ? AND id = ? AND deleted_at IS NULL AND archived_at IS NULL",
        )
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(note_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| {
            ApiError::RecordNotFound(format!(
                "Note cursor \"{note_id}\" did not match any visible note."
            ))
        })
    }

    /// Resolves a visible note by full ID or unique hexadecimal prefix.
    ///
    /// # Arguments
    ///
    /// * `note_ref` - Full 32-character ID or at least 4-character prefix.
    ///
    /// # Returns
    ///
    /// Returns the matching non-deleted and non-archived note or a note reference error.
    pub async fn get_note_by_ref(&self, note_ref: &str) -> Result<NoteRecord, ApiError> {
        validate_note_ref(note_ref)?;

        let pattern = format!("{note_ref}%");
        let notes = sqlx::query_as::<_, NoteRecord>(
            "SELECT id, content, role, field_id, created_at, updated_at, archived_at, deleted_at, current_revision_id \
             FROM notes WHERE workspace_id = ? AND id LIKE ? AND deleted_at IS NULL AND archived_at IS NULL ORDER BY id ASC LIMIT 2",
        )
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(pattern)
        .fetch_all(&self.pool)
        .await?;

        match notes.as_slice() {
            [note] => Ok(note.clone()),
            [] => Err(ApiError::RecordNotFound(format!(
                "Note reference \"{note_ref}\" did not match any note."
            ))),
            _ => Err(ApiError::AmbiguousNoteReference(format!(
                "Note reference \"{note_ref}\" matched multiple notes."
            ))),
        }
    }

    /// Updates note content, optional field, and optional tag associations.
    ///
    /// # Arguments
    ///
    /// * `note_ref` - Full note ID or unique prefix.
    /// * `input` - Normalized update input.
    ///
    /// # Returns
    ///
    /// Returns the updated note record.
    pub async fn update_note(
        &self,
        note_ref: &str,
        input: UpdateNoteInput,
    ) -> Result<NoteRecord, ApiError> {
        let note = self.get_note_by_ref(note_ref).await?;
        let mut transaction = self.pool.begin().await?;
        let revision_id = new_id();
        let field = match input.field.as_deref() {
            Some(field) => Some(get_or_create_field_in_transaction(&mut transaction, field).await?),
            None => None,
        };

        sqlx::query(
            "INSERT INTO note_revisions (id, workspace_id, note_id, content, title, device_id, created_at) \
             VALUES (?, ?, ?, ?, NULL, ?, unixepoch())",
        )
        .bind(&revision_id)
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(&note.id)
        .bind(&input.content)
        .bind(input.device_id.as_deref())
        .execute(&mut *transaction)
        .await?;

        match &field {
            Some(field) => {
                sqlx::query(
                    "UPDATE notes SET content = ?, field_id = ?, updated_at = unixepoch(), current_revision_id = ? WHERE workspace_id = ? AND id = ?",
                )
                .bind(&input.content)
                .bind(&field.id)
                .bind(&revision_id)
                .bind(DEFAULT_WORKSPACE_ID)
                .bind(&note.id)
                .execute(&mut *transaction)
                .await?;
            }
            None => {
                sqlx::query(
                    "UPDATE notes SET content = ?, updated_at = unixepoch(), current_revision_id = ? WHERE workspace_id = ? AND id = ?",
                )
                .bind(&input.content)
                .bind(&revision_id)
                .bind(DEFAULT_WORKSPACE_ID)
                .bind(&note.id)
                .execute(&mut *transaction)
                .await?;
            }
        }

        if let Some(tags) = input.tags.as_deref() {
            replace_note_tags_in_transaction(&mut transaction, &note.id, tags).await?;
        }
        if let Some(links) = input.links.as_deref() {
            replace_note_links_in_transaction(&mut transaction, &note.id, links).await?;
        }

        let updated = select_note_by_id(&mut transaction, &note.id).await?;
        record_sync_change_in_transaction(
            &mut transaction,
            SyncChangeInput {
                entity_type: "note_revision",
                entity_id: revision_id.clone(),
                operation: "insert",
                base_revision_id: note.current_revision_id.clone(),
                new_revision_id: Some(revision_id.clone()),
                payload: serde_json::json!({
                    "id": revision_id,
                    "workspace_id": DEFAULT_WORKSPACE_ID,
                    "note_id": note.id,
                    "content": input.content,
                    "title": null,
                    "device_id": input.device_id,
                    "created_at": updated.updated_at,
                    "base_revision_id": note.current_revision_id
                }),
            },
        )
        .await?;
        record_sync_change_in_transaction(
            &mut transaction,
            SyncChangeInput {
                entity_type: "note",
                entity_id: updated.id.clone(),
                operation: "update",
                base_revision_id: None,
                new_revision_id: updated.current_revision_id.clone(),
                payload: note_payload(&updated),
            },
        )
        .await?;
        transaction.commit().await?;

        Ok(updated)
    }

    /// Archives a note by setting `archived_at`.
    ///
    /// # Arguments
    ///
    /// * `note_ref` - Full note ID or unique prefix.
    ///
    /// # Returns
    ///
    /// Returns the archived note record.
    pub async fn archive_note(&self, note_ref: &str) -> Result<NoteRecord, ApiError> {
        let note = self.get_note_by_ref(note_ref).await?;
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "UPDATE notes SET archived_at = unixepoch(), updated_at = unixepoch() WHERE workspace_id = ? AND id = ?",
        )
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(&note.id)
        .execute(&mut *transaction)
        .await?;

        let archived = select_note_by_id(&mut transaction, &note.id).await?;
        record_sync_change_in_transaction(
            &mut transaction,
            SyncChangeInput {
                entity_type: "note",
                entity_id: archived.id.clone(),
                operation: "update",
                base_revision_id: None,
                new_revision_id: archived.current_revision_id.clone(),
                payload: note_payload(&archived),
            },
        )
        .await?;
        transaction.commit().await?;

        Ok(archived)
    }

    /// Soft deletes a note by setting `deleted_at`.
    ///
    /// # Arguments
    ///
    /// * `note_ref` - Full note ID or unique prefix.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after the note is soft deleted.
    pub async fn delete_note(&self, note_ref: &str) -> Result<(), ApiError> {
        let note = self.get_note_by_ref(note_ref).await?;
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "UPDATE notes SET deleted_at = unixepoch(), updated_at = unixepoch() WHERE workspace_id = ? AND id = ?",
        )
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(&note.id)
        .execute(&mut *transaction)
        .await?;

        let deleted = select_note_by_id(&mut transaction, &note.id).await?;
        record_sync_change_in_transaction(
            &mut transaction,
            SyncChangeInput {
                entity_type: "note",
                entity_id: deleted.id.clone(),
                operation: "delete",
                base_revision_id: None,
                new_revision_id: deleted.current_revision_id.clone(),
                payload: note_payload(&deleted),
            },
        )
        .await?;
        transaction.commit().await?;

        Ok(())
    }

    /// Lists revisions for a note ordered by creation time.
    ///
    /// # Arguments
    ///
    /// * `note_ref` - Full note ID or unique prefix.
    ///
    /// # Returns
    ///
    /// Returns revision records for the note.
    pub async fn list_note_revisions(
        &self,
        note_ref: &str,
    ) -> Result<Vec<NoteRevisionRecord>, ApiError> {
        let note = self.get_note_by_ref(note_ref).await?;

        sqlx::query_as::<_, NoteRevisionRecord>(
            "SELECT id, note_id, content, title, device_id, created_at \
             FROM note_revisions WHERE workspace_id = ? AND note_id = ? ORDER BY created_at ASC, rowid ASC",
        )
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(note.id)
        .fetch_all(&self.pool)
        .await
        .map_err(ApiError::from)
    }

    /// Lists tags associated with a note.
    ///
    /// # Arguments
    ///
    /// * `note_ref` - Full note ID or unique prefix.
    ///
    /// # Returns
    ///
    /// Returns tag records ordered by name.
    pub async fn list_note_tags(&self, note_ref: &str) -> Result<Vec<TagRecord>, ApiError> {
        let note = self.get_note_by_ref(note_ref).await?;

        self.list_note_tags_by_id(&note.id).await
    }

    /// Lists tags associated with a note ID.
    ///
    /// # Arguments
    ///
    /// * `note_id` - Exact note identifier.
    ///
    /// # Returns
    ///
    /// Returns tag records ordered by name.
    pub async fn list_note_tags_by_id(&self, note_id: &str) -> Result<Vec<TagRecord>, ApiError> {
        sqlx::query_as::<_, TagRecord>(
            "SELECT tags.id, tags.name, tags.created_at \
             FROM tags INNER JOIN note_tags ON tags.workspace_id = note_tags.workspace_id AND tags.id = note_tags.tag_id \
             WHERE note_tags.workspace_id = ? AND note_tags.note_id = ? ORDER BY tags.name ASC",
        )
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(note_id)
        .fetch_all(&self.pool)
        .await
        .map_err(ApiError::from)
    }

    /// Reads a field name by exact field ID.
    ///
    /// # Arguments
    ///
    /// * `field_id` - Optional exact field identifier.
    ///
    /// # Returns
    ///
    /// Returns the field name when the note has a field.
    pub async fn field_name_by_id(
        &self,
        field_id: Option<&str>,
    ) -> Result<Option<String>, ApiError> {
        match field_id {
            Some(field_id) => sqlx::query_scalar::<_, String>(
                "SELECT name FROM fields WHERE workspace_id = ? AND id = ?",
            )
            .bind(DEFAULT_WORKSPACE_ID)
            .bind(field_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(ApiError::from),
            None => Ok(None),
        }
    }

    /// Lists visible outgoing links for a note.
    ///
    /// # Arguments
    ///
    /// * `note_id` - Exact note identifier.
    ///
    /// # Returns
    ///
    /// Returns links whose target notes are non-deleted and non-archived.
    pub async fn list_visible_outgoing_links(
        &self,
        note_id: &str,
    ) -> Result<Vec<NoteLinkRecord>, ApiError> {
        sqlx::query_as::<_, NoteLinkRecord>(
            "SELECT note_links.id, note_links.source_note_id, note_links.target_note_id, note_links.anchor_text, note_links.position, note_links.created_at \
             FROM note_links \
             INNER JOIN notes target ON note_links.workspace_id = target.workspace_id AND note_links.target_note_id = target.id \
             WHERE note_links.workspace_id = ? \
             AND note_links.source_note_id = ? \
             AND target.deleted_at IS NULL \
             AND target.archived_at IS NULL \
             ORDER BY note_links.position ASC, note_links.created_at ASC, note_links.id ASC",
        )
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(note_id)
        .fetch_all(&self.pool)
        .await
        .map_err(ApiError::from)
    }

    /// Lists visible backlinks for a note.
    ///
    /// # Arguments
    ///
    /// * `note_id` - Exact note identifier.
    ///
    /// # Returns
    ///
    /// Returns links whose source notes are non-deleted and non-archived.
    pub async fn list_visible_backlinks(
        &self,
        note_id: &str,
    ) -> Result<Vec<NoteLinkRecord>, ApiError> {
        sqlx::query_as::<_, NoteLinkRecord>(
            "SELECT note_links.id, note_links.source_note_id, note_links.target_note_id, note_links.anchor_text, note_links.position, note_links.created_at \
             FROM note_links \
             INNER JOIN notes source ON note_links.workspace_id = source.workspace_id AND note_links.source_note_id = source.id \
             WHERE note_links.workspace_id = ? \
             AND note_links.target_note_id = ? \
             AND source.deleted_at IS NULL \
             AND source.archived_at IS NULL \
             ORDER BY note_links.created_at ASC, note_links.id ASC",
        )
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(note_id)
        .fetch_all(&self.pool)
        .await
        .map_err(ApiError::from)
    }

    /// Adds a tag association to a note.
    ///
    /// # Arguments
    ///
    /// * `note_ref` - Full note ID or unique prefix.
    /// * `tag_name` - Normalized tag name.
    ///
    /// # Returns
    ///
    /// Returns the tag associated with the note.
    pub async fn add_tag_to_note(
        &self,
        note_ref: &str,
        tag_name: &str,
    ) -> Result<TagRecord, ApiError> {
        let note = self.get_note_by_ref(note_ref).await?;
        let mut transaction = self.pool.begin().await?;
        let tag = get_or_create_tag_in_transaction(&mut transaction, tag_name).await?;

        let result = sqlx::query(
            "INSERT OR IGNORE INTO note_tags (workspace_id, note_id, tag_id, created_at) VALUES (?, ?, ?, unixepoch())",
        )
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(&note.id)
        .bind(&tag.id)
        .execute(&mut *transaction)
        .await?;

        if result.rows_affected() > 0 {
            record_sync_change_in_transaction(
                &mut transaction,
                SyncChangeInput {
                    entity_type: "note_tag",
                    entity_id: note_tag_entity_id(&note.id, &tag.id),
                    operation: "attach",
                    base_revision_id: None,
                    new_revision_id: None,
                    payload: note_tag_payload(&note.id, &tag.id),
                },
            )
            .await?;
        }

        transaction.commit().await?;
        Ok(tag)
    }

    /// Removes a tag association from a note.
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
        let note = self.get_note_by_ref(note_ref).await?;
        let mut transaction = self.pool.begin().await?;
        let tag_id = sqlx::query_scalar::<_, String>(
            "SELECT id FROM tags WHERE workspace_id = ? AND name = ?",
        )
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(tag_name)
        .fetch_optional(&mut *transaction)
        .await?;

        if let Some(tag_id) = tag_id {
            let result = sqlx::query(
                "DELETE FROM note_tags WHERE workspace_id = ? AND note_id = ? AND tag_id = ?",
            )
            .bind(DEFAULT_WORKSPACE_ID)
            .bind(&note.id)
            .bind(&tag_id)
            .execute(&mut *transaction)
            .await?;

            if result.rows_affected() > 0 {
                record_sync_change_in_transaction(
                    &mut transaction,
                    SyncChangeInput {
                        entity_type: "note_tag",
                        entity_id: note_tag_entity_id(&note.id, &tag_id),
                        operation: "detach",
                        base_revision_id: None,
                        new_revision_id: None,
                        payload: note_tag_payload(&note.id, &tag_id),
                    },
                )
                .await?;
            }
        }

        transaction.commit().await?;

        Ok(())
    }
}

/// Creates a note inside an existing transaction.
///
/// # Arguments
///
/// * `transaction` - Open SQLite transaction.
/// * `input` - Note creation input.
///
/// # Returns
///
/// Returns the created note and resolved metadata.
async fn create_note_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    input: CreateNoteInput,
) -> Result<CreatedNote, ApiError> {
    let field = match input.field.as_deref() {
        Some(name) => Some(get_or_create_field_in_transaction(transaction, name).await?),
        None => None,
    };
    let note_id = new_id();
    let revision_id = new_id();

    sqlx::query(
        "INSERT INTO notes \
         (id, workspace_id, content, role, field_id, created_at, updated_at, archived_at, deleted_at, current_revision_id) \
         VALUES (?, ?, ?, ?, ?, unixepoch(), unixepoch(), NULL, NULL, ?)",
    )
    .bind(&note_id)
    .bind(DEFAULT_WORKSPACE_ID)
    .bind(&input.content)
    .bind(&input.role)
    .bind(field.as_ref().map(|field| field.id.as_str()))
    .bind(&revision_id)
    .execute(&mut **transaction)
    .await?;

    sqlx::query(
        "INSERT INTO note_revisions (id, workspace_id, note_id, content, title, device_id, created_at) \
         VALUES (?, ?, ?, ?, NULL, ?, unixepoch())",
    )
    .bind(&revision_id)
    .bind(DEFAULT_WORKSPACE_ID)
    .bind(&note_id)
    .bind(&input.content)
    .bind(input.device_id.as_deref())
    .execute(&mut **transaction)
    .await?;

    let mut resolved_tags = Vec::with_capacity(input.tags.len());
    for tag_name in input.tags {
        let tag = get_or_create_tag_in_transaction(transaction, &tag_name).await?;
        let result = sqlx::query(
            "INSERT OR IGNORE INTO note_tags (workspace_id, note_id, tag_id, created_at) VALUES (?, ?, ?, unixepoch())",
        )
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(&note_id)
        .bind(&tag.id)
        .execute(&mut **transaction)
        .await?;
        if result.rows_affected() > 0 {
            record_sync_change_in_transaction(
                transaction,
                SyncChangeInput {
                    entity_type: "note_tag",
                    entity_id: note_tag_entity_id(&note_id, &tag.id),
                    operation: "attach",
                    base_revision_id: None,
                    new_revision_id: None,
                    payload: note_tag_payload(&note_id, &tag.id),
                },
            )
            .await?;
        }
        resolved_tags.push(tag.name);
    }

    let links = insert_note_links_in_transaction(transaction, &note_id, &input.links).await?;
    let note = select_note_by_id(transaction, &note_id).await?;
    record_sync_change_in_transaction(
        transaction,
        SyncChangeInput {
            entity_type: "note",
            entity_id: note.id.clone(),
            operation: "insert",
            base_revision_id: None,
            new_revision_id: note.current_revision_id.clone(),
            payload: note_payload(&note),
        },
    )
    .await?;
    record_sync_change_in_transaction(
        transaction,
        SyncChangeInput {
            entity_type: "note_revision",
            entity_id: revision_id.clone(),
            operation: "insert",
            base_revision_id: None,
            new_revision_id: Some(revision_id.clone()),
            payload: serde_json::json!({
                "id": revision_id,
                "workspace_id": DEFAULT_WORKSPACE_ID,
                "note_id": note_id,
                "content": input.content,
                "title": null,
                "device_id": input.device_id,
                "created_at": note.created_at,
                "base_revision_id": null
            }),
        },
    )
    .await?;

    Ok(CreatedNote {
        note,
        field: field.map(|field| field.name),
        tags: resolved_tags,
        links,
    })
}

/// Inserts outgoing note links inside an existing transaction.
///
/// # Arguments
///
/// * `transaction` - Open SQLite transaction.
/// * `source_note_id` - Exact source note identifier.
/// * `links` - Normalized link inputs.
///
/// # Returns
///
/// Returns persisted link records.
async fn insert_note_links_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    source_note_id: &str,
    links: &[NoteLinkInput],
) -> Result<Vec<NoteLinkRecord>, ApiError> {
    let mut created = Vec::with_capacity(links.len());

    for link in links {
        let target =
            resolve_visible_note_by_ref_in_transaction(transaction, &link.target_note_ref).await?;
        if target.id == source_note_id {
            return Err(ApiError::Validation);
        }

        let link_id = new_id();
        sqlx::query(
            "INSERT INTO note_links \
             (id, workspace_id, source_note_id, target_note_id, anchor_text, position, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, unixepoch())",
        )
        .bind(&link_id)
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(source_note_id)
        .bind(&target.id)
        .bind(link.anchor_text.as_deref())
        .bind(link.position)
        .execute(&mut **transaction)
        .await?;

        let record = select_note_link_by_id(transaction, &link_id).await?;
        record_sync_change_in_transaction(
            transaction,
            SyncChangeInput {
                entity_type: "note_link",
                entity_id: record.id.clone(),
                operation: "attach",
                base_revision_id: None,
                new_revision_id: None,
                payload: note_link_payload(&record),
            },
        )
        .await?;
        created.push(record);
    }

    Ok(created)
}

/// Replaces outgoing note links inside an existing transaction.
///
/// # Arguments
///
/// * `transaction` - Open SQLite transaction.
/// * `source_note_id` - Exact source note identifier.
/// * `links` - Normalized final links for the source note.
///
/// # Returns
///
/// Returns `Ok(())` after link associations and sync changes are updated.
async fn replace_note_links_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    source_note_id: &str,
    links: &[NoteLinkInput],
) -> Result<(), ApiError> {
    let current_links = sqlx::query_as::<_, NoteLinkRecord>(
        "SELECT id, source_note_id, target_note_id, anchor_text, position, created_at \
         FROM note_links WHERE workspace_id = ? AND source_note_id = ?",
    )
    .bind(DEFAULT_WORKSPACE_ID)
    .bind(source_note_id)
    .fetch_all(&mut **transaction)
    .await?;

    for link in current_links {
        sqlx::query("DELETE FROM note_links WHERE workspace_id = ? AND id = ?")
            .bind(DEFAULT_WORKSPACE_ID)
            .bind(&link.id)
            .execute(&mut **transaction)
            .await?;
        record_sync_change_in_transaction(
            transaction,
            SyncChangeInput {
                entity_type: "note_link",
                entity_id: link.id.clone(),
                operation: "detach",
                base_revision_id: None,
                new_revision_id: None,
                payload: note_link_payload(&link),
            },
        )
        .await?;
    }

    insert_note_links_in_transaction(transaction, source_note_id, links).await?;

    Ok(())
}

/// Replaces all tag associations for a note inside an existing transaction.
///
/// # Arguments
///
/// * `transaction` - Open SQLite transaction.
/// * `note_id` - Exact note identifier.
/// * `tag_names` - Normalized final tag names for the note.
///
/// # Returns
///
/// Returns `Ok(())` after tag associations and sync changes are updated.
async fn replace_note_tags_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    note_id: &str,
    tag_names: &[String],
) -> Result<(), ApiError> {
    let current_tags = sqlx::query_as::<_, TagRecord>(
        "SELECT tags.id, tags.name, tags.created_at \
         FROM tags INNER JOIN note_tags ON tags.workspace_id = note_tags.workspace_id AND tags.id = note_tags.tag_id \
         WHERE note_tags.workspace_id = ? AND note_tags.note_id = ?",
    )
    .bind(DEFAULT_WORKSPACE_ID)
    .bind(note_id)
    .fetch_all(&mut **transaction)
    .await?;
    let current_ids = current_tags
        .iter()
        .map(|tag| tag.id.clone())
        .collect::<HashSet<_>>();

    let mut target_tags = Vec::with_capacity(tag_names.len());
    for tag_name in tag_names {
        target_tags.push(get_or_create_tag_in_transaction(transaction, tag_name).await?);
    }
    let target_ids = target_tags
        .iter()
        .map(|tag| tag.id.clone())
        .collect::<HashSet<_>>();

    for tag in &target_tags {
        if !current_ids.contains(&tag.id) {
            sqlx::query(
                "INSERT OR IGNORE INTO note_tags (workspace_id, note_id, tag_id, created_at) VALUES (?, ?, ?, unixepoch())",
            )
            .bind(DEFAULT_WORKSPACE_ID)
            .bind(note_id)
            .bind(&tag.id)
            .execute(&mut **transaction)
            .await?;
            record_sync_change_in_transaction(
                transaction,
                SyncChangeInput {
                    entity_type: "note_tag",
                    entity_id: note_tag_entity_id(note_id, &tag.id),
                    operation: "attach",
                    base_revision_id: None,
                    new_revision_id: None,
                    payload: note_tag_payload(note_id, &tag.id),
                },
            )
            .await?;
        }
    }

    for tag in &current_tags {
        if !target_ids.contains(&tag.id) {
            sqlx::query(
                "DELETE FROM note_tags WHERE workspace_id = ? AND note_id = ? AND tag_id = ?",
            )
            .bind(DEFAULT_WORKSPACE_ID)
            .bind(note_id)
            .bind(&tag.id)
            .execute(&mut **transaction)
            .await?;
            record_sync_change_in_transaction(
                transaction,
                SyncChangeInput {
                    entity_type: "note_tag",
                    entity_id: note_tag_entity_id(note_id, &tag.id),
                    operation: "detach",
                    base_revision_id: None,
                    new_revision_id: None,
                    payload: note_tag_payload(note_id, &tag.id),
                },
            )
            .await?;
        }
    }

    Ok(())
}

/// Selects a note by exact ID inside a transaction.
///
/// # Arguments
///
/// * `transaction` - Open SQLite transaction.
/// * `note_id` - Exact note identifier.
///
/// # Returns
///
/// Returns the matching note record.
async fn select_note_by_id(
    transaction: &mut Transaction<'_, Sqlite>,
    note_id: &str,
) -> Result<NoteRecord, sqlx::Error> {
    sqlx::query_as::<_, NoteRecord>(
        "SELECT id, content, role, field_id, created_at, updated_at, archived_at, deleted_at, current_revision_id \
         FROM notes WHERE workspace_id = ? AND id = ?",
    )
    .bind(DEFAULT_WORKSPACE_ID)
    .bind(note_id)
    .fetch_one(&mut **transaction)
    .await
}

/// Selects a note link by exact ID inside a transaction.
///
/// # Arguments
///
/// * `transaction` - Open SQLite transaction.
/// * `link_id` - Exact link identifier.
///
/// # Returns
///
/// Returns the matching note link record.
async fn select_note_link_by_id(
    transaction: &mut Transaction<'_, Sqlite>,
    link_id: &str,
) -> Result<NoteLinkRecord, sqlx::Error> {
    sqlx::query_as::<_, NoteLinkRecord>(
        "SELECT id, source_note_id, target_note_id, anchor_text, position, created_at \
         FROM note_links WHERE workspace_id = ? AND id = ?",
    )
    .bind(DEFAULT_WORKSPACE_ID)
    .bind(link_id)
    .fetch_one(&mut **transaction)
    .await
}

/// Resolves a visible note by full ID or unique prefix inside a transaction.
///
/// # Arguments
///
/// * `transaction` - Open SQLite transaction.
/// * `note_ref` - Full note ID or unique prefix.
///
/// # Returns
///
/// Returns the matching non-deleted and non-archived note.
async fn resolve_visible_note_by_ref_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    note_ref: &str,
) -> Result<NoteRecord, ApiError> {
    validate_note_ref(note_ref)?;

    let pattern = format!("{note_ref}%");
    let notes = sqlx::query_as::<_, NoteRecord>(
        "SELECT id, content, role, field_id, created_at, updated_at, archived_at, deleted_at, current_revision_id \
         FROM notes WHERE workspace_id = ? AND id LIKE ? AND deleted_at IS NULL AND archived_at IS NULL ORDER BY id ASC LIMIT 2",
    )
    .bind(DEFAULT_WORKSPACE_ID)
    .bind(pattern)
    .fetch_all(&mut **transaction)
    .await?;

    match notes.as_slice() {
        [note] => Ok(note.clone()),
        [] => Err(ApiError::RecordNotFound(format!(
            "Note reference \"{note_ref}\" did not match any visible note."
        ))),
        _ => Err(ApiError::AmbiguousNoteReference(format!(
            "Note reference \"{note_ref}\" matched multiple visible notes."
        ))),
    }
}

/// Builds a sync payload for a note record.
///
/// # Arguments
///
/// * `note` - Note record to serialize.
///
/// # Returns
///
/// Returns a JSON payload containing the workspace-scoped note snapshot.
fn note_payload(note: &NoteRecord) -> serde_json::Value {
    serde_json::json!({
        "id": note.id,
        "workspace_id": DEFAULT_WORKSPACE_ID,
        "content": note.content,
        "role": note.role,
        "field_id": note.field_id,
        "created_at": note.created_at,
        "updated_at": note.updated_at,
        "archived_at": note.archived_at,
        "deleted_at": note.deleted_at,
        "current_revision_id": note.current_revision_id
    })
}

/// Builds a stable synthetic entity ID for a note/tag relation.
///
/// # Arguments
///
/// * `note_id` - Note identifier.
/// * `tag_id` - Tag identifier.
///
/// # Returns
///
/// Returns a relation identifier.
fn note_tag_entity_id(note_id: &str, tag_id: &str) -> String {
    format!("{note_id}:{tag_id}")
}

/// Builds a sync payload for a note/tag relation.
///
/// # Arguments
///
/// * `note_id` - Note identifier.
/// * `tag_id` - Tag identifier.
///
/// # Returns
///
/// Returns a JSON payload for the relation change.
fn note_tag_payload(note_id: &str, tag_id: &str) -> serde_json::Value {
    serde_json::json!({
        "workspace_id": DEFAULT_WORKSPACE_ID,
        "note_id": note_id,
        "tag_id": tag_id
    })
}

/// Builds a sync payload for a note link relation.
///
/// # Arguments
///
/// * `link` - Persisted note link record.
///
/// # Returns
///
/// Returns a JSON payload for the relation change.
fn note_link_payload(link: &NoteLinkRecord) -> serde_json::Value {
    serde_json::json!({
        "id": link.id,
        "workspace_id": DEFAULT_WORKSPACE_ID,
        "source_note_id": link.source_note_id,
        "target_note_id": link.target_note_id,
        "anchor_text": link.anchor_text,
        "position": link.position,
        "created_at": link.created_at
    })
}

/// Validates a note reference before SQL lookup.
///
/// # Arguments
///
/// * `note_ref` - Full note ID or prefix.
///
/// # Returns
///
/// Returns `Ok(())` when the reference can be queried safely.
fn validate_note_ref(note_ref: &str) -> Result<(), ApiError> {
    if note_ref.len() < 4 {
        return Err(ApiError::NoteReferenceTooShort);
    }

    if !note_ref
        .chars()
        .all(|character| character.is_ascii_hexdigit())
    {
        return Err(ApiError::InvalidNoteReference);
    }

    Ok(())
}

/// Validates a full note UUID before cursor lookup.
///
/// # Arguments
///
/// * `note_uuid` - Full note ID.
///
/// # Returns
///
/// Returns `Ok(())` when the UUID is a 32-character hexadecimal string.
fn validate_full_note_uuid(note_uuid: &str) -> Result<(), ApiError> {
    if note_uuid.len() != 32 {
        return Err(ApiError::Validation);
    }

    if !note_uuid
        .chars()
        .all(|character| character.is_ascii_hexdigit())
    {
        return Err(ApiError::Validation);
    }

    Ok(())
}

#[cfg(test)]
mod tests;
