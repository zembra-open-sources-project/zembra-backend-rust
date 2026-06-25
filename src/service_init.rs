//! Current-user daemon and service initialization.

use std::cell::RefCell;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Options accepted by `zembra-backend init service`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ServiceInitOptions {
    /// Whether to enable and start the user service after initialization.
    pub start: bool,
    /// Whether generated files may overwrite existing files.
    pub force: bool,
}

/// Platform family used by service initialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// macOS with Homebrew-managed user service lifecycle.
    Macos,
    /// Linux with systemd user service lifecycle.
    Linux,
    /// Unsupported platform.
    Unsupported,
}

/// Runtime inputs used to initialize service files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceInitConfig {
    /// Current operating system platform.
    pub platform: Platform,
    /// Current user's home directory.
    pub home_dir: PathBuf,
    /// Optional XDG data directory override.
    pub xdg_data_home: Option<PathBuf>,
    /// Optional XDG state directory override.
    pub xdg_state_home: Option<PathBuf>,
    /// Optional XDG config directory override.
    pub xdg_config_home: Option<PathBuf>,
    /// Absolute path to the running executable.
    pub executable_path: PathBuf,
}

/// Resolved Linux service paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxServicePaths {
    /// Directory containing the SQLite database.
    pub data_dir: PathBuf,
    /// Directory containing service log files.
    pub log_dir: PathBuf,
    /// Path to the generated systemd user unit.
    pub unit_path: PathBuf,
    /// Path to the user configuration file.
    pub config_path: PathBuf,
}

/// Error returned by service initialization.
#[derive(Debug, thiserror::Error)]
pub enum ServiceInitError {
    /// HOME is unavailable.
    #[error("HOME is not set")]
    MissingHome,
    /// Current executable path is unavailable.
    #[error("current executable path is unavailable: {0}")]
    CurrentExe(std::io::Error),
    /// File system operation failed.
    #[error("file operation failed at {path}: {source}")]
    Io {
        /// Path involved in the failed file operation.
        path: PathBuf,
        /// Original I/O error.
        source: std::io::Error,
    },
    /// Platform is unsupported.
    #[error("service initialization is only supported on macOS and Linux")]
    UnsupportedPlatform,
    /// systemctl command failed.
    #[error("command failed: {program} {args:?}")]
    CommandFailed {
        /// Program that failed.
        program: String,
        /// Arguments passed to the program.
        args: Vec<String>,
    },
    /// Required path is not absolute.
    #[error("path must be absolute: {0}")]
    NonAbsolutePath(PathBuf),
}

/// Abstraction for executing external commands.
pub trait CommandRunner {
    /// Runs an external command.
    ///
    /// # Arguments
    ///
    /// * `program` - Program name.
    /// * `args` - Program arguments.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` when the command exits successfully.
    fn run(&self, program: &str, args: &[&str]) -> Result<(), ServiceInitError>;
}

/// Production command runner.
#[derive(Debug, Clone, Copy)]
pub struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
    /// Runs an external command and checks its exit status.
    ///
    /// # Arguments
    ///
    /// * `program` - Program name.
    /// * `args` - Program arguments.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` when the command exits successfully.
    fn run(&self, program: &str, args: &[&str]) -> Result<(), ServiceInitError> {
        let status = Command::new(program)
            .args(args)
            .status()
            .map_err(|source| ServiceInitError::Io {
                path: PathBuf::from(program),
                source,
            })?;

        if status.success() {
            Ok(())
        } else {
            Err(ServiceInitError::CommandFailed {
                program: program.to_string(),
                args: args.iter().map(|arg| (*arg).to_string()).collect(),
            })
        }
    }
}

/// Test command runner that records commands without executing them.
#[derive(Debug, Default)]
pub struct CapturedCommandRunner {
    commands: RefCell<Vec<Vec<String>>>,
}

impl CapturedCommandRunner {
    /// Returns the commands captured by this runner.
    ///
    /// # Returns
    ///
    /// Returns a copy of captured command tokens.
    pub fn commands(&self) -> Vec<Vec<String>> {
        self.commands.borrow().clone()
    }
}

