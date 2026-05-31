use serde::Serialize;
use sqlx::FromRow;
use utoipa::ToSchema;

/// Database record for a tag.
#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
pub struct TagRecord {
    /// Stable tag identifier.
    pub id: String,
    /// Tag name within its current hierarchy level.
    pub name: String,
    /// Optional parent tag identifier; root tags have no parent.
    pub parent_tag_id: Option<String>,
    /// Full slash-delimited tag path.
    pub path: String,
    /// Tag depth from the root tag, starting at zero.
    pub depth: i64,
    /// Unix timestamp for tag creation.
    pub created_at: i64,
}
