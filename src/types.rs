use reth_primitives::{H256, U256};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub(crate) struct PeerData {
    pub enode_url: String,
    pub id: String,
    pub address: String,
    pub tcp_port: u16,
    pub client_version: String,
    pub eth_version: u8,
    pub capabilities: Vec<String>,
    pub chain: String,
    pub total_difficulty: U256,
    pub best_block: H256, // TODO: convert this to a blocknum with a lookup
    pub genesis_block_hash: H256,
    pub last_seen: String,
    pub country: String,
    pub city: String,
}
