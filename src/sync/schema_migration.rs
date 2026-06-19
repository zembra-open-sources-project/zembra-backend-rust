use crate::sync::schema_contract::TARGET_SCHEMA_CONTRACT_VERSION;

const POSTGRES_INITIAL_SCHEMA: &str =
    include_str!("../../vendor/zembra-schema/postgres/001_initial_schema.sql");

/// Applies existing zembra-schema Postgres contract migrations.
///
/// # Arguments
///
/// * `database_url` - Administrator Postgres connection URL.
///
/// # Returns
///
/// Returns `Ok(())` when the remote contract migration succeeds.
pub async fn apply_remote_schema_contract(database_url: &str) -> Result<(), SchemaMigrationError> {
    let pool = sqlx::PgPool::connect(database_url).await?;
    sqlx::raw_sql(POSTGRES_INITIAL_SCHEMA)
        .execute(&pool)
        .await?;
    pool.close().await;
    Ok(())
}

/// Returns the target remote schema contract migration SQL.
///
/// # Returns
///
/// Returns the SQL from `vendor/zembra-schema/postgres`.
pub fn target_remote_schema_sql() -> &'static str {
    POSTGRES_INITIAL_SCHEMA
}

/// Returns the migration target contract version.
///
/// # Returns
///
/// Returns the target schema contract version.
pub fn target_remote_schema_version() -> &'static str {
    TARGET_SCHEMA_CONTRACT_VERSION
}

/// Error returned by remote schema contract migration.
#[derive(Debug, thiserror::Error)]
pub enum SchemaMigrationError {
    /// Postgres connection or query failed.
    #[error("remote schema migration failed: {0}")]
    Database(#[from] sqlx::Error),
}

#[cfg(test)]
mod tests {
    use super::{target_remote_schema_sql, target_remote_schema_version};

    #[test]
    fn target_remote_schema_comes_from_zembra_schema_contract() {
        let sql = target_remote_schema_sql();

        assert!(sql.contains("CREATE TABLE schema_migrations"));
        assert!(sql.contains("CREATE TABLE note_links"));
        assert!(sql.contains("INSERT INTO schema_migrations"));
        assert_eq!(target_remote_schema_version(), "0.5.0");
    }
}
