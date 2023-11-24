use futures::StreamExt;
use reth_ecies::{stream::ECIESStream, util::pk2id};
use reth_eth_wire::{
    EthMessage, EthStream, HelloMessage, P2PStream, Status, UnauthedEthStream, UnauthedP2PStream,
};
use reth_primitives::{Chain, Hardfork, Head, NodeRecord, MAINNET, MAINNET_GENESIS};
use secp256k1::{SecretKey, SECP256K1};
use tokio::net::TcpStream;

type AuthedP2PStream = P2PStream<ECIESStream<TcpStream>>;
type AuthedEthStream = EthStream<P2PStream<ECIESStream<TcpStream>>>;

// Perform a P2P handshake with a peer
pub(crate) async fn handshake_p2p(
    peer: NodeRecord,
    key: SecretKey,
) -> eyre::Result<(AuthedP2PStream, HelloMessage)> {
    let outgoing = TcpStream::connect((peer.address, peer.tcp_port)).await?;
    let ecies_stream = ECIESStream::connect(outgoing, key, peer.id).await?;

    let our_peer_id = pk2id(&key.public_key(SECP256K1));
    let our_hello = HelloMessage::builder(our_peer_id).build();

    Ok(UnauthedP2PStream::new(ecies_stream)
        .handshake(our_hello)
        .await?)
}

// Perform a ETH Wire handshake with a peer
pub(crate) async fn handshake_eth(
    p2p_stream: AuthedP2PStream,
) -> eyre::Result<(AuthedEthStream, Status)> {
    let fork_filter = MAINNET.fork_filter(Head {
        timestamp: MAINNET.fork(Hardfork::Shanghai).as_timestamp().unwrap(),
        ..Default::default()
    });

    let status = Status::builder()
        .chain(Chain::mainnet())
        .genesis(MAINNET_GENESIS)
        .forkid(Hardfork::Shanghai.fork_id(&MAINNET).unwrap())
        .build();

    let status = Status {
        version: p2p_stream.shared_capability().version(),
        ..status
    };
    let eth_unauthed = UnauthedEthStream::new(p2p_stream);
    Ok(eth_unauthed.handshake(status, fork_filter).await?)
}

// Snoop by greedily capturing all broadcasts that the peer emits
// note: this node cannot handle request so will be disconnected by peer when challenged
pub(crate) async fn _snoop(peer: NodeRecord, mut eth_stream: AuthedEthStream) {
    while let Some(Ok(update)) = eth_stream.next().await {
        match update {
            EthMessage::NewPooledTransactionHashes66(txs) => {
                println!(
                    "Got {} new tx hashes from peer {}",
                    txs.0.len(),
                    peer.address
                );
            }
            EthMessage::NewBlock(block) => {
                println!("Got new block data {:?} from peer {}", block, peer.address);
            }
            EthMessage::NewPooledTransactionHashes68(txs) => {
                println!(
                    "Got {} new tx hashes from peer {}",
                    txs.hashes.len(),
                    peer.address
                );
            }
            EthMessage::NewBlockHashes(block_hashes) => {
                println!(
                    "Got {} new block hashes from peer {}",
                    block_hashes.0.len(),
                    peer.address
                );
            }
            EthMessage::GetNodeData(_) => {
                println!(
                    "Unable to serve GetNodeData request to peer {}",
                    peer.address
                );
            }
            EthMessage::GetReceipts(_) => {
                println!(
                    "Unable to serve GetReceipts request to peer {}",
                    peer.address
                );
            }
            EthMessage::GetBlockHeaders(_) => {
                println!(
                    "Unable to serve GetBlockHeaders request to peer {}",
                    peer.address
                );
            }
            EthMessage::GetBlockBodies(_) => {
                println!(
                    "Unable to serve GetBlockBodies request to peer {}",
                    peer.address
                );
            }
            EthMessage::GetPooledTransactions(_) => {
                println!(
                    "Unable to serve GetPooledTransactions request to peer {}",
                    peer.address
                );
            }
            _ => {}
        }
    }
}
