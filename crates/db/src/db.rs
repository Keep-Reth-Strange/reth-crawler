use aws_config::meta::region::RegionProviderChain;
use aws_sdk_dynamodb::error::SdkError;
use aws_sdk_dynamodb::operation::put_item::PutItemError;
use aws_sdk_dynamodb::operation::query::QueryError;
use aws_sdk_dynamodb::operation::scan::ScanError;
use aws_sdk_dynamodb::types::{
    AttributeDefinition, AttributeValue, KeySchemaElement, KeyType, ProvisionedThroughput,
    ScalarAttributeType, Select, TableStatus,
};
use aws_sdk_dynamodb::{config::Region, meta::PKG_VERSION, Client, Error};
use reth_crawler_types::PeerData;
use tokio_stream::StreamExt;

#[derive(Clone)]
pub struct PeerDB {
    client: Client,
}

impl PeerDB {
    pub async fn new() -> Self {
        let region_provider =
            RegionProviderChain::default_provider().or_else(Region::new("us-west-2"));
        let shared_config = aws_config::from_env().region(region_provider).load().await;
        let client = Client::new(&shared_config);

        PeerDB { client }
    }

    pub async fn add_peer(&self, peer_data: PeerData) -> Result<(), SdkError<PutItemError>> {
        let peer_id = AttributeValue::S(peer_data.id);
        let peer_ip = AttributeValue::S(peer_data.address);
        let client_version = AttributeValue::S(peer_data.client_version);
        let enode_url = AttributeValue::S(peer_data.enode_url);
        let port = AttributeValue::N(peer_data.tcp_port.to_string()); // numbers are sent over the network as string
                                                                      // let chain = AttributeValue::S(peer_data.chain);
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
            // .item("chain", chain)
            .item("country", country)
            .item("city", city)
            .item("last_seen", last_seen)
            .item("source_region", region_source)
            .send()
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }

    pub async fn all_peers(
        &self,
        page_size: Option<i32>,
    ) -> Result<Vec<PeerData>, SdkError<ScanError>> {
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
            Err(err) => Err(err),
        }
    }

    pub async fn node_by_id(
        &self,
        id: String,
    ) -> Result<Option<Vec<PeerData>>, SdkError<QueryError>> {
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

    pub async fn node_by_ip(
        &self,
        ip: String,
    ) -> Result<Option<Vec<PeerData>>, SdkError<QueryError>> {
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
