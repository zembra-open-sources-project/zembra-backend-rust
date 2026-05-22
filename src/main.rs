use std::net::SocketAddr;
use zembra_backend_rust::{app, config, error, logging, repositories, services, sync};

/// Starts the Zembra backend HTTP server.
///
/// # Returns
///
/// Returns `Ok(())` when the server exits cleanly, or an application error when
/// configuration loading, socket binding, or server execution fails.
#[tokio::main]
async fn main() -> Result<(), error::AppError> {
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
    axum::serve(listener, app).await?;

    Ok(())
}
