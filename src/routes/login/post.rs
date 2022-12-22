use crate::authentication::{validate_credentials, AuthenticationError, Credentials};
use crate::routes::error_chain_fmt;
use crate::session_state::TypedSession;
use actix_web::error::InternalError;
use actix_web::http::header::LOCATION;
use actix_web::web::{Data, Form};
use actix_web::HttpResponse;
use actix_web_flash_messages::FlashMessage;
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

//endregion

//region HTTP handlers
#[tracing::instrument(
    name="Login user",
    skip(form, pool, session),
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
)]
pub async fn login(
    form: Form<FormData>,
    pool: Data<PgPool>,
    session: TypedSession,
) -> Result<HttpResponse, InternalError<LoginError>> {
    let credentials = Credentials {
        username: form.0.username,
        password: form.0.password,
    };
    tracing::Span::current().record("username", &tracing::field::display(&credentials.username));

    match validate_credentials(credentials, &pool).await {
        Ok(user_id) => {
            session.renew(); //rotate session keys
            session
                .insert_user_id(user_id)
                .map_err(|e| login_redirect(LoginError::UnexpectedError(e.into())))?;
            tracing::Span::current().record("user_id", &tracing::field::display(&user_id));
            Ok(HttpResponse::SeeOther()
                .insert_header((LOCATION, "/admin/dashboard"))
                .finish())
        }
        Err(e) => {
            let e = match e {
                AuthenticationError::InvalidCredentials(_) => LoginError::AuthError(e.into()),
                AuthenticationError::UnexpectedError(_) => LoginError::UnexpectedError(e.into()),
            };
            Err(login_redirect(e))
        }
    }
}
//endregion

//region Helper functions
fn login_redirect(e: LoginError) -> InternalError<LoginError> {
    FlashMessage::error(e.to_string()).send();
    let response = HttpResponse::SeeOther()
        .insert_header((LOCATION, "/login"))
        .finish();
    InternalError::from_response(e, response)
}
//endregion
