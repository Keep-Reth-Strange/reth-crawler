use axum::extract::FromRef;
use reth_crawler_db::PeerDB;

#[derive(Clone, FromRef)]
pub struct AppState {
    store: PeerDB,
}

impl AppState {
    pub async fn new() -> Self {
        Self {
            store: PeerDB::new().await,
        }
    }
}
