use axum::{
    extract::{Path, Query},
    http::StatusCode,
    Json,
};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::{fs, process::Command, sync::Mutex};

use crate::cli::Agent;
use crate::container::{check_docker_availability, create_container, generate_container_name};

static CONTAINER_PATHS: Lazy<Mutex<HashMap<String, String>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Serialize)]
pub(super) struct FileDiff {
    path: String,
    status: String,
    diff: Option<String>,
}

#[derive(Serialize)]
pub(super) struct ChangeResponse {
    files: Vec<FileDiff>,
}

#[derive(Serialize)]
pub(super) struct ErrorResponse {
    error: String,
}

#[derive(Serialize)]
pub(super) struct DirEntryInfo {
    name: String,
    path: String,
    is_dir: bool,
}

#[derive(Deserialize)]
pub(super) struct ListQuery {
    path: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct StartRequest {
    path: String,
    agent: String,
}

#[derive(Serialize)]
pub(super) struct StartResponse {
    container: String,
}

pub(super) async fn list_dir(
    Query(ListQuery { path }): Query<ListQuery>,
) -> Result<Json<Vec<DirEntryInfo>>, (StatusCode, Json<ErrorResponse>)> {
    let base = path.unwrap_or_else(|| ".".to_string());
    let mut entries = fs::read_dir(&base).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    let mut result = Vec::new();
    while let Some(entry) = entries.next_entry().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })? {
        let file_type = entry.file_type().await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;
        result.push(DirEntryInfo {
            name: entry.file_name().to_string_lossy().into(),
            path: entry.path().display().to_string(),
            is_dir: file_type.is_dir(),
        });
    }
    Ok(Json(result))
}

pub(super) async fn start_container_api(
    Json(req): Json<StartRequest>,
) -> Result<Json<StartResponse>, (StatusCode, Json<ErrorResponse>)> {
    let path = PathBuf::from(&req.path);
    if !path.is_dir() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid path".into(),
            }),
        ));
    }

    let agent = match req.agent.to_lowercase().as_str() {
        "claude" => Agent::Claude,
        "gemini" => Agent::Gemini,
        "codex" => Agent::Codex,
        "qwen" => Agent::Qwen,
        "cursor" => Agent::Cursor,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "invalid agent".into(),
                }),
            ))
        }
    };

    if let Err(e) = check_docker_availability() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        ));
    }

    let container_name = generate_container_name(&path, &agent);
    if let Err(e) = create_container(&container_name, &path, None, &agent, None, false, false).await
    {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        ));
    }

    {
        let mut map = CONTAINER_PATHS.lock().await;
        map.insert(container_name.clone(), path.display().to_string());
    }

    Ok(Json(StartResponse {
        container: container_name,
    }))
}

pub(super) async fn get_changed(
    Path(container): Path<String>,
) -> Result<Json<ChangeResponse>, (StatusCode, Json<ErrorResponse>)> {
    let repo_path = {
        let map = CONTAINER_PATHS.lock().await;
        match map.get(&container) {
            Some(p) => p.clone(),
            None => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "unknown container".into(),
                    }),
                ))
            }
        }
    };

    let status_output = Command::new("docker")
        .args([
            "exec",
            "-w",
            &repo_path,
            &container,
            "git",
            "status",
            "--porcelain",
        ])
        .output()
        .await;

    match status_output {
        Ok(out) if out.status.success() => {
            let status_lines = String::from_utf8_lossy(&out.stdout);
            let mut files = Vec::new();

            for line in status_lines.lines() {
                if line.len() < 3 {
                    continue;
                }

                let status_chars: Vec<char> = line.chars().collect();
                let index_status = status_chars[0];
                let worktree_status = status_chars[1];
                let path = line[3..].to_string();

                let status = if index_status != ' ' && index_status != '?' {
                    index_status.to_string()
                } else {
                    worktree_status.to_string()
                };

                let diff_text = match (index_status, worktree_status) {
                    ('?', '?') => {
                        let cat_output = Command::new("docker")
                            .args(["exec", "-w", &repo_path, &container, "cat", &path])
                            .output()
                            .await;
                        match cat_output {
                            Ok(cat_out) if cat_out.status.success() => {
                                let content = String::from_utf8_lossy(&cat_out.stdout);
                                Some(format!(
                                    "--- /dev/null\n+++ {}\n@@ -0,0 +1,{} @@\n{}",
                                    path,
                                    content.lines().count(),
                                    content
                                        .lines()
                                        .map(|l| format!("+{}", l))
                                        .collect::<Vec<_>>()
                                        .join("\n"),
                                ))
                            }
                            _ => None,
                        }
                    }
                    _ => {
                        let diff_output = Command::new("docker")
                            .args([
                                "exec", "-w", &repo_path, &container, "git", "diff", "HEAD", "--",
                                &path,
                            ])
                            .output()
                            .await;
                        match diff_output {
                            Ok(diff_out) if diff_out.status.success() => {
                                let diff_content =
                                    String::from_utf8_lossy(&diff_out.stdout).to_string();
                                if diff_content.is_empty() {
                                    let staged_diff = Command::new("docker")
                                        .args([
                                            "exec", "-w", &repo_path, &container, "git", "diff",
                                            "--cached", "--", &path,
                                        ])
                                        .output()
                                        .await;
                                    match staged_diff {
                                        Ok(staged_out) if staged_out.status.success() => {
                                            let staged_content =
                                                String::from_utf8_lossy(&staged_out.stdout)
                                                    .to_string();
                                            if !staged_content.is_empty() {
                                                Some(staged_content)
                                            } else {
                                                None
                                            }
                                        }
                                        _ => None,
                                    }
                                } else {
                                    Some(diff_content)
                                }
                            }
                            _ => None,
                        }
                    }
                };

                files.push(FileDiff {
                    path,
                    status,
                    diff: diff_text,
                });
            }

            Ok(Json(ChangeResponse { files }))
        }
        Ok(out) => {
            let msg = String::from_utf8_lossy(&out.stderr).to_string();
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error: msg }),
            ))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}
