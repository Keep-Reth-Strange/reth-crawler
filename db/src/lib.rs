pub mod db;
pub mod types;

use tokio::{fs::OpenOptions, io::AsyncWriteExt};

// Re-exports
pub use db::PeerDB;
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
pub async fn save_peer(peer_data: PeerData, save_to_json: bool, db: PeerDB) {
    if save_to_json {
        match append_to_file(peer_data).await {
            Ok(_) => (),
            Err(e) => eprintln!("Error appending to file: {:?}", e),
        }
    } else {
        db.add_peer(peer_data).await.unwrap();
    }
}
