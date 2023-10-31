use chrono::{Duration, Utc};
use reth_crawler_db::{AwsPeerDB, PeerDB, SqlPeerDB};
use std::error::Error;

const PAGE_SIZE: Option<i32> = None;

async fn db_sync(update_time: i64) -> Result<(), Box<dyn Error>> {
    // dynamoDB setup
    let dynamo_db = AwsPeerDB::new().await;
    // sqliteDB setup
    let sqlite_db = SqlPeerDB::new().await;

    // take the time difference from last update (every 5 minutes)
    let now = Utc::now();
    let time_difference = now
        // + 1 is to be sure to collect all update peers. So we collect all peers that have been added or edited in the last 6 minutes.
        .checked_sub_signed(Duration::minutes(update_time + 1))
        .unwrap()
        .to_string();

    // scan table
    let peers = dynamo_db.all_last_peers(time_difference, PAGE_SIZE).await?;

    // update sqliteDB from dynamoDB
    for peer in peers {
        sqlite_db.add_peer(peer).await?;
    }

    Ok(())
}

pub async fn db_sync_handler(update_time: i64) -> Result<(), Box<dyn Error>> {
    // we can unwrap because `update_time` is fixed to +5 minutes.
    let mut interval = tokio::time::interval(Duration::seconds(update_time).to_std().unwrap());
    loop {
        interval.tick().await;
        let now = Utc::now();
        let update_time = (now.timestamp() - update_time) / 60; // Convert to minutes
        db_sync(update_time).await?
    }
}
