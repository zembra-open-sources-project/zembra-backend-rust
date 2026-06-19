use super::SupabaseClient;
use crate::repositories::sync::SyncChangeRecord;

/// Builds a local sync change for projection tests.
///
/// # Arguments
///
/// * `entity_type` - Entity type stored in `sync_changes`.
/// * `operation` - Operation stored in `sync_changes`.
/// * `payload` - JSON payload to project.
///
/// # Returns
///
/// Returns a sync change record.
fn change(entity_type: &str, operation: &str, payload: serde_json::Value) -> SyncChangeRecord {
    SyncChangeRecord {
        id: "change-1".to_string(),
        workspace_id: crate::repositories::taxonomy::DEFAULT_WORKSPACE_ID.to_string(),
        device_id: crate::repositories::sync::DEFAULT_DEVICE_ID.to_string(),
        entity_type: entity_type.to_string(),
        entity_id: "entity-1".to_string(),
        operation: operation.to_string(),
        base_revision_id: None,
        new_revision_id: None,
        payload: payload.to_string(),
        created_at: 123,
        applied_at: Some(123),
        supabase_committed_at: None,
    }
}

/// Reads a request JSON body for assertions.
///
/// # Arguments
///
/// * `request` - Request whose body should contain JSON bytes.
///
/// # Returns
///
/// Returns the parsed JSON body.
fn request_json(request: &reqwest::Request) -> serde_json::Value {
    let bytes = request.body().and_then(reqwest::Body::as_bytes).unwrap();
    serde_json::from_slice(bytes).unwrap()
}

#[test]
fn field_projection_builds_upsert_request() {
    let client = SupabaseClient::new("https://example.supabase.co", "sb_secret_test-key");
    let request = client
        .build_business_projection_request(&change(
            "field",
            "insert",
            serde_json::json!({
                "id": "field-1",
                "name": "Work",
                "created_at": 100
            }),
        ))
        .unwrap();
    let body = request_json(&request);

    assert_eq!(request.method(), reqwest::Method::POST);
    assert_eq!(
        request.url().as_str(),
        "https://example.supabase.co/rest/v1/fields"
    );
    assert_eq!(request.headers()["prefer"], "resolution=merge-duplicates");
    assert_eq!(body[0]["id"], "field-1");
    assert_eq!(
        body[0]["workspace_id"],
        crate::repositories::taxonomy::DEFAULT_WORKSPACE_ID
    );
}

#[test]
fn tag_projection_keeps_hierarchy_fields() {
    let client = SupabaseClient::new("https://example.supabase.co", "sb_secret_test-key");
    let request = client
        .build_business_projection_request(&change(
            "tag",
            "insert",
            serde_json::json!({
                "id": "tag-1",
                "name": "rust",
                "parent_tag_id": "tag-parent",
                "path": "dev/rust",
                "depth": 1,
                "created_at": 100
            }),
        ))
        .unwrap();
    let body = request_json(&request);

    assert_eq!(
        request.url().as_str(),
        "https://example.supabase.co/rest/v1/tags"
    );
    assert_eq!(body[0]["parent_tag_id"], "tag-parent");
    assert_eq!(body[0]["path"], "dev/rust");
    assert_eq!(body[0]["depth"], 1);
}

#[test]
fn note_projection_preserves_soft_state() {
    let client = SupabaseClient::new("https://example.supabase.co", "sb_secret_test-key");
    let request = client
        .build_business_projection_request(&change(
            "note",
            "delete",
            serde_json::json!({
                "id": "note-1",
                "content": "gone",
                "role": "Human",
                "field_id": null,
                "created_at": 100,
                "updated_at": 110,
                "archived_at": 105,
                "deleted_at": 110,
                "current_revision_id": "revision-1"
            }),
        ))
        .unwrap();
    let body = request_json(&request);

    assert_eq!(
        request.url().as_str(),
        "https://example.supabase.co/rest/v1/notes"
    );
    assert_eq!(body[0]["deleted_at"], 110);
    assert_eq!(body[0]["archived_at"], 105);
    assert_eq!(body[0]["current_revision_id"], "revision-1");
    assert_eq!(body[0]["last_change_id"], "change-1");
}

#[test]
fn note_revision_projection_sets_change_id() {
    let client = SupabaseClient::new("https://example.supabase.co", "sb_secret_test-key");
    let request = client
        .build_business_projection_request(&change(
            "note_revision",
            "insert",
            serde_json::json!({
                "id": "revision-1",
                "note_id": "note-1",
                "content": "body",
                "title": null,
                "device_id": "device-1",
                "created_at": 100,
                "base_revision_id": null
            }),
        ))
        .unwrap();
    let body = request_json(&request);

    assert_eq!(
        request.url().as_str(),
        "https://example.supabase.co/rest/v1/note_revisions"
    );
    assert_eq!(body[0]["note_id"], "note-1");
    assert_eq!(body[0]["device_id"], "device-1");
    assert_eq!(body[0]["change_id"], "change-1");
}

#[test]
fn note_tag_detach_builds_filtered_delete_request() {
    let client = SupabaseClient::new("https://example.supabase.co", "sb_secret_test-key");
    let request = client
        .build_business_projection_request(&change(
            "note_tag",
            "detach",
            serde_json::json!({
                "note_id": "note-1",
                "tag_id": "tag-1"
            }),
        ))
        .unwrap();
    let url = request.url().as_str();

    assert_eq!(request.method(), reqwest::Method::DELETE);
    assert!(url.starts_with("https://example.supabase.co/rest/v1/note_tags?"));
    assert!(url.contains("workspace_id=eq."));
    assert!(url.contains("note_id=eq.note-1"));
    assert!(url.contains("tag_id=eq.tag-1"));
}

#[test]
fn note_link_attach_and_detach_build_projection_requests() {
    let client = SupabaseClient::new("https://example.supabase.co", "sb_secret_test-key");
    let attach = client
        .build_business_projection_request(&change(
            "note_link",
            "attach",
            serde_json::json!({
                "id": "link-1",
                "source_note_id": "source",
                "target_note_id": "target",
                "anchor_text": "target",
                "position": 3,
                "created_at": 100
            }),
        ))
        .unwrap();
    let detach = client
        .build_business_projection_request(&change(
            "note_link",
            "detach",
            serde_json::json!({
                "id": "link-1"
            }),
        ))
        .unwrap();
    let attach_body = request_json(&attach);
    let detach_url = detach.url().as_str();

    assert_eq!(
        attach.url().as_str(),
        "https://example.supabase.co/rest/v1/note_links"
    );
    assert_eq!(attach_body[0]["source_note_id"], "source");
    assert_eq!(attach_body[0]["target_note_id"], "target");
    assert_eq!(detach.method(), reqwest::Method::DELETE);
    assert!(detach_url.contains("workspace_id=eq."));
    assert!(detach_url.contains("id=eq.link-1"));
}

#[test]
fn unsupported_projection_returns_payload_error() {
    let client = SupabaseClient::new("https://example.supabase.co", "sb_secret_test-key");
    let error = client
        .build_business_projection_request(&change(
            "attachment",
            "insert",
            serde_json::json!({"id": "attachment-1"}),
        ))
        .unwrap_err();

    assert!(error.to_string().contains("unsupported projection"));
}
