use axum::{
    body,
    http::StatusCode,
    response::{IntoResponse, Response},
};

pub enum ApiError {
    SomethingWentWrong,
    IdNotFound,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (body, status_code) = match self {
            ApiError::SomethingWentWrong => (
                body::boxed(body::Full::from("Something went wrong")),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            ApiError::IdNotFound => (
                body::boxed(body::Full::from("This record does not exist")),
                StatusCode::NOT_FOUND,
            ),
        };

        Response::builder().status(status_code).body(body).unwrap()
    }
}
