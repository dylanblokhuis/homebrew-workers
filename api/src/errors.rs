use axum::{
    body,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use migration::DbErr;

pub struct ApiError {
    status_code: StatusCode,
    message: Option<&'static str>,
}

impl ApiError {
    pub fn new(status_code: u16, message: &'static str) -> Self {
        Self {
            status_code: StatusCode::from_u16(status_code)
                .expect("Status Code used that doesn't exist"),
            message: Some(message),
        }
    }

    pub fn empty(status_code: u16) -> Self {
        Self {
            status_code: StatusCode::from_u16(status_code)
                .expect("Status Code used that doesn't exist"),
            message: None,
        }
    }

    pub fn db(err: DbErr) -> Self {
        println!("{:?}", err);

        Self {
            status_code: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Database error".into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if self.message.is_some() {
            Response::builder()
                .status(self.status_code)
                .body(body::boxed(body::Full::from(self.message.unwrap())))
                .unwrap()
        } else {
            Response::builder()
                .status(self.status_code)
                .body(body::boxed(body::Empty::new()))
                .unwrap()
        }
    }
}
