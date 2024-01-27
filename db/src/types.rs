//! Types used in the crawler and api-server.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

use aws_sdk_dynamodb::{
    error::SdkError,
    operation::{put_item::PutItemError, query::QueryError, scan::ScanError},
    types::AttributeValue,
};

/// A peer.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PeerData {
    /// Enode of the node.
    pub enode_url: String,
    /// Id of the node. This is just a random address tied to the node.
    pub id: String,
    /// Ip address.
    pub address: String,
    /// Tcp port.
    pub tcp_port: u16,
    /// Client name.
    pub client_name: String,
    /// Client version.
    pub client_version: String,
    /// Client build.
    pub client_build: String,
    /// CPU architecture.
    pub client_arch: String,
    /// Operating system.
    pub os: String,
    /// Code language used for the client.
    pub client_language: String,
    /// Eth wire protocol version.
    pub eth_version: u8,
    /// Capabilities of the node.
    pub capabilities: Vec<String>,
    /// Chain the node is syncing.
    pub chain: String,
    /// Total difficuly the node is aware of. After The Merge this is alwasy `58750003716598352816469`.
    pub total_difficulty: String,
    /// Block hash of the newest node the node is aware of.
    pub best_block: String, // TODO: convert this to a blocknum with a lookup
    /// Block hash of the genesis block.
    pub genesis_block_hash: String,
    /// Last time we update info of the node.
    pub last_seen: String,
    /// Country in which the node is currently running.
    pub country: String,
    /// City in which the node is currently running.
    pub city: String,
    /// `True` if the node is up to date with the tip of the chain. `False` if not. `None` when we are not able to get this data.
    pub synced: Option<bool>,
    /// Internet service provider the node is using.
    pub isp: String,
}

impl PeerData {
    /// Create a new peer.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        enode_url: String,
        id: String,
        address: String,
        tcp_port: u16,
        client_name: String,
        client_version: String,
        client_build: String,
        client_arch: String,
        os: String,
        client_language: String,
        capabilities: Vec<String>,
        last_seen: String,
        country: String,
        city: String,
        genesis_block_hash: String,
        best_block: String,
        total_difficulty: String,
        chain: String,
        eth_version: u8,
        synced: Option<bool>,
        isp: String,
    ) -> Self {
        Self {
            enode_url,
            id,
            address,
            tcp_port,
            client_name,
            client_version,
            client_build,
            client_arch,
            os,
            client_language,
            capabilities,
            last_seen,
            country,
            city,
            total_difficulty,
            chain,
            best_block,
            eth_version,
            genesis_block_hash,
            synced,
            isp,
        }
    }
}

impl From<&HashMap<String, AttributeValue>> for PeerData {
    fn from(value: &HashMap<String, AttributeValue>) -> Self {
        let peer_data = PeerData::new(
            as_string(value.get("enode_url"), &"".to_string()),
            as_string(value.get("peer-id"), &"".to_string()),
            as_string(value.get("peer-ip"), &"".to_string()),
            as_u16(value.get("port"), 30303),
            as_string(value.get("client_name"), &"".to_string()),
            as_string(value.get("client_version"), &"".to_string()),
            as_string(value.get("client_build"), &"".to_string()),
            as_string(value.get("client_architecture"), &"".to_string()),
            as_string(value.get("os"), &"".to_string()),
            as_string(value.get("client_language"), &"".to_string()),
            as_string_vec(value.get("capabilities")),
            as_string(value.get("last_seen"), &"".to_string()),
            as_string(value.get("country"), &"".to_string()),
            as_string(value.get("city"), &"".to_string()),
            as_string(value.get("genesis_block_hash"), &"".to_string()),
            as_string(value.get("best_block"), &"".to_string()),
            as_string(value.get("total_difficulty"), &"".to_string()),
            as_string(value.get("chain"), &"".to_string()),
            as_u8(value.get("eth_version"), 0),
            as_option_bool(value.get("synced"), None),
            as_string(value.get("isp"), &"".to_string()),
        );

        peer_data
    }
}

/// Helper function for AWS dynamo db.
pub fn as_string(val: Option<&AttributeValue>, default: &String) -> String {
    if let Some(v) = val {
        if let Ok(s) = v.as_s() {
            return s.to_owned();
        }
    }
    default.to_owned()
}

