use crate::models::note::NoteRecord;
use crate::models::note_link::NoteLinkRecord;
use crate::repositories::taxonomy::DEFAULT_WORKSPACE_ID;

/// Builds a sync payload for a note record.
///
/// # Arguments
///
/// * `note` - Note record to serialize.
///
/// # Returns
///
/// Returns a JSON payload containing the workspace-scoped note snapshot.
pub(super) fn note_payload(note: &NoteRecord) -> serde_json::Value {
    serde_json::json!({
        "id": note.id,
        "workspace_id": DEFAULT_WORKSPACE_ID,
        "content": note.content,
        "role": note.role,
        "field_id": note.field_id,
        "created_at": note.created_at,
        "updated_at": note.updated_at,
        "archived_at": note.archived_at,
        "deleted_at": note.deleted_at,
        "current_revision_id": note.current_revision_id
    })
}

/// Builds a stable synthetic entity ID for a note/tag relation.
///
/// # Arguments
///
/// * `note_id` - Note identifier.
/// * `tag_id` - Tag identifier.
///
/// # Returns
///
/// Returns a relation identifier.
pub(super) fn note_tag_entity_id(note_id: &str, tag_id: &str) -> String {
    format!("{note_id}:{tag_id}")
}

/// Builds a sync payload for a note/tag relation.
///
/// # Arguments
///
/// * `note_id` - Note identifier.
/// * `tag_id` - Tag identifier.
///
/// # Returns
///
/// Returns a JSON payload for the relation change.
pub(super) fn note_tag_payload(note_id: &str, tag_id: &str) -> serde_json::Value {
    serde_json::json!({
        "workspace_id": DEFAULT_WORKSPACE_ID,
        "note_id": note_id,
        "tag_id": tag_id
    })
}

/// Builds a sync payload for a note link relation.
///
/// # Arguments
///
/// * `link` - Persisted note link record.
///
/// # Returns
///
/// Returns a JSON payload for the relation change.
pub(super) fn note_link_payload(link: &NoteLinkRecord) -> serde_json::Value {
    serde_json::json!({
        "id": link.id,
        "workspace_id": DEFAULT_WORKSPACE_ID,
        "source_note_id": link.source_note_id,
        "target_note_id": link.target_note_id,
        "anchor_text": link.anchor_text,
        "position": link.position,
        "created_at": link.created_at
    })
}
