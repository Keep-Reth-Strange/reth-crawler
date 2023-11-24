use std::{fmt::Debug, sync::Arc};

use axum::extract::FromRef;
use reth_crawler_db::{db::SqlPeerDB, AwsPeerDB, PeerDB};

/// Stores the database.
#[derive(Clone, FromRef)]
pub struct AppState {
    store: Arc<dyn PeerDB>,
}

impl Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("store", &"database")
            .finish()
    }
}

impl AppState {
    /// Create an `AppState` with an AWS database.
    pub async fn new_aws() -> Self {
        Self {
            store: Arc::new(AwsPeerDB::new().await),
        }
    }

    /// Create an `AppState` with a SQLite database.
    pub async fn new_sql() -> Self {
        Self {
            store: Arc::new(SqlPeerDB::new().await),
        }
    }
}
