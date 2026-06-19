use crate::config::SyncSettings;
use crate::repositories::sync::{SyncRepository, SyncStateRecord};
use crate::sync::diff::{SyncDiffConflict, diff_snapshots};
use crate::sync::schema_contract::{SchemaContractVersions, TARGET_SCHEMA_CONTRACT_VERSION};
use crate::sync::supabase::{SupabaseClient, SupabaseError};
use std::sync::{Arc, RwLock};

/// Service for Supabase synchronization workflows.
#[derive(Debug, Clone)]
pub struct SyncService {
    /// Local synchronization repository.
    repository: SyncRepository,
    /// Runtime synchronization settings.
    settings: Arc<RwLock<SyncSettings>>,
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
            settings: Arc::new(RwLock::new(settings.clone())),
        }
    }

    /// Updates runtime sync settings.
    ///
    /// # Arguments
    ///
    /// * `settings` - New synchronization settings to use for later operations.
    pub fn update_settings(&self, settings: SyncSettings) {
        let mut current = self
            .settings
            .write()
            .expect("sync settings lock should not be poisoned");
        *current = settings;
    }

    /// Returns a clone of the current runtime sync settings.
    ///
    /// # Returns
    ///
    /// Returns the latest settings snapshot.
    pub fn settings(&self) -> SyncSettings {
        self.settings
            .read()
            .expect("sync settings lock should not be poisoned")
            .clone()
    }

    /// Runs a single push and pull cycle.
    ///
    /// # Returns
    ///
    /// Returns a summary of the sync cycle.
    pub async fn run_once(&self) -> Result<SyncRunSummary, SyncError> {
        let summary = self.sync_tables().await?;

        Ok(SyncRunSummary {
            pushed: summary.pushed,
            pulled: summary.pulled,
        })
    }

    /// Pushes local changes to Supabase.
    ///
    /// # Returns
    ///
    /// Returns a push summary.
    pub async fn push(&self) -> Result<SyncDirectionSummary, SyncError> {
        let settings = self.settings();
        self.ensure_enabled(&settings)?;
        let supabase = SupabaseClient::from_settings(&settings);
        self.ensure_schema_contract_ready(&supabase).await?;
        let (local, remote) = self.read_snapshots(&supabase).await?;
        let diff = diff_snapshots(&local, &remote);
        self.ensure_no_conflicts(&diff.conflicts)?;
        let write_remote = local.subset_for_diffs(&diff.write_remote);
        let processed = write_remote.row_count();
        self.write_remote_snapshot(&supabase, &write_remote).await?;
        self.verify_direction_converged(&supabase, SyncDirection::Push)
            .await?;
        Ok(SyncDirectionSummary { processed })
    }

    /// Pulls remote changes from Supabase.
    ///
    /// # Returns
    ///
    /// Returns a pull summary.
    pub async fn pull(&self) -> Result<SyncDirectionSummary, SyncError> {
        let settings = self.settings();
        self.ensure_enabled(&settings)?;
        let supabase = SupabaseClient::from_settings(&settings);
        self.ensure_schema_contract_ready(&supabase).await?;
        let (local, remote) = self.read_snapshots(&supabase).await?;
        let diff = diff_snapshots(&local, &remote);
        self.ensure_no_conflicts(&diff.conflicts)?;
        let write_local = remote.subset_for_diffs(&diff.write_local);
        let processed = write_local.row_count();
        self.repository
            .write_local_table_snapshot(&write_local)
            .await?;
        self.verify_direction_converged(&supabase, SyncDirection::Pull)
            .await?;
        Ok(SyncDirectionSummary { processed })
    }

    /// Reads local sync status.
    ///
    /// # Returns
    ///
    /// Returns status rows without exposing secrets.
    pub async fn status(&self) -> Result<SyncStatus, SyncError> {
        Ok(SyncStatus {
            enabled: self.settings().enabled,
            states: self.repository.list_states().await?,
        })
    }

    /// Ensures sync operations can call Supabase.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` when sync is enabled.
    fn ensure_enabled(&self, settings: &SyncSettings) -> Result<(), SyncError> {
        if settings.enabled {
            Ok(())
        } else {
            Err(SyncError::Disabled)
        }
    }

    /// Runs one complete bidirectional table synchronization.
    ///
    /// # Returns
    ///
    /// Returns the number of rows written to each side.
    async fn sync_tables(&self) -> Result<TableSyncSummary, SyncError> {
        let settings = self.settings();
        self.ensure_enabled(&settings)?;
        let supabase = SupabaseClient::from_settings(&settings);
        self.ensure_schema_contract_ready(&supabase).await?;
        let (local, remote) = self.read_snapshots(&supabase).await?;
        let diff = diff_snapshots(&local, &remote);
        self.ensure_no_conflicts(&diff.conflicts)?;

        let write_local = remote.subset_for_diffs(&diff.write_local);
        let write_remote = local.subset_for_diffs(&diff.write_remote);
        let pulled = write_local.row_count();
        let pushed = write_remote.row_count();

        self.repository
            .write_local_table_snapshot(&write_local)
            .await?;
        self.write_remote_snapshot(&supabase, &write_remote).await?;
        self.verify_converged(&supabase).await?;

        Ok(TableSyncSummary { pushed, pulled })
    }

    /// Reads local and Supabase synchronized table snapshots.
    ///
    /// # Arguments
    ///
    /// * `supabase` - Supabase REST client.
    ///
    /// # Returns
    ///
    /// Returns local and remote snapshots.
    async fn read_snapshots(
        &self,
        supabase: &SupabaseClient,
    ) -> Result<
        (
            crate::sync::table_snapshot::SyncTableSnapshot,
            crate::sync::table_snapshot::SyncTableSnapshot,
        ),
        SyncError,
    > {
        let local = self.repository.read_local_table_snapshot().await?;
        let remote = supabase.fetch_table_snapshot().await?;
        Ok((local, remote))
    }

    /// Ensures local and remote schema contracts are ready for table sync.
    ///
    /// # Arguments
    ///
    /// * `supabase` - Supabase REST client.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` when both sides are at the target contract version.
    async fn ensure_schema_contract_ready(
        &self,
        supabase: &SupabaseClient,
    ) -> Result<(), SyncError> {
        let local = self
            .repository
            .local_schema_contract_version()
            .await?
            .unwrap_or_else(|| "missing".to_string());
        let remote = supabase
            .fetch_schema_contract_version()
            .await?
            .unwrap_or_else(|| "missing".to_string());
        let versions = SchemaContractVersions { local, remote };

        if versions.is_ready() {
            Ok(())
        } else {
            Err(SyncError::SchemaContractMismatch {
                local: versions.local,
                remote: versions.remote,
                expected: TARGET_SCHEMA_CONTRACT_VERSION.to_string(),
            })
        }
    }

    /// Writes a partial snapshot to Supabase and records push failures locally.
    ///
    /// # Arguments
    ///
    /// * `supabase` - Supabase REST client.
    /// * `snapshot` - Rows to write.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after Supabase accepts the writes.
    async fn write_remote_snapshot(
        &self,
        supabase: &SupabaseClient,
        snapshot: &crate::sync::table_snapshot::SyncTableSnapshot,
    ) -> Result<(), SyncError> {
        if let Err(error) = supabase.upsert_table_snapshot(snapshot).await {
            let message = error.to_string();
            self.repository.record_error("push", &message).await?;
            return Err(error.into());
        }
        Ok(())
    }

    /// Verifies that local and remote snapshots have no remaining differences.
    ///
    /// # Arguments
    ///
    /// * `supabase` - Supabase REST client.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` when both sides match.
    async fn verify_converged(&self, supabase: &SupabaseClient) -> Result<(), SyncError> {
        let (local, remote) = self.read_snapshots(supabase).await?;
        let diff = diff_snapshots(&local, &remote);
        if diff.write_local.is_empty() && diff.write_remote.is_empty() && diff.conflicts.is_empty()
        {
            Ok(())
        } else {
            Err(SyncError::NotConverged {
                write_local: diff.write_local.len(),
                write_remote: diff.write_remote.len(),
                conflicts: diff.conflicts.len(),
            })
        }
    }

    /// Verifies that one synchronization direction has no remaining work.
    ///
    /// # Arguments
    ///
    /// * `supabase` - Supabase REST client.
    /// * `direction` - Direction that was just written.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` when that direction has converged.
    async fn verify_direction_converged(
        &self,
        supabase: &SupabaseClient,
        direction: SyncDirection,
    ) -> Result<(), SyncError> {
        let (local, remote) = self.read_snapshots(supabase).await?;
        let diff = diff_snapshots(&local, &remote);
        let remaining = match direction {
            SyncDirection::Push => diff.write_remote.len(),
            SyncDirection::Pull => diff.write_local.len(),
        };

        if remaining == 0 && diff.conflicts.is_empty() {
            Ok(())
        } else {
            Err(SyncError::NotConverged {
                write_local: diff.write_local.len(),
                write_remote: diff.write_remote.len(),
                conflicts: diff.conflicts.len(),
            })
        }
    }

    /// Stops synchronization when differences cannot be resolved safely.
    ///
    /// # Arguments
    ///
    /// * `conflicts` - Conflicts produced by snapshot comparison.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` when there are no conflicts.
    fn ensure_no_conflicts(&self, conflicts: &[SyncDiffConflict]) -> Result<(), SyncError> {
        if conflicts.is_empty() {
            return Ok(());
        }

        Err(SyncError::Conflict {
            count: conflicts.len(),
            first: conflicts
                .first()
                .map(|conflict| conflict.reason.clone())
                .unwrap_or_else(|| "unknown conflict".to_string()),
        })
    }
}

