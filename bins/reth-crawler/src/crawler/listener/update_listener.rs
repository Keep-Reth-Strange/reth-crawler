use crate::crawler::BlockHashNumHandle;
use crate::p2p::{handshake_eth, handshake_p2p};
use chrono::Utc;
use ethers::types::H256;
use futures::StreamExt;
use ipgeolocate::{Locator, Service};
use reth_crawler_db::db::PostgreSQLPeerDb;
use reth_crawler_db::{save_peer, PeerDB, PeerData, SqlPeerDB};
use reth_discv4::{DiscoveryUpdate, Discv4};
use reth_dns_discovery::{DnsDiscoveryHandle, DnsNodeRecordUpdate};
use reth_ecies::stream::ECIESStream;
use reth_eth_wire::{HelloMessage, P2PStream, Status};
use reth_network::{NetworkEvent, NetworkHandle};
use reth_primitives::{NodeRecord, PeerId};
use secp256k1::SecretKey;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time;
use tracing::info;

/// How many p2p failures we accept from a peer before banning it.
const P2P_FAILURE_THRESHOLD: u8 = 5;
/// Stop the async tasks for this duration in seconds so that the state could be properly initialized!
const SLEEP_TIME: u64 = 30;

/// Listener that starts all services:
/// - discovery
/// - dns discovery
/// - network
pub(crate) struct UpdateListener {
    /// Discovery protocol handle.
    discv4: Discv4,
    /// Dns discovery protocol handle.
    dnsdisc: DnsDiscoveryHandle,
    /// Network handle for incoming requests.
    network: NetworkHandle,
    /// State handle for `BlockUpdate` and `HashRequest` requests.
    state_handle: BlockHashNumHandle,
    /// Private key of the node of the crawler.
    key: SecretKey,
    /// Database.
    db: Arc<dyn PeerDB>,
    /// Mapping between peer and number of p2p failures.
    p2p_failures: Arc<RwLock<HashMap<PeerId, u64>>>,
}

impl UpdateListener {
    /// Create a new `UpdateListener`.
    pub(crate) async fn new(
        discv4: Discv4,
        dnsdisc: DnsDiscoveryHandle,
        network: NetworkHandle,
        state_handle: BlockHashNumHandle,
        key: SecretKey,
        local_db: bool,
    ) -> Self {
        let p2p_failures = Arc::from(RwLock::from(HashMap::new()));
        if local_db {
            UpdateListener {
                discv4,
                dnsdisc,
                key,
                db: Arc::new(SqlPeerDB::new().await),
                network,
                p2p_failures,
                state_handle,
            }
        } else {
            UpdateListener {
                discv4,
                dnsdisc,
                key,
                db: Arc::new(PostgreSQLPeerDb::new().await),
                network,
                p2p_failures,
                state_handle,
            }
        }
    }

    /// Start discovery v4 protocol.
    pub(crate) async fn start_discv4(&self) -> eyre::Result<()> {
        time::sleep(Duration::from_secs(SLEEP_TIME)).await;
        let mut discv4_stream = self.discv4.update_stream().await?;
        let key = self.key;
        info!("discv4 is starting...");
        while let Some(update) = discv4_stream.next().await {
            let state_handle = self.state_handle.clone();
            let db = self.db.clone();
            let captured_discv4 = self.discv4.clone();
            let p2p_failures = self.p2p_failures.clone();
            if let DiscoveryUpdate::Added(peer) | DiscoveryUpdate::DiscoveredAtCapacity(peer) =
                update
            {
                tokio::spawn(async move {
                    handshake_and_save_peer(
                        captured_discv4,
                        p2p_failures,
                        key,
                        peer,
                        state_handle,
                        db,
                    )
                    .await;
                });
            }
        }
        Ok(())
    }

    /// Start dns discovery protocol.
    pub(crate) async fn start_dnsdisc(&self) -> eyre::Result<()> {
        time::sleep(Duration::from_secs(SLEEP_TIME)).await;
        let mut dnsdisc_update_stream = self.dnsdisc.node_record_stream().await?;
        let key = self.key;
        info!("dnsdisc is starting...");
        while let Some(update) = dnsdisc_update_stream.next().await {
            let state_handle = self.state_handle.clone();
            let db = self.db.clone();
            let p2p_failures = self.p2p_failures.clone();
            let captured_discv4 = self.discv4.clone();
            let DnsNodeRecordUpdate {
                node_record: peer, ..
            } = update;
            tokio::spawn(async move {
                handshake_and_save_peer(captured_discv4, p2p_failures, key, peer, state_handle, db)
                    .await;
            });
        }
        Ok(())
    }

