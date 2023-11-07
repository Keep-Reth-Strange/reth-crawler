use chrono::{Days, NaiveDateTime, Utc};
use ethers::{
    abi::FixedBytes,
    providers::{Http, Middleware, Provider},
};
use futures::future;
use reth_crawler_db::{save_peer, AwsPeerDB};
use std::str::FromStr;
use std::sync::Arc;
use tracing::info;
const SYNCED_THRESHOLD: u64 = 100;
use ethers::types::H256;
use web3_dater::Web3Dater;

use chrono::{DateTime, FixedOffset};
use std::collections::HashMap;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let db = AwsPeerDB::new().await;
    let provider_url = "http://69.67.151.138:8645";
    let peers = db.all_nonexisted_sync_peers(None).await.unwrap();
    let mut handles = vec![];
    let provider = Provider::try_from(provider_url).expect("Provider must work correctly!");
    let last_block_number = provider.get_block_number().await.unwrap();
    let captured_db = Arc::from(db);
    info!("last block number: {}", last_block_number);
    for mut peer in peers {
        let provider = Provider::try_from(provider_url).expect("Provider must work correctly!");
        let captured_db = captured_db.clone();
        let transport = web3::transports::Http::new(provider_url).unwrap();
        let web3client = web3::Web3::new(transport);
        // Create a new instance of Web3Dater
        let mut dater = Web3Dater::new(web3client);
        handles.push(tokio::spawn(async move {
            // grab peer timetstamp
            let timestamp = peer.last_seen.clone();
            let timestamp = &timestamp
                .split("UTC")
                .take(1)
                .collect::<Vec<&str>>()
                .into_iter()
                .nth(0)
                .unwrap();
            let timestamp = &timestamp
                .split(".")
                .take(1)
                .collect::<Vec<&str>>()
                .into_iter()
                .nth(0)
                .unwrap();
            println!("time: {}", timestamp);
            let format = "%Y-%m-%d %H:%M:%S%.9f";
            //parse
            let last_seen = NaiveDateTime::parse_from_str(timestamp, format).unwrap();
            let last_seen = last_seen.and_utc().into();
            println!("{}", last_seen);
            let last_block_number = dater
                .get_block_by_date(last_seen, true)
                .await
                .unwrap()
                .number
                .expect("Not a pending block!");
            // check if peer is synced with the latest chain's blocks
            let mut synced = None;
            let peer_best_block_hash = H256::from_str(&peer.best_block).unwrap();
            info!("peer best block hash: {:#?}", peer_best_block_hash);
            if let Ok(Some(peer_best_block)) =
                provider.clone().get_block(peer_best_block_hash).await
            {
                let peer_best_block_number =
                    peer_best_block.number.expect("it's not a pending block!");
                if peer_best_block_number.as_u64() < last_block_number.as_u64() - SYNCED_THRESHOLD {
                    synced = Some(false);
                } else {
                    synced = Some(true);
                }
                info!("peer best block number: {}", peer_best_block_number);
                info!("sync: {}", synced.unwrap());
            }
            peer.synced = synced;
            let ttl = Utc::now()
                .checked_add_days(Days::new(3))
                .unwrap()
                .timestamp();
            if let Some(false) = peer.synced {
                return;
            }
            save_peer(peer, captured_db, ttl).await;
        }));
    }
    future::join_all(handles).await;
}
