//! Runtime version metadata exposed by the backend service.

use std::sync::OnceLock;

/// Version tracking information for the running backend build.
#[derive(Debug)]
pub struct VersionInfo {
    /// Cargo package version compiled into this binary.
    pub version: &'static str,
    /// Versioning policy declared in repository TOML metadata.
    pub version_policy: String,
    /// Release channel declared in repository TOML metadata.
    pub release_channel: String,
}

static VERSION_INFO: OnceLock<VersionInfo> = OnceLock::new();

/// Returns version metadata for the running backend build.
///
/// # Returns
///
/// Returns a stable reference to version metadata derived from repository TOML.
pub fn version_info() -> &'static VersionInfo {
    VERSION_INFO.get_or_init(|| VersionInfo {
        version: env!("CARGO_PKG_VERSION"),
        version_policy: read_metadata_value("version_policy"),
        release_channel: read_metadata_value("release_channel"),
    })
}

/// Reads a string value from `package.metadata.zembra` in the repository manifest.
///
/// # Arguments
///
/// * `key` - Metadata key to read from the Zembra package metadata table.
///
/// # Returns
///
/// Returns the configured string value, or an empty string when the key is absent or malformed.
fn read_metadata_value(key: &str) -> String {
    let manifest: toml::Value = include_str!("../Cargo.toml")
        .parse()
        .expect("Cargo.toml must be valid TOML");

    manifest
        .get("package")
        .and_then(|package| package.get("metadata"))
        .and_then(|metadata| metadata.get("zembra"))
        .and_then(|zembra| zembra.get(key))
        .and_then(toml::Value::as_str)
        .unwrap_or_default()
        .to_owned()
}