impl CommandRunner for CapturedCommandRunner {
    /// Records an external command without executing it.
    ///
    /// # Arguments
    ///
    /// * `program` - Program name.
    /// * `args` - Program arguments.
    ///
    /// # Returns
    ///
    /// Always returns `Ok(())`.
    fn run(&self, program: &str, args: &[&str]) -> Result<(), ServiceInitError> {
        let mut command = Vec::with_capacity(args.len() + 1);
        command.push(program.to_string());
        command.extend(args.iter().map(|arg| (*arg).to_string()));
        self.commands.borrow_mut().push(command);
        Ok(())
    }
}

impl ServiceInitConfig {
    /// Builds service initialization config from the current process.
    ///
    /// # Returns
    ///
    /// Returns process-derived service initialization config.
    pub fn from_current_process() -> Result<Self, ServiceInitError> {
        let platform = current_platform();
        let home_dir = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or(ServiceInitError::MissingHome)?;
        let executable_path = std::env::current_exe().map_err(ServiceInitError::CurrentExe)?;

        Ok(Self {
            platform,
            home_dir,
            xdg_data_home: std::env::var_os("XDG_DATA_HOME").map(PathBuf::from),
            xdg_state_home: std::env::var_os("XDG_STATE_HOME").map(PathBuf::from),
            xdg_config_home: std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
            executable_path,
        })
    }
}

/// Initializes current-user service files and optionally starts the service.
///
/// # Arguments
///
/// * `config` - Platform and path inputs.
/// * `options` - Service initialization options.
/// * `runner` - External command runner.
///
/// # Returns
///
/// Returns `Ok(())` after files are initialized and optional start actions run.
pub fn init_service(
    config: &ServiceInitConfig,
    options: ServiceInitOptions,
    runner: &impl CommandRunner,
) -> Result<(), ServiceInitError> {
    match config.platform {
        Platform::Linux => init_linux_service(config, options, runner),
        Platform::Macos => init_macos_service(config, options),
        Platform::Unsupported => Err(ServiceInitError::UnsupportedPlatform),
    }
}

/// Resolves Linux user service paths.
///
/// # Arguments
///
/// * `config` - Platform and path inputs.
///
/// # Returns
///
/// Returns XDG-aligned Linux service paths.
pub fn linux_service_paths(
    config: &ServiceInitConfig,
) -> Result<LinuxServicePaths, ServiceInitError> {
    let data_home = config
        .xdg_data_home
        .clone()
        .unwrap_or_else(|| config.home_dir.join(".local/share"));
    let state_home = config
        .xdg_state_home
        .clone()
        .unwrap_or_else(|| config.home_dir.join(".local/state"));
    let config_home = config
        .xdg_config_home
        .clone()
        .unwrap_or_else(|| config.home_dir.join(".config"));

    Ok(LinuxServicePaths {
        data_dir: data_home.join("zembra"),
        log_dir: state_home.join("zembra/logs"),
        unit_path: config_home.join("systemd/user/zembra-backend.service"),
        config_path: config.home_dir.join(".zembra.env"),
    })
}

/// Renders the systemd user unit.
///
/// # Arguments
///
/// * `paths` - Resolved Linux service paths.
/// * `executable_path` - Absolute path to `zembra-backend`.
///
/// # Returns
///
/// Returns the systemd unit file content.
pub fn render_systemd_user_unit(
    paths: &LinuxServicePaths,
    executable_path: &Path,
) -> Result<String, ServiceInitError> {
    if !executable_path.is_absolute() {
        return Err(ServiceInitError::NonAbsolutePath(
            executable_path.to_path_buf(),
        ));
    }

    Ok(format!(
        "[Unit]\nDescription=Zembra backend service\n\n[Service]\nExecStart={}\nWorkingDirectory={}\nRestart=on-failure\n\n[Install]\nWantedBy=default.target\n",
        DisplayPath(executable_path),
        DisplayPath(&paths.data_dir)
    ))
}

/// Initializes Linux systemd user service files.
///
/// # Arguments
///
/// * `config` - Platform and path inputs.
/// * `options` - Service initialization options.
/// * `runner` - External command runner.
///
/// # Returns
///
/// Returns `Ok(())` after Linux initialization completes.
fn init_linux_service(
    config: &ServiceInitConfig,
    options: ServiceInitOptions,
    runner: &impl CommandRunner,
) -> Result<(), ServiceInitError> {
    let paths = linux_service_paths(config)?;
    create_dir_all(&paths.data_dir)?;
    create_dir_all(&paths.log_dir)?;
    create_parent_dir(&paths.config_path)?;
    create_parent_dir(&paths.unit_path)?;
    write_file_if_allowed(
        &paths.config_path,
        &render_user_config(&paths),
        options.force,
    )?;
    write_file_if_allowed(
        &paths.unit_path,
        &render_systemd_user_unit(&paths, &config.executable_path)?,
        options.force,
    )?;

    if options.start {
        runner.run("systemctl", &["--user", "daemon-reload"])?;
        runner.run("systemctl", &["--user", "enable", "zembra-backend"])?;
        runner.run("systemctl", &["--user", "start", "zembra-backend"])?;
    }

    Ok(())
}

