mod crawler;
mod types;
use crawler::CrawlerFactory;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let (_, _, _) = CrawlerFactory::new().await.make().await.run().await;
}
