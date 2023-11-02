use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

use aws_sdk_dynamodb::{
    error::SdkError,
    operation::{put_item::PutItemError, query::QueryError, scan::ScanError},
    types::AttributeValue,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PeerData {
    pub enode_url: String,
    pub id: String,
    pub address: String,
    pub tcp_port: u16,
    pub client_version: String,
    pub eth_version: u8,
    pub capabilities: Vec<String>,
    pub chain: String,
    pub total_difficulty: String,
    pub best_block: String, // TODO: convert this to a blocknum with a lookup
    pub genesis_block_hash: String,
    pub last_seen: String,
    pub country: String,
    pub city: String,
}

impl PeerData {
    pub fn new(
        enode_url: String,
        id: String,
        address: String,
        tcp_port: u16,
        client_version: String,
        capabilities: Vec<String>,
        last_seen: String,
        country: String,
        city: String,
        genesis_block_hash: String,
        best_block: String,
        total_difficulty: String,
        chain: String,
        eth_version: u8,
    ) -> Self {
        Self {
            enode_url,
            id,
            address,
            tcp_port,
            client_version,
            capabilities,
            last_seen,
            country,
            city,
            total_difficulty: total_difficulty,
            chain,
            best_block: best_block,
            eth_version,
            genesis_block_hash: genesis_block_hash,
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
            as_string(value.get("client_version"), &"".to_string()),
            as_string_vec(value.get("capabilities")),
            as_string(value.get("last_seen"), &"".to_string()),
            as_string(value.get("country"), &"".to_string()),
            as_string(value.get("city"), &"".to_string()),
            as_string(value.get("genesis_block_hash"), &"".to_string()),
            as_string(value.get("best_block"), &"".to_string()),
            as_string(value.get("total_difficulty"), &"".to_string()),
            as_string(value.get("chain"), &"".to_string()),
            as_u8(value.get("eth_version"), 0),
        );

        peer_data
    }
}

pub fn as_string(val: Option<&AttributeValue>, default: &String) -> String {
    if let Some(v) = val {
        if let Ok(s) = v.as_s() {
            return s.to_owned();
        }
    }
    default.to_owned()
}

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

#[derive(Debug, Error)]
pub enum AddItemError {
    #[error("An error occurred adding a new item into the AWS database: {0}")]
    AwsAddItemError(#[from] SdkError<PutItemError>),
    #[error("An error occurred adding a new item into the in memory db")]
    InMemoryDbAddItemError(),
    #[error("An error occurred adding a new item into the SQL database: {0}")]
    SqlAddItemError(#[from] tokio_rusqlite::Error),
}

#[derive(Debug, Error)]
pub enum ScanTableError {
    #[error("An error occurred while performing a scan of the AWS database: {0}")]
    AwsScanError(#[from] SdkError<ScanError>),
    #[error("An error occurred while performing a scan of the in memory database")]
    InMemoryDbScanError(),
    #[error("An error occurred while performing a scan of the SQL database: {0}")]
    SqlScanError(#[from] tokio_rusqlite::Error),
}

#[derive(Debug, Error)]
pub enum QueryItemError {
    #[error("An error occurred querying the AWS database: {0}")]
    AwsQueryItemError(#[from] SdkError<QueryError>),
    #[error("An error occurred querying the in memory database")]
    InMemoryDbQueryItemError(),
    #[error("An error occurred querying the SQL database: {0}")]
    SqlQueryItemError(#[from] tokio_rusqlite::Error),
}

#[derive(Debug, Error)]
pub enum DeleteItemError {
    #[error("An error occurred deleting a new item into the SQL database: {0}")]
    SqlDeleteItemError(#[from] tokio_rusqlite::Error),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ClientData {
    pub client_version: String,
}
