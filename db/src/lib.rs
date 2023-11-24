#![warn(
    missing_debug_implementations,
    missing_docs,
    unreachable_pub,
    rustdoc::all
)]
#![deny(unused_must_use, rust_2018_idioms)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

//! This crate handles the database part for the crawler and api-server.

pub mod db;
pub mod types;

use std::sync::Arc;
use tokio::{fs::OpenOptions, io::AsyncWriteExt};

// Re-exports
pub use db::{AwsPeerDB, PeerDB, SqlPeerDB};
pub use types::PeerData;

/// Helper function to append a peer to file
pub async fn append_to_file(peer_data: PeerData) -> eyre::Result<()> {
    let json = serde_json::to_string(&peer_data)? + "\n";
    let mut file = OpenOptions::new()
        .write(true)
        .append(true)
        .create(true)
        .open("peers_data.json")
        .await?;
    file.write_all(json.as_bytes()).await?;
    Ok(())
}

/// Helper function to save a peer.
pub async fn save_peer(peer_data: PeerData, db: Arc<dyn PeerDB>) {
    db.add_peer(peer_data).await.unwrap();
}
