use crate::crawler::BlockHashNumHandle;
use ethers::providers::{Middleware, Provider, Ws};
use futures::StreamExt;

/// Listener that starts state service in order to keep updating it (LRU) with new blocks.
pub(crate) struct StateListener {
    /// State handle for `BlockUpdate` and `HashRequest` requests.
    state_handle: BlockHashNumHandle,
    /// Inner provider to use for block requests.
    provider: Provider<Ws>,
}

impl StateListener {
    /// Create a new `StateListener`.
    pub(crate) fn new(state_handle: BlockHashNumHandle, provider: Provider<Ws>) -> Self {
        Self {
            state_handle,
            provider,
        }
    }

    /// Start state to update it (LRU) with new blocks.
    pub(crate) async fn block_subscription_manager(&self) -> eyre::Result<()> {
        let mut block_subscription = self.provider.subscribe_blocks().await?;

        while let Some(block) = block_subscription.next().await {
            let block_hash = block.hash.expect("it's not a pending block");
            let block_number = block.number.expect("it's not a pending block");
            self.state_handle
                .new_block(block_hash, block_number)
                .await?;
        }

        Ok(())
    }
}
