use once_cell::sync::Lazy;
use reth_discv4::{Discv4, Discv4ConfigBuilder, DEFAULT_DISCOVERY_ADDRESS};
use reth_dns_discovery::{
    DnsDiscoveryConfig, DnsDiscoveryHandle, DnsDiscoveryService, DnsResolver,
};

use reth_network::config::rng_secret_key;
use reth_network::{NetworkConfig, NetworkHandle, NetworkManager, PeersConfig};
use reth_primitives::{mainnet_nodes, NodeRecord};
use reth_provider::test_utils::NoopProvider;
use secp256k1::SecretKey;
use std::sync::Arc;
use std::time::Duration;

use crate::crawler::CrawlerService;

pub static MAINNET_BOOT_NODES: Lazy<Vec<NodeRecord>> = Lazy::new(mainnet_nodes);

pub struct CrawlerFactory {
    key: SecretKey,
    discv4: Discv4,
    dnsdisc: DnsDiscoveryHandle,
    network: NetworkHandle,
}

impl CrawlerFactory {
    pub async fn new() -> Self {
        // Setup configs related to this 'node' by creating a new random
        let key = rng_secret_key();
        let enr = NodeRecord::from_secret_key(DEFAULT_DISCOVERY_ADDRESS, &key);
        // Setup discovery v4 protocol to find peers to talk to
        let mut discv4_cfg = Discv4ConfigBuilder::default();
        discv4_cfg
            .add_boot_nodes(MAINNET_BOOT_NODES.clone())
            .lookup_interval(Duration::from_secs(3));

        let peer_config = PeersConfig::default()
            .with_max_outbound(0)
            .with_max_inbound(10000);

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

        Self {
            key,
            discv4,
            dnsdisc,
            network: net_handle,
        }
    }

    pub async fn make(&self, sql_db: bool) -> CrawlerService {
        CrawlerService::new(
            self.discv4.clone(),
            self.dnsdisc.clone(),
            self.network.clone(),
            self.key,
            sql_db,
        )
        .await
    }
}
