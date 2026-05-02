use serde::Serialize;
use sqlx::FromRow;
use utoipa::ToSchema;

/// Database record for a tag.
#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
pub struct TagRecord {
    /// Stable tag identifier.
    pub id: String,
    /// Human-readable tag name.
    pub name: String,
    /// Unix timestamp for tag creation.
    pub created_at: i64,
}
