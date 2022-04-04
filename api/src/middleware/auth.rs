use axum::{
    body::BoxBody,
    extract::{Extension, FromRequest, RequestParts, TypedHeader},
    headers::{authorization::Bearer, Authorization},
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use entity::user;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use migration::sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::json;

pub static KEYS: Lazy<Keys> = Lazy::new(|| {
    let secret = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");
    Keys::new(secret.as_bytes())
});

// should be post
pub async fn authorize_route(
    Json(payload): Json<AuthPayload>,
    Extension(ref conn): Extension<DatabaseConnection>,
) -> Result<Json<AuthBody>, AuthError> {
    if payload.client_id.is_empty() || payload.client_secret.is_empty() {
        return Err(AuthError::MissingCredentials);
    }

    let admin_client_id = std::env::var("ADMIN_CLIENT_ID").expect("ADMIN_CLIENT_ID must be set");
    let admin_client_secret =
        std::env::var("ADMIN_CLIENT_SECRET").expect("ADMIN_CLIENT_SECRET must be set");

    let mut claims: Option<Claims> = None;

    if payload.client_id == admin_client_id || payload.client_secret == admin_client_secret {
        claims = Some(Claims {
            sub: 0,
            is_admin: true,
            exp: 2000000000,
        });
    }

    let maybe_user = user::Entity::find()
        .filter(user::Column::ClientId.eq(payload.client_id.as_str()))
        .filter(user::Column::ClientSecret.eq(payload.client_secret.as_str()))
        .one(conn)
        .await
        .unwrap();

    if let Some(user) = maybe_user {
        claims = Some(Claims {
            sub: user.id,
            is_admin: false,
            exp: 2000000000,
        });
    }

    if claims.is_none() {
        return Err(AuthError::WrongCredentials);
    }

    // Create the authorization token
    let token = encode(&Header::default(), &claims.unwrap(), &KEYS.encoding)
        .map_err(|_| AuthError::TokenCreation)?;

    // Send the authorized token
    Ok(Json(AuthBody::new(token)))
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

impl AuthBody {
    fn new(access_token: String) -> Self {
        Self {
            access_token,
            token_type: "Bearer".to_string(),
        }
    }
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AuthError::WrongCredentials => (StatusCode::UNAUTHORIZED, "Wrong credentials"),
            AuthError::MissingCredentials => (StatusCode::BAD_REQUEST, "Missing credentials"),
            AuthError::TokenCreation => (StatusCode::INTERNAL_SERVER_ERROR, "Token creation error"),
        };
        let body = Json(json!({
            "error": error_message,
        }));
        (status, body).into_response()
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
pub struct AuthBody {
    access_token: String,
    token_type: String,
}

#[derive(Debug, Deserialize)]
pub struct AuthPayload {
    client_id: String,
    client_secret: String,
}

#[derive(Debug)]
pub enum AuthError {
    WrongCredentials,
    MissingCredentials,
    TokenCreation,
}
