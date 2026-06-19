/// Target schema contract version consumed by this backend.
pub const TARGET_SCHEMA_CONTRACT_VERSION: &str = "0.5.0";

/// Local and remote schema contract versions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaContractVersions {
    /// Local SQLite schema contract version.
    pub local: String,
    /// Remote Supabase/Postgres schema contract version.
    pub remote: String,
}

impl SchemaContractVersions {
    /// Checks whether local and remote contract versions match the backend target.
    ///
    /// # Returns
    ///
    /// Returns `true` when both versions equal the target schema contract.
    pub fn is_ready(&self) -> bool {
        self.local == TARGET_SCHEMA_CONTRACT_VERSION
            && self.remote == TARGET_SCHEMA_CONTRACT_VERSION
    }
}
