#[path = "../src/state.rs"]
mod state;

use state::{
    clear_last_container, load_container_run_command, load_last_container, prepare_session_log,
    save_container_run_command, save_last_container,
};
use std::{env, fs, path::PathBuf, sync::Mutex};
use tempfile::tempdir;

struct TempHome {
    _dir: tempfile::TempDir,
    _guard: std::sync::MutexGuard<'static, ()>,
    original: Option<PathBuf>,
}

impl Drop for TempHome {
    fn drop(&mut self) {
        if let Some(ref path) = self.original {
            env::set_var("HOME", path);
        } else {
            env::remove_var("HOME");
        }
    }
}

fn setup_temp_home() -> TempHome {
    static HOME_LOCK: Mutex<()> = Mutex::new(());
    let guard = HOME_LOCK.lock().unwrap();
    let original = env::var_os("HOME").map(PathBuf::from);
    let dir = tempdir().expect("failed to create temp dir");
    env::set_var("HOME", dir.path());
    TempHome {
        _dir: dir,
        _guard: guard,
        original,
    }
}

#[test]
fn test_load_nonexistent_container() {
    let _dir = setup_temp_home();
    let loaded = load_last_container().expect("load should succeed");
    assert!(loaded.is_none());
}

#[test]
fn test_save_and_load_last_container() {
    let _dir = setup_temp_home();
    save_last_container("my_container").expect("save should succeed");
    let loaded = load_last_container().expect("load should succeed");
    assert_eq!(loaded, Some("my_container".to_string()));
}

#[test]
fn test_clear_last_container() {
    let _dir = setup_temp_home();
    save_last_container("to_clear").expect("save should succeed");
    clear_last_container().expect("clear should succeed");
    let loaded = load_last_container().expect("load should succeed");
    assert!(loaded.is_none());
}

#[test]
fn test_save_and_load_run_command() {
    let _dir = setup_temp_home();
    let container = "container_abc";
    let cmd = "cd '/tmp/project' && codex --yolo";
    save_container_run_command(container, cmd).expect("save run command should succeed");
    let loaded = load_container_run_command(container).expect("load should succeed");
    assert_eq!(loaded, Some(cmd.to_string()));
}

#[test]
fn test_load_missing_run_command() {
    let _dir = setup_temp_home();
    let loaded = load_container_run_command("does_not_exist").expect("load should succeed");
    assert!(loaded.is_none());
}

#[test]
fn test_prepare_session_log_creates_unique_paths() {
    let _dir = setup_temp_home();
    let container = "container_logs";

    let (first_path, first_container_path) =
        prepare_session_log(container).expect("prepare session log should succeed");

    let parent = first_path
        .parent()
        .expect("log path should have parent directory");
    assert_eq!(parent.file_name().and_then(|n| n.to_str()), Some("logs"));
    assert!(parent.exists());
    assert!(first_path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|name| name.starts_with("session-"))
        .unwrap_or(false));
    assert!(first_container_path.starts_with("/tmp/"));

    // Simulate a previous log to force generation of a distinct file name.
    fs::File::create(&first_path).expect("should be able to create placeholder log file");

    let (second_path, second_container_path) =
        prepare_session_log(container).expect("prepare session log should succeed again");

    assert_ne!(first_path, second_path);
    assert_ne!(first_container_path, second_container_path);
}
