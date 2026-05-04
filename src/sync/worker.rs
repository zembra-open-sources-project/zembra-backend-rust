use std::time::Duration;

use tracing::{debug, error, info};

/// Starts the background synchronization loop when enabled.
///
/// # Arguments
///
/// * `service` - Synchronization service to run.
/// * `settings` - Runtime synchronization settings.
pub fn spawn_background_sync(
    service: crate::services::sync::SyncService,
    settings: crate::config::SyncSettings,
) {
    if !settings.enabled {
        debug!("background synchronization is disabled");
        return;
    }

    let interval = Duration::from_secs(settings.interval_seconds);
    tokio::spawn(async move {
        info!(
            interval_seconds = settings.interval_seconds,
            "background synchronization worker started"
        );

        loop {
            match service.run_once().await {
                Ok(summary) => {
                    info!(
                        pushed = summary.pushed,
                        pulled = summary.pulled,
                        "background synchronization cycle finished"
                    );
                }
                Err(error) => {
                    error!(%error, "background synchronization cycle failed");
                }
            }

            tokio::time::sleep(interval).await;
        }
    });
}
