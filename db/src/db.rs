use crate::types::{AddItemError, PeerData, QueryItemError, ScanTableError};
use async_trait::async_trait;
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_dynamodb::types::{
    AttributeDefinition, AttributeValue, KeySchemaElement, KeyType, ProvisionedThroughput,
    ScalarAttributeType, Select, TableStatus,
};
use aws_sdk_dynamodb::{config::Region, meta::PKG_VERSION, Client, Error};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio_stream::StreamExt;

#[async_trait]
pub trait PeerDB: Send + Sync {
    async fn add_peer(&self, peer_data: PeerData) -> Result<(), AddItemError>;
    async fn all_peers(&self, page_size: Option<i32>) -> Result<Vec<PeerData>, ScanTableError>;
    async fn node_by_id(&self, id: String) -> Result<Option<Vec<PeerData>>, QueryItemError>;
    async fn node_by_ip(&self, ip: String) -> Result<Option<Vec<PeerData>>, QueryItemError>;
}

#[derive(Clone)]
pub struct AwsPeerDB {
    client: Client,
}

impl AwsPeerDB {
    pub async fn new() -> Self {
        let region_provider =
            RegionProviderChain::default_provider().or_else(Region::new("us-west-2"));
        let shared_config = aws_config::from_env().region(region_provider).load().await;
        let client = Client::new(&shared_config);

        AwsPeerDB { client }
    }
}

#[async_trait]
impl PeerDB for AwsPeerDB {
    async fn add_peer(&self, peer_data: PeerData) -> Result<(), AddItemError> {
        let peer_id = AttributeValue::S(peer_data.id);
        let peer_ip = AttributeValue::S(peer_data.address);
        let client_version = AttributeValue::S(peer_data.client_version);
        let enode_url = AttributeValue::S(peer_data.enode_url);
        let port = AttributeValue::N(peer_data.tcp_port.to_string()); // numbers are sent over the network as string
        let chain = AttributeValue::S(peer_data.chain);
        let genesis_hash = AttributeValue::S(peer_data.genesis_block_hash);
        let best_block = AttributeValue::S(peer_data.best_block);
        let total_difficulty = AttributeValue::S(peer_data.total_difficulty);
        let country = AttributeValue::S(peer_data.country);
        let city = AttributeValue::S(peer_data.city);
        let last_seen = AttributeValue::S(peer_data.last_seen);
        let region_source = AttributeValue::S(self.client.config().region().unwrap().to_string());

        match self
            .client
            .put_item()
            .table_name("eth-peer-data")
            .item("peer-id", peer_id)
            .item("peer-ip", peer_ip)
            .item("client_version", client_version)
            .item("enode_url", enode_url)
            .item("port", port)
            .item("chain", chain)
            .item("country", country)
            .item("city", city)
            .item("last_seen", last_seen)
            .item("source_region", region_source)
            .item("genesis_block_hash", genesis_hash)
            .item("best_block", best_block)
            .item("total_difficulty", total_difficulty)
            .send()
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    async fn all_peers(&self, page_size: Option<i32>) -> Result<Vec<PeerData>, ScanTableError> {
        let page_size = page_size.unwrap_or(50);
        let results: Result<Vec<_>, _> = self
            .client
            .scan()
            .table_name("eth-peer-data")
            .limit(page_size)
            .into_paginator()
            .items()
            .send()
            .collect()
            .await;

        match results {
            Ok(peers) => peers.iter().map(|peer| Ok(peer.into())).collect(),
            Err(err) => Err(err.into()),
        }
    }

    async fn node_by_id(&self, id: String) -> Result<Option<Vec<PeerData>>, QueryItemError> {
        let results = self
            .client
            .query()
            .table_name("eth-peer-data")
            .key_condition_expression("#id = :id")
            .expression_attribute_names("#id", "peer-id")
            .expression_attribute_values(":id", AttributeValue::S(id))
            .send()
            .await?;

        if let Some(nodes) = results.items {
            let node = nodes.iter().map(|v| v.into()).collect();
            Ok(Some(node))
        } else {
            Ok(None)
        }
    }

    async fn node_by_ip(&self, ip: String) -> Result<Option<Vec<PeerData>>, QueryItemError> {
        let results = self
            .client
            .query()
            .table_name("eth-peer-data")
            .index_name("peer-ip-index")
            .key_condition_expression("#ip = :ip")
            .expression_attribute_names("#ip", "peer-ip")
            .expression_attribute_values(":ip", AttributeValue::S(ip))
            .send()
            .await?;

        if let Some(nodes) = results.items {
            let node = nodes.iter().map(|v| v.into()).collect();
            Ok(Some(node))
        } else {
            Ok(None)
        }
    }
}

#[derive(Clone)]
pub struct InMemoryPeerDB {
    db: Arc<RwLock<HashMap<String, PeerData>>>,
}

impl InMemoryPeerDB {
    pub fn new() -> Self {
        Self {
            db: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl PeerDB for InMemoryPeerDB {
    async fn add_peer(&self, peer_data: PeerData) -> Result<(), AddItemError> {
        let mut db = self
            .db
            .write()
            .map_err(|_| AddItemError::InMemoryDbAddItemError())?;
        db.insert(peer_data.id.clone(), peer_data);
        Ok(())
    }

    async fn all_peers(&self, page_size: Option<i32>) -> Result<Vec<PeerData>, ScanTableError> {
        let page_size = page_size.unwrap_or(50);
        let db = self
            .db
            .read()
            .map_err(|_| ScanTableError::InMemoryDbScanError())?;
        Ok(db
            .iter()
            .map(|(_, peer_data)| peer_data.clone())
            .take(page_size as usize)
            .collect())
    }

    async fn node_by_id(&self, id: String) -> Result<Option<Vec<PeerData>>, QueryItemError> {
        let db = self
            .db
            .read()
            .map_err(|_| QueryItemError::InMemoryDbQueryItemError())?;
        Ok(Some(
            db.iter()
                .filter(|(peer_id, _)| **peer_id == id)
                .map(|(_, peer_data)| peer_data.clone())
                .collect(),
        ))
    }

    async fn node_by_ip(&self, ip: String) -> Result<Option<Vec<PeerData>>, QueryItemError> {
        let db = self
            .db
            .read()
            .map_err(|_| QueryItemError::InMemoryDbQueryItemError())?;
        Ok(Some(
            db.iter()
                .filter(|(_, peer_data)| peer_data.address == ip)
                .map(|(_, peer_data)| peer_data.clone())
                .collect(),
        ))
    }
}
