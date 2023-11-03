# Reth-crawler

![CI](https://github.com/Keep-Reth-Strange/reth-crawler/actions/workflows/ci.yml/badge.svg)

Reth crawler MVP!
Related to [this issue](https://github.com/paradigmxyz/reth/issues/4778).

A sample of the peer data:

```json
{

    {"enode_url":"enode://c9ec33f3d03e4a349d698e665bbb6ae16f0bd4cdc972e13272970ff9b2c79135325e44b7b0004e30ede1d08d43d3fd8c6e73409d29649f9bc49dca3cdcd592c6@86.10.78.89:30303","id":"0xc9ec33f3d03e4a349d698e665bbb6ae16f0bd4cdc972e13272970ff9b2c79135325e44b7b0004e30ede1d08d43d3fd8c6e73409d29649f9bc49dca3cdcd592c6","address":"86.10.78.89","tcp_port":30303,"client_version":"Nethermind/v1.21.0+bb9b72c0/linux-x64/dotnet7.0.11","eth_version":68,"capabilities":["eth/66","eth/67","eth/68"],"chain":"mainnet","total_difficulty":"58750003716598352816469","best_block":"0x371d17888e3af260d26a1f21604fcd9fdc54903a5533029694d436141039772b","genesis_block_hash":"0xd4e56740f876aef8c010b86a40d5f56745a118d0906a34e69aec8c0db1cb8fa3","last_seen":"2023-11-03 06:55:31.328511221 UTC","country":"United Kingdom","city":"Warwick"},

    {"enode_url":"enode://ba0099e07da3057013d87f8dd77010beddd29f7bf65ce63a6d0529599b33c7c27f7d9432c95f57fd8b49995013778d3c81ba66e23815cd1fb1dc1564287ae8a5@107.22.198.23:27698","id":"0xba0099e07da3057013d87f8dd77010beddd29f7bf65ce63a6d0529599b33c7c27f7d9432c95f57fd8b49995013778d3c81ba66e23815cd1fb1dc1564287ae8a5","address":"107.22.198.23","tcp_port":27698,"client_version":"Geth/v1.13.4-stable-3f907d6a/linux-amd64/go1.21.3","eth_version":68,"capabilities":["eth/67","eth/68","snap/1"],"chain":"mainnet","total_difficulty":"58750003716598352816469","best_block":"0x6d96fdae4fcd7cf9d2dc9180c0e96ea35e7d8debd585f08b01f2f04ed9c88ee0","genesis_block_hash":"0xd4e56740f876aef8c010b86a40d5f56745a118d0906a34e69aec8c0db1cb8fa3","last_seen":"2023-11-03 11:08:13.598936510 UTC","country":"United States","city":"Ashburn"},
}
```

## How to build the crawler

`reth-crawler` is not stable now and it's in active development. In order to try it, follow these steps:

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
