use std::sync::Arc;

use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use reth_crawler_db::{PeerDB, PeerData};

use super::app_state::AppState;

pub fn rest_router() -> Router<AppState> {
    Router::new()
        .route("/nodes", get(get_nodes))
        .route("/node/id/:id", get(get_node_by_id))
        .route("/node/ip/:ip", get(get_node_by_ip))
}

async fn get_nodes(State(store): State<Arc<dyn PeerDB>>) -> Json<Vec<PeerData>> {
    Json(store.all_peers(Some(50)).await.unwrap())
}

async fn get_node_by_id(
    State(store): State<Arc<dyn PeerDB>>,
    Path(id): Path<String>,
) -> Json<Option<Vec<PeerData>>> {
    Json(store.node_by_id(id).await.unwrap())
}

async fn get_node_by_ip(
    State(store): State<Arc<dyn PeerDB>>,
    Path(ip): Path<String>,
) -> Json<Option<Vec<PeerData>>> {
    Json(store.node_by_ip(ip).await.unwrap())
}
