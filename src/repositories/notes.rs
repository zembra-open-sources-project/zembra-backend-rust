use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::error::ApiError;
use crate::models::field::FieldRecord;
use crate::models::note::NoteRecord;
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
             FROM notes WHERE workspace_id = ? AND deleted_at IS NULL ORDER BY updated_at DESC LIMIT ?",
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
    ///
    /// # Returns
    ///
    /// Returns non-deleted and non-archived note records for the tag.
    pub async fn list_visible_notes_by_tag(
        &self,
        tag_id: &str,
    ) -> Result<Vec<NoteRecord>, ApiError> {
        sqlx::query_as::<_, NoteRecord>(
            "SELECT notes.id, notes.content, notes.role, notes.field_id, notes.created_at, notes.updated_at, notes.archived_at, notes.deleted_at, notes.current_revision_id \
             FROM notes \
             INNER JOIN note_tags ON notes.workspace_id = note_tags.workspace_id AND notes.id = note_tags.note_id \
             WHERE note_tags.workspace_id = ? \
             AND note_tags.tag_id = ? \
             AND notes.deleted_at IS NULL \
             AND notes.archived_at IS NULL \
             ORDER BY notes.updated_at DESC, notes.id DESC",
        )
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(tag_id)
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

    /// Resolves a note by full ID or unique hexadecimal prefix.
    ///
    /// # Arguments
    ///
    /// * `note_ref` - Full 32-character ID or at least 4-character prefix.
    ///
    /// # Returns
    ///
    /// Returns the matching note or a note reference error.
    pub async fn get_note_by_ref(&self, note_ref: &str) -> Result<NoteRecord, ApiError> {
        validate_note_ref(note_ref)?;

        let pattern = format!("{note_ref}%");
        let notes = sqlx::query_as::<_, NoteRecord>(
            "SELECT id, content, role, field_id, created_at, updated_at, archived_at, deleted_at, current_revision_id \
             FROM notes WHERE workspace_id = ? AND id LIKE ? AND deleted_at IS NULL ORDER BY id ASC LIMIT 2",
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

    /// Updates note content and creates a new current revision.
    ///
    /// # Arguments
    ///
    /// * `note_ref` - Full note ID or unique prefix.
    /// * `content` - New note content.
    /// * `device_id` - Optional device identifier.
    ///
    /// # Returns
    ///
    /// Returns the updated note record.
    pub async fn update_note_content(
        &self,
        note_ref: &str,
        content: &str,
        device_id: Option<&str>,
    ) -> Result<NoteRecord, ApiError> {
        let note = self.get_note_by_ref(note_ref).await?;
        let mut transaction = self.pool.begin().await?;
        let revision_id = new_id();

        sqlx::query(
            "INSERT INTO note_revisions (id, workspace_id, note_id, content, title, device_id, created_at) \
             VALUES (?, ?, ?, ?, NULL, ?, unixepoch())",
        )
        .bind(&revision_id)
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(&note.id)
        .bind(content)
        .bind(device_id)
        .execute(&mut *transaction)
        .await?;

        sqlx::query(
            "UPDATE notes SET content = ?, updated_at = unixepoch(), current_revision_id = ? WHERE workspace_id = ? AND id = ?",
        )
        .bind(content)
        .bind(&revision_id)
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(&note.id)
        .execute(&mut *transaction)
        .await?;

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
                    "content": content,
                    "title": null,
                    "device_id": device_id,
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

        sqlx::query_as::<_, TagRecord>(
            "SELECT tags.id, tags.name, tags.created_at \
             FROM tags INNER JOIN note_tags ON tags.workspace_id = note_tags.workspace_id AND tags.id = note_tags.tag_id \
             WHERE note_tags.workspace_id = ? AND note_tags.note_id = ? ORDER BY tags.name ASC",
        )
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(note.id)
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
    })
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
mod tests {
    use super::{CreateNoteInput, NotesRepository};
    use crate::error::ApiError;
    use crate::repositories::database::Database;
    use crate::repositories::sync::list_sync_changes;
    use crate::repositories::taxonomy::TaxonomyRepository;

    /// Creates an in-memory notes repository for tests.
    ///
    /// # Returns
    ///
    /// Returns a repository with migrated SQLite schema.
    async fn notes_repository() -> NotesRepository {
        let database = Database::connect("sqlite://:memory:").await.unwrap();
        NotesRepository::new(database.pool)
    }

    /// Creates an in-memory taxonomy repository for tests.
    ///
    /// # Returns
    ///
    /// Returns a repository with migrated SQLite schema.
    async fn taxonomy_repository() -> TaxonomyRepository {
        let database = Database::connect("sqlite://:memory:").await.unwrap();
        TaxonomyRepository::new(database.pool)
    }

    /// Creates an in-memory taxonomy repository with its pool for tests.
    ///
    /// # Returns
    ///
    /// Returns a taxonomy repository and the migrated SQLite pool.
    async fn taxonomy_repository_with_pool() -> (TaxonomyRepository, sqlx::SqlitePool) {
        let database = Database::connect("sqlite://:memory:").await.unwrap();
        let pool = database.pool.clone();
        (TaxonomyRepository::new(database.pool), pool)
    }

    /// Builds a default note creation input.
    ///
    /// # Arguments
    ///
    /// * `content` - Note body content.
    ///
    /// # Returns
    ///
    /// Returns a note creation input.
    fn input(content: &str) -> CreateNoteInput {
        CreateNoteInput {
            content: content.to_string(),
            field: Some("work".to_string()),
            tags: vec!["rust".to_string(), "sqlite".to_string()],
            role: "Human".to_string(),
            device_id: None,
        }
    }

    #[tokio::test]
    async fn create_note_writes_revision_field_and_tags() {
        let repository = notes_repository().await;

        let created = repository.create_note(input("hello")).await.unwrap();
        let revisions = repository
            .list_note_revisions(&created.note.id)
            .await
            .unwrap();
        let tags = repository.list_note_tags(&created.note.id).await.unwrap();

        assert_eq!(created.note.content, "hello");
        assert_eq!(created.note.role, "Human");
        assert_eq!(created.field.as_deref(), Some("work"));
        assert_eq!(created.tags, vec!["rust", "sqlite"]);
        assert_eq!(revisions.len(), 1);
        assert_eq!(revisions[0].content, "hello");
        assert_eq!(
            tags.iter().map(|tag| tag.name.as_str()).collect::<Vec<_>>(),
            vec!["rust", "sqlite"]
        );
    }

    #[tokio::test]
    async fn create_note_records_sync_changes() {
        let repository = notes_repository().await;

        let created = repository.create_note(input("hello sync")).await.unwrap();
        let changes = list_sync_changes(&repository.pool).await.unwrap();

        assert_eq!(changes.len(), 7);
        assert!(changes.iter().any(|change| {
            change.entity_type == "field"
                && change.operation == "insert"
                && change.payload.contains("\"name\":\"work\"")
        }));
        assert_eq!(
            changes
                .iter()
                .filter(|change| change.entity_type == "tag" && change.operation == "insert")
                .count(),
            2
        );
        assert_eq!(
            changes
                .iter()
                .filter(|change| change.entity_type == "note_tag" && change.operation == "attach")
                .count(),
            2
        );
        assert!(changes.iter().any(|change| {
            change.entity_type == "note"
                && change.entity_id == created.note.id
                && change.operation == "insert"
        }));
        assert!(changes.iter().any(|change| {
            change.entity_type == "note_revision" && change.operation == "insert"
        }));
    }

    #[tokio::test]
    async fn batch_create_rolls_back_when_one_item_fails() {
        let repository = notes_repository().await;
        let items = vec![
            input("first"),
            CreateNoteInput {
                content: "second".to_string(),
                field: None,
                tags: Vec::new(),
                role: "Robot".to_string(),
                device_id: None,
            },
        ];

        let result = repository.create_notes_batch(items).await;
        let notes = repository.list_notes(None).await.unwrap();

        assert!(matches!(result, Err(ApiError::Database(_))));
        assert!(notes.is_empty());
    }

    #[tokio::test]
    async fn update_note_content_writes_new_revision() {
        let repository = notes_repository().await;
        let created = repository.create_note(input("old")).await.unwrap();

        let updated = repository
            .update_note_content(&created.note.id, "new", None)
            .await
            .unwrap();
        let revisions = repository
            .list_note_revisions(&created.note.id)
            .await
            .unwrap();

        assert_eq!(updated.content, "new");
        assert_eq!(revisions.len(), 2);
        assert_eq!(revisions[1].content, "new");
        assert_eq!(updated.current_revision_id, Some(revisions[1].id.clone()));
    }

    #[tokio::test]
    async fn update_archive_and_delete_record_note_sync_changes() {
        let repository = notes_repository().await;
        let created = repository.create_note(input("old")).await.unwrap();

        repository
            .update_note_content(&created.note.id, "new", None)
            .await
            .unwrap();
        repository.archive_note(&created.note.id).await.unwrap();
        repository.delete_note(&created.note.id).await.unwrap();
        let changes = list_sync_changes(&repository.pool).await.unwrap();

        assert!(changes.iter().any(|change| {
            change.entity_type == "note_revision" && change.operation == "insert"
        }));
        assert!(
            changes
                .iter()
                .any(|change| { change.entity_type == "note" && change.operation == "update" })
        );
        assert!(
            changes
                .iter()
                .any(|change| { change.entity_type == "note" && change.operation == "delete" })
        );
    }

    #[tokio::test]
    async fn delete_note_hides_record_from_default_reads() {
        let repository = notes_repository().await;
        let created = repository.create_note(input("hidden")).await.unwrap();

        repository.delete_note(&created.note.id).await.unwrap();
        let list = repository.list_notes(None).await.unwrap();
        let get = repository.get_note_by_ref(&created.note.id).await;

        assert!(list.is_empty());
        assert!(matches!(get, Err(ApiError::RecordNotFound(_))));
    }

    #[tokio::test]
    async fn taxonomy_lists_records_by_name() {
        let repository = taxonomy_repository().await;

        repository.get_or_create_field("zeta").await.unwrap();
        repository.get_or_create_field("alpha").await.unwrap();
        repository.get_or_create_tag("rust").await.unwrap();
        repository.get_or_create_tag("api").await.unwrap();

        let fields = repository.list_fields(None).await.unwrap();
        let tags = repository.list_tags(None).await.unwrap();

        assert_eq!(
            fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha", "zeta"]
        );
        assert_eq!(
            tags.iter().map(|tag| tag.name.as_str()).collect::<Vec<_>>(),
            vec!["api", "rust"]
        );
    }

    #[tokio::test]
    async fn taxonomy_creates_sync_changes_only_for_new_records() {
        let (repository, pool) = taxonomy_repository_with_pool().await;

        repository.get_or_create_field("work").await.unwrap();
        repository.get_or_create_field("work").await.unwrap();
        repository.get_or_create_tag("rust").await.unwrap();
        repository.get_or_create_tag("rust").await.unwrap();
        let changes = list_sync_changes(&pool).await.unwrap();

        assert_eq!(
            changes
                .iter()
                .filter(|change| change.entity_type == "field" && change.operation == "insert")
                .count(),
            1
        );
        assert_eq!(
            changes
                .iter()
                .filter(|change| change.entity_type == "tag" && change.operation == "insert")
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn tag_association_is_idempotent_and_removable() {
        let repository = notes_repository().await;
        let created = repository.create_note(input("tagged")).await.unwrap();

        repository
            .add_tag_to_note(&created.note.id, "rust")
            .await
            .unwrap();
        repository
            .remove_tag_from_note(&created.note.id, "rust")
            .await
            .unwrap();
        let tags = repository.list_note_tags(&created.note.id).await.unwrap();

        assert_eq!(
            tags.iter().map(|tag| tag.name.as_str()).collect::<Vec<_>>(),
            vec!["sqlite"]
        );

        let changes = list_sync_changes(&repository.pool).await.unwrap();
        assert_eq!(
            changes
                .iter()
                .filter(|change| change.entity_type == "note_tag" && change.operation == "detach")
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn list_recent_notes_orders_and_filters_hidden_records() {
        let repository = notes_repository().await;
        let oldest = repository.create_note(input("oldest")).await.unwrap();
        let archived = repository.create_note(input("archived")).await.unwrap();
        let deleted = repository.create_note(input("deleted")).await.unwrap();
        let newest = repository.create_note(input("newest")).await.unwrap();

        sqlx::query("UPDATE notes SET updated_at = ? WHERE id = ?")
            .bind(2_000_000_010_i64)
            .bind(&oldest.note.id)
            .execute(&repository.pool)
            .await
            .unwrap();
        sqlx::query("UPDATE notes SET updated_at = ? WHERE id = ?")
            .bind(2_000_000_040_i64)
            .bind(&archived.note.id)
            .execute(&repository.pool)
            .await
            .unwrap();
        sqlx::query("UPDATE notes SET updated_at = ? WHERE id = ?")
            .bind(2_000_000_030_i64)
            .bind(&deleted.note.id)
            .execute(&repository.pool)
            .await
            .unwrap();
        sqlx::query("UPDATE notes SET updated_at = ? WHERE id = ?")
            .bind(2_000_000_020_i64)
            .bind(&newest.note.id)
            .execute(&repository.pool)
            .await
            .unwrap();
        repository.archive_note(&archived.note.id).await.unwrap();
        repository.delete_note(&deleted.note.id).await.unwrap();

        let recent = repository.list_recent_notes(10, None).await.unwrap();

        assert_eq!(
            recent
                .iter()
                .map(|note| note.content.as_str())
                .collect::<Vec<_>>(),
            vec!["newest", "oldest"]
        );
    }

    #[tokio::test]
    async fn list_recent_notes_applies_limit() {
        let repository = notes_repository().await;
        repository.create_note(input("first")).await.unwrap();
        repository.create_note(input("second")).await.unwrap();

        let recent = repository.list_recent_notes(1, None).await.unwrap();

        assert_eq!(recent.len(), 1);
    }

    #[tokio::test]
    async fn list_recent_notes_uses_full_note_uuid_cursor() {
        let repository = notes_repository().await;
        let oldest = repository.create_note(input("oldest")).await.unwrap();
        let cursor = repository.create_note(input("cursor")).await.unwrap();
        let newest = repository.create_note(input("newest")).await.unwrap();

        sqlx::query("UPDATE notes SET updated_at = ? WHERE id = ?")
            .bind(2_000_000_010_i64)
            .bind(&oldest.note.id)
            .execute(&repository.pool)
            .await
            .unwrap();
        sqlx::query("UPDATE notes SET updated_at = ? WHERE id = ?")
            .bind(2_000_000_020_i64)
            .bind(&cursor.note.id)
            .execute(&repository.pool)
            .await
            .unwrap();
        sqlx::query("UPDATE notes SET updated_at = ? WHERE id = ?")
            .bind(2_000_000_030_i64)
            .bind(&newest.note.id)
            .execute(&repository.pool)
            .await
            .unwrap();

        let recent = repository
            .list_recent_notes(10, Some(&cursor.note.id))
            .await
            .unwrap();

        assert_eq!(
            recent
                .iter()
                .map(|note| note.content.as_str())
                .collect::<Vec<_>>(),
            vec!["oldest"]
        );
    }

    #[tokio::test]
    async fn list_recent_notes_uses_id_tiebreaker_for_cursor() {
        let repository = notes_repository().await;
        let low_id = repository.create_note(input("low")).await.unwrap();
        let cursor_id = repository.create_note(input("cursor")).await.unwrap();
        let high_id = repository.create_note(input("high")).await.unwrap();

        sqlx::query("UPDATE notes SET id = ?, updated_at = ? WHERE id = ?")
            .bind("10000000000000000000000000000000")
            .bind(2_000_000_010_i64)
            .bind(&low_id.note.id)
            .execute(&repository.pool)
            .await
            .unwrap();
        sqlx::query("UPDATE notes SET id = ?, updated_at = ? WHERE id = ?")
            .bind("20000000000000000000000000000000")
            .bind(2_000_000_010_i64)
            .bind(&cursor_id.note.id)
            .execute(&repository.pool)
            .await
            .unwrap();
        sqlx::query("UPDATE notes SET id = ?, updated_at = ? WHERE id = ?")
            .bind("30000000000000000000000000000000")
            .bind(2_000_000_010_i64)
            .bind(&high_id.note.id)
            .execute(&repository.pool)
            .await
            .unwrap();

        let recent = repository
            .list_recent_notes(10, Some("20000000000000000000000000000000"))
            .await
            .unwrap();

        assert_eq!(
            recent
                .iter()
                .map(|note| note.content.as_str())
                .collect::<Vec<_>>(),
            vec!["low"]
        );
    }

    #[tokio::test]
    async fn list_recent_notes_rejects_invalid_or_hidden_cursor() {
        let repository = notes_repository().await;
        let archived = repository.create_note(input("archived")).await.unwrap();
        repository.archive_note(&archived.note.id).await.unwrap();

        let invalid = repository.list_recent_notes(10, Some("abcd")).await;
        let hidden = repository
            .list_recent_notes(10, Some(&archived.note.id))
            .await;
        let missing = repository
            .list_recent_notes(10, Some("ffffffffffffffffffffffffffffffff"))
            .await;

        assert!(matches!(invalid, Err(ApiError::Validation)));
        assert!(matches!(hidden, Err(ApiError::RecordNotFound(_))));
        assert!(matches!(missing, Err(ApiError::RecordNotFound(_))));
    }

    #[tokio::test]
    async fn random_tags_returns_existing_tags_when_limit_is_larger() {
        let repository = notes_repository().await;
        repository.create_note(input("first")).await.unwrap();

        let tags = repository.random_tags(20).await.unwrap();

        assert_eq!(tags.len(), 2);
        assert!(
            tags.iter()
                .all(|tag| tag.name == "rust" || tag.name == "sqlite")
        );
    }

    #[tokio::test]
    async fn list_visible_notes_by_tag_filters_hidden_records() {
        let repository = notes_repository().await;
        let visible = repository.create_note(input("visible")).await.unwrap();
        let archived = repository.create_note(input("archived")).await.unwrap();
        let deleted = repository.create_note(input("deleted")).await.unwrap();
        repository.archive_note(&archived.note.id).await.unwrap();
        repository.delete_note(&deleted.note.id).await.unwrap();
        let tag = repository
            .list_note_tags(&visible.note.id)
            .await
            .unwrap()
            .into_iter()
            .find(|tag| tag.name == "rust")
            .unwrap();

        let notes = repository.list_visible_notes_by_tag(&tag.id).await.unwrap();

        assert_eq!(
            notes
                .iter()
                .map(|note| note.content.as_str())
                .collect::<Vec<_>>(),
            vec!["visible"]
        );
    }

    #[tokio::test]
    async fn random_fields_returns_existing_fields_when_limit_is_larger() {
        let repository = notes_repository().await;
        repository.create_note(input("first")).await.unwrap();

        let fields = repository.random_fields(20).await.unwrap();

        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "work");
    }

    #[tokio::test]
    async fn list_visible_notes_by_field_filters_hidden_records_and_applies_limit() {
        let repository = notes_repository().await;
        let visible = repository.create_note(input("visible")).await.unwrap();
        repository
            .create_note(input("second visible"))
            .await
            .unwrap();
        let archived = repository.create_note(input("archived")).await.unwrap();
        let deleted = repository.create_note(input("deleted")).await.unwrap();
        repository.archive_note(&archived.note.id).await.unwrap();
        repository.delete_note(&deleted.note.id).await.unwrap();
        let field_id = visible.note.field_id.as_deref().unwrap();

        let notes = repository
            .list_visible_notes_by_field(field_id, 1)
            .await
            .unwrap();

        assert_eq!(notes.len(), 1);
        assert!(notes[0].content == "visible" || notes[0].content == "second visible");
    }
}
