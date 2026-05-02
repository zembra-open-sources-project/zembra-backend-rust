use sqlx::{Sqlite, SqlitePool, Transaction};
use uuid::Uuid;

use crate::models::field::FieldRecord;
use crate::models::tag::TagRecord;

/// Repository for field and tag data access.
#[derive(Debug, Clone)]
pub struct TaxonomyRepository {
    /// SQLx pool used by repository queries.
    pool: SqlitePool,
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
    pub async fn get_or_create_field(&self, name: &str) -> Result<FieldRecord, sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        let field = get_or_create_field_in_transaction(&mut transaction, name).await?;
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
    pub async fn get_or_create_tag(&self, name: &str) -> Result<TagRecord, sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        let tag = get_or_create_tag_in_transaction(&mut transaction, name).await?;
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
}

/// Returns an existing field by name or creates it in the provided transaction.
///
/// # Arguments
///
/// * `transaction` - Open SQLite transaction.
/// * `name` - Normalized field name.
///
/// # Returns
///
/// Returns the persisted field record.
pub async fn get_or_create_field_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    name: &str,
) -> Result<FieldRecord, sqlx::Error> {
    if let Some(field) =
        sqlx::query_as::<_, FieldRecord>("SELECT id, name, created_at FROM fields WHERE name = ?")
            .bind(name)
            .fetch_optional(&mut **transaction)
            .await?
    {
        return Ok(field);
    }

    let id = new_id();
    sqlx::query("INSERT INTO fields (id, name, created_at) VALUES (?, ?, unixepoch())")
        .bind(&id)
        .bind(name)
        .execute(&mut **transaction)
        .await?;

    sqlx::query_as::<_, FieldRecord>("SELECT id, name, created_at FROM fields WHERE id = ?")
        .bind(id)
        .fetch_one(&mut **transaction)
        .await
}

/// Returns an existing tag by name or creates it in the provided transaction.
///
/// # Arguments
///
/// * `transaction` - Open SQLite transaction.
/// * `name` - Normalized tag name.
///
/// # Returns
///
/// Returns the persisted tag record.
pub async fn get_or_create_tag_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    name: &str,
) -> Result<TagRecord, sqlx::Error> {
    if let Some(tag) =
        sqlx::query_as::<_, TagRecord>("SELECT id, name, created_at FROM tags WHERE name = ?")
            .bind(name)
            .fetch_optional(&mut **transaction)
            .await?
    {
        return Ok(tag);
    }

    let id = new_id();
    sqlx::query("INSERT INTO tags (id, name, created_at) VALUES (?, ?, unixepoch())")
        .bind(&id)
        .bind(name)
        .execute(&mut **transaction)
        .await?;

    sqlx::query_as::<_, TagRecord>("SELECT id, name, created_at FROM tags WHERE id = ?")
        .bind(id)
        .fetch_one(&mut **transaction)
        .await
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
        "SELECT id, name, created_at FROM fields ORDER BY name ASC LIMIT ?",
    )
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
        "SELECT id, name, created_at FROM tags ORDER BY name ASC LIMIT ?",
    )
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
