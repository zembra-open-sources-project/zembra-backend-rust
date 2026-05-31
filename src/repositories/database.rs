use std::path::Path;
use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Executor, SqlitePool};

use crate::repositories::taxonomy::DEFAULT_WORKSPACE_ID;

const INITIAL_SCHEMA_MIGRATION: &str =
    include_str!("../../vendor/zembra-schema/migrations/001_initial_schema.sql");
const NOTE_ROLE_MIGRATION: &str =
    include_str!("../../vendor/zembra-schema/migrations/002_add_note_role.sql");
const BIDIRECTIONAL_SYNC_MIGRATION: &str =
    include_str!("../../vendor/zembra-schema/migrations/003_add_bidirectional_sync.sql");
const HIERARCHICAL_TAGS_MIGRATION: &str =
    include_str!("../../vendor/zembra-schema/migrations/004_add_hierarchical_tags.sql");

/// SQLite database handle shared by application services.
#[derive(Debug, Clone)]
pub struct Database {
    /// SQLx connection pool used by request handlers and repositories.
    pub pool: SqlitePool,
}

impl Database {
    /// Opens the SQLite database, applies shared schema migrations, and returns the handle.
    ///
    /// # Arguments
    ///
    /// * `database_url` - SQLx-compatible SQLite connection URL.
    ///
    /// # Returns
    ///
    /// Returns a database handle when connection and migration succeed.
    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        ensure_parent_directory(database_url)?;

        let options = SqliteConnectOptions::from_str(database_url)?
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        let database = Self { pool };
        database.migrate().await?;

        Ok(database)
    }

    /// Applies v0.4.0 shared schema migrations to the database.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` when required tables and columns are available.
    pub async fn migrate(&self) -> Result<(), sqlx::Error> {
        if !table_exists(&self.pool, "schema_migrations").await? {
            if table_exists(&self.pool, "notes").await? {
                bootstrap_schema_migrations(&self.pool).await?;
            } else {
                self.pool.execute(INITIAL_SCHEMA_MIGRATION).await?;
            }
        }

        if !schema_version_exists(&self.pool, "0.2.0").await? {
            if column_exists(&self.pool, "notes", "role").await? {
                record_schema_version(&self.pool, "0.2.0").await?;
            } else {
                self.pool.execute(NOTE_ROLE_MIGRATION).await?;
            }
        }

        if !schema_version_exists(&self.pool, "0.3.0").await? {
            if table_exists(&self.pool, "workspaces").await?
                && table_exists(&self.pool, "sync_changes").await?
                && column_exists(&self.pool, "notes", "workspace_id").await?
            {
                record_schema_version(&self.pool, "0.3.0").await?;
            } else {
                self.pool.execute(BIDIRECTIONAL_SYNC_MIGRATION).await?;
            }
        }

        if !schema_version_exists(&self.pool, "0.4.0").await? {
            if column_exists(&self.pool, "tags", "parent_tag_id").await?
                && column_exists(&self.pool, "tags", "path").await?
                && column_exists(&self.pool, "tags", "depth").await?
            {
                record_schema_version(&self.pool, "0.4.0").await?;
            } else {
                self.pool.execute(HIERARCHICAL_TAGS_MIGRATION).await?;
            }
        }

        Ok(())
    }

    /// Checks whether the database can answer a simple query.
    ///
    /// # Returns
    ///
    /// Returns `true` when SQLite responds successfully.
    pub async fn is_initialized(&self) -> bool {
        sqlx::query_scalar::<_, i64>("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .is_ok()
    }
}

/// Creates migration metadata for databases that already contain shared tables.
///
/// # Arguments
///
/// * `executor` - SQLx executor used to create and seed `schema_migrations`.
///
/// # Returns
///
/// Returns `Ok(())` when the migration metadata reflects the existing schema.
async fn bootstrap_schema_migrations<'e, E>(executor: E) -> Result<(), sqlx::Error>
where
    E: Executor<'e, Database = sqlx::Sqlite> + Copy,
{
    executor
        .execute(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version TEXT PRIMARY KEY NOT NULL,
                applied_at INTEGER NOT NULL CHECK (applied_at >= 0)
            )",
        )
        .await?;
    record_schema_version(executor, "0.1.0").await?;

    if column_exists(executor, "notes", "role").await? {
        record_schema_version(executor, "0.2.0").await?;
    }

    if table_exists(executor, "workspaces").await?
        && table_exists(executor, "sync_changes").await?
        && column_exists(executor, "notes", "workspace_id").await?
    {
        ensure_default_workspace(executor).await?;
        record_schema_version(executor, "0.3.0").await?;
    }

    if column_exists(executor, "tags", "parent_tag_id").await?
        && column_exists(executor, "tags", "path").await?
        && column_exists(executor, "tags", "depth").await?
    {
        record_schema_version(executor, "0.4.0").await?;
    }

    Ok(())
}