/// Direction for post-write convergence checks.
#[derive(Debug, Clone, Copy)]
enum SyncDirection {
    /// Local SQLite to Supabase.
    Push,
    /// Supabase to local SQLite.
    Pull,
}

/// Summary for one bidirectional table synchronization.
#[derive(Debug, Clone)]
struct TableSyncSummary {
    /// Rows written to Supabase.
    pushed: usize,
    /// Rows written to local SQLite.
    pulled: usize,
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
    /// Snapshot comparison found unresolved conflicts.
    #[error("synchronization conflict count {count}: {first}")]
    Conflict {
        /// Number of unresolved conflicts.
        count: usize,
        /// First conflict reason.
        first: String,
    },
    /// Post-write verification found remaining differences.
    #[error(
        "synchronization did not converge: write_local={write_local}, write_remote={write_remote}, conflicts={conflicts}"
    )]
    NotConverged {
        /// Remaining rows that would be written to local.
        write_local: usize,
        /// Remaining rows that would be written to remote.
        write_remote: usize,
        /// Remaining conflict count.
        conflicts: usize,
    },
    /// Local and remote schema contracts do not match the backend target version.
    #[error("schema contract mismatch: local={local}, remote={remote}, expected={expected}")]
    SchemaContractMismatch {
        /// Local SQLite schema contract version.
        local: String,
        /// Remote Supabase/Postgres schema contract version.
        remote: String,
        /// Expected contract version.
        expected: String,
    },
}