/// Helper function for AWS dynamo db.
pub fn as_u16(val: Option<&AttributeValue>, default: u16) -> u16 {
    if let Some(v) = val {
        if let Ok(n) = v.as_n() {
            if let Ok(n) = n.parse::<u16>() {
                return n;
            }
        }
    }
    default
}

/// Helper function for AWS dynamo db.
pub fn as_u8(val: Option<&AttributeValue>, default: u8) -> u8 {
    if let Some(v) = val {
        if let Ok(n) = v.as_n() {
            if let Ok(n) = n.parse::<u8>() {
                return n;
            }
        }
    }
    default
}

/// Helper function for AWS dynamo db.
pub fn as_string_vec(val: Option<&AttributeValue>) -> Vec<String> {
    if let Some(val) = val {
        if let Ok(val) = val.as_l() {
            return val
                .iter()
                .map(|v| as_string(Some(v), &"".to_string()))
                .collect();
        }
    }
    vec![]
}

/// Helper function for AWS dynamo db.
pub fn as_option_bool(val: Option<&AttributeValue>, default: Option<bool>) -> Option<bool> {
    if let Some(v) = val {
        if let Ok(n) = v.as_bool() {
            return Some(*n);
        }
    }
    default
}

/// Error for the `add_peer` operation.
#[derive(Debug, Error)]
pub enum AddItemError {
    /// Error related to aws dynamo db.
    #[error("An error occurred adding a new item into the AWS database: {0}")]
    AwsAddItemError(#[from] SdkError<PutItemError>),
    /// Error related to in memory db.
    #[error("An error occurred adding a new item into the in memory db")]
    InMemoryDbAddItemError(),
    /// Error related to SQLite db.
    #[error("An error occurred adding a new item into the SQL database: {0}")]
    SqlAddItemError(#[from] tokio_rusqlite::Error),
    /// Error related to Postgres db.
    #[error("An error occurred adding a new item into the Postgres database: {0}")]
    PostgresAddItemError(#[from] tokio_postgres::Error),
}

/// Error for the `all_peers` operation.
#[derive(Debug, Error)]
pub enum ScanTableError {
    /// Error related to aws dynamo db.
    #[error("An error occurred while performing a scan of the AWS database: {0}")]
    AwsScanError(#[from] SdkError<ScanError>),
    /// Error related to in memory db.
    #[error("An error occurred while performing a scan of the in memory database")]
    InMemoryDbScanError(),
    /// Error related to SQLite db.
    #[error("An error occurred while performing a scan of the SQL database: {0}")]
    SqlScanError(#[from] tokio_rusqlite::Error),
    /// Error related to Postgres db.
    #[error("An error occurred while performing a scan of the Postgres database: {0}")]
    PostgresScanError(#[from] tokio_postgres::Error),
}

/// Error for the `node_by_id` or `node_by_ip` operations.
#[derive(Debug, Error)]
pub enum QueryItemError {
    /// Error related to aws dynamo db.
    #[error("An error occurred querying the AWS database: {0}")]
    AwsQueryItemError(#[from] SdkError<QueryError>),
    /// Error related to in memory db.
    #[error("An error occurred querying the in memory database")]
    InMemoryDbQueryItemError(),
    /// Error related to SQLite db.
    #[error("An error occurred querying the SQL database: {0}")]
    SqlQueryItemError(#[from] tokio_rusqlite::Error),
    /// Error related to Postgres db.
    #[error("An error occurred querying the Postgres database:: {0}")]
    PostgresQueryItemError(#[from] tokio_postgres::Error),
}

/// We don't use this...
// TODO: delete it
#[derive(Debug, Error)]
pub enum DeleteItemError {
    /// Error related to SQLite db.
    #[error("An error occurred deleting a new item into the SQL database: {0}")]
    SqlDeleteItemError(#[from] tokio_rusqlite::Error),
    /// Error related to Postgres db.
    #[error("An error occurred deleting a new item into the Postgres database: {0}")]
    PostgresDeleteItemError(#[from] tokio_postgres::Error),
}

/// Represents a client.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ClientData {
    /// Client version of the node.
    pub client_version: String,
}
