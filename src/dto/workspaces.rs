use serde::Serialize;
use utoipa::ToSchema;

/// One workspace summary returned by the workspace list API.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct WorkspaceSummary {
    /// Full workspace identifier used as the real API identity.
    pub workspace_id: String,
    /// Human-readable workspace name, or `null` when the workspace row has no name.
    pub workspace_name: Option<String>,
    /// Eight-character display hash derived from the workspace UUID.
    pub short_hash: String,
    /// Count of visible notes in this workspace.
    pub visible_note_count: i64,
    /// Creation timestamp of the latest visible note, or `null` for empty workspaces.
    pub latest_note_created_at: Option<i64>,
}

/// Response body for listing workspaces.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ListWorkspacesResponse {
    /// Ordered workspace summaries.
    pub workspaces: Vec<WorkspaceSummary>,
}
