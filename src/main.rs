#[tokio::main]
async fn main() {
    let worker_server = workers::run();
    let api_server = api::run();

    tokio::join!(worker_server, api_server);
}
