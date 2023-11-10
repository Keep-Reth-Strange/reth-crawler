use crate::p2p::{handshake_eth, handshake_p2p};
use chrono::Utc;
use ethers::providers::{Middleware, Provider, SubscriptionStream, Ws};
use ethers::types::{Block, H256, U64};
use futures::{Future, FutureExt, StreamExt};
use ipgeolocate::{Locator, Service};
use lru::LruCache;
use reth_crawler_db::{save_peer, AwsPeerDB, PeerDB, PeerData, SqlPeerDB};
use reth_discv4::{DiscoveryUpdate, Discv4};
use reth_dns_discovery::{DnsDiscoveryHandle, DnsNodeRecordUpdate};
use reth_network::{NetworkEvent, NetworkHandle};
use reth_primitives::{NodeRecord, PeerId};
use secp256k1::SecretKey;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio::sync::oneshot;
use tokio::time;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tracing::info;

const P2P_FAILURE_THRESHOLD: u8 = 5;
/// How many blocks can a node be lagging and still be considered `synced`.
const SYNCED_THRESHOLD: u64 = 100;
/// Stop the async tasks for this duration in seconds so that the state could be properly initialized!
const SLEEP_TIME: u64 = 12;

pub struct UpdateListener<'a> {
    discv4: Discv4,
    dnsdisc: DnsDiscoveryHandle,
    network: NetworkHandle,
    key: SecretKey,
    db: Arc<dyn PeerDB>,
    p2p_failures: Arc<RwLock<HashMap<PeerId, u64>>>,
    state_handle: BlockHashNumHandle,
    state: BlockHashNum<'a>,
}

/// This holds the mapping between block hash and block number of the latest `SYNCED_THRESHOLD` blocks.
pub struct BlockHashNum<'a> {
    /// Inner chache for mapping block hashes to block numbers.
    blocks_hash_to_number: LruCache<H256, U64>,
    /// Inner provider to use for block requests.
    provider: Provider<Ws>,
    /// Receiver half of the channel for block requests
    command_rx: UnboundedReceiverStream<HashRequest>,
    /// Copy of the sender half of the channel so handles can be created on demand.
    service_tx: UnboundedSender<HashRequest>,
    /// Block subscription stream.
    block_subscription: SubscriptionStream<'a, Ws, Block<H256>>,
}

impl<'a> BlockHashNum<'a> {
    /// Create a new service to resolve and cache block hashes / numbers mapping.
    pub fn new(
        provider: Provider<Ws>,
        block_subscription: SubscriptionStream<'a, Ws, Block<H256>>,
    ) -> (Self, BlockHashNumHandle) {
        let (service_tx, command_rx) = mpsc::unbounded_channel();
        let service = Self {
            blocks_hash_to_number: LruCache::new(
                NonZeroUsize::new(SYNCED_THRESHOLD as usize).expect("it's not zero!"),
            ),
            provider,
            command_rx: UnboundedReceiverStream::new(command_rx),
            service_tx,
            block_subscription,
        };

        let handle = service.handle();

        (service, handle)
    }

    /// Returns a handle to the service.
    pub fn handle(&self) -> BlockHashNumHandle {
        BlockHashNumHandle::new(self.service_tx.clone())
    }

    /// Backfill the inner LRU with the latest `SYNCED_THRESHOLD` blocks.
    pub async fn initialize(&mut self) -> eyre::Result<()> {
        let last_block_number = self.provider.get_block_number().await?;
        for block_number in
            (last_block_number.as_u64() - SYNCED_THRESHOLD)..=last_block_number.as_u64()
        {
            let block = self
                .provider
                .get_block(block_number)
                .await?
                .expect("it's not a pending block");
            let block_hash = block.hash.expect("it's not a pending block");
            let block_number = block.number.expect("it's not a pending block");
            self.blocks_hash_to_number.put(block_hash, block_number);
        }
        Ok(())
    }
}

pub struct HashRequest {
    /// The requested hash.
    hash: H256,
    /// The channel for returning the response.
    response: oneshot::Sender<bool>,
}

impl<'a> Future for BlockHashNum<'a> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        while let Poll::Ready(Some(request)) = this.command_rx.poll_next_unpin(cx) {
            // check if request.hash is inside the inner LRU
            let HashRequest { hash, response } = request;
            let is_present = this.blocks_hash_to_number.contains(&hash);
            let _ = response.send(is_present);
        }

        while let Poll::Ready(Some(block)) = this.block_subscription.poll_next_unpin(cx) {
            // handle new block updates here
            let block_hash = block.hash.expect("it's not a pending block");
            let block_number = block.number.expect("it's not a pending block");
            // update inner LRU
            this.blocks_hash_to_number.put(block_hash, block_number);
        }

        Poll::Pending
    }
}

/// A clone-able handle that sends requests to the block hash to num service
#[derive(Clone)]
pub struct BlockHashNumHandle {
    /// Sender half of the message channel.
    to_service: mpsc::UnboundedSender<HashRequest>,
}

