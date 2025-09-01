use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::get,
    Router,
};
use tower::ServiceExt;

use codesandbox::server;

#[tokio::test]
async fn websocket_route_requires_upgrade() {
    let app = Router::new().route("/terminal/:container", get(server::terminal_ws));

    let res = app
        .oneshot(
            Request::builder()
                .uri("/terminal/my-container?token=my-container")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn websocket_route_without_token_requires_upgrade() {
    let app = Router::new().route("/terminal/:container", get(server::terminal_ws));

    let res = app
        .oneshot(
            Request::builder()
                .uri("/terminal/my-container")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}
