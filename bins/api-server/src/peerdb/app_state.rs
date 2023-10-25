use std::sync::Arc;

use axum::extract::FromRef;
use reth_crawler_db::{db::SqlPeerDB, AwsPeerDB, PeerDB};

#[derive(Clone, FromRef)]
pub struct AppState {
    store: Arc<dyn PeerDB>,
}

impl AppState {
    pub async fn new_aws() -> Self {
        Self {
            store: Arc::new(AwsPeerDB::new().await),
        }
    }

    pub async fn new_sql() -> Self {
        Self {
            store: Arc::new(SqlPeerDB::new().await),
        }
    }
}