impl BlockHashNumHandle {
    /// Create a new `BlockHashNumHandle`.
    pub(crate) fn new(to_service: mpsc::UnboundedSender<HashRequest>) -> Self {
        Self { to_service }
    }

    /// Send a `HashRequest` in the channel.
    pub async fn is_block_hash_present(&self, hash: H256) -> eyre::Result<bool> {
        let (tx, rx) = oneshot::channel();
        let hash_request = HashRequest { hash, response: tx };
        self.to_service.send(hash_request)?;
        rx.await.map_err(|err| err.into())
    }
}

impl<'a> UpdateListener<'a> {
    pub async fn new(
        discv4: Discv4,
        dnsdisc: DnsDiscoveryHandle,
        network: NetworkHandle,
        key: SecretKey,
        local_db: bool,
        provider_url: String,
    ) -> UpdateListener<'a> {
        let p2p_failures = Arc::from(RwLock::from(HashMap::new()));
        // initialize a new http provider
        let provider = Provider::<Ws>::connect(provider_url)
            .await
            .expect("Provider must work correctly!");
        // create block subscription
        let block_subscription = provider
            .subscribe_blocks()
            .await
            .expect("Block subscription must be created");
        // create `BlockHashNum`
        let (state, state_handle) = BlockHashNum::new(provider, block_subscription);
        if local_db {
            UpdateListener {
                discv4,
                dnsdisc,
                key,
                db: Arc::new(SqlPeerDB::new().await),
                network,
                p2p_failures,
                state_handle,
                state,
            }
        } else {
            UpdateListener {
                discv4,
                dnsdisc,
                key,
                db: Arc::new(AwsPeerDB::new().await),
                network,
                p2p_failures,
                state_handle,
                state,
            }
        }
    }

    pub async fn start_discv4(&self) -> eyre::Result<()> {
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
                                return;
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

                    let (_, their_status) = match handshake_eth(p2p_stream).await {
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
                        // ban their IP - since our results show that we have multiple PeerIDs with the same IPs and no ClientVersion
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
                    let mut isp = String::default();

                    if let Ok(loc) = Locator::get(&ip_addr, service).await {
                        country = loc.country;
                        city = loc.city;
                        isp = loc.isp;
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

                    // check if peer is synced with the latest chain's blocks
                    let synced: Option<bool>;
                    if let Ok(result) = state_handle
                        .is_block_hash_present(their_status.blockhash.0.into())
                        .await
                    {
                        synced = Some(result);
                    } else {
                        synced = None;
                    }

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
                        synced,
                        isp,
                    };
                    save_peer(peer_data, db).await;
                });
            }
        }
        Ok(())
    }

    pub async fn start_dnsdisc(&self) -> eyre::Result<()> {
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
                            return;
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
                    // ban their IP - since our results show that we have multiple PeerIDs with the same IP and no ClientVersion
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
                let mut isp = String::default();

                if let Ok(loc) = Locator::get(&ip_addr, service).await {
                    country = loc.country;
                    city = loc.city;
                    isp = loc.isp;
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

                // check if peer is synced with the latest chain's blocks
                let synced: Option<bool>;
                if let Ok(result) = state_handle
                    .is_block_hash_present(their_status.blockhash.0.into())
                    .await
                {
                    synced = Some(result);
                } else {
                    synced = None;
                }

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
                    synced,
                    isp,
                };
                save_peer(peer_data, db).await;
            });
        }
        Ok(())
    }

    pub async fn start_network(&self) {
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
                        // immediately disconnect the peer since we don't need any data from it
                        peer_handle.remove_peer(peer_id);
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
                        let mut country = String::default();
                        let mut city = String::default();
                        let mut isp = String::default();
                        let service = Service::IpApi;
                        let ip_addr = remote_addr.ip().to_string();

                        if let Ok(loc) = Locator::get(&ip_addr, service).await {
                            country = loc.country;
                            city = loc.city;
                            isp = loc.isp;
                        }
                        // these peers inflate our numbers, same IP multiple generated ID
                        // TODO: ban them, but this isn't controlled by disc, and ban_ip semantics don't seem public to peers/network handles (?) - maybe peer_handle::reputation_change
                        if client_version.is_empty() {
                            info!("Peer {} with empty client_version - returning", ip_addr);
                            return;
                        }

                        // check if peer is synced with the latest chain's blocks
                        let synced: Option<bool>;
                        if let Ok(result) = state_handle
                            .is_block_hash_present(status.blockhash.0.into())
                            .await
                        {
                            synced = Some(result);
                        } else {
                            synced = None;
                        }

                        let peer_data = PeerData {
                            enode_url: enode_url.to_string(),
                            id: peer_id.to_string(),
                            tcp_port: remote_addr.port(),
                            address: remote_addr.ip().to_string(),
                            client_version: client_version.to_string(),
                            capabilities,
                            eth_version: u8::from(version),
                            chain,
                            total_difficulty,
                            best_block,
                            genesis_block_hash,
                            last_seen,
                            country,
                            city,
                            synced,
                            isp,
                        };
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
    }

    pub async fn initialize_state(&mut self) -> eyre::Result<()> {
        self.state.initialize().await?;
        Ok(())
    }
}
