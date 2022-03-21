use dotenv::dotenv;

#[tokio::main]
async fn main() {
    dotenv().ok();
    api::run().await;
}
