//! HTTP server startup and shutdown orchestration.

use std::net::SocketAddr;

use crate::{app, config, error, logging, repositories, services, sync};

/// Starts the Zembra backend HTTP server.
///
/// # Returns
///
/// Returns `Ok(())` when the server exits cleanly, or an application error when
/// configuration loading, socket binding, or server execution fails.
pub async fn run() -> Result<(), error::AppError> {
    let settings = config::Settings::load()?;
    let _logging_guard = logging::init(&settings.logging);
    let database_url = settings.database.sqlite_url();
    let database = repositories::database::Database::connect(&database_url).await?;
    let sync_service = services::sync::SyncService::new(database.pool.clone(), &settings.sync);
    sync::worker::spawn_background_sync(sync_service.clone());
    let sync_config = services::sync_config::SyncConfigService::from_user_config()?;
    let app = app::build_router_with_cors(
        app::AppState {
            database,
            sync: sync_service,
            sync_config,
        },
        settings.server.cors_origin_rules()?,
    );
    let host = settings.server.host_addr()?;
    let addr = SocketAddr::from((host, settings.server.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    logging::log_startup_summary(addr, &settings.database.path);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

/// Waits for a process shutdown signal.
///
/// # Returns
///
/// Returns after receiving Ctrl-C or, on Unix platforms, SIGTERM.
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(%error, "failed to listen for Ctrl-C shutdown signal");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                tracing::error!(%error, "failed to listen for SIGTERM shutdown signal");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("shutdown signal received");
}