/// Initializes macOS user configuration without managing Homebrew services.
///
/// # Arguments
///
/// * `config` - Platform and path inputs.
/// * `options` - Service initialization options.
///
/// # Returns
///
/// Returns `Ok(())` after macOS initialization completes.
fn init_macos_service(
    config: &ServiceInitConfig,
    options: ServiceInitOptions,
) -> Result<(), ServiceInitError> {
    let paths = LinuxServicePaths {
        data_dir: config.home_dir.join("Library/Application Support/Zembra"),
        log_dir: config.home_dir.join("Library/Logs/Zembra"),
        unit_path: config
            .home_dir
            .join("Library/LaunchAgents/zembra-backend.plist"),
        config_path: config.home_dir.join(".zembra.env"),
    };

    create_dir_all(&paths.data_dir)?;
    create_dir_all(&paths.log_dir)?;
    create_parent_dir(&paths.config_path)?;
    write_file_if_allowed(
        &paths.config_path,
        &render_user_config(&paths),
        options.force,
    )
}

/// Renders daemon-friendly user configuration.
///
/// # Arguments
///
/// * `paths` - Resolved service paths.
///
/// # Returns
///
/// Returns TOML configuration content.
fn render_user_config(paths: &LinuxServicePaths) -> String {
    format!(
        "[server]\nhost = \"127.0.0.1\"\nport = 3000\ncors_allowed_origins = []\n\n[database]\npath = \"{}\"\n\n[logging]\nlevel = \"INFO\"\npath = \"{}\"\n\n[sync]\nenabled = false\ninterval_seconds = 60\nsupabase_url = \"\"\nsecret_key = \"\"\nremote_database_password = \"\"\n",
        escape_toml_string(&paths.data_dir.join("zembra.db")),
        escape_toml_string(&paths.log_dir)
    )
}

/// Creates a directory and maps errors with path context.
///
/// # Arguments
///
/// * `path` - Directory path to create.
///
/// # Returns
///
/// Returns `Ok(())` when the directory exists.
fn create_dir_all(path: &Path) -> Result<(), ServiceInitError> {
    fs::create_dir_all(path).map_err(|source| ServiceInitError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Creates a file's parent directory.
///
/// # Arguments
///
/// * `path` - File path whose parent should exist.
///
/// # Returns
///
/// Returns `Ok(())` when the parent directory exists or no parent is needed.
fn create_parent_dir(path: &Path) -> Result<(), ServiceInitError> {
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }
    Ok(())
}

/// Writes a generated file when overwrite rules allow it.
///
/// # Arguments
///
/// * `path` - File path to write.
/// * `content` - File content.
/// * `force` - Whether existing files may be overwritten.
///
/// # Returns
///
/// Returns `Ok(())` when the file exists with desired content or was preserved.
fn write_file_if_allowed(path: &Path, content: &str, force: bool) -> Result<(), ServiceInitError> {
    if path.exists() && !force {
        return Ok(());
    }

    fs::write(path, content).map_err(|source| ServiceInitError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Returns the current supported platform.
///
/// # Returns
///
/// Returns macOS, Linux, or unsupported.
fn current_platform() -> Platform {
    if cfg!(target_os = "macos") {
        Platform::Macos
    } else if cfg!(target_os = "linux") {
        Platform::Linux
    } else {
        Platform::Unsupported
    }
}

/// Escapes a path for use in a TOML string.
///
/// # Arguments
///
/// * `path` - Path to display in TOML.
///
/// # Returns
///
/// Returns escaped string content.
fn escape_toml_string(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

/// Display adapter for filesystem paths.
struct DisplayPath<'a>(&'a Path);

impl fmt::Display for DisplayPath<'_> {
    /// Formats a path using lossless display where possible.
    ///
    /// # Arguments
    ///
    /// * `formatter` - Output formatter.
    ///
    /// # Returns
    ///
    /// Returns the formatter result.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0.display())
    }
}
