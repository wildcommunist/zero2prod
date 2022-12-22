use crate::authentication::{validate_credentials, AuthenticationError, Credentials};
use crate::routes::admin::dashboard::get_username;
use crate::session_state::TypedSession;
use crate::utils::{e500, see_other};
use actix_web::web::Form;
use actix_web::{web, HttpResponse};
use actix_web_flash_messages::FlashMessage;
use secrecy::{ExposeSecret, Secret};
use sqlx::PgPool;

//region Structs & Implementations
#[derive(serde::Deserialize)]
pub struct FormData {
    current_password: Secret<String>,
    new_password: Secret<String>,
    new_password_check: Secret<String>,
}
//endregion

//region HTTP handlers
pub async fn change_password(
    form: Form<FormData>,
    session: TypedSession,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = session.get_user_id().map_err(e500)?;
    if user_id.is_none() {
        return Ok(see_other("/login"));
    }
    let user_id = user_id.unwrap();

    // Check that the passwords match
    if form.new_password.expose_secret() != form.new_password_check.expose_secret() {
        FlashMessage::error("Your new passwords do not match").send();
        return Ok(see_other("/admin/password"));
    }

    let username = get_username(user_id, &pool).await.map_err(e500)?;

    let credentials = Credentials {
        username,
        password: form.0.current_password,
    };

    if let Err(e) = validate_credentials(credentials, &pool).await {
        return match e {
            AuthenticationError::InvalidCredentials(_) => {
                FlashMessage::error("You have entered incorrect password").send();
                Ok(see_other("/admin/password"))
            }
            AuthenticationError::UnexpectedError(_) => Err(e500(e)),
        };
    };
    todo!()
}
//endregion

//region Helper functions
//endregion
