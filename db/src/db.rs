//! The `PeerDB` trait works as an abstraction over the database.

use crate::types::{AddItemError, ClientData, PeerData, QueryItemError, ScanTableError};
use async_trait::async_trait;
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_dynamodb::{config::Region, Client};
use tokio_postgres::{Client as PostgresClient, NoTls};
use tokio_rusqlite::{params, Connection};
use tokio_stream::StreamExt;

/// The abstraction trait over the database.
#[async_trait]
pub trait PeerDB: Send + Sync {
    /// Add a peer into the database.
    async fn add_peer(&self, peer_data: PeerData) -> Result<(), AddItemError>;
    /// Returns all peers in the database.
    async fn all_peers(&self, page_size: Option<i32>) -> Result<Vec<PeerData>, ScanTableError>;
    /// Returns all active peers in the database.
    ///
    /// An active peer is a peer with `last_seen` greater than the input you use here.
    async fn all_active_peers(
        &self,
        last_seen: String,
        page_size: Option<i32>,
    ) -> Result<Vec<PeerData>, ScanTableError>;
    /// Get a node by its id.
    async fn node_by_id(&self, id: String) -> Result<Option<Vec<PeerData>>, QueryItemError>;
    /// Get a node by its ip address.
    async fn node_by_ip(&self, ip: String) -> Result<Option<Vec<PeerData>>, QueryItemError>;
    /// Returns all clients in the database.
    async fn clients(&self, page_size: Option<i32>) -> Result<Vec<ClientData>, ScanTableError>;
    /// Returns all clients in the database.
    ///
    /// An active client is a client whose peer has a `last_seen` greater than the input you use here.
    async fn active_clients(
        &self,
        last_seen: String,
        page_size: Option<i32>,
    ) -> Result<Vec<ClientData>, ScanTableError>;
}

/// Aws dynamo database.
#[derive(Clone, Debug)]
pub struct AwsPeerDB {
    client: Client,
}

impl AwsPeerDB {
    /// Create a new AWS dynamo db or connect to an already existing one.
    pub async fn new() -> Self {
        let region_provider =
            RegionProviderChain::default_provider().or_else(Region::new("us-west-2"));
        let shared_config = aws_config::from_env().region(region_provider).load().await;
        let client = Client::new(&shared_config);

        AwsPeerDB { client }
    }
}

