//! Global user initialization for configuration and SQLite database.

use std::collections::VecDeque;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Mutex;

use crate::config_init::{ConfigInitError, UserConfigInit, init_user_config_with_database_path};
use crate::repositories::database::Database;
use crate::repositories::workspaces::validate_workspace_name;

const MAX_WORKSPACE_NAME_ATTEMPTS: usize = 3;

/// Runtime inputs used to initialize the current user's local state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalInitConfig {
    /// Current user's home directory.
    pub home_dir: PathBuf,
}

/// Result of a global initialization run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalInit {
    /// Initialization created or completed local state.
    Initialized,
    /// Both default database and config file already existed.
    Skipped,
}

/// Error returned by global initialization.
#[derive(Debug, thiserror::Error)]
pub enum GlobalInitError {
    /// HOME is unavailable.
    #[error("HOME is not set")]
    MissingHome,
    /// User configuration initialization failed.
    #[error("{0}")]
    Config(#[from] ConfigInitError),
    /// SQLite database creation or migration failed.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    /// Reading workspace name input failed.
    #[error("workspace name input error: {0}")]
    WorkspaceNameInput(#[from] io::Error),
    /// Workspace name was missing or invalid after all allowed prompts.
    #[error("workspace name is required and must not contain whitespace")]
    InvalidWorkspaceName,
}

/// Source of workspace name input used by initialization.
pub trait WorkspaceNameInput {
    /// Reads one workspace name attempt.
    ///
    /// # Arguments
    ///
    /// * `attempt` - One-based attempt number shown to interactive users.
    ///
    /// # Returns
    ///
    /// Returns the raw workspace name attempt.
    fn read_workspace_name(&self, attempt: usize) -> Result<String, io::Error>;
}

/// Workspace name input backed by standard input.
#[derive(Debug, Clone, Copy, Default)]
pub struct StdinWorkspaceNameInput;

impl WorkspaceNameInput for StdinWorkspaceNameInput {
    /// Reads one workspace name attempt from standard input.
    ///
    /// # Arguments
    ///
    /// * `attempt` - One-based attempt number shown in the prompt.
    ///
    /// # Returns
    ///
    /// Returns the entered workspace name without trailing line endings.
    fn read_workspace_name(&self, attempt: usize) -> Result<String, io::Error> {
        print!("Workspace name ({attempt}/{MAX_WORKSPACE_NAME_ATTEMPTS}): ");
        io::stdout().flush()?;

        let mut workspace_name = String::new();
        io::stdin().read_line(&mut workspace_name)?;

        Ok(workspace_name.trim_end_matches(['\r', '\n']).to_string())
    }
}

/// Workspace name input backed by a fixed list of values.
#[derive(Debug, Default)]
pub struct StaticWorkspaceNameInput {
    /// Remaining workspace name attempts returned to initialization.
    values: Mutex<VecDeque<String>>,
}

impl StaticWorkspaceNameInput {
    /// Builds a static workspace name input source.
    ///
    /// # Arguments
    ///
    /// * `values` - Ordered workspace name attempts returned by the input source.
    ///
    /// # Returns
    ///
    /// Returns a static input source for tests and non-interactive callers.
    pub fn new<I, S>(values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            values: Mutex::new(values.into_iter().map(Into::into).collect()),
        }
    }
}

impl WorkspaceNameInput for StaticWorkspaceNameInput {
    /// Returns the next static workspace name attempt.
    ///
    /// # Arguments
    ///
    /// * `_attempt` - One-based attempt number, ignored by static input.
    ///
    /// # Returns
    ///
    /// Returns the next configured value, or an empty string when exhausted.
    fn read_workspace_name(&self, _attempt: usize) -> Result<String, io::Error> {
        Ok(self
            .values
            .lock()
            .expect("workspace name input lock")
            .pop_front()
            .unwrap_or_default())
    }
}

impl GlobalInitConfig {
    /// Builds global initialization inputs from the current process.
    ///
    /// # Returns
    ///
    /// Returns process-derived global initialization inputs.
    pub fn from_current_process() -> Result<Self, GlobalInitError> {
        let home_dir = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or(GlobalInitError::MissingHome)?;

        Ok(Self { home_dir })
    }

    /// Returns the default SQLite database file path.
    ///
    /// # Returns
    ///
    /// Returns `~/.zembra/zembra.sqlite3`.
    pub fn default_database_path(&self) -> PathBuf {
        self.home_dir.join(".zembra/zembra.sqlite3")
    }

    /// Returns the current user's configuration file path.
    ///
    /// # Returns
    ///
    /// Returns `~/.zembra.env`.
    pub fn config_path(&self) -> PathBuf {
        self.home_dir.join(".zembra.env")
    }
}

/// Initializes the default SQLite database and user configuration.
///
/// # Arguments
///
/// * `config` - Global initialization inputs.
///
/// # Returns
///
/// Returns whether initialization ran or was skipped.
pub async fn init_global(config: &GlobalInitConfig) -> Result<GlobalInit, GlobalInitError> {
    init_global_with_workspace_name_input(config, &StdinWorkspaceNameInput).await
}

/// Initializes the default SQLite database and user configuration with injectable workspace input.
///
/// # Arguments
///
/// * `config` - Global initialization inputs.
/// * `workspace_name_input` - Source used when a new database needs an initial workspace name.
///
/// # Returns
///
/// Returns whether initialization ran or was skipped.
pub async fn init_global_with_workspace_name_input(
    config: &GlobalInitConfig,
    workspace_name_input: &impl WorkspaceNameInput,
) -> Result<GlobalInit, GlobalInitError> {
    let database_path = config.default_database_path();
    let config_path = config.config_path();

    if database_path.exists() && config_path.exists() {
        return Ok(GlobalInit::Skipped);
    }

    let should_name_initial_workspace = !database_path.exists();
    let workspace_name = if should_name_initial_workspace {
        Some(read_valid_workspace_name(workspace_name_input)?)
    } else {
        None
    };

    let database_url = format!("sqlite://{}", database_path.display());
    let database = Database::connect(&database_url).await?;
    if let Some(workspace_name) = workspace_name {
        database
            .assign_initial_workspace_name(&workspace_name)
            .await?;
    }

    let user_config = UserConfigInit {
        home_dir: config.home_dir.clone(),
    };
    init_user_config_with_database_path(&user_config, &database_path)?;

    Ok(GlobalInit::Initialized)
}

/// Reads and validates a workspace name with the allowed retry limit.
///
/// # Arguments
///
/// * `workspace_name_input` - Source used to read workspace name attempts.
///
/// # Returns
///
/// Returns a valid workspace name or an initialization error.
fn read_valid_workspace_name(
    workspace_name_input: &impl WorkspaceNameInput,
) -> Result<String, GlobalInitError> {
    for attempt in 1..=MAX_WORKSPACE_NAME_ATTEMPTS {
        let workspace_name = workspace_name_input.read_workspace_name(attempt)?;
        if let Some(workspace_name) = validate_workspace_name(&workspace_name) {
            return Ok(workspace_name);
        }
    }

    Err(GlobalInitError::InvalidWorkspaceName)
}
