use chrono::{Duration, Utc};
use reth_crawler_db::{AwsPeerDB, PeerDB, SqlPeerDB};
use std::error::Error;
use tracing::info;

const PAGE_SIZE: Option<i32> = None;
/// This is the time validity for peers inside the sqlite db. It's in days.
/// After 3 days a peer is considered invalid and it's deleted from the sqlite db.
const PEERS_VALIDITY: i64 = 3;

async fn db_sync(update_time: i64, first_sync: bool) -> Result<(), Box<dyn Error>> {
    // dynamoDB setup
    let dynamo_db = AwsPeerDB::new().await;
    // sqliteDB setup
    let sqlite_db = SqlPeerDB::new().await;

    // take the time difference from last update (every 5 minutes)
    let now = Utc::now();
    let time_difference = now
        // + 1 is to be sure to collect all update peers. So we collect all peers that have been added or edited in the last 6 minutes.
        .checked_sub_signed(Duration::seconds(update_time + 60))
        .unwrap()
        .to_string();

    // scan table
    let peers = if first_sync {
        dynamo_db.all_peers(PAGE_SIZE).await?
    } else {
        dynamo_db.all_last_peers(time_difference, PAGE_SIZE).await?
    };

    // update sqliteDB from dynamoDB
    for peer in peers {
        sqlite_db.add_peer(peer).await?;
    }

    Ok(())
}

pub async fn db_sync_handler(update_time: i64) -> Result<(), Box<dyn Error>> {
    // we can unwrap because `update_time` is fixed to +5 minutes.
    let mut interval = tokio::time::interval(Duration::seconds(update_time).to_std().unwrap());
    let mut first_sync = true;
    loop {
        interval.tick().await;
        db_sync(update_time, first_sync).await?;
        first_sync = false;
    }
}
