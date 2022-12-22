use crate::authentication::{validate_credentials, AuthenticationError, Credentials};
use crate::routes::error_chain_fmt;
use actix_web::http::header::LOCATION;
use actix_web::http::StatusCode;
use actix_web::web::{Data, Form};
use actix_web::{HttpResponse, ResponseError};
use secrecy::Secret;
use serde::Deserialize;
use sqlx::PgPool;
use std::fmt::Formatter;

//region Structs & implementations
#[derive(Deserialize)]
pub struct FormData {
    username: String,
    password: Secret<String>,
}
//endregion

//region Enums
#[derive(thiserror::Error)]
pub enum LoginError {
    #[error("Authentication Failed")]
    AuthError(#[source] anyhow::Error),
    #[error("Something went wrong.")]
    UnexpectedError(#[source] anyhow::Error),
}

impl std::fmt::Debug for LoginError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(&self, f)
    }
}

impl ResponseError for LoginError {
    fn status_code(&self) -> StatusCode {
        match self {
            LoginError::AuthError(_) => StatusCode::UNAUTHORIZED,
            LoginError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

//endregion

//region HTTP handlers
#[tracing::instrument(
    name="Login user",
    skip(form, pool),
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
)]
pub async fn login(form: Form<FormData>, pool: Data<PgPool>) -> Result<HttpResponse, LoginError> {
    let credentials = Credentials {
        username: form.0.username,
        password: form.0.password,
    };
    tracing::Span::current().record("username", &tracing::field::display(&credentials.username));

    let user_id = validate_credentials(credentials, &pool)
        .await
        .map_err(|e| match e {
            AuthenticationError::InvalidCredentials(_) => LoginError::AuthError(e.into()),
            AuthenticationError::UnexpectedError(_) => LoginError::UnexpectedError(e.into()),
        })?;
    tracing::Span::current().record("user_id", &tracing::field::display(user_id));
    Ok(HttpResponse::SeeOther()
        .insert_header((LOCATION, "/"))
        .finish())
}
//endregion
