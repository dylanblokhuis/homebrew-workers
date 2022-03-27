use axum::body::Body;
use axum::http::header::HeaderName;
use axum::http::header::HOST;
use axum::http::HeaderValue;
use axum::http::Request;
use axum::http::Response;
use axum::http::StatusCode;
use deno_core::error::AnyError;
use deno_core::located_script_name;
use deno_core::CompiledWasmModuleStore;
use deno_core::Extension;
use deno_core::GetErrorClassFn;
use deno_core::JsErrorCreateFn;
use deno_core::JsRuntime;
use deno_core::RuntimeOptions;
use deno_core::SharedArrayBufferStore;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::js;
use deno_runtime::ops;
use deno_runtime::permissions::Permissions;
use deno_runtime::BootstrapOptions;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::str::FromStr;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::app::RuntimeChannelPayload;

pub struct Runtime {
    js_runtime: JsRuntime,
}

impl Runtime {
    pub fn new(script_path: PathBuf, permissions: Permissions) -> Self {
        Self {
            js_runtime: init(script_path, permissions),
        }
    }

    async fn run(&mut self, request: Request<Body>) -> JsResponse {
        let js_runtime = &mut self.js_runtime;

        {
            let scope = &mut js_runtime.handle_scope();
            let event_obj = v8::Object::new(scope);
            let request_obj = v8::Object::new(scope);

            let url_key = v8::String::new(scope, "url").unwrap();
            let url = format!(
                "http://{}{}",
                request.headers().get(HOST).unwrap().to_str().unwrap(),
                request.uri().path()
            );
            let url_value = v8::String::new(scope, &url).unwrap();

            request_obj.set(scope, url_key.into(), url_value.into());

            let method_key = v8::String::new(scope, "method").unwrap();
            let method_value = v8::String::new(scope, request.method().as_str()).unwrap();
            request_obj.set(scope, method_key.into(), method_value.into());

            let header_key = v8::String::new(scope, "headers").unwrap();
            let header_object = v8::Object::new(scope);
            for (key, value) in request.headers() {
                let key = v8::String::new(scope, key.as_str()).unwrap();
                let value = v8::String::new(scope, value.to_str().unwrap()).unwrap();

                header_object.set(scope, key.into(), value.into());
            }
            request_obj.set(scope, header_key.into(), header_object.into());

            let event_request_key = v8::String::new(scope, "request").unwrap();
            event_obj.set(scope, event_request_key.into(), request_obj.into());
            let event_respond_key = v8::String::new(scope, "respondWith").unwrap();

            let context = scope.get_current_context();
            let global = context.global(scope);

            let respond_with_func = global.get(scope, event_respond_key.into()).unwrap();
            event_obj.set(scope, event_respond_key.into(), respond_with_func);

            let name = v8::String::new(scope, "onRequest").unwrap();
            let func = global.get(scope, name.into()).unwrap();

            let cb = v8::Local::<v8::Function>::try_from(func).unwrap();
            let args = &[event_obj.into()];
            cb.call(scope, global.into(), args).unwrap();
        }

        {
            js_runtime.run_event_loop(false).await.unwrap();
        }

        let js_response = {
            let context = js_runtime.global_context();
            let scope = &mut js_runtime.handle_scope();
            let global = context.open(scope).global(scope);
            let name = v8::String::new(scope, "requestResult").unwrap();
            let response = global.get(scope, name.into()).unwrap();
            global.delete(scope, name.into()).unwrap();

            // let body_key = v8::String::new(scope, "body").unwrap();
            // let body = response
            //     .to_object(scope)
            //     .unwrap()
            //     .get(scope, body_key.into())
            //     .unwrap();
            // let uint8array = v8::Local::<v8::Uint8Array>::try_from(body).unwrap();
            // println!("{:?}", uint8array.copy_contents(dest));

            let js_response: JsResponse = deno_core::serde_v8::from_v8(scope, response).unwrap();

            js_response
        };

        js_response
    }

    pub fn terminate(&mut self) {
        let isolate = self.js_runtime.v8_isolate().thread_safe_handle();
        isolate.terminate_execution();
    }

    pub async fn handle_request(&mut self, rx: &mut mpsc::Receiver<RuntimeChannelPayload>) {
        loop {
            let sleep = tokio::time::sleep(Duration::from_secs(5));
            tokio::pin!(sleep);

            tokio::select! {
                Some((request, oneshot_tx)) = rx.recv() => {
                    let js_response = self.run(request).await;
                    let body = String::from_utf8(js_response.body.to_vec()).unwrap_or_else(|_| "".into());

                    let mut response = Response::new(Body::try_from(body).unwrap());
                    let headers = response.headers_mut();
                    for (key, value) in js_response.headers {
                        headers.insert(
                            HeaderName::from_str(key.as_str()).unwrap(),
                            HeaderValue::from_str(value.as_str()).unwrap(),
                        );
                    }

                    oneshot_tx.send((StatusCode::OK, response)).unwrap();
                }
                _ = &mut sleep => {
                    println!("5 seconds passed without a request, so we're killing this runtime.");
                    self.terminate();
                    break;
                }
            }
        }
    }
}

