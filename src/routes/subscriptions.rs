use crate::domain::{NewSubscriber, SubscriberEmail, SubscriberName};
use crate::email_client::EmailClient;
use crate::startup::ApplicationBaseUrl;
use actix_web::{web, HttpResponse, ResponseError};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use sqlx::types::chrono::Utc;
use sqlx::{PgPool, Postgres, Transaction};
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

#[derive(Debug)]
pub struct StoreTokenError(sqlx::Error);

impl ResponseError for StoreTokenError {}

impl std::fmt::Display for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "A database error was encountered while trying to store subscription token."
        )
    }
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
) -> Result<HttpResponse, actix_web::Error> {
    let new_subscriber = match form.0.try_into() {
        Err(_) => return Ok(HttpResponse::BadRequest().finish()),
        Ok(sub) => sub,
    };

    let mut tx = match pool.begin().await {
        Err(_) => return Ok(HttpResponse::InternalServerError().finish()),
        Ok(t) => t,
    };

    let subscriber_id = match insert_subscriber(&mut tx, &new_subscriber).await {
        Err(_) => return Ok(HttpResponse::InternalServerError().finish()),
        Ok(sub) => sub,
    };

    let activation_token = generate_subscription_token();

    store_token(&mut tx, subscriber_id, &activation_token).await?;

    if tx.commit().await.is_err() {
        return Ok(HttpResponse::InternalServerError().finish());
    }

    if send_confirmation_email(
        &email_client,
        new_subscriber,
        &base_url.0,
        &activation_token,
    )
    .await
    .is_err()
    {
        return Ok(HttpResponse::InternalServerError().finish());
    }

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