    /// Start network for handling incoming connections.
    pub(crate) async fn start_network(&self) -> eyre::Result<()> {
        time::sleep(Duration::from_secs(SLEEP_TIME)).await;
        let mut net_events = self.network.event_listener();
        info!("network is starting...");
        while let Some(event) = net_events.next().await {
            match event {
                NetworkEvent::SessionEstablished {
                    peer_id,
                    remote_addr,
                    client_version,
                    capabilities,
                    status,
                    version,
                    ..
                } => {
                    info!(
                        "Session Established with peer {}",
                        remote_addr.ip().to_string()
                    );
                    let state_handle = self.state_handle.clone();
                    let db = self.db.clone();
                    let peer_handle = self.network.peers_handle().clone();
                    tokio::spawn(async move {
                        let address = remote_addr.ip().to_string();
                        // these peers inflate our numbers, same IP multiple generated ID
                        // TODO: ban them, but this isn't controlled by disc, and ban_ip semantics don't seem public to peers/network handles (?) - maybe peer_handle::reputation_change
                        if client_version.is_empty() {
                            info!("Peer {} with empty client_version - returning", address);
                            return;
                        }
                        // immediately disconnect the peer since we don't need any data from it
                        peer_handle.remove_peer(peer_id);

                        // get peer's info
                        let enode_url = NodeRecord::new(remote_addr, peer_id);
                        let capabilities = capabilities
                            .as_ref()
                            .capabilities()
                            .to_vec()
                            .iter()
                            .map(|cap| cap.to_string())
                            .collect();
                        let chain = status.chain.to_string();
                        let total_difficulty = status.total_difficulty.to_string();
                        let best_block = status.blockhash.to_string();
                        let genesis_block_hash = status.genesis.to_string();
                        let last_seen = Utc::now().to_string();
                        let (country, city, isp) = geolocate(&address).await;
                        let synced = is_synced(state_handle, status.blockhash.0.into()).await;
                        let enode_url = enode_url.to_string();
                        let id = peer_id.to_string();
                        let tcp_port = remote_addr.port();
                        let (
                            client_name,
                            client_version,
                            client_build,
                            client_arch,
                            os,
                            client_language,
                        ) = split_version(&client_version);
                        let eth_version = u8::from(version);

                        let peer_data = PeerData::new(
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
                            genesis_block_hash,
                            best_block,
                            total_difficulty,
                            chain,
                            eth_version,
                            synced,
                            isp,
                        );
                        save_peer(peer_data, db).await;
                    });
                }
                NetworkEvent::PeerAdded(_) | NetworkEvent::PeerRemoved(_) => {}
                NetworkEvent::SessionClosed { peer_id, reason } => {
                    if let Some(reason) = reason {
                        info!(
                            "Session closed with peer {} for {}",
                            peer_id.to_string(),
                            reason
                        )
                    }
                }
            }
        }
        Ok(())
    }
}

/// Check if peer is synced with the latest chain's blocks.
async fn is_synced(state_handle: BlockHashNumHandle, hash: H256) -> Option<bool> {
    let synced: Option<bool>;
    if let Ok(result) = state_handle.is_block_hash_present(hash).await {
        synced = Some(result);
    } else {
        synced = None;
    }
    synced
}

/// Gelocate a peer.
async fn geolocate(ip_addr: &str) -> (String, String, String) {
    let service = Service::IpApi;
    let mut country = String::default();
    let mut city = String::default();
    let mut isp = String::default();
    if let Ok(loc) = Locator::get(ip_addr, service).await {
        country = loc.country;
        city = loc.city;
        isp = loc.isp;
    }
    (country, city, isp)
}

/// Helper function for p2p handshake.
async fn handshake_p2p_handle(
    captured_discv4: &Discv4,
    p2p_failures: Arc<RwLock<HashMap<PeerId, u64>>>,
    key: SecretKey,
    peer: NodeRecord,
) -> Option<(P2PStream<ECIESStream<TcpStream>>, HelloMessage)> {
    // kick a forced lookup
    captured_discv4.send_lookup(peer.id);
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
                return None;
            }
            p2p_failure_count += 1;
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
                return None;
            }
            // scope guard to drop wlock
            {
                // increment failure count
                let mut wlock = p2p_failures.write().unwrap();
                wlock.insert(peer.id, p2p_failure_count);
            }
            return None;
        }
    };
    Some((p2p_stream, their_hello))
}

