use crate::crawler::{BlockHashNum, CrawlerService};
use ethers::providers::{Provider, Ws};
use once_cell::sync::Lazy;
use reth_discv4::{Discv4, Discv4ConfigBuilder, DEFAULT_DISCOVERY_ADDRESS, DEFAULT_DISCOVERY_PORT};
use reth_dns_discovery::{DnsDiscoveryConfig, DnsDiscoveryService, DnsResolver};
use reth_network::config::rng_secret_key;
use reth_network::{NetworkConfig, NetworkManager, PeersConfig};
use reth_primitives::{mainnet_nodes, NodeRecord};
use reth_provider::test_utils::NoopProvider;
use std::sync::Arc;
use std::time::Duration;

/// Ethereum mainnet boot nodes.
///
/// These are hard coded Ethereum mainnet nodes that are helpful when a node runs for the first time and needs to find some other nodes.
static MAINNET_BOOT_NODES: Lazy<Vec<NodeRecord>> = Lazy::new(mainnet_nodes);

/// Builder for a [`CrawlerService`]
#[derive(Clone, Debug)]
pub(crate) struct CrawlerBuilder {
    /// Whether or not to use a local db
    local_db: bool,
    /// Eth RPC url
    eth_rpc_url: Option<String>,
    /// Max inbound connections for the crawler
    max_inbound: usize,
    /// Max outbound connections for the crawler
    max_outbound: usize,
    /// The lookup interval for the crawler
    lookup_interval: Duration,
}

impl Default for CrawlerBuilder {
    fn default() -> Self {
        Self {
            local_db: false,
            eth_rpc_url: None,
            max_inbound: 10000,
            max_outbound: 0,
            lookup_interval: Duration::from_secs(3),
        }
    }
}

impl CrawlerBuilder {
    /// Enable the local db
    pub(crate) fn with_local_db(mut self) -> Self {
        self.local_db = true;
        self
    }

    /// Disable the local db
    pub(crate) fn without_local_db(mut self) -> Self {
        self.local_db = false;
        self
    }

    /// Set the eth rpc url
    pub(crate) fn with_eth_rpc_url(mut self, eth_rpc_url: String) -> Self {
        self.eth_rpc_url = Some(eth_rpc_url);
        self
    }

    /// Build the [`CrawlerService`]
    pub(crate) async fn build(self, n_nodes: u16) -> CrawlerService {
        // Ensure the rpc url is set
        let provider_url = self.eth_rpc_url.expect("eth rpc url must be provided");
        // Create the connection with the provider
        let provider = Provider::<Ws>::connect(provider_url)
            .await
            .expect("provider must be set up correctly");

        let mut all_discv4 = vec![];
        let mut all_dnsdisc = vec![];
        let mut all_network = vec![];
        let mut all_key = vec![];

        // create 2 crawlers
        for i in 0..n_nodes {
            // Setup configs related to this 'node' by creating a new random
            let key = rng_secret_key();
            let mut enr = NodeRecord::from_secret_key(DEFAULT_DISCOVERY_ADDRESS, &key);
            enr.udp_port = DEFAULT_DISCOVERY_PORT + i;
            // Setup discovery v4 protocol to find peers to talk to
            let mut discv4_cfg = Discv4ConfigBuilder::default();
            discv4_cfg
                .add_boot_nodes(MAINNET_BOOT_NODES.clone())
                .lookup_interval(self.lookup_interval);

            let peer_config = PeersConfig::default()
                .with_max_outbound(self.max_outbound)
                .with_max_inbound(self.max_inbound);

            // disable discovery here since we already handle outbound connections (devp2p/eth handshakes in our case) for newly discovered peers "manually", and do not need Swarm/NetworkState to handle those outbound handshakes for us
            // we do however want inbound TCP (note: discv4 listens only for udp disc proto messages) connections to be handled
            let builder = NetworkConfig::<()>::builder(key)
                .disable_discovery()
                .peer_config(peer_config)
                .listener_port(DEFAULT_DISCOVERY_PORT + i);

            // spawn network
            let net_conf = builder.build(Arc::from(NoopProvider::default()));
            let network = NetworkManager::new(net_conf).await.unwrap();
            let net_handle = network.handle().clone();
            tokio::spawn(network);

            // start discovery protocol
            let discv4 = Discv4::spawn(enr.udp_addr(), enr, key, discv4_cfg.build())
                .await
                .unwrap();

            // dns discovery service
            let dnsdisc_cfg = DnsDiscoveryConfig::default();
            let (dns_disc_service, dnsdisc) = DnsDiscoveryService::new_pair(
                Arc::new(DnsResolver::from_system_conf().unwrap()),
                dnsdisc_cfg,
            );
            dns_disc_service.spawn();

            // push every item into the vecs
            all_discv4.push(discv4);
            all_dnsdisc.push(dnsdisc);
            all_network.push(net_handle);
            all_key.push(key);
        }

        // spawn state for retrieve block hashes from provider
        let (state, state_handle) = BlockHashNum::new();
        tokio::spawn(state);

        CrawlerService::new(
            all_discv4,
            all_dnsdisc,
            all_network,
            state_handle,
            all_key,
            self.local_db,
            provider,
        )
        .await
    }
}
