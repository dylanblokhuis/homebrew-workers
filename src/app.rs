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
use rand::{prelude::SliceRandom, thread_rng};
use std::{
    path::PathBuf,
    str::FromStr,
    sync::{Arc, Mutex},
    thread::{self},
    time::SystemTime,
};
use tokio::sync::{mpsc, oneshot};

use crate::{
    runtime::{self, RunOptions},
    V8HandlerResponse,
};

type RuntimeChannelPayload = (Request<Body>, oneshot::Sender<V8HandlerResponse>);

#[derive(Clone, Debug)]
pub struct V8Runtime {
    pub v8_sender: mpsc::UnboundedSender<RuntimeChannelPayload>,
}
pub struct App {
    pub name: String,
    pub path: PathBuf,
    pub runtimes: Arc<Mutex<Vec<V8Runtime>>>,
}

impl App {
    pub fn new(name: String, path: PathBuf) -> Self {
        Self {
            name,
            path,
            runtimes: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn get_runtime(&self) -> mpsc::UnboundedSender<RuntimeChannelPayload> {
        let mut senders = vec![];

        {
            let mut runtimes = self.runtimes.lock().unwrap();
            println!("before: {}", runtimes.len());

            if runtimes.is_empty() {
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

                let (tx, mut rx) = mpsc::unbounded_channel::<RuntimeChannelPayload>();
                thread::spawn(move || {
                    let mut runtime = spawn_v8_isolate(permissions);
                    handle_request(&mut runtime, &mut rx);
                    println!("Closing!");
                });

                println!("spawned a new runtime!");
                let tx2 = tx.clone();
                senders.push(V8Runtime { v8_sender: tx2 });
                *runtimes = senders;

                let runtimes2 = self.runtimes.clone();
                tokio::spawn(async move {
                    println!("Waiting for sender to close");
                    tx.closed().await;
                    println!("Sender closed");
                    let mut runtimes = runtimes2.lock().unwrap();
                    runtimes.drain(..);

                    println!("{:?}", runtimes);
                });
            }
        }

        let runtimes = self.runtimes.lock().unwrap();
        println!("after: {}", runtimes.len());

        let random_worker = runtimes
            .choose(&mut thread_rng())
            .expect("Could not found an open worker");

        random_worker.v8_sender.clone()
    }
}

fn spawn_v8_isolate(permissions: Permissions) -> JsRuntime {
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

    runtime::init(permissions, options)
}

#[tokio::main(flavor = "current_thread")]
async fn handle_request(
    runtime: &mut JsRuntime,
    rx: &mut mpsc::UnboundedReceiver<RuntimeChannelPayload>,
) {
    let mut last_request = 0;
    while let Some((request, oneshot_tx)) = rx.recv().await {
        let js_response = runtime::run_with_existing_runtime(runtime, request).await;
        let mut response = Response::new(Body::try_from(js_response.body).unwrap());
        let headers = response.headers_mut();
        for (key, value) in js_response.headers {
            headers.insert(
                HeaderName::from_str(key.as_str()).unwrap(),
                HeaderValue::from_str(value.as_str()).unwrap(),
            );
        }

        oneshot_tx.send((StatusCode::OK, response)).unwrap();
        last_request += 1;

        if last_request > 50 {
            rx.close();
        }
    }
}
