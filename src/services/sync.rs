use crate::config::SyncSettings;
use crate::repositories::sync::{SyncRepository, SyncStateRecord};
use crate::sync::supabase::{SupabaseClient, SupabaseError};

const SYNC_BATCH_LIMIT: i64 = 100;

/// Service for Supabase synchronization workflows.
#[derive(Debug, Clone)]
pub struct SyncService {
    /// Local synchronization repository.
    repository: SyncRepository,
    /// Supabase REST client.
    supabase: SupabaseClient,
    /// Whether sync operations are enabled.
    enabled: bool,
}

impl SyncService {
    /// Creates a sync service from runtime settings.
    ///
    /// # Arguments
    ///
    /// * `pool` - Shared SQLite pool.
    /// * `settings` - Sync settings.
    ///
    /// # Returns
    ///
    /// Returns a sync service.
    pub fn new(pool: sqlx::SqlitePool, settings: &SyncSettings) -> Self {
        Self {
            repository: SyncRepository::new(pool),
            supabase: SupabaseClient::from_settings(settings),
            enabled: settings.enabled,
        }
    }

    /// Runs a single push and pull cycle.
    ///
    /// # Returns
    ///
    /// Returns a summary of the sync cycle.
    pub async fn run_once(&self) -> Result<SyncRunSummary, SyncError> {
        let pushed = self.push().await?.processed;
        let pulled = self.pull().await?.processed;

        Ok(SyncRunSummary { pushed, pulled })
    }

    /// Pushes local changes to Supabase.
    ///
    /// # Returns
    ///
    /// Returns a push summary.
    pub async fn push(&self) -> Result<SyncDirectionSummary, SyncError> {
        self.ensure_enabled()?;
        let changes = self
            .repository
            .list_pending_push_changes(SYNC_BATCH_LIMIT)
            .await?;

        match self.supabase.upsert_sync_changes(&changes).await {
            Ok(()) => {
                self.repository.mark_push_success(&changes).await?;
                Ok(SyncDirectionSummary {
                    processed: changes.len(),
                })
            }
            Err(error) => {
                let message = error.to_string();
                self.repository.record_error("push", &message).await?;
                Err(error.into())
            }
        }
    }

    /// Pulls remote changes from Supabase.
    ///
    /// # Returns
    ///
    /// Returns a pull summary.
    pub async fn pull(&self) -> Result<SyncDirectionSummary, SyncError> {
        self.ensure_enabled()?;
        let state = self.repository.get_or_create_state("pull").await?;

        match self
            .supabase
            .fetch_remote_changes(
                state.last_change_created_at,
                &state.last_change_id,
                SYNC_BATCH_LIMIT,
            )
            .await
        {
            Ok(changes) => {
                let applied = self.repository.apply_remote_changes(&changes).await?;
                let (created_at, change_id) = changes
                    .last()
                    .map(|change| (change.created_at, change.id.as_str()))
                    .unwrap_or((state.last_change_created_at, state.last_change_id.as_str()));
                self.repository
                    .record_success("pull", created_at, change_id)
                    .await?;
                Ok(SyncDirectionSummary { processed: applied })
            }
            Err(error) => {
                let message = error.to_string();
                self.repository.record_error("pull", &message).await?;
                Err(error.into())
            }
        }
    }

    /// Reads local sync status.
    ///
    /// # Returns
    ///
    /// Returns status rows without exposing secrets.
    pub async fn status(&self) -> Result<SyncStatus, SyncError> {
        Ok(SyncStatus {
            enabled: self.enabled,
            states: self.repository.list_states().await?,
        })
    }

    /// Ensures sync operations can call Supabase.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` when sync is enabled.
    fn ensure_enabled(&self) -> Result<(), SyncError> {
        if self.enabled {
            Ok(())
        } else {
            Err(SyncError::Disabled)
        }
    }
}

/// Summary for one full sync run.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncRunSummary {
    /// Number of local changes pushed.
    pub pushed: usize,
    /// Number of remote changes pulled.
    pub pulled: usize,
}

/// Summary for one sync direction.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncDirectionSummary {
    /// Number of changes processed.
    pub processed: usize,
}

/// Sync status returned by services and handlers.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncStatus {
    /// Whether synchronization is enabled.
    pub enabled: bool,
    /// Local sync state rows.
    pub states: Vec<SyncStateRecord>,
}

/// Error returned by synchronization services.
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    /// Synchronization is disabled.
    #[error("synchronization is disabled")]
    Disabled,
    /// Local database operation failed.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    /// Supabase request failed.
    #[error("{0}")]
    Supabase(#[from] SupabaseError),
}
