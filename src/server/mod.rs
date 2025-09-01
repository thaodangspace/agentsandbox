use anyhow::{Context, Result};
use axum::{
    http::StatusCode,
    routing::{get, post},
    Extension, Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};

mod api;
mod terminal;

pub use terminal::terminal_ws;

use api::{get_changed, list_dir, start_container_api};

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
    let app = Router::new()
        .route("/api/changed/:container", get(get_changed))
        .route("/api/list", get(list_dir))
        .route("/api/start", post(start_container_api))
        .route("/terminal/:container", get(terminal_ws))
        .route("/shutdown", get(shutdown_handler))
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
