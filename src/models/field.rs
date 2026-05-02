use serde::Serialize;
use sqlx::FromRow;
use utoipa::ToSchema;

/// Database record for a note field.
#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
pub struct FieldRecord {
    /// Stable field identifier.
    pub id: String,
    /// Human-readable field name.
    pub name: String,
    /// Unix timestamp for field creation.
    pub created_at: i64,
}
