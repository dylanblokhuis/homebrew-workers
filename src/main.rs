use dotenv::dotenv;

#[tokio::main]
async fn main() {
    dotenv().ok();
    let worker_server = tokio::spawn(async move { workers::run().await });
    let api_server = tokio::spawn(async move { api::run().await });

    let (_, _) = tokio::join!(worker_server, api_server);
}
