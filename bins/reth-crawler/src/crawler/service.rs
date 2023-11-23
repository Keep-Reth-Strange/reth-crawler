use super::block_hash_num::BlockHashNumHandle;
use crate::crawler::listener::UpdateListener;
use ethers::providers::{Provider, Ws};
use futures::join;
use reth_discv4::Discv4;
use reth_dns_discovery::DnsDiscoveryHandle;
use reth_network::NetworkHandle;
use secp256k1::SecretKey;
use tracing::info;

pub struct CrawlerService {
    updates: UpdateListener,
}

impl CrawlerService {
    pub async fn new(
        discv4: Discv4,
        dnsdisc: DnsDiscoveryHandle,
        network: NetworkHandle,
        state_handle: BlockHashNumHandle,
        key: SecretKey,
        local_db: bool,
        provider: Provider<Ws>,
    ) -> Self {
        let updates = UpdateListener::new(
            discv4,
            dnsdisc,
            network,
            state_handle,
            key,
            local_db,
            provider,
        )
        .await;
        Self { updates }
    }

    pub async fn run(self) -> (eyre::Result<()>, eyre::Result<()>, (), eyre::Result<()>) {
        info!("start crawling...wait for the state to be initialized (30 seconds)...");
        join!(
            self.updates.start_discv4(),
            self.updates.start_dnsdisc(),
            self.updates.start_network(),
            self.updates.block_subscription_manager(),
        )
    }
}
