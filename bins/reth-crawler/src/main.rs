mod crawler;
mod p2p;
use clap::{Args, Parser, Subcommand};
use crawler::CrawlerBuilder;

#[derive(Parser)]
#[command(author, version)]
#[command(
    about = "Reth crawler",
    long_about = "Reth crawler is a standalone program that crawls the p2p network.

One can use this crawler to quantify how many Ethereum nodes exists and what is the distribution of clients."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start crawling the network
    Crawl(CrawlOpts),
}

#[derive(Args)]
struct CrawlOpts {
    #[arg(long)]
    /// Use a sqlite db for local testing.
    local_db: bool,

    /// Eth RPC url to use for getting full blocks and determining whether or not a node is synced. It **MUST** be a web socket url.
    #[arg(long, default_value = "wss://localhost:8546")]
    eth_rpc_url: String,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match &cli.command {
        Commands::Crawl(opts) => {
            let builder = if opts.local_db {
                CrawlerBuilder::default().with_local_db()
            } else {
                CrawlerBuilder::default().without_local_db()
            };

            let (_, _, _, _) = builder
                .with_eth_rpc_url(opts.eth_rpc_url.clone())
                .build()
                .await
                .run()
                .await;
        }
    }
}
