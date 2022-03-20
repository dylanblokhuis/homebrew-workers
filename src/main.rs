use app::App;
use axum::body::Body;
use axum::extract::Extension;
use axum::http::{Request, Response, StatusCode};
use axum::response::Html;
use axum::{routing::get, Router};
use dotenv::dotenv;
use migration::sea_orm::Database;
use migration::{Migrator, MigratorTrait};
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

#[tokio::main]
async fn main() {
    dotenv().ok();

    let conn = Database::connect(
        std::env::var("DATABASE_URL")
            .expect("No DATABASE_URL environment variable found.")
            .as_str(),
    )
    .await
    .expect("Database connection failed");

    Migrator::up(&conn, None).await.unwrap();

    let apps = vec![
        App::new(
            "example-worker".into(),
            PathBuf::from_str("./test/example-worker").unwrap(),
            "worker.js".into(),
        ),
        App::new(
            "some-app".into(),
            PathBuf::from_str("./some-app").unwrap(),
            "main.js".into(),
        ),
    ];
    let app_state = Arc::new(AppState { apps });

    let worker_app = Router::new()
        .route("/*key", get(handler))
        .layer(Extension(app_state));
    let worker_addr = SocketAddr::from(([0, 0, 0, 0], 3000));

    let admin_app = Router::new().route("/", get(admin_handler));
    let admin_addr = SocketAddr::from(([0, 0, 0, 0], 3001));

    println!("Admin listening on {}", admin_addr);
    println!("Workers listening on {}", worker_addr);

    let worker_server = axum::Server::bind(&worker_addr).serve(worker_app.into_make_service());
    let admin_server = axum::Server::bind(&admin_addr).serve(admin_app.into_make_service());

    let (_, _) = tokio::join!(worker_server, admin_server);
}

async fn admin_handler() -> Html<String> {
    Html("Admin".into())
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
