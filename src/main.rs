use zembra_backend_rust::{cli, config_init, error, init, server, service_init};

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
        cli::CliAction::Init => {
            let config = init::GlobalInitConfig::from_current_process()?;
            match init::init_global(&config).await? {
                init::GlobalInit::Initialized => {
                    println!(
                        "initialized database at {} and configuration at {}",
                        config.default_database_path().display(),
                        config.config_path().display()
                    );
                }
                init::GlobalInit::Skipped => {
                    println!("database and configuration already exist; skipping initialization");
                }
            }
            Ok(())
        }
        cli::CliAction::InitService(options) => {
            let config = service_init::ServiceInitConfig::from_current_process()?;
            let runner = service_init::SystemCommandRunner;
            service_init::init_service(&config, options, &runner)?;
            Ok(())
        }
        cli::CliAction::InitConfig(options) => {
            let config = config_init::UserConfigInit::from_current_process()?;
            config_init::init_user_config(&config, options)?;
            Ok(())
        }
    }
}
