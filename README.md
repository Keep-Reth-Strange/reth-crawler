# Reth-crawler

![CI](https://github.com/Keep-Reth-Strange/reth-crawler/actions/workflows/ci.yml/badge.svg)

Reth crawler MVP!
Related to [this issue](https://github.com/paradigmxyz/reth/issues/4778).

A sample of the peer data:

```json
{

    {
        "enode_url":"enode://0bb46e5532da93328bf6964309081f9fef7a19f29d91f93004f82ecd897766a15348f6863c4f36541f632d50c7d592c6b9181e439e9ede16c7e0cd18612cd048@158.220.96.114:34310","id":"0x0bb46e5532da93328bf6964309081f9fef7a19f29d91f93004f82ecd897766a15348f6863c4f36541f632d50c7d592c6b9181e439e9ede16c7e0cd18612cd048","address":"158.220.96.114","tcp_port":34310,"client_version":"Geth/v1.13.4-stable-3f907d6a/linux-amd64/go1.21.3","eth_version":68,"capabilities":["eth/67","eth/68","snap/1"],"chain":"mainnet","total_difficulty":"17179869184","best_block":"0xd4e56740f876aef8c010b86a40d5f56745a118d0906a34e69aec8c0db1cb8fa3","genesis_block_hash":"0xd4e56740f876aef8c010b86a40d5f56745a118d0906a34e69aec8c0db1cb8fa3","last_seen":"2023-11-08 15:41:04.469473084 UTC","country":"Germany","city":"Düsseldorf","synced":null,"isp":"Contabo GmbH"
    }
}
```

## How to build the crawler

`reth-crawler` is stable now and it's in active development. In order to try it, follow these steps:

### Clone the repository

```bash
git clone https://github.com/Keep-Reth-Strange/reth-crawler
```

### Run it

Open the directory where it's been installed previously and build it:

```bash
cargo build -p reth-crawler
```

Go to `target/debug` folder:

```bash
cd target/debug
```

Run it:

```bash
./reth-crawler crawl
```

### Run it locally without a centralized db

For local testing there is a flag to save peers in a `peers_data.db` file (sqlite db):

```bash
./reth-crawler crawl --local-db
```

## The API server

The API server is not necessary for crawling the network but it's very useful to serve the crawled data.

Right now it's configured by default to connect to the dynamo db database used by the crawler,
so if you have not set up an Amazon db before, it will not run.

We are now working on migrate our infra from Amazon to a new service provider and we'll also
let the API server be more flexible for local testing.

### How to run the API server

Open the directory where it's been installed previously and build it:

```bash
cargo build -p reth-crawler-api-server
```

Go to `target/debug` folder:

```bash
cd target/debug
```

Run it:

```bash
./reth-crawler-api-server start-api-server
```
