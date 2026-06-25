/// Legacy fixed workspace identifier used by earlier backend versions.
pub const LEGACY_FIXED_WORKSPACE_ID: &str = "00000000-0000-4000-8000-000000000300";

/// Workspace summary row returned by repository queries.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct WorkspaceSummaryRow {
    /// Full workspace identifier.
    pub workspace_id: String,
    /// Human-readable workspace name, or `None` when the schema row has no name.
    pub workspace_name: Option<String>,
    /// Count of visible notes in this workspace.
    pub visible_note_count: i64,
    /// Creation timestamp of the latest visible note, or `None` for empty workspaces.
    pub latest_note_created_at: Option<i64>,
}

/// Repository for workspace metadata and summary queries.
#[derive(Debug, Clone)]
pub struct WorkspacesRepository {
    /// SQLx pool used by repository queries.
    pool: sqlx::SqlitePool,
}

impl WorkspacesRepository {
    /// Creates a workspace repository backed by a SQLite pool.
    ///
    /// # Arguments
    ///
    /// * `pool` - Shared SQLite connection pool.
    ///
    /// # Returns
    ///
    /// Returns a repository value.
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self { pool }
    }

    /// Verifies that a workspace exists and can be used for CRUD requests.
    ///
    /// # Arguments
    ///
    /// * `workspace_id` - Full workspace UUID supplied by the client.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` when the workspace exists and is neither archived nor deleted.
    pub async fn ensure_active(&self, workspace_id: &str) -> Result<(), crate::error::ApiError> {
        if uuid::Uuid::parse_str(workspace_id).is_err() {
            return Err(workspace_not_found(workspace_id));
        }

        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(
                SELECT 1 FROM workspaces
                WHERE id = ?
                  AND archived_at IS NULL
                  AND deleted_at IS NULL
            )",
        )
        .bind(workspace_id)
        .fetch_one(&self.pool)
        .await?;

        if exists == 1 {
            Ok(())
        } else {
            Err(workspace_not_found(workspace_id))
        }
    }

    /// Lists workspace summaries ordered by visible note activity.
    ///
    /// # Returns
    ///
    /// Returns all workspaces with visible note counts and latest visible note timestamps.
    pub async fn list_summaries(&self) -> Result<Vec<WorkspaceSummaryRow>, sqlx::Error> {
        sqlx::query_as::<_, WorkspaceSummaryRow>(
            "SELECT workspaces.id AS workspace_id,
                    workspaces.workspace_name AS workspace_name,
                    COUNT(notes.id) AS visible_note_count,
                    MAX(notes.created_at) AS latest_note_created_at
             FROM workspaces
             LEFT JOIN notes
               ON notes.workspace_id = workspaces.id
              AND notes.deleted_at IS NULL
              AND notes.archived_at IS NULL
             GROUP BY workspaces.id, workspaces.workspace_name
             ORDER BY visible_note_count DESC,
                      latest_note_created_at IS NULL ASC,
                      latest_note_created_at DESC,
                      workspaces.id ASC",
        )
        .fetch_all(&self.pool)
        .await
    }
}

/// Builds the public not-found error for invalid workspace request scopes.
///
/// # Arguments
///
/// * `workspace_id` - Workspace identifier supplied by the client.
///
/// # Returns
///
/// Returns an API not-found error.
pub fn workspace_not_found(workspace_id: &str) -> crate::error::ApiError {
    crate::error::ApiError::RecordNotFound(format!(
        "Workspace \"{workspace_id}\" did not match any active workspace."
    ))
}

/// Returns the display short hash for a workspace identifier.
///
/// # Arguments
///
/// * `workspace_id` - Full workspace identifier.
///
/// # Returns
///
/// Returns the first 8 characters after removing UUID hyphens.
pub fn workspace_short_hash(workspace_id: &str) -> String {
    workspace_id.replace('-', "").chars().take(8).collect()
}

/// Validates and normalizes a workspace name entered by a user.
///
/// # Arguments
///
/// * `workspace_name` - Raw workspace name input.
///
/// # Returns
///
/// Returns a trimmed workspace name when it is non-empty and contains no whitespace.
pub fn validate_workspace_name(workspace_name: &str) -> Option<String> {
    let trimmed = workspace_name.trim();
    if trimmed.is_empty() || workspace_name.chars().any(char::is_whitespace) {
        return None;
    }

    Some(trimmed.to_string())
}
