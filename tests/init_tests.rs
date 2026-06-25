use std::fs;
use std::path::{Path, PathBuf};

use zembra_backend_rust::init::{GlobalInit, GlobalInitConfig, init_global};

fn test_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("zembra-init-{}-{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("test root should be created");
    path
}

fn config(root: &Path) -> GlobalInitConfig {
    GlobalInitConfig {
        home_dir: root.join("home"),
    }
}

#[tokio::test]
async fn global_init_creates_default_database_before_config_file() {
    let root = test_root("create");
    let result = init_global(&config(&root))
        .await
        .expect("global init should succeed");

    let database_path = root.join("home").join(".zembra/zembra.sqlite3");
    let config_path = root.join("home").join(".zembra.env");

    assert_eq!(result, GlobalInit::Initialized);
    assert!(database_path.exists());
    assert!(config_path.exists());

    let content = fs::read_to_string(config_path).expect("config should be readable");
    assert!(content.contains(&format!("path = \"{}\"", database_path.display())));
}

#[tokio::test]
async fn global_init_skips_when_database_and_config_file_exist() {
    let root = test_root("skip");
    let config = config(&root);
    let database_path = config.home_dir.join(".zembra/zembra.sqlite3");
    let config_path = config.home_dir.join(".zembra.env");
    fs::create_dir_all(database_path.parent().unwrap()).expect("database parent should exist");
    fs::write(&database_path, "existing database marker").expect("database marker should exist");
    fs::create_dir_all(config_path.parent().unwrap()).expect("config parent should exist");
    fs::write(&config_path, "existing = true\n").expect("config should exist");

    let result = init_global(&config)
        .await
        .expect("global init should succeed");

    assert_eq!(result, GlobalInit::Skipped);
    assert_eq!(
        fs::read_to_string(database_path).expect("database marker should remain"),
        "existing database marker"
    );
    assert_eq!(
        fs::read_to_string(config_path).expect("config should remain"),
        "existing = true\n"
    );
}
