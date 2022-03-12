use axum::body::Body;
use axum::http::header::HeaderName;
use axum::http::{HeaderValue, Request, Response};
use axum::{routing::get, Router};
use runtime::{run, JsResponse};
use std::net::SocketAddr;
use std::str::FromStr;
use std::thread;

mod runtime;

#[tokio::main]
async fn main() {
    let app = Router::new().route("/*key", get(handler));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[axum_macros::debug_handler]
async fn handler(req: Request<Body>) -> Response<Body> {
    let js_response = thread::spawn(move || handle_request_in_v8(req))
        .join()
        .expect("Thread panicked");

    let mut response = Response::new(Body::try_from(js_response.body).unwrap());
    let headers = response.headers_mut();
    for (key, value) in js_response.headers {
        headers.insert(
            HeaderName::from_str(key.as_str()).unwrap(),
            HeaderValue::from_str(value.as_str()).unwrap(),
        );
    }

    response
}

#[tokio::main(flavor = "current_thread")]
async fn handle_request_in_v8(req: Request<Body>) -> JsResponse {
    run(req).await
}
