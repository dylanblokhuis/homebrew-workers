use axum::body::Body;
use axum::http::Request;
use deno_core::error::AnyError;
use deno_core::futures::future::poll_fn;
use deno_core::located_script_name;
use deno_core::CompiledWasmModuleStore;
use deno_core::Extension;
use deno_core::GetErrorClassFn;
use deno_core::JsErrorCreateFn;
use deno_core::JsRuntime;
use deno_core::RuntimeOptions;
use deno_core::SharedArrayBufferStore;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::inspector_server::InspectorServer;
use deno_runtime::js;
use deno_runtime::ops;
use deno_runtime::permissions::Permissions;
use deno_runtime::permissions::PermissionsOptions;
use deno_runtime::BootstrapOptions;
use deno_tls::rustls::RootCertStore;
use serde::Deserialize;
use serde::Serialize;
use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

pub struct Runtime {
    js_runtime: JsRuntime,
}

pub struct RunOptions {
    pub bootstrap: BootstrapOptions,
    pub extensions: Vec<Extension>,
    pub unsafely_ignore_certificate_errors: Option<Vec<String>>,
    pub root_cert_store: Option<RootCertStore>,
    pub user_agent: String,
    pub seed: Option<u64>,
    // Callbacks invoked when creating new instance of WebWorker
    pub js_error_create_fn: Option<Rc<JsErrorCreateFn>>,
    pub maybe_inspector_server: Option<Arc<InspectorServer>>,
    pub should_break_on_first_statement: bool,
    pub get_error_class_fn: Option<GetErrorClassFn>,
    // pub origin_storage_dir: Option<std::path::PathBuf>,
    pub blob_store: BlobStore,
    pub shared_array_buffer_store: Option<SharedArrayBufferStore>,
    pub compiled_wasm_module_store: Option<CompiledWasmModuleStore>,
}

impl Runtime {
    pub fn new() -> Self {
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
            get_error_class_fn: Some(&get_error_class_name),
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

        Self {
            js_runtime: init(permissions, options),
        }
    }

    pub async fn run(&mut self, request: Request<Body>) -> JsResponse {
        let path = Path::new("worker.js");
        let js_code = std::fs::read_to_string(path).unwrap();

        let runtime = self.borrow_mut();
        let take = runtime.js_runtime.borrow_mut();
        take.execute_script("user", &js_code).unwrap();
        let cb_value = runtime.call_on_request(request);
        take.run_event_loop(false).await.unwrap();

        let scope = &mut take.handle_scope();
        let resolver = v8::PromiseResolver::new(scope).unwrap();
        resolver.resolve(scope, cb_value);
        let promise = resolver.get_promise(scope);

        let response: JsResponse = deno_core::serde_v8::from_v8(scope, promise.result(scope))
            .expect("Could not serialize Response object");
        println!("{:?}", response);
        // let mut scope = self.js_runtime.handle_scope();
        // println!(
        //     "{:?}",
        //     value.open(&mut scope).to_rust_string_lossy(&mut scope)
        // );

        response
    }

    fn call_on_request(&mut self, request: Request<Body>) -> v8::Local<'_, v8::Value> {
        let scope = &mut self.js_runtime.handle_scope();
        let context = v8::Context::new(scope);
        let request_obj = v8::Object::new(scope);

        let url_key = v8::String::new(scope, "url").unwrap();
        let url_value = v8::String::new(scope, &request.uri().to_string()).unwrap();
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

        let args = &[request_obj.into()];

        let context = scope.get_current_context();
        let name = v8::String::new(scope, "onRequest").unwrap();
        let global = context.global(scope);
        let func = global.get(scope, name.into()).unwrap();
        let cb = v8::Local::<v8::Function>::try_from(func).unwrap();
        let cb_value = cb.call(scope, global.into(), args).unwrap();

        return cb_value;

        // println!("{}", cb_value.to_rust_string_lossy(scope));
        // if cb_value.is_promise() {
        //     let promise = v8::Promise::try_from(cb_value).unwrap();
        //     promise.result(scope);
        //     let promise = resolver.get_promise(scope);
        //     resolver.resolve(scope, promise.into());

        //     cb_value = promise.result(scope);
        // }

        // poll_fn(|cx| self.js_runtime.poll_event_loop(cx, false)).await;

        // response
    }

    // pub fn poll_value(
    //     &mut self,
    //     global: &v8::Global<v8::Value>,
    //     cx: &mut Context,
    //   ) -> Poll<Result<v8::Global<v8::Value>, Error>> {

    //     let mut scope = self.handle_scope();
    //     let local = v8::Local::<v8::Value>::new(&mut scope, global);

    //     if let Ok(promise) = v8::Local::<v8::Promise>::try_from(local) {
    //       match promise.state() {
    //         v8::PromiseState::Pending => match state {
    //           Poll::Ready(Ok(_)) => {
    //             let msg = "Promise resolution is still pending but the event loop has already resolved.";
    //             Poll::Ready(Err(generic_error(msg)))
    //           }
    //           Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
    //           Poll::Pending => Poll::Pending,
    //         },
    //         v8::PromiseState::Fulfilled => {
    //           let value = promise.result(&mut scope);
    //           let value_handle = v8::Global::new(&mut scope, value);
    //           Poll::Ready(Ok(value_handle))
    //         }
    //         v8::PromiseState::Rejected => {
    //           let exception = promise.result(&mut scope);
    //           Poll::Ready(exception_to_err_result(&mut scope, exception, false))
    //         }
    //       }
    //     } else {
    //       let value_handle = v8::Global::new(&mut scope, local);
    //       Poll::Ready(Ok(value_handle))
    //     }
    //   }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JsResponse {
    pub headers: HashMap<String, String>,
    pub ok: bool,
    pub redirected: bool,
    pub status: u16,
    #[serde(rename = "statusText")]
    pub status_text: String,
    pub body: String,
}

pub fn get_error_class_name(e: &AnyError) -> &'static str {
    deno_runtime::errors::get_error_class_name(e).unwrap_or("Error")
}

pub fn init(permissions: Permissions, mut options: RunOptions) -> deno_core::JsRuntime {
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
            root_cert_store: options.root_cert_store.clone(),
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
        // deno_crypto::init(options.seed),
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
        // deno_tls::init(),
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

    js_runtime
}
