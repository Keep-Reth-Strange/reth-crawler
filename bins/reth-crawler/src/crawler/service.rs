use super::{block_hash_num::BlockHashNumHandle, listener::StateListener};
use crate::crawler::listener::UpdateListener;
use ethers::providers::{Provider, Ws};
use reth_discv4::Discv4;
use reth_dns_discovery::DnsDiscoveryHandle;
use reth_network::NetworkHandle;
use secp256k1::SecretKey;
use tokio::join;
use tracing::info;

/// Service that creates the `UpdateListener` and runs the crawler.
pub(crate) struct CrawlerService {
    state: StateListener,
    updates: Vec<UpdateListener>,
}

impl CrawlerService {
    /// Create a new `CrawlerService`.
    pub(crate) async fn new(
        discv4: Vec<Discv4>,
        dnsdisc: Vec<DnsDiscoveryHandle>,
        network: Vec<NetworkHandle>,
        state_handle: BlockHashNumHandle,
        key: Vec<SecretKey>,
        local_db: bool,
        provider: Provider<Ws>,
    ) -> Self {
        // verify every Vec has same length
        assert!(
            discv4.len() == dnsdisc.len()
                && discv4.len() == network.len()
                && discv4.len() == key.len()
        );

        let mut updates = vec![];
        for i in 0..discv4.len() {
            let update = UpdateListener::new(
                discv4[i].clone(),
                dnsdisc[i].clone(),
                network[i].clone(),
                state_handle.clone(),
                key[i],
                local_db,
            )
            .await;
            updates.push(update);
        }

        let state = StateListener::new(state_handle, provider);

        Self { state, updates }
    }

    /// Run the service.
    pub(crate) async fn run(self) -> eyre::Result<()> {
        info!("start crawling...wait for the state to be initialized (30 seconds)...");

        // Use a vector of futures for each type of service.
        let mut discv4_futures = Vec::new();
        let mut dnsdisc_futures = Vec::new();
        let mut network_futures = Vec::new();

        // Populate the vectors with futures.
        for update in self.updates.iter() {
            discv4_futures.push(update.start_discv4());
            dnsdisc_futures.push(update.start_dnsdisc());
            network_futures.push(update.start_network());
        }

        // Start the block subscription manager.
        let state_future = self.state.block_subscription_manager();

        // Use `join!` to run all futures concurrently
        let _ = join!(
            futures::future::join_all(discv4_futures),
            futures::future::join_all(dnsdisc_futures),
            futures::future::join_all(network_futures),
            state_future
        );

        Ok(())
    }
}
