use zembra_backend_rust::cli::{CliAction, ConfigInitOptions, ServiceInitOptions, parse_cli_args};

#[test]
fn empty_args_start_server() {
    let action = parse_cli_args(["zembra-backend"]).expect("empty args should parse");

    assert_eq!(action, CliAction::Serve);
}

#[test]
fn init_service_args_parse_default_options() {
    let action =
        parse_cli_args(["zembra-backend", "init", "service"]).expect("init service should parse");

    assert_eq!(
        action,
        CliAction::InitService(ServiceInitOptions {
            start: false,
            force: false,
        })
    );
}

#[test]
fn init_args_parse_global_initialization() {
    let action = parse_cli_args(["zembra-backend", "init"]).expect("global init should parse");

    assert_eq!(action, CliAction::Init);
}

#[test]
fn init_service_args_parse_start_and_force_options() {
    let action = parse_cli_args(["zembra-backend", "init", "service", "--start", "--force"])
        .expect("init service options should parse");

    assert_eq!(
        action,
        CliAction::InitService(ServiceInitOptions {
            start: true,
            force: true,
        })
    );
}

#[test]
fn config_init_args_parse_default_options() {
    let action =
        parse_cli_args(["zembra-backend", "config", "init"]).expect("config init should parse");

    assert_eq!(
        action,
        CliAction::InitConfig(ConfigInitOptions { force: false })
    );
}

#[test]
fn config_init_args_parse_force_option() {
    let action = parse_cli_args(["zembra-backend", "config", "init", "--force"])
        .expect("config init force should parse");

    assert_eq!(
        action,
        CliAction::InitConfig(ConfigInitOptions { force: true })
    );
}

#[test]
fn unknown_args_return_error() {
    let error = parse_cli_args(["zembra-backend", "unknown"]).expect_err("unknown command fails");

    assert!(error.to_string().contains("unsupported command"));
}