#[async_trait]
impl PeerDB for AwsPeerDB {
    async fn add_peer(&self, peer_data: PeerData) -> Result<(), AddItemError> {
        let capabilities = peer_data
            .capabilities
            .iter()
            .map(|cap| AttributeValue::S(cap.clone()))
            .collect();
        let peer_id = AttributeValue::S(peer_data.id);
        let peer_ip = AttributeValue::S(peer_data.address);
        let client_name = AttributeValue::S(peer_data.client_name);
        let client_version = AttributeValue::S(peer_data.client_version);
        let client_build = AttributeValue::S(peer_data.client_build);
        let client_arch = AttributeValue::S(peer_data.client_arch);
        let os: AttributeValue = AttributeValue::S(peer_data.os);
        let client_language = AttributeValue::S(peer_data.client_language);
        let enode_url = AttributeValue::S(peer_data.enode_url);
        let port = AttributeValue::N(peer_data.tcp_port.to_string()); // numbers are sent over the network as string
        let chain = AttributeValue::S(peer_data.chain);
        let genesis_hash = AttributeValue::S(peer_data.genesis_block_hash);
        let best_block = AttributeValue::S(peer_data.best_block);
        let total_difficulty = AttributeValue::S(peer_data.total_difficulty);
        let country = AttributeValue::S(peer_data.country);
        let city = AttributeValue::S(peer_data.city);
        let last_seen = AttributeValue::S(peer_data.last_seen);
        let region_source = AttributeValue::S(self.client.config().region().unwrap().to_string());
        let capabilities = AttributeValue::L(capabilities);
        let eth_version = AttributeValue::N(peer_data.eth_version.to_string());
        let synced = if let Some(synced) = peer_data.synced {
            AttributeValue::Bool(synced)
        } else {
            AttributeValue::Null(true)
        };
        let isp = AttributeValue::S(peer_data.isp);

        match self
            .client
            .put_item()
            .table_name("eth-peer-data")
            .item("peer-id", peer_id)
            .item("peer-ip", peer_ip)
            .item("client_name", client_name)
            .item("client_version", client_version)
            .item("client_build", client_build)
            .item("client_arch", client_arch)
            .item("os", os)
            .item("client_language", client_language)
            .item("enode_url", enode_url)
            .item("port", port)
            .item("chain", chain)
            .item("country", country)
            .item("city", city)
            .item("capabilities", capabilities)
            .item("eth_version", eth_version)
            .item("last_seen", last_seen)
            .item("source_region", region_source)
            .item("genesis_block_hash", genesis_hash)
            .item("best_block", best_block)
            .item("total_difficulty", total_difficulty)
            .item("synced", synced)
            .item("isp", isp)
            .send()
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    async fn all_peers(&self, page_size: Option<i32>) -> Result<Vec<PeerData>, ScanTableError> {
        let page_size = page_size.unwrap_or(1000);
        let results: Result<Vec<_>, _> = self
            .client
            .scan()
            .table_name("eth-peer-data")
            .limit(page_size)
            .into_paginator()
            .items()
            .send()
            .collect()
            .await;

        match results {
            Ok(peers) => peers.iter().map(|peer| Ok(peer.into())).collect(),
            Err(err) => Err(err.into()),
        }
    }

    async fn all_active_peers(
        &self,
        last_seen: String,
        page_size: Option<i32>,
    ) -> Result<Vec<PeerData>, ScanTableError> {
        let page_size = page_size.unwrap_or(1000);
        let results: Result<Vec<_>, _> = self
            .client
            .scan()
            .table_name("eth-peer-data")
            .filter_expression("last_seen > :last_seen_parameter")
            .expression_attribute_values(
                ":last_seen_parameter",
                AttributeValue::S(last_seen.clone()),
            )
            .limit(page_size)
            .into_paginator()
            .items()
            .send()
            .collect()
            .await;
        match results {
            Ok(peers) => peers.iter().map(|peer| Ok(peer.into())).collect(),
            Err(err) => Err(err.into()),
        }
    }

    async fn node_by_id(&self, id: String) -> Result<Option<Vec<PeerData>>, QueryItemError> {
        let results = self
            .client
            .query()
            .table_name("eth-peer-data")
            .key_condition_expression("#id = :id")
            .expression_attribute_names("#id", "peer-id")
            .expression_attribute_values(":id", AttributeValue::S(id))
            .send()
            .await?;

        if let Some(nodes) = results.items {
            let node = nodes.iter().map(|v| v.into()).collect();
            Ok(Some(node))
        } else {
            Ok(None)
        }
    }

    async fn node_by_ip(&self, ip: String) -> Result<Option<Vec<PeerData>>, QueryItemError> {
        let results = self
            .client
            .query()
            .table_name("eth-peer-data")
            .index_name("peer-ip-index")
            .key_condition_expression("#ip = :ip")
            .expression_attribute_names("#ip", "peer-ip")
            .expression_attribute_values(":ip", AttributeValue::S(ip))
            .send()
            .await?;

        if let Some(nodes) = results.items {
            let node = nodes.iter().map(|v| v.into()).collect();
            Ok(Some(node))
        } else {
            Ok(None)
        }
    }

    async fn clients(&self, page_size: Option<i32>) -> Result<Vec<ClientData>, ScanTableError> {
        let page_size = page_size.unwrap_or(1000);
        let results: Result<Vec<_>, _> = self
            .client
            .scan()
            .table_name("eth-peer-data")
            .limit(page_size)
            .into_paginator()
            .items()
            .send()
            .collect()
            .await;

        match results {
            Ok(peers) => peers
                .iter()
                .map(|peer| {
                    let client_version = Into::<PeerData>::into(peer).client_version;
                    Ok(ClientData { client_version })
                })
                .collect(),
            Err(err) => Err(err.into()),
        }
    }

    async fn active_clients(
        &self,
        last_seen: String,
        page_size: Option<i32>,
    ) -> Result<Vec<ClientData>, ScanTableError> {
        let page_size = page_size.unwrap_or(1000);
        let results: Result<Vec<_>, _> = self
            .client
            .scan()
            .table_name("eth-peer-data")
            .filter_expression("last_seen > :last_seen_parameter")
            .expression_attribute_values(
                ":last_seen_parameter",
                AttributeValue::S(last_seen.clone()),
            )
            .limit(page_size)
            .into_paginator()
            .items()
            .send()
            .collect()
            .await;

        match results {
            Ok(peers) => peers
                .iter()
                .map(|peer| {
                    let client_version = Into::<PeerData>::into(peer).client_version;
                    Ok(ClientData { client_version })
                })
                .collect(),
            Err(err) => Err(err.into()),
        }
    }
}

/// SQLite db.
#[derive(Debug)]
pub struct SqlPeerDB {
    db: Connection,
}

impl SqlPeerDB {
    /// Create a new SQLite db or connect to an already existing one.
    pub async fn new() -> Self {
        let db = Connection::open("peers_data.db").await.unwrap();
        // create `eth_peer_data` table if not exists
        let _ = db
            .call(|conn| {
                conn.execute(
                    "CREATE TABLE IF NOT EXISTS eth_peer_data (
                id TEXT PRIMARY KEY,
                ip TEXT NOT NULL,
                client_name TEXT NON NULL,
                client_version TEXT NOT NULL,
                client_build TEXT,
                client_arch TEXT,
                os TEXT,
                client_language TEXT,
                enode_url TEXT NOT NULL,
                port INTEGER NOT NULL,
                chain TEXT NOT NULL,
                genesis_hash TEXT NOT NULL,
                best_block TEXT NOT NULL,
                total_difficulty TEXT NOT NULL,
                country TEXT,
                city TEXT,
                last_seen TEXT NOT NULL,
                capabilities TEXT,
                eth_version INTEGER,
                synced BOOLEAN,
                isp TEXT
            );",
                    [],
                )
                .map_err(|err| err.into())
            })
            .await
            .unwrap();
        Self { db }
    }
}

