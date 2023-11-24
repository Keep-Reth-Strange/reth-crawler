use super::app_state::AppState;
use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use chrono::{Days, Utc};
use reth_crawler_db::{types::ClientData, PeerDB, PeerData};
use std::sync::Arc;

/// The max `last_seen` a node can have and still be considered active (in DAYS).
const MAX_LAST_SEEN: u64 = 5;

/// Router for all APIs.
pub(crate) fn rest_router() -> Router<AppState> {
    Router::new()
        .route("/nodes", get(get_nodes))
        .route("/active-nodes", get(get_active_nodes))
        .route("/node/id/:id", get(get_node_by_id))
        .route("/node/ip/:ip", get(get_node_by_ip))
        .route("/clients", get(get_clients))
        .route("/active-clients", get(get_active_clients))
}

async fn get_nodes(State(store): State<Arc<dyn PeerDB>>) -> Json<Vec<PeerData>> {
    Json(store.all_peers(Some(50)).await.unwrap())
}

async fn get_active_nodes(State(store): State<Arc<dyn PeerDB>>) -> Json<Vec<PeerData>> {
    let cutoff = Utc::now()
        .checked_sub_days(Days::new(MAX_LAST_SEEN))
        .expect("Data is not ouf of range")
        .to_string();
    Json(store.all_active_peers(cutoff, Some(50)).await.unwrap())
}

async fn get_clients(State(store): State<Arc<dyn PeerDB>>) -> Json<Vec<ClientData>> {
    Json(store.clients(Some(50)).await.unwrap())
}

async fn get_active_clients(State(store): State<Arc<dyn PeerDB>>) -> Json<Vec<ClientData>> {
    let cutoff = Utc::now()
        .checked_sub_days(Days::new(MAX_LAST_SEEN))
        .expect("Data is not ouf of range")
        .to_string();
    Json(store.active_clients(cutoff, Some(50)).await.unwrap())
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
