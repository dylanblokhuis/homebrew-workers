use axum::{body::Body, http::Request, response::Response};
use deno_runtime::permissions::{Permissions, PermissionsOptions};
use session::Session;
use std::{
    path::PathBuf,
    sync::Arc,
    thread::{self},
};
use tokio::sync::{mpsc, oneshot, RwLock};

use crate::runtime::Runtime;

pub type RuntimeChannelPayload = (Request<Body>, oneshot::Sender<Response<Body>>);

#[derive(Debug, Clone)]
pub struct App {
    pub session: Session,
    pub name: String,
    pub path: PathBuf,
    pub script_file_name: String,
    pub deployment: String,
    runtime: Arc<RwLock<Option<mpsc::Sender<RuntimeChannelPayload>>>>,
}

impl App {
    #[must_use]
    pub fn new(
        session: Session,
        name: String,
        path: PathBuf,
        script_file_name: String,
        deployment: String,
    ) -> Self {
        Self {
            session,
            name,
            path,
            script_file_name,
            deployment,
            runtime: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn get_runtime(&self) -> mpsc::Sender<RuntimeChannelPayload> {
        if let Some(runtime) = self.runtime.read().await.clone() {
            return runtime;
        }

        self.new_worker().await
    }

    async fn new_worker(&self) -> mpsc::Sender<RuntimeChannelPayload> {
        println!("New worker spawned from {:?}", self.path);
        let permission_options = PermissionsOptions {
            allow_env: None,
            allow_ffi: None,
            allow_hrtime: false,
            allow_run: None,
            allow_write: None,
            prompt: false,
            allow_net: None,
            allow_read: Some(vec![self.path.clone()]),
        };
        let permissions = Permissions::from_options(&permission_options);
        let (tx, mut rx) = mpsc::channel::<RuntimeChannelPayload>(10);

        let mut script_path = self.path.clone();
        script_path.push(self.script_file_name.clone());

        let session = self.session.clone();

        thread::spawn(move || {
            tokio::runtime::Builder::new_multi_thread()
                .thread_name("runtime-pool")
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let mut runtime = Runtime::new(session, script_path.as_path(), permissions);
                    runtime.handle_request(&mut rx).await;
                });
        });

        {
            let tx2 = tx.clone();
            *self.runtime.write().await = Some(tx2);
        }

        let runtime2 = self.runtime.clone();
        let tx2 = tx.clone();
        tokio::spawn(async move {
            tx2.closed().await;
            let mut item = runtime2.write().await;
            *item = None;
        });

        tx
    }
}
