use crate::authentication::UserId;
use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use crate::idempotency::{save_response, try_processing, IdempotencyKey, NextAction};
use crate::routes::error_chain_fmt;
use crate::utils::{e400, e500, see_other};
use actix_web::http::{header, StatusCode};
use actix_web::web::ReqData;
use actix_web::{HttpResponse, ResponseError};
use actix_web_flash_messages::FlashMessage;
use anyhow::Context;
use reqwest::header::HeaderValue;
use sqlx::PgPool;
use std::fmt::Formatter;

#[derive(serde::Deserialize)]
pub struct BodyData {
    title: String,
    html: String,
    plain: String,
    idempotency_key: String,
}

struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

//region PublishError & Implementations
#[derive(thiserror::Error)]
pub enum PublishError {
    #[error("Authentication error")]
    AuthError(#[source] anyhow::Error),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for PublishError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for PublishError {
    fn error_response(&self) -> HttpResponse {
        match self {
            PublishError::UnexpectedError(_) => {
                HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
            }
            PublishError::AuthError(_) => {
                let mut response = HttpResponse::new(StatusCode::UNAUTHORIZED);
                let header_value = HeaderValue::from_str(r#"Basic realm="publish""#).unwrap();
                response
                    .headers_mut()
                    .insert(header::WWW_AUTHENTICATE, header_value);
                response
            }
        }
    }
}
//endregion

//region HTTP handlers
#[tracing::instrument(
name="Publish a newsletter issue",
skip(form_data,pool,email_client)
fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
)]
pub async fn publish_newsletter(
    form_data: actix_web::web::Form<BodyData>,
    pool: actix_web::web::Data<PgPool>,
    email_client: actix_web::web::Data<EmailClient>,
    user_id: ReqData<UserId>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = user_id.into_inner();

    let BodyData {
        title,
        html,
        plain,
        idempotency_key,
    } = form_data.0;
    let idempotency_key: IdempotencyKey = idempotency_key.try_into().map_err(e400)?;

    match try_processing(&pool, &idempotency_key, *user_id)
        .await
        .map_err(e500)?
    {
        NextAction::StartProcessing => {}
        NextAction::ReturnSavedResponse(saved_response) => {
            success_message().send();
            return Ok(saved_response);
        }
    }

    tracing::Span::current().record("user_id", &tracing::field::display(*user_id));
    let subscribers = get_confirmed_subscribers(&pool).await.map_err(e500)?;

    for subscriber in &subscribers {
        match subscriber {
            Ok(sub) => {
                email_client
                    .send_email(&sub.email, &title, &html, &plain)
                    .await
                    .with_context(|| format!("Failed to send newsletter issue to {}", sub.email))
                    .map_err(e500)?;
            }
            Err(error) => {
                tracing::warn!(
                    error.cause_chain = ?error,
                    "Skipping a confirmed subscriber. Their stored contact details are invalid"
                );
            }
        }
    }

    success_message().send();

    let response = see_other("/admin/newsletter");
    let response = save_response(&pool, &idempotency_key, *user_id, response)
        .await
        .map_err(e500)?;

    Ok(response)
}

//endregion

//region Helper functions

fn success_message() -> FlashMessage {
    FlashMessage::info("The newsletter issue has been published!")
}

#[tracing::instrument(name = "Get confirmed subscribers", skip(pool))]
async fn get_confirmed_subscribers(
    pool: &PgPool,
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
    let confirmed_subscribers = sqlx::query!(
        r#"
        SELECT email
        FROM subscriptions
        WHERE status = 'confirmed'"#
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| match SubscriberEmail::parse(r.email) {
        Ok(email) => Ok(ConfirmedSubscriber { email }),
        Err(error) => Err(anyhow::anyhow!(error)),
    })
    .collect();

    Ok(confirmed_subscribers)
}
//endregion