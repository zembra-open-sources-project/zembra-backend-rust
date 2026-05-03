use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::error::ApiError;
use crate::models::note::NoteRecord;
use crate::models::revision::NoteRevisionRecord;
use crate::models::tag::TagRecord;
use crate::repositories::taxonomy::{
    get_or_create_field_in_transaction, get_or_create_tag_in_transaction, new_id,
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
             FROM notes WHERE deleted_at IS NULL ORDER BY updated_at DESC LIMIT ?",
        )
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
                     WHERE deleted_at IS NULL \
                     AND archived_at IS NULL \
                     AND (updated_at < ? OR (updated_at = ? AND id < ?)) \
                     ORDER BY updated_at DESC, id DESC LIMIT ?",
                )
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
                 FROM notes WHERE deleted_at IS NULL AND archived_at IS NULL ORDER BY updated_at DESC, id DESC LIMIT ?",
            )
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(ApiError::from),
        }
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
             FROM notes WHERE id = ? AND deleted_at IS NULL AND archived_at IS NULL",
        )
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
             FROM notes WHERE id LIKE ? AND deleted_at IS NULL ORDER BY id ASC LIMIT 2",
        )
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
            "INSERT INTO note_revisions (id, note_id, content, title, device_id, created_at) \
             VALUES (?, ?, ?, NULL, ?, unixepoch())",
        )
        .bind(&revision_id)
        .bind(&note.id)
        .bind(content)
        .bind(device_id)
        .execute(&mut *transaction)
        .await?;

        sqlx::query(
            "UPDATE notes SET content = ?, updated_at = unixepoch(), current_revision_id = ? WHERE id = ?",
        )
        .bind(content)
        .bind(&revision_id)
        .bind(&note.id)
        .execute(&mut *transaction)
        .await?;

        let updated = select_note_by_id(&mut transaction, &note.id).await?;
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
        sqlx::query(
            "UPDATE notes SET archived_at = unixepoch(), updated_at = unixepoch() WHERE id = ?",
        )
        .bind(&note.id)
        .execute(&self.pool)
        .await?;

        self.get_note_by_ref(&note.id).await
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
        sqlx::query(
            "UPDATE notes SET deleted_at = unixepoch(), updated_at = unixepoch() WHERE id = ?",
        )
        .bind(&note.id)
        .execute(&self.pool)
        .await?;

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
             FROM note_revisions WHERE note_id = ? ORDER BY created_at ASC, rowid ASC",
        )
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
             FROM tags INNER JOIN note_tags ON tags.id = note_tags.tag_id \
             WHERE note_tags.note_id = ? ORDER BY tags.name ASC",
        )
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

        sqlx::query(
            "INSERT OR IGNORE INTO note_tags (note_id, tag_id, created_at) VALUES (?, ?, unixepoch())",
        )
        .bind(&note.id)
        .bind(&tag.id)
        .execute(&mut *transaction)
        .await?;

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

        sqlx::query(
            "DELETE FROM note_tags \
             WHERE note_id = ? AND tag_id IN (SELECT id FROM tags WHERE name = ?)",
        )
        .bind(note.id)
        .bind(tag_name)
        .execute(&self.pool)
        .await?;

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
         (id, content, role, field_id, created_at, updated_at, archived_at, deleted_at, current_revision_id) \
         VALUES (?, ?, ?, ?, unixepoch(), unixepoch(), NULL, NULL, ?)",
    )
    .bind(&note_id)
    .bind(&input.content)
    .bind(&input.role)
    .bind(field.as_ref().map(|field| field.id.as_str()))
    .bind(&revision_id)
    .execute(&mut **transaction)
    .await?;

    sqlx::query(
        "INSERT INTO note_revisions (id, note_id, content, title, device_id, created_at) \
         VALUES (?, ?, ?, NULL, ?, unixepoch())",
    )
    .bind(&revision_id)
    .bind(&note_id)
    .bind(&input.content)
    .bind(input.device_id.as_deref())
    .execute(&mut **transaction)
    .await?;

    let mut resolved_tags = Vec::with_capacity(input.tags.len());
    for tag_name in input.tags {
        let tag = get_or_create_tag_in_transaction(transaction, &tag_name).await?;
        sqlx::query(
            "INSERT OR IGNORE INTO note_tags (note_id, tag_id, created_at) VALUES (?, ?, unixepoch())",
        )
        .bind(&note_id)
        .bind(&tag.id)
        .execute(&mut **transaction)
        .await?;
        resolved_tags.push(tag.name);
    }

    let note = select_note_by_id(transaction, &note_id).await?;

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
         FROM notes WHERE id = ?",
    )
    .bind(note_id)
    .fetch_one(&mut **transaction)
    .await
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
}
