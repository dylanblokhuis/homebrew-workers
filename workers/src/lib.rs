use app::App;
use axum::body::Body;
use axum::extract::Extension;
use axum::http::{Request, Response, StatusCode};
use axum::{routing::get, Router};
use migration::sea_orm::Database;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::oneshot::{self};

mod app;
mod runtime;

pub type V8HandlerResponse = (StatusCode, Response<Body>);
struct AppState {
    apps: Vec<App>,
}

pub async fn run() {
    let conn = Database::connect(
        std::env::var("DATABASE_URL")
            .expect("No DATABASE_URL environment variable found.")
            .as_str(),
    )
    .await
    .expect("Database connection failed");

    let apps = vec![
        App::new(
            "some-app".into(),
            PathBuf::from_str("./some-app").unwrap(),
            "main.js".into(),
        ),
        App::new(
            "example-worker".into(),
            PathBuf::from_str("./test/example-worker").unwrap(),
            "worker.js".into(),
        ),
    ];
    let app_state = Arc::new(AppState { apps });

    let worker_app = Router::new()
        .route("/*key", get(handler))
        .layer(Extension(app_state));
    let worker_addr = SocketAddr::from(([0, 0, 0, 0], 3000));

    println!("Workers listening on {}", worker_addr);

    axum::Server::bind(&worker_addr)
        .serve(worker_app.into_make_service())
        .await
        .unwrap();
}

#[axum_macros::debug_handler]
async fn handler(
    Extension(state): Extension<Arc<AppState>>,
    req: Request<Body>,
) -> V8HandlerResponse {
    let (tx, rx) = oneshot::channel::<V8HandlerResponse>();

    let header = req.headers().get("x-app");
    if let Some(header_value) = header {
        let app = state
            .apps
            .iter()
            .find(|it| it.name == header_value.to_str().unwrap())
            .unwrap();
        let runtime_channel = app.get_runtime().await;
        runtime_channel.send((req, tx)).await.unwrap();
    } else {
        let app = state.apps.get(0);
        if let Some(app) = app {
            let runtime_channel = app.get_runtime().await;
            runtime_channel.send((req, tx)).await.unwrap();
        } else {
            let request = Response::new(Body::empty());
            tx.send((StatusCode::BAD_REQUEST, request)).unwrap();
        }
    }

    rx.await.expect("Failed to receive value from V8 runtime.")
}