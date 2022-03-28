use app::App;
use async_zip::read::mem::ZipFileReader;
use axum::body::Body;
use axum::extract::Extension;
use axum::http::{Request, Response, StatusCode};
use axum::{routing::any, Router};
use migration::sea_orm::{Database, DatabaseConnection, EntityTrait};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::oneshot::{self};

use entity::user;

mod app;
mod runtime;

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

    let apps = setup(conn).await;
    let app_state = Arc::new(AppState { apps });

    let worker_app = Router::new()
        .route("/*key", any(handler))
        .layer(Extension(app_state));
    let worker_addr = SocketAddr::from(([0, 0, 0, 0], 3000));

    println!("Workers listening on {}", worker_addr);

    axum::Server::bind(&worker_addr)
        .serve(worker_app.into_make_service())
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

    let region = s3::Region::Custom {
        region: std::env::var("S3_REGION").expect("S3_REGION not found"),
        endpoint: std::env::var("S3_ENDPOINT").expect("S3_ENDPOINT not found"),
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

async fn setup(conn: DatabaseConnection) -> Vec<App> {
    let bucket = init_bucket();
    let users = user::Entity::find()
        .all(&conn)
        .await
        .expect("Failed to setup the initial users");

    let mut apps = vec![];
    for user in users.iter() {
        let path = user.latest_deployment.as_ref();
        if path.is_none() {
            continue;
        }

        let (bytes, code) = bucket.get_object(path.unwrap()).await.unwrap();
        if code != 200 {
            panic!("Couldn't get item from bucket");
        }

        let parent_dir = format!("/tmp/homebrew-workers/{}", user.id);
        tokio::fs::remove_dir_all(parent_dir.clone())
            .await
            .unwrap_or(());
        tokio::fs::create_dir_all(parent_dir.clone()).await.unwrap();

        let zip = ZipFileReader::new(&bytes).await.unwrap();
        let mut zip_2 = ZipFileReader::new(&bytes).await.unwrap();

        for (index, entry) in zip.entries().iter().enumerate() {
            if entry.dir() {
                continue;
            }

            let reader = zip_2.entry_reader(index).await.unwrap();
            let path_str = format!("{}/{}", parent_dir.clone(), entry.name());
            let path = Path::new(&path_str);
            tokio::fs::create_dir_all(path.parent().unwrap())
                .await
                .unwrap();

            let mut output = tokio::fs::File::create(path).await.unwrap();
            reader.copy_to_end_crc(&mut output, 65536).await.unwrap();
        }

        let app = App::new(
            user.name.clone(),
            PathBuf::from_str(parent_dir.clone().as_str()).unwrap(),
            "main.js".into(),
        );

        apps.push(app);
    }

    apps
}

#[axum_macros::debug_handler]
async fn handler(Extension(state): Extension<Arc<AppState>>, req: Request<Body>) -> Response<Body> {
    let (tx, rx) = oneshot::channel::<Response<Body>>();

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
            let mut response = Response::new(Body::empty());
            *response.status_mut() = StatusCode::BAD_REQUEST;
            tx.send(response).unwrap();
        }
    }

    rx.await.expect("Failed to receive value from V8 runtime.")
}
