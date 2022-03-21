use dotenv::dotenv;

#[tokio::main]
async fn main() {
    dotenv().ok();
    let worker_server = workers::run();
    let api_server = api::run();

    tokio::join!(worker_server, api_server);
}
