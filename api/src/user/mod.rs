use axum::{
    extract::{Extension, Multipart},
    routing::{get, post},
    Json, Router,
};
use entity::user;
use migration::sea_orm::{DatabaseConnection, EntityTrait, Set};
use s3::Bucket;
use sha256::digest_bytes;

use crate::{errors::ApiError, middleware::user::User};

pub fn router() -> Router {
    Router::new()
        .route("/", get(me))
        .route("/deploy", post(deploy))
}

#[axum_macros::debug_handler]
async fn me(user: User) -> Json<User> {
    Json(user)
}

#[axum_macros::debug_handler]
async fn deploy(
    user: User,
    mut multipart: Multipart,
    Extension(bucket): Extension<Bucket>,
    Extension(ref conn): Extension<DatabaseConnection>,
) -> Result<Json<String>, ApiError> {
    let field = multipart
        .next_field()
        .await
        .map_err(|_| ApiError::new(400, "Invalid multipart form"))?
        .ok_or_else(|| ApiError::new(400, "Empty multipart form"))?;

    let content_type = field.content_type().ok_or_else(|| {
        ApiError::new(
            400,
            "No content type found on file, so it's invalid by default.",
        )
    })?;
    if *content_type != "application/zip" {
        return Err(ApiError::new(
            400,
            "Delivered content-type is not application/zip",
        ));
    }

    let bytes = field.bytes().await.unwrap();
    let hash = digest_bytes(&bytes.to_vec());

    let file_name = format!("/{}/{}.zip", user.0.id, hash);
    let (_, code) = bucket
        .put_object_with_content_type(&file_name, &bytes.to_vec(), "application/zip")
        .await
        .map_err(|_| ApiError::new(500, "Failed to send request to S3 storage"))?;

    if code != 200 {
        return Err(ApiError::new(500, "Failed to put object into S3 storage."));
    }

    let model = user::ActiveModel {
        id: Set(user.0.id),
        latest_deployment: Set(Some(file_name.clone())),
        ..Default::default()
    };

    user::Entity::update(model)
        .exec(conn)
        .await
        .map_err(ApiError::db)?;

    Ok(Json(file_name))
}
