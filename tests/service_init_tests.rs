use std::fs;
use std::path::{Path, PathBuf};

use zembra_backend_rust::service_init::{
    CapturedCommandRunner, Platform, ServiceInitConfig, ServiceInitOptions, init_service,
    linux_service_paths, render_systemd_user_unit,
};

fn test_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "zembra-service-init-{}-{}",
        name,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("test root should be created");
    path
}

fn config(root: &Path) -> ServiceInitConfig {
    ServiceInitConfig {
        platform: Platform::Linux,
        home_dir: root.join("home"),
        xdg_data_home: None,
        xdg_state_home: None,
        xdg_config_home: None,
        executable_path: root.join("bin").join("zembra-backend"),
    }
}

#[test]
fn linux_paths_default_to_xdg_user_directories() {
    let root = test_root("xdg-defaults");
    let paths = linux_service_paths(&config(&root)).expect("paths should resolve");

    assert_eq!(
        paths.data_dir,
        root.join("home").join(".local/share/zembra")
    );
    assert_eq!(
        paths.log_dir,
        root.join("home").join(".local/state/zembra/logs")
    );
    assert_eq!(
        paths.unit_path,
        root.join("home")
            .join(".config/systemd/user/zembra-backend.service")
    );
    assert_eq!(paths.config_path, root.join("home").join(".zembra.env"));
}

#[test]
fn linux_paths_respect_xdg_environment_values() {
    let root = test_root("xdg-custom");
    let mut config = config(&root);
    config.xdg_data_home = Some(root.join("data-home"));
    config.xdg_state_home = Some(root.join("state-home"));
    config.xdg_config_home = Some(root.join("config-home"));

    let paths = linux_service_paths(&config).expect("paths should resolve");

    assert_eq!(paths.data_dir, root.join("data-home/zembra"));
    assert_eq!(paths.log_dir, root.join("state-home/zembra/logs"));
    assert_eq!(
        paths.unit_path,
        root.join("config-home/systemd/user/zembra-backend.service")
    );
}

#[test]
fn init_service_creates_config_and_user_unit_without_system_scope() {
    let root = test_root("create");
    let runner = CapturedCommandRunner::default();

    init_service(
        &config(&root),
        ServiceInitOptions {
            start: false,
            force: false,
        },
        &runner,
    )
    .expect("service init should succeed");

    let paths = linux_service_paths(&config(&root)).expect("paths should resolve");
    let config_content = fs::read_to_string(&paths.config_path).expect("config should exist");
    let unit_content = fs::read_to_string(&paths.unit_path).expect("unit should exist");

    assert!(paths.data_dir.is_dir());
    assert!(paths.log_dir.is_dir());
    assert!(config_content.contains("[database]"));
    assert!(config_content.contains("path = "));
    assert!(config_content.contains(paths.data_dir.to_str().unwrap()));
    assert!(config_content.contains(paths.log_dir.to_str().unwrap()));
    assert!(unit_content.contains("ExecStart="));
    assert!(unit_content.contains(config(&root).executable_path.to_str().unwrap()));
    assert!(!unit_content.contains("User="));
    assert!(!unit_content.contains("/etc/systemd/system"));
    assert!(!unit_content.contains("/var/lib"));
    assert!(!unit_content.contains("/var/log"));
    assert!(runner.commands().is_empty());
}

#[test]
fn init_service_does_not_overwrite_existing_config_without_force() {
    let root = test_root("no-overwrite");
    let config = config(&root);
    let paths = linux_service_paths(&config).expect("paths should resolve");
    fs::create_dir_all(paths.config_path.parent().unwrap()).expect("config parent should exist");
    fs::write(&paths.config_path, "existing = true\n").expect("existing config should be written");

    init_service(
        &config,
        ServiceInitOptions {
            start: false,
            force: false,
        },
        &CapturedCommandRunner::default(),
    )
    .expect("service init should succeed");

    assert_eq!(
        fs::read_to_string(&paths.config_path).expect("config should exist"),
        "existing = true\n"
    );
}

#[test]
fn init_service_overwrites_generated_files_with_force() {
    let root = test_root("force");
    let config = config(&root);
    let paths = linux_service_paths(&config).expect("paths should resolve");
    fs::create_dir_all(paths.config_path.parent().unwrap()).expect("config parent should exist");
    fs::write(&paths.config_path, "existing = true\n").expect("existing config should be written");

    init_service(
        &config,
        ServiceInitOptions {
            start: false,
            force: true,
        },
        &CapturedCommandRunner::default(),
    )
    .expect("service init should succeed");

    let content = fs::read_to_string(&paths.config_path).expect("config should exist");
    assert!(content.contains("[server]"));
    assert!(!content.contains("existing = true"));
}

#[test]
fn start_runs_systemd_user_commands_on_linux() {
    let root = test_root("start");
    let runner = CapturedCommandRunner::default();

    init_service(
        &config(&root),
        ServiceInitOptions {
            start: true,
            force: false,
        },
        &runner,
    )
    .expect("service init should succeed");

    assert_eq!(
        runner.commands(),
        vec![
            vec!["systemctl", "--user", "daemon-reload"],
            vec!["systemctl", "--user", "enable", "zembra-backend"],
            vec!["systemctl", "--user", "start", "zembra-backend"],
        ]
    );
}

#[test]
fn macos_start_does_not_run_brew_services() {
    let root = test_root("macos");
    let mut config = config(&root);
    config.platform = Platform::Macos;
    let runner = CapturedCommandRunner::default();

    init_service(
        &config,
        ServiceInitOptions {
            start: true,
            force: false,
        },
        &runner,
    )
    .expect("macOS init should succeed");

    assert!(runner.commands().is_empty());
}

#[test]
fn rendered_unit_uses_absolute_exec_start() {
    let root = test_root("unit");
    let paths = linux_service_paths(&config(&root)).expect("paths should resolve");

    let unit = render_systemd_user_unit(&paths, &config(&root).executable_path)
        .expect("unit should render");

    assert!(unit.contains("ExecStart="));
    assert!(unit.contains(config(&root).executable_path.to_str().unwrap()));
    assert!(unit.contains("WorkingDirectory="));
    assert!(unit.contains(paths.data_dir.to_str().unwrap()));
}
