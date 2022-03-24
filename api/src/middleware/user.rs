use axum::{
    async_trait,
    extract::{FromRequest, Path, RequestParts, TypedHeader},
    headers::{authorization::Bearer, Authorization},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use entity::user;
use jsonwebtoken::{decode, Validation};
use migration::sea_orm::{DatabaseConnection, EntityTrait};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::auth::Claims;

#[derive(Serialize, Deserialize)]
pub struct User(user::Model);

#[async_trait]
impl<B> FromRequest<B> for User
where
    B: Send,
{
    type Rejection = UserError;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        // extract the id from the path
        let id = {
            let path: Path<i32> = axum::extract::Path::from_request(req).await.unwrap();
            path.0
        };

        // now we deserialize the token and check if the user has perms
        let bearer = TypedHeader::<Authorization<Bearer>>::from_request(req).await;
        if bearer.is_err() {
            return Err(UserError::Unauthorized);
        }
        let token_data = decode::<Claims>(
            bearer.unwrap().token(),
            &crate::middleware::auth::KEYS.decoding,
            &Validation::default(),
        );
        if token_data.is_err() {
            return Err(UserError::Unauthorized);
        }

        let claims = token_data.unwrap().claims;
        if id != claims.sub && !claims.is_admin {
            return Err(UserError::Unauthorized);
        }

        let conn = req
            .extensions()
            .unwrap()
            .get::<DatabaseConnection>()
            .unwrap();
        let user = user::Entity::find_by_id(id).one(conn).await.unwrap();

        if user.is_none() {
            return Err(UserError::NotFound);
        }

        Ok(User(user.unwrap()))
    }
}

impl IntoResponse for UserError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            UserError::NotFound => (StatusCode::NOT_FOUND, "This user doesn't exist"),
            UserError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "Not authorized to access this user",
            ),
        };
        let body = Json(json!({
            "error": error_message,
        }));
        (status, body).into_response()
    }
}

#[derive(Debug)]
pub enum UserError {
    NotFound,
    Unauthorized,
}
