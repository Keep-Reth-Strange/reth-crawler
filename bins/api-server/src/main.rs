mod dynamodb;

use axum::routing;
use axum::Json;
use axum::Router;
use dynamodb::{rest_router, AppState};
use std::net::SocketAddr;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = Router::new()
        .route("/", routing::get(handler))
        .merge(rest_router())
        .with_state(AppState::new().await);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3030));
    info!("Server started, listening on {addr}");

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}

#[derive(serde::Serialize)]
struct Message {
    message: String,
}

async fn handler() -> Json<Message> {
    Json(Message {
        message: format!("Hello, World!"),
    })
}
