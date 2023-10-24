use axum::extract::FromRef;
use reth_crawler_db::AwsPeerDB;

#[derive(Clone, FromRef)]
pub struct AppState {
    store: AwsPeerDB,
}

impl AppState {
    pub async fn new() -> Self {
        Self {
            store: AwsPeerDB::new().await,
        }
    }
}
