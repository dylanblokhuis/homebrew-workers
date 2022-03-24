use axum::{routing::get, Json, Router};

use crate::middleware::user::User;

pub fn router() -> Router {
    Router::new().route("/", get(me))
}

#[axum_macros::debug_handler]
async fn me(user: User) -> Json<User> {
    Json(user)
}
