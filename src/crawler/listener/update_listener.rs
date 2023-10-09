use crate::types::PeerData;
use chrono::Utc;
use futures::StreamExt;
use reth_discv4::{DiscoveryUpdate, Discv4};
use secp256k1::SecretKey;
use std::time::Instant;

use crate::crawler::p2p_utils::{append_to_file, handshake_eth, handshake_p2p};
use ipgeolocate::{Locator, Service};
use once_cell::sync::Lazy;
use reth_primitives::{mainnet_nodes, NodeRecord};

pub static MAINNET_BOOT_NODES: Lazy<Vec<NodeRecord>> = Lazy::new(mainnet_nodes);

pub struct UpdateListener {
    discv4: Discv4, // TODO: need to make extensible for all (current/future) supported disc services
    key: SecretKey,
}

impl UpdateListener {
    pub fn new(discv4: Discv4, key: SecretKey) -> Self {
        UpdateListener { discv4, key }
    }

    // for now consume self and start - TODO: probably needs some health-checking/restart if this completes/fails/dies
    pub async fn start(self) -> eyre::Result<()> {
        let mut discv4_stream = self.discv4.update_stream().await?;
        while let Some(update) = discv4_stream.next().await {
            let captured_discv4 = self.discv4.clone();
            tokio::spawn(async move {
                if let DiscoveryUpdate::Added(peer) = update {
                    let (p2p_stream, their_hello) = match handshake_p2p(peer, self.key).await {
                        Ok(s) => s,
                        Err(e) => {
                            println!("Failed P2P handshake with peer {}, {}", peer.address, e);
                            return;
                        }
                    };

                    let (_eth_stream, their_status) = match handshake_eth(p2p_stream).await {
                        Ok(s) => s,
                        Err(e) => {
                            println!("Failed ETH handshake with peer {}, {}", peer.address, e);
                            return;
                        }
                    };

                    let last_seen = Utc::now().to_string();

                    println!(
                        "Successfully connected to a peer at {}:{} ({}) using eth-wire version eth/{}",
                        peer.address, peer.tcp_port, their_hello.client_version, their_status.version
                    );

                    // Boot nodes hard at work, lets not disturb them
                    if MAINNET_BOOT_NODES.contains(&peer) {
                        println!("last node was a bootnode: {}", peer);
                        return;
                    }

                    let lookup_start = Instant::now();
                    let lookup_res = captured_discv4.lookup(peer.id).await;
                    println!(
                        "recieved {:#?} from : {} with time taken: {:#?}",
                        lookup_res,
                        peer.id,
                        lookup_start.elapsed()
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
                    let chain = their_status.chain.to_string();

                    let total_difficulty = their_status.total_difficulty;
                    let best_block = their_status.blockhash;
                    let genesis_block_hash = their_status.genesis;

                    // collect data into `PeerData`
                    let peer_data = PeerData {
                        enode_url: peer.to_string(),
                        id: peer.id.to_string(),
                        address: ip_addr,
                        tcp_port: peer.tcp_port,
                        client_version: their_hello.client_version.clone(),
                        eth_version: their_status.version,
                        capabilities,
                        total_difficulty,
                        chain,
                        best_block,
                        genesis_block_hash,
                        last_seen,
                        country,
                        city,
                    };

                    // save data into JSON file
                    match append_to_file(peer_data).await {
                        Ok(_) => (),
                        Err(e) => eprintln!("Error appending to file: {:?}", e),
                    }
                }
            });
        }
        Ok(())
    }
}
