use axum::extract::Extension;
use axum::http::header;
use axum::routing::post;
use axum::Router;
use migration::sea_orm::Database;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::ServiceBuilderExt;

use crate::middleware::auth::authorize_route;

mod admin;
mod errors;
mod middleware;
mod user;

pub async fn run() {
    let conn = Database::connect(
        std::env::var("DATABASE_URL")
            .expect("No DATABASE_URL environment variable found.")
            .as_str(),
    )
    .await
    .expect("Database connection failed");

    let bucket = init_bucket();

    let sensitive_headers: Arc<[_]> = vec![header::AUTHORIZATION, header::COOKIE].into();
    let middleware = ServiceBuilder::new()
        .sensitive_request_headers(sensitive_headers.clone())
        .compression();

    let app = Router::new()
        .route("/authorize", post(authorize_route))
        .nest("/admin", admin::router())
        .nest("/:user_id", user::router())
        .layer(Extension(conn))
        .layer(Extension(bucket))
        .layer(middleware);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3001));

    println!("Api listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

fn init_bucket() -> s3::Bucket {
    let credentials = s3::creds::Credentials::new(
        Some(
            std::env::var("S3_ACCESS_KEY")
                .expect("S3_ACCESS_KEY not found")
                .as_str(),
        ),
        Some(
            std::env::var("S3_SECRET_KEY")
                .expect("S3_SECRET_KEY not found")
                .as_str(),
        ),
        None,
        None,
        None,
    )
    .unwrap();

    let maybe_endpoint = std::env::var("S3_ENDPOINT");

    if maybe_endpoint.is_err() {
        let bucket = s3::Bucket::new(
            std::env::var("S3_BUCKET")
                .expect("S3_BUCKET not found")
                .as_str(),
            s3::Region::from_str(
                std::env::var("S3_REGION")
                    .expect("S3_REGION not found")
                    .as_str(),
            )
            .expect("Unknown region"),
            credentials,
        )
        .unwrap();

        return bucket;
    }

    let region = s3::Region::Custom {
        region: std::env::var("S3_REGION").expect("S3_REGION not found"),
        endpoint: maybe_endpoint.unwrap(),
    };

    let bucket = s3::Bucket::new_with_path_style(
        std::env::var("S3_BUCKET")
            .expect("S3_BUCKET not found")
            .as_str(),
        region,
        credentials,
    )
    .unwrap();

    bucket
}
