use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use crate::routes::error_chain_fmt;
use actix_web::http::header::HeaderMap;
use actix_web::http::{header, StatusCode};
use actix_web::{HttpRequest, HttpResponse, ResponseError};
use anyhow::{anyhow, Context};
use reqwest::header::HeaderValue;
use secrecy::{ExposeSecret, Secret};
use sqlx::PgPool;
use std::fmt::Formatter;

#[derive(serde::Deserialize)]
pub struct BodyData {
    title: String,
    content: Content,
}

#[derive(serde::Deserialize)]
pub struct Content {
    html: String,
    text: String,
}

struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

struct Credentials {
    username: String,
    password: Secret<String>,
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
    skip(body,pool,email_client,request)
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
)]
pub async fn publish_newsletter(
    body: actix_web::web::Json<BodyData>,
    pool: actix_web::web::Data<PgPool>,
    email_client: actix_web::web::Data<EmailClient>,
    request: HttpRequest,
) -> Result<HttpResponse, PublishError> {
    let credentials = basic_authentication(request.headers()).map_err(PublishError::AuthError)?;
    tracing::Span::current().record("username", &tracing::field::display(&credentials.username));
    let user_id = validate_credentials(credentials, &pool).await?;
    tracing::Span::current().record("user_id", &tracing::field::display(&user_id));
    let subscribers = get_confirmed_subscribers(&pool).await?;

    for subscriber in subscribers {
        match subscriber {
            Ok(sub) => {
                email_client
                    .send_email(
                        &sub.email,
                        &body.title,
                        &body.content.html,
                        &body.content.text,
                    )
                    .await
                    .with_context(|| format!("Failed to send newsletter issue to {}", sub.email))?;
            }
            Err(error) => {
                tracing::warn!(
                    error.cause_chain = ?error,
                    "Skipping a confirmed subscriber. Their stored contact details are invalid"
                );
            }
        }
    }
    Ok(HttpResponse::Ok().finish())
}

//endregion

//region Helper functions
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

fn basic_authentication(headers: &HeaderMap) -> Result<Credentials, anyhow::Error> {
    let authorization_header = headers
        .get("Authorization")
        .context("The 'Authorization' header is missing")?
        .to_str()
        .context("The 'Authorization' header was not a valid UTF8 string")?;
    let base64_header_segment = authorization_header
        .strip_prefix("Basic ")
        .context("Malformed 'Authorization' header")?;
    let decoded_bytes = base64::decode_config(base64_header_segment, base64::STANDARD)
        .context("Failed to decode base64 values from 'Authorization' header")?;
    let decoded_credentials = String::from_utf8(decoded_bytes)
        .context("Failed to convert decoded bytes to credentials")?;
    let mut credentials = decoded_credentials.splitn(2, ':');
    let username = credentials
        .next()
        .ok_or_else(|| anyhow!("A username must be provided in basic auth"))?
        .to_string();
    let password = credentials
        .next()
        .ok_or_else(|| anyhow!("A password must be provided in basic auth"))?
        .to_string();
    Ok(Credentials {
        username,
        password: Secret::new(password),
    })
}

async fn validate_credentials(
    credentials: Credentials,
    pool: &PgPool,
) -> Result<uuid::Uuid, PublishError> {
    let user_id: Option<_> = sqlx::query!(
        r#"
        SELECT user_id
        FROM users
        WHERE username = $1 AND password = $2
        "#,
        credentials.username,
        credentials.password.expose_secret()
    )
    .fetch_optional(pool)
    .await
    .context("failed to perform a credentials validation query")
    .map_err(PublishError::UnexpectedError)?;
    user_id
        .map(|row| row.user_id)
        .ok_or_else(|| anyhow!("Invalid username or password."))
        .map_err(PublishError::AuthError)
}

//endregion
