mod peerdb;

use axum::routing;
use axum::Json;
use axum::Router;
use clap::{Args, Parser, Subcommand};
use peerdb::{rest_router, AppState};
use std::net::SocketAddr;
use tracing::info;

#[derive(Parser)]
#[command(author, version)]
#[command(
    about = "Reth crawler api server",
    long_about = "It starts the api server for the reth crawler project"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start crawling the network
    StartApiServer(CrawlOpts),
}

#[derive(Args)]
struct CrawlOpts {
    #[arg(long)]
    /// Save peers into an in memory db. This is useful for local testing because you don't have to setup a centralized database.
    sql_db: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Commands::StartApiServer(opts) => start_api_server(opts.sql_db),
    }
    .await
}

async fn start_api_server(sql_db: bool) -> Result<(), Box<dyn std::error::Error>> {
    let app = Router::new()
        .route("/", routing::get(handler))
        .merge(rest_router())
        .with_state(if sql_db {
            AppState::new_sql().await
        } else {
            AppState::new_aws().await
        });

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
