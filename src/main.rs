use zembra_backend_rust::{cli, error, server, service_init};

/// Dispatches the Zembra backend command line entry point.
///
/// # Returns
///
/// Returns `Ok(())` when the selected command exits cleanly, or an application
/// error when argument parsing, service initialization, configuration loading,
/// socket binding, or server execution fails.
#[tokio::main]
async fn main() -> Result<(), error::AppError> {
    match cli::parse_cli_args(std::env::args())? {
        cli::CliAction::Serve => server::run().await,
        cli::CliAction::InitService(options) => {
            let config = service_init::ServiceInitConfig::from_current_process()?;
            let runner = service_init::SystemCommandRunner;
            service_init::init_service(&config, options, &runner)?;
            Ok(())
        }
    }
}
