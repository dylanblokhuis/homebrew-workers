use axum::{
    body::Body,
    http::{header::HeaderName, HeaderValue, Request, Response, StatusCode},
};
use deno_core::JsRuntime;
use deno_runtime::{
    permissions::{Permissions, PermissionsOptions},
    BootstrapOptions,
};
use deno_web::BlobStore;
use std::{
    path::PathBuf,
    str::FromStr,
    sync::{Arc, RwLock},
    thread::{self},
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};

use crate::{
    runtime::{self, RunOptions},
    V8HandlerResponse,
};

type RuntimeChannelPayload = (Request<Body>, oneshot::Sender<V8HandlerResponse>);

pub struct App {
    pub name: String,
    pub path: PathBuf,
    pub runtime: Arc<RwLock<Option<mpsc::Sender<RuntimeChannelPayload>>>>,
}

impl App {
    pub fn new(name: String, path: PathBuf) -> Self {
        Self {
            name,
            path,
            runtime: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn get_runtime(&self) -> mpsc::Sender<RuntimeChannelPayload> {
        if self.runtime.read().unwrap().is_none() {
            self.new_worker().await;
        }

        let item = self.runtime.read().unwrap();
        let item = item.as_ref();
        item.unwrap().clone()
    }

    async fn new_worker(&self) {
        println!("New worker spawned");
        let permission_options = PermissionsOptions {
            allow_env: None,
            allow_ffi: None,
            allow_hrtime: false,
            allow_run: None,
            allow_write: None,
            prompt: false,
            allow_net: Some(vec![]),
            allow_read: Some(vec![self.path.to_path_buf()]),
        };
        let permissions = Permissions::from_options(&permission_options);
        let (tx, mut rx) = mpsc::channel::<RuntimeChannelPayload>(10);

        let name = self.name.clone();
        thread::spawn(move || {
            let mut runtime = spawn_v8_isolate(permissions);

            tokio::runtime::Builder::new_multi_thread()
                .thread_name("runtime-pool")
                .worker_threads(2)
                .enable_time()
                .build()
                .unwrap()
                .block_on(async {
                    handle_request(name, &mut runtime, &mut rx).await;
                });
        });

        {
            let tx2 = tx.clone();
            *self.runtime.write().unwrap() = Some(tx2);
        }

        let runtime2 = self.runtime.clone();
        tokio::spawn(async move {
            tx.closed().await;
            let mut item = runtime2.write().unwrap();
            *item = None;
        });
    }
}

fn spawn_v8_isolate(permissions: Permissions) -> JsRuntime {
    let options = RunOptions {
        bootstrap: BootstrapOptions {
            apply_source_maps: false,
            args: vec![],
            cpu_count: std::thread::available_parallelism().unwrap().into(),
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

    runtime::init(permissions, options)
}

async fn handle_request(
    name: String,
    runtime: &mut JsRuntime,
    rx: &mut mpsc::Receiver<RuntimeChannelPayload>,
) {
    loop {
        tokio::select! {
            Some((request, oneshot_tx)) = rx.recv() => {
                let js_response = runtime::run_with_existing_runtime(name.clone(), runtime, request).await;
                let mut response = Response::new(Body::try_from(js_response.body).unwrap());
                let headers = response.headers_mut();
                for (key, value) in js_response.headers {
                    headers.insert(
                        HeaderName::from_str(key.as_str()).unwrap(),
                        HeaderValue::from_str(value.as_str()).unwrap(),
                    );
                }

                oneshot_tx.send((StatusCode::OK, response)).unwrap();
            }
            _ = tokio::time::sleep(Duration::from_secs(5)) => {
                println!("5 seconds passed without a request, so we're killing this runtime.");
                break;
            }
        }
    }
}
