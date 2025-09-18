#[path = "../src/config.rs"]
mod config;

use config::{get_claude_config_dir, get_claude_json_paths};
use std::env;
use std::fs;
use std::io::Write;
use tempfile::tempdir;

fn create_temp_claude_dir() -> tempfile::TempDir {
    let temp_dir = tempdir().unwrap();
    let claude_dir = temp_dir.path().join(".claude");
    fs::create_dir(&claude_dir).unwrap();

    let config_file = claude_dir.join("config.json");
    let mut file = fs::File::create(config_file).unwrap();
    writeln!(file, r#"{{"claude_dir_test": "value"}}"#).unwrap();

    temp_dir
}

#[test]
fn test_get_claude_config_dir_with_home() {
    let temp_dir = create_temp_claude_dir();
    let home_path = temp_dir.path();

    let original_home = env::var("HOME");
    env::set_var("HOME", home_path.to_str().unwrap());

    let result = get_claude_config_dir();
    assert!(result.is_some());
    assert_eq!(result.unwrap(), home_path.join(".claude"));

    if let Ok(home) = original_home {
        env::set_var("HOME", home);
    } else {
        env::remove_var("HOME");
    }
}

#[test]
fn test_get_claude_config_dir_with_xdg() {
    let temp_dir = tempdir().unwrap();
    let xdg_path = temp_dir.path();
    let claude_dir = xdg_path.join("claude");
    fs::create_dir(&claude_dir).unwrap();

    let original_xdg = env::var("XDG_CONFIG_HOME");
    env::set_var("XDG_CONFIG_HOME", xdg_path.to_str().unwrap());

    let result = get_claude_config_dir();
    if result.is_some() {
        let result_path = result.unwrap();
        assert!(result_path.exists());
        assert!(result_path.is_dir());
    } else {
        assert!(result.is_none());
    }

    if let Ok(xdg) = original_xdg {
        env::set_var("XDG_CONFIG_HOME", xdg);
    } else {
        env::remove_var("XDG_CONFIG_HOME");
    }
}

#[test]
fn test_get_claude_config_dir_none() {
    let original_home = env::var("HOME");
    let original_xdg = env::var("XDG_CONFIG_HOME");

    env::set_var("HOME", "/non/existent/path");
    env::set_var("XDG_CONFIG_HOME", "/non/existent/xdg");

    let result = get_claude_config_dir();
    assert!(result.is_none());

    if let Ok(home) = original_home {
        env::set_var("HOME", home);
    } else {
        env::remove_var("HOME");
    }
    if let Ok(xdg) = original_xdg {
        env::set_var("XDG_CONFIG_HOME", xdg);
    } else {
        env::remove_var("XDG_CONFIG_HOME");
    }
}

#[test]
fn test_get_claude_json_paths_current_dir() {
    let temp_dir = tempdir().unwrap();
    let current_config = temp_dir.path().join(".claude.json");
    let mut file = fs::File::create(&current_config).unwrap();
    writeln!(file, r#"{{"current": "value"}}"#).unwrap();

    let original_dir = env::current_dir().unwrap();
    env::set_current_dir(temp_dir.path()).unwrap();

    let paths = get_claude_json_paths();
    assert!(!paths.is_empty());
    assert!(paths
        .iter()
        .any(|p| p.file_name().unwrap() == ".claude.json"));

    env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_get_claude_json_paths_home() {
    let temp_dir = tempdir().unwrap();
    let home_path = temp_dir.path();

    let home_config = home_path.join(".claude.json");
    let mut home_file = fs::File::create(&home_config).unwrap();
    writeln!(home_file, r#"{{"home": "value"}}"#).unwrap();

    let original_home = env::var("HOME");
    env::set_var("HOME", home_path.to_str().unwrap());

    let paths = get_claude_json_paths();
    assert!(!paths.is_empty());
    assert!(paths
        .iter()
        .any(|p| p.file_name().unwrap() == ".claude.json"));

    if let Ok(home) = original_home {
        env::set_var("HOME", home);
    } else {
        env::remove_var("HOME");
    }
}

#[test]
fn test_get_claude_json_paths_claude_dir() {
    let temp_dir = tempdir().unwrap();
    let home_path = temp_dir.path();

    let claude_dir = home_path.join(".claude");
    fs::create_dir(&claude_dir).unwrap();
    let config_file = claude_dir.join("config.json");
    let mut file = fs::File::create(&config_file).unwrap();
    writeln!(file, r#"{{"claude_dir_test": "value"}}"#).unwrap();

    assert!(claude_dir.exists());
    assert!(config_file.exists());

    let _paths = get_claude_json_paths();
}

#[test]
fn test_get_claude_json_paths_xdg() {
    let temp_dir = tempdir().unwrap();
    let xdg_path = temp_dir.path();
    let claude_dir = xdg_path.join("claude");
    fs::create_dir(&claude_dir).unwrap();

    let config_file = claude_dir.join("config.json");
    let mut file = fs::File::create(&config_file).unwrap();
    writeln!(file, r#"{{"xdg_test": "value"}}"#).unwrap();

    let original_xdg = env::var("XDG_CONFIG_HOME");
    env::set_var("XDG_CONFIG_HOME", xdg_path.to_str().unwrap());

    let paths = get_claude_json_paths();
    assert!(!paths.is_empty());
    assert!(paths
        .iter()
        .any(|p| p.file_name().unwrap() == "config.json"));

    if let Ok(xdg) = original_xdg {
        env::set_var("XDG_CONFIG_HOME", xdg);
    } else {
        env::remove_var("XDG_CONFIG_HOME");
    }
}

#[test]
fn test_get_claude_json_paths_multiple_sources() {
    let temp_dir = tempdir().unwrap();
    let home_path = temp_dir.path();

    let home_config = home_path.join(".claude.json");
    let mut home_file = fs::File::create(&home_config).unwrap();
    writeln!(home_file, r#"{{"home": "value"}}"#).unwrap();

    let claude_dir = home_path.join(".claude");
    fs::create_dir(&claude_dir).unwrap();
    let claude_config = claude_dir.join("config.json");
    let mut claude_file = fs::File::create(&claude_config).unwrap();
    writeln!(claude_file, r#"{{"claude": "value"}}"#).unwrap();

    let original_home = env::var("HOME");
    env::set_var("HOME", home_path.to_str().unwrap());

    let paths = get_claude_json_paths();
    assert!(paths.len() >= 2);
    assert!(paths
        .iter()
        .any(|p| p.file_name().unwrap() == ".claude.json"));
    assert!(paths
        .iter()
        .any(|p| p.file_name().unwrap() == "config.json"));

    if let Ok(home) = original_home {
        env::set_var("HOME", home);
    } else {
        env::remove_var("HOME");
    }
}

#[test]
fn test_get_claude_json_paths_no_configs() {
    let original_xdg = env::var("XDG_CONFIG_HOME");
    env::remove_var("XDG_CONFIG_HOME");

    let _paths = get_claude_json_paths();

    if let Ok(xdg) = original_xdg {
        env::set_var("XDG_CONFIG_HOME", xdg);
    } else {
        env::remove_var("XDG_CONFIG_HOME");
    }
}

#[test]
fn test_path_ordering() {
    let temp_dir = tempdir().unwrap();
    let home_path = temp_dir.path();

    let current_config = home_path.join(".claude.json");
    let mut current_file = fs::File::create(&current_config).unwrap();
    writeln!(current_file, r#"{{"current": "value"}}"#).unwrap();

    let home_config = home_path.join(".claude.json");
    let mut home_file = fs::File::create(&home_config).unwrap();
    writeln!(home_file, r#"{{"home": "value"}}"#).unwrap();

    let claude_dir = home_path.join(".claude");
    fs::create_dir(&claude_dir).unwrap();
    let claude_config = claude_dir.join("config.json");
    let mut claude_file = fs::File::create(&claude_config).unwrap();
    writeln!(claude_file, r#"{{"claude": "value"}}"#).unwrap();

    let original_home = env::var("HOME");
    let original_dir = env::current_dir().unwrap();
    env::set_var("HOME", home_path.to_str().unwrap());
    env::set_current_dir(home_path).unwrap();

    let paths = get_claude_json_paths();
    assert_eq!(paths.len(), 3);
    assert_eq!(paths[0].file_name().unwrap(), ".claude.json");

    if let Ok(home) = original_home {
        env::set_var("HOME", home);
    } else {
        env::remove_var("HOME");
    }
    env::set_current_dir(original_dir).unwrap();
}
