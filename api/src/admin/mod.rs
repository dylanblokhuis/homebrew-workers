use axum::{body, response::Html, routing::get, Router};
use tower::ServiceBuilder;
use tower_http::ServiceBuilderExt;

use crate::middleware::auth::is_admin_middleware;

pub fn router() -> Router {
    Router::new().route("/", get(admin_hello)).layer(
        ServiceBuilder::new()
            .map_request_body(body::boxed)
            .layer(axum::middleware::from_fn(is_admin_middleware)),
    )
}

async fn admin_hello() -> Html<String> {
    // Send the protected data to the user
    Html("Hello".into())
}
