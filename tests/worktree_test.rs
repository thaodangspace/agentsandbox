use std::{fs, process::Command};
use tempfile::tempdir;

#[path = "../src/worktree.rs"]
mod worktree;

use worktree::create_worktree;

// #[test]
// fn create_missing_branch() {
//     let tmp = tempdir().expect("temp dir");
//     let repo_path = tmp.path();
// 
//     Command::new("git")
//         .args(["config", "user.email", "test@example.com"])
//         .current_dir(repo_path)
//         .status()
//         .expect("git config email");
//     Command::new("git")
//         .args(["config", "user.name", "Test User"])
//         .current_dir(repo_path)
//         .status()
//         .expect("git config name");
//     Command::new("git")
//         .arg("init")
//         .current_dir(repo_path)
//         .status()
//         .expect("git init");
//     fs::write(repo_path.join("file.txt"), "test").expect("write file");
//     Command::new("git")
//         .args(["add", "."])
//         .current_dir(repo_path)
//         .status()
//         .expect("git add");
//     Command::new("git")
//         .args(["commit", "-m", "init"])
//         .current_dir(repo_path)
//         .status()
//         .expect("git commit");
// 
// 
// 
// 
// 
//     let worktree = create_worktree(repo_path, "feature").expect("create worktree");
//     assert!(worktree.exists());
// 
//     let branch_status = Command::new("git")
//         .args(["rev-parse", "--verify", "feature"])
//         .current_dir(repo_path)
//         .status()
//         .expect("git rev-parse");
//     assert!(branch_status.success());
// }

#[test]
fn reuse_existing_worktree() {
    let tmp = tempdir().expect("temp dir");
    let repo_path = tmp.path();

    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_path)
        .status()
        .expect("git config email");
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_path)
        .status()
        .expect("git config name");
    Command::new("git")
        .arg("init")
        .current_dir(repo_path)
        .status()
        .expect("git init");
    fs::write(repo_path.join("file.txt"), "test").expect("write file");
    Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .status()
        .expect("git add");
    Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(repo_path)
        .status()
        .expect("git commit");



    let first = create_worktree(repo_path, "feature").expect("create worktree");
    let second = create_worktree(repo_path, "feature").expect("reuse worktree");
    assert!(first.exists());
    assert_eq!(first, second);
}