/// Ensures the shared default workspace exists for workspace-scoped queries.
///
/// # Arguments
///
/// * `executor` - SQLx executor used to upsert the default workspace.
///
/// # Returns
///
/// Returns `Ok(())` when the default workspace is present.
async fn ensure_default_workspace<'e, E>(executor: E) -> Result<(), sqlx::Error>
where
    E: Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query(
        "INSERT OR IGNORE INTO workspaces (id, workspace_name, created_at, updated_at)
        VALUES (?, NULL, unixepoch(), unixepoch())",
    )
    .bind(DEFAULT_WORKSPACE_ID)
    .execute(executor)
    .await?;

    Ok(())
}

/// Records a shared schema migration version when it is not already present.
///
/// # Arguments
///
/// * `executor` - SQLx executor used to write `schema_migrations`.
/// * `version` - Schema version string to insert.
///
/// # Returns
///
/// Returns `Ok(())` when the version is present after the call.
async fn record_schema_version<'e, E>(executor: E, version: &str) -> Result<(), sqlx::Error>
where
    E: Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query(
        "INSERT OR IGNORE INTO schema_migrations (version, applied_at)
        VALUES (?, unixepoch())",
    )
    .bind(version)
    .execute(executor)
    .await?;

    Ok(())
}

/// Checks whether a shared schema migration version has already been applied.
///
/// # Arguments
///
/// * `executor` - SQLx executor used to query `schema_migrations`.
/// * `version` - Schema version string to look up.
///
/// # Returns
///
/// Returns `true` when the version exists in `schema_migrations`.
async fn schema_version_exists<'e, E>(executor: E, version: &str) -> Result<bool, sqlx::Error>
where
    E: Executor<'e, Database = sqlx::Sqlite>,
{
    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?)",
    )
    .bind(version)
    .fetch_one(executor)
    .await?;

    Ok(exists == 1)
}

/// Checks whether a SQLite table exists in the current database.
///
/// # Arguments
///
/// * `executor` - SQLx executor used to query `sqlite_master`.
/// * `table_name` - Table name to look up.
///
/// # Returns
///
/// Returns `true` when the table exists.
async fn table_exists<'e, E>(executor: E, table_name: &str) -> Result<bool, sqlx::Error>
where
    E: Executor<'e, Database = sqlx::Sqlite>,
{
    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?)",
    )
    .bind(table_name)
    .fetch_one(executor)
    .await?;

    Ok(exists == 1)
}

/// Checks whether a column exists in a SQLite table.
///
/// # Arguments
///
/// * `executor` - SQLx executor used to inspect SQLite schema metadata.
/// * `table_name` - Table name to inspect.
/// * `column_name` - Column name to look up.
///
/// # Returns
///
/// Returns `true` when the table contains the requested column.
async fn column_exists<'e, E>(
    executor: E,
    table_name: &str,
    column_name: &str,
) -> Result<bool, sqlx::Error>
where
    E: Executor<'e, Database = sqlx::Sqlite>,
{
    let query = format!(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('{}') WHERE name = ?)",
        table_name.replace('\'', "''")
    );
    let exists = sqlx::query_scalar::<_, i64>(&query)
        .bind(column_name)
        .fetch_one(executor)
        .await?;

    Ok(exists == 1)
}

/// Creates the parent directory for filesystem SQLite URLs when needed.
///
/// # Arguments
///
/// * `database_url` - SQLx-compatible SQLite connection URL.
///
/// # Returns
///
/// Returns `Ok(())` when no directory is needed or the directory exists.
fn ensure_parent_directory(database_url: &str) -> Result<(), std::io::Error> {
    let Some(path) = database_url.strip_prefix("sqlite://") else {
        return Ok(());
    };

    if path == ":memory:" || path.starts_with("file:") {
        return Ok(());
    }

    let path = Path::new(path);
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Database, schema_version_exists};

    /// Creates a legacy in-memory database missing migration metadata.
    ///
    /// # Returns
    ///
    /// Returns a database handle with v0.4.0 tables and no `schema_migrations`.
    async fn legacy_database_without_migration_metadata() -> Database {
        let database = Database::connect("sqlite://:memory:").await.unwrap();
        sqlx::query("DROP TABLE schema_migrations")
            .execute(&database.pool)
            .await
            .unwrap();

        database
    }

    #[tokio::test]
    async fn migrate_records_existing_legacy_schema_versions() {
        let database = legacy_database_without_migration_metadata().await;

        database.migrate().await.unwrap();

        assert!(
            schema_version_exists(&database.pool, "0.1.0")
                .await
                .unwrap()
        );
        assert!(
            schema_version_exists(&database.pool, "0.2.0")
                .await
                .unwrap()
        );
        assert!(
            schema_version_exists(&database.pool, "0.3.0")
                .await
                .unwrap()
        );
        assert!(
            schema_version_exists(&database.pool, "0.4.0")
                .await
                .unwrap()
        );
    }
}
