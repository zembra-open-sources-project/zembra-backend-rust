/// Legacy fixed workspace identifier used by earlier backend versions.
pub const LEGACY_FIXED_WORKSPACE_ID: &str = "00000000-0000-4000-8000-000000000300";

/// Workspace summary row returned by repository queries.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct WorkspaceSummaryRow {
    /// Full workspace identifier.
    pub workspace_id: String,
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

    /// Lists workspace summaries ordered by visible note activity.
    ///
    /// # Returns
    ///
    /// Returns all workspaces with visible note counts and latest visible note timestamps.
    pub async fn list_summaries(&self) -> Result<Vec<WorkspaceSummaryRow>, sqlx::Error> {
        sqlx::query_as::<_, WorkspaceSummaryRow>(
            "SELECT workspaces.id AS workspace_id,
                    COUNT(notes.id) AS visible_note_count,
                    MAX(notes.created_at) AS latest_note_created_at
             FROM workspaces
             LEFT JOIN notes
               ON notes.workspace_id = workspaces.id
              AND notes.deleted_at IS NULL
              AND notes.archived_at IS NULL
             GROUP BY workspaces.id
             ORDER BY visible_note_count DESC,
                      latest_note_created_at IS NULL ASC,
                      latest_note_created_at DESC,
                      workspaces.id ASC",
        )
        .fetch_all(&self.pool)
        .await
    }
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
