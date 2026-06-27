use crate::config::SyncSettings;
use crate::repositories::sync::{SyncRepository, SyncStateRecord};
use crate::sync::diff::{SyncDiffActionKind, SyncDiffConflict, diff_snapshots};
use crate::sync::schema_contract::{SchemaContractVersions, TARGET_SCHEMA_CONTRACT_VERSION};
use crate::sync::schema_migration::{SchemaMigrationError, apply_remote_schema_contract};
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
        self.ensure_schema_contract_ready(&settings, &supabase)
            .await?;
        let (local, remote) = self.read_snapshots(&supabase).await?;
        let diff = diff_snapshots(&local, &remote);
        self.ensure_no_conflicts(&diff.conflicts)?;
        let outbound_snapshot = local.subset_for_actions(
            &diff.actions,
            &[
                SyncDiffActionKind::UpsertRemote,
                SyncDiffActionKind::SyncChangeRemote,
            ],
        );
        let processed = outbound_snapshot.row_count()
            + diff
                .actions
                .iter()
                .filter(|action| action.kind == SyncDiffActionKind::DeleteRemote)
                .count();
        self.write_remote_snapshot(&supabase, &outbound_snapshot)
            .await?;
        supabase
            .delete_remote_actions(&diff.actions, &remote)
            .await?;
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
        self.ensure_schema_contract_ready(&settings, &supabase)
            .await?;
        let (local, remote) = self.read_snapshots(&supabase).await?;
        let diff = diff_snapshots(&local, &remote);
        self.ensure_no_conflicts(&diff.conflicts)?;
        let inbound_snapshot = remote.subset_for_actions(
            &diff.actions,
            &[
                SyncDiffActionKind::UpsertLocal,
                SyncDiffActionKind::SyncChangeLocal,
            ],
        );
        let processed = inbound_snapshot.row_count()
            + diff
                .actions
                .iter()
                .filter(|action| action.kind == SyncDiffActionKind::DeleteLocal)
                .count();
        self.repository
            .write_local_table_snapshot(&inbound_snapshot)
            .await?;
        self.repository
            .delete_local_actions(&diff.actions, &local)
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
        self.ensure_schema_contract_ready(&settings, &supabase)
            .await?;
        let (local, remote) = self.read_snapshots(&supabase).await?;
        let diff = diff_snapshots(&local, &remote);
        self.ensure_no_conflicts(&diff.conflicts)?;

        let inbound_snapshot = remote.subset_for_actions(
            &diff.actions,
            &[
                SyncDiffActionKind::UpsertLocal,
                SyncDiffActionKind::SyncChangeLocal,
            ],
        );
        let outbound_snapshot = local.subset_for_actions(
            &diff.actions,
            &[
                SyncDiffActionKind::UpsertRemote,
                SyncDiffActionKind::SyncChangeRemote,
            ],
        );
        let pulled = inbound_snapshot.row_count();
        let pushed = outbound_snapshot.row_count();

        self.repository
            .write_local_table_snapshot(&inbound_snapshot)
            .await?;
        self.write_remote_snapshot(&supabase, &outbound_snapshot)
            .await?;
        self.repository
            .delete_local_actions(&diff.actions, &local)
            .await?;
        supabase
            .delete_remote_actions(&diff.actions, &remote)
            .await?;
        self.verify_converged(&supabase).await?;

        Ok(TableSyncSummary {
            pushed: pushed
                + diff
                    .actions
                    .iter()
                    .filter(|action| action.kind == SyncDiffActionKind::DeleteRemote)
                    .count(),
            pulled: pulled
                + diff
                    .actions
                    .iter()
                    .filter(|action| action.kind == SyncDiffActionKind::DeleteLocal)
                    .count(),
        })
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
        settings: &SyncSettings,
        supabase: &SupabaseClient,
    ) -> Result<(), SyncError> {
        let versions = self.read_schema_contract_versions(supabase).await?;

        if versions.is_ready() {
            return Ok(());
        }

        if !settings.remote_database_password.trim().is_empty() {
            apply_remote_schema_contract(
                &settings.supabase_url,
                &settings.remote_database_password,
            )
            .await?;
            let versions = self.read_schema_contract_versions(supabase).await?;
            if versions.is_ready() {
                return Ok(());
            }
        }

        Err(SyncError::SchemaContractMismatch {
            local: versions.local,
            remote: versions.remote,
            expected: TARGET_SCHEMA_CONTRACT_VERSION.to_string(),
        })
    }

    /// Reads local and remote schema contract versions.
    ///
    /// # Arguments
    ///
    /// * `supabase` - Supabase REST client.
    ///
    /// # Returns
    ///
    /// Returns both schema contract versions, using `missing` for absent version rows.
    async fn read_schema_contract_versions(
        &self,
        supabase: &SupabaseClient,
    ) -> Result<SchemaContractVersions, SyncError> {
        let local = self
            .repository
            .local_schema_contract_version()
            .await?
            .unwrap_or_else(|| "missing".to_string());
        let remote = supabase
            .fetch_schema_contract_version()
            .await?
            .unwrap_or_else(|| "missing".to_string());
        Ok(SchemaContractVersions { local, remote })
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
        if diff.actions.is_empty() && diff.conflicts.is_empty() {
            Ok(())
        } else {
            Err(SyncError::NotConverged {
                local_actions: diff
                    .actions
                    .iter()
                    .filter(|action| {
                        matches!(
                            action.kind,
                            SyncDiffActionKind::UpsertLocal
                                | SyncDiffActionKind::DeleteLocal
                                | SyncDiffActionKind::SyncChangeLocal
                        )
                    })
                    .count(),
                remote_actions: diff
                    .actions
                    .iter()
                    .filter(|action| {
                        matches!(
                            action.kind,
                            SyncDiffActionKind::UpsertRemote
                                | SyncDiffActionKind::DeleteRemote
                                | SyncDiffActionKind::SyncChangeRemote
                        )
                    })
                    .count(),
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
            SyncDirection::Push => diff
                .actions
                .iter()
                .filter(|action| {
                    matches!(
                        action.kind,
                        SyncDiffActionKind::UpsertRemote
                            | SyncDiffActionKind::DeleteRemote
                            | SyncDiffActionKind::SyncChangeRemote
                    )
                })
                .count(),
            SyncDirection::Pull => diff
                .actions
                .iter()
                .filter(|action| {
                    matches!(
                        action.kind,
                        SyncDiffActionKind::UpsertLocal
                            | SyncDiffActionKind::DeleteLocal
                            | SyncDiffActionKind::SyncChangeLocal
                    )
                })
                .count(),
        };

        if remaining == 0 && diff.conflicts.is_empty() {
            Ok(())
        } else {
            Err(SyncError::NotConverged {
                local_actions: diff
                    .actions
                    .iter()
                    .filter(|action| {
                        matches!(
                            action.kind,
                            SyncDiffActionKind::UpsertLocal
                                | SyncDiffActionKind::DeleteLocal
                                | SyncDiffActionKind::SyncChangeLocal
                        )
                    })
                    .count(),
                remote_actions: diff
                    .actions
                    .iter()
                    .filter(|action| {
                        matches!(
                            action.kind,
                            SyncDiffActionKind::UpsertRemote
                                | SyncDiffActionKind::DeleteRemote
                                | SyncDiffActionKind::SyncChangeRemote
                        )
                    })
                    .count(),
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
    /// Remote schema contract migration failed.
    #[error("{0}")]
    SchemaMigration(#[from] SchemaMigrationError),
    /// Snapshot comparison found unresolved conflicts.
    #[error("synchronization conflict count {count}")]
    Conflict {
        /// Number of unresolved conflicts.
        count: usize,
    },
    /// Post-write verification found remaining differences.
    #[error(
        "synchronization did not converge: local_actions={local_actions}, remote_actions={remote_actions}, conflicts={conflicts}"
    )]
    NotConverged {
        /// Remaining actions that target local SQLite.
        local_actions: usize,
        /// Remaining actions that target Supabase.
        remote_actions: usize,
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
