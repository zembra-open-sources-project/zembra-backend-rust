use sqlx::{Sqlite, SqlitePool, Transaction};
use uuid::Uuid;

use crate::models::field::FieldRecord;
use crate::models::tag::TagRecord;
use crate::repositories::sync::{SyncChangeInput, record_sync_change_in_transaction};

/// Default workspace inserted by shared schema v0.3.0 for legacy local data.
pub const DEFAULT_WORKSPACE_ID: &str = "00000000-0000-4000-8000-000000000300";

/// Parsed hierarchical tag path ready for database writes.
struct TagPath {
    /// Non-empty tag path segments in root-to-leaf order.
    segments: Vec<String>,
}

impl TagPath {
    /// Parses a client tag string into non-empty hierarchical path segments.
    ///
    /// # Arguments
    ///
    /// * `name` - Normalized tag name or slash-delimited path.
    ///
    /// # Returns
    ///
    /// Returns a tag path with empty path segments removed.
    fn parse(name: &str) -> Self {
        let segments = name
            .split('/')
            .map(str::trim)
            .filter(|segment| !segment.is_empty())
            .map(ToOwned::to_owned)
            .collect();

        Self { segments }
    }
}

/// Repository for field and tag data access.
#[derive(Debug, Clone)]
pub struct TaxonomyRepository {
    /// SQLx pool used by repository queries.
    pool: SqlitePool,
}

/// Error returned when deleting a field is rejected before persistence.
#[derive(Debug, thiserror::Error)]
pub enum DeleteFieldError {
    /// The requested field does not exist in the workspace.
    #[error("Field \"{field_id}\" did not match any field in workspace \"{workspace_id}\".")]
    NotFound {
        /// Workspace searched for the field.
        workspace_id: String,
        /// Field identifier supplied by the client.
        field_id: String,
    },
    /// The field is still referenced by visible notes.
    #[error("Field \"{field_id}\" is still used by {visible_note_count} visible note(s).")]
    InUse {
        /// Field identifier supplied by the client.
        field_id: String,
        /// Number of visible notes using the field.
        visible_note_count: i64,
    },
    /// SQLite operation failed while deleting the field.
    #[error("Database operation failed.")]
    Database(#[from] sqlx::Error),
}

impl TaxonomyRepository {
    /// Creates a taxonomy repository backed by a SQLite pool.
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

    /// Returns an existing field by name or creates it.
    ///
    /// # Arguments
    ///
    /// * `name` - Normalized field name.
    ///
    /// # Returns
    ///
    /// Returns the persisted field record.
    #[allow(dead_code)]
    pub async fn get_or_create_field(&self, name: &str) -> Result<FieldRecord, sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        let field =
            get_or_create_field_in_transaction(&mut transaction, DEFAULT_WORKSPACE_ID, name)
                .await?;
        transaction.commit().await?;

        Ok(field)
    }

    /// Returns an existing tag by name or creates it.
    ///
    /// # Arguments
    ///
    /// * `name` - Normalized tag name.
    ///
    /// # Returns
    ///
    /// Returns the persisted tag record.
    #[allow(dead_code)]
    pub async fn get_or_create_tag(&self, name: &str) -> Result<TagRecord, sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        let tag =
            get_or_create_tag_in_transaction(&mut transaction, DEFAULT_WORKSPACE_ID, name).await?;
        transaction.commit().await?;

