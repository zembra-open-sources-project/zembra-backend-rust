use std::path::Path;
use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Executor, SqlitePool};

const INITIAL_SCHEMA_MIGRATION: &str =
    include_str!("../../vendor/zembra-schema/migrations/001_initial_schema.sql");
const NOTE_ROLE_MIGRATION: &str =
    include_str!("../../vendor/zembra-schema/migrations/002_add_note_role.sql");

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

    /// Applies v0.2.0 shared schema migrations to the database.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` when required tables and columns are available.
    pub async fn migrate(&self) -> Result<(), sqlx::Error> {
        if !table_exists(&self.pool, "schema_migrations").await? {
            self.pool.execute(INITIAL_SCHEMA_MIGRATION).await?;
        }

        if !schema_version_exists(&self.pool, "0.2.0").await? {
            self.pool.execute(NOTE_ROLE_MIGRATION).await?;
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
