use dotenv::dotenv;
use migration::{sea_orm::Database, Migrator, MigratorTrait};

#[tokio::main]
async fn main() {
    dotenv().ok();

    {
        let conn = Database::connect(
            std::env::var("DATABASE_URL")
                .expect("No DATABASE_URL environment variable found.")
                .as_str(),
        )
        .await
        .expect("Database connection failed");
    
        Migrator::up(&conn, None).await.unwrap();
    }
    
    let worker_server = tokio::spawn(async move { workers::run().await });
    let api_server = tokio::spawn(async move { api::run().await });

    let (_, _) = tokio::join!(worker_server, api_server);
}
