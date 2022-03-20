use app::App;
use axum::body::Body;
use axum::extract::Extension;
use axum::http::{Request, Response, StatusCode};
use axum::{routing::get, Router};
use rand::prelude::SliceRandom;
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
    let app = App::new(
        "some-app".into(),
        PathBuf::from_str("./some-app").unwrap(),
        "main.js".into(),
    );
    let apps = vec![app];
    let app_state = Arc::new(AppState { apps });

    let app = Router::new()
        .route("/*key", get(handler))
        .layer(Extension(app_state));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("Listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
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
        let app = state.apps.choose(&mut rand::thread_rng());
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