#[async_trait]
impl PeerDB for SqlPeerDB {
    async fn add_peer(&self, peer_data: PeerData) -> Result<(), AddItemError> {
        self.db
            .call(move |conn| {
                conn.execute(
                    "INSERT OR REPLACE INTO eth_peer_data (id, ip, client_name, client_version, client_build, client_arch, os, client_language, enode_url, port, chain, genesis_hash, best_block, total_difficulty, country, city, last_seen, capabilities, eth_version, synced, isp) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
                    params!(
                        &peer_data.id,
                        &peer_data.address,
                        &peer_data.client_name,
                        &peer_data.client_version,
                        &peer_data.client_build,
                        &peer_data.client_arch,
                        &peer_data.os,
                        &peer_data.client_language,
                        &peer_data.enode_url,
                        &peer_data.tcp_port,
                        &peer_data.chain,
                        &peer_data.genesis_block_hash,
                        &peer_data.best_block,
                        &peer_data.total_difficulty,
                        &peer_data.country,
                        &peer_data.city,
                        &peer_data.last_seen,
                        &peer_data.capabilities.join(","),
                        &peer_data.eth_version,
                        &peer_data.synced,
                        &peer_data.isp,
                    ),
                ).map_err(|err| err.into())
            })
            .await
            .map_err(AddItemError::SqlAddItemError)?;
        Ok(())
    }

    async fn all_peers(&self, _page_size: Option<i32>) -> Result<Vec<PeerData>, ScanTableError> {
        let peers = self
            .db
            .call(move |conn| {
                let mut stmt = conn.prepare("SELECT * from eth_peer_data")?;
                let rows = stmt.query_map([], |row| {
                    Ok(PeerData {
                        id: row.get(0)?,
                        address: row.get(1)?,
                        client_name: row.get(2)?,
                        client_version: row.get(3)?,
                        client_build: row.get(4)?,
                        client_arch: row.get(5)?,
                        os: row.get(6)?,
                        client_language: row.get(7)?,
                        enode_url: row.get(8)?,
                        tcp_port: row.get(9)?,
                        chain: row.get(10)?,
                        genesis_block_hash: row.get(11)?,
                        best_block: row.get(12)?,
                        total_difficulty: row.get(13)?,
                        country: row.get(14)?,
                        city: row.get(15)?,
                        last_seen: row.get(16)?,
                        capabilities: row
                            .get::<_, String>(17)?
                            .as_str()
                            .split(',')
                            .map(|s| s.to_string())
                            .collect(),
                        eth_version: row.get(18)?,
                        synced: row.get(19)?,
                        isp: row.get(20)?,
                    })
                })?;
                let mut peers = vec![];
                for peer_data in rows.flatten() {
                    peers.push(peer_data);
                }
                Ok(peers)
            })
            .await
            .map_err(ScanTableError::SqlScanError)?;

        Ok(peers)
    }

