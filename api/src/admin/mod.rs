use axum::extract::Path;
use axum::response::IntoResponse;
use axum::{body, extract::Extension, routing::get, Json, Router};
use entity::user;
use migration::sea_orm::ActiveValue::Set;
use migration::sea_orm::{DatabaseConnection, EntityTrait, ModelTrait};
use rand::distributions::Alphanumeric;
use rand::Rng;
use serde::Deserialize;
use tower::ServiceBuilder;
use tower_http::ServiceBuilderExt;

use crate::errors::ApiError;
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
) -> Result<Json<Vec<user::Model>>, ApiError> {
    let items = user::Entity::find().all(conn).await.map_err(ApiError::db)?;

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
) -> Result<Json<String>, ApiError> {
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
        .map_err(|_| ApiError::empty(500))?;

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
        .map_err(ApiError::db)?;

    if maybe_user.is_none() {
        return Err(ApiError::new(404, "No user found with this id"));
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
        .map_err(ApiError::db)?;

    if let Some(user) = maybe_user {
        user.delete(conn).await.map_err(ApiError::db)?;

        Ok(Json("Deleted user succesfully"))
    } else {
        Err(ApiError::new(404, "No user found with this id"))
    }
}
