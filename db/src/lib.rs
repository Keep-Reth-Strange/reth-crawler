pub mod db;
pub mod types;

use std::sync::Arc;
use tokio::{fs::OpenOptions, io::AsyncWriteExt};

// Re-exports
pub use db::{AwsPeerDB, InMemoryPeerDB, PeerDB, SqlPeerDB};
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
pub async fn save_peer(peer_data: PeerData, db: Arc<dyn PeerDB>, ttl: i64) {
    db.add_peer(peer_data, Some(ttl)).await.unwrap();
}
