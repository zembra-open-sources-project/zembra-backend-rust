use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::models::field::FieldRecord;
use crate::models::tag::TagRecord;

/// Query parameters used by fields and tags list endpoints.
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ListTaxonomyQuery {
    /// Maximum number of records returned when `all` is false.
    pub limit: Option<i64>,
    /// Whether to return all records.
    pub all: Option<bool>,
}

impl ListTaxonomyQuery {
    /// Resolves the SQL limit for this query.
    ///
    /// # Returns
    ///
    /// Returns `None` when all records should be returned.
    pub fn resolved_limit(&self) -> Option<i64> {
        if self.all.unwrap_or(false) {
            None
        } else {
            Some(self.limit.unwrap_or(5))
        }
    }
}

/// Response body for listing fields.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ListFieldsResponse {
    /// Field records.
    pub fields: Vec<FieldRecord>,
    /// Field names in the same order as `fields`.
    pub names: Vec<String>,
}

/// Request body for deleting an unused field.
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct DeleteFieldRequest {
    /// Workspace UUID used as the deletion scope.
    #[serde(default)]
    #[validate(length(min = 1))]
    pub workspace_id: String,
    /// Field identifier to delete.
    #[serde(default)]
    #[validate(length(min = 1))]
    pub field_id: String,
}

/// Response body for a successful field deletion.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DeleteFieldResponse {
    /// Identifier of the deleted field.
    pub field_id: String,
    /// Whether the field was deleted by this request.
    pub deleted: bool,
}

/// Response body for listing tags.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ListTagsResponse {
    /// Tag records.
    pub tags: Vec<TagRecord>,
    /// Tag names in the same order as `tags`.
    pub names: Vec<String>,
}
