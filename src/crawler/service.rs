use futures::join;
use reth_discv4::Discv4;
use reth_primitives::NodeRecord;
use secp256k1::SecretKey;
use tokio::sync::mpsc;

use crate::crawler::listener::UpdateListener;

use super::resolver::ResolverService;

pub struct CrawlerService {
    updates: UpdateListener,
    resolver: ResolverService,
}

impl CrawlerService {
    pub async fn new(discv4: Discv4, key: SecretKey) -> Self {
        let (tx, rx) = mpsc::unbounded_channel::<Vec<NodeRecord>>();
        let updates = UpdateListener::new(discv4.clone(), key, tx.clone()).await;
        let resolver = ResolverService::new(discv4, key, tx, rx).await;
        Self { updates, resolver }
    }

    pub async fn run(self) -> (eyre::Result<()>, eyre::Result<()>) {
        join!(self.updates.start(), self.resolver.start())
    }
}
