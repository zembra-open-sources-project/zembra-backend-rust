use serde_json::Value;

/// Payload for remote field insertion.
pub(super) struct FieldPayload<'a> {
    /// Field ID.
    pub id: &'a str,
    /// Field name.
    pub name: &'a str,
    /// Field creation timestamp.
    pub created_at: i64,
}

/// Payload for remote tag insertion.
pub(super) struct TagPayload<'a> {
    /// Tag ID.
    pub id: &'a str,
    /// Tag name.
    pub name: &'a str,
    /// Tag creation timestamp.
    pub created_at: i64,
}

/// Payload for remote note upsert operations.
pub(super) struct NotePayload<'a> {
    /// Note ID.
    pub id: &'a str,
    /// Note content.
    pub content: &'a str,
    /// Note role.
    pub role: &'a str,
    /// Optional field ID.
    pub field_id: Option<&'a str>,
    /// Note creation timestamp.
    pub created_at: i64,
    /// Note update timestamp.
    pub updated_at: i64,
    /// Optional archive timestamp.
    pub archived_at: Option<i64>,
    /// Optional deletion timestamp.
    pub deleted_at: Option<i64>,
    /// Optional current revision ID.
    pub current_revision_id: Option<&'a str>,
}

/// Payload for remote note revision insertion.
pub(super) struct NoteRevisionPayload<'a> {
    /// Revision ID.
    pub id: &'a str,
    /// Parent note ID.
    pub note_id: &'a str,
    /// Revision content.
    pub content: &'a str,
    /// Optional title.
    pub title: Option<&'a str>,
    /// Optional device ID.
    pub device_id: Option<&'a str>,
    /// Revision creation timestamp.
    pub created_at: i64,
    /// Optional base revision ID.
    pub base_revision_id: Option<&'a str>,
}

/// Payload for remote note tag relation changes.
pub(super) struct NoteTagPayload<'a> {
    /// Note ID.
    pub note_id: &'a str,
    /// Tag ID.
    pub tag_id: &'a str,
    /// Optional relation creation timestamp.
    pub created_at: Option<i64>,
}

/// Payload for remote note link attachment.
pub(super) struct NoteLinkAttachPayload<'a> {
    /// Link ID.
    pub id: &'a str,
    /// Source note ID.
    pub source_note_id: &'a str,
    /// Target note ID.
    pub target_note_id: &'a str,
    /// Optional anchor text.
    pub anchor_text: Option<&'a str>,
    /// Optional link position.
    pub position: Option<i64>,
    /// Optional relation creation timestamp.
    pub created_at: Option<i64>,
}

/// Payload for remote note link detachment.
pub(super) struct NoteLinkDetachPayload<'a> {
    /// Link ID.
    pub id: &'a str,
}

impl<'a> TryFrom<&'a Value> for FieldPayload<'a> {
    type Error = String;

    /// Parses a field payload from JSON.
    fn try_from(payload: &'a Value) -> Result<Self, Self::Error> {
        Ok(Self {
            id: required_text(payload, "id")?,
            name: required_text(payload, "name")?,
            created_at: required_i64(payload, "created_at")?,
        })
    }
}

impl<'a> TryFrom<&'a Value> for TagPayload<'a> {
    type Error = String;

    /// Parses a tag payload from JSON.
    fn try_from(payload: &'a Value) -> Result<Self, Self::Error> {
        Ok(Self {
            id: required_text(payload, "id")?,
            name: required_text(payload, "name")?,
            created_at: required_i64(payload, "created_at")?,
        })
    }
}

impl<'a> TryFrom<&'a Value> for NotePayload<'a> {
    type Error = String;

    /// Parses a note payload from JSON.
    fn try_from(payload: &'a Value) -> Result<Self, Self::Error> {
        Ok(Self {
            id: required_text(payload, "id")?,
            content: required_text(payload, "content")?,
            role: required_text(payload, "role")?,
            field_id: optional_text(payload, "field_id"),
            created_at: required_i64(payload, "created_at")?,
            updated_at: required_i64(payload, "updated_at")?,
            archived_at: optional_i64(payload, "archived_at"),
            deleted_at: optional_i64(payload, "deleted_at"),
            current_revision_id: optional_text(payload, "current_revision_id"),
        })
    }
}

impl<'a> TryFrom<&'a Value> for NoteRevisionPayload<'a> {
    type Error = String;

    /// Parses a note revision payload from JSON.
    fn try_from(payload: &'a Value) -> Result<Self, Self::Error> {
        Ok(Self {
            id: required_text(payload, "id")?,
            note_id: required_text(payload, "note_id")?,
            content: required_text(payload, "content")?,
            title: optional_text(payload, "title"),
            device_id: optional_text(payload, "device_id"),
            created_at: required_i64(payload, "created_at")?,
            base_revision_id: optional_text(payload, "base_revision_id"),
        })
    }
}

impl<'a> TryFrom<&'a Value> for NoteTagPayload<'a> {
    type Error = String;

    /// Parses a note tag payload from JSON.
    fn try_from(payload: &'a Value) -> Result<Self, Self::Error> {
        Ok(Self {
            note_id: required_text(payload, "note_id")?,
            tag_id: required_text(payload, "tag_id")?,
            created_at: optional_i64(payload, "created_at"),
        })
    }
}

impl<'a> TryFrom<&'a Value> for NoteLinkAttachPayload<'a> {
    type Error = String;

    /// Parses a note link attachment payload from JSON.
    fn try_from(payload: &'a Value) -> Result<Self, Self::Error> {
        Ok(Self {
            id: required_text(payload, "id")?,
            source_note_id: required_text(payload, "source_note_id")?,
            target_note_id: required_text(payload, "target_note_id")?,
            anchor_text: optional_text(payload, "anchor_text"),
            position: optional_i64(payload, "position"),
            created_at: optional_i64(payload, "created_at"),
        })
    }
}

impl<'a> TryFrom<&'a Value> for NoteLinkDetachPayload<'a> {
    type Error = String;

    /// Parses a note link detachment payload from JSON.
    fn try_from(payload: &'a Value) -> Result<Self, Self::Error> {
        Ok(Self {
            id: required_text(payload, "id")?,
        })
    }
}

/// Reads a required string field from payload.
///
/// # Arguments
///
/// * `payload` - JSON payload to inspect.
/// * `field` - Field name to read.
///
/// # Returns
///
/// Returns the field value or an error message.
fn required_text<'a>(payload: &'a Value, field: &str) -> Result<&'a str, String> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("missing text field {field}"))
}

/// Reads an optional string field from payload.
///
/// # Arguments
///
/// * `payload` - JSON payload to inspect.
/// * `field` - Field name to read.
///
/// # Returns
///
/// Returns the optional field value.
fn optional_text<'a>(payload: &'a Value, field: &str) -> Option<&'a str> {
    payload.get(field).and_then(Value::as_str)
}

/// Reads a required integer field from payload.
///
/// # Arguments
///
/// * `payload` - JSON payload to inspect.
/// * `field` - Field name to read.
///
/// # Returns
///
/// Returns the field value or an error message.
fn required_i64(payload: &Value, field: &str) -> Result<i64, String> {
    payload
        .get(field)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("missing integer field {field}"))
}

/// Reads an optional integer field from payload.
///
/// # Arguments
///
/// * `payload` - JSON payload to inspect.
/// * `field` - Field name to read.
///
/// # Returns
///
/// Returns the optional field value.
fn optional_i64(payload: &Value, field: &str) -> Option<i64> {
    payload.get(field).and_then(Value::as_i64)
}
