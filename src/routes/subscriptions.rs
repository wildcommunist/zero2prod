use crate::domain::{NewSubscriber, SubscriberEmail, SubscriberName};
use crate::email_client::EmailClient;
use crate::startup::ApplicationBaseUrl;
use actix_web::http::StatusCode;
use actix_web::{web, HttpResponse, ResponseError};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use sqlx::types::chrono::Utc;
use sqlx::{PgPool, Postgres, Transaction};
use std::error::Error;
use std::fmt::Formatter;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    pub email: String,
    pub name: String,
}

impl TryFrom<FormData> for NewSubscriber {
    type Error = String;
    fn try_from(form: FormData) -> Result<Self, Self::Error> {
        let name = SubscriberName::parse(form.name)?;
        let email = SubscriberEmail::parse(form.email)?;
        Ok(NewSubscriber { email, name })
    }
}

pub enum SubscribeError {
    ValidationError(String),
    StoreTokenError(StoreTokenError),
    SendEmailError(reqwest::Error),
    PoolError(sqlx::Error),
    InsertSubscriberError(sqlx::Error),
    TransactionCommitError(sqlx::Error),
}

// Implementations for different underlying error variants
impl From<String> for SubscribeError {
    fn from(e: String) -> Self {
        Self::ValidationError(e)
    }
}

impl From<reqwest::Error> for SubscribeError {
    fn from(e: reqwest::Error) -> Self {
        Self::SendEmailError(e)
    }
}

impl From<StoreTokenError> for SubscribeError {
    fn from(e: StoreTokenError) -> Self {
        Self::StoreTokenError(e)
    }
}

impl std::fmt::Display for SubscribeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SubscribeError::ValidationError(e) => write!(f, "{}", e),
            SubscribeError::StoreTokenError(_) => write!(
                f,
                "Failed to store the confirmation token for a new subscriber"
            ),
            SubscribeError::SendEmailError(_) => write!(f, "Failed to send confirmation email"),
            SubscribeError::PoolError(_) => {
                write!(f, "Failed to acquire postgres connection from the pool")
            }
            SubscribeError::InsertSubscriberError(_) => {
                write!(f, "Failed to insert new subscriber into the database")
            }
            SubscribeError::TransactionCommitError(_) => {
                write!(f, "Failed to commit database transaction")
            }
        }
    }
}

impl std::fmt::Debug for SubscribeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl std::error::Error for SubscribeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SubscribeError::ValidationError(_) => None,
            SubscribeError::StoreTokenError(e) => Some(e),
            SubscribeError::SendEmailError(e) => Some(e),
            SubscribeError::PoolError(e) => Some(e),
            SubscribeError::InsertSubscriberError(e) => Some(e),
            SubscribeError::TransactionCommitError(e) => Some(e),
        }
    }
}

impl ResponseError for SubscribeError {
    fn status_code(&self) -> StatusCode {
        match self {
            SubscribeError::ValidationError(_) => StatusCode::BAD_REQUEST,
            SubscribeError::StoreTokenError(_)
            | SubscribeError::SendEmailError(_)
            | SubscribeError::PoolError(_)
            | SubscribeError::InsertSubscriberError(_)
            | SubscribeError::TransactionCommitError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

pub struct StoreTokenError(sqlx::Error);

impl std::fmt::Display for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "A database error was encountered while trying to store subscription token."
        )
    }
}

impl std::fmt::Debug for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl std::error::Error for StoreTokenError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    writeln!(f, "{}\n", e)?;
    let mut current = e.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }

    Ok(())
}

#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(form,pool,email_client,base_url),
    fields(
        sibsciber_email = %form.email,
        sibsciber_name = %form.name,
    )
)]
pub async fn subscribe(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    base_url: web::Data<ApplicationBaseUrl>,
) -> Result<HttpResponse, SubscribeError> {
    let new_subscriber = form.0.try_into()?;
    let mut tx = pool.begin().await.map_err(SubscribeError::PoolError)?;
    let subscriber_id = insert_subscriber(&mut tx, &new_subscriber)
        .await
        .map_err(SubscribeError::InsertSubscriberError)?;
    let activation_token = generate_subscription_token();
    store_token(&mut tx, subscriber_id, &activation_token).await?;
    tx.commit()
        .await
        .map_err(SubscribeError::TransactionCommitError)?;
    send_confirmation_email(
        &email_client,
        new_subscriber,
        &base_url.0,
        &activation_token,
    )
    .await?;
    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(name = "Saving activation token", skip(tx, subscription_token))]
pub async fn store_token(
    tx: &mut Transaction<'_, Postgres>,
    subscriber_id: Uuid,
    subscription_token: &str,
) -> Result<(), StoreTokenError> {
    sqlx::query!(
        r#"INSERT INTO subscription_tokens (subscription_token, subscriber_id)
        VALUES ($1, $2)"#,
        subscription_token,
        subscriber_id,
    )
    .execute(tx)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        StoreTokenError(e)
    })?;
    Ok(())
}

#[tracing::instrument(
    name = "Send a confirmation email to a new subscriber",
    skip(email_client, new_subscriber, base_url, subscription_token)
)]
pub async fn send_confirmation_email(
    email_client: &EmailClient,
    new_subscriber: NewSubscriber,
    base_url: &str,
    subscription_token: &str,
) -> Result<(), reqwest::Error> {
    let confirmation_link = format!(
        "{}/subscriptions/confirm?subscription_token={}",
        base_url, subscription_token
    );
    let html_body = format!(
        "Please click the <a href=\"{}\">link</a> to confirm the subscription.",
        confirmation_link
    );
    let plain_body = format!(
        "[PLAIN] Please click the link to confirm your subscription: {}",
        confirmation_link
    );

    let subject = "Please confirm your subscription to my Newsletter";

    email_client
        .send_email(new_subscriber.email, subject, &html_body, &plain_body)
        .await
}

fn generate_subscription_token() -> String {
    let mut rng = thread_rng();
    std::iter::repeat_with(|| rng.sample(Alphanumeric))
        .map(char::from)
        .take(32)
        .collect()
}

#[tracing::instrument(
    name = "Saving new subscriber details in the database",
    skip(subscriber, tx)
)]
pub async fn insert_subscriber(
    tx: &mut Transaction<'_, Postgres>,
    subscriber: &NewSubscriber,
) -> Result<Uuid, sqlx::Error> {
    let subscriber_id = Uuid::new_v4();
    sqlx::query!(
        r#"
    INSERT INTO subscriptions (id,email,name,subscribed_at, status)
    VALUES ($1, $2, $3, $4, 'pending_confirmation')
    "#,
        subscriber_id,
        subscriber.email.as_ref(),
        subscriber.name.as_ref(),
        Utc::now()
    )
    .execute(tx)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query {:?}", e);
        e
    })?;
    Ok(subscriber_id)
}
