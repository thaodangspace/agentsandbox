use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query,
    },
    http::StatusCode,
    response::{IntoResponse, Response},
};
use base64::Engine as _;
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::sync::Mutex;

#[derive(Deserialize)]
pub struct TerminalParams {
    pub token: Option<String>,
    pub run: Option<String>,
    pub run_b64: Option<String>,
    pub cwd: Option<String>,
    pub cwd_b64: Option<String>,
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
    let resolved_cwd = if let Some(cwd_b64) = cwd_b64 {
        match base64::engine::general_purpose::STANDARD.decode(cwd_b64.as_bytes()) {
            Ok(bytes) => Some(String::from_utf8_lossy(&bytes).to_string()),
            Err(_) => cwd,
        }
    } else {
        cwd
    };

    if let Some(ref workdir) = resolved_cwd {
        let _ = Command::new("docker")
            .args(["exec", &container, "mkdir", "-p", workdir])
            .status()
            .await;
    }

    let autorun: Option<String> = if let Some(cmd_b64) = run_b64.clone() {
        match base64::engine::general_purpose::STANDARD.decode(cmd_b64.as_bytes()) {
            Ok(bytes) => Some(String::from_utf8_lossy(&bytes).to_string()),
            Err(_) => run.clone(),
        }
    } else {
        run.clone()
    };

    let mut docker_cmd = Command::new("docker");
    docker_cmd.arg("exec");
    docker_cmd.arg("-i");
    if let Some(ref workdir) = resolved_cwd {
        docker_cmd.args(["-w", workdir]);
    }
    let shell_start = if let Some(ref cmd) = autorun {
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

    if autorun.is_none() {
        if let Some(cmd_plain) = run {
            let _ = stdin.write_all(format!("{}\n", cmd_plain).as_bytes()).await;
            let _ = stdin.flush().await;
        }
    }

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
