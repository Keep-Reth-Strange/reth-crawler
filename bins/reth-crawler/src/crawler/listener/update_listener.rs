use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::p2p::{handshake_eth, handshake_p2p};
use chrono::Utc;
use futures::StreamExt;
use ipgeolocate::{Locator, Service};
use reth_crawler_db::{save_peer, PeerDB, PeerData};
use reth_discv4::{DiscoveryUpdate, Discv4};
use reth_dns_discovery::{DnsDiscoveryHandle, DnsNodeRecordUpdate};
use reth_primitives::{NodeRecord, PeerId};
use secp256k1::SecretKey;
use tokio::sync::mpsc::UnboundedSender;
use tracing::info;

pub struct UpdateListener {
    discv4: Discv4,
    dnsdisc: DnsDiscoveryHandle,
    key: SecretKey,
    db: PeerDB,
    p2p_failures: Arc<RwLock<HashMap<PeerId, u64>>>,
}

const P2P_FAILURE_THRESHOLD: u8 = 5;

impl UpdateListener {
    pub async fn new(
        discv4: Discv4,
        dnsdisc: DnsDiscoveryHandle,
        key: SecretKey,
        node_tx: UnboundedSender<Vec<NodeRecord>>,
    ) -> Self {
        let db = PeerDB::new().await;
        let p2p_failures = Arc::from(RwLock::from(HashMap::new()));
        UpdateListener {
            discv4,
            dnsdisc,
            key,
            db,
            p2p_failures,
        }
    }

    pub async fn start_discv4(&self, save_to_json: bool) -> eyre::Result<()> {
        let mut discv4_stream = self.discv4.update_stream().await?;
        let key = self.key;
        while let Some(update) = discv4_stream.next().await {
            let db: PeerDB = self.db.clone();
            let captured_discv4 = self.discv4.clone();
            let p2p_failures = self.p2p_failures.clone();
            if let DiscoveryUpdate::Added(peer) | DiscoveryUpdate::DiscoveredAtCapacity(peer) =
                update
            {
                tokio::spawn(async move {
                    // kick a forced lookup
                    let _ = captured_discv4.lookup(peer.id).await;
                    let mut p2p_failure_count: u64;
                    {
                        let rlock = p2p_failures.read().unwrap();
                        p2p_failure_count = *rlock.get(&peer.id).unwrap_or(&0);
                    }
                    let (p2p_stream, their_hello) = match handshake_p2p(peer, key).await {
                        Ok(s) => s,
                        Err(e) => {
                            info!("Failed P2P handshake with peer {}, {}", peer.address, e);
                            if e.to_string().contains("Too many peers") {
                                info!("Skip counting p2p_failure for peer: {}", peer.address);
                                return;
                            }
                            p2p_failure_count = p2p_failure_count + 1;
                            if p2p_failure_count >= P2P_FAILURE_THRESHOLD as u64 {
                                // ban this peer - TODO: we probably want Discv4Service::ban_until() semantics here, but that isn't exposed to us
                                // for now - permaban
                                info!(
                                    "PeerId {} has failed p2p handshake {} times, banning",
                                    peer.id, p2p_failure_count
                                );
                                captured_discv4.ban_ip(peer.address);
                                // scope guard to drop wlock
                                {
                                    // reset count to 0 since we've now banned
                                    let mut wlock = p2p_failures.write().unwrap();
                                    wlock.insert(peer.id, 0);
                                }
                                return;
                            }
                            // scope guard to drop wlock
                            {
                                // increment failure count
                                let mut wlock = p2p_failures.write().unwrap();
                                wlock.insert(peer.id, p2p_failure_count);
                            }
                            return;
                        }
                    };

                    let (eth_stream, their_status) = match handshake_eth(p2p_stream).await {
                        Ok(s) => s,
                        Err(e) => {
                            info!("Failed ETH handshake with peer {}, {}", peer.address, e);
                            // ban the peer permanently - we never want to process another disc packet for this again since we know its not on the same network
                            captured_discv4.ban_ip(peer.address);
                            return;
                        }
                    };

                    if their_hello.client_version.is_empty() {
                        info!(
                            "Peer {} with empty client_version - returning",
                            peer.address
                        );
                        // ban their IP - since our results show that we have multiple PeerIDs with multiple IPs and no ClientVersion
                        captured_discv4.ban_ip(peer.address);
                        return;
                    }
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
            let db: PeerDB = self.db.clone();
            let p2p_failures = self.p2p_failures.clone();
            let captured_discv4 = self.discv4.clone();
            let DnsNodeRecordUpdate {
                node_record: peer,
                fork_id,
            } = update;
            tokio::spawn(async move {
                // kick a forced lookup
                let _ = captured_discv4.lookup(peer.id).await;
                let mut p2p_failure_count: u64;
                {
                    let rlock = p2p_failures.read().unwrap();
                    p2p_failure_count = *rlock.get(&peer.id).unwrap_or(&0);
                }
                let (p2p_stream, their_hello) = match handshake_p2p(peer, key).await {
                    Ok(s) => s,
                    Err(e) => {
                        info!("Failed P2P handshake with peer {}, {}", peer.address, e);
                        if e.to_string().contains("Too many peers") {
                            info!("Skip counting p2p_failure for peer: {}", peer.address);
                            return;
                        }
                        p2p_failure_count = p2p_failure_count + 1;
                        if p2p_failure_count >= P2P_FAILURE_THRESHOLD as u64 {
                            // ban this peer - TODO: we probably want Discv4Service::ban_until() semantics here, but that isn't exposed to us
                            // for now - permaban
                            info!(
                                "PeerId {} has failed p2p handshake {} times, banning",
                                peer.id, p2p_failure_count
                            );
                            captured_discv4.ban_ip(peer.address);
                            // scope guard to drop wlock
                            {
                                // reset count to 0 since we've now banned
                                let mut wlock = p2p_failures.write().unwrap();
                                wlock.insert(peer.id, 0);
                            }
                            return;
                        }
                        // scope guard to drop wlock
                        {
                            // increment failure count
                            let mut wlock = p2p_failures.write().unwrap();
                            wlock.insert(peer.id, p2p_failure_count);
                        }
                        return;
                    }
                };

                let (_eth_stream, their_status) = match handshake_eth(p2p_stream).await {
                    Ok(s) => s,
                    Err(e) => {
                        info!("Failed ETH handshake with peer {}, {}", peer.address, e);
                        // ban the peer permanently - we never want to process another disc packet for this again since we know its not on the same network
                        captured_discv4.ban_ip(peer.address);
                        return;
                    }
                };
                if their_hello.client_version.is_empty() {
                    info!(
                        "Peer {} with empty client_version - returning",
                        peer.address
                    );
                    // ban their IP - since our results show that we have multiple PeerIDs with multiple IPs and no ClientVersion
                    captured_discv4.ban_ip(peer.address);
                    return;
                }
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