        Ok(tag)
    }

    /// Lists fields ordered by name.
    ///
    /// # Arguments
    ///
    /// * `limit` - Optional maximum record count.
    ///
    /// # Returns
    ///
    /// Returns field records ordered by name.
    pub async fn list_fields(&self, limit: Option<i64>) -> Result<Vec<FieldRecord>, sqlx::Error> {
        list_fields_with_pool(&self.pool, limit).await
    }

    /// Lists tags ordered by name.
    ///
    /// # Arguments
    ///
    /// * `limit` - Optional maximum record count.
    ///
    /// # Returns
    ///
    /// Returns tag records ordered by name.
    pub async fn list_tags(&self, limit: Option<i64>) -> Result<Vec<TagRecord>, sqlx::Error> {
        list_tags_with_pool(&self.pool, limit).await
    }

    /// Deletes a field only when no visible notes still use it.
    ///
    /// # Arguments
    ///
    /// * `workspace_id` - Active workspace identifier used as the deletion scope.
    /// * `field_id` - Field identifier to delete.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after deleting the field.
    pub async fn delete_unused_field(
        &self,
        workspace_id: &str,
        field_id: &str,
    ) -> Result<(), DeleteFieldError> {
        let mut transaction = self.pool.begin().await?;
        let field = sqlx::query_as::<_, FieldRecord>(
            "SELECT id, name, created_at FROM fields WHERE workspace_id = ? AND id = ?",
        )
        .bind(workspace_id)
        .bind(field_id)
        .fetch_optional(&mut *transaction)
        .await?;

        let Some(field) = field else {
            return Err(DeleteFieldError::NotFound {
                workspace_id: workspace_id.to_string(),
                field_id: field_id.to_string(),
            });
        };

        let visible_note_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM notes \
             WHERE workspace_id = ? \
               AND field_id = ? \
               AND deleted_at IS NULL \
               AND archived_at IS NULL",
        )
        .bind(workspace_id)
        .bind(field_id)
        .fetch_one(&mut *transaction)
        .await?;

        if visible_note_count > 0 {
            return Err(DeleteFieldError::InUse {
                field_id: field_id.to_string(),
                visible_note_count,
            });
        }

        sqlx::query(
            "UPDATE notes SET field_id = NULL \
             WHERE workspace_id = ? \
               AND field_id = ? \
               AND (deleted_at IS NOT NULL OR archived_at IS NOT NULL)",
        )
        .bind(workspace_id)
        .bind(field_id)
        .execute(&mut *transaction)
        .await?;

        sqlx::query("DELETE FROM fields WHERE workspace_id = ? AND id = ?")
            .bind(workspace_id)
            .bind(field_id)
            .execute(&mut *transaction)
            .await?;

        record_sync_change_in_transaction(
            &mut transaction,
            SyncChangeInput {
                workspace_id: workspace_id.to_string(),
                entity_type: "field",
                entity_id: field.id.clone(),
                operation: "delete",
                base_revision_id: None,
                new_revision_id: None,
                payload: serde_json::json!({
                    "id": field.id,
                    "workspace_id": workspace_id,
                    "name": field.name,
                    "created_at": field.created_at
                }),
            },
        )
        .await?;

        transaction.commit().await?;

        Ok(())
    }
}

/// Returns an existing field by name or creates it in the provided transaction.
///
/// # Arguments
///
/// * `transaction` - Open SQLite transaction.
/// * `workspace_id` - Workspace identifier for the field.
/// * `name` - Normalized field name.
///
/// # Returns
///
/// Returns the persisted field record.
pub async fn get_or_create_field_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: &str,
    name: &str,
) -> Result<FieldRecord, sqlx::Error> {
    if let Some(field) = sqlx::query_as::<_, FieldRecord>(
        "SELECT id, name, created_at FROM fields WHERE workspace_id = ? AND name = ?",
    )
    .bind(workspace_id)
    .bind(name)
    .fetch_optional(&mut **transaction)
    .await?
    {
        return Ok(field);
    }

    let id = new_id();
    sqlx::query(
        "INSERT INTO fields (id, workspace_id, name, created_at) VALUES (?, ?, ?, unixepoch())",
    )
    .bind(&id)
    .bind(workspace_id)
    .bind(name)
    .execute(&mut **transaction)
    .await?;

    let field = sqlx::query_as::<_, FieldRecord>(
        "SELECT id, name, created_at FROM fields WHERE workspace_id = ? AND id = ?",
    )
    .bind(workspace_id)
    .bind(&id)
    .fetch_one(&mut **transaction)
    .await?;

    record_sync_change_in_transaction(
        transaction,
        SyncChangeInput {
            workspace_id: workspace_id.to_string(),
            entity_type: "field",
            entity_id: field.id.clone(),
            operation: "insert",
            base_revision_id: None,
            new_revision_id: None,
            payload: serde_json::json!({
                "id": field.id,
                "workspace_id": workspace_id,
                "name": field.name,
                "created_at": field.created_at
            }),
        },
    )
    .await?;

    Ok(field)
}

