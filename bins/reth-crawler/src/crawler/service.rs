use futures::join;
use reth_discv4::Discv4;
use reth_dns_discovery::DnsDiscoveryHandle;
use reth_network::NetworkHandle;
use secp256k1::SecretKey;
use tracing::info;

use crate::crawler::listener::UpdateListener;

pub struct CrawlerService {
    updates: UpdateListener,
}

impl CrawlerService {
    pub async fn new(
        discv4: Discv4,
        dnsdisc: DnsDiscoveryHandle,
        network: NetworkHandle,
        key: SecretKey,
        local_db: bool,
        provider_url: String,
    ) -> Self {
        let updates =
            UpdateListener::new(discv4, dnsdisc, network, key, local_db, provider_url).await;
        Self { updates }
    }

    pub async fn run(self) -> (eyre::Result<()>, eyre::Result<()>, (), eyre::Result<()>) {
        // first initialize the state
        info!("start initializing the state...");
        let _ = self.updates.initialize_state().await;
        // then start crawling
        info!("start crawling...");
        join!(
            self.updates.start_discv4(),
            self.updates.start_dnsdisc(),
            self.updates.start_network(),
            self.updates.start_state(),
        )
    }
}
