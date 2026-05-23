use super::ids::{NoteId, RevisionId};
use super::links::{insert_note_links_in_transaction, replace_note_links_in_transaction};
use super::payloads::note_payload;
use super::revisions::insert_note_revision_in_transaction;
use super::tags::{attach_tag_to_note_in_transaction, replace_note_tags_in_transaction};
use super::types::{CreateNoteInput, CreatedNote, DailyNoteCountRow, UpdateNoteInput};
use super::validation::{validate_full_note_uuid, validate_note_ref};

use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::error::ApiError;
use crate::models::field::FieldRecord;
use crate::models::note::NoteRecord;
use crate::models::note_link::NoteLinkRecord;
use crate::models::tag::TagRecord;
use crate::repositories::sync::{SyncChangeInput, record_sync_change_in_transaction};
use crate::repositories::taxonomy::{
    DEFAULT_WORKSPACE_ID, get_or_create_field_in_transaction, new_id,
};

/// Repository for note data access.
#[derive(Debug, Clone)]
pub struct NotesRepository {
    /// SQLx pool used by repository queries.
    pub(super) pool: SqlitePool,
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

        insert_note_revision_in_transaction(
            &mut transaction,
            &revision_id,
            &note.id,
            &input.content,
            input.device_id.as_deref(),
        )
        .await?;

        update_note_row_in_transaction(
            &mut transaction,
            NoteId::new(&note.id),
            RevisionId::new(&revision_id),
            &input,
            field.as_ref(),
        )
        .await?;

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

    insert_note_row_in_transaction(
        transaction,
        NoteId::new(&note_id),
        RevisionId::new(&revision_id),
        &input,
        field.as_ref(),
    )
    .await?;

    insert_note_revision_in_transaction(
        transaction,
        &revision_id,
        &note_id,
        &input.content,
        input.device_id.as_deref(),
    )
    .await?;

    let note_for_tags = NoteRecord {
        id: note_id.clone(),
        content: input.content.clone(),
        role: input.role.clone(),
        field_id: field.as_ref().map(|field| field.id.clone()),
        created_at: 0,
        updated_at: 0,
        archived_at: None,
        deleted_at: None,
        current_revision_id: Some(revision_id.clone()),
    };
    let mut resolved_tags = Vec::with_capacity(input.tags.len());
    for tag_name in input.tags {
        let tag = attach_tag_to_note_in_transaction(transaction, &note_for_tags, &tag_name).await?;
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

/// Inserts a note row inside an existing transaction.
///
/// # Arguments
///
/// * `transaction` - Open SQLite transaction.
/// * `note_id` - Exact note identifier.
/// * `revision_id` - Current revision identifier.
/// * `input` - Note creation input.
/// * `field` - Optional resolved field record.
///
/// # Returns
///
/// Returns `Ok(())` after the note row is inserted.
async fn insert_note_row_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    note_id: NoteId<'_>,
    revision_id: RevisionId<'_>,
    input: &CreateNoteInput,
    field: Option<&FieldRecord>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO notes \
         (id, workspace_id, content, role, field_id, created_at, updated_at, archived_at, deleted_at, current_revision_id) \
         VALUES (?, ?, ?, ?, ?, unixepoch(), unixepoch(), NULL, NULL, ?)",
    )
    .bind(note_id.as_str())
    .bind(DEFAULT_WORKSPACE_ID)
    .bind(&input.content)
    .bind(&input.role)
    .bind(field.map(|field| field.id.as_str()))
    .bind(revision_id.as_str())
    .execute(&mut **transaction)
    .await?;

    Ok(())
}

/// Updates a note row inside an existing transaction.
///
/// # Arguments
///
/// * `transaction` - Open SQLite transaction.
/// * `note_id` - Exact note identifier.
/// * `revision_id` - New current revision identifier.
/// * `input` - Note update input.
/// * `field` - Optional resolved replacement field.
///
/// # Returns
///
/// Returns `Ok(())` after the note row is updated.
async fn update_note_row_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    note_id: NoteId<'_>,
    revision_id: RevisionId<'_>,
    input: &UpdateNoteInput,
    field: Option<&FieldRecord>,
) -> Result<(), sqlx::Error> {
    match field {
        Some(field) => {
            sqlx::query(
                "UPDATE notes SET content = ?, field_id = ?, updated_at = unixepoch(), current_revision_id = ? WHERE workspace_id = ? AND id = ?",
            )
            .bind(&input.content)
            .bind(&field.id)
            .bind(revision_id.as_str())
            .bind(DEFAULT_WORKSPACE_ID)
            .bind(note_id.as_str())
            .execute(&mut **transaction)
            .await?;
        }
        None => {
            sqlx::query(
                "UPDATE notes SET content = ?, updated_at = unixepoch(), current_revision_id = ? WHERE workspace_id = ? AND id = ?",
            )
            .bind(&input.content)
            .bind(revision_id.as_str())
            .bind(DEFAULT_WORKSPACE_ID)
            .bind(note_id.as_str())
            .execute(&mut **transaction)
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
pub(super) async fn select_note_by_id(
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
pub(super) async fn select_note_link_by_id(
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
pub(super) async fn resolve_visible_note_by_ref_in_transaction(
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
