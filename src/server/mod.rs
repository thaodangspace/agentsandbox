use anyhow::{Context, Result};
use axum::{
    body::{boxed, Body},
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query,
    },
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Extension, Json, Router,
};
use base64::Engine as _;
use futures::{SinkExt, StreamExt};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::sync::{oneshot, Mutex};
use tower::{service_fn, ServiceExt};
use tower_http::services::{ServeDir, ServeFile};

use crate::cli::Agent;
use crate::container::{check_docker_availability, create_container, generate_container_name};

static CONTAINER_PATHS: Lazy<Mutex<HashMap<String, String>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Serialize)]
struct FileDiff {
    path: String,
    status: String,
    diff: Option<String>,
}

#[derive(Serialize)]
struct ChangeResponse {
    files: Vec<FileDiff>,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Serialize)]
struct DirEntryInfo {
    name: String,
    path: String,
    is_dir: bool,
}

#[derive(Deserialize)]
struct ListQuery {
    path: Option<String>,
}

#[derive(Deserialize)]
struct StartRequest {
    path: String,
    agent: String,
}

#[derive(Serialize)]
struct StartResponse {
    container: String,
}

async fn list_dir(
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

async fn start_container_api(
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

async fn get_changed(
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

    // Get git status to find changed files
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

                // Determine the overall status
                let status = if index_status != ' ' && index_status != '?' {
                    index_status.to_string()
                } else {
                    worktree_status.to_string()
                };

                // Get the diff for this file
                let diff_text = match (index_status, worktree_status) {
                    ('?', '?') => {
                        // Untracked file - show entire content as added
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
                                        .join("\n")
                                ))
                            }
                            _ => None,
                        }
                    }
                    _ => {
                        // Use git diff for tracked files
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
                                    // Try diff against index for staged changes
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

#[derive(Deserialize)]
pub struct TerminalParams {
    token: Option<String>,
    run: Option<String>,
    run_b64: Option<String>,
    cwd: Option<String>,
    cwd_b64: Option<String>,
}

pub async fn terminal_ws(
    ws: WebSocketUpgrade,
    Path(container): Path<String>,
    Query(params): Query<TerminalParams>,
) -> Response {
    let token_matches = params
        .token
        .as_ref()
        .map(|t| t == &container)
        .unwrap_or(true);

    if token_matches {
        ws.on_upgrade(move |socket| {
            handle_terminal(
                socket,
                container,
                params.run,
                params.run_b64,
                params.cwd,
                params.cwd_b64,
            )
        })
    } else {
        (StatusCode::UNAUTHORIZED, "invalid token").into_response()
    }
}

async fn handle_terminal(
    mut socket: WebSocket,
    container: String,
    run: Option<String>,
    run_b64: Option<String>,
    cwd: Option<String>,
    cwd_b64: Option<String>,
) {
    // Resolve working directory (if provided via query params)
    let resolved_cwd = if let Some(cwd_b64) = cwd_b64 {
        match base64::engine::general_purpose::STANDARD.decode(cwd_b64.as_bytes()) {
            Ok(bytes) => Some(String::from_utf8_lossy(&bytes).to_string()),
            Err(_) => cwd,
        }
    } else {
        cwd
    };

    // If a working directory was provided, ensure it exists inside the container
    if let Some(ref workdir) = resolved_cwd {
        let _ = Command::new("docker")
            .args(["exec", &container, "mkdir", "-p", workdir])
            .status()
            .await;
    }

    // Decode autorun command if provided (prefer base64 param)
    let autorun: Option<String> = if let Some(cmd_b64) = run_b64.clone() {
        match base64::engine::general_purpose::STANDARD.decode(cmd_b64.as_bytes()) {
            Ok(bytes) => Some(String::from_utf8_lossy(&bytes).to_string()),
            Err(_) => run.clone(),
        }
    } else {
        run.clone()
    };

    // Build docker exec command, adding -w when we have a workdir
    let mut docker_cmd = Command::new("docker");
    docker_cmd.arg("exec");
    docker_cmd.arg("-i");
    if let Some(ref workdir) = resolved_cwd {
        docker_cmd.args(["-w", workdir]);
    }
    // Do not request a TTY from Docker here; allocate a PTY inside the
    // container using `script` so it works from non-TTY servers.
    // If an autorun command is present, run it via bash -lc '<cmd>; exec bash -l'
    // so the user stays in an interactive shell.
    let shell_start = if let Some(ref cmd) = autorun {
        // Safely single-quote the command for the shell that processes script -c
        let escaped = cmd.replace('\'', "'\\''");
        format!("bash -lc '{}; exec bash -l'", escaped)
    } else {
        "bash -l".to_string()
    };

    docker_cmd.args([
        &container,
        "/usr/bin/env",
        "TERM=xterm-256color",
        "/usr/bin/script",
        "-q",
        "-f",
        "-c",
        &shell_start,
        "-",
    ]);

    let mut child = match docker_cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            let _ = socket
                .send(Message::Text(format!("failed to start shell: {e}")))
                .await;
            return;
        }
    };

    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();

    let (sender, mut receiver) = socket.split();
    let sender = Arc::new(Mutex::new(sender));

    // If we embedded the autorun into shell startup, no need to inject via stdin here.
    // Keep stdin injection only when no autorun was provided (compat for /terminal?run=...).
    if autorun.is_none() {
        if let Some(cmd_plain) = run {
            let _ = stdin.write_all(format!("{}\n", cmd_plain).as_bytes()).await;
            let _ = stdin.flush().await;
        }
    }

    // Forward shell stdout to websocket as text frames
    let mut out_buf = [0u8; 4096];
    let mut err_buf = [0u8; 4096];
    let sender_stdout = Arc::clone(&sender);
    let stdout_task = tokio::spawn(async move {
        loop {
            match stdout.read(&mut out_buf).await {
                Ok(n) if n > 0 => {
                    let chunk = String::from_utf8_lossy(&out_buf[..n]).to_string();
                    if sender_stdout
                        .lock()
                        .await
                        .send(Message::Text(chunk))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                _ => break,
            }
        }
    });

    // Forward shell stderr to websocket as text frames
    let sender_stderr = Arc::clone(&sender);
    let stderr_task = tokio::spawn(async move {
        loop {
            match stderr.read(&mut err_buf).await {
                Ok(n) if n > 0 => {
                    let chunk = String::from_utf8_lossy(&err_buf[..n]).to_string();
                    if sender_stderr
                        .lock()
                        .await
                        .send(Message::Text(chunk))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                _ => break,
            }
        }
    });

    // Forward websocket messages to stdin
    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(t) => {
                if stdin.write_all(t.as_bytes()).await.is_err() {
                    break;
                }
                let _ = stdin.flush().await;
            }
            Message::Binary(b) => {
                if stdin.write_all(&b).await.is_err() {
                    break;
                }
                let _ = stdin.flush().await;
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    let _ = stdin.shutdown().await;
    let _ = stdout_task.await;
    let _ = stderr_task.await;
    let _ = child.kill().await;
}

async fn shutdown_handler(
    Extension(tx): Extension<Arc<Mutex<Option<oneshot::Sender<()>>>>>,
) -> StatusCode {
    if let Some(tx) = tx.lock().await.take() {
        let _ = tx.send(());
    }
    StatusCode::OK
}

pub async fn serve() -> Result<()> {
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let shutdown_tx = Arc::new(Mutex::new(Some(shutdown_tx)));
    let serve_dir = ServeDir::new("web/dist").fallback(ServeFile::new("web/dist/index.html"));
    let static_files = service_fn(move |req: Request<Body>| {
        let serve_dir = serve_dir.clone();
        async move {
            match serve_dir.oneshot(req).await {
                Ok(res) => Ok(res.map(boxed)),
                Err(err) => Ok::<_, std::convert::Infallible>(
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Unhandled internal error: {err}"),
                    )
                        .into_response(),
                ),
            }
        }
    });
    let app = Router::new()
        .route("/api/changed/:container", get(get_changed))
        .route("/api/list", get(list_dir))
        .route("/api/start", post(start_container_api))
        .route("/terminal/:container", get(terminal_ws))
        .route("/shutdown", get(shutdown_handler))
        .nest_service("/", static_files)
        .layer(Extension(shutdown_tx));
    let addr = SocketAddr::from(([0, 0, 0, 0], 6789));
    println!("Listening on {addr}");
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(async {
            shutdown_rx.await.ok();
        })
        .await?;
    Ok(())
}

pub async fn stop() -> Result<()> {
    reqwest::get("http://127.0.0.1:6789/shutdown")
        .await
        .context("failed to send shutdown signal")?;
    Ok(())
}