    async fn all_active_peers(
        &self,
        last_seen: String,
        _page_size: Option<i32>,
    ) -> Result<Vec<PeerData>, ScanTableError> {
        let peers = self
            .db
            .call(move |conn| {
                let mut stmt = conn.prepare("SELECT * from eth_peer_data WHERE last_seen > ?1")?;
                let rows = stmt.query_map([last_seen], |row| {
                    Ok(PeerData {
                        id: row.get(0)?,
                        address: row.get(1)?,
                        client_name: row.get(2)?,
                        client_version: row.get(3)?,
                        client_build: row.get(4)?,
                        client_arch: row.get(5)?,
                        os: row.get(6)?,
                        client_language: row.get(7)?,
                        enode_url: row.get(8)?,
                        tcp_port: row.get(9)?,
                        chain: row.get(10)?,
                        genesis_block_hash: row.get(11)?,
                        best_block: row.get(12)?,
                        total_difficulty: row.get(13)?,
                        country: row.get(14)?,
                        city: row.get(15)?,
                        last_seen: row.get(16)?,
                        capabilities: row
                            .get::<_, String>(17)?
                            .as_str()
                            .split(',')
                            .map(|s| s.to_string())
                            .collect(),
                        eth_version: row.get(18)?,
                        synced: row.get(19)?,
                        isp: row.get(20)?,
                    })
                })?;
                let mut peers = vec![];
                for peer_data in rows.flatten() {
                    peers.push(peer_data);
                }
                Ok(peers)
            })
            .await
            .map_err(ScanTableError::SqlScanError)?;

        Ok(peers)
    }

    async fn node_by_id(&self, id: String) -> Result<Option<Vec<PeerData>>, QueryItemError> {
        let peers = self
            .db
            .call(move |conn| {
                let mut stmt = conn.prepare("SELECT * from eth_peer_data WHERE id = ?1")?;
                let rows = stmt.query_map([id], |row| {
                    Ok(PeerData {
                        id: row.get(0)?,
                        address: row.get(1)?,
                        client_name: row.get(2)?,
                        client_version: row.get(3)?,
                        client_build: row.get(4)?,
                        client_arch: row.get(5)?,
                        os: row.get(6)?,
                        client_language: row.get(7)?,
                        enode_url: row.get(8)?,
                        tcp_port: row.get(9)?,
                        chain: row.get(10)?,
                        genesis_block_hash: row.get(11)?,
                        best_block: row.get(12)?,
                        total_difficulty: row.get(13)?,
                        country: row.get(14)?,
                        city: row.get(15)?,
                        last_seen: row.get(16)?,
                        capabilities: row
                            .get::<_, String>(17)?
                            .as_str()
                            .split(',')
                            .map(|s| s.to_string())
                            .collect(),
                        eth_version: row.get(18)?,
                        synced: row.get(19)?,
                        isp: row.get(20)?,
                    })
                })?;
                let mut peers = vec![];
                for peer_data in rows.flatten() {
                    peers.push(peer_data);
                }
                Ok(peers)
            })
            .await
            .map_err(QueryItemError::SqlQueryItemError)?;

        Ok(Some(peers))
    }

    async fn node_by_ip(&self, ip: String) -> Result<Option<Vec<PeerData>>, QueryItemError> {
        let peers = self
            .db
            .call(move |conn| {
                let mut stmt = conn.prepare("SELECT * from eth_peer_data WHERE ip = ?1")?;
                let rows = stmt.query_map([ip], |row| {
                    Ok(PeerData {
                        id: row.get(0)?,
                        address: row.get(1)?,
                        client_name: row.get(2)?,
                        client_version: row.get(3)?,
                        client_build: row.get(4)?,
                        client_arch: row.get(5)?,
                        os: row.get(6)?,
                        client_language: row.get(7)?,
                        enode_url: row.get(8)?,
                        tcp_port: row.get(9)?,
                        chain: row.get(10)?,
                        genesis_block_hash: row.get(11)?,
                        best_block: row.get(12)?,
                        total_difficulty: row.get(13)?,
                        country: row.get(14)?,
                        city: row.get(15)?,
                        last_seen: row.get(16)?,
                        capabilities: row
                            .get::<_, String>(17)?
                            .as_str()
                            .split(',')
                            .map(|s| s.to_string())
                            .collect(),
                        eth_version: row.get(18)?,
                        synced: row.get(19)?,
                        isp: row.get(20)?,
                    })
                })?;
                let mut peers = vec![];
                for peer_data in rows.flatten() {
                    peers.push(peer_data);
                }
                Ok(peers)
            })
            .await
            .map_err(QueryItemError::SqlQueryItemError)?;

        Ok(Some(peers))
    }

