use axum::extract::FromRef;
use reth_crawler_db::{db::SqlPeerDB, PeerDB};
use std::sync::Arc;

/// Stores the database.
#[derive(Clone, FromRef)]
pub(crate) struct AppState {
    store: Arc<dyn PeerDB>,
}

impl AppState {
    /// Create an `AppState` with a SQLite database.
    pub(crate) async fn new_sql() -> Self {
        Self {
            store: Arc::new(SqlPeerDB::new().await),
        }
    }
}