struct RunOptions {
    pub bootstrap: BootstrapOptions,
    pub extensions: Vec<Extension>,
    pub unsafely_ignore_certificate_errors: Option<Vec<String>>,
    pub user_agent: String,
    pub seed: Option<u64>,
    pub js_error_create_fn: Option<Rc<JsErrorCreateFn>>,
    pub get_error_class_fn: Option<GetErrorClassFn>,
    pub blob_store: BlobStore,
    pub shared_array_buffer_store: Option<SharedArrayBufferStore>,
    pub compiled_wasm_module_store: Option<CompiledWasmModuleStore>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct JsResponse {
    pub headers: HashMap<String, String>,
    pub ok: bool,
    pub redirected: bool,
    pub status: u16,
    #[serde(rename = "statusText")]
    pub status_text: String,
    pub body: deno_core::serde_v8::Buffer,
}

fn get_error_class_name(e: &AnyError) -> &'static str {
    deno_runtime::errors::get_error_class_name(e).unwrap_or("Error")
}

fn init(script_path: PathBuf, permissions: Permissions) -> deno_core::JsRuntime {
    let mut options = RunOptions {
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
        seed: Some(32),
        js_error_create_fn: None,
        get_error_class_fn: Some(&get_error_class_name),
        blob_store: BlobStore::default(),
        shared_array_buffer_store: None,
        compiled_wasm_module_store: None,
    };

    let unstable = options.bootstrap.unstable;
    let enable_testing_features = options.bootstrap.enable_testing_features;
    let perm_ext = Extension::builder()
        .state(move |state| {
            state.put::<Permissions>(permissions.clone());
            state.put(ops::UnstableChecker { unstable });
            state.put(ops::TestingFeaturesEnabled(enable_testing_features));
            Ok(())
        })
        .build();

    // Internal modules
    let mut extensions: Vec<Extension> = vec![
        // Web APIs
        deno_webidl::init(),
        deno_console::init(),
        deno_url::init(),
        deno_web::init::<Permissions>(
            options.blob_store.clone(),
            options.bootstrap.location.clone(),
        ),
        deno_fetch::init::<Permissions>(deno_fetch::Options {
            user_agent: options.user_agent.clone(),
            unsafely_ignore_certificate_errors: options.unsafely_ignore_certificate_errors.clone(),
            file_fetch_handler: Rc::new(deno_fetch::FsFetchHandler),
            ..Default::default()
        }),
        // deno_websocket::init::<Permissions>(
        //     options.user_agent.clone(),
        //     options.root_cert_store.clone(),
        //     options.unsafely_ignore_certificate_errors.clone(),
        // ),
        // deno_webstorage::init(options.origin_storage_dir.clone()),
        deno_crypto::init(options.seed),
        // deno_broadcast_channel::init(options.broadcast_channel.clone(), unstable),
        // deno_webgpu::init(unstable),
        // ffi
        // deno_ffi::init::<Permissions>(unstable),
        // Runtime ops
        // ops::runtime::init(main_module.clone()),
        // ops::worker_host::init(
        //     options.create_web_worker_cb.clone(),
        //     options.web_worker_preload_module_cb.clone(),
        // ),
        ops::fs_events::init(),
        ops::fs::init(),
        ops::io::init(),
        ops::io::init_stdio(),
        deno_tls::init(),
        // deno_net::init::<Permissions>(
        //     options.root_cert_store.clone(),
        //     unstable,
        //     options.unsafely_ignore_certificate_errors.clone(),
        // ),
        ops::os::init(None),
        ops::permissions::init(),
        ops::process::init(),
        ops::signal::init(),
        ops::tty::init(),
        ops::http::init(),
        // Permissions ext (worker specific state)
        perm_ext,
    ];
    extensions.extend(std::mem::take(&mut options.extensions));

    let mut js_runtime = JsRuntime::new(RuntimeOptions {
        module_loader: None,
        startup_snapshot: Some(js::deno_isolate_init()),
        js_error_create_fn: options.js_error_create_fn.clone(),
        get_error_class_fn: options.get_error_class_fn,
        shared_array_buffer_store: options.shared_array_buffer_store.clone(),
        compiled_wasm_module_store: options.compiled_wasm_module_store.clone(),
        extensions,
        ..Default::default()
    });

    let script = format!("bootstrap.mainRuntime({})", options.bootstrap.as_json());
    js_runtime
        .execute_script(&located_script_name!(), &script)
        .unwrap();

    let worker_funcs_script = format!(
        r#"
    async function respondWith(response) {{
        const serialized = {{
            headers: Object.fromEntries(response.headers),
            ok: response.ok,
            redirected: response.redirected,
            status: response.status,
            statusText: response.statusText,
            trailer: response.trailer,
            type: response.type,
            body: new Uint8Array(await response.arrayBuffer())
        }}
        
        window.requestResult = serialized
    }}

    window.respondWith = respondWith
    window.cwd = "{}";
    "#,
        script_path.parent().unwrap().to_str().unwrap()
    );

    js_runtime
        .execute_script("worker_funcs", worker_funcs_script.as_str())
        .unwrap();

    let js_code = std::fs::read_to_string(script_path.as_path()).unwrap();
    js_runtime
        .execute_script(script_path.to_str().unwrap(), &js_code)
        .unwrap();

    js_runtime
}
