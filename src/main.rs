use axum::body::Body;
use axum::extract::Extension;
use axum::http::header::HeaderName;
use axum::http::{HeaderValue, Request, Response};
use axum::{routing::get, Router};
use deno_runtime::permissions::{Permissions, PermissionsOptions};
use deno_runtime::BootstrapOptions;
use deno_web::BlobStore;
use rand::prelude::SliceRandom;
use std::net::SocketAddr;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::thread;
use tokio::sync::mpsc::{self, channel};
use tokio::sync::oneshot::{self, Sender};
use tokio::sync::Mutex;

use crate::runtime::RunOptions;

mod runtime;

type RuntimeChannel = mpsc::Sender<(Request<Body>, Sender<Response<Body>>)>;
struct AppState {
    senders: Vec<RuntimeChannel>,
}

#[tokio::main]
async fn main() {
    let senders: Arc<Mutex<Vec<RuntimeChannel>>> = Arc::new(Mutex::new(Vec::new()));

    for _ in 0..10 {
        let (tx, rx) = channel::<(Request<Body>, Sender<Response<Body>>)>(100);
        senders.lock().await.push(tx);
        thread::spawn(|| {
            get_js_runtime(rx);
        });
    }

    let lift_arc = Arc::try_unwrap(senders).unwrap();
    let senders_without_mutex = lift_arc.into_inner();
    let app_state = Arc::new(AppState {
        senders: senders_without_mutex,
    });

    let app = Router::new()
        .route("/*key", get(handler))
        .layer(Extension(app_state));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[axum_macros::debug_handler]
async fn handler(Extension(state): Extension<Arc<AppState>>, req: Request<Body>) -> Response<Body> {
    println!("FROM HANDLER: {}", req.uri());
    let (tx, rx) = oneshot::channel::<Response<Body>>();

    let random_item = state.senders.choose(&mut rand::thread_rng()).unwrap();
    random_item.send((req, tx)).await.unwrap();

    let response = rx.await.unwrap();
    response
}

#[tokio::main(flavor = "current_thread")]
async fn get_js_runtime(mut rx: mpsc::Receiver<(Request<Body>, Sender<Response<Body>>)>) {
    let options = RunOptions {
        bootstrap: BootstrapOptions {
            apply_source_maps: false,
            args: vec![],
            cpu_count: 1,
            debug_flag: false,
            enable_testing_features: false,
            location: None,
            no_color: false,
            is_tty: false,
            runtime_version: "x".to_string(),
            ts_version: "x".to_string(),
            unstable: false,
        },
        extensions: vec![],
        unsafely_ignore_certificate_errors: None,
        root_cert_store: None,
        user_agent: "hello_runtime".to_string(),
        seed: None,
        js_error_create_fn: None,
        maybe_inspector_server: None,
        should_break_on_first_statement: false,
        get_error_class_fn: Some(&runtime::get_error_class_name),
        blob_store: BlobStore::default(),
        shared_array_buffer_store: None,
        compiled_wasm_module_store: None,
    };

    let allowed_path = Path::new("./some-app");
    let permission_options = PermissionsOptions {
        allow_env: None,
        allow_ffi: None,
        allow_hrtime: false,
        allow_run: None,
        allow_write: None,
        prompt: false,
        allow_net: Some(vec![]),
        allow_read: Some(vec![allowed_path.to_path_buf()]),
    };
    let permissions = Permissions::from_options(&permission_options);
    let mut js_runtime = runtime::init(permissions, options);

    println!("Runtime created!");

    while let Some((request, req_tx)) = rx.recv().await {
        println!("FROM JS_RUNTIME: {}", request.uri());
        let js_response = runtime::run_with_existing_runtime(&mut js_runtime, request).await;
        let mut response = Response::new(Body::try_from(js_response.body).unwrap());
        let headers = response.headers_mut();
        for (key, value) in js_response.headers {
            headers.insert(
                HeaderName::from_str(key.as_str()).unwrap(),
                HeaderValue::from_str(value.as_str()).unwrap(),
            );
        }

        req_tx.send(response).unwrap();
    }
}
