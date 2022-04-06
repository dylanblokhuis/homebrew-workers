use axum::{
    body::BoxBody,
    extract::{Extension, FromRequest, RequestParts, TypedHeader},
    headers::{authorization::Bearer, Authorization},
    http::{Request, StatusCode},
    middleware::Next,
    response::IntoResponse,
    Json,
};
use entity::user;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use migration::sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::errors::ApiError;

pub static KEYS: Lazy<Keys> = Lazy::new(|| {
    let secret = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");
    Keys::new(secret.as_bytes())
});

// should be post
pub async fn authorize_route(
    Json(payload): Json<Payload>,
    Extension(ref conn): Extension<DatabaseConnection>,
) -> Result<Json<Body>, ApiError> {
    if payload.client_id.is_empty() || payload.client_secret.is_empty() {
        return Err(ApiError::new(400, "Missing credentials"));
    }

    let admin_client_id = std::env::var("ADMIN_CLIENT_ID").expect("ADMIN_CLIENT_ID must be set");
    let admin_client_secret =
        std::env::var("ADMIN_CLIENT_SECRET").expect("ADMIN_CLIENT_SECRET must be set");

    if payload.client_id == admin_client_id && payload.client_secret == admin_client_secret {
        let token = encode_token(&Claims {
            sub: 0,
            is_admin: true,
            exp: 2_000_000_000,
        })?;

        return Ok(Json(Body::new(token)));
    }

    let maybe_user = user::Entity::find()
        .filter(user::Column::ClientId.eq(payload.client_id.as_str()))
        .filter(user::Column::ClientSecret.eq(payload.client_secret.as_str()))
        .one(conn)
        .await
        .map_err(ApiError::db)?;

    if let Some(user) = maybe_user {
        let token = encode_token(&Claims {
            sub: user.id,
            is_admin: false,
            exp: 2_000_000_000,
        })?;

        return Ok(Json(Body::new(token)));
    }

    Err(ApiError::new(401, "Wrong credentials"))
}

fn encode_token(claims: &Claims) -> Result<String, ApiError> {
    let result = encode(&Header::default(), &claims, &KEYS.encoding)
        .map_err(|_| ApiError::new(500, "Failed to create token"))?;

    Ok(result)
}

pub async fn is_admin_middleware(
    request: Request<BoxBody>,
    next: Next<BoxBody>,
) -> impl IntoResponse {
    let mut parts = RequestParts::new(request);
    let bearer = TypedHeader::<Authorization<Bearer>>::from_request(&mut parts).await;
    if bearer.is_err() {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let request = parts.try_into_request().unwrap();

    let token_data = decode::<Claims>(
        bearer.unwrap().token(),
        &KEYS.decoding,
        &Validation::default(),
    );
    if token_data.is_err() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let data = token_data.unwrap();
    if !data.claims.is_admin {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(request).await)
}

impl Body {
    fn new(access_token: String) -> Self {
        Self {
            access_token,
            token_type: "Bearer".to_string(),
        }
    }
}
pub struct Keys {
    pub encoding: EncodingKey,
    pub decoding: DecodingKey,
}

impl Keys {
    fn new(secret: &[u8]) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: i32,
    pub is_admin: bool,
    exp: usize,
}

#[derive(Debug, Serialize)]
pub struct Body {
    access_token: String,
    token_type: String,
}

#[derive(Debug, Deserialize)]
pub struct Payload {
    client_id: String,
    client_secret: String,
}
