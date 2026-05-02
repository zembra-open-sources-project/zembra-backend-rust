use axum::Router;

/// Shared application state available to all handlers.
#[derive(Debug, Clone)]
pub struct AppState {
    /// SQLite database handle used by repositories and health checks.
    pub database: crate::repositories::database::Database,
}

/// Builds the root HTTP router for the backend service.
///
/// # Arguments
///
/// * `state` - Shared application state injected into route handlers.
///
/// # Returns
///
/// Returns an Axum router containing infrastructure routes only.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .merge(crate::routes::health::router())
        .merge(crate::routes::notes::router())
        .merge(crate::routes::taxonomy::router())
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode};
    use axum::response::Response;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    /// Sends a request to the application router in tests.
    ///
    /// # Arguments
    ///
    /// * `request` - HTTP request to dispatch through the router.
    ///
    /// # Returns
    ///
    /// Returns the HTTP response produced by the router.
    async fn send(request: Request<Body>) -> Response {
        let database = crate::repositories::database::Database::connect("sqlite://:memory:")
            .await
            .unwrap();
        let state = super::AppState { database };

        super::build_router(state).oneshot(request).await.unwrap()
    }

    /// Reads a response body as JSON.
    ///
    /// # Arguments
    ///
    /// * `response` - HTTP response to read.
    ///
    /// # Returns
    ///
    /// Returns parsed JSON.
    async fn response_json(response: Response) -> Value {
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    #[tokio::test]
    async fn health_route_returns_ok() {
        let response = send(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn create_note_route_returns_created_note() {
        let response = send(
            Request::builder()
                .method("POST")
                .uri("/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "content": "api note",
                        "field": "work",
                        "tags": ["rust", "rust", "api"],
                        "role": "Human",
                        "device_id": null
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await;
        let status = response.status();
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["note"]["content"], "api note");
        assert_eq!(body["metadata"]["field"], "work");
        assert_eq!(body["metadata"]["tags"], json!(["rust", "api"]));
    }

    #[tokio::test]
    async fn create_note_rejects_invalid_role() {
        let response = send(
            Request::builder()
                .method("POST")
                .uri("/notes")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "content": "api note",
                        "role": "Robot"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await;
        let status = response.status();
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["error"]["code"], "validation_error");
    }
}
