mod db_sync;
mod peerdb;

use axum::routing;
use axum::Json;
use axum::Router;
use clap::{Parser, Subcommand};
use db_sync::db_sync_handler;
use peerdb::{rest_router, AppState};
use std::net::SocketAddr;
use tokio::try_join;
use tracing::info;

/// Update time for the recurrent `db_sync()` task. 5 minutes.
const UPDATE_TIME: i64 = 5 * 60;

#[derive(Parser)]
#[command(author, version)]
#[command(
    about = "Reth crawler api server",
    long_about = "It starts the api server for the reth crawler project.
    
    It always uses a SQLite database and periodically fetches updates from the dynamoDB of the crawler."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start crawling the network
    StartApiServer,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let start_api_server_futures = {
        match cli.command {
            Commands::StartApiServer => start_api_server(),
        }
    };

    let db_sync_futures = { db_sync_handler(UPDATE_TIME) };

    let (_, _) = try_join!(start_api_server_futures, db_sync_futures)?;

    Ok(())
}

async fn start_api_server() -> Result<(), Box<dyn std::error::Error>> {
    let app = Router::new()
        .route("/", routing::get(handler))
        .merge(rest_router())
        .with_state(AppState::new_sql().await);

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
