#[path = "../src/language.rs"]
mod language;

use language::{detect_project_languages, ProjectLanguage};
use std::fs;
use tempfile::tempdir;

#[test]
fn detect_languages() {
    let tmp = tempdir().unwrap();
    fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
    fs::write(tmp.path().join("package.json"), "").unwrap();
    let langs = detect_project_languages(tmp.path());
    assert!(langs.contains(&ProjectLanguage::Rust));
    assert!(langs.contains(&ProjectLanguage::NodeJs));
}