/// Helper function for eth status handshake.
async fn handshake_eth_handle(
    captured_discv4: &Discv4,
    p2p_stream: P2PStream<ECIESStream<TcpStream>>,
    peer: NodeRecord,
) -> Option<Status> {
    let (_, their_status) = match handshake_eth(p2p_stream).await {
        Ok(s) => s,
        Err(e) => {
            info!("Failed ETH handshake with peer {}, {}", peer.address, e);
            // ban the peer permanently - we never want to process another disc packet for this again since we know its not on the same network
            captured_discv4.ban_ip(peer.address);
            return None;
        }
    };
    Some(their_status)
}

/// Helper function for handling handshakes and saving to the database.
async fn handshake_and_save_peer(
    captured_discv4: Discv4,
    p2p_failures: Arc<RwLock<HashMap<PeerId, u64>>>,
    key: SecretKey,
    peer: NodeRecord,
    state_handle: BlockHashNumHandle,
    db: Arc<dyn PeerDB>,
) {
    // handshake p2p
    let (p2p_stream, their_hello) = if let Some((p2p_stream, their_hello)) =
        handshake_p2p_handle(&captured_discv4, p2p_failures, key, peer).await
    {
        (p2p_stream, their_hello)
    } else {
        return;
    };

    // handshake eth
    let their_status = if let Some(their_status) =
        handshake_eth_handle(&captured_discv4, p2p_stream, peer).await
    {
        their_status
    } else {
        return;
    };

    // if client version is empty, skip that peer
    if their_hello.client_version.is_empty() {
        info!(
            "Peer {} with empty client_version - returning",
            peer.address
        );
        // ban their IP - since our results show that we have multiple PeerIDs with the same IPs and no ClientVersion
        captured_discv4.ban_ip(peer.address);
        return;
    }

    // get peer's info
    let last_seen = Utc::now().to_string();
    let address = peer.address.to_string();
    let (country, city, isp) = geolocate(&address).await;
    let capabilities: Vec<String> = their_hello
        .capabilities
        .iter()
        .map(|cap| cap.to_string())
        .collect();
    let chain = their_status.chain.to_string();
    let total_difficulty = their_status.total_difficulty.to_string();
    let best_block = their_status.blockhash.to_string();
    let genesis_block_hash = their_status.genesis.to_string();
    let synced = is_synced(state_handle, their_status.blockhash.0.into()).await;
    let enode_url = peer.to_string();
    let id = peer.id.to_string();
    let tcp_port = peer.tcp_port;
    let (client_name, client_version, client_build, client_arch, os, client_language) =
        split_version(&their_hello.client_version);
    let eth_version = their_status.version;

    info!(
        "Successfully connected to a peer at {}:{} ({}) using eth-wire version eth/{:#?}",
        peer.address, peer.tcp_port, their_hello.client_version, their_hello.protocol_version
    );

    // collect data into `PeerData` and save it
    let peer_data = PeerData::new(
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
        genesis_block_hash,
        best_block,
        total_difficulty,
        chain,
        eth_version,
        synced,
        isp,
    );
    save_peer(peer_data, db).await;
}

/// Separate the long `client_version` field from the hello handshake message into:
/// - `client_name`
/// - `client_version`
/// - `client_build`
/// - `client_arch`
/// - `os`
/// - `client_language`
fn split_version(input: &str) -> (String, String, String, String, String, String) {
    let split: Vec<&str> = input.split('/').collect();
    if split.len() == 4 {
        let client_name = split[0].to_string();
        let (client_version, client_build) = split_at_dash_or_plus(split[1]);
        let (os, client_arch) = split_at_dash_or_plus(split[2]);
        let client_language = split[3].to_string();
        (
            client_name,
            client_version,
            client_build,
            client_arch,
            os,
            client_language,
        )
    } else {
        (
            input.to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
        )
    }
}

/// Helper function to separate a string based on the first `-`.
///
/// If it doesn't find a `-` it tries with a `+` (thanks Nethermind...).
fn split_at_dash_or_plus(input: &str) -> (String, String) {
    let split: Option<(&str, &str)> = input.split_once('-');
    if let Some((before, after)) = split {
        (before.to_string(), after.to_string())
    } else {
        let split: Option<(&str, &str)> = input.split_once('+');
        if let Some((before, after)) = split {
            (before.to_string(), after.to_string())
        } else {
            (input.to_string(), "".to_string())
        }
    }
}