    async fn clients(&self, _page_size: Option<i32>) -> Result<Vec<ClientData>, ScanTableError> {
        let clients = self
            .db
            .call(move |conn| {
                let mut stmt = conn.prepare("SELECT * from eth_peer_data")?;
                let rows = stmt.query_map([], |row| {
                    Ok(PeerData {
                        id: row.get(0)?,
                        address: row.get(1)?,
                        client_name: row.get(2)?,
                        client_version: row.get(3)?,
                        client_build: row.get(4)?,
                        client_arch: row.get(5)?,
                        os: row.get(6)?,
                        client_language: row.get(7)?,
                        enode_url: row.get(8)?,
                        tcp_port: row.get(9)?,
                        chain: row.get(10)?,
                        genesis_block_hash: row.get(11)?,
                        best_block: row.get(12)?,
                        total_difficulty: row.get(13)?,
                        country: row.get(14)?,
                        city: row.get(15)?,
                        last_seen: row.get(16)?,
                        capabilities: row
                            .get::<_, String>(17)?
                            .as_str()
                            .split(',')
                            .map(|s| s.to_string())
                            .collect(),
                        eth_version: row.get(18)?,
                        synced: row.get(19)?,
                        isp: row.get(20)?,
                    })
                })?;
                let mut clients = vec![];
                for peer_data in rows.flatten() {
                    let client_data = ClientData {
                        client_version: peer_data.client_version,
                    };
                    clients.push(client_data);
                }
                Ok(clients)
            })
            .await
            .map_err(ScanTableError::SqlScanError)?;

        Ok(clients)
    }

    async fn active_clients(
        &self,
        last_seen: String,
        _page_size: Option<i32>,
    ) -> Result<Vec<ClientData>, ScanTableError> {
        let clients = self
            .db
            .call(move |conn| {
                let mut stmt = conn.prepare("SELECT * from eth_peer_data WHERE last_seen > ?1")?;
                let rows = stmt.query_map([last_seen], |row| {
                    Ok(PeerData {
                        id: row.get(0)?,
                        address: row.get(1)?,
                        client_name: row.get(2)?,
                        client_version: row.get(3)?,
                        client_build: row.get(4)?,
                        client_arch: row.get(5)?,
                        os: row.get(6)?,
                        client_language: row.get(7)?,
                        enode_url: row.get(8)?,
                        tcp_port: row.get(9)?,
                        chain: row.get(10)?,
                        genesis_block_hash: row.get(11)?,
                        best_block: row.get(12)?,
                        total_difficulty: row.get(13)?,
                        country: row.get(14)?,
                        city: row.get(15)?,
                        last_seen: row.get(16)?,
                        capabilities: row
                            .get::<_, String>(17)?
                            .as_str()
                            .split(',')
                            .map(|s| s.to_string())
                            .collect(),
                        eth_version: row.get(18)?,
                        synced: row.get(19)?,
                        isp: row.get(20)?,
                    })
                })?;
                let mut clients = vec![];
                for peer_data in rows.flatten() {
                    let client_data = ClientData {
                        client_version: peer_data.client_version,
                    };
                    clients.push(client_data);
                }
                Ok(clients)
            })
            .await
            .map_err(ScanTableError::SqlScanError)?;

        Ok(clients)
    }
}

/// PostgreSQL db.
#[derive(Debug)]
pub struct PostgreSQLPeerDb {
    db: PostgresClient,
}

