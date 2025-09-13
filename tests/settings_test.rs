#[path = "../src/settings.rs"]
mod settings;

use settings::load_settings;
use std::env;
use std::fs;
use tempfile::tempdir;

#[test]
fn default_when_missing() {
    let tmp = tempdir().unwrap();
    let original = env::var("AGENTSANDBOX_CONFIG_HOME").ok();
    env::set_var("AGENTSANDBOX_CONFIG_HOME", tmp.path());

    let settings = load_settings().unwrap();
    assert_eq!(settings.auto_remove_minutes, Some(60));
    assert_eq!(
        settings
            .skip_permission_flags
            .get("claude")
            .map(String::as_str),
        Some("--dangerously-skip-permissions")
    );
    assert_eq!(
        settings
            .skip_permission_flags
            .get("gemini")
            .map(String::as_str),
        Some("--yolo")
    );
    assert_eq!(
        settings
            .skip_permission_flags
            .get("qwen")
            .map(String::as_str),
        Some("--yolo")
    );
    assert_eq!(
        settings.env_files,
        vec![
            ".env",
            ".env.local",
            ".env.development.local",
            ".env.test.local",
            ".env.production.local",
        ]
    );

    if let Some(val) = original {
        env::set_var("AGENTSANDBOX_CONFIG_HOME", val);
    } else {
        env::remove_var("AGENTSANDBOX_CONFIG_HOME");
    }
}

#[test]
fn read_from_file() {
    let tmp = tempdir().unwrap();
    let config_dir = tmp.path();
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("settings.json"),
        r#"{ "auto_remove_minutes": 30, "skip_permission_flags": { "claude": "--dangerously-skip-permissions" }, "env_files": [".custom.env"] }"#,
    )
    .unwrap();

    let original = env::var("AGENTSANDBOX_CONFIG_HOME").ok();
    env::set_var("AGENTSANDBOX_CONFIG_HOME", config_dir);

    let settings = load_settings().unwrap();
    assert_eq!(settings.auto_remove_minutes, Some(30));
    assert_eq!(
        settings
            .skip_permission_flags
            .get("claude")
            .map(String::as_str),
        Some("--dangerously-skip-permissions")
    );
    assert_eq!(settings.env_files, vec![".custom.env".to_string()]);

    if let Some(val) = original {
        env::set_var("AGENTSANDBOX_CONFIG_HOME", val);
    } else {
        env::remove_var("AGENTSANDBOX_CONFIG_HOME");
    }
}
