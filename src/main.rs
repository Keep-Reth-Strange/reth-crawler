//! Low level example of connecting to and communicating with a peer.
//!
//! Run with
//!
//! ```not_rust
//! cargo run -p manual-p2p
//! ```
mod crawler;
mod types;
use crawler::CrawlerFactory;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // need to join with other services eventually
    CrawlerFactory::new().await.make().run().await
}
