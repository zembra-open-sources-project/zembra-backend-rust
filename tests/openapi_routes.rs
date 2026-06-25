mod support;

use axum::body::Body;
use axum::http::{Request, StatusCode};
#[tokio::test]
async fn openapi_json_lists_runtime_api_paths() {
    let response = support::app::send(
        Request::builder()
            .uri("/api-docs/openapi.json")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;

    assert_eq!(status, StatusCode::OK);
    assert!(body["paths"].get("/health").is_some());
    assert!(body["paths"].get("/notes").is_some());
    assert!(body["paths"].get("/notes/recent").is_some());
    assert!(body["paths"].get("/notes/stats/daily-counts").is_some());
    assert!(body["paths"].get("/notes/by-date").is_some());
    assert!(body["paths"].get("/random/notes").is_some());
    assert!(body["paths"].get("/random/tags").is_some());
    assert!(body["paths"].get("/random/fields").is_some());
    assert!(body["paths"].get("/notes/batch").is_some());
    assert!(body["paths"].get("/fields").is_some());
    assert!(body["paths"].get("/tags").is_some());
    assert!(body["paths"].get("/workspaces").is_some());
    assert!(body["paths"].get("/sync/status").is_some());
    assert!(body["paths"].get("/sync/config").is_some());
    assert!(body["paths"].get("/sync/config/test").is_some());
    assert!(body["paths"].get("/sync/run").is_some());
    assert!(body["paths"].get("/sync/push").is_some());
    assert!(body["paths"].get("/sync/pull").is_some());
    assert!(
        body["components"]["schemas"]["UpdateNoteRequest"]["properties"]
            .get("field")
            .is_some()
    );
    assert!(
        body["components"]["schemas"]["UpdateNoteRequest"]["properties"]
            .get("tags")
            .is_some()
    );
    assert!(
        body["components"]["schemas"]["CreateNoteRequest"]["properties"]
            .get("links")
            .is_some()
    );
    assert!(
        body["components"]["schemas"]["RecentNotesRequest"]["properties"]
            .get("role")
            .is_some()
    );
    assert!(
        body["components"]["schemas"]["UpdateNoteRequest"]["properties"]
            .get("links")
            .is_some()
    );
    assert!(
        body["components"]["schemas"]["NoteMetadata"]["properties"]
            .get("outgoing_links")
            .is_some()
    );
    assert!(
        body["components"]["schemas"]["NoteMetadata"]["properties"]
            .get("backlinks")
            .is_some()
    );
    assert!(
        body["components"]["schemas"]["TagRecord"]["properties"]
            .get("parent_tag_id")
            .is_some()
    );
    assert!(
        body["components"]["schemas"]["TagRecord"]["properties"]
            .get("path")
            .is_some()
    );
    assert!(
        body["components"]["schemas"]["TagRecord"]["properties"]
            .get("depth")
            .is_some()
    );
    assert!(
        body["components"]["schemas"]["HealthResponse"]["properties"]
            .get("version")
            .is_some()
    );
    assert!(
        body["components"]["schemas"]["HealthResponse"]["properties"]
            .get("version_policy")
            .is_some()
    );
    assert!(
        body["components"]["schemas"]["HealthResponse"]["properties"]
            .get("release_channel")
            .is_some()
    );
    assert!(
        body["components"]["schemas"]["WorkspaceSummary"]["properties"]
            .get("short_hash")
            .is_some()
    );
    assert!(
        body["components"]["schemas"]["WorkspaceSummary"]["properties"]
            .get("workspace_name")
            .is_some()
    );

    for (path, method) in [
        ("/notes", "get"),
        ("/notes", "post"),
        ("/notes/batch", "post"),
        ("/notes/recent", "post"),
        ("/notes/stats/daily-counts", "get"),
        ("/notes/by-date", "get"),
        ("/random/notes", "get"),
        ("/random/tags", "get"),
        ("/random/fields", "get"),
        ("/notes/{note_ref}", "get"),
        ("/notes/{note_ref}", "patch"),
        ("/notes/{note_ref}", "delete"),
        ("/notes/{note_ref}/archive", "post"),
        ("/notes/{note_ref}/tags", "get"),
        ("/notes/{note_ref}/tags/{tag_name}", "put"),
        ("/notes/{note_ref}/tags/{tag_name}", "delete"),
        ("/notes/{note_ref}/revisions", "get"),
    ] {
        assert_has_required_workspace_query(&body, path, method);
    }
}

#[tokio::test]
async fn swagger_ui_is_available() {
    let response = support::app::send(
        Request::builder()
            .uri("/swagger-ui/")
            .body(Body::empty())
            .unwrap(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
}

/// Asserts that an OpenAPI operation exposes required `workspace_id` query scope.
///
/// # Arguments
///
/// * `body` - Parsed OpenAPI JSON.
/// * `path` - OpenAPI path to inspect.
/// * `method` - Lowercase HTTP method to inspect.
fn assert_has_required_workspace_query(body: &serde_json::Value, path: &str, method: &str) {
    let parameters = body["paths"][path][method]["parameters"]
        .as_array()
        .unwrap_or_else(|| panic!("{method} {path} should expose parameters"));
    let workspace = parameters
        .iter()
        .find(|parameter| parameter["name"] == "workspace_id" && parameter["in"] == "query")
        .unwrap_or_else(|| panic!("{method} {path} should expose workspace_id query"));

    assert_eq!(workspace["required"], true);
}
