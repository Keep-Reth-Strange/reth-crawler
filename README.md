# Reth-crawler

![CI](https://github.com/Keep-Reth-Strange/reth-crawler/actions/workflows/ci.yml/badge.svg)

Reth crawler MVP!
Related to [this issue](https://github.com/paradigmxyz/reth/issues/4778).

A sample of the peer data:

```json
{
    "enode_url":"enode://82bab81d909cf8487f0898650520aac56a26d813b45c23c9068216d0205c0e09ded7c2f651bd584d3226a1c5cd6b0a6afc70ef86885adfb49e7482be15bc9515@89.134.96.250:30403",
    "id":"0x82ba…9515",
    "address": "81.221.227.209",
    "tcp_port": 30303,
    "client_version": "Nethermind/v1.21.0+bb9b72c0/linux-x64/dotnet7.0.11",
    "eth_version": 68,
    "last_seen": "2023-10-04 05:59:56.470468 UTC",
    "country": "Switzerland",
    "city": "Kriens"
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

For local testing there is a flag to save peers in a `peers_data.json` file:

```bash
./reth-crawler crawl --save-to-json
```
