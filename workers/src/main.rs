use dotenv::dotenv;

#[tokio::main]
async fn main() {
    dotenv().ok();
    workers::run().await;
}
