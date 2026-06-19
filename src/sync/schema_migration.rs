use crate::sync::schema_contract::TARGET_SCHEMA_CONTRACT_VERSION;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};

const POSTGRES_INITIAL_SCHEMA: &str =
    include_str!("../../vendor/zembra-schema/postgres/001_initial_schema.sql");

/// Applies existing zembra-schema Postgres contract migrations.
///
/// # Arguments
///
/// * `supabase_url` - Supabase project URL.
/// * `database_password` - Supabase database password.
///
/// # Returns
///
/// Returns `Ok(())` when the remote contract migration succeeds.
pub async fn apply_remote_schema_contract(
    supabase_url: &str,
    database_password: &str,
) -> Result<(), SchemaMigrationError> {
    let database_url = remote_database_url(supabase_url, database_password)?;
    let pool = sqlx::PgPool::connect(&database_url).await?;
    sqlx::raw_sql(POSTGRES_INITIAL_SCHEMA)
        .execute(&pool)
        .await?;
    pool.close().await;
    Ok(())
}

/// Builds the Supabase Postgres connection URL used for schema migrations.
///
/// # Arguments
///
/// * `supabase_url` - Supabase project URL.
/// * `database_password` - Supabase database password.
///
/// # Returns
///
/// Returns a Postgres connection URL for the project database.
pub fn remote_database_url(
    supabase_url: &str,
    database_password: &str,
) -> Result<String, SchemaMigrationError> {
    let project_ref = supabase_project_ref(supabase_url)?;
    let password = utf8_percent_encode(database_password.trim(), NON_ALPHANUMERIC).to_string();
    Ok(format!(
        "postgresql://postgres:{password}@db.{project_ref}.supabase.co:5432/postgres"
    ))
}

/// Extracts the Supabase project ref from a project URL.
///
/// # Arguments
///
/// * `supabase_url` - Supabase project URL.
///
/// # Returns
///
/// Returns the project ref from `<project-ref>.supabase.co`.
pub fn supabase_project_ref(supabase_url: &str) -> Result<String, SchemaMigrationError> {
    let trimmed = supabase_url.trim().trim_end_matches('/');
    let host = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed)
        .split('/')
        .next()
        .unwrap_or_default();
    let project_ref = host.strip_suffix(".supabase.co").unwrap_or_default();
    if project_ref.is_empty() || project_ref.contains('.') {
        return Err(SchemaMigrationError::InvalidSupabaseUrl(
            supabase_url.to_string(),
        ));
    }
    Ok(project_ref.to_string())
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
    /// Supabase project URL cannot be converted to a database host.
    #[error("invalid Supabase project URL for remote schema migration: {0}")]
    InvalidSupabaseUrl(String),
    /// Postgres connection or query failed.
    #[error("remote schema migration failed: {0}")]
    Database(#[from] sqlx::Error),
}

#[cfg(test)]
mod tests {
    use super::{
        remote_database_url, supabase_project_ref, target_remote_schema_sql,
        target_remote_schema_version,
    };

    #[test]
    fn target_remote_schema_comes_from_zembra_schema_contract() {
        let sql = target_remote_schema_sql();

        assert!(sql.contains("CREATE TABLE schema_migrations"));
        assert!(sql.contains("CREATE TABLE note_links"));
        assert!(sql.contains("INSERT INTO schema_migrations"));
        assert_eq!(target_remote_schema_version(), "0.5.0");
    }

    #[test]
    fn supabase_project_ref_reads_project_url() {
        assert_eq!(
            supabase_project_ref("https://xdeukuklunlzycltmkgg.supabase.co").unwrap(),
            "xdeukuklunlzycltmkgg"
        );
    }

    #[test]
    fn remote_database_url_uses_project_ref_and_encoded_password() {
        let url =
            remote_database_url("https://xdeukuklunlzycltmkgg.supabase.co", "pass word/@").unwrap();

        assert_eq!(
            url,
            "postgresql://postgres:pass%20word%2F%40@db.xdeukuklunlzycltmkgg.supabase.co:5432/postgres"
        );
    }

    #[test]
    fn supabase_project_ref_rejects_non_project_url() {
        assert!(supabase_project_ref("https://example.com").is_err());
    }
}
