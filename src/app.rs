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
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::response::Response;
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
}
