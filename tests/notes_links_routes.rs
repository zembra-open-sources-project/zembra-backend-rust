mod support;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;

#[tokio::test]
async fn note_routes_return_link_metadata() {
    let state = support::app::test_state().await;
    let target_id = support::notes::create_note(&state, "target").await;

    let create_response = support::app::send_with_state(
        state.clone(),
        Request::builder()
            .method("POST")
            .uri("/notes")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "content": "source",
                    "role": "Human",
                    "links": [{
                        "target_note_ref": target_id,
                        "anchor_text": "target",
                        "position": 2
                    }]
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    let status = create_response.status();
    let body = support::app::response_json(create_response).await;
    let source_id = body["note"]["id"].as_str().unwrap().to_string();

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(
        body["metadata"]["outgoing_links"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        body["metadata"]["outgoing_links"][0]["target_note_id"],
        json!(target_id)
    );

    let get_response = support::app::send_with_state(
        state.clone(),
        Request::builder()
            .method("GET")
            .uri(format!("/notes/{target_id}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let body = support::app::response_json(get_response).await;

    assert_eq!(body["metadata"]["backlinks"].as_array().unwrap().len(), 1);
    assert_eq!(
        body["metadata"]["backlinks"][0]["source_note_id"],
        json!(source_id)
    );

    let patch_response = support::app::send_with_state(
        state,
        Request::builder()
            .method("PATCH")
            .uri(format!("/notes/{source_id}"))
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "content": "source without links",
                    "links": []
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    let body = support::app::response_json(patch_response).await;

    assert!(
        body["metadata"]["outgoing_links"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
