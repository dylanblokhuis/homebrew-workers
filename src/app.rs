use axum::{
    body::Body,
    http::{header::HeaderName, HeaderValue, Request, Response, StatusCode},
};
use core::time;
use deno_core::JsRuntime;
use deno_runtime::{
    permissions::{Permissions, PermissionsOptions},
    BootstrapOptions,
};
use deno_web::BlobStore;
use rand::{prelude::SliceRandom, thread_rng, Rng};
use std::{
    path::PathBuf,
    str::FromStr,
    sync::{Arc, Mutex},
    thread::{self},
    time::Instant,
};
use tokio::sync::{mpsc, oneshot, RwLock};

use crate::{
    runtime::{self, RunOptions},
    V8HandlerResponse,
};

type RuntimeChannelPayload = (Request<Body>, oneshot::Sender<V8HandlerResponse>);

#[derive(Clone, Debug)]
pub struct V8Runtime {
    pub id: i32,
    pub v8_sender: mpsc::Sender<RuntimeChannelPayload>,
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

    pub async fn get_runtime(&self) -> mpsc::Sender<RuntimeChannelPayload> {
        if self.runtimes.lock().unwrap().len() == 0 {
            self.new_worker().await;
        }

        let runtimes = self.runtimes.lock().unwrap();
        println!("We have {} runtimes, picking one..", runtimes.len());
        let random_worker = runtimes
            .choose(&mut thread_rng())
            .expect("Could not found an open worker");

        random_worker.v8_sender.clone()
    }

    async fn new_worker(&self) {
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
        let (tx, mut rx) = mpsc::channel::<RuntimeChannelPayload>(1);

        thread::spawn(move || {
            let mut runtime = spawn_v8_isolate(permissions);
            handle_request(&mut runtime, &mut rx);
            println!("Closing!");
        });

        let mut runtimes = self.runtimes.lock().unwrap();

        let tx2 = tx.clone();
        let index = rand::thread_rng().gen::<i32>();
        runtimes.push(V8Runtime {
            id: index,
            v8_sender: tx2,
        });

        let runtimes2 = self.runtimes.clone();
        tokio::spawn(async move {
            println!("Waiting for sender to close");
            tx.closed().await;
            println!("{} Closed", index);
            let mut runtimes = runtimes2.lock().unwrap();
            for (key, runtime) in runtimes.clone().iter().enumerate() {
                if runtime.id == index {
                    runtimes.remove(key);
                }
            }
        });
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

#[tokio::main]
async fn handle_request(runtime: &mut JsRuntime, rx: &mut mpsc::Receiver<RuntimeChannelPayload>) {
    let last_request = Arc::new(RwLock::new(Instant::now()));

    loop {
        let last_request2 = Arc::clone(&last_request);

        let handle = tokio::spawn(async move {
            loop {
                if last_request2.read().await.elapsed().as_secs() > 5 {
                    break;
                }

                // We sleep here due to read locks being slow, maybe use a mpsc channel here instead?
                tokio::time::sleep(time::Duration::from_secs(1)).await;
            }
        });

        tokio::select! {
            Some((request, oneshot_tx)) = rx.recv() => {
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


                let mut last_request_lock = last_request.write().await;
                *last_request_lock = Instant::now();
            }
            _ = handle => {
                println!("5 seconds passed, so we're killing this runtime.");
                break;
            }
        }
    }

    // something.await.unwrap();
    println!("Closing handle_request thread");
}
