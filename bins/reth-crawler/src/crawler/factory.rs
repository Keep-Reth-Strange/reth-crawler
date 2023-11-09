use once_cell::sync::Lazy;
use reth_discv4::{Discv4, Discv4ConfigBuilder, DEFAULT_DISCOVERY_ADDRESS};
use reth_dns_discovery::{DnsDiscoveryConfig, DnsDiscoveryService, DnsResolver};

use reth_network::config::rng_secret_key;
use reth_network::{NetworkConfig, NetworkManager, PeersConfig};
use reth_primitives::{mainnet_nodes, NodeRecord};
use reth_provider::test_utils::NoopProvider;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

use crate::crawler::CrawlerService;

pub static MAINNET_BOOT_NODES: Lazy<Vec<NodeRecord>> = Lazy::new(mainnet_nodes);

/// Builder for a [`CrawlerService`]
#[derive(Clone, Debug)]
pub struct CrawlerBuilder {
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
    pub fn with_local_db(mut self) -> Self {
        self.local_db = true;
        self
    }

    /// Disable the local db
    pub fn without_local_db(mut self) -> Self {
        self.local_db = false;
        self
    }

    /// Set the eth rpc url
    pub fn with_eth_rpc_url(mut self, eth_rpc_url: String) -> Self {
        self.eth_rpc_url = Some(eth_rpc_url);
        self
    }

    /// Build the [`CrawlerService`]
    pub async fn build(self) -> CrawlerService {
        // Ensure the rpc url is set
        let provider_url = self.eth_rpc_url.expect("eth rpc url must be provided");

        // Setup configs related to this 'node' by creating a new random
        let key = rng_secret_key();
        let enr = NodeRecord::from_secret_key(DEFAULT_DISCOVERY_ADDRESS, &key);
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
            .peer_config(peer_config);

        let net_conf = builder.build(Arc::from(NoopProvider::default()));
        let network = NetworkManager::new(net_conf).await.unwrap();
        let net_handle = network.handle().clone();

        let dnsdisc_cfg = DnsDiscoveryConfig::default();
        // Start discovery protocol
        let discv4 = Discv4::spawn(enr.udp_addr(), enr, key, discv4_cfg.build())
            .await
            .unwrap();
        let (dns_disc_service, dnsdisc) = DnsDiscoveryService::new_pair(
            Arc::new(DnsResolver::from_system_conf().unwrap()),
            dnsdisc_cfg,
        );
        dns_disc_service.spawn();
        tokio::spawn(network);

        CrawlerService::new(
            discv4,
            dnsdisc,
            net_handle,
            key,
            self.local_db,
            provider_url,
        )
        .await
    }
}
