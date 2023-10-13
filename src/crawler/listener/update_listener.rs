use crate::{
    crawler::{db::PeerDB, p2p_utils::save_peer},
    types::PeerData,
};
use chrono::Utc;
use futures::StreamExt;
use reth_discv4::{DiscoveryUpdate, Discv4};
use reth_dns_discovery::{DnsDiscoveryHandle, DnsNodeRecordUpdate};
use secp256k1::SecretKey;
use std::time::Instant;
use tokio::sync::mpsc::UnboundedSender;

use crate::crawler::p2p_utils::{append_to_file, handshake_eth, handshake_p2p};
use ipgeolocate::{Locator, Service};
use once_cell::sync::Lazy;
use reth_primitives::{mainnet_nodes, NodeRecord};

pub static MAINNET_BOOT_NODES: Lazy<Vec<NodeRecord>> = Lazy::new(mainnet_nodes);

pub struct UpdateListener {
    discv4: Discv4,
    dnsdisc: DnsDiscoveryHandle,
    key: SecretKey,
    node_tx: UnboundedSender<Vec<NodeRecord>>,
    db: PeerDB,
}

impl UpdateListener {
    pub async fn new(
        discv4: Discv4,
        dnsdisc: DnsDiscoveryHandle,
        key: SecretKey,
        node_tx: UnboundedSender<Vec<NodeRecord>>,
    ) -> Self {
        let db = PeerDB::new().await;
        UpdateListener {
            discv4,
            dnsdisc,
            key,
            node_tx,
            db,
        }
    }

    pub async fn start_discv4(&self, save_to_json: bool) -> eyre::Result<()> {
        let mut discv4_stream = self.discv4.update_stream().await?;
        let key = self.key;
        while let Some(update) = discv4_stream.next().await {
            let captured_discv4 = self.discv4.clone();
            let node_tx = self.node_tx.clone();
            let db: PeerDB = self.db.clone();
            if let DiscoveryUpdate::Added(peer) = update {
                tokio::spawn(async move {
                    let (p2p_stream, their_hello) = match handshake_p2p(peer, key).await {
                        Ok(s) => s,
                        Err(e) => {
                            println!("Failed P2P handshake with peer {}, {}", peer.address, e);
                            return;
                        }
                    };
                    /*/
                    let (eth_stream, their_status) = match handshake_eth(p2p_stream).await {
                        Ok(s) => s,
                        Err(e) => {
                            println!("Failed ETH handshake with peer {}, {}", peer.address, e);
                            return;
                        }
                    }; */

                    let last_seen = Utc::now().to_string();

                    println!(
                        "Successfully connected to a peer at {}:{} ({}) using eth-wire version eth/{:#?}",
                        peer.address, peer.tcp_port, their_hello.client_version, their_hello.protocol_version
                    );

                    /*

                    // we're probably already traversing a bootnode from Discv4::bootstrap(), so no need to kick another lookup
                    if MAINNET_BOOT_NODES.contains(&peer) {
                        println!("last node was a bootnode: {}", peer);
                    } else {
                        let self_lookup = captured_discv4.lookup_self().await;
                        println!("Recieved {:#?} from self_lookup", self_lookup);
                        match self_lookup {
                            Ok(nodes) => {
                                println!("nodes len: {}", nodes.len());
                                if nodes.len() > 0 {
                                    println!("sending self lookup res to resolver");
                                    // send to resolver
                                    node_tx.send(nodes).unwrap();
                                }
                            }
                            Err(_) => {}
                        }

                        let lookup_start = Instant::now();
                        let lookup_res = captured_discv4.lookup(peer.id).await;
                        println!(
                            "Recieved {:#?} from : {} with time taken: {:#?}",
                            lookup_res,
                            peer.address,
                            lookup_start.elapsed()
                        );
                        match lookup_res {
                            Ok(nodes) => {
                                println!("nodes len: {}", nodes.len());
                                if nodes.len() > 0 {
                                    println!("sending to resolver");
                                    // send to resolver
                                    node_tx.send(nodes).unwrap();
                                }
                            }
                            Err(_) => {}
                        }
                    }*/

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

                    // collect data into `PeerData`
                    let peer_data = PeerData {
                        enode_url: peer.to_string(),
                        id: peer.id.to_string(),
                        address: ip_addr,
                        tcp_port: peer.tcp_port,
                        client_version: their_hello.client_version.clone(),
                        //eth_version: their_status.version,
                        capabilities,
                        //total_difficulty,
                        //chain,
                        //best_block,
                        //genesis_block_hash,
                        last_seen,
                        country,
                        city,
                    };
                    save_peer(peer_data, save_to_json, db).await;
                });
            }
        }
        Ok(())
    }

    pub async fn start_dnsdisc(&self, save_to_json: bool) -> eyre::Result<()> {
        let mut dnsdisc_update_stream = self.dnsdisc.node_record_stream().await?;
        let key = self.key;
        while let Some(update) = dnsdisc_update_stream.next().await {
            let db: PeerDB = self.db.clone();
            let DnsNodeRecordUpdate {
                node_record: peer,
                fork_id,
            } = update;
            let captured_discv4 = self.discv4.clone();
            tokio::spawn(async move {
                let (p2p_stream, their_hello) = match handshake_p2p(peer, key).await {
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
                }; */

                let last_seen = Utc::now().to_string();

                println!(
                        "Successfully connected to a peer at {}:{} ({}) using eth-wire version eth/{:#?}",
                        peer.address, peer.tcp_port, their_hello.client_version, their_hello.protocol_version
                    );
                /*
                               // we're probably already traversing a bootnode from Discv4::bootstrap(), so no need to kick another lookup
                               if MAINNET_BOOT_NODES.contains(&peer) {
                                   println!("last node was a bootnode: {}", peer);
                               } else {
                                   let self_lookup = captured_discv4.lookup_self().await;
                                   println!("Recieved {:#?} from self_lookup", self_lookup);
                                   match self_lookup {
                                       Ok(nodes) => {
                                           println!("nodes len: {}", nodes.len());
                                           if nodes.len() > 0 {
                                               println!("sending self lookup res to resolver");
                                               // send to resolver
                                               node_tx.send(nodes).unwrap();
                                           }
                                       }
                                       Err(_) => {}
                                   }

                                   let lookup_start = Instant::now();
                                   let lookup_res = captured_discv4.lookup(peer.id).await;
                                   println!(
                                       "Recieved {:#?} from : {} with time taken: {:#?}",
                                       lookup_res,
                                       peer.address,
                                       lookup_start.elapsed()
                                   );
                                   match lookup_res {
                                       Ok(nodes) => {
                                           println!("nodes len: {}", nodes.len());
                                           if nodes.len() > 0 {
                                               println!("sending to resolver");
                                               // send to resolver
                                               node_tx.send(nodes).unwrap();
                                           }
                                       }
                                       Err(_) => {}
                                   }
                               }
                */
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
                // let genesis_block_hash = their_status.genesis;

                // collect data into `PeerData`
                let peer_data = PeerData {
                    enode_url: peer.to_string(),
                    id: peer.id.to_string(),
                    address: ip_addr,
                    tcp_port: peer.tcp_port,
                    client_version: their_hello.client_version.clone(),
                    // eth_version: their_hello.protocol_version,
                    capabilities,
                    //total_difficulty,
                    //chain,
                    //best_block,
                    //genesis_block_hash,
                    last_seen,
                    country,
                    city,
                };
                save_peer(peer_data, save_to_json, db).await;
            });
        }
        Ok(())
    }
}
