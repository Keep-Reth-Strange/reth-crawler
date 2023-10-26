use crate::p2p::{handshake_eth, handshake_p2p};
use chrono::Utc;
use futures::StreamExt;
use ipgeolocate::{Locator, Service};
use once_cell::sync::Lazy;
use reth_crawler_db::{save_peer, AwsPeerDB, PeerDB, PeerData, SqlPeerDB};
use reth_discv4::{DiscoveryUpdate, Discv4};
use reth_dns_discovery::{DnsDiscoveryHandle, DnsNodeRecordUpdate};
use reth_primitives::{mainnet_nodes, NodeRecord};
use secp256k1::SecretKey;
use std::{sync::Arc, time::Instant};
use tokio::sync::mpsc::UnboundedSender;
use tracing::info;

pub struct UpdateListener {
    discv4: Discv4,
    dnsdisc: DnsDiscoveryHandle,
    key: SecretKey,
    node_tx: UnboundedSender<Vec<NodeRecord>>,
    db: Arc<dyn PeerDB>,
}

impl UpdateListener {
    pub async fn new(
        discv4: Discv4,
        dnsdisc: DnsDiscoveryHandle,
        key: SecretKey,
        node_tx: UnboundedSender<Vec<NodeRecord>>,
        sql_db: bool,
    ) -> Self {
        if sql_db {
            UpdateListener {
                discv4,
                dnsdisc,
                key,
                node_tx,
                db: Arc::new(SqlPeerDB::new().await),
            }
        } else {
            UpdateListener {
                discv4,
                dnsdisc,
                key,
                node_tx,
                db: Arc::new(AwsPeerDB::new().await),
            }
        }
    }

    pub async fn start_discv4(&self, save_to_json: bool) -> eyre::Result<()> {
        let mut discv4_stream = self.discv4.update_stream().await?;
        let key = self.key;
        while let Some(update) = discv4_stream.next().await {
            let captured_discv4 = self.discv4.clone();
            let node_tx = self.node_tx.clone();
            let db = self.db.clone();
            if let DiscoveryUpdate::Added(peer) | DiscoveryUpdate::DiscoveredAtCapacity(peer) =
                update
            {
                tokio::spawn(async move {
                    let (p2p_stream, their_hello) = match handshake_p2p(peer, key).await {
                        Ok(s) => s,
                        Err(e) => {
                            info!("Failed P2P handshake with peer {}, {}", peer.address, e);
                            return;
                        }
                    };

                    let (eth_stream, their_status) = match handshake_eth(p2p_stream).await {
                        Ok(s) => s,
                        Err(e) => {
                            info!("Failed ETH handshake with peer {}, {}", peer.address, e);
                            return;
                        }
                    };

                    let last_seen = Utc::now().to_string();

                    info!(
                        "Successfully connected to a peer at {}:{} ({}) using eth-wire version eth/{:#?}",
                        peer.address, peer.tcp_port, their_hello.client_version, their_hello.protocol_version
                    );

                    // get peer location
                    let service = Service::IpApi;
                    let ip_addr = peer.address.to_string();

                    let mut country = String::default();
                    let mut city = String::default();

                    match Locator::get(&ip_addr, service).await {
                        Ok(loc) => {
                            country = loc.country;
                            city = loc.city;
                        }
                        Err(_) => {
                            // leave `country` and `city` empty if not able to get them
                        }
                    }

                    let capabilities: Vec<String> = their_hello
                        .capabilities
                        .iter()
                        .map(|cap| cap.to_string())
                        .collect();

                    let chain = their_status.chain.to_string();

                    let total_difficulty = their_status.total_difficulty.to_string();
                    let best_block = their_status.blockhash.to_string();
                    let genesis_block_hash = their_status.genesis.to_string();

                    // collect data into `PeerData`
                    let peer_data = PeerData {
                        enode_url: peer.to_string(),
                        id: peer.id.to_string(),
                        address: ip_addr,
                        tcp_port: peer.tcp_port,
                        client_version: their_hello.client_version.clone(),
                        eth_version: their_status.version,
                        capabilities,
                        total_difficulty,
                        chain,
                        best_block,
                        genesis_block_hash,
                        last_seen,
                        country,
                        city,
                    };
                    save_peer(peer_data, save_to_json, db).await;
                });
            }
        }
        Ok(())
    }

    pub async fn start_dnsdisc(&self, save_to_json: bool) -> eyre::Result<()> {
        let mut dnsdisc_update_stream = self.dnsdisc.node_record_stream().await?;
        let key = self.key;
        while let Some(update) = dnsdisc_update_stream.next().await {
            let db = self.db.clone();
            let DnsNodeRecordUpdate {
                node_record: peer,
                fork_id,
            } = update;
            tokio::spawn(async move {
                let (p2p_stream, their_hello) = match handshake_p2p(peer, key).await {
                    Ok(s) => s,
                    Err(e) => {
                        info!("Failed P2P handshake with peer {}, {}", peer.address, e);
                        return;
                    }
                };

                let (_eth_stream, their_status) = match handshake_eth(p2p_stream).await {
                    Ok(s) => s,
                    Err(e) => {
                        info!("Failed ETH handshake with peer {}, {}", peer.address, e);
                        return;
                    }
                };

                let last_seen = Utc::now().to_string();

                info!(
                        "Successfully connected to a peer at {}:{} ({}) using eth-wire version eth/{:#?}",
                        peer.address, peer.tcp_port, their_hello.client_version, their_hello.protocol_version
                    );
                // get peer location
                let service = Service::IpApi;
                let ip_addr = peer.address.to_string();

                let mut country = String::default();
                let mut city = String::default();

                match Locator::get(&ip_addr, service).await {
                    Ok(loc) => {
                        country = loc.country;
                        city = loc.city;
                    }
                    Err(_) => {
                        // leave `country` and `city` empty if not able to get them
                    }
                }

                let capabilities: Vec<String> = their_hello
                    .capabilities
                    .iter()
                    .map(|cap| cap.to_string())
                    .collect();

                let chain = their_status.chain.to_string();

                let total_difficulty = their_status.total_difficulty.to_string();
                let best_block = their_status.blockhash.to_string();
                let genesis_block_hash = their_status.genesis.to_string();

                // collect data into `PeerData`
                let peer_data = PeerData {
                    enode_url: peer.to_string(),
                    id: peer.id.to_string(),
                    address: ip_addr,
                    tcp_port: peer.tcp_port,
                    client_version: their_hello.client_version.clone(),
                    eth_version: their_status.version,
                    capabilities,
                    total_difficulty,
                    chain,
                    best_block,
                    genesis_block_hash,
                    last_seen,
                    country,
                    city,
                };
                save_peer(peer_data, save_to_json, db).await;
            });
        }
        Ok(())
    }
}
