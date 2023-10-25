mod crawler;
mod p2p;
use clap::{Args, Parser, Subcommand};
use crawler::CrawlerFactory;

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
    #[arg(long, conflicts_with = "save_to_json")]
    /// Save peers into an in memory db. This is useful for local testing because you don't have to setup a centralized database.
    sql_db: bool,
    #[arg(long, conflicts_with = "sql_db")]
    /// Save file into a json file called `peers_node.json` instead of saving them into a database.
    save_to_json: bool,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match &cli.command {
        Commands::Crawl(opts) => {
            let (_, _) = CrawlerFactory::new()
                .await
                .make(opts.sql_db)
                .await
                .run(opts.save_to_json)
                .await;
        }
    }
}
