use std::time::Duration;

use tracing::{debug, error, info};

/// Starts the background synchronization loop when enabled.
///
/// # Arguments
///
/// * `service` - Synchronization service to run.
pub fn spawn_background_sync(service: crate::services::sync::SyncService) {
    tokio::spawn(async move {
        info!("background synchronization worker started");

        loop {
            let settings = service.settings();
            if settings.enabled {
                match service.run_once().await {
                    Ok(summary) => {
                        info!(
                            pushed = summary.pushed,
                            pulled = summary.pulled,
                            "background synchronization cycle finished"
                        );
                    }
                    Err(error) => {
                        error!(%error, ?error, "background synchronization cycle failed");
                    }
                }
            } else {
                debug!("background synchronization is disabled");
            }

            tokio::time::sleep(Duration::from_secs(settings.interval_seconds)).await;
        }
    });
}
