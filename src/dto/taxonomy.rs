use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

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

/// Response body for listing fields.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ListFieldsResponse {
    /// Field records.
    pub fields: Vec<FieldRecord>,
    /// Field names in the same order as `fields`.
    pub names: Vec<String>,
}

/// Response body for listing tags.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ListTagsResponse {
    /// Tag records.
    pub tags: Vec<TagRecord>,
    /// Tag names in the same order as `tags`.
    pub names: Vec<String>,
}
