use chrono::{Duration, Utc};
use reth_crawler_db::{db::PostgreSQLPeerDb, PeerDB, SqlPeerDB};
use std::error::Error;

const PAGE_SIZE: Option<i32> = None;

/// Updates the SQLite db used for APIs requests from the AWS db used for the crawler.
async fn db_sync(update_time: i64, first_sync: bool) -> Result<(), Box<dyn Error>> {
    // dynamoDB setup
    let dynamo_db = PostgreSQLPeerDb::new().await;
    // sqliteDB setup
    let sqlite_db = SqlPeerDB::new().await;

    // take the time difference from last update (every 5 minutes)
    let now = Utc::now();
    let time_difference = now
        // + 60 is to be sure to collect all update peers. So we collect all peers that have been added or edited in the last 6 minutes.
        .checked_sub_signed(Duration::seconds(update_time + 60))
        .unwrap()
        .to_string();

    // scan table
    let peers = if first_sync {
        dynamo_db.all_peers(PAGE_SIZE).await?
    } else {
        dynamo_db
            .all_active_peers(time_difference, PAGE_SIZE)
            .await?
    };

    // update sqliteDB from dynamoDB
    for peer in peers {
        sqlite_db.add_peer(peer).await?;
    }

    Ok(())
}

/// Handler function that keeps updating the SQLite db used for serving APIs requests.
pub(crate) async fn db_sync_handler(update_time: i64) -> Result<(), Box<dyn Error>> {
    // we can unwrap because `update_time` is fixed to +5 minutes.
    let mut interval = tokio::time::interval(Duration::seconds(update_time).to_std().unwrap());
    let mut first_sync = true;
    loop {
        interval.tick().await;
        db_sync(update_time, first_sync).await?;
        first_sync = false;
    }
}
