mod api_doc;
mod app;
mod config;
mod dto;
mod error;
mod handlers;
mod models;
mod repositories;
mod routes;
mod services;

use std::net::{Ipv4Addr, SocketAddr};

use tracing::info;

/// Starts the Zembra backend HTTP server.
///
/// # Returns
///
/// Returns `Ok(())` when the server exits cleanly, or an application error when
/// configuration loading, socket binding, or server execution fails.
#[tokio::main]
async fn main() -> Result<(), error::AppError> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let settings = config::Settings::load()?;
    let database_url = settings.database.sqlite_url();
    let database = repositories::database::Database::connect(&database_url).await?;
    let app = app::build_router(app::AppState { database });
    let host = Ipv4Addr::from(settings.server.host);
    let addr = SocketAddr::from((host, settings.server.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!(%addr, %database_url, "starting Zembra backend");
    axum::serve(listener, app).await?;

    Ok(())
}
