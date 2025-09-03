use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::{get, get_service},
    Router,
};
use tower::ServiceExt;
use tower_http::services::{ServeDir, ServeFile};

use agentsandbox::server;

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

#[tokio::test]
async fn serves_frontend_index() {
    let app = Router::new().fallback_service(
        get_service(ServeDir::new("web/dist").fallback(ServeFile::new("web/dist/index.html")))
            .handle_error(|_| async move { StatusCode::INTERNAL_SERVER_ERROR }),
    );

    let res = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn serves_frontend_container_route() {
    let app = Router::new().fallback_service(
        get_service(ServeDir::new("web/dist").fallback(ServeFile::new("web/dist/index.html")))
            .handle_error(|_| async move { StatusCode::INTERNAL_SERVER_ERROR }),
    );

    let res = app
        .oneshot(
            Request::builder()
                .uri("/container/test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
}