impl PostgreSQLPeerDb {
    /// Create a new PostgreSQL db or connect to an already existing one.
    pub async fn new() -> Self {
        let postgres_username = std::env::var("POSTGRES_USERNAME")
            .expect("you should have `POSTGRES_USERNAME` env variable for postgres db");
        let postgres_pass = std::env::var("POSTGRES_PASS")
            .expect("you should have `POSTGRES_PASS` env variable for postgres db");
        let ip_addr = std::env::var("IP_ADDR")
            .expect("you should have `IP_ADDR` env variable for postgres db");
        let db_name = std::env::var("DB_NAME")
            .expect("you should have `DB_NAME` env variable for postgres db");
        let (client, connection) = tokio_postgres::connect(
            &format!(
                "postgresql://{}:{}@{}/{}",
                postgres_username, postgres_pass, ip_addr, db_name
            ),
            NoTls,
        )
        .await
        .unwrap();

        // The connection object performs the actual communication with the database,
        // so spawn it off to run on its own.
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("Connection error: {}", e);
            }
        });

        Self { db: client }
    }
}

#[async_trait]
impl PeerDB for PostgreSQLPeerDb {
    async fn add_peer(&self, peer_data: PeerData) -> Result<(), AddItemError> {
        self.db
            .execute(
                "INSERT INTO eth_peer_data (id, ip, client_name, client_version, client_build, client_arch, os, client_language, enode_url, port, chain, genesis_hash, best_block, total_difficulty, country, city, last_seen, capabilities, eth_version, synced, isp) 
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21)
                ON CONFLICT (id) DO UPDATE 
                SET ip = EXCLUDED.ip, 
                    client_name = EXCLUDED.client_name, 
                    client_version = EXCLUDED.client_version, 
                    client_build = EXCLUDED.client_build, 
                    client_arch = EXCLUDED.client_arch, 
                    os = EXCLUDED.os, 
                    client_language = EXCLUDED.client_language, 
                    enode_url = EXCLUDED.enode_url, 
                    port = EXCLUDED.port, 
                    chain = EXCLUDED.chain, 
                    genesis_hash = EXCLUDED.genesis_hash, 
                    best_block = EXCLUDED.best_block, 
                    total_difficulty = EXCLUDED.total_difficulty, 
                    country = EXCLUDED.country, 
                    city = EXCLUDED.city, 
                    last_seen = EXCLUDED.last_seen, 
                    capabilities = EXCLUDED.capabilities, 
                    eth_version = EXCLUDED.eth_version, 
                    synced = EXCLUDED.synced, 
                    isp = EXCLUDED.isp",
                    &[
                        &peer_data.id,
                        &peer_data.address,
                        &peer_data.client_name,
                        &peer_data.client_version,
                        &peer_data.client_build,
                        &peer_data.client_arch,
                        &peer_data.os,
                        &peer_data.client_language,
                        &peer_data.enode_url,
                        &(peer_data.tcp_port as i32),
                        &peer_data.chain,
                        &peer_data.genesis_block_hash,
                        &peer_data.best_block,
                        &peer_data.total_difficulty,
                        &peer_data.country,
                        &peer_data.city,
                        &peer_data.last_seen,
                        &peer_data.capabilities.join(","),
                        &(peer_data.eth_version as i32),
                        &peer_data.synced,
                        &peer_data.isp
                    ]
                ).await?;
        Ok(())
    }

    async fn all_peers(&self, _page_size: Option<i32>) -> Result<Vec<PeerData>, ScanTableError> {
        let stmt = "SELECT * from eth_peer_data";

        let rows = self.db.query(stmt, &[]).await?;

        let peers: Vec<PeerData> = rows
            .into_iter()
            .map(|row| PeerData {
                id: row.get(0),
                address: row.get(1),
                client_name: row.get(2),
                client_version: row.get(3),
                client_build: row.get(4),
                client_arch: row.get(5),
                os: row.get(6),
                client_language: row.get(7),
                enode_url: row.get(8),
                tcp_port: row.get::<_, u32>(9) as u16,
                chain: row.get(10),
                genesis_block_hash: row.get(11),
                best_block: row.get(12),
                total_difficulty: row.get(13),
                country: row.get(14),
                city: row.get(15),
                last_seen: row.get(16),
                capabilities: row
                    .get::<_, String>(17)
                    .as_str()
                    .split(',')
                    .map(|s| s.to_string())
                    .collect(),
                eth_version: row.get::<_, u32>(18) as u8,
                synced: row.get(19),
                isp: row.get(20),
            })
            .collect();

        Ok(peers)
    }

    async fn all_active_peers(
        &self,
        last_seen: String,
        _page_size: Option<i32>,
    ) -> Result<Vec<PeerData>, ScanTableError> {
        let stmt = "SELECT * from eth_peer_data WHERE last_seen > ?1";

        let rows = self.db.query(stmt, &[&last_seen]).await?;

        let peers: Vec<PeerData> = rows
            .into_iter()
            .map(|row| PeerData {
                id: row.get(0),
                address: row.get(1),
                client_name: row.get(2),
                client_version: row.get(3),
                client_build: row.get(4),
                client_arch: row.get(5),
                os: row.get(6),
                client_language: row.get(7),
                enode_url: row.get(8),
                tcp_port: row.get::<_, u32>(9) as u16,
                chain: row.get(10),
                genesis_block_hash: row.get(11),
                best_block: row.get(12),
                total_difficulty: row.get(13),
                country: row.get(14),
                city: row.get(15),
                last_seen: row.get(16),
                capabilities: row
                    .get::<_, String>(17)
                    .as_str()
                    .split(',')
                    .map(|s| s.to_string())
                    .collect(),
                eth_version: row.get::<_, u32>(18) as u8,
                synced: row.get(19),
                isp: row.get(20),
            })
            .collect();

        Ok(peers)
    }

    async fn node_by_id(&self, id: String) -> Result<Option<Vec<PeerData>>, QueryItemError> {
        let stmt = "SELECT * from eth_peer_data WHERE id = ?1";

        let rows = self.db.query(stmt, &[&id]).await?;

        let peers: Vec<PeerData> = rows
            .into_iter()
            .map(|row| PeerData {
                id: row.get(0),
                address: row.get(1),
                client_name: row.get(2),
                client_version: row.get(3),
                client_build: row.get(4),
                client_arch: row.get(5),
                os: row.get(6),
                client_language: row.get(7),
                enode_url: row.get(8),
                tcp_port: row.get::<_, u32>(9) as u16,
                chain: row.get(10),
                genesis_block_hash: row.get(11),
                best_block: row.get(12),
                total_difficulty: row.get(13),
                country: row.get(14),
                city: row.get(15),
                last_seen: row.get(16),
                capabilities: row
                    .get::<_, String>(17)
                    .as_str()
                    .split(',')
                    .map(|s| s.to_string())
                    .collect(),
                eth_version: row.get::<_, u32>(18) as u8,
                synced: row.get(19),
                isp: row.get(20),
            })
            .collect();

        Ok(Some(peers))
    }

    async fn node_by_ip(&self, ip: String) -> Result<Option<Vec<PeerData>>, QueryItemError> {
        let stmt = "SELECT * from eth_peer_data WHERE ip = ?1";

        let rows = self.db.query(stmt, &[&ip]).await?;

        let peers: Vec<PeerData> = rows
            .into_iter()
            .map(|row| PeerData {
                id: row.get(0),
                address: row.get(1),
                client_name: row.get(2),
                client_version: row.get(3),
                client_build: row.get(4),
                client_arch: row.get(5),
                os: row.get(6),
                client_language: row.get(7),
                enode_url: row.get(8),
                tcp_port: row.get::<_, u32>(9) as u16,
                chain: row.get(10),
                genesis_block_hash: row.get(11),
                best_block: row.get(12),
                total_difficulty: row.get(13),
                country: row.get(14),
                city: row.get(15),
                last_seen: row.get(16),
                capabilities: row
                    .get::<_, String>(17)
                    .as_str()
                    .split(',')
                    .map(|s| s.to_string())
                    .collect(),
                eth_version: row.get::<_, u32>(18) as u8,
                synced: row.get(19),
                isp: row.get(20),
            })
            .collect();

        Ok(Some(peers))
    }

    async fn clients(&self, _page_size: Option<i32>) -> Result<Vec<ClientData>, ScanTableError> {
        let stmt = "SELECT client_name from eth_peer_data";

        let rows = self.db.query(stmt, &[]).await?;

        let clients: Vec<ClientData> = rows
            .into_iter()
            .map(|row| ClientData {
                client_version: row.get(0),
            })
            .collect();

        Ok(clients)
    }

    async fn active_clients(
        &self,
        last_seen: String,
        _page_size: Option<i32>,
    ) -> Result<Vec<ClientData>, ScanTableError> {
        let stmt = "SELECT client_name from eth_peer_data WHERE last_seen > ?1";

        let rows = self.db.query(stmt, &[&last_seen]).await?;

        let clients: Vec<ClientData> = rows
            .into_iter()
            .map(|row| ClientData {
                client_version: row.get(0),
            })
            .collect();

        Ok(clients)
    }
}
