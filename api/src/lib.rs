use axum::extract::Extension;
use axum::http::header;
use axum::routing::post;
use axum::Router;
use migration::sea_orm::Database;
use migration::{Migrator, MigratorTrait};
use std::net::SocketAddr;
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::ServiceBuilderExt;

use crate::admin::router;
use crate::middleware::auth::authorize_route;
mod admin;
mod middleware;

pub async fn run() {
    let conn = Database::connect(
        std::env::var("DATABASE_URL")
            .expect("No DATABASE_URL environment variable found.")
            .as_str(),
    )
    .await
    .expect("Database connection failed");

    Migrator::up(&conn, None).await.unwrap();

    let sensitive_headers: Arc<[_]> = vec![header::AUTHORIZATION, header::COOKIE].into();
    let middleware = ServiceBuilder::new()
        .sensitive_request_headers(sensitive_headers.clone())
        .compression();

    let app = Router::new()
        .route("/authorize", post(authorize_route))
        .nest("/admin", router())
        .layer(Extension(conn))
        .layer(middleware);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3001));

    println!("Admin listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
