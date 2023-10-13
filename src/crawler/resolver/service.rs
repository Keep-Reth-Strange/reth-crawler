use crate::crawler::{
    db::PeerDB,
    p2p_utils::{append_to_file, handshake_eth, handshake_p2p},
};
use crate::types::PeerData;
use chrono::Utc;
use futures::future::join;
use ipgeolocate::{Locator, Service};
use once_cell::sync::Lazy;
use reth_discv4::Discv4;
use reth_dns_discovery::DnsDiscoveryHandle;
use reth_primitives::NodeRecord;
use secp256k1::SecretKey;
use std::time::Instant;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use reth_primitives::mainnet_nodes;

pub static MAINNET_BOOT_NODES: Lazy<Vec<NodeRecord>> = Lazy::new(mainnet_nodes);

pub struct ResolverService {
    key: SecretKey,
    node_tx: UnboundedSender<Vec<NodeRecord>>,
    node_rx: UnboundedReceiver<Vec<NodeRecord>>,
    db: PeerDB,
}

impl ResolverService {
    pub async fn new(
        key: SecretKey,
        node_tx: UnboundedSender<Vec<NodeRecord>>,
        node_rx: UnboundedReceiver<Vec<NodeRecord>>,
    ) -> Self {
        let db = PeerDB::new().await;
        ResolverService {
            key,
            node_tx,
            node_rx,
            db,
        }
    }

    // we use this walk down a list of nodes returned by a force lookup(node/self), and attempt to handshake
    // runs the risk of duplicates, but this is an attempt to expand the reach of the crawler
    pub async fn start(mut self, save_to_json: bool) -> eyre::Result<()> {
        while let Some(records) = self.node_rx.recv().await {
            let _ = records.iter().for_each(|peer| {
                println!("attempting to handshake peer: {}", peer);
                let peer = peer.clone();
                let db = self.db.clone();
                tokio::spawn(async move {
                    let (p2p_stream, their_hello) = match handshake_p2p(peer, self.key).await {
                        Ok(s) => s,
                        Err(e) => {
                            println!("Failed P2P handshake with peer {}, {}", peer.address, e);
                            return;
                        }
                    };
                    /*
                    let (_eth_stream, their_status) = match handshake_eth(p2p_stream).await {
                        Ok(s) => s,
                        Err(e) => {
                            println!("Failed ETH handshake with peer {}, {}", peer.address, e);
                            return;
                        }
                    };*/

                    let last_seen = Utc::now().to_string();
                    println!(
                        "Successfully connected to a peer at {}:{} ({}) using eth-wire version eth",
                        peer.address, peer.tcp_port, their_hello.client_version
                    );
                    // get peer location
                    let service = Service::IpApi;
                    let ip_addr = peer.address.to_string();

                    let mut country = String::default();
                    let mut city = String::default();

                    match Locator::get(&ip_addr, service).await {
                        Ok(loc) => {
                            country = loc.country;
                            city = loc.city;
                        }
                        Err(e) => {
                            eprintln!("Error getting location: {:?}", e);
                        }
                    }

                    let capabilities: Vec<String> = their_hello
                        .capabilities
                        .iter()
                        .map(|cap| cap.to_string())
                        .collect();
                    //let chain = their_status.chain.to_string();

                    //let total_difficulty = their_status.total_difficulty;
                    //let best_block = their_status.blockhash;
                    //let genesis_block_hash = their_status.genesis;
                    //let eth_version = their_status.version;

                    // collect data into `PeerData`
                    let peer_data = PeerData {
                        enode_url: peer.to_string(),
                        id: peer.id.to_string(),
                        address: ip_addr,
                        tcp_port: peer.tcp_port,
                        client_version: their_hello.client_version.clone(),
                        capabilities,
                        //eth_version,
                        //total_difficulty,
                        //best_block,
                        //genesis_block_hash,
                        country,
                        city,
                        last_seen,
                        //chain,
                    };
                    //db.add_peer(peer_data).await.unwrap();
                    // save data into JSON file
                    match append_to_file(peer_data).await {
                        Ok(_) => (),
                        Err(e) => eprintln!("Error appending to file: {:?}", e),
                    }
                });
            });
        }
        Ok(())
    }
}
