use once_cell::sync::Lazy;
use reth_discv4::{Discv4, Discv4ConfigBuilder, DEFAULT_DISCOVERY_ADDRESS};
use reth_dns_discovery::{
    DnsDiscoveryConfig, DnsDiscoveryHandle, DnsDiscoveryService, DnsNodeRecordUpdate, DnsResolver,
};
use reth_network::config::rng_secret_key;
use reth_primitives::{mainnet_nodes, NodeRecord};
use secp256k1::SecretKey;
use std::sync::Arc;
use std::time::Duration;

use crate::crawler::CrawlerService;

pub static MAINNET_BOOT_NODES: Lazy<Vec<NodeRecord>> = Lazy::new(mainnet_nodes);

pub struct CrawlerFactory {
    key: SecretKey,
    discv4: Discv4,
    dnsdisc: DnsDiscoveryHandle,
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

        Self {
            key,
            discv4,
            dnsdisc,
        }
    }

    pub async fn make(&self, sql_db: bool) -> CrawlerService {
        CrawlerService::new(self.discv4.clone(), self.dnsdisc.clone(), self.key, sql_db).await
    }
}
