use aws_config::meta::region::RegionProviderChain;
use aws_sdk_dynamodb::error::SdkError;
use aws_sdk_dynamodb::operation::put_item::PutItemError;
use aws_sdk_dynamodb::types::{
    AttributeDefinition, AttributeValue, KeySchemaElement, KeyType, ProvisionedThroughput,
    ScalarAttributeType, Select, TableStatus,
};
use aws_sdk_dynamodb::{config::Region, meta::PKG_VERSION, Client, Error};

use crate::types::PeerData;

#[derive(Clone)]
pub struct PeerDB {
    client: Client,
}

impl PeerDB {
    pub(crate) async fn new() -> Self {
        let region_provider =
            RegionProviderChain::default_provider().or_else(Region::new("us-west-2"));
        let shared_config = aws_config::from_env().region(region_provider).load().await;
        let client = Client::new(&shared_config);

        PeerDB { client }
    }

    pub(crate) async fn add_peer(&self, peer_data: PeerData) -> Result<(), SdkError<PutItemError>> {
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
}
