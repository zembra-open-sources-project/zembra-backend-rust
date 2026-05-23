mod support;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;

#[tokio::test]
async fn recent_notes_route_returns_ordered_visible_notes() {
    let state = support::app::test_state().await;
    let old = support::notes::create_note(&state, "old").await;
    let archived = support::notes::create_note(&state, "archived").await;
    let deleted = support::notes::create_note(&state, "deleted").await;
    let new = support::notes::create_note(&state, "new").await;
    support::notes::set_updated_at(&state, &old, 2_000_000_010).await;
    support::notes::set_updated_at(&state, &archived, 2_000_000_040).await;
    support::notes::set_updated_at(&state, &deleted, 2_000_000_030).await;
    support::notes::set_updated_at(&state, &new, 2_000_000_020).await;

    let service =
        zembra_backend_rust::services::notes::NotesService::new(state.database.pool.clone());
    service.archive_note(&archived).await.unwrap();
    service.delete_note(&deleted).await.unwrap();

    let response = support::app::send_with_state(
        state,
        Request::builder()
            .method("POST")
            .uri("/notes/recent")
            .header("content-type", "application/json")
            .body(Body::from(json!({"limit": 10}).to_string()))
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["notes"][0]["content"], "new");
    assert_eq!(body["notes"][1]["content"], "old");
    assert_eq!(body["notes"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn recent_notes_route_uses_default_and_custom_limit() {
    let state = support::app::test_state().await;
    support::notes::create_note(&state, "first").await;
    support::notes::create_note(&state, "second").await;

    let response = support::app::send_with_state(
        state,
        Request::builder()
            .method("POST")
            .uri("/notes/recent")
            .header("content-type", "application/json")
            .body(Body::from(json!({ "limit": 1 }).to_string()))
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["notes"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn recent_notes_route_uses_note_uuid_cursor() {
    let state = support::app::test_state().await;
    let old = support::notes::create_note(&state, "old").await;
    let cursor = support::notes::create_note(&state, "cursor").await;
    let new = support::notes::create_note(&state, "new").await;
    support::notes::set_updated_at(&state, &old, 2_000_000_010).await;
    support::notes::set_updated_at(&state, &cursor, 2_000_000_020).await;
    support::notes::set_updated_at(&state, &new, 2_000_000_030).await;

    let response = support::app::send_with_state(
        state,
        Request::builder()
            .method("POST")
            .uri("/notes/recent")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "limit": 10,
                    "note_uuid": cursor
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["notes"].as_array().unwrap().len(), 1);
    assert_eq!(body["notes"][0]["content"], "old");
}

#[tokio::test]
async fn recent_notes_route_rejects_invalid_limit() {
    let response = support::app::send(
        Request::builder()
            .method("POST")
            .uri("/notes/recent")
            .header("content-type", "application/json")
            .body(Body::from(json!({ "limit": 101 }).to_string()))
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "validation_error");
}

#[tokio::test]
async fn recent_notes_route_rejects_invalid_note_uuid() {
    let response = support::app::send(
        Request::builder()
            .method("POST")
            .uri("/notes/recent")
            .header("content-type", "application/json")
            .body(Body::from(json!({ "note_uuid": "abcd" }).to_string()))
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "validation_error");
}

#[tokio::test]
async fn recent_notes_route_returns_not_found_for_hidden_note_uuid() {
    let state = support::app::test_state().await;
    let archived = support::notes::create_note(&state, "archived").await;
    let service =
        zembra_backend_rust::services::notes::NotesService::new(state.database.pool.clone());
    service.archive_note(&archived).await.unwrap();

    let response = support::app::send_with_state(
        state,
        Request::builder()
            .method("POST")
            .uri("/notes/recent")
            .header("content-type", "application/json")
            .body(Body::from(json!({ "note_uuid": archived }).to_string()))
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "record_not_found");
}

#[tokio::test]
async fn daily_note_counts_route_returns_thirty_local_days_with_counts() {
    use chrono::{Duration, Local, TimeZone};

    let state = support::app::test_state().await;
    let today = Local::now().date_naive();
    let yesterday = today - Duration::days(1);
    let today_timestamp = Local::now().timestamp();
    let yesterday_timestamp = Local
        .from_local_datetime(&yesterday.and_hms_opt(12, 0, 0).unwrap())
        .single()
        .unwrap()
        .timestamp();
    let first_today = support::notes::create_note(&state, "today 1").await;
    let second_today = support::notes::create_note(&state, "today 2").await;
    let archived_today = support::notes::create_note(&state, "archived today").await;
    let deleted_yesterday = support::notes::create_note(&state, "deleted yesterday").await;
    let visible_yesterday = support::notes::create_note(&state, "visible yesterday").await;

    support::notes::set_created_at(&state, &first_today, today_timestamp).await;
    support::notes::set_created_at(&state, &second_today, today_timestamp).await;
    support::notes::set_created_at(&state, &archived_today, today_timestamp).await;
    support::notes::set_created_at(&state, &deleted_yesterday, yesterday_timestamp).await;
    support::notes::set_created_at(&state, &visible_yesterday, yesterday_timestamp).await;

    let service =
        zembra_backend_rust::services::notes::NotesService::new(state.database.pool.clone());
    service.archive_note(&archived_today).await.unwrap();
    service.delete_note(&deleted_yesterday).await.unwrap();

    let response = support::app::send_with_state(
        state,
        Request::builder()
            .uri("/notes/stats/daily-counts")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;
    let days = body["days"].as_array().unwrap();
    let today_key = today.format("%Y-%m-%d").to_string();
    let yesterday_key = yesterday.format("%Y-%m-%d").to_string();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(days.len(), 30);
    assert_eq!(days.last().unwrap()["date"], today_key);
    assert_eq!(days.last().unwrap()["count"], 2);
    assert_eq!(days[28]["date"], yesterday_key);
    assert_eq!(days[28]["count"], 1);
    assert!(days.iter().take(28).all(|day| day["count"] == 0));
}

#[tokio::test]
async fn notes_by_date_route_returns_ordered_visible_notes_for_date() {
    use chrono::{Duration, Local, TimeZone};

    let state = support::app::test_state().await;
    let target_date = Local::now().date_naive();
    let other_date = target_date - Duration::days(1);
    let older_timestamp = Local
        .from_local_datetime(&target_date.and_hms_opt(9, 0, 0).unwrap())
        .single()
        .unwrap()
        .timestamp();
    let newer_timestamp = Local
        .from_local_datetime(&target_date.and_hms_opt(17, 0, 0).unwrap())
        .single()
        .unwrap()
        .timestamp();
    let other_timestamp = Local
        .from_local_datetime(&other_date.and_hms_opt(12, 0, 0).unwrap())
        .single()
        .unwrap()
        .timestamp();
    let older = support::notes::create_note(&state, "older target").await;
    let newer = support::notes::create_note(&state, "newer target").await;
    let other = support::notes::create_note(&state, "other date").await;

    support::notes::set_created_at(&state, &older, older_timestamp).await;
    support::notes::set_created_at(&state, &newer, newer_timestamp).await;
    support::notes::set_created_at(&state, &other, other_timestamp).await;

    let target_key = target_date.format("%Y-%m-%d").to_string();
    let response = support::app::send_with_state(
        state,
        Request::builder()
            .uri(format!("/notes/by-date?date={target_key}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["date"], target_key);
    assert_eq!(body["notes"].as_array().unwrap().len(), 2);
    assert_eq!(body["notes"][0]["content"], "newer target");
    assert_eq!(body["notes"][1]["content"], "older target");
}

#[tokio::test]
async fn notes_by_date_route_filters_archived_and_deleted_notes() {
    use chrono::Local;

    let state = support::app::test_state().await;
    let target_date = Local::now().date_naive();
    let timestamp = Local::now().timestamp();
    let visible = support::notes::create_note(&state, "visible").await;
    let archived = support::notes::create_note(&state, "archived").await;
    let deleted = support::notes::create_note(&state, "deleted").await;

    support::notes::set_created_at(&state, &visible, timestamp).await;
    support::notes::set_created_at(&state, &archived, timestamp).await;
    support::notes::set_created_at(&state, &deleted, timestamp).await;

    let service =
        zembra_backend_rust::services::notes::NotesService::new(state.database.pool.clone());
    service.archive_note(&archived).await.unwrap();
    service.delete_note(&deleted).await.unwrap();

    let target_key = target_date.format("%Y-%m-%d").to_string();
    let response = support::app::send_with_state(
        state,
        Request::builder()
            .uri(format!("/notes/by-date?date={target_key}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["notes"].as_array().unwrap().len(), 1);
    assert_eq!(body["notes"][0]["content"], "visible");
}

#[tokio::test]
async fn notes_by_date_route_returns_empty_for_date_without_notes() {
    let response = support::app::send(
        Request::builder()
            .uri("/notes/by-date?date=2026-05-22")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["date"], "2026-05-22");
    assert_eq!(body["notes"], json!([]));
}

#[tokio::test]
async fn notes_by_date_route_rejects_missing_or_invalid_date() {
    for uri in ["/notes/by-date", "/notes/by-date?date=2026-13-40"] {
        let response =
            support::app::send(Request::builder().uri(uri).body(Body::empty()).unwrap()).await;
        let status = response.status();
        let body = support::app::response_json(response).await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["error"]["code"], "validation_error");
    }
}

#[tokio::test]
async fn random_notes_route_returns_random_visible_notes() {
    let state = support::app::test_state().await;
    support::notes::create_note(&state, "first").await;
    support::notes::create_note(&state, "second").await;
    let archived = support::notes::create_note(&state, "archived").await;
    let deleted = support::notes::create_note(&state, "deleted").await;
    let service =
        zembra_backend_rust::services::notes::NotesService::new(state.database.pool.clone());
    service.archive_note(&archived).await.unwrap();
    service.delete_note(&deleted).await.unwrap();

    let response = support::app::send_with_state(
        state,
        Request::builder()
            .uri("/random/notes?n=50")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;
    let notes = body["notes"].as_array().unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(notes.len(), 2);
    assert!(
        notes
            .iter()
            .all(|note| note["content"] == "first" || note["content"] == "second")
    );
}

#[tokio::test]
async fn random_notes_route_applies_limit() {
    let state = support::app::test_state().await;
    support::notes::create_note(&state, "first").await;
    support::notes::create_note(&state, "second").await;
    support::notes::create_note(&state, "third").await;

    let response = support::app::send_with_state(
        state,
        Request::builder()
            .uri("/random/notes?n=2")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["notes"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn random_notes_route_returns_empty_when_no_visible_notes_exist() {
    let response = support::app::send(
        Request::builder()
            .uri("/random/notes?n=5")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["notes"], json!([]));
}

#[tokio::test]
async fn random_notes_route_rejects_invalid_n() {
    for uri in ["/random/notes?n=0", "/random/notes?n=51"] {
        let response =
            support::app::send(Request::builder().uri(uri).body(Body::empty()).unwrap()).await;
        let status = response.status();
        let body = support::app::response_json(response).await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["error"]["code"], "validation_error");
    }
}

#[tokio::test]
async fn random_tags_route_groups_visible_notes_by_tag() {
    let state = support::app::test_state().await;
    let shared = support::notes::create_tagged_note(
        &state,
        "shared",
        vec!["rust".to_string(), "sqlite".to_string()],
    )
    .await;
    let rust_only =
        support::notes::create_tagged_note(&state, "rust only", vec!["rust".to_string()]).await;
    let archived =
        support::notes::create_tagged_note(&state, "archived", vec!["rust".to_string()]).await;
    let deleted =
        support::notes::create_tagged_note(&state, "deleted", vec!["sqlite".to_string()]).await;
    let service =
        zembra_backend_rust::services::notes::NotesService::new(state.database.pool.clone());
    service.archive_note(&archived).await.unwrap();
    service.delete_note(&deleted).await.unwrap();

    let response = support::app::send_with_state(
        state,
        Request::builder()
            .uri("/random/tags?n=2&count=10")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;
    let groups = body["tagged_notes"].as_array().unwrap();
    let shared_count = groups
        .iter()
        .filter(|group| {
            group["notes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|note| note["id"] == shared)
        })
        .count();
    let rust_count = groups
        .iter()
        .flat_map(|group| group["notes"].as_array().unwrap())
        .filter(|note| note["id"] == rust_only)
        .count();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(groups.len(), 2);
    assert_eq!(shared_count, 2);
    assert_eq!(rust_count, 1);
    assert!(
        groups
            .iter()
            .flat_map(|group| group["notes"].as_array().unwrap())
            .all(|note| note["content"] != "archived" && note["content"] != "deleted")
    );
}

#[tokio::test]
async fn random_tags_route_limits_cumulative_notes() {
    let state = support::app::test_state().await;
    for content in ["first", "second", "third"] {
        support::notes::create_tagged_note(&state, content, vec!["rust".to_string()]).await;
    }

    let response = support::app::send_with_state(
        state,
        Request::builder()
            .uri("/random/tags?n=1&count=2")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;
    let groups = body["tagged_notes"].as_array().unwrap();
    let notes_count = groups
        .iter()
        .map(|group| group["notes"].as_array().unwrap().len())
        .sum::<usize>();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(groups.len(), 1);
    assert_eq!(notes_count, 2);
}

#[tokio::test]
async fn random_tags_route_returns_existing_tags_when_n_is_larger() {
    let state = support::app::test_state().await;
    support::notes::create_tagged_note(&state, "rust", vec!["rust".to_string()]).await;
    support::notes::create_tagged_note(&state, "sqlite", vec!["sqlite".to_string()]).await;

    let response = support::app::send_with_state(
        state,
        Request::builder()
            .uri("/random/tags?n=20")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["tagged_notes"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn random_tags_route_uses_default_limit() {
    let state = support::app::test_state().await;
    for name in ["alpha", "beta", "gamma", "delta"] {
        support::notes::create_tagged_note(&state, name, vec![name.to_string()]).await;
    }
    for index in 0..25 {
        support::notes::create_tagged_note(
            &state,
            &format!("extra {index}"),
            vec!["alpha".to_string()],
        )
        .await;
    }

    let response = support::app::send_with_state(
        state,
        Request::builder()
            .uri("/random/tags")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["tagged_notes"].as_array().unwrap().len(), 3);
    assert!(
        body["tagged_notes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|group| group["notes"].as_array().unwrap().len())
            .sum::<usize>()
            <= 20
    );
}

#[tokio::test]
async fn random_tags_route_rejects_invalid_query_values() {
    for uri in [
        "/random/tags?n=0",
        "/random/tags?n=21",
        "/random/tags?count=0",
        "/random/tags?count=101",
    ] {
        let response =
            support::app::send(Request::builder().uri(uri).body(Body::empty()).unwrap()).await;
        let status = response.status();
        let body = support::app::response_json(response).await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["error"]["code"], "validation_error");
    }
}

#[tokio::test]
async fn random_fields_route_limits_cumulative_notes() {
    let state = support::app::test_state().await;
    for content in ["first", "second", "third"] {
        support::notes::create_field_note(&state, content, "work").await;
    }

    let response = support::app::send_with_state(
        state,
        Request::builder()
            .uri("/random/fields?n=1&count=2")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;
    let groups = body["field_notes"].as_array().unwrap();
    let notes_count = groups
        .iter()
        .map(|group| group["notes"].as_array().unwrap().len())
        .sum::<usize>();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(groups.len(), 1);
    assert_eq!(notes_count, 2);
}

#[tokio::test]
async fn random_fields_route_filters_hidden_notes_and_keeps_empty_fields() {
    let state = support::app::test_state().await;
    let visible = support::notes::create_field_note(&state, "visible", "work").await;
    let archived = support::notes::create_field_note(&state, "archived", "work").await;
    let deleted = support::notes::create_field_note(&state, "deleted", "empty").await;
    let service =
        zembra_backend_rust::services::notes::NotesService::new(state.database.pool.clone());
    service.archive_note(&archived).await.unwrap();
    service.delete_note(&deleted).await.unwrap();

    let response = support::app::send_with_state(
        state,
        Request::builder()
            .uri("/random/fields?n=2&count=10")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;
    let groups = body["field_notes"].as_array().unwrap();
    let all_notes = groups
        .iter()
        .flat_map(|group| group["notes"].as_array().unwrap())
        .collect::<Vec<_>>();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(groups.len(), 2);
    assert!(all_notes.iter().any(|note| note["id"] == visible));
    assert!(
        all_notes
            .iter()
            .all(|note| { note["content"] != "archived" && note["content"] != "deleted" })
    );
    assert!(
        groups
            .iter()
            .any(|group| group["notes"].as_array().unwrap().is_empty())
    );
}

#[tokio::test]
async fn random_fields_route_returns_existing_fields_when_n_is_larger() {
    let state = support::app::test_state().await;
    support::notes::create_field_note(&state, "work", "work").await;
    support::notes::create_field_note(&state, "life", "life").await;

    let response = support::app::send_with_state(
        state,
        Request::builder()
            .uri("/random/fields?n=20&count=20")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["field_notes"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn random_fields_route_uses_default_query_values() {
    let state = support::app::test_state().await;
    for name in ["alpha", "beta", "gamma", "delta"] {
        support::notes::create_field_note(&state, name, name).await;
    }

    let response = support::app::send_with_state(
        state,
        Request::builder()
            .uri("/random/fields")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["field_notes"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn random_fields_route_rejects_invalid_query_values() {
    for uri in [
        "/random/fields?n=0",
        "/random/fields?n=21",
        "/random/fields?count=0",
        "/random/fields?count=101",
    ] {
        let response =
            support::app::send(Request::builder().uri(uri).body(Body::empty()).unwrap()).await;
        let status = response.status();
        let body = support::app::response_json(response).await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["error"]["code"], "validation_error");
    }
}
