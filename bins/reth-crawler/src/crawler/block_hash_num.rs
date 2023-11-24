use ethers::types::{H256, U64};
use futures::{Future, StreamExt};
use lru::LruCache;
use std::{
    num::NonZeroUsize,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::sync::{
    mpsc::{self, UnboundedSender},
    oneshot,
};
use tokio_stream::wrappers::UnboundedReceiverStream;

/// How many blocks can a node be lagging and still be considered `synced`.
const SYNCED_THRESHOLD: u64 = 100;

/// This holds the mapping between block hash and block number of the latest `SYNCED_THRESHOLD` blocks.
#[derive(Debug)]
pub(crate) struct BlockHashNum {
    /// Inner chache for mapping block hashes to block numbers.
    blocks_hash_to_number: LruCache<H256, U64>,
    /// Receiver half of the channel for block requests
    command_rx: UnboundedReceiverStream<HashRequest>,
    /// Copy of the sender half of the channel so handles can be created on demand.
    service_tx: UnboundedSender<HashRequest>,
    /// Receiver half of the channel for block updates.
    block_subscription_rx: UnboundedReceiverStream<BlockUpdate>,
    /// Copy of the sender half of the channel so handles can be created on demand.
    block_subscription_tx: UnboundedSender<BlockUpdate>,
}

impl BlockHashNum {
    /// Create a new service to resolve and cache block hashes / numbers mapping.
    pub(crate) fn new() -> (Self, BlockHashNumHandle) {
        let (service_tx, command_rx) = mpsc::unbounded_channel();
        let (block_subscription_tx, block_subscription_rx) = mpsc::unbounded_channel();
        let service = Self {
            blocks_hash_to_number: LruCache::new(
                NonZeroUsize::new(SYNCED_THRESHOLD as usize).expect("it's not zero!"),
            ),
            command_rx: UnboundedReceiverStream::new(command_rx),
            service_tx,
            block_subscription_rx: UnboundedReceiverStream::new(block_subscription_rx),
            block_subscription_tx,
        };
        let handle = service.handle();

        (service, handle)
    }
    /// Returns a handle to the service.
    fn handle(&self) -> BlockHashNumHandle {
        BlockHashNumHandle::new(self.service_tx.clone(), self.block_subscription_tx.clone())
    }
}

impl Future for BlockHashNum {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        while let Poll::Ready(Some(request)) = this.command_rx.poll_next_unpin(cx) {
            // check if request.hash is inside the inner LRU
            let HashRequest { hash, response } = request;
            let is_present = this.blocks_hash_to_number.contains(&hash);
            let _ = response.send(is_present);
        }

        while let Poll::Ready(Some(block_update)) = this.block_subscription_rx.poll_next_unpin(cx) {
            // add block to the LRU cache
            let BlockUpdate {
                hash,
                number,
                response,
            } = block_update;
            this.blocks_hash_to_number.put(hash, number);
            let _ = response.send(());
        }

        Poll::Pending
    }
}

/// Hash requets to be sent in the channel.
#[derive(Debug)]
struct HashRequest {
    /// The requested hash.
    hash: H256,
    /// The channel for returning the response.
    response: oneshot::Sender<bool>,
}

/// A clone-able handle that sends requests to the block hash to num service
#[derive(Clone, Debug)]
pub(crate) struct BlockHashNumHandle {
    /// Sender half of the message channel for hash requests.
    to_service_hash_request: mpsc::UnboundedSender<HashRequest>,
    /// Sender half of the message channel for block updates.
    to_service_block_update: mpsc::UnboundedSender<BlockUpdate>,
}

impl BlockHashNumHandle {
    /// Create a new `BlockHashNumHandle`.
    fn new(
        to_service_hash_request: UnboundedSender<HashRequest>,
        to_service_block_update: UnboundedSender<BlockUpdate>,
    ) -> Self {
        Self {
            to_service_hash_request,
            to_service_block_update,
        }
    }

    /// Send a `HashRequest` in the channel.
    pub(crate) async fn is_block_hash_present(&self, hash: H256) -> eyre::Result<bool> {
        let (tx, rx) = oneshot::channel();
        let hash_request = HashRequest { hash, response: tx };
        self.to_service_hash_request.send(hash_request)?;
        rx.await.map_err(|err| err.into())
    }

    /// Send a `BlockUpdate` in the channel.
    pub(crate) async fn new_block(&self, hash: H256, number: U64) -> eyre::Result<()> {
        let (tx, rx) = oneshot::channel();
        let block_update = BlockUpdate {
            hash,
            number,
            response: tx,
        };
        self.to_service_block_update.send(block_update)?;
        rx.await.map_err(|err| err.into())
    }
}

/// Assuming BlockUpdate is the type of data you want to send
#[derive(Debug)]
struct BlockUpdate {
    /// The hash of the block.
    hash: H256,
    /// The block number.
    number: U64,
    /// The channel for returning the response
    response: oneshot::Sender<()>,
}
