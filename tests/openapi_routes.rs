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
