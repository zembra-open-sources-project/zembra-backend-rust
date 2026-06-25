use std::fs;
use std::path::{Path, PathBuf};

use zembra_backend_rust::config_init::{
    ConfigInitOptions, UserConfigInit, init_user_config, render_documented_user_config,
};

fn test_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "zembra-config-init-{}-{}",
        name,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("test root should be created");
    path
}

fn config(root: &Path) -> UserConfigInit {
    UserConfigInit {
        home_dir: root.join("home"),
    }
}

#[test]
fn documented_config_contains_one_comment_before_each_field() {
    let content = render_documented_user_config();

    for field in [
        "host",
        "port",
        "cors_allowed_origins",
        "path",
        "level",
        "enabled",
        "interval_seconds",
        "supabase_url",
        "secret_key",
        "remote_database_password",
    ] {
        let field_line = content
            .lines()
            .position(|line| line.trim_start().starts_with(&format!("{field} =")))
            .unwrap_or_else(|| panic!("{field} field should exist"));
        let previous_line = content
            .lines()
            .nth(field_line.saturating_sub(1))
            .expect("field should have previous line");

        assert!(
            previous_line.trim_start().starts_with('#'),
            "{field} should have a TOML comment on the previous line"
        );
    }
}

#[test]
fn config_init_creates_zembra_env_with_documented_defaults() {
    let root = test_root("create");
    let path = init_user_config(&config(&root), ConfigInitOptions { force: false })
        .expect("config init should succeed");

    assert_eq!(path, root.join("home").join(".zembra.env"));

    let content = fs::read_to_string(path).expect("config should exist");
    assert!(content.contains("# HTTP server bind address."));
    assert!(content.contains("host = \"127.0.0.1\""));
    assert!(content.contains("# SQLite database file path."));
    assert!(content.contains(&format!(
        "path = \"{}\"",
        root.join("home").join(".local/share/zembra/zembra.db").display()
    )));
    assert!(!content.contains("path = \"data/zembra.db\""));
    assert!(content.contains("# Supabase secret key used only by the local backend."));
    assert!(content.contains("secret_key = \"\""));
}

#[test]
fn config_init_does_not_overwrite_existing_file_without_force() {
    let root = test_root("preserve");
    let config = config(&root);
    let path = config.home_dir.join(".zembra.env");
    fs::create_dir_all(path.parent().unwrap()).expect("config parent should exist");
    fs::write(&path, "existing = true\n").expect("existing config should be written");

    init_user_config(&config, ConfigInitOptions { force: false })
        .expect("config init should succeed");

    assert_eq!(
        fs::read_to_string(path).expect("config should exist"),
        "existing = true\n"
    );
}

#[test]
fn config_init_overwrites_existing_file_with_force() {
    let root = test_root("force");
    let config = config(&root);
    let path = config.home_dir.join(".zembra.env");
    fs::create_dir_all(path.parent().unwrap()).expect("config parent should exist");
    fs::write(&path, "existing = true\n").expect("existing config should be written");

    init_user_config(&config, ConfigInitOptions { force: true })
        .expect("config init should succeed");

    let content = fs::read_to_string(path).expect("config should exist");
    assert!(content.contains("[server]"));
    assert!(!content.contains("existing = true"));
}
