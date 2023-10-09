use reth_discv4::Discv4;
use secp256k1::SecretKey;

use crate::crawler::listener::UpdateListener;

pub struct CrawlerService {
    updates: UpdateListener,
}

impl CrawlerService {
    pub fn new(discv4: Discv4, key: SecretKey) -> Self {
        let updates = UpdateListener::new(discv4.clone(), key);
        Self { updates }
    }

    pub async fn run(self) -> eyre::Result<()> {
        self.updates.start().await
    }
}
