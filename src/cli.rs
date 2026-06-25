//! Command line parsing for the Zembra backend binary.

pub use crate::service_init::ServiceInitOptions;

/// Top-level action selected from command line arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliAction {
    /// Start the HTTP server.
    Serve,
    /// Initialize current-user daemon or service files.
    InitService(ServiceInitOptions),
}

/// Parses command line arguments into an executable action.
///
/// # Arguments
///
/// * `args` - Full command line argument sequence including argv[0].
///
/// # Returns
///
/// Returns the selected CLI action, or an error string for unsupported
/// commands and options.
pub fn parse_cli_args<I, S>(args: I) -> Result<CliAction, crate::error::AppError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut args = args.into_iter().map(Into::into);
    let _program = args.next();
    let remaining = args.collect::<Vec<_>>();

    if remaining.is_empty() {
        return Ok(CliAction::Serve);
    }

    match remaining.as_slice() {
        [command, subcommand, options @ ..] if command == "init" && subcommand == "service" => {
            parse_service_options(options).map(CliAction::InitService)
        }
        [command, ..] => Err(crate::error::AppError::Cli(format!(
            "unsupported command: {command}"
        ))),
        [] => Ok(CliAction::Serve),
    }
}

/// Parses `init service` options.
///
/// # Arguments
///
/// * `options` - Option strings following `init service`.
///
/// # Returns
///
/// Returns service initialization options, or an error for unknown options.
fn parse_service_options(options: &[String]) -> Result<ServiceInitOptions, crate::error::AppError> {
    let mut parsed = ServiceInitOptions::default();

    for option in options {
        match option.as_str() {
            "--start" => parsed.start = true,
            "--force" => parsed.force = true,
            unknown => {
                return Err(crate::error::AppError::Cli(format!(
                    "unsupported init service option: {unknown}"
                )));
            }
        }
    }

    Ok(parsed)
}
