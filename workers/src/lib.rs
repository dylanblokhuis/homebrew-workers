#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(clippy::future_not_send)]
#![allow(clippy::diverging_sub_expression)]
use app::App;
use async_zip::read::mem::ZipFileReader;
use axum::body::Body;
use axum::extract::Extension;
use axum::http::{Request, Response, StatusCode};
use axum::{routing::any, Router};
use migration::sea_orm::{Database, EntityTrait};
use session::Session;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot::{self};
use tokio::sync::RwLock;

use entity::user;

pub mod app;
mod runtime;
mod snapshot;

#[derive(Clone)]
struct AppState {
    apps: Arc<RwLock<Vec<App>>>,
}

/// # Errors
///
/// Will return `Err` if webserver panics
pub async fn run(maybe_default_app: Option<App>) -> anyhow::Result<()> {
    let apps: Arc<RwLock<Vec<App>>>;
    if let Some(default_app) = maybe_default_app {
        apps = Arc::new(RwLock::new(vec![default_app]));
    } else {
        apps = Arc::new(RwLock::new(setup(&[]).await));

        let apps2 = apps.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));

            loop {
                interval.tick().await;
                let new_apps = {
                    let existing_apps = apps2.read().await;
                    setup(&existing_apps).await
                };
                *apps2.write().await = new_apps;
            }
        });
    }

    let app_state = AppState { apps };

    let worker_app = Router::new()
        .route("/*key", any(handler))
        .layer(Extension(Arc::new(app_state)));
    let worker_addr = SocketAddr::from(([0, 0, 0, 0], 3000));

    println!("Workers listening on {}", worker_addr);

    axum::Server::bind(&worker_addr)
        .serve(worker_app.into_make_service())
        .await?;

    Ok(())
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

async fn setup(existing_apps: &[App]) -> Vec<App> {
    let conn = Database::connect(
        std::env::var("DATABASE_URL")
            .expect("No DATABASE_URL environment variable found.")
            .as_str(),
    )
    .await
    .expect("Database connection failed");

    let bucket = init_bucket();
    let users = user::Entity::find()
        .all(&conn)
        .await
        .expect("Failed to setup the initial users");

    let mut apps: Vec<App> = vec![];
    for user in &users {
        let deployment_path = match &user.latest_deployment {
            Some(deployment_path) => deployment_path,
            None => continue,
        };

        let maybe_app_has_no_new = existing_apps
            .iter()
            .find(|app| &app.deployment == deployment_path && app.name == user.name);

        if let Some(app) = maybe_app_has_no_new {
            apps.push(app.clone());
            continue;
        }

        let (bytes, code) = bucket.get_object(&deployment_path).await.unwrap();
        assert!(code == 200, "Couldn't get item from bucket");

        let parent_dir = format!("/tmp/homebrew-workers/{}", user.id);
        tokio::fs::remove_dir_all(&parent_dir).await.unwrap_or(());
        tokio::fs::create_dir_all(&parent_dir).await.unwrap();

        let zip = ZipFileReader::new(&bytes).await.unwrap();
        let mut zip_2 = ZipFileReader::new(&bytes).await.unwrap();

        for (index, entry) in zip.entries().iter().enumerate() {
            if entry.dir() {
                continue;
            }

            let reader = zip_2.entry_reader(index).await.unwrap();
            let path_str = format!("{}/{}", &parent_dir, entry.name());
            let path = Path::new(&path_str);
            tokio::fs::create_dir_all(path.parent().unwrap())
                .await
                .unwrap();

            let mut output = tokio::fs::File::create(path).await.unwrap();
            reader.copy_to_end_crc(&mut output, 65536).await.unwrap();
        }

        let session = Session {
            user_id: user.id,
            conn: conn.clone(),
        };

        let app = App::new(
            session,
            user.name.clone(),
            PathBuf::from_str(parent_dir.clone().as_str()).unwrap(),
            "main.js".into(),
            deployment_path.into(),
        );

        println!("New deployment found: {}", deployment_path);
        apps.push(app);
    }

    // temporary so the order of keys doesnt get messed up
    apps.sort_by(|a, b| a.deployment.cmp(&b.deployment));
    apps
}

async fn handler(Extension(state): Extension<Arc<AppState>>, req: Request<Body>) -> Response<Body> {
    let (tx, rx) = oneshot::channel::<Response<Body>>();

    let header = req.headers().get("x-app");
    if let Some(header_value) = header {
        let guard = state.apps.read().await;
        let app = guard
            .iter()
            .find(|it| it.name == header_value.to_str().unwrap())
            .unwrap();
        let runtime_channel = app.get_runtime().await;
        runtime_channel.send((req, tx)).await.unwrap();
    } else {
        let guard = state.apps.read().await;
        let app = guard.get(0);
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