/// Returns an existing tag by name or creates it in the provided transaction.
///
/// # Arguments
///
/// * `transaction` - Open SQLite transaction.
/// * `workspace_id` - Workspace identifier for the tag path.
/// * `name` - Normalized tag name.
///
/// # Returns
///
/// Returns the persisted tag record.
pub async fn get_or_create_tag_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: &str,
    name: &str,
) -> Result<TagRecord, sqlx::Error> {
    let tag_path = TagPath::parse(name);
    let mut parent_tag_id: Option<String> = None;
    let mut current_path = String::new();
    let mut leaf_tag: Option<TagRecord> = None;

    for (depth, segment) in tag_path.segments.iter().enumerate() {
        if current_path.is_empty() {
            current_path.push_str(segment);
        } else {
            current_path.push('/');
            current_path.push_str(segment);
        }

        let existing = sqlx::query_as::<_, TagRecord>(
            "SELECT id, name, parent_tag_id, path, depth, created_at FROM tags WHERE workspace_id = ? AND path = ?",
        )
        .bind(workspace_id)
        .bind(&current_path)
        .fetch_optional(&mut **transaction)
        .await?;

        let tag = match existing {
            Some(tag) => tag,
            None => {
                let id = new_id();
                sqlx::query(
                    "INSERT INTO tags (id, workspace_id, name, parent_tag_id, path, depth, created_at) \
                     VALUES (?, ?, ?, ?, ?, ?, unixepoch())",
                )
                .bind(&id)
                .bind(workspace_id)
                .bind(segment)
                .bind(parent_tag_id.as_deref())
                .bind(&current_path)
                .bind(depth as i64)
                .execute(&mut **transaction)
                .await?;

                let tag = sqlx::query_as::<_, TagRecord>(
                    "SELECT id, name, parent_tag_id, path, depth, created_at FROM tags WHERE workspace_id = ? AND id = ?",
                )
                .bind(workspace_id)
                .bind(&id)
                .fetch_one(&mut **transaction)
                .await?;

                record_sync_change_in_transaction(
                    transaction,
                    SyncChangeInput {
                        workspace_id: workspace_id.to_string(),
                        entity_type: "tag",
                        entity_id: tag.id.clone(),
                        operation: "insert",
                        base_revision_id: None,
                        new_revision_id: None,
                        payload: serde_json::json!({
                            "id": tag.id,
                            "workspace_id": workspace_id,
                            "name": segment,
                            "parent_tag_id": parent_tag_id.as_deref(),
                            "path": tag.path,
                            "depth": depth as i64,
                            "created_at": tag.created_at
                        }),
                    },
                )
                .await?;

                tag
            }
        };

        parent_tag_id = Some(tag.id.clone());
        leaf_tag = Some(tag);
    }

    leaf_tag.ok_or(sqlx::Error::RowNotFound)
}

/// Lists field records using a pool.
///
/// # Arguments
///
/// * `pool` - SQLite pool.
/// * `limit` - Optional maximum record count.
///
/// # Returns
///
/// Returns field records ordered by name.
async fn list_fields_with_pool(
    pool: &SqlitePool,
    limit: Option<i64>,
) -> Result<Vec<FieldRecord>, sqlx::Error> {
    let limit = limit.unwrap_or(i64::MAX);

    sqlx::query_as::<_, FieldRecord>(
        "SELECT id, name, created_at FROM fields WHERE workspace_id = ? ORDER BY name ASC LIMIT ?",
    )
    .bind(DEFAULT_WORKSPACE_ID)
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// Lists tag records using a pool.
///
/// # Arguments
///
/// * `pool` - SQLite pool.
/// * `limit` - Optional maximum record count.
///
/// # Returns
///
/// Returns tag records ordered by name.
async fn list_tags_with_pool(
    pool: &SqlitePool,
    limit: Option<i64>,
) -> Result<Vec<TagRecord>, sqlx::Error> {
    let limit = limit.unwrap_or(i64::MAX);

    sqlx::query_as::<_, TagRecord>(
        "SELECT id, name, parent_tag_id, path, depth, created_at FROM tags WHERE workspace_id = ? ORDER BY path ASC LIMIT ?",
    )
    .bind(DEFAULT_WORKSPACE_ID)
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// Creates a new 32-character lowercase hexadecimal identifier.
///
/// # Returns
///
/// Returns a random UUID without hyphens.
pub fn new_id() -> String {
    Uuid::new_v4().simple().to_string()
}
