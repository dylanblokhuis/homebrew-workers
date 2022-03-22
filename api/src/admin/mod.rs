use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{body, extract::Extension, routing::get, Json, Router};
use entity::user;
use migration::sea_orm::ActiveValue::Set;
use migration::sea_orm::{DatabaseConnection, EntityTrait, ModelTrait};
use rand::distributions::Alphanumeric;
use rand::Rng;
use serde::Deserialize;
use tower::ServiceBuilder;
use tower_http::ServiceBuilderExt;

use crate::middleware::auth::is_admin_middleware;

pub fn router() -> Router {
    Router::new()
        .route("/users", get(get_users).post(create_user))
        .route("/users/:id", get(get_user_by_id).delete(delete_user_by_id))
        .layer(
            ServiceBuilder::new()
                .map_request_body(body::boxed)
                .layer(axum::middleware::from_fn(is_admin_middleware)),
        )
}

#[axum_macros::debug_handler]
async fn get_users(
    Extension(ref conn): Extension<DatabaseConnection>,
) -> Result<Json<Vec<user::Model>>, AdminError> {
    let items = user::Entity::find()
        .all(conn)
        .await
        .map_err(|_| AdminError::SomethingWentWrong)?;

    Ok(Json(items))
}

#[derive(Debug, Deserialize)]
struct CreateUser {
    name: String,
}

#[axum_macros::debug_handler]
async fn create_user(
    Json(params): Json<CreateUser>,
    Extension(ref conn): Extension<DatabaseConnection>,
) -> Result<Json<String>, AdminError> {
    let client_id: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();

    let client_secret: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();

    let to_be_inserted = user::ActiveModel {
        name: Set(params.name),
        client_id: Set(client_id),
        client_secret: Set(client_secret),
        created_at: Set(chrono::DateTime::into(chrono::Utc::now())),
        ..Default::default()
    };

    let insert_res = user::Entity::insert(to_be_inserted)
        .exec(conn)
        .await
        .map_err(|_| AdminError::SomethingWentWrong)?;

    Ok(Json(format!("Created user: {}", insert_res.last_insert_id)))
}

#[axum_macros::debug_handler]
async fn get_user_by_id(
    Path(user_id): Path<i32>,
    Extension(ref conn): Extension<DatabaseConnection>,
) -> impl IntoResponse {
    let maybe_user = user::Entity::find_by_id(user_id)
        .one(conn)
        .await
        .map_err(|_| AdminError::SomethingWentWrong)?;

    if maybe_user.is_none() {
        return Err(AdminError::IdNotFound);
    }

    Ok(Json(maybe_user.unwrap()))
}

#[axum_macros::debug_handler]
async fn delete_user_by_id(
    Path(user_id): Path<i32>,
    Extension(ref conn): Extension<DatabaseConnection>,
) -> impl IntoResponse {
    let maybe_user = user::Entity::find_by_id(user_id)
        .one(conn)
        .await
        .map_err(|_| AdminError::SomethingWentWrong)?;

    if let Some(user) = maybe_user {
        user.delete(conn)
            .await
            .map_err(|_| AdminError::SomethingWentWrong)?;

        Ok(Json("Deleted user succesfully"))
    } else {
        Err(AdminError::IdNotFound)
    }
}

enum AdminError {
    SomethingWentWrong,
    IdNotFound,
}

impl IntoResponse for AdminError {
    fn into_response(self) -> Response {
        let (body, status_code) = match self {
            AdminError::SomethingWentWrong => (
                body::boxed(body::Full::from("Something went wrong")),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            AdminError::IdNotFound => (
                body::boxed(body::Full::from("This record does not exist")),
                StatusCode::NOT_FOUND,
            ),
        };

        Response::builder().status(status_code).body(body).unwrap()
    }
}
